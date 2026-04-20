#!/usr/bin/env bash
set -euo pipefail

echo "Building release binary..."
cargo build -p agent-cli --release

BINARY="target/release/agile-agent"
if [[ ! -f "$BINARY" ]]; then
    echo "Binary not found: $BINARY"
    exit 1
fi

SIZE=$(stat -c%s "$BINARY" 2>/dev/null || stat -f%z "$BINARY" 2>/dev/null)
MAX_SIZE=$((50 * 1024 * 1024))  # 50MB

echo "Binary size: $SIZE bytes"

if [[ "$SIZE" -gt "$MAX_SIZE" ]]; then
    echo "ERROR: Binary exceeds max size of $MAX_SIZE bytes"
    exit 1
fi

echo "Binary size OK (under $MAX_SIZE bytes)"
