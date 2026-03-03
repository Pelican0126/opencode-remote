# TG Opencode Remote

给只会复制粘贴的用户：一条命令拉下来，填 `BOT_TOKEN`，就能在 Telegram 里和已安装的 `opencode` 交互。

## 一键安装（只复制这一条）

> 建议在 Git Bash 运行

```bash
git clone https://github.com/Pelican0126/opencode-remote.git && cd opencode-remote && chmod +x install.sh start.sh && ./install.sh
```

安装器会交互式询问 `BOT_TOKEN`，填完自动启动。

## 后续启动（只复制这一条）

```bash
cd opencode-remote && ./start.sh
```

## Telegram 常用命令

- `/menu`
- `/run <任务>`（直接发文本也可以）
- 图片 + 文字（caption）也支持
- `/status`
- `/cancel` 或 `/interrupt`
- `/projects`
- `/new <项目名>`
- `/use <项目名>`
- 删除项目：`/menu -> Projects -> Delete Project`

## 发布前安全检查

```bash
./scripts/check-secrets.sh
```
