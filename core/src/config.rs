//! Persisted settings and the data directory.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Where recordings are written. Empty means the platform video folder.
    pub output_dir: String,
    /// Rotate to a new file at this size in bytes; 0 disables splitting.
    pub split_size_bytes: u64,
    /// Remux the finished `.ts` to `.mp4` (stream copy, lossless).
    pub remux_mp4: bool,
    /// After remux, also write a compressed copy using `convert_preset`.
    pub convert_on_finish: bool,
    /// Preset id from `media::PRESETS`.
    pub convert_preset: String,
    /// Keep the intermediate `.ts` next to the result.
    pub keep_ts: bool,
    /// Retries for a failed segment or playlist fetch before giving up.
    pub segment_retries: u32,
    /// Seconds between live playlist refreshes when nothing new is listed.
    pub poll_interval_secs: u64,
    pub ffmpeg_path: String,
    pub ffprobe_path: String,
    /// Models tracked on the Record page.
    pub models: Vec<String>,
    /// Extra MOUFLON pairs (`pkey -> pdkey`) saved from Settings.
    pub pdkeys: std::collections::BTreeMap<String, String>,
    /// Optional proxy for the site API, e.g. `socks5h://127.0.0.1:9050`.
    /// The CDN legs stay direct, like the addon's production setup.
    pub proxy: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            output_dir: String::new(),
            split_size_bytes: 1024 * 1024 * 1024,
            remux_mp4: true,
            convert_on_finish: false,
            convert_preset: "h264_balanced".into(),
            keep_ts: false,
            segment_retries: 3,
            poll_interval_secs: 2,
            ffmpeg_path: String::new(),
            ffprobe_path: String::new(),
            models: Vec::new(),
            pdkeys: Default::default(),
            proxy: String::new(),
        }
    }
}

impl Config {
    pub fn load() -> Self {
        let Ok(bytes) = std::fs::read(config_path()) else {
            return Self::default();
        };
        serde_json::from_slice(&bytes).unwrap_or_default()
    }

    pub fn save(&self) -> std::io::Result<()> {
        if let Some(parent) = config_path().parent() {
            std::fs::create_dir_all(parent)?;
        }
        // Write then rename: a kill mid-save must not truncate the settings.
        let tmp = config_path().with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(&tmp, config_path())?;
        Ok(())
    }

    /// Resolved output directory: the configured one, else the video folder.
    pub fn resolved_output_dir(&self) -> PathBuf {
        let raw = self.output_dir.trim();
        if !raw.is_empty() {
            return PathBuf::from(shellexpand(raw));
        }
        default_output_dir()
    }

    pub fn split_size(&self) -> Option<u64> {
        (self.split_size_bytes > 0).then_some(self.split_size_bytes)
    }
}

fn shellexpand(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = directories::UserDirs::new() {
            return home.home_dir().join(rest).to_string_lossy().into_owned();
        }
    }
    path.to_string()
}

pub fn app_data_dir() -> PathBuf {
    if let Some(dirs) = directories::ProjectDirs::from("click", "ghostyshell", "Stripchat Recorder") {
        return dirs.data_dir().to_path_buf();
    }
    PathBuf::from(".stripchat-recorder")
}

pub fn config_path() -> PathBuf {
    app_data_dir().join("config.json")
}

pub fn default_output_dir() -> PathBuf {
    directories::UserDirs::new()
        .and_then(|d| d.video_dir().map(|v| v.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Stripchat Recorder")
}

pub fn parse_size(raw: &str) -> Option<u64> {
    let value = raw.trim().to_lowercase();
    if value.is_empty() {
        return None;
    }
    let (num, mult) = if let Some(v) = value.strip_suffix("gib").or_else(|| value.strip_suffix('g')) {
        (v, 1024 * 1024 * 1024)
    } else if let Some(v) = value.strip_suffix("mib").or_else(|| value.strip_suffix('m')) {
        (v, 1024 * 1024)
    } else if let Some(v) = value.strip_suffix("kib").or_else(|| value.strip_suffix('k')) {
        (v, 1024)
    } else {
        (value.as_str(), 1)
    };
    let amount: f64 = num.trim().parse().ok()?;
    if amount <= 0.0 {
        return None;
    }
    Some((amount * mult as f64) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_parser() {
        assert_eq!(parse_size("1G"), Some(1024 * 1024 * 1024));
        assert_eq!(parse_size("512m"), Some(512 * 1024 * 1024));
        assert_eq!(parse_size("0"), None);
        assert_eq!(parse_size("nope"), None);
    }

    #[test]
    fn defaults_survive_a_missing_config() {
        let config = Config::default();
        assert!(config.split_size().is_some());
        assert!(config.output_dir.is_empty());
    }
}
