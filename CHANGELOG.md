# Changelog

Semua perubahan penting pada proyek **`dlp`** akan didokumentasikan di sini.

Format mengikuti [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [1.0.0] - 2026-09-01 (Stable Production Release 🎉)

### Added
- **System Health Diagnostics (`dlp doctor`)**:
  - Pengecekan otomatis ketersediaan & versi `yt-dlp`, `ffmpeg`, `ffprobe`.
  - Verifikasi direktori preset dan konfigurasi lokal.
  - Pengujian *health check* konektivitas API LRCLIB dan TikWM.
  - Integrasi opsi `🩺 System Diagnostics (Doctor)` di dalam Interactive Wizard UI.
- **Shell Auto-Completions Generator (`dlp completions`)**:
  - Subcommand `dlp completions <SHELL>` untuk menghasilkan skrip *autocompletion* native untuk **Bash**, **Zsh**, **Fish**, **PowerShell**, dan **Elvish** menggunakan `clap_complete`.
- **TDD Test Suite**:
  - Penambahan pengujian unit diagnostik di `tests/doctor_test.rs` (Total: 21/21 tests pass).

## [0.7.0] - 2026-09-01

### Added
- **Smart Classification & Policy Engine**:
  - Modul `SmartClassifier` (`src/classifier.rs`) untuk klasifikasi media otomatis berdasarkan pola URL, domain, orientasi rasio aspek, dan kategori metadata.
  - Mode universal `dlp <URL>` yang otomatis memilih preset terbaik (`music`, `tiktok`, `video`) tanpa mengharuskan pengetikan subcommand.
  - Dukungan auto-klasifikasi per-item pada *Batch Download* (`dlp batch urls.txt`).

## [0.6.0] - 2026-09-01

### Added
- **TikTok / Shorts / Reels Specialization**:
  - **TikWM Native Engine (Primary)**: Pengunduhan tanpa watermark berkecepatan tinggi dengan auto rate-limit retry.
  - **10-Client Impersonation Rotation (Secondary Fallback)**: Proteksi anti-bot rotasi TLS fingerprint browser (`safari-18.0`, `chrome-136`, `edge-101`, `firefox-135`, dll.).
  - Pola penamaan & pengelompokan folder creator otomatis: `%(uploader,creator)s/%(upload_date>%Y-%m-%d)s_%(id)s_%(title).60s.%(ext)s`.
  - Penegakan batas resolusi vertikal optimal (`max_vertical = 1440`).

## [0.5.1] - 2026-09-01

### Added
- **Native LRCLIB Synced Lyrics Fetcher (Rust)**:
  - Pencarian lirik tersinkronisasi murni via API LRCLIB tanpa ketergantungan script Python eksternal.
  - *Track-by-track directory synchronizer* untuk album/playlist lengkap.

## [0.5.0] - 2026-09-01

### Added
- **Music Specialization Engine**:
  - **1:1 Square Cropped Cover Art**: Integrasi filter FFmpeg (`ThumbnailsConvertor:-vf crop=ih:ih -c:v mjpeg`).
  - **Extended Music Metadata Tags**: Otomatis memetakan tag Artis, Album, dan Judul ke metadata internal file audio.

## [0.4.0] - 2026-09-01

### Added
- **Batch Download Engine**:
  - Subcommand `dlp batch <FILE_OR_URLS>`.
  - Batch Summary Report dengan statistik total, berhasil, dan gagal.

## [0.3.0] - 2026-09-01

### Added
- **Interactive Terminal Wizard UI**:
  - Auto-launch wizard interaktif dengan sistem navigasi *Back* lengkap.

## [0.2.0] - 2026-09-01

### Added
- **Preset System & TOML Loading**:
  - Embedded default presets: `video.toml`, `music.toml`, `tiktok.toml`.

## [0.1.0] - 2026-09-01

### Added
- **Core CLI Parser & Orchestrator**:
  - Ekstraksi JSON metadata, deteksi orientasi, seleksi kualitas dinamis, dependency checker.
