//! Stripchat API and CDN client.
//!
//! Two legs: the site API gives the room's `streamName` and live state; the
//! CDN gives the HLS master (with the MOUFLON pkey list) and the variant
//! playlists. Everything is plain GET with a browser user agent.
//!
//! The site API sits behind Cloudflare and returns 403 from flagged or
//! datacenter addresses, so an optional proxy is honoured for API calls. The
//! CDN (doppiocdn) is not gated and stays direct, which keeps segment
//! throughput and matches the addon's production setup.

use crate::config::Config;
use crate::error::{Error, Result};
use crate::hls::{self, Variant};
use crate::mouflon::{self, KeyRing};
use serde::Deserialize;

pub const API_BASE: &str = "https://stripchat.com";
pub const CDN_BASE: &str = "https://edge-hls.doppiocdn.com";
pub const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

/// Only these hosts are ever fetched for media (SSRF guard and accident guard).
pub fn is_allowed_media_host(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    let host = rest.split(['/', '?']).next().unwrap_or("");
    let host = host.split(':').next().unwrap_or("");
    let ok = |prefix: &str| {
        ["doppiocdn.com", "doppiocdn.org", "doppiocdn.net", "doppiocdn.media"]
            .iter()
            .any(|tld| host == format!("{prefix}.{tld}"))
    };
    ok("edge-hls") || ok("media-hls")
}

/// Shared HTTP client. One per recording so the connection pool is reused.
pub struct Http {
    agent: ureq::Agent,
    /// The API leg may go through a proxy; media never does.
    api_agent: ureq::Agent,
}

impl Http {
    pub fn new(config: &Config) -> Result<Self> {
        // No redirects: a 302 from the CDN must not carry the request (and the
        // body we write to disk) to a host the allow-list never checked.
        let plain = || {
            ureq::AgentBuilder::new()
                .timeout(std::time::Duration::from_secs(30))
                .redirects(0)
                .build()
        };
        let configured = config.proxy.trim();
        let api_agent = if configured.is_empty() {
            plain()
        } else {
            // ureq spells the SOCKS5 scheme `socks5://`; accept the curl-style
            // `socks5h://` and `socks5://` alike.
            let url = configured
                .strip_prefix("socks5h://")
                .map(|rest| format!("socks5://{rest}"))
                .unwrap_or_else(|| configured.to_string());
            let proxy = ureq::Proxy::new(&url)
                .map_err(|e| Error::Other(format!("bad proxy {configured:?}: {e}")))?;
            ureq::AgentBuilder::new()
                .timeout(std::time::Duration::from_secs(30))
                .redirects(0)
                .proxy(proxy)
                .build()
        };
        Ok(Self {
            agent: plain(),
            api_agent,
        })
    }

    fn get(&self, url: &str, referer: &str, accept: &str) -> Result<String> {
        self.api_agent
            .get(url)
            .set("User-Agent", USER_AGENT)
            .set("Referer", referer)
            .set("Accept", accept)
            .call()?
            .into_string()
            .map_err(|e| Error::Http(e.to_string()))
    }

    /// A media body as bytes over the shared direct agent. One TLS session for
    /// every segment is the difference between keeping up with a live stream
    /// and falling permanently behind it.
    pub fn media_bytes(&self, url: &str) -> Result<Vec<u8>> {
        if !is_allowed_media_host(url) {
            return Err(Error::Other(format!(
                "refusing to fetch a non-CDN URL: {}",
                crate::util::short_url(url, 80)
            )));
        }
        let resp = self
            .agent
            .get(url)
            .set("User-Agent", USER_AGENT)
            .set("Referer", &format!("{API_BASE}/"))
            .set("Origin", API_BASE)
            .call()?;
        let mut buf = Vec::new();
        std::io::Read::read_to_end(&mut resp.into_reader(), &mut buf)
            .map_err(|e| Error::Http(e.to_string()))?;
        Ok(buf)
    }

    fn get_media(&self, url: &str) -> Result<String> {
        // Playlist URLs come from the CDN master, so they get the same guard as
        // segments: a master cannot point us at an arbitrary host.
        if !is_allowed_media_host(url) {
            return Err(Error::Other(format!(
                "refusing to fetch a non-CDN playlist: {}",
                crate::util::short_url(url, 80)
            )));
        }
        self.agent
            .get(url)
            .set("User-Agent", USER_AGENT)
            .set("Referer", &format!("{API_BASE}/"))
            .set("Origin", API_BASE)
            .call()?
            .into_string()
            .map_err(|e| Error::Http(e.to_string()))
    }

