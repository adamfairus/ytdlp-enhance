use std::path::Path;
use std::process::Command;
use crate::config::Config;
use crate::error::Result;
use crate::preset::PresetManager;

pub struct Doctor;

impl Doctor {
    pub fn run_diagnostics(preset_manager: &PresetManager, config: &Config) -> Result<()> {
        println!();
        println!("╭──────────────────────────────────────────────────╮");
        println!("│           🩺 dlp — System Health Doctor          │");
        println!("│        Diagnostics & Environment Verification    │");
        println!("╰──────────────────────────────────────────────────╯");
        println!();

        let mut all_healthy = true;

        // 1. Check yt-dlp binary and version
        print!("• Checking 'yt-dlp' binary... ");
        match Command::new("yt-dlp").arg("--version").output() {
            Ok(out) if out.status.success() => {
                let ver = String::from_utf8_lossy(&out.stdout).trim().to_string();
                println!("✅ OK (version: {})", ver);
            }
            _ => {
                println!("❌ NOT FOUND (Install yt-dlp on your PATH)");
                all_healthy = false;
            }
        }

        // 2. Check ffmpeg binary and version
        print!("• Checking 'ffmpeg' binary... ");
        match Command::new("ffmpeg").arg("-version").output() {
            Ok(out) if out.status.success() => {
                let first_line = String::from_utf8_lossy(&out.stdout)
                    .lines()
                    .next()
                    .unwrap_or("Installed")
                    .to_string();
                println!("✅ OK ({})", first_line);
            }
            _ => {
                println!("❌ NOT FOUND (Install ffmpeg on your PATH)");
                all_healthy = false;
            }
        }

        // 3. Check ffprobe binary
        print!("• Checking 'ffprobe' binary... ");
        match Command::new("ffprobe").arg("-version").output() {
            Ok(out) if out.status.success() => {
                println!("✅ OK");
            }
            _ => {
                println!("⚠️  WARNING (ffprobe not found, optional but recommended)");
            }
        }

        // 4. Check Config & Presets
        let active_presets = preset_manager.list();
        println!("• Active Presets Loaded: ✅ {} preset(s)", active_presets.len());

        if let Some(config_dir) = dirs::config_dir() {
            let user_preset_dir = config_dir.join("dlp").join("presets");
            let exists = user_preset_dir.exists();
            println!(
                "• User Custom Presets Directory ({}): {}",
                user_preset_dir.display(),
                if exists { "✅ Found" } else { "ℹ️  None (Using built-in defaults)" }
            );
        }

        if let Some(dl_dir) = &config.download_dir {
            let p = Path::new(dl_dir);
            println!(
                "• Configured Download Directory ({}): {}",
                dl_dir,
                if p.exists() { "✅ Exists" } else { "⚠️  Directory does not exist yet" }
            );
        }

        // 5. Test LRCLIB API connectivity
        print!("• Testing LRCLIB API connectivity... ");
        match ureq::get("https://lrclib.net/api/get?track_name=test&artist_name=test")
            .timeout(std::time::Duration::from_secs(4))
            .call()
        {
            Ok(_) | Err(ureq::Error::Status(404, _)) => {
                println!("✅ Online & Reachable");
            }
            Err(e) => {
                println!("⚠️  Unreachable ({})", e);
            }
        }

        // 6. Test TikWM API connectivity
        print!("• Testing TikWM API connectivity... ");
        match ureq::get("https://www.tikwm.com/api/?url=https://www.tiktok.com/@test/video/123")
            .timeout(std::time::Duration::from_secs(4))
            .call()
        {
            Ok(_) => {
                println!("✅ Online & Reachable");
            }
            Err(e) => {
                println!("⚠️  Unreachable ({})", e);
            }
        }

        println!();
        if all_healthy {
            println!("🎉 All critical core dependencies and services are healthy and ready!\n");
        } else {
            println!("⚠️  Some dependencies are missing. Please review the errors above.\n");
        }

        Ok(())
    }
}
