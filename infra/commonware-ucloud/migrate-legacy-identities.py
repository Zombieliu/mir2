#!/usr/bin/env python3
"""Resume-safe migration of legacy Postgres identities into Gate 14 consensus."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import time
import urllib.error
import urllib.request
from collections import Counter
from pathlib import Path
from typing import Any


def request_json(
    method: str, url: str, body: dict[str, Any] | None = None, timeout: int = 20
) -> dict[str, Any]:
    payload = None if body is None else json.dumps(body).encode()
    request = urllib.request.Request(
        url,
        data=payload,
        method=method,
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(request, timeout=timeout) as response:
        return json.loads(response.read())


def env_value(path: Path, name: str) -> str:
    for raw_line in path.read_text().splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, value = line.split("=", 1)
        if key == name:
            return value
    raise RuntimeError(f"{name} is missing from {path}")


def query_json_lines(database_url: str, sql: str) -> list[dict[str, Any]]:
    result = subprocess.run(
        ["psql", database_url, "-X", "-q", "-A", "-t", "-c", sql],
        check=True,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    return [json.loads(line) for line in result.stdout.splitlines() if line.strip()]


def short_hash(value: str) -> str:
    return hashlib.sha256(value.encode()).hexdigest()[:16]


def unique_control_name(
    original: str, account_id: str, character_index: int, duplicate_names: set[str]
) -> str:
    if original not in duplicate_names:
        return original
    suffix = "~" + short_hash(f"{account_id}:{character_index}")[:8]
    encoded = original.encode()
    maximum_prefix_bytes = 256 - len(suffix.encode())
    if len(encoded) > maximum_prefix_bytes:
        encoded = encoded[:maximum_prefix_bytes]
        while True:
            try:
                original = encoded.decode()
                break
            except UnicodeDecodeError:
                encoded = encoded[:-1]
    return original + suffix


def submit(control_url: str, command: dict[str, Any], key: str) -> None:
    last_error: Exception | None = None
    for attempt in range(8):
        try:
            status = request_json("GET", f"{control_url}/v1/status")
            request_json(
                "POST",
                f"{control_url}/v1/control/commands",
                {
                    "sequence": int(status["finalizedHeight"]) + 1,
                    "idempotencyKey": key,
                    "submittedAtMs": int(time.time() * 1000),
                    "command": command,
                },
                timeout=30,
            )
            return
        except (urllib.error.URLError, TimeoutError, KeyError, ValueError) as error:
            last_error = error
            time.sleep(min(0.5 * (attempt + 1), 3))
    raise RuntimeError(f"Gate 14 command failed after retries: {last_error}")


def prestage(
    validator_urls: list[str],
    base_sequence: int,
    commands: list[tuple[str, dict[str, Any]]],
    progress_every: int,
) -> int:
    # The fixed timestamp makes an interrupted prestage idempotent. It is in
    # the past, so every prepared command is immediately eligible to propose.
    submitted_at_ms = 1_700_000_000_000
    for offset, (key, command) in enumerate(commands, start=1):
        envelope = {
            "sequence": base_sequence + offset,
            "idempotencyKey": key,
            "submittedAtMs": submitted_at_ms,
            "command": command,
        }
        accepted = 0
        last_error: Exception | None = None
        for validator_url in validator_urls:
            try:
                request_json(
                    "POST", f"{validator_url}/v1/commands", envelope, timeout=20
                )
                accepted += 1
            except (urllib.error.URLError, TimeoutError, ValueError) as error:
                last_error = error
        if accepted < 3:
            raise RuntimeError(
                f"sequence {base_sequence + offset} reached only "
                f"{accepted}/4 validators: {last_error}"
            )
        if offset % progress_every == 0:
            print(
                json.dumps(
                    {
                        "prestaged": offset,
                        "total": len(commands),
                        "targetHeight": base_sequence + len(commands),
                    }
                ),
                flush=True,
            )
    return base_sequence + len(commands)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--gateway-env", type=Path, required=True)
    parser.add_argument(
        "--database-url-key", default="MIR2_ACCOUNT_STORE_DATABASE_URL"
    )
    parser.add_argument("--control-url", default="http://127.0.0.1:19500")
    parser.add_argument("--validator-url", default="http://127.0.0.1:19400")
    parser.add_argument(
        "--validator-urls",
        default=(
            "http://127.0.0.1:19400,http://127.0.0.1:19401,"
            "http://127.0.0.1:19402,http://127.0.0.1:19403"
        ),
    )
    parser.add_argument(
        "--prestage",
        action="store_true",
        help="prepare all remaining commands on the validators before waiting",
    )
    parser.add_argument("--progress-every", type=int, default=25)
    args = parser.parse_args()

    database_url = env_value(args.gateway_env, args.database_url_key)
    accounts = query_json_lines(
        database_url,
        "SELECT json_build_object('accountId', account_id) FROM accounts "
        "ORDER BY account_id",
    )
    characters = query_json_lines(
        database_url,
        "SELECT json_build_object("
        "'accountId', account_id, "
        "'characterIndex', character_index, "
        "'name', character_name"
        ") FROM characters ORDER BY account_id, character_index",
    )
    state = request_json("GET", f"{args.validator_url}/v1/state")
    existing_accounts = state.get("accounts", {})
    duplicate_names = {
        name
        for name, count in Counter(row["name"] for row in characters).items()
        if count > 1
    }
    total = len(accounts) + len(characters)
    completed = 0
    submitted = 0

    if args.prestage:
        commands: list[tuple[str, dict[str, Any]]] = []
        virtual_accounts = json.loads(json.dumps(existing_accounts))
        for row in accounts:
            account_id = row["accountId"]
            if account_id not in virtual_accounts:
                commands.append(
                    (
                        f"legacy-account-{short_hash(account_id)}",
                        {"type": "createAccount", "accountId": account_id},
                    )
                )
                virtual_accounts[account_id] = {"characters": {}}
        for row in characters:
            account_id = row["accountId"]
            character_index = int(row["characterIndex"])
            character_id = f"{account_id}:{character_index}"
            existing_characters = virtual_accounts[account_id].get("characters", {})
            if character_id not in existing_characters:
                commands.append(
                    (
                        f"legacy-character-{short_hash(character_id)}",
                        {
                            "type": "createCharacter",
                            "accountId": account_id,
                            "characterId": character_id,
                            "name": unique_control_name(
                                row["name"],
                                account_id,
                                character_index,
                                duplicate_names,
                            ),
                        },
                    )
                )
                existing_characters[character_id] = {}
        base_sequence = int(state["lastSequence"])
        target_height = prestage(
            [url.strip() for url in args.validator_urls.split(",") if url.strip()],
            base_sequence,
            commands,
            args.progress_every,
        )
        submitted = len(commands)
        while True:
            status = request_json("GET", f"{args.validator_url}/v1/status")
            finalized_height = int(status["finalizedHeight"])
            print(
                json.dumps(
                    {
                        "finalizing": finalized_height,
                        "targetHeight": target_height,
                    }
                ),
                flush=True,
            )
            if finalized_height >= target_height:
                break
            time.sleep(1)

    else:
        for row in accounts:
            account_id = row["accountId"]
            if account_id not in existing_accounts:
                submit(
                    args.control_url,
                    {"type": "createAccount", "accountId": account_id},
                    f"legacy-account-{short_hash(account_id)}",
                )
                existing_accounts[account_id] = {"characters": {}}
                submitted += 1
            completed += 1
            if completed % args.progress_every == 0:
                print(
                    json.dumps(
                        {
                            "completed": completed,
                            "total": total,
                            "submitted": submitted,
                        }
                    ),
                    flush=True,
                )

        for row in characters:
            account_id = row["accountId"]
            character_index = int(row["characterIndex"])
            character_id = f"{account_id}:{character_index}"
            existing_characters = existing_accounts[account_id].get("characters", {})
            if character_id not in existing_characters:
                submit(
                    args.control_url,
                    {
                        "type": "createCharacter",
                        "accountId": account_id,
                        "characterId": character_id,
                        "name": unique_control_name(
                            row["name"],
                            account_id,
                            character_index,
                            duplicate_names,
                        ),
                    },
                    f"legacy-character-{short_hash(character_id)}",
                )
                existing_characters[character_id] = {}
                submitted += 1
            completed += 1
            if completed % args.progress_every == 0:
                print(
                    json.dumps(
                        {
                            "completed": completed,
                            "total": total,
                            "submitted": submitted,
                        }
                    ),
                    flush=True,
                )

    final_state = request_json("GET", f"{args.validator_url}/v1/state")
    final_status = request_json("GET", f"{args.validator_url}/v1/status")
    migrated_accounts = final_state.get("accounts", {})
    migrated_characters = sum(
        len(account.get("characters", {})) for account in migrated_accounts.values()
    )
    evidence = {
        "accepted": (
            len(migrated_accounts) >= len(accounts)
            and migrated_characters >= len(characters)
        ),
        "sourceAccounts": len(accounts),
        "sourceCharacters": len(characters),
        "finalizedAccounts": len(migrated_accounts),
        "finalizedCharacters": migrated_characters,
        "duplicateLegacyNamesCanonicalized": len(duplicate_names),
        "commandsSubmittedThisRun": submitted,
        "finalizedHeight": final_state["finalizedHeight"],
        "stateRoot": final_status["stateRoot"],
        "finishedAtMs": int(time.time() * 1000),
    }
    print(json.dumps(evidence, indent=2), flush=True)
    return 0 if evidence["accepted"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
