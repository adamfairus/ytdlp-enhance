use inquire::{Confirm, InquireError, Select, Text};
use std::path::Path;
use crate::batch::{read_urls_from_file, run_batch};
use crate::config::Config;
use crate::downloader::Downloader;
use crate::error::{DlpError, Result};
use crate::lyrics::LyricsFetcher;
use crate::metadata::VideoMetadata;
use crate::preset::{Preset, PresetManager};
use crate::quality::QualityPreference;

enum StepResult<T> {
    Value(T),
    Back,
    Exit,
}

pub fn run_interactive(preset_manager: &PresetManager, config: &Config) -> Result<()> {
    loop {
        println!();
        println!("╭──────────────────────────────────────────────────╮");
        println!("│             🦀 dlp — Downloader CLI              │");
        println!("│      Smart yt-dlp & ffmpeg Orchestrator          │");
        println!("╰──────────────────────────────────────────────────╯");
        println!();

        let menu_options = vec![
            "🎬 Video (General MP4)",
            "🎵 Music (Extract Opus / Audio)",
            "📱 TikTok / Shorts (Vertical MP4)",
            "📦 Batch Download (from file)",
            "🩺 System Diagnostics (Doctor)",
            "📑 View All Presets",
            "🚪 Exit",
        ];

        let selection = match Select::new("What would you like to do?", menu_options).prompt() {
            Ok(s) => s,
            Err(InquireError::OperationCanceled) | Err(InquireError::OperationInterrupted) => {
                println!("\nGoodbye! 👋\n");
                return Ok(());
            }
            Err(e) => return Err(DlpError::PromptError(e)),
        };

        if selection == "📦 Batch Download (from file)" {
            match run_interactive_batch(preset_manager, config) {
                Ok(StepResult::Value(_)) | Ok(StepResult::Back) => continue,
                Ok(StepResult::Exit) => {
                    println!("\nGoodbye! 👋\n");
                    return Ok(());
                }
                Err(e) => {
                    eprintln!("\n❌ Batch Error: {e}");
                    let _ = Text::new("Press Enter to return to main menu...").prompt();
                    continue;
                }
            }
        }

        let preset_name = match selection {
            "🎬 Video (General MP4)" => "video",
            "🎵 Music (Extract Opus / Audio)" => "music",
            "📱 TikTok / Shorts (Vertical MP4)" => "tiktok",
            "🩺 System Diagnostics (Doctor)" => {
                let _ = crate::doctor::Doctor::run_diagnostics(preset_manager, config);
                let _ = Text::new("Press Enter to return to main menu...").prompt();
                continue;
            }
            "📑 View All Presets" => {
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
                println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                let _ = Text::new("Press Enter to return to main menu...").prompt();
                continue;
            }
            "🚪 Exit" => {
                println!("Goodbye! 👋\n");
                return Ok(());
            }
            _ => "video",
        };

        let preset = preset_manager
            .get(preset_name)
            .ok_or_else(|| DlpError::PresetNotFound(preset_name.to_string()))?;

        // Run the download wizard for the selected preset
        match run_download_wizard(preset, config) {
            Ok(StepResult::Value(_)) => {
                let again = match Confirm::new("Do you want to download another media?")
                    .with_default(false)
                    .prompt()
                {
                    Ok(ans) => ans,
                    _ => false,
                };
                if !again {
                    println!("\nThank you for using dlp! Goodbye! 👋\n");
                    return Ok(());
                }
            }
            Ok(StepResult::Back) => {
                continue;
            }
            Ok(StepResult::Exit) => {
                println!("\nGoodbye! 👋\n");
                return Ok(());
            }
            Err(e) => {
                eprintln!("\n❌ Error: {e}");
                let _ = Text::new("Press Enter to return to main menu...").prompt();
                continue;
            }
        }
    }
}

