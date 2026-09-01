use crate::metadata::VideoMetadata;
use crate::orientation::Orientation;
use crate::tiktok::TikTokFallback;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
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

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Classification {
    pub media_type: MediaType,
    pub confidence: f32, // 0.0 to 1.0
    pub reasons: Vec<String>,
}

impl Classification {
    pub fn default_preset_name(&self) -> &'static str {
        self.media_type.default_preset_name()
    }

    pub fn display_label(&self) -> &'static str {
        self.media_type.display_label()
    }
}

pub struct SmartClassifier;

impl SmartClassifier {
    /// Automatically detects whether a given media URL and metadata represents Music, Vertical Video, or Standard Video
    pub fn classify(url: &str, meta: &VideoMetadata) -> Classification {
        let u = url.to_lowercase();

        let mut music_score: f32 = 0.0;
        let mut vertical_score: f32 = 0.0;
        let mut standard_score: f32 = 0.0;

        let mut music_reasons: Vec<String> = Vec::new();
        let mut vertical_reasons: Vec<String> = Vec::new();
        let mut standard_reasons: Vec<String> = Vec::new();

        // 1. Domain / URL patterns
        if u.contains("music.youtube.com")
            || u.contains("soundcloud.com")
            || u.contains("spotify.com")
            || u.contains("bandcamp.com")
            || u.contains("deezer.com")
        {
            music_score += 0.50;
            music_reasons.push("music platform URL".to_string());
        }

        if TikTokFallback::is_tiktok_url(url)
            || u.contains("/shorts/")
            || u.contains("/reel/")
            || u.contains("/reels/")
            || u.contains("instagram.com/p/")
        {
            vertical_score += 0.55;
            vertical_reasons.push("short-form / vertical video URL".to_string());
        }

        // 2. Orientation
        match meta.orientation() {
            Orientation::Vertical => {
                vertical_score += 0.40;
                vertical_reasons.push("vertical aspect ratio (e.g. 9:16)".to_string());
            }
            Orientation::Horizontal => {
                standard_score += 0.30;
                standard_reasons.push("horizontal aspect ratio (e.g. 16:9)".to_string());
            }
            Orientation::Square => {}
        }

        // 3. Audio & Category
        if meta.is_audio_only() {
            music_score += 0.35;
            music_reasons.push("audio-only stream detected".to_string());
        }

        let has_music_category = meta
            .categories
            .as_ref()
            .map_or(false, |cats| cats.iter().any(|c| c.to_lowercase().contains("music")));

        if has_music_category {
            music_score += 0.20;
            music_reasons.push("category tagged as Music".to_string());
        }

        if let Some(d) = meta.duration {
            if d < 600.0 && (has_music_category || meta.is_audio_only()) {
                music_score += 0.10;
                music_reasons.push("standard track duration (< 10 min)".to_string());
            }
        }

        // Pick highest scoring MediaType
        let (media_type, raw_score, mut reasons) = if music_score > vertical_score && music_score > standard_score {
            (MediaType::Music, music_score, music_reasons)
        } else if vertical_score > music_score && vertical_score > standard_score {
            (MediaType::VerticalVideo, vertical_score, vertical_reasons)
        } else {
            (MediaType::StandardVideo, standard_score, standard_reasons)
        };

        if reasons.is_empty() {
            reasons.push("default standard video fallback".to_string());
        }

        let confidence = raw_score.clamp(0.60, 0.99);

        Classification {
            media_type,
            confidence,
            reasons,
        }
    }
}
