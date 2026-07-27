#!/usr/bin/env python3
"""Quorum-sourced, certificate-verifying catch-up for restarted validators."""

from __future__ import annotations

import json
import urllib.error
import urllib.request
from collections import defaultdict
from typing import Any

VALIDATORS = [f"http://127.0.0.1:{19400 + index}" for index in range(4)]
BATCH_SIZE = 100


def request_json(
    method: str, url: str, body: Any | None = None, timeout: int = 15
) -> Any:
    payload = None if body is None else json.dumps(body).encode()
    request = urllib.request.Request(
        url,
        data=payload,
        method=method,
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(request, timeout=timeout) as response:
        return json.loads(response.read())


def main() -> int:
    statuses: dict[str, dict[str, Any]] = {}
    unavailable: list[str] = []
    for validator in VALIDATORS:
        try:
            statuses[validator] = request_json("GET", f"{validator}/v1/status")
        except (urllib.error.URLError, TimeoutError, ValueError):
            unavailable.append(validator)

    groups: dict[tuple[int, str], list[str]] = defaultdict(list)
    for validator, status in statuses.items():
        groups[(int(status["finalizedHeight"]), status["stateRoot"])].append(validator)
    quorum_groups = [
        (height, state_root, validators)
        for (height, state_root), validators in groups.items()
        if len(validators) >= 3
    ]
    if not quorum_groups:
        print(
            json.dumps(
                {
                    "healthy": False,
                    "error": "no 3/4 validator state quorum",
                    "responding": len(statuses),
                    "unavailable": len(unavailable),
                },
                sort_keys=True,
            )
        )
        return 1

    quorum_height, quorum_root, sources = max(quorum_groups, key=lambda group: group[0])
    source = sources[0]
    repaired: list[dict[str, Any]] = []
    for validator, status in statuses.items():
        height = int(status["finalizedHeight"])
        root = status["stateRoot"]
        if height == quorum_height:
            if root != quorum_root:
                print(
                    json.dumps(
                        {
                            "healthy": False,
                            "error": "conflicting state root at quorum height",
                            "validator": validator,
                            "height": height,
                        },
                        sort_keys=True,
                    )
                )
                return 1
            continue
        if height > quorum_height:
            continue
        records = request_json("GET", f"{source}/v1/finality?after={height}")
        imported = 0
        for start in range(0, len(records), BATCH_SIZE):
            result = request_json(
                "POST",
                f"{validator}/v1/import",
                records[start : start + BATCH_SIZE],
            )
            imported += int(result["imported"])
        repaired.append(
            {
                "validator": validator,
                "fromHeight": height,
                "toHeight": quorum_height,
                "imported": imported,
            }
        )

    final = {
        validator: request_json("GET", f"{validator}/v1/status")
        for validator in statuses
    }
    converged = all(
        int(status["finalizedHeight"]) == quorum_height
        and status["stateRoot"] == quorum_root
        for status in final.values()
    )
    print(
        json.dumps(
            {
                "healthy": converged,
                "quorumHeight": quorum_height,
                "quorumStateRoot": quorum_root,
                "responding": len(final),
                "unavailable": len(unavailable),
                "repaired": repaired,
            },
            sort_keys=True,
        )
    )
    return 0 if converged else 1


if __name__ == "__main__":
    raise SystemExit(main())
