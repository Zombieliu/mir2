"""Independent action-count, causal-budget and false-qualification checks."""
import base64
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import unittest

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("comparable_workload", HERE / "workload.py")
W = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(W)


def navigation():
    return {"schema": "mir2.comparable.navigation.v1", "map": W.definition()["map"],
            "source": "original-library-cell-valid-and-movement-exclusion", "gameCodeUnmodified": True,
            "encoding": "row-major-lsb0-bitset", "eligibleBitsBase64": base64.b64encode(b"\xff" * 80000).decode(),
            "eligibleCount": 640000,
            "exclusions": {"movementRadius": 3, "npcRadius": 3, "allDoorCells": True, "safeZones": True}}


class WorkloadTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.nav = navigation()
        cls.nav_bytes = json.dumps(cls.nav).encode()
        cls.nav_sha = hashlib.sha256(cls.nav_bytes).hexdigest()
        cls.functional = W.materialize(cls.nav, 2, navigation_file_sha256=cls.nav_sha)
        cls.capacity = W.materialize(cls.nav, 100, purpose="capacity", navigation_file_sha256=cls.nav_sha)

    def test_fixed_per_actor_minute_counts_and_absolute_deadlines(self):
        for plan in [self.functional, self.capacity]:
            minutes = plan["durationMs"] // 60000
            for actor in range(plan["population"]):
                rows = [a for a in plan["actions"] if a["actorOrdinal"] == actor]
                for minute in range(minutes):
                    selected = [a for a in rows if minute * 60000 <= a["dueMs"] < (minute + 1) * 60000]
                    self.assertEqual(sum(a["kind"] == "walk" for a in selected), 30)
                    self.assertEqual(sum(a["kind"] == "chat" for a in selected), 2)
                for action in rows:
                    self.assertEqual(action["deadlineMs"] - action["dueMs"], 2000 if action["kind"] == "walk" else 2500)

    def test_all_population_chat_phases_stay_between_own_primary_actions(self):
        for population in [2, 25, 50, 100]:
            for scenario, interval in [("dense-movement-chat", 2000), ("dispersed-melee", 1200)]:
                plan = W.materialize(self.nav, population, scenario=scenario,
                                     purpose="functional" if population == 2 else "capacity", navigation_file_sha256=self.nav_sha)
                self.assertEqual(plan["scheduling"]["minimumSameActorOfferGapMs"], interval // 2)
                for a in plan["actions"]:
                    if a["kind"] == "chat":
                        phase = a["actorOrdinal"] * interval // population
                        self.assertEqual((a["dueMs"] - phase) % interval, interval // 2)
                for ordinal in range(population):
                    due = sorted(a["dueMs"] for a in plan["actions"] if a["actorOrdinal"] == ordinal)
                    self.assertGreaterEqual(min(b - a for a, b in zip(due, due[1:])), interval // 2)

    def test_schedule_revision_preserves_counts_thresholds_and_resource_contract(self):
        old = json.loads((HERE / "workload.v1.json").read_text(encoding="utf-8"))
        new = copy.deepcopy(W.definition())
        new["schema"] = old["schema"]
        for scenario in new["scenarios"].values():
            self.assertEqual(scenario.pop("chatPhaseRule"), "same-primary-phase-with-midpoint-offset")
        self.assertEqual(new, old)

    def test_no_overlapping_lanes_or_far_expected_peers(self):
        occupied = set()
        by_id = {a["ordinal"]: a for a in self.capacity["actors"]}
        for actor in by_id.values():
            for pos in actor["lane"]:
                cell = (pos["x"], pos["y"])
                self.assertNotIn(cell, occupied)
                occupied.add(cell)
            for peer in actor["peerOrdinals"]:
                for a in actor["lane"]:
                    for b in by_id[peer]["lane"]:
                        self.assertLessEqual(max(abs(a["x"] - b["x"]), abs(a["y"] - b["y"])), 4)

    def test_odd_population_preserves_complete_groups(self):
        self.assertEqual(W.groups(25)[-1], [22, 23, 24])
        self.assertEqual(sorted(x for group in W.groups(25) for x in group), list(range(25)))

    def test_nonce_unique_and_binds_exact_bytes(self):
        rows = [a for a in self.capacity["actions"] if a["kind"] == "chat"]
        self.assertEqual(len({a["nonce"] for a in rows}), len(rows))
        for action in rows:
            self.assertEqual(hashlib.sha256(action["nonce"].encode()).hexdigest(), action["nonceSha256"])

    def test_deterministic_full_plan_and_real_file_hash_separate_from_canonical(self):
        actual = W.materialize(self.nav, 100, purpose="capacity", navigation_file_sha256=self.nav_sha)
        self.assertEqual(actual, self.capacity)
        self.assertEqual(actual["navigationSha256"], hashlib.sha256(self.nav_bytes).hexdigest())
        self.assertNotEqual(actual["navigationSha256"], actual["navigationCanonicalSha256"])

    def test_changes_to_threshold_or_action_rejected(self):
        for mutation in [lambda p: p["thresholds"].update(logoutAckDeadlineMs=6000),
                         lambda p: p["actions"][0].update(deadlineMs=4000),
                         lambda p: p.update(capacityClaimAllowed=True),
                         lambda p: p["actors"][0].update(peerOrdinals=[])]:
            changed = copy.deepcopy(self.functional)
            mutation(changed)
            with self.assertRaisesRegex(ValueError, "materialized_plan_changed"):
                W.validate_plan(changed, self.nav, navigation_file_sha256=self.nav_sha)

    def test_core_and_planned_manifest_never_claim_capacity(self):
        self.assertFalse(self.capacity["capacityQualificationEligible"])
        for layer in ["full-stack-local", "cloud-production"]:
            plan = W.materialize(self.nav, 25, purpose="capacity", layer=layer, navigation_file_sha256=self.nav_sha)
            self.assertTrue(plan["capacityQualificationEligible"])
            self.assertFalse(plan["capacityClaimAllowed"])

    def test_window_stickiness_does_not_relabel_not_sent_or_timeout(self):
        policy = self.capacity["assessment"]
        self.assertEqual(policy["windowMs"], 60000)
        self.assertIs(policy["perActor"], True)
        self.assertIs(policy["allCompleteWindowsRequired"], True)
        self.assertIs(policy["failedActionCanBecomeCompleted"], False)
        self.assertIs(policy["notSentIsTimeout"], False)
        self.assertIs(policy["notSentIsCompleted"], False)
        self.assertEqual(self.capacity["generatorSourceSha256"], hashlib.sha256((HERE / "workload.py").read_bytes()).hexdigest())

    def test_short_capacity_and_wrong_purpose_population_refused(self):
        for pop, purpose in [(2, "capacity"), (25, "functional"), (True, "functional"), (1000, "capacity")]:
            with self.assertRaises(ValueError):
                W.materialize(self.nav, pop, purpose=purpose, navigation_file_sha256=self.nav_sha)

    def test_unused_variant_map_or_missing_exclusions_refused(self):
        wrong = copy.deepcopy(self.nav)
        wrong["map"]["rawSha256"] = "d2edc72331d0383dd9cdf6e097e03a781f962e36b56e6c3544fb023f20ed9143"
        with self.assertRaisesRegex(ValueError, "navigation_map_identity"):
            W.materialize(wrong, 2, navigation_file_sha256=self.nav_sha)
        wrong = copy.deepcopy(self.nav)
        wrong["exclusions"]["movementRadius"] = 0
        with self.assertRaisesRegex(ValueError, "navigation_transition_exclusions"):
            W.materialize(wrong, 2, navigation_file_sha256=self.nav_sha)

    def test_bad_base64_length_and_false_eligible_count_refused(self):
        for key, value in [("eligibleBitsBase64", "!"), ("eligibleBitsBase64", "AA=="), ("eligibleCount", 1)]:
            wrong = copy.deepcopy(self.nav)
            wrong[key] = value
            with self.assertRaises(ValueError):
                W.materialize(wrong, 2, navigation_file_sha256=self.nav_sha)


if __name__ == "__main__":
    unittest.main()
