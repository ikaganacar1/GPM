#!/bin/bash
# GPM Installation Script
# This script installs GPM (GPU & LLM Monitoring Service)
#
# Usage:
#   curl -sSL https://raw.githubusercontent.com/ikaganacar1/GPM/main/install.sh | bash
#   Or with specific version:
#   curl -sSL https://raw.githubusercontent.com/ikaganacar1/GPM/main/install.sh | bash -s -- --version v0.1.0

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Default values
VERSION="${VERSION:-latest}"
REPO="${REPO:-ikaganacar1/GPM}"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"
CONFIG_DIR="${CONFIG_DIR:-$HOME/.config/gpm}"
DATA_DIR="${DATA_DIR:-$HOME/.local/share/gpm}"
FRONTEND_PORT="${FRONTEND_PORT:-8011}"
API_PORT="${API_PORT:-8010}"
PROXY_PORT="${PROXY_PORT:-11434}"
SKIP_SYSTEMD="${SKIP_SYSTEMD:-false}"

# Parse arguments
while [[ $# -gt 0 ]]; do
  case $1 in
    --version)
      VERSION="$2"
      shift 2
      ;;
    --dir)
      INSTALL_DIR="$2"
      shift 2
      ;;
    --no-systemd)
      SKIP_SYSTEMD=true
      shift
      ;;
    -h|--help)
      echo "GPM Installation Script"
      echo ""
      echo "Usage: $0 [options]"
      echo ""
      echo "Options:"
      echo "  --version VERSION   Specific version to install (default: latest)"
      echo "  --dir DIR            Installation directory (default: ~/.local/bin)"
      echo "  --no-systemd         Skip systemd service installation"
      echo "  -h, --help           Show this help message"
      echo ""
      echo "Environment variables:"
      echo "  VERSION              Version to install"
      echo "  REPO                 Repository name (default: ikaganacar1/GPM)"
      echo "  INSTALL_DIR          Installation directory"
      echo "  FRONTEND_PORT        Dashboard port (default: 8011)"
      echo "  API_PORT             API port (default: 8010)"
      echo "  PROXY_PORT           Ollama proxy port (default: 11434)"
      exit 0
      ;;
    *)
      echo "Unknown option: $1"
      echo "Run '$0 --help' for usage"
      exit 1
      ;;
  esac
done

echo -e "${BLUE}GPM - GPU & LLM Monitoring Service${NC}"
echo "======================================"
echo ""

# Check prerequisites
echo -e "${GREEN}[1/6] Checking prerequisites...${NC}"

# Check if NVIDIA GPU is available
if command -v nvidia-smi &> /dev/null; then
    GPU_COUNT=$(nvidia-smi --list-gpus | wc -l)
    echo -e "  ${GREEN}✓${NC} NVIDIA GPU detected ($GPU_COUNT GPU(s))"
else
    echo -e "  ${YELLOW}⚠${NC}  nvidia-smi not found. GPM requires NVIDIA GPU."
    echo -e "  ${YELLOW}⚠${NC}  Installation will continue, but GPU monitoring may not work."
fi

# Check if Rust is available (for building from source if needed)
if ! command -v cargo &> /dev/null; then
    echo -e "  ${YELLOW}⚠${NC}  Rust not found. Will use pre-built binaries."
fi

# Detect architecture
ARCH=$(uname -m)
case $ARCH in
    x86_64)
        BINARY_ARCH="x86_64"
        echo -e "  ${GREEN}✓${NC} Architecture: x86_64"
        ;;
    aarch64)
        BINARY_ARCH="arm64"
        echo -e "  ${GREEN}✓${NC} Architecture: ARM64"
        ;;
    *)
        echo -e "  ${RED}✗${NC} Unsupported architecture: $ARCH"
        echo -e "  ${YELLOW}⚠${NC}  Falling back to source installation..."
        SOURCE_BUILD=true
        ;;
esac

# Create directories
echo -e "${GREEN}[2/6] Creating directories...${NC}"
mkdir -p "$INSTALL_DIR"
mkdir -p "$CONFIG_DIR"
mkdir -p "$DATA_DIR"
mkdir -p "$DATA_DIR/archive"
echo -e "  ${GREEN}✓${NC} Install dir: $INSTALL_DIR"
echo -e "  ${GREEN}✓${NC} Config dir: $CONFIG_DIR"
echo -e "  ${GREEN}✓${NC} Data dir: $DATA_DIR"

# Download or build binaries
echo -e "${GREEN}[3/6] Installing binaries...${NC}"

if [ "$SOURCE_BUILD" = true ]; then
    # Build from source
    if ! command -v cargo &> /dev/null; then
        echo -e "  ${RED}✗${NC} Rust/Cargo not found. Please install Rust first:"
        echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        exit 1
    fi

    TEMP_DIR=$(mktemp -d)
    cd "$TEMP_DIR"
    echo -e "  ${YELLOW}⏳${NC}  Cloning repository..."
    git clone "https://github.com/$REPO.git" gpm
    cd gpm
    echo -e "  ${YELLOW}⏳${NC}  Building (this may take a few minutes)..."
    cargo build --release --package gpm-core
    cp target/release/gpm "$INSTALL_DIR/"
    cp target/release/gpm-server "$INSTALL_DIR/"
    cd -
    rm -rf "$TEMP_DIR"
