# 🦀 dlp — Intelligent CLI Wrapper & Orchestration Layer

`dlp` adalah *orchestration layer* cerdas berbasis Rust di atas **`yt-dlp`** dan **`ffmpeg`**. Aplikasi ini mengotomatiskan inspeksi metadata, deteksi orientasi video (Horizontal vs Vertical vs Square), pemilihan kualitas format dinamis, **Smart Classification & Policy Engine Otomatis**, sistem **Presets TOML**, **Music Specialization (1:1 Cover Art & Lirik LRCLIB)**, **TikTok / Shorts Specialization (TikWM Engine + 10-Client Fallback)**, **Batch Download**, **System Diagnostics (`doctor`)**, **Shell Auto-Completions**, serta **Interactive Terminal Wizard UI**.

---

## 🚀 Instalasi Cepat (One-Line Installer)

Pasang `dlp` secara instan menggunakan script instalasi otomatis:
```bash
curl -fsSL https://raw.githubusercontent.com/adamfairus/ytdlp-enhance/main/install.sh | bash
```

---

## ✨ Fitur Utama (v2.1.0 Architecture Hardening Release)

- **🎯 Confidence-Based Smart Classification (v2.1)**:
  - Evaluasi multi-sinyal berbasis probabilitas/confidence (URL domain, orientasi video, ketersediaan audio stream, kategori, dan durasi).
  - Transparansi skor confidence (e.g. 80%, 95%) dan daftar sinyal keputusan pada output `dlp --explain`.
- **🧩 Decoupled Engine Layer & Clean Facade (v2.1)**:
  - Ekstraksi `YtDlpEngine` (`src/engine.rs`) untuk memutus *god object* dan ketergantungan sirkular antara Downloader dan Provider.
  - `Downloader` dirampingkan menjadi fasad bersih (57 baris) yang mendelegasikan perintah ke `ProviderRegistry` dan `YtDlpEngine`.
- **📡 Decoupled Event System (v2.1)**:
  - Arsitektur event-driven melalui `DownloadEvent` enum dan `EventDispatcher` thread-safe (`EventListener`).
- **🛡️ Structured Recovery Policy (v2.1)**:
  - Pemisahan deteksi kegagalan (*Failure Detection*) dari keputusan pemulihan (*Recovery Decision*) melalui `FailureContext`, `RecoveryPolicy`, dan `RecoveryAction`.
- **⚡ True Queue TaskScheduler (v2.1)**:
  - Penjadwalan antrean dengan *Priority Queuing* (`Urgent`, `High`, `Normal`, `Low`), *Task State Machine* (`Pending` $\rightarrow$ `Running` $\rightarrow$ `Retrying` $\rightarrow$ `Completed` / `Failed`), dan penegakan batas konkurensi per-platform.
- **📦 Cross-Platform Distribution & Single Binary (v2.0)**:
  - Distribusi binary mandiri ultra-ringkas (3.4 MB stripped / 1.7 MB compressed) dengan kompilasi LTO fat dan optimasi level 3.
  - Script instalasi otomatis satu-baris `install.sh` yang mendeteksi OS/arsitektur, mengonfigurasi `$PATH`, dan memverifikasi dependensi.
  - Skema migrasi konfigurasi otomatis `dlp config migrate` antar versi mayor.
- **🧪 Comprehensive Verification Matrix & Regression Protection**:
  - 18 test suites dengan **75 unit, integration, snapshot, dan regression tests** (100% Green).
  - Golden regression tests untuk URL edge cases (YouTube Shorts, Live, Music, embed, TikTok desktop & shortlink, URL queries).
  - Terminal snapshot testing untuk memastikan stabilitas visual Decision Trace, Diagnostic Report, dan Batch Scheduler.
  - Property-based testing untuk penegakan batas resolusi, orientasi video, dan kebijakan preset.
