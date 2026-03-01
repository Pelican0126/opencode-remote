from __future__ import annotations

import hashlib
import json
import shutil
from datetime import datetime, timezone
from pathlib import Path

BACKUP_ROOT = ".openclaw-backups"


def _sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()


def create_backup(root: Path, files: list[Path]) -> tuple[str, Path]:
    ts = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    backup_id = f"bk-{ts}"
    base = root / BACKUP_ROOT / backup_id
    base.mkdir(parents=True, exist_ok=True)

    manifest = {"backup_id": backup_id, "timestamp": ts, "files": []}
    for f in files:
        rel = f.relative_to(root)
        dst = base / rel
        dst.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(f, dst)
        manifest["files"].append({"path": str(rel), "sha256": _sha256(f)})

    (base / "manifest.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2), encoding="utf-8")
    return backup_id, base


def rollback_latest(root: Path) -> dict:
    bdir = root / BACKUP_ROOT
    all_b = sorted([p for p in bdir.iterdir() if p.is_dir()])
    if not all_b:
        raise FileNotFoundError("no backups found")
    latest = all_b[-1]
    return rollback_by_id(root, latest.name)


def rollback_by_id(root: Path, backup_id: str) -> dict:
    base = root / BACKUP_ROOT / backup_id
    manifest = json.loads((base / "manifest.json").read_text(encoding="utf-8"))
    restored: list[str] = []
    for item in manifest.get("files", []):
        rel = Path(item["path"])
        src = base / rel
        dst = root / rel
        dst.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(src, dst)
        restored.append(str(rel))
    return {"backup_id": backup_id, "restored": restored}
