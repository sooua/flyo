# Smoke test for the deployed flyo binary:
#   - Embedded UI bundle loads (HTML, CSS, JS, hashed-asset caching)
#   - SPA fallback for unknown paths
#   - Favicon data URI contains the stack logo paths
#   - Range edge cases (single byte, suffix, prefix at file boundary)
#   - Special-character filenames (space, Unicode, emoji)
#   - New file endpoint (zero-byte file via upload)

$ErrorActionPreference = 'Stop'
Get-Process flyo -ErrorAction SilentlyContinue | Stop-Process -Force

$tmp = Join-Path $env:TEMP "flyo-smoke-$(Get-Random)"
New-Item -ItemType Directory $tmp -Force | Out-Null
$share = Join-Path $tmp "share"
New-Item -ItemType Directory $share -Force | Out-Null

@"
Webd.Root ./share
Webd.Listen 39217
Webd.User rlumS admin admin123
Webd.Guest rl
"@ | Set-Content (Join-Path $tmp "webd.conf") -Encoding ASCII

$exe = Join-Path $PSScriptRoot "..\target\release\flyo.exe"
$proc = Start-Process -FilePath $exe -WorkingDirectory $tmp `
    -RedirectStandardOutput (Join-Path $tmp "out") -RedirectStandardError (Join-Path $tmp "err") `
    -PassThru -NoNewWindow
Start-Sleep -Seconds 1
if ($proc.HasExited) {
    Write-Host "flyo failed to start:" -ForegroundColor Red
    Get-Content (Join-Path $tmp "out"); Get-Content (Join-Path $tmp "err")
    exit 1
}

$base = 'http://127.0.0.1:39217'
$pass = 0
$fail = 0
function Assert($name, $cond) {
    if ($cond) { Write-Host "  PASS  $name" -ForegroundColor Green; $script:pass++ }
    else       { Write-Host "  FAIL  $name" -ForegroundColor Red;   $script:fail++ }
}

