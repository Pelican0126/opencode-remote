from __future__ import annotations

import argparse
import json
from datetime import datetime
from getpass import getpass
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


def _prompt(default: str, label: str, *, secret: bool = False) -> str:
    shown = "***" if (secret and default) else default
    tip = f" [{shown}]" if shown else ""
    raw = getpass(f"{label}{tip}: ") if secret else input(f"{label}{tip}: ")
    val = raw.strip()
    return default if val == "" else val


def cmd_api_wizard(root: Path, lang: str = "en") -> int:
    env_file = root / ".env"
    current = ApiEnv.load(root)

    is_zh = lang == "zh"
    print("交互式 .env 写入向导" if is_zh else "interactive .env writer")
    print((f"目标文件: {env_file}") if is_zh else (f"target: {env_file}"))
    print("（直接回车可保留当前值）" if is_zh else "(press Enter to keep current value)")

    region = _prompt(current.region or "intl", "REGION (cn/intl)")
    if region not in {"cn", "intl"}:
        print("REGION 非法，自动回退到 intl" if is_zh else "invalid REGION, fallback to intl")
        region = "intl"

    glm_api_key = _prompt(current.glm_api_key, "GLM_API_KEY", secret=True)
    glm_base_url_cn = _prompt(current.glm_base_url_cn or "https://open.bigmodel.cn/api/coding/paas/v4", "GLM_BASE_URL_CN")
    glm_base_url_intl = _prompt(current.glm_base_url_intl or "https://api.z.ai/api/coding/paas/v4", "GLM_BASE_URL_INTL")
    glm_model = _prompt(current.glm_model or "GLM-5", "GLM_MODEL")

    kimi_api_key = _prompt(current.kimi_api_key, "KIMI_API_KEY", secret=True)
    kimi_base_url_cn = _prompt(current.kimi_base_url_cn or "https://api.moonshot.cn/v1", "KIMI_BASE_URL_CN")
    kimi_base_url_intl = _prompt(current.kimi_base_url_intl or "https://api.moonshot.ai/v1", "KIMI_BASE_URL_INTL")
    kimi_model = _prompt(current.kimi_model or "moonshot-v1-8k", "KIMI_MODEL")

    endpoint_path = _prompt(current.endpoint_path or "/chat/completions", "API_ENDPOINT_PATH")
    if not endpoint_path.startswith("/"):
        endpoint_path = "/" + endpoint_path

    content = "\n".join(
        [
            "# region: cn/intl",
            f"REGION={region}",
            "",
            "# GLM (Z.AI / BigModel Coding Plan)",
            f"GLM_API_KEY={glm_api_key}",
            f"GLM_BASE_URL_CN={glm_base_url_cn}",
            f"GLM_BASE_URL_INTL={glm_base_url_intl}",
            f"GLM_MODEL={glm_model}",
            "",
            "# Kimi (Moonshot)",
            f"KIMI_API_KEY={kimi_api_key}",
            f"KIMI_BASE_URL_CN={kimi_base_url_cn}",
            f"KIMI_BASE_URL_INTL={kimi_base_url_intl}",
            f"KIMI_MODEL={kimi_model}",
            "",
            "# Optional endpoint path (OpenAI-compatible default)",
            f"API_ENDPOINT_PATH={endpoint_path}",
            "",
        ]
    )

    env_file.write_text(content, encoding="utf-8")
    print(f"written: {env_file}")
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


def _print_tui_header(root: Path, t: dict[str, str]) -> None:
    print("\n" + "=" * 56)
    print(t["title"])
    print(f" {t['cwd']}: {root}")
    print("=" * 56)


def _run_and_report(label: str, fn, t: dict[str, str]) -> None:
    print(f"\n→ {label}")
    code = fn()
    status = t["ok"] if code == 0 else f"{t['failed']} (exit={code})"
    print(f"✓ {t['done']}: {status}")


