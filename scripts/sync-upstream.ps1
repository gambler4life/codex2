param(
  [string]$BranchName
)

$ErrorActionPreference = "Stop"

$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
Push-Location $RepoRoot
try {
  $status = git status --porcelain
  if ($status) {
    throw "Worktree is not clean. Commit or stash local changes before syncing upstream."
  }

  git fetch upstream main

  if (-not $BranchName) {
    $stamp = Get-Date -Format "yyyyMMdd-HHmmss"
    $BranchName = "sync/upstream-$stamp"
  }

  git switch -c $BranchName
  git merge --no-ff upstream/main

  node --check codex-cli\bin\codex2.js
  node -e "JSON.parse(require('fs').readFileSync('codex-cli/package.json','utf8')); JSON.parse(require('fs').readFileSync('codex2/models/qwen.models.json','utf8')); console.log('codex2 smoke checks passed')"

  Write-Host "Created upstream sync branch: $BranchName"
  Write-Host "Review, test, then merge this branch back to main when ready."
} finally {
  Pop-Location
}
