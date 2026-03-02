# openclaw-json-repair-kit

一句话：这是一个给新手准备的 JSON 诊断与修复工具，支持扫描、修复、备份回滚，以及可选的 AI 修复流程。

## 特性

- 自动扫描目录中的 JSON 问题并输出报告
- 支持修复并在修改前自动备份
- 支持一键回滚最近一次备份
- 提供交互式 TUI，新手可按菜单操作
- 提供 `.env.example` 模板与 API 连通性检查
- 默认不包含任何真实 API key/token

## 快速开始（3 分钟）

1. 克隆仓库并进入目录
2. 运行预检脚本（检查 Python、虚拟环境、依赖状态）
3. 安装依赖并启动 TUI

```bash
git clone <your-private-repo-url>
cd openclaw-json-repair-kit
bash scripts/preflight.sh
python3 -m venv .venv
source .venv/bin/activate
python -m pip install -U pip
python -m pip install -r requirements.txt
cp .env.example .env
python -m kit tui
```

提示：如果你只想先体验本地 JSON 扫描/修复，`.env` 可先保留模板值；只有 AI 模式和 API 测试需要你填真实 key。

## Windows / macOS / Linux

Windows (PowerShell):

```powershell
git clone <your-private-repo-url>
cd openclaw-json-repair-kit
powershell -ExecutionPolicy Bypass -File .\scripts\preflight.ps1
py -m venv .venv
.\.venv\Scripts\Activate.ps1
python -m pip install -U pip
python -m pip install -r requirements.txt
Copy-Item .env.example .env
python -m kit tui
```

macOS / Linux:

```bash
git clone <your-private-repo-url>
cd openclaw-json-repair-kit
bash scripts/preflight.sh
python3 -m venv .venv
source .venv/bin/activate
python -m pip install -U pip
python -m pip install -r requirements.txt
cp .env.example .env
python -m kit tui
```

常用命令：

```bash
python -m kit scan
python -m kit fix --apply --backup
python -m kit rollback --latest
python -m kit api validate
python -m kit api test
python -m kit api wizard
```

## 常见问题

Q1: 扫描路径填什么？
- 填目录，不是单个文件。例如 `./config`、`/etc/myapp`、`C:\Users\you\project\config`。

Q2: 执行 `python -m kit ...` 报 `ModuleNotFoundError`？
- 通常是没激活虚拟环境或没安装依赖。先激活 `.venv`，再执行 `python -m pip install -r requirements.txt`。

Q3: Windows 提示脚本执行策略受限？
- 先执行 `Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass`，然后再运行脚本。

Q4: AI 修复为什么失败？
- 请先执行 `python -m kit api wizard` 填写 `.env`，再执行 `python -m kit api validate` 和 `python -m kit api test`。

## 卸载 / 清理

仅删除本地环境与缓存，不影响你的源码：

- 删除虚拟环境：`.venv/`
- 删除缓存：`__pycache__/`、`.pytest_cache/`
- 删除运行输出：`reports/`、`logs/`、`build/`、`dist/`
- 删除本地敏感配置：`.env`

示例（macOS / Linux）：

```bash
rm -rf .venv __pycache__ .pytest_cache reports logs build dist .env
```

## 隐私与安全

- 仓库中只保留 `.env.example`，不提交真实密钥
- `.env`、证书、私钥等敏感文件已在 `.gitignore` 中忽略
- 不要在 Issue、截图、日志中暴露 token/key/base_url
- 发现漏洞请查看 `SECURITY.md` 的上报流程

## 贡献指南

欢迎提交 PR，建议流程：

1. 新建分支：`git checkout -b feat/your-change`
2. 本地安装依赖并运行测试：`python -m pytest -q`
3. 更新相关文档（如 README/脚本说明）
4. 提交 PR，说明改动动机、影响范围、验证方式

请避免提交以下内容：真实密钥、生产配置、无关大文件、格式化噪音改动。
