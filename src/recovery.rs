use std::fmt;
use std::thread;
use std::time::Duration;

/// Categorization of download/extraction failures for intelligent self-healing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FailureCategory {
    /// Transient network error (timeout, connection reset, HTTP 5xx, socket disconnect)
    Transient { reason: String },
    /// Requested format code or resolution stream is not available
    FormatUnavailable {
        requested: Option<String>,
        details: String,
    },
    /// Anti-bot protection, rate limit (HTTP 429), or 403 Forbidden challenge
    BotBlockOrExtractor { reason: String },
    /// FFmpeg conversion, muxing, or post-processing failure
    FFmpegProcessing { reason: String },
    /// Permanent error (video deleted, private, geo-blocked, copyright claim)
    Permanent { reason: String },
    /// General unclassified failure
    Unknown(String),
}

impl FailureCategory {
    /// Parse stderr output from yt-dlp/ffmpeg and categorize the failure.
    pub fn classify(stderr: &str) -> Self {
        let lower = stderr.to_lowercase();

        // 1. Permanent errors (unrecoverable without user credentials or different URL)
        if lower.contains("video unavailable")
            || lower.contains("this video has been removed")
            || lower.contains("private video")
            || lower.contains("this video is private")
            || lower.contains("copyright claim")
            || lower.contains("account has been terminated")
            || lower.contains("geo-restricted")
            || lower.contains("not available in your country")
            || lower.contains("confirm your age")
            || lower.contains("sign in to confirm your age")
            || lower.contains("members-only content")
            || lower.contains("is not a valid url")
        {
            let reason = Self::extract_primary_error(stderr)
                .unwrap_or_else(|| "Media is permanently unavailable, private, or restricted".to_string());
            return FailureCategory::Permanent { reason };
        }

// 2. Format Unavailable
        if (lower.contains("format") || lower.contains("formats"))
            && (lower.contains("not available")
                || lower.contains("unavailable")
                || lower.contains("no video formats found")
                || lower.contains("could not find format"))
        {
            let requested = Self::extract_format_code(stderr);
            let details = Self::extract_primary_error(stderr)
                .unwrap_or_else(|| "Requested format is not available on this stream".to_string());
            return FailureCategory::FormatUnavailable { requested, details };
        }

        // 3. Bot challenge / Extractor block
        if lower.contains("sign in to confirm you're not a bot")
            || lower.contains("bot detection")
            || lower.contains("http error 429")
            || lower.contains("too many requests")
            || lower.contains("http error 403")
            || lower.contains("forbidden")
            || lower.contains("blocked by cloudflare")
        {
            let reason = Self::extract_primary_error(stderr)
                .unwrap_or_else(|| "Anti-bot challenge or rate limit triggered".to_string());
            return FailureCategory::BotBlockOrExtractor { reason };
        }

        // 4. Transient network errors
        if lower.contains("timed out")
            || lower.contains("connection reset")
            || lower.contains("network is unreachable")
            || lower.contains("temporary failure in name resolution")
            || lower.contains("http error 500")
            || lower.contains("http error 502")
            || lower.contains("http error 503")
            || lower.contains("http error 504")
            || lower.contains("incompleteread")
            || lower.contains("connection refused")
            || lower.contains("read timed out")
            || lower.contains("remote end closed connection")
        {
            let reason = Self::extract_primary_error(stderr)
                .unwrap_or_else(|| "Transient network timeout or server error".to_string());
            return FailureCategory::Transient { reason };
        }

        // 5. FFmpeg processing error
        if lower.contains("ffmpeg: error")
            || lower.contains("conversion failed")
            || lower.contains("postprocessing: error")
            || lower.contains("postprocessing failed")
            || lower.contains("failed to merge formats")
        {
            let reason = Self::extract_primary_error(stderr)
                .unwrap_or_else(|| "FFmpeg post-processing / remuxing failed".to_string());
            return FailureCategory::FFmpegProcessing { reason };
        }

        // 6. Fallback to unknown
        let reason = Self::extract_primary_error(stderr).unwrap_or_else(|| stderr.trim().to_string());
        FailureCategory::Unknown(reason)
    }

    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            FailureCategory::Transient { .. }
                | FailureCategory::FormatUnavailable { .. }
                | FailureCategory::BotBlockOrExtractor { .. }
        )
    }

    fn extract_primary_error(stderr: &str) -> Option<String> {
        for line in stderr.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("ERROR:") {
                return Some(rest.trim().to_string());
            }
            if let Some(rest) = trimmed.strip_prefix("[download] Got error:") {
                return Some(rest.trim().to_string());
            }
            if trimmed.contains("yt_dlp.utils") {
                if let Some(pos) = trimmed.find(':') {
                    return Some(trimmed[pos + 1..].trim().to_string());
                }
            }
        }

        stderr
            .lines()
            .map(|l| l.trim())
            .find(|l| !l.is_empty())
            .map(|s| s.to_string())
    }

    fn extract_format_code(stderr: &str) -> Option<String> {
        for line in stderr.lines() {
            if let Some(idx) = line.find("format") {
                let slice = &line[idx..];
                for word in slice.split_whitespace() {
                    let cleaned = word.trim_matches(|c: char| !c.is_ascii_digit());
                    if !cleaned.is_empty() && cleaned.chars().all(|c| c.is_ascii_digit()) {
                        return Some(cleaned.to_string());
                    }
                }
            }
        }
        None
    }
}

