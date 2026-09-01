use dlp::orientation::Orientation;
use dlp::preset::{Preset, PresetManager};
use dlp::quality::QualityPreference;

#[test]
fn test_preset_manager_loads_defaults() {
    let manager = PresetManager::load_all();

    let video_preset = manager.get("video").expect("video preset should exist");
    assert_eq!(video_preset.name, "video");
    assert_eq!(video_preset.container, "mp4");
    assert_eq!(video_preset.max_horizontal, Some(2160));
    assert_eq!(video_preset.max_vertical, Some(1440));
    assert!(!video_preset.extract_audio);

    let music_preset = manager.get("music").expect("music preset should exist");
    assert_eq!(music_preset.name, "music");
    assert_eq!(music_preset.container, "opus");
    assert!(music_preset.extract_audio);
    assert_eq!(music_preset.audio_format.as_deref(), Some("opus"));
    assert!(music_preset.crop_thumbnail_square);
    assert!(music_preset.write_lyrics);
    assert!(!music_preset.embed_lyrics);
    assert_eq!(music_preset.lyrics_format.as_deref(), Some("lrc"));
    assert!(music_preset.clean_metadata);
    assert!(music_preset.parse_music_tags);

    let tiktok_preset = manager.get("tiktok").expect("tiktok preset should exist");
    assert_eq!(tiktok_preset.name, "tiktok");
    assert_eq!(tiktok_preset.container, "mp4");
    assert_eq!(tiktok_preset.max_vertical, Some(1440));
    assert!(!tiktok_preset.extract_audio);
    assert_eq!(
        tiktok_preset.output_template.as_deref(),
        Some("%(uploader,creator)s/%(upload_date>%Y-%m-%d)s_%(id)s_%(title).60s.%(ext)s")
    );
}

#[test]
fn test_preset_orientation_policy_enforcement() {
    let toml_str = r#"
        name = "custom"
        container = "mp4"
        quality = "best"
        max_horizontal = 1440
        max_vertical = 1080
    "#;

    let preset = Preset::from_toml(toml_str).expect("should parse preset");

    // Horizontal video with "best" quality -> capped at max_horizontal (1440)
    let eff_horiz = preset
        .effective_quality_preference(None, Orientation::Horizontal)
        .unwrap();
    assert_eq!(eff_horiz, QualityPreference::SpecificHeight(1440));

    // Vertical video with "best" quality -> capped at max_vertical (1080)
    let eff_vert = preset
        .effective_quality_preference(None, Orientation::Vertical)
        .unwrap();
    assert_eq!(eff_vert, QualityPreference::SpecificHeight(1080));

    // Explicit override 720 (below cap 1080) -> kept at 720
    let eff_vert_720 = preset
        .effective_quality_preference(Some("720"), Orientation::Vertical)
        .unwrap();
    assert_eq!(eff_vert_720, QualityPreference::SpecificHeight(720));

    // Explicit override 4k (above cap 1080 for vertical) -> capped at 1080
    let eff_vert_4k = preset
        .effective_quality_preference(Some("4k"), Orientation::Vertical)
        .unwrap();
    assert_eq!(eff_vert_4k, QualityPreference::SpecificHeight(1080));
}
