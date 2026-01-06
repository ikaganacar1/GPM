#!/bin/bash
# GPM Full Deployment Script
# Starts monitoring service, API server, and frontend dashboard with reverse proxy

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_ROOT"

FRONTEND_PORT=8011
API_PORT=8010

# Check for --background flag
if [ "$1" = "--background" ] || [ "$1" = "-b" ]; then
    BACKGROUND_MODE=true
else
    BACKGROUND_MODE=false
fi

echo -e "${BLUE}GPM - GPU & LLM Monitoring${NC}"
echo "======================================"
echo ""

# Function to cleanup on exit
cleanup() {
    echo ""
    echo -e "${YELLOW}Stopping services...${NC}"
    pkill -f "gpm-dashboard/server.py" 2>/dev/null || true
    pkill -f "target/release/gpm-server" 2>/dev/null || true
    pkill -f "target/release/gpm" 2>/dev/null || true
    sleep 1
    echo -e "${GREEN}All services stopped${NC}"
    exit 0
}

# Only trap signals if not in background mode
if [ "$BACKGROUND_MODE" = false ]; then
    trap cleanup SIGINT SIGTERM
fi

# Start monitoring service (gpm)
echo -e "${GREEN}[1/3] Starting monitoring service...${NC}"
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
    echo -e "${GREEN}Monitoring service started${NC}"
else
    echo -e "${YELLOW}Monitoring service already running${NC}"
fi

# Start API server (gpm-server)
echo -e "${GREEN}[2/3] Starting API server on port $API_PORT...${NC}"
if ! lsof -Pi :$API_PORT -sTCP:LISTEN -t >/dev/null 2>&1; then
    ./target/release/gpm-server > /tmp/gpm-server.log 2>&1 &
    sleep 2
    echo -e "${GREEN}API server started${NC}"
else
    echo -e "${YELLOW}API server already running${NC}"
fi

# Start frontend with reverse proxy
echo -e "${GREEN}[3/3] Starting frontend dashboard on port $FRONTEND_PORT...${NC}"
cd gpm-dashboard

# Stop existing frontend if running
if lsof -Pi :$FRONTEND_PORT -sTCP:LISTEN -t >/dev/null 2>&1; then
    echo -e "${YELLOW}Stopping existing frontend...${NC}"
    pkill -f "gpm-dashboard/server.py" 2>/dev/null || true
    sleep 1
fi

# Always rebuild frontend
echo -e "${YELLOW}Building frontend...${NC}"
npm run build

# Check if server.py exists
if [ ! -f "server.py" ]; then
    echo -e "${RED}server.py not found! Cannot start frontend.${NC}"
    cleanup
fi

python3 server.py $FRONTEND_PORT > /tmp/gpm-dashboard.log 2>&1 &
sleep 1
echo -e "${GREEN}Frontend started${NC}"

cd "$PROJECT_ROOT"

echo ""
echo -e "${GREEN}======================================${NC}"
echo -e "${GREEN}GPM is now running!${NC}"
echo ""
echo "  Dashboard:  ${BLUE}http://localhost:$FRONTEND_PORT${NC}"
echo "  API:        ${BLUE}http://localhost:$API_PORT${NC}"
echo "  Prometheus: ${BLUE}http://localhost:9090/metrics${NC}"
echo "  Database:   ~/.local/share/gpm/gpm.db"
echo ""
echo -e "${YELLOW}For Cloudflare Tunnel, forward to port $FRONTEND_PORT${NC}"
echo ""

# Exit if in background mode
if [ "$BACKGROUND_MODE" = true ]; then
    echo -e "${GREEN}Services running in background${NC}"
    echo "Run 'scripts/stop-all.sh' to stop all services"
    exit 0
fi

echo "Press Ctrl+C to stop all services"
echo ""

# Keep script running
while true; do
    sleep 1

    # Check if services are still running
    if ! pgrep -f "target/release/gpm" > /dev/null; then
        echo -e "${RED}Monitoring service stopped unexpectedly!${NC}"
        cleanup
    fi

    if ! lsof -Pi :$API_PORT -sTCP:LISTEN -t >/dev/null 2>&1; then
        echo -e "${RED}API server stopped unexpectedly!${NC}"
        cleanup
    fi

    if ! lsof -Pi :$FRONTEND_PORT -sTCP:LISTEN -t >/dev/null 2>&1; then
        echo -e "${RED}Frontend service stopped unexpectedly!${NC}"
        cleanup
    fi
done
