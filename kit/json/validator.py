from __future__ import annotations

import json
from dataclasses import dataclass, asdict
from pathlib import Path
from typing import Any


@dataclass
class JsonIssue:
    file: str
    line: int
    col: int
    message: str
    context: str

    def to_dict(self) -> dict[str, Any]:
        return asdict(self)


def validate_json_file(path: Path) -> tuple[bool, JsonIssue | None]:
    text = path.read_text(encoding="utf-8", errors="replace")
    try:
        json.loads(text)
        return True, None
    except json.JSONDecodeError as e:
        lines = text.splitlines()
        lo = max(1, e.lineno - 2)
        hi = min(len(lines), e.lineno + 2)
        context = "\n".join(f"{i:>5}: {lines[i-1]}" for i in range(lo, hi + 1)) if lines else ""
        return False, JsonIssue(
            file=str(path),
            line=e.lineno,
            col=e.colno,
            message=e.msg,
            context=context,
        )
