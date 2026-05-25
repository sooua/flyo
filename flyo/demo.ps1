# One-click demo: start flyo against a tempdir share, open the browser.
# Ctrl+C to stop. The tempdir is cleaned up on exit.

$ErrorActionPreference = 'Stop'
Get-Process flyo -ErrorAction SilentlyContinue | Stop-Process -Force

$port = 39212
$tmp = Join-Path $env:TEMP "flyo-demo"
if (Test-Path $tmp) { Remove-Item -Recurse -Force $tmp }
New-Item -ItemType Directory $tmp -Force | Out-Null
$share = Join-Path $tmp "share"
New-Item -ItemType Directory $share -Force | Out-Null

# Seed with some demo content so the empty state is informative.
Set-Content (Join-Path $share "README.txt") "Welcome to flyo!`nDrop files anywhere on this page to upload."
New-Item -ItemType Directory (Join-Path $share "Pictures") -Force | Out-Null
New-Item -ItemType Directory (Join-Path $share "Videos") -Force | Out-Null
"sample data" | Set-Content (Join-Path $share "Pictures\.placeholder")
"sample" | Set-Content (Join-Path $share "Videos\.placeholder")

@"
Webd.Root ./share
Webd.Listen $port
Webd.User rlumS admin admin123
Webd.User rl  reader reader
Webd.Guest rl
"@ | Set-Content (Join-Path $tmp "webd.conf") -Encoding ASCII

Write-Host ""
Write-Host "  flyo demo:           http://127.0.0.1:$port/" -ForegroundColor Cyan
Write-Host "  admin login:         admin / admin123 (full permissions)"
Write-Host "  reader login:        reader / reader  (list + download only)"
Write-Host "  guest (no login):    list + download only"
Write-Host ""
Write-Host "  Share folder:        $share"
Write-Host "  Press Ctrl+C to stop."
Write-Host ""

Start-Process "http://127.0.0.1:$port/"

try {
    Set-Location $tmp
    & (Join-Path $PSScriptRoot "..\target\release\flyo.exe")
} finally {
    Get-Process flyo -ErrorAction SilentlyContinue | Stop-Process -Force
    Set-Location $PSScriptRoot
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
    Write-Host "Cleaned up." -ForegroundColor DarkGray
}
