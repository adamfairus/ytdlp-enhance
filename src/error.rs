use thiserror::Error;

#[derive(Debug, Error)]
pub enum DlpError {
    #[error("Missing external dependency: '{0}'. Please ensure it is installed and available in your PATH.")]
    MissingDependency(String),

    #[error("Failed to execute process: {0}")]
    ProcessExecution(#[from] std::io::Error),

    #[error("yt-dlp command failed with exit code {code}: {stderr}")]
    YtDlpFailed { code: i32, stderr: String },

    #[error("Failed to parse metadata JSON: {0}")]
    JsonParse(#[from] serde_json::Error),

    #[error("No URL provided. Use 'dlp <URL>' or run 'dlp' for interactive mode.")]
    MissingUrl,

    #[error("Invalid quality '{0}'. Supported: 'best', '4k', 'uhd', '1440', 'qhd', '1080', 'fhd', '720', 'hd', '480', 'sd', '360'.")]
    InvalidQuality(String),

    #[error("Preset '{0}' not found. Run 'dlp presets' to list all available presets.")]
    PresetNotFound(String),

    #[error("Failed to parse configuration / preset: {0}")]
    ConfigParse(String),

    #[error("Interactive session cancelled.")]
    Cancelled,

    #[error("Terminal prompt error: {0}")]
    PromptError(#[from] inquire::InquireError),
}

pub type Result<T> = std::result::Result<T, DlpError>;
