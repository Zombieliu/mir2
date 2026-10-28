"""Freeze reviewed external inputs and record pure self-tests, never run Gateway."""
import datetime
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parent
NAMES = ["README.md", "RECEIPT-SCHEMA.json", "platinum_176.json", "WORLD-INPUTS.json",
         "prepare_inputs.py", "freeze_inputs.py", "guarded_seed.py", "run_isolated_dense.py",
         "dense_network_driver.mjs", "dense_observation.mjs", "tests/observation.test.mjs",
         "tests/test_seed.py", "tests/test_wrapper.py", "vendor/protocol-client.mjs",
         "vendor/protocol-observation.mjs", "vendor/protocol-navigation.mjs", "vendor/ws.cjs",
         "maps/0.map.gz", "maps/1.map.gz", "maps/0132.map.gz", "maps/d021.map.gz", "CHANGES-V2.json", "CHANGES-V3.json",
         "ordinary_transfer.mjs", "tests/transfer.test.mjs", "CHANGES-V4.json",
         "tests/dead-effects.test.mjs", "reclassify_actual_v4.mjs", "CHANGES-V5.json",
         "tests/death-attribution.test.mjs", "CHANGES-V6.json"]


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    manifest_path = ROOT / "FROZEN-INPUTS.json"
    if manifest_path.exists():
        raise RuntimeError("Freeze-once operation refused: use a new manifest version after root review")
    commands = [
        ["node", "--test", "tests/observation.test.mjs"],
        ["node", "--test", "tests/dead-effects.test.mjs"],
        ["node", "--test", "tests/death-attribution.test.mjs"],
        ["node", "--test", "tests/transfer.test.mjs"],
        [sys.executable, "-m", "unittest", "discover", "-s", "tests", "-p", "test_*.py", "-v"],
        ["node", "--check", "dense_network_driver.mjs"],
        ["node", "--check", "dense_observation.mjs"],
        ["node", "--check", "ordinary_transfer.mjs"],
        ["node", "--check", "reclassify_actual_v4.mjs"],
        ["node", "dense_network_driver.mjs", "--plan-staging"],
    ]
    test_receipts = []
    for command in commands:
        result = subprocess.run(command, cwd=ROOT, capture_output=True, check=False, timeout=30)
        receipt = {"command": [Path(command[0]).name, *command[1:]], "exitCode": result.returncode,
                   "stdout": result.stdout.decode("utf-8", "replace"), "stderr": result.stderr.decode("utf-8", "replace")}
        test_receipts.append(receipt)
        if result.returncode:
            raise RuntimeError("Pure fixture self-test failed: " + " ".join(command[1:]))
    node_passed = re.search(r"^# pass (\d+)$", test_receipts[0]["stdout"], re.MULTILINE)
    dead_effect_passed = re.search(r"^# pass (\d+)$", test_receipts[1]["stdout"], re.MULTILINE)
    death_attribution_passed = re.search(r"^# pass (\d+)$", test_receipts[2]["stdout"], re.MULTILINE)
    transfer_passed = re.search(r"^# pass (\d+)$", test_receipts[3]["stdout"], re.MULTILINE)
    python_passed = re.search(r"Ran (\d+) tests", test_receipts[4]["stderr"])
    if not node_passed or not dead_effect_passed or not death_attribution_passed or not transfer_passed or not python_passed or any("# fail 0\n" not in r["stdout"] or "# skipped 0\n" not in r["stdout"] for r in test_receipts[:4]):
        raise RuntimeError("Actual self-test counts and zero failures/skips are required")
    utc = datetime.datetime.now(datetime.timezone.utc).isoformat()
    self_test = {"schema": "mir2.dense-fixture-pure-selftest.v6", "createdUtc": utc,
                 "nodeClassifierTestsPassed": int(node_passed.group(1)), "nodeTransferTestsPassed": int(transfer_passed.group(1)),
                 "nodeDeadEffectTestsPassed": int(dead_effect_passed.group(1)),
                 "nodeDeathAttributionTestsPassed": int(death_attribution_passed.group(1)),
                 "pythonFileAndWrapperTestsPassed": int(python_passed.group(1)),
                 "gatewayProcessesStarted": 0, "livePlayerStoresTouched": False,
                 "shortLivedPythonGuardTestChildrenOnly": True, "syntheticReceiptTestsAreGameplayAcceptance": False,
                 "commands": test_receipts, "fullP1Accepted": False, "nativeInputAccepted": False,
                 "nativeVisualAccepted": False, "humanAccepted": False,
                 "ordinaryProgressionAccepted": False, "loadAccepted": False,
                 "retainedOfflineEvidence": [{"relativePath": name, "bytes": (ROOT / name).stat().st_size, "sha256": sha(ROOT / name)}
                                             for name in ["RED-V5-PROTOTYPES-01.log", "RED-V5-ATTRIBUTION-01.log", "RED-V5-01.json", "GREEN-V6-PROTOTYPES-01.log", "GREEN-V6-ATTRIBUTION-01.log", "RED-GREEN-06.json", "ACTUAL-V4-RECLASSIFIED-02.json"]]}
    (ROOT / "SELFTEST-06.json").write_text(json.dumps(self_test, indent=2) + "\n", encoding="utf-8")
    inputs = {"schema": "mir2.r17.prepared-dense-frozen-inputs.v6", "createdUtc": utc,
              "sourceRevision": "6032ef8b3e27dd97bad0b20c8676ef9f185db64b",
              "exactGatewaySha256": "1ed52738885292b734c4055a9fd7d585b79d519779084bca2a617d0a9f5eb9ab",
              "previousOrdinarySmokeInputsModified": False, "previousDenseV1InputsModified": False,
              "previousDenseV1ManifestSha256": "fa0e1a984485b79bff0264270f45dfd810d7f6b85023b81db6d5818c42d81e4e",
              "previousDenseV2InputsAndActualFailurePreserved": True,
              "previousDenseV2ManifestSha256": "ffafffb613ba2aa35d07466401f2116dd664bfc0803df9e8790a8095f5312398",
              "previousDenseV3InputsAndActualFailuresPreserved": True,
              "previousDenseV3ManifestSha256": "a88078ab2eb546c9bf2c58717f311d9e6196f331907fb288e08a66b56277507b",
              "previousDenseV4InputsActualFailuresAndReportedPassesPreserved": True,
              "previousDenseV4ManifestSha256": "054689ac86e4b19f98bc836b1d783a5f6a60d08e0459183a233bb72891809192",
              "previousDenseV5InputsOfflineReclassificationAndReportedEvidencePreserved": True,
              "previousDenseV5ManifestSha256": "9ab8f6a4cf664080be42895cc022a6efb6a2d36fe4f6cf7368161e661e0cc57e",
              "sameR17GatewayExecutionAuthorized": False,
              "reviewBeforeGatewayExecutionRequired": True,
              "files": [{"relativePath": name, "bytes": (ROOT / name).stat().st_size, "sha256": sha(ROOT / name)} for name in NAMES]}
    manifest_path.write_text(json.dumps(inputs, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"frozenInputCount": len(NAMES), "manifestSha256": sha(manifest_path), "selfTestSha256": sha(ROOT / "SELFTEST-06.json"), "noGatewayStarted": True}))


if __name__ == "__main__":
    main()
