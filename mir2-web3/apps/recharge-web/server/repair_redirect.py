#!/usr/bin/env python3
"""One-time guarded repair for V1's ambiguous relative redir argument.

Uses the deployed operator's private diagnostics. Changes exactly one line
inside our own snippet, hot reloads Caddy and never restarts a game service.
"""
import argparse
import json
import os
from pathlib import Path
import tempfile
import time
from deploy import CADDY, ROOT, need, run, sha, snapshot


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--expected-caddy-sha256", required=True)
    parser.add_argument("--expected-site-sha256", required=True)
    parser.add_argument("--expected-gateway-revision", required=True)
    parser.add_argument("--expected-gateway-pid", required=True)
    parser.add_argument("--apply", action="store_true")
    args = parser.parse_args()
    original = CADDY.read_bytes()
    current = ROOT / "current"
    release = ROOT / "releases" / args.expected_site_sha256
    need(sha(original) == args.expected_caddy_sha256 and current.is_symlink()
         and current.resolve() == release
         and sha((release / "release.json").read_bytes()) == args.expected_site_sha256,
         "configurationCompareAndSwapRejected")
    fixed = Path(__file__).with_name("caddy.snippet").read_bytes()
    broken = fixed.replace(b"redir * /recharge/ 308", b"redir /recharge/ 308")
    need(fixed != broken and original.count(broken) == 1, "oldOwnedSnippetNotExpected")
    new = original.replace(broken, fixed, 1)
    before = snapshot()
    need(before["gatewayRevision"] == args.expected_gateway_revision
         and before["playtest"]["MainPID"] == args.expected_gateway_pid, "gatewayChanged")
    env = dict(os.environ)
    for entry in Path("/proc/" + before["caddy"]["MainPID"] + "/environ").read_bytes().split(b"\0"):
        if b"=" in entry:
            key, value = entry.split(b"=", 1)
            env[key.decode()] = value.decode()
    descriptor, filename = tempfile.mkstemp(prefix=".recharge-redirect-", dir="/etc/caddy")
    candidate = Path(filename)
    try:
        with os.fdopen(descriptor, "wb") as stream:
            stream.write(new)
            stream.flush()
            os.fsync(stream.fileno())
        metadata = CADDY.stat()
        candidate.chmod(metadata.st_mode & 0o777)
        os.chown(candidate, metadata.st_uid, metadata.st_gid)
        run(["caddy", "validate", "--config", str(candidate), "--adapter", "caddyfile"], env)
        result = {"ok": True, "applied": False, "before": before,
                  "caddyBeforeSha256": sha(original), "caddyAfterSha256": sha(new)}
        if args.apply:
            backup = Path("/var/backups") / ("mir2-recharge-redirect-" + time.strftime("%Y%m%d-%H%M%S", time.gmtime()))
            backup.mkdir(mode=0o700)
            (backup / "Caddyfile").write_bytes(original)
            need(CADDY.read_bytes() == original and snapshot() == before
                 and current.resolve() == release, "stateChangedBeforeRepair")
            candidate.replace(CADDY)
            try:
                run(["systemctl", "reload", "caddy.service"])
                after = snapshot()
                need(before == after, "serviceOrConfigurationChanged")
                result.update(applied=True, after=after, backup=str(backup))
                (backup / "receipt.json").write_text(json.dumps(result) + "\n")
            except Exception:
                if CADDY.read_bytes() == new:
                    restore = CADDY.with_name(".recharge-redirect-restore")
                    restore.write_bytes(original)
                    restore.chmod(metadata.st_mode & 0o777)
                    os.chown(restore, metadata.st_uid, metadata.st_gid)
                    restore.replace(CADDY)
                    run(["systemctl", "reload", "caddy.service"])
                raise
        print(json.dumps(result))
    finally:
        if candidate.exists():
            candidate.unlink()


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(json.dumps({"ok": False, "error": str(error) if isinstance(error, RuntimeError) else type(error).__name__}))
        raise SystemExit(1)
