"""Freeze the approved six-file P5 slice without changing the Git index or source."""
from pathlib import Path
import datetime
import difflib
import hashlib
import json
import re
import subprocess
import sys

sys.stdout.reconfigure(encoding="utf-8", errors="replace")
ROOT = Path("C:/Users/Administrator/.codex/worktrees/p5/mir2-player-journey/mir2-web3")
PROOF = Path("C:/mir2-playtest-releases/20261004-native-r18/guild-rank-implementation-01")
PLAN = Path("C:/mir2-playtest-releases/20261004-native-r18/guild-management-plan-01")
BASE = "321316b791120a6fe549e4006623e63333a5543f"
FILES = [
    "apps/simulation/src/runtime/shared_guilds.rs",
    "apps/simulation/src/runtime/shared_guild_management.rs",
    "apps/gateway/src/shared_guilds.rs",
    "apps/simulation/tests/shared_guild_rank_management.rs",
    "apps/gateway/tests/shared_guild_rank_management.rs",
    "docs/SHARED-GUILD-RANK-RENAME-20261004.md",
]
EXISTING = [FILES[0], FILES[2]]
PHASES = ["gateway-red-01", "gateway-green-01", "simulation-green-01", "guild-adjacent-01"]


def sha(data):
    return hashlib.sha256(data).hexdigest()


def git(*args, cwd=ROOT, check=True):
    run = subprocess.run(["git", "-c", "gc.auto=0", *args], cwd=cwd, capture_output=True)
    if check and run.returncode:
        raise RuntimeError(run.stderr.decode("utf-8", errors="replace"))
    return run


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


assert not (PROOF / "freeze-receipt.json").exists(), "frozen receipt already exists"
head = git("rev-parse", "HEAD").stdout.decode().strip()
assert head == BASE, head
status = git("status", "--porcelain=v1", "--untracked-files=all").stdout.decode("utf-8")
observed = {line[3:].removeprefix("mir2-web3/") for line in status.splitlines() if line}
assert observed == set(FILES), (observed, FILES)
git("diff", "--check")

source_bytes = {relative: (ROOT / relative).read_bytes() for relative in FILES}
final_inputs = {
    relative: {"sha256": sha(data), "bytes": len(data)}
    for relative, data in source_bytes.items()
}

run_results = []
green_total = 0
for phase in PHASES:
    receipt = json.loads((PROOF / (phase + ".json")).read_text(encoding="utf-8"))
    log_bytes = (PROOF / (phase + ".log")).read_bytes()
    assert sha(log_bytes) == receipt["log_sha256"]
    assert receipt["source_unchanged_during_run"] is True
    assert receipt["exit_code"] == (101 if phase == "gateway-red-01" else 0)
    if phase != "gateway-red-01":
        for relative, expected in receipt["inputs"].items():
            assert final_inputs[relative] == expected, (phase, relative)
    log = log_bytes.decode("utf-8", errors="replace")
    tests = re.findall(r"^test (\w+) \.\.\. (.*)$", log, flags=re.MULTILINE)
    summary = re.findall(r"^test result: (.+)$", log, flags=re.MULTILINE)
    counts = [tuple(map(int, match)) for match in re.findall(r"(\d+) passed; (\d+) failed;", "\n".join(summary))]
    passed = sum(count[0] for count in counts)
    failed = sum(count[1] for count in counts)
    assert len(tests) == passed + failed, (phase, tests, counts)
    if phase != "gateway-red-01":
        green_total += passed
        assert failed == 0
    run_results.append({
        "phase": phase, "exit_code": receipt["exit_code"],
        "command": receipt["command"], "passed": passed, "failed": failed,
        "test_names": [name for name, _ in tests], "raw_summaries": summary,
        "log_sha256": sha(log_bytes), "receipt_sha256": sha((PROOF / (phase + ".json")).read_bytes()),
    })
assert green_total == 21
assert run_results[0]["passed"] == 2 and run_results[0]["failed"] == 6
gateway_red = json.loads((PROOF / "gateway-red-01.json").read_text(encoding="utf-8"))
assert sha((PROOF / "gateway-red-input.rs").read_bytes()) == gateway_red["inputs"][FILES[4]]["sha256"]
assert final_inputs[FILES[4]] == gateway_red["inputs"][FILES[4]]
write_json(PROOF / "test-results.json", {
    "fixture_scope": "prepared isolated canonical guild; ordinary authenticated typed Gateway packets; direct authority fault probes",
    "red_gateway_fixture_unchanged": True, "green_and_adjacent_total_passed": green_total,
    "runs": run_results,
    "not_executed": ["native UI", "TCP/WSS", "PostgreSQL/CAS runtime", "production deployment", "human acceptance"],
})

