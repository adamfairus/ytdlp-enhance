use std::process::{Command, Stdio};
use crate::error::{DlpError, Result};
use crate::metadata::VideoMetadata;
use crate::preset::Preset;
use crate::progress::ProgressTracker;
use crate::quality::QualityPreference;
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

    /// Fetch video metadata (TikWM as primary for TikTok, yt-dlp for all other sources)
    pub fn fetch_metadata(url: &str) -> Result<VideoMetadata> {
        if TikTokFallback::is_tiktok_url(url) {
            if let Ok(meta) = TikTokFallback::fetch_metadata(url) {
                return Ok(meta);
            }
            println!("⚠️  TikWM metadata fetch failed. Falling back to yt-dlp with impersonation...");
        }

        let mut cmd = Command::new("yt-dlp");
        cmd.arg("--dump-single-json").arg("--no-playlist");

        if TikTokFallback::is_tiktok_url(url) {
            cmd.arg("--impersonate").arg("chrome");
        }

        cmd.arg(url);

        let output = cmd.output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let code = output.status.code().unwrap_or(-1);
            return Err(DlpError::YtDlpFailed { code, stderr });
        }

        let json_str = String::from_utf8_lossy(&output.stdout);
        VideoMetadata::from_json(&json_str)
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

    fn download_via_ytdlp(
        url: &str,
        preset: &Preset,
        effective_quality: &QualityPreference,
        override_output_dir: Option<&str>,
    ) -> Result<()> {
        let mut cmd = Command::new("yt-dlp");

        if TikTokFallback::is_tiktok_url(url) {
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
                .arg("(?i)\\s*[\\(\\[](?:official\\s*(?:video|audio|music\\s*video|lyric\\s*video|visualizer|mv)?|mv|performance\\s*video|audio|lyrics?|color\\s*coded\\s*lyrics?)[\\)\\]]")
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

        // Custom clean progress rendering
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::null());

        let mut child = cmd.spawn()?;
        let stdout = child.stdout.take().ok_or_else(|| {
            DlpError::ProcessExecution(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Could not capture yt-dlp stdout",
            ))
        })?;

        let mut tracker = ProgressTracker::new();
        tracker.process_stream(stdout);

        let status = child.wait()?;

        if !status.success() {
            let code = status.code().unwrap_or(-1);
            return Err(DlpError::YtDlpFailed {
                code,
                stderr: "Download process failed.".to_string(),
            });
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
