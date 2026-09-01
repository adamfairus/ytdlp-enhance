use dlp::quality::QualityPreference;
use dlp::recovery::{
    DiagnosticReport, FailureCategory, FailureContext, RecoveryAction, RecoveryPolicy,
};

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

#[test]
fn test_failure_context_construction() {
    let ctx = FailureContext::new(Some(1), "test error", "download", 1);
    assert_eq!(ctx.exit_code, Some(1));
    assert_eq!(ctx.stderr, "test error");
    assert_eq!(ctx.operation, "download");
    assert_eq!(ctx.attempt, 1);
    assert_eq!(ctx.provider, None);

    let ctx_with_prov = ctx.with_provider("youtube");
    assert_eq!(ctx_with_prov.provider, Some("youtube".to_string()));
}

#[test]
fn test_recovery_policy_decide_retry_with_backoff() {
    let policy = RecoveryPolicy::default();
    let cat = FailureCategory::Transient {
        reason: "Connection reset by peer".to_string(),
    };

    // Attempt 1: backoff delay = 2 * 2^0 = 2 secs
    let ctx1 = FailureContext::new(Some(1), "Connection reset", "download", 1);
    let action1 = policy.decide(&ctx1, &cat, &QualityPreference::Best, None);
    assert_eq!(
        action1,
        RecoveryAction::RetryWithBackoff {
            delay_secs: 2,
            reason: "Connection reset by peer".to_string(),
        }
    );

    // Attempt 2: backoff delay = 2 * 2^1 = 4 secs
    let ctx2 = FailureContext::new(Some(1), "Connection reset", "download", 2);
    let action2 = policy.decide(&ctx2, &cat, &QualityPreference::Best, None);
    assert_eq!(
        action2,
        RecoveryAction::RetryWithBackoff {
            delay_secs: 4,
            reason: "Connection reset by peer".to_string(),
        }
    );

    // Attempt 3 (attempt == max_transient_retries): abort
    let ctx3 = FailureContext::new(Some(1), "Connection reset", "download", 3);
    let action3 = policy.decide(&ctx3, &cat, &QualityPreference::Best, None);
    assert_eq!(
        action3,
        RecoveryAction::Abort {
            reason: "Exceeded max retries (3): Connection reset by peer".to_string(),
        }
    );
}

#[test]
fn test_recovery_policy_decide_rotate_impersonation() {
    let policy = RecoveryPolicy::default();
    let cat = FailureCategory::BotBlockOrExtractor {
        reason: "Sign in to confirm you're not a bot".to_string(),
    };
    let ctx = FailureContext::new(Some(1), "bot block", "download", 1);

    // None -> safari-18
    let action1 = policy.decide(&ctx, &cat, &QualityPreference::Best, None);
    assert_eq!(
        action1,
        RecoveryAction::RotateImpersonation {
            client: "safari-18".to_string(),
            reason: "Sign in to confirm you're not a bot".to_string(),
        }
    );

    // safari-18 -> chrome-136
    let action2 = policy.decide(&ctx, &cat, &QualityPreference::Best, Some("safari-18"));
    assert_eq!(
        action2,
        RecoveryAction::RotateImpersonation {
            client: "chrome-136".to_string(),
            reason: "Sign in to confirm you're not a bot".to_string(),
        }
    );

    // chrome-136 -> firefox-135
    let action3 = policy.decide(&ctx, &cat, &QualityPreference::Best, Some("chrome-136"));
    assert_eq!(
        action3,
        RecoveryAction::RotateImpersonation {
            client: "firefox-135".to_string(),
            reason: "Sign in to confirm you're not a bot".to_string(),
        }
    );

    // firefox-135 -> Abort
    let action4 = policy.decide(&ctx, &cat, &QualityPreference::Best, Some("firefox-135"));
    assert_eq!(
        action4,
        RecoveryAction::Abort {
            reason: "All TLS impersonation options exhausted".to_string(),
        }
    );
}

#[test]
fn test_recovery_policy_decide_fallback_format() {
    let policy = RecoveryPolicy::default();
    let cat = FailureCategory::FormatUnavailable {
        requested: Some("1080".to_string()),
        details: "1080p stream unavailable".to_string(),
    };
    let ctx = FailureContext::new(Some(1), "format unavailable", "download", 1);

    // SpecificHeight(1080) -> fallback step is SpecificHeight(720)
    let action1 = policy.decide(&ctx, &cat, &QualityPreference::SpecificHeight(1080), None);
    assert_eq!(
        action1,
        RecoveryAction::FallbackFormat {
            next_quality: QualityPreference::SpecificHeight(720),
            reason: "Format unavailable, falling back from 1080p stream unavailable".to_string(),
        }
    );

    // Best has no further fallback step -> Abort
    let action2 = policy.decide(&ctx, &cat, &QualityPreference::Best, None);
    assert_eq!(
        action2,
        RecoveryAction::Abort {
            reason: "No further format fallbacks available".to_string(),
        }
    );
}

#[test]
fn test_recovery_policy_decide_skip_permanent() {
    let policy = RecoveryPolicy::default();
    let cat = FailureCategory::Permanent {
        reason: "This video is private".to_string(),
    };
    let ctx = FailureContext::new(Some(1), "Private video", "download", 1);

    let action = policy.decide(&ctx, &cat, &QualityPreference::Best, None);
    assert_eq!(
        action,
        RecoveryAction::SkipPermanent {
            reason: "This video is private".to_string(),
        }
    );
}

#[test]
fn test_recovery_policy_decide_ffmpeg_and_unknown() {
    let policy = RecoveryPolicy::default();
    let ctx = FailureContext::new(Some(1), "error", "download", 1);

    let cat_ffmpeg = FailureCategory::FFmpegProcessing {
        reason: "Conversion failed".to_string(),
    };
    let action1 = policy.decide(&ctx, &cat_ffmpeg, &QualityPreference::Best, None);
    assert_eq!(
        action1,
        RecoveryAction::Abort {
            reason: "Conversion failed".to_string(),
        }
    );

    let cat_unknown = FailureCategory::Unknown("Unexpected EOF".to_string());
    let action2 = policy.decide(&ctx, &cat_unknown, &QualityPreference::Best, None);
    assert_eq!(
        action2,
        RecoveryAction::Abort {
            reason: "Unexpected EOF".to_string(),
        }
    );
}

