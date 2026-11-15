//! The recording loop: poll the live playlist, download each new segment
//! exactly once, and append it to the part file.
//!
//! Durability rules, which exist because a live stream ends without warning:
//! - a segment is downloaded into memory and only appended once it is whole,
//!   so a dropped connection can never leave half a fragment in the file;
//! - fMP4 output starts with the `#EXT-X-MAP` init and every later fragment is
//!   appended after it, so even a hard kill leaves a playable file;
//! - stopping for any reason (end of stream, duration, cancel, an unrecoverable
//!   fetch error) closes the file cleanly and returns what was saved.

use crate::config::Config;
use crate::error::{Error, Result};
use crate::hls;
use crate::jobs::JobContext;
use crate::mouflon::{self, KeyRing};
use crate::site;
use crate::util;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndReason {
    StreamEnded,
    Duration,
    Cancelled,
}

impl EndReason {
    pub fn as_str(self) -> &'static str {
        match self {
            EndReason::StreamEnded => "stream ended",
            EndReason::Duration => "duration reached",
            EndReason::Cancelled => "stopped",
        }
    }
}

#[derive(Debug, Clone)]
pub struct RecordOptions {
    pub model: String,
    /// `source` (default), `auto`, or an exact variant name such as `1080p`.
    pub quality: String,
    pub duration_secs: Option<u64>,
    /// Exact output path; overrides the output directory.
    pub output: Option<PathBuf>,
    pub config: Config,
}

