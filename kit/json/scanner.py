from __future__ import annotations

import os
from pathlib import Path

EXCLUDED_DIRS = {".git", "node_modules", "dist", "build", ".venv", ".pytest_cache"}


def scan_json_files(root: Path) -> list[Path]:
    files: list[Path] = []
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d not in EXCLUDED_DIRS]
        for fn in filenames:
            if fn.lower().endswith(".json"):
                files.append(Path(dirpath) / fn)
    return sorted(files)
