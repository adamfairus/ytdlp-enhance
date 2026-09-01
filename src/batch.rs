use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime};
use serde::{Deserialize, Serialize};
use crate::classifier::SmartClassifier;
use crate::downloader::Downloader;
use crate::error::{DlpError, Result};
#[allow(unused_imports)]
use crate::event::{DownloadEvent, EventDispatcher, EventListener};
use crate::lyrics::LyricsFetcher;
use crate::preset::{Preset, PresetManager};
#[allow(unused_imports)]
use crate::scheduler::{PlatformCategory, ScheduledPlan, TaskPriority, TaskScheduler, TaskState};
use crate::throttle::PlatformRateLimiter;

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

fn process_single_item(
    url: &str,
    preset_manager: &PresetManager,
    explicit_preset: Option<&str>,
    quality_override: Option<&str>,
    lyrics_override: Option<bool>,
    output_dir: Option<&str>,
    worker_label: Option<usize>,
) -> Result<String> {
    let prefix = match worker_label {
        Some(w) => format!("[Worker {}] ", w),
        None => "".to_string(),
    };

    // 1. Fetch metadata
    println!("{}🔍 Fetching metadata for: {}", prefix, url);
    let meta = Downloader::fetch_metadata(url)?;

    // 2. Resolve Preset
    let preset_name = if let Some(p) = explicit_preset {
        p.to_string()
    } else {
        let detected = SmartClassifier::classify(url, &meta);
        detected.default_preset_name().to_string()
    };

    let mut preset: Preset = preset_manager
        .get(&preset_name)
        .ok_or_else(|| DlpError::PresetNotFound(preset_name.clone()))?
        .clone();

    if let Some(want_lyrics) = lyrics_override {
        preset.write_lyrics = want_lyrics;
    }

    // 3. Resolve quality
    let orientation = meta.orientation();
    let effective_quality = preset.effective_quality_preference(quality_override, orientation)?;

    // 4. Download
    println!("{}⬇️  Downloading: {}", prefix, meta.title);
    Downloader::download(url, &preset, &effective_quality, output_dir)?;

    // 5. Lyrics sync for music
    if preset.write_lyrics {
        let base_dir = output_dir.map(Path::new).unwrap_or_else(|| Path::new("."));
        LyricsFetcher::sync_lyrics_for_directory(base_dir, meta.uploader.as_deref());
    }

    Ok(meta.title)
}

