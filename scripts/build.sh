#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cd "$REPO_ROOT/electron-app"
npm ci
npm run package

printf '\n构建完成，安装包位于 dist-electron/ 目录。\n'
