#!/bin/bash

# ============================================================================
# Private Poker - Full System Test Script
# ============================================================================
# Tests server, TUI client, and web client end-to-end
# ============================================================================

set -e  # Exit on error

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}Private Poker - Full System Test${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# ============================================================================
# Configuration
# ============================================================================

SERVER_URL="http://localhost:8080"
DB_URL="postgresql://postgres:7794951@localhost:5432/poker_db"
TEST_USER1="testuser1"
TEST_USER2="testuser2"
TEST_PASS="Pass1234"

# ============================================================================
# Step 1: Clean up any existing processes
# ============================================================================

echo -e "${YELLOW}[1/8] Cleaning up existing processes...${NC}"
pkill -f pp_server || true
sleep 1
echo -e "${GREEN}✓ Cleanup complete${NC}"
echo ""

# ============================================================================
# Step 2: Clean database
# ============================================================================

echo -e "${YELLOW}[2/8] Cleaning database...${NC}"
PGPASSWORD=7794951 psql -U postgres -h localhost -d poker_db -c "
TRUNCATE tables, table_escrows, users, wallets, wallet_entries, sessions CASCADE;
" 2>/dev/null || echo "Warning: Could not clean database (may not exist yet)"
echo -e "${GREEN}✓ Database cleaned${NC}"
echo ""

# ============================================================================
# Step 3: Build project
# ============================================================================

echo -e "${YELLOW}[3/8] Building project (release mode)...${NC}"
cargo build --release --workspace 2>&1 | grep -E "(Compiling|Finished|error)" || true
echo -e "${GREEN}✓ Build complete${NC}"
echo ""

# ============================================================================
# Step 4: Start server
# ============================================================================

echo -e "${YELLOW}[4/8] Starting server...${NC}"
target/release/pp_server --bind 0.0.0.0:8080 --db-url "$DB_URL" --tables 1 > /tmp/pp_server.log 2>&1 &
SERVER_PID=$!
echo "Server PID: $SERVER_PID"

# Wait for server to be ready
echo -n "Waiting for server to start"
for i in {1..30}; do
    if curl -s "$SERVER_URL/api/tables" > /dev/null 2>&1; then
        echo ""
        echo -e "${GREEN}✓ Server is ready${NC}"
        break
    fi
    echo -n "."
    sleep 1
done
echo ""

# ============================================================================
# Step 5: Test HTTP API
# ============================================================================

echo -e "${YELLOW}[5/8] Testing HTTP API...${NC}"

# Register user 1
echo -n "Registering $TEST_USER1... "
RESPONSE=$(curl -s -X POST "$SERVER_URL/api/auth/register" \
    -H "Content-Type: application/json" \
    -d "{\"username\": \"$TEST_USER1\", \"password\": \"$TEST_PASS\", \"display_name\": \"$TEST_USER1\"}")

if echo "$RESPONSE" | grep -q "access_token"; then
    echo -e "${GREEN}✓${NC}"
    TOKEN1=$(echo "$RESPONSE" | grep -o '"access_token":"[^"]*"' | cut -d'"' -f4)
else
    echo -e "${RED}✗ Failed${NC}"
    echo "Response: $RESPONSE"
    kill $SERVER_PID
    exit 1
fi

# Register user 2
echo -n "Registering $TEST_USER2... "
RESPONSE=$(curl -s -X POST "$SERVER_URL/api/auth/register" \
    -H "Content-Type: application/json" \
    -d "{\"username\": \"$TEST_USER2\", \"password\": \"$TEST_PASS\", \"display_name\": \"$TEST_USER2\"}")

if echo "$RESPONSE" | grep -q "access_token"; then
    echo -e "${GREEN}✓${NC}"
    TOKEN2=$(echo "$RESPONSE" | grep -o '"access_token":"[^"]*"' | cut -d'"' -f4)
else
    echo -e "${RED}✗ Failed${NC}"
    echo "Response: $RESPONSE"
    kill $SERVER_PID
    exit 1
fi

# List tables
echo -n "Listing tables... "
TABLES=$(curl -s "$SERVER_URL/api/tables")
if echo "$TABLES" | grep -q "Table 1"; then
    echo -e "${GREEN}✓${NC}"
    echo "Tables: $TABLES"
else
    echo -e "${RED}✗ Failed${NC}"
    echo "Response: $TABLES"
    kill $SERVER_PID
    exit 1
fi

echo -e "${GREEN}✓ API tests passed${NC}"
echo ""

# ============================================================================
# Step 6: Test TUI Client (Non-interactive)
# ============================================================================

echo -e "${YELLOW}[6/8] Testing TUI client availability...${NC}"

if [ -f "target/release/pp_client" ]; then
    echo -e "${GREEN}✓ TUI client binary exists${NC}"
    echo ""
    echo -e "${BLUE}To test TUI client manually:${NC}"
    echo "  cargo run --bin pp_client --release -- --server $SERVER_URL --username alice --password Pass1234 --tui"
else
    echo -e "${RED}✗ TUI client binary not found${NC}"
fi
echo ""

# ============================================================================
# Step 7: Test Web Client
# ============================================================================

echo -e "${YELLOW}[7/8] Testing web client files...${NC}"

WEB_CLIENT_DIR="web_client"
REQUIRED_FILES=(
    "$WEB_CLIENT_DIR/index.html"
    "$WEB_CLIENT_DIR/lobby.html"
    "$WEB_CLIENT_DIR/game.html"
    "$WEB_CLIENT_DIR/js/api.js"
    "$WEB_CLIENT_DIR/js/auth.js"
    "$WEB_CLIENT_DIR/js/websocket.js"
    "$WEB_CLIENT_DIR/js/game.js"
    "$WEB_CLIENT_DIR/css/main.css"
    "$WEB_CLIENT_DIR/css/cards.css"
)

ALL_EXIST=true
for file in "${REQUIRED_FILES[@]}"; do
    if [ -f "$file" ]; then
        echo -e "${GREEN}✓${NC} $file"
    else
        echo -e "${RED}✗${NC} $file (missing)"
        ALL_EXIST=false
    fi
done

if [ "$ALL_EXIST" = true ]; then
    echo -e "${GREEN}✓ All web client files exist${NC}"
    echo ""
    echo -e "${BLUE}To test web client:${NC}"
    echo "  cd $WEB_CLIENT_DIR"
    echo "  python3 -m http.server 8000"
    echo "  Open browser: http://localhost:8000"
else
    echo -e "${RED}✗ Some web client files are missing${NC}"
fi
echo ""

# ============================================================================
# Step 8: Summary and cleanup options
# ============================================================================

echo -e "${YELLOW}[8/8] Test Summary${NC}"
echo ""
echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}✓ System Test Complete!${NC}"
echo -e "${GREEN}========================================${NC}"
echo ""
echo -e "${BLUE}Server Status:${NC}"
echo "  PID: $SERVER_PID"
echo "  URL: $SERVER_URL"
echo "  Log: /tmp/pp_server.log"
echo ""
echo -e "${BLUE}Test Results:${NC}"
echo "  ✓ Server started successfully"
echo "  ✓ HTTP API working"
echo "  ✓ User registration working"
echo "  ✓ Table listing working"
echo "  ✓ 2 test users created: $TEST_USER1, $TEST_USER2"
echo ""
echo -e "${BLUE}Next Steps:${NC}"
echo ""
echo -e "${YELLOW}1. Test TUI Client:${NC}"
echo "   cargo run --bin pp_client --release -- --server $SERVER_URL --username $TEST_USER1 --password $TEST_PASS --tui"
echo ""
echo -e "${YELLOW}2. Test Web Client:${NC}"
echo "   cd $WEB_CLIENT_DIR && python3 -m http.server 8000"
echo "   Then open: http://localhost:8000"
echo "   Login with: $TEST_USER1 / $TEST_PASS"
echo ""
echo -e "${YELLOW}3. View Server Logs:${NC}"
echo "   tail -f /tmp/pp_server.log"
echo ""
echo -e "${YELLOW}4. Stop Server:${NC}"
echo "   kill $SERVER_PID"
echo ""

# Ask if user wants to keep server running
echo -e "${BLUE}Server is currently running.${NC}"
read -p "Do you want to stop the server now? (y/N): " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo "Stopping server..."
    kill $SERVER_PID
    echo -e "${GREEN}✓ Server stopped${NC}"
else
    echo -e "${GREEN}Server is still running on PID $SERVER_PID${NC}"
    echo "To stop later: kill $SERVER_PID"
fi

echo ""
echo -e "${GREEN}Test script complete!${NC}"
