//! Stripchat "MOUFLON v2" segment-URI obfuscation.
//!
//! Segment URIs are not encrypted media: the *path token* is obfuscated so the
//! player cannot be pointed at the CDN directly. The scheme is
//! `reverse(token) -> base64 decode -> XOR with SHA-256(pdkey) (cyclic)`, then
//! the plaintext token is substituted back into the URI.
//!
//! `pkey` comes from the master playlist's `#EXT-X-MOUFLON:PSCH:v2:` tags;
//! `pdkey` is the matching secret from the key registry. Pairs are not part of
//! the protocol, they rotate, and they come from community reverse engineering,
//! so they are data (config/env), never constants in code.

use crate::error::{Error, Result};
use base64::Engine;
use sha2::{Digest, Sha256};

/// `pkey -> pdkey`, the full registry.
pub type KeyRing = std::collections::BTreeMap<String, String>;

/// Parse `PD_KEY_<n>=<pkey>:<pdkey>` environment variables into a ring.
pub fn keys_from_env() -> KeyRing {
    let mut ring = KeyRing::new();
    for (name, value) in std::env::vars() {
        if !name.starts_with("PD_KEY_") {
            continue;
        }
        if let Some((pkey, pdkey)) = value.split_once(':') {
            let (pkey, pdkey) = (pkey.trim(), pdkey.trim());
            if !pkey.is_empty() && !pdkey.is_empty() {
                ring.insert(pkey.to_string(), pdkey.to_string());
            }
        }
    }
    ring
}

/// The pkeys advertised by a master or media playlist.
pub fn psch_keys(playlist: &str) -> Vec<String> {
    const TAG: &str = "#EXT-X-MOUFLON:PSCH:v2:";
    playlist
        .lines()
        .filter_map(|line| line.trim().strip_prefix(TAG))
        .map(|key| key.trim().to_string())
        .filter(|key| {
            !key.is_empty()
                && key
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric())
        })
        .collect()
}

/// Pick the first advertised pkey we hold a pdkey for, else the first one.
pub fn pick_pkey(playlist: &str, ring: &KeyRing) -> Option<String> {
    let found = psch_keys(playlist);
    found
        .iter()
        .find(|key| ring.contains_key(*key))
        .or_else(|| found.first())
        .cloned()
}

/// The CDN serves an advert playlist instead of the stream when the pkey has
/// rotated out; the caller must re-read the master.
pub fn is_stale(playlist: &str) -> bool {
    playlist.contains("#EXT-X-MOUFLON-ADVERT")
}

/// Split a media URI into (prefix, token, suffix) using the addon's rule:
/// the token sits between the first `_<digits>_` and the next `_<digits>`.
fn split_token(uri: &str) -> Option<(&str, &str, &str)> {
    let bytes = uri.as_bytes();
    let mut start = None;
    let mut i = 0;
    // Find `_<digits>_`
    while i < bytes.len() {
        if bytes[i] == b'_' {
            let mut j = i + 1;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                j += 1;
            }
            if j > i + 1 && j < bytes.len() && bytes[j] == b'_' {
                start = Some(j + 1);
                break;
            }
        }
        i += 1;
    }
    let start = start?;
    // Token runs until `_<digits>` (the extension follows that group).
    let rest = &uri[start..];
    let rest_bytes = rest.as_bytes();
    let mut k = 0;
    while k < rest_bytes.len() {
        if rest_bytes[k] == b'_' {
            let mut j = k + 1;
            while j < rest_bytes.len() && rest_bytes[j].is_ascii_digit() {
                j += 1;
            }
            if j > k + 1 {
                return Some((&uri[..start], &rest[..k], &rest[k..]));
            }
        }
        k += 1;
    }
    None
}

fn xor_sha256(token: &str, pdkey: &str) -> Result<String> {
    let reversed: String = token.chars().rev().collect();
    let padded = match reversed.len() % 4 {
        2 => format!("{reversed}=="),
        3 => format!("{reversed}="),
        _ => reversed,
    };
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(padded.as_bytes())
        .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(padded.as_bytes()))
        .map_err(|e| Error::Other(format!("MOUFLON token is not base64: {e}")))?;
    let digest = Sha256::digest(pdkey.as_bytes());
    let plain: Vec<u8> = decoded
        .iter()
        .enumerate()
        .map(|(i, b)| b ^ digest[i % 32])
        .collect();
    if plain.is_empty() || !plain.iter().all(|b| (0x20..=0x7e).contains(b)) {
        return Err(Error::Other(
            "MOUFLON token did not decrypt to printable ASCII (wrong pdkey?)".into(),
        ));
    }
    String::from_utf8(plain).map_err(|e| Error::Other(e.to_string()))
}

/// Decrypt one `#EXT-X-MOUFLON:URI:` value. Returns the URI unchanged when it
/// carries no token.
pub fn decrypt_uri(uri: &str, pdkey: &str) -> Result<String> {
    let Some((prefix, token, suffix)) = split_token(uri) else {
        return Ok(uri.to_string());
    };
    Ok(format!("{prefix}{}{suffix}", xor_sha256(token, pdkey)?))
}