- **🛠️ Developer Experience (DX) & Configuration Management (v1.6)**:
  - Subcommand `dlp config [show | path | init | set <KEY> <VALUE>]` untuk inspeksi dan modifikasi konfigurasi global `~/.config/dlp/config.toml` tanpa perlu membuka teks editor manual.
  - Subcommand `dlp debug <URL> [--raw]` untuk membedah data mentah metadata JSON dari extractor (sangat cocok untuk debugging atau di-*pipe* ke `jq`).
  - Diagnostik `dlp doctor` yang diperluas: deteksi akselerator multi-koneksi `aria2c`, verifikasi Provider Registry, dan uji izin tulis direktori unduhan.
- **🧩 Plugin / Provider Architecture (v1.5)**: Dekopling logika platform berbasis Rust Trait (`pub trait Provider` & `ProviderRegistry`) untuk routing otomatis platform (TikTok, YouTube, Generic) yang modular dan mudah diekstensikan.
- **🎵 Music Pipeline 2.0 (v1.4)**: Pengorganisasian folder album otomatis (`Artist/Album/01 - Track Title.opus`), pemetaan tag metadata audio lengkap (`Artist`, `Album Artist`, `Track`, `Track Number`, `Disc Number`, `Album`, `Genre`, `Release Date`), serta pencarian lirik LRCLIB pintar yang kebal dari prefix nomor trek dan embel-embel remaster.
- **🔍 Decision Trace & UX Transparency (v1.3)**: Flag diagnostik `--explain` untuk membedah transparansi rantai keputusan cerdas `dlp` (platform, orientasi, policy engine, format selector, dan tahapan pipeline postprocessing) tanpa mengunduh.
- **🧹 Multi-Tier Metadata Normalization (v1.3)**: Sanitasi otomatis judul dari sampah tag MV/Official Video/Performance Video/Remastered, pembersihan uploader/artist (`- Topic`), dan penamaan berkas bersih bebas karakter ilegal.
- **⚡ Controlled Parallel Download Queue (v1.2)**: Mengunduh puluhan media secara paralel dengan batas konkurensi terkelola (`dlp batch urls.txt -c 3` atau via `config.toml` `concurrency = 3`).
- **📋 Smart Queue Scheduler (v1.2)**: Pra-analisis cerdas antrean URL sebelum dieksekusi, mengelompokkan tugas per platform (YouTube, TikTok, Music) dan menegakkan perlindungan *rate-limit* otomatis untuk mencegah pemblokiran IP.
- **🛡️ Smart Self-Healing Error Recovery (v1.1)**: Deteksi cerdas kegagalan download (`Transient`, `FormatUnavailable`, `BotBlockOrExtractor`, `FFmpegProcessing`, `Permanent`):
  - Auto-retry dengan *exponential backoff* pada gangguan jaringan sementara.
  - *Format Stepping Fallback*: Otomatis menurunkan resolusi (4K → 1440p → 1080p → 720p → Best) jika format requested tidak tersedia.
  - Rotasi TLS browser impersonation otomatis (`safari-18`, `chrome-136`, `firefox-135`) saat terdeteksi proteksi anti-bot.
  - Diagnosis error informatif dan terstruktur di terminal.
