"""Read only the explicitly selected isolated File store after normal shutdown.

No password, account key, ledger, item payload or rental identity is exported.
The current simple common fixture carrier is supported; complex socket/rental
or derived-stat carriers fail closed rather than guess a Rust conversion.
"""
from __future__ import annotations

import argparse
import importlib.util
import json
import os
from pathlib import Path
import tempfile

SPEC = importlib.util.spec_from_file_location("modern_disk_verifier", Path(__file__).with_name("verify_report.py"))
v = importlib.util.module_from_spec(SPEC); SPEC.loader.exec_module(v)


def original_atom(item, equipment):
    required = {"quantity", "durability_current", "durability_max", "identified", "soul_bound_id", "cursed", "gem_count",
                "added_stats", "added_attack", "added_defence", "socketed", "socket_slots", "user_item_metadata",
                "rental_binding_flags", "rental_owner_name", "rental_expiry_binary_datetime", "rental_locked",
                "sealed_expiry_time_binary_datetime", "sealed_next_time_binary_datetime"}
    v.need(type(item) is dict and required <= set(item), "persisted_item_fields_missing")
    meta = item["user_item_metadata"]
    fields = {"item_index", "awake_type", "awake_values", "refined_value", "refine_added", "refine_success_chance",
              "wedding_ring", "expire_info", "rental_information", "sealed_info", "slots", "is_shop_item", "gm_made"}
    v.need(type(meta) is dict and fields <= set(meta), "persisted_original_item_metadata_missing")
    v.need(item["socketed"] == [] and item["socket_slots"] == 0 and meta["slots"] == []
           and item["added_attack"] == item["added_defence"] == 0 and item.get("added_luck", 0) == 0
           and item["rental_binding_flags"] == item["rental_expiry_binary_datetime"] == 0
           and item["rental_owner_name"] == "" and item["rental_locked"] is False
           and item["sealed_expiry_time_binary_datetime"] == item["sealed_next_time_binary_datetime"] == 0
           and meta["rental_information"] is None and meta["sealed_info"] is None, "unsupported_complex_persisted_item_carrier")
    uid = item.get("user_item_unique_id" if equipment else "unique_id")
    v.need(v.integer(uid) and uid > 0 and v.integer(meta["item_index"]) and meta["item_index"] > 0, "persisted_item_identity_missing")
    awake_type = item["awake_type"] if equipment else meta["awake_type"]
    awake_values = item["awake_values"] if equipment else meta["awake_values"]
    atom = {"unique_id": uid, "item_index": meta["item_index"], "count": item["quantity"],
            "current_dura": item["durability_current"] if item["durability_current"] is not None else 0,
            "max_dura": item["durability_max"] if item["durability_max"] is not None else 0,
            "added_stats": item["added_stats"], "awake_type": awake_type, "awake_values": awake_values,
            "cursed": item["cursed"], "gem_count": item["gem_count"], "identified": item["identified"],
            "soul_bound_id": item["soul_bound_id"] if item["soul_bound_id"] is not None else -1, **{key: meta[key] for key in ["refined_value", "refine_added", "refine_success_chance",
            "wedding_ring", "expire_info", "rental_information", "sealed_info", "slots", "is_shop_item", "gm_made"]}}
    return uid, atom


def store_items(save):
    result = {}
    for field, grid in [("inventory_items_json", "inventoryItems"), ("equipment_items_json", "equipmentItems"), ("belt_items_json", "beltItems")]:
        values = save.get(field)
        v.need(type(values) is list, "persisted_carried_array_missing")
        for value in values:
            v.need(type(value) is str, "persisted_item_json_string_required")
            item = v.decode(value)
            uid, atom = original_atom(item, grid == "equipmentItems")
            v.need(uid not in result and "slot" in item and "key" in item, "persisted_item_ownership_ambiguous")
            result[uid] = {"grid": grid, "slot": item["slot"], "container": item.get("container"), "key": item["key"], "userItem": atom}
    return result


