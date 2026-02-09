#!/usr/bin/env bash
set -euo pipefail

PORT=$((18080 + RANDOM % 500))
ADDR="127.0.0.1:${PORT}"
BASE_URL="http://${ADDR}"
if [[ -n "${EMBEDDB_SMOKE_DIR:-}" ]]; then
  TMP_DIR="${EMBEDDB_SMOKE_DIR}"
  mkdir -p "${TMP_DIR}"
  PRESERVE_TMP_DIR=1
else
  TMP_DIR="$(mktemp -d)"
  PRESERVE_TMP_DIR=0
fi
LOG_FILE="${TMP_DIR}/server.log"
PID=""

cleanup() {
  if [[ -n "${PID}" ]] && kill -0 "${PID}" 2>/dev/null; then
    kill "${PID}" 2>/dev/null || true
    wait "${PID}" 2>/dev/null || true
  fi
  if [[ "${PRESERVE_TMP_DIR}" -eq 0 ]]; then
    rm -rf "${TMP_DIR}"
  fi
}
trap cleanup EXIT

export EMBEDDB_ADDR="${ADDR}"
export EMBEDDB_DATA_DIR="${TMP_DIR}/data"

cargo run -p embeddb-server --features http >"${LOG_FILE}" 2>&1 &
PID=$!

for _ in $(seq 1 80); do
  if curl --silent --show-error --fail "${BASE_URL}/health" >/dev/null 2>&1; then
    break
  fi
  sleep 0.25
done

if ! curl --silent --show-error --fail "${BASE_URL}/health" >/dev/null 2>&1; then
  echo "HTTP server did not become ready"
  tail -n 120 "${LOG_FILE}" || true
  exit 1
fi

curl --silent --show-error --fail \
  -H "content-type: application/json" \
  -d '{
    "name":"notes",
    "schema":{
      "columns":[
        {"name":"title","data_type":"String","nullable":false},
        {"name":"body","data_type":"String","nullable":false}
      ]
    },
    "embedding_fields":["title","body"]
  }' \
  "${BASE_URL}/tables" >/dev/null

curl --silent --show-error --fail \
  -H "content-type: application/json" \
  -d '{"fields":{"title":"Hello","body":"World"}}' \
  "${BASE_URL}/tables/notes/rows" >/dev/null

curl --silent --show-error --fail -X POST "${BASE_URL}/tables/notes/jobs/process" >/dev/null

curl --silent --show-error --fail \
  -H "content-type: application/json" \
  -d '{"query_text":"Hello\nWorld","k":1}' \
  "${BASE_URL}/tables/notes/search-text" >/dev/null

curl --silent --show-error --fail -X POST "${BASE_URL}/tables/notes/flush" >/dev/null
curl --silent --show-error --fail -X POST "${BASE_URL}/tables/notes/compact" >/dev/null

ROW="$(curl --silent --show-error --fail "${BASE_URL}/tables/notes/rows/1")"
echo "${ROW}" | grep -q '"title":"Hello"'