    /// True when the API answers at all (any non-5xx response counts).
    pub fn api_reachable(&self) -> bool {
        self.api_agent
            .get("https://stripchat.com/api/front/v1/broadcasts/stripchat")
            .set("User-Agent", USER_AGENT)
            .set("Accept", "application/json")
            .call()
            .map(|r| r.status() < 500)
            .unwrap_or(false)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Cam {
    #[serde(rename = "streamName")]
    pub stream_name: Option<String>,
    #[serde(rename = "isLive", default)]
    pub is_live: bool,
    #[serde(rename = "isCamActive", default)]
    pub is_cam_active: bool,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub model_name: Option<String>,
}

impl Cam {
    pub fn is_public_live(&self) -> bool {
        self.is_live || self.is_cam_active || self.status == "public"
    }
    pub fn stream_name(&self) -> Result<&str> {
        self.stream_name
            .as_deref()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| Error::Offline("room is not live (no streamName)".into()))
    }
}

#[derive(Deserialize)]
struct Broadcast {
    item: Option<Cam>,
}

/// Room metadata. A 404 or a missing item means the model is offline.
pub fn fetch_cam(http: &Http, username: &str) -> Result<Cam> {
    let url = format!("{API_BASE}/api/front/v1/broadcasts/{}", username);
    let body = match http.get(&url, &format!("{API_BASE}/{username}"), "application/json") {
        Ok(body) => body,
        Err(Error::Http(msg)) if msg.contains("404") => {
            return Err(Error::Offline(format!("no such room: {username}")))
        }
        Err(Error::Http(msg)) if msg.contains("403") => {
            return Err(Error::Offline(format!(
                "{username}: the site API refused this address (HTTP 403). \
                 Set a proxy in Settings if this keeps happening."
            )))
        }
        Err(e) => return Err(e),
    };
    let parsed: Broadcast = serde_json::from_str(&body)
        .map_err(|e| Error::Http(format!("bad broadcasts JSON: {e}")))?;
    parsed
        .item
        .ok_or_else(|| Error::Offline(format!("{username} is offline")))
}

pub fn master_url(stream_name: &str) -> String {
    format!("{CDN_BASE}/hls/{s}/master/{s}_auto.m3u8", s = stream_name)
}

fn with_pkey(url: &str, pkey: &str) -> String {
    let has_pkey = url.contains("pkey=");
    let has_psch = url.contains("psch=");
    let sep = if url.contains('?') { '&' } else { '?' };
    match (has_pkey, has_psch) {
        (true, true) => url.to_string(),
        (true, false) => format!("{url}{sep}psch=v2"),
        _ => format!("{url}{sep}psch=v2&pkey={pkey}"),
    }
}

/// Master playlist without a pkey: only read for its PSCH key list.
pub fn fetch_master(http: &Http, stream_name: &str) -> Result<String> {
    http.get_media(&master_url(stream_name))
}

/// Master playlist with the chosen pkey: the variants are only usable then.
/// The flag reports the advert playlist, i.e. a rotated pkey.
pub fn fetch_master_with_pkey(
    http: &Http,
    stream_name: &str,
    pkey: &str,
) -> Result<(String, bool)> {
    let body = http.get_media(&with_pkey(&master_url(stream_name), pkey))?;
    let stale = mouflon::is_stale(&body);
    Ok((body, stale))
}

/// Variant playlist plus its advert flag (stale pkey).
pub fn fetch_variant_playlist(http: &Http, url: &str, pkey: &str) -> Result<(String, bool)> {
    let body = http.get_media(&with_pkey(url, pkey))?;
    let stale = mouflon::is_stale(&body);
    Ok((body, stale))
}

/// Full session resolution: room, pkey, variant list.
pub fn resolve(
    http: &Http,
    username: &str,
    ring: &KeyRing,
) -> Result<(Cam, String, Vec<Variant>)> {
    let cam = fetch_cam(http, username)?;
    if !cam.is_public_live() {
        return Err(Error::Offline(format!("{username} is offline")));
    }
    let stream_name = cam.stream_name()?.to_string();

    let bare = fetch_master(http, &stream_name)?;
    let pkey = mouflon::pick_pkey(&bare, ring).ok_or_else(|| {
        Error::Other(
            "the playlist advertises no MOUFLON key we hold; add a pkey:pdkey pair in Settings"
                .into(),
        )
    })?;
    let (master, stale) = fetch_master_with_pkey(http, &stream_name, &pkey)?;
    if stale {
        return Err(Error::Other(
            "the CDN served the advert playlist; the pkey has rotated".into(),
        ));
    }
    let variants = hls::parse_master_playlist(&master_url(&stream_name), &master);
    if variants.is_empty() {
        return Err(Error::Other(
            "the master playlist listed no variants".into(),
        ));
    }
    Ok((cam, pkey, variants))
}

/// Choose a variant by name (`source`, `1080p`, ...) or the highest quality.
pub fn pick_variant<'a>(variants: &'a [Variant], quality: &str) -> Option<&'a Variant> {
    if quality.is_empty() || quality == "source" || quality == "auto" {
        return hls::best_variant(variants);
    }
    variants
        .iter()
        .find(|v| v.name.as_deref() == Some(quality))
        .or_else(|| hls::best_variant(variants))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_cdn_hosts_pass_the_guard() {
        assert!(is_allowed_media_host(
            "https://media-hls.doppiocdn.com/b-hls-20/1/x.mp4"
        ));
        assert!(is_allowed_media_host("https://edge-hls.doppiocdn.org/hls/1"));
        assert!(!is_allowed_media_host("https://evil.doppiocdn.com.attacker.net/x"));
        assert!(!is_allowed_media_host("http://media-hls.doppiocdn.com/x"));
        assert!(!is_allowed_media_host("file:///etc/passwd"));
    }

    #[test]
    fn pkey_params_are_appended_once() {
        let base = "https://media-hls.doppiocdn.com/a.m3u8?x=1";
        let once = with_pkey(base, "K");
        assert!(once.contains("psch=v2"));
        assert!(once.contains("pkey=K"));
        assert_eq!(with_pkey(&once, "K"), once);
    }

    #[test]
    fn cam_liveness_rules() {
        let live = Cam {
            stream_name: Some("123".into()),
            is_live: false,
            is_cam_active: false,
            status: "public".into(),
            model_name: None,
        };
        assert!(live.is_public_live());
        assert_eq!(live.stream_name().unwrap(), "123");
    }
}
