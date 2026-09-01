pub mod batch;
pub mod classifier;
pub mod cli;
pub mod completions;
pub mod config;
pub mod doctor;
pub mod downloader;
pub mod error;
pub mod interactive;
pub mod lyrics;
pub mod metadata;
pub mod normalizer;
pub mod orientation;
pub mod preset;
pub mod progress;
pub mod provider;
pub mod quality;
pub mod recovery;
pub mod scheduler;
pub mod tiktok;

use std::path::Path;
use clap::Parser;
use classifier::SmartClassifier;
use cli::{Cli, Commands};
use completions::Completions;
use config::Config;
use doctor::Doctor;
use downloader::Downloader;
use error::{DlpError, Result};
use interactive::run_interactive;
use lyrics::LyricsFetcher;
use preset::{Preset, PresetManager};
use quality::QualityPreference;

pub fn run() -> Result<()> {
    let args = Cli::parse();
    let app_config = Config::load();
    let preset_manager = PresetManager::load_all();

    // 1. If no command and no valid non-empty URL provided, launch Interactive UI
    let has_non_empty_url = args.url.as_ref().map(|u| !u.trim().is_empty()).unwrap_or(false);
    if args.command.is_none() && !has_non_empty_url {
        return run_interactive(&preset_manager, &app_config);
    }

    // 2. Handle 'dlp doctor' command
    if let Some(Commands::Doctor) = &args.command {
        return Doctor::run_diagnostics(&preset_manager, &app_config);
    }

    // 3. Handle 'dlp completions' command
    if let Some(Commands::Completions(c)) = &args.command {
        Completions::generate_to_stdout(c.shell);
        return Ok(());
    }

    // 4. Handle 'dlp presets' command
    if let Some(Commands::Presets) = &args.command {
        println!("\n📑 Available Presets:");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        for p in preset_manager.list() {
            let desc = p.description.as_deref().unwrap_or("No description");
            let mode = if p.extract_audio {
                format!("Audio ({})", p.audio_format.as_deref().unwrap_or("opus"))
            } else {
                format!("Video ({})", p.container)
            };
            println!("• {:<10} [{}] - {}", p.name, mode, desc);
        }
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
        return Ok(());
    }

    // 5. Handle 'dlp batch' command
    if let Some(Commands::Batch(b)) = &args.command {
        let urls = batch::resolve_inputs_to_urls(&b.inputs)?;
        if urls.is_empty() {
            println!("⚠️  No URLs found in the specified input.");
            return Ok(());
        }

        let explicit_preset = if b.preset.eq_ignore_ascii_case("auto") {
            None
        } else {
            Some(b.preset.clone())
        };

        let effective_output_dir = b.output_dir.as_deref().or(app_config.download_dir.as_deref());
        let cp_path = batch::determine_checkpoint_path(&b.inputs, effective_output_dir);
        let effective_concurrency = b.concurrency.unwrap_or(app_config.download.concurrency);

        batch::run_batch(
            &urls,
            &preset_manager,
            explicit_preset.as_deref(),
            b.quality.as_deref(),
            b.lyrics,
            effective_output_dir,
            b.resume,
            Some(&cp_path),
            effective_concurrency,
        )?;
        return Ok(());
    }

    // 6. Resolve single-media execution parameters
    let (url, explicit_preset, quality_override, lyrics_override, info_only, explain, output_dir) = match &args.command {
        Some(Commands::Video(d)) => (
            d.url.trim().to_string(),
            Some("video".to_string()),
            d.quality.as_deref(),
            d.lyrics,
            d.info_only,
            d.explain,
            d.output_dir.as_deref(),
        ),
        Some(Commands::Music(d)) => (
            d.url.trim().to_string(),
            Some("music".to_string()),
            d.quality.as_deref(),
            d.lyrics,
            d.info_only,
            d.explain,
            d.output_dir.as_deref(),
        ),
        Some(Commands::Tiktok(d)) => (
            d.url.trim().to_string(),
            Some("tiktok".to_string()),
            d.quality.as_deref(),
            d.lyrics,
            d.info_only,
            d.explain,
            d.output_dir.as_deref(),
        ),
        None => {
            let u = match &args.url {
                Some(u) if !u.trim().is_empty() => u.trim().to_string(),
                _ => return Err(DlpError::MissingUrl),
            };
            (
                u,
                args.preset,
                args.quality.as_deref(),
                args.lyrics,
                args.info_only,
                args.explain,
                args.output_dir.as_deref(),
            )
        }
        Some(Commands::Presets) | Some(Commands::Doctor) | Some(Commands::Completions(_)) | Some(Commands::Batch(_)) => unreachable!(),
    };

    // 7. Dependency check
    Downloader::verify_dependencies()?;

    // 8. Fetch & Inspect Metadata
    println!("\n🔍 Inspecting media metadata...");
    let meta = Downloader::fetch_metadata(&url)?;

    // 9. Automatic Classification / Preset Resolution
    let preset_name = if let Some(p_name) = explicit_preset {
        p_name
    } else {
        let detected = SmartClassifier::classify(&url, &meta);
        println!("🤖 Auto-Classification: {}", detected.display_label());
        detected.default_preset_name().to_string()
    };

    let mut preset: Preset = preset_manager
        .get(&preset_name)
        .ok_or_else(|| DlpError::PresetNotFound(preset_name.clone()))?
        .clone();

    if let Some(want_lyrics) = lyrics_override {
        preset.write_lyrics = want_lyrics;
    }

    let orientation = meta.orientation();
    let available_res = meta.available_resolutions();
    let res_strings: Vec<String> = available_res.iter().map(|r| format!("{}p", r)).collect();
    let res_display = if res_strings.is_empty() {
        "Auto".to_string()
    } else {
        res_strings.join(", ")
    };

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("🎬 Title       : {}", meta.title);
    if let Some(uploader) = &meta.uploader {
        println!("👤 Uploader    : {}", uploader);
    }
    println!("⏱️  Duration    : {}", meta.format_duration());
    println!("📐 Orientation : {}", orientation.display_name());
    println!("📺 Available   : {}", res_display);
    println!("⚙️  Active Preset: {} ({})", preset.name, if preset.extract_audio { "Audio" } else { "Video" });
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // If info-only flag is set, stop here
    if info_only {
        println!("ℹ️  Info only mode. Exiting without downloading.\n");
        return Ok(());
    }

    // 10. Compute Effective Quality with Orientation Policy
    let effective_quality = preset.effective_quality_preference(quality_override, orientation)?;

    // If explain flag is set, display detailed Decision Trace and exit
    if explain {
        let trace = normalizer::DecisionTrace::build(
            &url,
            &meta,
            &preset,
            &effective_quality,
            output_dir,
        );
        trace.print_trace();
        return Ok(());
    }

    if preset.extract_audio {
        println!("🎵 Target Mode   : Audio Extraction ({})\n", preset.audio_format.as_deref().unwrap_or("opus"));
    } else {
        let chosen_display = match &effective_quality {
            QualityPreference::Best => "Best available".to_string(),
            QualityPreference::SpecificHeight(h) => {
                if let Some(matched) = effective_quality.select_best_resolution(&available_res) {
                    format!("{}p (Capped at {}p)", matched, h)
                } else {
                    format!("{}p", h)
                }
            }
        };
        println!("🎯 Target Quality : {}\n", chosen_display);
    }

    // 11. Execute Download
    let effective_output_dir = output_dir.or(app_config.download_dir.as_deref());
    Downloader::download(&url, &preset, &effective_quality, effective_output_dir)?;

    // 12. Synchronize Track-by-Track Synced Lyrics (LRCLIB) for Music Mode
    if preset.write_lyrics {
        let base_dir = effective_output_dir.map(Path::new).unwrap_or_else(|| Path::new("."));
        LyricsFetcher::sync_lyrics_for_directory(base_dir, meta.uploader.as_deref());
    }

    Ok(())
}
