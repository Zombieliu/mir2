#!/usr/bin/env python3
"""Renew the finalized Home Node placement before it expires."""

from __future__ import annotations

import argparse
import json
import os
import time
import urllib.error
import urllib.request
from typing import Any


def request_json(
    method: str, url: str, body: dict[str, Any] | None = None, timeout: int = 30
) -> dict[str, Any]:
    payload = None if body is None else json.dumps(body).encode()
    headers = {"Content-Type": "application/json"}
    control_token = os.environ.get("GATE14_CONTROL_TOKEN", "").strip()
    if control_token:
        headers["Authorization"] = f"Bearer {control_token}"
    request = urllib.request.Request(
        url,
        data=payload,
        method=method,
        headers=headers,
    )
    with urllib.request.urlopen(request, timeout=timeout) as response:
        return json.loads(response.read())


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--control-url", default="http://127.0.0.1:19500")
    parser.add_argument("--validator-url", default="http://127.0.0.1:19400")
    parser.add_argument("--zone-id", default="primary")
    parser.add_argument("--renew-before-ms", type=int, default=6 * 60 * 60 * 1000)
    parser.add_argument("--ttl-ms", type=int, default=24 * 60 * 60 * 1000)
    parser.add_argument("--force", action="store_true")
    args = parser.parse_args()

    state = request_json("GET", f"{args.validator_url}/v1/state")
    placement = state.get("placements", {}).get(args.zone_id)
    if placement is None:
        print(json.dumps({"renewed": False, "error": "placement is missing"}))
        return 1
    now_ms = int(time.time() * 1000)
    remaining_ms = int(placement["expiresAtMs"]) - now_ms
    if not args.force and remaining_ms > args.renew_before_ms:
        print(
            json.dumps(
                {
                    "renewed": False,
                    "zoneId": args.zone_id,
                    "generation": placement["generation"],
                    "remainingMs": remaining_ms,
                },
                sort_keys=True,
            )
        )
        return 0

    generation = int(placement["generation"]) + 1
    command = {
        "type": "placeZone",
        "zoneId": args.zone_id,
        "generation": generation,
        "primaryHostId": placement["primaryHostId"],
        "replicaHostIds": placement["replicaHostIds"],
        "expiresAtMs": now_ms + args.ttl_ms,
    }
    last_error: Exception | None = None
    for attempt in range(8):
        try:
            status = request_json("GET", f"{args.control_url}/v1/status")
            result = request_json(
                "POST",
                f"{args.control_url}/v1/control/commands",
                {
                    "sequence": int(status["finalizedHeight"]) + 1,
                    "idempotencyKey": (
                        f"auto-renew-{args.zone_id}-generation-{generation}"
                    ),
                    "submittedAtMs": int(time.time() * 1000),
                    "command": command,
                },
            )
            print(
                json.dumps(
                    {
                        "renewed": True,
                        "zoneId": args.zone_id,
                        "generation": generation,
                        "expiresAtMs": command["expiresAtMs"],
                        "finalizedHeight": result["finalizedHeight"],
                        "stateRoot": result["stateRoot"],
                    },
                    sort_keys=True,
                )
            )
            return 0
        except (urllib.error.URLError, TimeoutError, KeyError, ValueError) as error:
            last_error = error
            time.sleep(min(attempt + 1, 3))
    print(json.dumps({"renewed": False, "error": str(last_error)}, sort_keys=True))
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
