#!/usr/bin/env bash
set -euo pipefail

required_cmds=(bash curl git grep sed awk tar uname)
for cmd in "${required_cmds[@]}"; do
  if ! command -v "${cmd}" >/dev/null 2>&1; then
    echo "Missing required command: ${cmd}" >&2
    exit 1
  fi
done

if ! command -v cargo >/dev/null 2>&1; then
  echo "Missing cargo. Install Rust stable via rustup before running CI." >&2
  exit 1
fi

if ! command -v rustc >/dev/null 2>&1; then
  echo "Missing rustc. Install Rust stable via rustup before running CI." >&2
  exit 1
fi

if ! command -v rustup >/dev/null 2>&1; then
  echo "Missing rustup. Install rustup so CI can verify rustfmt/clippy components." >&2
  exit 1
fi

echo "Self-hosted runner preflight passed"
echo "OS: $(uname -s)"
echo "Arch: $(uname -m)"
echo "cargo: $(cargo --version)"
echo "rustc: $(rustc --version)"
