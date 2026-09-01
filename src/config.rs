use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use crate::error::{DlpError, Result};

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
    pub fn config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("dlp").join("config.toml"))
    }

    pub fn load() -> Self {
        if let Some(config_path) = Self::config_path() {
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

    pub fn save(&self) -> Result<PathBuf> {
        let path = Self::config_path().ok_or_else(|| {
            DlpError::ConfigParse("Could not resolve user config directory".to_string())
        })?;

        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        let toml_str = toml::to_string_pretty(self)
            .map_err(|e| DlpError::ConfigParse(e.to_string()))?;
        fs::write(&path, toml_str)?;
        Ok(path)
    }

    pub fn set_value(&mut self, key: &str, value: &str) -> Result<()> {
        match key {
            "default_preset" => self.default_preset = value.to_string(),
            "download_dir" => {
                if value.trim().is_empty() || value.eq_ignore_ascii_case("none") {
                    self.download_dir = None;
                } else {
                    self.download_dir = Some(value.to_string());
                }
            }
            "download.concurrency" | "concurrency" => {
                let c = value.parse::<usize>().map_err(|_| {
                    DlpError::ConfigParse(format!("Invalid concurrency integer: {}", value))
                })?;
                self.download.concurrency = c.max(1);
            }
            "download.retry_delay_sec" | "retry_delay_sec" => {
                let d = value.parse::<u64>().map_err(|_| {
                    DlpError::ConfigParse(format!("Invalid retry delay integer: {}", value))
                })?;
                self.download.retry_delay_sec = d;
            }
            "download.rate_limit" | "rate_limit" => {
                if value.trim().is_empty() || value.eq_ignore_ascii_case("none") {
                    self.download.rate_limit = None;
                } else {
                    self.download.rate_limit = Some(value.to_string());
                }
            }
            _ => {
                return Err(DlpError::ConfigParse(format!(
                    "Unknown configuration key '{}'. Available keys: default_preset, download_dir, concurrency, retry_delay_sec, rate_limit",
                    key
                )));
            }
        }
        Ok(())
    }
}
