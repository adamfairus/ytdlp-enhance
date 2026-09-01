use crate::engine::YtDlpEngine;
use crate::error::Result;
use crate::metadata::VideoMetadata;
use crate::preset::Preset;
use crate::provider::ProviderRegistry;
use crate::quality::QualityPreference;

/// High-level facade for media downloading and metadata inspection.
/// Delegates execution to registered providers and the underlying engine layer.
pub struct Downloader;

impl Downloader {
    /// Verify that essential external dependencies (yt-dlp and ffmpeg) are installed.
    pub fn verify_dependencies() -> Result<()> {
        YtDlpEngine::verify_dependencies()
    }

    /// Check if a binary can be executed successfully with the provided test arguments.
    pub fn check_binary(bin: &str, test_args: &[&str]) -> Result<()> {
        YtDlpEngine::check_binary(bin, test_args)
    }

    /// Fetch video metadata, utilizing the local cache if enabled.
    pub fn fetch_metadata_cached(url: &str, use_cache: bool) -> Result<VideoMetadata> {
        let cache = crate::cache::MetadataCache::new();
        if use_cache {
            if let Some(cached) = cache.get(url) {
                return Ok(cached);
            }
        }

        let registry = ProviderRegistry::new();
        let meta = registry.find_provider(url).analyze(url)?;

        cache.set(url, &meta, None);

        Ok(meta)
    }

    /// Fetch video metadata through the Provider registry (cached by default).
    pub fn fetch_metadata(url: &str) -> Result<VideoMetadata> {
        Self::fetch_metadata_cached(url, true)
    }

    /// Execute the download process through the Provider registry.
    pub fn download(
        url: &str,
        preset: &Preset,
        effective_quality: &QualityPreference,
        override_output_dir: Option<&str>,
    ) -> Result<()> {
        let registry = ProviderRegistry::new();
        registry
            .find_provider(url)
            .download(url, preset, effective_quality, override_output_dir)
    }

    /// Backward-compatibility forwarding: Fetch metadata directly via yt-dlp engine.
    pub fn fetch_metadata_ytdlp(url: &str, impersonate_client: Option<&str>) -> Result<VideoMetadata> {
        YtDlpEngine::fetch_metadata(url, impersonate_client)
    }

    /// Backward-compatibility forwarding: Execute download directly via yt-dlp engine.
    pub fn download_via_ytdlp(
        url: &str,
        preset: &Preset,
        effective_quality: &QualityPreference,
        override_output_dir: Option<&str>,
    ) -> Result<()> {
        YtDlpEngine::download(url, preset, effective_quality, override_output_dir)
    }
}
