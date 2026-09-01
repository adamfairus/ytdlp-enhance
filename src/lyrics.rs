use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
pub struct LrcResponse {
    pub id: Option<u64>,
    #[serde(rename = "trackName")]
    pub track_name: Option<String>,
    #[serde(rename = "artistName")]
    pub artist_name: Option<String>,
    #[serde(rename = "plainLyrics")]
    pub plain_lyrics: Option<String>,
    #[serde(rename = "syncedLyrics")]
    pub synced_lyrics: Option<String>,
    pub instrumental: Option<bool>,
}

pub struct LyricsFetcher;

impl LyricsFetcher {
    /// Clean title from common noisy MV tags and track numbers for high match rate
    pub fn clean_title(title: &str) -> String {
        let mut cleaned = title.trim().to_string();

        // 1. Strip leading track number prefix e.g. "01 - ", "01. ", "1 - "
        if let Some(pos) = cleaned.find(" - ") {
            let prefix = &cleaned[..pos];
            if !prefix.is_empty() && prefix.chars().all(|c| c.is_ascii_digit()) {
                cleaned = cleaned[pos + 3..].trim().to_string();
            }
        } else if let Some(pos) = cleaned.find(". ") {
            let prefix = &cleaned[..pos];
            if !prefix.is_empty() && prefix.chars().all(|c| c.is_ascii_digit()) {
                cleaned = cleaned[pos + 2..].trim().to_string();
            }
        }

        let unwanted = [
            "[Official Music Video]",
            "(Official Music Video)",
            "[Official Video]",
            "(Official Video)",
            "[Official MV]",
            "(Official MV)",
            "[Official Audio]",
            "(Official Audio)",
            "[Performance Video]",
            "(Performance Video)",
            "[Visualizer]",
            "(Visualizer)",
            "[Lyric Video]",
            "(Lyric Video)",
            "[Lyrics Video]",
            "(Lyrics Video)",
            "[MV]",
            "(MV)",
            "[M/V]",
            "(M/V)",
            "[Audio]",
            "(Audio)",
            "[Color Coded Lyrics]",
            "(Color Coded Lyrics)",
            "[Remastered]",
            "(Remastered)",
            "[4K Remaster]",
            "(4K Remaster)",
        ];

        for u in unwanted {
            let lower_u = u.to_lowercase();
            while let Some(pos) = cleaned.to_lowercase().find(&lower_u) {
                cleaned.replace_range(pos..pos + u.len(), "");
            }
        }

        cleaned = cleaned.trim().trim_end_matches(['-', '_', '|', ':']).trim().to_string();
        while cleaned.contains("  ") {
            cleaned = cleaned.replace("  ", " ");
        }

        cleaned
    }

    /// Clean artist from ' - Topic' suffix and take main artist before comma
    pub fn clean_artist(artist: &str) -> String {
        let mut a = artist.trim().trim_end_matches(" - Topic").trim();
        if let Some((first, _)) = a.split_once(',') {
            a = first.trim();
        }
        a.to_string()
    }

    /// Fetch synced lyrics from LRCLIB API with exact get and search fallbacks
    pub fn fetch_lyrics(title: &str, artist: Option<&str>, duration: Option<f64>) -> Option<String> {
        let clean_t = Self::clean_title(title);
        let clean_a = artist.map(Self::clean_artist);

        // 1. Try exact get endpoint
        let mut url = format!(
            "https://lrclib.net/api/get?track_name={}",
            urlencoding::encode(&clean_t)
        );

        if let Some(art) = &clean_a {
            if !art.is_empty() {
                url.push_str(&format!("&artist_name={}", urlencoding::encode(art)));
            }
        }

        if let Some(d) = duration {
            if d > 0.0 {
                url.push_str(&format!("&duration={}", d.round() as u64));
            }
        }

        if let Ok(resp) = ureq::get(&url)
            .set("User-Agent", "dlp-cli-rust (https://github.com/adamf/dlp)")
            .timeout(std::time::Duration::from_secs(6))
            .call()
        {
            if resp.status() == 200 {
                if let Ok(lrc) = resp.into_json::<LrcResponse>() {
                    if let Some(synced) = lrc.synced_lyrics {
                        if !synced.trim().is_empty() {
                            return Some(synced);
                        }
                    }
                    if let Some(plain) = lrc.plain_lyrics {
                        if !plain.trim().is_empty() {
                            return Some(plain);
                        }
                    }
                }
            }
        }

        // 2. Fallback to search query
        let search_query = match &clean_a {
            Some(art) if !art.is_empty() => format!("{} {}", art, clean_t),
            _ => clean_t,
        };

        let search_url = format!(
            "https://lrclib.net/api/search?q={}",
            urlencoding::encode(&search_query)
        );

        if let Ok(resp) = ureq::get(&search_url)
            .set("User-Agent", "dlp-cli-rust (https://github.com/adamf/dlp)")
            .timeout(std::time::Duration::from_secs(6))
            .call()
        {
            if resp.status() == 200 {
                if let Ok(results) = resp.into_json::<Vec<LrcResponse>>() {
                    for item in results {
                        if let Some(synced) = item.synced_lyrics {
                            if !synced.trim().is_empty() {
                                return Some(synced);
                            }
                        }
                        if let Some(plain) = item.plain_lyrics {
                            if !plain.trim().is_empty() {
                                return Some(plain);
                            }
                        }
                    }
                }
            }
        }

        None
    }

    /// Automatically scans directory for downloaded audio tracks and fetches synced .lrc for each song
    pub fn sync_lyrics_for_directory(base_dir: &Path, default_artist: Option<&str>) {
        let mut audio_files = Vec::new();
        Self::collect_audio_files(base_dir, &mut audio_files);

        if audio_files.is_empty() {
            return;
        }

        for audio_path in audio_files {
            let lrc_path = audio_path.with_extension("lrc");
            if lrc_path.exists() {
                continue; // Already has lyrics
            }

            let file_stem = match audio_path.file_stem().and_then(|s| s.to_str()) {
                Some(s) => s,
                None => continue,
            };

            // Derive artist: check grandparent or parent folder (e.g. Artist/Album/Title.opus)
            let detected_artist = audio_path
                .parent()
                .and_then(|p| p.parent())
                .and_then(|p| p.file_name())
                .and_then(|s| s.to_str())
                .or(default_artist);

            println!("🔍 Fetching lyrics for track: '{}'...", file_stem);
            if let Some(lrc_content) = Self::fetch_lyrics(file_stem, detected_artist, None) {
                if fs::write(&lrc_path, lrc_content).is_ok() {
                    println!("📝 Synced lyrics saved: {}", lrc_path.display());
                }
            } else {
                println!("ℹ️  No lyrics found for '{}'", file_stem);
            }
        }

        // Clean up accidental playlist-level lrc files (e.g. "Album - *.lrc")
        if let Ok(entries) = fs::read_dir(base_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() && p.extension().and_then(|s| s.to_str()) == Some("lrc") {
                    if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
                        if name.starts_with("Album - ") {
                            let _ = fs::remove_file(p);
                        }
                    }
                }
            }
        }
    }

    fn collect_audio_files(dir: &Path, files: &mut Vec<PathBuf>) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    Self::collect_audio_files(&path, files);
                } else if path.is_file() {
                    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                        if matches!(ext, "opus" | "mp3" | "m4a" | "flac" | "ogg") {
                            files.push(path);
                        }
                    }
                }
            }
        }
    }
}
