pub mod batch;
pub mod cache;
pub mod classifier;
pub mod cli;
pub mod completions;
pub mod config;
pub mod doctor;
pub mod downloader;
pub mod engine;
pub mod error;
pub mod event;
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
pub mod throttle;
pub mod tiktok;

use std::path::Path;
use std::sync::Arc;
use clap::Parser;
use classifier::SmartClassifier;
use cli::{Cli, Commands};
use completions::Completions;
use config::Config;
use doctor::Doctor;
use downloader::Downloader;
use error::{DlpError, Result};
pub use event::{
    init_logging, DownloadEvent, EventDispatcher, EventListener, JsonLinesEventListener,
    TracingEventListener,
};
use interactive::run_interactive;
use lyrics::LyricsFetcher;
use preset::{Preset, PresetManager};
use quality::QualityPreference;

pub fn run() -> Result<()> {
    let args = Cli::parse();
    init_logging(args.verbose, args.quiet, args.json);

    let mut dispatcher = EventDispatcher::default();
    dispatcher.register(Box::new(TracingEventListener));
    if args.json {
        dispatcher.register(Box::new(JsonLinesEventListener));
    }

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

    // 5. Handle 'dlp config' command
    if let Some(Commands::Config(c)) = &args.command {
        match &c.subcommand {
            None | Some(cli::ConfigSubcommands::Show) => {
                let path_str = config::Config::config_path()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "Unknown".to_string());
                println!("\n⚙️  dlp Configuration ({})", path_str);
                println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                let toml_str = toml::to_string_pretty(&app_config)
                    .unwrap_or_else(|_| "Error formatting config".to_string());
                println!("{}", toml_str.trim());
                println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
            }
            Some(cli::ConfigSubcommands::Path) => {
                if let Some(p) = config::Config::config_path() {
                    println!("{}", p.display());
                } else {
                    println!("Unknown config path");
                }
            }
            Some(cli::ConfigSubcommands::Init) => {
                let default_cfg = config::Config::default();
                let saved_path = default_cfg.save()?;
                println!("✅ Initialized default configuration at {}", saved_path.display());
            }
            Some(cli::ConfigSubcommands::Set { key, value }) => {
                let mut cfg = config::Config::load();
                cfg.set_value(key, value)?;
                let saved_path = cfg.save()?;
                println!("✅ Updated '{}' = '{}' in {}", key, value, saved_path.display());
            }
            Some(cli::ConfigSubcommands::Migrate) => {
                let mut cfg = config::Config::load();
                let did_migrate = cfg.migrate();
                let saved_path = cfg.save()?;
                if did_migrate {
                    println!("✅ Successfully migrated configuration to v2.0 schema at {}", saved_path.display());
                } else {
                    println!("ℹ️  Configuration is already up to date with v2.0 schema at {}", saved_path.display());
                }
            }
        }
        return Ok(());
    }

    // 6. Handle 'dlp debug' command
    if let Some(Commands::Debug(d)) = &args.command {
        println!("🔍 Fetching raw extractor metadata for '{}'...", d.url);
        let mut cmd = std::process::Command::new("yt-dlp");
        cmd.arg("--dump-single-json").arg("--no-playlist");
        if crate::tiktok::TikTokFallback::is_tiktok_url(&d.url) {
            cmd.arg("--impersonate").arg("chrome");
        }
        cmd.arg(&d.url);

        let output = cmd.output()?;
        if output.status.success() {
            let json_str = String::from_utf8_lossy(&output.stdout);
            if d.raw {
                println!("{}", json_str.trim());
            } else {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&json_str) {
                    let pretty = serde_json::to_string_pretty(&parsed).unwrap_or_else(|_| json_str.to_string());
                    println!("{}", pretty);
                } else {
                    println!("{}", json_str.trim());
                }
            }
            return Ok(());
        } else {
            let err = String::from_utf8_lossy(&output.stderr);
            eprintln!("❌ Raw extractor error:\n{}", err);
            return Err(error::DlpError::YtDlpFailed {
                code: output.status.code().unwrap_or(-1),
                stderr: err.to_string(),
            });
        }
    }

    // 7. Handle 'dlp batch' command
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

        let effective_hq = b.hq.as_deref().or(args.hq.as_deref());
        let effective_vq = b.vq.as_deref().or(args.vq.as_deref());

        batch::run_batch_extended(
            &urls,
            &preset_manager,
            explicit_preset.as_deref(),
            b.quality.as_deref(),
            effective_hq,
            effective_vq,
            b.lyrics,
            effective_output_dir,
            b.resume,
            Some(&cp_path),
            effective_concurrency,
            Some(Arc::new(dispatcher)),
        )?;
        return Ok(());
    }

    // 8. Handle 'dlp cache' command
    if let Some(Commands::Cache { action }) = &args.command {
        let cache = crate::cache::MetadataCache::new();
        match action {
            cli::CacheAction::Clean => {
                let count = cache.purge_expired();
                println!("🧹 Cleaned {} expired cache entries.", count);
            }
            cli::CacheAction::Purge => {
                cache.purge_all()?;
                println!("🗑️  Purged all metadata cache entries.");
            }
            cli::CacheAction::Path => {
                println!("{}", cache.cache_dir().display());
            }
        }
        return Ok(());
    }

    // 9. Resolve single-media execution parameters
    let (url, explicit_preset, quality_override, lyrics_override, info_only, explain, output_dir, no_cache) = match &args.command {
        Some(Commands::Video(d)) => (
            d.url.trim().to_string(),
            Some("video".to_string()),
            d.quality.as_deref(),
            d.lyrics,
            d.info_only,
            d.explain,
            d.output_dir.as_deref(),
            args.no_cache || d.no_cache,
        ),
        Some(Commands::Music(d)) => (
            d.url.trim().to_string(),
            Some("music".to_string()),
            d.quality.as_deref(),
            d.lyrics,
            d.info_only,
            d.explain,
            d.output_dir.as_deref(),
            args.no_cache || d.no_cache,
        ),
        Some(Commands::Tiktok(d)) => (
            d.url.trim().to_string(),
            Some("tiktok".to_string()),
            d.quality.as_deref(),
            d.lyrics,
            d.info_only,
            d.explain,
            d.output_dir.as_deref(),
            args.no_cache || d.no_cache,
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
                args.no_cache,
            )
        }
        Some(Commands::Presets)
        | Some(Commands::Doctor)
        | Some(Commands::Completions(_))
        | Some(Commands::Batch(_))
        | Some(Commands::Config(_))
        | Some(Commands::Debug(_))
        | Some(Commands::Cache { .. }) => unreachable!(),
    };

    // 10. Dependency check
    Downloader::verify_dependencies()?;

    // 11. Fetch & Inspect Metadata
    println!("\n🔍 Inspecting media metadata...");
    let meta = Downloader::fetch_metadata_cached(&url, !no_cache)?;
    dispatcher.dispatch(&DownloadEvent::MetadataFetched {
        url: url.clone(),
        title: meta.title.clone(),
    });

    // 9. Automatic Classification / Preset Resolution
    let preset_name = if let Some(p_name) = explicit_preset {
        p_name
    } else {
        let detected = SmartClassifier::classify(&url, &meta);
        dispatcher.dispatch(&DownloadEvent::ClassificationCompleted {
            url: url.clone(),
            classification: detected.clone(),
        });
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

    // 10. Compute Effective Quality with Orientation Policy & Overrides
    let target_quality = match orientation {
        crate::orientation::Orientation::Horizontal => args.hq.as_deref().or(quality_override),
        crate::orientation::Orientation::Vertical => args.vq.as_deref().or(quality_override),
        crate::orientation::Orientation::Square => quality_override,
    };
    let effective_quality = preset
        .effective_quality_preference(target_quality, orientation)?
        .for_orientation(orientation);

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
            QualityPreference::VerticalResolution(w) => {
                format!("{}p (Vertical)", w)
            }
        };
        println!("🎯 Target Quality : {}\n", chosen_display);
    }

    // 11. Execute Download
    let effective_output_dir = output_dir.or(app_config.download_dir.as_deref());
    Downloader::download_with_dispatcher(&url, &preset, &effective_quality, effective_output_dir, Some(&dispatcher))?;

    // 12. Synchronize Track-by-Track Synced Lyrics (LRCLIB) for Music Mode
    if preset.write_lyrics {
        let base_dir = effective_output_dir.map(Path::new).unwrap_or_else(|| Path::new("."));
        LyricsFetcher::sync_lyrics_for_directory(base_dir, meta.uploader.as_deref());
    }

    Ok(())
}
