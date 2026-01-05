#!/bin/bash
# GPM Frontend Deployment Script
# This script starts the dashboard on port 8009

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DASHBOARD_DIR="$PROJECT_ROOT/gpm-dashboard"
PORT=8009

cd "$DASHBOARD_DIR"

echo -e "${GREEN}GPM Dashboard${NC}"
echo "======================================"
echo ""

# Check if already running
if lsof -Pi :$PORT -sTCP:LISTEN -t >/dev/null 2>&1; then
    echo -e "${YELLOW}Port $PORT is already in use${NC}"
    echo "To stop: fuser -k $PORT/tcp"
    exit 1
fi

# Build the frontend
echo -e "${YELLOW}Building frontend...${NC}"
npm run build

# Serve the built files
echo -e "${GREEN}Starting dashboard on http://localhost:$PORT${NC}"
echo ""
echo "To stop: fuser -k $PORT/tcp"
echo ""

cd dist && python3 -m http.server $PORT
