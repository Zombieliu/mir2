#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Verify Gate 21 fault evidence")
    parser.add_argument("--evidence-dir", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    return parser.parse_args()


def load(root: Path, name: str) -> dict:
    path = root / name
    if not path.is_file():
        raise SystemExit(f"missing Gate 21 fault evidence: {path}")
    return json.loads(path.read_text(encoding="utf-8"))


def digest(root: Path, name: str) -> str:
    return hashlib.sha256((root / name).read_bytes()).hexdigest()


def main() -> None:
    args = parse_args()
    names = {
        "resource": "gate21-fault-resource-attestation.json",
        "standby": "gate21-standby-zone-kill.json",
        "active": "gate21-active-zone-kill-session.json",
        "preflight": "gate21-infra-preflight.json",
        "gateway": "gate21-gateway-kill.json",
        "redis": "gate21-redis-primary-failover.json",
        "commonware": "gate21-commonware-validator-kill.json",
        "rolling": "gate21-rolling-upgrade-session.json",
        "rolling_return": "gate21-rolling-upgrade-return-session.json",
        "partition": "gate21-network-partition-session.json",
        "postgres": "gate21-postgres-primary-failover.json",
        "manifest": "gate21-fault-runtime-manifest.json",
    }
    evidence = {key: load(args.evidence_dir, name) for key, name in names.items()}
    manifest = evidence["manifest"]
    assertions = {
        "referenceAndHarnessResourcesAttested": evidence["resource"].get("success")
        is True,
        "standbyLossPreservedGameplay": evidence["standby"].get("success") is True,
        "activeZoneFailoverPreservedRealSession": evidence["active"].get("success")
        is True,
        "activeZoneFailoverMetFiveSecondRto": evidence["active"].get(
            "waitingForPromotionMs", float("inf")
        )
        < 5_000,
        "gatewayLossLeftTwoHealthyReplicas": evidence["gateway"].get(
            "healthyGatewayCount", 0
        )
        >= 2
        and evidence["gateway"].get("recoveryRtoMs", float("inf")) < 10_000,
        "redisSentinelPromotedWritableMaster": evidence["redis"].get("success")
        is True
        and evidence["redis"].get("redisMasterAddress")
        != evidence["preflight"].get("redisMasterAddress"),
        "commonwareThreeOfFourFinalityRecovered": evidence["commonware"].get(
            "accepted"
        )
        is True
        and evidence["commonware"].get("commonwareRelease") == "v2026.2.0",
        "rollingUpgradeUsedDifferentImages": manifest.get("rollingUpgrade", {}).get(
            "previousImage"
        )
        != manifest.get("rollingUpgrade", {}).get("currentImage"),
        "rollingUpgradePreservedRealSession": evidence["rolling"].get("success")
        is True
        and evidence["rolling"].get("waitingForPromotionMs", float("inf")) < 5_000
        and evidence["rolling_return"].get("success") is True
        and evidence["rolling_return"].get("waitingForPromotionMs", float("inf"))
        < 5_000,
        "networkPartitionPreservedRealSession": evidence["partition"].get("success")
        is True
        and evidence["partition"].get("waitingForPromotionMs", float("inf")) < 5_000,
        "postgresStandbyBecameWritable": evidence["postgres"].get("success") is True
        and evidence["postgres"].get("postgresServerAddress")
        != manifest.get("preflightPostgresServer"),
    }
    output = {
        "schemaVersion": 1,
        "gate": 21,
        "profileId": "mir2-regional-v1",
        "gitCommit": manifest.get("gitCommit"),
        "faults": [
            "active-zone-host-kill",
            "standby-zone-host-kill",
            "gateway-kill",
            "redis-primary-failover",
            "postgres-primary-failover",
            "commonware-validator-kill",
            "rolling-zone-host-upgrade",
            "network-partition-active-to-control",
        ],
        "sourceSha256": {
            key: digest(args.evidence_dir, name) for key, name in names.items()
        },
        "assertions": assertions,
        "success": all(assertions.values()),
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(
        json.dumps(output, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    if not output["success"]:
        raise SystemExit("Gate 21 fault acceptance failed")
    print(f"Gate 21 fault evidence written to {args.output}")


if __name__ == "__main__":
    main()
