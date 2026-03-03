#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ENV_FILE="$ROOT_DIR/runtime.env"

if [[ ! -f "$ENV_FILE" ]]; then
  printf "[ERROR] runtime.env not found. Run ./install.sh first.\n" >&2
  exit 1
fi

set -a
# shellcheck disable=SC1090
source "$ENV_FILE"
set +a

exec cargo run --release --manifest-path "$ROOT_DIR/rust-bot/Cargo.toml"
