# Custom Codex build

This fork adds selection support to the Codex composer. Press `Alt+Shift+A` to select the complete prompt, type or delete to replace the selection, or press `Ctrl+C` to copy it.

The branch `custom/select-all-0.149.0` starts from the OpenAI `rust-v0.149.0` tag. Tags named `custom-v*` run `.github/workflows/custom-windows-release.yml`, test the selection behavior, and publish the Windows `codex.exe` and `codex-code-mode-host.exe` binaries as one archive.

Keep the installed commands separate:

- `codex` is the tested custom release used for normal coding.
- `codex-dev` runs the local debug build from this source tree.
- `codex-official` runs the untouched OpenAI standalone release for comparison and recovery.

For a new OpenAI release, create a branch from its `rust-vX.Y.Z` tag, cherry-pick the custom commits, resolve any upstream conflicts, run the TUI tests, and push a `custom-vX.Y.Z.N` tag. Do not replace the custom `codex` command until that tagged build passes.