- **💾 Batch Checkpoint & Resume (v1.1)**: State persistence persisten (`.dlp_checkpoint.json`) dengan flag `--resume` untuk melewati URL yang telah sukses dan hanya memproses item tertunda/gagal.
- **🩺 System Health Doctor (v1.0)**: Subcommand `dlp doctor` untuk memverifikasi ketersediaan `yt-dlp`, `ffmpeg`, `ffprobe`, konfigurasi, dan konektivitas API LRCLIB & TikWM.
- **🐚 Shell Auto-Completions (v1.0)**: Subcommand `dlp completions <SHELL>` untuk menghasilkan skrip *autocompletion* native untuk **Bash**, **Zsh**, **Fish**, **PowerShell**, dan **Elvish**.
- **🤖 Smart Media Classification (v0.7)**: Otomatis mendeteksi tipe konten (*Music*, *Shorts/TikTok Vertical*, atau *Standard Video*) dari URL dan orientasi metadata tanpa perlu memasukkan flag manual. Cukup jalankan `dlp <URL>`.
- **📱 TikTok / Shorts Specialization (v0.6)**: Optimasi khusus untuk video vertikal (TikTok, YouTube Shorts, Instagram Reels):
  - **TikWM Native Engine (Primary)**: Pengunduhan berkecepatan tinggi murni tanpa watermark.
  - **10-Client Impersonation Rotation (Secondary Fallback)**: Proteksi anti-bot rotasi TLS fingerprint browser.
  - Pengelompokan folder otomatis berdasarkan Creator/Uploader (`Creator/YYYY-MM-DD_ID_Title.mp4`).
  - Penegakan resolusi vertikal optimal (`max_vertical = 1440p`).
- **🎵 Music Specialization (v0.5)**: Ekstraksi audio Opus native kualitas studio lengkap dengan 1:1 Square Cropped Cover Art dan lirik sinkron LRCLIB (`.lrc`).
- **📦 Smart Batch Download (v0.4)**: Mengunduh puluhan/ratusan media sekaligus dari berkas teks (`urls.txt`) atau daftar link langsung.
- **🧙 Interactive Terminal Wizard (v0.3)**: Cukup ketik `dlp` untuk membuka wizard interaktif berbasis menu.
- **📐 Orientation Detection**: Deteksi otomatis `Horizontal`, `Vertical`, atau `Square`.

---

## 🚀 Panduan Penggunaan

### 1. Smart Universal Download (Auto-Classification)
Cukup berikan URL apa saja, `dlp` akan otomatis menentukan format dan perlakuan terbaik:
```bash
# Otomatis terdeteksi sebagai Musik (Opus + 1:1 cover + lirik .lrc):
dlp "https://music.youtube.com/watch?v=A9EpZWrQ3dM"

# Otomatis terdeteksi sebagai TikTok (MP4 Vertikal no-watermark ke folder creator):
dlp "https://www.tiktok.com/@ryoun_e/video/7680213075999427860"

# Otomatis terdeteksi sebagai Video Standar (MP4 1080p):
dlp "https://www.youtube.com/watch?v=dQw4w9WgXcQ"
```

### 2. Mode Interaktif (Interactive Wizard)
```bash
dlp
```

### 3. Decision Trace Diagnostik (`--explain`)
Melihat rantai evaluasi keputusan cerdas (kebijakan format, orientasi, preset, dan tahapan post-processing) tanpa mengeksekusi pengunduhan:
```bash
dlp "https://www.youtube.com/watch?v=dQw4w9WgXcQ" --explain
```

### 4. Smart Batch Download (Parallel Queue & Resume Checkpoint)
```bash
# Otomatis mengklasifikasikan dan mengunduh tiap URL di urls.txt:
dlp batch urls.txt

# Unduh paralel dengan 3 worker threads (controlled concurrency):
dlp batch urls.txt -c 3

# Melanjutkan batch yang terputus dengan 3 worker threads (skip yang sukses, retry yang tertunda):
dlp batch urls.txt --resume -c 3
```

### 5. Diagnostik Sistem (Doctor)
```bash
dlp doctor
```

### 6. Menghasilkan Shell Auto-Completions
```bash
# Untuk Zsh:
dlp completions zsh > ~/.zfunc/_dlp

# Untuk Bash:
dlp completions bash > ~/.bash_completion.d/dlp

# Untuk Fish:
dlp completions fish > ~/.config/fish/completions/dlp.fish
```

---

## 🧪 Testing & TDD

Jalankan seluruh test suite dengan:
```bash
cargo test
```
Semua 75 pengujian unit, integrasi, snapshot, dan regression terverifikasi 100% *green*.
