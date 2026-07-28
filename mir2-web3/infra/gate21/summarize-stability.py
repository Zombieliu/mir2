#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import statistics
from pathlib import Path

MINIMUM_ACTIVE_DURATION_MS = 900_000
MAXIMUM_MEMORY_GROWTH_PERCENT = 5.0
MAXIMUM_WAL_BYTES = 1_073_741_824


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Summarize Gate 21 stability samples")
    parser.add_argument("--samples", required=True, type=Path)
    parser.add_argument("--load", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--sample-interval-seconds", required=True, type=int)
    return parser.parse_args()


def read_samples(path: Path) -> list[dict]:
    samples = [
        json.loads(line)
        for line in path.read_text(encoding="utf-8").splitlines()
        if line.strip()
    ]
    if not samples:
        raise SystemExit("Gate 21 stability sample stream is empty")
    timestamps = [sample["sampledAtMs"] for sample in samples]
    if timestamps != sorted(timestamps) or len(timestamps) != len(set(timestamps)):
        raise SystemExit("Gate 21 stability timestamps are not strictly increasing")
    return samples


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> None:
    args = parse_args()
    samples = read_samples(args.samples)
    load = json.loads(args.load.read_text(encoding="utf-8"))
    expected_samples = MINIMUM_ACTIVE_DURATION_MS // (args.sample_interval_seconds * 1000)
    minimum_samples = max(3, expected_samples - 2)
    window = max(3, min(12, len(samples) // 4))
    baseline_memory = statistics.median(
        sample["referenceMemoryBytes"] for sample in samples[:window]
    )
    ending_memory = statistics.median(
        sample["referenceMemoryBytes"] for sample in samples[-window:]
    )
    memory_growth_percent = (
        ((ending_memory - baseline_memory) / baseline_memory) * 100
        if baseline_memory
        else float("inf")
    )
    maximum_wal_bytes = max(sample["walBytes"] for sample in samples)
    sampled_duration_ms = samples[-1]["sampledAtMs"] - samples[0]["activeStartMs"]
    maximum_gap_ms = max(
        (
            right["sampledAtMs"] - left["sampledAtMs"]
            for left, right in zip(samples, samples[1:])
        ),
        default=0,
    )
    assertions = {
        "loadEvidenceAccepted": load.get("success") is True,
        "loadMeasuredFullFifteenMinutes": load.get("measuredActiveDurationMs", 0)
        >= MINIMUM_ACTIVE_DURATION_MS,
        "sampleStreamCoveredFullActiveWindow": sampled_duration_ms
        >= MINIMUM_ACTIVE_DURATION_MS,
        "sampleCountMatchesCadence": len(samples) >= minimum_samples,
        "sampleCadenceHadNoDoubleGap": maximum_gap_ms
        <= args.sample_interval_seconds * 2_000,
        "shortWindowReferenceMemoryGrowthWithinFivePercent": memory_growth_percent
        <= MAXIMUM_MEMORY_GROWTH_PERCENT,
        "durableUncompressedWalStayedWithinOneGiB": maximum_wal_bytes
        <= MAXIMUM_WAL_BYTES,
    }
    output = {
        "schemaVersion": 1,
        "gate": 21,
        "profileId": "mir2-regional-v1-3000-15m",
        "activeStartMs": samples[0]["activeStartMs"],
        "sampledDurationMs": sampled_duration_ms,
        "sampleIntervalSeconds": args.sample_interval_seconds,
        "sampleCount": len(samples),
        "maximumSampleGapMs": maximum_gap_ms,
        "memory": {
            "windowSamples": window,
            "baselineMedianBytes": baseline_memory,
            "endingMedianBytes": ending_memory,
            "observedGrowthPercent": memory_growth_percent,
            "maximumObservedBytes": max(
                sample["referenceMemoryBytes"] for sample in samples
            ),
        },
        "wal": {
            "maximumObservedBytes": maximum_wal_bytes,
            "limitBytes": MAXIMUM_WAL_BYTES,
        },
        "sourceSha256": {
            "samples": sha256(args.samples),
            "load": sha256(args.load),
        },
        "assertions": assertions,
        "success": all(assertions.values()),
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(
        json.dumps(output, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    if not output["success"]:
        raise SystemExit("Gate 21 stability acceptance failed")
    print(f"Gate 21 stability evidence written to {args.output}")


if __name__ == "__main__":
    main()
