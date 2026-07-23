#!/usr/bin/env python3
"""Gate 14.1-14.4 Docker fault and recovery acceptance.

The script intentionally controls Docker from the host instead of mounting the
Docker socket into a container. It leaves the recovered stack running by
default so a human can inspect every API and database after automation passes.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
import urllib.error
import urllib.request
from dataclasses import dataclass
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_COMPOSE = ROOT / "infra" / "gate14" / "docker-compose.yml"
DEFAULT_EVIDENCE = ROOT / "docs" / "generated" / "gate14" / "gate14-acceptance.json"
VALIDATORS = [f"http://127.0.0.1:{19400 + index}" for index in range(4)]
GATEWAY_A = "http://127.0.0.1:19500"
GATEWAY_B = "http://127.0.0.1:19501"
PROJECTOR_A = "http://127.0.0.1:19600"
PROJECTOR_B = "http://127.0.0.1:19601"
ZONE_A = "http://127.0.0.1:19100"
ZONE_B = "http://127.0.0.1:19101"


class AcceptanceError(RuntimeError):
    pass


@dataclass
class DockerCompose:
    file: Path

    def run(self, *arguments: str, capture: bool = False) -> str:
        command = ["docker", "compose", "-f", str(self.file), *arguments]
        completed = subprocess.run(
            command,
            cwd=ROOT,
            check=False,
            text=True,
            stdout=subprocess.PIPE if capture else None,
            stderr=subprocess.STDOUT if capture else None,
        )
        if completed.returncode != 0:
            output = completed.stdout or ""
            raise AcceptanceError(
                f"command failed ({completed.returncode}): {' '.join(command)}\n{output}"
            )
        return (completed.stdout or "").strip()


def request_json(
    method: str,
    url: str,
    body: Any | None = None,
    timeout: float = 10.0,
) -> Any:
    data = None if body is None else json.dumps(body, separators=(",", ":")).encode()
    headers = {"content-type": "application/json"} if data is not None else {}
    request = urllib.request.Request(url, data=data, headers=headers, method=method)
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            payload = response.read()
            return json.loads(payload) if payload else None
    except (urllib.error.URLError, urllib.error.HTTPError, TimeoutError) as error:
        detail = getattr(error, "read", lambda: b"")()
        raise AcceptanceError(
            f"{method} {url} failed: {error}; {detail.decode(errors='replace')}"
        ) from error


def wait_json(url: str, predicate, timeout: float = 120.0) -> Any:
    deadline = time.monotonic() + timeout
    last_error: Exception | None = None
    while time.monotonic() < deadline:
        try:
            value = request_json("GET", url, timeout=3)
            if predicate(value):
                return value
        except Exception as error:  # noqa: BLE001 - retain the last probe error
            last_error = error
        time.sleep(0.25)
    raise AcceptanceError(f"timed out waiting for {url}; last error: {last_error}")


def wait_text(url: str, expected: str = "ok", timeout: float = 120.0) -> str:
    deadline = time.monotonic() + timeout
    last_error: Exception | None = None
    while time.monotonic() < deadline:
        try:
            with urllib.request.urlopen(url, timeout=3) as response:
                value = response.read().decode()
                if expected in value:
                    return value
        except Exception as error:  # noqa: BLE001
            last_error = error
        time.sleep(0.25)
    raise AcceptanceError(f"timed out waiting for {url}; last error: {last_error}")


def now_ms() -> int:
    return int(time.time() * 1000)


def submit(gateway: str, command: dict[str, Any], key: str) -> dict[str, Any]:
    status = request_json("GET", f"{gateway}/v1/status")
    sequence = int(status["finalizedHeight"]) + 1
    envelope = {
        "sequence": sequence,
        "idempotencyKey": key,
        "submittedAtMs": now_ms(),
        "command": command,
    }
    response = request_json("POST", f"{gateway}/v1/control/commands", envelope, timeout=30)
    if not response.get("accepted") or response.get("finalizedHeight", 0) < sequence:
        raise AcceptanceError(f"command {key} was not finalized: {response}")
    return response


def acquire(gateway: str, session_id: str) -> dict[str, Any]:
    response = request_json(
        "POST",
        f"{gateway}/v1/sessions/acquire",
        {
            "sessionId": session_id,
            "accountId": "gate14-alice",
            "characterId": "gate14-hero",
            "zoneId": "mir2-map-0",
            "ttlMs": 300_000,
        },
        timeout=30,
    )
    if not response.get("accepted"):
        raise AcceptanceError(f"session acquisition failed: {response}")
    return response


def wait_projection(url: str, height: int, redis: bool | None = None) -> dict[str, Any]:
    def ready(status: dict[str, Any]) -> bool:
        if not status.get("databaseAvailable"):
            return False
        if int(status.get("finalizedHeight", 0)) < height:
            return False
        return redis is None or bool(status.get("redisAvailable")) is redis

    return wait_json(f"{url}/v1/status", ready, timeout=120)


def postgres_scalar(compose: DockerCompose, service: str, sql: str) -> str:
    return compose.run(
        "exec",
        "-T",
        service,
        "psql",
        "-U",
        "gate14",
        "-d",
        "gate14",
        "-tAc",
        sql,
        capture=True,
    ).strip()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--compose-file", type=Path, default=DEFAULT_COMPOSE)
    parser.add_argument("--evidence", type=Path, default=DEFAULT_EVIDENCE)
    parser.add_argument("--skip-build", action="store_true")
    parser.add_argument("--reset", action="store_true")
    parser.add_argument("--down-after", action="store_true")
    args = parser.parse_args()
    compose = DockerCompose(args.compose_file.resolve())
    evidence: dict[str, Any] = {
        "gate": "14.1-14.4-no-single-point-vertical-poc",
        "accepted": False,
        "startedAtMs": now_ms(),
        "commonwareRelease": "v2026.2.0",
        "milestones": {},
        "faults": [],
    }
    try:
        if args.reset:
            compose.run("down", "-v", "--remove-orphans")
        if not args.skip_build:
            compose.run("build", "validator-0", "dubhe-a")
        up = ["up", "-d"]
        up.extend(
            [
                "validator-0",
                "validator-1",
                "validator-2",
                "validator-3",
                "postgres-a",
                "postgres-b",
                "redis-a",
                "redis-b",
                "gateway-a",
                "gateway-b",
                "projector-a",
                "projector-b",
                "dubhe-a",
                "dubhe-b",
            ]
        )
        compose.run(*up)
        for validator in VALIDATORS:
            wait_text(f"{validator}/healthz")
        wait_json(
            f"{GATEWAY_A}/v1/status",
            lambda value: value.get("healthy") is True,
        )
        wait_json(
            f"{GATEWAY_B}/v1/status",
            lambda value: value.get("healthy") is True,
        )
        wait_text(f"{ZONE_A}/readyz", expected="")
        wait_text(f"{ZONE_B}/readyz", expected="")

        placement_expiry = now_ms() + 900_000
        initial_commands = [
            (
                "register-dubhe-a",
                {
                    "type": "registerZoneHost",
                    "hostId": "dubhe-a",
                    "endpoint": "dubhe-a:7020",
                    "failureDomain": "rack-a",
                    "maxSessions": 128,
                    "maxZones": 8,
                },
            ),
            (
                "register-dubhe-b",
                {
                    "type": "registerZoneHost",
                    "hostId": "dubhe-b",
                    "endpoint": "dubhe-b:7020",
                    "failureDomain": "rack-b",
                    "maxSessions": 128,
                    "maxZones": 8,
                },
            ),
            (
                "place-map-0-generation-1",
                {
                    "type": "placeZone",
                    "zoneId": "mir2-map-0",
                    "generation": 1,
                    "primaryHostId": "dubhe-a",
                    "replicaHostIds": ["dubhe-b"],
                    "expiresAtMs": placement_expiry,
                },
            ),
            (
                "create-account-alice",
                {"type": "createAccount", "accountId": "gate14-alice"},
            ),
            (
                "create-character-hero",
                {
                    "type": "createCharacter",
                    "accountId": "gate14-alice",
                    "characterId": "gate14-hero",
                    "name": "Gate14Hero",
                },
            ),
            (
                "verified-loot-1",
                {
                    "type": "grantVerifiedLoot",
                    "accountId": "gate14-alice",
                    "characterId": "gate14-hero",
                    "itemId": "red-potion",
                    "quantity": 5,
                    "receiptId": "gate14-receipt-1",
                },
            ),
            (
                "gold-quest-100",
                {
                    "type": "changeGold",
                    "accountId": "gate14-alice",
                    "characterId": "gate14-hero",
                    "delta": 100,
                    "reason": "gate14 quest reward",
                },
            ),
            (
                "consume-potion-2",
                {
                    "type": "consumeItem",
                    "accountId": "gate14-alice",
                    "characterId": "gate14-hero",
                    "itemId": "red-potion",
                    "quantity": 2,
                },
            ),
        ]
        for key, command in initial_commands:
            submit(GATEWAY_A, command, key)
        lease_a = acquire(GATEWAY_A, "gate14-session")
        base_height = int(lease_a["finalizedHeight"])
        if lease_a["lease"]["gatewayId"] != "gateway-a":
            raise AcceptanceError("initial session lease is not owned by gateway-a")
        projection_a = wait_projection(PROJECTOR_A, base_height, redis=True)
        projection_b = wait_projection(PROJECTOR_B, base_height, redis=True)
        evidence["milestones"]["goal1Commonware"] = {
            "accepted": True,
            "validators": [
                request_json("GET", f"{validator}/v1/status") for validator in VALIDATORS
            ],
            "eventDriven": True,
            "quorum": 3,
        }
        evidence["milestones"]["goal2DynamicGateway"] = {
            "accepted": True,
            "initialLease": lease_a,
            "gatewayA": request_json("GET", f"{GATEWAY_A}/v1/status"),
            "gatewayB": request_json("GET", f"{GATEWAY_B}/v1/status"),
        }
        evidence["milestones"]["goal3AuthoritativeStateAndProjection"] = {
            "accepted": True,
            "projectorA": projection_a,
            "projectorB": projection_b,
        }

        # 3/4 finality and lagging-validator certificate catch-up.
        compose.run("stop", "validator-3")
        quorum_command = submit(
            GATEWAY_A,
            {
                "type": "changeGold",
                "accountId": "gate14-alice",
                "characterId": "gate14-hero",
                "delta": 25,
                "reason": "3-of-4 validator fault proof",
            },
            "gold-with-validator-3-down",
        )
        quorum_height = int(quorum_command["finalizedHeight"])
        gateway_degraded = wait_json(
            f"{GATEWAY_A}/v1/status",
            lambda value: value.get("respondingValidators") == 3
            and int(value.get("finalizedHeight", 0)) >= quorum_height,
        )
        compose.run("start", "validator-3")
        wait_text(f"{VALIDATORS[3]}/healthz")
        records = request_json(
            "GET", f"{VALIDATORS[0]}/v1/finality?after={base_height}"
        )
        request_json("POST", f"{VALIDATORS[3]}/v1/import", records)
        validator_three = wait_json(
            f"{VALIDATORS[3]}/v1/status",
            lambda value: int(value.get("finalizedHeight", 0)) >= quorum_height,
        )
        evidence["faults"].append(
            {
                "fault": "validator-3-stop-and-catch-up",
                "accepted": True,
                "degradedGateway": gateway_degraded,
                "recoveredValidator": validator_three,
            }
        )

        # Gateway takeover increments the finalized session fencing token.
        compose.run("stop", "gateway-a")
        lease_b = acquire(GATEWAY_B, "gate14-session")
        if (
            lease_b["lease"]["gatewayId"] != "gateway-b"
            or int(lease_b["lease"]["fencingToken"]) != 2
        ):
            raise AcceptanceError(f"gateway failover lease is not fenced: {lease_b}")
        evidence["faults"].append(
            {
                "fault": "gateway-a-stop",
                "accepted": True,
                "takeoverLease": lease_b,
            }
        )

        # Redis is cache only: finalized writes and Postgres projection continue.
        compose.run("stop", "redis-a")
        redis_fault_command = submit(
            GATEWAY_B,
            {
                "type": "changeGold",
                "accountId": "gate14-alice",
                "characterId": "gate14-hero",
                "delta": -10,
                "reason": "redis cache outage proof",
            },
            "gold-with-redis-a-down",
        )
        redis_fault_height = int(redis_fault_command["finalizedHeight"])
        projector_a_degraded = wait_projection(
            PROJECTOR_A, redis_fault_height, redis=False
        )
        projector_b_healthy = wait_projection(
            PROJECTOR_B, redis_fault_height, redis=True
        )
        compose.run("start", "redis-a")
        request_json("POST", f"{PROJECTOR_A}/v1/rebuild", {})
        projector_a_recovered = wait_projection(
            PROJECTOR_A, redis_fault_height, redis=True
        )
        evidence["faults"].append(
            {
                "fault": "redis-a-stop",
                "accepted": True,
                "databaseContinued": projector_a_degraded,
                "independentProjection": projector_b_healthy,
                "recovered": projector_a_recovered,
            }
        )

        # One Postgres projection can disappear and rebuild from Commonware.
        compose.run("stop", "postgres-a")
        postgres_fault_command = submit(
            GATEWAY_B,
            {
                "type": "grantVerifiedLoot",
                "accountId": "gate14-alice",
                "characterId": "gate14-hero",
                "itemId": "red-potion",
                "quantity": 2,
                "receiptId": "gate14-receipt-2",
            },
            "loot-with-postgres-a-down",
        )
        postgres_fault_height = int(postgres_fault_command["finalizedHeight"])
        projector_b_during_fault = wait_projection(
            PROJECTOR_B, postgres_fault_height, redis=True
        )
        compose.run("start", "postgres-a")
        compose.run("restart", "projector-a")
        projector_a_rebuilt = wait_projection(
            PROJECTOR_A, postgres_fault_height, redis=True
        )
        evidence["faults"].append(
            {
                "fault": "postgres-a-stop",
                "accepted": True,
                "projectorB": projector_b_during_fault,
                "projectorARebuilt": projector_a_rebuilt,
            }
        )

        # Zone Host failover is a finalized placement generation, not an env edit.
        compose.run("stop", "dubhe-a")
        failover_command = submit(
            GATEWAY_B,
            {
                "type": "placeZone",
                "zoneId": "mir2-map-0",
                "generation": 2,
                "primaryHostId": "dubhe-b",
                "replicaHostIds": [],
                "expiresAtMs": now_ms() + 900_000,
            },
            "place-map-0-generation-2",
        )
        final_height = int(failover_command["finalizedHeight"])
        route_b = wait_json(
            f"{GATEWAY_B}/v1/routes/mir2-map-0",
            lambda value: value.get("primaryEndpoint") == "dubhe-b:7020"
            and int(value["placement"]["generation"]) == 2,
        )
        compose.run("start", "dubhe-a")
        wait_text(f"{ZONE_A}/readyz", expected="")
        compose.run("start", "gateway-a")
        gateway_a_recovered = wait_json(
            f"{GATEWAY_A}/v1/status",
            lambda value: value.get("healthy") is True
            and int(value.get("finalizedHeight", 0)) >= final_height,
        )
        evidence["faults"].append(
            {
                "fault": "dubhe-a-stop",
                "accepted": True,
                "finalizedRoute": route_b,
                "gatewayARecovered": gateway_a_recovered,
            }
        )

        projection_a = wait_projection(PROJECTOR_A, final_height, redis=True)
        projection_b = wait_projection(PROJECTOR_B, final_height, redis=True)
        validator_statuses = [
            wait_json(
                f"{validator}/v1/status",
                lambda value: int(value.get("finalizedHeight", 0)) >= final_height,
            )
            for validator in VALIDATORS
        ]
        roots = {status["stateRoot"] for status in validator_statuses}
        roots.update([projection_a["stateRoot"], projection_b["stateRoot"]])
        if len(roots) != 1:
            raise AcceptanceError(f"final state roots diverged: {roots}")

        db_checks: dict[str, Any] = {}
        for service in ("postgres-a", "postgres-b"):
            db_checks[service] = {
                "height": int(
                    postgres_scalar(
                        compose,
                        service,
                        "SELECT finalized_height FROM gate14_projection_meta LIMIT 1",
                    )
                ),
                "stateRoot": postgres_scalar(
                    compose,
                    service,
                    "SELECT state_root FROM gate14_projection_meta LIMIT 1",
                ),
                "gold": int(
                    postgres_scalar(
                        compose,
                        service,
                        "SELECT gold FROM gate14_characters WHERE character_id='gate14-hero'",
                    )
                ),
                "redPotion": int(
                    postgres_scalar(
                        compose,
                        service,
                        "SELECT quantity FROM gate14_inventory WHERE character_id='gate14-hero' AND item_id='red-potion'",
                    )
                ),
                "commandCount": int(
                    postgres_scalar(
                        compose,
                        service,
                        "SELECT count(*) FROM gate14_finalized_commands",
                    )
                ),
            }
            if (
                db_checks[service]["height"] != final_height
                or db_checks[service]["gold"] != 115
                or db_checks[service]["redPotion"] != 5
                or db_checks[service]["commandCount"] != final_height
            ):
                raise AcceptanceError(
                    f"{service} projection has lost or duplicated state: {db_checks[service]}"
                )

        final_state = request_json("GET", f"{VALIDATORS[0]}/v1/state")
        final_lease = final_state["sessionLeases"]["gate14-session"]
        if (
            final_lease["gatewayId"] != "gateway-b"
            or int(final_lease["fencingToken"]) != 2
        ):
            raise AcceptanceError(f"final session lease is not fenced: {final_lease}")
        evidence["milestones"]["goal4FaultRecovery"] = {
            "accepted": True,
            "finalHeight": final_height,
            "stateRoot": roots.pop(),
            "validatorStatuses": validator_statuses,
            "projectorA": projection_a,
            "projectorB": projection_b,
            "databaseChecks": db_checks,
            "finalSessionLease": final_lease,
            "finalRoute": route_b,
        }
        evidence["accepted"] = True
        evidence["completedAtMs"] = now_ms()
        args.evidence.parent.mkdir(parents=True, exist_ok=True)
        args.evidence.write_text(
            json.dumps(evidence, indent=2, ensure_ascii=False) + "\n",
            encoding="utf-8",
        )
        print(json.dumps(evidence, indent=2, ensure_ascii=False))
        return 0
    except Exception as error:  # noqa: BLE001
        evidence["error"] = str(error)
        evidence["completedAtMs"] = now_ms()
        args.evidence.parent.mkdir(parents=True, exist_ok=True)
        args.evidence.write_text(
            json.dumps(evidence, indent=2, ensure_ascii=False) + "\n",
            encoding="utf-8",
        )
        print(f"Gate 14 acceptance failed: {error}", file=sys.stderr)
        try:
            print(compose.run("ps", capture=True), file=sys.stderr)
        except Exception:
            pass
        return 1
    finally:
        if args.down_after:
            compose.run("down")


if __name__ == "__main__":
    raise SystemExit(main())
