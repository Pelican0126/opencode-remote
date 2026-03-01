from __future__ import annotations

import json
from pathlib import Path
from typing import Any


def _minimal_from_schema(schema: dict[str, Any]) -> dict[str, Any]:
    out: dict[str, Any] = {}
    props = schema.get("properties", {})
    for key in schema.get("required", []):
        p = props.get(key, {})
        t = p.get("type")
        if t == "object":
            out[key] = {}
        elif t == "array":
            out[key] = []
        elif t in {"number", "integer"}:
            out[key] = 0
        elif t == "boolean":
            out[key] = False
        else:
            out[key] = ""
    if not out:
        out["_note"] = "schema detected; no required fields"
    return out


def generate_template(root: Path) -> dict[str, Any]:
    schema_dir = root / "schema"
    if schema_dir.exists() and schema_dir.is_dir():
        return {"_template_source": "schema/", "_note": "minimal template inferred from schema directory"}

    schema_files = sorted(root.rglob("*.schema.json"))
    if schema_files:
        schema_path = schema_files[0]
        schema = json.loads(schema_path.read_text(encoding="utf-8"))
        return {
            "_template_source": str(schema_path.relative_to(root)),
            **_minimal_from_schema(schema),
        }

    return {
        "_template_source": "skeleton-inferred",
        "name": "",
        "version": "",
        "items": [],
        "meta": {"_inferred": True},
    }
