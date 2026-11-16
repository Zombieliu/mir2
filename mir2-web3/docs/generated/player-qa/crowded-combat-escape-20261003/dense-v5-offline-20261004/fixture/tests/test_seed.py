import copy
import importlib.util
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("guarded_seed", ROOT / "guarded_seed.py")
seed = importlib.util.module_from_spec(spec)
spec.loader.exec_module(seed)
PROFILE = json.loads((ROOT / "platinum_176.json").read_text(encoding="utf-8-sig"))
COHORT = [{"accountId": "dnfixture" + str(i), "name": "Dense" + str(i), "className": c} for i, c in enumerate(seed.BASE_VITALS)]


def fixture():
    accounts = {}
    for i, a in enumerate(COHORT):
        character = {"index": i + 1, "name": a["name"], "class": a["className"], "gender": "Male", "level": 1}
        save = {"revision": 3, "character": copy.deepcopy(character), "map_file_name": "0", "map_title": "BichonProvince", "position": {"x": 288, "y": 611}, "direction": "Up", "bind_point": {"map_file_name": "0", "position": {"x": 288, "y": 616}}, "hp": 18, "max_hp": 18, "mp": 13, "max_mp": 13, "experience": 0, "max_experience": 100, "gold": 700, "inventory_items_json": [json.dumps({"unique_id": i * 100 + 1, "name": "WoodenSword"})], "belt_items_json": [], "equipment_items_json": [], "quest_states_json": ["unchanged quest"], "skill_states_json": [], "buff_states_json": [], "stage5_systems_json": "unchanged social/cooldown state"}
        accounts[a["accountId"]] = {"password": "PRIVATE-DUMMY-TEST-HASH", "gm_level": 0, "characters": [character], "saves": {str(i + 1): save}, "storage_password": "PRIVATE-DUMMY-TEST-STORAGE"}
    return {"schemaVersion": 6, "nextCharacterIndex": 4, "accounts": accounts, "sharedGuilds": {}, "sharedHeroes": {}, "sharedConquests": {}}


