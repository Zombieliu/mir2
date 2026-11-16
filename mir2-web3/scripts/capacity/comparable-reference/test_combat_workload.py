import base64
import copy
import hashlib
import json
import unittest

import combat_workload
import workload


class CombatContractTests(unittest.TestCase):
    def setUp(self):
        # Synthetic geometry only; never a runtime map or capacity result.
        self.nav = {"schema": "mir2.comparable.navigation.v1", "map": workload.definition()["map"],
                    "source": "original-library-cell-valid-and-movement-exclusion", "gameCodeUnmodified": True,
                    "encoding": "row-major-lsb0-bitset", "eligibleBitsBase64": base64.b64encode(b"\xff" * 80000).decode(),
                    "eligibleCount": 640000, "exclusions": {"movementRadius": 3, "npcRadius": 3, "allDoorCells": True, "safeZones": True}}
        self.sha = hashlib.sha256(json.dumps(self.nav).encode()).hexdigest()

    def make(self):
        return combat_workload.materialize(self.nav, 2, "functional", "game-core-local", navigation_file_sha256=self.sha)

    def test_combat_binds_policy_and_nonce_without_changing_dense(self):
        before = workload.materialize(self.nav, 2, "dense-movement-chat", "functional", "game-core-local", navigation_file_sha256=self.sha)
        plan = self.make()
        self.assertEqual(len(plan["actions"]), 104)
        self.assertEqual(sum(a["kind"] == "attack" for a in plan["actions"]), 100)
        self.assertFalse(plan["capacityClaimAllowed"])
        for a in plan["actions"]:
            if a["kind"] == "chat":
                self.assertIn(plan["planId"], a["nonce"])
                self.assertEqual(a["nonceSha256"], hashlib.sha256(a["nonce"].encode()).hexdigest())
        combat_workload.validate_plan(plan, self.nav, navigation_file_sha256=self.sha)
        self.assertEqual(before, workload.materialize(self.nav, 2, "dense-movement-chat", "functional", "game-core-local", navigation_file_sha256=self.sha))

    def test_mutated_healing_contract_rejected(self):
        plan = copy.deepcopy(self.make())
        plan["combatContract"]["potions"]["hpTriggerRatio"] = .9
        with self.assertRaises(ValueError):
            combat_workload.validate_plan(plan, self.nav, navigation_file_sha256=self.sha)

    def test_old_melee_plan_without_policy_rejected(self):
        old = workload.materialize(self.nav, 2, "dispersed-melee", "functional", "game-core-local", navigation_file_sha256=self.sha)
        with self.assertRaises(ValueError):
            combat_workload.validate_plan(old, self.nav, navigation_file_sha256=self.sha)


if __name__ == "__main__":
    unittest.main()
