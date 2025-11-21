#!/bin/bash

# Debug script to test game flow step by step

set -e

SERVER_URL="http://localhost:8080"

echo "=== Testing Game Flow ==="
echo ""

# Step 1: Register user 1
echo "1. Registering user1..."
RESPONSE1=$(curl -s -X POST "$SERVER_URL/api/auth/register" \
    -H "Content-Type: application/json" \
    -d '{"username": "user1", "password": "Pass1234", "display_name": "User1"}')

TOKEN1=$(echo "$RESPONSE1" | grep -o '"access_token":"[^"]*"' | cut -d'"' -f4)
echo "   Token: ${TOKEN1:0:20}..."
echo ""

# Step 2: Register user 2
echo "2. Registering user2..."
RESPONSE2=$(curl -s -X POST "$SERVER_URL/api/auth/register" \
    -H "Content-Type: application/json" \
    -d '{"username": "user2", "password": "Pass5678", "display_name": "User2"}')

TOKEN2=$(echo "$RESPONSE2" | grep -o '"access_token":"[^"]*"' | cut -d'"' -f4)
echo "   Token: ${TOKEN2:0:20}..."
echo ""

# Step 3: List tables
echo "3. Listing tables..."
TABLES=$(curl -s "$SERVER_URL/api/tables")
echo "   $TABLES"
echo ""

# Step 4: Get WebSocket URLs
WS_URL1="ws://localhost:8080/ws/1?token=$TOKEN1"
WS_URL2="ws://localhost:8080/ws/1?token=$TOKEN2"

echo "4. WebSocket URLs ready"
echo "   User1: ${WS_URL1:0:50}..."
echo "   User2: ${WS_URL2:0:50}..."
echo ""

echo "=== Manual Testing Instructions ==="
echo ""
echo "In Terminal 1 - User1 TUI Client:"
echo "cargo run --bin pp_client --release -- --server $SERVER_URL --username user1 --password Pass1234 --tui"
echo ""
echo "In Terminal 2 - User2 TUI Client:"
echo "cargo run --bin pp_client --release -- --server $SERVER_URL --username user2 --password Pass5678 --tui"
echo ""
echo "Expected flow:"
echo "1. Both see 'Table 1' in lobby"
echo "2. Both select table 1"
echo "3. Both enter buy-in (e.g., 1000)"
echo "4. Game should start when both have joined"
echo ""
echo "If game doesn't start, check:"
echo "- Both users actually joined (not just spectating)"
echo "- Minimum 2 players configured"
echo "- Table state in database"
