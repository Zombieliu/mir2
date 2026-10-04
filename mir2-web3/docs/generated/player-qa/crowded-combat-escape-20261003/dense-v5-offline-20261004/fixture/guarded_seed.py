"""Prepared-state fixture only. Never invoke against a running or live realm.

There is deliberately no seed CLI: the owned-process wrapper passes an in-memory
Popen guard after it has observed ordinary logout and a normal SIGTERM exit 0.
Pure transform tests do not open a Gateway or a real account store.
"""
import copy
import hashlib
import json
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parent
SOURCE = "6032ef8b3e27dd97bad0b20c8676ef9f185db64b"
ELF_SHA = "1ed52738885292b734c4055a9fd7d585b79d519779084bca2a617d0a9f5eb9ab"
PROFILE_SHA = "328ecec1474f9bde809b6a57508cb9dd9d3b0be82c136fb4dabcd8bba85b66cd"
BASE_VITALS = {"Warrior": (419, 116), "Wizard": (128, 541), "Taoist": (239, 260)}
MAX_FUNDS = 20_000


def sha_bytes(value):
    return hashlib.sha256(value).hexdigest()


def pointer(value):
    return str(value).replace("~", "~0").replace("/", "~1")


def diffs(before, after, prefix=""):
    if type(before) is not type(after):
        return [{"pointer": prefix, "old": before, "new": after}]
    if isinstance(before, dict):
        if set(before) != set(after):
            raise ValueError("Offline seeding may not add/remove schema fields")
        return [row for key in before for row in diffs(before[key], after[key], prefix + "/" + pointer(key))]
    if isinstance(before, list):
        if len(before) != len(after):
            raise ValueError("Offline seeding may not add/remove records or item UIDs")
        return [row for i, (left, right) in enumerate(zip(before, after)) for row in diffs(left, right, prefix + "/" + str(i))]
    return [] if before == after else [{"pointer": prefix, "old": before, "new": after}]


def validate_cohort(store, cohort):
    if store.get("schemaVersion") != 6 or not isinstance(store.get("accounts"), dict):
        raise ValueError("Only a normally written schema 6 File store is accepted")
    if len(cohort) != 3 or {a["className"] for a in cohort} != set(BASE_VITALS):
        raise ValueError("Exactly three fresh declared class accounts are required")
    ids = [a["accountId"] for a in cohort]
    if len(set(ids)) != 3 or any(not str(a).startswith("dn") for a in ids):
        raise ValueError("Fixture account namespace/uniqueness guard failed")
    # The server's built-in demo record may exist, but must remain untouched.
    if set(store["accounts"]) - set(ids) - {"demo"}:
        raise ValueError("Unexpected non-fixture accounts: refuse shared/live input")
    for account in cohort:
        record = store["accounts"].get(account["accountId"])
        if not isinstance(record, dict) or record.get("gm_level", 0) != 0:
            raise ValueError("Normal non-GM fixture account required")
        chars = record.get("characters", [])
        if len(chars) != 1 or chars[0].get("class") != account["className"] or chars[0].get("name") != account["name"]:
            raise ValueError("Fresh character identity/class mismatch")
        index = str(chars[0].get("index"))
        saves = record.get("saves", {})
        if set(saves) != {index} or saves[index].get("character") != chars[0]:
            raise ValueError("Character list and durable save disagree")
        save = saves[index]
        if not isinstance(save.get("revision"), int) or save["revision"] < 0:
            raise ValueError("Save revision is not coherent")
        if not isinstance(save.get("bind_point"), dict):
            raise ValueError("A normally established town bind point is required")
        seen = set()
        for field in ("inventory_items_json", "belt_items_json", "equipment_items_json"):
            uid_field = "user_item_unique_id" if field == "equipment_items_json" else "unique_id"
            for raw in save.get(field, []):
                item = json.loads(raw)
                uid = item.get(uid_field)
                if type(uid) is not int or not 0 <= uid <= 2**64 - 1:
                    raise ValueError("Missing or invalid u64 item UID in fixture collection: " + field)
                if uid in seen:
                    raise ValueError("Duplicate item UID across fixture inventory/belt/equipment")
                seen.add(uid)
    return True


