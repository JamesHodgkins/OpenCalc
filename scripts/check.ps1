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
# A coarse tripwire, not a benchmark: the budgets have roughly 500x headroom, so this
# only fails on a real regression such as an accidental quadratic loop (docs/PERFORMANCE.md).
Step "performance budgets (core)" { cargo bench -p opencalc-core --bench recalc -- --quick --check }

# File-size rule: about 400 production lines per file (lines before #[cfg(test)],
# or before a file-level #![cfg(test)] for files that are entirely tests). Files under
# a tests/ or benches/ directory are test code and are not counted.
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
    if ($rel -match '/(tests|benches)/') { return }
    $lines = Get-Content $_.FullName
    $test = ($lines | Select-String -Pattern '^#!?\[cfg\(test\)\]' | Select-Object -First 1).LineNumber
    $prod = if ($test) { $test - 1 } else { $lines.Count }
    $allowed = if ($exceptions.ContainsKey($rel)) { $exceptions[$rel] } else { $limit }
    if ($prod -gt $allowed) { $bad += "{0}: {1} production lines (allowed {2})" -f $rel, $prod, $allowed }
    elseif ($exceptions.ContainsKey($rel) -and $prod -le $limit) {
        $bad += "{0}: now {1} lines, within the limit: remove it from line_limit_exceptions.txt" -f $rel, $prod
    }
}
if ($bad.Count -gt 0) { $bad | ForEach-Object { Write-Host $_ }; Write-Host "FAILED: file-size limit"; exit 1 }

# Function-length rule: no production function is longer than 150 lines, counted from
# its `fn` line to its closing brace. Like the file rule, it covers production code
# only (lines before #[cfg(test)]; tests/ and benches/ are skipped). There is no
# exceptions list: split the function instead.
Write-Host "== function-length limit (150 lines)"
$fnLimit = 150
$long = @()
Get-ChildItem crates -Recurse -Filter *.rs | ForEach-Object {
    $rel = (Resolve-Path -Relative $_.FullName).TrimStart(".\").Replace("\", "/")
    if ($rel -match '/(tests|benches)/') { return }
    $lines = Get-Content $_.FullName
    $test = ($lines | Select-String -Pattern '^#!?\[cfg\(test\)\]' | Select-Object -First 1).LineNumber
    $end = if ($test) { $test - 1 } else { $lines.Count }
    for ($i = 0; $i -lt $end; $i++) {
        if ($lines[$i] -notmatch '^\s*(pub(\([a-z]+\))?\s+)?(const\s+)?(async\s+)?fn\s+\w+') { continue }
        $depth = 0; $started = $false; $j = $i
        while ($j -lt $end) {
            # Strings, character literals and line comments are blanked so their braces do not count.
            $l = $lines[$j] -replace '"([^"\\]|\\.)*"', '""' -replace "'.'", "''" -replace '//.*$', ''
            foreach ($ch in $l.ToCharArray()) {
                if ($ch -eq '{') { $depth++; $started = $true }
                elseif ($ch -eq '}') { $depth-- }
            }
            if ($started -and $depth -le 0) { break }
            if (-not $started -and $lines[$j] -match ';\s*$') { break }
            $j++
        }
        $len = $j - $i + 1
        if ($len -gt $fnLimit) { $long += "{0}:{1}: {2} lines (limit {3})" -f $rel, ($i + 1), $len, $fnLimit }
    }
}
if ($long.Count -gt 0) { $long | ForEach-Object { Write-Host $_ }; Write-Host "FAILED: function-length limit"; exit 1 }

Write-Host "All checks passed."
