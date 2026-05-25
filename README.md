# Flyn — flyo workspace

[![CI](https://github.com/webd90kb/webd/actions/workflows/ci.yml/badge.svg)](https://github.com/webd90kb/webd/actions/workflows/ci.yml)
[![Release](https://github.com/webd90kb/webd/actions/workflows/release.yml/badge.svg)](https://github.com/webd90kb/webd/actions/workflows/release.yml)

A Rust workspace containing:

- [`flyo/`](flyo/) — lightweight self-hosted file server (a modern rewrite of
  [webd](https://github.com/webd90kb/webd)). Single ~2 MB binary with embedded
  modern UI. **What you actually run.**
- [`flyo-proxy/`](flyo-proxy/) — optional HTTPS / IP allowlist / rate-limit
  reverse proxy that sits in front of flyo. Only needed for public-internet
  deployments.
- [`flyo/docs/`](flyo/docs/) — pure-static marketing + reference site
  (auto-deployed to GitHub Pages on push to main).
- `design/` — Pencil-exported screenshots of the UI used as references during
  rewrites. (Git-ignored; regeneratable.)

## Build everything

```powershell
# Frontend first (embeds into flyo binary)
pnpm --dir flyo/web install
pnpm --dir flyo/web build

# Then the Rust workspace
cargo build --release
# → target/release/flyo.exe
# → target/release/flyo-proxy.exe
```

## Quick demo (flyo alone)

```powershell
cd flyo
.\demo.ps1
```

Opens `http://127.0.0.1:39212` with admin/admin123 + reader/reader + guest
seeded.

## Quick demo (flyo + proxy with self-signed HTTPS)

```powershell
cd flyo
.\demo.ps1                                  # terminal A
```

```powershell
cd flyo-proxy
@'
Proxy.Listen   127.0.0.1:8443
Proxy.Upstream http://127.0.0.1:39212
Proxy.SelfSigned localhost
'@ | Set-Content flyo-proxy.conf
..\target\release\flyo-proxy.exe            # terminal B
```

Then `https://localhost:8443/` (accept the self-signed cert prompt).

## Test suites

| What                  | How                                  | Assertions |
|-----------------------|--------------------------------------|-----------:|
| flyo unit             | `cargo test -p flyo --release`       | 29 |
| flyo-proxy unit       | `cargo test -p flyo-proxy --release` | 11 |
| flyo auth e2e         | `flyo\test_auth.ps1`                 | 14 |
| flyo file API e2e     | `flyo\test_api.ps1`                  | 26 |
| flyo-proxy e2e        | `flyo-proxy\test_proxy.ps1`          | 10 |
| flyo UI/range smoke   | `flyo\test_smoke.ps1`                | 33 |
| flyo concurrency      | `flyo\test_concurrent.ps1`           |  5 |
| flyo docs smoke       | `flyo\test_docs_smoke.ps1`           | 26 |
| flyo binary integrity | `flyo\test_binary.ps1`               | 20 |
| **Total**             |                                      | **174** |

Run **everything** at once with a single coloured summary:

```powershell
.\run_all_tests.ps1
```

## Continuous Integration

| Workflow | Trigger | What it does |
|---|---|---|
| [`ci.yml`](.github/workflows/ci.yml) | every push / PR | Full test suite on Windows + Linux. Cross-builds for Windows MSVC, Linux x64/ARM64 (musl), macOS x64/ARM64. OpenWrt mipsel built best-effort. |
| [`release.yml`](.github/workflows/release.yml) | tag matching `v*` | Same build matrix, plus auto-publishes a GitHub Release with sha256-summed archives for every target. |
| [`docs.yml`](.github/workflows/docs.yml) | push touching `flyo/docs/**` | Deploys the static docs site to GitHub Pages. |

## Project status

- ✅ Workspace + 2 crates building from one root
- ✅ Modern Notion-inspired UI with MDI icon set, i18n (en/zh), light/dark themes
- ✅ Documentation site (4 pages, GitHub-Pages friendly)
- ✅ Optional reverse proxy (HTTPS / IP rules / rate limit / security headers)
- ✅ Custom .exe icon + PE metadata (Windows)
- ✅ Cross-platform CI matrix (Windows / Linux x64 / Linux ARM / macOS / OpenWrt mipsel)
- ✅ 174-assertion test pipeline across unit / e2e / smoke / concurrency / binary integrity
