# Master test runner — runs every test suite and prints a single summary.
#
# Order is deliberate: cheap & fast first (unit tests), then e2e, then smoke
# tests that depend on the built binary. A failure anywhere is recorded but
# we keep going so the engineer sees the full picture, not just the first
# breakage.

$ErrorActionPreference = 'Continue'
$root = Split-Path -Parent $PSCommandPath
Set-Location $root

$started = Get-Date
$results = @()

function Run-Suite($name, $command) {
    Write-Host ""
    Write-Host ("=" * 72) -ForegroundColor DarkGray
    Write-Host "  $name" -ForegroundColor Cyan
    Write-Host ("=" * 72) -ForegroundColor DarkGray
    $t0 = Get-Date
    & $command
    $code = $LASTEXITCODE
    $dt = (Get-Date) - $t0
    $script:results += [pscustomobject]@{
        Suite    = $name
        Status   = if ($code -eq 0) { 'PASS' } else { 'FAIL' }
        Duration = "{0:F1}s" -f $dt.TotalSeconds
        Code     = $code
    }
}

# ---- 1. Build first so everything has fresh artefacts ----
Run-Suite "BUILD · frontend (pnpm)" {
    Push-Location "$root\flyo\web"
    pnpm build 2>&1 | Select-Object -Last 4
    Pop-Location
}
Run-Suite "BUILD · cargo workspace (release)" {
    cargo build --release 2>&1 | Select-Object -Last 4
}

# ---- 2. Unit tests ----
Run-Suite "UNIT · flyo (cargo test)" {
    cargo test --release -p flyo 2>&1 | Select-String -Pattern "test result|FAILED|panicked" |
        Select-Object -Last 5
}
Run-Suite "UNIT · flyo-proxy (cargo test)" {
    cargo test --release -p flyo-proxy 2>&1 | Select-String -Pattern "test result|FAILED|panicked" |
        Select-Object -Last 5
}

# ---- 3. End-to-end against the built binary ----
Run-Suite "E2E · auth (cookie session, login/logout)" {
    & "$root\flyo\test_auth.ps1" | Select-Object -Last 2
}
Run-Suite "E2E · file API (list/upload/range/rename/delete)" {
    & "$root\flyo\test_api.ps1" | Select-Object -Last 2
}
Run-Suite "E2E · flyo-proxy (HTTPS, security headers, forwarding)" {
    & "$root\flyo-proxy\test_proxy.ps1" | Select-Object -Last 2
}

# ---- 4. New smoke tests ----
Run-Suite "SMOKE · UI bundle + range edges + special chars" {
    & "$root\flyo\test_smoke.ps1" | Select-Object -Last 2
}
Run-Suite "SMOKE · concurrent listings + uploads (atomic rename)" {
    & "$root\flyo\test_concurrent.ps1" | Select-Object -Last 2
}
Run-Suite "SMOKE · docs site (4 pages, internal links)" {
    & "$root\flyo\test_docs_smoke.ps1" | Select-Object -Last 2
}
Run-Suite "SMOKE · binary integrity (PE metadata, embedded icon)" {
    & "$root\flyo\test_binary.ps1" | Select-Object -Last 2
}

# ---- Summary ----
$elapsed = (Get-Date) - $started
Write-Host ""
Write-Host ("=" * 72) -ForegroundColor White
Write-Host ("  SUMMARY  ({0:F1}s total)" -f $elapsed.TotalSeconds) -ForegroundColor White
Write-Host ("=" * 72) -ForegroundColor White
$results | Format-Table -AutoSize Suite, Status, Duration | Out-Host

$failed = ($results | Where-Object Status -eq 'FAIL').Count
$passed = ($results | Where-Object Status -eq 'PASS').Count
$total  = $results.Count

if ($failed -eq 0) {
    Write-Host ("✓ ALL {0} SUITES PASSED" -f $total) -ForegroundColor Green
    exit 0
} else {
    Write-Host ("✗ {0}/{1} SUITES FAILED" -f $failed, $total) -ForegroundColor Red
    exit 1
}
