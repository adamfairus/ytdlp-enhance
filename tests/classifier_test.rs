use dlp::classifier::{MediaType, SmartClassifier};
use dlp::metadata::VideoMetadata;

#[test]
fn test_classify_youtube_music() {
    let meta = VideoMetadata {
        id: "abc".to_string(),
        title: "Song Title".to_string(),
        uploader: Some("Artist".to_string()),
        duration: Some(180.0),
        width: Some(1920),
        height: Some(1080),
        filesize: None,
        filesize_approx: None,
        formats: None,
        webpage_url: None,
        thumbnail: None,
        categories: Some(vec!["Music".to_string()]),
        extractor: Some("youtube".to_string()),
        subtitles: None,
        automatic_captions: None,
    };

    let classified = SmartClassifier::classify("https://music.youtube.com/watch?v=abc", &meta);
    assert_eq!(classified, MediaType::Music);
    assert_eq!(classified.default_preset_name(), "music");
}

#[test]
fn test_classify_tiktok_url() {
    let meta = VideoMetadata {
        id: "123".to_string(),
        title: "Dance".to_string(),
        uploader: Some("creator".to_string()),
        duration: Some(30.0),
        width: Some(1080),
        height: Some(1920),
        filesize: None,
        filesize_approx: None,
        formats: None,
        webpage_url: None,
        thumbnail: None,
        categories: None,
        extractor: Some("tiktok".to_string()),
        subtitles: None,
        automatic_captions: None,
    };

    let classified = SmartClassifier::classify("https://www.tiktok.com/@creator/video/123", &meta);
    assert_eq!(classified, MediaType::VerticalVideo);
    assert_eq!(classified.default_preset_name(), "tiktok");
}

#[test]
fn test_classify_vertical_shorts() {
    let meta = VideoMetadata {
        id: "short123".to_string(),
        title: "Shorts Video".to_string(),
        uploader: Some("channel".to_string()),
        duration: Some(45.0),
        width: Some(1080),
        height: Some(1920),
        filesize: None,
        filesize_approx: None,
        formats: None,
        webpage_url: None,
        thumbnail: None,
        categories: None,
        extractor: Some("youtube".to_string()),
        subtitles: None,
        automatic_captions: None,
    };

    let classified = SmartClassifier::classify("https://www.youtube.com/shorts/short123", &meta);
    assert_eq!(classified, MediaType::VerticalVideo);
}

#[test]
fn test_classify_standard_video() {
    let meta = VideoMetadata {
        id: "vid123".to_string(),
        title: "Long Tutorial".to_string(),
        uploader: Some("Tech".to_string()),
        duration: Some(1200.0),
        width: Some(1920),
        height: Some(1080),
        filesize: None,
        filesize_approx: None,
        formats: None,
        webpage_url: None,
        thumbnail: None,
        categories: Some(vec!["Science & Technology".to_string()]),
        extractor: Some("youtube".to_string()),
        subtitles: None,
        automatic_captions: None,
    };

    let classified = SmartClassifier::classify("https://www.youtube.com/watch?v=vid123", &meta);
    assert_eq!(classified, MediaType::StandardVideo);
    assert_eq!(classified.default_preset_name(), "video");
}
