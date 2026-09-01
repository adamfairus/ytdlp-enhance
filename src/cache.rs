use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CachedMetadata {
    pub url: String,
    pub cached_at: u64,
    pub expires_at: u64,
    pub metadata: crate::metadata::VideoMetadata,
}

pub struct MetadataCache {
    cache_dir: PathBuf,
    default_ttl_secs: u64,
}

impl Default for MetadataCache {
    fn default() -> Self {
        Self::new()
    }
}

impl MetadataCache {
    /// Initialize with default cache path (~/.cache/dlp/metadata) and 2-hour TTL (7200s).
    pub fn new() -> Self {
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from(".cache"))
            .join("dlp")
            .join("metadata");
        Self::with_dir(cache_dir, 7200)
    }

    /// Initialize with a custom directory and default TTL in seconds.
    pub fn with_dir(dir: PathBuf, default_ttl_secs: u64) -> Self {
        Self {
            cache_dir: dir,
            default_ttl_secs,
        }
    }

    /// Normalizes URL by removing tracking parameters (si, feature, utm_*, etc.)
    /// and generates a deterministic hex key.
    pub fn normalize_key(url: &str) -> String {
        let url_trimmed = url.trim();
        let without_fragment = match url_trimmed.split_once('#') {
            Some((u, _)) => u,
            None => url_trimmed,
        };

        let normalized_url = if let Some((base, query_str)) = without_fragment.split_once('?') {
            let tracking_keys = [
                "si",
                "feature",
                "fbclid",
                "gclid",
                "utm_source",
                "utm_medium",
                "utm_campaign",
                "utm_term",
                "utm_content",
            ];

            let mut remaining_params: Vec<&str> = query_str
                .split('&')
                .filter(|param| {
                    if param.is_empty() {
                        return false;
                    }
                    let key = param.split_once('=').map(|(k, _)| k).unwrap_or(param);
                    !tracking_keys.contains(&key) && !key.starts_with("utm_")
                })
                .collect();

            if remaining_params.is_empty() {
                base.to_string()
            } else {
                remaining_params.sort();
                format!("{}?{}", base, remaining_params.join("&"))
            }
        } else {
            without_fragment.to_string()
        };

        let mut hasher = DefaultHasher::new();
        normalized_url.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }

    /// Resolve the file path for a cached URL.
    pub fn cache_file_path(&self, url: &str) -> PathBuf {
        let key = Self::normalize_key(url);
        self.cache_dir.join(format!("{}.json", key))
    }

    /// Retrieve cached metadata if present and not expired.
    /// If expired or invalid, removes the cached file and returns None.
    pub fn get(&self, url: &str) -> Option<crate::metadata::VideoMetadata> {
        let file_path = self.cache_file_path(url);
        if !file_path.exists() {
            return None;
        }

        let content = std::fs::read_to_string(&file_path).ok()?;
        let cached: CachedMetadata = match serde_json::from_str(&content) {
            Ok(c) => c,
            Err(_) => {
                let _ = std::fs::remove_file(&file_path);
                return None;
            }
        };

        let now = current_timestamp();
        if now < cached.expires_at {
            Some(cached.metadata)
        } else {
            let _ = std::fs::remove_file(&file_path);
            None
        }
    }

    /// Persist metadata to cache. Live streams are excluded from caching.
    pub fn set(&self, url: &str, meta: &crate::metadata::VideoMetadata, custom_ttl: Option<u64>) {
        if meta.is_live.unwrap_or(false) {
            return;
        }

        if std::fs::create_dir_all(&self.cache_dir).is_err() {
            return;
        }

        let now = current_timestamp();
        let ttl = custom_ttl.unwrap_or(self.default_ttl_secs);
        let expires_at = now.saturating_add(ttl);

        let entry = CachedMetadata {
            url: url.to_string(),
            cached_at: now,
            expires_at,
            metadata: meta.clone(),
        };

        if let Ok(json) = serde_json::to_string_pretty(&entry) {
            let file_path = self.cache_file_path(url);
            let _ = std::fs::write(file_path, json);
        }
    }

    /// Purge expired cache entries from the cache directory.
    pub fn purge_expired(&self) -> usize {
        if !self.cache_dir.exists() {
            return 0;
        }

        let read_dir = match std::fs::read_dir(&self.cache_dir) {
            Ok(rd) => rd,
            Err(_) => return 0,
        };

        let now = current_timestamp();
        let mut count = 0;

        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("json") {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(cached) = serde_json::from_str::<CachedMetadata>(&content) {
                        if now >= cached.expires_at && std::fs::remove_file(&path).is_ok() {
                            count += 1;
                        }
                    }
                }
            }
        }

        count
    }

    /// Purge all cached files in the cache directory.
    pub fn purge_all(&self) -> std::io::Result<()> {
        if !self.cache_dir.exists() {
            return Ok(());
        }

        for entry in std::fs::read_dir(&self.cache_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                std::fs::remove_file(path)?;
            }
        }

        Ok(())
    }

    /// Return a reference to the cache directory path.
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }
}
