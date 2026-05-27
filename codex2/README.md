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

## Chat Completions Providers

Codex2 adds `wire_api = "chat"` for OpenAI-compatible Chat Completions
providers. The bridge maps Codex's Responses-style request history and function
tools onto `/chat/completions`, then maps streamed chat deltas, reasoning text,
tool calls, and token usage back into Codex response events.

## Upstream Updates

Use `scripts/sync-upstream.ps1` to create a branch, fetch `upstream/main`, and
merge upstream changes for review. Keep Codex2 changes small and isolated so
upstream bug fixes are easy to merge.
