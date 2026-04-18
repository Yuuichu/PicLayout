#!/usr/bin/env bash
set -e

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "=== 编译 Rust debug 版本 ==="
cd "$REPO_ROOT/rust-core"
cargo build

echo ""
echo "=== 启动 Electron 开发模式 ==="
cd "$REPO_ROOT/electron-app"
npm install
npm run dev
