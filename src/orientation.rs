use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Orientation {
    Horizontal,
    Vertical,
    Square,
}

impl Orientation {
    pub fn from_dimensions(width: u32, height: u32) -> Self {
        if width > height {
            Orientation::Horizontal
        } else if height > width {
            Orientation::Vertical
        } else {
            Orientation::Square
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Orientation::Horizontal => "Horizontal (Landscape - 16:9 / 4:3)",
            Orientation::Vertical => "Vertical (Portrait - 9:16 Shorts/TikTok/Reels)",
            Orientation::Square => "Square (1:1)",
        }
    }

    pub fn short_name(&self) -> &'static str {
        match self {
            Orientation::Horizontal => "Horizontal",
            Orientation::Vertical => "Vertical",
            Orientation::Square => "Square",
        }
    }
}
