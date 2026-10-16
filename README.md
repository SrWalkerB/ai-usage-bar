# AI Usage Monitor

Tray app (Tauri 2 + React) that shows your Codex and Claude plan usage: session/weekly bars, time to reset, auto refresh.

## Run

```sh
pnpm install
pnpm tauri dev      # dev
pnpm tauri build    # installers in src-tauri/target/release/bundle
```

The app lives in the system tray: left click opens/closes the panel, right click shows the menu.

## Connecting

| Provider | Login | Shortcut |
| --- | --- | --- |
| Codex | Device code (ChatGPT account) | "Use Codex CLI login" reads `~/.codex/auth.json` |
| Claude | OAuth: open login, paste the code shown at the end | "Use Claude Code login" reads `~/.claude/.credentials.json` |

- Tokens from the app's own login are stored in `%APPDATA%\dev.walker.aiusage\data.json` and refreshed automatically.
- "Via CLI" connections re-read the CLI's file on every sync and **never** refresh it (refresh tokens rotate; refreshing would log the CLI out). If the CLI's token expires, open the CLI once.

## How usage is read

- Codex: `GET https://chatgpt.com/backend-api/wham/usage` → `rate_limit.primary_window` / `secondary_window`.
- Claude: `GET https://api.anthropic.com/api/oauth/usage` → `five_hour`, `seven_day`, `seven_day_opus`, `seven_day_sonnet`.

Each window's kind (session/daily/weekly) comes from its length, not its position, and only windows the API returns are drawn — plans with only a weekly limit show a single bar.

These are internal endpoints used by the official CLIs and may change without notice.
