#!/usr/bin/env python3
"""Read-only final service/feed check using an existing trusted SSH helper."""
import argparse
import datetime
import json
from pathlib import Path
import subprocess
import sys
from acceptance import http, need, sha


def main(args):
    applied = json.loads(args.applied_receipt.read_text(encoding="utf-8-sig"))
    need(applied["ok"] and applied["applied"] and applied["before"] == applied["after"], "qualifiedApplyRequired")
    result = subprocess.run([sys.executable, str(args.remote_helper), "--user", "ubuntu",
                             "--command-file", str(args.lifecycle_file), "--timeout", "30"],
                            capture_output=True, text=True, timeout=45)
    need(result.returncode == 0, "trustedLifecycleReadFailed")
    state = json.loads(result.stdout)
    before = applied["before"]
    need(state["ok"] and state["test"]["ok"] and state["test"]["revision"] == before["gatewayRevision"]
         and state["unit"]["MainPID"] == before["playtest"]["MainPID"]
         and state["unit"]["NRestarts"] == "0" and state["original"]["pids"] == before["originalPids"]
         and state["original"]["ok"] and state["tcp7200"] == 0
         and state["files"]["/etc/mir2-playtest/gateway.env"]["sha256"] == before["gatewayEnvSha256"],
         "servicesChangedSinceWebsiteApply")
    capacity = state["test"]["capacity"]
    need(all(capacity[k] == 0 for k in ("currentActiveSessions", "currentLoginInFlight",
         "currentNewCharacterInFlight", "currentReconnectLeases", "currentStartGameInFlight", "currentWsConnections")),
         "ordinaryQaCleanupIncomplete")
    checks = []
    for host in ("https://165.154.65.136.sslip.io", "https://assets.mir2.obelisk.build"):
        for suffix, pin in (("latest.json", args.native_manifest_sha256), ("latest.p7s", args.native_cms_sha256)):
            url = host + "/client-updates/" + suffix
            code, raw, _ = http(url)
            need(code == 200 and sha(raw) == pin, "signedNativeFeedChanged")
            if suffix == "latest.json":
                need(json.loads(raw)["sequence"] == args.native_sequence, "nativeFeedSequenceMismatch")
            checks.append({"url": url, "status": code, "sha256": sha(raw), "bytes": len(raw)})
    report = {"schema": "mir2.recharge-web.postflight.v1", "passed": True,
        "observedAtUtc": datetime.datetime.now(datetime.timezone.utc).isoformat(), "tlsVerified": True,
        "observedNativeFeedSequence": args.native_sequence, "nativeOriginCdnByteMatch": True,
        "websiteModifiedNativeFeed": False,
        "services": state, "publicChecks": checks, "websiteApply": applied,
        "gameGatewayRestartedByWebsite": False, "originalRealmRestartedByWebsite": False}
    args.report.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8", newline="\n")
    print(json.dumps({"passed": True, "gatewayRevision": state["test"]["revision"],
                     "gatewayPid": state["unit"]["MainPID"], "capacity": capacity,
                     "observedNativeFeedSequence": args.native_sequence, "publicChecks": len(checks)}))


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    for name in ("applied-receipt", "remote-helper", "lifecycle-file", "report"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--native-manifest-sha256", required=True)
    parser.add_argument("--native-cms-sha256", required=True)
    parser.add_argument("--native-sequence", required=True, type=int)
    try:
        main(parser.parse_args())
    except Exception as error:
        from acceptance import Stop
        print(json.dumps({"passed": False, "error": str(error) if isinstance(error, Stop) else type(error).__name__}))
        raise SystemExit(1)
