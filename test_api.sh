#!/usr/bin/env bash

# ============================================================
# API cURL Test Script for ona_rust
# ============================================================

BASE_URL="https://3000--019d7c59-bb2a-7a26-b0be-8b675bf4584a.us-east-1-01.gitpod.dev"

PASS_COUNT=0
FAIL_COUNT=0

TEST_USERNAME="test_user_$(date +%s)"
TEST_PASSWORD="test_pass_$(date +%s)"

TOKEN=""
TODO_ID=""
CUSTOM_CATEGORY="temp_cat_$(date +%s)"
RENAMED_CATEGORY="${CUSTOM_CATEGORY}_renamed"

check() {
  local test_name="$1"
  local expected_status="$2"
  local actual_status="$3"
  local body="$4"

  local passed=0
  IFS='|' read -r -a expected_list <<< "$expected_status"
  for status in "${expected_list[@]}"; do
    if [[ "$actual_status" == "$status" ]]; then
      passed=1
      break
    fi
  done

  echo "----------------------------------------"
  echo "TEST: ${test_name}"
  echo "EXPECTED HTTP STATUS: ${expected_status}"
  echo "ACTUAL HTTP STATUS:   ${actual_status}"
  echo "RESPONSE BODY: ${body}"

  if [[ "$passed" -eq 1 ]]; then
    echo "RESULT: PASS"
    PASS_COUNT=$((PASS_COUNT + 1))
  else
    echo "RESULT: FAIL"
    FAIL_COUNT=$((FAIL_COUNT + 1))
  fi
}