def inspect_store(store, states):
    accounts = store.get("accounts")
    v.need(type(accounts) is dict and type(states) is dict and states, "isolated_store_or_expected_states_missing")
    samples = []
    for actor, expected in sorted(states.items()):
        name = expected.get("name")
        v.need(type(name) is str and name, "expected_character_identity_missing")
        matches = []
        for account in accounts.values():
            characters, saves = account.get("characters"), account.get("saves")
            v.need(type(characters) is list and type(saves) is dict, "stored_character_or_save_namespace_missing")
            for character in characters:
                if character.get("name") == name:
                    v.need(account.get("gm_level") == 0 and account.get("is_banned") is False, "ordinary_unbanned_account_required")
                    matching = [save for save in saves.values() if save.get("character", {}).get("name") == name
                                and save["character"].get("index") == character.get("index")]
                    v.need(len(matching) == 1, "unique_persisted_character_save_missing")
                    matches.append((character, matching[0]))
        v.need(len(matches) == 1, "persisted_character_identity_ambiguous")
        character, save = matches[0]
        expected_items = v.carried(expected.get("carried"), "modern")
        actual_items = store_items(save)
        v.need(set(actual_items) == set(expected_items["items"]), "persisted_owned_UID_set_differs")
        for uid, actual in actual_items.items():
            public = expected_items["items"][uid]
            semantic = v.complete_modern_user_item(public)
            v.need(actual["grid"] == public["grid"] and actual["slot"] == public["slot"]
                   and (actual["grid"] == "equipmentItems" or actual["container"] == public["payload"].get("container"))
                   and actual["key"] == semantic["key"] and actual["userItem"] == semantic["userItem"], "persisted_complete_item_or_grid_differs")
        character_fields = ["name", "level", "class", "gender"]
        v.need(all(character.get(k) == expected.get(k) == save["character"].get(k) for k in character_fields)
               and character["level"] == 40, "persisted_character_metadata_differs")
        position = {"x": expected.get("x"), "y": expected.get("y")}
        v.need(save.get("map_file_name") == expected.get("map") == "DogYoHyun" and save.get("position") == position
               and save.get("direction") == expected.get("direction") and save.get("hp") == expected.get("hp")
               and save.get("mp") == expected.get("mp"), "persisted_final_transform_or_vitals_differs")
        capacity = save.get("inventory_capacity")
        v.need(v.integer(capacity) and capacity >= 6 and capacity == expected["carried"].get("inventoryCapacity")
               and capacity - 6 == expected["carried"].get("maxBagSlots")
               and save.get("gold") == expected["carried"].get("gold"), "persisted_gold_or_capacity_differs")
        samples.append({"actorOrdinal": actor, "identityMatches": True, "map": save["map_file_name"], "position": position,
                        "direction":save["direction"],"hp":save["hp"],"mp":save["mp"],
                        "level": character["level"], "gold": save["gold"], "inventoryCapacity": capacity, "maxBagSlots": capacity-6,
                        "itemCounts": {key: sum(i["grid"] == key for i in actual_items.values()) for key in ["inventoryItems", "equipmentItems", "beltItems"]},
                        "completeItemsAndGridMatch": True, "finalTransformAndVitalsMatch": True,
                        "carriedFingerprint": expected_items["fingerprint"],
                        "persistedItemSemanticsSha256": v.digest(v.encoded(actual_items, sorted_keys=True))})
    return samples


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["plan", "summary", "events", "controller", "resources", "injector", "runtime", "accounts", "output"]:
        parser.add_argument("--"+name, type=Path, required=True)
    args = parser.parse_args(argv)
    result = {"schema": "mir2.comparable.modern-file-disk-readback.v1", "passed": False,
              "sampleCount": 0, "samples": [], "privateValuesExported": False, "capacityQualified": False}
    try:
        objects, meta = {}, {}
        for name in ["plan", "summary", "controller", "resources", "injector", "runtime"]:
            objects[name], meta[name] = v.read_file(getattr(args, name), jsonl=name in ["events", "resources", "injector"])
        objects["events"], meta["events"] = v.read_events(args.events, "modern", objects["plan"].get("population"))
        plan, report, events, controller = (objects[k] for k in ["plan", "summary", "events", "controller"])
        if plan.get("scenario")=="dispersed-melee":result["schema"]="mir2.comparable.modern-file-disk-readback.v2"
        elif plan.get("scenario")=="dense-movement-chat":result["schema"]="mir2.comparable.modern-file-disk-readback.v3"
        v.validate_plan(plan)
        v.need(report.get("schema") == "mir2.comparable.modern-report.v1" and report.get("backend") == "file", "isolated_File_report_required")
        checks=v.Checks();windows,actions,packets,admissions=v.verify_actions(plan,report,events,"modern",checks)
        v.verify_modern_lifecycle(plan,report,events,packets,admissions,checks)
        resource=v.verify_resources(plan,report,controller,objects["resources"],objects["injector"],meta,checks,
                                    event_span_ms=v.observed_driver_span(events,"modern"))
        v.need(not checks.failures and resource["valid"] and all(w["passed"] for w in windows), "public_protocol_or_resource_prerequisite_failed")
        runtime=objects["runtime"]
        v.need(runtime.get("schema") == "mir2.comparable.modern-runtime.v1" and runtime.get("backend") == "file"
               and runtime.get("sessionCache") == "in-memory" and runtime.get("planSha256") == meta["plan"]["sha256"]
               and runtime.get("binarySha256") == report.get("binarySha256"), "runtime_File_authority_unbound")
        if plan.get("scenario")=="dense-movement-chat":
            state_events,state_meta=v.read_dense_disk_state_events(args.events)
            v.validate_dense_state_binding(state_events,meta["events"])
            states,anchors=v.modern_disk_expected_states(plan,report,state_events,all_actor_dense=True)
            result.update(denseStateSourceSha256=state_meta["sha256"],denseStateSourceBytes=state_meta["bytes"],denseStateSourceRows=state_meta["rows"],denseStatePassAudit=state_events.audit)
        else:
            states,anchors=v.modern_disk_expected_states(plan,report,events,packets=packets,admissions=admissions)
        if plan.get("scenario") in ["dispersed-melee","dense-movement-chat"]:
            result.update(expectedStateBasis="all-actor-last-normal-LogOutSuccess-raw-reconstruction",allActorFinalStateAnchors=anchors)
        account_path=args.accounts.resolve(strict=True)
        allowed=(Path(tempfile.gettempdir())/"mir2-comparable-capacity-20261113/modern-audit").resolve()
        v.need(account_path.name == "accounts.json" and account_path.parent.name == "private"
               and account_path.is_relative_to(allowed) and (os.name != "nt" or account_path.drive.upper() == "C:")
               and Path(runtime["accountStore"]).resolve(strict=True) == account_path, "isolated_File_path_not_runtime_bound")
        store, store_meta=v.read_file(args.accounts)
        samples=inspect_store(store,states)
        result.update(passed=True,sampleCount=len(samples),samples=samples,accountFileSha256=store_meta["sha256"],accountFileBytes=store_meta["bytes"],
                      planSha256=meta["plan"]["sha256"],summarySha256=meta["summary"]["sha256"],eventsSha256=meta["events"]["sha256"],
                      controllerSha256=meta["controller"]["sha256"],runtimeReceiptSha256=meta["runtime"]["sha256"],
                      readAfterNormalStop=True,accountStorePathBoundToRuntime=True,sourceScope="direct private File characterSave; no session-cache query")
    except (v.EvidenceError,KeyError,TypeError,ValueError,OSError,AttributeError,IndexError) as error:
        result["failureCode"]=str(error) if isinstance(error,v.EvidenceError) else "disk_readback_schema_or_io_insufficient"
    result["readerSourceSha256"]=v.digest(Path(__file__).read_bytes())
    result["validatorSourceSha256"]=v.digest(Path(v.__file__).read_bytes())
    target=args.output.resolve()
    v.need(os.name != "nt" or target.drive.upper() == "C:","output_must_be_on_C")
    v.need(not target.exists() and not args.output.is_symlink(),"exclusive_new_public_output_required")
    with target.open("x",encoding="utf-8") as stream:json.dump(result,stream,indent=2,ensure_ascii=False);stream.write("\n")
    print(json.dumps({"passed":result["passed"],"sampleCount":result["sampleCount"],"failureCode":result.get("failureCode"),"outputSha256":v.digest(target.read_bytes())}))
    return 0 if result["passed"] else 1


if __name__=="__main__":raise SystemExit(main())
