# Codex2

Codex2 is a fork launcher for Codex with separate state under `~/.codex2`.

## Update Banner

The `codex2` launcher injects `-c check_for_update_on_startup=false` unless
`CODEX2_ENABLE_UPDATE_CHECK` is set. This keeps upstream's update notification
out of the forked CLI while leaving the normal `codex` command untouched.

The installer also creates `~/.codex2/config.toml` with update checks disabled
when that file does not already exist.

## Qwen Profiles

The installer copies Qwen profile files into `~/.codex2`:

- `qwen.config.toml`: DashScope international endpoint.
- `qwen-cn.config.toml`: DashScope China endpoint.
- `qwen-us.config.toml`: DashScope US endpoint.

Set a DashScope key:

```powershell
$env:DASHSCOPE_API_KEY = "..."
```

Launch with one of the profiles:

```powershell
codex2 --profile-v2 qwen
codex2 --profile-v2 qwen-cn
codex2 --profile-v2 qwen-us
```

Inside the TUI, `/model` will show the Qwen catalog from
`~/.codex2/models/qwen.models.json`.

## DeepSeek And Kimi

Upstream Codex currently supports the OpenAI Responses wire protocol. Qwen has
an official OpenAI-compatible Responses endpoint, so it can be configured
without Rust changes. DeepSeek and Kimi publish OpenAI-compatible Chat
Completions endpoints in their official docs, so first-class support for them
requires adding a Chat Completions wire adapter to Codex2 and then building the
native Rust binary.

## Upstream Updates

Use `scripts/sync-upstream.ps1` to create a branch, fetch `upstream/main`, and
merge upstream changes for review. Keep Codex2 changes small and isolated so
upstream bug fixes are easy to merge.
