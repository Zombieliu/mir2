#!/usr/bin/env python3
"""Regression checks for source selection and audit failure classification."""
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("equipment_audit", Path(__file__).with_name("audit-equipment-level100.py"))
audit = importlib.util.module_from_spec(spec)
spec.loader.exec_module(audit)


def item(index, name, **kwargs):
    return {"item_index": index, "name": name, "required_gender": 3, "required_class": 31,
            "required_type": 0, "required_amount": 1, "class_based": False, "level_based": False, **kwargs}


class EquipmentAuditTests(unittest.TestCase):
    def test_assassin_standing_armour_follows_same_tier_weapon(self):
        self.assertEqual(audit.armour_standing_sample(58, "Assassin", True, 114), ("AArmour/58", 528))
        self.assertEqual(audit.armour_standing_sample(58, "Assassin", True, 13), ("CArmour/58", 824))
        self.assertEqual(audit.armour_standing_sample(58, "Assassin", True, 0), ("CArmour/58", 824))
        self.assertEqual(audit.armour_standing_sample(58, "Assassin", True, -1), ("AArmour/58", 528))
        self.assertEqual(audit.armour_standing_sample(58, "Archer", True, 219), ("CArmour/58", 824))

    def test_different_icon_ids_do_not_make_same_world_shape_unique(self):
        self.assertEqual(audit.appearance_key({"slot": "weapon", "shape": 55, "image": 3880}),
                         audit.appearance_key({"slot": "weapon", "shape": 55, "image": 3885}))
        self.assertNotEqual(audit.appearance_key({"slot": "ring", "shape": 0, "image": 1}),
                            audit.appearance_key({"slot": "ring", "shape": 0, "image": 2}))

    def test_public_urls_reject_traversal_but_keep_configured_junction_path(self):
        for url in ("/../secret", "https://example.test/a.png", "/C:/secret", "/a\\..\\secret"):
            with self.assertRaises(ValueError):
                audit.public_path(url)
        self.assertEqual(audit.public_path("/generated/crystal-packs/full/index.json"),
                         audit.PUBLIC.absolute() / "generated/crystal-packs/full/index.json")

    def test_assassin_requires_both_real_source_files(self):
        specs = audit.motion_specs({"item_type": 1, "shape": 110}, "Assassin", True)
        self.assertEqual([(s[0], s[1]) for s in specs], [("AWeapon/10 L", 512), ("AWeapon/10 R", 512),
                                                                      ("AWeapon/10 L", 416), ("AWeapon/10 R", 416)])
        self.assertIn("Attack4", specs[0][2])
        self.assertEqual(specs[2][2]["DeadCommonOffset"], (387, 1, 3))

    def test_archer_bow_has_distinct_common_and_alternate_offsets(self):
        specs = audit.motion_specs({"item_type": 1, "shape": 217}, "Archer", True)
        self.assertEqual([(s[0], s[1]) for s in specs], [("ARWeapon/17", 416), ("ARWeapon/17 S", 352)])
        self.assertEqual(specs[1][2]["WalkingBow"], (0, 6, 0))
        self.assertNotIn("Walking", specs[0][2])
        self.assertEqual(specs[0][2]["AttackRange3"], (224, 8, 0))

    def test_female_body_and_weapon_offsets_are_not_interchangeable(self):
        body = audit.motion_specs({"item_type": 2, "shape": 6}, "Warrior", True)
        weapon = audit.motion_specs({"item_type": 1, "shape": 13}, "Warrior", True)
        self.assertEqual(body[0][1], 808)
        self.assertEqual(weapon[0][1], 416)

    def test_dynamic_class_requires_exact_singleton_flag(self):
        root = item(1, "Family", class_based=True)
        wrong = item(2, "FamilyBroad", required_class=7)
        right = item(3, "FamilyWizard", required_class=2)
        self.assertEqual(audit.resolve_item([root, wrong, right], root, 100, 2)["item_index"], 3)

    def test_dynamic_level_tie_rules_match_source(self):
        combined = item(1, "Family", class_based=True, level_based=True)
        first = item(2, "FamilyA", required_class=2, required_amount=50)
        last = item(3, "FamilyB", required_class=2, required_amount=50)
        self.assertEqual(audit.resolve_item([combined, first, last], combined, 100, 2)["item_index"], 3)
        level_only = {**combined, "class_based": False}
        self.assertEqual(audit.resolve_item([level_only, first, last], level_only, 100, 2)["item_index"], 2)

    def test_intentional_no_draw_is_distinct_from_absent_frame(self):
        assets = audit.Assets({"libraries": []})
        assets.loaded["sample"] = {"frameSlotCount": 1, "_frames": {0: {"noDraw": True}}}
        self.assertEqual(assets.frame("sample", 0)["status"], "source_no_draw")
        self.assertEqual(assets.frame("sample", 1)["status"], "frame_out_of_range")

    def test_caster_candidate_does_not_choose_physical_only_weapon(self):
        def row(index, name, stats):
            return {"itemIndex": index, "name": name, "slot": "weapon", "requiredType": 0,
                    "requiredAmount": 43, "classBased": False, "levelBased": False,
                    "nameHasReplacementCharacter": False, "shape": index, "image": index,
                    "requiredClass": 7, "stats": stats, "icon": {}, "bindings": [{
                        "class": "Wizard", "gender": "Male", "resourceReady": True,
                        "effectiveItemIndexAt100": index}]}
        plan = audit.allocate([row(1, "Physical", {"5": 50}), row(2, "Magical", {"7": 13})])
        chosen = next(x for x in plan if x["class"] == "Wizard" and x["gender"] == "Male"
                      and x["targetLevel"] == 50 and x["slot"] == "weapon")
        self.assertEqual(chosen["sourceName"], "Magical")


if __name__ == "__main__":
    unittest.main(verbosity=2)
