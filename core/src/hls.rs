//! HLS playlist parsing and variant selection.
//!
//! Live playlists are re-fetched on a timer; only segments whose sequence
//! number has not been recorded yet are downloaded. Nothing here is
//! Stripchat specific, so the Chaturbate build reuses it unchanged.
//!
//! Segment encryption is not handled here: Stripchat obfuscates the *URIs*
//! (see `mouflon`) and the CDN serves plain fMP4 fragments, and Chaturbate has
//! no key layer at all.

use crate::error::{Error, Result};

#[derive(Debug, Clone)]
pub struct Segment {
    pub seq: u64,
    pub uri: String,
    pub duration: f64,
}

#[derive(Debug, Clone)]
pub struct MediaPlaylist {
    pub media_sequence: u64,
    pub target_duration: f64,
    pub end_list: bool,
    /// `#EXT-X-MAP` init segment (fMP4 streams), absolute URL.
    pub map: Option<String>,
    pub segments: Vec<Segment>,
}

#[derive(Debug, Clone)]
pub struct Variant {
    pub uri: String,
    pub bandwidth: u64,
    pub resolution: Option<(u32, u32)>,
    pub name: Option<String>,
}

/// Resolve `uri` against the playlist URL it was read from.
fn resolve(base: &str, uri: &str) -> String {
    if uri.starts_with("http://") || uri.starts_with("https://") {
        return uri.to_string();
    }
    let Some((scheme, rest)) = base.split_once("://") else {
        return uri.to_string();
    };
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    if uri.starts_with('/') {
        format!("{scheme}://{authority}{uri}")
    } else {
        let dir = match path.rfind('/') {
            Some(i) => &path[..=i],
            None => "/",
        };
        format!("{scheme}://{authority}{dir}{uri}")
    }
}

fn parse_attrs(line: &str) -> Vec<(String, String)> {
    let body = line.split_once(':').map(|(_, b)| b).unwrap_or(line);
    let mut out = Vec::new();
    let mut current = String::new();
    let mut in_quote = false;
    let mut parts = Vec::new();
    for ch in body.chars() {
        match ch {
            '"' => {
                in_quote = !in_quote;
            }
            ',' if !in_quote => parts.push(std::mem::take(&mut current)),
            _ => current.push(ch),
        }
    }
    parts.push(current);
    for part in parts {
        if let Some((k, v)) = part.split_once('=') {
            out.push((k.trim().to_string(), v.trim().trim_matches('"').to_string()));
        }
    }
    out
}

fn attr<'a>(attrs: &'a [(String, String)], key: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

/// Parse a media playlist. `base_url` is the URL it was fetched from.
pub fn parse_media_playlist(base_url: &str, text: &str) -> Result<MediaPlaylist> {
    if !text.contains("#EXTM3U") {
        return Err(Error::Other("not an m3u8 playlist".into()));
    }
    let mut media_sequence = 0u64;
    let mut target_duration = 0f64;
    let mut end_list = false;
    let mut map = None;
    let mut pending_duration = 0f64;
    let mut segments = Vec::new();

    for line in text.lines().map(str::trim) {
        if let Some(rest) = line.strip_prefix("#EXT-X-MEDIA-SEQUENCE:") {
            media_sequence = rest.trim().parse().unwrap_or(0);
        } else if let Some(rest) = line.strip_prefix("#EXT-X-TARGETDURATION:") {
            target_duration = rest.trim().parse().unwrap_or(0.0);
        } else if line == "#EXT-X-ENDLIST" {
            end_list = true;
        } else if line.starts_with("#EXT-X-MAP:") {
            map = attr(&parse_attrs(line), "URI").map(|u| resolve(base_url, u));
        } else if let Some(rest) = line.strip_prefix("#EXTINF:") {
            pending_duration = rest
                .split(',')
                .next()
                .unwrap_or("0")
                .trim()
                .parse()
                .unwrap_or(0.0);
        } else if !line.is_empty() && !line.starts_with('#') {
            let seq = media_sequence + segments.len() as u64;
            segments.push(Segment {
                seq,
                uri: resolve(base_url, line),
                duration: pending_duration,
            });
            pending_duration = 0.0;
        }
    }

    Ok(MediaPlaylist {
        media_sequence,
        target_duration,
        end_list,
        map,
        segments,
    })
}