/// Structured diagnostic report for clear, user-friendly terminal output.
#[derive(Debug, Clone)]
pub struct DiagnosticReport {
    pub category: FailureCategory,
    pub fallback_action: Option<String>,
}

impl DiagnosticReport {
    pub fn new(category: FailureCategory, fallback_action: Option<String>) -> Self {
        Self {
            category,
            fallback_action,
        }
    }

    pub fn print_block(&self) {
        println!("\n╔══════════════════════════════════════════════════╗");
        println!("║            ⚠️  DIAGNOSTIC & RECOVERY             ║");
        println!("╠══════════════════════════════════════════════════╣");

        match &self.category {
            FailureCategory::Transient { reason } => {
                println!("║ Type    : Transient Network Error                ║");
                println!("║ Reason  : {:<39}║", truncate_str(reason, 39));
                if let Some(action) = &self.fallback_action {
                    println!("║ Action  : {:<39}║", truncate_str(action, 39));
                }
            }
            FailureCategory::FormatUnavailable { requested, details } => {
                println!("║ Type    : Requested Format Unavailable           ║");
                if let Some(req) = requested {
                    println!("║ Format  : Code {:<34}║", truncate_str(req, 34));
                }
                println!("║ Details : {:<39}║", truncate_str(details, 39));
                if let Some(action) = &self.fallback_action {
                    println!("║ Fallback: {:<39}║", truncate_str(action, 39));
                }
            }
            FailureCategory::BotBlockOrExtractor { reason } => {
                println!("║ Type    : Anti-Bot Challenge / Rate Limit        ║");
                println!("║ Reason  : {:<39}║", truncate_str(reason, 39));
                if let Some(action) = &self.fallback_action {
                    println!("║ Action  : {:<39}║", truncate_str(action, 39));
                }
            }
            FailureCategory::FFmpegProcessing { reason } => {
                println!("║ Type    : FFmpeg Processing Error                ║");
                println!("║ Reason  : {:<39}║", truncate_str(reason, 39));
                if let Some(action) = &self.fallback_action {
                    println!("║ Action  : {:<39}║", truncate_str(action, 39));
                }
            }
            FailureCategory::Permanent { reason } => {
                println!("║ Type    : Permanent Error (Unrecoverable)        ║");
                println!("║ Reason  : {:<39}║", truncate_str(reason, 39));
                println!("║ Action  : Skipping item without retry            ║");
            }
            FailureCategory::Unknown(reason) => {
                println!("║ Type    : Unclassified Download Error            ║");
                println!("║ Reason  : {:<39}║", truncate_str(reason, 39));
                if let Some(action) = &self.fallback_action {
                    println!("║ Action  : {:<39}║", truncate_str(action, 39));
                }
            }
        }

        println!("╚══════════════════════════════════════════════════╝\n");
    }
}

impl fmt::Display for DiagnosticReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.category {
            FailureCategory::Transient { reason } => {
                write!(f, "Transient Network Error: {}", reason)
            }
            FailureCategory::FormatUnavailable { requested, details } => {
                if let Some(req) = requested {
                    write!(f, "Format Unavailable ({}): {}", req, details)
                } else {
                    write!(f, "Format Unavailable: {}", details)
                }
            }
            FailureCategory::BotBlockOrExtractor { reason } => {
                write!(f, "Anti-Bot/Rate-Limit: {}", reason)
            }
            FailureCategory::FFmpegProcessing { reason } => {
                write!(f, "FFmpeg Error: {}", reason)
            }
            FailureCategory::Permanent { reason } => {
                write!(f, "Permanent Error: {}", reason)
            }
            FailureCategory::Unknown(reason) => {
                write!(f, "Error: {}", reason)
            }
        }
    }
}

/// Helper function to perform exponential backoff sleep with user feedback.
pub fn sleep_with_backoff(attempt: u32, base_delay_secs: u64) {
    let delay = base_delay_secs * 2u64.pow(attempt.saturating_sub(1));
    println!("⏳ Backing off for {}s before retrying (attempt {})...", delay, attempt);
    thread::sleep(Duration::from_secs(delay));
}

fn truncate_str(s: &str, max_len: usize) -> String {
    let clean = s.replace('\n', " ");
    if clean.chars().count() <= max_len {
        clean
    } else {
        let mut truncated: String = clean.chars().take(max_len.saturating_sub(3)).collect();
        truncated.push_str("...");
        truncated
    }
}
