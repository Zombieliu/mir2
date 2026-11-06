"""Capture only the authorized frozen ownership files and retained test logs.

No build, repository mutation, remote access, publication, or player-store access.
Commands in receipts are explicit normalized replay commands, not reconstructed
literal PowerShell transcripts. Raw stdout/stderr remains in its original file.
"""
from __future__ import annotations

import datetime
import difflib
import hashlib
import json
from pathlib import Path
import re
import subprocess

PROJECT = Path("C:/Users/Administrator/.codex/worktrees/crowded-combat-control/mir2-player-journey/mir2-web3")
BASELINE_PROJECT = Path("C:/Users/Administrator/.codex/worktrees/r17/mir2-player-journey/mir2-web3")
BASE = "3e1f9167097ff239faeffdde809a17b2c0009b6c"
R17 = "6032ef8b3e27dd97bad0b20c8676ef9f185db64b"
OUTPUT = Path(__file__).resolve().parent
LOG_ROOT = Path("C:/mir2-build")
ENVIRONMENT = {"CARGO_TARGET_DIR": "C:/mir2-build/gateway-tests-r49", "CARGO_BUILD_JOBS": "2"}
FILES = [
    "apps/simulation/src/runtime/zone/types.rs",
    "apps/simulation/src/runtime/zone/runtime.rs",
    "apps/simulation/src/runtime/zone/runtime/experience_ownership.rs",
    "apps/simulation/src/runtime/zone/runtime/checkpoint.rs",
    "apps/simulation/src/runtime/zone/runtime/entity_combat.rs",
    "apps/simulation/src/runtime/zone/runtime/vampire_ai.rs",
    "apps/simulation/src/runtime/zone/runtime/thunder_ai.rs",
    "apps/simulation/src/runtime/zone/runtime/shinsu_ai.rs",
    "apps/simulation/src/runtime/zone/runtime/pet_special_ai.rs",
    "apps/simulation/src/runtime/zone/runtime/journey_events.rs",
    "apps/simulation/tests/shared_monster_ownership.rs",
    "docs/CLASSIC-EXPERIENCE-OWNERSHIP-20261004.md",
]
NEW_FILES = {
    "apps/simulation/src/runtime/zone/runtime/experience_ownership.rs",
    "apps/simulation/tests/shared_monster_ownership.rs",
    "docs/CLASSIC-EXPERIENCE-OWNERSHIP-20261004.md",
}
AI_TARGETS = [
    "shared_monster_vampire_ai", "shared_monster_thunder_ai",
    "shared_monster_revival_ai", "shared_monster_shinsu_ai",
    "shared_monster_pet_special_ai", "shared_monster_hugger_ai",
    "shared_monster_spider_ai", "shared_monster_hell_ai",
    "shared_monster_kirin_snow_ai", "shared_monster_mud_boulder_ai",
    "shared_monster_horned_encounter_ai", "shared_monster_stone_trap_ai",
]
T = ["--test", "shared_monster_ownership"]
L = ["--lib", "runtime::zone::runtime::experience_ownership::tests"]
TAIL = ["--", "--test-threads=1", "--nocapture"]
RUNS: list[dict] = []


def run_receipt(suffix: str, arguments: list[str], expected: tuple[int, int],
                stage: str, note: str = "", baseline: bool = False,
                package: str = "mir2-simulation") -> None:
    RUNS.append({"log": f"shared-monster-ownership-{suffix}-20261004.log",
                 "arguments": arguments, "expected": expected,
                 "source_stage": stage, "note": note,
                 "baseline": baseline, "package": package})


run_receipt("red-01", T, (5, 11), "pre-implementation red",
            "One armour fixture used the already-resolved fallback scalar; its failure is not production armour evidence. Original failure remains retained.")
run_receipt("red-02", T, (6, 10), "corrected-fixture semantic red",
            "Production ownership wiring still unchanged. Authoritative DC/AC fixture correction only; ten genuine semantic failures.")
run_receipt("environment-red-03", T + ["map_quake_lethal_damage_preserves_an_existing_player_claim"], (0, 1), "unwired-new-types environmental red",
            "Actual AI98 quake HP damage and death occurred, but the retained player claim produced no award.")
run_receipt("master-baseline-04", T + ["wild_lethal_hit_on_real_master_pet_never_becomes_a_wild_kill_reward"], (1, 0), "pre-wiring existing Master guard")
run_receipt("green-01", T, (21, 0), "ownership implementation green")
run_receipt("private-green-02", L, (5, 0), "initial private rule green",
            "Internal AI6/58/113 arbitration and v4 injection tests only; ordinary entity target selection is not verified.")
