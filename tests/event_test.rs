use std::sync::{Arc, Mutex};

use dlp::classifier::{Classification, MediaType};
use dlp::engine::YtDlpEngine;
use dlp::event::{
    init_logging, DownloadEvent, EventDispatcher, EventListener, JsonLinesEventListener,
    TracingEventListener,
};
use dlp::preset::Preset;
use dlp::quality::QualityPreference;

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

#[test]
fn test_download_with_dispatcher_lifecycle_events() {
    let mut dispatcher = EventDispatcher::new();
    let events = Arc::new(Mutex::new(Vec::new()));
    dispatcher.register(Box::new(CollectorListener::new(Arc::clone(&events))));

    let preset = Preset::default();
    let quality = QualityPreference::Best;
    let test_url = "invalid_scheme://event_lifecycle_test";

    let result = YtDlpEngine::download_with_dispatcher(
        test_url,
        &preset,
        &quality,
        None,
        Some(&dispatcher),
    );

    // Unrecoverable scheme error produces Err
    assert!(result.is_err());

    let recorded = events.lock().unwrap();
    // Must record at least DownloadStarted and Failed
    assert!(recorded.len() >= 2);

    // 1. First event must be DownloadStarted
    match &recorded[0] {
        DownloadEvent::DownloadStarted { url, format } => {
            assert_eq!(url, test_url);
            assert_eq!(format, "bestvideo+bestaudio/best");
        }
        other => panic!("Expected DownloadStarted as first event, got {:?}", other),
    }

    // 2. Final event must be Failed
    match recorded.last().unwrap() {
        DownloadEvent::Failed { url, error } => {
            assert_eq!(url, test_url);
            assert!(!error.is_empty());
        }
        other => panic!("Expected Failed as last event, got {:?}", other),
    }
}

#[test]
fn test_download_event_json_serialization_all_variants() {
    // 1. MetadataFetched
    let ev1 = DownloadEvent::MetadataFetched {
        url: "https://example.com/watch?v=abc".to_string(),
        title: "Test Title".to_string(),
    };
    let s1 = serde_json::to_string(&ev1).unwrap();
    let v1: serde_json::Value = serde_json::from_str(&s1).unwrap();
    assert_eq!(v1["event"], "metadata_fetched");
    assert_eq!(v1["url"], "https://example.com/watch?v=abc");
    assert_eq!(v1["title"], "Test Title");

    // 2. ClassificationCompleted
    let ev2 = DownloadEvent::ClassificationCompleted {
        url: "https://example.com/watch?v=music".to_string(),
        classification: Classification {
            media_type: MediaType::Music,
            confidence: 0.96,
            reasons: vec!["Audio-only track".to_string(), "Topic channel".to_string()],
        },
    };
    let s2 = serde_json::to_string(&ev2).unwrap();
    let v2: serde_json::Value = serde_json::from_str(&s2).unwrap();
    assert_eq!(v2["event"], "classification_completed");
    assert_eq!(v2["url"], "https://example.com/watch?v=music");
    assert_eq!(v2["classification"]["media_type"], "music");
    assert!((v2["classification"]["confidence"].as_f64().unwrap() - 0.96).abs() < 0.01);
    assert_eq!(v2["classification"]["reasons"].as_array().unwrap().len(), 2);

    // 3. DownloadStarted
    let ev3 = DownloadEvent::DownloadStarted {
        url: "https://example.com/video".to_string(),
        format: "bestvideo+bestaudio/best".to_string(),
    };
    let s3 = serde_json::to_string(&ev3).unwrap();
    let v3: serde_json::Value = serde_json::from_str(&s3).unwrap();
    assert_eq!(v3["event"], "download_started");
    assert_eq!(v3["format"], "bestvideo+bestaudio/best");

    // 4. Progress
    let ev4 = DownloadEvent::Progress {
        percent: 75.5,
        speed: Some("15.2MiB/s".to_string()),
        eta: Some("00:04".to_string()),
    };
    let s4 = serde_json::to_string(&ev4).unwrap();
    let v4: serde_json::Value = serde_json::from_str(&s4).unwrap();
    assert_eq!(v4["event"], "progress");
    assert!((v4["percent"].as_f64().unwrap() - 75.5).abs() < 0.01);
    assert_eq!(v4["speed"], "15.2MiB/s");
    assert_eq!(v4["eta"], "00:04");

    // 5. Retry
    let ev5 = DownloadEvent::Retry {
        attempt: 2,
        max_retries: 4,
        reason: "HTTP 429 Too Many Requests".to_string(),
    };
    let s5 = serde_json::to_string(&ev5).unwrap();
    let v5: serde_json::Value = serde_json::from_str(&s5).unwrap();
    assert_eq!(v5["event"], "retry");
    assert_eq!(v5["attempt"], 2);
    assert_eq!(v5["max_retries"], 4);
    assert_eq!(v5["reason"], "HTTP 429 Too Many Requests");

    // 6. Fallback
    let ev6 = DownloadEvent::Fallback {
        from_quality: "1440p".to_string(),
        to_quality: "1080p".to_string(),
    };
    let s6 = serde_json::to_string(&ev6).unwrap();
    let v6: serde_json::Value = serde_json::from_str(&s6).unwrap();
    assert_eq!(v6["event"], "fallback");
    assert_eq!(v6["from_quality"], "1440p");
    assert_eq!(v6["to_quality"], "1080p");

    // 7. Completed
    let ev7 = DownloadEvent::Completed {
        url: "https://example.com/video".to_string(),
        output_path: "/tmp/downloads/video.mp4".to_string(),
    };
    let s7 = serde_json::to_string(&ev7).unwrap();
    let v7: serde_json::Value = serde_json::from_str(&s7).unwrap();
    assert_eq!(v7["event"], "completed");
    assert_eq!(v7["output_path"], "/tmp/downloads/video.mp4");

    // 8. Failed
    let ev8 = DownloadEvent::Failed {
        url: "https://example.com/video".to_string(),
        error: "Sign in required".to_string(),
    };
    let s8 = serde_json::to_string(&ev8).unwrap();
    let v8: serde_json::Value = serde_json::from_str(&s8).unwrap();
    assert_eq!(v8["event"], "failed");
    assert_eq!(v8["error"], "Sign in required");
}

