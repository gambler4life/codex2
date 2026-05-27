@echo off
setlocal
set "REPO_ROOT=%~dp0.."

if not defined CODEX_HOME set "CODEX_HOME=%USERPROFILE%\.codex2"
set "CODEX2=1"

if exist "%REPO_ROOT%\codex-rs\target\release\codex.exe" (
  "%REPO_ROOT%\codex-rs\target\release\codex.exe" %*
  exit /b %ERRORLEVEL%
)

if exist "%REPO_ROOT%\codex-cli\bin\codex2.js" (
  node "%REPO_ROOT%\codex-cli\bin\codex2.js" %*
  exit /b %ERRORLEVEL%
)

echo codex2 native binary was not found.
echo Run: powershell -ExecutionPolicy Bypass -File "%REPO_ROOT%\scripts\install-codex2.ps1"
exit /b 1
