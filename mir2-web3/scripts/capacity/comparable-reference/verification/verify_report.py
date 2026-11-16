"""Independent, offline verification of the common core-local workload evidence.

This module opens public plan/report/packet/resource projections only. It never
starts a game, imports an adapter scorer, reads account stores or uses a network.
Missing proof is a failure, not a zero or an inferred success. See README.md.
"""
from __future__ import annotations

import argparse
import copy
from collections import defaultdict
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import re

HERE = Path(__file__).resolve().parent
COMMON = HERE.parent
HEX = re.compile(r"[0-9a-f]{64}\Z")
MAX_FILE_BYTES = 128 * 2**20
MAX_ROWS = 2_000_000
MAX_EVENT_FILE_BYTES = 1024 * 2**20
MAX_EVENT_ROWS = 8_000_000
MAX_EVENT_LINE_BYTES = 2 * 2**20
# The actual 100-player all-observer trace exceeds the first retained-evidence
# budget. A separate 4-GiB verifier Job measured 625MB commit before that bound;
# this explicit 2x envelope is still guarded by the same 4-GiB actual-run cap.
MAX_SELECTED_ROWS = 500_000
MAX_SELECTED_BYTES = 256 * 2**20
MAX_REFERENCE_KEYS = 100_000
COMBAT_SHA = "8b373b5d03a71c468934d3591c6d06ad65ce6fe2eeb3208acca39ee0ae25a25b"
COMBAT_CANONICAL_SHA = "8cd44de50283077d0cbc9991abb9af531894000d0e624146c64eb2b34e6dfd32"
SLOTS = ["weapon", "armour", "helmet", "torch", "necklace", "braceletLeft", "braceletRight", "ringLeft", "ringRight", "amulet", "belt", "boots", "stone", "mount"]


class EvidenceError(ValueError):
    pass


def need(condition, code):
    if not condition:
        raise EvidenceError(code)


def number(value):
    return type(value) in (int, float) and math.isfinite(value)


def integer(value):
    return type(value) is int and value >= 0


def digest(blob):
    return hashlib.sha256(blob).hexdigest()


def encoded(value, *, sorted_keys=False):
    return json.dumps(value, sort_keys=sorted_keys, ensure_ascii=False, separators=(",", ":"), allow_nan=False).encode("utf-8")


def no_duplicates(pairs):
    result = {}
    for key, value in pairs:
        need(key not in result, "duplicate_json_key")
        result[key] = value
    return result


def decode(blob):
    def finite_float(text):
        value = float(text)
        need(math.isfinite(value), "nonfinite_json")
        return value
    try:
        return json.loads(blob, object_pairs_hook=no_duplicates,
                          parse_float=finite_float,
                          parse_constant=lambda unused: (_ for _ in ()).throw(EvidenceError("nonfinite_json")))
    except (UnicodeError, json.JSONDecodeError) as error:
        raise EvidenceError("invalid_json") from error


def read_file(path, *, jsonl=False):
    path = Path(path)
    need(not path.is_symlink() and path.is_file(), "regular_input_file_required")
    before = path.stat()
    need(before.st_size <= MAX_FILE_BYTES, "input_size_bound")
    with path.open("rb") as stream:
        blob = stream.read(MAX_FILE_BYTES + 1)
    after = path.stat()
    need(len(blob) <= MAX_FILE_BYTES and len(blob) == before.st_size == after.st_size
         and before.st_mtime_ns == after.st_mtime_ns, "input_changed_or_exceeded_bound")
    if jsonl:
        need(not blob or blob.endswith(b"\n"), "truncated_jsonl_last_line")
        lines = blob.splitlines()
        need(len(lines) <= MAX_ROWS and all(lines), "jsonl_row_bound_or_blank_line")
        value = [decode(line) for line in lines]
        need(all(type(row) is dict for row in value), "jsonl_object_required")
    else:
        value = decode(blob)
        need(type(value) is dict, "json_object_required")
    return value, {"sha256": digest(blob), "bytes": len(blob)}


class Checks:
    def __init__(self):
        self.failures = []

    def require(self, condition, code, actor=None, ticket=None):
        if not condition:
            row = {"code": code}
            if integer(actor): row["actorOrdinal"] = actor
            if integer(ticket): row["ticketSequence"] = ticket
            if row not in self.failures: self.failures.append(row)
        return bool(condition)


def p95(values):
    return sorted(values)[math.ceil(len(values) * .95) - 1] if values else None