def transform_store(before, cohort, mode, profile, staging=None, gold=MAX_FUNDS):
    validate_cohort(before, cohort)
    if profile.get("profileId") != "platinum_176" or profile.get("version") != 26:
        raise ValueError("Pinned original profile required")
    after = copy.deepcopy(before)
    allowed = set()
    exp = next(row["requiredExperience"] for row in profile["experienceCurve"] if row["level"] == 30)
    if mode not in {"level", "travel"}:
        raise ValueError("Unknown bounded seed mode")
    if mode == "level" and (type(gold) is not int or gold < 0 or gold > MAX_FUNDS):
        raise ValueError("Gold must be an explicit bounded allowance <= 20000")
    if mode == "travel":
        if not staging or staging.get("map") != "D021" or staging.get("title") != "WoomaTempleEntrance" or staging.get("staticWalkable") is not True or staging.get("sourceValidated") is not True:
            raise ValueError("Source-validated, static-walkable D021 staging is required")
        if type(staging.get("x")) is not int or type(staging.get("y")) is not int:
            raise ValueError("Integer staging coordinate required")
    for account in cohort:
        key = account["accountId"]
        record = after["accounts"][key]
        character = record["characters"][0]
        index = str(character["index"])
        save = record["saves"][index]
        base = "/accounts/" + pointer(key)
        save_base = base + "/saves/" + pointer(index)
        if mode == "level":
            if character.get("level") != 1 or save.get("skill_states_json") or save.get("buff_states_json") or save.get("hp", 0) <= 0:
                raise ValueError("Level preparation must start from living fresh level 1, without learned spells or buffs")
            hp, mp = BASE_VITALS[account["className"]]
            character["level"] = 30
            save["character"]["level"] = 30
            save.update(hp=hp, max_hp=hp, mp=mp, max_mp=mp, experience=0, max_experience=exp, gold=gold)
            allowed.update({base + "/characters/0/level", save_base + "/character/level"})
            allowed.update(save_base + "/" + k for k in ("hp", "max_hp", "mp", "max_mp", "experience", "max_experience", "gold"))
        else:
            if character.get("level") != 30 or save.get("hp", 0) <= 0:
                raise ValueError("Prepared travel may not revive or change progression")
            save.update(map_file_name="D021", map_title="WoomaTempleEntrance", position={"x": staging["x"], "y": staging["y"]}, direction="Down")
            allowed.update(save_base + "/" + k for k in ("map_file_name", "map_title", "position/x", "position/y", "direction"))
        save["revision"] += 1
        allowed.add(save_base + "/revision")
    changes = diffs(before, after)
    if any(row["pointer"] not in allowed for row in changes):
        raise ValueError("Forbidden field modified by offline transform")
    validate_cohort(after, cohort)
    return after, {"schema": "mir2.prepared-state-field-diff.v1", "mode": mode, "preparedState": True, "ordinaryProgressionAccepted": False, "sourceRevision": SOURCE, "level": 30, "boundedGoldAllowance": gold if mode == "level" else None, "diffs": changes, "schemaPreserved": True, "credentialsItemsBindSkillsBuffsPreserved": True, "fullP1Accepted": False, "nativeInputAccepted": False, "nativeVisualAccepted": False, "loadAccepted": False, "humanAccepted": False}


class StoppedOwnedRealm:
    def __init__(self, process, store_path, private_root, binary_sha):
        if not isinstance(process, subprocess.Popen) or binary_sha != ELF_SHA:
            raise ValueError("Exact owned-process capability required")
        self.process = process
        self.store_path = Path(store_path)
        self.private_root = Path(private_root)
        self.binary_sha = binary_sha

    def check(self):
        if self.process.poll() != 0:
            raise ValueError("Owned Gateway must have exited normally with code 0 before seeding")
        private = self.private_root.resolve(strict=True)
        store = self.store_path.resolve(strict=True)
        if not private.is_relative_to(ROOT) or not store.is_relative_to(private) or store.name != "accounts.json":
            raise ValueError("Store path escaped new private fixture directory")
        for candidate in (self.store_path, *self.store_path.parents, self.private_root, *self.private_root.parents):
            if candidate.is_symlink() or (hasattr(candidate, "is_junction") and candidate.is_junction()):
                raise ValueError("Symlink/junction ancestor of a private store is not accepted")
        if store.stat().st_nlink != 1:
            raise ValueError("Hardlinked private store is not accepted")
        return store


def seed_stopped_realm(guard, expected_sha, cohort, mode, staging=None, gold=MAX_FUNDS):
    store = guard.check()
    raw = store.read_bytes()
    if sha_bytes(raw) != expected_sha:
        raise ValueError("Store changed since owned normal shutdown; refuse stale seed")
    before = json.loads(raw.decode("utf-8-sig"))
    profile_bytes = (ROOT / "platinum_176.json").read_bytes()
    if sha_bytes(profile_bytes) != PROFILE_SHA:
        raise ValueError("Pinned source profile SHA changed; offline preparation prohibited")
    profile = json.loads(profile_bytes.decode("utf-8-sig"))
    after, receipt = transform_store(before, cohort, mode, profile, staging, gold)
    encoded = (json.dumps(after, ensure_ascii=False, indent=2) + "\n").encode("utf-8")
    # Retain exact pre-edit input privately. Neither it nor account credentials
    # are ever copied into the public proof package.
    baseline = store.parent / ("before-" + mode + ".private.json")
    with baseline.open("xb") as handle:
        handle.write(raw)
    os.chmod(baseline, 0o600)
    temporary = store.with_name("accounts.seed.tmp")
    with temporary.open("xb") as handle:
        handle.write(encoded)
        handle.flush()
        os.fsync(handle.fileno())
    guard.check()
    if sha_bytes(store.read_bytes()) != expected_sha:
        temporary.unlink()
        raise ValueError("Concurrent store change detected before replacement")
    os.replace(temporary, store)
    os.chmod(store, 0o600)
    receipt.update(beforeStoreSha256=expected_sha, afterStoreSha256=sha_bytes(encoded), ownedGatewayExitCode=0)
    return receipt
