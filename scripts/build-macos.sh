#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REQUESTED_ARCH="${1:-$(uname -m)}"

# Homebrew installs rustup as a keg-only formula, so make its proxies visible
# without requiring every developer to modify their shell profile first.
if ! command -v rustup >/dev/null 2>&1 && command -v brew >/dev/null 2>&1; then
  RUSTUP_PREFIX="$(brew --prefix rustup 2>/dev/null || true)"
  if [[ -n "$RUSTUP_PREFIX" ]]; then
    export PATH="$RUSTUP_PREFIX/bin:$PATH"
  fi
fi

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "macOS packages must be built on macOS." >&2
  exit 1
fi

case "$REQUESTED_ARCH" in
  arm64 | aarch64)
    RUST_TARGET="aarch64-apple-darwin"
    ELECTRON_ARCH="arm64"
    ;;
  x64 | x86_64)
    RUST_TARGET="x86_64-apple-darwin"
    ELECTRON_ARCH="x64"
    ;;
  *)
    echo "Unsupported architecture: $REQUESTED_ARCH (use arm64 or x64)." >&2
    exit 1
    ;;
esac

for command_name in cargo rustup node npm cmake; do
  if ! command -v "$command_name" >/dev/null 2>&1; then
    echo "Missing build dependency: $command_name" >&2
    exit 1
  fi
done

NODE_MAJOR="$(node -p 'process.versions.node.split(".")[0]')"
NODE_MINOR="$(node -p 'process.versions.node.split(".")[1]')"
if (( NODE_MAJOR < 22 || NODE_MAJOR >= 25 || (NODE_MAJOR == 22 && NODE_MINOR < 12) )); then
  echo "Frameverse requires Node.js 22.12-24; Node.js 22 LTS is recommended." >&2
  exit 1
fi

echo "=== Step 1: Build Rust sidecar for $RUST_TARGET ==="
rustup target add "$RUST_TARGET"
cd "$REPO_ROOT/rust-core"
cargo build --release --target "$RUST_TARGET"

SIDECAR_DIR="$REPO_ROOT/electron-app/build/sidecar"
mkdir -p "$SIDECAR_DIR"
cp "$REPO_ROOT/rust-core/target/$RUST_TARGET/release/rust-core" "$SIDECAR_DIR/rust-core"
chmod 755 "$SIDECAR_DIR/rust-core"

echo "=== Step 2: Install locked Node dependencies ==="
cd "$REPO_ROOT/electron-app"
npm ci

echo "=== Step 3: Package Frameverse for macOS $ELECTRON_ARCH ==="
BUILDER_ARGS=("--$ELECTRON_ARCH")
if [[ "${PICLAYOUT_NOTARIZE:-0}" == "1" ]]; then
  BUILDER_ARGS+=("-c.mac.notarize=true")
elif [[ -z "${CSC_LINK:-}" && -z "${CSC_NAME:-}" ]] \
  && ! security find-identity -v -p codesigning 2>/dev/null | grep -q "Developer ID Application"; then
  echo "No Developer ID identity found; using ad-hoc signing for local testing."
  BUILDER_ARGS+=("-c.mac.identity=-")
fi
npm run electron:build:mac -- "${BUILDER_ARGS[@]}"

echo "=== Build complete: $REPO_ROOT/dist-electron ==="
