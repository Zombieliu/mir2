#!/usr/bin/env python3
"""Gate 15 real-player, dynamic-placement, and Zone failover acceptance."""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
BASE_COMPOSE = ROOT / "infra" / "gate14" / "docker-compose.yml"
GATE15_COMPOSE = ROOT / "infra" / "gate15" / "docker-compose.yml"
EVIDENCE = ROOT / "docs" / "generated" / "gate15" / "gate15-acceptance.json"
PLAYER_READY = ROOT / "docs" / "generated" / "gate15" / "players-ready.json"
PLAYER_MARKER = ROOT / "docs" / "generated" / "gate15" / "failover.marker"
PLAYER_REPORT = ROOT / "docs" / "generated" / "gate15" / "gate15-players.json"

CONTROL_A = "http://127.0.0.1:20500"
CONTROL_B = "http://127.0.0.1:20501"
PLAYER_GATEWAY_A = "http://127.0.0.1:19710"
PLAYER_GATEWAY_B = "http://127.0.0.1:19711"
VALIDATORS = [f"http://127.0.0.1:{20400 + index}" for index in range(4)]


class AcceptanceError(RuntimeError):
    pass


def compose(arguments: list[str], *, check: bool = True) -> subprocess.CompletedProcess[str]:
    command = [
        "docker",
        "compose",
        "-f",
        str(BASE_COMPOSE),
        "-f",
        str(GATE15_COMPOSE),
        *arguments,
    ]
    return subprocess.run(
        command,
        cwd=ROOT,
        check=check,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )


def request_json(
    method: str, url: str, body: dict[str, Any] | None = None, timeout: float = 5
) -> Any:
    payload = None if body is None else json.dumps(body).encode()
    request = urllib.request.Request(
        url,
        data=payload,
        method=method,
        headers={"Content-Type": "application/json"},
    )
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            return json.loads(response.read())
    except (urllib.error.URLError, TimeoutError, json.JSONDecodeError) as error:
        raise AcceptanceError(f"{method} {url} failed: {error}") from error


def wait_json(url: str, predicate, label: str, timeout: float = 90) -> Any:
    deadline = time.monotonic() + timeout
    last_error: Exception | None = None
    while time.monotonic() < deadline:
        try:
            value = request_json("GET", url)
            if predicate(value):
                return value
        except Exception as error:  # acceptance polling records the final error
            last_error = error
        time.sleep(0.25)
    raise AcceptanceError(f"timed out waiting for {label}: {last_error}")


def wait_file(path: Path, label: str, timeout: float = 30) -> dict[str, Any]:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if path.exists():
            return json.loads(path.read_text())
        time.sleep(0.1)
    raise AcceptanceError(f"timed out waiting for {label}: {path}")


def wait_logs(service: str, needle: str, timeout: float = 30) -> str:
    deadline = time.monotonic() + timeout
    output = ""
    while time.monotonic() < deadline:
        result = compose(["logs", "--no-color", service], check=False)
        output = result.stdout
        if needle in output:
            return output
        time.sleep(0.25)
    raise AcceptanceError(f"timed out waiting for {service} log containing {needle!r}")


def submit(control: str, command: dict[str, Any], key: str) -> dict[str, Any]:
    status = request_json("GET", f"{control}/v1/status")
    sequence = int(status["finalizedHeight"]) + 1
    return request_json(
        "POST",
        f"{control}/v1/control/commands",
        {
            "sequence": sequence,
            "idempotencyKey": key,
            "submittedAtMs": int(time.time() * 1000),
            "command": command,
        },
        timeout=25,
    )


def seed_control_state() -> None:
    expiry = int(time.time() * 1000) + 3_600_000
    commands = [
        (
            "gate15-register-dubhe-a",
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
            "gate15-register-dubhe-b",
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
            "gate15-place-primary-a",
            {
                "type": "placeZone",
                "zoneId": "mir2-map-0",
                "generation": 1,
                "primaryHostId": "dubhe-a",
                "replicaHostIds": ["dubhe-b"],
                "expiresAtMs": expiry,
            },
        ),
    ]
    for index in range(2):
        commands.append(
            (
                f"gate15-account-{index}",
                {
                    "type": "createAccount",
                    "accountId": f"gate15-player-{index}",
                },
            )
        )
        for character_index in range(4):
            commands.append(
                (
                    f"gate15-character-{index}-{character_index}",
                    {
                        "type": "createCharacter",
                        "accountId": f"gate15-player-{index}",
                        "characterId": f"gate15-player-{index}:{character_index}",
                        "name": f"Gate15P{index}Slot{character_index}",
                    },
                )
            )
    for key, command in commands:
        submit(CONTROL_A, command, key)


