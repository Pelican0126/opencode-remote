# openclaw-json-repair-kit

**Language / 语言：** [简体中文](#简体中文) | [English](#english)

---

## 简体中文

这是一个给“小白”也能用的 JSON 修复工具：
- 自动扫描 JSON 问题
- 自动修复并自动备份
- 一键回滚
- 交互式填写 `.env`
- API 连通性检测

> ✅ 已移除任何会话中的私有 API Key / 私有 Base URL。仓库只保留安全模板与默认示例。

---

### 1）3 分钟安装（Linux / macOS）

```bash
git clone https://github.com/Pelican0126/openclaw-json-repair-kit.git
cd openclaw-json-repair-kit
python3 -m venv .venv
source .venv/bin/activate
pip install -U pip
pip install -r requirements.txt
```

### 2）Windows 安装（PowerShell）

```powershell
git clone https://github.com/Pelican0126/openclaw-json-repair-kit.git
cd openclaw-json-repair-kit
py -m venv .venv
.\.venv\Scripts\Activate.ps1
python -m pip install -U pip
pip install -r requirements.txt
```

如果报执行策略错误：
```powershell
Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass
```

---

### 3）直接启动交互菜单（推荐）

```bash
python -m kit tui
```

启动后会让你做两件事：
1. 选择语言（中文/英文）
2. 输入“JSON 扫描路径”

---

### 4）扫描路径怎么填（最重要）

填“**存放 JSON 文件的目录**”，不是单个文件。

示例：
- Linux/macOS：`/home/ubuntu/myapp/config`
- Windows：`C:\Users\you\project\config`
- 相对路径：`./config` 或 `../service-a/config`
- 直接回车：默认当前目录

如果你填错了（路径不存在/不是目录），程序会自动回退到当前目录并提示你。

---

### 5）菜单每个选项干什么

- `1 扫描 JSON`：先查问题
- `2 生成模板`：快速生成 JSON 模板
- `3 修复 JSON（应用+备份）`：自动修复，并先备份
- `4 回滚最近备份`：不满意就撤销
- `5 校验 API 环境变量`：检查 `.env` 填写是否完整
- `6 API 连通性测试`：测 DNS/TLS/HTTP
- `7 全量检查`：一键跑完整链路
- `8 交互式写入 .env`：向导填写 API 配置（推荐）

---

### 6）小白推荐流程（按这个来）

1. `python -m kit tui`
2. 选语言 + 填扫描目录
3. 先跑 `1`（扫描）
4. 再跑 `3`（修复+备份）
5. 如果结果不满意，跑 `4`（回滚）
6. 要测 API 时，先跑 `8`（写 `.env`），再跑 `5` 和 `6`

---

### 7）安全提醒

- `.env` 不会提交到 Git（已在 `.gitignore`）
- 不要把 key 发到群里/工单里
- 交互式 `.env` 向导输入 key 时不会明文回显

---

## English

A beginner-friendly toolkit to:
- scan JSON errors,
- fix JSON with backup,
- rollback safely,
- fill `.env` interactively,
- test API connectivity.

> ✅ No private API key or private base URL is stored in this repo.

### Quick Start

```bash
git clone https://github.com/Pelican0126/openclaw-json-repair-kit.git
cd openclaw-json-repair-kit
python3 -m venv .venv
source .venv/bin/activate
pip install -U pip
pip install -r requirements.txt
python -m kit tui
```

### What to input for scan path?

Input a **directory containing JSON files** (not a single file):
- Linux/macOS: `/home/ubuntu/myapp/config`
- Windows: `C:\Users\you\project\config`
- Relative: `./config`, `../service-a/config`
- Press Enter to use current directory.

### Recommended flow

1. Run `python -m kit tui`
2. Select language + scan directory
3. Run `1` (scan)
4. Run `3` (fix + backup)
5. If needed, run `4` (rollback)
6. For API checks: run `8` (env wizard), then `5` and `6`
