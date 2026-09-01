use crate::metadata::VideoMetadata;
use crate::orientation::Orientation;
use crate::tiktok::TikTokFallback;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaType {
    Music,
    VerticalVideo,
    StandardVideo,
}

impl MediaType {
    pub fn default_preset_name(&self) -> &'static str {
        match self {
            MediaType::Music => "music",
            MediaType::VerticalVideo => "tiktok",
            MediaType::StandardVideo => "video",
        }
    }

    pub fn display_label(&self) -> &'static str {
        match self {
            MediaType::Music => "🎵 Music / Audio",
            MediaType::VerticalVideo => "📱 Vertical Short-Form Video",
            MediaType::StandardVideo => "🎬 Standard Horizontal Video",
        }
    }
}

pub struct SmartClassifier;

impl SmartClassifier {
    /// Automatically detects whether a given media URL and metadata represents Music, Vertical Video, or Standard Video
    pub fn classify(url: &str, meta: &VideoMetadata) -> MediaType {
        let u = url.to_lowercase();

        // 1. Explicit domain / URL pattern checks
        if u.contains("music.youtube.com")
            || u.contains("soundcloud.com")
            || u.contains("spotify.com")
            || u.contains("deezer.com")
            || u.contains("bandcamp.com")
        {
            return MediaType::Music;
        }

        if TikTokFallback::is_tiktok_url(url)
            || u.contains("/shorts/")
            || u.contains("/reel/")
            || u.contains("/reels/")
            || u.contains("instagram.com/p/")
        {
            return MediaType::VerticalVideo;
        }

        // 2. Orientation & Metadata checks
        if meta.orientation() == Orientation::Vertical {
            return MediaType::VerticalVideo;
        }

        if meta.is_audio_only() {
            return MediaType::Music;
        }

        if let Some(cats) = &meta.categories {
            if cats.iter().any(|c| c.eq_ignore_ascii_case("Music")) {
                if let Some(d) = meta.duration {
                    // Audio tracks / songs under 10 minutes from music categories
                    if d < 600.0 && meta.is_audio_only() {
                        return MediaType::Music;
                    }
                }
            }
        }

        MediaType::StandardVideo
    }
}
