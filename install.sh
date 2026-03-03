#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ENV_FILE="$ROOT_DIR/runtime.env"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    printf "[ERROR] Missing required command: %s\n" "$1" >&2
    exit 1
  fi
}

prompt_default() {
  local prompt="$1"
  local def="$2"
  local value
  read -r -p "$prompt [$def]: " value
  if [[ -z "$value" ]]; then
    printf "%s" "$def"
  else
    printf "%s" "$value"
  fi
}

printf "== TG Opencode Remote Installer ==\n"
require_cmd opencode
require_cmd cargo

read -r -p "Telegram BOT_TOKEN: " BOT_TOKEN
if [[ -z "$BOT_TOKEN" ]]; then
  printf "[ERROR] BOT_TOKEN is required.\n" >&2
  exit 1
fi

WORKSPACE_ROOT="$(prompt_default "WORKSPACE_ROOT" "workspace")"
TASK_HISTORY_FILE="$(prompt_default "TASK_HISTORY_FILE" "data/task-history.json")"
DEFAULT_MODEL="$(prompt_default "DEFAULT_MODEL" "gpt-5.3-codex")"
FALLBACK_MODEL="$(prompt_default "FALLBACK_MODEL" "GLM-5")"
DEFAULT_AGENT="$(prompt_default "DEFAULT_AGENT" "build")"
DEFAULT_PROJECT_NAME="$(prompt_default "DEFAULT_PROJECT_NAME" "main")"

cat >"$ENV_FILE" <<EOF
BOT_TOKEN=$BOT_TOKEN
WORKSPACE_ROOT=$WORKSPACE_ROOT
TASK_HISTORY_FILE=$TASK_HISTORY_FILE
DEFAULT_MODEL=$DEFAULT_MODEL
FALLBACK_MODEL=$FALLBACK_MODEL
DEFAULT_AGENT=$DEFAULT_AGENT
DEFAULT_THINKING=
DEFAULT_PROJECT_NAME=$DEFAULT_PROJECT_NAME
OPENCODE_BIN=opencode
OPENCODE_EXTRA_ARGS=
MAX_OUTPUT_CHARS=0
EOF

chmod 600 "$ENV_FILE"

printf "\n[1/2] Building rust-bot...\n"
cargo build --release --manifest-path "$ROOT_DIR/rust-bot/Cargo.toml"

printf "[2/2] Done. Starting bot now...\n\n"
exec "$ROOT_DIR/start.sh"