patch = git("diff", "--binary", "--", *EXISTING).stdout
baseline_files = {}
for relative in EXISTING:
    repo_relative = "mir2-web3/" + relative
    data = git("show", BASE + ":" + repo_relative).stdout
    target = PROOF / "baseline-source" / repo_relative
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_bytes(data)
    baseline_files[relative] = {"git_blob_sha256": sha(data), "bytes": len(data), "red_worktree": gateway_red["inputs"][relative]}
for relative in FILES:
    repo_relative = "mir2-web3/" + relative
    target = PROOF / "final-source" / repo_relative
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_bytes(source_bytes[relative])
    if relative not in EXISTING:
        # Match a normal text Git patch while retaining exact bytes independently.
        content = source_bytes[relative].decode("utf-8").replace("\r\n", "\n")
        assert content.endswith("\n")
        body = "".join(difflib.unified_diff([], content.splitlines(keepends=True), fromfile="/dev/null", tofile="b/" + repo_relative))
        patch += ("diff --git a/" + repo_relative + " b/" + repo_relative + "\nnew file mode 100644\n" + body).encode("utf-8")
(PROOF / "final.patch").write_bytes(patch)
reverse_check = git("apply", "--check", "--reverse", "--verbose", str(PROOF / "final.patch"), cwd=ROOT.parent, check=False)
(PROOF / "patch-check.log").write_bytes(reverse_check.stdout + reverse_check.stderr)
assert reverse_check.returncode == 0, reverse_check.stderr.decode("utf-8", errors="replace")

crystal_index = json.loads((PLAN / "source-index.json").read_text(encoding="utf-8"))
crystal_proofs = []
for entry in crystal_index["files"]:
    if entry["kind"] == "crystal" and entry["relative_path"] in {
        "Shared/Enums.cs", "Server/MirObjects/PlayerObject.cs", "Server/MirObjects/GuildObject.cs",
        "Server/MirNetwork/MirConnection.cs", "Client/MirScenes/GameScene.cs",
    }:
        assert sha(Path(entry["absolute_path"]).read_bytes()) == entry["sha256"]
        crystal_proofs.append({"path": entry["absolute_path"], "sha256": entry["sha256"]})
assert len(crystal_proofs) == 5

for relative, data in source_bytes.items():
    assert (ROOT / relative).read_bytes() == data, relative
assert git("status", "--porcelain=v1", "--untracked-files=all").stdout.decode("utf-8") == status
assert git("rev-parse", "HEAD").stdout.decode().strip() == BASE
git("diff", "--check")
manifest_files = []
for path in sorted(PROOF.rglob("*")):
    if path.is_file() and path.name not in {"manifest.json", "freeze-receipt.json"}:
        data = path.read_bytes()
        manifest_files.append({"path": path.relative_to(PROOF).as_posix(), "bytes": len(data), "sha256": sha(data)})
write_json(PROOF / "manifest.json", {"files": manifest_files})
receipt = {
    "scope": "P5 shared guild rank rename authority/API Candidate; six approved files only",
    "frozen_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
    "checkout": ROOT.as_posix(), "base_head": BASE, "observed_head": head,
    "source_files": final_inputs, "baseline_git_blobs": baseline_files,
    "crystal_verified_current_bytes": crystal_proofs,
    "frozen_prior_plan_sha256": sha((PLAN / "PLAN.md").read_bytes()),
    "git_status": status, "diff_check_exit": 0,
    "reverse_patch_check_exit": reverse_check.returncode,
    "patch_sha256": sha(patch), "manifest_sha256": sha((PROOF / "manifest.json").read_bytes()),
    "test_results_sha256": sha((PROOF / "test-results.json").read_bytes()),
    "green_and_adjacent_passed": 21, "gateway_red_passed": 2, "gateway_red_failed": 6,
    "red_actual_cargo_exit": 101,
    "red_external_postprint_note": "GBK console encoding error after raw log and receipt persisted; runner stdout encoding fixed; no RED rerun",
    "freeze_preflight_note": "First external freezer stopped before writes because porcelain paths included mir2-web3/; explicit prefix normalization added; source/test bytes untouched; original error retained",
    "no_commit_push_publish_network_or_player_store": True,
    "source_frozen": True,
    "open_gates": ["native typed status/pending", "partial rank UI delta", "routing terminal refresh", "AOI rank profile", "kick/promote/add/options/leave", "ordinary guild war", "ordinary kill Guild EXP producer/consumer", "native/network/PostgreSQL/human acceptance"],
}
write_json(PROOF / "freeze-receipt.json", receipt)
print(json.dumps({
    "freeze_receipt": str(PROOF / "freeze-receipt.json"), "patch": str(PROOF / "final.patch"),
    "manifest_sha256": receipt["manifest_sha256"], "patch_sha256": receipt["patch_sha256"],
    "freeze_receipt_sha256": sha((PROOF / "freeze-receipt.json").read_bytes()),
    "approved_files": len(FILES), "green_passed": green_total, "red_failed": 6,
}, indent=2))
