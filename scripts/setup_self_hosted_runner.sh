#!/usr/bin/env bash
set -euo pipefail

if ! command -v rustup >/dev/null 2>&1; then
  echo "rustup is required. Install from https://rustup.rs and re-run." >&2
  exit 1
fi

rustup toolchain install stable
rustup default stable
rustup component add rustfmt clippy

if ! command -v cargo-audit >/dev/null 2>&1; then
  cargo install --locked cargo-audit
fi

bash scripts/install_gitleaks.sh
bash scripts/ci_self_hosted_preflight.sh

echo "Self-hosted runner setup completed."
