use std::sync::{Arc, Mutex};

use dlp::classifier::{Classification, MediaType};
use dlp::event::{DownloadEvent, EventDispatcher, EventListener};

struct CollectorListener {
    events: Arc<Mutex<Vec<DownloadEvent>>>,
}

impl CollectorListener {
    fn new(events: Arc<Mutex<Vec<DownloadEvent>>>) -> Self {
        Self { events }
    }
}

impl EventListener for CollectorListener {
    fn on_event(&self, event: &DownloadEvent) {
        self.events.lock().unwrap().push(event.clone());
    }
}

#[test]
fn test_all_download_event_variants_creation() {
    // 1. MetadataFetched
    let meta_event = DownloadEvent::MetadataFetched {
        url: "https://www.youtube.com/watch?v=dQw4w9WgXcQ".to_string(),
        title: "Never Gonna Give You Up".to_string(),
    };
    match &meta_event {
        DownloadEvent::MetadataFetched { url, title } => {
            assert_eq!(url, "https://www.youtube.com/watch?v=dQw4w9WgXcQ");
            assert_eq!(title, "Never Gonna Give You Up");
        }
        _ => panic!("Expected MetadataFetched variant"),
    }

    // 2. ClassificationCompleted
    let classification = Classification {
        media_type: MediaType::Music,
        confidence: 0.98,
        reasons: vec!["Audio track detected".to_string(), "Topic channel".to_string()],
    };
    let class_event = DownloadEvent::ClassificationCompleted {
        url: "https://www.youtube.com/watch?v=dQw4w9WgXcQ".to_string(),
        classification: classification.clone(),
    };
    match &class_event {
        DownloadEvent::ClassificationCompleted {
            url,
            classification: c,
        } => {
            assert_eq!(url, "https://www.youtube.com/watch?v=dQw4w9WgXcQ");
            assert_eq!(c.media_type, MediaType::Music);
            assert_eq!(c.confidence, 0.98);
            assert_eq!(c.reasons.len(), 2);
        }
        _ => panic!("Expected ClassificationCompleted variant"),
    }

    // 3. DownloadStarted
    let start_event = DownloadEvent::DownloadStarted {
        url: "https://www.youtube.com/watch?v=dQw4w9WgXcQ".to_string(),
        format: "bestvideo+bestaudio/best".to_string(),
    };
    match &start_event {
        DownloadEvent::DownloadStarted { url, format } => {
            assert_eq!(url, "https://www.youtube.com/watch?v=dQw4w9WgXcQ");
            assert_eq!(format, "bestvideo+bestaudio/best");
        }
        _ => panic!("Expected DownloadStarted variant"),
    }

    // 4. Progress (with and without optional values)
    let progress_full = DownloadEvent::Progress {
        percent: 64.2,
        speed: Some("12.5MiB/s".to_string()),
        eta: Some("00:10".to_string()),
    };
    match &progress_full {
        DownloadEvent::Progress { percent, speed, eta } => {
            assert!((percent - 64.2).abs() < f32::EPSILON);
            assert_eq!(speed.as_deref(), Some("12.5MiB/s"));
            assert_eq!(eta.as_deref(), Some("00:10"));
        }
        _ => panic!("Expected Progress variant"),
    }

    let progress_none = DownloadEvent::Progress {
        percent: 0.0,
        speed: None,
        eta: None,
    };
    match &progress_none {
        DownloadEvent::Progress { percent, speed, eta } => {
            assert_eq!(*percent, 0.0);
            assert!(speed.is_none());
            assert!(eta.is_none());
        }
        _ => panic!("Expected Progress variant"),
    }

    // 5. Retry
    let retry_event = DownloadEvent::Retry {
        attempt: 2,
        max_retries: 3,
        reason: "Connection reset by peer".to_string(),
    };
    match &retry_event {
        DownloadEvent::Retry {
            attempt,
            max_retries,
            reason,
        } => {
            assert_eq!(*attempt, 2);
            assert_eq!(*max_retries, 3);
            assert_eq!(reason, "Connection reset by peer");
        }
        _ => panic!("Expected Retry variant"),
    }

    // 6. Fallback
    let fallback_event = DownloadEvent::Fallback {
        from_quality: "1080p".to_string(),
        to_quality: "720p".to_string(),
    };
    match &fallback_event {
        DownloadEvent::Fallback {
            from_quality,
            to_quality,
        } => {
            assert_eq!(from_quality, "1080p");
            assert_eq!(to_quality, "720p");
        }
        _ => panic!("Expected Fallback variant"),
    }

    // 7. Completed
    let completed_event = DownloadEvent::Completed {
        url: "https://www.youtube.com/watch?v=dQw4w9WgXcQ".to_string(),
        output_path: "/downloads/video.mp4".to_string(),
    };
    match &completed_event {
        DownloadEvent::Completed { url, output_path } => {
            assert_eq!(url, "https://www.youtube.com/watch?v=dQw4w9WgXcQ");
            assert_eq!(output_path, "/downloads/video.mp4");
        }
        _ => panic!("Expected Completed variant"),
    }

    // 8. Failed
    let failed_event = DownloadEvent::Failed {
        url: "https://www.youtube.com/watch?v=dQw4w9WgXcQ".to_string(),
        error: "HTTP 403 Forbidden".to_string(),
    };
    match &failed_event {
        DownloadEvent::Failed { url, error } => {
            assert_eq!(url, "https://www.youtube.com/watch?v=dQw4w9WgXcQ");
            assert_eq!(error, "HTTP 403 Forbidden");
        }
        _ => panic!("Expected Failed variant"),
    }

    // Verify clone, partial_eq, and debug formatting
    let cloned = completed_event.clone();
    assert_eq!(cloned, completed_event);
    assert_ne!(completed_event, failed_event);
    assert!(!format!("{:?}", completed_event).is_empty());
}

