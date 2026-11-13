"""Materialize one immutable action plan for both ordinary-protocol adapters.

No process, service, account, network or mutable game data is touched here.
Navigation must come from the actual original library and the same raw map.
"""
from __future__ import annotations

import argparse
import base64
import binascii
import copy
import hashlib
import json
import math
from pathlib import Path

HERE = Path(__file__).resolve().parent
DEFINITION = HERE / "workload.v2.json"


def require(condition, code):
    if not condition:
        raise ValueError(code)


def canonical_bytes(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False, allow_nan=False).encode("utf-8")


def sha256(value):
    return hashlib.sha256(canonical_bytes(value)).hexdigest()


def definition():
    return json.loads(DEFINITION.read_text(encoding="utf-8"))


def groups(population):
    require(type(population) is int and population in definition()["populations"], "unsupported_population")
    result = []
    current = 0
    while current < population:
        size = 3 if population - current == 3 else 2
        result.append(list(range(current, current + size)))
        current += size
    return result


def validate_navigation(nav, rules):
    require(nav.get("schema") == "mir2.comparable.navigation.v1", "navigation_schema")
    require(canonical_bytes(nav.get("map")) == canonical_bytes(rules["map"]), "navigation_map_identity")
    require(nav.get("source") == "original-library-cell-valid-and-movement-exclusion", "navigation_origin")
    require(nav.get("gameCodeUnmodified") is True, "original_game_code_modified")
    width, height = rules["map"]["width"], rules["map"]["height"]
    require(nav.get("encoding") == "row-major-lsb0-bitset", "navigation_encoding")
    require(type(nav.get("eligibleBitsBase64")) is str, "navigation_bitset")
    try:
        bits = base64.b64decode(nav["eligibleBitsBase64"], validate=True)
    except (ValueError, binascii.Error) as error:
        raise ValueError("navigation_base64") from error
    require(len(bits) == (width * height + 7) // 8, "navigation_bitset_length")
    count = sum(value.bit_count() for value in bits)
    require(type(nav.get("eligibleCount")) is int and nav["eligibleCount"] == count and count > 0, "navigation_eligible_count")
    require(canonical_bytes(nav.get("exclusions")) == canonical_bytes({"movementRadius": 3, "npcRadius": 3, "allDoorCells": True, "safeZones": True}), "navigation_transition_exclusions")
    cells = ["".join("1" if bits[(y * width + x) // 8] & (1 << ((y * width + x) % 8)) else "0"
                     for x in range(width)) for y in range(height)]
    return cells


def find_rectangle(cells, width, height, used=None):
    used = used or set()
    for y in range(4, len(cells) - height - 3):
        for x in range(4, len(cells[0]) - width - 3):
            if all(cells[yy][xx] == "1" and (xx, yy) not in used
                   for yy in range(y, y + height) for xx in range(x, x + width)):
                return x, y
    raise ValueError("no_verified_layout")


def actor_layout(population, scenario, cells):
    members = groups(population)
    actors = []
    if scenario == "dense-movement-chat":
        columns = min(10, population)
        ox, oy = find_rectangle(cells, columns * 2, math.ceil(population / columns))
        for group_id, group in enumerate(members):
            for ordinal in group:
                x, y = ox + (ordinal % columns) * 2, oy + ordinal // columns
                actors.append({"ordinal": ordinal, "group": group_id,
                               "peerOrdinals": [other for other in group if other != ordinal],
                               "start": {"x": x, "y": y},
                               "lane": [{"x": x, "y": y}, {"x": x + 1, "y": y}]})
    else:
        used = set()
        for group_id, group in enumerate(members):
            x, y = find_rectangle(cells, 2, 2, used)
            # Exclude a Chebyshev40 neighborhood from subsequent group origins.
            for yy in range(max(0, y - 40), min(len(cells), y + 42)):
                for xx in range(max(0, x - 40), min(len(cells[0]), x + 42)):
                    used.add((xx, yy))
            offsets = [(0, 0), (1, 0), (0, 1)]
            for slot, ordinal in enumerate(group):
                dx, dy = offsets[slot]
                pos = {"x": x + dx, "y": y + dy}
                actors.append({"ordinal": ordinal, "group": group_id,
                               "peerOrdinals": [other for other in group if other != ordinal],
                               "start": pos, "lane": [dict(pos), dict(pos)],
                               "attackTargetOrdinal": group[(slot + 1) % len(group)]})
    return sorted(actors, key=lambda actor: actor["ordinal"])


def materialize(nav, population, scenario="dense-movement-chat", purpose="functional", layer="game-core-local", *, navigation_file_sha256):
    rules = definition()
    require(scenario in rules["scenarios"], "unsupported_scenario")
    require(purpose in {"functional", "capacity"}, "unsupported_purpose")
    require((purpose == "functional" and population == 2) or (purpose == "capacity" and population in {25, 50, 100}), "purpose_population")
    require(layer in rules["qualificationLayers"], "unsupported_layer")
    require(type(navigation_file_sha256) is str and len(navigation_file_sha256) == 64 and
            set(navigation_file_sha256) <= set("0123456789abcdef"), "actual_navigation_file_sha_required")
    cells = validate_navigation(nav, rules)
    actors = actor_layout(population, scenario, cells)
    duration = rules["functionalDurationMs" if purpose == "functional" else "capacityDurationMs"]
    scenario_rules = rules["scenarios"][scenario]
    kind = "walk" if scenario == "dense-movement-chat" else "attack"
    interval = scenario_rules[kind + "IntervalMs"]
    require(scenario_rules.get("chatPhaseRule") == "same-primary-phase-with-midpoint-offset", "explicit_chat_phase_required")
    require(interval % 2 == 0 and scenario_rules["chatFirstOffsetMs"] % interval == interval // 2
            and scenario_rules["chatIntervalMs"] % interval == 0, "chat_must_stay_at_primary_midpoint")
    definition_file_sha = hashlib.sha256(DEFINITION.read_bytes()).hexdigest()
    generator_sha = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    seed = sha256({"definition": definition_file_sha, "generator": generator_sha, "navigation": navigation_file_sha256, "population": population,
                   "scenario": scenario, "purpose": purpose, "layer": layer})[:20]
    actions = []
    for actor in actors:
        ordinal, ticket = actor["ordinal"], 0
        due = ordinal * interval // population
        step = 0
        scheduled = []
        while due < duration:
            action = {"actorOrdinal": ordinal, "kind": kind, "dueMs": due,
                      "deadlineMs": due + rules["thresholds"][kind + "DeadlineMs"]}
            if kind == "walk":
                action.update(direction="Right" if step % 2 == 0 else "Left", target=dict(actor["lane"][1 if step % 2 == 0 else 0]))
            else:
                action["targetActorOrdinal"] = actor["attackTargetOrdinal"]
            scheduled.append(action)
            due += interval
            step += 1
        # Use the SAME actor phase as its primary action. A separate 1000-ms
        # population spread previously placed actor99's chat only10ms after
        # its walk at100 players, inside one Windows scheduling quantum.
        due = scenario_rules["chatFirstOffsetMs"] + ordinal * interval // population
        while due < duration:
            scheduled.append({"actorOrdinal": ordinal, "kind": "chat", "dueMs": due,
                              "deadlineMs": due + rules["thresholds"]["chatDeadlineMs"]})
            due += scenario_rules["chatIntervalMs"]
        for action in sorted(scheduled, key=lambda row: (row["dueMs"], row["kind"])):
            ticket += 1
            action["ticketSequence"] = ticket
            if action["kind"] == "chat":
                action["nonce"] = f"m2cmp:{seed}:{ordinal}:{ticket}"
                action["nonceSha256"] = hashlib.sha256(action["nonce"].encode()).hexdigest()
            actions.append(action)
    actions.sort(key=lambda row: (row["dueMs"], row["actorOrdinal"], row["ticketSequence"]))
    result = {"schema": "mir2.comparable.plan.v1", "definitionSha256": definition_file_sha,
              "generatorSourceSha256": generator_sha,
              "definitionCanonicalSha256": sha256(rules), "navigationSha256": navigation_file_sha256,
              "navigationCanonicalSha256": sha256(nav), "planId": seed, "scenario": scenario, "purpose": purpose,
              "qualificationLayer": layer, "population": population, "durationMs": duration,
              "map": copy.deepcopy(rules["map"]), "actorTemplate": rules["actorTemplate"],
              "actors": actors, "actions": actions, "thresholds": rules["thresholds"],
              "resourceContract": rules["resourceContract"], "causality": rules["causality"],
              "scheduling": {"version": 2, "chatPhaseRule": scenario_rules["chatPhaseRule"],
                             "primaryIntervalMs": interval, "minimumSameActorOfferGapMs": interval // 2,
                             "chatOffsetWithinPrimaryIntervalMs": interval // 2},
              "assessment": {"windowMs": 60000, "perActor": True, "allCompleteWindowsRequired": True,
                             "failedActionCanBecomeCompleted": False, "notSentIsTimeout": False,
                             "notSentIsCompleted": False, "safetyOrEvidenceFailureIsHardFailure": True},
              "equipmentActorOrdinals": list(range(min(3, population))), "equipmentRoundsPerActor": 2,
              "settleBeforeLogoutMs": rules["settleBeforeLogoutMs"],
              "capacityQualificationEligible": layer != "game-core-local" and purpose == "capacity",
              "capacityClaimAllowed": False}
    return result


def validate_plan(plan, nav, *, navigation_file_sha256):
    require(type(plan) is dict and plan.get("schema") == "mir2.comparable.plan.v1", "plan_schema")
    expected = materialize(nav, plan.get("population"), plan.get("scenario"), plan.get("purpose"),
                           plan.get("qualificationLayer"), navigation_file_sha256=navigation_file_sha256)
    require(canonical_bytes(plan) == canonical_bytes(expected), "materialized_plan_changed")
    return expected


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--navigation", type=Path, required=True)
    parser.add_argument("--population", type=int, required=True)
    parser.add_argument("--scenario", choices=definition()["scenarios"], default="dense-movement-chat")
    parser.add_argument("--purpose", choices=["functional", "capacity"], default="functional")
    parser.add_argument("--layer", choices=definition()["qualificationLayers"], default="game-core-local")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    navigation_bytes = args.navigation.read_bytes()
    plan = materialize(json.loads(navigation_bytes), args.population, args.scenario, args.purpose, args.layer,
                       navigation_file_sha256=hashlib.sha256(navigation_bytes).hexdigest())
    with args.output.open("x", encoding="utf-8", newline="\n") as stream:
        json.dump(plan, stream, indent=2, ensure_ascii=False, allow_nan=False)
        stream.write("\n")
    print(json.dumps({"plan": str(args.output), "sha256": hashlib.sha256(args.output.read_bytes()).hexdigest(),
                      "actors": plan["population"], "actions": len(plan["actions"]), "capacityClaimAllowed": plan["capacityClaimAllowed"]}))


if __name__ == "__main__":
    main()
