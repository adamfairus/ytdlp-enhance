#!/usr/bin/env bash
# ==============================================================================
# 🦀 dlp — Universal Single-Line Automated Installer (v2.0)
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/adamfairus/ytdlp-enhance/main/install.sh | bash
# ==============================================================================

set -euo pipefail

BOLD='\033[1m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
RESET='\033[0m'

REPO="adamfairus/ytdlp-enhance"
BINARY_NAME="dlp"

echo -e "\n${CYAN}${BOLD}╔═══════════════════════════════════════════════════════════╗${RESET}"
echo -e "${CYAN}${BOLD}║       🦀 dlp — Universal Installer & Setup Wizard         ║${RESET}"
echo -e "${CYAN}${BOLD}║   Intelligent Orchestration Layer for yt-dlp & ffmpeg     ║${RESET}"
echo -e "${CYAN}${BOLD}╚═══════════════════════════════════════════════════════════╝${RESET}\n"

# 1. Detect OS & Architecture
OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"

case "$ARCH" in
    x86_64|amd64)
        TARGET_ARCH="x86_64"
        ;;
    aarch64|arm64)
        TARGET_ARCH="aarch64"
        ;;
    *)
        echo -e "${RED}❌ Unsupported architecture: $ARCH${RESET}"
        exit 1
        ;;
esac

case "$OS" in
    linux)
        TARGET_OS="unknown-linux-gnu"
        ;;
    darwin)
        TARGET_OS="apple-darwin"
        ;;
    *)
        echo -e "${RED}❌ Unsupported operating system: $OS${RESET}"
        exit 1
        ;;
esac

echo -e "• Detected Platform : ${GREEN}$OS ($TARGET_ARCH)${RESET}"

# 2. Determine Installation Target Directory
if [ "$(id -u)" -eq 0 ]; then
    INSTALL_DIR="/usr/local/bin"
else
    INSTALL_DIR="$HOME/.local/bin"
fi

mkdir -p "$INSTALL_DIR"
echo -e "• Target Directory  : ${GREEN}$INSTALL_DIR${RESET}"

# 3. Locate or Download Release Binary
TARGET_BIN="$INSTALL_DIR/$BINARY_NAME"

# If running inside local repository with compiled release binary, copy it
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")" 2>/dev/null && pwd || echo "")"
if [ -n "$SCRIPT_DIR" ] && [ -f "$SCRIPT_DIR/target/release/$BINARY_NAME" ]; then
    echo -e "• Installing locally built release binary..."
    cp -f "$SCRIPT_DIR/target/release/$BINARY_NAME" "$TARGET_BIN"
else
    echo -e "• Fetching latest release binary from GitHub ($REPO)..."
    TMP_DIR="$(mktemp -d)"
    trap 'rm -rf "$TMP_DIR"' EXIT

    LATEST_TAG=$(curl -s "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')
    if [ -z "$LATEST_TAG" ]; then
        LATEST_TAG="v2.0.0"
    fi

    DOWNLOAD_URL="https://github.com/$REPO/releases/download/${LATEST_TAG}/dlp-${LATEST_TAG}-${TARGET_ARCH}.tar.gz"
    echo -e "• Downloading: ${CYAN}$DOWNLOAD_URL${RESET}"

    if curl -sLf "$DOWNLOAD_URL" -o "$TMP_DIR/dlp.tar.gz"; then
        tar -xzf "$TMP_DIR/dlp.tar.gz" -C "$TMP_DIR"
        cp -f "$TMP_DIR/$BINARY_NAME" "$TARGET_BIN"
    else
        # Fallback to direct binary copy or building from source if cargo exists
        if command -v cargo >/dev/null 2>&1 && [ -d "$SCRIPT_DIR" ]; then
            echo -e "${YELLOW}ℹ️  Pre-built archive not found on mirror. Compiling via cargo...${RESET}"
            cargo build --release --manifest-path "$SCRIPT_DIR/Cargo.toml"
            cp -f "$SCRIPT_DIR/target/release/$BINARY_NAME" "$TARGET_BIN"
        else
            echo -e "${RED}❌ Failed to download release asset from $DOWNLOAD_URL${RESET}"
            exit 1
        fi
    fi
fi

chmod +x "$TARGET_BIN"
echo -e "✅ Installed ${BOLD}$BINARY_NAME${RESET} to ${GREEN}$TARGET_BIN${RESET}"

# 4. PATH Configuration Check
case ":$PATH:" in
    *":$INSTALL_DIR:"*)
        echo -e "• PATH Check        : ${GREEN}OK ($INSTALL_DIR is in your PATH)${RESET}"
        ;;
    *)
        echo -e "${YELLOW}⚠️  $INSTALL_DIR is not in your current PATH.${RESET}"
        echo -e "   Add the following line to your ~/.bashrc or ~/.zshrc:"
        echo -e "   ${CYAN}export PATH=\"\$HOME/.local/bin:\$PATH\"${RESET}\n"
        ;;
esac

# 5. Core Dependencies Inspection
echo -e "\n${BOLD}🔍 Inspecting Core Dependencies:${RESET}"

if command -v yt-dlp >/dev/null 2>&1; then
    YTDLP_VER=$(yt-dlp --version 2>/dev/null || echo "installed")
    echo -e "• yt-dlp : ${GREEN}✅ Installed (version: $YTDLP_VER)${RESET}"
else
    echo -e "• yt-dlp : ${YELLOW}⚠️  NOT FOUND${RESET} (Recommended: pip install -U yt-dlp)"
fi

if command -v ffmpeg >/dev/null 2>&1; then
    echo -e "• ffmpeg : ${GREEN}✅ Installed${RESET}"
else
    echo -e "• ffmpeg : ${YELLOW}⚠️  NOT FOUND${RESET} (Recommended: sudo apt install ffmpeg / brew install ffmpeg)"
fi

if command -v aria2c >/dev/null 2>&1; then
    echo -e "• aria2c : ${GREEN}✅ Installed (multi-connection acceleration ready)${RESET}"
else
    echo -e "• aria2c : ${CYAN}ℹ️  Optional${RESET} (Install aria2 for high-speed multi-connection downloads)"
fi

# 6. Initialize Config if missing
"$TARGET_BIN" config init >/dev/null 2>&1 || true

echo -e "\n${GREEN}${BOLD}🎉 Installation Complete!${RESET}"
echo -e "Run ${CYAN}${BOLD}dlp doctor${RESET} to verify your system readiness."
echo -e "Run ${CYAN}${BOLD}dlp --help${RESET} to explore presets, batching, and intelligent features.\n"
