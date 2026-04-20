#!/usr/bin/env bash
set -euo pipefail

echo "Checking agent-tui compiles without default features (no core dependency)..."
cargo check -p agent-tui --no-default-features
echo "OK: agent-tui compiles without core dependency"
