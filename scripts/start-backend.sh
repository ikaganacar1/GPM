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
if pgrep -f "gpumon" > /dev/null; then
    echo -e "${YELLOW}Service is already running${NC}"
    echo "PID: $(pgrep -f 'gpumon')"
    echo ""
    echo "To restart:"
    echo "  1. Stop: pkill -f gpumon"
    echo "  2. Run this script again"
    exit 0
fi

# Build if needed
if [ ! -f "target/release/gpumon" ]; then
    echo -e "${YELLOW}Building release binary...${NC}"
    cargo build --release --package gpumon-core
fi

# Create data directory
mkdir -p ~/.local/share/gpumon
mkdir -p ~/.config/gpumon

# Copy example config if not exists
if [ ! -f ~/.config/gpumon/config.toml ]; then
    echo -e "${YELLOW}Creating default config...${NC}"
    cp config.example.toml ~/.config/gpumon/config.toml 2>/dev/null || true
fi

# Start the service
echo -e "${GREEN}Starting GPU monitoring service...${NC}"
./target/release/gpumon &

BACKEND_PID=$!
echo "Service started with PID: $BACKEND_PID"
echo ""
echo "Logs are visible in journalctl (if running as service) or in output"
echo ""
echo "To stop: pkill -f gpumon"
echo "Prometheus metrics: http://localhost:9090/metrics"
