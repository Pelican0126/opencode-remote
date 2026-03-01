# openclaw-json-repair-kit

**Language / 语言：** [简体中文](#简体中文) | [English](#english)

---

## 简体中文

这是一个给"小白"也能用的 JSON 修复工具：
- ✅ 自动扫描 JSON 问题
- ✅ 自动修复并自动备份  
- ✅ 一键回滚
- ✅ AI 智能修复模式（循环尝试直到成功）
- ✅ 交互式填写 `.env`
- ✅ API 连通性检测

> ✅ 已移除任何私有的 API Key / 私有 Base URL。仓库只保留安全模板与默认示例。`.env` 不会提交到 Git。

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
1. 选择语言（输入 `en` 或 `zh`，直接回车默认中文）
2. 输入"JSON 扫描路径"

---

### 4）扫描路径怎么填（最重要！）

**填"存放 JSON 文件的目录"，不是单个文件！**

#### 常见扫描路径示例：

| 场景 | 路径示例 |
|------|----------|
| 本地项目 | `/home/ubuntu/myapp/config` |
| Windows | `C:\Users\you\project\config` |
| 相对路径 | `./config` 或 `../service-a/config` |
| 直接回车 | 默认当前目录 |

#### 腾讯云 VPS 默认路径示例：

| 服务 | 路径示例 |
|------|----------|
| Nginx 配置 | `/etc/nginx/conf.d/` |
| 网站根目录 | `/var/www/html/` |
| 应用配置 | `/home/ubuntu/myapp/config/` |
| Docker 配置 | `/opt/docker/` |

#### 阿里云 VPS 默认路径示例：

| 服务 | 路径示例 |
|------|----------|
| Nginx 配置 | `/etc/nginx/conf.d/` |
| 网站根目录 | `/var/www/html/` |
| 应用配置 | `/root/myapp/config/` |
| Docker 配置 | `/opt/docker/` |

如果你填错了（路径不存在/不是目录），程序会自动回退到当前目录并提示你。

---

### 5）菜单每个选项干什么

- `1` 扫描 JSON：先查问题
- `2` 生成模板：快速生成 JSON 模板
- `3` 修复 JSON（应用+备份）：选择文件后自动修复，并先备份
- `4` 回滚最近备份：不满意就撤销
- `5` 校验 API 环境变量：检查 `.env` 填写是否完整
- `6` AI 修复模式：使用 AI 循环尝试修复（需要 API）
- `7` 全量检查：一键跑完整链路（pytest + scan + fix + rollback + api）
- `8` 交互式写入 .env：向导填写 API 配置（推荐）
- `9` API 连通性测试：测 DNS/TLS/HTTP

---

### 6）CLI 模式用法

除了交互式菜单，还支持命令行模式：

```bash
# 扫描 JSON 文件
python -m kit scan

# 修复 JSON（仅扫描，不应用）
python -m kit fix

# 修复并应用（推荐）
python -m kit fix --apply --backup

# 使用 AI 修复模式（需要 API）
python -m kit fix --apply --backup --ai

# 回滚到上一版本
python -m kit rollback --latest

# API 相关
python -m kit api validate
python -m kit api test
python -m kit api wizard
```

---

### 7）AI 修复模式说明

AI 修复模式使用大语言模型循环尝试修复 JSON：
- 最多循环 5 轮（可配置）
- 每轮尝试解析，如果成功就停止
- 如果所有轮次都失败，报告错误

使用方式：
- TUI：选择菜单选项 `6`
- CLI：`python -m kit fix --apply --backup --ai`

**注意**：AI 模式需要在 `.env` 中配置 API Key，可使用 `python -m kit api wizard` 配置。

---

### 8）小白推荐流程（按这个来）

1. `python -m kit tui`
2. 选语言 + 填扫描目录
3. 先跑 `1`（扫描）查看有哪些问题
4. 再跑 `3`（修复+备份），按提示选择要修复的文件
5. 如果结果不满意，跑 `4`（回滚）
6. 要用 AI 模式时，先跑 `8`（写 `.env`），再跑 `6`
7. 跑 `9` 测试 API 是否可用

---

### 9）安全提醒

- `.env` 不会提交到 Git（已在 `.gitignore`）
- 不要把 key 发到群里/工单里
- 交互式 `.env` 向导输入 key 时不会明文回显

---

## English

A beginner-friendly toolkit to:
- ✅ Scan JSON errors
- ✅ Fix JSON with backup
- ✅ Rollback safely
- ✅ AI repair mode (loops until parse passes or max rounds)
- ✅ Fill `.env` interactively
- ✅ Test API connectivity

> ✅ No private API key or private base URL is stored in this repo. `.env` is gitignored.

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

| Scenario | Path Example |
|----------|--------------|
| Local project | `/home/ubuntu/myapp/config` |
| Windows | `C:\Users\you\project\config` |
| Relative | `./config`, `../service-a/config` |
| Press Enter | Current directory |

#### Tencent Cloud VPS Default Paths:

| Service | Path Example |
|---------|--------------|
| Nginx config | `/etc/nginx/conf.d/` |
| Web root | `/var/www/html/` |
| App config | `/home/ubuntu/myapp/config/` |
| Docker config | `/opt/docker/` |

#### Alibaba Cloud VPS Default Paths:

| Service | Path Example |
|---------|--------------|
| Nginx config | `/etc/nginx/conf.d/` |
| Web root | `/var/www/html/` |
| App config | `/root/myapp/config/` |
| Docker config | `/opt/docker/` |

### CLI Usage

```bash
# Scan JSON files
python -m kit scan

# Fix JSON (scan only, no apply)
python -m kit fix

# Fix and apply (recommended)
python -m kit fix --apply --backup

# AI repair mode (requires API)
python -m kit fix --apply --backup --ai

# Rollback
python -m kit rollback --latest

# API commands
python -m kit api validate
python -m kit api test
python -m kit api wizard
```

### AI Repair Mode

AI repair mode uses LLMs to iteratively fix JSON:
- Up to 5 rounds (configurable)
- Stops if parse succeeds
- Reports error if all rounds fail

Usage:
- TUI: Select menu option `6`
- CLI: `python -m kit fix --apply --backup --ai`

**Note**: AI mode requires API Key in `.env`. Run `python -m kit api wizard` to configure.

### Recommended Flow

1. Run `python -m kit tui`
2. Select language + scan directory
3. Run `1` (scan) to see issues
4. Run `3` (fix + backup), select files to fix
5. If needed, run `4` (rollback)
6. For AI mode: run `8` (env wizard) first, then `6`
7. Run `9` to test API connectivity
