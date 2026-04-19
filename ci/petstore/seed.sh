#!/usr/bin/env bash
set -euo pipefail

PROXY="http://localhost:8081"
# Use Docker service name for Petstore when going through the mitmproxy proxy.
# The proxy runs inside the Docker network where 'petstore' resolves correctly.
# 'localhost:8080' would resolve to the proxy's own container, not Petstore.
BASE="http://petstore:8080/api/v3"

echo "=== Seeding Petstore via mitmproxy ==="

# Smoke check: verify H2 persistence (should have data if volume mounted correctly)
echo "--- Smoke check ---"
curl -sf --proxy "$PROXY" "$BASE/openapi.json" > /dev/null
echo "OpenAPI endpoint reachable"

# Pet endpoints
echo "--- Pet endpoints ---"
curl -sf --proxy "$PROXY" -X POST "$BASE/pet" \
  -H "Content-Type: application/json" \
  -d '{"id":1,"name":"doggie","category":{"id":1,"name":"Dogs"},"photoUrls":["url1"],"tags":[{"id":1,"name":"tag1"}],"status":"available"}' > /dev/null

curl -sf --proxy "$PROXY" -X POST "$BASE/pet" \
  -H "Content-Type: application/json" \
  -d '{"id":2,"name":"kitty","category":{"id":2,"name":"Cats"},"photoUrls":["url2"],"tags":[{"id":2,"name":"tag2"}],"status":"pending"}' > /dev/null

curl -sf --proxy "$PROXY" -X POST "$BASE/pet" \
  -H "Content-Type: application/json" \
  -d '{"id":3,"name":"birdie","category":{"id":3,"name":"Birds"},"photoUrls":["url3"],"tags":[{"id":3,"name":"tag3"}],"status":"sold"}' > /dev/null

curl -sf --proxy "$PROXY" -X PUT "$BASE/pet" \
  -H "Content-Type: application/json" \
  -d '{"id":1,"name":"doggie-updated","category":{"id":1,"name":"Dogs"},"photoUrls":["url1"],"tags":[{"id":1,"name":"tag1"}],"status":"available"}' > /dev/null

curl -sf --proxy "$PROXY" "$BASE/pet/1" > /dev/null
curl -sf --proxy "$PROXY" "$BASE/pet/findByStatus?status=available" > /dev/null
curl -sf --proxy "$PROXY" "$BASE/pet/findByStatus?status=pending" > /dev/null
curl -sf --proxy "$PROXY" "$BASE/pet/findByStatus?status=sold" > /dev/null
curl -sf --proxy "$PROXY" -X DELETE "$BASE/pet/3" > /dev/null

# Store endpoints
echo "--- Store endpoints ---"
curl -sf --proxy "$PROXY" "$BASE/store/inventory" > /dev/null

curl -sf --proxy "$PROXY" -X POST "$BASE/store/order" \
  -H "Content-Type: application/json" \
  -d '{"id":1,"petId":1,"quantity":1,"shipDate":"2026-04-17T00:00:00.000Z","status":"placed","complete":false}' > /dev/null

curl -sf --proxy "$PROXY" "$BASE/store/order/1" > /dev/null
curl -sf --proxy "$PROXY" -X DELETE "$BASE/store/order/1" > /dev/null

# User endpoints
echo "--- User endpoints ---"
curl -sf --proxy "$PROXY" -X POST "$BASE/user" \
  -H "Content-Type: application/json" \
  -d '{"id":1,"username":"testuser","firstName":"Test","lastName":"User","email":"test@example.com","password":"password123","phone":"555-1234","userStatus":1}' > /dev/null

curl -sf --proxy "$PROXY" "$BASE/user/testuser" > /dev/null
curl -sf --proxy "$PROXY" "$BASE/user/login?username=testuser&password=password123" > /dev/null
curl -sf --proxy "$PROXY" -X DELETE "$BASE/user/testuser" > /dev/null

echo "=== Seed complete ==="
