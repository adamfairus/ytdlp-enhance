use crate::downloader::Downloader;
use crate::error::Result;
use crate::metadata::VideoMetadata;
use crate::preset::Preset;
use crate::quality::QualityPreference;
use crate::tiktok::TikTokFallback;

/// Core Provider Trait defining media extraction and download behaviors across platforms.
pub trait Provider: Send + Sync {
    /// Human-readable identifier of the provider (e.g. "YouTube", "TikTok", "Generic").
    fn name(&self) -> &'static str;

    /// Checks if this provider can handle the given URL.
    fn detect(&self, url: &str) -> bool;

    /// Inspects and parses metadata for the media.
    fn analyze(&self, url: &str) -> Result<VideoMetadata>;

    /// Executes the download using platform-specific optimization pipelines.
    fn download(
        &self,
        url: &str,
        preset: &Preset,
        effective_quality: &QualityPreference,
        override_output_dir: Option<&str>,
    ) -> Result<()>;

    /// Enforces platform-specific safe concurrency bounds (e.g. 2 for TikTok to prevent bans).
    fn max_safe_concurrency(&self, desired: usize) -> usize {
        desired.max(1)
    }
}

/// Specialized TikTok Provider using TikWM high-speed engine with 10-client impersonation fallback.
pub struct TikTokProvider;

impl Provider for TikTokProvider {
    fn name(&self) -> &'static str {
        "TikTok"
    }

    fn detect(&self, url: &str) -> bool {
        TikTokFallback::is_tiktok_url(url)
    }

    fn analyze(&self, url: &str) -> Result<VideoMetadata> {
        if let Ok(meta) = TikTokFallback::fetch_metadata(url) {
            return Ok(meta);
        }
        println!("⚠️  TikWM metadata fetch failed. Falling back to yt-dlp with impersonation...");
        Downloader::fetch_metadata_ytdlp(url, Some("chrome"))
    }

    fn download(
        &self,
        url: &str,
        preset: &Preset,
        effective_quality: &QualityPreference,
        override_output_dir: Option<&str>,
    ) -> Result<()> {
        match TikTokFallback::download(url, override_output_dir) {
            Ok(_) => Ok(()),
            Err(e) => {
                println!("⚠️  TikWM download failed: {e}. Engaging yt-dlp 10-client impersonation rotation...");
                TikTokFallback::download_with_impersonation_rotation(
                    url,
                    preset,
                    effective_quality,
                    override_output_dir,
                )
            }
        }
    }

    fn max_safe_concurrency(&self, desired: usize) -> usize {
        desired.clamp(1, 2)
    }
}

/// Specialized YouTube / YouTube Music Provider with format stepping and resilient retry.
pub struct YouTubeProvider;

impl Provider for YouTubeProvider {
    fn name(&self) -> &'static str {
        "YouTube"
    }

    fn detect(&self, url: &str) -> bool {
        let lower = url.to_lowercase();
        lower.contains("youtube.com") || lower.contains("youtu.be")
    }

    fn analyze(&self, url: &str) -> Result<VideoMetadata> {
        Downloader::fetch_metadata_ytdlp(url, None)
    }

    fn download(
        &self,
        url: &str,
        preset: &Preset,
        effective_quality: &QualityPreference,
        override_output_dir: Option<&str>,
    ) -> Result<()> {
        Downloader::download_via_ytdlp(url, preset, effective_quality, override_output_dir)
    }

    fn max_safe_concurrency(&self, desired: usize) -> usize {
        desired.max(1)
    }
}

/// Generic Provider fallback supporting 1000+ sites via standard yt-dlp extraction.
pub struct GenericProvider;

impl Provider for GenericProvider {
    fn name(&self) -> &'static str {
        "Generic"
    }

    fn detect(&self, _url: &str) -> bool {
        true
    }

    fn analyze(&self, url: &str) -> Result<VideoMetadata> {
        Downloader::fetch_metadata_ytdlp(url, None)
    }

    fn download(
        &self,
        url: &str,
        preset: &Preset,
        effective_quality: &QualityPreference,
        override_output_dir: Option<&str>,
    ) -> Result<()> {
        Downloader::download_via_ytdlp(url, preset, effective_quality, override_output_dir)
    }

    fn max_safe_concurrency(&self, desired: usize) -> usize {
        desired.max(1)
    }
}

/// Central Provider Registry that automatically routes URLs to the best matching provider.
pub struct ProviderRegistry {
    providers: Vec<Box<dyn Provider>>,
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: vec![
                Box::new(TikTokProvider),
                Box::new(YouTubeProvider),
                Box::new(GenericProvider),
            ],
        }
    }

    /// Finds the first registered provider that claims support for the URL.
    pub fn find_provider(&self, url: &str) -> &dyn Provider {
        for p in &self.providers {
            if p.detect(url) {
                return p.as_ref();
            }
        }
        self.providers.last().unwrap().as_ref()
    }

    /// Returns a list of all registered provider names.
    pub fn list_providers(&self) -> Vec<&'static str> {
        self.providers.iter().map(|p| p.name()).collect()
    }
}