/// Parse a master playlist. Returns an empty list for a media playlist.
pub fn parse_master_playlist(base_url: &str, text: &str) -> Vec<Variant> {
    let mut variants = Vec::new();
    let mut pending: Vec<(String, String)> = Vec::new();
    for line in text.lines().map(str::trim) {
        if line.starts_with("#EXT-X-STREAM-INF:") {
            pending = parse_attrs(line);
        } else if !line.is_empty() && !line.starts_with('#') && !pending.is_empty() {
            let bandwidth = attr(&pending, "BANDWIDTH")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            let resolution = attr(&pending, "RESOLUTION").and_then(|v| {
                let (w, h) = v.split_once('x')?;
                Some((w.parse().ok()?, h.parse().ok()?))
            });
            variants.push(Variant {
                uri: resolve(base_url, line),
                bandwidth,
                resolution,
                name: attr(&pending, "NAME").map(str::to_string),
            });
            pending.clear();
        }
    }
    variants
}

/// Pick the highest-resolution, highest-bandwidth variant (source quality).
pub fn best_variant(variants: &[Variant]) -> Option<&Variant> {
    variants
        .iter()
        .max_by_key(|v| (v.resolution.map(|(w, h)| w * h).unwrap_or(0), v.bandwidth))
        .or_else(|| variants.first())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_media_playlist() {
        let text = "#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:4\n\
#EXT-X-MEDIA-SEQUENCE:120\n\
#EXT-X-MAP:URI=\"init.mp4\"\n\
#EXTINF:3.9,\nseg120.ts\n#EXTINF:4.0,\nseg121.ts\n#EXT-X-ENDLIST\n";
        let pl = parse_media_playlist("https://cdn.example/a/b/index.m3u8", text).unwrap();
        assert_eq!(pl.media_sequence, 120);
        assert_eq!(pl.target_duration, 4.0);
        assert!(pl.end_list);
        assert_eq!(pl.map.as_deref(), Some("https://cdn.example/a/b/init.mp4"));
        assert_eq!(pl.segments.len(), 2);
        assert_eq!(pl.segments[0].seq, 120);
        assert_eq!(pl.segments[1].uri, "https://cdn.example/a/b/seg121.ts");
    }

    #[test]
    fn parses_master_and_chooses_source() {
        let text = "#EXTM3U\n\
#EXT-X-STREAM-INF:BANDWIDTH=800000,RESOLUTION=640x360\nlow.m3u8\n\
#EXT-X-STREAM-INF:BANDWIDTH=3000000,RESOLUTION=1280x720,NAME=\"source\"\nhigh.m3u8\n";
        let variants = parse_master_playlist("https://cdn.example/x/master.m3u8", text);
        assert_eq!(variants.len(), 2);
        let best = best_variant(&variants).unwrap();
        assert_eq!(best.uri, "https://cdn.example/x/high.m3u8");
        assert_eq!(best.name.as_deref(), Some("source"));
    }

    #[test]
    fn rejects_non_playlists() {
        assert!(parse_media_playlist("https://x/", "<html>nope</html>").is_err());
    }
}

#[cfg(test)]
mod real_master {
    use super::*;
    #[test]
    fn parses_a_real_stripchat_master() {
        let text = "#EXTM3U\n#EXT-X-VERSION:6\n#EXT-X-MOUFLON:PSCH:v2:Ook7quaiNgiyuhai\n#EXT-X-STREAM-INF:BANDWIDTH=2989465,CODECS=\"avc1.4d0029,mp4a.40.2\",RESOLUTION=1280x720,FRAME-RATE=29.000,CLOSED-CAPTIONS=NONE,NAME=\"source\"\nhttps://media-hls.doppiocdn.com/b-hls-20/288906106/288906106.m3u8?playlistType=standard&psch=v2&pkey=Ook7quaiNgiyuhai&preferredVideoCodec=h264\n#EXT-X-STREAM-INF:BANDWIDTH=693862,CODECS=\"avc1.4d0015,mp4a.40.2\",RESOLUTION=426x240,FRAME-RATE=26.000,CLOSED-CAPTIONS=NONE,NAME=\"240p\"\nhttps://media-hls.doppiocdn.com/b-hls-20/288906106/288906106_240p.m3u8?playlistType=standard&psch=v2&pkey=Ook7quaiNgiyuhai&preferredVideoCodec=h264\n";
        let variants = parse_master_playlist("https://edge-hls.doppiocdn.com/hls/1/master/1_auto.m3u8", text);
        assert_eq!(variants.len(), 2, "{variants:?}");
    }
}
