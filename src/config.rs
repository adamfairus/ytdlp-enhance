use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_preset")]
    pub default_preset: String,
    pub download_dir: Option<String>,
}

fn default_preset() -> String {
    "video".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_preset: default_preset(),
            download_dir: None,
        }
    }
}

impl Config {
    pub fn load() -> Self {
        if let Some(config_dir) = dirs::config_dir() {
            let config_path = config_dir.join("dlp").join("config.toml");
            if config_path.exists() {
                if let Ok(content) = fs::read_to_string(&config_path) {
                    if let Ok(cfg) = toml::from_str::<Config>(&content) {
                        return cfg;
                    }
                }
            }
        }
        Config::default()
    }
}
