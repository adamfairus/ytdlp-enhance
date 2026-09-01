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

/// Task execution state within the scheduler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Pending,
    Running,
    Completed,
    Retrying { attempt: u32 },
    Failed,
}

impl Default for TaskState {
    fn default() -> Self {
        Self::Pending
    }
}

/// Priority level for scheduled tasks. Higher values are prioritized.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TaskPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Urgent = 3,
}

impl Default for TaskPriority {
    fn default() -> Self {
        Self::Normal
    }
}

/// A scheduled task unit for batch execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledTask {
    pub id: usize,
    pub url: String,
    pub platform: PlatformCategory,
    pub priority: TaskPriority,
    pub state: TaskState,
    pub retry_count: u32,
}

impl ScheduledTask {
    pub fn new(id: usize, url: impl Into<String>, platform: PlatformCategory, priority: TaskPriority) -> Self {
        Self {
            id,
            url: url.into(),
            platform,
            priority,
            state: TaskState::Pending,
            retry_count: 0,
        }
    }
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
                priority: TaskPriority::Normal,
                state: TaskState::Pending,
                retry_count: 0,
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

    pub fn to_scheduler(&self, global_concurrency: usize) -> TaskScheduler {
        let mut scheduler = TaskScheduler::new(global_concurrency);
        for task in &self.tasks {
            scheduler.add_task(&task.url, task.priority);
        }
        scheduler
    }
}

/// Dynamic, priority-aware, platform-bounded task scheduler engine.
#[derive(Debug, Clone)]
pub struct TaskScheduler {
    pub global_concurrency: usize,
    pub tasks: Vec<ScheduledTask>,
    next_task_id: usize,
}

impl TaskScheduler {
    /// Creates a new TaskScheduler with the specified global concurrency limit.
    pub fn new(global_concurrency: usize) -> Self {
        Self {
            global_concurrency: global_concurrency.max(1),
            tasks: Vec::new(),
            next_task_id: 1,
        }
    }

    /// Initializes a TaskScheduler from a list of URLs with default Normal priority.
    pub fn from_urls(urls: &[String], global_concurrency: usize) -> Self {
        let mut scheduler = Self::new(global_concurrency);
        for url in urls {
            scheduler.add_task(url, TaskPriority::Normal);
        }
        scheduler
    }

    /// Adds a new task with specified priority, returning its assigned task ID.
    pub fn add_task(&mut self, url: &str, priority: TaskPriority) -> usize {
        let id = self.next_task_id;
        self.next_task_id += 1;
        let platform = ScheduledPlan::detect_platform(url);
        let task = ScheduledTask {
            id,
            url: url.to_string(),
            platform,
            priority,
            state: TaskState::Pending,
            retry_count: 0,
        };
        self.tasks.push(task);
        id
    }

    /// Retrieves the next highest-priority runnable task adhering to platform & global concurrency bounds.
    /// Transitions the returned task's state to `TaskState::Running`.
    pub fn next_runnable(&mut self) -> Option<ScheduledTask> {
        if self.running_count() >= self.global_concurrency {
            return None;
        }

        let mut selected_idx: Option<usize> = None;

        for (idx, task) in self.tasks.iter().enumerate() {
            let is_runnable_state = matches!(task.state, TaskState::Pending | TaskState::Retrying { .. });
            if !is_runnable_state {
                continue;
            }

            let platform_running = self
                .tasks
                .iter()
                .filter(|t| t.state == TaskState::Running && t.platform == task.platform)
                .count();

            if platform_running >= task.platform.max_safe_concurrency(self.global_concurrency) {
                continue;
            }

            match selected_idx {
                None => {
                    selected_idx = Some(idx);
                }
                Some(best_idx) => {
                    // Pick higher priority; in case of ties, preserve FIFO (earlier index)
                    if task.priority > self.tasks[best_idx].priority {
                        selected_idx = Some(idx);
                    }
                }
            }
        }

        if let Some(idx) = selected_idx {
            self.tasks[idx].state = TaskState::Running;
            Some(self.tasks[idx].clone())
        } else {
            None
        }
    }

    /// Transitions a task to `TaskState::Completed`.
    pub fn complete_task(&mut self, task_id: usize) {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == task_id) {
            task.state = TaskState::Completed;
        }
    }

    /// Handles task failure: retries if retryable and below `max_retries`, else transitions to `TaskState::Failed`.
    pub fn fail_task(&mut self, task_id: usize, retryable: bool, max_retries: u32) {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == task_id) {
            if retryable && task.retry_count < max_retries {
                task.retry_count += 1;
                task.state = TaskState::Retrying {
                    attempt: task.retry_count,
                };
            } else {
                task.state = TaskState::Failed;
            }
        }
    }

    /// Returns true when all tasks are either Completed or Failed (no Pending, Running, or Retrying).
    pub fn is_finished(&self) -> bool {
        self.tasks
            .iter()
            .all(|t| matches!(t.state, TaskState::Completed | TaskState::Failed))
    }

    /// Returns number of currently running tasks.
    pub fn running_count(&self) -> usize {
        self.tasks
            .iter()
            .filter(|t| t.state == TaskState::Running)
            .count()
    }

    /// Returns number of successfully completed tasks.
    pub fn completed_count(&self) -> usize {
        self.tasks
            .iter()
            .filter(|t| t.state == TaskState::Completed)
            .count()
    }

    /// Returns number of failed tasks.
    pub fn failed_count(&self) -> usize {
        self.tasks
            .iter()
            .filter(|t| t.state == TaskState::Failed)
            .count()
    }

    /// Finds a task by ID.
    pub fn get_task(&self, task_id: usize) -> Option<&ScheduledTask> {
        self.tasks.iter().find(|t| t.id == task_id)
    }

    /// Finds a mutable task by ID.
    pub fn get_task_mut(&mut self, task_id: usize) -> Option<&mut ScheduledTask> {
        self.tasks.iter_mut().find(|t| t.id == task_id)
    }

    /// Returns a slice of all scheduled tasks.
    pub fn tasks(&self) -> &[ScheduledTask] {
        &self.tasks
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
