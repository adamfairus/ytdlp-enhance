use indicatif::{ProgressBar, ProgressStyle};
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

pub struct ProgressTracker {
    pb: ProgressBar,
    current_speed: String,
    current_eta: String,
    current_size: String,
    last_file_path: Option<String>,
}

impl ProgressTracker {
    pub fn new() -> Self {
        let pb = ProgressBar::new(100);
        let style = ProgressStyle::default_bar()
            .template("Downloading...\n\n{bar:30.cyan/blue} {percent}%\n\n{msg}")
            .unwrap_or_else(|_| ProgressStyle::default_bar())
            .progress_chars("█░");

        pb.set_style(style);
        pb.set_message("Speed     -\nETA       -\nSize      -");

        Self {
            pb,
            current_speed: "-".to_string(),
            current_eta: "-".to_string(),
            current_size: "-".to_string(),
            last_file_path: None,
        }
    }

    fn update_stats(&self) {
        self.pb.set_message(format!(
            "Speed     {:<14}\nETA       {:<14}\nSize      {:<14}",
            self.current_speed, self.current_eta, self.current_size
        ));
    }

    pub fn process_stream<R: Read>(&mut self, reader: R) {
        let buf = BufReader::new(reader);
        for line_res in buf.lines() {
            if let Ok(line) = line_res {
                self.parse_line(&line);
            }
        }
    }