class SeedTests(unittest.TestCase):
    def test_exact_collection_uid_fields_accept_some_zero_and_u64_max_without_rewriting(self):
        before = fixture()
        for account in before["accounts"].values():
            save = next(iter(account["saves"].values()))
            save["belt_items_json"] = [json.dumps({"unique_id": 10_000, "key": "medicine", "slot": 0, "container": "belt", "quantity": 2})]
            save["equipment_items_json"] = [json.dumps({"user_item_unique_id": 0, "key": "weapon", "slot": "weapon", "quantity": 1}), json.dumps({"user_item_unique_id": 2**64 - 1, "key": "armour", "slot": "armour", "quantity": 1})]
        self.assertTrue(seed.validate_cohort(before, COHORT))
        after, receipt = seed.transform_store(before, COHORT, "level", PROFILE)
        for a in COHORT:
            old = next(iter(before["accounts"][a["accountId"]]["saves"].values()))
            new = next(iter(after["accounts"][a["accountId"]]["saves"].values()))
            for field in ["inventory_items_json", "belt_items_json", "equipment_items_json"]:
                self.assertEqual(new[field], old[field], field)
        self.assertFalse(any("items_json" in row["pointer"] for row in receipt["diffs"]))

    def test_uid_rejects_bool_string_null_missing_negative_float_and_u64_overflow(self):
        for field, uid_field in [("inventory_items_json", "unique_id"), ("belt_items_json", "unique_id"), ("equipment_items_json", "user_item_unique_id")]:
            for value in [True, False, "12", None, -1, 1.0, 2**64]:
                store = fixture(); save = next(iter(store["accounts"][COHORT[0]["accountId"]]["saves"].values()))
                save[field] = [json.dumps({uid_field: value, "key": "item"})]
                with self.assertRaises(ValueError, msg=(field, value)):
                    seed.validate_cohort(store, COHORT)
            store = fixture(); save = next(iter(store["accounts"][COHORT[0]["accountId"]]["saves"].values()))
            save[field] = [json.dumps({"key": "item"})]
            with self.assertRaises(ValueError, msg=(field, "missing")):
                seed.validate_cohort(store, COHORT)

    def test_collection_uid_guard_has_no_legacy_alias_or_slot_fallback(self):
        for field, alias in [("inventory_items_json", "uniqueId"), ("belt_items_json", "uniqueId"), ("equipment_items_json", "unique_id"), ("equipment_items_json", "uniqueId")]:
            store = fixture(); save = next(iter(store["accounts"][COHORT[0]["accountId"]]["saves"].values()))
            save[field] = [json.dumps({alias: 42, "slot": "weapon", "key": "item"})]
            with self.assertRaises(ValueError, msg=(field, alias)):
                seed.validate_cohort(store, COHORT)

    def test_duplicate_real_uids_across_inventory_belt_and_equipment_are_refused(self):
        for field in ["belt_items_json", "equipment_items_json"]:
            store = fixture(); save = next(iter(store["accounts"][COHORT[0]["accountId"]]["saves"].values()))
            uid = json.loads(save["inventory_items_json"][0])["unique_id"]
            uid_field = "user_item_unique_id" if field == "equipment_items_json" else "unique_id"
            save[field] = [json.dumps({uid_field: uid})]
            with self.assertRaises(ValueError, msg=field):
                seed.validate_cohort(store, COHORT)
        store = fixture(); save = next(iter(store["accounts"][COHORT[0]["accountId"]]["saves"].values()))
        save["equipment_items_json"] = [json.dumps({"user_item_unique_id": 0}), json.dumps({"user_item_unique_id": 0})]
        with self.assertRaises(ValueError):
            seed.validate_cohort(store, COHORT)

    def test_source_vitals_exp_bounded_wallet_and_coherent_revisions(self):
        before = fixture()
        after, receipt = seed.transform_store(before, COHORT, "level", PROFILE)
        for a in COHORT:
            record = after["accounts"][a["accountId"]]
            save = next(iter(record["saves"].values()))
            hp, mp = seed.BASE_VITALS[a["className"]]
            self.assertEqual((save["hp"], save["max_hp"], save["mp"], save["max_mp"]), (hp, hp, mp, mp))
            self.assertEqual(save["max_experience"], 2_000_000)
            self.assertEqual(save["gold"], 20_000)
            self.assertEqual(save["revision"], 4)
            self.assertEqual(record["characters"][0], save["character"])
        self.assertEqual(before, fixture())
        public = json.dumps(receipt)
        self.assertNotIn("PRIVATE-DUMMY", public)
        self.assertNotIn("password", public)
        self.assertFalse(receipt["fullP1Accepted"])

    def test_schema_live_account_gm_item_uid_and_baseline_guards(self):
        for change in ("schema", "foreign", "gm", "duplicate", "desync", "dead", "buff", "learned"):
            store = fixture()
            account = store["accounts"][COHORT[0]["accountId"]]
            save = next(iter(account["saves"].values()))
            if change == "schema": store["schemaVersion"] = 5
            if change == "foreign": store["accounts"]["human-player"] = {}
            if change == "gm": account["gm_level"] = 10
            if change == "duplicate": save["belt_items_json"] = save["inventory_items_json"][:]
            if change == "desync": save["character"]["level"] = 2
            if change == "dead": save["hp"] = 0
            if change == "buff": save["buff_states_json"] = ["immunity"]
            if change == "learned": save["skill_states_json"] = ["injected spell"]
            with self.assertRaises(ValueError, msg=change): seed.transform_store(store, COHORT, "level", PROFILE)
        for gold in [-1, 20_001, True]:
            with self.assertRaises(ValueError): seed.transform_store(fixture(), COHORT, "level", PROFILE, gold=gold)

    def test_travel_writes_only_map_transform_revision_preserves_uid_bind_hp_skills(self):
        level, _ = seed.transform_store(fixture(), COHORT, "level", PROFILE)
        stage = {"map": "D021", "title": "WoomaTempleEntrance", "x": 58, "y": 57, "staticWalkable": True, "sourceValidated": True}
        after, receipt = seed.transform_store(level, COHORT, "travel", PROFILE, stage)
        for change in receipt["diffs"]:
            self.assertRegex(change["pointer"], r"/saves/\d+/(map_file_name|map_title|position/[xy]|direction|revision)$")
        for a in COHORT:
            b = next(iter(level["accounts"][a["accountId"]]["saves"].values()))
            s = next(iter(after["accounts"][a["accountId"]]["saves"].values()))
            for field in ["hp", "mp", "gold", "bind_point", "inventory_items_json", "skill_states_json", "buff_states_json", "stage5_systems_json"]:
                self.assertEqual(s[field], b[field], field)
        with self.assertRaises(ValueError): seed.transform_store(level, COHORT, "travel", PROFILE, {**stage, "staticWalkable": False})

    def test_capability_rejects_unowned_object_and_wrong_binary(self):
        with self.assertRaises(ValueError): seed.StoppedOwnedRealm(object(), ROOT / "accounts.json", ROOT, seed.ELF_SHA)
        with self.assertRaises(ValueError): seed.StoppedOwnedRealm(object(), ROOT / "accounts.json", ROOT, "0" * 64)

    def test_recursive_diff_refuses_added_schema_fields_and_items(self):
        with self.assertRaises(ValueError): seed.diffs({"a": 1}, {"a": 1, "secret": 1})
        with self.assertRaises(ValueError): seed.diffs([], ["item"])

    def test_owned_process_must_stop_zero_path_and_expected_hash_are_guarded(self):
        # Harmless Python children exercise Popen ownership/exit handling only.
        # They do not execute Gateway or count as gameplay acceptance.
        with tempfile.TemporaryDirectory(prefix="seed-selftest-", dir=ROOT) as temporary:
            private = Path(temporary).resolve()
            self.assertTrue(private.is_relative_to(ROOT))
            store = private / "accounts.json"
            raw = (json.dumps(fixture()) + "\n").encode("utf-8")
            store.write_bytes(raw)
            running = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(1)"], stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            guard = seed.StoppedOwnedRealm(running, store, private, seed.ELF_SHA)
            with self.assertRaises(ValueError): guard.check()
            running.wait(timeout=5)
            self.assertEqual(guard.check(), store)
            with self.assertRaises(ValueError): seed.seed_stopped_realm(guard, "0" * 64, COHORT, "level")
            actual_profile_sha = seed.PROFILE_SHA
            seed.PROFILE_SHA = "0" * 64
            try:
                with self.assertRaises(ValueError): seed.seed_stopped_realm(guard, seed.sha_bytes(raw), COHORT, "level")
            finally:
                seed.PROFILE_SHA = actual_profile_sha
            receipt = seed.seed_stopped_realm(guard, seed.sha_bytes(raw), COHORT, "level")
            self.assertEqual(receipt["ownedGatewayExitCode"], 0)
            self.assertEqual(receipt["afterStoreSha256"], seed.sha_bytes(store.read_bytes()))
            self.assertEqual((private / "before-level.private.json").read_bytes(), raw)
            with self.assertRaises(ValueError): seed.seed_stopped_realm(guard, seed.sha_bytes(raw), COHORT, "travel", {})
            foreign_guard = seed.StoppedOwnedRealm(running, ROOT / "platinum_176.json", private, seed.ELF_SHA)
            with self.assertRaises(ValueError): foreign_guard.check()
            failed = subprocess.Popen([sys.executable, "-c", "raise SystemExit(2)"], stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            failed.wait(timeout=5)
            with self.assertRaises(ValueError): seed.StoppedOwnedRealm(failed, store, private, seed.ELF_SHA).check()

    def test_hardlinked_store_and_symlink_ancestor_are_refused(self):
        with tempfile.TemporaryDirectory(prefix="link-selftest-", dir=ROOT) as temporary:
            base = Path(temporary).resolve()
            self.assertTrue(base.is_relative_to(ROOT))
            real = base / "real"
            real.mkdir()
            store = real / "accounts.json"
            store.write_text(json.dumps(fixture()), encoding="utf-8")
            stopped = subprocess.Popen([sys.executable, "-c", "pass"], stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            stopped.wait(timeout=5)
            alias = real / "hardlink.private.json"
            os.link(store, alias)
            with self.assertRaises(ValueError): seed.StoppedOwnedRealm(stopped, store, real, seed.ELF_SHA).check()
            alias.unlink()
            link = base / "ancestor-link"
            try:
                os.symlink(real, link, target_is_directory=True)
            except OSError as error:
                self.skipTest("Windows symbolic-link privilege unavailable; hardlink rejection passed: " + str(error.winerror))
            with self.assertRaises(ValueError): seed.StoppedOwnedRealm(stopped, link / "accounts.json", link, seed.ELF_SHA).check()


if __name__ == "__main__":
    unittest.main()
