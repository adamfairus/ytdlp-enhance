use dlp::scheduler::{PlatformCategory, ScheduledPlan};

#[test]
fn test_platform_detection() {
    assert_eq!(
        ScheduledPlan::detect_platform("https://www.youtube.com/watch?v=dQw4w9WgXcQ"),
        PlatformCategory::YouTube
    );
    assert_eq!(
        ScheduledPlan::detect_platform("https://youtu.be/dQw4w9WgXcQ"),
        PlatformCategory::YouTube
    );
    assert_eq!(
        ScheduledPlan::detect_platform("https://music.youtube.com/watch?v=abcdef"),
        PlatformCategory::YouTubeMusic
    );
    assert_eq!(
        ScheduledPlan::detect_platform("https://www.tiktok.com/@creator/video/1234567890"),
        PlatformCategory::TikTok
    );
    assert_eq!(
        ScheduledPlan::detect_platform("https://example.com/stream.mp4"),
        PlatformCategory::Generic
    );
}

#[test]
fn test_scheduled_plan_grouping() {
    let urls = vec![
        "https://www.youtube.com/watch?v=vid1".to_string(),
        "https://music.youtube.com/watch?v=music1".to_string(),
        "https://www.tiktok.com/@creator/video/111".to_string(),
        "https://www.youtube.com/watch?v=vid2".to_string(),
        "https://www.tiktok.com/@creator/video/222".to_string(),
    ];

    let plan = ScheduledPlan::from_urls(&urls);
    assert_eq!(plan.tasks.len(), 5);

    let yt_tasks = plan.grouped.get(&PlatformCategory::YouTube).unwrap();
    assert_eq!(yt_tasks.len(), 2);

    let music_tasks = plan.grouped.get(&PlatformCategory::YouTubeMusic).unwrap();
    assert_eq!(music_tasks.len(), 1);

    let tiktok_tasks = plan.grouped.get(&PlatformCategory::TikTok).unwrap();
    assert_eq!(tiktok_tasks.len(), 2);
}

#[test]
fn test_platform_concurrency_capping() {
    // TikTok should cap to 2 to prevent rate-limit bans
    assert_eq!(PlatformCategory::TikTok.max_safe_concurrency(5), 2);
    assert_eq!(PlatformCategory::TikTok.max_safe_concurrency(1), 1);

    // YouTube and Generic should allow desired concurrency
    assert_eq!(PlatformCategory::YouTube.max_safe_concurrency(5), 5);
    assert_eq!(PlatformCategory::Generic.max_safe_concurrency(8), 8);
}