try {
    # ============ 1. UI bundle integrity ============
    Write-Host "[UI bundle]" -ForegroundColor Cyan

    $root = Invoke-WebRequest "$base/" -UseBasicParsing
    Assert "GET / returns 200"                ($root.StatusCode -eq 200)
    Assert "GET / content-type text/html"     ($root.Headers['Content-Type'] -like '*text/html*')
    Assert "GET / cache-control no-cache"     ($root.Headers['Cache-Control'] -like '*no-cache*')
    Assert "Root HTML references Inter font"  ($root.Content -like '*fonts.googleapis.com*Inter*')
    Assert "Favicon data URI is stack logo"   ($root.Content -like '*fill-opacity*')

    # Extract hashed asset paths from the inlined script + link tags
    $cssMatch = [regex]::Match($root.Content, '/assets/(index-[A-Za-z0-9_-]+)\.css')
    $jsMatch  = [regex]::Match($root.Content, '/assets/(index-[A-Za-z0-9_-]+)\.js')
    Assert "Index references hashed CSS asset" ($cssMatch.Success)
    Assert "Index references hashed JS asset"  ($jsMatch.Success)

    $css = Invoke-WebRequest "$base/assets/$($cssMatch.Groups[1].Value).css" -UseBasicParsing
    $js  = Invoke-WebRequest "$base/assets/$($jsMatch.Groups[1].Value).js"  -UseBasicParsing
    Assert "Hashed CSS asset 200"             ($css.StatusCode -eq 200)
    Assert "Hashed CSS asset Content-Type"    ($css.Headers['Content-Type'] -like '*text/css*')
    Assert "Hashed CSS immutable cache"       ($css.Headers['Cache-Control'] -like '*immutable*')
    Assert "Hashed JS asset 200"              ($js.StatusCode -eq 200)
    Assert "Hashed JS asset Content-Type"     ($js.Headers['Content-Type'] -like '*javascript*')
    Assert "JS bundle contains 'flyo'"        ($js.Content -like '*flyo*')

    # ============ 2. SPA fallback ============
    Write-Host "[SPA fallback]" -ForegroundColor Cyan
    $spa = Invoke-WebRequest "$base/some/deep/spa/route" -UseBasicParsing
    Assert "Unknown path returns 200"          ($spa.StatusCode -eq 200)
    Assert "Unknown path returns index HTML"   ($spa.RawContentLength -eq $root.RawContentLength)

    # ============ 3. Login session ============
    Invoke-RestMethod "$base/api/login" -Method Post -ContentType 'application/json' `
        -Body '{"user":"admin","pass":"admin123"}' -SessionVariable s | Out-Null

    # ============ 4. Range edge cases ============
    Write-Host "[Range edge cases]" -ForegroundColor Cyan
    # Upload a 100-byte known body
    $body = -join (0..99 | ForEach-Object { '#' })  # 100 hash chars
    Invoke-RestMethod "$base/api/upload?path=/probe.txt" -Method Post -Body $body -WebSession $s | Out-Null

    $r = Invoke-WebRequest "$base/api/file?path=/probe.txt" -WebSession $s -Headers @{Range='bytes=0-0'}
    Assert "Single-byte range -> 206"          ($r.StatusCode -eq 206)
    Assert "Single-byte range body length 1"   ($r.Content.Length -eq 1)

    $r = Invoke-WebRequest "$base/api/file?path=/probe.txt" -WebSession $s -Headers @{Range='bytes=-10'}
    Assert "Suffix range -10 returns 10 bytes" ($r.Content.Length -eq 10)

    $r = Invoke-WebRequest "$base/api/file?path=/probe.txt" -WebSession $s -Headers @{Range='bytes=50-'}
    Assert "Open range 50- returns 50 bytes"   ($r.Content.Length -eq 50)

    try {
        Invoke-WebRequest "$base/api/file?path=/probe.txt" -WebSession $s -Headers @{Range='bytes=200-300'} | Out-Null
        Assert "Out-of-range returns 416" $false
    } catch {
        Assert "Out-of-range returns 416"      ($_.Exception.Response.StatusCode.value__ -eq 416)
    }

    # ============ 5. Special-character filenames ============
    # Use curl.exe to bypass PowerShell's tendency to re-encode % in URLs.
    Write-Host "[Special filenames]" -ForegroundColor Cyan
    $sid = ($s.Cookies.GetCookies($base) | Where-Object Name -eq 'flyo_sid').Value
    foreach ($name in @("file with space.txt", "中文文件.md", "emoji-🎉.bin")) {
        $payload  = "payload for $name"
        $encoded  = [System.Uri]::EscapeDataString($name)
        $bodyFile = New-TemporaryFile
        [System.IO.File]::WriteAllText($bodyFile.FullName, $payload, [System.Text.UTF8Encoding]::new($false))
        $up = curl.exe -s -o NUL -w "%{http_code}" -X POST -b "flyo_sid=$sid" `
            --data-binary "@$($bodyFile.FullName)" `
            "$base/api/upload?path=/$encoded"
        Remove-Item $bodyFile.FullName -EA SilentlyContinue
        Assert "POST upload of '$name' -> 200"     ($up -eq '200')
        $diskPath = Join-Path $share $name
        Assert "'$name' present on disk"           (Test-Path -LiteralPath $diskPath)
        $down = curl.exe -s -b "flyo_sid=$sid" "$base/api/file?path=/$encoded"
        Assert "GET of '$name' round-trips bytes"  ($down -eq $payload)
    }

    $list = Invoke-RestMethod "$base/api/list?path=/" -WebSession $s
    Assert "Listing includes spaced filename"  (($list.entries | Where-Object name -eq 'file with space.txt').Count -eq 1)
    Assert "Listing includes Chinese filename" (($list.entries | Where-Object name -eq '中文文件.md').Count -eq 1)

    # ============ 6. New file creation (empty body upload) ============
    Write-Host "[Empty file creation]" -ForegroundColor Cyan
    Invoke-RestMethod "$base/api/upload?path=/empty.txt" -Method Post -Body '' -WebSession $s -ContentType 'application/octet-stream' | Out-Null
    $emptyPath = Join-Path $share "empty.txt"
    Assert "Empty file exists on disk"         (Test-Path $emptyPath)
    Assert "Empty file is 0 bytes"             ((Get-Item $emptyPath).Length -eq 0)
}
finally {
    if ($proc -and -not $proc.HasExited) { Stop-Process -Id $proc.Id -Force -EA SilentlyContinue }
    Start-Sleep -Milliseconds 200
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}

Write-Host ""
if ($fail -eq 0) {
    Write-Host "All $pass smoke tests passed." -ForegroundColor Green
    exit 0
} else {
    Write-Host "$fail smoke tests failed ($pass passed)." -ForegroundColor Red
    exit 1
}
