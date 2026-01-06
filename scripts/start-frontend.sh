#!/bin/bash
# GPM Frontend Deployment Script
# This script starts the dashboard with reverse proxy on port 8011
# The proxy serves frontend static files and forwards /api requests to port 8010

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DASHBOARD_DIR="$PROJECT_ROOT/gpm-dashboard"
FRONTEND_PORT=8011
API_PORT=8010

cd "$DASHBOARD_DIR"

echo -e "${GREEN}GPM Dashboard${NC}"
echo "======================================"
echo ""

# Check if API server is running
if ! lsof -Pi :$API_PORT -sTCP:LISTEN -t >/dev/null 2>&1; then
    echo -e "${RED}API server is not running on port $API_PORT${NC}"
    echo "Start it with: ./scripts/start-web.sh"
    exit 1
fi

# Check if frontend already running
if lsof -Pi :$FRONTEND_PORT -sTCP:LISTEN -t >/dev/null 2>&1; then
    echo -e "${YELLOW}Port $FRONTEND_PORT is already in use${NC}"
    echo "To stop: fuser -k $FRONTEND_PORT/tcp"
    exit 1
fi

# Build the frontend
if [ ! -d "dist" ]; then
    echo -e "${YELLOW}Building frontend...${NC}"
    npm run build
fi

# Serve with reverse proxy
echo -e "${GREEN}Starting dashboard on http://localhost:$FRONTEND_PORT${NC}"
echo "  Frontend: static files"
echo "  API proxy: -> http://localhost:$API_PORT"
echo ""
echo "To stop: fuser -k $FRONTEND_PORT/tcp"
echo ""

python3 server.py $FRONTEND_PORT
