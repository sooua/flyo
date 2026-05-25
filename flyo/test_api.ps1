# End-to-end test for the file API.
# Sets up an isolated temp share root, starts flyo against it, exercises every
# /api/* endpoint, then cleans up.

$ErrorActionPreference = 'Stop'
Get-Process flyo -ErrorAction SilentlyContinue | Stop-Process -Force

$tmp = Join-Path $env:TEMP "flyo-e2e-$(Get-Random)"
New-Item -ItemType Directory $tmp -Force | Out-Null
$share = Join-Path $tmp "share"
New-Item -ItemType Directory $share -Force | Out-Null

@"
Webd.Root ./share
Webd.Listen 39215
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

$base = 'http://127.0.0.1:39215'
$pass = 0
$fail = 0
function Assert($name, $cond) {
    if ($cond) { Write-Host "  PASS  $name" -ForegroundColor Green; $script:pass++ }
    else       { Write-Host "  FAIL  $name" -ForegroundColor Red;   $script:fail++ }
}
function Expect-Status($name, [int]$expected, [scriptblock]$call) {
    try {
        & $call | Out-Null
        Assert "$name -> $expected" $false
    } catch {
        $actual = $_.Exception.Response.StatusCode.value__
        Assert "$name -> $expected (got $actual)" ($actual -eq $expected)
    }
}

try {
    # --- Anonymous (guest) baseline ---
    $r = Invoke-RestMethod -Uri "$base/api/list?path=/" -Method Get -SessionVariable sGuest
    Assert "guest can list empty root" ($r.entries.Count -eq 0)

    Expect-Status "guest cannot upload" 403 {
        Invoke-RestMethod -Uri "$base/api/upload?path=/forbidden.txt" -Method Post `
            -Body 'x' -WebSession $sGuest
    }
    Expect-Status "guest cannot mkdir" 403 {
        Invoke-RestMethod -Uri "$base/api/mkdir?path=/forbid" -Method Post -WebSession $sGuest
    }
    Expect-Status "guest cannot delete" 403 {
        Invoke-RestMethod -Uri "$base/api/delete?path=/anything" -Method Post -WebSession $sGuest
    }

    # --- Login as admin ---
    Invoke-RestMethod -Uri "$base/api/login" -Method Post -ContentType 'application/json' `
        -Body '{"user":"admin","pass":"admin123"}' -SessionVariable s | Out-Null

    # --- Upload a file ---
    $payload = "hello, flyo world!"  # 18 bytes
    Invoke-RestMethod -Uri "$base/api/upload?path=/hello.txt" -Method Post `
        -Body $payload -WebSession $s | Out-Null
    Assert "uploaded file exists on disk" (Test-Path (Join-Path $share "hello.txt"))
    Assert "uploaded file has correct bytes" `
        ((Get-Content -Raw (Join-Path $share "hello.txt")) -eq $payload)

    # --- Listing reflects upload ---
    $r = Invoke-RestMethod -Uri "$base/api/list?path=/" -Method Get -WebSession $s
    Assert "listing has hello.txt" (@($r.entries | Where-Object name -eq 'hello.txt').Count -eq 1)
    $entry = $r.entries | Where-Object name -eq 'hello.txt'
    Assert "listing reports correct size (18)" ($entry.size -eq 18)
    Assert "listing reports not a directory" (-not $entry.is_dir)

    # --- Full download ---
    $body = (Invoke-WebRequest -Uri "$base/api/file?path=/hello.txt" -WebSession $s).Content
    Assert "GET full body matches" ($body -eq $payload)

    # --- Range download ---
    $resp = Invoke-WebRequest -Uri "$base/api/file?path=/hello.txt" -WebSession $s `
        -Headers @{Range='bytes=0-4'}
    Assert "partial GET status 206" ($resp.StatusCode -eq 206)
    Assert "partial GET body 'hello'" ($resp.Content -eq 'hello')
    Assert "partial GET has Content-Range" `
        ($resp.Headers['Content-Range'] -like 'bytes 0-4/18*')

    $resp = Invoke-WebRequest -Uri "$base/api/file?path=/hello.txt" -WebSession $s `
        -Headers @{Range='bytes=-6'}
    Assert "suffix range gives last 6 bytes" ($resp.Content -eq 'world!')

    # --- mkdir / rename / nested upload ---
    Invoke-RestMethod -Uri "$base/api/mkdir?path=/sub" -Method Post -WebSession $s | Out-Null
    Assert "mkdir created /sub" (Test-Path (Join-Path $share "sub") -PathType Container)

    Invoke-RestMethod -Uri "$base/api/upload?path=/sub/inner.bin" -Method Post `
        -Body ([byte[]](1,2,3,4,5,6,7,8)) -WebSession $s | Out-Null
    Assert "nested upload worked" (Test-Path (Join-Path $share "sub\inner.bin"))

    Invoke-RestMethod -Uri "$base/api/rename?from=/hello.txt&to=/renamed.txt" -Method Post `
        -WebSession $s | Out-Null
    Assert "rename removed source" (-not (Test-Path (Join-Path $share "hello.txt")))
    Assert "rename created destination" (Test-Path (Join-Path $share "renamed.txt"))

    # --- Delete moves into .Trash ---
    Invoke-RestMethod -Uri "$base/api/delete?path=/renamed.txt" -Method Post -WebSession $s `
        | Out-Null
    Assert "delete removed from root" (-not (Test-Path (Join-Path $share "renamed.txt")))
    $trash = Get-ChildItem (Join-Path $share ".Trash") -ErrorAction SilentlyContinue
    Assert "delete moved into .Trash" (@($trash).Count -ge 1)

    $rootList = Invoke-RestMethod -Uri "$base/api/list?path=/" -Method Get -WebSession $s
    Assert ".Trash never appears in listing" `
        (@($rootList.entries | Where-Object name -eq '.Trash').Count -eq 0)

    # --- Path traversal protection ---
    Expect-Status "rejects ../escape" 400 {
        Invoke-RestMethod -Uri "$base/api/list?path=/../" -Method Get -WebSession $s
    }
    Expect-Status "rejects absolute-windows-drive path" 400 {
        Invoke-RestMethod -Uri "$base/api/list?path=C:/Windows" -Method Get -WebSession $s
    }

    # --- Reader has 'l' but not 'u' or 'm' ---
    Invoke-RestMethod -Uri "$base/api/login" -Method Post -ContentType 'application/json' `
        -Body '{"user":"reader","pass":"reader"}' -SessionVariable sR | Out-Null
    Invoke-RestMethod -Uri "$base/api/list?path=/" -Method Get -WebSession $sR | Out-Null
    Assert "reader can list" $true
    Expect-Status "reader cannot upload" 403 {
        Invoke-RestMethod -Uri "$base/api/upload?path=/x.txt" -Method Post `
            -Body 'x' -WebSession $sR
    }
    Expect-Status "reader cannot delete" 403 {
        Invoke-RestMethod -Uri "$base/api/delete?path=/sub" -Method Post -WebSession $sR
    }
}
finally {
    if ($proc -and -not $proc.HasExited) { Stop-Process -Id $proc.Id -Force -EA SilentlyContinue }
    Get-Process flyo -ErrorAction SilentlyContinue | Stop-Process -Force
    Start-Sleep -Milliseconds 200
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}

Write-Host ""
if ($fail -eq 0) {
    Write-Host "All $pass API tests passed." -ForegroundColor Green
    exit 0
} else {
    Write-Host "$fail tests failed ($pass passed)." -ForegroundColor Red
    exit 1
}
