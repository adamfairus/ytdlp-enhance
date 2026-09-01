use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use serde::{Deserialize, Serialize};
use crate::classifier::SmartClassifier;
use crate::downloader::Downloader;
use crate::error::{DlpError, Result};
use crate::lyrics::LyricsFetcher;
use crate::preset::{Preset, PresetManager};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ItemStatus {
    Completed { title: String, timestamp: String },
    Failed { error: String, timestamp: String },
    Pending,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BatchCheckpoint {
    pub items: HashMap<String, ItemStatus>,
}

impl BatchCheckpoint {
    pub fn new() -> Self {
        Self {
            items: HashMap::new(),
        }
    }

    pub fn load_from_path(path: &Path) -> Self {
        if path.exists() {
            if let Ok(content) = fs::read_to_string(path) {
                if let Ok(cp) = serde_json::from_str(&content) {
                    return cp;
                }
            }
        }
        Self::new()
    }

    pub fn save_to_path(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                let _ = fs::create_dir_all(parent);
            }
        }
        let json = serde_json::to_string_pretty(self)?;
        fs::write(path, json)?;
        Ok(())
    }

    pub fn mark_completed(&mut self, url: &str, title: &str) {
        self.items.insert(
            url.to_string(),
            ItemStatus::Completed {
                title: title.to_string(),
                timestamp: current_timestamp(),
            },
        );
    }

    pub fn mark_failed(&mut self, url: &str, error: &str) {
        self.items.insert(
            url.to_string(),
            ItemStatus::Failed {
                error: error.to_string(),
                timestamp: current_timestamp(),
            },
        );
    }

    pub fn is_completed(&self, url: &str) -> bool {
        matches!(self.items.get(url), Some(ItemStatus::Completed { .. }))
    }

    pub fn get_status(&self, url: &str) -> Option<&ItemStatus> {
        self.items.get(url)
    }
}

fn current_timestamp() -> String {
    match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
        Ok(d) => format!("unix:{}", d.as_secs()),
        Err(_) => "unknown".to_string(),
    }
}

pub fn determine_checkpoint_path(inputs: &[String], output_dir: Option<&str>) -> PathBuf {
    if let Some(first_file) = inputs.iter().find(|i| Path::new(i).is_file()) {
        PathBuf::from(format!("{}.dlp_checkpoint.json", first_file))
    } else if let Some(dir) = output_dir {
        Path::new(dir).join(".dlp_checkpoint.json")
    } else {
        PathBuf::from(".dlp_checkpoint.json")
    }
}

#[derive(Debug, Clone, Default)]
pub struct BatchReport {
    pub total: usize,
    pub succeeded: usize,
    pub skipped: usize,
    pub failed: Vec<(String, String)>,
}

impl BatchReport {
    pub fn print_summary(&self) {
        println!("\n╔══════════════════════════════════════════════════╗");
        println!("║               📊 BATCH DOWNLOAD REPORT           ║");
        println!("╠══════════════════════════════════════════════════╣");
        println!("║  Total URLs Processed : {:<24} ║", self.total);
        println!("║  ✅ Succeeded         : {:<24} ║", self.succeeded);
        if self.skipped > 0 {
            println!("║  ⏭️  Skipped (Resumed)  : {:<24} ║", self.skipped);
        }
        println!("║  ❌ Failed            : {:<24} ║", self.failed.len());
        println!("╚══════════════════════════════════════════════════╝");

        if !self.failed.is_empty() {
            println!("\n⚠️  Failed URLs:");
            for (idx, (url, err)) in self.failed.iter().enumerate() {
                println!("{}. URL: {}", idx + 1, url);
                println!("   Reason: {}\n", err);
            }
        } else {
            println!("🎉 All items in the batch processed successfully!\n");
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
    resume: bool,
    checkpoint_path: Option<&Path>,
) -> Result<BatchReport> {
    if urls.is_empty() {
        println!("⚠️  Batch queue is empty. No URLs to download.");
        return Ok(BatchReport::default());
    }

    Downloader::verify_dependencies()?;

    let default_cp_path = PathBuf::from(".dlp_checkpoint.json");
    let resolved_cp_path = checkpoint_path.unwrap_or(&default_cp_path);

    let mut checkpoint = if resume {
        let loaded = BatchCheckpoint::load_from_path(resolved_cp_path);
        println!("💾 Loaded checkpoint from '{}' (Resumed mode)", resolved_cp_path.display());
        loaded
    } else {
        BatchCheckpoint::new()
    };

    let total = urls.len();
    let mut report = BatchReport {
        total,
        succeeded: 0,
        skipped: 0,
        failed: Vec::new(),
    };

    let mode_desc = explicit_preset.unwrap_or("Auto-Detect per Item");
    println!("\n🚀 Starting smart batch processing for {} media items...", total);
    println!("⚙️  Batch Strategy: {}", mode_desc);
    if resume {
        println!("🔄 Mode          : Resume / Checkpoint Enabled");
    }
    println!();

    for (index, url) in urls.iter().enumerate() {
        let item_num = index + 1;

        // Check if item was previously completed when resuming
        if resume && checkpoint.is_completed(url) {
            let title = match checkpoint.get_status(url) {
                Some(ItemStatus::Completed { title, .. }) => title.as_str(),
                _ => "Unknown",
            };
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("📦 [{}/{}] ⏭️  Skipping (Already Downloaded): {}", item_num, total, url);
            println!("   Title: {}", title);
            report.skipped += 1;
            continue;
        }

        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("📦 [{}/{}] Processing: {}", item_num, total, url);

        // 1. Fetch metadata
        let meta = match Downloader::fetch_metadata(url) {
            Ok(m) => m,
            Err(e) => {
                let err_msg = format!("Metadata error: {e}");
                eprintln!("❌ Failed to inspect metadata: {e}");
                checkpoint.mark_failed(url, &err_msg);
                let _ = checkpoint.save_to_path(resolved_cp_path);
                report.failed.push((url.clone(), err_msg));
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
                let err_msg = format!("Preset '{}' not found", preset_name);
                checkpoint.mark_failed(url, &err_msg);
                let _ = checkpoint.save_to_path(resolved_cp_path);
                report.failed.push((url.clone(), err_msg));
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
                let err_msg = format!("Quality resolution error: {e}");
                checkpoint.mark_failed(url, &err_msg);
                let _ = checkpoint.save_to_path(resolved_cp_path);
                report.failed.push((url.clone(), err_msg));
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
                checkpoint.mark_completed(url, &meta.title);
                let _ = checkpoint.save_to_path(resolved_cp_path);
                report.succeeded += 1;

                // Synchronize lyrics for music tracks
                if preset.write_lyrics {
                    let base_dir = output_dir.map(Path::new).unwrap_or_else(|| Path::new("."));
                    LyricsFetcher::sync_lyrics_for_directory(base_dir, meta.uploader.as_deref());
                }
            }
            Err(e) => {
                let err_msg = format!("Download error: {e}");
                eprintln!("❌ Download failed: {e}");
                checkpoint.mark_failed(url, &err_msg);
                let _ = checkpoint.save_to_path(resolved_cp_path);
                report.failed.push((url.clone(), err_msg));
            }
        }
    }

    report.print_summary();
    Ok(report)
}
