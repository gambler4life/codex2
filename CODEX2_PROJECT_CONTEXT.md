# Codex2 Project Context

This file captures the current Codex2 work so future sessions can start in
`C:\Users\master\codex2` instead of mixing this project with
`C:\Users\master\psclonereal`.

## Project Location

- Repo: `C:\Users\master\codex2`
- Main Rust workspace: `C:\Users\master\codex2\codex-rs\Cargo.toml`
- Wrapper/package files: repo root, `codex-cli`, and `codex2`
- GitHub remote: `https://github.com/gambler4life/codex2.git`
- Branch: `main`

## Objective

Codex2 is a separate fork/custom build of the OpenAI Codex CLI. The goal is to
keep normal Codex available while developing a separate `codex2` command with:

- independent install/name/config from the normal Codex CLI
- support for non-OpenAI/OpenAI-compatible providers such as DeepSeek and Xiaomi/MiMo
- cheaper executor-model workflows later, with OpenAI models as orchestrators
- `/goal` support preserved
- easier future merging from upstream Codex CLI

## Current Status

- `codex2` command exists and resolves on the system.
- Release binary has been rebuilt successfully.
- The repo was clean and pushed after the latest provider setup work.
- Latest known commit after provider command work:
  `9722ebe11c Add provider setup slash commands`

## Recent Work Completed

- Removed the practical need to hand-edit provider TOML for simple providers.
- Added in-program TUI slash commands:
  - `/providers`
  - `/providers add <id> <base_url> <model> [chat|responses]`
  - `/api-key <id> <api_key>`
- `/api-key` avoids slash-command history/recall so secrets are not kept in the
  local slash-command recall path.
- Provider setup writes profile files and one-model catalogs under `~/.codex2`.
- README usage documentation was updated.
- Chat Completions provider support already exists via `wire_api = "chat"`.
- DeepSeek/MiMo/Xiaomi-style profiles/catalog support was added earlier.
- Startup update-check behavior was adjusted for Codex2 profiles.

## Provider Usage

Inside Codex2:

```text
/providers
/providers add deepseek https://api.deepseek.com deepseek-v4-pro chat
/api-key deepseek sk-...
```

Then restart:

```powershell
codex2 --profile deepseek
```

Generated files live under `~/.codex2`, for example:

```text
deepseek.config.toml
models/deepseek.models.json
```

## Build And Test Commands

Run Rust commands from:

```powershell
cd C:\Users\master\codex2\codex-rs
```

Useful commands:

```powershell
cargo fmt -- --config imports_granularity=Item
$env:Path = 'C:\Program Files\Git\usr\bin;' + $env:Path; just test -p codex-tui provider_setup --lib
$env:Path = 'C:\Program Files\Git\usr\bin;' + $env:Path; just fix -p codex-tui
cargo build -p codex-cli --bin codex --release
codex2 --version
```

If the release build cannot replace `target\release\codex.exe`, stop only the
Codex2 release-path process:

```powershell
Get-Process codex -ErrorAction SilentlyContinue |
  Where-Object { $_.Path -eq 'C:\Users\master\codex2\codex-rs\target\release\codex.exe' } |
  Stop-Process -Force
```

## Commit Procedure

- Stage exact files only.
- Do not use `git add -A`.
- Use a message file on Windows PowerShell:

```powershell
Set-Content -Path .git\codex-commit-msg.txt -Value "Message" -Encoding ASCII
git commit -F .git\codex-commit-msg.txt
git log -1 --pretty=%B
```

- Never add `Co-Authored-By` or other co-author trailers.
- Push to `origin main` after successful commits unless asked not to.

## Important Notes

- Keep Codex2 separate from `psclonereal`.
- Start future Codex2 sessions with working directory `C:\Users\master\codex2`.
- The normal OpenAI Codex install may also run as `codex.exe`; do not kill those
  processes unless explicitly intended.
- `codex2` currently stores `/api-key` values in the profile TOML. A future
  improvement would be encrypted/keyring-backed storage.
- Larger future work: sub-agent/executor architecture using cheaper models,
  queued user messages after the next tool call, and cleaner upstream merge flow.
