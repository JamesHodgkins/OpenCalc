# Build the release binaries and zip them for Windows.
#   powershell -File scripts/package.ps1
# Writes dist/opencalc-<version>-windows-x64.zip holding opencalc.exe (the app),
# opencalc-cli.exe (the command-line checker), the licence and the README.

$ErrorActionPreference = "Stop"
Set-Location (Split-Path $PSScriptRoot -Parent)

cargo build --release -p opencalc-ui -p opencalc-cli
if ($LASTEXITCODE -ne 0) { Write-Host "FAILED: release build"; exit $LASTEXITCODE }

$version = (Select-String -Path Cargo.toml -Pattern '^version\s*=\s*"(.+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
$name = "opencalc-$version-windows-x64"
$stage = Join-Path "dist" $name
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory -Force $stage | Out-Null
Copy-Item target/release/opencalc.exe, target/release/opencalc-cli.exe, LICENSE, README.md $stage

$zip = "dist/$name.zip"
if (Test-Path $zip) { Remove-Item -Force $zip }
Compress-Archive -Path $stage -DestinationPath $zip
Write-Host "Wrote $zip"
Get-Item $zip | ForEach-Object { "{0:N1} MB" -f ($_.Length / 1MB) }
