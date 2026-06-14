#!/bin/bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUILD_DIR="$ROOT_DIR/build"
APP_NAME="SquareGolf Connector"
APP_DIR="$BUILD_DIR/$APP_NAME"
EXE_NAME="SquareGolf Connector.exe"

case "$(uname -s)" in
    MINGW*|MSYS*|CYGWIN*) ;;
    *)
        echo "This script must be run on Windows with a Rust/Tauri toolchain." >&2
        exit 1
        ;;
esac

rm -rf "$APP_DIR"
mkdir -p "$APP_DIR"

cargo build \
    --manifest-path "$ROOT_DIR/src-tauri/Cargo.toml" \
    --release \
    --bin squaregolf-connector

SOURCE_EXE="$ROOT_DIR/src-tauri/target/release/squaregolf-connector.exe"
if [[ ! -f "$SOURCE_EXE" ]]; then
    echo "Rust app executable was not produced at $SOURCE_EXE" >&2
    exit 1
fi

cp "$SOURCE_EXE" "$APP_DIR/$EXE_NAME"

echo "Built $APP_DIR/$EXE_NAME"
