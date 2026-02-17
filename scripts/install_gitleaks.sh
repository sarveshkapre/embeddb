#!/usr/bin/env bash
set -euo pipefail

version="${GITLEAKS_VERSION:-8.30.0}"
os="$(uname -s | tr '[:upper:]' '[:lower:]')"
arch="$(uname -m)"

case "${os}" in
  linux) os_tag="linux" ;;
  darwin) os_tag="darwin" ;;
  *)
    echo "Unsupported OS for gitleaks install: ${os}" >&2
    exit 1
    ;;
esac

case "${arch}" in
  x86_64|amd64) arch_tag="x64" ;;
  arm64|aarch64) arch_tag="arm64" ;;
  *)
    echo "Unsupported architecture for gitleaks install: ${arch}" >&2
    exit 1
    ;;
esac

if command -v gitleaks >/dev/null 2>&1; then
  installed="$(gitleaks version 2>/dev/null || true)"
  if echo "${installed}" | grep -q "${version}"; then
    echo "gitleaks ${version} already installed"
    exit 0
  fi
fi

asset="gitleaks_${version}_${os_tag}_${arch_tag}.tar.gz"
url="https://github.com/gitleaks/gitleaks/releases/download/v${version}/${asset}"

workdir="$(mktemp -d)"
trap 'rm -rf "${workdir}"' EXIT

mkdir -p "${HOME}/.local/bin"

echo "Installing gitleaks v${version} from ${url}"
curl -fsSL "${url}" -o "${workdir}/gitleaks.tar.gz"
tar -xzf "${workdir}/gitleaks.tar.gz" -C "${workdir}"
install -m 0755 "${workdir}/gitleaks" "${HOME}/.local/bin/gitleaks"

echo "Installed: $(${HOME}/.local/bin/gitleaks version | head -n1)"
