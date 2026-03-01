# openclaw-json-repair-kit

一个可离线测试的 JSON 修复 + API 配置校验工具包。

## 1) 安装 OpenCode（opencode）

### macOS / Linux
```bash
curl -fsSL https://opencode.ai/install | bash
opencode --version
```

### npm（跨平台）
```bash
npm install -g opencode-ai
opencode --version
```

### macOS + brew
```bash
brew install anomalyco/tap/opencode
opencode --version
```

## 2) 国内/国际 一键启动 opencode

### PowerShell
```powershell
.\tools\opencode-cn.ps1
.\tools\opencode-intl.ps1
```

### bash
```bash
./tools/opencode-cn.sh
./tools/opencode-intl.sh
```

## 3) 在 OpenCode 里连接 provider

1. 在项目目录启动 opencode（先选 cn 或 intl 脚本）
2. 输入 `/connect`
3. 配置：
   - `Z.AI`（或 `Zhipu AI Coding Plan`）
   - `Moonshot AI`
4. 输入 `/models`
   - 选择 `GLM-5`
   - 选择 `Kimi`（K2/K2.5 视列表）
5. 最小验证：
```bash
opencode run "Hello from openclaw-json-repair-kit"
```

## 4) 本项目工具用法

先准备环境：
```bash
python3 -m venv .venv
source .venv/bin/activate  # Windows 用 .\.venv\Scripts\Activate.ps1
pip install -U pip
pip install -r requirements.txt
cp .env.example .env
```

### JSON
```bash
python -m kit scan
python -m kit template
python -m kit fix --apply --backup
python -m kit rollback --latest
```

### API
```bash
python -m kit api init
python -m kit api validate
python -m kit api test
```

## 5) 常见错误排查

- 国内/国际 base url 混用导致 401
- endpoint path 不匹配（默认 `/chat/completions`，可在 `.env` 里改 `API_ENDPOINT_PATH`）
- 代理/网络问题导致 DNS/TLS/HTTP 失败

## 6) 安全说明

- `.env` 已在 `.gitignore` 中，**不要提交密钥**
- 日志与报告不会打印 API key

## CLI 功能说明

- `scan`：递归扫描 JSON（默认排除 `.git node_modules dist build .venv .pytest_cache`）
- `validate`：严格 parse，输出文件/行/列/上下文（通过 `scan` 报告）
- `template`：优先基于 `*.schema.json` 或 `schema/` 生成最小模板，否则返回 skeleton 并标注 inferred
- `fix`：确定性可逆修复（BOM、注释剥离、尾随逗号、可证明安全的单引号替换）
- `backup/rollback`：修复前备份到 `.openclaw-backups/<backup_id>/`，带 `manifest.json`，失败自动回滚
- `report`：生成 text + json 两种报告，位于 `reports/`


## 快速上手（从 GitHub 克隆）

```bash
git clone https://github.com/Pelican0126/openclaw-json-repair-kit.git
cd openclaw-json-repair-kit
python3 -m venv .venv
source .venv/bin/activate   # Windows: .\.venv\Scripts\Activate.ps1
pip install -U pip
pip install -r requirements.txt
cp .env.example .env
```

然后执行：

```bash
python -m kit scan
python -m kit fix --apply --backup
python -m kit rollback --latest
python -m kit api validate
python -m kit api test
```