fn run_interactive_batch(preset_manager: &PresetManager, config: &Config) -> Result<StepResult<()>> {
    loop {
        let file_prompt = Text::new("Enter path to URLs file (or 'b' to go back):")
            .with_help_message("e.g. urls.txt or /path/to/links.txt")
            .prompt();

        let path_str = match file_prompt {
            Ok(val) => {
                let trimmed = val.trim().to_string();
                if trimmed.eq_ignore_ascii_case("b") || trimmed.eq_ignore_ascii_case("back") {
                    return Ok(StepResult::Back);
                }
                if trimmed.is_empty() {
                    println!("⚠️  File path cannot be empty.\n");
                    continue;
                }
                trimmed
            }
            Err(InquireError::OperationCanceled) => return Ok(StepResult::Back),
            Err(InquireError::OperationInterrupted) => return Ok(StepResult::Exit),
            Err(e) => return Err(DlpError::PromptError(e)),
        };

        let path = Path::new(&path_str);
        if !path.is_file() {
            println!("⚠️  File '{}' not found. Please verify the path.\n", path_str);
            continue;
        }

        let urls = match read_urls_from_file(path) {
            Ok(u) => u,
            Err(e) => {
                eprintln!("❌ Failed to parse batch file: {e}\n");
                continue;
            }
        };

        if urls.is_empty() {
            println!("⚠️  The file contains no valid URLs (all lines are empty or commented out).\n");
            continue;
        }

        println!("\n📋 Found {} valid URLs in file.", urls.len());

        let preset_choices = vec![
            "🎬 Video (MP4 Video)",
            "🎵 Music (Opus Audio)",
            "📱 TikTok (Vertical MP4)",
            "⬅️  Back to Menu",
        ];

        let preset_sel = match Select::new("Select preset for this batch:", preset_choices).prompt() {
            Ok(s) => s,
            Err(InquireError::OperationCanceled) => return Ok(StepResult::Back),
            Err(InquireError::OperationInterrupted) => return Ok(StepResult::Exit),
            Err(e) => return Err(DlpError::PromptError(e)),
        };

        let preset_name = match preset_sel {
            "🎬 Video (MP4 Video)" => "video",
            "🎵 Music (Opus Audio)" => "music",
            "📱 TikTok (Vertical MP4)" => "tiktok",
            _ => return Ok(StepResult::Back),
        };

        let mut preset = preset_manager
            .get(preset_name)
            .ok_or_else(|| DlpError::PresetNotFound(preset_name.to_string()))?
            .clone();

        if preset.extract_audio {
            let want_lyrics = match Confirm::new("Download synced lyrics (.lrc) for batch items?")
                .with_default(preset.write_lyrics)
                .with_help_message("Automatically fetches .lrc synced lyrics from LRCLIB")
                .prompt()
            {
                Ok(ans) => ans,
                Err(InquireError::OperationCanceled) => return Ok(StepResult::Back),
                Err(InquireError::OperationInterrupted) => return Ok(StepResult::Exit),
                Err(e) => return Err(DlpError::PromptError(e)),
            };
            preset.write_lyrics = want_lyrics;
        }

        let confirm = match Confirm::new(&format!("Start batch downloading {} items?", urls.len()))
            .with_default(true)
            .prompt()
        {
            Ok(c) => c,
            Err(InquireError::OperationCanceled) => return Ok(StepResult::Back),
            Err(InquireError::OperationInterrupted) => return Ok(StepResult::Exit),
            Err(e) => return Err(DlpError::PromptError(e)),
        };

        if !confirm {
            return Ok(StepResult::Back);
        }

        run_batch(
            &urls,
            preset_manager,
            Some(&preset.name),
            None,
            Some(preset.write_lyrics),
            config.download_dir.as_deref(),
            false,
            None,
        )?;

        let _ = Text::new("Press Enter to return to main menu...").prompt();
        return Ok(StepResult::Value(()));
    }
}