impl RecordOptions {
    pub fn for_model(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            quality: "source".into(),
            duration_secs: None,
            output: None,
            config: Config::load(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RecordReport {
    pub paths: Vec<PathBuf>,
    pub bytes: u64,
    pub segments: u64,
    pub quality: String,
    pub resolution: String,
    pub end_reason: EndReason,
    pub seconds: u64,
    /// Set when recording stopped on an error after some bytes were saved.
    pub note: Option<String>,
}

/// Append-only part file with size-based rotation.
struct PartWriter {
    dir: PathBuf,
    /// Stem as asked for; `unique` disambiguates a collision.
    stem: String,
    unique: u32,
    split: Option<u64>,
    part: u32,
    file: Option<File>,
    part_bytes: u64,
    total_bytes: u64,
    paths: Vec<PathBuf>,
    /// The `#EXT-X-MAP` URI currently written into the open part.
    current_map: Option<String>,
    init_cache: Option<(String, Vec<u8>)>,
}

impl PartWriter {
    fn new(dir: PathBuf, stem: String, split: Option<u64>) -> Result<Self> {
        std::fs::create_dir_all(&dir)?;
        let mut writer = Self {
            dir,
            stem,
            unique: 0,
            split,
            part: 0,
            file: None,
            part_bytes: 0,
            total_bytes: 0,
            paths: Vec::new(),
            current_map: None,
            init_cache: None,
        };
        writer.open_part()?;
        Ok(writer)
    }

    fn path_for(&self, part: u32) -> PathBuf {
        let stem = if self.unique == 0 {
            self.stem.clone()
        } else {
            format!("{}-{}", self.stem, self.unique)
        };
        if self.split.is_some() {
            self.dir
                .join(format!("{stem}-part{part:03}.recording.mp4"))
        } else {
            self.dir.join(format!("{stem}.recording.mp4"))
        }
    }

    fn open_part(&mut self) -> Result<()> {
        self.part += 1;
        // `create_new` refuses an existing file: two recordings started in the
        // same second must never append into one another's parts.
        for _ in 0..64 {
            let path = self.path_for(self.part);
            match OpenOptions::new().create_new(true).write(true).open(&path) {
                Ok(file) => {
                    self.part_bytes = 0;
                    self.file = Some(file);
                    self.paths.push(path);
                    self.current_map = None;
                    return Ok(());
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    self.unique += 1;
                }
                Err(e) => return Err(e.into()),
            }
        }
        Err(Error::Other("could not find a free file name".into()))
    }

    fn write(&mut self, data: &[u8]) -> Result<()> {
        if let Some(file) = self.file.as_mut() {
            file.write_all(data)?;
            file.flush()?;
            self.part_bytes += data.len() as u64;
            self.total_bytes += data.len() as u64;
        }
        Ok(())
    }

    /// Write the init segment when it appears or changes (a stream restart).
    fn ensure_init(&mut self, map: Option<&str>, bytes: &[u8]) -> Result<()> {
        let Some(uri) = map else { return Ok(()) };
        if self.current_map.as_deref() == Some(uri) {
            return Ok(());
        }
        self.write(bytes)?;
        self.current_map = Some(uri.to_string());
        Ok(())
    }

    fn rotate_if_needed(&mut self) -> Result<bool> {
        let Some(limit) = self.split else {
            return Ok(false);
        };
        if self.part_bytes < limit {
            return Ok(false);
        }
        self.file = None;
        self.open_part()?;
        Ok(true)
    }

    /// Close the file and drop any part that never received a byte, so a
    /// rotation at the very end does not leave an empty `.recording.mp4`.
    fn finish(&mut self) {
        self.file = None;
        self.paths.retain(|path| {
            let keep = path.metadata().map(|m| m.len() > 0).unwrap_or(false);
            if !keep {
                let _ = std::fs::remove_file(path);
            }
            keep
        });
    }
}

fn download(http: &site::Http, ctx: &JobContext, url: &str, retries: u32) -> Result<Vec<u8>> {
    if !site::is_allowed_media_host(url) {
        return Err(Error::Other(format!(
            "refusing to fetch a non-CDN URL: {}",
            &url[..url.len().min(80)]
        )));
    }
    let mut last = None;
    for attempt in 0..=retries {
        ctx.check_cancel()?;
        match http.media_bytes(url) {
            Ok(buf) if !buf.is_empty() => return Ok(buf),
            Ok(_) => last = Some(Error::Http("empty segment".into())),
            Err(e) => last = Some(e),
        }
        if attempt < retries {
            std::thread::sleep(Duration::from_millis(400 * (attempt as u64 + 1)));
        }
    }
    Err(last.unwrap_or_else(|| Error::Http("segment download failed".into())))
}

fn init_bytes(http: &site::Http, ctx: &JobContext, writer: &mut PartWriter, map: &str) -> Result<Vec<u8>> {
    if let Some((uri, bytes)) = &writer.init_cache {
        if uri == map {
            return Ok(bytes.clone());
        }
    }
    let bytes = download(http, ctx, map, 4)?;
    writer.init_cache = Some((map.to_string(), bytes.clone()));
    Ok(bytes)
}

/// What the site API says right now. `Unknown` is not `Offline`: an unreachable
/// API must never end a recording.
enum Liveness {
    Live,
    Offline,
    Unknown,
}

fn liveness(http: &site::Http, model: &str) -> Liveness {
    match site::fetch_cam(http, model) {
        Ok(cam) if cam.is_public_live() => Liveness::Live,
        Ok(_) => Liveness::Offline,
        Err(_) => Liveness::Unknown,
    }
}

/// Record until the stream ends, the duration elapses, or the job is cancelled.
pub fn record(
    opts: &RecordOptions,
    ring: &KeyRing,
    ctx: &JobContext,
) -> Result<RecordReport> {
    let config = &opts.config;
    let started = Instant::now();

    let http = site::Http::new(config)?;
    let (cam, mut pkey, variants) = site::resolve(&http, &opts.model, ring)?;
    let mut variant = site::pick_variant(&variants, &opts.quality)
        .ok_or_else(|| Error::Other("no playable variant".into()))?
        .clone();
    let quality = variant
        .name
        .clone()
        .unwrap_or_else(|| "auto".to_string());
    let resolution = variant
        .resolution
        .map(|(w, h)| format!("{w}x{h}"))
        .unwrap_or_else(|| "source".to_string());
    let mut pdkey = ring
        .get(&pkey)
        .cloned()
        .ok_or_else(|| Error::Other(format!("no pdkey for pkey {pkey}")))?;

    ctx.message(format!(
        "{} at {} ({}), keyed",
        opts.model, resolution, quality
    ));

    let dir = match &opts.output {
        Some(path) => path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from(".")),
        None => config.resolved_output_dir(),
    };
    let stem = match &opts.output {
        Some(path) => path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| format!("{}-{}", util::slug(&opts.model), util::timestamp())),
        None => format!("{}-{}", util::slug(&opts.model), util::timestamp()),
    };
    let mut writer = PartWriter::new(dir, stem, config.split_size())?;
    let mut next_seq: Option<u64> = None;
    let mut segments: u64 = 0;
    let mut end_reason = EndReason::StreamEnded;
    let mut consecutive_failures = 0u32;

    let result: Result<()> = (|| {
        loop {
            ctx.check_cancel().map_err(|_| Error::Cancelled(ctx.job_id()))?;
            if let Some(limit) = opts.duration_secs {
                if started.elapsed() >= Duration::from_secs(limit) {
                    end_reason = EndReason::Duration;
                    return Ok(());
                }
            }

            let (playlist, stale) = match site::fetch_variant_playlist(&http, &variant.uri, &pkey) {
                Ok(pair) => pair,
                Err(e) => {
                    consecutive_failures += 1;
                    ctx.check_cancel().map_err(|_| Error::Cancelled(ctx.job_id()))?;
                    // Only a definite "the room is not live" ends the recording.
                    // An unreachable API (403, timeout, proxy down) must not.
                    if matches!(liveness(&http, &opts.model), Liveness::Offline) {
                        end_reason = EndReason::StreamEnded;
                        return Ok(());
                    }
                    if consecutive_failures > 8 {
                        return Err(e);
                    }
                    std::thread::sleep(Duration::from_secs(2));
                    continue;
                }
            };
            consecutive_failures = 0;

            if stale {
                // The pkey rotated out: re-read the master, pick a new key, and
                // re-parse the variants so the variant URL carries the new key.
                let bare = site::fetch_master(&http, cam.stream_name()?)?;
                let refreshed = mouflon::pick_pkey(&bare, ring)
                    .ok_or_else(|| Error::Other("pkey rotated and no replacement known".into()))?;
                if refreshed == pkey {
                    return Err(Error::Other("the CDN rejects the pkey we hold".into()));
                }
                let (master, still_stale) =
                    site::fetch_master_with_pkey(&http, cam.stream_name()?, &refreshed)?;
                if still_stale {
                    return Err(Error::Other(
                        "the CDN served the advert playlist for the new pkey".into(),
                    ));
                }
                let variants = hls::parse_master_playlist(
                    &site::master_url(cam.stream_name()?),
                    &master,
                );
                let next = site::pick_variant(&variants, &opts.quality)
                    .ok_or_else(|| Error::Other("no playable variant after the key rotated".into()))?;
                variant = next.clone();
                pkey = refreshed;
                pdkey = ring
                    .get(&pkey)
                    .cloned()
                    .ok_or_else(|| Error::Other(format!("no pdkey for pkey {pkey}")))?;
                next_seq = None;
                continue;
            }

            let plain = mouflon::decode_playlist(&playlist, &pdkey)?;
            let media = hls::parse_media_playlist(&variant.uri, &plain)?;
            if media.segments.is_empty() {
                std::thread::sleep(Duration::from_millis(800));
                continue;
            }

            let last = media.segments[media.segments.len() - 1].seq;
            let start = match next_seq {
                Some(next) if next > last => {
                    // The CDN restarted its numbering; resume at this window
                    // instead of filtering everything out forever.
                    tracing::info!("media sequence went backwards, resuming");
                    media.segments[0].seq
                }
                Some(next) => next,
                None => media.segments[0].seq,
            };
            next_seq = Some(start);
            for segment in media.segments.iter().filter(|s| s.seq >= start) {
                ctx.check_cancel().map_err(|_| Error::Cancelled(ctx.job_id()))?;
                let data = match download(&http, ctx, &segment.uri, config.segment_retries) {
                    Ok(data) => data,
                    Err(Error::Cancelled(id)) => return Err(Error::Cancelled(id)),
                    Err(e) => {
                        // A vanished segment usually means the stream ended and
                        // the window slid; an unreachable API does not.
                        if matches!(liveness(&http, &opts.model), Liveness::Offline) {
                            end_reason = EndReason::StreamEnded;
                            return Ok(());
                        }
                        return Err(e);
                    }
                };
                if let Some(map) = media.map.as_deref() {
                    let init = init_bytes(&http, ctx, &mut writer, map)?;
                    writer.ensure_init(Some(map), &init)?;
                }
                writer.write(&data)?;
                next_seq = Some(segment.seq + 1);
                segments += 1;
                ctx.progress(writer.total_bytes, 0);
                if writer.rotate_if_needed()? {
                    ctx.message(format!(
                        "part {} started ({})",
                        writer.part,
                        util::format_bytes(writer.total_bytes)
                    ));
                }
            }

            if media.end_list
                && next_seq.unwrap_or(0) >= media.media_sequence + media.segments.len() as u64
            {
                end_reason = EndReason::StreamEnded;
                return Ok(());
            }
            let wait_ms = ((media.target_duration.max(1.0) * 1000.0) as u64 / 2).clamp(500, 2000);
            std::thread::sleep(Duration::from_millis(wait_ms));
        }
    })();

    writer.finish();
    let mut note = None;
    match result {
        Ok(()) => {}
        Err(Error::Cancelled(_)) => end_reason = EndReason::Cancelled,
        Err(e) => {
            // Keep whatever was recorded; report the failure with the files.
            if writer.total_bytes == 0 {
                for path in &writer.paths {
                    let _ = std::fs::remove_file(path);
                }
                return Err(e);
            }
            tracing::warn!("recording stopped with an error after {} bytes: {e}", writer.total_bytes);
            note = Some(e.to_string());
        }
    }

    let paths: Vec<PathBuf> = writer
        .paths
        .iter()
        .filter(|p| p.metadata().map(|m| m.len() > 0).unwrap_or(false))
        .cloned()
        .collect();

    Ok(RecordReport {
        paths,
        bytes: writer.total_bytes,
        segments,
        quality,
        resolution,
        end_reason,
        seconds: started.elapsed().as_secs(),
        note,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::JobManager;

    struct TempDir(PathBuf);
    impl TempDir {
        fn new(tag: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "scr-test-{}-{}",
                tag,
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn writer_appends_and_rotates_at_segment_boundaries() {
        let dir = TempDir::new("writer");
        let mut writer = PartWriter::new(dir.0.clone(), "stream".into(), Some(10)).unwrap();
        writer.ensure_init(Some("https://cdn/init.mp4"), b"INIT").unwrap();
        writer.write(b"AAAA").unwrap();
        assert!(!writer.rotate_if_needed().unwrap());
        writer.write(b"BBBB").unwrap();
        assert!(writer.rotate_if_needed().unwrap());
        // New part starts with the init again, so it plays alone.
        writer.ensure_init(Some("https://cdn/init.mp4"), b"INIT").unwrap();
        writer.write(b"CCCC").unwrap();
        writer.finish();

        let first = std::fs::read(dir.0.join("stream-part001.recording.mp4")).unwrap();
        let second = std::fs::read(dir.0.join("stream-part002.recording.mp4")).unwrap();
        assert_eq!(first, b"INITAAAABBBB");
        assert_eq!(second, b"INITCCCC");
        assert_eq!(writer.total_bytes, 20);
    }

    #[test]
    fn a_failed_job_keeps_what_was_written() {
        let dir = TempDir::new("keep");
        let mgr = JobManager::new();
        let path = dir.0.clone();
        let id = mgr.spawn("record", "test", |_| {}, move |ctx| {
            let mut writer = PartWriter::new(path.clone(), "x".into(), None).unwrap();
            writer.write(b"already here").unwrap();
            writer.finish();
            ctx.message(path.join("x.mp4").to_string_lossy());
            Err(Error::Other("network gone".into()))
        });
        for _ in 0..200 {
            if mgr.get(id).map(|j| j.state != crate::jobs::JobState::Running).unwrap_or(false) {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(std::fs::read(dir.0.join("x.recording.mp4")).unwrap(), b"already here");
    }
}
