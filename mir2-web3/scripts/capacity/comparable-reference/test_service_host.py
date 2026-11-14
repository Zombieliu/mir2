import json
import os
import socket
from pathlib import Path
import sys
import tempfile
import threading
import time
import unittest

from service_host import child_environment, run


class NormalShutdownTests(unittest.TestCase):
    @unittest.skipUnless(os.name == "nt", "Windows listener inventory")
    def test_readiness_finds_exact_owner_without_connecting(self):
        from run_stage import ipv4_listeners
        with socket.socket() as sock:
            sock.bind(("127.0.0.1", 0))
            sock.listen()
            self.assertIn(("127.0.0.1", sock.getsockname()[1], os.getpid()), ipv4_listeners())
            sock.settimeout(.05)
            with self.assertRaises(TimeoutError):
                sock.accept()

    def test_environment_is_child_only_and_explicit(self):
        from unittest.mock import patch
        with patch.dict(os.environ, {"MIR2_UNRELATED": "old", "CRYSTAL_OLD": "old", "NORMAL_VAR": "kept"}):
            child = child_environment({"MIR2_TEST": "fresh", "MIR2_UNRELATED": None})
            self.assertNotIn("MIR2_UNRELATED", child)
            self.assertNotIn("CRYSTAL_OLD", child)
            self.assertEqual(child["MIR2_TEST"], "fresh")
            self.assertEqual(child["NORMAL_VAR"], "kept")
            self.assertEqual(os.environ["MIR2_UNRELATED"], "old")
        with self.assertRaises(ValueError):
            child_environment({"PATH": "replacement"})

    @unittest.skipUnless(os.name == "nt", "C-only native wrapper")
    def test_child_receives_normal_shutdown_and_actual_exit_recorded(self):
        with tempfile.TemporaryDirectory(prefix="mir2-normal-stop-") as directory:
            root = Path(directory).resolve()
            child = root / "child.py"
            child.write_text("import sys\nline=sys.stdin.readline()\nprint(line.strip())\nsys.exit(0 if line=='shutdown\\n' else 2)\n", encoding="utf-8")
            stop = root / "controller-stop.txt"
            def signal():
                deadline = time.monotonic() + 10
                while not (root / "service-started.json").exists() and time.monotonic() < deadline:
                    time.sleep(.01)
                stop.write_text("normal test stop\n", encoding="utf-8")
            thread = threading.Thread(target=signal)
            thread.start()
            code = run({"schema": "mir2.comparable.service-host.v1", "command": [sys.executable, str(child)],
                        "cwd": str(root), "stopMode": "stdin", "stopFile": str(stop), "wallSeconds": 60,
                        "environment": {"MIR2_GATEWAY_STDIN_SHUTDOWN": "1"}}, root)
            thread.join(timeout=1)
            self.assertEqual(code, 0)
            result = json.loads((root / "service-result.json").read_text(encoding="utf-8"))
            self.assertEqual(result["actualExitCode"], 0)
            self.assertEqual(result["stopReason"], "controller-marker")
            self.assertFalse(result["forceKillUsed"])
            self.assertEqual((root / "game-stdout.log").read_text().strip(), "shutdown")


if __name__ == "__main__":
    unittest.main()
