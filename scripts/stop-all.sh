#!/bin/bash
# GPM Stop Script
# Stops all GPM services

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${YELLOW}Stopping GPM services...${NC}"

# Stop backend
if pgrep -f "gpumon" > /dev/null; then
    echo "Stopping backend service..."
    pkill -f "gpumon"
    echo -e "${GREEN}Backend stopped${NC}"
else
    echo -e "${YELLOW}Backend not running${NC}"
fi

# Stop frontend
if lsof -Pi :8009 -sTCP:LISTEN -t >/dev/null 2>&1; then
    echo "Stopping frontend dashboard..."
    fuser -k 8009/tcp 2>/dev/null || true
    echo -e "${GREEN}Frontend stopped${NC}"
else
    echo -e "${YELLOW}Frontend not running${NC}"
fi

echo ""
echo -e "${GREEN}All GPM services stopped${NC}"
