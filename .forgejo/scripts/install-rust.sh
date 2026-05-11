#!/bin/env bash
set -euxo pipefail

# Usage: install-rust.sh [toolchain] [COMPONENT1] [COMPONENT2] ...
# Default toolchain is stable
TOOLCHAIN="${1:-stable}"
shift || true

# Install rustup if needed
if ! command -v rustup >/dev/null 2>&1; then
  curl -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain "$TOOLCHAIN"
fi

. "$HOME/.cargo/env"

# rustup override unset may fail if no override is set, so || true prevents exit
if [[ "$TOOLCHAIN" == "stable" ]]; then
  rustup override unset || true
  rustup default stable
fi

rustup toolchain install "$TOOLCHAIN" --profile minimal

# Install any additional components
for component in "$@"; do
    rustup component add "$component" --toolchain "$TOOLCHAIN"
done

rustc "+$TOOLCHAIN" -V
cargo "+$TOOLCHAIN" -V
