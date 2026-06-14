#!/bin/bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUILD_DIR="$ROOT_DIR/build"
APP_NAME="SquareGolf Connector.app"
APP_DIR="$BUILD_DIR/$APP_NAME"
TAURI_APP_DIR="$ROOT_DIR/src-tauri/target/release/bundle/macos/$APP_NAME"

if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "This script only builds the macOS Tauri app bundle." >&2
    exit 1
fi

if ! cargo tauri --version >/dev/null 2>&1; then
    echo "cargo-tauri is required. Install it with: cargo install tauri-cli --version '^2'" >&2
    exit 1
fi

rm -rf "$APP_DIR"

(cd "$ROOT_DIR/src-tauri" && cargo tauri build --bundles app)

if [[ ! -d "$TAURI_APP_DIR" ]]; then
    echo "Tauri app bundle was not produced at $TAURI_APP_DIR" >&2
    exit 1
fi

mkdir -p "$BUILD_DIR"
cp -R "$TAURI_APP_DIR" "$APP_DIR"

echo "Built $APP_DIR"
