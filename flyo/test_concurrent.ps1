# Boot flyo, run the Python concurrency test against it, tear down.

$ErrorActionPreference = 'Stop'
Get-Process flyo -EA SilentlyContinue | Stop-Process -Force

$tmp = Join-Path $env:TEMP "flyo-conc-$(Get-Random)"
New-Item -ItemType Directory $tmp -Force | Out-Null
$share = Join-Path $tmp "share"
New-Item -ItemType Directory $share -Force | Out-Null

@"
Webd.Root ./share
Webd.Listen 39218
Webd.User rlumS admin admin123
Webd.Guest rl
"@ | Set-Content (Join-Path $tmp "webd.conf") -Encoding ASCII

$exe = Join-Path $PSScriptRoot "..\target\release\flyo.exe"
$proc = Start-Process -FilePath $exe -WorkingDirectory $tmp `
    -RedirectStandardOutput (Join-Path $tmp "out") -RedirectStandardError (Join-Path $tmp "err") `
    -PassThru -NoNewWindow
Start-Sleep -Seconds 1
if ($proc.HasExited) {
    Write-Host "flyo failed to start" -ForegroundColor Red
    Get-Content (Join-Path $tmp "out"); Get-Content (Join-Path $tmp "err")
    exit 1
}

$env:FLYO_BASE  = 'http://127.0.0.1:39218'
$env:FLYO_SHARE = $share

try {
    python (Join-Path $PSScriptRoot "tools\concurrent_test.py")
    $code = $LASTEXITCODE
} finally {
    if ($proc -and -not $proc.HasExited) { Stop-Process -Id $proc.Id -Force -EA SilentlyContinue }
    Start-Sleep -Milliseconds 200
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
    Remove-Item env:\FLYO_BASE  -EA SilentlyContinue
    Remove-Item env:\FLYO_SHARE -EA SilentlyContinue
}

exit $code
