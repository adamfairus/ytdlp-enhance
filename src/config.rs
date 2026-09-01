use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadPolicy {
    #[serde(default = "default_concurrency")]
    pub concurrency: usize,
    #[serde(default = "default_retry_delay")]
    pub retry_delay_sec: u64,
    pub rate_limit: Option<String>,
}

fn default_concurrency() -> usize {
    1
}

fn default_retry_delay() -> u64 {
    2
}

impl Default for DownloadPolicy {
    fn default() -> Self {
        Self {
            concurrency: default_concurrency(),
            retry_delay_sec: default_retry_delay(),
            rate_limit: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_preset")]
    pub default_preset: String,
    pub download_dir: Option<String>,
    #[serde(default)]
    pub download: DownloadPolicy,
}

fn default_preset() -> String {
    "video".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_preset: default_preset(),
            download_dir: None,
            download: DownloadPolicy::default(),
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
