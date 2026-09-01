use clap::{Args, Parser, Subcommand};
use clap_complete::Shell;

#[derive(Parser, Debug)]
#[command(
    name = "dlp",
    author = "Adam F",
    version = "1.4.0",
    about = "Smart orchestration and CLI wrapper for yt-dlp and ffmpeg",
    long_about = "dlp is an intelligent CLI orchestration layer above yt-dlp and ffmpeg that inspects metadata, detects video orientation, and applies customizable presets and batch downloads."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// URL of the video to download (when no subcommand is used)
    #[arg(value_name = "URL")]
    pub url: Option<String>,

    /// Desired quality tier or resolution (e.g. 'best', '4k', '1440', '1080', '720', '480', 'sd', 'hd')
    #[arg(short = 'q', long = "quality")]
    pub quality: Option<String>,

    /// Use a specific preset (e.g. 'video', 'music', 'tiktok')
    #[arg(short = 'p', long = "preset")]
    pub preset: Option<String>,

    /// Enable or disable lyrics/subtitles fetching
    #[arg(long = "lyrics")]
    pub lyrics: Option<bool>,

    /// Only inspect and display metadata without downloading
    #[arg(short = 'i', long = "info")]
    pub info_only: bool,

    /// Explain the decision trace (policies, format selection, post-processing) without downloading
    #[arg(long = "explain")]
    pub explain: bool,

    /// Custom output directory
    #[arg(short = 'o', long = "output-dir")]
    pub output_dir: Option<String>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// Download video using the 'video' preset
    Video(DownloadArgs),

    /// Extract audio/music using the 'music' preset
    Music(DownloadArgs),

    /// Download vertical video/shorts using the 'tiktok' preset
    Tiktok(DownloadArgs),

    /// Batch download from a file (e.g. urls.txt) or multiple URLs
    Batch(BatchArgs),

    /// List all available presets
    Presets,

    /// Run system diagnostics and verify dependencies
    Doctor,

    /// Generate shell completion scripts (bash, zsh, fish, powershell, elvish)
    Completions(CompletionsArgs),
}

#[derive(Args, Debug, Clone)]
pub struct DownloadArgs {
    /// URL of the media
    #[arg(value_name = "URL")]
    pub url: String,

    /// Override quality tier or resolution
    #[arg(short = 'q', long = "quality")]
    pub quality: Option<String>,

    /// Enable or disable lyrics/subtitles fetching
    #[arg(long = "lyrics")]
    pub lyrics: Option<bool>,

    /// Only inspect and display metadata without downloading
    #[arg(short = 'i', long = "info")]
    pub info_only: bool,

    /// Explain the decision trace (policies, format selection, post-processing) without downloading
    #[arg(long = "explain")]
    pub explain: bool,

    /// Custom output directory
    #[arg(short = 'o', long = "output-dir")]
    pub output_dir: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub struct BatchArgs {
    /// Path to URLs file (e.g. urls.txt) or inline URLs
    #[arg(value_name = "FILE_OR_URLS", required = true)]
    pub inputs: Vec<String>,

    /// Preset to use for the batch (e.g. 'video', 'music', 'tiktok', or 'auto')
    #[arg(short = 'p', long = "preset", default_value = "auto")]
    pub preset: String,

    /// Override quality tier or resolution
    #[arg(short = 'q', long = "quality")]
    pub quality: Option<String>,

    /// Enable or disable lyrics for the batch
    #[arg(long = "lyrics")]
    pub lyrics: Option<bool>,

    /// Custom output directory
    #[arg(short = 'o', long = "output-dir")]
    pub output_dir: Option<String>,

    /// Resume batch download from checkpoint, skipping already downloaded items
    #[arg(long = "resume")]
    pub resume: bool,

    /// Number of concurrent downloads for batch mode (default: 1, sequential)
    #[arg(short = 'c', long = "concurrency")]
    pub concurrency: Option<usize>,
}

#[derive(Args, Debug, Clone)]
pub struct CompletionsArgs {
    /// Target shell for autocompletion
    #[arg(value_enum)]
    pub shell: Shell,
}
