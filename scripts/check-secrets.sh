#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

PATTERN='([0-9]{7,}:[A-Za-z0-9_-]{20,}|sk-[A-Za-z0-9_-]{12,}|xoxb-[A-Za-z0-9-]{10,}|ghp_[A-Za-z0-9]{20,})'

if command -v rg >/dev/null 2>&1; then
  if rg -n -S "$PATTERN" \
    --glob '!.git/**' \
    --glob '!.opencode-runtime/**' \
    --glob '!rust-bot/target/**' \
    --glob '!node_modules/**' \
    --glob '!.env.example' \
    .; then
    printf "\n[ERROR] Potential secrets detected.\n" >&2
    exit 1
  fi
else
  if grep -RInE "$PATTERN" . \
    --exclude-dir=.git \
    --exclude-dir=.opencode-runtime \
    --exclude-dir=node_modules \
    --exclude-dir=target \
    --exclude=.env.example; then
    printf "\n[ERROR] Potential secrets detected.\n" >&2
    exit 1
  fi
fi

printf "No obvious secrets found.\n"