fn run_download_wizard(preset: &Preset, config: &Config) -> Result<StepResult<()>> {
    loop {
        // Step 1: Prompt URL (with Back support)
        let url_prompt = Text::new("Enter media URL (or type 'b' to go back):")
            .with_help_message("Paste the video/audio link, or type 'b' / 'back' / press Esc to return to main menu")
            .prompt();

        let url = match url_prompt {
            Ok(val) => {
                let trimmed = val.trim().to_string();
                if trimmed.eq_ignore_ascii_case("b") || trimmed.eq_ignore_ascii_case("back") {
                    return Ok(StepResult::Back);
                }
                if trimmed.is_empty() {
                    println!("⚠️  URL cannot be empty.\n");
                    continue;
                }
                trimmed
            }
            Err(InquireError::OperationCanceled) => return Ok(StepResult::Back),
            Err(InquireError::OperationInterrupted) => return Ok(StepResult::Exit),
            Err(e) => return Err(DlpError::PromptError(e)),
        };

        // Step 2: Verify dependencies
        Downloader::verify_dependencies()?;

        // Step 3: Inspect metadata
        println!("\n🔍 Inspecting media metadata...");
        let meta = match Downloader::fetch_metadata(&url) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("❌ Failed to fetch metadata: {e}");
                let retry = match Confirm::new("Try entering URL again? (No = Back to main menu)")
                    .with_default(true)
                    .prompt()
                {
                    Ok(ans) => ans,
                    _ => false,
                };
                if retry {
                    continue;
                } else {
                    return Ok(StepResult::Back);
                }
            }
        };

        let orientation = meta.orientation();
        let available_res = meta.available_resolutions();
        let res_strings: Vec<String> = available_res.iter().map(|r| format!("{}p", r)).collect();
        let res_display = if res_strings.is_empty() {
            "Auto".to_string()
        } else {
            res_strings.join(", ")
        };

        println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("🎬 Title       : {}", meta.title);
        if let Some(uploader) = &meta.uploader {
            println!("👤 Uploader    : {}", uploader);
        }
        println!("⏱️  Duration    : {}", meta.format_duration());
        println!("📐 Orientation : {}", orientation.display_name());
        println!("📺 Available   : {}", res_display);
        println!("⚙️  Active Mode : {} [{}]", preset.name, if preset.extract_audio { "Audio" } else { "Video" });
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

        let mut active_preset = preset.clone();

        // Step 4: Quality & Options Selection
        let effective_quality = if active_preset.extract_audio {
            // Ask whether user wants synced lyrics for this music track
            let want_lyrics = match Confirm::new("Download synced lyrics (.lrc)?")
                .with_default(active_preset.write_lyrics)
                .with_help_message("Automatically queries and saves .lrc lyrics file from LRCLIB")
                .prompt()
            {
                Ok(ans) => ans,
                Err(InquireError::OperationCanceled) => return Ok(StepResult::Back),
                Err(InquireError::OperationInterrupted) => return Ok(StepResult::Exit),
                Err(e) => return Err(DlpError::PromptError(e)),
            };
            active_preset.write_lyrics = want_lyrics;

            QualityPreference::Best
        } else {
            let mut quality_choices = Vec::new();

            // Highest resolution size for Best Available
            let best_size_str = available_res
                .first()
                .and_then(|&r| meta.estimated_size_for_resolution(r))
                .map(|sz| format!(" (~{})", VideoMetadata::format_filesize(sz)))
                .unwrap_or_default();

            quality_choices.push(format!("🌟 Best Available{}", best_size_str));

            for &res in &available_res {
                let size_str = if let Some(sz) = meta.estimated_size_for_resolution(res) {
                    format!(" (~{})", VideoMetadata::format_filesize(sz))
                } else {
                    "".to_string()
                };

                let label = match res {
                    2160 => format!("📺 2160p (4K UHD){}", size_str),
                    1440 => format!("📺 1440p (2K QHD){}", size_str),
                    1080 => format!("📺 1080p (Full HD){}", size_str),
                    720 => format!("📺 720p (HD){}", size_str),
                    480 => format!("📺 480p (SD){}", size_str),
                    360 => format!("📺 360p{}", size_str),
                    other => format!("📺 {}p{}", other, size_str),
                };
                if !quality_choices.contains(&label) {
                    quality_choices.push(label);
                }
            }
            quality_choices.push("⬅️  Back to URL / Main Menu".to_string());

            let selected_quality = match Select::new("Select download quality:", quality_choices).prompt() {
                Ok(s) => s,
                Err(InquireError::OperationCanceled) => return Ok(StepResult::Back),
                Err(InquireError::OperationInterrupted) => return Ok(StepResult::Exit),
                Err(e) => return Err(DlpError::PromptError(e)),
            };

            let chosen_quality = if selected_quality == "⬅️  Back to URL / Main Menu" {
                continue;
            } else if selected_quality == "🌟 Best Available" {
                active_preset.effective_quality_preference(Some("best"), orientation)?
            } else {
                let height_str: String = selected_quality
                    .chars()
                    .skip_while(|c| !c.is_ascii_digit())
                    .take_while(|c| c.is_ascii_digit())
                    .collect();

                if let Ok(h) = height_str.parse::<u32>() {
                    active_preset.effective_quality_preference(Some(&h.to_string()), orientation)?
                } else {
                    active_preset.effective_quality_preference(Some("best"), orientation)?
                }
            };

            // Step 4b: Subtitle Selection for Video Mode (Always Separate .srt)
            let available_langs = meta.available_subtitle_languages();
            if !available_langs.is_empty() {
                let mut sub_choices = vec!["🚫 No Subtitles (None)".to_string()];
                if available_langs.contains(&"id".to_string()) {
                    sub_choices.push("🇮🇩 Indonesian (.srt)".to_string());
                }
                if available_langs.contains(&"en".to_string()) {
                    sub_choices.push("🇬🇧 English (.srt)".to_string());
                }
                if available_langs.contains(&"ko".to_string()) {
                    sub_choices.push("🇰🇷 Korean (.srt)".to_string());
                }
                if available_langs.contains(&"ja".to_string()) {
                    sub_choices.push("🇯🇵 Japanese (.srt)".to_string());
                }
                sub_choices.push("🌐 All Available Languages (.srt)".to_string());
                sub_choices.push("📝 Select Specific Language(s)...".to_string());
                sub_choices.push("⬅️  Back to Quality".to_string());

                let sub_sel = match Select::new("Download subtitles (Standalone .srt)?", sub_choices).prompt() {
                    Ok(s) => s,
                    Err(InquireError::OperationCanceled) => return Ok(StepResult::Back),
                    Err(InquireError::OperationInterrupted) => return Ok(StepResult::Exit),
                    Err(e) => return Err(DlpError::PromptError(e)),
                };

                if sub_sel == "⬅️  Back to Quality" {
                    continue;
                } else if sub_sel == "🚫 No Subtitles (None)" {
                    active_preset.write_lyrics = false;
                    active_preset.sub_langs = None;
                } else if sub_sel == "🇮🇩 Indonesian (.srt)" {
                    active_preset.write_lyrics = true;
                    active_preset.sub_langs = Some(vec!["id".to_string()]);
                } else if sub_sel == "🇬🇧 English (.srt)" {
                    active_preset.write_lyrics = true;
                    active_preset.sub_langs = Some(vec!["en".to_string()]);
                } else if sub_sel == "🇰🇷 Korean (.srt)" {
                    active_preset.write_lyrics = true;
                    active_preset.sub_langs = Some(vec!["ko".to_string()]);
                } else if sub_sel == "🇯🇵 Japanese (.srt)" {
                    active_preset.write_lyrics = true;
                    active_preset.sub_langs = Some(vec!["ja".to_string()]);
                } else if sub_sel == "🌐 All Available Languages (.srt)" {
                    active_preset.write_lyrics = true;
                    active_preset.sub_langs = Some(vec!["all".to_string()]);
                } else if sub_sel == "📝 Select Specific Language(s)..." {
                    let multi = match inquire::MultiSelect::new("Select languages to download (.srt):", available_langs.clone()).prompt() {
                        Ok(m) => m,
                        Err(InquireError::OperationCanceled) => return Ok(StepResult::Back),
                        Err(InquireError::OperationInterrupted) => return Ok(StepResult::Exit),
                        Err(e) => return Err(DlpError::PromptError(e)),
                    };
                    if !multi.is_empty() {
                        active_preset.write_lyrics = true;
                        active_preset.sub_langs = Some(multi);
                    } else {
                        active_preset.write_lyrics = false;
                        active_preset.sub_langs = None;
                    }
                }
            }

            chosen_quality
        };

        // Step 5: Confirm Download
        let confirm_prompt = Confirm::new("Start download now?")
            .with_default(true)
            .prompt();

        let confirm = match confirm_prompt {
            Ok(c) => c,
            Err(InquireError::OperationCanceled) => return Ok(StepResult::Back),
            Err(InquireError::OperationInterrupted) => return Ok(StepResult::Exit),
            Err(e) => return Err(DlpError::PromptError(e)),
        };

        if !confirm {
            let back_menu = match Confirm::new("Return to main menu? (No = re-enter URL)")
                .with_default(true)
                .prompt()
            {
                Ok(b) => b,
                _ => true,
            };
            if back_menu {
                return Ok(StepResult::Back);
            } else {
                continue;
            }
        }

        // Step 6: Execute Download
        let effective_output_dir = config.download_dir.as_deref();
        Downloader::download(&url, &active_preset, &effective_quality, effective_output_dir)?;

        // Step 7: Native Lyrics Auto-Fetcher (if enabled)
        if active_preset.write_lyrics {
            let base_dir = effective_output_dir.map(Path::new).unwrap_or_else(|| Path::new("."));
            LyricsFetcher::sync_lyrics_for_directory(base_dir, meta.uploader.as_deref());
        }

        return Ok(StepResult::Value(()));
    }
}
