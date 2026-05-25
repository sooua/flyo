# Flyo

A Rust rewrite of [webd](https://github.com/webd90kb/webd) — lightweight self-hosted web file server with a modern embedded UI.

## Status

| Phase | Status |
|---|---|
| Scaffold + size baseline | ✅ |
| `webd.conf` compatible config | ✅ |
| Cookie-session auth | ✅ |
| Core file API (list / file+Range / upload / mkdir / rename / delete) | ✅ |
| Modern embedded UI (Preact + Vite) | ✅ |
| Cross-platform CI | pending |
| Documentation site rewrite | ✅ |
| Optional [`flyo-proxy`](../flyo-proxy/) (HTTPS / IP rules / rate limit) | ✅ |

## Quick demo

From inside the `flyo/` directory:

```powershell
.\demo.ps1
```

This launches flyo against a temp share folder, seeds it with some sample
content, and opens `http://127.0.0.1:39212/` in your default browser. Try:

| User    | Password   | Permissions                        |
|---------|------------|------------------------------------|
| admin   | admin123   | `rlumS` — full (upload, delete, rename) |
| reader  | reader     | `rl`    — list + download           |
| guest   | (no login) | `rl`    — list + download           |

## Size measurements

| Binary | Size | Notes |
|---|---|---|
| `webd.exe` (original C) | 88.5 KB | Reference |
| `flyo.exe` (full features + embedded UI) | **2.0 MB** | ~23× webd, ~20% of 10 MB budget |

UI bundle (gzipped): index.html 0.5 KB + CSS 2.9 KB + JS 13.5 KB = **~17 KB total**

## Build from source

Flyo is part of the [Flyn workspace](../). Build from the workspace root:

```powershell
# 1. Build the frontend
pnpm --dir flyo/web install
pnpm --dir flyo/web build

# 2. Build the binary (embeds web/dist/ at compile time)
cargo build --release

# 3. The artifact
.\target\release\flyo.exe         # one level up from this README
```

## Configuration

Flyo searches, in order:

1. `./flyo.conf`
2. `./webd.conf`  *(drop-in compatible with the original webd format)*
3. `/etc/flyo.conf`
4. `/etc/webd.conf`

Supported directives (case-insensitive):

```conf
Webd.Root     /path/to/share
Webd.Listen   9212                 # port only → 0.0.0.0:9212
Webd.Listen   [::]:9212            # dual-stack
Webd.User     rlumS admin pass     # perm-tag user pass  (max 3 users)
Webd.User     rl    reader pass
Webd.Guest    rl                   # or 0 to fully disable guest
Webd.Hide                          # tray icon hide (Windows)
Webd.Browser  "C:\Path\to\firefox.exe"
```

Permission tags:

| Tag | Meaning |
|---|---|
| `r` | access (download) |
| `l` | list directories |
| `u` | upload + mkdir |
| `m` | delete / move / rename |
| `S` | show hidden (`.`-prefixed) files |
| `T` | use media player web page (`/.player.htm`) |
| `D` | force `Content-Disposition: attachment` |

## HTTP API

| Method | Path | Perms | Notes |
|---|---|---|---|
| GET  | `/api/whoami` | none | returns `{authenticated, user, perms}` |
| POST | `/api/login`  | none | JSON body `{user, pass}`; sets `flyo_sid` cookie |
| POST | `/api/logout` | session | clears cookie and all sessions for the user |
| GET  | `/api/list?path=` | `l` | JSON list of entries |
| GET  | `/api/file?path=` | `r` | Range-aware download |
| POST | `/api/upload?path=` | `u` | streaming body → atomic write |
| POST | `/api/mkdir?path=` | `u` | create directory |
| POST | `/api/rename?from=&to=` | `m` | move / rename |
| POST | `/api/delete?path=` | `m` | move into `.Trash/` with timestamp prefix |
| GET  | `/__health` | none | liveness probe |
| GET  | `/__config` | none | current effective config (debug) |

## Tests

- 29 cargo unit tests: `cargo test --release`
- 14 auth e2e tests: `.\test_auth.ps1`
- 26 file API e2e tests: `.\test_api.ps1`

## Frontend dev

```powershell
pnpm --dir web dev
# Vite serves on :5173 with /api/* proxied to localhost:9215 (start flyo there).
```

## Docs site

The marketing + reference site lives in [`docs/`](docs/) — pure static HTML/CSS,
no build step, GitHub-Pages and Vercel friendly. Pages:

- `index.html` — Hero, features, comparison vs webd
- `install.html` — Per-platform install guide (Win / Linux / macOS / OpenWrt / Android)
- `config.html` — `webd.conf` directive reference
- `api.html` — HTTP API reference

Preview locally (needs Python 3):

```powershell
.\docs\serve.ps1
```

## Roadmap

- Cross-platform CI (Windows / Linux x64 / Linux ARM / OpenWrt mipsel)
- OAuth / ACME inside flyo-proxy (deferred — see [`flyo-proxy/README.md`](../flyo-proxy/README.md))
