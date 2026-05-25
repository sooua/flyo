"""Concurrency smoke test against a running flyo.

Fires:
  - 50 parallel GET /api/list — proves the server doesn't deadlock under load
  - 10 parallel POST /api/upload with unique payloads — proves atomic-rename
    isolation: no truncated files, no name collisions, every byte that was
    sent is on disk.

Run from any cwd; the script expects a flyo bound to 127.0.0.1:39218 with a
share directory pointed at via env var FLYO_SHARE (so we can verify what
ended up on disk).
"""

from __future__ import annotations

import hashlib
import os
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
from typing import Tuple
from urllib import request, parse

BASE = os.environ.get("FLYO_BASE", "http://127.0.0.1:39218")
SHARE = Path(os.environ["FLYO_SHARE"])
USER = "admin"
PASS = "admin123"


def login() -> str:
    """Return the session cookie value after a successful login."""
    body = b'{"user":"%s","pass":"%s"}' % (USER.encode(), PASS.encode())
    req = request.Request(f"{BASE}/api/login", data=body, method="POST",
                          headers={"Content-Type": "application/json"})
    with request.urlopen(req) as r:
        for cookie in r.headers.get_all("Set-Cookie") or []:
            if cookie.startswith("flyo_sid="):
                return cookie.split(";", 1)[0]
    raise RuntimeError("no flyo_sid cookie returned")


def get(url: str, cookie: str) -> Tuple[int, bytes]:
    req = request.Request(url, headers={"Cookie": cookie})
    with request.urlopen(req) as r:
        return r.status, r.read()


def post(url: str, payload: bytes, cookie: str) -> int:
    req = request.Request(url, data=payload, method="POST",
                          headers={"Cookie": cookie,
                                   "Content-Type": "application/octet-stream"})
    with request.urlopen(req) as r:
        return r.status


def list_dir(cookie: str) -> int:
    status, _ = get(f"{BASE}/api/list?path=/", cookie)
    return status


def upload_unique(idx: int, cookie: str) -> Tuple[int, str, str]:
    name = f"conc-{idx:03d}.bin"
    payload = (f"payload-{idx}-".encode() * 100)[:1024]  # ~1 KB, deterministic
    digest = hashlib.sha256(payload).hexdigest()
    encoded = parse.quote(name)
    status = post(f"{BASE}/api/upload?path=/{encoded}", payload, cookie)
    return status, name, digest


def main() -> int:
    cookie = login()

    pass_n = 0
    fail_n = 0

    def report(name: str, ok: bool) -> None:
        nonlocal pass_n, fail_n
        if ok:
            print(f"  PASS  {name}")
            pass_n += 1
        else:
            print(f"  FAIL  {name}")
            fail_n += 1

    # ---- 50 parallel listings ----
    print("[Concurrent listings × 50]")
    with ThreadPoolExecutor(max_workers=32) as ex:
        results = list(ex.map(lambda _: list_dir(cookie), range(50)))
    report("All 50 list calls returned 200", all(s == 200 for s in results))

    # ---- 10 parallel uploads ----
    print("[Concurrent uploads × 10]")
    with ThreadPoolExecutor(max_workers=10) as ex:
        outcomes = list(ex.map(lambda i: upload_unique(i, cookie), range(10)))

    all_ok_status = all(s == 200 for s, _, _ in outcomes)
    report("All 10 upload calls returned 200", all_ok_status)

    # Every file landed on disk
    all_on_disk = all((SHARE / name).is_file() for _, name, _ in outcomes)
    report("All 10 files exist on disk", all_on_disk)

    # Every file's bytes match the digest we computed before sending
    all_bytes_ok = True
    for _, name, digest in outcomes:
        p = SHARE / name
        if not p.is_file():
            all_bytes_ok = False
            break
        actual = hashlib.sha256(p.read_bytes()).hexdigest()
        if actual != digest:
            all_bytes_ok = False
            print(f"  MISMATCH  {name}: expected {digest[:12]}, got {actual[:12]}")
            break
    report("All 10 files have byte-perfect content (atomic rename held)", all_bytes_ok)

    # No half-uploaded temp files left over
    leftovers = [p.name for p in SHARE.iterdir() if "flyo-tmp-" in p.name]
    report("No half-uploaded *.flyo-tmp-* files left behind", not leftovers)

    print()
    if fail_n == 0:
        print(f"All {pass_n} concurrency tests passed.")
        return 0
    print(f"{fail_n} concurrency tests failed ({pass_n} passed).")
    return 1


if __name__ == "__main__":
    sys.exit(main())
