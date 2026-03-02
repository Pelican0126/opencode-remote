#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

echo "[preflight] openclaw-json-repair-kit"

if command -v python3 >/dev/null 2>&1; then
  PYTHON_BIN="python3"
elif command -v python >/dev/null 2>&1; then
  PYTHON_BIN="python"
else
  echo "[error] Python not found. Install Python 3.10+ first."
  exit 1
fi

PY_VERSION="$($PYTHON_BIN -c 'import sys; print(f"{sys.version_info.major}.{sys.version_info.minor}.{sys.version_info.micro}")')"
echo "[ok] Python: $PYTHON_BIN ($PY_VERSION)"

if ! $PYTHON_BIN -c 'import sys; raise SystemExit(0 if sys.version_info >= (3, 10) else 1)'; then
  echo "[error] Python 3.10+ is required."
  exit 1
fi

if [ -f ".venv/bin/activate" ]; then
  echo "[ok] Virtual environment exists: .venv"
else
  echo "[warn] .venv not found"
  echo "      Next: $PYTHON_BIN -m venv .venv"
fi

if [ -f "requirements.txt" ]; then
  if $PYTHON_BIN -m pip --version >/dev/null 2>&1; then
    MISSING="$($PYTHON_BIN - <<'PY'
import importlib.util

required = ["pytest", "httpx", "respx", "dotenv"]
missing = [name for name in required if importlib.util.find_spec(name) is None]
print(",".join(missing))
PY
)"
    if [ -z "$MISSING" ]; then
      echo "[ok] Core dependencies appear installed"
    else
      echo "[warn] Missing Python packages: $MISSING"
      echo "      Next: $PYTHON_BIN -m pip install -r requirements.txt"
    fi
  else
    echo "[warn] pip is unavailable in current Python environment"
    echo "      Next: $PYTHON_BIN -m ensurepip --upgrade"
  fi
else
  echo "[warn] requirements.txt not found"
fi

if [ -f ".env" ]; then
  echo "[ok] .env exists"
elif [ -f ".env.example" ]; then
  echo "[warn] .env missing"
  echo "      Next: cp .env.example .env"
else
  echo "[warn] .env.example not found"
fi

echo
echo "[next] Suggested commands:"
echo "  1) $PYTHON_BIN -m venv .venv"
echo "  2) source .venv/bin/activate"
echo "  3) $PYTHON_BIN -m pip install -r requirements.txt"
echo "  4) cp .env.example .env"
echo "  5) $PYTHON_BIN -m kit tui"
