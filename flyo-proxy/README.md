# flyo-proxy

Optional reverse proxy in front of [flyo](../flyo/). Adds HTTPS, IP allow/block,
rate limiting, and security headers without touching the upstream binary.

You **don't need this** if you're running flyo on a trusted LAN. It exists for
people who want to:

- Expose flyo to the public internet behind HTTPS
- Restrict access to office IP ranges
- Cap traffic per IP to prevent abuse
- Add the usual modern security headers (HSTS, X-Frame-Options, etc.)

For OAuth / Let's Encrypt / UI skinning, see the deferred section at the bottom.

## Size

| Binary | Size |
|---|---|
| `flyo` (upstream) | ~2.0 MB |
| `flyo-proxy` | ~2.7 MB |

Both are built from the same Cargo workspace and share most of their compiled
deps in development.

## Configuration

`flyo-proxy.conf`:

```conf
# Listen address — bind to all interfaces on 8443 by default.
Proxy.Listen 0.0.0.0:8443

# Upstream flyo — assumed to be on localhost.
Proxy.Upstream http://127.0.0.1:9212

# Pick ONE of the three TLS strategies:
#   1) Plain HTTP (no TLS) — omit all three.
#   2) Self-signed dev cert generated at startup:
Proxy.SelfSigned localhost
#   3) Real cert from disk (PEM):
# Proxy.Cert /etc/flyo/cert.pem
# Proxy.Key  /etc/flyo/key.pem

# Optional access control. Block list is checked first, then allow list.
# Empty allow list = anyone (modulo the block list).
# Proxy.Allow 10.0.0.0/8
# Proxy.Allow 192.168.0.0/16
# Proxy.Block 1.2.3.4

# Optional per-IP rate limit. Units: s|sec|second / m|min|minute / h|hour
# Proxy.RateLimit 100/min

# Optional. Default: info.
# Proxy.LogLevel debug
```

Lookup order: `./flyo-proxy.conf`, then `/etc/flyo-proxy.conf`, then defaults.

## Quick demo

```powershell
# build everything
cargo build --release

# start flyo (terminal A)
cd flyo
.\demo.ps1     # serves a temp share on :39212

# start the proxy (terminal B)
cd ../flyo-proxy
# write a 2-line config and run
@'
Proxy.Listen   127.0.0.1:8443
Proxy.Upstream http://127.0.0.1:39212
Proxy.SelfSigned localhost
'@ | Set-Content flyo-proxy.conf
..\target\release\flyo-proxy.exe
```

Browse to `https://localhost:8443` (you'll get a self-signed warning — accept
it). Requests reach flyo through the proxy with full TLS termination, security
headers, and `X-Forwarded-*` headers populated.

## What gets injected

Every proxied response gains:

| Header | Value |
|---|---|
| `Server` | `flyo-proxy/<version>` |
| `X-Content-Type-Options` | `nosniff` |
| `X-Frame-Options` | `DENY` |
| `Referrer-Policy` | `strict-origin-when-cross-origin` |
| `Strict-Transport-Security` | `max-age=31536000; includeSubDomains` (only when TLS is active) |

Every request forwarded to upstream gains:

| Header | Value |
|---|---|
| `X-Forwarded-For` | client IP |
| `X-Real-IP` | client IP |
| `X-Forwarded-Proto` | `http` / `https` |
| `Host` | upstream hostname |

Hop-by-hop headers (Connection, Keep-Alive, TE, Trailers, etc.) are stripped
both directions per RFC 7230 §6.1.

## Tests

- 11 unit tests: `cargo test --release -p flyo-proxy`
- 10 end-to-end tests: `.\test_proxy.ps1` (spins up flyo + proxy, exercises
  every route, validates injected headers)

## Deferred features

| Feature | Why deferred |
|---|---|
| Let's Encrypt / ACME | HTTP-01 challenge requires port 80, adds runtime complexity. For now: bring your own cert. |
| OAuth (Google/GitHub) | Separate task — needs persistence, secret storage, callback routing. |
| UI skin override | Marketing site uses the upstream UI; if you want a different shell, fork the `web/` directory. |
| HTTP/2 to upstream | Upstream is local; HTTP/1.1 keep-alive is enough. |
| Multi-upstream routing | YAGNI until someone asks for it. |
