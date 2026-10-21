"""Run with: python -m unittest discover -s scripts -p test_hunting_material_model.py"""
import copy
import json
import unittest

import hunting_economy_model as model


class SharedMaterialVectors(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.policy = model.load_policy()
        cls.vectors = json.loads((model.ROOT / "fixtures/hunting-material-vectors.json").read_text(encoding="utf-8"))

    def policy_for(self, case):
        policy = copy.deepcopy(self.policy)
        policy.update(copy.deepcopy(case.get("policy_override", {})))
        return policy

    def test_strict_valuation_vectors_and_canonical_order(self):
        for case in self.vectors["valid_valuations"]:
            with self.subTest(case["name"]):
                before = copy.deepcopy(case["input"])
                normalized = model.normalize_material_valuations(case["input"])
                self.assertEqual(normalized, case["expected"])
                self.assertEqual(model.parse_material_valuations(json.dumps(case["input"])), normalized)
                self.assertEqual(case["input"], before)
        for case in self.vectors["invalid_valuations"]:
            with self.subTest(case["name"]):
                with self.assertRaises(model.PolicyError):
                    model.normalize_material_valuations(case["input"])
                with self.assertRaises(model.PolicyError):
                    model.parse_material_valuations(json.dumps(case["input"]))
        for case in self.vectors["invalid_json"]:
            with self.subTest(case["name"]):
                with self.assertRaises(model.PolicyError):
                    model.parse_material_valuations(case["input"])

    def test_quote_vectors_do_not_credit_gold_or_debit_bound_gold(self):
        for case in self.vectors["quotes"]:
            with self.subTest(case["name"]):
                policy = self.policy_for(case)
                before = copy.deepcopy(policy)
                result = model.quote_material_conversion(policy, case["unit_gold"], case["quantity"])
                self.assertEqual(result, case["expected"])
                self.assertEqual(set(result), {"gold_equivalent", "license_atoms_debit", "fee_unbound_gold"})
                catalog = {"schema_version": 1, "valuation_version": "fixture-v1",
                           "entries": [{"item_index": 1001, "unit_gold": case["unit_gold"]}]}
                self.assertEqual(model.quote_material_conversion_for_item(policy, catalog, 1001, case["quantity"]), result)
                self.assertEqual(policy, before)

    def test_invalid_quote_vectors(self):
        for case in self.vectors["invalid_quotes"]:
            with self.subTest(case["name"]):
                with self.assertRaises(model.PolicyError) as caught:
                    model.quote_material_conversion(self.policy_for(case), case["unit_gold"], case["quantity"])
                self.assertEqual(caught.exception.code, case["expected_error"])

    def test_unconfigured_vectors_have_no_npc_or_market_fallback(self):
        for case in self.vectors["unconfigured_quotes"]:
            with self.subTest(case["name"]):
                with self.assertRaises(model.PolicyError) as caught:
                    model.quote_material_conversion_for_item(self.policy_for(case), case["valuations"], case["item_index"], case["quantity"])
                self.assertEqual(caught.exception.code, case["expected_error"])

    def test_split_vectors_preserve_quota_and_cannot_reduce_fee(self):
        for case in self.vectors["split_batches"]:
            with self.subTest(case["name"]):
                combined = model.quote_material_conversion(self.policy, case["unit_gold"], sum(case["quantities"]))
                split = [model.quote_material_conversion(self.policy, case["unit_gold"], quantity) for quantity in case["quantities"]]
                split_fee = sum(quote["fee_unbound_gold"] for quote in split)
                self.assertEqual(combined["fee_unbound_gold"], case["expected_combined_fee"])
                self.assertEqual(split_fee, case["expected_split_fee"])
                self.assertGreaterEqual(split_fee, combined["fee_unbound_gold"])
                self.assertEqual(sum(quote["license_atoms_debit"] for quote in split), combined["license_atoms_debit"])

    def test_constructor_ranges_types_and_version_bytes(self):
        for version in ("", " invalid", "x\u0085y", "界" * 43):
            with self.subTest(version=version):
                with self.assertRaises(model.PolicyError):
                    model.normalize_material_valuations({"schema_version": 1, "valuation_version": version, "entries": []})
        for field, values in (("unit_gold", (-1, 0.5, True, "1")), ("quantity", (-1, 0.5, True, "1"))):
            for value in values:
                with self.subTest(field=field, value=value):
                    args = {"unit_gold": 1, "quantity": 1, field: value}
                    with self.assertRaises(model.PolicyError):
                        model.quote_material_conversion(self.policy, **args)
        invalid_policy = copy.deepcopy(self.policy)
        invalid_policy["conversion_fee"]["denominator"] = 0
        with self.assertRaises(model.PolicyError) as caught:
            model.quote_material_conversion(invalid_policy, 100, 1)
        self.assertEqual(caught.exception.code, "invalid_policy")

    def test_empty_production_config_and_disabled_policy_only_plan(self):
        catalog = model.load_material_valuations()
        self.assertEqual(catalog["schema_version"], 1)
        self.assertEqual(catalog["entries"], [])
        self.assertFalse(self.policy["enabled"])
        with self.assertRaises(model.PolicyError) as caught:
            model.quote_material_conversion_for_item(self.policy, catalog, 1001, 1)
        self.assertEqual(caught.exception.code, "unconfigured_material")
        self.assertEqual(model.quote_material_conversion(self.policy, 1, 1)["fee_unbound_gold"], 1)


if __name__ == "__main__":
    unittest.main()
