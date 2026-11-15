//! The recording library: what is on disk in the output folder.

use crate::error::Result;
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
pub struct Recording {
    pub name: String,
    pub path: String,
    pub size_bytes: u64,
    /// Unix seconds.
    pub modified: u64,
    /// Still being written by a running job.
    pub in_progress: bool,
}

pub fn list(dir: &Path) -> Result<Vec<Recording>> {
    let mut out = Vec::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
        Err(e) => return Err(e.into()),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let lower = name.to_ascii_lowercase();
        if !(lower.ends_with(".mp4") || lower.ends_with(".mkv")) {
            continue;
        }
        let meta = match entry.metadata() {
            Ok(meta) => meta,
            Err(_) => continue,
        };
        let modified = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        out.push(Recording {
            in_progress: lower.contains(".recording."),
            name,
            path: path.to_string_lossy().into_owned(),
            size_bytes: meta.len(),
            modified,
        });
    }
    out.sort_by(|a, b| b.modified.cmp(&a.modified));
    Ok(out)
}

/// Delete a recording, but only a media file inside `dir`.
///
/// The path arrives from the webview, so it is treated as untrusted: it must
/// resolve inside the recordings folder and be a video file.
pub fn delete_within(dir: &Path, path: &Path) -> Result<()> {
    let root = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
    let target = path.canonicalize()?;
    if !target.starts_with(&root) {
        return Err(crate::error::Error::Other(
            "refusing to delete a file outside the recordings folder".into(),
        ));
    }
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    if !(name.ends_with(".mp4") || name.ends_with(".mkv")) {
        return Err(crate::error::Error::Other(
            "refusing to delete a file that is not a recording".into(),
        ));
    }
    std::fs::remove_file(&target)?;
    Ok(())
}

pub fn reveal(path: &Path) -> Result<()> {
    let target = path.parent().unwrap_or(path);
    let cmd: (&str, Vec<&str>) = if cfg!(target_os = "macos") {
        ("open", vec!["-R", path.to_str().unwrap_or_default()])
    } else if cfg!(windows) {
        ("explorer", vec![path.to_str().unwrap_or_default()])
    } else {
        ("xdg-open", vec![target.to_str().unwrap_or_default()])
    };
    std::process::Command::new(cmd.0)
        .args(cmd.1)
        .spawn()
        .map(|_| ())
        .map_err(crate::error::Error::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn lists_only_media_and_flags_partials() {
        let dir = std::env::temp_dir().join(format!("scr-lib-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.recording.mp4"), b"x").unwrap();
        std::fs::write(dir.join("b.mp4"), b"xy").unwrap();
        std::fs::write(dir.join("c.txt"), b"skip").unwrap();

        let rows = list(&dir).unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().any(|r| r.name == "a.recording.mp4" && r.in_progress));
        assert!(rows.iter().any(|r| r.name == "b.mp4" && !r.in_progress));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_directory_is_empty_not_an_error() {
        let dir = PathBuf::from("/nope/definitely/not/here");
        assert!(list(&dir).unwrap().is_empty());
    }
}