run_receipt("checkpoint-adjacent-03", ["--lib", "runtime::zone::runtime::checkpoint::tests"], (15, 0), "current scoped adjacent checkpoint")
run_receipt("adjacent-summon-ai-04", [value for target in ["shared_entity_combat"] + AI_TARGETS for value in ("--test", target)], (20, 1), "current scoped adjacent",
            "Cargo stopped after shared_entity_combat 4/4 and shared_monster_hell_ai 16/17. Eleven other requested targets were not executed in this run.")
run_receipt("adjacent-summon-ai-05", [value for target in AI_TARGETS if target != "shared_monster_hell_ai" for value in ("--test", target)], (92, 0), "current remaining eleven adjacent targets",
            "Only the eleven targets not executed by the stopped prior run; no blanket retry or test weakening.")
run_receipt("hell-r17-baseline-06", ["--test", "shared_monster_hell_ai", "purification_then_same_bit_reapplication_starts_a_fresh_counted_lease"], (0, 1), "unchanged exact R17 comparison",
            "Exact same failure at shared_monster_hell_ai.rs:524. Frozen checkout was read-only; no fixture/runtime/cache reset.", baseline=True)
run_receipt("current-return-07", T + L, (6, 0), "current return after R17 comparison; final periodic-zero rule",
            "Global filter ran six private tests and zero public tests; all 21 public tests were filtered. Zero execution is not public acceptance.")
run_receipt("final-green-08", T, (21, 0), "final public ownership green",
            "Final semantic code includes trusted periodic-zero distinction. Later changed hunk layout only; no semantic edits afterward.")
for filter_text, expected in [("group_experience_tests", 5), ("summon_damage_tests", 1),
                              ("soulfire_practice_tests", 9), ("entity_combat::tests", 25),
                              ("reward_rate_tests", 2)]:
    suffix = "lib-" + filter_text.replace("::", "-")
    run_receipt(suffix, ["--lib", filter_text], (expected, 0), "current scoped adjacent library")
for filter_text, expected in [("poison", 8), ("summon", 9)]:
    run_receipt("shared-zone-" + filter_text, ["--test", "shared_zone", filter_text], (expected, 0), "final current scoped shared-zone adjacent")
for filter_text, expected in [("kill_award", 3), ("world_checkpoint", 4),
                              ("shared_drop_aoi_tests", 2), ("guild_kill_source_tests", 2)]:
    run_receipt("gateway-" + filter_text, ["--lib", filter_text], (expected, 0), "current scoped Gateway boundary",
                "Existing living-player reward/checkpoint/drop/durable-source tests; not dead-level-up end-to-end acceptance.", package="mir2-gateway")


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def git_read(*args: str, cwd: Path = PROJECT) -> bytes:
    return subprocess.run(["git", *args], cwd=cwd, check=True,
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE).stdout


def file_hashes() -> dict:
    return {relative: {"sha256": sha((PROJECT / relative).read_bytes()),
                       "bytes": (PROJECT / relative).stat().st_size}
            for relative in FILES}


def decode_log(raw: bytes) -> str:
    if raw.startswith((b"\xff\xfe", b"\xfe\xff")):
        return raw.decode("utf-16")
    return raw.decode("utf-8-sig", errors="replace")


def write_json(name: str, value: object) -> None:
    (OUTPUT / name).write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")


