use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use crate::error::{DlpError, Result};
use crate::orientation::Orientation;
use crate::quality::QualityPreference;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Preset {
    pub name: String,
    pub description: Option<String>,
    #[serde(default = "default_container")]
    pub container: String,
    #[serde(default = "default_quality")]
    pub quality: String,
    pub max_horizontal: Option<u32>,
    pub max_vertical: Option<u32>,
    #[serde(default = "default_true")]
    pub embed_metadata: bool,
    #[serde(default = "default_true")]
    pub embed_thumbnail: bool,
    #[serde(default)]
    pub crop_thumbnail_square: bool,
    #[serde(default)]
    pub extract_audio: bool,
    pub audio_format: Option<String>,
    pub audio_quality: Option<String>,
    #[serde(default)]
    pub write_lyrics: bool,
    #[serde(default)]
    pub embed_lyrics: bool,
    pub lyrics_format: Option<String>,
    pub sub_langs: Option<Vec<String>>,
    #[serde(default)]
    pub clean_metadata: bool,
    #[serde(default)]
    pub parse_music_tags: bool,
    pub output_template: Option<String>,
    pub output_dir: Option<String>,
}

fn default_container() -> String {
    "mp4".to_string()
}

fn default_quality() -> String {
    "best".to_string()
}

fn default_true() -> bool {
    true
}

impl Preset {
    pub fn from_toml(content: &str) -> Result<Self> {
        toml::from_str(content).map_err(|e| DlpError::ConfigParse(e.to_string()))
    }

    /// Determines the effective quality limit considering orientation policies.
    pub fn effective_quality_preference(
        &self,
        override_quality: Option<&str>,
        orientation: Orientation,
    ) -> Result<QualityPreference> {
        let qual_str = override_quality.unwrap_or(&self.quality);
        let base_pref = QualityPreference::parse(qual_str)?;

        // Apply max horizontal/vertical limit from preset
        let effective_pref = match orientation {
            Orientation::Horizontal => {
                if let Some(max_h) = self.max_horizontal {
                    match base_pref {
                        QualityPreference::Best => QualityPreference::SpecificHeight(max_h),
                        QualityPreference::SpecificHeight(h) if h > max_h => {
                            QualityPreference::SpecificHeight(max_h)
                        }
                        other => other,
                    }
                } else {
                    base_pref
                }
            }
            Orientation::Vertical => {
                if let Some(max_v) = self.max_vertical {
                    match base_pref {
                        QualityPreference::Best => QualityPreference::SpecificHeight(max_v),
                        QualityPreference::SpecificHeight(h) if h > max_v => {
                            QualityPreference::SpecificHeight(max_v)
                        }
                        other => other,
                    }
                } else {
                    base_pref
                }
            }
            Orientation::Square => base_pref,
        };

        Ok(effective_pref)
    }
}

#[derive(Debug, Clone)]
pub struct PresetManager {
    presets: HashMap<String, Preset>,
}

impl PresetManager {
    pub fn load_all() -> Self {
        let mut manager = Self {
            presets: HashMap::new(),
        };

        // 1. Load embedded default presets
        let default_video = include_str!("../presets/video.toml");
        let default_music = include_str!("../presets/music.toml");
        let default_tiktok = include_str!("../presets/tiktok.toml");

        if let Ok(p) = Preset::from_toml(default_video) {
            manager.presets.insert(p.name.clone(), p);
        }
        if let Ok(p) = Preset::from_toml(default_music) {
            manager.presets.insert(p.name.clone(), p);
        }
        if let Ok(p) = Preset::from_toml(default_tiktok) {
            manager.presets.insert(p.name.clone(), p);
        }

        // 2. Load custom presets from user config dir (~/.config/dlp/presets/*.toml)
        if let Some(config_dir) = dirs::config_dir() {
            let presets_dir = config_dir.join("dlp").join("presets");
            manager.load_from_dir(&presets_dir);
        }

        // 3. Load from local ./presets folder if present
        manager.load_from_dir(Path::new("presets"));

        manager
    }

    fn load_from_dir(&mut self, dir: &Path) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("toml") {
                    if let Ok(content) = fs::read_to_string(&path) {
                        if let Ok(preset) = Preset::from_toml(&content) {
                            self.presets.insert(preset.name.clone(), preset);
                        }
                    }
                }
            }
        }
    }

    pub fn get(&self, name: &str) -> Option<&Preset> {
        self.presets.get(name)
    }

    pub fn list(&self) -> Vec<&Preset> {
        let mut list: Vec<&Preset> = self.presets.values().collect();
        list.sort_by(|a, b| a.name.cmp(&b.name));
        list
    }
}
