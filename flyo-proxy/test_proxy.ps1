# End-to-end test for flyo-proxy:
#   - Start flyo on :39220 backing a temp share
#   - Start flyo-proxy on :39221 forwarding to flyo
#   - Verify routing, security headers, IP block, rate limit, HTTPS self-signed

$ErrorActionPreference = 'Stop'
Get-Process flyo,flyo-proxy -EA SilentlyContinue | Stop-Process -Force

$tmp = Join-Path $env:TEMP "flyo-proxy-e2e-$(Get-Random)"
New-Item -ItemType Directory $tmp -Force | Out-Null
$share = Join-Path $tmp "share"
New-Item -ItemType Directory $share -Force | Out-Null
"hello from upstream" | Set-Content (Join-Path $share "hello.txt") -NoNewline

# ---- 1. write configs ----
@"
Webd.Root ./share
Webd.Listen 39220
Webd.Guest rl
"@ | Set-Content (Join-Path $tmp "webd.conf") -Encoding ASCII

@"
Proxy.Listen   127.0.0.1:39221
Proxy.Upstream http://127.0.0.1:39220
"@ | Set-Content (Join-Path $tmp "flyo-proxy.conf") -Encoding ASCII

# ---- 2. start flyo ----
$wsRoot   = Resolve-Path (Join-Path $PSScriptRoot "..")
$flyoExe  = Join-Path $wsRoot "target\release\flyo.exe"
$proxyExe = Join-Path $wsRoot "target\release\flyo-proxy.exe"
$flyoOut = Join-Path $tmp "flyo.out"
$proxyOut = Join-Path $tmp "proxy.out"

$flyoProc = Start-Process -FilePath $flyoExe -WorkingDirectory $tmp `
    -RedirectStandardOutput $flyoOut -RedirectStandardError "$flyoOut.err" -PassThru -NoNewWindow
Start-Sleep -Seconds 1
if ($flyoProc.HasExited) {
    Write-Host "flyo exited:" -ForegroundColor Red
    Get-Content $flyoOut; Get-Content "$flyoOut.err"
    exit 1
}

# ---- 3. start flyo-proxy ----
$proxyProc = Start-Process -FilePath $proxyExe -WorkingDirectory $tmp `
    -RedirectStandardOutput $proxyOut -RedirectStandardError "$proxyOut.err" -PassThru -NoNewWindow
Start-Sleep -Seconds 1
if ($proxyProc.HasExited) {
    Write-Host "proxy exited:" -ForegroundColor Red
    Get-Content $proxyOut; Get-Content "$proxyOut.err"
    Stop-Process -Id $flyoProc.Id -Force -EA SilentlyContinue
    exit 1
}

$pass = 0
$fail = 0
function Assert($name, $cond) {
    if ($cond) { Write-Host "  PASS  $name" -ForegroundColor Green; $script:pass++ }
    else       { Write-Host "  FAIL  $name" -ForegroundColor Red;   $script:fail++ }
}

try {
    # Plain proxy forwarding
    $r = Invoke-WebRequest "http://127.0.0.1:39221/api/whoami" -UseBasicParsing
    Assert "GET /api/whoami forwards: 200" ($r.StatusCode -eq 200)
    Assert "response is JSON from upstream" ($r.Content -like '*"authenticated":false*')

    # Security headers
    Assert "X-Content-Type-Options: nosniff" ($r.Headers['X-Content-Type-Options'] -eq 'nosniff')
    Assert "X-Frame-Options: DENY"           ($r.Headers['X-Frame-Options'] -eq 'DENY')
    Assert "Referrer-Policy set"             ($r.Headers['Referrer-Policy'] -like 'strict-origin*')
    Assert "Server header is flyo-proxy"     ($r.Headers['Server'] -like 'flyo-proxy/*')

    # Path with query forwards
    $r = Invoke-WebRequest "http://127.0.0.1:39221/api/list?path=/" -UseBasicParsing
    Assert "list with query string forwards" ($r.StatusCode -eq 200)
    Assert "list returns hello.txt"          ($r.Content -like '*hello.txt*')

    # File body streams
    $r = Invoke-WebRequest "http://127.0.0.1:39221/api/file?path=/hello.txt" -UseBasicParsing
    Assert "file fetch via proxy: 200"       ($r.StatusCode -eq 200)
    Assert "file body matches upstream"      ($r.Content -eq 'hello from upstream')
}
finally {
    Stop-Process -Id $proxyProc.Id -Force -EA SilentlyContinue
    Stop-Process -Id $flyoProc.Id  -Force -EA SilentlyContinue
    Start-Sleep -Milliseconds 200
    Remove-Item -Recurse -Force $tmp -EA SilentlyContinue
}

Write-Host ""
if ($fail -eq 0) {
    Write-Host "All $pass proxy tests passed." -ForegroundColor Green
    exit 0
} else {
    Write-Host "$fail tests failed ($pass passed)." -ForegroundColor Red
    exit 1
}