fn run_batch_sequential(
    urls: &[String],
    preset_manager: &PresetManager,
    explicit_preset: Option<&str>,
    quality_override: Option<&str>,
    lyrics_override: Option<bool>,
    output_dir: Option<&str>,
    resume: bool,
    checkpoint_path: &Path,
) -> Result<BatchReport> {
    let mut checkpoint = if resume {
        let loaded = BatchCheckpoint::load_from_path(checkpoint_path);
        println!("💾 Loaded checkpoint from '{}' (Resumed mode)", checkpoint_path.display());
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
    println!("⚙️  Batch Strategy: {}", mode_desc);
    if resume {
        println!("🔄 Mode          : Resume / Checkpoint Enabled");
    }
    println!();

    for (index, url) in urls.iter().enumerate() {
        let item_num = index + 1;

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

        match process_single_item(
            url,
            preset_manager,
            explicit_preset,
            quality_override,
            lyrics_override,
            output_dir,
            None,
        ) {
            Ok(title) => {
                println!("✅ Finished [{}/{}]: {}", item_num, total, title);
                checkpoint.mark_completed(url, &title);
                let _ = checkpoint.save_to_path(checkpoint_path);
                report.succeeded += 1;
            }
            Err(e) => {
                let err_msg = format!("{e}");
                eprintln!("❌ Download failed: {err_msg}");
                checkpoint.mark_failed(url, &err_msg);
                let _ = checkpoint.save_to_path(checkpoint_path);
                report.failed.push((url.clone(), err_msg));
            }
        }
    }

    report.print_summary();
    Ok(report)
}

pub fn run_batch_parallel(
    urls: &[String],
    preset_manager: &PresetManager,
    explicit_preset: Option<&str>,
    quality_override: Option<&str>,
    lyrics_override: Option<bool>,
    output_dir: Option<&str>,
    resume: bool,
    checkpoint_path: &Path,
    concurrency: usize,
) -> Result<BatchReport> {
    run_batch_parallel_with_dispatcher(
        urls,
        preset_manager,
        explicit_preset,
        quality_override,
        lyrics_override,
        output_dir,
        resume,
        checkpoint_path,
        concurrency,
        None,
    )
}

pub fn run_batch_parallel_with_dispatcher(
    urls: &[String],
    preset_manager: &PresetManager,
    explicit_preset: Option<&str>,
    quality_override: Option<&str>,
    lyrics_override: Option<bool>,
    output_dir: Option<&str>,
    resume: bool,
    checkpoint_path: &Path,
    concurrency: usize,
    dispatcher: Option<Arc<EventDispatcher>>,
) -> Result<BatchReport> {
    let total = urls.len();
    let num_workers = concurrency.min(total).max(1);

    println!("⚡ Controlled Parallel Queue: {} worker threads active", num_workers);
    if resume {
        println!("🔄 Mode                    : Resume / Checkpoint Enabled");
    }
    println!();

    let scheduler = Arc::new(Mutex::new(TaskScheduler::from_urls(urls, num_workers)));

    let checkpoint = Arc::new(Mutex::new(if resume {
        BatchCheckpoint::load_from_path(checkpoint_path)
    } else {
        BatchCheckpoint::new()
    }));

    let report = Arc::new(Mutex::new(BatchReport {
        total,
        succeeded: 0,
        skipped: 0,
        failed: Vec::new(),
    }));

    let checkpoint_file = checkpoint_path.to_path_buf();
    let explicit_preset = explicit_preset.map(|s| s.to_string());
    let quality_override = quality_override.map(|s| s.to_string());
    let output_dir = output_dir.map(|s| s.to_string());
    let preset_mgr = Arc::new(preset_manager.clone());

    // Platform-aware rate limiter & concurrency throttling
    let rate_limiter = Arc::new(PlatformRateLimiter::new());

    let mut handles = Vec::new();

    for worker_id in 0..num_workers {
        let scheduler = Arc::clone(&scheduler);
        let cp = Arc::clone(&checkpoint);
        let rep = Arc::clone(&report);
        let pm = Arc::clone(&preset_mgr);
        let cp_file = checkpoint_file.clone();
        let exp_p = explicit_preset.clone();
        let q_ov = quality_override.clone();
        let out_d = output_dir.clone();
        let rate_limiter = Arc::clone(&rate_limiter);
        let disp = dispatcher.clone();

        let handle = thread::spawn(move || {
            loop {
                let (task, is_finished) = {
                    let mut sched = scheduler.lock().unwrap();
                    let finished = sched.is_finished();
                    let next = sched.next_runnable();
                    (next, finished)
                };

                if is_finished {
                    break;
                }

                let task = match task {
                    Some(t) => t,
                    None => {
                        // Workers wait briefly if other workers are holding platform concurrency slots
                        thread::sleep(Duration::from_millis(50));
                        continue;
                    }
                };

                let item_num = task.id;
                let url = task.url.clone();

                // 1. Check resume skip
                if resume {
                    let is_done = {
                        let lock = cp.lock().unwrap();
                        lock.is_completed(&url)
                    };
                    if is_done {
                        println!("[Worker {}] 📦 [{}/{}] ⏭️  Skipping (Already Downloaded): {}", worker_id + 1, item_num, total, url);
                        let mut rep_lock = rep.lock().unwrap();
                        rep_lock.skipped += 1;
                        {
                            let mut sched = scheduler.lock().unwrap();
                            sched.complete_task(task.id);
                        }
                        continue;
                    }
                }

                println!("[Worker {}] 📦 [{}/{}] Processing: {}", worker_id + 1, item_num, total, url);

                // 2. Throttle & rate-limit request based on platform category
                rate_limiter.acquire_permit(task.platform);

                // 3. Process single item
                let res = process_single_item(
                    &url,
                    &pm,
                    exp_p.as_deref(),
                    q_ov.as_deref(),
                    lyrics_override,
                    out_d.as_deref(),
                    Some(worker_id + 1),
                );

                match res {
                    Ok(title) => {
                        println!("[Worker {}] ✅ Finished [{}/{}]: {}", worker_id + 1, item_num, total, title);
                        let mut cp_lock = cp.lock().unwrap();
                        cp_lock.mark_completed(&url, &title);
                        let _ = cp_lock.save_to_path(&cp_file);

                        let mut rep_lock = rep.lock().unwrap();
                        rep_lock.succeeded += 1;

                        {
                            let mut sched = scheduler.lock().unwrap();
                            sched.complete_task(task.id);
                        }

                        if let Some(ref d) = disp {
                            d.dispatch(&DownloadEvent::Completed {
                                url: url.clone(),
                                output_path: title,
                            });
                        }
                    }
                    Err(e) => {
                        let err_msg = format!("{e}");
                        eprintln!("[Worker {}] ❌ Failed [{}/{}]: {}", worker_id + 1, item_num, total, err_msg);
                        let mut cp_lock = cp.lock().unwrap();
                        cp_lock.mark_failed(&url, &err_msg);
                        let _ = cp_lock.save_to_path(&cp_file);

                        let mut rep_lock = rep.lock().unwrap();
                        rep_lock.failed.push((url.clone(), err_msg.clone()));

                        {
                            let mut sched = scheduler.lock().unwrap();
                            sched.fail_task(task.id, false, 0);
                        }

                        if let Some(ref d) = disp {
                            d.dispatch(&DownloadEvent::Failed {
                                url: url.clone(),
                                error: err_msg,
                            });
                        }
                    }
                }
            }
        });

        handles.push(handle);
    }

    for h in handles {
        let _ = h.join();
    }

    let final_report = report.lock().unwrap().clone();
    final_report.print_summary();
    Ok(final_report)
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
    concurrency: usize,
) -> Result<BatchReport> {
    run_batch_with_dispatcher(
        urls,
        preset_manager,
        explicit_preset,
        quality_override,
        lyrics_override,
        output_dir,
        resume,
        checkpoint_path,
        concurrency,
        None,
    )
}

pub fn run_batch_with_dispatcher(
    urls: &[String],
    preset_manager: &PresetManager,
    explicit_preset: Option<&str>,
    quality_override: Option<&str>,
    lyrics_override: Option<bool>,
    output_dir: Option<&str>,
    resume: bool,
    checkpoint_path: Option<&Path>,
    concurrency: usize,
    dispatcher: Option<Arc<EventDispatcher>>,
) -> Result<BatchReport> {
    if urls.is_empty() {
        println!("⚠️  Batch queue is empty. No URLs to download.");
        return Ok(BatchReport::default());
    }

    Downloader::verify_dependencies()?;

    let plan = ScheduledPlan::from_urls(urls);
    let effective_concurrency = concurrency.max(1);
    plan.print_summary(effective_concurrency);

    let default_cp_path = PathBuf::from(".dlp_checkpoint.json");
    let resolved_cp_path = checkpoint_path.unwrap_or(&default_cp_path);

    if effective_concurrency > 1 {
        run_batch_parallel_with_dispatcher(
            urls,
            preset_manager,
            explicit_preset,
            quality_override,
            lyrics_override,
            output_dir,
            resume,
            resolved_cp_path,
            effective_concurrency,
            dispatcher,
        )
    } else {
        run_batch_sequential(
            urls,
            preset_manager,
            explicit_preset,
            quality_override,
            lyrics_override,
            output_dir,
            resume,
            resolved_cp_path,
        )
    }
}
