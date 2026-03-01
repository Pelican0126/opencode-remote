#!/usr/bin/env bash
set -euo pipefail
export OPENCODE_CONFIG="$(pwd)/opencode.intl.json"
exec opencode
