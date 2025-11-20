#!/bin/bash
# Test script for HTTP join fix

echo "=== Testing WebSocket Join Fix ==="
echo

# Register/login alice
echo "1. Registering alice..."
curl -s -X POST http://localhost:8080/api/auth/register \
  -H "Content-Type: application/json" \
  -d '{"username":"alice_test","password":"Pass1234","display_name":"Alice"}' | jq .

echo
echo "2. Logging in as alice_test..."
LOGIN_RESPONSE=$(curl -s -X POST http://localhost:8080/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"alice_test","password":"Pass1234"}')

echo "$LOGIN_RESPONSE" | jq .

TOKEN=$(echo "$LOGIN_RESPONSE" | jq -r '.access_token')
echo "Token: ${TOKEN:0:50}..."

echo
echo "3. Listing tables..."
curl -s http://localhost:8080/api/tables | jq .

echo
echo "4. Joining table 1 via HTTP API..."
curl -s -X POST http://localhost:8080/api/tables/1/join \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"buy_in_amount":1000}' | jq .

echo
echo "5. Getting table state..."
curl -s http://localhost:8080/api/tables/1 \
  -H "Authorization: Bearer $TOKEN" | jq .

echo
echo "=== Test complete ==="
echo "If you see player in the table state, the HTTP join is working!"
echo "Now you can connect via WebSocket and should receive game updates."
