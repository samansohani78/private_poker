#!/bin/bash

# Test Game Flow Script
# This script verifies that the game works end-to-end with 2 players

set -e

SERVER_URL="http://localhost:8080"

echo "========================================="
echo "Testing Full Game Flow"
echo "========================================="
echo ""

# Login Alice
echo "1. Logging in as Alice..."
ALICE_TOKEN=$(curl -s -X POST "$SERVER_URL/api/auth/login" \
  -H "Content-Type: application/json" \
  -d '{"username": "alice", "password": "Pass1234"}' | jq -r '.access_token')

if [ -z "$ALICE_TOKEN" ] || [ "$ALICE_TOKEN" == "null" ]; then
  echo "❌ Failed to login as Alice"
  exit 1
fi
echo "✓ Alice logged in"

# Login Bob
echo "2. Logging in as Bob..."
BOB_TOKEN=$(curl -s -X POST "$SERVER_URL/api/auth/login" \
  -H "Content-Type: application/json" \
  -d '{"username": "bob", "password": "Pass5678"}' | jq -r '.access_token')

if [ -z "$BOB_TOKEN" ] || [ "$BOB_TOKEN" == "null" ]; then
  echo "❌ Failed to login as Bob"
  exit 1
fi
echo "✓ Bob logged in"

# Check tables
echo "3. Checking tables..."
TABLES=$(curl -s "$SERVER_URL/api/tables")
echo "Available tables: $TABLES"

# Alice joins table
echo "4. Alice joining Table 1..."
ALICE_JOIN=$(curl -s -X POST "$SERVER_URL/api/tables/1/join" \
  -H "Authorization: Bearer $ALICE_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"buy_in_amount": 1000}')
echo "Alice join response: $ALICE_JOIN"

# Wait a moment
sleep 1

# Bob joins table
echo "5. Bob joining Table 1..."
BOB_JOIN=$(curl -s -X POST "$SERVER_URL/api/tables/1/join" \
  -H "Authorization: Bearer $BOB_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"buy_in_amount": 1000}')
echo "Bob join response: $BOB_JOIN"

# Wait a moment
sleep 1

# Check table state
echo "6. Checking table state..."
TABLE_STATE=$(curl -s "$SERVER_URL/api/tables/1")
echo "Table state: $TABLE_STATE"

# Get player count
PLAYER_COUNT=$(echo "$TABLE_STATE" | jq -r '.player_count')
echo ""
echo "========================================="
echo "Test Results:"
echo "  Players at table: $PLAYER_COUNT"
echo "  Expected: 2"
echo ""
if [ "$PLAYER_COUNT" == "2" ]; then
  echo "✓ Game should start automatically!"
  echo ""
  echo "Next steps:"
  echo "  1. Open two terminals"
  echo "  2. Terminal 1: cargo run --bin pp_client --release -- --server http://localhost:8080 --username alice --password Pass1234 --tui"
  echo "  3. Terminal 2: cargo run --bin pp_client --release -- --server http://localhost:8080 --username bob --password Pass5678 --tui"
  echo ""
  echo "Or test the web client:"
  echo "  1. cd web_client && python3 -m http.server 8000"
  echo "  2. Open http://localhost:8000 in browser"
  echo "  3. Login as alice/Pass1234 or bob/Pass5678"
else
  echo "❌ Players not joining correctly"
fi
echo "========================================="
