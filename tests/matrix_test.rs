use dlp::orientation::Orientation;
use dlp::preset::Preset;
use dlp::quality::QualityPreference;

#[test]
fn test_quality_and_orientation_matrix() {
    let heights = [2160, 1440, 1080, 720, 480, 360];

    for h in heights {
        let q = QualityPreference::SpecificHeight(h);
        let selector = q.to_format_selector();
        assert!(selector.contains(&format!("height<={}", h)));
        assert!(selector.contains("bestvideo"));
        assert!(selector.contains("bestaudio"));
    }

    let best = QualityPreference::Best;
    assert_eq!(best.to_format_selector(), "bestvideo+bestaudio/best");
}

#[test]
fn test_preset_policy_enforcement_matrix() {
    let toml_str = r#"
        name = "matrix_test"
        container = "mp4"
        quality = "best"
        max_horizontal = 1080
        max_vertical = 720
    "#;
    let preset = Preset::from_toml(toml_str).unwrap();

    // 1. Horizontal video requesting 4K -> clamped to 1080
    let horiz_4k = preset
        .effective_quality_preference(Some("4k"), Orientation::Horizontal)
        .unwrap();
    assert_eq!(horiz_4k, QualityPreference::SpecificHeight(1080));

    // 2. Horizontal video requesting 720 -> kept at 720
    let horiz_720 = preset
        .effective_quality_preference(Some("720"), Orientation::Horizontal)
        .unwrap();
    assert_eq!(horiz_720, QualityPreference::SpecificHeight(720));

    // 3. Vertical video requesting 1080 -> clamped to 720
    let vert_1080 = preset
        .effective_quality_preference(Some("1080"), Orientation::Vertical)
        .unwrap();
    assert_eq!(vert_1080, QualityPreference::SpecificHeight(720));

    // 4. Square video with default policy -> resolves to best
    let sq_default = preset
        .effective_quality_preference(None, Orientation::Square)
        .unwrap();
    assert_eq!(sq_default, QualityPreference::Best);
}
