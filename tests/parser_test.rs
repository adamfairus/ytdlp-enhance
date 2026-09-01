use dlp::metadata::VideoMetadata;
use dlp::orientation::Orientation;
use dlp::quality::QualityPreference;

#[test]
fn test_orientation_detection() {
    // Landscape (16:9, 4:3, etc.)
    assert_eq!(
        Orientation::from_dimensions(1920, 1080),
        Orientation::Horizontal
    );
    assert_eq!(
        Orientation::from_dimensions(3840, 2160),
        Orientation::Horizontal
    );
    assert_eq!(
        Orientation::from_dimensions(640, 480),
        Orientation::Horizontal
    );

    // Portrait / Vertical (Shorts, TikTok, Reels)
    assert_eq!(
        Orientation::from_dimensions(1080, 1920),
        Orientation::Vertical
    );
    assert_eq!(
        Orientation::from_dimensions(720, 1280),
        Orientation::Vertical
    );

    // Square (1:1)
    assert_eq!(
        Orientation::from_dimensions(1080, 1080),
        Orientation::Square
    );
}

#[test]
fn test_quality_preference_parsing() {
    assert_eq!(
        QualityPreference::parse("best").unwrap(),
        QualityPreference::Best
    );
    assert_eq!(
        QualityPreference::parse("BEST").unwrap(),
        QualityPreference::Best
    );
    assert_eq!(
        QualityPreference::parse("4k").unwrap(),
        QualityPreference::SpecificHeight(2160)
    );
    assert_eq!(
        QualityPreference::parse("2160p").unwrap(),
        QualityPreference::SpecificHeight(2160)
    );
    assert_eq!(
        QualityPreference::parse("1440p").unwrap(),
        QualityPreference::SpecificHeight(1440)
    );
    assert_eq!(
        QualityPreference::parse("qhd").unwrap(),
        QualityPreference::SpecificHeight(1440)
    );
    assert_eq!(
        QualityPreference::parse("1080").unwrap(),
        QualityPreference::SpecificHeight(1080)
    );
    assert_eq!(
        QualityPreference::parse("fhd").unwrap(),
        QualityPreference::SpecificHeight(1080)
    );
    assert_eq!(
        QualityPreference::parse("720p").unwrap(),
        QualityPreference::SpecificHeight(720)
    );
    assert_eq!(
        QualityPreference::parse("hd").unwrap(),
        QualityPreference::SpecificHeight(720)
    );
    assert_eq!(
        QualityPreference::parse("480p").unwrap(),
        QualityPreference::SpecificHeight(480)
    );
    assert_eq!(
        QualityPreference::parse("sd").unwrap(),
        QualityPreference::SpecificHeight(480)
    );

    // Invalid input
    assert!(QualityPreference::parse("invalid_tier").is_err());
}

#[test]
fn test_format_selector_generation() {
    let best = QualityPreference::Best;
    assert_eq!(best.to_format_selector(), "bestvideo+bestaudio/best");

    let fhd = QualityPreference::SpecificHeight(1080);
    assert_eq!(
        fhd.to_format_selector(),
        "bestvideo[height<=1080]+bestaudio/best[height<=1080]/best"
    );
}

#[test]
fn test_quality_resolution_matching() {
    let available = vec![2160, 1440, 1080, 720, 480];

    // Best takes top
    assert_eq!(
        QualityPreference::Best.select_best_resolution(&available),
        Some(2160)
    );

    // Exact match
    let q1080 = QualityPreference::SpecificHeight(1080);
    assert_eq!(q1080.select_best_resolution(&available), Some(1080));

    // Nearest <= target
    let q900 = QualityPreference::SpecificHeight(900);
    assert_eq!(q900.select_best_resolution(&available), Some(720));

    // Below lowest -> fallback to lowest available
    let q240 = QualityPreference::SpecificHeight(240);
    assert_eq!(q240.select_best_resolution(&available), Some(480));
}

#[test]
fn test_video_metadata_deserialization() {
    let raw_json = r#"{
        "id": "abc_xyz",
        "title": "Rust Tutorial for Beginners",
        "uploader": "Rustacean",
        "duration": 3665.0,
        "width": 2560,
        "height": 1440,
        "formats": [
            {"format_id": "1", "vcodec": "avc1", "acodec": "none", "height": 1440},
            {"format_id": "2", "vcodec": "avc1", "acodec": "none", "height": 1080},
            {"format_id": "3", "vcodec": "avc1", "acodec": "none", "height": 720},
            {"format_id": "4", "vcodec": "none", "acodec": "mp4a", "height": null}
        ]
    }"#;

    let meta = VideoMetadata::from_json(raw_json).expect("should parse JSON");
    assert_eq!(meta.title, "Rust Tutorial for Beginners");
    assert_eq!(meta.format_duration(), "01:01:05");
    assert_eq!(meta.orientation(), Orientation::Horizontal);
    assert_eq!(meta.available_resolutions(), vec![1440, 1080, 720]);
}
