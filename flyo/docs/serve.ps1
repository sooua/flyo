# Local preview of the docs site (pure static files).
# Uses Python's built-in HTTP server. Opens the browser automatically.

$ErrorActionPreference = 'Stop'
$port = 39214

Set-Location $PSScriptRoot
Write-Host ""
Write-Host "  Docs preview: http://127.0.0.1:$port/index.html" -ForegroundColor Cyan
Write-Host "  Ctrl+C to stop." -ForegroundColor DarkGray
Write-Host ""

Start-Process "http://127.0.0.1:$port/index.html"
python -m http.server $port --bind 127.0.0.1
