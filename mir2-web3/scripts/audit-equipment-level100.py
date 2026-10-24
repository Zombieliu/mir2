#!/usr/bin/env python3
"""Read-only equipment/packed-resource audit; never changes game configuration.

The output is evidence and a source-candidate allocation, not playable content.
Run from any directory: python scripts/audit-equipment-level100.py
"""
from __future__ import annotations

import argparse
from collections import Counter, defaultdict
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import struct

PROJECT = Path(__file__).resolve().parents[1]
WORKSPACE = PROJECT.parent
PUBLIC = PROJECT / "apps/web/public"
CLASSES = {"Warrior": 1, "Wizard": 2, "Taoist": 4, "Assassin": 8, "Archer": 16}
GENDERS = {"Male": 1, "Female": 2}
SLOTS = {1: "weapon", 2: "armour", 4: "helmet", 5: "necklace", 6: "bracelet", 7: "ring", 9: "belt", 10: "boots"}
# Crystal/Client/MirObjects/Frames.cs:157-197. Source, not inferred sheet length.
COMMON = {"Standing": (0, 4, 0), "Walking": (32, 6, 0), "Running": (80, 6, 0),
          "Attack1": (136, 6, 0), "Attack2": (184, 6, 0), "Attack3": (232, 8, 0),
          "Spell": (296, 6, 0), "Struck": (360, 3, 0), "Die": (384, 4, 0), "Dead": (387, 1, 3)}
ASSASSIN = {**COMMON, "Attack4": (416, 6, 0), "Sneak": (464, 6, 0), "Dash": (80, 3, 3)}
BOW = {"WalkingBow": (0, 6, 0), "RunningBow": (48, 6, 0), "AttackRange1": (96, 8, 0),
       "AttackRange2": (160, 8, 0), "AttackRange3": (224, 8, 0), "Jump": (288, 8, 0)}
BOW_NORMAL = {k: COMMON[k] for k in ("Standing", "Spell", "Struck", "Die", "Dead")}
# PlayerObject's altAnim switch excludes AttackRange3. It uses the base bow
# library with the common gender block, despite sharing range frame numbers.
BOW_NORMAL["AttackRange3"] = BOW["AttackRange3"]
ASSASSIN_DEAD_COMMON = {"DeadCommonOffset": COMMON["Dead"]}
PINS: dict[Path, str] = {}


