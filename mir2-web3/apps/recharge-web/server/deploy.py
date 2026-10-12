#!/usr/bin/env python3
"""Root operator for the static recharge site; never edits or restarts Gateway.

Upload the seven allowlisted public files, release.json, caddy.snippet and this
operator to a new /home/ubuntu/numeron-recharge-* directory first. Secrets and
the full Caddy configuration never appear in output. --apply is a separate
explicit step after the candidate validates. Ordinary player QA follows.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import time
import urllib.request

NAMES = {"index.html", "styles.css", "mark.svg", "app.js", "core.js", "locales.js", "activities.js"}
PUBLIC_LINE = b"            @mir2_playtest_public path /ws /health /spectator/matches /spectator/ws /ai-live/status /v1/billing/return /v1/billing/stripe/webhook"
PREVIOUS_API_ADDITION = b" /v1/billing/status /v1/billing/checkout"
API_ADDITION = PREVIOUS_API_ADDITION + b" /v1/identity/sessions/revoke"
ACTIVITY_API_ADDITION = API_ADDITION + b" /v1/activities/status /v1/activities/claim"
MARKER = b"    # BEGIN isolated Mir2 invited playtest 20260929"
ROOT = Path("/srv/mir2-recharge")
CADDY = Path("/etc/caddy/Caddyfile")

def need(value, code):
    if not value:
        raise RuntimeError(code)

def sha(data):
    return hashlib.sha256(data).hexdigest()

def run(args, env=None):
    result = subprocess.run(args, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                            env=env, timeout=30)
    need(result.returncode == 0, "operatorCommandFailed:" + args[0])
    return result.stdout

def unit(name):
    raw = run(["systemctl", "show", name, "-p", "MainPID", "-p", "NRestarts", "-p", "Result"]).decode()
    return dict(line.split("=", 1) for line in raw.splitlines())

def health(port):
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    with opener.open("http://127.0.0.1:%d/health" % port, timeout=5) as response:
        return json.load(response)

def snapshot():
    original_pids = re.findall(r"pid=(\d+)", run(["ss", "-ltnpH", "sport = :7110"]).decode())
    need(len(original_pids) == 1, "originalRealmPidAmbiguous")
    return {"playtest": unit("mir2-playtest.service"), "originalPids": original_pids,
            "caddy": unit("caddy.service"), "gatewayRevision": health(7210)["revision"],
            "gatewayEnvSha256": sha(Path("/etc/mir2-playtest/gateway.env").read_bytes())}

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--staging", required=True)
    parser.add_argument("--release-sha256", required=True)
    parser.add_argument("--expected-caddy-sha256", required=True)
    parser.add_argument("--expected-gateway-revision", required=True)
    parser.add_argument("--expected-gateway-pid", required=True)
    parser.add_argument("--expected-site-sha256")
    parser.add_argument("--enable-logout-route", action="store_true")
    parser.add_argument("--enable-activities-routes", action="store_true")
    parser.add_argument("--apply", action="store_true")
    args = parser.parse_args()
    need(not (args.enable_logout_route and args.enable_activities_routes), "routeUpgradeAmbiguous")
    stage = Path(args.staging)
    need(stage.resolve() == stage and stage.parent == Path("/home/ubuntu")
         and stage.name.startswith("numeron-recharge-"), "stagingPathRejected")
    original = CADDY.read_bytes()
    need(sha(original) == args.expected_caddy_sha256, "caddyCompareAndSwapRejected")
    before = snapshot()
    need(before["gatewayRevision"] == args.expected_gateway_revision
         and before["playtest"]["MainPID"] == args.expected_gateway_pid, "gatewayChangedBeforeDeployment")
    need(health(7110)["ok"] is True, "originalRealmNotHealthy")
    manifest_bytes = (stage / "release.json").read_bytes()
    need(sha(manifest_bytes) == args.release_sha256, "releaseManifestHashMismatch")
    manifest = json.loads(manifest_bytes)
    need(manifest["schemaVersion"] == 1 and manifest["application"] == "numeron-recharge-web"
         and len(manifest["files"]) == len(NAMES)
         and {file["name"] for file in manifest["files"]} == NAMES, "releaseManifestRejected")
    for file in manifest["files"]:
        path = stage / file["name"]
        need(path.is_file() and not path.is_symlink(), "stagedFileRejected")
        data = path.read_bytes()
        need(len(data) == file["bytes"] and sha(data) == file["sha256"], "stagedHashMismatch")
    snippet = (stage / "caddy.snippet").read_bytes()
    need(b"# BEGIN NUMERON RECHARGE WEB V1" in snippet
         and b"# END NUMERON RECHARGE WEB V1" in snippet
         and b"/srv/mir2-recharge/current" in snippet, "snippetRejected")
    previous_site = None
    if args.expected_site_sha256:
        need(re.fullmatch(r"[0-9a-f]{64}", args.expected_site_sha256), "previousSiteHashInvalid")
        previous_site = ROOT / "releases" / args.expected_site_sha256
        need((ROOT / "current").is_symlink() and (ROOT / "current").resolve() == previous_site
             and sha((previous_site / "release.json").read_bytes()) == args.expected_site_sha256,
             "previousSiteCompareAndSwapRejected")
        need(original.count(snippet) == 1, "existingSiteRoutesChanged")
        if args.enable_logout_route:
            need(original.count(PUBLIC_LINE + PREVIOUS_API_ADDITION + b"\n") == 1,
                 "previousAuthenticatedRoutesNotExpected")
            new = original.replace(PUBLIC_LINE + PREVIOUS_API_ADDITION + b"\n",
                                   PUBLIC_LINE + API_ADDITION + b"\n", 1)
        elif args.enable_activities_routes:
            need(original.count(PUBLIC_LINE + API_ADDITION + b"\n") == 1,
                 "previousAuthenticatedRoutesNotExpected")
            new = original.replace(PUBLIC_LINE + API_ADDITION + b"\n",
                                   PUBLIC_LINE + ACTIVITY_API_ADDITION + b"\n", 1)
            need(new.replace(PUBLIC_LINE + ACTIVITY_API_ADDITION + b"\n",
                             PUBLIC_LINE + API_ADDITION + b"\n", 1) == original,
                 "unrelatedRouteBytesChanged")
        else:
            need(original.count(PUBLIC_LINE + ACTIVITY_API_ADDITION + b"\n") == 1,
                 "existingSiteRoutesChanged")
            new = original
    else:
        need(original.count(PUBLIC_LINE + b"\n") == 1 and original.count(MARKER) == 1
             and b"# BEGIN NUMERON RECHARGE WEB" not in original, "existingRoutesNotExpected")
        new = original.replace(PUBLIC_LINE + b"\n", PUBLIC_LINE + ACTIVITY_API_ADDITION + b"\n", 1)
        new = new.replace(MARKER, b"\n" + snippet + b"\n" + MARKER, 1)
        # Prove all pre-existing route bytes, including private headers, remain exact.
        restored = new.replace(b"\n" + snippet + b"\n", b"", 1).replace(PUBLIC_LINE + ACTIVITY_API_ADDITION, PUBLIC_LINE, 1)
        need(restored == original, "unrelatedRouteBytesChanged")
    env = dict(os.environ)
    process_env = Path("/proc/" + before["caddy"]["MainPID"] + "/environ").read_bytes()
    for entry in process_env.split(b"\0"):
        if b"=" in entry:
            key, value = entry.split(b"=", 1)
            env[key.decode()] = value.decode()
    descriptor, candidate_name = tempfile.mkstemp(prefix=".recharge-candidate-", dir="/etc/caddy")
    candidate = Path(candidate_name)
    try:
        with os.fdopen(descriptor, "wb") as stream:
            stream.write(new)
            stream.flush()
            os.fsync(stream.fileno())
        run(["caddy", "validate", "--config", str(candidate), "--adapter", "caddyfile"], env)
        receipt = {"ok": True, "applied": False, "releaseSha256": args.release_sha256,
                   "caddyBeforeSha256": sha(original), "caddyAfterSha256": sha(new),
                   "before": before, "files": len(NAMES),
                   "bytes": sum(file["bytes"] for file in manifest["files"])}
        if args.apply:
            release = ROOT / "releases" / args.release_sha256
            need(not release.exists() and (previous_site is not None or not (ROOT / "current").exists()), "existingSiteNotExpected")
            backup = Path("/var/backups") / ("mir2-recharge-" + time.strftime("%Y%m%d-%H%M%S", time.gmtime()))
            backup.mkdir(mode=0o700)
            (backup / "Caddyfile").write_bytes(original)
            (backup / "before.json").write_text(json.dumps(before))
            release.mkdir(mode=0o755, parents=True)
            for name in NAMES | {"release.json"}:
                shutil.copyfile(stage / name, release / name)
                (release / name).chmod(0o644)
            link = ROOT / ".current-candidate"
            need(not link.exists() and not link.is_symlink(), "candidateLinkAlreadyExists")
            link.symlink_to(release)
            need(CADDY.read_bytes() == original and snapshot() == before, "stateChangedBeforeActivation")
            if previous_site:
                need((ROOT / "current").resolve() == previous_site, "siteChangedBeforeActivation")
            metadata = CADDY.stat()
            candidate.chmod(metadata.st_mode & 0o777)
            os.chown(candidate, metadata.st_uid, metadata.st_gid)
            link.replace(ROOT / "current")
            if new != original:
                candidate.replace(CADDY)
            try:
                if new != original:
                    run(["systemctl", "reload", "caddy.service"])
                need(health(7210)["ok"] and health(7110)["ok"], "realmHealthAfterReloadFailed")
                after = snapshot()
                need(after == before, "realmPidOrConfigurationChanged")
                receipt.update(applied=True, after=after, backup=str(backup))
                (backup / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
            except Exception:
                # Restore this proxy configuration only if we still own its bytes.
                # Never restore a database, game config, game binary or player save.
                if new != original and sha(CADDY.read_bytes()) == sha(new):
                    restore = CADDY.with_name(".recharge-restore")
                    restore.write_bytes(original)
                    restore.chmod(metadata.st_mode & 0o777)
                    os.chown(restore, metadata.st_uid, metadata.st_gid)
                    restore.replace(CADDY)
                    run(["systemctl", "reload", "caddy.service"])
                if previous_site and (ROOT / "current").resolve() == release:
                    restore_link = ROOT / ".current-restore"
                    restore_link.symlink_to(previous_site)
                    restore_link.replace(ROOT / "current")
                raise
        print(json.dumps(receipt, sort_keys=True))
    finally:
        if candidate.exists():
            candidate.unlink()

if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        # Command output/Caddy contents may contain private route headers.
        print(json.dumps({"ok": False, "error": str(error) if isinstance(error, RuntimeError) else type(error).__name__}))
        raise SystemExit(1)
