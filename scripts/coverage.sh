#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
coverage_dir="${repo_root}/target/coverage"
lcov_path="${coverage_dir}/lcov.info"

if ! command -v cargo >/dev/null 2>&1; then
  echo "error: cargo is required but was not found on PATH." >&2
  exit 1
fi

if ! cargo llvm-cov --version >/dev/null 2>&1; then
  cat >&2 <<'EOF'
error: cargo-llvm-cov is required.
install it with: cargo install cargo-llvm-cov
EOF
  exit 1
fi

if ! command -v rustup >/dev/null 2>&1; then
  cat >&2 <<'EOF'
error: rustup is required to verify llvm-tools-preview.
install it with: https://rustup.rs/
EOF
  exit 1
fi

if ! rustup component list --installed 2>/dev/null | grep -q '^llvm-tools'; then
  cat >&2 <<'EOF'
error: llvm-tools-preview is required.
install it with: rustup component add llvm-tools-preview
EOF
  exit 1
fi

mkdir -p "${coverage_dir}"

(
  cd "${repo_root}"
  cargo llvm-cov --workspace --all-features
  cargo llvm-cov report --lcov --output-path target/coverage/lcov.info
)

echo "LCOV report written to ${lcov_path}"
