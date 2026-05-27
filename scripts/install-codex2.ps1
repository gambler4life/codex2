param(
  [switch]$BuildFromSource
)

$ErrorActionPreference = "Stop"

$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$CargoRoot = Join-Path $RepoRoot "codex-rs"
$NativeExe = Join-Path $CargoRoot "target\release\codex.exe"

function Get-TargetTriple {
  if ($IsWindows -or $env:OS -eq "Windows_NT") {
    if ($env:PROCESSOR_ARCHITECTURE -eq "ARM64") {
      return "aarch64-pc-windows-msvc"
    }
    return "x86_64-pc-windows-msvc"
  }

  throw "install-codex2.ps1 currently supports Windows installs only."
}

function Find-GlobalCodexVendor {
  $targetTriple = Get-TargetTriple
  $platformPackage = if ($targetTriple -eq "aarch64-pc-windows-msvc") {
    "codex-win32-arm64"
  } else {
    "codex-win32-x64"
  }
  $npmRoot = (& npm root -g 2>$null)
  if (-not $npmRoot) {
    return $null
  }

  $vendor = Join-Path $npmRoot "@openai\codex\node_modules\@openai\$platformPackage\vendor"
  if (Test-Path (Join-Path $vendor "$targetTriple\bin\codex.exe")) {
    return $vendor
  }

  return $null
}

if ($BuildFromSource) {
  Push-Location $CargoRoot
  try {
    cargo build -p codex-cli --bin codex --release
  } finally {
    Pop-Location
  }
}

if (-not (Test-Path $NativeExe)) {
  $vendorSrc = Find-GlobalCodexVendor
  if ($null -eq $vendorSrc) {
    throw "No source-built codex.exe or global @openai/codex vendor was found. Install Codex once with npm install -g @openai/codex, or rerun this script with -BuildFromSource."
  }

  $vendorDest = Join-Path $RepoRoot "codex-cli\vendor"
  if (Test-Path $vendorDest) {
    Remove-Item -LiteralPath $vendorDest -Recurse -Force
  }
  Copy-Item -LiteralPath $vendorSrc -Destination $vendorDest -Recurse
}

$UserBin = Join-Path $env:USERPROFILE ".local\bin"
New-Item -ItemType Directory -Force -Path $UserBin | Out-Null

$ShimPath = Join-Path $UserBin "codex2.cmd"
$RepoShim = Join-Path $RepoRoot "bin\codex2.cmd"
$Shim = @"
@echo off
call "$RepoShim" %*
"@
Set-Content -Path $ShimPath -Value $Shim -Encoding ASCII

$Codex2Home = Join-Path $env:USERPROFILE ".codex2"
New-Item -ItemType Directory -Force -Path $Codex2Home | Out-Null

Write-Host "Installed codex2 command: $ShimPath"
Write-Host "Codex2 home: $Codex2Home"
