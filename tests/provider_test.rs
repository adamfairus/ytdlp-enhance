use dlp::provider::ProviderRegistry;

#[test]
fn test_provider_detection_routing() {
    let registry = ProviderRegistry::new();

    let tiktok = registry.find_provider("https://www.tiktok.com/@creator/video/123456789");
    assert_eq!(tiktok.name(), "TikTok");

    let tiktok_short = registry.find_provider("https://vt.tiktok.com/ZSxxxxxx/");
    assert_eq!(tiktok_short.name(), "TikTok");

    let youtube_standard = registry.find_provider("https://www.youtube.com/watch?v=dQw4w9WgXcQ");
    assert_eq!(youtube_standard.name(), "YouTube");

    let youtube_shortlink = registry.find_provider("https://youtu.be/dQw4w9WgXcQ");
    assert_eq!(youtube_shortlink.name(), "YouTube");

    let youtube_music = registry.find_provider("https://music.youtube.com/watch?v=A9EpZWrQ3dM");
    assert_eq!(youtube_music.name(), "YouTube");

    let generic = registry.find_provider("https://vimeo.com/12345678");
    assert_eq!(generic.name(), "Generic");
}

#[test]
fn test_provider_concurrency_caps() {
    let registry = ProviderRegistry::new();

    let tiktok = registry.find_provider("https://www.tiktok.com/@user/video/111");
    assert_eq!(tiktok.max_safe_concurrency(5), 2);
    assert_eq!(tiktok.max_safe_concurrency(1), 1);

    let youtube = registry.find_provider("https://www.youtube.com/watch?v=222");
    assert_eq!(youtube.max_safe_concurrency(5), 5);
    assert_eq!(youtube.max_safe_concurrency(10), 10);

    let generic = registry.find_provider("https://example.com/media.mp4");
    assert_eq!(generic.max_safe_concurrency(8), 8);
}

#[test]
fn test_provider_registry_listing() {
    let registry = ProviderRegistry::new();
    let names = registry.list_providers();

    assert_eq!(names, vec!["TikTok", "YouTube", "Generic"]);
}
