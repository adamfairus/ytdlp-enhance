use serde::{Deserialize, Serialize};
use crate::metadata::VideoMetadata;
use crate::orientation::Orientation;
use crate::preset::Preset;
use crate::quality::QualityPreference;
use crate::scheduler::PlatformCategory;

/// Normalized metadata representing a clean, platform-independent media profile.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NormalizedMetadata {
    pub platform: String,
    pub content_type: String,
    pub clean_title: String,
    pub clean_artist: Option<String>,
    pub clean_album: Option<String>,
    pub orientation: Orientation,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub duration_secs: u64,
    pub duration_formatted: String,
    pub has_subtitles: bool,
    pub available_resolutions: Vec<u32>,
    pub sanitized_filename: String,
}

pub struct MetadataNormalizer;

impl MetadataNormalizer {
    /// Cleans noisy YouTube/TikTok video titles (removes MV, Official Video, brackets, etc.)
    pub fn clean_title(title: &str) -> String {
        let mut cleaned = title.trim().to_string();

        // Common title noise patterns
        let patterns = [
            "(official music video)",
            "[official music video]",
            "(official video)",
            "[official video]",
            "(official mv)",
            "[official mv]",
            "(official m/v)",
            "[official m/v]",
            "(official audio)",
            "[official audio]",
            "(performance video)",
            "[performance video]",
            "(visualizer)",
            "[visualizer]",
            "(lyric video)",
            "[lyric video]",
            "(lyrics video)",
            "[lyrics video]",
            "(color coded lyrics)",
            "[color coded lyrics]",
            "(audio)",
            "[audio]",
            "(mv)",
            "[mv]",
            "(m/v)",
            "[m/v]",
            "- official video",
            "- official audio",
            "(remastered)",
            "[remastered]",
            "(4k remaster)",
            "[4k remaster]",
            "- remastered",
        ];

        let mut changed = true;
        while changed {
            changed = false;
            for pattern in &patterns {
                let lower = cleaned.to_lowercase();
                if let Some(pos) = lower.find(pattern) {
                    let end = pos + pattern.len();
                    cleaned.replace_range(pos..end, "");
                    changed = true;
                    break;
                }
            }
        }

        // Clean extra whitespace and trailing punctuation
        cleaned = cleaned.trim().trim_end_matches(['-', '_', '|', ':', ' ']).trim().to_string();
        while cleaned.contains("  ") {
            cleaned = cleaned.replace("  ", " ");
        }

        if cleaned.is_empty() {
            title.trim().to_string()
        } else {
            cleaned
        }
    }

    /// Cleans uploader or artist names (e.g. removes " - Topic")
    pub fn clean_artist(artist: &str) -> String {
        let trimmed = artist.trim();
        if let Some(stripped) = trimmed.strip_suffix("- Topic") {
            stripped.trim().to_string()
        } else if let Some(stripped) = trimmed.strip_suffix("– Topic") {
            stripped.trim().to_string()
        } else {
            trimmed.to_string()
        }
    }

    /// Sanitizes file names to be valid across Windows, Linux, and macOS file systems.
    pub fn sanitize_filename(name: &str, ext: &str) -> String {
        let mut clean = String::new();
        for ch in name.chars() {
            match ch {
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => clean.push('_'),
                '\0'..='\x1f' => {}
                other => clean.push(other),
            }
        }

        let clean_trimmed = clean.trim().trim_end_matches(['.', ' ']);
        let base_name = if clean_trimmed.is_empty() {
            "media".to_string()
        } else {
            clean_trimmed.chars().take(200).collect()
        };

        if ext.is_empty() {
            base_name
        } else {
            let clean_ext = ext.trim_start_matches('.');
            format!("{}.{}", base_name, clean_ext)
        }
    }

    /// Normalizes raw VideoMetadata into structured NormalizedMetadata.
    pub fn normalize(url: &str, meta: &VideoMetadata, preset: &Preset) -> NormalizedMetadata {
        let platform = match crate::scheduler::ScheduledPlan::detect_platform(url) {
            PlatformCategory::YouTube => "YouTube",
            PlatformCategory::YouTubeMusic => "YouTube Music",
            PlatformCategory::TikTok => "TikTok",
            PlatformCategory::Generic => "Generic",
        };

        let content_type = if preset.extract_audio {
            "Music / Audio"
        } else if meta.orientation() == Orientation::Vertical {
            "Shorts / Vertical Video"
        } else {
            "Standard Video"
        };

        let clean_title = Self::clean_title(&meta.title);
        let clean_artist = meta.uploader.as_deref().map(Self::clean_artist);
        let orientation = meta.orientation();
        let ext = if preset.extract_audio {
            preset.audio_format.as_deref().unwrap_or("opus")
        } else {
            &preset.container
        };
        let sanitized_filename = Self::sanitize_filename(&clean_title, ext);

        NormalizedMetadata {
            platform: platform.to_string(),
            content_type: content_type.to_string(),
            clean_title,
            clean_artist,
            clean_album: None,
            orientation,
            width: meta.width,
            height: meta.height,
            duration_secs: meta.duration.unwrap_or(0.0) as u64,
            duration_formatted: meta.format_duration(),
            has_subtitles: meta.subtitles.is_some() || meta.automatic_captions.is_some(),
            available_resolutions: meta.available_resolutions(),
            sanitized_filename,
        }
    }
}

