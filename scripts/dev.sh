#!/usr/bin/env bash
set -e

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cd "$REPO_ROOT/rust-core"
if [[ "${PICLAYOUT_RUST_PROFILE:-release}" == "debug" ]]; then
  echo "=== Building Rust debug sidecar ==="
  cargo build
else
  echo "=== Building Rust release sidecar ==="
  cargo build --release
fi

echo ""
echo "=== Starting Electron dev mode ==="
cd "$REPO_ROOT/electron-app"
npm install
npm run dev