else
    # Download pre-built binary
    DOWNLOAD_URL="https://github.com/$REPO/releases/download/$VERSION/gpm-$BINARY_ARCH"
    echo -e "  ${YELLOW}⏳${NC}  Downloading from: $DOWNLOAD_URL"

    if curl -L "$DOWNLOAD_URL" -o "$INSTALL_DIR/gpm" 2>/dev/null; then
        chmod +x "$INSTALL_DIR/gpm"
        echo -e "  ${GREEN}✓${NC} gpm installed"
    else
        echo -e "  ${YELLOW}⚠${NC}  Pre-built binary not found. Building from source..."
        TEMP_DIR=$(mktemp -d)
        cd "$TEMP_DIR"
        git clone "https://github.com/$REPO.git" gpm
        cd gpm
        cargo build --release --package gpm-core
        cp target/release/gpm "$INSTALL_DIR/"
        cp target/release/gpm-server "$INSTALL_DIR/"
        cd -
        rm -rf "$TEMP_DIR"
    fi
fi

# Verify installation
if "$INSTALL_DIR/gpm" --help &> /dev/null; then
    echo -e "  ${GREEN}✓${NC} Binary verified"
else
    echo -e "  ${RED}✗${NC} Binary verification failed"
    exit 1
fi

# Create default configuration
echo -e "${GREEN}[4/6] Creating configuration...${NC}"

if [ ! -f "$CONFIG_DIR/config.toml" ]; then
    cat > "$CONFIG_DIR/config.toml" <<EOF
[service]
poll_interval_secs = 2
data_dir = "$DATA_DIR"

[gpu]
enable_nvml = true
fallback_to_nvidia_smi = false

[ollama]
enabled = true
enable_proxy = true
proxy_port = $PROXY_PORT
backend_url = "http://localhost:11435"
api_port = $PROXY_PORT
api_url = "http://localhost:$PROXY_PORT"

[storage]
retention_days = 7
enable_parquet_archival = true
archive_dir = "$DATA_DIR/archive"

[telemetry]
enable_opentelemetry = false
enable_prometheus = true
metrics_port = 9090
EOF
    echo -e "  ${GREEN}✓${NC} Configuration created"
else
    echo -e "  ${YELLOW}⚠${NC}  Configuration already exists, skipping"
fi

# Install systemd service
if [ "$SKIP_SYSTEMD" = false ] && command -v systemctl &> /dev/null; then
    echo -e "${GREEN}[5/6] Installing systemd service...${NC}"

    cat > "$HOME/.config/systemd/user/gpm.service" <<EOF
[Unit]
Description=GPM GPU & LLM Monitor
After=network.target

[Service]
Type=simple
ExecStart=$INSTALL_DIR/gpm
Restart=on-failure
RestartSec=5s

[Install]
WantedBy=default.target
EOF

    cat > "$HOME/.config/systemd/user/gpm-server.service" <<EOF
[Unit]
Description=GPM Web API Server
After=network.target gpm.service

[Service]
Type=simple
ExecStart=$INSTALL_DIR/gpm-server
Restart=on-failure
RestartSec=5s

[Install]
WantedBy=default.target
EOF

    systemctl --user daemon-reload 2>/dev/null || true
    echo -e "  ${GREEN}✓${NC} Systemd service installed"
    echo -e "  ${YELLOW}→${NC}  Enable with: systemctl --user enable gpm"
    echo -e "  ${YELLOW}→${NC}  Start with: systemctl --user start gpm"
else
    echo -e "${GREEN}[5/6] ${YELLOW}Skipping systemd service${NC}"
    echo -e "  Start manually with: $INSTALL_DIR/gpm"
fi

# Run the service
echo -e "${GREEN}[6/6] Starting GPM...${NC}"

if command -v systemctl &> /dev/null && [ "$SKIP_SYSTEMD" = false ]; then
    systemctl --user start gpm 2>/dev/null || true
    systemctl --user start gpm-server 2>/dev/null || true
    echo -e "  ${GREEN}✓${NC} GPM service started"
else
    # Start in background
    "$INSTALL_DIR/gpm" > /tmp/gpm.log 2>&1 &
    "$INSTALL_DIR/gpm-server" > /tmp/gpm-server.log 2>&1 &
    echo -e "  ${GREEN}✓${NC} GPM started in background"
    echo -e "  ${YELLOW}→${NC}  Logs: /tmp/gpm.log"
fi

echo ""
echo -e "${GREEN}======================================${NC}"
echo -e "${GREEN}GPM installed successfully!${NC}"
echo ""
echo "Access the dashboard at: ${BLUE}http://localhost:$FRONTEND_PORT${NC}"
echo ""
echo "Commands:"
echo "  gpm              # Start monitoring service"
echo "  gpm-server       # Start web API server"
echo ""
echo "Configuration: $CONFIG_DIR/config.toml"
echo "Database: $DATA_DIR/gpm.db"
echo ""
echo -e "${YELLOW}For Ollama integration:${NC}"
echo "  1. Run Ollama on port 11435:"
echo "     ${BLUE}OLLAMA_HOST=127.0.0.1:11435 ollama serve${NC}"
echo "  2. Use through GPM proxy on port $PROXY_PORT"
echo ""
echo "Uninstall:"
echo "  systemctl --user stop gpm gpm-server"
echo "  systemctl --user disable gpm gpm-server"
echo "  rm -rf $INSTALL_DIR/gpm $INSTALL_DIR/gpm-server"
echo "  rm $HOME/.config/systemd/user/gpm.service"
echo "  rm $HOME/.config/systemd/user/gpm-server.service"
