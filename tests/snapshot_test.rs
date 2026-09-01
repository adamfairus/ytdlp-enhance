use dlp::metadata::VideoMetadata;
use dlp::normalizer::DecisionTrace;
use dlp::preset::Preset;
use dlp::quality::QualityPreference;
use dlp::recovery::{DiagnosticReport, FailureCategory};
use dlp::scheduler::{PlatformCategory, ScheduledPlan};

#[test]
fn test_decision_trace_terminal_snapshot() {
    let meta_json = r#"{
        "id": "snapshot_id",
        "title": "Test Snapshot Title (Official Music Video)",
        "uploader": "Official Channel - Topic",
        "duration": 215.0,
        "width": 1920,
        "height": 1080
    }"#;
    let meta = VideoMetadata::from_json(meta_json).unwrap();

    let preset_toml = r#"
        name = "video"
        container = "mp4"
        quality = "best"
        max_horizontal = 1080
    "#;
    let preset = Preset::from_toml(preset_toml).unwrap();

    let trace = DecisionTrace::build(
        "https://www.youtube.com/watch?v=snapshot_id",
        &meta,
        &preset,
        &QualityPreference::SpecificHeight(1080),
        None,
    );

    assert_eq!(trace.platform, "YouTube");
    assert_eq!(trace.content_type, "Standard Video");
    assert_eq!(trace.resolution, "1920x1080");
    assert_eq!(trace.duration, "03:35");
    assert_eq!(trace.policy_name, "video");
    assert_eq!(trace.output_filename, "Test Snapshot Title.mp4");
}

#[test]
fn test_diagnostic_report_terminal_snapshot() {
    let report = DiagnosticReport::new(
        FailureCategory::BotBlockOrExtractor {
            reason: "Sign in to confirm you're not a bot".to_string(),
        },
        Some("Engaging Safari-18 TLS fingerprint rotation...".to_string()),
    );

    let display_str = report.to_string();
    assert!(display_str.contains("Anti-Bot/Rate-Limit"));
    assert!(report.category.is_retryable());
}

#[test]
fn test_scheduler_plan_snapshot() {
    let urls = vec![
        "https://www.youtube.com/watch?v=1".to_string(),
        "https://www.tiktok.com/@u/video/2".to_string(),
        "https://music.youtube.com/watch?v=3".to_string(),
    ];

    let plan = ScheduledPlan::from_urls(&urls);

    assert_eq!(plan.tasks.len(), 3);
    assert_eq!(plan.grouped.get(&PlatformCategory::YouTube).unwrap().len(), 1);
    assert_eq!(plan.grouped.get(&PlatformCategory::TikTok).unwrap().len(), 1);
    assert_eq!(plan.grouped.get(&PlatformCategory::YouTubeMusic).unwrap().len(), 1);
    assert_eq!(PlatformCategory::TikTok.max_safe_concurrency(4), 2);
}
