# Codex2

Codex2 is a fork launcher for Codex with separate state under `~/.codex2`.

## Update Banner

The `codex2` launcher injects `-c check_for_update_on_startup=false` unless
`CODEX2_ENABLE_UPDATE_CHECK` is set. This keeps upstream's update notification
out of the forked CLI while leaving the normal `codex` command untouched.

The installer also creates `~/.codex2/config.toml` with update checks disabled
when that file does not already exist.

## Bundled Profiles

The installer copies provider profile files into `~/.codex2`:

- `qwen.config.toml`: DashScope international endpoint.
- `qwen-cn.config.toml`: DashScope China endpoint.
- `qwen-us.config.toml`: DashScope US endpoint.
- `deepseek.config.toml`: DeepSeek Chat Completions endpoint.
- `mimo.config.toml`: Xiaomi MiMo Chat Completions endpoint.
- `xiaomi.config.toml`: Alias for the Xiaomi MiMo profile.
- `xiamo.config.toml`: Typo-friendly alias for the Xiaomi MiMo profile.

Set the provider key you want to use:

```powershell
$env:DASHSCOPE_API_KEY = "..."
$env:DEEPSEEK_API_KEY = "..."
$env:MIMO_API_KEY = "..."
```

Launch with one of the profiles:

```powershell
codex2 --profile qwen
codex2 --profile qwen-cn
codex2 --profile qwen-us
codex2 --profile deepseek
codex2 --profile mimo
codex2 --profile xiaomi
codex2 --profile xiamo
```

Inside the TUI, `/model` will show the active profile's bundled model catalog.

## Adding Providers In The TUI

Codex2 includes slash commands for simple provider setup without hand-editing
TOML:

```text
/providers
/providers add <id> <base_url> <model> [chat|responses]
/api-key <id> <api_key>
```

Example:

```text
/providers add deepseek https://api.deepseek.com deepseek-v4-pro chat
/api-key deepseek sk-...
```

`/providers add` writes `<id>.config.toml` and a one-model catalog under
`~/.codex2`. `/api-key` stores the key in that profile. Restart with
`codex2 --profile <id>` to use it.

## Chat Completions Providers

Codex2 adds `wire_api = "chat"` for OpenAI-compatible Chat Completions
providers. The bridge maps Codex's Responses-style request history and function
tools onto `/chat/completions`, then maps streamed chat deltas, reasoning text,
tool calls, and token usage back into Codex response events.

## Upstream Updates

Use `scripts/sync-upstream.ps1` to create a branch, fetch `upstream/main`, and
merge upstream changes for review. Keep Codex2 changes small and isolated so
upstream bug fixes are easy to merge.