#[test]
fn test_event_dispatcher_register_and_dispatch() {
    let mut dispatcher = EventDispatcher::new();
    assert!(dispatcher.is_empty());
    assert_eq!(dispatcher.len(), 0);

    let received = Arc::new(Mutex::new(Vec::new()));
    let listener = CollectorListener::new(Arc::clone(&received));

    dispatcher.register(Box::new(listener));
    assert_eq!(dispatcher.len(), 1);
    assert!(!dispatcher.is_empty());

    let event1 = DownloadEvent::MetadataFetched {
        url: "https://example.com".to_string(),
        title: "Test Title".to_string(),
    };
    let event2 = DownloadEvent::Progress {
        percent: 50.0,
        speed: Some("5MB/s".to_string()),
        eta: Some("00:05".to_string()),
    };
    let event3 = DownloadEvent::Completed {
        url: "https://example.com".to_string(),
        output_path: "/tmp/out.mp4".to_string(),
    };

    dispatcher.dispatch(&event1);
    dispatcher.dispatch(&event2);
    dispatcher.dispatch(&event3);

    let events = received.lock().unwrap();
    assert_eq!(events.len(), 3);
    assert_eq!(events[0], event1);
    assert_eq!(events[1], event2);
    assert_eq!(events[2], event3);
}

#[test]
fn test_event_dispatcher_multiple_listeners() {
    let mut dispatcher = EventDispatcher::default();
    assert!(dispatcher.is_empty());

    let received_a = Arc::new(Mutex::new(Vec::new()));
    let received_b = Arc::new(Mutex::new(Vec::new()));

    dispatcher.register(Box::new(CollectorListener::new(Arc::clone(&received_a))));
    dispatcher.register(Box::new(CollectorListener::new(Arc::clone(&received_b))));
    assert_eq!(dispatcher.len(), 2);

    let event = DownloadEvent::Failed {
        url: "https://fail.com".to_string(),
        error: "Network unreachable".to_string(),
    };

    dispatcher.dispatch(&event);

    assert_eq!(received_a.lock().unwrap().as_slice(), &[event.clone()]);
    assert_eq!(received_b.lock().unwrap().as_slice(), &[event]);
}

#[test]
fn test_event_dispatcher_empty_dispatch_no_panic() {
    let dispatcher = EventDispatcher::new();
    let event = DownloadEvent::Retry {
        attempt: 1,
        max_retries: 3,
        reason: "Rate limited".to_string(),
    };

    // Dispatching to zero listeners should be a clean no-op and never panic
    dispatcher.dispatch(&event);
    assert_eq!(dispatcher.len(), 0);
    assert!(dispatcher.is_empty());
}

#[test]
fn test_event_listener_trait_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Box<dyn EventListener>>();
}
