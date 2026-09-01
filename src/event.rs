use crate::classifier::Classification;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum DownloadEvent {
    MetadataFetched {
        url: String,
        title: String,
    },
    ClassificationCompleted {
        url: String,
        classification: Classification,
    },
    DownloadStarted {
        url: String,
        format: String,
    },
    Progress {
        percent: f32,
        speed: Option<String>,
        eta: Option<String>,
    },
    Retry {
        attempt: u32,
        max_retries: u32,
        reason: String,
    },
    Fallback {
        from_quality: String,
        to_quality: String,
    },
    Completed {
        url: String,
        output_path: String,
    },
    Failed {
        url: String,
        error: String,
    },
}

pub trait EventListener: Send + Sync {
    fn on_event(&self, event: &DownloadEvent);
}

pub struct JsonLinesEventListener;

impl JsonLinesEventListener {
    pub fn new() -> Self {
        Self
    }
}

impl Default for JsonLinesEventListener {
    fn default() -> Self {
        Self::new()
    }
}

impl EventListener for JsonLinesEventListener {
    fn on_event(&self, event: &DownloadEvent) {
        if let Ok(json) = serde_json::to_string(event) {
            println!("{}", json);
        }
    }
}

pub struct TracingEventListener;

impl TracingEventListener {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TracingEventListener {
    fn default() -> Self {
        Self::new()
    }
}

impl EventListener for TracingEventListener {
    fn on_event(&self, event: &DownloadEvent) {
        match event {
            DownloadEvent::MetadataFetched { url, title } => {
                tracing::info!(url = %url, title = %title, "Metadata fetched");
            }
            DownloadEvent::ClassificationCompleted { url, classification } => {
                tracing::info!(
                    url = %url,
                    media_type = ?classification.media_type,
                    confidence = %classification.confidence,
                    "Classification completed"
                );
            }
            DownloadEvent::DownloadStarted { url, format } => {
                tracing::info!(url = %url, format = %format, "Download started");
            }
            DownloadEvent::Progress { percent, speed, eta } => {
                tracing::info!(
                    percent = %percent,
                    speed = speed.as_deref().unwrap_or("unknown"),
                    eta = eta.as_deref().unwrap_or("unknown"),
                    "Download progress"
                );
            }
            DownloadEvent::Retry { attempt, max_retries, reason } => {
                tracing::warn!(
                    attempt = %attempt,
                    max_retries = %max_retries,
                    reason = %reason,
                    "Retrying download"
                );
            }
            DownloadEvent::Fallback { from_quality, to_quality } => {
                tracing::warn!(
                    from_quality = %from_quality,
                    to_quality = %to_quality,
                    "Falling back to lower quality"
                );
            }
            DownloadEvent::Completed { url, output_path } => {
                tracing::info!(url = %url, output_path = %output_path, "Download completed");
            }
            DownloadEvent::Failed { url, error } => {
                tracing::error!(url = %url, error = %error, "Download failed");
            }
        }
    }
}

pub fn init_logging(verbose: bool, quiet: bool, json_mode: bool) {
    let default_level = if quiet {
        "error"
    } else if verbose {
        "debug"
    } else {
        "info"
    };

    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(default_level));

    if json_mode {
        let _ = tracing_subscriber::fmt()
            .json()
            .with_env_filter(filter)
            .with_writer(std::io::stderr)
            .try_init();
    } else {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(std::io::stderr)
            .try_init();
    }
}

pub struct EventDispatcher {
    listeners: Vec<Box<dyn EventListener>>,
}

impl EventDispatcher {
    pub fn new() -> Self {
        Self {
            listeners: Vec::new(),
        }
    }

    pub fn register(&mut self, listener: Box<dyn EventListener>) {
        self.listeners.push(listener);
    }

    pub fn dispatch(&self, event: &DownloadEvent) {
        for listener in &self.listeners {
            listener.on_event(event);
        }
    }

    pub fn len(&self) -> usize {
        self.listeners.len()
    }

    pub fn is_empty(&self) -> bool {
        self.listeners.is_empty()
    }
}

impl Default for EventDispatcher {
    fn default() -> Self {
        Self::new()
    }
}
