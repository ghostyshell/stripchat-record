//! MOUFLON key registry assembly.
//!
//! Pairs are protocol data, not code: the CDN rotates pkeys and the matching
//! pdkeys are recovered by the community. Three sources stack, later wins:
//! the bundled defaults, `PD_KEY_*` environment variables, then the pairs saved
//! in Settings.

use crate::mouflon::KeyRing;

/// Known-good pairs at the time of writing. They go stale; Settings and the
/// environment exist so a user can paste a fresh pair without a new build.
pub const BUNDLED: &[(&str, &str)] = &[
    ("Ook7quaiNgiyuhai", "EQueeGh2kaewa3ch"),
    ("Zeechoej4aleeshi", "ubahjae7goPoodi6"),
];

pub fn ring(custom: &KeyRing) -> KeyRing {
    let mut ring = KeyRing::new();
    for (pkey, pdkey) in BUNDLED {
        ring.insert(pkey.to_string(), pdkey.to_string());
    }
    ring.extend(crate::mouflon::keys_from_env());
    for (pkey, pdkey) in custom {
        if !pkey.trim().is_empty() && !pdkey.trim().is_empty() {
            ring.insert(pkey.trim().to_string(), pdkey.trim().to_string());
        }
    }
    ring
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_pairs_override_bundled_ones() {
        let mut custom = KeyRing::new();
        custom.insert(BUNDLED[0].0.to_string(), "replacement".to_string());
        let ring = ring(&custom);
        assert_eq!(ring.get(BUNDLED[0].0).map(String::as_str), Some("replacement"));
        assert!(ring.contains_key(BUNDLED[1].0));
    }
}
