# The commit checks from SPEC section 4.10, plus the file-size rule.
# Run before every commit:  powershell -File scripts/check.ps1
# Exits non-zero on the first failure.

$ErrorActionPreference = "Stop"
Set-Location (Split-Path $PSScriptRoot -Parent)

function Step($name, [scriptblock]$run) {
    Write-Host "== $name"
    & $run
    if ($LASTEXITCODE -ne 0) { Write-Host "FAILED: $name"; exit $LASTEXITCODE }
}

Step "cargo fmt --all --check" { cargo fmt --all --check }
Step "cargo clippy (warnings are errors)" { cargo clippy --workspace --all-targets -- -D warnings }
Step "cargo test" { cargo test --workspace }

# File-size rule: about 400 production lines per file (lines before #[cfg(test)]).
# Files that already exceed it are listed in scripts/line_limit_exceptions.txt with
# their current size. They may shrink but never grow: a ratchet, so the debt
# cannot get worse while it waits for its refactor (see docs/TECH_DEBT.md).
Write-Host "== file-size limit (400 production lines)"
$limit = 400
$exceptions = @{}
Get-Content "scripts/line_limit_exceptions.txt" | Where-Object { $_ -and -not $_.StartsWith("#") } | ForEach-Object {
    $path, $max = $_ -split "\s+"
    $exceptions[$path] = [int]$max
}
$bad = @()
Get-ChildItem crates -Recurse -Filter *.rs | ForEach-Object {
    $rel = (Resolve-Path -Relative $_.FullName).TrimStart(".\").Replace("\", "/")
    $lines = Get-Content $_.FullName
    $test = ($lines | Select-String -Pattern '^#\[cfg\(test\)\]' | Select-Object -First 1).LineNumber
    $prod = if ($test) { $test - 1 } else { $lines.Count }
    $allowed = if ($exceptions.ContainsKey($rel)) { $exceptions[$rel] } else { $limit }
    if ($prod -gt $allowed) { $bad += "{0}: {1} production lines (allowed {2})" -f $rel, $prod, $allowed }
    elseif ($exceptions.ContainsKey($rel) -and $prod -le $limit) {
        $bad += "{0}: now {1} lines, within the limit: remove it from line_limit_exceptions.txt" -f $rel, $prod
    }
}
if ($bad.Count -gt 0) { $bad | ForEach-Object { Write-Host $_ }; Write-Host "FAILED: file-size limit"; exit 1 }

Write-Host "All checks passed."
