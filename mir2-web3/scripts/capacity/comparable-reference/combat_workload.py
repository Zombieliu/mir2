"""Materialize the explicit combat extension without changing frozen dense plans."""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

import workload

HERE = Path(__file__).resolve().parent


def materialize(nav, population, purpose, layer, *, navigation_file_sha256):
    contract_path = HERE / "combat-contract.v1.json"
    contract_bytes = contract_path.read_bytes()
    contract = json.loads(contract_bytes)
    plan = workload.materialize(nav, population, "dispersed-melee", purpose, layer,
                                navigation_file_sha256=navigation_file_sha256)
    old_id = plan["planId"]
    contract_sha = hashlib.sha256(contract_bytes).hexdigest()
    new_id = hashlib.sha256((old_id + ":" + contract_sha).encode()).hexdigest()[:20]
    plan["baseGeneratorSourceSha256"] = plan["generatorSourceSha256"]
    plan["generatorSourceSha256"] = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    plan["combatContract"] = contract
    plan["combatContractSha256"] = contract_sha
    plan["combatContractCanonicalSha256"] = hashlib.sha256(workload.canonical_bytes(contract)).hexdigest()
    plan["planId"] = new_id
    for action in plan["actions"]:
        if action["kind"] == "chat":
            action["nonce"] = action["nonce"].replace(old_id, new_id, 1)
            action["nonceSha256"] = hashlib.sha256(action["nonce"].encode()).hexdigest()
    return plan


def validate_plan(plan, nav, *, navigation_file_sha256):
    expected = materialize(nav, plan.get("population"), plan.get("purpose"), plan.get("qualificationLayer"),
                           navigation_file_sha256=navigation_file_sha256)
    workload.require(workload.canonical_bytes(plan) == workload.canonical_bytes(expected), "combat_plan_changed")
    return expected


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--navigation", type=Path, required=True)
    parser.add_argument("--population", type=int, required=True)
    parser.add_argument("--purpose", choices=["functional", "capacity"], required=True)
    parser.add_argument("--layer", choices=["game-core-local", "full-stack-local", "cloud-production"], default="game-core-local")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    data = args.navigation.read_bytes()
    plan = materialize(json.loads(data), args.population, args.purpose, args.layer,
                       navigation_file_sha256=hashlib.sha256(data).hexdigest())
    with args.output.open("x", encoding="utf-8", newline="\n") as stream:
        json.dump(plan, stream, ensure_ascii=False, indent=2, allow_nan=False)
        stream.write("\n")
    print(json.dumps({"planSha256": hashlib.sha256(args.output.read_bytes()).hexdigest(),
                      "combatContractSha256": plan["combatContractSha256"], "actors": args.population,
                      "primaryActions": len(plan["actions"]), "capacityClaimAllowed": False}))


if __name__ == "__main__":
    main()
