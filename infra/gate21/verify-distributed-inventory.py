#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
from collections import Counter, defaultdict
from pathlib import Path


PROFILE_ID = "mir2-regional-v1-3000-15m"
ROLE_REQUIREMENTS = {
    "gateway": (3, 4, 8, 100),
    "zone-active": (4, 8, 16, 200),
    "zone-standby": (4, 8, 16, 200),
    "postgres": (2, 8, 32, 500),
    "redis": (3, 2, 8, 100),
    "commonware": (4, 2, 2, 100),
    "load-generator": (1, 4, 8, 100),
    "zone-replicator": (1, 2, 4, 500),
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Verify Gate 21 distributed resource inventory"
    )
    parser.add_argument("--inventory", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    inventory = json.loads(args.inventory.read_text(encoding="utf-8"))
    nodes = inventory.get("nodes", [])
    counts = Counter(node.get("role") for node in nodes)
    ids = [node.get("id") for node in nodes]
    failure_domains: dict[str, set[str]] = defaultdict(set)
    for node in nodes:
        failure_domains[node.get("role", "")].add(node.get("failureDomain", ""))

    role_capacity = {}
    for role, (count, cpu, memory_gib, disk_gib) in ROLE_REQUIREMENTS.items():
        selected = [node for node in nodes if node.get("role") == role]
        role_capacity[role] = (
            len(selected) == count
            and all(node.get("cpu", 0) >= cpu for node in selected)
            and all(node.get("memoryGiB", 0) >= memory_gib for node in selected)
            and all(node.get("diskGiB", 0) >= disk_gib for node in selected)
            and all(node.get("networkMbps", 0) >= 1_000 for node in selected)
            and all(node.get("nvme") is True for node in selected)
        )

    zone_pairs: dict[str, list[dict]] = defaultdict(list)
    for node in nodes:
        if node.get("role") in {"zone-active", "zone-standby"}:
            zone_pairs[node.get("pair", "")].append(node)
    pairs_are_cross_domain = len(zone_pairs) == 4 and all(
        len(pair) == 2
        and {node.get("role") for node in pair}
        == {"zone-active", "zone-standby"}
        and len({node.get("failureDomain") for node in pair}) == 2
        for pair in zone_pairs.values()
    )

    assertions = {
        "schemaAndProfileMatch": inventory.get("schemaVersion") == 1
        and inventory.get("profileId") == PROFILE_ID,
        "singleLowLatencyRegionIsDeclared": bool(inventory.get("region"))
        and inventory.get("region") != "replace-with-one-low-latency-region"
        and inventory.get("maximumMeasuredInterNodeRttMs", float("inf")) <= 2,
        "nodeIdsAreUniqueAndNonEmpty": len(ids) == len(set(ids))
        and all(isinstance(node_id, str) and node_id for node_id in ids),
        "allRoleResourcesMeetReference": all(role_capacity.values()),
        "gatewaysSpanThreeFailureDomains": len(failure_domains["gateway"]) >= 3,
        "postgresSpansTwoFailureDomains": len(failure_domains["postgres"]) >= 2,
        "redisSpansThreeFailureDomains": len(failure_domains["redis"]) >= 3,
        "commonwareSpansFourFailureDomains": len(failure_domains["commonware"]) >= 4,
        "zonePairsUseDifferentFailureDomains": pairs_are_cross_domain,
    }
    output = {
        "schemaVersion": 1,
        "profileId": PROFILE_ID,
        "region": inventory.get("region"),
        "maximumMeasuredInterNodeRttMs": inventory.get(
            "maximumMeasuredInterNodeRttMs"
        ),
        "nodeCount": len(nodes),
        "roleCounts": dict(sorted(counts.items())),
        "totalCpu": sum(node.get("cpu", 0) for node in nodes),
        "totalMemoryGiB": sum(node.get("memoryGiB", 0) for node in nodes),
        "roleCapacity": role_capacity,
        "assertions": assertions,
        "success": all(assertions.values()),
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(
        json.dumps(output, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    if not output["success"]:
        raise SystemExit("Gate 21 distributed inventory acceptance failed")
    print(f"Gate 21 distributed inventory accepted: {args.output}")


if __name__ == "__main__":
    main()
