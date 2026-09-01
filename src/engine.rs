use std::io::Read;
use std::process::{Command, Stdio};
use std::thread;
use crate::error::{DlpError, Result};
use crate::event::{DownloadEvent, EventDispatcher};
use crate::metadata::VideoMetadata;
use crate::preset::Preset;
use crate::progress::ProgressTracker;
use crate::quality::QualityPreference;
use crate::recovery::{DiagnosticReport, FailureCategory, FailureContext, RecoveryAction, RecoveryPolicy};
use crate::tiktok::TikTokFallback;

pub struct YtDlpEngine;

impl YtDlpEngine {
    /// Verify that essential external binaries (yt-dlp and ffmpeg) are installed and accessible.
    pub fn verify_dependencies() -> Result<()> {
        Self::check_binary("yt-dlp", &["--version"])?;
        Self::check_binary("ffmpeg", &["-version"])?;
        Ok(())
    }

    /// Check if a binary can be executed successfully with the provided test arguments.
    pub fn check_binary(bin: &str, test_args: &[&str]) -> Result<()> {
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

    /// Fetch metadata via yt-dlp with automatic transient retry & anti-bot protection.
    pub fn fetch_metadata(url: &str, impersonate_client: Option<&str>) -> Result<VideoMetadata> {
        Self::fetch_metadata_with_dispatcher(url, impersonate_client, None)
    }

    /// Fetch metadata via yt-dlp with optional event dispatching on success.
    pub fn fetch_metadata_with_dispatcher(
        url: &str,
        impersonate_client: Option<&str>,
        dispatcher: Option<&EventDispatcher>,
    ) -> Result<VideoMetadata> {
        let mut impersonate_client = impersonate_client;
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
                let meta = VideoMetadata::from_json(&json_str)?;
                if let Some(d) = dispatcher {
                    d.dispatch(&DownloadEvent::MetadataFetched {
                        url: url.to_string(),
                        title: meta.title.clone(),
                    });
                }
                return Ok(meta);
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

    /// Self-healing download executor with intelligent retry, format fallback, and TLS rotation.
    pub fn download(
        url: &str,
        preset: &Preset,
        effective_quality: &QualityPreference,
        override_output_dir: Option<&str>,
    ) -> Result<()> {
        Self::download_with_dispatcher(url, preset, effective_quality, override_output_dir, None)
    }

    /// Self-healing download executor with event dispatching to registered listeners.
    pub fn download_with_dispatcher(
        url: &str,
        preset: &Preset,
        effective_quality: &QualityPreference,
        override_output_dir: Option<&str>,
        dispatcher: Option<&EventDispatcher>,
    ) -> Result<()> {
        let format_desc = if preset.extract_audio {
            preset.audio_format.as_deref().unwrap_or("opus").to_string()
        } else {
            effective_quality.to_format_selector()
        };

        if let Some(d) = dispatcher {
            d.dispatch(&DownloadEvent::DownloadStarted {
                url: url.to_string(),
                format: format_desc,
            });
        }

        let policy = RecoveryPolicy::default();
        let mut current_quality = effective_quality.clone();
        let mut impersonate_override: Option<String> = None;
        let mut transient_attempts = 0u32;

        loop {
            let attempt_result = Self::execute_single_download(
                url,
                preset,
                &current_quality,
                override_output_dir,
                impersonate_override.as_deref(),
            );

            match attempt_result {
                Ok(()) => {
                    if let Some(d) = dispatcher {
                        d.dispatch(&DownloadEvent::Completed {
                            url: url.to_string(),
                            output_path: override_output_dir
                                .or(preset.output_dir.as_deref())
                                .unwrap_or(".")
                                .to_string(),
                        });
                    }
                    return Ok(());
                }
                Err(DlpError::YtDlpFailed { code, stderr }) => {
                    let category = FailureCategory::classify(&stderr);
                    let ctx = FailureContext::new(Some(code), &stderr, "download", transient_attempts + 1);
                    let action = policy.decide(&ctx, &category, &current_quality, impersonate_override.as_deref());

                    match action {
                        RecoveryAction::FallbackFormat { next_quality, reason: _ } => {
                            let from_str = match &current_quality {
                                QualityPreference::Best => "best".to_string(),
                                QualityPreference::SpecificHeight(h) => format!("{}p", h),
                            };
                            let to_str = match &next_quality {
                                QualityPreference::Best => "best".to_string(),
                                QualityPreference::SpecificHeight(h) => format!("{}p", h),
                            };
                            if let Some(d) = dispatcher {
                                d.dispatch(&DownloadEvent::Fallback {
                                    from_quality: from_str,
                                    to_quality: to_str,
                                });
                            }

                            let fallback_desc = match &next_quality {
                                QualityPreference::Best => "best available stream".to_string(),
                                QualityPreference::SpecificHeight(h) => format!("{}p resolution", h),
                            };
                            let report = DiagnosticReport::new(
                                category,
                                Some(format!("Fallback to {} → retrying...", fallback_desc)),
                            );
                            report.print_block();
                            current_quality = next_quality;
                            continue;
                        }
                        RecoveryAction::RetryWithBackoff { delay_secs, reason } => {
                            transient_attempts += 1;
                            if let Some(d) = dispatcher {
                                d.dispatch(&DownloadEvent::Retry {
                                    attempt: transient_attempts,
                                    max_retries: policy.max_transient_retries,
                                    reason: reason.clone(),
                                });
                            }

                            let report = DiagnosticReport::new(
                                category,
                                Some(format!(
                                    "Attempt {}/{} failed. Backing off...",
                                    transient_attempts, policy.max_transient_retries
                                )),
                            );
                            report.print_block();
                            println!("⏳ Backing off for {}s before retrying (attempt {})...", delay_secs, transient_attempts);
                            thread::sleep(std::time::Duration::from_secs(delay_secs));
                            continue;
                        }
                        RecoveryAction::RotateImpersonation { client, reason: _ } => {
                            let report = DiagnosticReport::new(
                                category,
                                Some(format!("Rotating TLS client fingerprint to '{}'...", client)),
                            );
                            report.print_block();
                            impersonate_override = Some(client);
                            continue;
                        }
                        RecoveryAction::SkipPermanent { reason: _ } => {
                            let report = DiagnosticReport::new(category, None);
                            report.print_block();
                            if let Some(d) = dispatcher {
                                d.dispatch(&DownloadEvent::Failed {
                                    url: url.to_string(),
                                    error: stderr.clone(),
                                });
                            }
                            return Err(DlpError::YtDlpFailed { code, stderr });
                        }
                        RecoveryAction::Abort { reason } => {
                            let report = DiagnosticReport::new(category, Some(reason.clone()));
                            report.print_block();
                            if let Some(d) = dispatcher {
                                d.dispatch(&DownloadEvent::Failed {
                                    url: url.to_string(),
                                    error: reason,
                                });
                            }
                            return Err(DlpError::YtDlpFailed { code, stderr });
                        }
                    }
                }
                Err(other_err) => {
                    if let Some(d) = dispatcher {
                        d.dispatch(&DownloadEvent::Failed {
                            url: url.to_string(),
                            error: other_err.to_string(),
                        });
                    }
                    return Err(other_err);
                }
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
