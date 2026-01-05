#!/bin/bash
# GPM Web Deployment Script
# Starts the web API server and frontend dashboard

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_ROOT"

echo -e "${BLUE}GPM - Web Mode${NC}"
echo "======================================"
echo ""

# Function to cleanup on exit
cleanup() {
    echo ""
    echo -e "${YELLOW}Stopping services...${NC}"
    pkill -f "python3 -m http.server 8009" 2>/dev/null || true
    pkill -f "web-server" 2>/dev/null || true
    echo -e "${GREEN}All services stopped${NC}"
    exit 0
}

# Trap SIGINT and SIGTERM
trap cleanup SIGINT SIGTERM

# Build binaries
echo -e "${YELLOW}Building binaries...${NC}"
cargo build --release --bin web-server

# Start web API server
echo -e "${GREEN}[1/2] Starting web API server on port 8010...${NC}"
./target/release/web-server > /tmp/gpumon-web-api.log 2>&1 &
API_PID=$!
sleep 2

# Check if API started
if ! curl -s http://localhost:8010/api/info > /dev/null 2>&1; then
    echo -e "${RED}Failed to start API server${NC}"
    cat /tmp/gpumon-web-api.log
    exit 1
fi
echo -e "${GREEN}API server started (PID: $API_PID)${NC}"

# Start frontend
echo -e "${GREEN}[2/2] Starting frontend on port 8009...${NC}"
cd gpumon-dashboard

# Build frontend if needed
if [ ! -d "dist" ]; then
    echo -e "${YELLOW}Building frontend...${NC}"
    npm run build
fi

cd dist
python3 -m http.server 8009 > /dev/null 2>&1 &
WEB_PID=$!
sleep 1

echo ""
echo -e "${GREEN}======================================${NC}"
echo -e "${GREEN}GPM Web is now running!${NC}"
echo ""
echo "  Dashboard:  ${BLUE}http://localhost:8009${NC}"
echo "  API:        ${BLUE}http://localhost:8010/api${NC}"
echo ""
echo "Press Ctrl+C to stop all services"
echo ""

# Keep script running
while true; do
    sleep 1

    # Check if services are still running
    if ! pgrep -f "web-server" > /dev/null; then
        echo -e "${RED}API server stopped unexpectedly!${NC}"
        cleanup
    fi

    if ! lsof -Pi :8009 -sTCP:LISTEN -t >/dev/null 2>&1; then
        echo -e "${RED}Frontend service stopped unexpectedly!${NC}"
        cleanup
    fi
done
