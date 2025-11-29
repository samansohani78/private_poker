#!/bin/bash
# Load testing script for Private Poker server
#
# Requirements: wrk (https://github.com/wg/wrk)
# Install: sudo apt-get install wrk (Ubuntu/Debian)
#
# Usage:
#   ./scripts/load_test.sh [server_url] [duration] [connections] [threads]
#
# Examples:
#   ./scripts/load_test.sh                              # Default: localhost, 30s, 100 connections, 4 threads
#   ./scripts/load_test.sh http://localhost:8080 60 200 8  # Custom settings

set -e

# Configuration
SERVER_URL="${1:-http://localhost:8080}"
DURATION="${2:-30s}"
CONNECTIONS="${3:-100}"
THREADS="${4:-4}"

# Colors
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${GREEN}=== Private Poker Load Test ===${NC}"
echo ""
echo "Configuration:"
echo "  Server URL:    ${SERVER_URL}"
echo "  Duration:      ${DURATION}"
echo "  Connections:   ${CONNECTIONS}"
echo "  Threads:       ${THREADS}"
echo ""

# Check if wrk is installed
if ! command -v wrk &> /dev/null; then
    echo -e "${RED}ERROR: wrk is not installed${NC}"
    echo "Install with: sudo apt-get install wrk"
    exit 1
fi

# Test 1: Health endpoint
echo -e "${YELLOW}Test 1: Health endpoint${NC}"
wrk -t${THREADS} -c${CONNECTIONS} -d${DURATION} --latency "${SERVER_URL}/health"
echo ""

# Test 2: List tables endpoint (requires auth, will get 401s but tests throughput)
echo -e "${YELLOW}Test 2: List tables endpoint${NC}"
wrk -t${THREADS} -c${CONNECTIONS} -d${DURATION} --latency "${SERVER_URL}/api/tables"
echo ""

echo -e "${GREEN}Load testing complete!${NC}"
echo ""
echo "Performance targets:"
echo "  Health check:  < 10ms p95"
echo "  List tables:   < 50ms p95"
echo "  Join table:    < 100ms p95"
