# openclaw-json-repair-kit

**Language / 语言：** [简体中文](#简体中文) | [English](#english)

---

# 简体中文

一个**可离线运行**的工具包，用于：
- **JSON 扫描/校验**（定位到文件/行列）
- **确定性修复**（可逆、可备份）
- **一键回滚**（恢复到修复前状态）
- **API 环境变量体检 + 连通性测试**（DNS/TLS/HTTP）
- **交互式 TUI**（新手友好，一条命令菜单操作）

> 适用场景：你在 VPS / Windows / macOS / Docker 里跑 OpenClaw 或相关工具时，经常遇到 JSON 配置格式问题、以及 provider/base_url/endpoint 配错导致的 401/404/超时。

## 快速上手（推荐：TUI）

```bash
git clone https://github.com/Pelican0126/openclaw-json-repair-kit.git
cd openclaw-json-repair-kit

python3 -m venv .venv
source .venv/bin/activate   # Windows 见下

pip install -U pip
pip install -r requirements.txt

python -m kit tui
```

启动后会先让你选择语言（中文/英文），然后出现菜单。

## 功能概览

- `scan`：递归扫描 JSON（默认排除 `.git node_modules dist build .venv .pytest_cache`）
- `template`：优先基于 `*.schema.json` 或 `schema/` 生成最小模板，否则给出 inferred skeleton
- `fix`：确定性可逆修复（BOM、注释剥离、尾随逗号、可证明安全的单引号替换）
- `backup/rollback`：修复前备份到 `.openclaw-backups/<backup_id>/`，带 `manifest.json`，可回滚
- `api validate`：校验 `.env` 是否完整/合法
- `api test`：最小请求连通测试（DNS/TLS/HTTP 状态 + 耗时）
- `tui`：交互式菜单

## 常用命令（非 TUI）

### JSON
```bash
python -m kit scan
python -m kit template
python -m kit fix --apply --backup
python -m kit rollback --latest
```

### API
```bash
python -m kit api wizard   # 交互式写入 .env（推荐）
python -m kit api validate
python -m kit api test
```

## `.env` 配置说明

1) 复制示例：
```bash
cp .env.example .env
```

2) 推荐直接用向导写入（避免漏字段/写错）：
```bash
python -m kit api wizard
```

> 安全：`.env` 已在 `.gitignore`；向导输入 key 时不会回显（getpass）。

## 跨平台运行

### VPS / Linux（Ubuntu/Debian/CentOS）
```bash
python3 -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt
python -m kit tui
```

### Windows（PowerShell）
```powershell
py -m venv .venv
.\.venv\Scripts\Activate.ps1
python -m pip install -U pip
pip install -r requirements.txt
python -m kit tui
```

如果激活脚本报执行策略错误：
```powershell
Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass
```

### macOS
和 Linux 一样：
```bash
python3 -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt
python -m kit tui
```

### Docker（环境隔离/更“干净”）

> 如果你希望我把 Dockerfile / compose 也一起补齐到仓库里，我可以直接加。

基本运行（挂载当前目录 + 读取 .env）：
```bash
docker run --rm -it \
  -v $(pwd):/app \
  -w /app \
  --env-file .env \
  python:3.11-slim bash -lc "python -m venv .venv && . .venv/bin/activate && pip install -U pip && pip install -r requirements.txt && python -m kit tui"
```

Windows PowerShell：
```powershell
docker run --rm -it -v ${PWD}:/app -w /app --env-file .env python:3.11-slim bash -lc "python -m venv .venv && . .venv/bin/activate && pip install -U pip && pip install -r requirements.txt && python -m kit tui"
```

## 常见问题（FAQ）

- **api validate 失败？**
  - `.env` 缺字段/URL/model/key。
  - 用 `python -m kit api wizard` 重新写一遍通常最快。

- **api test 报 URL 协议缺失？**
  - base_url 没写 `https://`。

- **容器里没有交互？**
  - Docker 必须 `-it`。

## 安全说明

- `.env` 不要提交（已在 `.gitignore`）
- 报告/日志不会打印 API key

---

# English

An **offline-friendly** toolkit for:
- **JSON scan/validate** (file + line/col)
- **Deterministic JSON fixes** (reversible, with backups)
- **One-command rollback**
- **API env validation + connectivity test** (DNS/TLS/HTTP)
- **Interactive TUI** (menu-driven)

## Quick Start (recommended: TUI)

```bash
git clone https://github.com/Pelican0126/openclaw-json-repair-kit.git
cd openclaw-json-repair-kit

python3 -m venv .venv
source .venv/bin/activate

pip install -U pip
pip install -r requirements.txt

python -m kit tui
```

The TUI will ask you to pick **English/Chinese** on startup.

## Commands (non-TUI)

### JSON
```bash
python -m kit scan
python -m kit template
python -m kit fix --apply --backup
python -m kit rollback --latest
```

### API
```bash
python -m kit api wizard   # interactive .env writer (recommended)
python -m kit api validate
python -m kit api test
```

## Cross-platform

### Linux / VPS
Same as Quick Start.

### Windows (PowerShell)
```powershell
py -m venv .venv
.\.venv\Scripts\Activate.ps1
python -m pip install -U pip
pip install -r requirements.txt
python -m kit tui
```

### macOS
Same as Linux.

### Docker
Run an ephemeral container and mount the project directory:
```bash
docker run --rm -it \
  -v $(pwd):/app \
  -w /app \
  --env-file .env \
  python:3.11-slim bash -lc "python -m venv .venv && . .venv/bin/activate && pip install -U pip && pip install -r requirements.txt && python -m kit tui"
```

## Security notes

- `.env` is gitignored; do NOT commit secrets.
- Reports/logs won’t print API keys.
