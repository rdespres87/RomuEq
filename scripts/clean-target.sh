#!/usr/bin/env bash
# scripts/clean-target.sh - Clean up old Cargo build artifacts while keeping release deliverables.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

echo "==> Cleaning cross-compilation caches..."
rm -rf "${ROOT_DIR}/target/x86_64-pc-windows-msvc" \
       "${ROOT_DIR}/target/x86_64-unknown-linux-gnu" \
       "${ROOT_DIR}/target/x86_64-apple-darwin"

if command -v cargo-sweep >/dev/null 2>&1; then
    echo "==> Running cargo-sweep to limit build artifacts..."
    cargo sweep --maxsize 3GB "${ROOT_DIR}" || true
else
    echo "==> cargo-sweep not found. Install with: cargo install cargo-sweep"
fi

echo ""
echo "==> Current target disk usage:"
du -sh "${ROOT_DIR}/target"