/// Rich UX Decision Trace explaining why DLP makes specific policy, format, and post-processing decisions.
#[derive(Debug, Clone)]
pub struct DecisionTrace {
    pub platform: String,
    pub content_type: String,
    pub orientation: String,
    pub resolution: String,
    pub duration: String,
    pub policy_name: String,
    pub policy_rules: Vec<String>,
    pub selected_format_desc: Vec<String>,
    pub post_processing_steps: Vec<String>,
    pub output_filename: String,
    pub output_dir: Option<String>,
}

impl DecisionTrace {
    pub fn build(
        url: &str,
        meta: &VideoMetadata,
        preset: &Preset,
        effective_quality: &QualityPreference,
        override_output_dir: Option<&str>,
    ) -> Self {
        let norm = MetadataNormalizer::normalize(url, meta, preset);

        let resolution = match (meta.width, meta.height) {
            (Some(w), Some(h)) => format!("{}x{}", w, h),
            _ => "Unknown".to_string(),
        };

        let mut policy_rules = Vec::new();
        policy_rules.push(format!("Preset: {}", preset.name));
        if let Some(max_h) = preset.max_horizontal {
            policy_rules.push(format!("max_horizontal = {}p", max_h));
        }
        if let Some(max_v) = preset.max_vertical {
            policy_rules.push(format!("max_vertical = {}p", max_v));
        }
        policy_rules.push(format!("preferred = {}", preset.quality));

        let mut selected_format_desc = Vec::new();
        if preset.extract_audio {
            let fmt = preset.audio_format.as_deref().unwrap_or("opus");
            selected_format_desc.push(format!("audio stream: 251/140/bestaudio"));
            selected_format_desc.push(format!("codec: {}", fmt));
        } else {
            let format_str = effective_quality.to_format_selector();
            selected_format_desc.push(format!("selector: {}", format_str));
            selected_format_desc.push(format!("container: {}", preset.container));
        }

        let mut post_processing_steps = Vec::new();
        if preset.extract_audio {
            post_processing_steps.push("ffmpeg audio extraction".to_string());
            if preset.crop_thumbnail_square {
                post_processing_steps.push("ffmpeg 1:1 square cover crop".to_string());
            }
            if preset.write_lyrics {
                post_processing_steps.push("LRCLIB synced lyrics (.lrc sidecar)".to_string());
            }
        } else {
            post_processing_steps.push(format!("ffmpeg remux/merge to {}", preset.container));
            if preset.embed_metadata {
                post_processing_steps.push("metadata embedding".to_string());
            }
            if preset.embed_thumbnail {
                post_processing_steps.push("thumbnail embedding".to_string());
            }
            if preset.write_lyrics || preset.sub_langs.is_some() {
                post_processing_steps.push("subtitles → external .srt sidecar".to_string());
            }
        }

        let target_dir = override_output_dir
            .or(preset.output_dir.as_deref())
            .map(|s| s.to_string());

        Self {
            platform: norm.platform,
            content_type: norm.content_type,
            orientation: norm.orientation.display_name().to_string(),
            resolution,
            duration: norm.duration_formatted,
            policy_name: preset.name.clone(),
            policy_rules,
            selected_format_desc,
            post_processing_steps,
            output_filename: norm.sanitized_filename,
            output_dir: target_dir,
        }
    }

    pub fn print_trace(&self) {
        println!("\n╔══════════════════════════════════════════════════╗");
        println!("║               🔍 DLP DECISION TRACE              ║");
        println!("╠══════════════════════════════════════════════════╣");
        println!("║ Platform       : {:<32}║", self.platform);
        println!("║ Content        : {:<32}║", self.content_type);
        println!("║ Orientation    : {:<32}║", self.orientation);
        println!("║ Resolution     : {:<32}║", self.resolution);
        println!("║ Duration       : {:<32}║", self.duration);
        println!("╠══════════════════════════════════════════════════╣");
        println!("║ Detected Policy:                                 ║");
        for rule in &self.policy_rules {
            println!("║   → {:<45}║", rule);
        }
        println!("╠══════════════════════════════════════════════════╣");
        println!("║ Selected Format:                                 ║");
        for fmt in &self.selected_format_desc {
            println!("║   → {:<45}║", fmt);
        }
        println!("╠══════════════════════════════════════════════════╣");
        println!("║ Post-processing Pipeline:                        ║");
        for step in &self.post_processing_steps {
            println!("║   → {:<45}║", step);
        }
        println!("╠══════════════════════════════════════════════════╣");
        println!("║ Output:                                          ║");
        println!("║   → {:<45}║", truncate_str(&self.output_filename, 45));
        if let Some(dir) = &self.output_dir {
            println!("║   Folder: {:<41}║", truncate_str(dir, 41));
        }
        println!("╚══════════════════════════════════════════════════╝\n");
    }
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        let mut truncated: String = s.chars().take(max_len.saturating_sub(3)).collect();
        truncated.push_str("...");
        truncated
    }
}
