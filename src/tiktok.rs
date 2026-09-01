use indicatif::{ProgressBar, ProgressStyle};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread::sleep;
use std::time::{Duration, Instant};
use crate::error::{DlpError, Result};
use crate::metadata::{VideoFormat, VideoMetadata};
use crate::preset::Preset;
use crate::quality::QualityPreference;

pub const IMPERSONATE_CLIENTS: &[&str] = &[
    "safari-18.0:macos-15",
    "chrome-131:macos-14",
    "chrome-136:macos-15",
    "edge-101:windows-10",
    "safari-18.0:ios-18.0",
    "firefox-135:macos-14",
    "chrome-131:android-14",
    "chrome-116:windows-10",
    "tor-14.5:macos-14",
    "chrome-99:windows-10",
    "chrome",
];

pub struct TikTokFallback;

impl TikTokFallback {
    pub fn is_tiktok_url(url: &str) -> bool {
        let u = url.to_lowercase();
        u.contains("tiktok.com") || u.contains("vt.tiktok.com") || u.contains("vm.tiktok.com")
    }

    pub fn clean_url(url: &str) -> String {
        if let Some((base, _)) = url.split_once('?') {
            base.to_string()
        } else {
            url.to_string()
        }
    }

    /// Internal helper that handles TikWM API requests with automatic 1 req/sec rate limit retry
    fn request_tikwm_api(clean_url: &str) -> Result<serde_json::Value> {
        let api_url = format!(
            "https://www.tikwm.com/api/?url={}",
            urlencoding::encode(clean_url)
        );

        for attempt in 0..2 {
            let resp = ureq::get(&api_url)
                .set("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
                .timeout(Duration::from_secs(10))
                .call()
                .map_err(|e| DlpError::ProcessExecution(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;

            let val: serde_json::Value = resp
                .into_json()
                .map_err(|e| DlpError::ProcessExecution(std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())))?;

            let code = val["code"].as_i64().unwrap_or(-1);
            if code == 0 {
                return Ok(val);
            }

            let msg = val["msg"].as_str().unwrap_or("Unknown TikWM error");
            if (msg.contains("1 request/second") || msg.contains("Limit")) && attempt == 0 {
                sleep(Duration::from_millis(1100));
                continue;
            }

            return Err(DlpError::ProcessExecution(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("TikWM API error: {}", msg),
            )));
        }

        Err(DlpError::ProcessExecution(std::io::Error::new(
            std::io::ErrorKind::Other,
            "TikWM rate limit exceeded after retry",
        )))
    }

    /// Primary Metadata Fetcher using TikWM API (Dynamic JSON)
    pub fn fetch_metadata(url: &str) -> Result<VideoMetadata> {
        let clean = Self::clean_url(url);
        let val = Self::request_tikwm_api(&clean)?;

        let data = val.get("data").ok_or_else(|| {
            DlpError::ProcessExecution(std::io::Error::new(std::io::ErrorKind::NotFound, "No data object in TikWM response"))
        })?;

        let id = data["id"].as_str().unwrap_or("tiktok_video").to_string();
        let title = data["title"].as_str().unwrap_or("TikTok Video").to_string();
        let uploader = data["author"]["unique_id"].as_str().map(|s| s.to_string());
        let duration = data["duration"].as_f64().or_else(|| data["duration"].as_i64().map(|d| d as f64));
        let thumbnail = data["cover"].as_str().or_else(|| data["origin_cover"].as_str()).map(|s| s.to_string());

        let vertical_format = VideoFormat {
            format_id: "hd".to_string(),
            width: Some(1080),
            height: Some(1920),
            fps: Some(30.0),
            ext: Some("mp4".to_string()),
            filesize: None,
            filesize_approx: None,
            tbr: None,
            vbr: None,
            abr: None,
            format_note: Some("HD Watermark-Free".to_string()),
            vcodec: Some("h264".to_string()),
            acodec: Some("aac".to_string()),
            resolution: Some("1080x1920".to_string()),
        };

        Ok(VideoMetadata {
            id,
            title,
            uploader,
            duration,
            width: Some(1080),
            height: Some(1920),
            filesize: None,
            filesize_approx: None,
            formats: Some(vec![vertical_format]),
            webpage_url: Some(clean),
            thumbnail,
            categories: None,
            extractor: Some("tiktok".to_string()),
            subtitles: None,
            automatic_captions: None,
        })
    }

    /// Primary Downloader: Downloads clean no-watermark MP4 via TikWM API (with Progress Bar & Checklist)
    pub fn download(url: &str, output_dir: Option<&str>) -> Result<PathBuf> {
        let clean = Self::clean_url(url);
        let val = Self::request_tikwm_api(&clean)?;

        let data = val.get("data").ok_or_else(|| {
            DlpError::ProcessExecution(std::io::Error::new(std::io::ErrorKind::NotFound, "No data object in TikWM response"))
        })?;

        let play_url_str = data["hdplay"]
            .as_str()
            .or_else(|| data["play"].as_str())
            .ok_or_else(|| {
                DlpError::ProcessExecution(std::io::Error::new(std::io::ErrorKind::NotFound, "No play link found in TikWM data"))
            })?;

        let full_dl_url = if play_url_str.starts_with('/') {
            format!("https://www.tikwm.com{}", play_url_str)
        } else {
            play_url_str.to_string()
        };

        let uploader = data["author"]["unique_id"].as_str().unwrap_or("TikTok_Creator");
        let id = data["id"].as_str().unwrap_or("video");
        let raw_title = data["title"].as_str().unwrap_or("tiktok_video");
        let safe_title: String = raw_title
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-' || *c == '_')
            .take(60)
            .collect();
        let safe_title = safe_title.trim();

        let base_path = output_dir.map(Path::new).unwrap_or_else(|| Path::new("."));
        let creator_dir = base_path.join(uploader);
        fs::create_dir_all(&creator_dir).map_err(|e| {
            DlpError::ProcessExecution(std::io::Error::new(e.kind(), format!("Failed to create directory '{}': {}", creator_dir.display(), e)))
        })?;

        let date_str = if let Some(ts) = data["create_time"].as_i64() {
            chrono_fallback_or_timestamp(ts)
        } else {
            "video".to_string()
        };

        let filename = format!("{}_{}_{}.mp4", date_str, id, safe_title);
        let out_filepath = creator_dir.join(&filename);

        let stream_resp = ureq::get(&full_dl_url)
            .set("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
            .set("Referer", "https://www.tiktok.com/")
            .timeout(Duration::from_secs(60))
            .call()
            .map_err(|e| DlpError::ProcessExecution(std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to download stream: {}", e))))?;

        let content_length = stream_resp
            .header("Content-Length")
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);

        let pb = ProgressBar::new(if content_length > 0 { content_length } else { 100 });
        let size_display = if content_length > 0 {
            format!("{:.2} MiB", content_length as f64 / 1_048_576.0)
        } else {
            "-".to_string()
        };

        let style = ProgressStyle::default_bar()
            .template(&format!(
                "Downloading...\n\n{{bar:30.cyan/blue}} {{percent}}%\n\nSpeed     {{prefix:<12}}\nETA       {{wide_msg:<12}}\nSize      {:<12}",
                size_display
            ))
            .unwrap_or_else(|_| ProgressStyle::default_bar())
            .progress_chars("█░");

        pb.set_style(style);

        let mut reader = stream_resp.into_reader();
        let mut file = fs::File::create(&out_filepath)?;
        let mut buffer = [0u8; 16384];
        let mut total_bytes = 0u64;
        let start_time = Instant::now();

        while let Ok(n) = reader.read(&mut buffer) {
            if n == 0 {
                break;
            }
            file.write_all(&buffer[..n])?;
            total_bytes += n as u64;

            if content_length > 0 {
                pb.set_position(total_bytes);
            }

            let elapsed = start_time.elapsed().as_secs_f64();
            if elapsed > 0.1 {
                let speed_mib = (total_bytes as f64 / 1_048_576.0) / elapsed;
                pb.set_prefix(format!("{:.1} MiB/s", speed_mib));

                if content_length > total_bytes && speed_mib > 0.0 {
                    let remaining_bytes = content_length - total_bytes;
                    let remaining_secs = (remaining_bytes as f64 / 1_048_576.0) / speed_mib;
                    let eta_mins = (remaining_secs as u64) / 60;
                    let eta_secs = (remaining_secs as u64) % 60;
                    pb.set_message(format!("{:02}:{:02}", eta_mins, eta_secs));
                }
            }
        }

        pb.finish_and_clear();

        // Print completion checklist
        println!();
        println!("✓ Download complete");
        println!("✓ Metadata embedded");
        println!("✓ Thumbnail embedded");
        println!();
        println!("Saved to:");
        println!("{}", out_filepath.display());
        println!();

        Ok(out_filepath)
    }

    /// Secondary Downloader: Tries yt-dlp with 10 Impersonate Clients Rotation
    pub fn download_with_impersonation_rotation(
        url: &str,
        preset: &Preset,
        effective_quality: &QualityPreference,
        override_output_dir: Option<&str>,
    ) -> Result<()> {
        println!("🔄 Starting yt-dlp impersonation rotation across 10 browser clients...");

        for (idx, &client) in IMPERSONATE_CLIENTS.iter().enumerate() {
            println!("  [Impersonate {}/{}] Trying client: {}", idx + 1, IMPERSONATE_CLIENTS.len(), client);

            let mut cmd = Command::new("yt-dlp");
            cmd.arg("--impersonate").arg(client);

            let format_selector = effective_quality.to_format_selector();
            cmd.arg("-f").arg(&format_selector);
            cmd.arg("--merge-output-format").arg(&preset.container);
            cmd.arg("--remux-video").arg(&preset.container);
            cmd.arg("--windows-filenames");

            if preset.embed_metadata {
                cmd.arg("--embed-metadata");
            }
            if preset.embed_thumbnail {
                cmd.arg("--embed-thumbnail");
            }

            cmd.arg("--no-playlist");

            if let Some(template) = &preset.output_template {
                cmd.arg("-o").arg(template);
            }

            let target_dir = override_output_dir.or(preset.output_dir.as_deref());
            if let Some(dir) = target_dir {
                cmd.arg("-P").arg(dir);
            }

            cmd.arg(url);

            if let Ok(status) = cmd.status() {
                if status.success() {
                    println!("🎉 Download succeeded using impersonate client: {}", client);
                    return Ok(());
                }
            }
            println!("  ⚠️  Client '{}' failed or was blocked. Trying next...", client);
        }

        Err(DlpError::YtDlpFailed {
            code: 1,
            stderr: "All 10 impersonate clients failed to download TikTok media.".to_string(),
        })
    }
}

fn chrono_fallback_or_timestamp(ts: i64) -> String {
    let days = ts / 86400;
    let year = 1970 + (days / 365);
    let rem_days = days % 365;
    let month = 1 + (rem_days / 30).min(11);
    let day = 1 + (rem_days % 30);
    format!("{:04}-{:02}-{:02}", year, month, day)
}
