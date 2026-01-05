#!/bin/bash
# GPM Full Deployment Script
# Starts both backend service and frontend dashboard

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_ROOT"

echo -e "${BLUE}GPM - GPU & LLM Monitoring${NC}"
echo "======================================"
echo ""

# Function to cleanup on exit
cleanup() {
    echo ""
    echo -e "${YELLOW}Stopping services...${NC}"
    pkill -f "python3 -m http.server 8009" 2>/dev/null || true
    pkill -f "target/release/gpm" 2>/dev/null || true
    echo -e "${GREEN}All services stopped${NC}"
    exit 0
}

# Trap SIGINT and SIGTERM
trap cleanup SIGINT SIGTERM

# Start backend
echo -e "${GREEN}[1/2] Starting backend service...${NC}"
if ! pgrep -f "target/release/gpm" > /dev/null; then
    if [ ! -f "target/release/gpm" ]; then
        echo -e "${YELLOW}Building backend...${NC}"
        cargo build --release --package gpm-core
    fi

    mkdir -p ~/.local/share/gpm
    mkdir -p ~/.config/gpm

    if [ ! -f ~/.config/gpm/config.toml ] && [ -f config.example.toml ]; then
        cp config.example.toml ~/.config/gpm/config.toml
    fi

    ./target/release/gpm > /tmp/gpm.log 2>&1 &
    sleep 2
    echo -e "${GREEN}Backend started${NC}"
else
    echo -e "${YELLOW}Backend already running${NC}"
fi

# Start frontend
echo -e "${GREEN}[2/2] Starting frontend dashboard...${NC}"
cd gpm-dashboard

if ! lsof -Pi :8009 -sTCP:LISTEN -t >/dev/null 2>&1; then
    if [ ! -d "dist" ]; then
        echo -e "${YELLOW}Building frontend...${NC}"
        npm run build
    fi

    cd dist
    python3 -m http.server 8009 > /dev/null 2>&1 &
    sleep 1
    echo -e "${GREEN}Frontend started${NC}"
else
    echo -e "${YELLOW}Frontend already running${NC}"
fi

cd "$PROJECT_ROOT"

echo ""
echo -e "${GREEN}======================================${NC}"
echo -e "${GREEN}GPM is now running!${NC}"
echo ""
echo "  Dashboard:  ${BLUE}http://localhost:8009${NC}"
echo "  Metrics:    ${BLUE}http://localhost:9090/metrics${NC}"
echo "  Database:   ~/.local/share/gpm/gpm.db"
echo ""
echo "Press Ctrl+C to stop all services"
echo ""

# Keep script running
while true; do
    sleep 1

    # Check if services are still running
    if ! pgrep -f "target/release/gpm" > /dev/null; then
        echo -e "${RED}Backend service stopped unexpectedly!${NC}"
        cleanup
    fi

    if ! lsof -Pi :8009 -sTCP:LISTEN -t >/dev/null 2>&1; then
        echo -e "${RED}Frontend service stopped unexpectedly!${NC}"
        cleanup
    fi
done
