use dlp::metadata::VideoMetadata;
use dlp::normalizer::DecisionTrace;
use dlp::preset::Preset;
use dlp::quality::QualityPreference;

#[test]
fn test_decision_trace_video_pipeline() {
    let meta_json = r#"{
        "id": "test_video",
        "title": "Amazing 4K Nature Video (Official Video)",
        "uploader": "NatureChannel",
        "duration": 120.0,
        "width": 3840,
        "height": 2160,
        "webpage_url": "https://www.youtube.com/watch?v=test_video"
    }"#;

    let meta = VideoMetadata::from_json(meta_json).unwrap();
    let preset = Preset {
        name: "video".to_string(),
        description: None,
        container: "mp4".to_string(),
        quality: "1080p".to_string(),
        max_horizontal: Some(1080),
        max_vertical: None,
        embed_metadata: true,
        embed_thumbnail: true,
        crop_thumbnail_square: false,
        extract_audio: false,
        audio_format: None,
        audio_quality: None,
        write_lyrics: false,
        embed_lyrics: false,
        lyrics_format: None,
        sub_langs: None,
        clean_metadata: true,
        parse_music_tags: false,
        output_template: None,
        output_dir: Some("Downloads".to_string()),
    };

    let effective_quality = QualityPreference::SpecificHeight(1080);
    let trace = DecisionTrace::build(
        "https://www.youtube.com/watch?v=test_video",
        &meta,
        &preset,
        &effective_quality,
        None,
    );

    assert_eq!(trace.platform, "YouTube");
    assert_eq!(trace.content_type, "Standard Video");
    assert_eq!(trace.resolution, "3840x2160");
    assert_eq!(trace.policy_name, "video");
    assert!(trace.policy_rules.iter().any(|r| r.contains("max_horizontal = 1080p")));
    assert!(trace.selected_format_desc.iter().any(|s| s.contains("container: mp4")));
    assert!(trace.post_processing_steps.iter().any(|s| s.contains("ffmpeg remux/merge to mp4")));
    assert_eq!(trace.output_filename, "Amazing 4K Nature Video.mp4");
}

#[test]
fn test_decision_trace_music_pipeline() {
    let meta_json = r#"{
        "id": "music_track",
        "title": "IVE - Accendio (Official MV)",
        "uploader": "IVE - Topic",
        "duration": 190.0,
        "width": 1920,
        "height": 1080,
        "webpage_url": "https://music.youtube.com/watch?v=music_track"
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

    let effective_quality = QualityPreference::Best;
    let trace = DecisionTrace::build(
        "https://music.youtube.com/watch?v=music_track",
        &meta,
        &preset,
        &effective_quality,
        Some("/custom/music/path"),
    );

    assert_eq!(trace.platform, "YouTube Music");
    assert_eq!(trace.content_type, "Music / Audio");
    assert!(trace.post_processing_steps.iter().any(|s| s.contains("ffmpeg 1:1 square cover crop")));
    assert!(trace.post_processing_steps.iter().any(|s| s.contains("LRCLIB synced lyrics")));
    assert_eq!(trace.output_dir, Some("/custom/music/path".to_string()));
    assert_eq!(trace.output_filename, "IVE - Accendio.opus");
}
