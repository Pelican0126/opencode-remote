# Public Release Notes

## Quick Start

```bash
chmod +x install.sh start.sh
./install.sh
```

The installer asks for `BOT_TOKEN` interactively, builds the Rust bot, and starts it.

## GitHub One-Click Install

```bash
git clone https://github.com/Pelican0126/opencode-remote.git && cd opencode-remote && chmod +x install.sh start.sh && ./install.sh
```

## Security Checklist Before Publish

1. Do not include `.env`, `.env.*` (except `.env.example`).
2. Do not include `.opencode-runtime/`, `data/`, `workspace/`, `sandbox/`, `bot.log`.
3. Run:

```bash
./scripts/check-secrets.sh
```
