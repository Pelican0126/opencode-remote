from __future__ import annotations

import json
import re
from dataclasses import dataclass, asdict
from pathlib import Path
from typing import Any

import httpx

from ..api.env import ApiEnv
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


def _extract_json_from_text(reply: str) -> str:
    txt = reply.strip()
    if txt.startswith("```"):
        parts = txt.split("```")
        if len(parts) >= 3:
            txt = parts[1]
            if txt.startswith("json"):
                txt = txt[4:]
    return txt.strip()


def _ai_repair_text(root: Path, broken_text: str, error_text: str | None, max_rounds: int = 5) -> tuple[str, list[dict[str, Any]], str | None]:
    env = ApiEnv.load(root)
    base_url = (env.kimi_base_url() or "").rstrip("/")
    api_key = env.kimi_api_key
    model = env.kimi_model or "moonshot-v1-8k"
    endpoint = env.endpoint_path if env.endpoint_path.startswith("/") else "/chat/completions"

    if not base_url or not api_key:
        return broken_text, [], "AI repair requires KIMI_BASE_URL/KIMI_API_KEY in .env"

    url = base_url + endpoint
    history: list[dict[str, Any]] = []
    current = broken_text

    for i in range(1, max_rounds + 1):
        prompt = (
            "You are a strict JSON repair engine. Fix the JSON so it can be parsed by json.loads(). "
            "Return ONLY pure JSON text, no markdown, no explanation.\n\n"
            f"Round: {i}/{max_rounds}\n"
            f"Last parse error: {error_text or 'unknown'}\n"
            "Broken JSON:\n"
            f"{current}"
        )
        payload = {
            "model": model,
            "messages": [
                {"role": "system", "content": "Return valid JSON only."},
                {"role": "user", "content": prompt},
            ],
            "temperature": 0,
        }
        headers = {"Authorization": f"Bearer {api_key}", "Content-Type": "application/json"}

        try:
            with httpx.Client(http2=False, follow_redirects=True, trust_env=False, timeout=30.0) as client:
                resp = client.post(url, headers=headers, json=payload)
            if resp.status_code >= 400:
                return current, history, f"AI HTTP {resp.status_code}: {resp.text[:200]}"

            data = resp.json()
            content = (
                data.get("choices", [{}])[0]
                .get("message", {})
                .get("content", "")
            )
            candidate = _extract_json_from_text(content)

            try:
                json.loads(candidate)
                history.append({"name": f"ai_repair_round_{i}", "changed": candidate != current, "parse_ok_after_step": True})
                return candidate, history, None
            except Exception as e:
                history.append({"name": f"ai_repair_round_{i}", "changed": candidate != current, "parse_ok_after_step": False})
                current = candidate
                error_text = str(e)
        except Exception as e:
            return current, history, f"AI request failed: {e}"

    return current, history, f"AI repair failed after {max_rounds} rounds"


def fix_files(root: Path, files: list[Path], apply: bool, backup: bool, use_ai: bool = False) -> dict[str, Any]:
    backup_id = None
    if backup and files:
        backup_id, _ = create_backup(root, files)

    results: list[FixResult] = []
    for fp in files:
        raw = fp.read_text(encoding="utf-8", errors="replace")
        fixed, steps = _apply_steps(raw)

        if use_ai:
            ok, issue = validate_json_file(fp)
            err_text = None if ok else (issue.message if issue else "unknown parse error")
            candidate = fixed if fixed != raw else raw
            ai_fixed, ai_steps, ai_err = _ai_repair_text(root, candidate, err_text)
            steps.extend(ai_steps)
            if ai_err:
                steps.append({"name": "ai_repair_error", "changed": False, "parse_ok_after_step": False, "error": ai_err})
            fixed = ai_fixed

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
        "use_ai": use_ai,
        "results": [r.to_dict() for r in results],
    }