    pub fn parse_line(&mut self, line: &str) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return;
        }

        // 1. Detect FFmpeg Merging / Postprocessing
        if trimmed.starts_with("[Merger]") || trimmed.starts_with("[VideoRemuxer]") || trimmed.starts_with("[Metadata]") || trimmed.starts_with("[ThumbnailsConvertor]") || trimmed.starts_with("[EmbedThumbnail]") {
            self.current_speed = "Processing (FFmpeg)".to_string();
            self.current_eta = "Finishing".to_string();
            self.update_stats();
            if let Some(merged) = trimmed.strip_prefix("[Merger] Merging formats into") {
                let p = merged.trim().trim_matches('"').to_string();
                self.last_file_path = Some(p);
            }
            return;
        }

        // 2. Detect Destination File Path
        if let Some(dest) = trimmed.strip_prefix("[download] Destination:") {
            let p = dest.trim().to_string();
            if !p.ends_with(".srt") && !p.ends_with(".vtt") && !p.ends_with(".jpg") && !p.ends_with(".webp") && !p.ends_with(".image") {
                self.last_file_path = Some(p);
            }
        } else if trimmed.contains("has already been downloaded") {
            if let Some(dest) = trimmed.strip_prefix("[download]") {
                let p = dest.replace("has already been downloaded", "").trim().to_string();
                if !p.is_empty() {
                    self.last_file_path = Some(p);
                }
            }
            self.pb.set_position(100);
            self.current_speed = "Cached".to_string();
            self.current_eta = "Done".to_string();
            self.update_stats();
            return;
        }

        // 3. Ignore Subtitle & Thumbnail progress (only track actual audio/video stream)
        if trimmed.contains(".srt") || trimmed.contains(".vtt") || trimmed.contains(".webp") || trimmed.contains(".jpg") || trimmed.contains(".image") {
            return;
        }

        // 4. Parse Aria2 Download Output: [#59bb07 19MiB/84MiB(23%) CN:16 DL:5.0MiB ETA:12s]
        if trimmed.starts_with("[#") && trimmed.contains('%') {
            // Extract percentage: (23%)
            if let Some(open_paren) = trimmed.find('(') {
                if let Some(close_paren) = trimmed[open_paren..].find("%)") {
                    let pct_str = &trimmed[open_paren + 1..open_paren + close_paren];
                    if let Ok(pct) = pct_str.parse::<u64>() {
                        self.pb.set_position(pct.min(100));
                    }
                }
            }

            // Extract Speed: DL:5.0MiB or DL:500KiB
            if let Some(dl_pos) = trimmed.find("DL:") {
                let after_dl = &trimmed[dl_pos + 3..];
                let speed_str = after_dl.split_whitespace().next().unwrap_or("").trim_end_matches(']');
                if !speed_str.is_empty() {
                    self.current_speed = format!("{}/s", speed_str);
                }
            }

            // Extract ETA: ETA:12s or ETA:1m20s
            if let Some(eta_pos) = trimmed.find("ETA:") {
                let after_eta = &trimmed[eta_pos + 4..];
                let eta_str = after_eta.split_whitespace().next().unwrap_or("").trim_end_matches(']');
                if !eta_str.is_empty() {
                    self.current_eta = eta_str.to_string();
                }
            }

            // Extract Size: 19MiB/84MiB
            if let Some(first_space) = trimmed.find(' ') {
                if let Some(open_paren) = trimmed[first_space..].find('(') {
                    let size_part = trimmed[first_space + 1..first_space + open_paren].trim();
                    if let Some((_, total)) = size_part.split_once('/') {
                        self.current_size = total.to_string();
                    } else {
                        self.current_size = size_part.to_string();
                    }
                }
            }

            self.update_stats();
            return;
        }

        // 5. Parse Custom DLP_PROGRESS line: DLP_PROGRESS: 87.5%|12.4MiB/s|00:08|1.42GiB
        if let Some(data) = trimmed.strip_prefix("__DLP__:") {
            let parts: Vec<&str> = data.split('|').collect();
            if parts.len() >= 4 {
                let percent_str = parts[0].trim().trim_end_matches('%');
                if let Ok(pct) = percent_str.parse::<f64>() {
                    self.pb.set_position(pct.round().min(100.0) as u64);
                }

                let speed = parts[1].trim();
                let eta = parts[2].trim();
                let total = parts[3].trim();

                if !speed.is_empty() && speed != "NA" {
                    self.current_speed = speed.to_string();
                }
                if !eta.is_empty() && eta != "NA" {
                    self.current_eta = eta.to_string();
                }
                if !total.is_empty() && total != "NA" {
                    self.current_size = total.to_string();
                }

                self.update_stats();
            }
            return;
        }

        // 6. Standard [download] line fallback: [download]  87.0% of  1.42GiB at 12.40MiB/s ETA 00:08
        if trimmed.starts_with("[download]") && trimmed.contains('%') {
            if let Some(pct_pos) = trimmed.find('%') {
                let prefix_part = &trimmed[..pct_pos];
                if let Some(last_space) = prefix_part.rfind(' ') {
                    if let Ok(pct) = prefix_part[last_space + 1..].parse::<f64>() {
                        self.pb.set_position(pct.round().min(100.0) as u64);
                    }
                }
            }

            if let Some(at_pos) = trimmed.find(" at ") {
                let after_at = &trimmed[at_pos + 4..];
                let speed = after_at.split_whitespace().next().unwrap_or("-");
                if speed != "-" {
                    self.current_speed = speed.to_string();
                }
            }

            if let Some(eta_pos) = trimmed.find("ETA ") {
                let after_eta = &trimmed[eta_pos + 4..];
                let eta = after_eta.split_whitespace().next().unwrap_or("-");
                if eta != "-" {
                    self.current_eta = eta.to_string();
                }
            }

            if let Some(of_pos) = trimmed.find(" of ") {
                let after_of = &trimmed[of_pos + 4..];
                let size = after_of.split_whitespace().next().unwrap_or("-");
                if size != "-" {
                    self.current_size = size.to_string();
                }
            }

            self.update_stats();
        }
    }

    pub fn finish_and_print_checklist(
        &self,
        embed_metadata: bool,
        embed_thumbnail: bool,
        lyrics_saved: bool,
        saved_path: Option<&Path>,
    ) {
        self.pb.finish_and_clear();
        println!();
        println!("✓ Download complete");
        if embed_metadata {
            println!("✓ Metadata embedded");
        }
        if embed_thumbnail {
            println!("✓ Thumbnail embedded");
        }
        if lyrics_saved {
            println!("✓ Standalone subtitle/lyrics saved");
        }

        println!();
        println!("Saved to:");
        if let Some(p) = saved_path {
            println!("{}", p.display());
        } else if let Some(p) = &self.last_file_path {
            println!("{}", p);
        } else {
            println!("./");
        }
        println!();
    }
}