/// Rewrite a MOUFLON playlist into plain HLS that `hls::parse_media_playlist`
/// understands: decrypted URIs replace the placeholder line that follows each
/// `#EXT-X-MOUFLON:URI:` tag, PSCH tags are dropped, and LL-HLS partial tags
/// are removed (we record whole segments, not parts).
pub fn decode_playlist(playlist: &str, pdkey: &str) -> Result<String> {
    let mut out: Vec<String> = Vec::new();
    let mut pending: Option<String> = None;

    for line in playlist.lines() {
        let trimmed = line.trim_end();
        if let Some(raw) = trimmed.strip_prefix("#EXT-X-MOUFLON:URI:") {
            pending = Some(decrypt_uri(raw.trim(), pdkey)?);
            continue;
        }
        if let Some(uri) = pending.take() {
            if !trimmed.is_empty() && !trimmed.starts_with('#') {
                out.push(uri);
                continue;
            }
            // No placeholder line followed: keep the tag and the URI together
            // so a malformed playlist fails loudly instead of silently.
            pending = Some(uri);
        }
        out.push(trimmed.to_string());
    }
    if let Some(uri) = pending {
        out.push(uri);
    }

    let stripped = out
        .into_iter()
        .filter(|line| {
            let t = line.trim_start();
            !(t.starts_with("#EXT-X-MOUFLON:PSCH:")
                || t.starts_with("#EXT-X-PART:")
                || t.starts_with("#EXT-X-PART-INF:")
                || t.starts_with("#EXT-X-PRELOAD-HINT:")
                || t.starts_with("#EXT-X-SERVER-CONTROL:")
                || t.starts_with("#EXT-X-RENDITION-REPORT:"))
        })
        .map(|line| {
            if line.starts_with("#EXTINF:") && !line.ends_with(',') {
                format!("{line},")
            } else {
                line
            }
        })
        .collect::<Vec<_>>();

    Ok(stripped.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Conformance vector from the addon's test suite.
    const ENCRYPTED: &str = "https://media-hls.doppiocdn.com/b-hls-16/94190182/94190182_1536_gjJHf3ZVwpfKPh9k7v97V2_1782130340.mp4";
    const DECRYPTED: &str = "https://media-hls.doppiocdn.com/b-hls-16/94190182/94190182_1536_wOwbVLqyDRL5NYgC_1782130340.mp4";
    const PDKEY: &str = "EQueeGh2kaewa3ch";

    #[test]
    fn conformance_vector() {
        assert_eq!(decrypt_uri(ENCRYPTED, PDKEY).unwrap(), DECRYPTED);
    }

    #[test]
    fn token_with_a_slash_decrypts() {
        let uri = "https://media-hls.doppiocdn.com/b/1/1_1025_Avcmf/oAzoIqNsFVZ6GfE/_1794734621.mp4";
        let out = decrypt_uri(uri, PDKEY).unwrap();
        assert!(out.starts_with("https://media-hls.doppiocdn.com/b/1/1_1025_"));
        assert!(out.ends_with("_1794734621.mp4"));
    }

    #[test]
    fn wrong_key_is_an_error() {
        assert!(decrypt_uri(ENCRYPTED, "nope").is_err());
    }

    #[test]
    fn playlist_decoding_replaces_placeholder_and_strips_llhls() {
        let playlist = "#EXTM3U\n\
#EXT-X-MOUFLON:PSCH:v2:Ook7quaiNgiyuhai\n\
#EXT-X-TARGETDURATION:2\n\
#EXT-X-MAP:URI=\"https://cdn/x/init_a.mp4\"\n\
#EXTINF:1.967\n\
#EXT-X-MOUFLON:URI:https://media-hls.doppiocdn.com/b-hls-16/94190182/94190182_1536_gjJHf3ZVwpfKPh9k7v97V2_1782130340.mp4\n\
https://media-hls.doppiocdn.com/b-hls-16/media.mp4\n\
#EXT-X-PART:DURATION=0.5,URI=\"part.mp4\"\n";
        let out = decode_playlist(playlist, PDKEY).unwrap();
        assert!(out.contains(DECRYPTED));
        assert!(!out.contains("media.mp4"));
        assert!(!out.contains("MOUFLON"));
        assert!(!out.contains("#EXT-X-PART:"));
    }

    #[test]
    fn picks_a_key_we_hold() {
        let playlist = "#EXTM3U\n#EXT-X-MOUFLON:PSCH:v2:aaaaaaaaaaaaaaaa\n#EXT-X-MOUFLON:PSCH:v2:Ook7quaiNgiyuhai\n";
        let mut ring = KeyRing::new();
        ring.insert("Ook7quaiNgiyuhai".into(), PDKEY.into());
        assert_eq!(pick_pkey(playlist, &ring).as_deref(), Some("Ook7quaiNgiyuhai"));
    }
}
