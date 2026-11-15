//! Headless CLI over `stripchat-core`.
//!
//! Recording is append-only per segment, so a Ctrl+C at any moment leaves a
//! playable `.recording.mp4` behind. The `record` command still finishes the
//! remux when the stream ends normally.

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use stripchat_core::config::{self, Config};
use stripchat_core::jobs::{set_progress_hook, JobManager, JobState};
use stripchat_core::keys;
use stripchat_core::media;
use stripchat_core::recorder::{self, RecordOptions};
use stripchat_core::util::{format_bytes, format_duration};
use stripchat_core::{doctor, library, Result};

#[derive(Parser)]
#[command(
    name = "stripchat-recorder",
    version,
    about = "Record Stripchat live rooms from the CDN, no screen capture"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Record a live room until it ends (or --duration elapses)
    Record {
        /// Model username, as it appears in the site URL
        model: String,
        /// Seconds to record; omit to record until the stream ends
        #[arg(long)]
        duration: Option<u64>,
        /// source (default), auto, or an exact variant such as 1080p
        #[arg(long, default_value = "source")]
        quality: String,
        /// Rotate to a new file at this size (1G, 512M, or 0 to disable)
        #[arg(long, default_value = "1G")]
        split_size: String,
        /// Output directory
        #[arg(long)]
        output_dir: Option<PathBuf>,
        /// Exact output path, overrides --output-dir
        #[arg(long)]
        output: Option<PathBuf>,
        /// After recording, compress with this preset (copy, h264_balanced, ...)
        #[arg(long)]
        convert: Option<String>,
        /// Keep the intermediate .recording.mp4
        #[arg(long)]
        keep_source: bool,
        /// Proxy for the site API, e.g. socks5h://127.0.0.1:9050
        #[arg(long)]
        proxy: Option<String>,
    },
    /// Convert or compress an existing file
    Convert {
        file: PathBuf,
        /// Preset id from `presets`
        #[arg(long, default_value = "h264_balanced")]
        preset: String,
        /// Output path (default: input with the preset id appended)
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// List the presets available to `convert`
    Presets,
    /// List recorded files
    Library {
        #[arg(long)]
        dir: Option<PathBuf>,
    },
    /// Check ffmpeg, the output folder, the key ring and the site API
    Doctor,
    /// Show the loaded MOUFLON key pairs (pkeys only, never pdkeys)
    Keys,
}

fn main() -> std::process::ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "stripchat_core=info".into()),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err}");
            std::process::ExitCode::FAILURE
        }
    }
}

/// Ctrl+C asks the job to stop at the next segment boundary, so the file is
/// finalized and the remux still runs. A second Ctrl+C falls through to the
/// default handler and kills the process; segment appends are atomic, so even
/// that leaves a playable `.recording.mp4`.
fn install_ctrlc() -> Arc<AtomicBool> {
    let flag = Arc::new(AtomicBool::new(false));
    let handler_flag = flag.clone();
    let _ = ctrlc::set_handler(move || {
        handler_flag.store(true, Ordering::SeqCst);
    });
    flag
}