extract_token() {
  local body="$1"
  local token_value=""

  if command -v jq >/dev/null 2>&1; then
    token_value="$(echo "$body" | jq -r '.token // empty' 2>/dev/null)"
  fi

  if [[ -z "$token_value" ]]; then
    token_value="$(echo "$body" | grep -o '"token"[[:space:]]*:[[:space:]]*"[^"]*"' | sed 's/.*"token"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/' | head -n1)"
  fi

  echo "$token_value"
}

request() {
  local method="$1"
  local path="$2"
  local data="${3:-}"
  local auth_token="${4:-}"
  local _out_status="${5:-HTTP_STATUS}"
  local _out_body="${6:-RESPONSE_BODY}"

  local url="${BASE_URL}${path}"
  local response

  if [[ -n "$auth_token" && -n "$data" ]]; then
    response="$(curl -sS -X "$method" "$url" -H "Authorization: Bearer ${auth_token}" -H "Content-Type: application/json" -d "$data" -w $'\n%{http_code}' 2>&1)"
  elif [[ -n "$auth_token" ]]; then
    response="$(curl -sS -X "$method" "$url" -H "Authorization: Bearer ${auth_token}" -w $'\n%{http_code}' 2>&1)"
  elif [[ -n "$data" ]]; then
    response="$(curl -sS -X "$method" "$url" -H "Content-Type: application/json" -d "$data" -w $'\n%{http_code}' 2>&1)"
  else
    response="$(curl -sS -X "$method" "$url" -w $'\n%{http_code}' 2>&1)"
  fi

  printf -v "$_out_status" '%s' "$(echo "$response" | tail -n1)"
  printf -v "$_out_body"   '%s' "$(echo "$response" | sed '$d')"
}

echo "BASE_URL=${BASE_URL}"
echo "TEST_USERNAME=${TEST_USERNAME}"
echo

# ============================================================
# Error Case: Unauthorized access to Todo endpoint
# ============================================================
request "GET" "/todos?page=1"
check "Unauthorized TODO list access" 401 "$HTTP_STATUS" "$RESPONSE_BODY"

# ============================================================
# Auth: Register
# ============================================================
REGISTER_PAYLOAD="{\"username\":\"${TEST_USERNAME}\",\"password\":\"${TEST_PASSWORD}\"}"
request "POST" "/auth/register" "$REGISTER_PAYLOAD"
check "Register user" 200 "$HTTP_STATUS" "$RESPONSE_BODY"
TOKEN="$(extract_token "$RESPONSE_BODY")"
echo "Extracted TOKEN: ${TOKEN}"
echo

# ============================================================
# Error Case: Duplicate user registration
# ============================================================
request "POST" "/auth/register" "$REGISTER_PAYLOAD"
check "Duplicate register" "400|409" "$HTTP_STATUS" "$RESPONSE_BODY"

# ============================================================
# Auth: Login
# ============================================================
request "POST" "/auth/login" "$REGISTER_PAYLOAD"
check "Login user" 200 "$HTTP_STATUS" "$RESPONSE_BODY"

# ============================================================
# Categories: GET
# ============================================================
request "GET" "/categories" "" "$TOKEN"
check "Get categories" 200 "$HTTP_STATUS" "$RESPONSE_BODY"

# ============================================================
# Categories: POST custom category
# ============================================================
CATEGORY_PAYLOAD="{\"name\":\"${CUSTOM_CATEGORY}\"}"
request "POST" "/categories" "$CATEGORY_PAYLOAD" "$TOKEN"
check "Create custom category" 201 "$HTTP_STATUS" "$RESPONSE_BODY"

# ============================================================
# Categories: PUT rename category
# ============================================================
RENAME_PAYLOAD="{\"new_name\":\"${RENAMED_CATEGORY}\"}"
request "PUT" "/categories/${CUSTOM_CATEGORY}" "$RENAME_PAYLOAD" "$TOKEN"
check "Rename custom category" 200 "$HTTP_STATUS" "$RESPONSE_BODY"

# ============================================================
# Error Case: Delete built-in category (work)
# ============================================================
request "DELETE" "/categories/work" "" "$TOKEN"
check "Delete built-in category work" "400|403" "$HTTP_STATUS" "$RESPONSE_BODY"

# ============================================================
# Todos: GET page=1
# ============================================================
request "GET" "/todos?page=1" "" "$TOKEN"
check "Get todos page 1" 200 "$HTTP_STATUS" "$RESPONSE_BODY"

# ============================================================
# Todos: POST create todo
# ============================================================
TODO_PAYLOAD="{\"text\":\"Buy milk from API test\",\"category\":\"${RENAMED_CATEGORY}\"}"
request "POST" "/todos" "$TODO_PAYLOAD" "$TOKEN"
check "Create todo" 201 "$HTTP_STATUS" "$RESPONSE_BODY"

if command -v jq >/dev/null 2>&1; then
  TODO_ID="$(echo "$RESPONSE_BODY" | jq -r '.id // empty' 2>/dev/null)"
fi
if [[ -z "$TODO_ID" ]]; then
  TODO_ID="$(echo "$RESPONSE_BODY" | grep -o '"id"[[:space:]]*:[[:space:]]*[0-9]\+' | sed 's/.*:[[:space:]]*\([0-9]\+\)/\1/' | head -n1)"
fi
echo "Extracted TODO_ID: ${TODO_ID}"
echo

# ============================================================
# Todos: PUT update todo text
# ============================================================
if [[ -n "$TODO_ID" ]]; then
  UPDATE_TODO_PAYLOAD='{"text":"Buy oat milk from API test"}'
  request "PUT" "/todos/${TODO_ID}" "$UPDATE_TODO_PAYLOAD" "$TOKEN"
  check "Update todo text" 200 "$HTTP_STATUS" "$RESPONSE_BODY"

  # ============================================================
  # Todos: PATCH mark todo done
  # ============================================================
  request "PATCH" "/todos/${TODO_ID}/done" "" "$TOKEN"
  check "Mark todo done" 200 "$HTTP_STATUS" "$RESPONSE_BODY"

  # ============================================================
  # Error Case: PATCH done on already-done todo
  # ============================================================
  request "PATCH" "/todos/${TODO_ID}/done" "" "$TOKEN"
  check "Mark already-done todo done" 400 "$HTTP_STATUS" "$RESPONSE_BODY"

  # ============================================================
  # Todos: DELETE todo
  # ============================================================
  request "DELETE" "/todos/${TODO_ID}" "" "$TOKEN"
  check "Delete todo" 204 "$HTTP_STATUS" "$RESPONSE_BODY"
else
  check "Update todo text (skipped: missing ID)" 200 "N/A" "$RESPONSE_BODY"
  check "Mark todo done (skipped: missing ID)" 200 "N/A" "$RESPONSE_BODY"
  check "Delete todo (skipped: missing ID)" 204 "N/A" "$RESPONSE_BODY"
fi

# ============================================================
# Error Case: Delete non-existent Todo ID
# ============================================================
request "DELETE" "/todos/99999999" "" "$TOKEN"
check "Delete non-existent todo" 404 "$HTTP_STATUS" "$RESPONSE_BODY"

# ============================================================
# Categories: DELETE custom category (renamed)
# ============================================================
request "DELETE" "/categories/${RENAMED_CATEGORY}" "" "$TOKEN"
check "Delete renamed custom category" 204 "$HTTP_STATUS" "$RESPONSE_BODY"

# ============================================================
# Auth: Logout
# ============================================================
request "POST" "/auth/logout" "" "$TOKEN"
check "Logout user" 204 "$HTTP_STATUS" "$RESPONSE_BODY"

# ============================================================
# Error Case: Revoked token is rejected after logout
# ============================================================
request "GET" "/todos?page=1" "" "$TOKEN"
check "Access with revoked token after logout" 401 "$HTTP_STATUS" "$RESPONSE_BODY"

# ============================================================
# Summary
# ============================================================
TOTAL_COUNT=$((PASS_COUNT + FAIL_COUNT))
echo "========================================"
echo "TEST SUMMARY"
echo "TOTAL: ${TOTAL_COUNT}"
echo "PASS : ${PASS_COUNT}"
echo "FAIL : ${FAIL_COUNT}"
echo "========================================"

if [[ "$FAIL_COUNT" -gt 0 ]]; then
  exit 1
else
  exit 0
fi
