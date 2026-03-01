from __future__ import annotations

import json
from pathlib import Path
from typing import Any


def write_reports(root: Path, payload: dict[str, Any], stem: str) -> tuple[Path, Path]:
    out = root / "reports"
    out.mkdir(parents=True, exist_ok=True)
    text_path = out / f"{stem}.txt"
    json_path = out / f"{stem}.json"

    text_lines = [f"report: {stem}"]
    if "issues" in payload:
        text_lines.append(f"issues: {len(payload['issues'])}")
    if "results" in payload:
        text_lines.append(f"results: {len(payload['results'])}")
    text_lines.append("")
    text_lines.append(json.dumps(payload, ensure_ascii=False, indent=2))

    text_path.write_text("\n".join(text_lines), encoding="utf-8")
    json_path.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
    return text_path, json_path
