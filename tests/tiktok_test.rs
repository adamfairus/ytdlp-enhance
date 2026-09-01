use dlp::tiktok::TikTokFallback;

#[test]
fn test_is_tiktok_url() {
    assert!(TikTokFallback::is_tiktok_url("https://www.tiktok.com/@ryoun_e/video/7680213075999427860"));
    assert!(TikTokFallback::is_tiktok_url("https://vt.tiktok.com/ZSjXq123/"));
    assert!(TikTokFallback::is_tiktok_url("https://vm.tiktok.com/ZM8xY/"));
    assert!(!TikTokFallback::is_tiktok_url("https://www.youtube.com/watch?v=dQw4w9WgXcQ"));
}

#[test]
fn test_clean_tiktok_url() {
    let raw = "https://www.tiktok.com/@ryoun_e/video/7680213075999427860?is_from_webapp=1&sender_device=pc&web_id=7679184587146872337";
    assert_eq!(
        TikTokFallback::clean_url(raw),
        "https://www.tiktok.com/@ryoun_e/video/7680213075999427860"
    );
}