def main() -> None:
    before = file_hashes()
    actual_baseline = git_read("rev-parse", "HEAD", cwd=BASELINE_PROJECT).decode().strip()
    if actual_baseline != R17:
        raise RuntimeError(f"Unexpected frozen baseline: {actual_baseline}")
    git_read("diff", "--check", BASE, "--", *FILES)
    prefix = git_read("rev-parse", "--show-prefix").decode().strip()
    # Preserve ordinary repository-root paths, including the project prefix.
    patch = git_read("diff", "--no-ext-diff", BASE, "--",
                     *[path for path in FILES if path not in NEW_FILES]).decode("utf-8")
    for relative in FILES:
        if relative not in NEW_FILES:
            continue
        content = (PROJECT / relative).read_text(encoding="utf-8").splitlines(keepends=True)
        patch += f"diff --git a/{prefix}{relative} b/{prefix}{relative}\nnew file mode 100644\n"
        patch += "".join(difflib.unified_diff([], content, fromfile="/dev/null", tofile=f"b/{prefix}{relative}"))
    (OUTPUT / "implementation.diff").write_text(patch, encoding="utf-8", newline="\n")
    result_re = re.compile(r"test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out; finished in ([^\r\n]+)")
    receipts = []
    for description in RUNS:
        path = LOG_ROOT / description["log"]
        raw = path.read_bytes()
        text = decode_log(raw)
        blocks = []
        target = None
        for line in text.splitlines():
            if " Running " in line:
                target = line.strip()
            match = result_re.search(line)
            if match:
                blocks.append({"running_target": target, "result": match[1],
                               **{key: int(match[index]) for key, index in
                                  [("passed", 2), ("failed", 3), ("ignored", 4), ("measured", 5), ("filtered", 6)]},
                               "duration": match[7], "result_line": line.strip()})
        passed = sum(block["passed"] for block in blocks)
        failed = sum(block["failed"] for block in blocks)
        if (passed, failed) != description["expected"]:
            raise RuntimeError(f"Unexpected observed outcome: {path}: {passed}/{failed}")
        if any(block["ignored"] for block in blocks):
            raise RuntimeError(f"Unexpected ignored test in {path}")
        arguments = ["cargo", "+1.95.0", "test", "--locked", "-p", description["package"], *description["arguments"]]
        if description["baseline"]:
            arguments += ["--", "--exact", "--test-threads=1", "--nocapture"]
        else:
            arguments += TAIL
        receipts.append({
            "log_path": path.as_posix(), "log_sha256": sha(raw), "log_bytes": len(raw),
            "log_mtime_utc": datetime.datetime.fromtimestamp(path.stat().st_mtime, datetime.timezone.utc).isoformat(),
            "workdir": (BASELINE_PROJECT if description["baseline"] else PROJECT).as_posix(),
            "base_source_commit": R17 if description["baseline"] else BASE,
            "source_stage": description["source_stage"],
            "replay_argv": arguments, "command_kind": "explicit normalized replay; not literal original shell transcript",
            "environment": ENVIRONMENT, "rust_toolchain": "1.95.0", "maximum_compile_jobs": 2,
            "observed_test_blocks": blocks, "observed_passed": passed, "observed_failed": failed,
            "observed_failed_test_names": [match[1] for line in text.splitlines()
                                           if (match := re.fullmatch(r"test (.*?) \.\.\. FAILED", line))],
            "expected_cargo_exit_success": failed == 0, "note": description["note"],
        })
    after = file_hashes()
    if before != after:
        raise RuntimeError("Authorized frozen sources changed during evidence capture")
    captured = datetime.datetime.now(datetime.timezone.utc).isoformat()
    write_json("source-manifest.json", {
        "captured_at_utc": captured, "project": PROJECT.as_posix(), "implementation_base": BASE,
        "runtime_r17_base": R17, "git_repository_prefix": prefix,
        "head_observed_at_capture": git_read("rev-parse", "HEAD").decode().strip(),
        "source_scope": "Exactly twelve authorized ownership files; unrelated integration excluded",
        "source_frozen": True, "hashes_stable_during_capture": True, "files": before,
        "compiler_companion_only_defaults": {
            "apps/simulation/src/runtime/zone/runtime/shinsu_ai.rs": 1,
            "apps/simulation/src/runtime/zone/runtime/pet_special_ai.rs": 1,
            "apps/simulation/src/runtime/zone/runtime/journey_events.rs": 3,
        },
        "diff": {"path": (OUTPUT / "implementation.diff").as_posix(), "sha256": sha((OUTPUT / "implementation.diff").read_bytes())},
    })
    write_json("run-receipts.json", {
        "captured_at_utc": captured, "evidence_kind": "public trusted Zone command fixtures and scoped existing tests",
        "native_or_live_network_acceptance": False, "human_acceptance": False,
        "full_suite_clean": False, "old_baseline_failure_retained": True,
        "raw_logs_are_not_overwritten": True, "commands_are_normalized_replays": True,
        "runs": receipts,
    })
    lines = [
        "# Frozen first same-map EXPOwner implementation evidence", "",
        "Implementation baseline: " + BASE + ". R17 gameplay/failure comparison: " + R17 + ".",
        "Twelve authorized files are frozen. Root may integrate unrelated map work; this patch excludes it.",
        "No commit, release build, upload, live-store access, deployment, or F-drive installation occurred.", "",
        "The complete patch uses ordinary repository-root paths (mir2-web3/...). source-manifest.json commits",
        "on-disk bytes for the exact write set; run-receipts.json has explicit normalized replay argv,",
        "observed per-target counts, filtered/ignored counts, original log paths, hashes and retained failures.",
        "These are repeatable commands, not claims that the original PowerShell transcript was logged.", "",
        "## Results", "",
        "Corrected semantic RED: 16 tests, 6 passed / 10 failed. Environmental AI98 RED: 1 failed.",
        "Final public ownership GREEN: 21/21. Final private rules: 6/6. Checkpoint adjacent: 15/15.",
        "Public fixture assertions include actual HP/damage, actor identity, EXP recipient, quest target",
        "and ground item/owner identity. This does not prove authenticated TCP/WSS or human/native acceptance.",
        "One adjacent Hell purification test fails identically at line524 in unchanged R17 and current.",
        "It is retained as an independent unresolved fixture/eligibility issue, not relabeled as all green.", "",
        "| Retained log | Passed | Failed |", "| --- | ---: | ---: |",
    ]
    for receipt in receipts:
        lines.append(f"| {Path(receipt['log_path']).name} | {receipt['observed_passed']} | {receipt['observed_failed']} |")
    lines += [
        "", "The current-return-07 public target executed zero tests (21 filtered). Only final-green-08",
        "counts as the final 21-test public result. The stopped adjacent-04 run left eleven targets unexecuted;",
        "adjacent-05 executes exactly those eleven (92/92). No ignored tests occur in these scopes.", "",
        "## Remaining source-parity and acceptance gates", "",
        "- Cross-map online spawned identity, teleport vs logout/despawn, global revocation and routing remain manager work.",
        "- Same-map fresh Join revocation is an identity safety boundary, not full Crystal global Node parity.",
        "- Ground Owner remains existing 18000ms, not original 60000ms; no duration changes this round.",
        "- Existing same-zone equal group formula and generic Boss contribution policy are preserved only for scope.",
        "  No explicit human acceptance of the Boss difference or group source parity is implied.",
        "- AI6/58/113 clearing and zero periodic-poison exceptions are private trusted rules; selection and zero-poison",
        "  full-chain behavior are not implemented/accepted. Normal positive poison producers and tick gates are unchanged.",
        "- PK LastHitter, pet PvP and remaining species, ordinary two-account TCP/WSS and native frontend are separate gates.",
        "- User daily/weekly EXP remains unchanged.",
        "- Zone schema/root v5 authenticates owner/deadline/forced impact. Legacy v1-v4 ignores forward fields.",
        "  A future rollback to old R17 must restore a matching pre-upgrade world checkpoint; old binary rejects v5.", "",
        "## Located dead-level-up risk; not an end-to-end pass", "",
        "Crystal HumanObject.cs:845-865 refills HP/MP on LevelUp but SetHP does not clear existing Dead.",
        "Current apps/simulation/src/runtime/leveling.rs:218 fills HP, Gateway routing.rs:10945-10952",
        "synchronizes vitals on LevelChanged, and zone/runtime.rs:1394-1404 derives Dead solely from HP0.",
        "A dead solo poison owner's new Zone award could therefore trigger unintentional resurrection and status clearing.",
        "The read-only personal/durable/Gateway consumption trace has no subsequent alive rejection, but outbound alone",
        "does not prove actual XP delivery, persistence or safe death state. This round deliberately leaves that code",
        "untouched. Root will design/run a separate complete authoritative dead-XP/level-up Gate before expanding it.", "",
        "The repository scoped report is docs/CLASSIC-EXPERIENCE-OWNERSHIP-20261004.md. Source and evidence are frozen;",
        "no next-round implementation is started by this worker.",
    ]
    (OUTPUT / "README.md").write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")
    write_json("evidence-manifest.json", {
        "captured_at_utc": captured,
        "files": {path.name: {"sha256": sha(path.read_bytes()), "bytes": path.stat().st_size}
                  for path in sorted(OUTPUT.iterdir()) if path.is_file() and path.name != "evidence-manifest.json"},
    })
    print(json.dumps({"output": OUTPUT.as_posix(), "authorized_source_files": len(FILES),
                      "retained_run_logs": len(receipts), "source_hashes_stable": True,
                      "final_public_passed": 21, "final_private_passed": 6,
                      "full_suite_clean": False, "baseline_failure_retained": True}, ensure_ascii=False))


if __name__ == "__main__":
    main()