#[test]
fn test_download_event_json_roundtrip() {
    let events = vec![
        DownloadEvent::MetadataFetched {
            url: "https://test.com".to_string(),
            title: "Test".to_string(),
        },
        DownloadEvent::ClassificationCompleted {
            url: "https://test.com".to_string(),
            classification: Classification {
                media_type: MediaType::VerticalVideo,
                confidence: 0.99,
                reasons: vec!["TikTok URL".to_string()],
            },
        },
        DownloadEvent::DownloadStarted {
            url: "https://test.com".to_string(),
            format: "best".to_string(),
        },
        DownloadEvent::Progress {
            percent: 50.0,
            speed: None,
            eta: None,
        },
        DownloadEvent::Retry {
            attempt: 1,
            max_retries: 3,
            reason: "timeout".to_string(),
        },
        DownloadEvent::Fallback {
            from_quality: "4k".to_string(),
            to_quality: "1080p".to_string(),
        },
        DownloadEvent::Completed {
            url: "https://test.com".to_string(),
            output_path: "out.mp4".to_string(),
        },
        DownloadEvent::Failed {
            url: "https://test.com".to_string(),
            error: "fatal".to_string(),
        },
    ];

    for event in events {
        let serialized = serde_json::to_string(&event).expect("Serialization failed");
        let deserialized: DownloadEvent =
            serde_json::from_str(&serialized).expect("Deserialization failed");
        assert_eq!(event, deserialized);
    }
}

