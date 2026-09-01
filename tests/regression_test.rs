use dlp::classifier::SmartClassifier;
use dlp::lyrics::LyricsFetcher;
use dlp::metadata::VideoMetadata;
use dlp::normalizer::MetadataNormalizer;
use dlp::orientation::Orientation;
use dlp::provider::ProviderRegistry;
use dlp::tiktok::TikTokFallback;

#[test]
fn test_regression_url_edge_cases() {
    let registry = ProviderRegistry::new();

    // 1. YouTube variants
    let yt_variants = [
        "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
        "https://m.youtube.com/watch?v=dQw4w9WgXcQ",
        "https://youtu.be/dQw4w9WgXcQ",
        "https://youtu.be/dQw4w9WgXcQ?si=abcdef123456",
        "https://www.youtube.com/shorts/3iM_L16k630",
        "https://www.youtube.com/live/dQw4w9WgXcQ",
        "https://www.youtube.com/embed/dQw4w9WgXcQ",
    ];
    for url in yt_variants {
        let p = registry.find_provider(url);
        assert_eq!(p.name(), "YouTube", "Failed for URL variant: {}", url);
    }

    // 2. TikTok variants
    let tiktok_variants = [
        "https://www.tiktok.com/@creator/video/7123456789012345678",
        "https://m.tiktok.com/v/7123456789012345678.html",
        "https://vt.tiktok.com/ZSxxxxxxx/",
        "https://vm.tiktok.com/ZMxxxxxxx/",
        "https://www.tiktok.com/@user/video/7123456789012345678?is_from_webapp=1&sender_device=pc",
    ];
    for url in tiktok_variants {
        let p = registry.find_provider(url);
        assert_eq!(p.name(), "TikTok", "Failed for TikTok URL variant: {}", url);
        assert!(TikTokFallback::is_tiktok_url(url));
    }
}

#[test]
fn test_regression_smart_classification_matrix() {
    let music_meta = VideoMetadata::from_json(r#"{
        "id": "123",
        "title": "K-Pop Hit",
        "uploader": "Artist",
        "duration": 180.0,
        "width": 1920,
        "height": 1080
    }"#).unwrap();

    let preset = SmartClassifier::classify("https://music.youtube.com/watch?v=123", &music_meta);
    assert_eq!(preset.default_preset_name(), "music");

    let shorts_preset = SmartClassifier::classify("https://www.youtube.com/shorts/abc", &music_meta);
    assert_eq!(shorts_preset.default_preset_name(), "tiktok");

    let tiktok_preset = SmartClassifier::classify("https://www.tiktok.com/@user/video/123", &music_meta);
    assert_eq!(tiktok_preset.default_preset_name(), "tiktok");

    let vertical_meta = VideoMetadata::from_json(r#"{
        "id": "vertical_id",
        "title": "Vertical Video",
        "uploader": "Creator",
        "duration": 45.0,
        "width": 1080,
        "height": 1920
    }"#).unwrap();

    let vertical_preset = SmartClassifier::classify("https://example.com/video", &vertical_meta);
    assert_eq!(vertical_preset.default_preset_name(), "tiktok");

    let standard_preset = SmartClassifier::classify("https://www.youtube.com/watch?v=standard", &music_meta);
    assert_eq!(standard_preset.default_preset_name(), "video");
}

#[test]
fn test_regression_metadata_sanitizer_edge_cases() {
    let raw_title = "IVE - Accendio [Official MV] (Performance Video) (Color Coded Lyrics)";
    let cleaned = MetadataNormalizer::clean_title(raw_title);
    assert_eq!(cleaned, "IVE - Accendio");

    let raw_remaster = "Queen - Bohemian Rhapsody [Remastered] (4K Remaster)";
    let cleaned_remaster = MetadataNormalizer::clean_title(raw_remaster);
    assert_eq!(cleaned_remaster, "Queen - Bohemian Rhapsody");

    let track_num = "007 - Secret Agent Song (Official Video)";
    let cleaned_lyrics_title = LyricsFetcher::clean_title(track_num);
    assert_eq!(cleaned_lyrics_title, "Secret Agent Song");

    let illegal_filename = "Song: Title / Subtitle * Edition? <2024> | Extra \"Quotes\" \\ Backslash";
    let safe_filename = MetadataNormalizer::sanitize_filename(illegal_filename, "mp4");
    assert!(!safe_filename.contains(':'));
    assert!(!safe_filename.contains('/'));
    assert!(!safe_filename.contains('\\'));
    assert!(!safe_filename.contains('*'));
    assert!(!safe_filename.contains('?'));
    assert!(!safe_filename.contains('<'));
    assert!(!safe_filename.contains('>'));
    assert!(!safe_filename.contains('|'));
    assert!(!safe_filename.contains('"'));
}

#[test]
fn test_regression_orientation_boundary_conditions() {
    assert_eq!(Orientation::from_dimensions(1920, 1080), Orientation::Horizontal);
    assert_eq!(Orientation::from_dimensions(1080, 1920), Orientation::Vertical);
    assert_eq!(Orientation::from_dimensions(1080, 1080), Orientation::Square);
    assert_eq!(Orientation::from_dimensions(1081, 1080), Orientation::Horizontal);
    assert_eq!(Orientation::from_dimensions(1080, 1081), Orientation::Vertical);
}
