# Docs site smoke test — serve docs/ via python http.server, verify each page
# responds 200 with the right content-type and that internal links resolve.

$ErrorActionPreference = 'Stop'
Get-Process python -EA SilentlyContinue | Stop-Process -Force

$docsDir = Join-Path $PSScriptRoot "docs"
$port = 39233
$py = Start-Process "python" -ArgumentList "-m","http.server",$port,"--bind","127.0.0.1" `
    -WorkingDirectory $docsDir -PassThru -NoNewWindow `
    -RedirectStandardOutput "$env:TEMP\pyout" -RedirectStandardError "$env:TEMP\pyerr"
Start-Sleep 1

$base = "http://127.0.0.1:$port"
$pass = 0
$fail = 0
function Assert($name, $cond) {
    if ($cond) { Write-Host "  PASS  $name" -ForegroundColor Green; $script:pass++ }
    else       { Write-Host "  FAIL  $name" -ForegroundColor Red;   $script:fail++ }
}

try {
    foreach ($page in @("index.html", "install.html", "config.html", "api.html")) {
        $r = Invoke-WebRequest "$base/$page" -UseBasicParsing
        Assert "GET /$page returns 200"            ($r.StatusCode -eq 200)
        Assert "GET /$page content-type is HTML"   ($r.Headers['Content-Type'] -like '*text/html*')
        Assert "GET /$page body looks substantive" ($r.RawContentLength -gt 4000)

        # Each page must contain the new stack favicon path
        Assert "/$page embeds stack favicon SVG"   ($r.Content -like '*fill-opacity*')

        # Each page must reference docs.css
        Assert "/$page references docs.css"        ($r.Content -like '*docs.css*')
    }

    # docs.css 200 with the right MIME
    $css = Invoke-WebRequest "$base/docs.css" -UseBasicParsing
    Assert "GET /docs.css returns 200"             ($css.StatusCode -eq 200)
    Assert "GET /docs.css is text/css"             ($css.Headers['Content-Type'] -like '*text/css*')
    Assert "docs.css contains stack-logo SVG"      ($css.Content -like '*fill-opacity*')

    # Internal cross-links resolve (install ↔ config ↔ api are linked from each)
    $idx = Invoke-WebRequest "$base/index.html" -UseBasicParsing
    foreach ($target in @("install.html", "config.html", "api.html")) {
        Assert "index.html links to $target"       ($idx.Content -like "*$target*")
    }
}
finally {
    Stop-Process -Id $py.Id -Force -EA SilentlyContinue
}

Write-Host ""
if ($fail -eq 0) {
    Write-Host "All $pass docs smoke tests passed." -ForegroundColor Green
    exit 0
} else {
    Write-Host "$fail docs smoke tests failed ($pass passed)." -ForegroundColor Red
    exit 1
}
