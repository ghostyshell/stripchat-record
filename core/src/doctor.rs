//! Environment checks shown in the app and by `stripchat-recorder doctor`.

use crate::config::Config;
use crate::mouflon::KeyRing;
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
pub struct Check {
    pub name: String,
    pub ok: bool,
    pub detail: String,
    /// What to do about it, when it is not ok.
    pub fix: String,
}

fn check(name: &str, ok: bool, detail: impl Into<String>, fix: impl Into<String>) -> Check {
    Check {
        name: name.into(),
        ok,
        detail: detail.into(),
        fix: fix.into(),
    }
}

pub fn run(config: &Config, ring: &KeyRing) -> Vec<Check> {
    let mut checks = Vec::new();

    let ffmpeg = crate::media::ffmpeg(&config.ffmpeg_path);
    checks.push(match &ffmpeg {
        Some(path) => check("ffmpeg", true, path.display().to_string(), ""),
        None => check(
            "ffmpeg",
            false,
            "not found",
            "Install ffmpeg, or set its path in Settings. Recordings still play without it.",
        ),
    });
    let ffprobe = crate::media::ffprobe(&config.ffprobe_path);
    checks.push(match &ffprobe {
        Some(path) => check("ffprobe", true, path.display().to_string(), ""),
        None => check(
            "ffprobe",
            false,
            "not found",
            "Ships with ffmpeg. Conversion progress needs it.",
        ),
    });

    let dir = config.resolved_output_dir();
    let writable = std::fs::create_dir_all(&dir).is_ok() && is_writable(&dir);
    checks.push(check(
        "output folder",
        writable,
        dir.display().to_string(),
        "Pick a folder you can write to in Settings.",
    ));

    checks.push(check(
        "encryption keys",
        !ring.is_empty(),
        format!("{} pkey/pdkey pair(s) loaded", ring.len()),
        "Add a pair in Settings, or set PD_KEY_<n>=<pkey>:<pdkey> in the environment.",
    ));

    let api = crate::site::Http::new(config).map(|h| h.api_reachable()).unwrap_or(false);
    checks.push(check(
        "Stripchat API",
        api,
        if config.proxy.trim().is_empty() {
            "direct".to_string()
        } else {
            format!("via {}", config.proxy)
        },
        "The site API is Cloudflare-gated and refuses some addresses; set a proxy in Settings.",
    ));

    checks
}

fn is_writable(dir: &Path) -> bool {
    let probe = dir.join(".scr-write-probe");
    match std::fs::write(&probe, b"ok") {
        Ok(()) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}