def digest(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def read_bytes(path: Path) -> bytes:
    raw = path.read_bytes()
    observed = digest(raw)
    if path in PINS and PINS[path] != observed:
        raise RuntimeError(f"Input changed during audit: {path.name}")
    PINS[path] = observed
    return raw


def read_json(path: Path):
    return json.loads(read_bytes(path))


def public_path(url: str) -> Path:
    if not isinstance(url, str) or not url.startswith("/") or ":" in url or "\\" in url or ".." in url.split("/"):
        raise ValueError(f"Nonlocal resource URL: {url!r}")
    # Full resource storage can be an existing directory junction. Validate the
    # URL's lexical root; reading that configured junction is intentional.
    path = (PUBLIC / url.lstrip("/")).absolute()
    if not path.is_relative_to(PUBLIC.absolute()):
        raise ValueError("Resource escaped public root")
    return path


def resolve_item(items: list[dict], origin: dict, level: int, class_flag: int) -> dict:
    """Crystal GetRealItem's exact prefix/gender and tie-breaking semantics."""
    if not origin["class_based"] and not origin["level_based"]:
        return origin
    matching = [i for i in items if i["name"].startswith(origin["name"])
                and i["required_gender"] == origin["required_gender"]]
    if origin["class_based"] and not origin["level_based"]:
        return next((i for i in matching if i["required_class"] == class_flag), origin)
    result = origin
    for item in matching:
        if item["required_type"] != 0 or item["required_amount"] > level:
            continue
        if origin["class_based"]:
            if item["required_class"] == class_flag and result["required_amount"] <= item["required_amount"]:
                result = item
        elif result["required_amount"] < item["required_amount"]:
            result = item
    return result


def motion_specs(item: dict, class_name: str, female: bool) -> list[tuple]:
    shape = item["shape"]
    if item["item_type"] == 2:
        result = [(f"CArmour/{shape:02}", 808 if female else 0, COMMON)]
        if class_name == "Assassin":
            result.append((f"AArmour/{shape:02}", 512 if female else 0, ASSASSIN))
        elif class_name == "Archer":
            result.append((f"ARArmour/{shape:02}", 352 if female else 0, BOW))
        return result
    if item["item_type"] != 1:
        return []
    if class_name == "Assassin" and 100 <= shape < 200:
        result = [(f"AWeapon/{shape - 100:02} {side}", 512 if female else 0, ASSASSIN)
                  for side in ("L", "R")]
        # Dead drops altAnim. Male frame IDs are already covered by Die;
        # explicitly check the female common +416 block in the same L/R files.
        if female:
            result.extend((f"AWeapon/{shape - 100:02} {side}", 416, ASSASSIN_DEAD_COMMON) for side in ("L", "R"))
        return result
    if class_name == "Archer" and shape >= 200:
        return [(f"ARWeapon/{shape - 200:02}", 416 if female else 0, BOW_NORMAL),
                (f"ARWeapon/{shape - 200:02} S", 352 if female else 0, BOW)]
    return [(f"CWeapon/{shape:02}", 416 if female else 0, COMMON)]


class Assets:
    def __init__(self, index: dict):
        self.libraries = {x["key"].lower(): x for x in index["libraries"]}
        self.loaded = {}
        self.pages = {}
        self.frames = {}
        self.motions = {}
        self.manifest_errors = {}

    def library(self, key: str):
        key = key.lower()
        if key in self.loaded:
            return self.loaded[key]
        entry = self.libraries.get(key)
        if not entry:
            self.loaded[key] = None
            return None
        path = public_path(entry["manifestUrl"])
        if not path.is_file():
            self.manifest_errors[key] = "manifest_missing"
            self.loaded[key] = None
            return None
        raw = read_bytes(path)
        if digest(raw) != entry["manifestSha256"]:
            self.manifest_errors[key] = "manifest_hash_mismatch"
        data = json.loads(raw)
        data["_frames"] = {f["index"]: f for f in data["frames"]}
        self.loaded[key] = data
        return data

    def page(self, url: str) -> dict:
        if url in self.pages:
            return self.pages[url]
        path = public_path(url)
        result = {"url": url, "status": "png_missing"}
        if path.is_file():
            raw = read_bytes(path)
            if len(raw) < 33 or raw[:8] != b"\x89PNG\r\n\x1a\n" or raw[12:16] != b"IHDR":
                result["status"] = "png_invalid_header"
            else:
                width, height = struct.unpack(">II", raw[16:24])
                result.update(width=width, height=height, sha256=digest(raw), bytes=len(raw))
                result["status"] = "ok" if result["sha256"] == path.stem else "png_hash_mismatch"
        self.pages[url] = result
        return result

    def frame(self, library: str, index: int) -> dict:
        cache_key = (library.lower(), index)
        if cache_key in self.frames:
            return self.frames[cache_key]
        data = self.library(library)
        result = {"library": library, "index": index}
        if data is None:
            result["status"] = self.manifest_errors.get(library.lower(), "library_missing")
        elif library.lower() in self.manifest_errors:
            result["status"] = self.manifest_errors[library.lower()]
        elif index not in data["_frames"]:
            result["status"] = "frame_out_of_range" if index < 0 or index >= data["frameSlotCount"] else "frame_record_missing"
        else:
            frame = data["_frames"][index]
            if frame.get("noDraw") or frame.get("status") == "noDraw":
                result["status"] = "source_no_draw"
            else:
                image = frame.get("image") or {}
                url = image.get("imageUrl") or frame.get("imageUrl")
                if not url:
                    result["status"] = "frame_image_missing"
                else:
                    page = self.page(url)
                    result["status"] = page["status"]
                    x, y, w, h = (image.get(k) for k in ("x", "y", "width", "height"))
                    if result["status"] == "ok" and not (
                        all(isinstance(v, int) for v in (x, y, w, h)) and x >= 0 and y >= 0
                        and w > 0 and h > 0 and x + w <= page["width"] and y + h <= page["height"]
                        and w == frame["width"] and h == frame["height"]
                    ):
                        result["status"] = "invalid_frame_rectangle"
                    result.update(imageUrl=url, rect={"x": x, "y": y, "width": w, "height": h})
                    mask = frame.get("mask")
                    if mask and mask.get("imageUrl"):
                        if self.page(mask["imageUrl"])["status"] != "ok":
                            result["status"] = "mask_page_invalid"
        self.frames[cache_key] = result
        return result

    def motion(self, library: str, offset: int, actions: dict) -> str:
        key = f"{library}|{offset}|{','.join(actions)}"
        if key in self.motions:
            return key
        checks, issues = [], []
        counts = Counter()
        for action, (start, count, skip) in actions.items():
            for direction in range(8):
                status = Counter()
                for phase in range(count):
                    frame_id = offset + start + (count + skip) * direction + phase
                    frame = self.frame(library, frame_id)
                    status[frame["status"]] += 1
                    counts[frame["status"]] += 1
                    if frame["status"] not in ("ok", "source_no_draw") and len(issues) < 24:
                        issues.append({"action": action, "direction": direction, "phase": phase,
                                       "index": frame_id, "status": frame["status"]})
                checks.append({"action": action, "direction": direction, "counts": dict(status)})
        hard_errors = sum(v for k, v in counts.items() if k not in ("ok", "source_no_draw"))
        self.motions[key] = {"library": library, "genderOffset": offset, "counts": dict(counts),
                             "referencesPresent": hard_errors == 0,
                             "drawableEveryActionDirection": all(c["counts"].get("ok", 0) > 0 for c in checks),
                             "actions": checks, "issues": issues}
        return key


def inventory(items: list[dict], whitelist: set[str], assets: Assets) -> list[dict]:
    rows = []
    for item in items:
        if item["item_type"] not in SLOTS:
            continue
        icon = assets.frame("Items", item["image"])
        ground = assets.frame("DNItems", item["image"])
        bindings = []
        for class_name, class_flag in CLASSES.items():
            if not item["required_class"] & class_flag:
                continue
            effective = resolve_item(items, item, 100, class_flag)
            for gender, flag in GENDERS.items():
                if not item["required_gender"] & flag:
                    continue
                motion_keys = [assets.motion(*s) for s in motion_specs(effective, class_name, gender == "Female")]
                # CharacterDialog resolves armour/weapon, but not helmet, before StateItem drawing.
                paper = assets.frame("StateItem", (item if item["item_type"] == 4 else effective)["image"]) if item["item_type"] in (1, 2, 4) else None
                motion_ok = all(assets.motions[k]["referencesPresent"] and assets.motions[k]["drawableEveryActionDirection"] for k in motion_keys)
                ready = icon["status"] == ground["status"] == "ok" and (paper is None or paper["status"] == "ok") and motion_ok
                bindings.append({"class": class_name, "gender": gender, "effectiveItemIndexAt100": effective["item_index"],
                                 "effectiveNameAt100": effective["name"], "effectiveShapeAt100": effective["shape"],
                                 "paperdoll": paper, "motionKeys": motion_keys, "resourceReady": ready})
        rows.append({"itemIndex": item["item_index"], "name": item["name"], "slot": SLOTS[item["item_type"]],
                     "itemType": item["item_type"], "requiredType": item["required_type"], "requiredAmount": item["required_amount"],
                     "requiredClass": item["required_class"], "requiredGender": item["required_gender"], "grade": item["grade"],
                     "shape": item["shape"], "image": item["image"], "classBased": item["class_based"], "levelBased": item["level_based"],
                     "stats": {str(s["stat"]): s["value"] for s in item["stats"]},
                     "nameHasReplacementCharacter": "\ufffd" in item["name"],
                     "defaultProfileAllowed": item["name"].lower() in whitelist, "icon": icon, "ground": ground,
                     "bindings": bindings, "allBindingsResourceReady": bool(bindings) and all(b["resourceReady"] for b in bindings)})
    return rows


def appearance_key(row: dict) -> tuple:
    """Conservative identifier reuse; different identifiers do not prove new art."""
    # Weapon/armour image IDs can change while the in-world library stays the
    # same. Never count that as a new world appearance. Other slots have icons
    # only; ID novelty is recorded without claiming pixel uniqueness.
    return ("world_shape", row["shape"]) if row["slot"] in ("weapon", "armour") else ("icon_id", row["image"])


def armour_standing_sample(shape: int, class_name: str, female: bool, weapon_shape: int) -> tuple[str, int]:
    """PlayerObject Standing: Assassin dual blades use the alternate body."""
    alternate = class_name == "Assassin" and (weapon_shape < 0 or 100 <= weapon_shape < 200)
    family = "AArmour" if alternate else "CArmour"
    offset = (512 if alternate else 808) if female else 0
    return f"{family}/{shape:02}", offset + 4 * 4


def allocate(rows: list[dict]) -> list[dict]:
    """Planning allocation only: new 71-100 data is not written into the game."""
    plan = []
    for class_name in CLASSES:
        for gender in GENDERS:
            used = defaultdict(set)
            primary_stat = {"Warrior": "5", "Wizard": "7", "Taoist": "9", "Assassin": "5", "Archer": "5"}[class_name]
            for target in (1, 10, 20, 30, 40, 50, 60, 70, 80, 90, 100):
                desired = {50: 43, 60: 50, 70: 55, 80: 60, 90: 65, 100: 70}.get(target, target)
                for slot in SLOTS.values():
                    if target == 1 and slot not in ("weapon", "armour"):
                        continue
                    pool = []
                    for row in rows:
                        if row["slot"] != slot or row["requiredType"] != 0 or row["nameHasReplacementCharacter"]:
                            continue
                        if (gender == "Male" and "(F)" in row["name"]) or (gender == "Female" and "(M)" in row["name"]):
                            continue
                        if row["requiredAmount"] > desired:
                            continue
                        binding = next((b for b in row["bindings"] if b["class"] == class_name and b["gender"] == gender and b["resourceReady"]), None)
                        if binding is None:
                            continue
                        # Select the concrete resolved variant. Cloned future
                        # templates must not retain a dynamic family switch.
                        if binding["effectiveItemIndexAt100"] != row["itemIndex"]:
                            continue
                        if slot == "weapon" and target >= 20 and row["stats"].get(primary_stat, 0) <= 0:
                            continue
                        signature = appearance_key(row)
                        class_weapon_penalty = int(slot == "weapon" and ((class_name == "Assassin" and not 100 <= row["shape"] < 200) or (class_name == "Archer" and row["shape"] < 200)))
                        source_band_penalty = int(target >= 50 and row["requiredAmount"] < max(33, desired - 15))
                        combat_role_penalty = int(slot == "weapon" and class_name == "Warrior" and (row["stats"].get("7", 0) > 0 or row["stats"].get("9", 0) > 0))
                        # Class/role and source tier come before novel IDs:
                        # do not fill a level-100 kit with low-level underwear
                        # or a caster's weapon merely to avoid visual reuse.
                        exact_class_penalty = int(row["requiredClass"] != CLASSES[class_name])
                        score = (class_weapon_penalty, source_band_penalty, combat_role_penalty,
                                 int(signature in used[slot]), abs(row["requiredAmount"] - desired),
                                 exact_class_penalty, row["itemIndex"])
                        pool.append((score, row, signature))
                    if not pool:
                        plan.append({"class": class_name, "gender": gender, "targetLevel": target, "slot": slot,
                                     "status": "no_verified_candidate"})
                        continue
                    _, chosen, signature = min(pool, key=lambda x: x[0])
                    reused = signature in used[slot]
                    used[slot].add(signature)
                    plan.append({"class": class_name, "gender": gender, "targetLevel": target, "slot": slot,
                                 "status": ("reuse_existing_world_shape" if signature[0] == "world_shape" else "reuse_existing_icon_id") if reused else "verified_source_candidate",
                                 "sourceItemIndex": chosen["itemIndex"], "sourceName": chosen["name"],
                                 "sourceRequiredLevel": chosen["requiredAmount"], "sourceShape": chosen["shape"],
                                 "sourceImage": chosen["image"], "requiresNewTemplate": target != chosen["requiredAmount"],
                                 "appearanceKey": list(signature), "pixelUniquenessProven": False,
                                 "icon": chosen["icon"]})
    return plan


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, default=PROJECT / "docs/generated/equipment-level100-20261019")
    args = parser.parse_args()
    out = args.output_dir.resolve()
    if not out.is_relative_to((PROJECT / "docs/generated").resolve()):
        raise SystemExit("Output must remain inside this project's docs/generated directory")
    source_dir = PROJECT / "packages/game-data/data/generated"
    items = read_json(source_dir / "crystal_item_manifest.json")["items"]
    profile = read_json(PROJECT / "packages/game-data/data/content_profiles/platinum_176.json")
    index = read_json(PUBLIC / "generated/crystal-packs/full/index.json")
    for relative in ("scripts/audit-equipment-level100.py", "packages/game-data/src/lib.rs", "packages/protocol/src/types.rs",
                     "apps/simulation/src/runtime/leveling.rs", "apps/simulation/src/config.rs",
                     "../Crystal/Client/MirObjects/Frames.cs", "../Crystal/Client/MirObjects/PlayerObject.cs",
                     "../Crystal/Client/MirScenes/Dialogs/CharacterDialog.cs", "../Crystal/Shared/Functions/Functions.cs"):
        read_bytes((PROJECT / relative).resolve())
    assets = Assets(index)
    rows = inventory(items, {n.lower() for n in profile["itemWhitelist"]}, assets)
    plan = allocate(rows)
    # Include the entire relevant library reserve, even libraries unused by current templates.
    family_counts = Counter()
    template_referenced_libraries = {m["library"].lower() for m in assets.motions.values()}
    reserve = []
    for entry in index["libraries"]:
        family = entry["key"].split("/")[0]
        if family not in {"CArmour", "CWeapon", "AArmour", "AWeapon", "ARArmour", "ARWeapon"}:
            continue
        family_counts[family] += 1
        data = assets.library(entry["key"])
        if data:
            for page in data["pages"]:
                assets.page(page["imageUrl"])
        if family in ("CArmour", "CWeapon"):
            actions, female_offset = COMMON, 808 if family == "CArmour" else 416
        elif family in ("AArmour", "AWeapon"):
            actions, female_offset = ASSASSIN, 512
        elif family == "ARArmour" or entry["key"].endswith(" S"):
            actions, female_offset = BOW, 352
        else:
            actions, female_offset = BOW_NORMAL, 416
        keys = [assets.motion(entry["key"], offset, actions) for offset in (0, female_offset)]
        if family == "AWeapon":
            keys.append(assets.motion(entry["key"], 416, ASSASSIN_DEAD_COMMON))
        reserve.append({"library": entry["key"], "family": family, "frameSlots": entry["frameSlotCount"],
                        "referencedByCurrentTemplates": entry["key"].lower() in template_referenced_libraries,
                        "motionKeysMaleFemale": keys[:2], "supplementalMotionKeys": keys[2:],
                        "resourceReadyMaleFemale": [assets.motions[k]["referencesPresent"] and assets.motions[k]["drawableEveryActionDirection"] for k in keys[:2]],
                        "allResourceReferencesReady": all(assets.motions[k]["referencesPresent"] and assets.motions[k]["drawableEveryActionDirection"] for k in keys)})
    drops = read_json(source_dir / "crystal_drop_manifest.json")
    drop_refs = defaultdict(set)
    def visit_drop(entry: dict, table_key: str) -> None:
        name = entry.get("item_name", "").lower()
        if name:
            drop_refs[name].add(table_key)
        for child in (entry.get("group") or {}).get("entries", []):
            visit_drop(child, table_key)
    for table in drops["tables"]:
        for section in table["sections"]:
            for entry in section["entries"]:
                visit_drop(entry, table["table_key"])
    for row in rows:
        row["rawDropTableReferences"] = sorted(drop_refs[row["name"].lower()])
    bins = []
    for low, high in ((1, 40), (41, 50), (51, 60), (61, 70), (71, 80), (81, 90), (91, 100)):
        selected = [r for r in rows if r["requiredType"] == 0 and low <= r["requiredAmount"] <= high]
        bins.append({"minLevel": low, "maxLevel": high, "templates": len(selected),
                     "allBindingsResourceReady": sum(r["allBindingsResourceReady"] for r in selected),
                     "bySlot": dict(Counter(r["slot"] for r in selected))})
    readiness = []
    for cls in CLASSES:
        for gender in GENDERS:
            for slot in SLOTS.values():
                candidates = [(r, b) for r in rows if r["slot"] == slot for b in r["bindings"] if b["class"] == cls and b["gender"] == gender]
                good = [(r, b) for r, b in candidates if b["resourceReady"]]
                readiness.append({"class": cls, "gender": gender, "slot": slot, "templates": len(candidates),
                                  "resourceReady": len(good), "rawImageIds": len({r["image"] for r, b in good}),
                                  "rawShapeIds": len({r["shape"] for r, b in good}) if slot in ("weapon", "armour") else None,
                                  "effectiveShapeIdsAt100": len({b["effectiveShapeAt100"] for r, b in good}) if slot in ("weapon", "armour") else None})
    motion_issues = {k: v for k, v in assets.motions.items() if not v["referencesPresent"] or not v["drawableEveryActionDirection"]}
    summary = {"schema": "mir2-equipment-level100-audit/1", "generatedAt": datetime.now(timezone.utc).isoformat(),
               "scope": "Current local imported templates and packed resources; source-based availability, not gameplay acceptance.",
               "productDecision": {"classes": list(CLASSES), "maxLevel": 100, "slgUnlockLevel": 100, "implementationStatus": "planning_only"},
               "totalImportedItems": len(items), "coreEquipmentTemplates": len(rows), "levelBins": bins,
               "attributeRequiredTemplates": sum(r["requiredType"] != 0 for r in rows),
               "defaultProfile": {"id": profile["profileId"], "acceptanceLevel": profile["acceptanceLevel"],
                                  "allowedClasses": profile["allowedClasses"], "equipmentTemplates": sum(r["defaultProfileAllowed"] for r in rows)},
               "resourceLibraryFamilies": dict(family_counts), "fullPackLibraryCount": len(index["libraries"]),
               "manifestCountChecked": sum(v is not None for v in assets.loaded.values()),
               "manifestErrors": assets.manifest_errors, "pngPagesChecked": len(assets.pages),
               "pngBytesChecked": sum(p.get("bytes", 0) for p in assets.pages.values()),
               "pngPageStatuses": dict(Counter(p["status"] for p in assets.pages.values())),
               "uniqueFrameReferences": len(assets.frames), "frameStatuses": dict(Counter(f["status"] for f in assets.frames.values())),
               "motionBindingsChecked": len(assets.motions), "motionBindingsWithGaps": len(motion_issues),
               "reserveLibrariesNotReferencedByCurrentTemplates": sum(not r["referencedByCurrentTemplates"] for r in reserve),
               "reserveLibrariesWithMotionGaps": sum(not r["allResourceReferencesReady"] for r in reserve),
               "allBindingsResourceReadyTemplates": sum(r["allBindingsResourceReady"] for r in rows),
               "templatesWithNameEncodingIssue": sum(r["nameHasReplacementCharacter"] for r in rows),
               "templatesWithRawDropTableReference": sum(bool(r["rawDropTableReferences"]) for r in rows),
               "auxiliaryWearableCountsNotInCoreSlots": {str(t): sum(i["item_type"] == t for i in items) for t in (8, 11, 12)},
               "candidateAllocationStatuses": dict(Counter(p["status"] for p in plan)),
               "candidateAllocationCount": len(plan), "readinessByClassGenderSlot": readiness,
               "limits": ["A template count includes class/level variants and is not a count of unique art sets.",
                          "Source noDraw is recorded separately from missing/corrupt assets.",
                          "Candidate allocations reuse source pixels; no item requirements, stats, drops, saves or production switches were changed.",
                          "Allocation statuses track identifier reuse within each class/gender/slot progression, not unique pixels or unique full sets; shared art across classes remains possible.",
                          "Slot planning represents one design per bracelet/ring type, not both worn copies.",
                          "Raw drop-table references do not prove live spawning or ordinary attainability; dynamic class/level variants may be reached through origin templates.",
                          "Only equipment and core/special unmounted actions were audited; riding, fishing, hair, wings, effects, map, monster and skill completeness remain separate.",
                          "At100 effective variants follow Crystal GetRealItem; raw name, image and requirement stay separately recorded.",
                          "This is full-pack local availability, not Thin package delivery or ordinary game UI/combat validation."]}
    changed = [str(path.relative_to(WORKSPACE)) for path, expected in PINS.items() if digest(path.read_bytes()) != expected]
    if changed:
        raise RuntimeError(f"Inputs drifted; no accepted audit output: {changed[:10]}")
    summary["inputFilesPinned"] = len(PINS)
    summary["inputHashesStableAfterAudit"] = True
    out.mkdir(parents=True, exist_ok=True)
    outputs = {"summary.json": summary, "inventory.json": rows, "candidate-allocation.json": plan,
               "library-reserve.json": reserve,
               "motion-gaps.json": motion_issues, "motion-coverage.json": assets.motions,
               "page-verification.json": list(assets.pages.values()),
               "source-pins.json": [{"path": path.relative_to(WORKSPACE).as_posix(), "sha256": sha} for path, sha in sorted(PINS.items())]}
    for name, value in outputs.items():
        (out / name).write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({k: summary[k] for k in ("coreEquipmentTemplates", "levelBins", "resourceLibraryFamilies", "manifestCountChecked", "manifestErrors", "pngPagesChecked", "pngBytesChecked", "pngPageStatuses", "frameStatuses", "motionBindingsChecked", "motionBindingsWithGaps", "allBindingsResourceReadyTemplates", "candidateAllocationStatuses", "inputFilesPinned", "inputHashesStableAfterAudit")}, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