#[test]
fn test_json_lines_event_listener_emitting_valid_json() {
    let listener = JsonLinesEventListener::new();
    let events = vec![
        DownloadEvent::MetadataFetched {
            url: "https://example.com/1".to_string(),
            title: "Video 1".to_string(),
        },
        DownloadEvent::ClassificationCompleted {
            url: "https://example.com/2".to_string(),
            classification: Classification {
                media_type: MediaType::StandardVideo,
                confidence: 0.88,
                reasons: vec!["16:9 ratio".to_string()],
            },
        },
        DownloadEvent::DownloadStarted {
            url: "https://example.com/3".to_string(),
            format: "1080p".to_string(),
        },
        DownloadEvent::Progress {
            percent: 100.0,
            speed: Some("20MiB/s".to_string()),
            eta: Some("00:00".to_string()),
        },
        DownloadEvent::Retry {
            attempt: 1,
            max_retries: 3,
            reason: "temporary connection reset".to_string(),
        },
        DownloadEvent::Fallback {
            from_quality: "1080p".to_string(),
            to_quality: "720p".to_string(),
        },
        DownloadEvent::Completed {
            url: "https://example.com/4".to_string(),
            output_path: "/out.mp4".to_string(),
        },
        DownloadEvent::Failed {
            url: "https://example.com/5".to_string(),
            error: "ExtractorError".to_string(),
        },
    ];

    for ev in &events {
        // on_event must execute smoothly and serialize valid JSON
        listener.on_event(ev);
        let s = serde_json::to_string(ev).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert!(parsed.is_object());
        assert!(parsed.get("event").is_some());
    }
}

#[test]
fn test_tracing_event_listener_handles_all_variants() {
    let listener = TracingEventListener::new();
    let events = vec![
        DownloadEvent::MetadataFetched {
            url: "https://example.com/meta".to_string(),
            title: "Meta Title".to_string(),
        },
        DownloadEvent::ClassificationCompleted {
            url: "https://example.com/class".to_string(),
            classification: Classification {
                media_type: MediaType::Music,
                confidence: 0.92,
                reasons: vec!["Opus format".to_string()],
            },
        },
        DownloadEvent::DownloadStarted {
            url: "https://example.com/start".to_string(),
            format: "bestvideo".to_string(),
        },
        DownloadEvent::Progress {
            percent: 33.3,
            speed: Some("5MB/s".to_string()),
            eta: Some("00:15".to_string()),
        },
        DownloadEvent::Retry {
            attempt: 1,
            max_retries: 2,
            reason: "timeout".to_string(),
        },
        DownloadEvent::Fallback {
            from_quality: "2160p".to_string(),
            to_quality: "1440p".to_string(),
        },
        DownloadEvent::Completed {
            url: "https://example.com/complete".to_string(),
            output_path: "/path/video.mp4".to_string(),
        },
        DownloadEvent::Failed {
            url: "https://example.com/failed".to_string(),
            error: "network unavailable".to_string(),
        },
    ];

    for ev in &events {
        // Calling on_event must dispatch cleanly to tracing without panics
        listener.on_event(ev);
    }
}

#[test]
fn test_init_logging_helper() {
    // Calling init_logging with various parameter flags must not panic
    init_logging(false, false, false);
    init_logging(true, false, false);
    init_logging(false, true, false);
    init_logging(false, false, true);
}

#[test]
fn test_cli_logging_and_json_flags() {
    use clap::Parser;
    use dlp::cli::Cli;

    // Test --verbose / -v flag
    let cli_v = Cli::try_parse_from(["dlp", "-v", "https://example.com"]).unwrap();
    assert!(cli_v.verbose);
    assert!(!cli_v.quiet);
    assert!(!cli_v.json);

    // Test --quiet / -q flag
    let cli_q = Cli::try_parse_from(["dlp", "-q", "https://example.com"]).unwrap();
    assert!(!cli_q.verbose);
    assert!(cli_q.quiet);
    assert!(!cli_q.json);

    // Test --json flag
    let cli_json = Cli::try_parse_from(["dlp", "--json", "https://example.com"]).unwrap();
    assert!(!cli_json.verbose);
    assert!(!cli_json.quiet);
    assert!(cli_json.json);

    // Test -Q for quality
    let cli_qual = Cli::try_parse_from(["dlp", "-Q", "1080", "https://example.com"]).unwrap();
    assert_eq!(cli_qual.quality.as_deref(), Some("1080"));

    // Test subcommand with flags
    let cli_sub = Cli::try_parse_from(["dlp", "video", "https://example.com", "-v", "-q", "--json"]).unwrap();
    assert!(cli_sub.verbose);
    assert!(cli_sub.quiet);
    assert!(cli_sub.json);
}


