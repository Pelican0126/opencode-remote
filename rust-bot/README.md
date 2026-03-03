# tg-opencode-rs

Rust implementation of the Telegram opencode runner bot.

## Run

```bash
cargo run
```

Use the same environment variables as the TypeScript version (`BOT_TOKEN`, `WORKSPACE_ROOT`, `DEFAULT_MODEL`, etc.).

## Notes

- Keeps command/menu behavior aligned with the TS bot.
- Supports `/del this project` with button confirmation and deletes the project folder.
- Persists contexts and task history to `TASK_HISTORY_FILE`.
