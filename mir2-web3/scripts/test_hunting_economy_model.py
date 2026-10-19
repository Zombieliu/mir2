"""Run with: python -m unittest discover -s scripts -p test_hunting_economy_model.py"""

import copy
import json
import unittest

import hunting_economy_model as model


class SharedPolicyVectors(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.policy = model.load_policy()
        cls.vectors = json.loads((model.ROOT / "fixtures/hunting-economy-vectors.json").read_text(encoding="utf-8"))

    def policy_for(self, case):
        policy = copy.deepcopy(self.policy)
        policy.update(copy.deepcopy(case.get("policy_override", {})))
        return policy

    def assert_expected(self, actual, expected):
        self.assertEqual({key: actual[key] for key in expected}, expected)

    def test_disabled_default_and_units(self):
        self.assertFalse(self.policy["enabled"])
        self.assertEqual(model.LICENSE_ATOMS_PER_UNIT, 1_000_000)
        self.assertEqual(model.GOLD_PER_LICENSE, 1_000_000)
        self.assertEqual(self.policy["baseline_daily_mint_atoms"], 720_000)
        self.assertEqual(model.daily_mint_cap_atoms(self.policy), 1_080_000)

    def test_recalibrated_capacity_with_fixed_all_in_destruction(self):
        days = model.simulate(self.policy, 30, 7_200_000, 715_200)
        self.assertEqual(sum(day["conversion"]["bound_gold_debit"] for day in days), 21_456_000)
        self.assertEqual(sum(day["conversion"]["net_unbound_gold_credit"] for day in days), 21_026_880)
        for day in days:
            self.assertEqual(day["settlement"]["minted_atoms"], 720_000)
            self.assertEqual(day["settlement"]["decayed_atoms"], 4_800)
            self.assertEqual(day["conversion"]["fee_unbound_gold"], 14_304)
            self.assertEqual(day["closing_pool"]["stock_atoms"], 0)

    def test_settlements(self):
        for case in self.vectors["settlements"]:
            with self.subTest(case["name"]):
                original_pool = copy.deepcopy(case["pool"])
                first = model.plan_settlement(self.policy_for(case), case["pool"], case["destruction_gold_equivalent"])
                self.assert_expected(first, case["expected"])
                self.assertEqual(case["pool"], original_pool)
                self.assertEqual(first, model.plan_settlement(self.policy_for(case), case["pool"], case["destruction_gold_equivalent"]))

    def test_settlement_sequences(self):
        for case in self.vectors["settlement_sequences"]:
            pool = copy.deepcopy(case["initial_pool"])
            for index, step in enumerate(case["steps"]):
                with self.subTest(case=case["name"], index=index):
                    result = model.plan_settlement(self.policy_for(case), pool, step["destruction_gold_equivalent"])
                    self.assert_expected(result, step["expected"])
                    pool = result["next_pool"]

    def test_conversions(self):
        for case in self.vectors["conversions"]:
            with self.subTest(case["name"]):
                self.assertEqual(model.quote_conversion(self.policy_for(case), case["gross_gold"]), case["expected"])

    def test_invalid_policies(self):
        for case in self.vectors["invalid_policies"]:
            with self.subTest(case["name"]):
                with self.assertRaises(model.PolicyError) as caught:
                    model.validate_policy(self.policy_for(case))
                self.assertEqual(caught.exception.code, case["expected_error"])

    def test_invalid_settlements(self):
        for case in self.vectors["invalid_settlements"]:
            with self.subTest(case["name"]):
                with self.assertRaises(model.PolicyError) as caught:
                    model.plan_settlement(self.policy_for(case), case["pool"], case["destruction_gold_equivalent"])
                self.assertEqual(caught.exception.code, case["expected_error"])

    def test_invalid_conversions(self):
        for case in self.vectors["invalid_conversions"]:
            with self.subTest(case["name"]):
                with self.assertRaises(model.PolicyError) as caught:
                    model.quote_conversion(self.policy_for(case), case["gross_gold"])
                self.assertEqual(caught.exception.code, case["expected_error"])

    def test_business_days(self):
        for case in self.vectors["business_days"]:
            with self.subTest(case["name"]):
                self.assertEqual(model.business_day_key(case["unix_seconds"]), case["expected_day_key"])

    def test_settlement_windows(self):
        for case in self.vectors["settlement_windows"]:
            with self.subTest(case["name"]):
                self.assertEqual(model.settlement_window(case["day_key"]), (case["expected_start"], case["expected_end"]))

    def test_invalid_times(self):
        for group, function, argument in (
            ("invalid_business_days", model.business_day_key, "unix_seconds"),
            ("invalid_settlement_windows", model.settlement_window, "day_key"),
        ):
            for case in self.vectors[group]:
                with self.subTest(case["name"]):
                    with self.assertRaises(model.PolicyError) as caught:
                        function(case[argument])
                    self.assertEqual(caught.exception.code, case["expected_error"])

    def test_range_and_noninteger_inputs_rejected(self):
        for amount in (-1, model.U64_MAX + 1, 0.5, True):
            with self.subTest(amount=amount):
                with self.assertRaises(model.PolicyError):
                    model.quote_conversion(self.policy, amount)
        with self.assertRaises(model.PolicyError):
            model.validate_policy({**self.policy, "unexpected": 1})

    def test_fee_cannot_be_reduced_by_splitting(self):
        combined = model.quote_conversion(self.policy, 102)["fee_unbound_gold"]
        split = 2 * model.quote_conversion(self.policy, 51)["fee_unbound_gold"]
        self.assertGreaterEqual(split, combined)

    def test_offline_insufficient_request_does_not_partially_consume_pool(self):
        day = model.simulate(self.policy, 1, 7_200_000, 1_000_000)[0]
        self.assertEqual(day["conversion_status"], "insufficient_global_quota")
        self.assertIsNone(day["conversion"])
        self.assertEqual(day["closing_pool"], day["settlement"]["next_pool"])


if __name__ == "__main__":
    unittest.main()
