//! Stripchat Recorder engine.
//!
//! The app records a live room straight from the CDN instead of capturing a
//! screen: the site API names the room's `streamName`, the CDN master lists
//! quality variants, the variant playlist hides segment URIs behind MOUFLON v2,
//! and the segments themselves are plain fMP4 fragments that append into a
//! file. No browser, no encoder, no re-encode.
//!
//! Modules:
//! - `site`: Stripchat API and CDN requests
//! - `hls`: playlist parsing and variant choice
//! - `mouflon`: pkey/pdkey handling and URI deobfuscation
//! - `recorder`: the live recording loop and the append-only writer
//! - `media`: ffmpeg remux and compression presets
//! - `jobs`: background queue with progress and cancel
//! - `library`, `doctor`, `config`, `keys`, `util`

pub mod config;
pub mod doctor;
pub mod error;
pub mod hls;
pub mod jobs;
pub mod keys;
pub mod library;
pub mod media;
pub mod mouflon;
pub mod recorder;
pub mod site;
pub mod util;

pub use config::Config;
pub use error::{Error, Result};
pub use recorder::{EndReason, RecordOptions, RecordReport};
