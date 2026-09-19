#
# ezer installer for PowerShell — build from this repository and install
# `ezer.exe` under $env:EZER_HOME\bin (default: %USERPROFILE%\.ezer\bin).
#
# This fork does not download SpaceXAI / grok.com artifacts.

$ErrorActionPreference = 'Stop'

$EzerHome = if ($env:EZER_HOME) { $env:EZER_HOME } else { Join-Path $env:USERPROFILE '.ezer' }
$BinDir = if ($env:EZER_BIN_DIR) { $env:EZER_BIN_DIR } else { Join-Path $EzerHome 'bin' }
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..\..')).Path

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Error 'cargo is required. Install Rust from https://rustup.rs'
}

New-Item -ItemType Directory -Force -Path $BinDir | Out-Null
Write-Host "Building ezer from $RepoRoot ..."
cargo build -p xai-grok-pager-bin --release --bin ezer --manifest-path (Join-Path $RepoRoot 'Cargo.toml')
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$src = Join-Path $RepoRoot 'target\release\ezer.exe'
Copy-Item -Force $src (Join-Path $BinDir 'ezer.exe')
Write-Host "Installed $(Join-Path $BinDir 'ezer.exe')"

$config = Join-Path $EzerHome 'config.toml'
if (-not (Test-Path $config)) {
    @'
[models]
default = "my-model"

[model.my-model]
model = "my-model"
base_url = "http://192.168.0.63:8788/v1"
api_backend = "responses"

[cli]
auto_update = false
'@ | Set-Content -Path $config -Encoding utf8
    Write-Host "Wrote starter config at $config"
}

Write-Host "Add $BinDir to PATH, then run: ezer --help"
