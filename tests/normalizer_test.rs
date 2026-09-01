use dlp::metadata::VideoMetadata;
use dlp::normalizer::MetadataNormalizer;
use dlp::preset::Preset;

#[test]
fn test_clean_title_patterns() {
    assert_eq!(
        MetadataNormalizer::clean_title("IVE - LOVE DIVE (Official MV)"),
        "IVE - LOVE DIVE"
    );
    assert_eq!(
        MetadataNormalizer::clean_title("Taylor Swift - Anti-Hero (Official Music Video)"),
        "Taylor Swift - Anti-Hero"
    );
    assert_eq!(
        MetadataNormalizer::clean_title("NewJeans 'Ditto' [Performance Video]"),
        "NewJeans 'Ditto'"
    );
    assert_eq!(
        MetadataNormalizer::clean_title("Kenshi Yonezu - KICK BACK (Visualizer)"),
        "Kenshi Yonezu - KICK BACK"
    );
    assert_eq!(
        MetadataNormalizer::clean_title("YOASOBI - IDOL [Official Audio]"),
        "YOASOBI - IDOL"
    );
    assert_eq!(
        MetadataNormalizer::clean_title("Track Name (Color Coded Lyrics)"),
        "Track Name"
    );
}

#[test]
fn test_clean_artist() {
    assert_eq!(
        MetadataNormalizer::clean_artist("IVE - Topic"),
        "IVE"
    );
    assert_eq!(
        MetadataNormalizer::clean_artist("NewJeans – Topic"),
        "NewJeans"
    );
    assert_eq!(
        MetadataNormalizer::clean_artist("Taylor Swift"),
        "Taylor Swift"
    );
}

#[test]
fn test_sanitize_filename() {
    let raw = "What/If:This*Was?A<Forbidden>Filename|";
    let sanitized = MetadataNormalizer::sanitize_filename(raw, "mp4");
    assert_eq!(sanitized, "What_If_This_Was_A_Forbidden_Filename_.mp4");

    let simple = MetadataNormalizer::sanitize_filename("Clean Title", "opus");
    assert_eq!(simple, "Clean Title.opus");
}

#[test]
fn test_normalize_metadata() {
    let meta_json = r#"{
        "id": "A9EpZWrQ3dM",
        "title": "IVE - ELEVEN (Official MV)",
        "uploader": "StarshipTV - Topic",
        "duration": 184.0,
        "width": 1920,
        "height": 1080,
        "webpage_url": "https://music.youtube.com/watch?v=A9EpZWrQ3dM"
    }"#;

    let meta = VideoMetadata::from_json(meta_json).unwrap();
    let preset = Preset {
        name: "music".to_string(),
        description: None,
        container: "opus".to_string(),
        quality: "best".to_string(),
        max_horizontal: None,
        max_vertical: None,
        embed_metadata: true,
        embed_thumbnail: true,
        crop_thumbnail_square: true,
        extract_audio: true,
        audio_format: Some("opus".to_string()),
        audio_quality: Some("0".to_string()),
        write_lyrics: true,
        embed_lyrics: false,
        lyrics_format: Some("lrc".to_string()),
        sub_langs: None,
        clean_metadata: true,
        parse_music_tags: true,
        output_template: None,
        output_dir: None,
    };

    let norm = MetadataNormalizer::normalize("https://music.youtube.com/watch?v=A9EpZWrQ3dM", &meta, &preset);
    assert_eq!(norm.platform, "YouTube Music");
    assert_eq!(norm.content_type, "Music / Audio");
    assert_eq!(norm.clean_title, "IVE - ELEVEN");
    assert_eq!(norm.clean_artist, Some("StarshipTV".to_string()));
    assert_eq!(norm.sanitized_filename, "IVE - ELEVEN.opus");
}
