# Binary-level integrity checks: PE size budgets, embedded version metadata,
# extractable icon resource.

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing

$wsRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$flyo   = Join-Path $wsRoot "target\release\flyo.exe"
$proxy  = Join-Path $wsRoot "target\release\flyo-proxy.exe"

$pass = 0
$fail = 0
function Assert($name, $cond) {
    if ($cond) { Write-Host "  PASS  $name" -ForegroundColor Green; $script:pass++ }
    else       { Write-Host "  FAIL  $name" -ForegroundColor Red;   $script:fail++ }
}

# ---- flyo.exe ----
Write-Host "[flyo.exe]" -ForegroundColor Cyan
Assert "exists" (Test-Path $flyo)
if (Test-Path $flyo) {
    $size = (Get-Item $flyo).Length
    $kb = [Math]::Round($size / 1024, 1)
    Write-Host "       size: $kb KB"
    Assert "size < 10 MB budget"               ($size -lt 10MB)
    Assert "size > 1 MB (sanity)"              ($size -gt 1MB)

    $vi = (Get-Item $flyo).VersionInfo
    Assert "ProductName == 'Flyo'"             ($vi.ProductName -eq 'Flyo')
    Assert "FileDescription set"               ($vi.FileDescription -like 'Flyo*')
    Assert "CompanyName == 'Flyo'"             ($vi.CompanyName -eq 'Flyo')
    Assert "LegalCopyright contains MIT"       ($vi.LegalCopyright -like '*MIT*')

    # Extract the embedded icon — proves the ICO resource is intact
    try {
        $icon = [System.Drawing.Icon]::ExtractAssociatedIcon($flyo)
        Assert "Embedded icon extracts"        ($null -ne $icon)
        if ($icon) {
            Assert "Icon dimensions plausible" ($icon.Width -ge 16 -and $icon.Height -ge 16)
        }
    } catch {
        Assert "Embedded icon extracts" $false
    }
}

# ---- flyo-proxy.exe ----
Write-Host "[flyo-proxy.exe]" -ForegroundColor Cyan
Assert "exists" (Test-Path $proxy)
if (Test-Path $proxy) {
    $psize = (Get-Item $proxy).Length
    $pkb = [Math]::Round($psize / 1024, 1)
    Write-Host "       size: $pkb KB"
    Assert "proxy size < 10 MB"                ($psize -lt 10MB)
    Assert "proxy size > 1 MB"                 ($psize -gt 1MB)
}

# ---- bundle size sanity (embedded UI) ----
Write-Host "[UI bundle (built dist)]" -ForegroundColor Cyan
$dist = Join-Path $PSScriptRoot "web\dist"
if (Test-Path $dist) {
    $jsFiles  = Get-ChildItem -Recurse $dist -Filter "*.js"
    $cssFiles = Get-ChildItem -Recurse $dist -Filter "*.css"
    $htmlFiles = Get-ChildItem -Recurse $dist -Filter "*.html"

    Assert "exactly one index.html"            ($htmlFiles.Count -eq 1)
    Assert "exactly one JS chunk"              ($jsFiles.Count -eq 1)
    Assert "exactly one CSS chunk"             ($cssFiles.Count -eq 1)

    $jsSize  = ($jsFiles  | Measure-Object Length -Sum).Sum
    $cssSize = ($cssFiles | Measure-Object Length -Sum).Sum
    Write-Host ("       JS: {0:F1} KB  CSS: {1:F1} KB" -f ($jsSize/1024), ($cssSize/1024))
    Assert "JS bundle < 100 KB"                ($jsSize -lt 100KB)
    Assert "CSS bundle < 50 KB"                ($cssSize -lt 50KB)
}

# ---- favicon ICO sanity ----
Write-Host "[favicon ICO]" -ForegroundColor Cyan
$ico = Join-Path $PSScriptRoot "assets\flyo.ico"
Assert "assets/flyo.ico exists"                (Test-Path $ico)
if (Test-Path $ico) {
    $icoSize = (Get-Item $ico).Length
    Assert "ICO size > 1 KB (multi-resolution)" ($icoSize -gt 1KB)
    Assert "ICO size < 20 KB"                  ($icoSize -lt 20KB)
}

Write-Host ""
if ($fail -eq 0) {
    Write-Host "All $pass binary tests passed." -ForegroundColor Green
    exit 0
} else {
    Write-Host "$fail binary tests failed ($pass passed)." -ForegroundColor Red
    exit 1
}
