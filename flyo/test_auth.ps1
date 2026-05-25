# End-to-end auth smoke test.
# Sets up an isolated temp share root, starts flyo, exercises auth endpoints, cleans up.

$ErrorActionPreference = 'Stop'
Get-Process flyo -ErrorAction SilentlyContinue | Stop-Process -Force

$tmp = Join-Path $env:TEMP "flyo-auth-e2e-$(Get-Random)"
New-Item -ItemType Directory $tmp -Force | Out-Null
New-Item -ItemType Directory (Join-Path $tmp "share") -Force | Out-Null

@"
Webd.Root ./share
Webd.Listen 39213
Webd.User rlumS admin admin123
Webd.User rl reader reader
Webd.Guest rl
"@ | Set-Content (Join-Path $tmp "webd.conf") -Encoding ASCII

$exe = Join-Path $PSScriptRoot "..\target\release\flyo.exe"
$logFile = Join-Path $tmp "flyo.log"
$proc = Start-Process -FilePath $exe -WorkingDirectory $tmp `
    -RedirectStandardOutput $logFile -RedirectStandardError "$logFile.err" `
    -PassThru -NoNewWindow
Start-Sleep -Seconds 1
if ($proc.HasExited) {
    Write-Host "flyo exited early. Logs:" -ForegroundColor Red
    if (Test-Path $logFile)        { Get-Content $logFile }
    if (Test-Path "$logFile.err")  { Get-Content "$logFile.err" }
    exit 1
}

$base = 'http://127.0.0.1:39213'
$pass = 0
$fail = 0
function Assert($name, $cond) {
    if ($cond) { Write-Host "  PASS  $name" -ForegroundColor Green; $script:pass++ }
    else       { Write-Host "  FAIL  $name" -ForegroundColor Red;   $script:fail++ }
}

try {
    # 1. Anonymous whoami → guest
    $r = Invoke-RestMethod -Uri "$base/api/whoami" -Method Get -SessionVariable s1
    Assert "anon whoami: authenticated=false" (-not $r.authenticated)
    Assert "anon whoami: perms.access=true (guest rl)" $r.perms.access
    Assert "anon whoami: perms.upload=false" (-not $r.perms.upload)

    # 2. Wrong password → 401
    try {
        Invoke-RestMethod -Uri "$base/api/login" -Method Post -ContentType 'application/json' `
            -Body '{"user":"admin","pass":"wrong"}' -SessionVariable s2 | Out-Null
        Assert "wrong pass returns 401" $false
    } catch {
        Assert "wrong pass returns 401" ($_.Exception.Response.StatusCode.value__ -eq 401)
    }

    # 3. Correct credentials → 200 + session cookie
    $r = Invoke-RestMethod -Uri "$base/api/login" -Method Post -ContentType 'application/json' `
        -Body '{"user":"admin","pass":"admin123"}' -SessionVariable s3
    Assert "login succeeds" ($r.authenticated -eq $true)
    Assert "login returns admin perms.upload=true" $r.perms.upload
    Assert "login returns admin perms.modify=true" $r.perms.modify
    Assert "session cookie present" ($s3.Cookies.GetCookies($base) | Where-Object Name -eq 'flyo_sid')

    # 4. whoami with session → admin
    $r = Invoke-RestMethod -Uri "$base/api/whoami" -Method Get -WebSession $s3
    Assert "authed whoami: user=admin" ($r.user -eq 'admin')
    Assert "authed whoami: perms.modify=true" $r.perms.modify

    # 5. logout → clears cookie + future whoami is guest
    Invoke-RestMethod -Uri "$base/api/logout" -Method Post -WebSession $s3 | Out-Null
    $r = Invoke-RestMethod -Uri "$base/api/whoami" -Method Get -WebSession $s3
    Assert "after logout: authenticated=false" (-not $r.authenticated)

    # 6. Read-only user
    $r = Invoke-RestMethod -Uri "$base/api/login" -Method Post -ContentType 'application/json' `
        -Body '{"user":"reader","pass":"reader"}' -SessionVariable s4
    Assert "reader login OK" ($r.user -eq 'reader')
    Assert "reader has no upload perm" (-not $r.perms.upload)
    Assert "reader has list perm" $r.perms.list
}
finally {
    if ($proc -and -not $proc.HasExited) { Stop-Process -Id $proc.Id -Force -EA SilentlyContinue }
    Get-Process flyo -ErrorAction SilentlyContinue | Stop-Process -Force
    Start-Sleep -Milliseconds 200
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}

Write-Host ""
if ($fail -eq 0) {
    Write-Host "All $pass auth tests passed." -ForegroundColor Green
    exit 0
} else {
    Write-Host "$fail tests failed ($pass passed)." -ForegroundColor Red
    exit 1
}
