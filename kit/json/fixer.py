from __future__ import annotations

import json
import re
from dataclasses import dataclass, asdict
from pathlib import Path
from typing import Any

from .backup import create_backup, rollback_by_id
from .validator import validate_json_file

_COMMENT_RE = re.compile(r"//.*?$|/\*.*?\*/", re.M | re.S)
_TRAILING_COMMA_RE = re.compile(r",\s*(\]|\})")
_SAFE_OBJ_SINGLE = re.compile(r"'([A-Za-z0-9_\- ]+)'\s*:")
_SAFE_VAL_SINGLE = re.compile(r":\s*'([^'\\\n\r]*)'")


@dataclass
class FixResult:
    file: str
    changed: bool
    success: bool
    steps: list[dict[str, Any]]
    error: str | None = None

    def to_dict(self) -> dict[str, Any]:
        return asdict(self)


def _apply_steps(text: str) -> tuple[str, list[dict[str, Any]]]:
    steps: list[dict[str, Any]] = []

    def apply(name: str, fn):
        nonlocal text
        new_text = fn(text)
        changed = new_text != text
        text = new_text
        ok = True
        try:
            json.loads(text)
        except Exception:
            ok = False
        steps.append({"name": name, "changed": changed, "parse_ok_after_step": ok})

    apply("strip_bom", lambda s: s.lstrip("\ufeff") if s.startswith("\ufeff") else s)
    apply("remove_comments", lambda s: _COMMENT_RE.sub("", s))
    apply("remove_trailing_commas", lambda s: _TRAILING_COMMA_RE.sub(r"\1", s))

    def quote_swap(s: str) -> str:
        t = _SAFE_OBJ_SINGLE.sub(r'"\1":', s)
        t = _SAFE_VAL_SINGLE.sub(lambda m: ': "' + m.group(1).replace('"', '\\"') + '"', t)
        return t

    apply("safe_single_to_double", quote_swap)
    return text, steps


def fix_files(root: Path, files: list[Path], apply: bool, backup: bool) -> dict[str, Any]:
    backup_id = None
    if backup and files:
        backup_id, _ = create_backup(root, files)

    results: list[FixResult] = []
    for fp in files:
        raw = fp.read_text(encoding="utf-8", errors="replace")
        fixed, steps = _apply_steps(raw)
        changed = fixed != raw
        success = False
        err = None
        try:
            json.loads(fixed)
            success = True
            if apply and changed:
                fp.write_text(fixed, encoding="utf-8")
                ok, issue = validate_json_file(fp)
                if not ok:
                    raise RuntimeError(f"post-write parse failed: {issue.message if issue else 'unknown'}")
        except Exception as e:
            err = str(e)
        results.append(FixResult(str(fp), changed, success, steps, err))

    if apply and any((not r.success) for r in results) and backup_id:
        rollback_by_id(root, backup_id)

    return {
        "backup_id": backup_id,
        "applied": apply,
        "results": [r.to_dict() for r in results],
    }
