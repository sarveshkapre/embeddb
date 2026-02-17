#!/usr/bin/env bash
set -euo pipefail

PORT=$((18580 + RANDOM % 500))
ADDR="127.0.0.1:${PORT}"
BASE_URL="http://${ADDR}"
TMP_DIR="$(mktemp -d)"
LOG_FILE="${TMP_DIR}/server.log"
PID=""

cleanup() {
  if [[ -n "${PID}" ]] && kill -0 "${PID}" 2>/dev/null; then
    kill "${PID}" 2>/dev/null || true
    wait "${PID}" 2>/dev/null || true
  fi
  rm -rf "${TMP_DIR}"
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

INDEX="$(curl --silent --show-error --fail "${BASE_URL}/")"
echo "${INDEX}" | grep -q "EmbedDB Console"
echo "${INDEX}" | grep -q "Create demo dataset"

APP_JS="$(curl --silent --show-error --fail "${BASE_URL}/assets/app.js")"
echo "${APP_JS}" | grep -q "seedDemo"

echo "${APP_JS}" | grep -q "snapshot/restore"

STYLES="$(curl --silent --show-error --fail "${BASE_URL}/assets/styles.css")"
echo "${STYLES}" | grep -q ".hero"

echo "${STYLES}" | grep -q ".modal"

curl --silent --show-error --fail \
  -H "content-type: application/json" \
  -d '{"name":"notes","schema":{"columns":[{"name":"title","data_type":"String","nullable":false}]},"embedding_fields":["title"]}' \
  "${BASE_URL}/tables" >/dev/null

SNAP_EXPORT_DIR="${TMP_DIR}/snapshot"
SNAP_EXPORT="$(curl --silent --show-error --fail \
  -H "content-type: application/json" \
  -d "{\"dest_dir\":\"${SNAP_EXPORT_DIR}\"}" \
  "${BASE_URL}/snapshot/export")"
echo "${SNAP_EXPORT}" | grep -q '"files_copied"'

RESTORE_DIR="${TMP_DIR}/restored"
SNAP_RESTORE="$(curl --silent --show-error --fail \
  -H "content-type: application/json" \
  -d "{\"snapshot_dir\":\"${SNAP_EXPORT_DIR}\",\"data_dir\":\"${RESTORE_DIR}\"}" \
  "${BASE_URL}/snapshot/restore")"
echo "${SNAP_RESTORE}" | grep -q '"bytes_copied"'

if [[ ! -f "${RESTORE_DIR}/wal.log" ]]; then
  echo "Restored snapshot is missing wal.log"
  exit 1
fi
