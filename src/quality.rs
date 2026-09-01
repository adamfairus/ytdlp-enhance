use crate::error::{DlpError, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QualityPreference {
    Best,
    SpecificHeight(u32),
    VerticalResolution(u32), // Width <= u32 for portrait/vertical video (e.g. 1080 for 1080x1920)
}

impl QualityPreference {
    pub fn parse(input: &str) -> Result<Self> {
        let normalized = input.trim().to_lowercase();
        let cleaned = normalized.trim_end_matches('p');

        match cleaned {
            "best" | "max" | "highest" => Ok(QualityPreference::Best),
            "4k" | "uhd" | "2160" => Ok(QualityPreference::SpecificHeight(2160)),
            "2k" | "qhd" | "1440" => Ok(QualityPreference::SpecificHeight(1440)),
            "fhd" | "1080" => Ok(QualityPreference::SpecificHeight(1080)),
            "hd" | "720" => Ok(QualityPreference::SpecificHeight(720)),
            "sd" | "480" => Ok(QualityPreference::SpecificHeight(480)),
            "360" => Ok(QualityPreference::SpecificHeight(360)),
            "240" => Ok(QualityPreference::SpecificHeight(240)),
            "144" => Ok(QualityPreference::SpecificHeight(144)),
            other => {
                if let Ok(num) = other.parse::<u32>() {
                    if num > 0 {
                        return Ok(QualityPreference::SpecificHeight(num));
                    }
                }
                Err(DlpError::InvalidQuality(input.to_string()))
            }
        }
    }

    /// Adapts quality preference to video orientation (mapping vertical heights to width limits e.g. 1080 -> 1080x1920)
    pub fn for_orientation(&self, orientation: crate::orientation::Orientation) -> Self {
        match (orientation, self) {
            (crate::orientation::Orientation::Vertical, QualityPreference::SpecificHeight(h)) => {
                QualityPreference::VerticalResolution(*h)
            }
            _ => self.clone(),
        }
    }

    /// Generates yt-dlp format selector string.
    pub fn to_format_selector(&self) -> String {
        match self {
            QualityPreference::Best => "bestvideo+bestaudio/best".to_string(),
            QualityPreference::SpecificHeight(h) => {
                format!("bestvideo[height<={h}]+bestaudio/best[height<={h}]/best")
            }
            QualityPreference::VerticalResolution(w) => {
                format!("bestvideo[width<={w}]+bestaudio/best[width<={w}]/best")
            }
        }
    }

    /// Finds the best matching resolution from available heights list.
    pub fn select_best_resolution(&self, available: &[u32]) -> Option<u32> {
        if available.is_empty() {
            return None;
        }
        match self {
            QualityPreference::Best => available.first().copied(),
            QualityPreference::SpecificHeight(target) | QualityPreference::VerticalResolution(target) => {
                // Find highest resolution <= target, or fallback to lowest available if all > target
                available
                    .iter()
                    .filter(|&&h| h <= *target)
                    .max()
                    .copied()
                    .or_else(|| available.last().copied())
            }
        }
    }

    /// Provides next fallback quality if the requested format/resolution is unavailable.
    pub fn fallback_step(&self) -> Option<QualityPreference> {
        match self {
            QualityPreference::SpecificHeight(h) if *h > 1440 => Some(QualityPreference::SpecificHeight(1440)),
            QualityPreference::SpecificHeight(h) if *h > 1080 => Some(QualityPreference::SpecificHeight(1080)),
            QualityPreference::SpecificHeight(h) if *h > 720 => Some(QualityPreference::SpecificHeight(720)),
            QualityPreference::SpecificHeight(h) if *h > 480 => Some(QualityPreference::SpecificHeight(480)),
            QualityPreference::SpecificHeight(_) => Some(QualityPreference::Best),
            QualityPreference::VerticalResolution(w) if *w > 1440 => Some(QualityPreference::VerticalResolution(1440)),
            QualityPreference::VerticalResolution(w) if *w > 1080 => Some(QualityPreference::VerticalResolution(1080)),
            QualityPreference::VerticalResolution(w) if *w > 720 => Some(QualityPreference::VerticalResolution(720)),
            QualityPreference::VerticalResolution(w) if *w > 480 => Some(QualityPreference::VerticalResolution(480)),
            QualityPreference::VerticalResolution(_) => Some(QualityPreference::Best),
            QualityPreference::Best => None,
        }
    }
}

