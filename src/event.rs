use crate::classifier::Classification;

#[derive(Debug, Clone, PartialEq)]
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