def start_players() -> subprocess.Popen[str]:
    for path in (PLAYER_READY, PLAYER_MARKER, PLAYER_REPORT):
        path.unlink(missing_ok=True)
    env = os.environ.copy()
    env.update(
        {
            "MIR2_GATE15_PLAYER_WS_URLS": (
                "ws://127.0.0.1:19710/ws,ws://127.0.0.1:19711/ws"
            ),
            "MIR2_GATE15_PLAYERS_READY": str(PLAYER_READY),
            "MIR2_GATE15_FAILOVER_MARKER": str(PLAYER_MARKER),
            "MIR2_GATE15_PLAYERS_OUT": str(PLAYER_REPORT),
            "MIR2_GATE15_PLAYER_DURATION_MS": "35000",
        }
    )
    return subprocess.Popen(
        ["node", "scripts/gate15_players.mjs"],
        cwd=ROOT,
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--reset", action="store_true")
    parser.add_argument("--skip-build", action="store_true")
    args = parser.parse_args()
    EVIDENCE.parent.mkdir(parents=True, exist_ok=True)
    if args.reset:
        compose(
            ["--profile", "reverse", "down", "-v", "--remove-orphans"],
            check=False,
        )

    up = ["up", "-d"]
    if not args.skip_build:
        up.append("--build")
    compose(up)
    wait_json(
        f"{CONTROL_A}/v1/status",
        lambda value: value.get("healthy") is True,
        "control Gateway A quorum",
    )
    wait_json(
        f"{CONTROL_B}/v1/status",
        lambda value: value.get("healthy") is True,
        "control Gateway B quorum",
    )
    seed_control_state()
    wait_json(
        f"{PLAYER_GATEWAY_A}/health",
        lambda value: (value.get("gate15") or {}).get("placementCount") == 1,
        "real player Gateway A placement",
    )
    wait_json(
        f"{PLAYER_GATEWAY_B}/health",
        lambda value: (value.get("gate15") or {}).get("placementCount") == 1,
        "real player Gateway B placement",
    )

    players = start_players()
    player_stdout = ""
    try:
        ready = wait_file(PLAYER_READY, "two real players", timeout=30)
        wait_logs("zone-replicator", "sessions=2", timeout=30)

        compose(["stop", "dubhe-a"])
        expiry = int(time.time() * 1000) + 3_600_000
        failover_command = submit(
            CONTROL_B,
            {
                "type": "placeZone",
                "zoneId": "mir2-map-0",
                "generation": 2,
                "primaryHostId": "dubhe-b",
                "replicaHostIds": ["dubhe-a"],
                "expiresAtMs": expiry,
            },
            "gate15-failover-primary-b",
        )
        PLAYER_MARKER.write_text(
            json.dumps(
                {
                    "at": int(time.time() * 1000),
                    "finalizedHeight": failover_command["finalizedHeight"],
                    "primary": "dubhe-b",
                    "generation": 2,
                },
                indent=2,
            )
            + "\n"
        )
        try:
            player_stdout, _ = players.communicate(timeout=55)
        except subprocess.TimeoutExpired as error:
            players.kill()
            player_stdout, _ = players.communicate()
            raise AcceptanceError("real-player fault harness timed out") from error
        if players.returncode != 0:
            raise AcceptanceError(
                f"real-player fault harness failed ({players.returncode}):\n{player_stdout}"
            )
        player_report = json.loads(PLAYER_REPORT.read_text())

        # Recover A as the new standby. Stop the old A->B direction before A
        # starts empty, then run the B->A direction to restore the checkpoint.
        compose(["stop", "zone-replicator"], check=False)
        compose(["start", "dubhe-a"])
        deadline = time.monotonic() + 60
        while time.monotonic() < deadline:
            status = compose(["ps", "--status", "running", "dubhe-a"], check=False)
            if "dubhe-a" in status.stdout:
                break
            time.sleep(0.5)
        else:
            raise AcceptanceError("dubhe-a did not recover")
        compose(
            [
                "--profile",
                "reverse",
                "up",
                "-d",
                "zone-replicator-b-to-a",
            ]
        )
        reverse_logs = wait_logs(
            "zone-replicator-b-to-a", "installed checkpoint", timeout=30
        )
        # Validator containers can receive new bridge addresses while Dubhe A
        # is recovered. Restart the disposable projectors so their HTTP clients
        # resolve the live validators and the manual environment ends all-green.
        compose(["restart", "projector-a", "projector-b"])
        projector_health = [
            wait_json(
                "http://127.0.0.1:20600/v1/status",
                lambda value: value.get("healthy") is True,
                "projector A recovery",
            ),
            wait_json(
                "http://127.0.0.1:20601/v1/status",
                lambda value: value.get("healthy") is True,
                "projector B recovery",
            ),
        ]

        route = request_json("GET", f"{CONTROL_B}/v1/routes/mir2-map-0")
        final_state = request_json("GET", f"{VALIDATORS[0]}/v1/state")
        validator_statuses = [
            request_json("GET", f"{validator}/v1/status") for validator in VALIDATORS
        ]
        gateway_health = [
            request_json("GET", f"{PLAYER_GATEWAY_A}/health"),
            request_json("GET", f"{PLAYER_GATEWAY_B}/health"),
        ]
        roots = {status["stateRoot"] for status in validator_statuses}
        assertions = {
            "twoRealPlayersStarted": bool(ready.get("ready")),
            "playersSurvivedZoneFailure": bool(player_report.get("ok")),
            "placementFinalizedOnDubheB": (
                route["placement"]["generation"] == 2
                and route["placement"]["primaryHostId"] == "dubhe-b"
            ),
            "allValidatorsAgree": len(roots) == 1,
            "bothRealGatewaysObserveQuorum": all(
                (health.get("gate15") or {}).get("healthy") is True
                for health in gateway_health
            ),
            "twoFencedPlayerLeasesFinalized": all(
                (
                    f"player:{player['accountId']}:{player['characterIndex']}"
                    in final_state["sessionLeases"]
                )
                for player in player_report["players"]
            ),
            "recoveredAReceivedReverseCheckpoint": "installed checkpoint"
            in reverse_logs,
            "bothProjectorsHealthy": all(
                status.get("healthy") is True for status in projector_health
            ),
        }
        evidence = {
            "ok": all(assertions.values()),
            "generatedAt": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
            "gate": "15.1-15.4",
            "assertions": assertions,
            "initialPlayers": ready,
            "playerFaultRun": player_report,
            "playerHarnessOutput": player_stdout,
            "finalRoute": route,
            "gatewayHealth": gateway_health,
            "projectorHealth": projector_health,
            "validatorStatuses": validator_statuses,
            "finalizedHeight": final_state["finalizedHeight"],
            "stateRoot": validator_statuses[0]["stateRoot"],
            "sessionLeases": final_state["sessionLeases"],
            "reverseReplicatorLogTail": reverse_logs.splitlines()[-20:],
        }
        EVIDENCE.write_text(json.dumps(evidence, indent=2) + "\n")
        if not evidence["ok"]:
            raise AcceptanceError(f"Gate 15 assertions failed: {assertions}")
        print(
            "Gate 15 acceptance passed: "
            f"height={evidence['finalizedHeight']} "
            f"root={evidence['stateRoot']} "
            f"postFailover={','.join(str(player['zoneResponsesAfterFailover']) for player in player_report['players'])}"
        )
        print(f"Wrote {EVIDENCE}")
        return 0
    finally:
        if players.poll() is None:
            players.kill()
            players.wait()


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (AcceptanceError, subprocess.CalledProcessError) as error:
        print(f"Gate 15 acceptance failed: {error}", file=sys.stderr)
        raise SystemExit(1)
