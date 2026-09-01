use std::fs;
use std::path::Path;
use crate::classifier::SmartClassifier;
use crate::downloader::Downloader;
use crate::error::{DlpError, Result};
use crate::lyrics::LyricsFetcher;
use crate::preset::{Preset, PresetManager};

#[derive(Debug, Clone, Default)]
pub struct BatchReport {
    pub total: usize,
    pub succeeded: usize,
    pub failed: Vec<(String, String)>,
}

impl BatchReport {
    pub fn print_summary(&self) {
        println!("\n╔══════════════════════════════════════════════════╗");
        println!("║               📊 BATCH DOWNLOAD REPORT           ║");
        println!("╠══════════════════════════════════════════════════╣");
        println!("║  Total URLs Processed : {:<24} ║", self.total);
        println!("║  ✅ Succeeded         : {:<24} ║", self.succeeded);
        println!("║  ❌ Failed            : {:<24} ║", self.failed.len());
        println!("╚══════════════════════════════════════════════════╝");

        if !self.failed.is_empty() {
            println!("\n⚠️  Failed URLs:");
            for (idx, (url, err)) in self.failed.iter().enumerate() {
                println!("{}. URL: {}", idx + 1, url);
                println!("   Reason: {}\n", err);
            }
        } else {
            println!("🎉 All items in the batch downloaded successfully!\n");
        }
    }
}

pub fn read_urls_from_file(path: &Path) -> Result<Vec<String>> {
    let content = fs::read_to_string(path).map_err(|e| {
        DlpError::ProcessExecution(std::io::Error::new(
            e.kind(),
            format!("Cannot read batch file '{}': {}", path.display(), e),
        ))
    })?;

    let urls: Vec<String> = content
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty() && !line.starts_with('#') && !line.starts_with("//"))
        .map(|line| line.to_string())
        .collect();

    Ok(urls)
}

pub fn resolve_inputs_to_urls(inputs: &[String]) -> Result<Vec<String>> {
    let mut all_urls = Vec::new();

    for input in inputs {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            continue;
        }

        let p = Path::new(trimmed);
        if p.is_file() {
            let file_urls = read_urls_from_file(p)?;
            all_urls.extend(file_urls);
        } else {
            all_urls.push(trimmed.to_string());
        }
    }

    Ok(all_urls)
}

pub fn run_batch(
    urls: &[String],
    preset_manager: &PresetManager,
    explicit_preset: Option<&str>,
    quality_override: Option<&str>,
    lyrics_override: Option<bool>,
    output_dir: Option<&str>,
) -> Result<BatchReport> {
    if urls.is_empty() {
        println!("⚠️  Batch queue is empty. No URLs to download.");
        return Ok(BatchReport::default());
    }

    Downloader::verify_dependencies()?;

    let total = urls.len();
    let mut report = BatchReport {
        total,
        succeeded: 0,
        failed: Vec::new(),
    };

    let mode_desc = explicit_preset.unwrap_or("Auto-Detect per Item");
    println!("\n🚀 Starting smart batch processing for {} media items...", total);
    println!("⚙️  Batch Strategy: {}\n", mode_desc);

    for (index, url) in urls.iter().enumerate() {
        let item_num = index + 1;
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("📦 [{}/{}] Processing: {}", item_num, total, url);

        // 1. Fetch metadata
        let meta = match Downloader::fetch_metadata(url) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("❌ Failed to inspect metadata: {e}");
                report.failed.push((url.clone(), format!("Metadata error: {e}")));
                continue;
            }
        };

        // 2. Resolve Preset (Explicit or Dynamic Smart Classification)
        let preset_name = if let Some(p) = explicit_preset {
            p.to_string()
        } else {
            let detected = SmartClassifier::classify(url, &meta);
            println!("🤖 Auto-Classified: {}", detected.display_label());
            detected.default_preset_name().to_string()
        };

        let mut preset: Preset = match preset_manager.get(&preset_name) {
            Some(p) => p.clone(),
            None => {
                report.failed.push((url.clone(), format!("Preset '{}' not found", preset_name)));
                continue;
            }
        };

        if let Some(want_lyrics) = lyrics_override {
            preset.write_lyrics = want_lyrics;
        }

        println!("🎬 Title       : {}", meta.title);
        println!("⏱️  Duration    : {}", meta.format_duration());
        println!("📐 Orientation : {}", meta.orientation().display_name());
        println!("⚙️  Active Preset: {} [{}]", preset.name, if preset.extract_audio { "Audio" } else { "Video" });

        // 3. Resolve quality
        let orientation = meta.orientation();
        let effective_quality = match preset.effective_quality_preference(quality_override, orientation) {
            Ok(q) => q,
            Err(e) => {
                report.failed.push((url.clone(), format!("Quality resolution error: {e}")));
                continue;
            }
        };

        if !preset.extract_audio {
            let available_res = meta.available_resolutions();
            if let Some(matched) = effective_quality.select_best_resolution(&available_res) {
                println!("🎯 Target Quality : {}p", matched);
            }
        }

        // 4. Execute download
        println!("⬇️  Downloading...");
        match Downloader::download(url, &preset, &effective_quality, output_dir) {
            Ok(_) => {
                println!("✅ Finished [{}/{}]", item_num, total);
                report.succeeded += 1;

                // Synchronize lyrics for music tracks
                if preset.write_lyrics {
                    let base_dir = output_dir.map(Path::new).unwrap_or_else(|| Path::new("."));
                    LyricsFetcher::sync_lyrics_for_directory(base_dir, meta.uploader.as_deref());
                }
            }
            Err(e) => {
                eprintln!("❌ Download failed: {e}");
                report.failed.push((url.clone(), format!("Download error: {e}")));
            }
        }
    }

    report.print_summary();
    Ok(report)
}
