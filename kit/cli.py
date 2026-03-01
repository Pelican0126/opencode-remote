from __future__ import annotations

import argparse
import json
from datetime import datetime
from pathlib import Path

from .json.scanner import scan_json_files
from .json.validator import validate_json_file
from .json.templater import generate_template
from .json.fixer import fix_files
from .json.backup import rollback_latest
from .json.report import write_reports
from .api.env import ApiEnv
from .api.validator import validate_env
from .api.tester import run_connectivity_test


def cmd_scan(root: Path) -> int:
    issues = []
    for fp in scan_json_files(root):
        ok, issue = validate_json_file(fp)
        if not ok and issue:
            issues.append(issue.to_dict())
    payload = {"issues": issues}
    stem = f"scan-{datetime.utcnow().strftime('%Y%m%d-%H%M%S')}"
    txt, js = write_reports(root, payload, stem)
    print(f"scan complete, issues={len(issues)}")
    print(f"reports: {txt} | {js}")
    if issues:
        print(json.dumps(issues, ensure_ascii=False, indent=2))
        return 2
    return 0


def cmd_template(root: Path) -> int:
    tpl = generate_template(root)
    print(json.dumps(tpl, ensure_ascii=False, indent=2))
    return 0


def cmd_fix(root: Path, apply: bool, backup: bool) -> int:
    files = scan_json_files(root)
    result = fix_files(root, files, apply=apply, backup=backup)
    stem = f"fix-{datetime.utcnow().strftime('%Y%m%d-%H%M%S')}"
    txt, js = write_reports(root, result, stem)
    print(f"fix complete, applied={apply}, backup_id={result.get('backup_id')}")
    print(f"reports: {txt} | {js}")
    failed = [r for r in result["results"] if not r["success"]]
    return 1 if failed else 0


def cmd_rollback(root: Path, latest: bool) -> int:
    if not latest:
        raise SystemExit("only --latest supported")
    payload = rollback_latest(root)
    print(json.dumps(payload, ensure_ascii=False, indent=2))
    return 0


def cmd_api_init(root: Path) -> int:
    env_example = root / ".env.example"
    if env_example.exists():
        print(".env.example already exists")
    else:
        raise SystemExit(".env.example missing in project root")
    print("next: copy .env.example to .env and fill API keys")
    return 0


def cmd_api_validate(root: Path) -> int:
    env = ApiEnv.load(root)
    ok, issues = validate_env(env)
    if ok:
        print("api env valid")
        return 0
    print("api env invalid:")
    print(json.dumps([i.to_dict() for i in issues], ensure_ascii=False, indent=2))
    return 2


def cmd_api_test(root: Path) -> int:
    env = ApiEnv.load(root)
    payload = run_connectivity_test(env)
    print(json.dumps(payload, ensure_ascii=False, indent=2))
    has_err = any((r.get("error") is not None or (r.get("http_status") or 0) >= 400) for r in payload["results"])
    return 1 if has_err else 0


def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(prog="kit")
    sub = p.add_subparsers(dest="cmd", required=True)

    sub.add_parser("scan")
    sub.add_parser("template")

    p_fix = sub.add_parser("fix")
    p_fix.add_argument("--apply", action="store_true")
    p_fix.add_argument("--backup", action="store_true")

    p_rb = sub.add_parser("rollback")
    p_rb.add_argument("--latest", action="store_true")

    p_api = sub.add_parser("api")
    api_sub = p_api.add_subparsers(dest="api_cmd", required=True)
    api_sub.add_parser("init")
    api_sub.add_parser("validate")
    api_sub.add_parser("test")

    return p


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    root = Path.cwd()

    if args.cmd == "scan":
        return cmd_scan(root)
    if args.cmd == "template":
        return cmd_template(root)
    if args.cmd == "fix":
        return cmd_fix(root, apply=args.apply, backup=args.backup)
    if args.cmd == "rollback":
        return cmd_rollback(root, latest=args.latest)
    if args.cmd == "api":
        if args.api_cmd == "init":
            return cmd_api_init(root)
        if args.api_cmd == "validate":
            return cmd_api_validate(root)
        if args.api_cmd == "test":
            return cmd_api_test(root)

    return 0