def validate_plan(plan):
    scheduling = plan.get("scheduling")
    version = scheduling.get("version") if type(scheduling) is dict else 1
    need(type(version) is int and version in [1, 2] and (version != 1 or scheduling is None), "unsupported_scheduling_version")
    definition_bytes = (COMMON / f"workload.v{version}.json").read_bytes()
    rules = decode(definition_bytes)
    need(rules.get("schema") == f"mir2.comparable.workload.v{version}", "definition_schema_mismatch")
    if version == 2:
        legacy = decode((COMMON / "workload.v1.json").read_bytes())
        comparable = decode(encoded(rules)); comparable["schema"] = legacy["schema"]
        for row in comparable["scenarios"].values(): row.pop("chatPhaseRule", None)
        need(comparable == legacy, "phase_revision_changed_non_phase_contract")
        need(plan.get("definitionSha256") == digest(definition_bytes)
             and plan.get("definitionCanonicalSha256") == digest(encoded(rules, sorted_keys=True)), "explicit_definition_v2_hash_mismatch")
    need(plan.get("schema") == "mir2.comparable.plan.v1", "unsupported_plan_schema")
    need(plan.get("qualificationLayer") == "game-core-local" and plan.get("scenario") in ["dense-movement-chat", "dispersed-melee"], "unsupported_layer_or_scenario")
    population, duration = plan.get("population"), plan.get("durationMs")
    need((plan.get("purpose") == "functional" and population == 2 and duration == 60000)
         or (plan.get("purpose") == "capacity" and population in [25, 50, 100] and duration == 300000), "purpose_population_duration")
    need(plan.get("thresholds") == rules["thresholds"] and plan.get("resourceContract") == rules["resourceContract"]
         and plan.get("causality") == rules["causality"] and plan.get("map") == rules["map"]
         and plan.get("actorTemplate") == rules["actorTemplate"], "common_contract_drift")
    need(plan.get("assessment") == {"windowMs": 60000, "perActor": True, "allCompleteWindowsRequired": True,
         "failedActionCanBecomeCompleted": False, "notSentIsTimeout": False, "notSentIsCompleted": False,
         "safetyOrEvidenceFailureIsHardFailure": True}, "assessment_contract_drift")
    need(plan.get("capacityClaimAllowed") is False and plan.get("settleBeforeLogoutMs") == 11000, "claim_or_settle_contract")
    need(plan.get("equipmentActorOrdinals") == list(range(min(3, population))) and plan.get("equipmentRoundsPerActor") == 2, "equipment_contract")
    melee = plan["scenario"] == "dispersed-melee"
    if melee:
        need(version == 2 and plan.get("combatContractSha256") == COMBAT_SHA
             and plan.get("combatContractCanonicalSha256") == COMBAT_CANONICAL_SHA
             and digest(encoded(plan.get("combatContract"), sorted_keys=True)) == COMBAT_CANONICAL_SHA, "fixed_external_combat_contract_required")
    scenario = rules["scenarios"][plan["scenario"]]
    interval = scenario["attackIntervalMs" if melee else "walkIntervalMs"]
    if version == 2:
        need(scheduling == {"version": 2, "chatPhaseRule": "same-primary-phase-with-midpoint-offset",
             "primaryIntervalMs": interval, "minimumSameActorOfferGapMs": interval // 2,
             "chatOffsetWithinPrimaryIntervalMs": interval // 2}
             and scenario.get("chatPhaseRule") == scheduling["chatPhaseRule"] and interval % 2 == 0
             and scenario["chatFirstOffsetMs"] % interval == interval // 2
             and scenario["chatIntervalMs"] % interval == 0, "explicit_common_midpoint_phase_required")
    actors, actions = plan.get("actors"), plan.get("actions")
    need(type(actors) is list and len(actors) == population and type(actions) is list, "plan_actor_action_lists")
    occupied, expected = set(), []
    for ordinal, actor in enumerate(actors):
        peers, lane = actor.get("peerOrdinals"), actor.get("lane")
        need(actor.get("ordinal") == ordinal and type(peers) is list and peers and len(set(peers)) == len(peers)
             and all(integer(p) and p < population and p != ordinal for p in peers), "plan_peer_namespace")
        need(type(lane) is list and len(lane) == 2 and lane[0] == actor.get("start")
             and all(type(p) is dict and type(p.get("x")) is int and type(p.get("y")) is int and 0 < p["x"] < 800 and 0 < p["y"] < 800 for p in lane)
             and lane[1] == (lane[0] if melee else {"x": lane[0]["x"] + 1, "y": lane[0]["y"]}), "plan_movement_lane")
        if melee:
            target = actor.get("attackTargetOrdinal")
            need(target in peers and max(abs(actors[target]["start"][k] - lane[0][k]) for k in ["x", "y"]) == 1, "melee_target_not_adjacent_declared_peer")
        for cell in (lane[:1] if melee else lane):
            key = (cell["x"], cell["y"])
            need(key not in occupied, "overlapping_plan_lanes"); occupied.add(key)
        offers = []
        for step, due in enumerate(range(ordinal * interval // population, duration, interval)):
            offers.append(({"actorOrdinal": ordinal, "kind": "attack", "dueMs": due, "deadlineMs": due + rules["thresholds"]["attackDeadlineMs"], "targetActorOrdinal": actor["attackTargetOrdinal"]} if melee else
                           {"actorOrdinal": ordinal, "kind": "walk", "dueMs": due, "deadlineMs": due + rules["thresholds"]["walkDeadlineMs"],
                            "direction": "Right" if step % 2 == 0 else "Left", "target": lane[1 if step % 2 == 0 else 0]}))
        chat_phase = ordinal * (interval if version == 2 else 1000) // population
        for due in range(scenario["chatFirstOffsetMs"] + chat_phase, duration, scenario["chatIntervalMs"]):
            offers.append({"actorOrdinal": ordinal, "kind": "chat", "dueMs": due, "deadlineMs": due + rules["thresholds"]["chatDeadlineMs"]})
        if version == 2:
            times = sorted(a["dueMs"] for a in offers)
            need(all(b - a >= interval // 2 for a, b in zip(times, times[1:])), "same_actor_offer_gap_below_declared_midpoint")
        for ticket, offer in enumerate(sorted(offers, key=lambda a: (a["dueMs"], a["kind"])), 1):
            offer["ticketSequence"] = ticket
            if offer["kind"] == "chat":
                nonce = f"m2cmp:{plan['planId']}:{ordinal}:{ticket}"
                offer.update(nonce=nonce, nonceSha256=digest(nonce.encode()))
            expected.append(offer)
    need(actions == sorted(expected, key=lambda a: (a["dueMs"], a["actorOrdinal"], a["ticketSequence"])), "complete_fixed_plan_required")
    return rules


def raw_packet(event, engine):
    if engine == "modern" and event.get("event") == "received":
        return {"actor": event.get("actorOrdinal"), "connection": event.get("connection"),
                "sequence": event.get("receiveSeq"), "at": event.get("atMs"),
                "packet": event.get("packet", event.get("type")), "body": event.get("payload", event)}
    if engine == "crystal" and event.get("kind") == "packet" and event.get("direction") == "received":
        return {"actor": event.get("actorOrdinal"), "connection": event.get("connection"),
                "sequence": event.get("sequence"), "at": event.get("monotonicMs"),
                "packet": event.get("packet"), "body": event.get("detail") or {}}
    return None


def receipt(value, engine):
    return {"actor": value.get("observerOrdinal" if engine == "modern" else "receiverActorOrdinal"),
            "connection": value.get("connection"), "sequence": value.get("receiveSeq" if engine == "modern" else "receiveSequence"),
            "at": value.get("atMs" if engine == "modern" else "receivedAtMs"), "packet": value.get("packet")}


def packet_key(value):
    return value.get("actor"), value.get("connection"), value.get("sequence")


class StreamedEvents(list):
    """Ordered required evidence; full-file facts are retained separately."""
    pass


def actual_event_times(event, engine):
    if engine == "modern":
        return [event["atMs"]] if event.get("event") in ["received", "sent", "admitted", "clientCloseStarted", "transportClosed"] and number(event.get("atMs")) else []
    times = [event["monotonicMs"]] if event.get("kind") in ["packet", "admitted"] and number(event.get("monotonicMs")) else []
    if event.get("kind") == "transportClosed":
        times += [event["result"][k] for k in ["startedAtMs", "closedAtMs"] if number(event.get("result", {}).get(k))]
    return times


def scan_event_file(path, visitor):
    path = Path(path)
    need(not path.is_symlink() and path.is_file(), "regular_event_file_required")
    before = path.stat()
    need(before.st_size <= MAX_EVENT_FILE_BYTES, "verifier_event_file_size_bound_not_server_failure")
    sha, count, size = hashlib.sha256(), 0, 0
    with path.open("rb") as stream:
        while True:
            line = stream.readline(MAX_EVENT_LINE_BYTES + 1)
            if not line: break
            need(len(line) <= MAX_EVENT_LINE_BYTES, "verifier_event_line_size_bound_not_server_failure")
            need(line.endswith(b"\n"), "truncated_jsonl_last_line")
            need(line.strip(), "blank_event_line")
            count += 1; size += len(line)
            need(count <= MAX_EVENT_ROWS and size <= MAX_EVENT_FILE_BYTES, "verifier_event_stream_bound_not_server_failure")
            sha.update(line)
            event = decode(line)
            need(type(event) is dict, "event_object_required")
            visitor(count, event, len(line))
    after = path.stat()
    identity = lambda value: (value.st_size, value.st_mtime_ns, value.st_ino, value.st_dev)
    need(identity(before) == identity(after) and size == before.st_size, "event_file_changed_during_scan")
    return {"sha256": sha.hexdigest(), "bytes": size, "rows": count}, identity(after)


def read_events(path, engine, population):
    """Two full bounded scans; only irrelevant raw packets are not retained."""
    need(engine in ["modern", "crystal"] and type(population) is int and 1 <= population <= 100, "event_engine_or_population_unsupported")
    refs, latest_snapshots, last_sequences = set(), {}, {}
    audit = {"anySynthetic": False, "rawPacketsAudited": 0, "firstAtMs": None, "lastAtMs": None, "combatDetected": False}
    control_rows = 0

    def add_ref(key):
        if all(integer(v) for v in key) and key[1] > 0 and key[2] > 0:
            refs.add(key)
            need(len(refs) <= MAX_REFERENCE_KEYS, "verifier_reference_bound_not_server_failure")

    def find_refs(value):
        stack = [value]
        while stack:
            current = stack.pop()
            if type(current) is dict:
                actor = current.get("observerOrdinal", current.get("receiverActorOrdinal", current.get("actorOrdinal")))
                seq = current.get("receiveSeq", current.get("receiveSequence"))
                add_ref((actor, current.get("connection"), seq))
                stack.extend(child for child in current.values() if type(child) in [dict, list])
            elif type(current) is list:
                stack.extend(child for child in current if type(child) in [dict, list])

    def first_pass(unused_line, event, unused_size):
        nonlocal control_rows
        audit["combatDetected"] |= event.get("kind") == "attackModeAll" or event.get("event") == "combatBaseline" or (event.get("event", event.get("kind")) in ["offered", "sent", "actionSent"] and event.get("action", event.get("kind")) == "attack")
        audit["anySynthetic"] |= event.get("synthetic") is True or event.get("liveEvidence") is False
        need(event.get("hardEvidence") is not True, "adapter_reported_hard_evidence_failure")
        for at in actual_event_times(event, engine):
            audit["firstAtMs"] = at if audit["firstAtMs"] is None else min(at, audit["firstAtMs"])
            audit["lastAtMs"] = at if audit["lastAtMs"] is None else max(at, audit["lastAtMs"])
        raw = raw_packet(event, engine)
        if raw:
            key = packet_key(raw)
            need(all(integer(v) for v in key) and key[0] < population and key[1] > 0 and key[2] > 0 and number(raw["at"]), "raw_packet_identity_invalid_or_duplicate")
            need(type(raw["packet"]) is str and raw["packet"], "raw_packet_type_missing")
            session = key[:2]
            need(key[2] > last_sequences.get(session, 0), "raw_receive_sequence_duplicate_or_regressed")
            last_sequences[session] = key[2]
            need(len(last_sequences) <= 1024, "verifier_connection_bound_not_server_failure")
            audit["rawPacketsAudited"] += 1
            if raw["packet"] == "worldSnapshot": latest_snapshots[session] = key
        else:
            control_rows += 1
            need(control_rows <= MAX_SELECTED_ROWS, "verifier_control_row_bound_not_server_failure")
            find_refs(event)
            # All modern original-gear namespaces and final owner snapshots.
            # Capture the exact last packet key at each mutation; no hash search.
            if engine == "modern" and event.get("event") == "sent" and event.get("command", {}).get("type") in ["removeItem", "equipItem", "logOut"]:
                latest = latest_snapshots.get((event.get("actorOrdinal"), event.get("connection")))
                if latest: add_ref(latest)

    first, first_identity = scan_event_file(path, first_pass)
    selected = StreamedEvents(); selected_bytes = 0
    required_types = {"Connected", "ClientVersion", "LoginSuccess", "StartGame", "MapInformation", "UserInformation", "UserLocation",
                      "LogOutSuccess", "LogOutFailed", "RemoveItem", "EquipItem", "ObjectPlayer", "ObjectRemove"}
    if audit["combatDetected"]:
        required_types |= {"worldSnapshot", "ObjectAttack", "ObjectRangeAttack", "Struck", "ObjectStruck", "DamageIndicator", "HealthChanged", "Death", "ObjectDied", "UseItem", "DuraChanged", "DeleteItem", "RefreshItem", "AddBuff", "RemoveBuff", "ChangeAMode"}

    def second_pass(unused_line, event, length):
        nonlocal selected_bytes
        raw = raw_packet(event, engine)
        if raw is None or packet_key(raw) in refs or raw["packet"] in required_types:
            selected_bytes += length
            need(len(selected) < MAX_SELECTED_ROWS and selected_bytes <= MAX_SELECTED_BYTES, "verifier_selected_evidence_bound_not_server_failure")
            selected.append(event)

    second, second_identity = scan_event_file(path, second_pass)
    need(first == second and first_identity == second_identity, "event_file_changed_between_full_scans")
    selected.total_rows = first["rows"]
    selected.any_synthetic = audit["anySynthetic"]
    selected.observed_span_ms = None if audit["firstAtMs"] is None else audit["lastAtMs"] - audit["firstAtMs"]
    selected.audit = {"strategy": "two-complete-scans-required-packets", "fullRows": first["rows"], "retainedRows": len(selected),
                      "retainedSerializedBytes": selected_bytes, "rawPacketsAudited": audit["rawPacketsAudited"],
                      "referenceKeys": len(refs), "fullFileHashesAndStatsMatched": True,
                      "rawFileByteLimit": MAX_EVENT_FILE_BYTES, "rowLimit": MAX_EVENT_ROWS,
                      "lineByteLimit": MAX_EVENT_LINE_BYTES, "retainedByteLimit": MAX_SELECTED_BYTES, "retainedRowLimit": MAX_SELECTED_ROWS}
    return selected, {**first, "streamAudit": selected.audit}


def index_evidence(events, engine, checks):
    packets, admissions, sends = {}, defaultdict(list), defaultdict(list)
    for index, event in enumerate(events):
        packet = raw_packet(event, engine)
        if packet:
            packet["index"] = index
            key = packet_key(packet)
            valid = all(integer(v) for v in key) and key[1] > 0 and key[2] > 0 and number(packet["at"])
            if checks.require(valid and key not in packets, "raw_packet_identity_invalid_or_duplicate", packet["actor"]):
                packets[key] = packet
        if event.get("event" if engine == "modern" else "kind") == "admitted":
            at = event.get("atMs" if engine == "modern" else "monotonicMs")
            admissions[event.get("actorOrdinal")].append((at, event.get("connection"), event))
        if (engine == "modern" and event.get("event") == "sent") or (engine == "crystal" and event.get("kind") == "packet" and event.get("direction") == "sent"):
            at = event.get("atMs" if engine == "modern" else "monotonicMs")
            sends[(event.get("actorOrdinal"), at)].append(event)
    for sessions in admissions.values():
        checks.require(all(number(row[0]) and integer(row[1]) and row[1] > 0 for row in sessions), "admission_clock_or_connection_missing")
        sessions.sort(key=lambda row: row[0] if number(row[0]) else -1)
        checks.require(len({row[1] for row in sessions}) == len(sessions), "connection_epoch_reused")
    return packets, admissions, sends


def read_dense_disk_state_events(path):
    """Additional File-closure view; the existing scoring two-pass reader stays unchanged."""
    required={"Connected","ClientVersion","LoginSuccess","StartGame","MapInformation","UserInformation","UserLocation","worldSnapshot",
              "LogOutSuccess","LogOutFailed","HealthChanged","DuraChanged","DeleteItem","UseItem","RefreshItem","RemoveItem","EquipItem",
              "NewItem","AddItem","GainGold","GainedGold","LoseGold","GoldChanged","GainedCredit","LoseCredit","CreditChanged","InventorySizeChanged","BagSizeChanged","Death"}
    selected=StreamedEvents();size=0;raw_count=0;types=defaultdict(int)
    def collect(unused_line,event,length):
        nonlocal size,raw_count
        raw=raw_packet(event,"modern")
        if raw:raw_count+=1;types[raw["packet"]]+=1
        if raw is None or raw["packet"] in required:
            size+=length
            need(len(selected)<MAX_SELECTED_ROWS and size<=MAX_SELECTED_BYTES,"verifier_dense_state_evidence_bound_not_server_failure")
            selected.append(event)
    metadata,unused_identity=scan_event_file(path,collect)
    selected.full_source_metadata=metadata
    selected.audit={"strategy":"additional-complete-scan-owner-state-view","fullRows":metadata["rows"],"retainedRows":len(selected),
                    "retainedSerializedBytes":size,"rawPacketsAudited":raw_count,"rawPacketTypes":dict(types),
                    "requiredPacketTypes":sorted(required),"scoringStreamChanged":False,"retainedByteLimit":MAX_SELECTED_BYTES,"retainedRowLimit":MAX_SELECTED_ROWS}
    return selected,{**metadata,"stateAudit":selected.audit}


def validate_dense_state_binding(events,metadata):
    need(isinstance(events,StreamedEvents) and type(getattr(events,"full_source_metadata",None))is dict,"dense_all_actor_state_pass_missing")
    need(all(events.full_source_metadata.get(k)==metadata.get(k) for k in ["sha256","bytes","rows"]),"dense_all_actor_state_pass_not_same_events_file")


def normalize_actions(events, engine, checks):
    records = defaultdict(lambda: {"sent": [], "outcomes": [], "receipts": [], "timeouts": [], "offers": [], "notSent": []})
    for event in events:
        kind = event.get("event" if engine == "modern" else "kind")
        if engine == "modern":
            if kind not in {"actionSent", "actionTerminal", "actionTimeout", "notSent", "sendUncertain"}: continue
            value = event
        else:
            if kind not in {"offered", "sent", "ownerReceipt", "peerReceipt", "timeout", "notSent", "actionOutcome", "sendOutcomeUnknown"}: continue
            value = event.get("result", event)
        actor, ticket = value.get("actorOrdinal"), value.get("ticketSequence")
        if not checks.require(integer(actor) and integer(ticket) and ticket > 0, "action_ticket_missing"):
            continue
        row = records[(actor, ticket)]
        if kind in {"sendUncertain", "sendOutcomeUnknown"}:
            checks.require(False, "uncertain_send_is_causally_ambiguous", actor, ticket)
        if value.get("hardEvidence") is True or value.get("sendOutcomeUnknown") is True:
            checks.require(False, "adapter_reported_hard_evidence_failure", actor, ticket)
        if kind in {"actionSent", "sent"}: row["sent"].append(value)
        if kind == "offered": row["offers"].append(value)
        if kind == "notSent": row["notSent"].append(value)
        if kind in {"actionTerminal", "actionOutcome"}: row["outcomes"].append(value)
        if kind in {"actionTimeout", "timeout"}: row["timeouts"].append(value)
        if kind == "actionTerminal": row["receipts"].extend(receipt(r, engine) for r in value.get("receipts", []))
        if kind in {"ownerReceipt", "peerReceipt"}: row["receipts"].append(receipt(value.get("detail", {}), engine))
    return records


def active_connection(admissions, actor, at):
    candidates = [row for row in admissions.get(actor, []) if number(row[0]) and row[0] <= at]
    return candidates[-1][1] if candidates else None


def verify_actions(plan, report, events, engine, checks):
    if plan.get("scenario") == "dispersed-melee":
        return verify_melee_actions(plan, report, events, engine, checks)
    epoch = report.get("measurementEpochMs" if engine == "modern" else "measurementStartedAtMs")
    end = report.get("observationEndedAtMs", report.get("measurementEndedAtMs"))
    need(number(epoch) and number(end), "actual_measurement_anchor_or_end_missing")
    checks.require(end >= epoch + plan["durationMs"], "full_measurement_duration_not_observed")
    packets, admissions, sends = index_evidence(events, engine, checks)
    records = normalize_actions(events, engine, checks)
    expected_keys = {(a["actorOrdinal"], a["ticketSequence"]) for a in plan["actions"]}
    checks.require(set(records) == expected_keys, "missing_or_unplanned_action_evidence")
    for actor in plan["actors"]:
        ordinal = actor["ordinal"]
        checks.require(active_connection(admissions, ordinal, epoch) is not None, "actor_not_admitted_before_measurement", ordinal)
        checks.require(not any(epoch < row[0] < epoch + plan["durationMs"] for row in admissions.get(ordinal, []) if number(row[0])), "connection_replaced_during_measurement", ordinal)
    derived, used_receipts, used_sends, fences = [], set(), set(), defaultdict(list)
    for action in plan["actions"]:
        actor, ticket = action["actorOrdinal"], action["ticketSequence"]
        row = records.get((actor, ticket))
        out = {**action, "sentAtMs": None, "completed": False, "timeout": False, "notSent": False, "rejected": False,
               "latencyMs": None, "scheduleDelayMs": None, "terminalAtMs": None}
        if row is None:
            derived.append(out); continue
        checks.require(len(row["sent"]) <= 1 and len(row["notSent"]) <= 1 and not (row["sent"] and row["notSent"]), "duplicate_send_or_sent_notSent_conflict", actor, ticket)
        if engine == "crystal": checks.require(len(row["offers"]) == 1, "offered_record_missing_or_duplicate", actor, ticket)
        spec = (row["sent"] or row["notSent"] or row["outcomes"] or row["timeouts"] or row["offers"])[0]
        planned = spec.get("dueAtMs" if engine == "modern" else "plannedAtMs")
        deadline = spec.get("deadlineAtMs")
        checks.require(number(planned) and planned == epoch + action["dueMs"] and deadline == epoch + action["deadlineMs"], "absolute_anchor_or_deadline_drift", actor, ticket)
        checks.require(spec.get("kind" if engine == "modern" else "action", spec.get("kind")) == action["kind"], "action_kind_drift", actor, ticket)
        if not row["sent"]:
            out["notSent"] = bool(row["notSent"])
            checks.require(out["notSent"] and not row["timeouts"] and not row["receipts"], "unsent_action_evidence_inconsistent", actor, ticket)
            derived.append(out); continue
        sent = row["sent"][0]; when = sent.get("sentAtMs")
        if not checks.require(number(when) and number(planned) and number(deadline), "actual_send_time_missing", actor, ticket):
            derived.append(out); continue
        out["sentAtMs"], out["scheduleDelayMs"] = when, when - planned
        checks.require(planned <= when <= deadline, "send_outside_original_absolute_budget", actor, ticket)
        raw_sends = sends.get((actor, when), [])
        if engine == "modern":
            raw_sends = [e for e in raw_sends if e.get("command", {}).get("type") == action["kind"] and
                         (e["command"].get("direction") == action.get("direction") if action["kind"] == "walk" else e["command"].get("message") == action["nonce"])]
        else:
            raw_sends = [e for e in raw_sends if e.get("sequence") == sent.get("sentSequence") and e.get("correlation", {}).get("ticketSequence") == ticket]
        checks.require(len(raw_sends) == 1, "unique_actual_send_packet_missing", actor, ticket)
        if len(raw_sends) == 1: used_sends.add(id(raw_sends[0]))
        connection = raw_sends[0].get("connection") if len(raw_sends) == 1 else None
        checks.require(connection == active_connection(admissions, actor, when), "send_connection_not_current_admission", actor, ticket)
        expected = [actor] + plan["actors"][actor]["peerOrdinals"]
        checks.require(sent.get("expectedPeerOrdinals") == plan["actors"][actor]["peerOrdinals"], "observer_set_drift", actor, ticket)
        floor_raw = sent.get("receiveSequenceFloors")
        floors = ({int(k): v for k, v in floor_raw.items()} if type(floor_raw) is dict and all(str(k).isdigit() for k in floor_raw)
                  else {r.get("actorOrdinal"): r.get("receiveSeq") for r in floor_raw} if type(floor_raw) is list else {})
        checks.require(set(floors) == set(expected) and all(integer(v) for v in floors.values()), "fresh_receive_floor_evidence_missing", actor, ticket)
        checks.require(type(floor_raw) in [dict, list] and len(floor_raw) == len(floors), "duplicate_receive_floor_observer", actor, ticket)
        if engine == "modern" and type(floor_raw) is list:
            checks.require(all(r.get("connection") == active_connection(admissions, r.get("actorOrdinal"), when) for r in floor_raw), "receive_floor_connection_epoch_missing_or_stale", actor, ticket)
        ids = ({r.get("observerOrdinal"): r.get("objectId") for r in sent.get("expectedSharedIds", [])}
               if engine == "modern" else {observer: sent.get("expectedSharedId") for observer in expected})
        if engine == "modern":
            id_rows = sent.get("expectedSharedIds", [])
            checks.require(len(id_rows) == len(ids) and set(ids) == set(expected if action["kind"] == "chat" else expected[1:])
                           and all(r.get("connection") == active_connection(admissions, r.get("observerOrdinal"), when) for r in id_rows), "shared_id_observer_or_connection_ambiguous", actor, ticket)
        if action["kind"] == "walk":
            origin = sent.get("before" if engine == "modern" else "origin")
            # Some modern actionSent versions expose actualTarget but keep before
            # only in the terminal projection; both are from the same unique gate.
            if origin is None and engine == "modern" and row["outcomes"]:
                origin = row["outcomes"][0].get("before")
            checks.require(type(origin) is dict and origin.get("y") == action["target"]["y"]
                           and number(origin.get("x")) and origin["x"] + (1 if action["direction"] == "Right" else -1) == action["target"]["x"], "walk_origin_direction_target_inconsistent", actor, ticket)
        seen = {}
        for ref in row["receipts"]:
            key = packet_key(ref)
            if key in seen: continue  # repeated identical terminal projection, not another receipt
            raw = packets.get(key)
            valid = raw is not None and raw["at"] == ref["at"] and raw["packet"] == ref["packet"]
            if not checks.require(valid, "receipt_without_matching_raw_packet", actor, ticket): continue
            checks.require(key not in used_receipts, "one_packet_reused_by_multiple_actions", actor, ticket)
            used_receipts.add(key); seen[key] = raw
            receiver = raw["actor"]
            checks.require(receiver in expected and raw["connection"] == active_connection(admissions, receiver, when)
                           and integer(floors.get(receiver)) and raw["sequence"] > floors.get(receiver, math.inf)
                           and raw["at"] >= when, "receipt_connection_floor_or_time_ambiguous", actor, ticket)
        grouped = defaultdict(list)
        for raw in seen.values(): grouped[raw["actor"]].append(raw)
        checks.require(all(len(values) == 1 for values in grouped.values()), "multiple_receipts_for_one_observer", actor, ticket)
        correct, rejected = {}, False
        for observer, values in grouped.items():
            raw = values[0]; body = raw["body"]
            location = body.get("location", body.get("position", {"x": body.get("x"), "y": body.get("y")}))
            if action["kind"] == "walk":
                required_packet = "UserLocation" if observer == actor else "ObjectWalk"
                matching = raw["packet"] == required_packet and location == action["target"]
                if observer != actor: matching &= integer(ids.get(observer)) and ids.get(observer) > 0 and body.get("objectId") == ids.get(observer)
                if observer == actor and raw["packet"] == "UserLocation" and location != action["target"]:
                    origin = sent.get("before" if engine == "modern" else "origin")
                    if origin is None and engine == "modern" and row["outcomes"]:
                        origin = row["outcomes"][0].get("before")
                    rejected = location == origin
                    checks.require(rejected, "unexpected_authoritative_owner_transform", actor, ticket)
            else:
                text = body.get("text", body.get("message"))
                nonce_match = (type(text) is str and text.endswith(action["nonce"]) and
                               (len(text) == len(action["nonce"]) or text[-len(action["nonce"])-1].isspace() or text[-len(action["nonce"])-1] == ":")) if engine == "modern" else body.get("nonceHash") == action["nonceSha256"]
                matching = raw["packet"] == "ObjectChat" and integer(ids.get(observer)) and ids.get(observer) > 0 and body.get("objectId") == ids.get(observer) and nonce_match
            checks.require(matching or (rejected and observer == actor), "raw_receipt_does_not_match_planned_action", actor, ticket)
            if matching: correct[observer] = raw["at"]
        complete_receipts = set(correct) == set(expected)
        last = max(correct.values()) if correct else None
        timeout_marked = bool(row["timeouts"]) or any(r.get("timedOut" if engine == "modern" else "timeout") is True for r in row["outcomes"])
        for timeout in row["timeouts"]:
            observed = timeout.get("atMs" if engine == "modern" else "observedAtMs")
            checks.require(number(observed) and observed >= deadline, "timeout_before_original_absolute_deadline", actor, ticket)
        out["rejected"] = rejected
        out["timeout"] = timeout_marked or (not rejected and (not complete_receipts or last > deadline))
        out["completed"] = complete_receipts and last <= deadline and not timeout_marked and not rejected
        if complete_receipts:
            out["terminalAtMs"] = last
        elif rejected:
            out["terminalAtMs"] = max(r["at"] for r in seen.values())
        if out["completed"]: out["latencyMs"] = last - when
        checks.require(out["terminalAtMs"] is not None, "sent_action_remains_causally_unresolved", actor, ticket)
        checks.require(row["outcomes"] or row["timeouts"], "original_outcome_not_recorded", actor, ticket)
        if timeout_marked and complete_receipts and last <= deadline:
            checks.require(False, "timeout_conflicts_with_original_on_time_receipts", actor, ticket)
        for outcome in row["outcomes"]:
            claimed = outcome.get("passed" if engine == "modern" else "completedOnTime")
            checks.require(type(claimed) is bool and claimed == out["completed"], "claimed_outcome_disagrees_with_raw_evidence", actor, ticket)
        fences[actor].append(out)
        derived.append(out)
    for actor, actions in fences.items():
        previous = None
        for action in sorted(actions, key=lambda a: a["sentAtMs"]):
            if previous is not None:
                checks.require(previous["terminalAtMs"] is not None and previous["terminalAtMs"] <= action["sentAtMs"], "new_send_before_previous_unique_terminal", actor, action["ticketSequence"])
            previous = action
    allowed = ({"clientVersion", "login", "startGame", "keepAlive", "logOut", "removeItem", "equipItem", "walk", "chat"}
               if engine == "modern" else {"ClientVersion", "Login", "StartGame", "KeepAlive", "LogOut", "RemoveItem", "EquipItem", "Walk", "Chat"})
    for rows in sends.values():
        for raw in rows:
            command = raw.get("command", {}).get("type") if engine == "modern" else raw.get("packet")
            checks.require(command in allowed, "unsupported_or_debug_command_in_run")
            if command in ["walk", "chat", "Walk", "Chat"]:
                checks.require(id(raw) in used_sends, "extra_gameplay_send_or_retry_outside_plan")
    windows = []
    for actor in range(plan["population"]):
        for minute in range(plan["durationMs"] // 60000):
            rows = [r for r in derived if r["actorOrdinal"] == actor and minute * 60000 <= r["dueMs"] < (minute + 1) * 60000]
            completed = [r for r in rows if r["completed"]]
            count, timeouts = len(rows), sum(r["timeout"] for r in rows)
            move = p95([r["latencyMs"] for r in completed if r["kind"] == "walk"])
            chat = p95([r["latencyMs"] for r in completed if r["kind"] == "chat"])
            schedule = p95([r["scheduleDelayMs"] for r in rows if r["sentAtMs"] is not None])
            threshold = plan["thresholds"]
            passed = (count > 0 and len(completed) / count >= threshold["minimumCompletionRatio"]
                      and timeouts / count < threshold["maximumTimeoutRatioExclusive"]
                      and move is not None and move <= threshold["movementP95Ms"]
                      and chat is not None and chat <= threshold["chatP95Ms"]
                      and schedule is not None and schedule <= threshold["scheduleP95Ms"])
            windows.append({"actorOrdinal": actor, "minute": minute + 1, "offered": count, "completed": len(completed),
                            "timeout": timeouts, "notSent": sum(r["notSent"] for r in rows), "rejected": sum(r["rejected"] for r in rows),
                            "completionRatio": len(completed) / count if count else None, "movementP95Ms": move,
                            "chatP95Ms": chat, "scheduleP95Ms": schedule, "passed": passed})
    return windows, derived, packets, admissions


def carried(value, engine):
    """Keep all projected item fields; no name/UID/item payload is exported."""
    need(type(value) is dict, "full_carried_state_missing")
    items = {}
    if engine == "modern":
        fields = ["inventoryItems", "equipmentItems", "beltItems"]
        need(all(type(value.get(key)) is list for key in fields)
             and all(number(value.get(key)) for key in ["gold", "inventoryCapacity", "maxBagSlots"]), "full_carried_state_missing")
        for grid in fields:
            for item in value[grid]:
                uid = item.get("uniqueId")
                need(integer(uid) and uid > 0 and uid not in items and "slot" in item, "item_ownership_or_slot_ambiguous")
                items[uid] = {"grid": grid, "slot": item["slot"], "payload": {k: v for k, v in item.items() if k != "slot"}}
        scalar = {key: value[key] for key in ["gold", "inventoryCapacity", "maxBagSlots"]}
        hash_object = {**scalar, **{name: sorted(value[key], key=lambda item: str(item["uniqueId"]))
                                 for name, key in zip(["inventory", "equipment", "belt"], fields)}}
        fingerprint = digest(encoded(hash_object, sorted_keys=True))
    else:
        owned = value.get("owned")
        need(type(owned) is dict and type(owned.get("slots")) is list and type(owned.get("capacity")) is dict
             and all(number(owned.get(k)) for k in ["gold", "credit"])
             and owned.get("beltRule") == "original Inventory indices0..5; alias of inventory, not duplicated item ownership", "full_native_item_state_missing")
        capacity = owned["capacity"]
        need(all(integer(capacity.get(k)) for k in ["inventoryIncludingBelt", "equipment", "questInventory", "belt", "bag"])
             and capacity["belt"] == min(6, capacity["inventoryIncludingBelt"])
             and capacity["bag"] == max(0, capacity["inventoryIncludingBelt"] - 6), "native_capacity_or_belt_alias_invalid")
        for item in owned["slots"]:
            uid, grid, slot = item.get("uniqueId"), item.get("grid"), item.get("slot")
            need(integer(uid) and uid > 0 and uid not in items and grid in ["inventory", "equipment", "questInventory"]
                 and integer(slot) and slot < capacity[{"inventory": "inventoryIncludingBelt", "equipment": "equipment", "questInventory": "questInventory"}[grid]]
                 and HEX.fullmatch(item.get("originalUserItemSaveSha256", "")) is not None, "native_item_fields_or_ownership_missing")
            items[uid] = {"grid": grid, "slot": slot, "payload": {k: v for k, v in item.items() if k not in ["grid", "slot"]}}
        scalar = {k: v for k, v in owned.items() if k != "slots"}
        # Original ItemState explicitly serializes this ordered object with Compact.
        fingerprint = digest(encoded(owned))
        need(value.get("fingerprintSha256") == fingerprint, "native_item_fingerprint_does_not_match_full_projection")
    need(len({(i["grid"], str(i["slot"])) for i in items.values()}) == len(items), "two_items_in_one_slot")
    return {"items": items, "scalars": scalar, "fingerprint": fingerprint}


def same_carried(first, second):
    return first["items"] == second["items"] and first["scalars"] == second["scalars"]


def complete_modern_user_item(entry):
    """The two grid DTOs differ; the complete original UserItem does not."""
    payload = entry["payload"]
    value = payload.get("tooltipSource", {}).get("userItem")
    fields = {"unique_id", "item_index", "count", "current_dura", "max_dura", "added_stats", "awake_type",
              "awake_values", "cursed", "gem_count", "gm_made", "identified", "is_shop_item", "refined_value",
              "refine_added", "refine_success_chance", "rental_information", "sealed_info", "expire_info",
              "slots", "soul_bound_id", "wedding_ring"}
    need(type(value) is dict and fields <= set(value), "complete_original_UserItem_projection_missing")
    need(all(type(value[k]) is int for k in ["unique_id", "item_index", "count", "current_dura", "max_dura", "awake_type",
         "gem_count", "refined_value", "refine_added", "refine_success_chance", "wedding_ring", "soul_bound_id"])
         and all(type(value[k]) is bool for k in ["cursed", "gm_made", "identified", "is_shop_item"])
         and all(type(value[k]) is list for k in ["slots", "added_stats", "awake_values"]), "original_UserItem_field_types_missing_or_invalid")
    current = payload.get("durabilityCurrent")
    maximum = payload.get("durabilityMax")
    need(value["unique_id"] == payload.get("uniqueId") and value["count"] == payload.get("quantity")
         and value["current_dura"] == (0 if current is None else current)
         and value["max_dura"] == (0 if maximum is None else maximum), "original_UserItem_disagrees_with_grid_identity_or_fields")
    return {"key": payload.get("key"), "userItem": value}


def initial_weapon_uid(state, engine):
    selected = [(uid, item) for uid, item in state["items"].items()
                if item["grid"] == ("equipmentItems" if engine == "modern" else "equipment")
                and item["slot"] == ("weapon" if engine == "modern" else 0)]
    need(len(selected) == 1, "unique_initial_weapon_required")
    uid, item = selected[0]
    index = (complete_modern_user_item(item)["userItem"]["item_index"] if engine == "modern" else item["payload"].get("itemIndex"))
    need(index == 221, "common_initial_weapon_221_required")
    return uid


class Lifecycle:
    def __init__(self, engine, events, packets, checks):
        self.engine, self.events, self.packets, self.checks = engine, events, packets, checks

    def ref(self, value, actor, packet=None):
        value = receipt(value, self.engine)
        raw = self.packets.get(packet_key(value))
        need(raw is not None and raw["actor"] == actor and raw["at"] == value["at"]
             and raw["packet"] == (packet or value["packet"]), "lifecycle_receipt_raw_packet_missing")
        return raw

    def ui(self, actor, connection):
        values = [p for p in self.packets.values() if p["actor"] == actor and p["connection"] == connection and p["packet"] == "UserInformation"]
        need(len(values) == 1, "unique_fresh_UserInformation_missing")
        value = values[0]
        if self.engine == "modern":
            login_sends = [e for e in self.events if e.get("event") == "sent" and e.get("actorOrdinal") == actor and e.get("connection") == connection and e.get("command", {}).get("type") == "login"]
            start_sends = [e for e in self.events if e.get("event") == "sent" and e.get("actorOrdinal") == actor and e.get("connection") == connection and e.get("command", {}).get("type") == "startGame"]
            send_clock = "atMs"
        else:
            login_sends = [e for e in self.events if e.get("kind") == "packet" and e.get("direction") == "sent" and e.get("actorOrdinal") == actor and e.get("connection") == connection and e.get("packet") == "Login"]
            start_sends = [e for e in self.events if e.get("kind") == "packet" and e.get("direction") == "sent" and e.get("actorOrdinal") == actor and e.get("connection") == connection and e.get("packet") == "StartGame"]
            send_clock = "monotonicMs"
        logins = [p for p in self.packets.values() if p["actor"] == actor and p["connection"] == connection and p["packet"] == "LoginSuccess"]
        starts = [p for p in self.packets.values() if p["actor"] == actor and p["connection"] == connection and p["packet"] == "StartGame"]
        need(len(login_sends) == len(start_sends) == len(logins) == len(starts) == 1, "normal_login_StartGame_fresh_bootstrap_missing")
        need(number(login_sends[0].get(send_clock)) and number(start_sends[0].get(send_clock))
             and login_sends[0][send_clock] <= logins[0]["at"] <= start_sends[0][send_clock] <= starts[0]["at"] <= value["at"]
             and logins[0]["sequence"] < starts[0]["sequence"] < value["sequence"], "fresh_bootstrap_packet_order_invalid")
        if self.engine == "modern": need(starts[0]["body"].get("result") == 4, "fresh_StartGame_not_success")
        if self.engine == "crystal":
            body = value["body"]
            need(body.get("identityMatchesFixture") is True and integer(body.get("realCharacterIndex"))
                 and HEX.fullmatch(body.get("nameHash", "")) is not None and body.get("hp", 0) > 0
                 and body.get("level") == 40, "fresh_native_identity_or_owner_invalid")
            carried(body.get("authoritativeItemState"), "crystal")
        return value

    def snapshot(self, value, actor, full_carried):
        raw = self.ref(value, actor, "worldSnapshot")
        state = carried(full_carried, "modern")
        need(raw["body"].get("carriedFingerprint") == state["fingerprint"], "fresh_roster_not_bound_to_received_snapshot")
        need(raw["body"].get("map") == "DogYoHyun" and raw["body"].get("playerHp", 0) > 0, "fresh_snapshot_map_or_liveness_invalid")
        ui = self.ui(actor, raw["connection"])
        owner = raw["body"].get("owner", {})
        position = ui["body"].get("location", ui["body"])
        need(ui["sequence"] < raw["sequence"] and owner.get("level") == 40 and owner.get("dead") is not True
             and owner.get("x") == position.get("x") and owner.get("y") == position.get("y")
             and raw["body"].get("playerObjectId") == ui["body"].get("objectId"), "fresh_snapshot_not_bound_to_exact_UserInformation")
        return raw, state

    def exit(self, actor, connection, *, outer=None):
        """Verify actual send/ACK/close events, not elapsed/passed flags alone."""
        if self.engine == "modern":
            sent = [e for e in self.events if e.get("event") == "sent" and e.get("actorOrdinal") == actor and e.get("connection") == connection and e.get("command", {}).get("type") == "logOut"]
            starts = [e for e in self.events if e.get("event") == "clientCloseStarted" and e.get("actorOrdinal") == actor and e.get("connection") == connection]
            closes = [(i, e) for i, e in enumerate(self.events) if e.get("event") == "transportClosed" and e.get("actorOrdinal") == actor and e.get("connection") == connection]
            need(len(sent) == len(starts) == len(closes) == 1, "unique_normal_logout_or_close_missing")
            send_at, close_at = sent[0].get("atMs"), starts[0].get("atMs")
            closed = closes[0][1]
            need(closed.get("validCloseFrameReceived") is True and closed.get("code") != 1006, "real_WebSocket_Close_frame_missing")
            end = closed.get("atMs")
        else:
            sent = [e for e in self.events if e.get("kind") == "packet" and e.get("direction") == "sent" and e.get("actorOrdinal") == actor and e.get("connection") == connection and e.get("packet") == "LogOut"]
            closes = [(i, e["result"]) for i, e in enumerate(self.events) if e.get("kind") == "transportClosed" and e.get("result", {}).get("actorOrdinal") == actor and e.get("result", {}).get("connection") == connection]
            need(len(sent) == len(closes) == 1, "unique_normal_logout_or_close_missing")
            send_at = sent[0].get("monotonicMs"); closed = closes[0][1]
            close_at, end = closed.get("startedAtMs"), closed.get("closedAtMs")
            need(closed.get("localTcpCloseCompleted") is True and closed.get("receiveLoopCompleted") is True, "native_TCP_close_not_complete")
        acks = [p for p in self.packets.values() if p["actor"] == actor and p["connection"] == connection and p["packet"] == "LogOutSuccess"]
        need(len(acks) == 1 and all(number(t) for t in [send_at, close_at, end]), "normal_logout_timestamps_missing")
        ack = acks[0]
        need(send_at <= ack["at"] <= send_at + 5000 and ack["at"] <= close_at <= end <= close_at + 5000, "original_logout_or_close_5s_deadline_failed")
        if outer is not None: need(end <= outer, "equipment_outer_deadline_exceeded_by_close")
        return {"ack": ack, "sentAtMs": send_at, "endedAtMs": end, "closeIndex": closes[0][0]}


def verify_modern_lifecycle(plan, report, events, packets, admissions, checks, *, gear_only=False):
    if plan.get("scenario") == "dispersed-melee" and not gear_only:
        return verify_melee_lifecycle(plan, report, events, packets, admissions, checks, "modern")
    life = Lifecycle("modern", events, packets, checks)
    originals, previous, count = {}, {}, 0
    for event in events:
        if event.get("event") == "equipmentOriginalState":
            actor = event.get("actorOrdinal")
            need(actor in plan["equipmentActorOrdinals"] and actor not in originals, "duplicate_or_unknown_original_gear_state")
            originals[actor] = carried(event.get("state"), "modern"); previous[actor] = originals[actor]
    expected_order = [(a, r, kind) for a in plan["equipmentActorOrdinals"] for r in range(2) for kind in ["removeItem", "equipItem"]]
    steps = [e for e in events if e.get("event") == "equipmentStep"]
    need([(e.get("actorOrdinal"), e.get("round"), e.get("type")) for e in steps] == expected_order, "all_eight_equipment_steps_required")
    weapons = {actor: initial_weapon_uid(originals[actor], "modern") for actor in plan["equipmentActorOrdinals"]}
    for step in steps:
        actor, uid, operation = step["actorOrdinal"], step.get("uniqueId"), step["type"]
        need(uid == weapons[actor], "both_equipment_rounds_must_use_initial_weapon_UID")
        ack = life.ref(step.get("ack", {}), actor, "RemoveItem" if operation == "removeItem" else "EquipItem")
        body = ack["body"]
        need(body.get("uniqueId") == uid and body.get("to") == step.get("to") and body.get("success") is True
             and str(body.get("grid")).lower() == "inventory", "equipment_ack_identity_or_result_invalid")
        began = ack["at"] - step.get("ackElapsedMs", math.inf)
        need(number(began) and 0 <= step["ackElapsedMs"] <= 15000, "original_equipment_ACK_deadline_failed")
        command = [e for e in events if e.get("event") == "sent" and e.get("actorOrdinal") == actor and e.get("connection") == ack["connection"]
                   and e.get("command", {}).get("type") == operation and e["command"].get("uniqueId") == uid and e["command"].get("to") == step["to"] and began <= e.get("atMs", -1) <= ack["at"]]
        need(len(command) == 1, "equipment_actual_send_missing_or_ambiguous")
        if step["round"] == 0 and operation == "removeItem":
            admitted = [entry[2] for entry in admissions.get(actor, []) if entry[1] == ack["connection"]]
            need(len(admitted) == 1, "original_gear_admission_namespace_ambiguous")
            original_event = next(e for e in events if e.get("event") == "equipmentOriginalState" and e.get("actorOrdinal") == actor)
            authenticated, unused_state = life.snapshot(admitted[0].get("snapshot", {}), actor, original_event["state"])
            send_index, original_index = events.index(command[0]), events.index(original_event)
            need(authenticated["index"] < original_index < send_index and authenticated["at"] <= command[0]["atMs"], "original_gear_snapshot_not_before_mutation")
            latest = [p for p in packets.values() if p["actor"] == actor and p["connection"] == ack["connection"]
                      and p["packet"] == "worldSnapshot" and authenticated["index"] <= p["index"] < send_index]
            need(latest and max(latest, key=lambda p: p["index"])["body"].get("carriedFingerprint") == originals[actor]["fingerprint"], "original_gear_roster_changed_before_mutation")
        fresh, after = life.snapshot(step.get("state", {}), actor, step.get("freshState"))
        need(fresh["connection"] != ack["connection"] and ack["at"] <= fresh["at"] <= ack["at"] + 15000
             and fresh["at"] <= report["measurementEpochMs"], "equipment_fresh_connection_or_15s_deadline_failed")
        closed = life.exit(actor, ack["connection"], outer=ack["at"] + 15000)
        need(closed["closeIndex"] < fresh["index"], "fresh_state_precedes_old_connection_close")
        before = previous[actor]
        need(uid in before["items"] and uid in after["items"] and before["scalars"] == after["scalars"]
             and set(before["items"]) == set(after["items"]), "equipment_item_or_full_roster_lost")
        for item_id in before["items"]:
            if item_id == uid:
                need(complete_modern_user_item(before["items"][uid]) == complete_modern_user_item(after["items"][uid]), "equipment_original_UserItem_fields_changed")
                first_payload, second_payload = before["items"][uid]["payload"], after["items"][uid]["payload"]
                changed = {key for key in set(first_payload) | set(second_payload)
                           if first_payload.get(key) != second_payload.get(key) or (key in first_payload) != (key in second_payload)}
                # ItemState::snapshot and EquipmentState::snapshot expose these
                # different grid/display fields; underlying UserItem is equal.
                display = {"addedLuck", "attack", "container", "defence", "equipSlot", "sellValue", "shape", "stateImage",
                           "socketSlots", "sealedExpiryTimeBinaryDatetime", "sealedNextTimeBinaryDatetime"}
                need(changed <= display, "equipment_non_grid_projection_fields_changed")
            else:
                need(before["items"][item_id] == after["items"][item_id], "unrelated_carried_item_changed")
        old, new = before["items"][uid], after["items"][uid]
        target_slot = step["to"] if operation == "removeItem" else SLOTS[step["to"]] if integer(step.get("to")) and step["to"] < len(SLOTS) else None
        need(old["grid"] == ("equipmentItems" if operation == "removeItem" else "inventoryItems")
             and new["grid"] == ("inventoryItems" if operation == "removeItem" else "equipmentItems") and new["slot"] == target_slot, "equipment_grid_or_destination_wrong")
        if operation == "equipItem": need(same_carried(after, originals[actor]), "equipment_roundtrip_does_not_restore_all_fields")
        previous[actor] = after; count += 1
    if gear_only: return {"originals": originals, "count": count}
    cleanup = report.get("cleanup", [])
    need(len(cleanup) == plan["population"] and {x.get("actorOrdinal") for x in cleanup} == set(range(plan["population"])), "all_actor_cleanup_required")
    exits = {}
    epoch = report["measurementEpochMs"]
    for row in cleanup:
        actor = row["actorOrdinal"]; connection = active_connection(admissions, actor, epoch)
        exits[actor] = life.exit(actor, connection)
        need(exits[actor]["sentAtMs"] >= report["observationEndedAtMs"] + plan["settleBeforeLogoutMs"], "final_logout_before_common_settle_phase")
    saved = [e for e in events if e.get("event") == "savedVerificationState"]
    selected = list(dict.fromkeys([0, plan["population"] - 1]))
    need(all(len(admissions.get(actor, [])) == 1 + (4 if actor in plan["equipmentActorOrdinals"] else 0) + (1 if actor in selected else 0)
             for actor in range(plan["population"])), "unexpected_login_retry_or_extra_connection")
    need(len(saved) == len(selected) and {e.get("actorOrdinal") for e in saved} == set(selected), "final_fresh_saved_verification_missing")
    for row in saved:
        actor = row["actorOrdinal"]; before, after = row.get("before"), row.get("after")
        need(type(before) is dict and type(after) is dict and all(before.get(k) == after.get(k) and before.get(k) is not None for k in ["map", "x", "y", "name", "level", "class", "gender"])
             and before["map"] == "DogYoHyun" and before["level"] == 40 and before.get("hp", 0) > 0 and after.get("hp", 0) > 0, "saved_owner_or_transform_not_preserved")
        original = carried(before.get("carried"), "modern")
        if actor in originals: need(same_carried(original, originals[actor]), "final_saved_carried_differs_from_initial_fixture")
        proof = row.get("bootstrap", {})
        fresh, actual = life.snapshot(proof.get("snapshot", {}), actor, after.get("carried"))
        need(same_carried(original, actual) and fresh["connection"] != active_connection(admissions, actor, epoch)
             and exits[actor]["closeIndex"] < fresh["index"], "saved_carried_or_fresh_socket_not_preserved")
        ui = life.ui(actor, fresh["connection"])
        pos = ui["body"].get("location", ui["body"])
        need(pos.get("x") == after["x"] and pos.get("y") == after["y"], "saved_transform_not_bound_to_raw_UserInformation")
        positions = [p for p in packets.values() if p["actor"] == actor and p["connection"] == active_connection(admissions, actor, epoch) and p["packet"] in ["UserLocation", "UserInformation"] and p["at"] <= exits[actor]["ack"]["at"]]
        need(positions, "final_authoritative_transform_missing")
        last = max(positions, key=lambda p: (p["at"], p["sequence"]))["body"]
        pos = last.get("location", last)
        need(pos.get("x") == before["x"] and pos.get("y") == before["y"], "saved_position_differs_from_last_authoritative_transform")
        life.exit(actor, fresh["connection"])
    return {"valid": True, "equipmentStrongChecks": count, "normalFinalExits": len(exits),
            "freshSavedActors": selected, "saveScope": "fresh normal-protocol map/transform/identity and complete carried fields"}


def verify_crystal_lifecycle(plan, report, events, packets, admissions, disk, *, gear_only=False):
    if plan.get("scenario") == "dispersed-melee" and not gear_only:
        return verify_melee_lifecycle(plan, report, events, packets, admissions, Checks(), "crystal", disk=disk)
    life = Lifecycle("crystal", events, packets, Checks())
    original, previous, current, identities = {}, {}, {}, {}
    for actor in range(plan["population"]):
        need(admissions.get(actor), "initial_native_admission_missing")
        ui = life.ui(actor, admissions[actor][0][1])
        need(ui["body"].get("position") == plan["actors"][actor]["start"], "native_initial_transform_differs_from_plan")
        original[actor] = carried(ui["body"]["authoritativeItemState"], "crystal")
        previous[actor] = original[actor]; current[actor] = ui["connection"]
        identities[actor] = (ui["body"]["realCharacterIndex"], ui["body"]["nameHash"])
    rounds = [e["result"] for e in events if e.get("kind") == "equipmentRoundtrip"]
    need([(r.get("actorOrdinal"), r.get("round")) for r in rounds] == [(a, r) for a in plan["equipmentActorOrdinals"] for r in [1, 2]], "complete_native_equipment_roundtrips_missing")
    weapons = {actor: initial_weapon_uid(original[actor], "crystal") for actor in plan["equipmentActorOrdinals"]}
    count = 0
    for row in rounds:
        actor = row["actorOrdinal"]
        for operation, key in [("RemoveItem", "removedState"), ("EquipItem", "equippedState")]:
            ack, fresh = row.get("remove" if operation == "RemoveItem" else "equip", {}), row.get(key, {})
            old_connection, new_connection = fresh.get("priorConnection"), fresh.get("connection")
            need(old_connection == current[actor] and integer(new_connection) and new_connection != old_connection, "native_gear_connection_epoch_invalid")
            raw = packets.get((actor, old_connection, ack.get("receiveSequence")))
            sent = [e for e in events if e.get("kind") == "packet" and e.get("direction") == "sent" and e.get("actorOrdinal") == actor
                    and e.get("connection") == old_connection and e.get("sequence") == ack.get("sentSequence") and e.get("packet") == operation]
            need(raw is not None and raw["packet"] == operation and raw["at"] == ack.get("receivedAtMs") and len(sent) == 1
                 and sent[0].get("monotonicMs") == ack.get("sentAtMs"), "native_equipment_send_or_ACK_missing")
            deadline = ack.get("deadlineAtMs"); when = ack.get("sentAtMs")
            need(number(deadline) and number(when) and when <= raw["at"] <= deadline <= when + 15000, "native_equipment_ACK_original_deadline_failed")
            uid, destination = raw["body"].get("uniqueID"), raw["body"].get("to")
            need(uid == weapons[actor], "both_equipment_rounds_must_use_initial_weapon_UID")
            need(raw["body"].get("success") is True and raw["body"].get("grid") == 1
                 and integer(uid) and uid > 0 and integer(destination)
                 and row.get("itemIdentityHash") == digest(str(uid).encode()), "native_equipment_ACK_identity_invalid")
            ui = life.ui(actor, new_connection)
            need(ui["sequence"] == fresh.get("userInformationReceiveSequence") and ui["at"] == fresh.get("userInformationReceivedAtMs")
                 and raw["at"] <= ui["at"] <= raw["at"] + 15000 and ui["at"] <= report["measurementStartedAtMs"], "native_equipment_fresh_UI_or_deadline_failed")
            closed = life.exit(actor, old_connection, outer=raw["at"] + 15000)
            need(closed["closeIndex"] < ui["index"], "native_fresh_UI_precedes_old_close")
            need((ui["body"]["realCharacterIndex"], ui["body"]["nameHash"]) == identities[actor], "native_gear_reentry_identity_changed")
            after = carried(ui["body"]["authoritativeItemState"], "crystal"); before = previous[actor]
            need(set(before["items"]) == set(after["items"]) and before["scalars"] == after["scalars"] and uid in before["items"], "native_gear_full_roster_or_scalars_changed")
            for item_id in before["items"]:
                need(before["items"][item_id]["payload"] == after["items"][item_id]["payload"], "native_complete_item_Save_fields_changed")
                if item_id != uid: need(before["items"][item_id] == after["items"][item_id], "native_unrelated_item_moved")
            first, second = before["items"][uid], after["items"][uid]
            need(first["grid"] == ("equipment" if operation == "RemoveItem" else "inventory")
                 and second["grid"] == ("inventory" if operation == "RemoveItem" else "equipment") and second["slot"] == destination, "native_equipment_destination_wrong")
            if operation == "EquipItem": need(same_carried(after, original[actor]), "native_roundtrip_all_fields_not_restored")
            previous[actor], current[actor] = after, new_connection; count += 1
    if gear_only: return {"originals": original, "identities": identities, "count": count}
    epoch = report["measurementStartedAtMs"]
    final_positions, exits = {}, {}
    for actor in range(plan["population"]):
        connection = active_connection(admissions, actor, epoch)
        exits[actor] = life.exit(actor, connection)
        need(exits[actor]["sentAtMs"] >= report["measurementEndedAtMs"] + plan["settleBeforeLogoutMs"], "native_logout_before_common_settle_phase")
        positions = [p for p in packets.values() if p["actor"] == actor and p["connection"] == connection and p["packet"] in ["UserInformation", "UserLocation"] and p["at"] <= exits[actor]["ack"]["at"]]
        need(positions, "last_native_authoritative_transform_missing")
        final_positions[actor] = max(positions, key=lambda p: (p["at"], p["sequence"]))["body"].get("position")
    selected = list(dict.fromkeys([0, plan["population"] - 1]))
    need(all(len(admissions.get(actor, [])) == 1 + (4 if actor in plan["equipmentActorOrdinals"] else 0) + (1 if actor in selected else 0)
             for actor in range(plan["population"])), "unexpected_native_login_retry_or_extra_connection")
    reentries = report.get("reentries", [])
    need(len(reentries) == len(selected) and {r.get("actorOrdinal") for r in reentries} == set(selected), "final_native_reentry_selection_missing")
    for row in reentries:
        actor, connection = row["actorOrdinal"], row.get("connection")
        ui = life.ui(actor, connection)
        need(connection != active_connection(admissions, actor, epoch) and exits[actor]["closeIndex"] < ui["index"]
             and ui["body"].get("position") == final_positions[actor]
             and (ui["body"]["realCharacterIndex"], ui["body"]["nameHash"]) == identities[actor]
             and same_carried(carried(ui["body"]["authoritativeItemState"], "crystal"), original[actor]), "native_fresh_final_state_not_preserved")
        life.exit(actor, connection)
    need(type(disk) is dict and disk.get("schema") == "mir2.comparable.crystal-disk-readback.v1"
         and disk.get("originalLoaderUsed") is True and HEX.fullmatch(disk.get("accountFileSha256", "")) is not None, "native_original_LoadAccounts_disk_proof_missing")
    samples = disk.get("samples", [])
    need(disk.get("sampleCount") == plan["population"] == len(samples)
         and {s.get("actorOrdinal") for s in samples} == set(range(plan["population"])), "native_disk_readback_all_actors_required")
    for sample in samples:
        actor = sample["actorOrdinal"]
        need(sample.get("identityMatchesFixture") is True and sample.get("mapIndex") == 440
             and sample.get("position") == final_positions[actor] and sample.get("hp", 0) > 0 and sample.get("level") == 40
             and same_carried(carried(sample.get("itemState"), "crystal"), original[actor]), "native_disk_state_differs_from_authoritative_final_state")
    return {"valid": True, "equipmentStrongChecks": count, "normalFinalExits": len(exits), "freshSavedActors": selected,
            "originalLoaderDiskActors": len(samples), "saveScope": "map/transform/identity, complete original UserItem.Save digests and carried scalars"}


def modern_disk_expected_states(plan, report, events, *, packets=None, admissions=None, all_actor_dense=False):
    """Derive each supported actor's *last* real normal logout, including fresh2.

    Fresh verification clients have no combat tracker and emit no second
    logoutOwnedState. Their initial full owned checkpoint and received mutation
    packets still permit independent reconstruction through the actual last ACK.
    Nothing is inserted into, or represented as a new event in, the input trace.
    """
    population=plan["population"]
    if plan.get("scenario")!="dispersed-melee" and not all_actor_dense:
        selected=[e for e in events if e.get("event")=="savedVerificationState"]
        states={e["actorOrdinal"]:e["after"] for e in selected}
        need(len(selected)==len(states)==population and set(states)==set(range(population)),"all_actor_fresh_full_states_required_for_disk_readback")
        return states,[]
    need(plan.get("scenario") in ["dispersed-melee","dense-movement-chat"],"all_actor_disk_scenario_unsupported")
    need(type(report)is dict and integer(population) and 1<=population<=100,"all_actor_disk_report_or_population_missing")
    epoch,end=report.get("measurementEpochMs"),report.get("observationEndedAtMs")
    need(number(epoch) and number(end) and end>=epoch,"all_actor_disk_measurement_clock_missing")
    if packets is None or admissions is None:
        checks=Checks();packets,admissions,unused=index_evidence(events,"modern",checks)
        need(not checks.failures,"all_actor_disk_raw_identity_invalid")
    life=Lifecycle("modern",events,packets,Checks());states,anchors={},[]
    def ref(raw):
        return {"observerOrdinal":raw["actor"],"connection":raw["connection"],"receiveSeq":raw["sequence"],"atMs":raw["at"],"packet":raw["packet"]}
    for actor in range(population):
        need(admissions.get(actor),"all_actor_disk_admission_missing")
        active=active_connection(admissions,actor,epoch);latest=admissions[actor][-1][1]
        need(active is not None,"all_actor_disk_not_admitted_before_measurement")
        closed=life.exit(actor,latest);ack=closed["ack"]
        need(closed["sentAtMs"]>=end+plan["settleBeforeLogoutMs"],"all_actor_disk_last_logout_before_settle")
        later=[p for p in packets.values() if p["actor"]==actor and p["packet"]=="LogOutSuccess" and p["index"]>ack["index"]]
        need(not later,"all_actor_disk_last_ACK_connection_ambiguous")
        ui=life.ui(actor,latest);original_ui=life.ui(actor,active)
        identity={k:ui["body"].get(k) for k in ["name","level","class","gender"]}
        need(identity["level"]==40 and all(value is not None and value==original_ui["body"].get(k) for k,value in identity.items()),"all_actor_disk_last_UI_identity_changed")
        candidates=[]
        for event in events:
            if event.get("event") not in ["ownedSnapshotEvidence","potionSourceState","potionStateEvidence"] or event.get("actorOrdinal")!=actor or event.get("connection")!=latest:continue
            raw,state=life.snapshot(event.get("sourceReceipt",{}),actor,event.get("state"))
            if raw["sequence"]<ack["sequence"]:candidates.append((raw,state))
        need(candidates,"all_actor_disk_full_owned_checkpoint_missing")
        base,state=max(candidates,key=lambda pair:pair[0]["sequence"]);state=copy.deepcopy(state)
        owner=base["body"].get("owner",{});hp,mp=base["body"].get("playerHp"),base["body"].get("playerMp")
        position,direction=combat_position(owner),owner.get("direction")
        need(all(owner.get(k)==value for k,value in identity.items()),"all_actor_disk_checkpoint_identity_mismatch")
        scope=sorted([p for p in packets.values() if p["actor"]==actor and p["connection"]==latest and base["sequence"]<p["sequence"]<ack["sequence"]],key=lambda p:p["sequence"])
        applied=[]
        for raw in scope:
            body,kind=raw["body"],raw["packet"]
            if kind in ["DuraChanged","DeleteItem"]:modern_item_patch(state,raw)
            elif kind=="HealthChanged":hp,mp=body.get("hp"),body.get("mp")
            elif kind=="UserLocation":position=combat_position(body);direction=body.get("direction",direction)
            elif kind=="worldSnapshot":
                need(body.get("carriedFingerprint")==carried(modern_carried_projection(state),"modern")["fingerprint"],"all_actor_disk_unexplained_owned_roster_change")
                owner=body.get("owner",{})
                need(body.get("map")=="DogYoHyun" and body.get("playerObjectId")==ui["body"].get("objectId") and all(owner.get(k)==value for k,value in identity.items()),"all_actor_disk_snapshot_identity_or_map_changed")
                hp,mp=body.get("playerHp"),body.get("playerMp");position,direction=combat_position(owner),owner.get("direction")
            need(kind not in ["RefreshItem","UseItem"],"all_actor_disk_mutation_without_complete_checkpoint")
            if all_actor_dense:
                need(kind not in ["NewItem","AddItem","GainGold","GainedGold","LoseGold","GoldChanged","GainedCredit","LoseCredit","CreditChanged","InventorySizeChanged","BagSizeChanged","Death"],"dense_all_actor_unsupported_state_operation_without_checkpoint")
            if kind in ["DuraChanged","DeleteItem","HealthChanged","UserLocation","worldSnapshot"]:applied.append(ref(raw))
        need(type(hp)is int and hp>0 and integer(mp) and combat_direction(direction)is not None and type(position)is dict
             and all(type(position.get(k))is int for k in ["x","y"]),"all_actor_disk_final_vitals_or_transform_missing")
        full=modern_carried_projection(state);fingerprint=carried(full,"modern")["fingerprint"]
        final={**identity,"map":"DogYoHyun","x":position["x"],"y":position["y"],"direction":direction,"hp":hp,"mp":mp,"items":fingerprint,"carried":full}
        producer=[e for e in events if e.get("event")=="logoutOwnedState" and e.get("actorOrdinal")==actor and e.get("connection")==latest]
        if producer:
            recorded,recorded_items=modern_logout_state(actor,latest,ack,events,packets,life)
            need(all(recorded.get(k)==value for k,value in final.items() if k!="carried") and same_carried(recorded_items,state),"all_actor_disk_producer_freeze_disagrees_with_reconstruction")
        states[actor]=final
        anchors.append({"schema":"mir2.comparable.modern-last-logout-state-anchor.v1","actorOrdinal":actor,"connection":latest,
                        "initialUserInformation":ref(ui),"baseSnapshot":ref(base),"logOutSuccess":ref(ack),
                        "appliedPacketCount":len(applied),"appliedPacketRefsSha256":digest(encoded(applied,sorted_keys=True)),
                        "producerFrozenStateCrossChecked":bool(producer),"basis":"complete-owned-checkpoint-and-all-raw-state-operations-through-last-normal-ACK"})
    return states,anchors


def verify_modern_disk(plan, events, disk, metadata, *, report=None, packets=None, admissions=None, dense_state_events=None):
    if plan.get("scenario")=="dispersed-melee":
        return verify_modern_disk_v2(plan,report,events,disk,metadata,packets=packets,admissions=admissions)
    if type(disk)is dict and disk.get("schema")=="mir2.comparable.modern-file-disk-readback.v3":
        return verify_modern_disk_v3(plan,report,dense_state_events,disk,metadata)
    need(type(disk) is dict and disk.get("schema") == "mir2.comparable.modern-file-disk-readback.v1", "modern_File_disk_proof_missing")
    need(disk.get("readAfterNormalStop") is True and disk.get("accountStorePathBoundToRuntime") is True
         and disk.get("privateValuesExported") is False and HEX.fullmatch(disk.get("accountFileSha256", "")) is not None
         and integer(disk.get("accountFileBytes")) and disk["accountFileBytes"] > 0, "modern_File_readback_provenance_insufficient")
    for label, key in [("planSha256", "plan"), ("summarySha256", "summary"), ("eventsSha256", "events"), ("controllerSha256", "controller")]:
        need(disk.get(label) == metadata[key]["sha256"], "modern_File_readback_not_bound_to_case")
    pair=disk.get("readerSourceSha256"),disk.get("validatorSourceSha256")
    need(pair in {(digest(Path(__file__).with_name("read_modern_disk.py").read_bytes()),digest(Path(__file__).read_bytes())),
                  ("d3eec8d840501c66abf7f37128554d1016c9aa3fd26cd65aeaa04f2e9b9d5a3c","762218c45e0c88beb319ce3765960cd25b69f177c8cb9ac58667d6eb87a0c73f")}, "modern_File_reader_source_not_current_reviewed_code")
    states = {e.get("actorOrdinal"): e.get("after") for e in events if e.get("event") == "savedVerificationState"}
    samples = disk.get("samples", [])
    need(set(states) == set(range(plan["population"])) and disk.get("sampleCount") == plan["population"] == len(samples)
         and {s.get("actorOrdinal") for s in samples} == set(states), "all_modern_File_actors_not_read_back")
    for sample in samples:
        expected = states[sample["actorOrdinal"]]; full = carried(expected.get("carried"), "modern")
        need(sample.get("identityMatches") is True and sample.get("completeItemsAndGridMatch") is True
             and sample.get("finalTransformAndVitalsMatch") is True and sample.get("carriedFingerprint") == full["fingerprint"]
             and sample.get("map") == expected.get("map") and sample.get("position") == {"x": expected.get("x"), "y": expected.get("y")}
             and sample.get("level") == expected.get("level") and sample.get("gold") == expected["carried"].get("gold")
             and sample.get("inventoryCapacity") == expected["carried"].get("inventoryCapacity")
             and sample.get("maxBagSlots") == expected["carried"].get("maxBagSlots"), "modern_File_receipt_does_not_match_fresh_owner_state")
    return {"valid": True, "actors": len(samples), "privateStoreOpenedByVerifier": False,
            "scope": "separate read-only direct File authority reader; full simple fixture UserItem carrier"}


def verify_modern_disk_v2(plan, report, events, disk, metadata, *, packets=None, admissions=None):
    need(type(disk)is dict and disk.get("schema")=="mir2.comparable.modern-file-disk-readback.v2" and disk.get("passed") is True,"modern_all_actor_File_v2_proof_missing")
    need(disk.get("readAfterNormalStop") is True and disk.get("accountStorePathBoundToRuntime") is True
         and disk.get("privateValuesExported") is False and HEX.fullmatch(disk.get("accountFileSha256","")) is not None
         and integer(disk.get("accountFileBytes")) and disk["accountFileBytes"]>0,"modern_File_readback_provenance_insufficient")
    for label,key in [("planSha256","plan"),("summarySha256","summary"),("eventsSha256","events"),("controllerSha256","controller")]:
        need(disk.get(label)==metadata[key]["sha256"],"modern_File_readback_not_bound_to_case")
    need(disk.get("readerSourceSha256")==digest(Path(__file__).with_name("read_modern_disk.py").read_bytes())
         and disk.get("validatorSourceSha256")==digest(Path(__file__).read_bytes()),"modern_File_reader_source_not_current_reviewed_code")
    states,anchors=modern_disk_expected_states(plan,report,events,packets=packets,admissions=admissions)
    need(disk.get("expectedStateBasis")=="all-actor-last-normal-LogOutSuccess-raw-reconstruction"
         and encoded(disk.get("allActorFinalStateAnchors"),sorted_keys=True)==encoded(anchors,sorted_keys=True),"modern_File_all_actor_last_ACK_anchors_unbound")
    samples=disk.get("samples",[])
    need(type(samples)is list and disk.get("sampleCount")==plan["population"]==len(samples)
         and all(integer(s.get("actorOrdinal")) for s in samples) and {s.get("actorOrdinal") for s in samples}==set(states),"all_modern_File_actors_not_read_back")
    for sample in samples:
        expected=states[sample["actorOrdinal"]];full=carried(expected["carried"],"modern");semantics={}
        for uid,item in full["items"].items():
            semantics[uid]={"grid":item["grid"],"slot":item["slot"],"container":None if item["grid"]=="equipmentItems" else item["payload"].get("container"),**complete_modern_user_item(item)}
        need(sample.get("identityMatches") is True and sample.get("completeItemsAndGridMatch") is True
             and sample.get("finalTransformAndVitalsMatch") is True and sample.get("carriedFingerprint")==full["fingerprint"]
             and sample.get("persistedItemSemanticsSha256")==digest(encoded(semantics,sorted_keys=True)),"modern_File_complete_UserItem_or_grid_semantics_mismatch")
        need(type(sample.get("hp"))is int and type(sample.get("mp"))is int and type(sample.get("direction"))is type(expected["direction"])
             and type(sample.get("position"))is dict and all(type(sample["position"].get(k))is int for k in ["x","y"])
             and sample.get("map")==expected["map"] and sample.get("position")=={"x":expected["x"],"y":expected["y"]}
             and all(sample.get(k)==expected[k] for k in ["direction","hp","mp","level"])
             and sample.get("gold")==expected["carried"]["gold"] and sample.get("inventoryCapacity")==expected["carried"]["inventoryCapacity"]
             and sample.get("maxBagSlots")==expected["carried"]["maxBagSlots"]
             and sample.get("itemCounts")=={grid:sum(item["grid"]==grid for item in full["items"].values()) for grid in ["inventoryItems","equipmentItems","beltItems"]},"modern_File_last_ACK_vitals_transform_or_counts_mismatch")
    return {"valid":True,"actors":len(samples),"privateStoreOpenedByVerifier":False,"allActorLastAckAnchorsValidated":True,
            "scope":"all actors; exact last normal ACK HP/MP/transform and complete persisted UserItem grid/count/durability semantics"}


def verify_modern_disk_v3(plan, report, events, disk, metadata):
    need(type(disk)is dict and disk.get("schema")=="mir2.comparable.modern-file-disk-readback.v3" and disk.get("passed") is True,"modern_all_actor_dense_File_v3_proof_missing")
    need(disk.get("readAfterNormalStop") is True and disk.get("accountStorePathBoundToRuntime") is True
         and disk.get("privateValuesExported") is False and HEX.fullmatch(disk.get("accountFileSha256","")) is not None
         and integer(disk.get("accountFileBytes")) and disk["accountFileBytes"]>0,"modern_File_readback_provenance_insufficient")
    for label,key in [("planSha256","plan"),("summarySha256","summary"),("eventsSha256","events"),("controllerSha256","controller")]:
        need(disk.get(label)==metadata[key]["sha256"],"modern_File_readback_not_bound_to_case")
    need(disk.get("readerSourceSha256")==digest(Path(__file__).with_name("read_modern_disk.py").read_bytes())
         and disk.get("validatorSourceSha256")==digest(Path(__file__).read_bytes()),"modern_File_reader_source_not_current_reviewed_code")
    validate_dense_state_binding(events,metadata["events"])
    need(plan.get("scenario")=="dense-movement-chat","dense_all_actor_disk_scenario_required")
    states,anchors=modern_disk_expected_states(plan,report,events,all_actor_dense=True)
    need(disk.get("expectedStateBasis")=="all-actor-last-normal-LogOutSuccess-raw-reconstruction"
         and encoded(disk.get("allActorFinalStateAnchors"),sorted_keys=True)==encoded(anchors,sorted_keys=True),"modern_File_all_actor_last_ACK_anchors_unbound")
    need(disk.get("denseStateSourceSha256")==metadata["events"]["sha256"] and disk.get("denseStateSourceBytes")==metadata["events"]["bytes"]
         and disk.get("denseStateSourceRows")==metadata["events"]["rows"],"dense_all_actor_state_file_binding_missing")
    samples=disk.get("samples",[])
    need(type(samples)is list and disk.get("sampleCount")==plan["population"]==len(samples)
         and all(integer(s.get("actorOrdinal")) for s in samples) and {s.get("actorOrdinal") for s in samples}==set(states),"all_modern_File_actors_not_read_back")
    for sample in samples:
        expected=states[sample["actorOrdinal"]];full=carried(expected["carried"],"modern");semantics={}
        for uid,item in full["items"].items():
            semantics[uid]={"grid":item["grid"],"slot":item["slot"],"container":None if item["grid"]=="equipmentItems" else item["payload"].get("container"),**complete_modern_user_item(item)}
        need(sample.get("identityMatches") is True and sample.get("completeItemsAndGridMatch") is True
             and sample.get("finalTransformAndVitalsMatch") is True and sample.get("carriedFingerprint")==full["fingerprint"]
             and sample.get("persistedItemSemanticsSha256")==digest(encoded(semantics,sorted_keys=True)),"modern_File_complete_UserItem_or_grid_semantics_mismatch")
        need(type(sample.get("hp"))is int and type(sample.get("mp"))is int and type(sample.get("direction"))is type(expected["direction"])
             and type(sample.get("position"))is dict and all(type(sample["position"].get(k))is int for k in ["x","y"])
             and sample.get("map")==expected["map"] and sample.get("position")=={"x":expected["x"],"y":expected["y"]}
             and all(sample.get(k)==expected[k] for k in ["direction","hp","mp","level"])
             and sample.get("gold")==expected["carried"]["gold"] and sample.get("inventoryCapacity")==expected["carried"]["inventoryCapacity"]
             and sample.get("maxBagSlots")==expected["carried"]["maxBagSlots"]
             and sample.get("itemCounts")=={grid:sum(item["grid"]==grid for item in full["items"].values()) for grid in ["inventoryItems","equipmentItems","beltItems"]},"modern_File_last_ACK_vitals_transform_or_counts_mismatch")
    return {"valid":True,"actors":len(samples),"privateStoreOpenedByVerifier":False,"allActorLastAckAnchorsValidated":True,
            "scope":"all actors; exact last normal ACK HP/MP/transform and complete persisted UserItem grid/count/durability semantics"}


def load_resource_validator():
    path = COMMON / "windows_service_job.py"
    spec = importlib.util.spec_from_file_location("independent_resource_gate", path)
    module = importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
    return module.assess_resource_sample


def observed_driver_span(events, engine):
    if isinstance(events, StreamedEvents): return events.observed_span_ms
    values = []
    for event in events:
        values.extend(actual_event_times(event, engine))
    return max(values) - min(values) if values else None


def verify_resources(plan, report, controller, resources, injector, metadata, checks, *, event_span_ms=None):
    start = len(checks.failures)
    checks.require(controller.get("schema") == "mir2.comparable.stage-controller.v1"
                   and controller.get("qualificationLayer") == "game-core-local", "controller_schema_or_scope")
    checks.require(controller.get("requiredRoles") == ["gameServer"] and controller.get("declaredDependencies") == [], "core_controller_dependency_declaration")
    checks.require(controller.get("resourceNominalIntervalMs") == 1000 and controller.get("resourceSampleMaxGapMs") == 2500, "resource_sampling_contract_drift")
    for label in ["actualExitCode", "driverActualExitCode", "serverActualExitCode"]:
        checks.require(type(controller.get(label)) is int and controller[label] == 0, "controller_or_producer_actual_exit_failed")
    checks.require(controller.get("normalStopProven") is True and controller.get("abnormalCleanup") is False, "normal_shutdown_not_proven")
    for label, role in [("serverJobClose", "gameServer"), ("injectorJobClose", "injector")]:
        closed = controller.get(label, {})
        checks.require(closed.get("closed") is True and closed.get("allMembersExited") is True
                       and closed.get("killAppliedByClose") is False and closed.get("failedLaunches") == []
                       and closed.get("rootExitCodes") == {role: 0}, "job_exit_or_close_not_normal")
    checks.require(integer(report.get("processId")) and report.get("processId") > 0
                   and report.get("processId") == controller.get("injectorJobRootPid"), "driver_process_identity_unbound")
    checks.require(controller.get("planSha256") == metadata["plan"]["sha256"] == report.get("planSha256"), "plan_file_hash_unbound")
    checks.require(report.get("eventsSha256") == metadata["events"]["sha256"], "adapter_events_file_hash_unbound")
    evidence = controller.get("driverEvidence", {})
    for name in ["summary", "events"]:
        pin = evidence.get(name, {})
        relative = pin.get("relativePath")
        checks.require(type(relative) is str and relative and not Path(relative).is_absolute()
                       and ".." not in relative.replace("\\", "/").split("/")
                       and pin.get("sha256") == metadata[name]["sha256"] and pin.get("bytes") == metadata[name]["bytes"], "controller_driver_artifact_binding_missing")
    checks.require(controller.get("resourcesSha256") == metadata["resources"]["sha256"]
                   and controller.get("resourcesRowCount") == len(resources), "server_resource_file_binding_missing")
    checks.require(controller.get("injectorResourcesSha256") == metadata["injector"]["sha256"], "injector_resource_file_binding_missing")
    times = [controller.get(k) for k in ["readyAtMs", "driverStartedAtMs", "driverExitedAtMs", "normalShutdownStartedAtMs"]]
    if not checks.require(all(number(x) and x >= 0 for x in times) and times == sorted(times) and times[1] < times[2], "controller_lifecycle_clock_order_invalid"):
        return {"valid": False, "serverSamples": len(resources), "injectorSamples": len(injector)}
    ready, began, ended, shutdown = times
    checks.require(ended - began >= plan["durationMs"], "driver_lifetime_shorter_than_complete_workload_window")
    anchor = report.get("measurementEpochMs", report.get("measurementStartedAtMs"))
    observed_end = report.get("observationEndedAtMs", report.get("measurementEndedAtMs"))
    checks.require(number(anchor) and number(observed_end) and 0 <= observed_end - anchor <= ended - began,
                   "driver_observation_duration_exceeds_owned_process_lifetime")
    if event_span_ms is not None:
        checks.require(number(event_span_ms) and 0 <= event_span_ms <= ended - began,
                       "raw_driver_timeline_exceeds_owned_process_lifetime")
    assess = load_resource_validator()
    maxima = {}
    for label, rows, role, rootpid, cpus, memory, mask in [
            ("server", resources, "gameServer", controller.get("gameJobRootPid"), 4, 8 * 2**30, "0xf"),
            ("injector", injector, "injector", controller.get("injectorJobRootPid"), 8, 4 * 2**30, "0xff0")]:
        stamps, peaks, roots = [], [], set()
        for row in rows:
            at = row.get("resourceSampleAtMs")
            if checks.require(number(at), "resource_monotonic_sample_time_missing"): stamps.append(at)
            checks.require(row.get("schema") == "mir2.shared-service-job-sample.v1", "resource_sample_schema")
            wanted = row.get("requestedLimits", {})
            checks.require(wanted.get("logicalCpus") == cpus and wanted.get("jobCommitLimitBytes") == memory
                           and wanted.get("affinityMaskHex") == mask, "resource_budget_mismatch")
            host = wanted.get("hostLogicalCpus")
            checks.require(integer(host) and cpus <= host <= 64 and wanted.get("cpuRatePer10000") == round(10000 * cpus / host), "resource_cpu_rate_not_host_scaled")
            checks.require(row.get("roles", {}).get(role) == rootpid, "resource_root_pid_not_controller_owned")
            for root in row.get("roots", []):
                if root.get("role") == role: roots.add((root.get("pid"), root.get("creationTime100ns")))
            try:
                gate = assess(row, required_roles=[role], declared_dependencies=[])
                checks.require(gate["qualified"] is True, "resource_sample_independently_invalid")
            except (ValueError, TypeError, KeyError, OverflowError):
                checks.require(False, "resource_sample_malformed_or_unknown")
            if number(row.get("jobPeakCommitBytes")): peaks.append(row["jobPeakCommitBytes"])
        gaps = [b - a for a, b in zip(stamps, stamps[1:])]
        checks.require(len(stamps) == len(rows) and len(rows) >= 2 and all(0 < gap <= 2500 for gap in gaps), "resource_sampling_gap_or_incomplete")
        checks.require(len(roots) == 1, "resource_root_lifecycle_changed")
        if stamps:
            if label == "server":
                checks.require(ready <= stamps[0] <= began and ended <= stamps[-1] <= shutdown, "server_samples_do_not_cover_driver_lifetime")
            else:
                checks.require(began <= stamps[0] <= began + 2500 and ended - 2500 <= stamps[-1] <= ended, "injector_sampling_edges_incomplete")
        maxima[label] = {"samples": len(rows), "maximumGapMs": max(gaps) if gaps else None,
                         "peakReportedJobCommitBytes": max(peaks) if peaks else None,
                         "affinityMaskHex": mask, "commitLimitBytes": memory, "logicalCpus": cpus}
    checks.require(controller.get("resourceEvidenceComplete") is True, "controller_resource_evidence_incomplete")
    return {"valid": len(checks.failures) == start, "budgets": maxima, "driverLifetimeMs": ended - began,
            "driverAndControllerMonotonicClocksCombined": False, "osExcluded": True,
            "scope": "game-core-local; server and injector separate"}


def verify_bundle(plan, report, events, controller, resources, injector, metadata, *, disk=None, combat_contract=None, source_artifacts=None, dense_state_events=None):
    checks = Checks()
    result = {"schema": "mir2.comparable.independent-verification.v7", "passed": False,
              "qualificationLayer": "game-core-local", "capacityClaimAllowed": False,
              "productionStackQualified": False, "capacityQualified": False, "windows": [],
              "lifecycle": {"valid": False}, "resources": {"valid": False}, "inputs": metadata,
              "trustBoundary": "public adapter packet projections and owned-controller OS receipts; no private data is read"}
    try:
        validate_plan(plan)
        engine = {"mir2.comparable.modern-report.v1": "modern", "mir2.comparable.crystal-report.v1": "crystal"}.get(report.get("schema"))
        need(engine is not None, "unsupported_adapter_report_schema")
        synthetic = (report.get("synthetic") is True or report.get("notARealRun") is True or report.get("liveEvidence") is False or getattr(events, "any_synthetic", False)
                     or any(e.get("synthetic") is True or e.get("liveEvidence") is False for e in events))
        result.update(engine=engine, planId=plan["planId"], population=plan["population"], durationMs=plan["durationMs"], synthetic=synthetic)
        checks.require(controller.get("engine") == engine, "controller_adapter_engine_mismatch")
        checks.require(report.get("planId") == plan["planId"], "adapter_plan_id_mismatch")
        if plan.get("scenario") == "dispersed-melee":
            need(metadata.get("combatContract", {}).get("sha256") == COMBAT_SHA and combat_contract == plan.get("combatContract")
                 and report.get("combatContractSha256") == COMBAT_SHA, "external_combat_contract_bytes_not_bound")
            result["adapterSourceEvidence"] = verify_combat_sources(report, controller, metadata, source_artifacts, engine)
        elif source_artifacts is not None:
            result["adapterSourceEvidence"] = verify_combat_sources(report, controller, metadata, source_artifacts, engine, melee=False)
        if engine == "crystal":
            checks.require(report.get("evidenceComplete") is True and report.get("evidenceBudgetExceeded") is False
                           and report.get("evidenceWriteFailed") is False and report.get("eventRows") == getattr(events, "total_rows", len(events)), "native_evidence_incomplete")
        if isinstance(events, StreamedEvents): result["eventReadAudit"] = events.audit
        windows, actions, packets, admissions = verify_actions(plan, report, events, engine, checks)
        result["windows"] = windows
        result["totals"] = {"offered": len(actions), "completed": sum(a["completed"] for a in actions),
                            "timeout": sum(a["timeout"] for a in actions), "notSent": sum(a["notSent"] for a in actions)}
        try:
            lifecycle = (verify_modern_lifecycle(plan, report, events, packets, admissions, checks) if engine == "modern"
                         else verify_crystal_lifecycle(plan, report, events, packets, admissions, disk))
            if engine == "modern": lifecycle["fileDiskReadback"] = verify_modern_disk(plan, events, disk, metadata, report=report, packets=packets, admissions=admissions, dense_state_events=dense_state_events)
            result["lifecycle"] = lifecycle
        except (EvidenceError, KeyError, TypeError, IndexError, AttributeError) as error:
            checks.require(False, str(error) if isinstance(error, EvidenceError) else "lifecycle_schema_insufficient")
        result["resources"] = verify_resources(plan, report, controller, resources, injector, metadata, checks,
                                               event_span_ms=observed_driver_span(events, engine))
        result["workloadPassed"] = len(windows) == plan["population"] * (plan["durationMs"] // 60000) and all(w["passed"] for w in windows)
        result["consistentWithContract"] = result["workloadPassed"] and result["lifecycle"]["valid"] and result["resources"]["valid"] and not checks.failures
        result["passed"] = result["consistentWithContract"] and not synthetic
        result["syntheticEvidenceCannotQualifyActualRun"] = True
    except (EvidenceError, KeyError, TypeError, ValueError, OverflowError, IndexError, AttributeError) as error:
        checks.require(False, str(error) if isinstance(error, EvidenceError) else "schema_insufficient_or_malformed")
    result["hardFailures"] = checks.failures
    result["evidenceSufficient"] = not checks.failures
    return result


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["plan", "summary", "events", "controller", "resources", "injector"]:
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--disk-readback", type=Path, help="Required original LoadAccounts or modern File public projection")
    parser.add_argument("--combat-contract", type=Path, help="Required pinned external contract for melee")
    parser.add_argument("--adapter-sources", type=Path, help="Melee: modern source manifest or native A8 source-lock")
    parser.add_argument("--native-build", type=Path, help="Melee native original build receipt")
    parser.add_argument("--server-ready", type=Path, help="Melee native actual server-ready projection")
    parser.add_argument("--runtime-receipt", type=Path, help="Melee modern actual owned runtime receipt")
    parser.add_argument("--build-manifest", type=Path, help="Exact optional base-plus-overlay build manifest")
    parser.add_argument("--output", type=Path, help="Exclusive new result; Windows output must be on C")
    args = parser.parse_args(argv)
    try:
        objects, metadata = {}, {}
        for name in ["plan", "summary", "controller", "resources", "injector"]:
            objects[name], metadata[name] = read_file(getattr(args, name), jsonl=name in ["events", "resources", "injector"])
        engine = {"mir2.comparable.modern-report.v1": "modern", "mir2.comparable.crystal-report.v1": "crystal"}.get(objects["summary"].get("schema"))
        objects["events"], metadata["events"] = read_events(args.events, engine, objects["plan"].get("population"))
        disk = None
        if args.disk_readback:
            disk, metadata["diskReadback"] = read_file(args.disk_readback)
        dense_state_events = None
        if type(disk)is dict and disk.get("schema")=="mir2.comparable.modern-file-disk-readback.v3":
            dense_state_events,metadata["denseStateEvents"]=read_dense_disk_state_events(args.events)
            validate_dense_state_binding(dense_state_events,metadata["events"])
        combat_contract = None
        if args.combat_contract:
            combat_contract, metadata["combatContract"] = read_file(args.combat_contract)
        source_artifacts = None
        if args.adapter_sources:
            source_artifacts = read_combat_sources(args, engine, metadata)
        result = verify_bundle(objects["plan"], objects["summary"], objects["events"], objects["controller"],
                               objects["resources"], objects["injector"], metadata, disk=disk, combat_contract=combat_contract, source_artifacts=source_artifacts, dense_state_events=dense_state_events)
    except (EvidenceError, OSError, MemoryError, RecursionError, KeyError, TypeError, ValueError, IndexError, AttributeError) as error:
        code=(str(error) if isinstance(error,EvidenceError) else "verifier_memory_or_read_bound_not_server_failure"
              if isinstance(error,(OSError,MemoryError,RecursionError)) else "adapter_or_input_schema_insufficient")
        result = {"schema": "mir2.comparable.independent-verification.v7", "passed": False,
                  "capacityClaimAllowed": False, "capacityQualified": False, "evidenceSufficient": False,
                  "hardFailures": [{"code":code}]}
    result["verifierSourceSha256"] = digest(Path(__file__).read_bytes())
    text = json.dumps(result, indent=2, ensure_ascii=False, allow_nan=False) + "\n"
    if args.output:
        target = args.output.resolve()
        need(os.name != "nt" or target.drive.upper() == "C:", "output_must_be_on_C")
        need(not target.is_symlink(), "output_symlink_forbidden")
        with target.open("x", encoding="utf-8") as stream: stream.write(text)
    else:
        print(text, end="")
    return 0 if result["passed"] else 1




def verify_melee_actions(plan, report, events, engine, checks):
    epoch = report.get("measurementEpochMs" if engine == "modern" else "measurementStartedAtMs")
    end = report.get("observationEndedAtMs", report.get("measurementEndedAtMs"))
    need(number(epoch) and number(end), "actual_measurement_anchor_or_end_missing")
    checks.require(end >= epoch + plan["durationMs"], "full_measurement_duration_not_observed")
    packets, admissions, sends = index_evidence(events, engine, checks)
    context = combat_context(packets)
    records = normalize_combat_actions(events, engine, checks)
    expected_keys = {(a["actorOrdinal"], a["ticketSequence"]) for a in plan["actions"]}
    checks.require(set(records) == expected_keys, "missing_or_unplanned_action_evidence")
    for actor in plan["actors"]:
        ordinal = actor["ordinal"]
        checks.require(active_connection(admissions, ordinal, epoch) is not None, "actor_not_admitted_before_measurement", ordinal)
        checks.require(not any(epoch < row[0] < epoch + plan["durationMs"] for row in admissions.get(ordinal, []) if number(row[0])), "connection_replaced_during_measurement", ordinal)
    derived, used_receipts, used_sends, fences = [], set(), set(), defaultdict(list)
    for action in plan["actions"]:
        actor, ticket = action["actorOrdinal"], action["ticketSequence"]
        row = records.get((actor, ticket))
        out = {**action, "sentAtMs": None, "completed": False, "timeout": False, "notSent": False, "rejected": False,
               "latencyMs": None, "scheduleDelayMs": None, "terminalAtMs": None}
        if row is None:
            derived.append(out); continue
        checks.require(len(row["sent"]) <= 1 and len(row["notSent"]) <= 1 and not (row["sent"] and row["notSent"]), "duplicate_send_or_sent_notSent_conflict", actor, ticket)
        if engine == "crystal": checks.require(len(row["offers"]) == 1, "offered_record_missing_or_duplicate", actor, ticket)
        spec = (row["sent"] or row["notSent"] or row["outcomes"] or row["timeouts"] or row["offers"])[0]
        planned = spec.get("dueAtMs" if engine == "modern" else "plannedAtMs")
        deadline = spec.get("deadlineAtMs")
        checks.require(number(planned) and planned == epoch + action["dueMs"] and deadline == epoch + action["deadlineMs"], "absolute_anchor_or_deadline_drift", actor, ticket)
        checks.require(spec.get("kind" if engine == "modern" else "action", spec.get("kind")) == action["kind"], "action_kind_drift", actor, ticket)
        if not row["sent"]:
            out["notSent"] = bool(row["notSent"])
            checks.require(out["notSent"] and not row["timeouts"] and not row["receipts"], "unsent_action_evidence_inconsistent", actor, ticket)
            derived.append(out); continue
        sent = row["sent"][0]; when = sent.get("sentAtMs")
        if not checks.require(number(when) and number(planned) and number(deadline), "actual_send_time_missing", actor, ticket):
            derived.append(out); continue
        out["sentAtMs"], out["scheduleDelayMs"] = when, when - planned
        checks.require(planned <= when <= deadline, "send_outside_original_absolute_budget", actor, ticket)
        raw_sends = sends.get((actor, when), [])
        if engine == "modern":
            raw_sends = [e for e in raw_sends if e.get("command", {}).get("type") == ("attackDirection" if action["kind"] == "attack" else action["kind"]) and
                         (e["command"].get("message") == action["nonce"] if action["kind"] == "chat" else e["command"].get("spell") == 0 and e["command"].get("direction") == sent.get("direction"))]
        else:
            raw_sends = [e for e in raw_sends if e.get("sequence") == sent.get("sentSequence") and e.get("correlation", {}).get("ticketSequence") == ticket and e.get("packet") == ("Attack" if action["kind"] == "attack" else "Chat")]
            if action["kind"] == "attack" and len(raw_sends) == 1:
                correlation = raw_sends[0].get("correlation", {})
                need(correlation.get("targetActorOrdinal") == action["targetActorOrdinal"] and correlation.get("expectedTargetSharedId") == sent.get("expectedTargetSharedId")
                     and correlation.get("spell") == 0 and combat_direction(correlation.get("direction")) == combat_direction(sent.get("direction")), "native_actual_attack_send_projection_invalid")
        checks.require(len(raw_sends) == 1, "unique_actual_send_packet_missing", actor, ticket)
        if len(raw_sends) == 1: used_sends.add(id(raw_sends[0]))
        connection = raw_sends[0].get("connection") if len(raw_sends) == 1 else None
        checks.require(connection == active_connection(admissions, actor, when), "send_connection_not_current_admission", actor, ticket)
        expected = [actor] + plan["actors"][actor]["peerOrdinals"]
        checks.require(sent.get("expectedPeerOrdinals") == plan["actors"][actor]["peerOrdinals"], "observer_set_drift", actor, ticket)
        floor_raw = sent.get("receiveSequenceFloors")
        floors = ({int(k): v for k, v in floor_raw.items()} if type(floor_raw) is dict and all(str(k).isdigit() for k in floor_raw)
                  else {r.get("actorOrdinal"): r.get("receiveSeq") for r in floor_raw} if type(floor_raw) is list else {})
        checks.require(set(floors) == set(expected) and all(integer(v) for v in floors.values()), "fresh_receive_floor_evidence_missing", actor, ticket)
        checks.require(type(floor_raw) in [dict, list] and len(floor_raw) == len(floors), "duplicate_receive_floor_observer", actor, ticket)
        if engine == "modern" and type(floor_raw) is list:
            checks.require(all(r.get("connection") == active_connection(admissions, r.get("actorOrdinal"), when) for r in floor_raw), "receive_floor_connection_epoch_missing_or_stale", actor, ticket)
        ids = ({r.get("observerOrdinal"): r.get("objectId") for r in sent.get("expectedSharedIds", [])}
               if engine == "modern" else {observer: sent.get("expectedSharedId") for observer in expected})
        if engine == "modern":
            id_rows = sent.get("expectedSharedIds", [])
            checks.require(len(id_rows) == len(ids) and set(ids) == set(expected if action["kind"] == "chat" else expected[1:])
                           and all(r.get("connection") == active_connection(admissions, r.get("observerOrdinal"), when) for r in id_rows), "shared_id_observer_or_connection_ambiguous", actor, ticket)
        if action["kind"] == "walk":
            origin = sent.get("before" if engine == "modern" else "origin")
            # Some modern actionSent versions expose actualTarget but keep before
            # only in the terminal projection; both are from the same unique gate.
            if origin is None and engine == "modern" and row["outcomes"]:
                origin = row["outcomes"][0].get("before")
            checks.require(type(origin) is dict and origin.get("y") == action["target"]["y"]
                           and number(origin.get("x")) and origin["x"] + (1 if action["direction"] == "Right" else -1) == action["target"]["x"], "walk_origin_direction_target_inconsistent", actor, ticket)
        seen = {}
        for ref in row["receipts"]:
            key = packet_key(ref)
            if key in seen: continue  # repeated identical terminal projection, not another receipt
            raw = packets.get(key)
            valid = raw is not None and raw["at"] == ref["at"] and raw["packet"] == ref["packet"]
            if not checks.require(valid, "receipt_without_matching_raw_packet", actor, ticket): continue
            checks.require(key not in used_receipts, "one_packet_reused_by_multiple_actions", actor, ticket)
            used_receipts.add(key); seen[key] = raw
            receiver = raw["actor"]
            checks.require(receiver in expected and raw["connection"] == active_connection(admissions, receiver, when)
                           and integer(floors.get(receiver)) and raw["sequence"] > floors.get(receiver, math.inf)
                           and raw["at"] >= when, "receipt_connection_floor_or_time_ambiguous", actor, ticket)
        grouped = defaultdict(list)
        for raw in seen.values(): grouped[raw["actor"]].append(raw)
        if action["kind"] != "attack": checks.require(all(len(values) == 1 for values in grouped.values()), "multiple_receipts_for_one_observer", actor, ticket)
        correct, rejected = {}, False
        if action["kind"] == "attack":
            correct, outcome = melee_attack_terminal(plan, action, sent, row, list(seen.values()), packets, admissions, floors, ids, when, engine, context)
            out["targetOutcome"] = outcome
        for observer, values in ([] if action["kind"] == "attack" else grouped.items()):
            raw = values[0]; body = raw["body"]
            location = body.get("location", body.get("position", {"x": body.get("x"), "y": body.get("y")}))
            if action["kind"] == "walk":
                required_packet = "UserLocation" if observer == actor else "ObjectWalk"
                matching = raw["packet"] == required_packet and location == action["target"]
                if observer != actor: matching &= integer(ids.get(observer)) and ids.get(observer) > 0 and body.get("objectId") == ids.get(observer)
                if observer == actor and raw["packet"] == "UserLocation" and location != action["target"]:
                    origin = sent.get("before" if engine == "modern" else "origin")
                    if origin is None and engine == "modern" and row["outcomes"]:
                        origin = row["outcomes"][0].get("before")
                    rejected = location == origin
                    checks.require(rejected, "unexpected_authoritative_owner_transform", actor, ticket)
            else:
                text = body.get("text", body.get("message"))
                nonce_match = (type(text) is str and text.endswith(action["nonce"]) and
                               (len(text) == len(action["nonce"]) or text[-len(action["nonce"])-1].isspace() or text[-len(action["nonce"])-1] == ":")) if engine == "modern" else body.get("nonceHash") == action["nonceSha256"]
                matching = raw["packet"] == "ObjectChat" and integer(ids.get(observer)) and ids.get(observer) > 0 and body.get("objectId") == ids.get(observer) and nonce_match
            checks.require(matching or (rejected and observer == actor), "raw_receipt_does_not_match_planned_action", actor, ticket)
            if matching: correct[observer] = raw["at"]
        complete_receipts = set(correct) == set(expected)
        last = max(correct.values()) if correct else None
        timeout_marked = bool(row["timeouts"]) or any(r.get("timedOut" if engine == "modern" else "timeout") is True for r in row["outcomes"])
        for timeout in row["timeouts"]:
            observed = timeout.get("atMs" if engine == "modern" else "observedAtMs")
            checks.require(number(observed) and observed >= deadline, "timeout_before_original_absolute_deadline", actor, ticket)
        out["rejected"] = rejected
        out["timeout"] = timeout_marked or (not rejected and (not complete_receipts or last > deadline))
        out["completed"] = complete_receipts and last <= deadline and not timeout_marked and not rejected
        if complete_receipts:
            out["terminalAtMs"] = last
        elif rejected:
            out["terminalAtMs"] = max(r["at"] for r in seen.values())
        if out["completed"]: out["latencyMs"] = last - when
        checks.require(out["terminalAtMs"] is not None, "sent_action_remains_causally_unresolved", actor, ticket)
        checks.require(row["outcomes"] or row["timeouts"], "original_outcome_not_recorded", actor, ticket)
        if timeout_marked and complete_receipts and last <= deadline:
            checks.require(False, "timeout_conflicts_with_original_on_time_receipts", actor, ticket)
        for outcome in row["outcomes"]:
            claimed = outcome.get("passed" if engine == "modern" else "completedOnTime")
            checks.require(type(claimed) is bool and claimed == out["completed"], "claimed_outcome_disagrees_with_raw_evidence", actor, ticket)
        fences[actor].append(out)
        derived.append(out)
    for actor, actions in fences.items():
        previous = None
        for action in sorted(actions, key=lambda a: a["sentAtMs"]):
            if previous is not None:
                checks.require(previous["terminalAtMs"] is not None and previous["terminalAtMs"] <= action["sentAtMs"], "new_send_before_previous_unique_terminal", actor, action["ticketSequence"])
            previous = action
    allowed = ({"clientVersion", "login", "startGame", "keepAlive", "logOut", "removeItem", "equipItem", "attackDirection", "chat", "changeAMode", "useItem"}
               if engine == "modern" else {"ClientVersion", "Login", "StartGame", "KeepAlive", "LogOut", "RemoveItem", "EquipItem", "Attack", "Chat", "ChangeAMode", "UseItem"})
    for rows in sends.values():
        for raw in rows:
            command = raw.get("command", {}).get("type") if engine == "modern" else raw.get("packet")
            checks.require(command in allowed, "unsupported_or_debug_command_in_run")
            if command in ["attackDirection", "chat", "Attack", "Chat"]:
                checks.require(id(raw) in used_sends, "extra_gameplay_send_or_retry_outside_plan")
    windows = []
    for actor in range(plan["population"]):
        for minute in range(plan["durationMs"] // 60000):
            rows = [r for r in derived if r["actorOrdinal"] == actor and minute * 60000 <= r["dueMs"] < (minute + 1) * 60000]
            completed = [r for r in rows if r["completed"]]
            count, timeouts = len(rows), sum(r["timeout"] for r in rows)
            move = p95([r["latencyMs"] for r in completed if r["kind"] == "attack"])
            chat = p95([r["latencyMs"] for r in completed if r["kind"] == "chat"])
            schedule = p95([r["scheduleDelayMs"] for r in rows if r["sentAtMs"] is not None])
            threshold = plan["thresholds"]
            passed = (count > 0 and len(completed) / count >= threshold["minimumCompletionRatio"]
                      and timeouts / count < threshold["maximumTimeoutRatioExclusive"]
                      and move is not None and move <= threshold["attackP95Ms"]
                      and chat is not None and chat <= threshold["chatP95Ms"]
                      and schedule is not None and schedule <= threshold["scheduleP95Ms"])
            windows.append({"actorOrdinal": actor, "minute": minute + 1, "offered": count, "completed": len(completed),
                            "timeout": timeouts, "notSent": sum(r["notSent"] for r in rows), "rejected": sum(r["rejected"] for r in rows),
                            "completionRatio": len(completed) / count if count else None, "attackP95Ms": move,
                            "chatP95Ms": chat, "scheduleP95Ms": schedule, "passed": passed})
    verify_combat_baselines(plan, report, events, packets, admissions, engine)
    verify_potions(plan, report, events, packets, admissions, derived, engine)
    return windows, derived, packets, admissions


def normalize_combat_actions(events, engine, checks):
    primary = [e for e in events if not (engine == "modern" and e.get("kind") == "potion"
               and e.get("event") in {"actionSent", "actionTerminal", "actionTimeout", "notSent"})]
    rows = normalize_actions(primary, engine, checks)
    if engine == "crystal":
        for e in events:
            value = e.get("result", e)
            proof = value.get("combatEvidence") if e.get("kind") in {"actionOutcome", "lateClosure"} else None
            if type(proof) is dict:
                key = value.get("actorOrdinal"), value.get("ticketSequence")
                need(key in rows and proof.get("invalidReason") is None, "native_combat_original_evidence_invalid")
                rows[key]["receipts"].extend(receipt(f, engine) for f in proof.get("facts", []))
    return rows


def combat_position(body):
    value = body.get("location", body.get("position", body))
    return {k: value.get(k) for k in ["x", "y"]} if type(value) is dict else None


def combat_direction(value):
    names = ["Up", "UpRight", "Right", "DownRight", "Down", "DownLeft", "Left", "UpLeft"]
    return names.index(value) if value in names else value if type(value) is int and 0 <= value < 8 else None


def combat_context(packets):
    sessions, prior = defaultdict(list), {}
    for p in packets.values(): sessions[(p["actor"], p["connection"])].append(p)
    for values in sessions.values():
        values.sort(key=lambda p: p["sequence"])
        need(all(b["at"]>=a["at"] for a,b in zip(values,values[1:])),"combat_receive_clock_regressed_within_connection")
        hp, owner_id, authenticated = None, None, False
        for p in values:
            body = p["body"]
            if p["packet"] == "UserInformation": authenticated, owner_id = True, body.get("objectId")
            if p["packet"] == "LogOutSuccess": authenticated = False
            if not authenticated: continue
            if p["packet"] == "HealthChanged":
                prior[packet_key(p)] = hp
                need(type(body.get("hp")) is int and body["hp"] > 0 and integer(body.get("mp")), "combat_death_or_invalid_health")
                if "hpBefore" in body: need(body["hpBefore"] == hp, "native_raw_hp_before_disagrees_with_prior_packet")
                hp = body["hp"]
            elif p["packet"] in ["UserInformation", "worldSnapshot"]:
                value = body.get("playerHp") if p["packet"] == "worldSnapshot" else body.get("hp")
                mp = body.get("playerMp") if p["packet"] == "worldSnapshot" else body.get("mp")
                need(type(value) is int and value > 0 and integer(mp), "combat_snapshot_death_or_missing_hp")
                hp = value
            need(p["packet"] != "Death" and not(p["packet"]=="ObjectDied" and body.get("objectId")==owner_id), "combat_death_is_hard_failure")
    return {"sessions": sessions, "hpBefore": prior}


def combat_ui(context, actor, connection):
    values = [p for p in context["sessions"].get((actor, connection), []) if p["packet"] == "UserInformation"]
    need(len(values) == 1, "combat_unique_owner_identity_packet_missing")
    return values[0]


def combat_shared_id(context, observer, connection, floor, position, engine, target_ui):
    if engine == "crystal": return target_ui["body"].get("objectId")
    visible,names = {},{}
    def observe(uid,body):
        name=body.get("name")
        if type(name)is str and name:
            if name in names:visible.pop(names[name],None)
            names[name]=uid
        visible[uid]={"position":combat_position(body),"name":name}
    for p in context["sessions"].get((observer, connection), []):
        if p["sequence"] > floor: break
        if p["packet"] == "ObjectPlayer": observe(p["body"].get("objectId"),p["body"])
        elif p["packet"] == "ObjectRemove": visible.pop(p["body"].get("objectId"), None)
        elif p["packet"] == "worldSnapshot":
            for entity in p["body"].get("entities", []):
                if entity.get("kind") == "player" and entity.get("objectId") != p["body"].get("playerObjectId"):
                    observe(entity.get("objectId"),entity)
    # Compact ObjectPlayer fixtures retain the actual wire name but omit its
    # position. Bind that identity to this actor's real UserInformation; never
    # take a claimed shared ID from actionSent as its own proof.
    expected_name=target_ui["body"].get("name")
    matches=[uid for uid,row in visible.items() if integer(uid) and uid>0 and
             (row["name"]==expected_name and (row["position"]==position or row["position"]=={"x":None,"y":None})
              if type(expected_name)is str and expected_name else row["position"]==position)]
    need(len(matches) == 1, "combat_shared_player_at_planned_cell_not_unique")
    return matches[0]


def melee_attack_terminal(plan, action, sent, row, seen, packets, admissions, floors, ids, when, engine, context):
    actor, target = action["actorOrdinal"], action["targetActorOrdinal"]
    peers = plan["actors"][actor]["peerOrdinals"]
    origin, destination = plan["actors"][actor]["start"], plan["actors"][target]["start"]
    delta = destination["x"] - origin["x"], destination["y"] - origin["y"]
    deltas = [(0,-1),(1,-1),(1,0),(1,1),(0,1),(-1,1),(-1,0),(-1,-1)]
    need(delta in deltas and combat_direction(sent.get("direction")) == deltas.index(delta), "combat_attack_geometry_or_direction_drift")
    connections = {a: active_connection(admissions, a, when) for a in [actor] + peers}
    target_ui = combat_ui(context, target, connections[target])
    attacker_ui = combat_ui(context, actor, connections[actor])
    victim_id = target_ui["body"].get("objectId")
    need(integer(victim_id) and victim_id > 0 and combat_position(target_ui["body"]) == destination, "combat_target_owner_identity_or_position_invalid")
    for peer in peers:
        expected_id = combat_shared_id(context, peer, connections[peer], floors[peer], origin, engine, attacker_ui)
        need(ids.get(peer) == expected_id, "combat_attacker_shared_namespace_not_bound")
    attacker_id = ids[target]
    if engine == "modern":
        actual = sent.get("actualTarget", {})
        shared_target = combat_shared_id(context, actor, connections[actor], floors[actor], destination, engine, target_ui)
        need(actual.get("actorOrdinal") == target and actual.get("connection") == connections[target]
             and actual.get("objectId") == shared_target and actual.get("ownerObjectId") == victim_id
             and combat_position(actual) == destination, "combat_actual_target_not_bound_to_plan_and_namespace")
    else:
        need(sent.get("targetActorOrdinal") == target and sent.get("expectedTargetSharedId") == victim_id
             and sent.get("target") == destination and sent.get("origin") == origin, "native_attack_target_or_shared_identity_drift")
        for outcome in row["outcomes"]:
            proof = outcome.get("combatEvidence", {})
            expected = proof.get("expectedObservers", {})
            need(proof.get("actorOrdinal") == actor and proof.get("targetActorOrdinal") == target
                 and proof.get("attackerSharedId") == attacker_id and proof.get("targetSharedId") == victim_id
                 and proof.get("sentAtMs") == when and proof.get("deadlineAtMs") == sent.get("deadlineAtMs")
                 and set(expected) == {str(a) for a in connections}
                 and all(expected[str(a)].get("connection") == connections[a]
                         and expected[str(a)].get("receiveSequenceFloor") == floors[a] for a in connections), "native_combat_spec_not_bound_to_actual_send")
    need(seen, "combat_target_terminal_missing")
    upper = max(p["at"] for p in seen)
    owners = [p for p in context["sessions"][(actor, connections[actor])] if p["packet"] == "UserLocation"
              and p["sequence"]>floors[actor] and when<=p["at"]<=upper]
    need(len(owners) == 1 and combat_position(owners[0]["body"]) == origin
         and combat_direction(owners[0]["body"].get("direction")) == deltas.index(delta), "combat_owner_authoritative_response_missing_or_wrong")
    correct = {actor: owners[0]["at"]};peer_animations=[]
    for peer in peers:
        animations = [p for p in context["sessions"][(peer,connections[peer])] if p["packet"] == "ObjectAttack" and p["body"].get("objectId") == ids[peer]
                      and p["sequence"]>floors[peer] and when<=p["at"]<=upper]
        need(len(animations) == 1 and combat_position(animations[0]["body"]) == origin
             and combat_direction(animations[0]["body"].get("direction")) == deltas.index(delta)
             and type(animations[0]["body"].get("spell"))is int and animations[0]["body"].get("spell") == 0, "combat_all_peer_ordinary_attack_animation_missing")
        correct[peer] = animations[0]["at"]
        peer_animations.extend(animations)
    target_packets = [p for p in context["sessions"][(target, connections[target])]
                      if p["sequence"] > floors[target] and when <= p["at"] <= upper]
    impacts, indicators, decreases, launches = [], [], [], []
    for p in target_packets:
        body, kind = p["body"], p["packet"]
        if kind == "ObjectAttack":
            if body.get("objectId") == attacker_id: launches.append(p)
            else:
                direction = combat_direction(body.get("direction")); pos = combat_position(body)
                need(direction is None or pos is None or {"x":pos["x"]+deltas[direction][0],"y":pos["y"]+deltas[direction][1]} != destination, "combat_competing_target_launch")
        need(kind != "ObjectRangeAttack", "combat_competing_range_attack")
        need(not(kind in ["ObjectRemove","ObjectDied"] and body.get("objectId") in [victim_id,attacker_id]),"combat_participant_died_or_left_AOI")
        if kind in ["Struck", "ObjectStruck"]:
            victim = victim_id if kind == "Struck" else body.get("objectId")
            if victim == victim_id:
                need(body.get("attackerId", body.get("attacker_id")) == attacker_id, "combat_competing_or_wrong_attacker")
                need(engine != "crystal" or kind == "Struck", "native_unexpected_self_ObjectStruck")
                impacts.append(p)
        if kind == "DamageIndicator" and body.get("objectId", body.get("object_id")) == victim_id:
            indicators.append(p)
        if kind == "HealthChanged":
            prior = context["hpBefore"].get(packet_key(p));need(type(prior) is int, "combat_prior_hp_not_reconstructible")
            if body["hp"] < prior: decreases.append(p)
    need(len(launches) == 1 and len(indicators) == 1 and len(impacts) <= 1 and len(decreases) <= 1, "combat_target_chain_missing_or_ambiguous")
    indicator = indicators[0];body = indicator["body"]
    need(launches[0]["sequence"]<indicator["sequence"] and (not impacts or launches[0]["sequence"]<impacts[0]["sequence"]<indicator["sequence"]),"combat_launch_struck_indicator_FIFO_broken")
    types = [body[key] for key in ["type", "damage_type", "damageType"] if key in body];damage = body.get("damage")
    need(types and all(type(value) is int for value in types) and type(damage) is int, "combat_damage_type_or_magnitude_missing")
    need(all(value == types[0] for value in types), "combat_damage_type_aliases_conflict")
    dtype = types[0]
    if dtype == 1 and damage == 0:
        need(not impacts and not decreases, "combat_miss_contested_by_damage_or_struck")
        outcome, terminal = "explicitMiss", indicator
    elif dtype == 0 and damage == 0:
        need(len(impacts) == 1 and not decreases, "combat_zero_block_missing_attacker_bound_struck")
        outcome, terminal = "explicitZeroBlock", indicator
    else:
        magnitude = damage if engine == "modern" else -damage
        need(dtype == 0 and magnitude > 0 and len(impacts) == len(decreases) == 1, "combat_positive_hit_chain_incomplete")
        health = decreases[0];prior = context["hpBefore"][packet_key(health)]
        need(prior - health["body"]["hp"] == magnitude, "combat_hp_operation_not_exact_damage_delta")
        if engine == "crystal":
            need(launches[0]["sequence"] < impacts[0]["sequence"] < indicator["sequence"] < health["sequence"], "native_hit_FIFO_chain_broken")
            need(not any(p["packet"] == "HealthChanged" and impacts[0]["sequence"] < p["sequence"] < health["sequence"] for p in target_packets), "native_interleaved_health_in_hit_chain")
        outcome, terminal = "hit", health
    # Every fact used for positive completion must be explicitly referenced by this ticket.
    required = launches + impacts + indicators + decreases + owners + peer_animations
    seen_keys = {packet_key(p) for p in seen}
    need(all(packet_key(p) in seen_keys for p in required), "combat_required_chain_not_bound_to_ticket_receipts")
    correct[target] = max(correct[target], terminal["at"], indicator["at"], *(p["at"] for p in impacts))
    for original in row["outcomes"]:
        if original.get("passed" if engine=="modern" else "completedOnTime") is True:
            claim=(original.get("outcome",{}).get("kind") if engine=="modern" else original.get("combatEvidence",{}).get("terminalKind"))
            expected=outcome if engine=="modern" else {"hit":"positiveDamage","explicitMiss":"miss","explicitZeroBlock":"explicitZeroDamage"}[outcome]
            need(claim==expected,"claimed_combat_terminal_disagrees_with_raw_chain")
    return correct, outcome


def verify_combat_baselines(plan, report, events, packets, admissions, engine):
    epoch = report["measurementEpochMs" if engine == "modern" else "measurementStartedAtMs"]
    life = Lifecycle(engine, events, packets, Checks())
    baselines = ([e for e in events if e.get("event") == "combatBaseline"] if engine == "modern" else report.get("combatBaselines", []))
    need(len(baselines) == plan["population"] and {e.get("actorOrdinal") for e in baselines} == set(range(plan["population"])), "all_melee_baselines_required")
    maxima = {}
    for row in baselines:
        actor = row["actorOrdinal"];connection = active_connection(admissions, actor, epoch);ui = life.ui(actor, connection)
        body = ui["body"]
        need(row.get("connection") == connection and combat_position(body) == plan["actors"][actor]["start"]
             and body.get("magicCount") == 0 and body.get("hasHero") is False and body.get("creatureSummoned") is False
             and body.get("intelligentCreatureCount") == 0, "melee_baseline_identity_skills_or_pets_invalid")
        if engine == "modern":
            need(life.ref(row.get("userInformation", {}), actor, "UserInformation") is ui
                 and row.get("knownSkills") == [] and row.get("activeBuffs") == [] and row.get("hero") is None
                 and row.get("intelligentCreatures") == [], "modern_melee_baseline_evidence_missing")
            maximum = body.get("maxHp");source = life.ref(row.get("sourceReceipt", {}), actor, "worldSnapshot")
            need(source["connection"] == connection and source["at"] <= epoch and ui["sequence"] < source["sequence"], "modern_melee_baseline_snapshot_not_fresh")
        else:
            maximum = row.get("maximumHp")
            need(row.get("initialUserInformationReceiveSequence") == ui["sequence"] and row.get("initialUserInformationAtMs") == ui["at"]
                 and row.get("activeOwnedBuffCount") == 0, "native_melee_baseline_not_bound_to_UI")
        need(type(maximum) is int and maximum > 0 and body.get("hp") == maximum, "melee_initial_full_hp_not_proved")
        maxima[actor] = maximum
        modes = ([e for e in events if e.get("event") == "combatMode" and e.get("actorOrdinal") == actor]
                 if engine == "modern" else [e["result"] for e in events if e.get("kind") == "attackModeAll" and e.get("result", {}).get("actorOrdinal") == actor])
        need(len(modes) == 1 and modes[0].get("mode") == 5 and modes[0].get("connection") == connection, "ordinary_attack_mode_all_missing")
        mode = modes[0]
        ack = (life.ref(mode.get("ack", {}), actor, "ChangeAMode") if engine == "modern"
               else packets.get((actor, connection, mode.get("receiveSequence"))))
        need(ack is not None and ack["packet"] == "ChangeAMode" and ack["body"].get("mode") == 5 and ack["at"] <= epoch, "attack_mode_ACK_unbound")
        sends = [e for e in events if e.get("actorOrdinal") == actor and e.get("connection") == connection
                 and ((engine == "modern" and e.get("event") == "sent" and e.get("command", {}).get("type") == "changeAMode" and e["command"].get("mode") == 5)
                      or (engine == "crystal" and e.get("kind") == "packet" and e.get("direction") == "sent" and e.get("packet") == "ChangeAMode" and e.get("sequence") == mode.get("sentSequence")))]
        clock = "atMs" if engine == "modern" else "monotonicMs"
        need(len(sends) == 1 and number(sends[0].get(clock)) and sends[0][clock] <= ack["at"] <= sends[0][clock] + 2500, "attack_mode_normal_send_or_deadline_missing")
        owned_id = body.get("objectId")
        need(not any(p["actor"] == actor and p["connection"] == connection and p["packet"] == "AddBuff"
                     and p["body"].get("objectId") == owned_id for p in packets.values()), "melee_owned_buff_is_hard_failure")
    actual_modes=[e for e in events if (engine=="modern" and e.get("event")=="sent" and e.get("command",{}).get("type")=="changeAMode")
                  or (engine=="crystal" and e.get("kind")=="packet" and e.get("direction")=="sent" and e.get("packet")=="ChangeAMode")]
    need(len(actual_modes)==plan["population"],"extra_or_unbound_attack_mode_send")
    return maxima


def potion_stock(state, engine):
    rows = {}
    for uid, item in state["items"].items():
        p = item["payload"]
        is_potion = p.get("itemIndex") == 662 if engine == "crystal" else p.get("key") == "crystal-item-662"
        if is_potion:
            need(item["grid"] in (["inventory"] if engine == "crystal" else ["inventoryItems", "beltItems"]), "potion_in_non_inventory_grid")
            count = p.get("count" if engine == "crystal" else "quantity")
            need(type(count) is int and count > 0, "potion_stock_count_invalid")
            rows[uid] = count
    return rows


def native_mutable_state(ui, packets, end_sequence, *, include_end=True):
    """Original UseItem has no count. Derive only its documented ACK decrement.

    Touched Save-byte hashes cannot be recomputed without private binary fields;
    their full equality is independently closed by final fresh UI and LoadAccounts.
    """
    state = copy.deepcopy(carried(ui["body"].get("authoritativeItemState"), "crystal"));touched = set()
    hp, mp, position = ui["body"].get("hp"), ui["body"].get("mp"), combat_position(ui["body"])
    values = sorted([p for p in packets.values() if p["actor"] == ui["actor"] and p["connection"] == ui["connection"]
                     and ui["sequence"] < p["sequence"] and (p["sequence"] <= end_sequence if include_end else p["sequence"] < end_sequence)], key=lambda p:p["sequence"])
    for p in values:
        body, kind = p["body"], p["packet"]
        if kind == "HealthChanged": hp, mp = body.get("hp"), body.get("mp")
        if kind == "UserLocation": position = combat_position(body)
        if kind in ["DuraChanged", "DeleteItem", "UseItem"]:
            uid = body.get("uniqueID");need(uid in state["items"], "native_owned_mutation_unknown_UID")
            entry = state["items"][uid];touched.add(uid)
            if kind == "DuraChanged":
                dura = body.get("currentDura");need(integer(dura) and dura <= entry["payload"].get("maxDura", -1), "native_invalid_durability_operation")
                entry["payload"]["currentDura"] = dura
            else:
                if kind == "UseItem":
                    need(body.get("success") is True and body.get("grid") == 1 and entry["payload"].get("itemIndex") == 662
                         and body.get("remainingCountPresentInNativePacket") is False, "native_UseItem_ACK_or_count_boundary_invalid")
                    count = 1
                else: count = body.get("count")
                need(type(count) is int and 0 < count <= entry["payload"].get("count", 0), "native_invalid_count_operation")
                entry["payload"]["count"] -= count
                if entry["payload"]["count"] == 0: del state["items"][uid]
        need(kind != "RefreshItem", "native_refresh_item_full_projection_missing")
    return state, touched, {"hp":hp,"mp":mp,"position":position}


def native_semantics_equal(expected, actual, touched):
    if expected["scalars"] != actual["scalars"] or set(expected["items"]) != set(actual["items"]): return False
    for uid in expected["items"]:
        a,b=copy.deepcopy(expected["items"][uid]),copy.deepcopy(actual["items"][uid])
        if uid in touched:
            a["payload"].pop("originalUserItemSaveSha256", None);b["payload"].pop("originalUserItemSaveSha256", None)
        if a != b:return False
    return True


def verify_potions(plan, report, events, packets, admissions, primaries, engine):
    epoch = report["measurementEpochMs" if engine == "modern" else "measurementStartedAtMs"]
    maxima = verify_combat_baselines(plan, report, events, packets, admissions, engine)
    life = Lifecycle(engine, events, packets, Checks());context = combat_context(packets)
    need(not any(e.get("event", e.get("kind")) in ["potionFailure", "potionTimeout"] for e in events), "issued_potion_failure_is_hard_failure")
    issued = ([e for e in events if e.get("event") == "potionSent"] if engine == "modern"
              else [e["result"] for e in events if e.get("kind") == "potion"])
    actual_sends = [e for e in events if (engine == "modern" and e.get("event") == "sent" and e.get("command", {}).get("type") == "useItem")
                    or (engine == "crystal" and e.get("kind") == "packet" and e.get("direction") == "sent" and e.get("packet") == "UseItem")]
    need(len(issued) == len(actual_sends), "unaccounted_potion_send_or_retry")
    used, previous, used_acks = set(), {}, set()
    for row in sorted(issued,key=lambda e:e.get("sentAtMs", -1)):
        actor,connection,when,uid = row.get("actorOrdinal"),row.get("connection"),row.get("sentAtMs"),row.get("uniqueId")
        need(actor in maxima and number(when) and epoch <= when < epoch+plan["durationMs"] and connection == active_connection(admissions,actor,when), "potion_issue_outside_current_measurement")
        sends=[e for e in actual_sends if e.get("actorOrdinal")==actor and e.get("connection")==connection and e.get("atMs" if engine=="modern" else "monotonicMs")==when]
        need(len(sends)==1 and id(sends[0]) not in used, "potion_actual_send_not_unique");used.add(id(sends[0]))
        need(actor not in previous or when-previous[actor][0]>=2000 and when>=previous[actor][1], "potion_interval_or_pending_gate_broken")
        need(all(p["sentAtMs"] is None or p["actorOrdinal"]!=actor or not(p["sentAtMs"]<=when<(p["terminalAtMs"] or math.inf)) for p in primaries), "potion_issued_while_primary_unresolved")
        if engine == "modern":
            sequence=row.get("potionSequence");terminals=[e for e in events if e.get("event")=="potionTerminal" and e.get("actorOrdinal")==actor and e.get("potionSequence")==sequence]
            need(len(terminals)==1, "potion_unique_terminal_missing");terminal=terminals[0]
            before_rows=[e for e in events if e.get("event")=="potionSourceState" and e.get("actorOrdinal")==actor and e.get("potionSequence")==sequence]
            after_rows=[e for e in events if e.get("event")=="potionStateEvidence" and e.get("actorOrdinal")==actor and e.get("potionSequence")==sequence]
            need(len(before_rows)==len(after_rows)==1, "potion_independent_full_stock_state_missing")
            before_raw,before=life.snapshot(before_rows[0].get("sourceReceipt",{}),actor,before_rows[0].get("state"))
            after_raw,after=life.snapshot(after_rows[0].get("sourceReceipt",{}),actor,after_rows[0].get("state"))
            ack=life.ref(terminal.get("proof",{}).get("ack",{}),actor,"UseItem")
            need(ack["body"].get("uniqueId")==uid and ack["body"].get("grid")=="Inventory" and ack["body"].get("success") is True
                 and sends[0]["command"].get("uniqueId")==uid and sends[0]["command"].get("grid")=="inventory", "ordinary_potion_UID_grid_ACK_invalid")
            need(before_raw["connection"]==ack["connection"]==after_raw["connection"]==connection and before_raw["sequence"]<ack["sequence"]<after_raw["sequence"]
                 and before_raw["at"]<=when<=ack["at"]<=after_raw["at"]<=when+2500, "potion_stock_state_or_original_deadline_failed")
            need(terminal.get("passed") is True and terminal.get("timedOut") is False, "potion_original_failure_sticky")
            hp=row.get("hpAtIssue");ending=after_raw["at"]
            expected_after=copy.deepcopy(before)
            need(uid in expected_after["items"],"potion_selected_UID_not_owned")
            selected_item=expected_after["items"][uid]["payload"];selected_item["quantity"]-=1
            user=selected_item.get("tooltipSource",{}).get("userItem")
            if type(user)is dict:user["count"]=selected_item["quantity"]
            if not selected_item["quantity"]:del expected_after["items"][uid]
            for raw in context["sessions"][(actor,connection)]:
                if before_raw["sequence"]<raw["sequence"]<=after_raw["sequence"]:
                    if raw["packet"]=="DuraChanged":modern_item_patch(expected_after,raw)
                    if raw["packet"]=="DeleteItem":need(raw["body"].get("uniqueId")==uid and raw["body"].get("count")==1,"potion_unrelated_stock_deleted")
            need(same_carried(expected_after,after),"potion_full_roster_has_unexplained_change")
        else:
            ack=packets.get((actor,connection,row.get("receiveSequence")))
            need(ack is not None and ack["packet"]=="UseItem" and ack["at"]==row.get("receivedAtMs") and sends[0].get("sequence")==row.get("sentSequence")
                 and ack["body"].get("uniqueID")==uid and ack["body"].get("grid")==1 and ack["body"].get("success") is True
                 and ack["body"].get("remainingCountPresentInNativePacket") is False, "native_potion_ACK_unbound_or_false_count_claim")
            before,after=carried(row.get("before"),engine),carried(row.get("after"),engine)
            prior_packets=[p for p in context["sessions"][(actor,connection)] if p["at"]<=when and p["sequence"]<ack["sequence"]]
            need(prior_packets,"native_potion_prior_receive_boundary_missing")
            expected,touched,prior_state=native_mutable_state(life.ui(actor,connection),packets,prior_packets[-1]["sequence"])
            need(native_semantics_equal(expected,before,touched), "native_potion_prior_stock_not_derived_from_raw_owned_state")
            expected_after,changed,unused=native_mutable_state(life.ui(actor,connection),packets,ack["sequence"])
            need(native_semantics_equal(expected_after,after,changed),"native_potion_post_ACK_roster_not_exact_derived_state")
            need(row.get("freshStockSnapshotClaimed") is False and row.get("finalFreshAndAllDiskReadbackRequired") is True
                 and row.get("laterHpIncreaseIsUniquePotionProof") is False, "native_potion_immediate_evidence_boundary_misrepresented")
            hp=row.get("hpAtDecision");ending=ack["at"]
        deadline=row.get("deadlineAtMs")
        need(packet_key(ack) not in used_acks,"potion_ACK_reused_by_multiple_doses");used_acks.add(packet_key(ack))
        need(number(deadline) and when<=ending<=deadline<=when+2500, "potion_ACK_original_deadline_failed")
        prior=[p for p in context["sessions"][(actor,connection)] if p["at"]<=when and p["sequence"]<ack["sequence"] and p["packet"] in ["HealthChanged","worldSnapshot","UserInformation"]]
        need(prior and type(hp) is int and 0<hp<=maxima[actor]*.7, "potion_reactive_hp_trigger_invalid")
        latest=prior[-1];observed_hp=latest["body"].get("playerHp") if latest["packet"]=="worldSnapshot" else latest["body"].get("hp")
        need(observed_hp==hp, "potion_trigger_not_bound_to_prior_authoritative_hp")
        first,last=potion_stock(before,engine),potion_stock(after,engine)
        expected_stock={key:value-(1 if key==uid else 0) for key,value in first.items()};expected_stock={key:value for key,value in expected_stock.items() if value>0}
        need(uid in first and last==expected_stock and sum(first.values())-sum(last.values())==1, "potion_stock_not_exactly_one")
        need(before["scalars"]==after["scalars"], "potion_changed_carried_scalars")
        for key in set(before["items"])|set(after["items"]):
            if key==uid:continue
            # Ordinary durability may progress during the 2.5s dose gate; other identity/grid fields must not change.
            a,b=copy.deepcopy(before["items"].get(key)),copy.deepcopy(after["items"].get(key))
            need(a is not None and b is not None and a["grid"]==b["grid"] and a["slot"]==b["slot"], "potion_unrelated_owned_item_moved_or_lost")
        need(all(p["sentAtMs"] is None or p["actorOrdinal"]!=actor or not(when<=p["sentAtMs"]<ending) for p in primaries), "primary_send_while_potion_pending")
        previous[actor]=(when,ending)
    need({packet_key(p) for p in packets.values() if p["packet"]=="UseItem"}==used_acks,"unsolicited_or_unaccounted_UseItem_ACK")
    return {"issued":len(issued),"nativeImmediateStockIsAckDerived":engine=="crystal","hpIncreaseUsedAsPotionProof":False}


def modern_carried_projection(state):
    return {**state["scalars"], **{grid:[{**copy.deepcopy(item["payload"]),"slot":item["slot"]}
             for uid,item in state["items"].items() if item["grid"]==grid]
             for grid in ["inventoryItems","equipmentItems","beltItems"]}}


def modern_item_patch(state, raw):
    body=raw["body"];uid=body.get("uniqueId");need(uid in state["items"],"modern_owned_patch_unknown_UID")
    item=state["items"][uid]["payload"];user=item.get("tooltipSource",{}).get("userItem")
    if raw["packet"]=="DuraChanged":
        value=body.get("currentDura");need(integer(value) and value<=item.get("durabilityMax",-1),"modern_durability_patch_invalid")
        item["durabilityCurrent"]=value
        if type(user) is dict:user["current_dura"]=value
    else:
        count=body.get("count");need(type(count) is int and 0<count<=item.get("quantity",0),"modern_count_patch_invalid")
        item["quantity"]-=count
        if type(user) is dict:user["count"]=item["quantity"]
        if not item["quantity"]:del state["items"][uid]


def modern_logout_state(actor, connection, ack, events, packets, life):
    rows=[e for e in events if e.get("event")=="logoutOwnedState" and e.get("actorOrdinal")==actor and e.get("connection")==connection]
    need(len(rows)==1,"unique_LogOutSuccess_frozen_owner_state_missing");row=rows[0];source=row.get("source",{})
    need(life.ref(source.get("logOutSuccess",{}),actor,"LogOutSuccess") is ack,"logout_state_not_bound_to_exact_ACK")
    snapshot=life.ref(source.get("snapshot",{}),actor,"worldSnapshot")
    raw_session=sorted([p for p in packets.values() if p["actor"]==actor and p["connection"]==connection and p["sequence"]<=ack["sequence"]],key=lambda p:p["sequence"])
    snapshots=[p for p in raw_session if p["packet"]=="worldSnapshot"]
    need(snapshots and snapshot is snapshots[-1],"logout_base_is_not_last_full_received_snapshot")
    candidates=[]
    for e in events:
        if e.get("event") not in ["ownedSnapshotEvidence","potionSourceState","potionStateEvidence"] or e.get("actorOrdinal")!=actor or e.get("connection")!=connection:continue
        raw,state=life.snapshot(e.get("sourceReceipt",{}),actor,e.get("state"))
        if raw["sequence"]<=snapshot["sequence"]:candidates.append((raw,state))
    need(candidates,"logout_full_carried_checkpoint_missing")
    base,state=max(candidates,key=lambda x:x[0]["sequence"]);state=copy.deepcopy(state)
    hp,mp=base["body"].get("playerHp"),base["body"].get("playerMp")
    position=combat_position(base["body"].get("owner",{}));direction=base["body"].get("owner",{}).get("direction")
    for raw in raw_session:
        if raw["sequence"]<=base["sequence"]:continue
        body,kind=raw["body"],raw["packet"]
        if kind in ["DuraChanged","DeleteItem"]:modern_item_patch(state,raw)
        if kind=="HealthChanged":hp,mp=body.get("hp"),body.get("mp")
        if kind=="UserLocation":position=combat_position(body);direction=body.get("direction",direction)
        if kind=="worldSnapshot":
            need(body.get("carriedFingerprint")==carried(modern_carried_projection(state),"modern")["fingerprint"],"unexplained_owned_roster_change_before_logout")
            hp,mp=body.get("playerHp"),body.get("playerMp");position=combat_position(body.get("owner",{}));direction=body.get("owner",{}).get("direction",direction)
    expected_patches=[p for p in raw_session if snapshot["sequence"]<p["sequence"]<ack["sequence"] and p["packet"] in ["HealthChanged","DuraChanged","DeleteItem"]]
    declared=[life.ref(ref,actor) for ref in source.get("patches",[])]
    need([packet_key(p) for p in declared]==[packet_key(p) for p in expected_patches],"logout_patch_list_omits_or_reorders_authoritative_packets")
    actual=carried(row.get("carried"),"modern");owner=row.get("state",{})
    identity=life.ui(actor,connection)["body"]
    need(all(owner.get(k)==base["body"].get("owner",{}).get(k)==identity.get(k) and owner.get(k)is not None
             for k in ["name","level","class","gender"]),"logout_owner_identity_not_bound_to_received_UI")
    need(same_carried(actual,state) and owner.get("items")==actual["fingerprint"] and owner.get("map")=="DogYoHyun"
         and {"x":owner.get("x"),"y":owner.get("y") }==position and owner.get("hp")==hp and owner.get("mp")==mp
         and owner.get("direction")==direction and type(hp) is int and hp>0,"logout_frozen_owner_state_not_reconstructed_from_raw")
    return {**owner,"carried":row["carried"]},actual


def verify_melee_lifecycle(plan, report, events, packets, admissions, checks, engine, *, disk=None):
    life=Lifecycle(engine,events,packets,checks)
    gear=(verify_modern_lifecycle(plan,report,events,packets,admissions,checks,gear_only=True) if engine=="modern"
          else verify_crystal_lifecycle(plan,report,events,packets,admissions,None,gear_only=True))
    epoch=report["measurementEpochMs" if engine=="modern" else "measurementStartedAtMs"]
    observed=report["observationEndedAtMs" if engine=="modern" else "measurementEndedAtMs"]
    selected=list(dict.fromkeys([0,plan["population"]-1]));final={};states={};exits={}
    need(len(report.get("cleanup",[]))==plan["population"] and {r.get("actorOrdinal") for r in report["cleanup"]}==set(range(plan["population"])),"all_melee_actor_cleanup_required")
    for actor in range(plan["population"]):
        conn=active_connection(admissions,actor,epoch);exits[actor]=life.exit(actor,conn);ack=exits[actor]["ack"]
        need(exits[actor]["sentAtMs"]>=observed+plan["settleBeforeLogoutMs"],"melee_logout_before_common_settle_phase")
        if engine=="modern":final[actor],states[actor]=modern_logout_state(actor,conn,ack,events,packets,life)
        else:
            value=ack["body"].get("lastAuthoritativeOwnerState");need(type(value)is dict,"native_logout_ACK_full_final_state_missing")
            actual=carried(value.get("itemState"),"crystal");expected,touched,vitals=native_mutable_state(life.ui(actor,conn),packets,ack["sequence"])
            need(native_semantics_equal(expected,actual,touched) and value.get("hp")==vitals["hp"] and value.get("mp")==vitals["mp"]
                 and value.get("position")==vitals["position"] and value.get("mapIndex")==440 and value.get("hp",0)>0,
                 "native_logout_state_differs_from_prior_authoritative_packets")
            final[actor],states[actor]=value,actual
        need(len(admissions.get(actor,[]))==1+(4 if actor in plan["equipmentActorOrdinals"] else 0)+(1 if actor in selected else 0),"melee_unplanned_reconnect_or_auth_retry")
    if engine=="modern":
        fresh_rows=[e for e in events if e.get("event")=="savedVerificationState"]
        need(len(fresh_rows)==len(selected) and {r.get("actorOrdinal") for r in fresh_rows}==set(selected),"melee_selected_fresh_save_evidence_missing")
        for row in fresh_rows:
            actor=row["actorOrdinal"];before,after=row.get("before",{}),row.get("after",{})
            need(all(before.get(k)==final[actor].get(k)==after.get(k) and before.get(k)is not None for k in ["map","x","y","direction","name","class","gender","level","hp","mp"])
                 and same_carried(carried(before.get("carried"),engine),states[actor]),"melee_fresh_save_not_exact_final_ACK_state")
            raw,fresh=life.snapshot(row.get("bootstrap",{}).get("snapshot",{}),actor,after.get("carried"));ui=life.ui(actor,raw["connection"])
            need(raw["connection"]!=active_connection(admissions,actor,epoch) and exits[actor]["closeIndex"]<raw["index"]
                 and same_carried(fresh,states[actor]) and ui["body"].get("hp")==final[actor]["hp"] and ui["body"].get("mp")==final[actor]["mp"]
                 and all(ui["body"].get(k)==final[actor].get(k) for k in ["name","level","class","gender"]),"melee_fresh_initial_UI_vitals_or_roster_mismatch")
            life.exit(actor,raw["connection"])
    else:
        rows=report.get("reentries",[]);need(len(rows)==len(selected) and {r.get("actorOrdinal") for r in rows}==set(selected),"native_melee_fresh_selection_missing")
        for row in rows:
            actor=row["actorOrdinal"];ui=life.ui(actor,row.get("connection"));body=ui["body"]
            need(ui["connection"]!=active_connection(admissions,actor,epoch) and exits[actor]["closeIndex"]<ui["index"]
                 and body.get("hp")==final[actor]["hp"] and body.get("mp")==final[actor]["mp"] and body.get("position")==final[actor]["position"]
                 and (body.get("realCharacterIndex"),body.get("nameHash"))==gear["identities"][actor]
                 and same_carried(carried(body.get("authoritativeItemState"),engine),states[actor]),"native_melee_fresh_HP_durability_stock_not_preserved")
            life.exit(actor,ui["connection"])
        need(type(disk)is dict and disk.get("schema")=="mir2.comparable.crystal-disk-readback.v1" and disk.get("originalLoaderUsed") is True
             and HEX.fullmatch(disk.get("accountFileSha256","")) is not None,"native_melee_original_disk_proof_missing")
        samples=disk.get("samples",[])
        need(disk.get("sampleCount")==plan["population"]==len(samples) and {s.get("actorOrdinal") for s in samples}==set(range(plan["population"])),"native_melee_all_actor_disk_readback_required")
        for s in samples:
            actor=s["actorOrdinal"]
            need(s.get("identityMatchesFixture") is True and s.get("mapIndex")==440 and s.get("position")==final[actor]["position"]
                 and s.get("hp")==final[actor]["hp"] and s.get("mp")==final[actor]["mp"] and s.get("level")==40
                 and same_carried(carried(s.get("itemState"),engine),states[actor]),"native_melee_disk_HP_durability_stock_mismatch")
    return {"valid":True,"equipmentStrongChecks":gear["count"],"normalFinalExits":len(exits),"freshSavedActors":selected,
            "saveScope":"last same-connection LogOutSuccess state; exact initial fresh UI pools and full carried state; all-actor stopped disk proof",
            "nativeImmediateStockIsAckDerived":engine=="crystal","nativeTouchedSaveHashesClosedByFreshAndDisk":engine=="crystal"}


def hash_source_artifact(path, maximum=256*2**20):
    path=Path(path)
    need(path.is_absolute() and (os.name!="nt" or path.drive.upper()=="C:"),"adapter_source_artifact_absolute_C_path_required")
    need(not path.is_symlink() and path.is_file() and all(not p.is_symlink() and not(getattr(p.lstat(),"st_file_attributes",0)&0x400) for p in [path,*path.parents]),"adapter_source_symlink_or_missing")
    before=path.stat();need(0<before.st_size<=maximum,"adapter_source_artifact_size_bound")
    sha=hashlib.sha256();size=0
    with path.open("rb") as stream:
        while chunk:=stream.read(2**20):
            size+=len(chunk);need(size<=maximum,"adapter_source_artifact_size_bound");sha.update(chunk)
    after=path.stat()
    need((before.st_size,before.st_mtime_ns,before.st_ino)==(after.st_size,after.st_mtime_ns,after.st_ino) and size==before.st_size,"adapter_source_artifact_changed")
    return {"bytes":size,"sha256":sha.hexdigest()}


def read_combat_sources(args, engine, metadata):
    need(args.adapter_sources.is_absolute() and (os.name!="nt" or args.adapter_sources.drive.upper()=="C:"),"adapter_source_manifest_absolute_C_path_required")
    manifest_fact=hash_source_artifact(args.adapter_sources,2**20)
    manifest,metadata["adapterSources"]=read_file(args.adapter_sources);root=args.adapter_sources.parent.resolve(strict=True)
    need(manifest_fact==metadata["adapterSources"],"adapter_source_manifest_changed_between_reads")
    result={"manifest":manifest,"verifiedFiles":[]}
    modern=engine=="modern";rows=manifest.get("files",[])
    need(type(rows)is list and rows and len(rows)<=(16 if modern else 64),"adapter_source_manifest_files_missing")
    for row in rows:
        name=row.get("path");need(type(name)is str and re.fullmatch(r"[A-Za-z0-9_.-]+",name) and name not in [".",".."],"adapter_source_relative_basename_required")
        path=root/name if modern else root/"source"/name
        fact=hash_source_artifact(path,(2 if modern else 32)*2**20)
        need(type(row.get("bytes"))is int and row.get("bytes")==fact["bytes"] and row.get("sha256")==fact["sha256"],"adapter_source_file_pin_mismatch")
        if modern:
            text=path.read_text(encoding="utf-8")
            for quote,relative in re.findall(r'''(?:\bfrom\s*|\bimport\s*\(?\s*)(['"])(\.[^'"]+)\1''',text):
                need(relative.startswith("./") and relative[2:] in {r.get("path") for r in rows},"adapter_static_import_graph_not_closed")
        result["verifiedFiles"].append({"path":name,**fact})
    need(len({r["path"].casefold() for r in result["verifiedFiles"]})==len(rows),"duplicate_adapter_source_file")
    if modern:
        ws=manifest.get("wsModule",{});path=Path(ws.get("path",""))
        need(path.is_absolute() and (os.name!="nt" or path.drive.upper()=="C:"),"adapter_ws_module_absolute_C_path_required")
        fact=hash_source_artifact(path,2*2**20);need(type(ws.get("bytes"))is int and ws.get("bytes")==fact["bytes"] and ws.get("sha256")==fact["sha256"],"adapter_ws_module_pin_mismatch")
        result["verifiedWsModule"]=fact
        need(args.runtime_receipt is not None,"melee_runtime_receipt_required")
        result["runtime"],metadata["runtimeReceipt"]=read_file(args.runtime_receipt)
        if args.build_manifest:
            result["buildManifest"],metadata["buildManifest"]=read_file(args.build_manifest)
            source_input=Path(result["buildManifest"].get("sourceInputManifest",""))
            need(source_input.is_absolute() and (os.name!="nt" or source_input.drive.upper()=="C:"),"overlay_source_input_manifest_absolute_C_path_required")
            fact=hash_source_artifact(source_input,4*2**20)
            need(fact["sha256"]==result["buildManifest"].get("sourceInputManifestSha256"),"overlay_source_input_manifest_hash_mismatch")
            result["verifiedBuildSourceInput"]=metadata["sourceInputManifest"]=fact
    else:
        need(args.native_build is not None and args.server_ready is not None,"native_build_and_ready_artifacts_required")
        result["build"],metadata["nativeBuild"]=read_file(args.native_build)
        result["ready"],metadata["serverReady"]=read_file(args.server_ready)
        result["verifiedAdapter"]=hash_source_artifact(Path(result["build"].get("adapter","")),32*2**20)
        result["verifiedRuntime"]={}
        for row in manifest.get("runtimeDlls",[]):
            name=row.get("name");need(name in ["CrystalReferenceServer.dll","Server.Library.dll","Shared.dll"],"native_unexpected_runtime_artifact")
            fact=hash_source_artifact(root/"frozen-runtime"/name,32*2**20)
            need(row.get("bytes")==fact["bytes"] and row.get("sha256")==fact["sha256"],"native_frozen_runtime_pin_mismatch")
            result["verifiedRuntime"][name]=fact
    return result


def verify_combat_sources(report, controller, metadata, artifacts, engine, *, melee=True):
    need(type(artifacts)is dict and type(artifacts.get("manifest"))is dict,"melee_adapter_source_artifacts_required")
    manifest=artifacts["manifest"];source_sha=metadata.get("adapterSources",{}).get("sha256")
    need(HEX.fullmatch(source_sha or "") is not None and artifacts.get("verifiedFiles")==manifest.get("files"),"adapter_source_files_not_independently_verified")
    if engine=="crystal":
        build=artifacts.get("build",{});runtime=artifacts.get("verifiedRuntime",{})
        need(manifest.get("schema")=="mir2.comparable.crystal-source-lock.v1" and manifest.get("gameSourceCompiled") is False
             and manifest.get("compileUsesFrozenSourceCopy") is True and report.get("sourceLockSha256")==source_sha,"native_combat_source_lock_unbound")
        need({"NativeActor.cs","NativeCombat.cs","PendingCombatAction.cs","ReactivePotions.cs","Runner.cs"}<={r["path"] for r in manifest["files"]},"native_combat_source_modules_missing")
        need(build.get("schema")=="mir2.comparable.crystal-build.v1" and build.get("exitCodes")==[0,0] and all(type(v)is int for v in build["exitCodes"])
             and build.get("sourceLockSha256")==source_sha and build.get("gameSourceCompiled") is False
             and build.get("sourceAndRuntimePostBuildHashCheckPassed") is True and build.get("adapterSha256")==report.get("adapterSha256")
             ==artifacts.get("verifiedAdapter",{}).get("sha256"),"native_combat_build_or_actual_adapter_pin_missing")
        need(set(runtime)=={"CrystalReferenceServer.dll","Server.Library.dll","Shared.dll"} and report.get("frozenSharedSha256")==runtime["Shared.dll"]["sha256"]
             and report.get("serverReadySha256")==metadata.get("serverReady",{}).get("sha256"),"native_combat_original_runtime_or_ready_unbound")
        return {"valid":True,"manifestSha256":source_sha,"sourceFileCount":len(manifest["files"]),"originalGameSourceCompiled":False,
                "adapterSha256":report["adapterSha256"],"nativePacketParserAndSaveSerializationIsTrustBoundary":True}
    required={"driver.mjs","contract.mjs"}|({"combat.mjs","melee-driver.mjs"} if melee else set())
    need(manifest.get("schema")=="mir2.comparable.modern-adapter-sources.v1" and manifest.get("entrypoint")==("melee-driver.mjs" if melee else "driver.mjs")
         and required<={r.get("path") for r in manifest["files"]},"modern_melee_actual_import_graph_missing")
    entry=next(r for r in manifest["files"] if r["path"]==manifest["entrypoint"]);runtime=artifacts.get("runtime",{})
    need(artifacts.get("verifiedWsModule")=={k:manifest.get("wsModule",{}).get(k) for k in ["bytes","sha256"]},"modern_actual_ws_module_not_verified")
    projected={k:manifest[k] for k in ["entrypoint","files","wsModule"]}
    projected["dependencyScope"]="static-esm-closure-plus-explicit-websocket-module"
    if type(report.get("adapterSource"))is dict and "schema" in report["adapterSource"]:
        projected["schema"]=manifest["schema"]
    need(report.get("adapterSourceManifestSha256")==controller.get("adapterSourceManifestSha256")==runtime.get("adapterSourceManifestSha256")==source_sha
         and report.get("adapterEntrypointSha256")==controller.get("driverEntrypointSha256")==runtime.get("adapterEntrypointSha256")==entry["sha256"]
         and report.get("adapterSource")==projected,"modern_melee_source_manifest_not_bound_to_actual_producers")
    proof=controller.get("adapterSourceProof",{})
    need(controller.get("adapterSourceBindingStable") is True and proof.get("actualCommandEntrypointMatched") is True
         and all(proof.get(k)==manifest.get(k) for k in ["schema","entrypoint","files","wsModule"])
         and controller.get("runtimeReceiptSha256")==metadata.get("runtimeReceipt",{}).get("sha256")
         and controller.get("serverBinarySha256")==runtime.get("binarySha256")==report.get("binarySha256")
         and HEX.fullmatch(report.get("binarySha256", "")) is not None,"modern_melee_controller_predeclared_source_or_binary_binding_missing")
    need(runtime.get("schema")=="mir2.comparable.modern-runtime.v1" and runtime.get("backend")=="file" and runtime.get("sessionCache")=="in-memory"
         and runtime.get("planSha256")==metadata.get("plan",{}).get("sha256") and runtime.get("sourceRevision")==report.get("sourceRevision"),"modern_melee_runtime_not_bound")
    if report.get("sourceRevisionIsBaseOnly") is True:
        build=artifacts.get("buildManifest",{});additional=report.get("additionalSource",{})
        schemas={"mir2.comparable.observer-timing-build.v1":("observer-timing-only",True,"base-plus-observer-timing-overlay"),
                 "mir2.comparable.spectator-scene-build.v1":("spectator-scene-projection",False,"base-plus-spectator-scene-projection")}
        need(build.get("schema") in schemas,"reviewed_overlay_build_manifest_required")
        kind,timing,proof_name=schemas[build["schema"]]
        declared_timing=build.get("observerTimingOnly",timing)
        need(type(declared_timing)is bool and declared_timing is timing,"reviewed_overlay_explicit_timing_flag_conflict")
        if "observerTimingOnly" not in build or "sourceInputManifest" in build:
            need(build.get("cleanBaseBuild") is False and artifacts.get("verifiedBuildSourceInput")==metadata.get("sourceInputManifest")
                 and type(metadata.get("sourceInputManifest"))is dict and metadata["sourceInputManifest"].get("sha256")==build.get("sourceInputManifestSha256"),"reviewed_overlay_source_inputs_not_actually_verified")
        need(build.get("changesKind")==kind and type(build.get("buildActualExitCode"))is int and build.get("buildActualExitCode")==0
             and build.get("baseRevision")==report["sourceRevision"]=="31e9b6a74661c31d6515dd7322b466285f975d71"
             and build.get("binarySha256")==report["binarySha256"] and report.get("sourceProof")==proof_name
             and additional.get("buildManifestSchema")==build["schema"] and additional.get("buildManifestSha256")==metadata.get("buildManifest",{}).get("sha256")
             ==controller.get("buildManifestSha256") and additional.get("cleanBaseBuild") is False
             and additional.get("behaviorFixClaimed") is False and additional.get("capacityClaimAllowed") is False
             and HEX.fullmatch(build.get("sourceInputManifestSha256", "")) is not None
             and runtime.get("buildManifestSha256")==metadata.get("buildManifest",{}).get("sha256")
             and all(runtime.get(k)==build.get(k) for k in ["changesKind","sourceInputManifestSha256","buildActualExitCode"])
             and additional.get("observerTimingOnly") is timing
             and all(additional.get(k)==build.get(k) for k in ["changesKind","baseRevision","binarySha256","sourceInputManifestSha256","buildActualExitCode"]),"modern_overlay_cannot_be_claimed_as_clean_base")
    else:
        need(report.get("sourceProof")=="owned-controller-receipt" and report.get("additionalSource") in [None,{}]
             and not any(runtime.get(k) is not None for k in ["changesKind","sourceInputManifestSha256","buildManifestSchema","additionalSource"]),"modern_overlay_declaration_missing_or_ambiguous")
    return {"valid":True,"manifestSha256":source_sha,"entrypointSha256":entry["sha256"],"sourceFileCount":len(manifest["files"]),
            "controllerLaunchAndDriverEndBindingStable":True,"sourceRevisionIsBaseOnly":report.get("sourceRevisionIsBaseOnly") is True}


if __name__ == "__main__":
    raise SystemExit(main())

