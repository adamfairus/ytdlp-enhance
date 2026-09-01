# 🦀 dlp — Intelligent CLI Wrapper & Orchestration Layer

`dlp` adalah *orchestration layer* cerdas berbasis Rust di atas **`yt-dlp`** dan **`ffmpeg`**. Aplikasi ini mengotomatiskan inspeksi metadata, deteksi orientasi video (Horizontal vs Vertical vs Square), pemilihan kualitas format dinamis, **Smart Classification & Policy Engine Otomatis**, sistem **Presets TOML**, **Music Specialization (1:1 Cover Art & Lirik LRCLIB)**, **TikTok / Shorts Specialization (TikWM Engine + 10-Client Fallback)**, **Batch Download**, **System Diagnostics (`doctor`)**, **Shell Auto-Completions**, serta **Interactive Terminal Wizard UI**.

---

## ✨ Fitur Utama (v1.0.0 Stable Release)

- **⚡ Fast Native Binary**: Dibangun dengan Rust murni untuk performa tinggi, efisiensi memori, dan nol ketergantungan script runtime Python.
- **🩺 System Health Doctor (v1.0)**: Subcommand `dlp doctor` untuk memverifikasi ketersediaan `yt-dlp`, `ffmpeg`, `ffprobe`, konfigurasi, dan konektivitas API LRCLIB & TikWM.
- **🐚 Shell Auto-Completions (v1.0)**: Subcommand `dlp completions <SHELL>` untuk menghasilkan skrip *autocompletion* native untuk **Bash**, **Zsh**, **Fish**, **PowerShell**, dan **Elvish**.
- **🤖 Smart Media Classification (v0.7)**: Otomatis mendeteksi tipe konten (*Music*, *Shorts/TikTok Vertical*, atau *Standard Video*) dari URL dan orientasi metadata tanpa perlu memasukkan flag manual. Cukup jalankan `dlp <URL>`.
- **📱 TikTok / Shorts Specialization (v0.6)**: Optimasi khusus untuk video vertikal (TikTok, YouTube Shorts, Instagram Reels):
  - **TikWM Native Engine (Primary)**: Pengunduhan berkecepatan tinggi murni tanpa watermark.
  - **10-Client Impersonation Rotation (Secondary Fallback)**: Proteksi anti-bot rotasi TLS fingerprint browser (`safari-18.0`, `chrome-136`, `edge-101`, `firefox-135`, dll.).
  - Pengelompokan folder otomatis berdasarkan Creator/Uploader (`Creator/YYYY-MM-DD_ID_Title.mp4`).
  - Penegakan resolusi vertikal optimal (`max_vertical = 1440p`).
- **🎵 Music Specialization (v0.5)**: Ekstraksi audio Opus native kualitas studio lengkap dengan:
  - **1:1 Square Cropped Cover Art** (FFmpeg otomatis memotong thumbnail 16:9 menjadi cover kotak 1:1).
  - **Native Synced Lyrics Fetcher (LRCLIB)**: Otomatis mencari dan menyimpan berkas `.lrc` mandiri langsung dari Rust tanpa Python eksternal.
  - **Extended Metadata Tags** (Artist, Album, Title, Release Date).
- **📦 Smart Batch Download (v0.4)**: Mengunduh puluhan/ratusan media sekaligus dari berkas teks (`urls.txt`) atau daftar link langsung dengan auto-klasifikasi per-item dan *Batch Summary Report*.
- **🧙 Interactive Terminal Wizard (v0.3)**: Cukup ketik `dlp` untuk membuka wizard interaktif berbasis menu, prompt lirik, dynamic quality selector, dan navigasi *Back* yang intuitif.
- **📐 Orientation Detection**: Otomatis mendeteksi rasio aspek:
  - `Horizontal` (16:9 / 4:3)
  - `Vertical` (9:16 Shorts / TikTok / Reels)
  - `Square` (1:1)

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

### 3. Smart Batch Download
```bash
# Otomatis mengklasifikasikan dan mengunduh tiap URL di urls.txt:
dlp batch urls.txt
```

### 4. Diagnostik Sistem (Doctor)
```bash
dlp doctor
```

### 5. Menghasilkan Shell Auto-Completions
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
Semua 21 pengujian unit dan integrasi terverifikasi 100% *green*.