fn run(cli: Cli) -> Result<()> {
    let mut cfg = Config::load();
    let interrupted = install_ctrlc();
    match cli.command {
        Command::Record {
            model,
            duration,
            quality,
            split_size,
            output_dir,
            output,
            convert,
            keep_source,
            proxy,
        } => {
            if let Some(proxy) = proxy {
                cfg.proxy = proxy;
            }
            if let Some(dir) = output_dir {
                cfg.output_dir = dir.to_string_lossy().into_owned();
            }
            // `0`/`none` disables splitting; anything else must parse.
            cfg.split_size_bytes = if matches!(
                split_size.to_ascii_lowercase().as_str(),
                "0" | "none" | "off" | "no"
            ) {
                0
            } else {
                config::parse_size(&split_size)
                    .ok_or_else(|| stripchat_core::Error::Other(format!("bad --split-size {split_size:?}")))?
            };
            let ring = keys::ring(&cfg.pdkeys);
            let opts = RecordOptions {
                model: model.clone(),
                quality,
                duration_secs: duration,
                output,
                config: cfg.clone(),
            };

            let manager = Arc::new(JobManager::new());
            let hook_manager = manager.clone();
            set_progress_hook(Some(Arc::new(move |id, done, total| {
                // Recording reports bytes with no total; conversion reports a
                // known total. Only the recording line is a live byte counter.
                if total == 0
                    && hook_manager
                        .get(id)
                        .map(|j| j.state == JobState::Running)
                        .unwrap_or(false)
                {
                    eprint!("\r  recorded {}", format_bytes(done));
                }
            })));

            let job_config = cfg.clone();
            let id = manager.spawn("record", &model, |_job| {}, move |ctx| {
                let report = recorder::record(&opts, &ring, ctx)?;
                ctx.message(format!(
                    "{} in {} ({} segment(s), {})",
                    format_bytes(report.bytes),
                    format_duration(report.seconds),
                    report.segments,
                    report.end_reason.as_str()
                ));
                let ffmpeg = media::ffmpeg(&job_config.ffmpeg_path);
                let ffprobe = media::ffprobe(&job_config.ffprobe_path);
                let convert_id = convert.as_deref().or(if job_config.convert_on_finish {
                    Some(job_config.convert_preset.as_str())
                } else {
                    None
                });
                for path in &report.paths {
                    ctx.message(format!("finalizing {}", path.display()));
                    let out = media::finalize(
                        ffmpeg.as_deref(),
                        ffprobe.as_deref(),
                        path,
                        convert_id,
                        keep_source || job_config.keep_ts,
                        ctx,
                    )?;
                    ctx.message(format!("saved {}", out.display()));
                    if let Some(note) = &report.note {
                        eprintln!("\n  note: {note}");
                    }
                }
                Ok(())
            });

            while manager
                .get(id)
                .map(|j| j.state == JobState::Running)
                .unwrap_or(false)
            {
                if interrupted.swap(false, Ordering::SeqCst) {
                    eprintln!("\n  stopping at the next segment and finalizing ...");
                    manager.cancel(id);
                }
                std::thread::sleep(Duration::from_millis(250));
            }
            eprintln!();
            let job = manager.get(id).expect("job");
            match job.state {
                JobState::Done => println!("{}", job.message),
                _ => {
                    eprintln!("recording failed: {}", job.message);
                    return Err(stripchat_core::Error::Other(job.message));
                }
            }
        }
        Command::Convert {
            file,
            preset,
            output,
        } => {
            let ffmpeg = media::ffmpeg(&cfg.ffmpeg_path).ok_or_else(|| {
                stripchat_core::Error::Other(
                    "ffmpeg not found; install it or set the path in Settings".into(),
                )
            })?;
            let ffprobe = media::ffprobe(&cfg.ffprobe_path);
            let out = output.unwrap_or_else(|| {
                let stem = file
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "output".into());
                file.with_file_name(format!("{stem}.{preset}.mp4"))
            });
            let manager = JobManager::new();
            let label = file.display().to_string();
            let out_for_job = out.clone();
            let id = manager.spawn("convert", &label, |_| {}, move |ctx| {
                let total = ffprobe
                    .as_deref()
                    .and_then(|probe| media::duration(probe, &file));
                let chosen = media::preset(&preset);
                media::encode(&ffmpeg, &file, &out_for_job, chosen.args, total, true, ctx)
            });
            while manager
                .get(id)
                .map(|j| j.state == JobState::Running)
                .unwrap_or(false)
            {
                if interrupted.swap(false, Ordering::SeqCst) {
                    manager.cancel(id);
                }
                std::thread::sleep(Duration::from_millis(200));
            }
            let job = manager.get(id).expect("job");
            if job.state == JobState::Done {
                println!("saved {}", out.display());
            } else {
                return Err(stripchat_core::Error::Other(job.message));
            }
        }
        Command::Presets => {
            for preset in media::PRESETS {
                println!("{:16} {:22} {}", preset.id, preset.label, preset.detail);
            }
        }
        Command::Library { dir } => {
            let dir = dir.unwrap_or_else(|| cfg.resolved_output_dir());
            for row in library::list(&dir)? {
                let marker = if row.in_progress { " (recording)" } else { "" };
                println!(
                    "{:<10} {}  {}{}",
                    format_bytes(row.size_bytes),
                    row.path,
                    row.name,
                    marker
                );
            }
        }
        Command::Doctor => {
            let ring = keys::ring(&cfg.pdkeys);
            let mut all_ok = true;
            for check in doctor::run(&cfg, &ring) {
                all_ok &= check.ok;
                println!(
                    "{} {:<16} {}{}",
                    if check.ok { "ok  " } else { "FAIL" },
                    check.name,
                    check.detail,
                    if check.ok || check.fix.is_empty() {
                        String::new()
                    } else {
                        format!("\n     fix: {}", check.fix)
                    }
                );
            }
            if !all_ok {
                return Err(stripchat_core::Error::Other("one or more checks failed".into()));
            }
        }
        Command::Keys => {
            let ring = keys::ring(&cfg.pdkeys);
            if ring.is_empty() {
                println!("no key pairs loaded");
            }
            for pkey in ring.keys() {
                println!("{pkey}");
            }
            println!("\n{} pair(s). pdkeys are never printed.", ring.len());
        }
    }
    Ok(())
}
