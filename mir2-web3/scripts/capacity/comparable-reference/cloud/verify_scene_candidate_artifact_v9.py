"""Offline NEXT-candidate V9 template: unbound pins refuse before evidence writes. No network."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import stat
import subprocess
import tarfile
import zipfile

SCHEMA = "mir2.scene-candidate-artifact-verification.v9"
MAP_ROOT = "mir2-web3/apps/web/lib/generated/crystal-map-pack"
WORKFLOW = ".github/workflows/gateway-release.yml"
BRANCH = "codex/windows-player-journey"
TOPIC_README = "mir2-web3/docs/evidence/comparable-capacity-20261113/README.zh-CN.md"
NON_NATIVE_SCRIPT = ".github/scripts/numeron-r16-release.mjs"
NATIVE_OBJECTS = ["mir2-web3", ".github/workflows", "mir2-web3/apps/gateway", "mir2-web3/apps/simulation",
                  "mir2-web3/packages", "mir2-web3/vendor", "mir2-web3/Cargo.toml", "mir2-web3/Cargo.lock",
                  "mir2-web3/scripts", "mir2-web3/infra", MAP_ROOT]
GATES = {"schema_union", "monthly_card_postgres", "billing_postgres", "activities_postgres",
         "linux-release-security", "gateway-package", "gateway-postgres-startup"}
SOURCE_PATHS = {
    "mir2-web3/apps/gateway/src/routing.rs", "mir2-web3/apps/gateway/src/session.rs",
    "mir2-web3/apps/gateway/src/web.rs", "mir2-web3/apps/gateway/src/spectator.rs",
    "mir2-web3/apps/gateway/src/routing/tests/spectator_scene_projection_tests.rs",
    "mir2-web3/apps/simulation/src/runtime/packets.rs", "mir2-web3/apps/simulation/src/runtime/session.rs",
    "mir2-web3/apps/simulation/src/world_runtime.rs",
    "mir2-web3/apps/simulation/src/runtime/spectator_scene_snapshot_tests.rs",
}

CANDIDATE = "7403bc5ee5cbb472477db101b8be603136049dcf"
PARENT = "38a911769a5ec2d012c538dcb95f454ef1ca37c9"
BASELINE = "82efff2c8c0eef72a950501d84acad3c6c3de958"
BASELINE_BINARY = "6ed4ee817b2577858860a9bf53cea12845d5ab2cb0d855eb5ffa678ba9ccc01a"
SOURCE_INPUTS_SHA = "ba6199fe86a43715433ce563892ea14d964e480b78ae011aea34d6346259c401"
NATIVE_DELTA_PATHS_SHA = "eb11fd14837aa9ab49d882d01a1914c841cda4a2f87984c75f0edccc56f9bba8"
LIVE_MAP_SHA = "89bbbf17f63d34116ba53ccc6188aa7ebbf0de9b10ba2236f36fa24dba30e6fc"
LIVE_MAP_READER_SHA = "a78b9b8590fe55394af1ec43c5b31f8681280dbacdafa3bbd982e16bde5714f4"
NATIVE_SCOPES = ['mir2-web3/apps/gateway', 'mir2-web3/apps/simulation', 'mir2-web3/packages', 'mir2-web3/vendor', 'mir2-web3/Cargo.toml', 'mir2-web3/Cargo.lock', 'mir2-web3/scripts', 'mir2-web3/infra', '.github/workflows']
COMMIT_DELTA_PATHS = set(['mir2-web3/apps/gateway/src/routing.rs', 'mir2-web3/apps/gateway/src/routing/tests/accepted_attack_transform_tests.rs', 'mir2-web3/apps/gateway/src/routing/tests/zone_player_attack_mode_tests.rs', 'mir2-web3/apps/gateway/src/spectator.rs', 'mir2-web3/apps/gateway/src/spectator_capacity_profile_tests.rs', 'mir2-web3/apps/gateway/src/spectator_merge_owned_tests.rs', 'mir2-web3/apps/simulation/src/runtime/zone/runtime.rs', 'mir2-web3/docs/BACKEND-1TO1-PROGRESS.md', 'mir2-web3/docs/CRYSTAL-1TO1-ROADMAP.md', 'mir2-web3/docs/CRYSTAL-SERVER-PARITY.md'])



# TEMPLATE ONLY. Binding requires a separately reviewed new source freeze.
TEMPLATE_ONLY = False
CANDIDATE = '8f3099f4a0114a7825ff61db36b548d3bfb5b3fa'
PARENT = '63da6bd0d7fe34d582e9678698ac32904208e64b'
BASELINE = "7403bc5ee5cbb472477db101b8be603136049dcf"
BASELINE_BINARY = "26c131881c6c1e9de4ca2179d9f7a05f97d07f7e29269c80dca3dcd04cae1082"
SOURCE_INPUTS_SHA = 'c2c78cb75c00438c7b7e1c683e28d5599891e16f4f0ffea62a3656bd04d4e209'
NATIVE_DELTA_PATHS_SHA = 'f7f0ee32ffd70067ffdefb320ecc3f3d347acf8cbba93f156e8073d034fb598a'
LIVE_MAP_SHA = 'b44d5f3c250b508f8005f4157c35af9516bbf767abcba5c77722dc740083761e'
LIVE_MAP_READER_SHA = '3ca9db6b99a84d09e8d1d2020e014e80afa744a13cd83723b49a690a04b9be8e'
COMMIT_DELTA_PATHS = {'mir2-web3/apps/simulation/src/runtime/shared_guild_experience_tests.rs', 'mir2-web3/apps/simulation/src/runtime/shared_guild_experience.rs'}
EXPECTED_NATIVE_FILE_COUNT = 28
EXPECTED_RUN_ID = 43539916415
EXPECTED_NATIVE_JOB_ID = 132277833911
EXPECTED_BINARY_SHA256 = 'edd9f48cdb36ddd96334598aca4abc2f2e212e46ef2cbb60d4a1f5ab15a7ded5'
EXPECTED_BINARY_BYTES = 103748080
EXPECTED_SOURCE_CHANGES_KIND = "mentor-force-unused-lookup-only"


INTEGRATION = '63da6bd0d7fe34d582e9678698ac32904208e64b'
INTEGRATION_PARENT = 'e8b5e44d2953c60c98b78463320726df6c5e0d71'
CANDIDATE_DOCS_DELTA_PATHS = ['mir2-web3/docs/FRONTEND-1TO1-GAPS.md', 'mir2-web3/docs/evidence/r16-route-lookahead-20261114/README.md', 'mir2-web3/docs/evidence/r16-route-lookahead-20261114/local-verification.json', 'mir2-web3/docs/evidence/r16-route-lookahead-20261114/name-hit-after.txt', 'mir2-web3/docs/evidence/r16-route-lookahead-20261114/name-hit-before.txt', 'mir2-web3/docs/evidence/r16-route-lookahead-20261114/npc-hit-after.png', 'mir2-web3/docs/evidence/r16-route-lookahead-20261114/npc-hit-before.json', 'mir2-web3/docs/evidence/r16-route-lookahead-20261114/npc-hit-before.png', 'mir2-web3/docs/evidence/r16-route-lookahead-20261114/partial-route-r4.json', 'mir2-web3/docs/evidence/r16-route-lookahead-20261114/retained-attempts.json', 'mir2-web3/docs/evidence/r16-route-lookahead-20261114/route-after.txt', 'mir2-web3/docs/evidence/r16-route-lookahead-20261114/route-arrival-after.png', 'mir2-web3/docs/evidence/r16-route-lookahead-20261114/route-before.txt', 'mir2-web3/docs/evidence/r16-route-lookahead-20261114/route-regression.json', 'mir2-web3/docs/evidence/r16-route-lookahead-20261114/source-qualification.json', 'mir2-web3/docs/evidence/r16-route-lookahead-20261114/visual-review.json']
REQUIRED_SOURCE_PATHS = ['mir2-web3/apps/gateway/src/routing.rs', 'mir2-web3/apps/gateway/src/routing/tests/spectator_scene_projection_tests.rs', 'mir2-web3/apps/gateway/src/session.rs', 'mir2-web3/apps/gateway/src/spectator.rs', 'mir2-web3/apps/gateway/src/web.rs', 'mir2-web3/apps/simulation/src/runtime/packets.rs', 'mir2-web3/apps/simulation/src/runtime/session.rs', 'mir2-web3/apps/simulation/src/runtime/shared_guild_experience.rs', 'mir2-web3/apps/simulation/src/runtime/shared_guild_experience_tests.rs', 'mir2-web3/apps/simulation/src/runtime/spectator_scene_snapshot_tests.rs', 'mir2-web3/apps/simulation/src/world_runtime.rs']
TESTED_SOURCE_SHA256 = {'mir2-web3/apps/simulation/src/runtime/shared_guild_experience.rs': '318eae56c055957be8848d79f5b858bd199955a982034d9093120a015c93a998', 'mir2-web3/apps/simulation/src/runtime/shared_guild_experience_tests.rs': '9323797c2ca2886b32fd5192afb9982dbcc530a3d5bbd11a68524949fb09e919'}

def require_bound_candidate():
    require(TEMPLATE_ONLY is False, "template_only_next_candidate_unbound")
    require(all(isinstance(x, str) and re.fullmatch(r"[0-9a-f]{40}", x)
                for x in [CANDIDATE, PARENT]) and CANDIDATE != BASELINE,
            "exact_new_candidate_commit_and_parent_required")
    require(all(is_sha(x) for x in [SOURCE_INPUTS_SHA, NATIVE_DELTA_PATHS_SHA,
            LIVE_MAP_SHA, LIVE_MAP_READER_SHA, EXPECTED_BINARY_SHA256])
            and EXPECTED_BINARY_SHA256 != BASELINE_BINARY, "fresh_fixed_candidate_hashes_required")
    require(type(EXPECTED_RUN_ID) is int and EXPECTED_RUN_ID > 0
            and type(EXPECTED_NATIVE_JOB_ID) is int and EXPECTED_NATIVE_JOB_ID > 0,
            "fresh_fixed_native_run_job_required")
    require(type(EXPECTED_BINARY_BYTES) is int and 0 < EXPECTED_BINARY_BYTES <= 268435456,
            "fresh_fixed_elf_size_required")
    require(isinstance(COMMIT_DELTA_PATHS, set) and COMMIT_DELTA_PATHS
            and all(isinstance(p, str) and p for p in COMMIT_DELTA_PATHS)
            and type(EXPECTED_NATIVE_FILE_COUNT) is int and EXPECTED_NATIVE_FILE_COUNT > 0,
            "fresh_exact_native_source_paths_required")


class Refusal(Exception):
    pass


def require(condition, code):
    if not condition:
        raise Refusal(code)


def digest(value):
    return hashlib.sha256(value).hexdigest()


def is_sha(value, length=64):
    return isinstance(value, str) and re.fullmatch("[0-9a-f]{" + str(length) + "}", value) is not None


def sha(path):
    value = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for block in iter(lambda: stream.read(1048576), b""):
            value.update(block)
    return value.hexdigest()


def regular(path, maximum):
    path = Path(path).absolute()
    for item in [path, *path.parents]:
        require(not item.is_symlink() and not (hasattr(item, "is_junction") and item.is_junction()), "linked_input_path")
    info = path.stat()
    require(path.is_file() and info.st_nlink == 1 and 0 < info.st_size <= maximum, "bounded_regular_input_required")
    return path


def json_file(path, maximum=4194304):
    path = regular(path, maximum)
    value = json.loads(path.read_bytes())
    require(isinstance(value, dict), "json_object_required")
    return value


def safe(name):
    require(isinstance(name, str) and "\\" not in name and ":" not in name and "\0" not in name, "unsafe_archive_name")
    value = PurePosixPath(name)
    require(not value.is_absolute() and value.parts and all(p not in ["", ".", ".."] for p in name.rstrip("/").split("/")), "unsafe_archive_name")
    return value


def local(root, row, maximum=4194304):
    require(isinstance(row, dict) and is_sha(row.get("sha256")), "explicit_input_sha_required")
    path = root.joinpath(*safe(row.get("path")).parts)
    regular(path, maximum)
    require(sha(path) == row["sha256"], "input_sha_mismatch")
    return path


def exclusive_output(value, repo=None):
    path = Path(value).absolute()
    require(path.drive.replace("\\\\?\\", "").lower() == "c:", "c_only_output_required")
    require(not path.exists(), "exclusive_output_required")
    parent = path.parent.resolve(strict=True)
    for item in [path.parent, *path.parent.parents]:
        require(not item.is_symlink() and not (hasattr(item, "is_junction") and item.is_junction()), "linked_output_parent")
    if repo is not None:
        require(not parent.is_relative_to(Path(repo).resolve()), "outside_repository_output_required")
    return parent / path.name


def write_new(path, value):
    with Path(path).open("x", encoding="utf8", newline="\n") as stream:
        stream.write(json.dumps(value, indent=2) + "\n")


def unzip(source, destination):
    destination.mkdir(mode=0o700)
    seen, total = set(), 0
    with zipfile.ZipFile(source) as archive:
        for entry in archive.infolist():
            name = safe(entry.filename)
            key = name.as_posix().casefold()
            require(key not in seen and len(seen) < 16384, "duplicate_or_too_many_zip_members")
            seen.add(key)
            kind = stat.S_IFMT(entry.external_attr >> 16)
            require(not entry.flag_bits & 1 and kind in [0, stat.S_IFREG, stat.S_IFDIR], "zip_link_or_encryption_rejected")
            require(0 <= entry.file_size <= 268435456, "zip_member_too_large")
            total += entry.file_size
            require(total <= 536870912, "zip_expansion_bound_exceeded")
            target = destination.joinpath(*name.parts)
            if entry.is_dir():
                target.mkdir(parents=True, exist_ok=True, mode=0o700)
                continue
            target.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
            count = 0
            with archive.open(entry) as src, target.open("xb") as dst:
                for block in iter(lambda: src.read(1048576), b""):
                    count += len(block)
                    require(count <= entry.file_size, "zip_member_size_changed")
                    dst.write(block)
            require(count == entry.file_size, "zip_member_size_mismatch")


def unpack_release(source, destination):
    destination.mkdir(mode=0o700)
    seen = set()
    with tarfile.open(source, "r:gz") as archive:
        for entry in archive:
            require(entry.name in {"mir2-gateway", "zone_host", "README.txt", "RELEASE.json"} and entry.name not in seen and entry.isfile(), "release_tar_member_rejected")
            seen.add(entry.name)
            require(0 < entry.size <= (65536 if entry.name.endswith((".json", ".txt")) else 268435456), "release_tar_member_too_large")
            count = 0
            with archive.extractfile(entry) as src, (destination / entry.name).open("xb") as dst:
                for block in iter(lambda: src.read(1048576), b""):
                    count += len(block)
                    require(count <= entry.size, "release_tar_member_size_changed")
                    dst.write(block)
            require(count == entry.size, "release_tar_member_size_mismatch")
    require(seen == {"mir2-gateway", "zone_host", "README.txt", "RELEASE.json"}, "complete_release_members_required")


def only(root, name):
    require(safe(name).name == name, "flat_expected_filename_required")
    found = [p for p in root.rglob(name) if p.is_file()]
    require(len(found) == 1, "exactly_one_artifact_member_required:" + name)
    return found[0]


def git(repo, *args):
    process = subprocess.run(["git", *args], cwd=repo, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, check=False)
    require(process.returncode == 0, "local_git_object_query_failed")
    return process.stdout


def git_maps(repo, revision):
    entries = []
    for raw in git(repo, "ls-tree", "-rz", revision, "--", MAP_ROOT).split(b"\0"):
        if not raw:
            continue
        meta, name = raw.split(b"\t", 1)
        mode, kind, oid = meta.split()
        require(kind == b"blob" and mode in [b"100644", b"100755"], "git_map_not_regular_blob")
        entries.append((PurePosixPath(name.decode()).relative_to(MAP_ROOT).as_posix(), oid))
    require(0 < len(entries) <= 16384, "git_map_count_invalid")
    result = []
    process = subprocess.Popen(["git", "cat-file", "--batch"], cwd=repo, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
    try:
        for name, oid in entries:
            process.stdin.write(oid + b"\n"); process.stdin.flush()
            header = process.stdout.readline().split()
            require(len(header) == 3 and header[0] == oid and header[1] == b"blob", "git_map_batch_header_invalid")
            size = int(header[2]); require(0 <= size <= 16777216, "git_map_too_large")
            content = process.stdout.read(size)
            require(len(content) == size and process.stdout.read(1) == b"\n" and not content.startswith(b"version https://git-lfs.github.com/spec/v1"), "git_map_missing_or_lfs_pointer")
            result.append({"path": name, "bytes": size, "sha256": digest(content)})
        process.stdin.close()
        require(process.wait(timeout=10) == 0, "git_map_batch_failed")
    finally:
        if process.stdin and not process.stdin.closed:
            process.stdin.close()
        if process.poll() is None:
            process.kill(); process.wait()
        process.stdout.close()
    return sorted(result, key=lambda row: row["path"])


def native_commit_identity(repo, pins, source):
    require_bound_candidate()
    revision = pins["candidateSource"]
    require(git(repo, "rev-list", "--parents", "-n", "1", revision).decode().split() == [CANDIDATE, PARENT], "candidate_exact_single_parent_required")
    require(git(repo, "rev-list", "--parents", "-n", "1", INTEGRATION).decode().split() == [INTEGRATION, INTEGRATION_PARENT], "integration_exact_single_parent_required")
    changed = git(repo, "diff", "--name-only", INTEGRATION_PARENT, INTEGRATION).decode().splitlines()
    require(set(changed) == COMMIT_DELTA_PATHS and len(changed) == len(COMMIT_DELTA_PATHS), "exact_reviewed_integration_commit_delta_required")
    docs = git(repo, "diff", "--name-only", INTEGRATION, revision).decode().splitlines()
    require(docs == CANDIDATE_DOCS_DELTA_PATHS and all(p.startswith("mir2-web3/docs/") for p in docs), "exact_candidate_docs_delta_required")
    require(not git(repo, "diff", "--name-only", INTEGRATION, revision, "--", *NATIVE_SCOPES), "compiled_inputs_changed_after_tested_integration")
    rows = source.get("nativeGitObjects")
    require(isinstance(rows, list) and [r.get("path") for r in rows] == NATIVE_OBJECTS, "all_native_git_object_pins_required")
    actual = [{"path": path, "gitObject": git(repo, "rev-parse", revision + ":" + path).decode().strip()} for path in NATIVE_OBJECTS]
    require(rows == actual, "native_git_object_pin_mismatch")
    workflow = git(repo, "show", revision + ":" + WORKFLOW)
    native = re.search(rb"(?ms)^  build-playtest-wallet-gateway:\r?\n.*?(?=^  [A-Za-z0-9_-]+:\r?$|\Z)", workflow)
    require(native is not None and b"bash scripts/build-playtest-wallet-release.sh" in native.group(0), "exact_native_build_script_job_required")
    return {"candidateSourceIsIntegrationCommit": False, "candidateParent": PARENT,
            "integrationCommit": INTEGRATION, "integrationParent": INTEGRATION_PARENT,
            "integrationIsExactParentOfCandidate": True, "compiledInputsIdenticalToIntegration": True,
            "exactWholeRepositoryIntegrationDelta": sorted(changed), "exactWholeRepositoryCommitDelta": docs,
            "nativeBuildInputObjects": actual, "nativeJobBodySha256": digest(native.group(0)),
            "nonNativeDeltaClaimed": True}


def source_proof(repo, pins, source):
    require_bound_candidate()
    require(source.get("candidateDocsDeltaPaths") == CANDIDATE_DOCS_DELTA_PATHS
            and source.get("integrationCommitDeltaPaths") == sorted(COMMIT_DELTA_PATHS), "source_declared_commit_deltas_mismatch")
    revision, integration, baseline = pins["candidateSource"], pins["integrationCommit"], pins["baselineSource"]
    require(source.get("schema") == "mir2.comparable.exact-native-source-inputs.v2" and source.get("changesKind") == EXPECTED_SOURCE_CHANGES_KIND and source.get("sourceRevision") == revision and source.get("baselineSource") == baseline and source.get("integrationParent") == INTEGRATION_PARENT and source.get("integrationCommit") == INTEGRATION and source.get("candidateParent") == PARENT and source.get("sourceNormalization") == "exact", "exact_current_source_manifest_required")
    require(source.get("nativeDeltaScopes") == NATIVE_SCOPES, "complete_native_scope_required")
    changed = git(repo, "diff", "--name-only", baseline, revision, "--", *NATIVE_SCOPES).decode().splitlines()
    require(digest(json.dumps(changed, separators=(",", ":")).encode()) == NATIVE_DELTA_PATHS_SHA and source.get("nativeScopedDeltaPaths") == changed, "exact_cumulative_native_delta_required")
    records = source.get("files")
    require(isinstance(records, list) and len(records) == len(changed) == EXPECTED_NATIVE_FILE_COUNT and [r.get("path") for r in records] == changed, "all_exact_reviewed_native_scope_file_pins_required")
    for row in records:
        require(is_sha(row.get("sha256")) and type(row.get("bytes")) is int, "source_file_pin_invalid")
        content = git(repo, "show", revision + ":" + row["path"])
        require(len(content) == row["bytes"] and digest(content) == row["sha256"], "committed_source_blob_sha_mismatch")
    required = source.get("requiredSourceFiles")
    require(isinstance(required, list) and [r.get("path") for r in required] == REQUIRED_SOURCE_PATHS, "all_prior_scene_and_new_tested_source_pins_required")
    for row in required:
        content = git(repo, "show", revision + ":" + row["path"])
        require(type(row.get("bytes")) is int and row["bytes"] == len(content) and row.get("sha256") == digest(content), "required_source_pin_mismatch")
    require(source.get("testedProductionAndTestsSha256") == TESTED_SOURCE_SHA256, "exact_tested_fix_source_pins_required")
    for path, expected in TESTED_SOURCE_SHA256.items():
        require(digest(git(repo, "show", revision + ":" + path)) == expected and digest(git(repo, "show", INTEGRATION + ":" + path)) == expected, "tested_fix_source_differs_from_candidate")
    protected = ["mir2-web3/packages", "mir2-web3/vendor", "mir2-web3/Cargo.toml", "mir2-web3/Cargo.lock", MAP_ROOT, WORKFLOW, "mir2-web3/scripts/build-playtest-wallet-release.sh"]
    for path in protected:
        require(git(repo, "rev-parse", baseline + ":" + path) == git(repo, "rev-parse", revision + ":" + path), "protected_data_protocol_lock_workflow_or_packaging_changed:" + path)
    identity = native_commit_identity(repo, pins, source)
    return {"baselineSource": baseline, "integrationParent": INTEGRATION_PARENT, "integrationCommit": integration,
            "candidateSource": revision, "reviewedSourcePaths": changed,
            "exactIntegrationCommitDelta": sorted(COMMIT_DELTA_PATHS), "sourceNormalization": "exact",
            "candidateDocsDeltaPaths": CANDIDATE_DOCS_DELTA_PATHS, "requiredSourceFiles": required, "testedProductionAndTestsSha256": TESTED_SOURCE_SHA256,
            "protectedProtocolDataCargoAndMapsIdentical": True,
            "protectedWorkflowAndPackagingScriptIdentical": True, "nativeInputIdentity": identity}


def validate_pins(pins):
    require_bound_candidate()
    require(pins.get("schema") == "mir2.scene-candidate-pins.v9" and pins.get("templateOnly") is False and pins.get("reviewedByOperator") is True, "actual_explicit_reviewed_pins_required")
    require(pins.get("candidateSource") == CANDIDATE and pins.get("integrationCommit") == INTEGRATION and pins.get("integrationParent") == INTEGRATION_PARENT and pins.get("baselineSource") == BASELINE and pins.get("baselineBinarySha256") == BASELINE_BINARY and pins.get("sourceNormalization") == "exact", "exact_current_source_and_baseline_pins_required")
    require(pins.get("sourceInputs", {}).get("sha256") == SOURCE_INPUTS_SHA, "fixed_current_source_manifest_pin_required")
    require(pins.get("maps") == {"fileCount": 1620, "totalFileBytes": 42111309}, "original_complete_map_pins_required")
    require(pins.get("liveMaps", {}).get("sha256") == LIVE_MAP_SHA and pins["liveMaps"].get("readerSourceSha256") == LIVE_MAP_READER_SHA, "original_actual_live_map_and_reader_pins_required")
    require(type(pins.get("runId")) is int and pins["runId"] == EXPECTED_RUN_ID and type(pins.get("nativeJobId")) is int and pins["nativeJobId"] == EXPECTED_NATIVE_JOB_ID and pins.get("workflow") == WORKFLOW and pins.get("branch") == BRANCH, "exact_run_job_branch_required")
    require(re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]{0,127}", pins.get("releaseTag", "")) is not None, "safe_release_tag_required")
    require(is_sha(pins.get("binary", {}).get("sha256")) and type(pins["binary"].get("bytes")) is int and 0 < pins["binary"]["bytes"] <= 268435456, "explicit_binary_pin_required")
    require(pins["binary"]["sha256"] == EXPECTED_BINARY_SHA256 and pins["binary"]["bytes"] == EXPECTED_BINARY_BYTES, "fixed_actual_new_elf_required")
    require(pins.get("workflowInputs") == {"release_tag": pins["releaseTag"], "playtest_wallet_no_docker": True, "publish_r2": False, "numeron_operation": "off"}, "build_only_workflow_inputs_required")


def verify(case_dir, pins_path, pins_sha, repo, output):
    require_bound_candidate()
    root, repo = Path(case_dir).resolve(strict=True), Path(repo).resolve(strict=True)
    require(is_sha(pins_sha) and sha(regular(pins_path, 1048576)) == pins_sha, "reviewed_pins_sha_mismatch")
    pins = json_file(pins_path, 1048576); validate_pins(pins)
    api_path = local(root, pins["actionsMetadata"]); api = json_file(api_path)
    run, job = api.get("workflowRun", {}), api.get("nativeJob", {})
    require(run.get("id") == pins["runId"] and run.get("head_sha") == pins["candidateSource"] and run.get("head_branch") == BRANCH and run.get("path") == WORKFLOW, "authenticated_run_metadata_mismatch")
    require(run.get("status") == "completed" and run.get("conclusion") == "success", "whole_workflow_must_be_completed_success")
    require(job.get("id") == pins["nativeJobId"] and job.get("run_id") == pins["runId"] and job.get("head_sha") == pins["candidateSource"] and job.get("name") == "build-playtest-wallet-gateway" and job.get("status") == "completed" and job.get("conclusion") == "success", "native_job_not_exact_success")
    artifacts = {}
    for role, prefix in [("gateway", "mir2-playtest-wallet-gateway-linux-x64-"), ("nativeEvidence", "mir2-playtest-wallet-build-evidence-")]:
        row = pins["artifacts"][role]
        require(type(row.get("id")) is int and row["id"] > 0 and row.get("name") == prefix + pins["candidateSource"], "explicit_artifact_identity_required")
        matches = [r for r in api.get("artifacts", []) if r.get("id") == row["id"]]
        require(len(matches) == 1, "artifact_id_not_unique_in_metadata")
        actual = matches[0]; origin = actual.get("workflow_run", {})
        require(actual.get("name") == row["name"] and actual.get("digest") == "sha256:" + row["sha256"] and actual.get("expired") is False and origin.get("id") == pins["runId"] and origin.get("head_sha") == pins["candidateSource"] and origin.get("head_branch") == BRANCH, "artifact_digest_or_origin_mismatch")
        artifacts[role] = local(root, row, 268435456)
        require(actual.get("size_in_bytes") == artifacts[role].stat().st_size, "artifact_download_size_mismatch")
    source_path = local(root, pins["sourceInputs"]); source = json_file(source_path)
    compatibility = source_proof(repo, pins, source)
    integration_path = local(root, pins["integrationEvidence"]); integration = json_file(integration_path)
    require(integration.get("schema") == "mir2.comparable.code-integration.v2" and integration.get("canonicalCommit") == pins["integrationCommit"] and integration.get("previousCanonicalHead") == pins["integrationParent"] and integration.get("candidateSource") == CANDIDATE and integration.get("candidateParent") == PARENT and integration.get("candidateDocsDeltaPaths") == CANDIDATE_DOCS_DELTA_PATHS and integration.get("compiledInputScopes") == NATIVE_SCOPES and integration.get("compiledInputsIdenticalFromIntegrationToCandidate") is True and set(integration.get("paths", [])) == COMMIT_DELTA_PATHS and integration.get("sourceInputManifestSha256") == sha(source_path), "explicit_integration_evidence_mismatch")
    require(integration.get("exactCommittedSourceManifest") is True, "explicit_clean_committed_source_binding_required")
    live_path = local(root, pins["liveMaps"]); live = json_file(live_path)
    require(live.get("schema") == "mir2.scene-live-map-proof.v6" and live.get("sourceRevision") == pins["baselineSource"] and live.get("binarySha256") == pins["baselineBinarySha256"] and live.get("identityBefore") == live.get("identityAfter") and type(live.get("identityAfter", {}).get("pid")) is int and live["identityAfter"]["pid"] > 1 and live.get("complete") is True, "complete_actual_live_map_proof_required")
    require(is_sha(pins["liveMaps"].get("readerSourceSha256")) and live.get("readerSourceSha256") == pins["liveMaps"]["readerSourceSha256"], "live_map_reader_not_pinned")
    destination = exclusive_output(output, repo); destination.mkdir(mode=0o700)
    bundle, evidence, release = [destination / name for name in ["gateway-bundle", "native-evidence", "release"]]
    unzip(artifacts["gateway"], bundle); unzip(artifacts["nativeEvidence"], evidence)
    package = only(bundle, "mir2-gateway-linux-x64-" + pins["releaseTag"] + ".tar.gz")
    require(sha(package) == pins["packageSha256"], "reviewed_package_sha_mismatch")
    unpack_release(package, release)
    manifest_path = release / "RELEASE.json"; manifest = json_file(manifest_path, 65536); binary = regular(release / "mir2-gateway", 268435456)
    require(manifest.get("name") == "mir2-gateway" and manifest.get("gitRevision") == pins["candidateSource"] and manifest.get("gitDirty") is False and manifest.get("binarySha256") == pins["binary"]["sha256"] == sha(binary) and manifest.get("binarySizeBytes") == pins["binary"]["bytes"] == binary.stat().st_size, "release_source_binary_or_cleanliness_mismatch")
    with binary.open("rb") as stream:
        header = stream.read(64)
    require(header[:7] == b"\x7fELF\x02\x01\x01" and header[16:18] in [b"\x02\x00", b"\x03\x00"] and header[18:24] == b"\x3e\x00\x01\x00\x00\x00", "linux_x64_elf_required")
    startup = json_file(only(evidence, "gateway-postgres-startup.json")); gates = json_file(only(bundle, "playtest-release-verification.json"))
    require(startup.get("sourceRevision") == pins["candidateSource"] and startup.get("binarySha256") == pins["binary"]["sha256"] and startup.get("passed") is True and startup.get("actualReleaseSubprocess") is True and type(startup.get("gatewayExitCode")) is int and startup["gatewayExitCode"] == 0 and type(startup.get("redisExitCode")) is int and startup["redisExitCode"] == 0, "actual_native_startup_not_proven")
    require(gates.get("sourceRevision") == pins["candidateSource"] and isinstance(gates.get("passed"), list) and len(gates["passed"]) == 7 and set(gates["passed"]) == GATES and gates.get("buildEnvironment") == "native-linux-non-root" and gates.get("productionPublish") is False, "seven_native_gates_required")
    maps = json_file(only(bundle, "crystal-map-pack-manifest.json"))
    require(maps.get("sourceRevision") == pins["candidateSource"] and maps.get("sourceTree") == pins["candidateSource"] + ":" + MAP_ROOT, "map_manifest_source_mismatch")
    archive = only(bundle, safe(maps.get("archive")).name)
    require(sha(archive) == maps.get("archiveSha256") and archive.stat().st_size == maps.get("archiveBytes"), "map_archive_sha_size_mismatch")
    actual, seen, total = [], set(), 0
    with tarfile.open(archive, "r:gz") as source_archive:
        for member in source_archive:
            name = safe(member.name); require(name.is_relative_to("crystal-map-pack") and name.as_posix().casefold() not in seen and len(seen) < 16384, "map_prefix_duplicate_or_count_invalid")
            seen.add(name.as_posix().casefold())
            if member.isdir():
                continue
            require(member.isfile() and 0 <= member.size <= 16777216, "map_member_not_regular_or_too_large")
            total += member.size; require(total <= 629145600, "map_expansion_bound_exceeded")
            content = source_archive.extractfile(member).read(member.size + 1)
            require(len(content) == member.size and not content.startswith(b"version https://git-lfs.github.com/spec/v1"), "map_size_or_lfs_pointer_rejected")
            actual.append({"path": member.name, "bytes": len(content), "sha256": digest(content)})
    actual.sort(key=lambda row: row["path"])
    require(actual == maps.get("files") and len(actual) == maps.get("fileCount") == pins.get("maps", {}).get("fileCount") and total == maps.get("totalFileBytes") == pins["maps"].get("totalFileBytes"), "all_map_members_must_match_manifest_and_pin")
    normalized = [{**r, "path": PurePosixPath(r["path"]).relative_to("crystal-map-pack").as_posix()} for r in actual]
    require(normalized == git_maps(repo, pins["candidateSource"]) == live.get("files"), "all_git_artifact_live_maps_must_match")
    result = {"schema": SCHEMA, "passed": True, "verifiedAtUtc": datetime.now(timezone.utc).isoformat(), "candidateRevision": pins["candidateSource"], "integrationCommit": pins["integrationCommit"], "baselineRevision": pins["baselineSource"], "baselineBinarySha256": pins["baselineBinarySha256"], "runId": pins["runId"], "nativeJobId": pins["nativeJobId"], "wholeWorkflowStatus": "completed", "wholeWorkflowConclusion": "success", "artifacts": pins["artifacts"], "binarySha256": pins["binary"]["sha256"], "binarySizeBytes": pins["binary"]["bytes"], "artifactDirectory": str(release), "releaseManifestSha256": sha(manifest_path), "candidatePinsSha256": pins_sha, "actionsMetadataSha256": sha(api_path), "sourceInputManifestSha256": sha(source_path), "sourceCompatibility": compatibility, "maps": {"fileCount": len(actual), "totalFileBytes": total, "allGitArtifactLiveMatch": True, "liveProofSha256": sha(live_path)}, "nativeRequiredGates": sorted(GATES), "nativeStartupActualExitCode": 0, "verifierSourceSha256": sha(__file__), "deploymentApproved": False, "capacityQualified": False}
    write_new(destination / "candidate-verification.json", result)
    return result


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--case-dir", type=Path, required=True)
    parser.add_argument("--pins", type=Path, required=True)
    parser.add_argument("--pins-sha256", required=True)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args(argv)
    value = verify(args.case_dir, args.pins, args.pins_sha256, args.repo, args.output)
    print(json.dumps({"passed": value["passed"], "candidateRevision": value["candidateRevision"], "deploymentApproved": False}))


if __name__ == "__main__":
    try:
        main()
    except (Refusal, OSError, ValueError, KeyError, TypeError, tarfile.TarError, zipfile.BadZipFile) as error:
        print(json.dumps({"passed": False, "errorClass": type(error).__name__, "error": str(error) if isinstance(error, Refusal) else "invalid_or_incomplete_local_evidence"}))
        raise SystemExit(1)
