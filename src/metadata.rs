use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::error::Result;
use crate::orientation::Orientation;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoFormat {
    pub format_id: String,
    pub ext: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fps: Option<f64>,
    pub vcodec: Option<String>,
    pub acodec: Option<String>,
    pub filesize: Option<u64>,
    pub filesize_approx: Option<u64>,
    pub tbr: Option<f64>,
    pub vbr: Option<f64>,
    pub abr: Option<f64>,
    pub format_note: Option<String>,
    pub resolution: Option<String>,
}

impl VideoFormat {
    pub fn is_video_only(&self) -> bool {
        self.vcodec.as_deref().unwrap_or("none") != "none"
            && self.acodec.as_deref().unwrap_or("none") == "none"
    }

    pub fn is_audio_only(&self) -> bool {
        self.vcodec.as_deref().unwrap_or("none") == "none"
            && self.acodec.as_deref().unwrap_or("none") != "none"
    }

    pub fn has_video(&self) -> bool {
        self.vcodec.as_deref().unwrap_or("none") != "none"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoMetadata {
    pub id: String,
    pub title: String,
    pub uploader: Option<String>,
    pub duration: Option<f64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub filesize: Option<u64>,
    pub filesize_approx: Option<u64>,
    pub formats: Option<Vec<VideoFormat>>,
    pub webpage_url: Option<String>,
    pub thumbnail: Option<String>,
    pub categories: Option<Vec<String>>,
    pub extractor: Option<String>,
    pub subtitles: Option<HashMap<String, serde_json::Value>>,
    pub automatic_captions: Option<HashMap<String, serde_json::Value>>,
}

impl VideoMetadata {
    pub fn is_audio_only(&self) -> bool {
        if let Some(formats) = &self.formats {
            !formats.is_empty() && formats.iter().all(|f| f.is_audio_only())
        } else {
            false
        }
    }

    pub fn from_json(json_str: &str) -> Result<Self> {
        let metadata: VideoMetadata = serde_json::from_str(json_str)?;
        Ok(metadata)
    }

    pub fn orientation(&self) -> Orientation {
        match (self.width, self.height) {
            (Some(w), Some(h)) if w > 0 && h > 0 => Orientation::from_dimensions(w, h),
            _ => Orientation::Horizontal,
        }
    }

    pub fn available_resolutions(&self) -> Vec<u32> {
        let mut resolutions = Vec::new();
        if let Some(formats) = &self.formats {
            for f in formats {
                if let Some(h) = f.height {
                    if h > 0 && !resolutions.contains(&h) {
                        resolutions.push(h);
                    }
                }
            }
        }
        resolutions.sort_by(|a, b| b.cmp(a));
        resolutions
    }

    pub fn estimated_size_for_resolution(&self, height: u32) -> Option<u64> {
        let duration = self.duration.unwrap_or(0.0);

        // Find best audio filesize
        let mut best_audio_size = 0u64;
        if let Some(formats) = &self.formats {
            for f in formats {
                if f.is_audio_only() {
                    let sz = f.filesize.or(f.filesize_approx).unwrap_or_else(|| {
                        if let Some(abr) = f.abr.or(f.tbr) {
                            if duration > 0.0 {
                                ((abr * 1000.0 / 8.0) * duration) as u64
                            } else {
                                0
                            }
                        } else {
                            0
                        }
                    });
                    if sz > best_audio_size {
                        best_audio_size = sz;
                    }
                }
            }
        }

        if let Some(formats) = &self.formats {
            let mut best_video_size: Option<u64> = None;
            for f in formats {
                if f.height == Some(height) {
                    let sz = f.filesize.or(f.filesize_approx).or_else(|| {
                        if let Some(vbr) = f.vbr.or(f.tbr) {
                            if duration > 0.0 {
                                Some(((vbr * 1000.0 / 8.0) * duration) as u64)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    });

                    if let Some(s) = sz {
                        if s > 0 && best_video_size.map_or(true, |cur| s > cur) {
                            best_video_size = Some(s);
                        }
                    }
                }
            }

            if let Some(v_sz) = best_video_size {
                return Some(v_sz + best_audio_size);
            }
        }

        self.filesize.or(self.filesize_approx)
    }

    pub fn format_filesize(bytes: u64) -> String {
        if bytes >= 1_073_741_824 {
            format!("{:.2} GB", bytes as f64 / 1_073_741_824.0)
        } else if bytes >= 1_048_576 {
            format!("{:.1} MB", bytes as f64 / 1_048_576.0)
        } else if bytes >= 1024 {
            format!("{:.0} KB", bytes as f64 / 1024.0)
        } else {
            format!("{} B", bytes)
        }
    }

    pub fn available_subtitle_languages(&self) -> Vec<String> {
        let mut langs = Vec::new();

        if let Some(subs) = &self.subtitles {
            for k in subs.keys() {
                if !k.starts_with("live_chat") && !langs.contains(k) {
                    langs.push(k.clone());
                }
            }
        }

        if let Some(auto_subs) = &self.automatic_captions {
            for k in auto_subs.keys() {
                if !k.starts_with("live_chat") && !langs.contains(k) {
                    langs.push(k.clone());
                }
            }
        }

        langs.sort();
        langs
    }

    pub fn format_duration(&self) -> String {
        match self.duration {
            Some(d) if d > 0.0 => {
                let total_secs = d as u64;
                let hours = total_secs / 3600;
                let minutes = (total_secs % 3600) / 60;
                let seconds = total_secs % 60;

                if hours > 0 {
                    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
                } else {
                    format!("{:02}:{:02}", minutes, seconds)
                }
            }
            _ => "Unknown".to_string(),
        }
    }
}
