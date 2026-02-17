#!/usr/bin/env bash
set -euo pipefail

export PATH="$HOME/.local/bin:$PATH"

bash scripts/ci_self_hosted_preflight.sh
rustup component add rustfmt clippy

if ! command -v cargo-audit >/dev/null 2>&1; then
  cargo install --locked cargo-audit
fi

if ! command -v gitleaks >/dev/null 2>&1; then
  bash scripts/install_gitleaks.sh
fi

cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p embeddb-server --features http,contract-tests
bash scripts/http_process_smoke.sh
bash scripts/http_console_smoke.sh
cargo build --workspace
cargo audit
gitleaks detect --no-banner --redact --source .

echo "Local self-hosted CI sequence passed."
