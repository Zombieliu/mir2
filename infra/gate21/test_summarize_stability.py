from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("summarize-stability.py")


class SummarizeStabilityTest(unittest.TestCase):
    def run_summary(self, sample_count: int) -> subprocess.CompletedProcess[str]:
        active_start_ms = 1_000_000
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            samples = root / "samples.jsonl"
            load = root / "load.json"
            output = root / "output.json"
            samples.write_text(
                "\n".join(
                    json.dumps(
                        {
                            "sampledAtMs": active_start_ms + index * 15_000,
                            "activeStartMs": active_start_ms,
                            "referenceMemoryBytes": 1_000_000,
                            "replicatorMemoryBytes": 100_000,
                            "walBytes": index * 1_024,
                        }
                    )
                    for index in range(sample_count)
                )
                + "\n",
                encoding="utf-8",
            )
            load.write_text(
                json.dumps({"success": True, "measuredActiveDurationMs": 900_000}),
                encoding="utf-8",
            )
            result = subprocess.run(
                [
                    sys.executable,
                    str(SCRIPT),
                    "--samples",
                    str(samples),
                    "--load",
                    str(load),
                    "--output",
                    str(output),
                    "--sample-interval-seconds",
                    "15",
                ],
                check=False,
                capture_output=True,
                text=True,
            )
            if output.is_file():
                result.output_json = json.loads(output.read_text(encoding="utf-8"))
            return result

    def test_accepts_complete_fifteen_minute_window(self) -> None:
        result = self.run_summary(61)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(result.output_json["success"])
        self.assertEqual(
            result.output_json["profileId"], "mir2-regional-v1-3000-15m"
        )
        self.assertEqual(result.output_json["sampledDurationMs"], 900_000)
        self.assertIn("observedGrowthPercent", result.output_json["memory"])

    def test_rejects_shortened_window(self) -> None:
        result = self.run_summary(60)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(result.output_json["success"])
        self.assertFalse(
            result.output_json["assertions"]["sampleStreamCoveredFullActiveWindow"]
        )


if __name__ == "__main__":
    unittest.main()
