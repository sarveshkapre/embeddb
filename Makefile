SHELL := /bin/bash

.PHONY: setup dev test lint typecheck build check release fmt smoke-http smoke-http-console

setup:
	cargo fetch

fmt:
	cargo fmt --all

dev:
	cargo run -p embeddb-cli -- --help

test:
	cargo test --workspace

lint:
	cargo clippy --workspace --all-targets -- -D warnings

typecheck:
	cargo check --workspace

build:
	cargo build --workspace

check: fmt lint typecheck test build

smoke-http:
	bash scripts/http_process_smoke.sh

smoke-http-console:
	bash scripts/http_console_smoke.sh

release:
	cargo build --workspace --release
