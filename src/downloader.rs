use std::io::Read;
use std::process::{Command, Stdio};
use std::thread;
use crate::error::{DlpError, Result};
use crate::metadata::VideoMetadata;
use crate::preset::Preset;
use crate::progress::ProgressTracker;
use crate::quality::QualityPreference;
use crate::recovery::{sleep_with_backoff, DiagnosticReport, FailureCategory};
use crate::tiktok::TikTokFallback;

pub struct Downloader;

impl Downloader {
    /// Verify that essential external binaries (yt-dlp and ffmpeg) are installed and accessible.
    pub fn verify_dependencies() -> Result<()> {
        Self::check_binary("yt-dlp", &["--version"])?;
        Self::check_binary("ffmpeg", &["-version"])?;
        Ok(())
    }

    fn check_binary(bin: &str, test_args: &[&str]) -> Result<()> {
        let output = Command::new(bin)
            .args(test_args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();

        match output {
            Ok(status) if status.success() => Ok(()),
            _ => Err(DlpError::MissingDependency(bin.to_string())),
        }
    }

    /// Fetch video metadata with automatic transient retry & anti-bot protection.
    pub fn fetch_metadata(url: &str) -> Result<VideoMetadata> {
        if TikTokFallback::is_tiktok_url(url) {
            if let Ok(meta) = TikTokFallback::fetch_metadata(url) {
                return Ok(meta);
            }
            println!("⚠️  TikWM metadata fetch failed. Falling back to yt-dlp with impersonation...");
        }

        let mut impersonate_client: Option<&str> = if TikTokFallback::is_tiktok_url(url) {
            Some("chrome")
        } else {
            None
        };
        let mut attempt = 0u32;

        loop {
            let mut cmd = Command::new("yt-dlp");
            cmd.arg("--dump-single-json").arg("--no-playlist");

            if let Some(client) = impersonate_client {
                cmd.arg("--impersonate").arg(client);
            }

            cmd.arg(url);

            let output = cmd.output()?;

            if output.status.success() {
                let json_str = String::from_utf8_lossy(&output.stdout);
                return VideoMetadata::from_json(&json_str);
            }

            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let code = output.status.code().unwrap_or(-1);
            let category = FailureCategory::classify(&stderr);

            if category.is_retryable() && attempt < 2 {
                attempt += 1;
                match &category {
                    FailureCategory::BotBlockOrExtractor { .. } => {
                        println!("⚠️  Anti-bot challenge during metadata fetch. Rotating TLS fingerprint...");
                        impersonate_client = Some("safari-18");
                        continue;
                    }
                    FailureCategory::Transient { .. } => {
                        println!("⚠️  Transient network issue during metadata fetch. Retrying in 1s...");
                        thread::sleep(std::time::Duration::from_secs(1));
                        continue;
                    }
                    _ => {}
                }
            }

            let report = DiagnosticReport::new(category, None);
            report.print_block();
            return Err(DlpError::YtDlpFailed { code, stderr });
        }
    }

    /// Execute the download process (TikWM as primary for TikTok, clean custom progress bar)
    pub fn download(
        url: &str,
        preset: &Preset,
        effective_quality: &QualityPreference,
        override_output_dir: Option<&str>,
    ) -> Result<()> {
        if TikTokFallback::is_tiktok_url(url) {
            match TikTokFallback::download(url, override_output_dir) {
                Ok(_) => return Ok(()),
                Err(e) => {
                    println!("⚠️  TikWM download failed: {e}. Engaging yt-dlp 10-client impersonation rotation...");
                    return TikTokFallback::download_with_impersonation_rotation(
                        url,
                        preset,
                        effective_quality,
                        override_output_dir,
                    );
                }
            }
        }

        Self::download_via_ytdlp(url, preset, effective_quality, override_output_dir)
    }

    /// Self-healing download executor with intelligent retry, format fallback, and TLS rotation.
    fn download_via_ytdlp(
        url: &str,
        preset: &Preset,
        effective_quality: &QualityPreference,
        override_output_dir: Option<&str>,
    ) -> Result<()> {
        let mut current_quality = effective_quality.clone();
        let mut impersonate_override: Option<String> = None;
        let mut transient_attempts = 0u32;
        let max_transient_retries = 3u32;

        loop {
            let attempt_result = Self::execute_single_download(
                url,
                preset,
                &current_quality,
                override_output_dir,
                impersonate_override.as_deref(),
            );

            match attempt_result {
                Ok(()) => return Ok(()),
                Err(DlpError::YtDlpFailed { code, stderr }) => {
                    let category = FailureCategory::classify(&stderr);

                    match category {
                        FailureCategory::FormatUnavailable { requested, details } => {
                            if let Some(next_q) = current_quality.fallback_step() {
                                let fallback_desc = match &next_q {
                                    QualityPreference::Best => "best available stream".to_string(),
                                    QualityPreference::SpecificHeight(h) => format!("{}p resolution", h),
                                };
                                let report = DiagnosticReport::new(
                                    FailureCategory::FormatUnavailable { requested, details },
                                    Some(format!("Fallback to {} → retrying...", fallback_desc)),
                                );
                                report.print_block();
                                current_quality = next_q;
                                continue;
                            } else {
                                let report = DiagnosticReport::new(
                                    FailureCategory::FormatUnavailable { requested, details },
                                    Some("No further format fallbacks available.".to_string()),
                                );
                                report.print_block();
                                return Err(DlpError::YtDlpFailed { code, stderr });
                            }
                        }
                        FailureCategory::Transient { reason } => {
                            if transient_attempts < max_transient_retries {
                                transient_attempts += 1;
                                let report = DiagnosticReport::new(
                                    FailureCategory::Transient { reason },
                                    Some(format!(
                                        "Attempt {}/{} failed. Backing off...",
                                        transient_attempts, max_transient_retries
                                    )),
                                );
                                report.print_block();
                                sleep_with_backoff(transient_attempts, 2);
                                continue;
                            } else {
                                let report = DiagnosticReport::new(
                                    FailureCategory::Transient { reason },
                                    Some("Max transient retries exceeded. Aborting.".to_string()),
                                );
                                report.print_block();
                                return Err(DlpError::YtDlpFailed { code, stderr });
                            }
                        }
                        FailureCategory::BotBlockOrExtractor { reason } => {
                            let next_impersonate = match impersonate_override.as_deref() {
                                None => Some("safari-18"),
                                Some("safari-18") => Some("chrome-136"),
                                Some("chrome-136") => Some("firefox-135"),
                                _ => None,
                            };

                            if let Some(client) = next_impersonate {
                                let report = DiagnosticReport::new(
                                    FailureCategory::BotBlockOrExtractor { reason },
                                    Some(format!("Rotating TLS client fingerprint to '{}'...", client)),
                                );
                                report.print_block();
                                impersonate_override = Some(client.to_string());
                                continue;
                            } else {
                                let report = DiagnosticReport::new(
                                    FailureCategory::BotBlockOrExtractor { reason },
                                    Some("All anti-bot impersonation strategies exhausted.".to_string()),
                                );
                                report.print_block();
                                return Err(DlpError::YtDlpFailed { code, stderr });
                            }
                        }
                        FailureCategory::Permanent { reason } => {
                            let report = DiagnosticReport::new(FailureCategory::Permanent { reason }, None);
                            report.print_block();
                            return Err(DlpError::YtDlpFailed { code, stderr });
                        }
                        FailureCategory::FFmpegProcessing { reason } => {
                            let report = DiagnosticReport::new(FailureCategory::FFmpegProcessing { reason }, None);
                            report.print_block();
                            return Err(DlpError::YtDlpFailed { code, stderr });
                        }
                        FailureCategory::Unknown(reason) => {
                            let report = DiagnosticReport::new(FailureCategory::Unknown(reason), None);
                            report.print_block();
                            return Err(DlpError::YtDlpFailed { code, stderr });
                        }
                    }
                }
                Err(other_err) => return Err(other_err),
            }
        }
    }

    /// Single execution run of yt-dlp with simultaneous stdout progress parsing and stderr error capturing.
    fn execute_single_download(
        url: &str,
        preset: &Preset,
        effective_quality: &QualityPreference,
        override_output_dir: Option<&str>,
        impersonate_override: Option<&str>,
    ) -> Result<()> {
        let mut cmd = Command::new("yt-dlp");

        if let Some(imp) = impersonate_override {
            cmd.arg("--impersonate").arg(imp);
        } else if TikTokFallback::is_tiktok_url(url) {
            cmd.arg("--impersonate").arg("chrome");
        }

        if preset.extract_audio {
            cmd.arg("--ignore-config");
            cmd.arg("-x");
            let audio_fmt = preset.audio_format.as_deref().unwrap_or("opus");
            cmd.arg("--audio-format").arg(audio_fmt);

            if let Some(audio_q) = &preset.audio_quality {
                cmd.arg("--audio-quality").arg(audio_q);
            }
            cmd.arg("-f").arg("251/140/bestaudio/best");
            cmd.arg("--remux-video").arg(audio_fmt);
        } else {
            let format_selector = effective_quality.to_format_selector();
            cmd.arg("-f").arg(&format_selector);
            cmd.arg("--merge-output-format").arg(&preset.container);
            cmd.arg("--remux-video").arg(&preset.container);
        }

        cmd.arg("--windows-filenames");

        if preset.embed_metadata {
            cmd.arg("--embed-metadata");
        }

        if preset.embed_thumbnail {
            cmd.arg("--embed-thumbnail");
        }

        if preset.crop_thumbnail_square {
            cmd.arg("--convert-thumbnails").arg("jpg");
            cmd.arg("--postprocessor-args")
                .arg("ThumbnailsConvertor+ffmpeg_o:-vf crop=ih:ih -c:v mjpeg");
            cmd.arg("--postprocessor-args")
                .arg("EmbedThumbnail:-map_metadata 0");
        }

        if preset.clean_metadata {
            cmd.arg("--replace-in-metadata")
                .arg("uploader,artist")
                .arg(" - Topic$")
                .arg("");

            cmd.arg("--replace-in-metadata")
                .arg("title")
                .arg("(?i)\\s*[\\(\\[](?:official\\s*(?:video|audio|music\\s*video|lyric\\s*video|visualizer|mv)?|mv|performance\\s*video|audio|lyrics?|color\\s*coded\\s*lyrics?|remastered(?:\\s*\\d+)?|\\d+k\\s*remaster)[\\)\\]]")
                .arg("");
        }

        // Subtitles / Lyrics: ALWAYS generate standalone sidecar files (.srt for video, .lrc for music). NEVER embed.
        if preset.write_lyrics || preset.sub_langs.is_some() {
            cmd.arg("--write-subs")
                .arg("--write-auto-subs");

            if let Some(langs) = &preset.sub_langs {
                if !langs.is_empty() {
                    cmd.arg("--sub-langs").arg(langs.join(","));
                } else {
                    cmd.arg("--sub-langs").arg("all,-live_chat");
                }
            } else {
                cmd.arg("--sub-langs").arg("all,-live_chat");
            }

            let default_sub_ext = if preset.extract_audio { "lrc" } else { "srt" };
            let sub_fmt = preset.lyrics_format.as_deref().unwrap_or(default_sub_ext);
            cmd.arg("--sub-format").arg(format!("{}/best", sub_fmt));
            cmd.arg("--convert-subs").arg(sub_fmt);
        }

        if preset.parse_music_tags {
            cmd.arg("--parse-metadata")
                .arg("playlist_index:%(track_number)s");
            cmd.arg("--parse-metadata")
                .arg("%(artist,uploader)s:^(?P<main_artist>[^,]+)");
            cmd.arg("--parse-metadata")
                .arg("%(main_artist,artist,uploader)s:%(meta_artist)s");
            cmd.arg("--parse-metadata")
                .arg("%(main_artist,artist,uploader)s:%(meta_album_artist)s");
            cmd.arg("--parse-metadata")
                .arg("%(album,playlist_title)s:%(meta_album)s");
            cmd.arg("--parse-metadata")
                .arg("%(genre)s:%(meta_genre)s");
            cmd.arg("--parse-metadata")
                .arg("%(disc_number)s:%(meta_disc)s");
            cmd.arg("--parse-metadata")
                .arg("%(release_date>%Y-%m-%d,upload_date>%Y-%m-%d,timestamp>%Y-%m-%d)s:%(meta_date)s");
            cmd.arg("--parse-metadata")
                .arg("%(release_date>%Y,upload_date>%Y,timestamp>%Y)s:%(meta_year)s");
            cmd.arg("--parse-metadata")
                .arg("%(release_timestamp>%Y-%m-%d %H\\:%M\\:%S,timestamp>%Y-%m-%d %H\\:%M\\:%S,upload_date>%Y-%m-%d 00\\:00\\:00)s:%(meta_creation_time)s");
        }

        cmd.arg("--no-playlist");
        cmd.arg("--newline");
        cmd.arg("--progress");
        cmd.arg("--progress-template")
            .arg("download:__DLP__:%(progress._percent_str)s|%(progress._speed_str)s|%(progress._eta_str)s|%(progress._total_bytes_str,progress._total_bytes_estimate_str)s");

        if let Some(template) = &preset.output_template {
            cmd.arg("-o").arg(template);
        }

        let target_dir = override_output_dir.or(preset.output_dir.as_deref());
        if let Some(dir) = target_dir {
            cmd.arg("-P").arg(dir);
        }

        cmd.arg(url);
        cmd.env("PYTHONUNBUFFERED", "1");

        // Custom clean progress rendering on stdout + thread capture on stderr
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = cmd.spawn()?;
        let stdout = child.stdout.take().ok_or_else(|| {
            DlpError::ProcessExecution(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Could not capture yt-dlp stdout",
            ))
        })?;

        let stderr_pipe = child.stderr.take();
        let stderr_handle = thread::spawn(move || {
            let mut buf = String::new();
            if let Some(mut err) = stderr_pipe {
                let _ = err.read_to_string(&mut buf);
            }
            buf
        });

        let mut tracker = ProgressTracker::new();
        tracker.process_stream(stdout);

        let status = child.wait()?;
        let stderr = stderr_handle.join().unwrap_or_default();

        if !status.success() {
            let code = status.code().unwrap_or(-1);
            return Err(DlpError::YtDlpFailed { code, stderr });
        }

        let has_subs = preset.write_lyrics || preset.sub_langs.is_some();
        tracker.finish_and_print_checklist(
            preset.embed_metadata,
            preset.embed_thumbnail,
            has_subs,
            None,
        );

        Ok(())
    }
}
