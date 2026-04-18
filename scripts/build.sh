#!/usr/bin/env bash
set -e

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "=== Step 1: 编译 Rust release 版本 ==="
cd "$REPO_ROOT/rust-core"
cargo build --release
echo "rust-core.exe 编译完成: target/release/rust-core.exe"

echo ""
echo "=== Step 2: 安装 Node 依赖 ==="
cd "$REPO_ROOT/electron-app"
npm install

echo ""
echo "=== Step 3: 打包 Electron（含 Rust 可执行文件）==="
npm run electron:build

echo ""
echo "=== 构建完成！安装包位于 dist-electron/ 目录 ==="
