"""Capture exact local source/log hashes. Does not use credentials or network."""
from datetime import datetime, timedelta, timezone
import hashlib
import json
from pathlib import Path
import re
import subprocess

EVIDENCE = Path(__file__).resolve().parent
PROJECT = EVIDENCE.parents[2]
SOURCES = [
    "apps/gateway/Cargo.toml", "apps/gateway/src/lib.rs",
    "apps/gateway/src/fiat_billing.rs", "apps/gateway/src/waffo_billing.rs",
    "apps/gateway/src/waffo_billing_protocol.rs", "apps/gateway/src/waffo_billing_tests.rs",
    "apps/gateway/src/stripe_billing.rs", "apps/gateway/src/web_billing.rs",
    "apps/gateway/src/web_billing_tests.rs", "apps/simulation/src/activities.rs",
    "apps/simulation/src/billing.rs", "apps/simulation/src/billing_provenance.rs",
    "apps/simulation/src/config.rs", "apps/simulation/src/config_schema_compat_tests.rs",
    "apps/simulation/src/procurement_fees.rs", "apps/simulation/src/promotion/attribution_migration.rs",
    "apps/simulation/tests/promotion_postgres.rs", "apps/simulation/tests/billing_providers.rs",
    "apps/simulation/tests/waffo_restore.rs", "apps/game-client/client-bevy/src/native_billing.rs",
    "apps/game-client/platform-windows/src/native_protocol.rs",
    "apps/game-client/platform-windows/src/billing_browser.rs",
    "apps/game-client/platform-windows/src/shell_bridge.rs",
    "apps/game-client/platform-windows/src/gateway.rs",
    "apps/recharge-web/public/core.js", "apps/recharge-web/public/app.js",
    "apps/recharge-web/public/index.html", "apps/recharge-web/public/styles.css",
    "apps/recharge-web/public/locales.js", "apps/recharge-web/public/activities.js",
    "apps/recharge-web/server/deploy.py", "apps/recharge-web/tests/core.test.mjs",
    "apps/recharge-web/tests/activities.test.mjs", "apps/recharge-web/tests/payment-channels.test.mjs",
    "apps/recharge-web/tests/test_deploy.py", "config/waffo-billing.env.example",
]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def contents(name):
    return (EVIDENCE / name).read_text(encoding="utf-8-sig", errors="replace")


def rust_results(name):
    matches = re.findall(r"test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored", contents(name))
    assert matches and all(result == "ok" and int(failed) == 0 for result, _, failed, _ in matches), name
    return [{"result": result, "passed": int(passed), "failed": int(failed), "ignored": int(ignored)}
            for result, passed, failed, ignored in matches]


def git(*args):
    return subprocess.check_output(["git", *args], cwd=PROJECT, text=True).strip()


simulation = rust_results("simulation-tests.log")
gateway = rust_results("gateway-billing-tests.log")
assert sum(row["passed"] for row in simulation) == 64
assert sum(row["passed"] for row in gateway) == 80
backend_log = contents("backend-all-targets-check.log")
assert "Finished `dev`" in backend_log and not re.search(r"^error(?:\[|:)", backend_log, re.M)
assert "# pass 35" in contents("portal-tests.log") and "# fail 0" in contents("portal-tests.log")
assert "Ran 22 tests" in contents("proxy-tests.log") and "OK" in contents("proxy-tests.log")
native = json.loads(contents("native-validation.json"))
host = json.loads(contents("native-host-validation.json"))
assert host["hostCompilePassed"] and host["temporaryLockRestored"]
assert host["originalLockSha256"] == host["finalLockSha256"]
assert digest(PROJECT / "apps/game-client/platform-windows/Cargo.lock") == host["originalLockSha256"]
assert all(digest(PROJECT / path) == expected for path, expected in native["sourceSha256"].items())

record = {
    "schema": "mir2.waffo.local-validation.v1",
    "capturedAt": datetime.now(timezone(timedelta(hours=8))).isoformat(),
    "branch": git("branch", "--show-current"),
    "baseHead": git("rev-parse", "HEAD"),
    "sourceState": "shared uncommitted working tree; hashes include retained pre-existing WIP",
    "sourceSha256": {path: digest(PROJECT / path) for path in SOURCES},
    "checks": {
        "simulation": simulation,
        "gatewayBilling": gateway,
        "waffoRestore": rust_results("waffo-restore-after-fix.log"),
        "paidHistoryRestore": rust_results("paid-history-restore-regression.log"),
        "ordinaryRestore": rust_results("ordinary-backup-restore-regression.log"),
        "backendAllTargetsLocked": True,
        "portalNodePassed": 35, "proxyPythonPassed": 22,
        "sharedNativePassed": native["sharedNativeLogic"]["passed"],
        "nativeTransportPassed": native["nativeTransportAndBrowser"]["passed"],
        "nativeHostTemporaryLockPassed": host["hostCompilePassed"],
        "nativeGpu": native["nativeGpu"],
    },
    "logSha256": {path.name: digest(path) for path in sorted(EVIDENCE.glob("*.log"))},
    "packageRevision": None,
    "publicDeploymentRevision": None,
    "realWaffoMerchantAcceptance": False,
    "waffoEnabledByDefault": False,
}
(EVIDENCE / "validation.json").write_text(json.dumps(record, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
print(json.dumps({"baseHead": record["baseHead"], "sourceFiles": len(SOURCES), "logs": len(record["logSha256"]),
                  "gatewayPassed": 80, "simulationPassed": 64, "nativeHostPassed": True}))
