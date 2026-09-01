use dlp::quality::QualityPreference;
use dlp::recovery::{DiagnosticReport, FailureCategory};

#[test]
fn test_classify_transient_network_errors() {
    let timeout_err = "ERROR: [download] Got error: <urlopen error timed out>";
    let cat = FailureCategory::classify(timeout_err);
    assert!(matches!(cat, FailureCategory::Transient { .. }));
    assert!(cat.is_retryable());

    let conn_reset = "ERROR: [youtube] 12345: Unable to download API page: Connection reset by peer (caused by ConnectionResetError(104, 'Connection reset by peer'))";
    let cat2 = FailureCategory::classify(conn_reset);
    assert!(matches!(cat2, FailureCategory::Transient { .. }));

    let http_503 = "ERROR: [download] HTTP Error 503: Service Temporarily Unavailable";
    let cat3 = FailureCategory::classify(http_503);
    assert!(matches!(cat3, FailureCategory::Transient { .. }));
}

#[test]
fn test_classify_format_unavailable() {
    let format_err = "ERROR: [youtube] abc123: Requested format is not available. Use --list-formats for a list of available formats";
    let cat = FailureCategory::classify(format_err);
    assert!(matches!(cat, FailureCategory::FormatUnavailable { .. }));
    assert!(cat.is_retryable());

    let format_num_err = "ERROR: requested format 399 is not available";
    let cat2 = FailureCategory::classify(format_num_err);
    match cat2 {
        FailureCategory::FormatUnavailable { requested, .. } => {
            assert_eq!(requested, Some("399".to_string()));
        }
        _ => panic!("Expected FormatUnavailable variant"),
    }
}

#[test]
fn test_classify_bot_and_rate_limit() {
    let bot_err = "ERROR: [youtube] Sign in to confirm you're not a bot. This helps protect our community.";
    let cat = FailureCategory::classify(bot_err);
    assert!(matches!(cat, FailureCategory::BotBlockOrExtractor { .. }));
    assert!(cat.is_retryable());

    let http_429 = "ERROR: [youtube] HTTP Error 429: Too Many Requests";
    let cat2 = FailureCategory::classify(http_429);
    assert!(matches!(cat2, FailureCategory::BotBlockOrExtractor { .. }));
}

#[test]
fn test_classify_permanent_errors() {
    let private_err = "ERROR: [youtube] abc123: Private video. Sign in if you've been granted access to this video";
    let cat = FailureCategory::classify(private_err);
    assert!(matches!(cat, FailureCategory::Permanent { .. }));
    assert!(!cat.is_retryable());

    let deleted_err = "ERROR: [youtube] abc123: This video has been removed by the uploader";
    let cat2 = FailureCategory::classify(deleted_err);
    assert!(matches!(cat2, FailureCategory::Permanent { .. }));
    assert!(!cat2.is_retryable());

    let copyright_err = "ERROR: [youtube] abc123: Video unavailable. This video contains content from SME, who has blocked it on copyright grounds.";
    let cat3 = FailureCategory::classify(copyright_err);
    assert!(matches!(cat3, FailureCategory::Permanent { .. }));
    assert!(!cat3.is_retryable());
}

#[test]
fn test_classify_ffmpeg_processing() {
    let ffmpeg_err = "ERROR: Postprocessing: Error opening output files: Conversion failed!";
    let cat = FailureCategory::classify(ffmpeg_err);
    assert!(matches!(cat, FailureCategory::FFmpegProcessing { .. }));
}

#[test]
fn test_quality_fallback_stepping() {
    let q_4k = QualityPreference::SpecificHeight(2160);
    assert_eq!(q_4k.fallback_step(), Some(QualityPreference::SpecificHeight(1440)));

    let q_2k = QualityPreference::SpecificHeight(1440);
    assert_eq!(q_2k.fallback_step(), Some(QualityPreference::SpecificHeight(1080)));

    let q_1080 = QualityPreference::SpecificHeight(1080);
    assert_eq!(q_1080.fallback_step(), Some(QualityPreference::SpecificHeight(720)));

    let q_720 = QualityPreference::SpecificHeight(720);
    assert_eq!(q_720.fallback_step(), Some(QualityPreference::SpecificHeight(480)));

    let q_480 = QualityPreference::SpecificHeight(480);
    assert_eq!(q_480.fallback_step(), Some(QualityPreference::Best));

    let q_best = QualityPreference::Best;
    assert_eq!(q_best.fallback_step(), None);
}

#[test]
fn test_diagnostic_report_display() {
    let report = DiagnosticReport::new(
        FailureCategory::FormatUnavailable {
            requested: Some("399".to_string()),
            details: "Format not available".to_string(),
        },
        Some("Fallback: 1080p selected".to_string()),
    );

    let display_str = report.to_string();
    assert!(display_str.contains("Format Unavailable (399)"));
}
