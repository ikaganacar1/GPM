#!/bin/bash
# GPM Stop Script
# Stops all GPM services

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

FRONTEND_PORT=8011

echo -e "${YELLOW}Stopping GPM services...${NC}"

# Stop monitoring service
if pgrep -f "target/release/gpm" > /dev/null; then
    echo "Stopping monitoring service..."
    pkill -f "target/release/gpm"
    echo -e "${GREEN}Monitoring service stopped${NC}"
else
    echo -e "${YELLOW}Monitoring service not running${NC}"
fi

# Stop API server
if pgrep -f "gpm-server" > /dev/null; then
    echo "Stopping API server..."
    pkill -f "gpm-server"
    echo -e "${GREEN}API server stopped${NC}"
else
    echo -e "${YELLOW}API server not running${NC}"
fi

# Stop frontend (reverse proxy)
if pgrep -f "gpm-dashboard/server.py" > /dev/null; then
    echo "Stopping frontend dashboard..."
    pkill -f "gpm-dashboard/server.py"
    echo -e "${GREEN}Frontend stopped${NC}"
elif lsof -Pi :$FRONTEND_PORT -sTCP:LISTEN -t >/dev/null 2>&1; then
    echo "Stopping frontend dashboard on port $FRONTEND_PORT..."
    fuser -k $FRONTEND_PORT/tcp 2>/dev/null || true
    echo -e "${GREEN}Frontend stopped${NC}"
else
    echo -e "${YELLOW}Frontend not running${NC}"
fi

# Also kill old port 8009 if still in use
if lsof -Pi :8009 -sTCP:LISTEN -t >/dev/null 2>&1; then
    echo "Stopping old frontend on port 8009..."
    fuser -k 8009/tcp 2>/dev/null || true
    echo -e "${GREEN}Old frontend stopped${NC}"
fi

echo ""
echo -e "${GREEN}All GPM services stopped${NC}"