def cmd_tui(root: Path) -> int:
    lang_pick = input("Language / 语言 [en/zh] (default: zh): ").strip().lower() or "zh"
    lang = "zh" if lang_pick.startswith("z") else "en"
    t = {
        "title": " openclaw-json-repair-kit | 交互式终端" if lang == "zh" else " openclaw-json-repair-kit | Interactive TUI",
        "cwd": "工作目录" if lang == "zh" else "cwd",
        "ok": "成功" if lang == "zh" else "OK",
        "failed": "失败" if lang == "zh" else "FAILED",
        "done": "完成" if lang == "zh" else "done",
        "choose": "请选择 [0-8]: " if lang == "zh" else "Select [0-8]: ",
        "bye": "已退出" if lang == "zh" else "bye",
        "invalid": "无效选项" if lang == "zh" else "invalid option",
        "continue": "按回车继续..." if lang == "zh" else "Press Enter to continue...",
    }

    scan_input_tip = "请输入 JSON 扫描路径（回车=当前目录）: " if lang == "zh" else "Enter JSON scan path (Enter = current directory): "
    scan_raw = input(scan_input_tip).strip()
    scan_root = (Path(scan_raw).expanduser() if scan_raw else root).resolve()
    if not scan_root.exists() or not scan_root.is_dir():
        print((f"路径无效，回退到当前目录: {root}") if lang == "zh" else (f"Invalid path, fallback to current directory: {root}"))
        scan_root = root

    while True:
        _print_tui_header(root, t)
        print((f" JSON扫描路径: {scan_root}") if lang == "zh" else (f" JSON scan path: {scan_root}"))
        print(" 1) 扫描 JSON 文件" if lang == "zh" else " 1) scan JSON files")
        print(" 2) 生成模板" if lang == "zh" else " 2) generate template")
        print(" 3) 修复 JSON（应用+备份）" if lang == "zh" else " 3) fix JSON (apply + backup)")
        print(" 4) 回滚最近备份" if lang == "zh" else " 4) rollback latest backup")
        print(" 5) 校验 API 环境变量" if lang == "zh" else " 5) API env validate")
        print(" 6) API 连通性测试" if lang == "zh" else " 6) API connectivity test")
        print(" 7) 全量检查（pytest+scan+fix+rollback+api）" if lang == "zh" else " 7) full check (pytest + scan + fix + rollback + api)")
        print(" 8) 交互式写入 .env" if lang == "zh" else " 8) write .env (interactive wizard)")
        print(" 0) 退出" if lang == "zh" else " 0) quit")

        choice = input("\n" + t["choose"]).strip()

        if choice == "0":
            print(t["bye"])
            return 0
        if choice == "1":
            _run_and_report("scan", lambda: cmd_scan(scan_root), t)
        elif choice == "2":
            _run_and_report("template", lambda: cmd_template(scan_root), t)
        elif choice == "3":
            _run_and_report("fix --apply --backup", lambda: cmd_fix(scan_root, apply=True, backup=True), t)
        elif choice == "4":
            _run_and_report("rollback --latest", lambda: cmd_rollback(scan_root, latest=True), t)
        elif choice == "5":
            _run_and_report("api validate", lambda: cmd_api_validate(root), t)
        elif choice == "6":
            _run_and_report("api test", lambda: cmd_api_test(root), t)
        elif choice == "7":
            from subprocess import run
            import sys

            def full_check() -> int:
                steps = [
                    (root, [sys.executable, "-m", "pytest", "-q"]),
                    (scan_root, [sys.executable, "-m", "kit", "scan"]),
                    (scan_root, [sys.executable, "-m", "kit", "template"]),
                    (scan_root, [sys.executable, "-m", "kit", "fix", "--apply", "--backup"]),
                    (scan_root, [sys.executable, "-m", "kit", "rollback", "--latest"]),
                    (root, [sys.executable, "-m", "kit", "api", "validate"]),
                    (root, [sys.executable, "-m", "kit", "api", "test"]),
                ]
                for cwd, step in steps:
                    print("$", " ".join(step), f"(cwd={cwd})")
                    rc = run(step, cwd=cwd).returncode
                    if rc != 0:
                        return rc
                return 0

            _run_and_report("full check", full_check, t)
        elif choice == "8":
            _run_and_report("api wizard", lambda: cmd_api_wizard(root, lang=lang), t)
        else:
            print(t["invalid"])

        input("\n" + t["continue"])


def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(prog="kit")
    sub = p.add_subparsers(dest="cmd", required=True)

    sub.add_parser("scan")
    sub.add_parser("template")
    sub.add_parser("tui")

    p_fix = sub.add_parser("fix")
    p_fix.add_argument("--apply", action="store_true")
    p_fix.add_argument("--backup", action="store_true")

    p_rb = sub.add_parser("rollback")
    p_rb.add_argument("--latest", action="store_true")

    p_api = sub.add_parser("api")
    api_sub = p_api.add_subparsers(dest="api_cmd", required=True)
    api_sub.add_parser("init")
    api_sub.add_parser("wizard")
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
    if args.cmd == "tui":
        return cmd_tui(root)
    if args.cmd == "fix":
        return cmd_fix(root, apply=args.apply, backup=args.backup)
    if args.cmd == "rollback":
        return cmd_rollback(root, latest=args.latest)
    if args.cmd == "api":
        if args.api_cmd == "init":
            return cmd_api_init(root)
        if args.api_cmd == "wizard":
            lang_pick = input("Language / 语言 [en/zh] (default: zh): ").strip().lower() or "zh"
            lang = "zh" if lang_pick.startswith("z") else "en"
            return cmd_api_wizard(root, lang=lang)
        if args.api_cmd == "validate":
            return cmd_api_validate(root)
        if args.api_cmd == "test":
            return cmd_api_test(root)

    return 0
