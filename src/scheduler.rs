use std::collections::HashMap;

/// Categorization of supported media platforms for intelligent scheduling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlatformCategory {
    YouTube,
    YouTubeMusic,
    TikTok,
    Generic,
}

impl PlatformCategory {
    pub fn display_name(&self) -> &'static str {
        match self {
            PlatformCategory::YouTube => "YouTube Video",
            PlatformCategory::YouTubeMusic => "YouTube Music",
            PlatformCategory::TikTok => "TikTok / Shorts",
            PlatformCategory::Generic => "Generic / Direct Media",
        }
    }

    /// Enforces platform-aware concurrency caps (e.g. TikTok/TikWM rate protection).
    pub fn max_safe_concurrency(&self, desired_concurrency: usize) -> usize {
        match self {
            // TikWM API is sensitive to concurrent requests from the same IP; cap to 2 workers
            PlatformCategory::TikTok => desired_concurrency.clamp(1, 2),
            PlatformCategory::YouTube | PlatformCategory::YouTubeMusic | PlatformCategory::Generic => {
                desired_concurrency.max(1)
            }
        }
    }
}

/// A scheduled task unit for batch execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledTask {
    pub id: usize,
    pub url: String,
    pub platform: PlatformCategory,
}

/// Smart scheduling plan for batch execution.
#[derive(Debug, Clone, Default)]
pub struct ScheduledPlan {
    pub tasks: Vec<ScheduledTask>,
    pub grouped: HashMap<PlatformCategory, Vec<ScheduledTask>>,
}

impl ScheduledPlan {
    pub fn from_urls(urls: &[String]) -> Self {
        let mut tasks = Vec::new();
        let mut grouped: HashMap<PlatformCategory, Vec<ScheduledTask>> = HashMap::new();

        for (idx, url) in urls.iter().enumerate() {
            let platform = Self::detect_platform(url);
            let task = ScheduledTask {
                id: idx + 1,
                url: url.clone(),
                platform,
            };
            grouped.entry(platform).or_default().push(task.clone());
            tasks.push(task);
        }

        Self { tasks, grouped }
    }

    pub fn detect_platform(url: &str) -> PlatformCategory {
        let lower = url.to_lowercase();
        if lower.contains("tiktok.com") || lower.contains("douyin.com") {
            PlatformCategory::TikTok
        } else if lower.contains("music.youtube.com") {
            PlatformCategory::YouTubeMusic
        } else if lower.contains("youtube.com") || lower.contains("youtu.be") {
            PlatformCategory::YouTube
        } else {
            PlatformCategory::Generic
        }
    }

    pub fn print_summary(&self, effective_concurrency: usize) {
        println!("\n╔══════════════════════════════════════════════════╗");
        println!("║             📋 SMART QUEUE SCHEDULER             ║");
        println!("╠══════════════════════════════════════════════════╣");
        println!("║  Total Scheduled Items : {:<24}║", self.tasks.len());
        println!("║  Target Concurrency    : {:<24}║", effective_concurrency);
        println!("╠══════════════════════════════════════════════════╣");
        for (plat, list) in &self.grouped {
            let line = format!("• {}: {} item(s)", plat.display_name(), list.len());
            println!("║  {:<48}║", truncate_str(&line, 48));
        }
        println!("╚══════════════════════════════════════════════════╝\n");
    }
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        let mut truncated: String = s.chars().take(max_len.saturating_sub(3)).collect();
        truncated.push_str("...");
        truncated
    }
}
