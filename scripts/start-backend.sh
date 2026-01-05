#!/bin/bash
# GPM Backend Deployment Script
# This script starts the GPU monitoring service

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_ROOT"

echo -e "${GREEN}GPM - GPU & LLM Monitoring Service${NC}"
echo "======================================"
echo ""

# Check if already running
if pgrep -f "target/release/gpm" > /dev/null; then
    echo -e "${YELLOW}Service is already running${NC}"
    echo "PID: $(pgrep -f 'target/release/gpm')"
    echo ""
    echo "To restart:"
    echo "  1. Stop: pkill -f 'target/release/gpm'"
    echo "  2. Run this script again"
    exit 0
fi

# Build if needed
if [ ! -f "target/release/gpm" ]; then
    echo -e "${YELLOW}Building release binary...${NC}"
    cargo build --release --package gpm-core
fi

# Create data directory
mkdir -p ~/.local/share/gpm
mkdir -p ~/.config/gpm

# Copy example config if not exists
if [ ! -f ~/.config/gpm/config.toml ]; then
    echo -e "${YELLOW}Creating default config...${NC}"
    cp config.example.toml ~/.config/gpm/config.toml 2>/dev/null || true
fi

# Start the service
echo -e "${GREEN}Starting GPU monitoring service...${NC}"
./target/release/gpm &

BACKEND_PID=$!
echo "Service started with PID: $BACKEND_PID"
echo ""
echo "Logs are visible in journalctl (if running as service) or in output"
echo ""
echo "To stop: pkill -f 'target/release/gpm'"
echo "Prometheus metrics: http://localhost:9090/metrics"
