import importlib.util
import os
from pathlib import Path
import sys
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
spec = importlib.util.spec_from_file_location("dense_wrapper", ROOT / "run_isolated_dense.py")
wrapper = importlib.util.module_from_spec(spec)
spec.loader.exec_module(wrapper)


class WrapperTests(unittest.TestCase):
    def test_dangerous_inherited_mir2_flags_are_cleared_and_listeners_are_loopback_file(self):
        old = {k: os.environ.get(k) for k in ["MIR2_QA_NATURAL_MOVEMENT_DELAY_MS", "MIR2_QA_NATURAL_KILL_DAMAGE_MULTIPLIER", "MIR2_GM_ACCOUNTS", "MIR2_QUEST_CADENCE", "CRYSTAL_CLIENT_ROOT"]}
        try:
            for k in old:
                os.environ[k] = "UNTRUSTED-INHERITED-VALUE"
            private = ROOT / "never-created-test-run" / "private-realm"
            env = wrapper.new_env(private / "accounts.json", "http://127.0.0.1:12345", 12346, {})
            for k in old:
                self.assertNotIn(k, env)
            self.assertEqual(env["MIR2_GATEWAY_ALLOW_UNSAFE_LOCAL_PLAYER_COMMANDS"], "0")
            self.assertEqual(env["MIR2_GATEWAY_WEB_ADDR"], "127.0.0.1:12345")
            self.assertEqual(env["MIR2_GATEWAY_TCP_ADDR"], "127.0.0.1:12346")
            self.assertEqual(env["MIR2_ACCOUNT_STORE_BACKEND"], "file")
            self.assertEqual(env["MIR2_CONTENT_PROFILE"], "platinum_176")
        finally:
            for k, v in old.items():
                if v is None: os.environ.pop(k, None)
                else: os.environ[k] = v

    def test_missing_capacity_is_unknown_not_zero(self):
        self.assertEqual(wrapper.active_count({"capacity": {"currentActiveSessions": 0}}), 0)
        for bad in [{}, {"capacity": {}}, {"capacity": {"currentActiveSessions": "0"}}, {"capacity": {"currentActiveSessions": False}}]:
            with self.assertRaises(RuntimeError): wrapper.active_count(bad)

    def test_all_false_acceptance_flags(self):
        self.assertGreaterEqual(len(wrapper.false_acceptance()), 6)
        self.assertTrue(all(v is False for v in wrapper.false_acceptance().values()))


if __name__ == "__main__":
    unittest.main()
