"""Offline V7 integration/CI source, Actions/package/map verification. No network."""
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

SCHEMA = "mir2.scene-candidate-artifact-verification.v7"
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


def non_native_delta(repo, pins):
    integration, revision = pins["integrationCommit"], pins["candidateSource"]
    require(git(repo, "rev-list", "--parents", "-n", "1", revision).decode().split() == [revision, integration], "ci_source_must_be_exact_direct_child_of_integration")
    changed = git(repo, "diff", "--name-only", integration, revision).decode().splitlines()
    require(changed == [NON_NATIVE_SCRIPT], "only_exact_reviewed_non_native_script_may_differ")
    actual = {"path": NON_NATIVE_SCRIPT, "integrationSha256": digest(git(repo, "show", integration + ":" + NON_NATIVE_SCRIPT)),
              "ciSha256": digest(git(repo, "show", revision + ":" + NON_NATIVE_SCRIPT))}
    require(actual == pins["canonicalNonNativeDelta"] and actual["integrationSha256"] != actual["ciSha256"], "explicit_non_native_old_new_hash_mismatch")
    objects = []
    for path in NATIVE_OBJECTS:
        before = git(repo, "rev-parse", integration + ":" + path).decode().strip()
        after = git(repo, "rev-parse", revision + ":" + path).decode().strip()
        require(before == after, "native_build_input_object_changed:" + path)
        objects.append({"path": path, "integrationObject": before, "ciObject": after})
    workflow = git(repo, "show", revision + ":" + WORKFLOW)
    native = re.search(rb"(?ms)^  build-playtest-wallet-gateway:\r?\n.*?(?=^  [A-Za-z0-9_-]+:\r?$|\Z)", workflow)
    require(native is not None and b"bash scripts/build-playtest-wallet-release.sh" in native.group(0) and
            NON_NATIVE_SCRIPT.encode() not in native.group(0) and PurePosixPath(NON_NATIVE_SCRIPT).name.encode() not in native.group(0), "native_job_must_not_invoke_changed_numeron_script")
    scopes = ["mir2-web3/apps/gateway", "mir2-web3/apps/simulation", "mir2-web3/packages", "mir2-web3/vendor", "mir2-web3/scripts"]
    query = subprocess.run(["git", "grep", "-n", "-F", PurePosixPath(NON_NATIVE_SCRIPT).name, revision, "--", *scopes], cwd=repo, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, check=False)
    require(query.returncode == 1 and not query.stdout, "changed_numeron_script_is_referenced_by_native_inputs")
    return {**actual, "exactWholeRepositoryDelta": changed, "nativeBuildInputObjects": objects,
            "nativeJobBodySha256": digest(native.group(0)), "nativeJobDirectCallExcluded": True,
            "nativeSourceAndScriptReferenceScopes": scopes, "nativeInputScriptReferenceExcluded": True}


def source_proof(repo, pins, source):
    revision, integration, baseline = pins["candidateSource"], pins["integrationCommit"], pins["baselineSource"]
    require(source.get("changesKind") == "spectator-scene-projection" and is_sha(source.get("baseRevision"), 40), "reviewed_scene_source_manifest_required")
    records = source.get("files")
    require(isinstance(records, list) and len(records) == len(SOURCE_PATHS) and {r.get("path") for r in records} == SOURCE_PATHS, "exact_scene_source_set_required")
    scopes = ["mir2-web3/apps/gateway", "mir2-web3/apps/simulation", "mir2-web3/packages", "mir2-web3/vendor/dubhe-network-core", "mir2-web3/Cargo.toml", "mir2-web3/Cargo.lock", MAP_ROOT]
    actual_parents = git(repo, "rev-list", "--parents", "-n", "1", integration).decode().split()
    require(actual_parents == [integration, pins["integrationParent"]], "integration_actual_single_parent_mismatch")
    require(not git(repo, "diff", "--name-only", baseline, pins["integrationParent"], "--", *scopes).strip(), "backend_between_baseline_and_actual_parent_changed")
    commit_delta = set(git(repo, "diff", "--name-only", pins["integrationParent"], integration).decode().splitlines())
    require(commit_delta == SOURCE_PATHS | {TOPIC_README}, "candidate_commit_must_be_exact_nine_backend_plus_topic_readme")
    changed = set(git(repo, "diff", "--name-only", baseline, integration, "--", *scopes).decode().splitlines())
    require(changed == SOURCE_PATHS, "candidate_backend_diff_outside_reviewed_scene_set")
    for row in records:
        require(is_sha(row.get("sha256")) and type(row.get("bytes")) is int, "source_file_pin_invalid")
        content = git(repo, "show", integration + ":" + row["path"])
        require(content == git(repo, "show", revision + ":" + row["path"]), "ci_source_differs_from_tested_backend_blob")
        if pins["sourceNormalization"] == "exact":
            require(len(content) == row["bytes"] and digest(content) == row["sha256"], "committed_source_blob_sha_mismatch")
        else:
            raw_root = Path(source.get("buildSourceSnapshot", ""))
            require(raw_root.is_absolute() and raw_root.drive.replace("\\\\?\\", "").lower() == "c:", "explicit_c_windows_source_root_required")
            raw_path = regular(raw_root.joinpath(*safe(row["path"]).parts), 4194304)
            raw = raw_path.read_bytes()
            require(len(raw) == row["bytes"] and digest(raw) == row["sha256"] and raw.replace(b"\r\n", b"\n") == content, "raw_windows_source_pin_or_only_crlf_normalization_mismatch")
    for protected in ["mir2-web3/packages", "mir2-web3/Cargo.toml", "mir2-web3/Cargo.lock", MAP_ROOT]:
        require(git(repo, "rev-parse", baseline + ":" + protected) == git(repo, "rev-parse", revision + ":" + protected), "protected_data_protocol_or_lock_changed")
    delta = non_native_delta(repo, pins)
    return {"baselineSource": baseline, "integrationParent": pins["integrationParent"], "integrationCommit": integration, "candidateSource": revision,
            "reviewedSourcePaths": sorted(changed), "exactIntegrationCommitDelta": sorted(commit_delta), "sourceNormalization": pins["sourceNormalization"],
            "protectedProtocolDataCargoAndMapsIdentical": True, "canonicalNonNativeDelta": delta}


def validate_pins(pins):
    require(pins.get("schema") == "mir2.scene-candidate-pins.v7" and pins.get("templateOnly") is False and pins.get("reviewedByOperator") is True, "actual_explicit_reviewed_pins_required")
    require(is_sha(pins.get("candidateSource"), 40) and is_sha(pins.get("baselineSource"), 40) and pins["candidateSource"] != pins["baselineSource"] and is_sha(pins.get("baselineBinarySha256")), "explicit_source_and_baseline_pins_required")
    require(is_sha(pins.get("integrationParent"), 40) and is_sha(pins.get("integrationCommit"), 40) and pins["integrationCommit"] != pins["candidateSource"] and pins.get("sourceNormalization") in ["exact", "crlf-to-lf"], "explicit_integration_parent_commit_and_normalization_required")
    delta = pins.get("canonicalNonNativeDelta", {})
    require(set(delta) == {"path", "integrationSha256", "ciSha256"} and delta.get("path") == NON_NATIVE_SCRIPT and is_sha(delta.get("integrationSha256")) and is_sha(delta.get("ciSha256")), "explicit_reviewed_non_native_delta_required")
    require(type(pins.get("runId")) is int and pins["runId"] > 0 and type(pins.get("nativeJobId")) is int and pins["nativeJobId"] > 0 and pins.get("workflow") == WORKFLOW and pins.get("branch") == BRANCH, "exact_run_job_branch_required")
    require(re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]{0,127}", pins.get("releaseTag", "")) is not None, "safe_release_tag_required")
    require(is_sha(pins.get("binary", {}).get("sha256")) and type(pins["binary"].get("bytes")) is int and 0 < pins["binary"]["bytes"] <= 268435456, "explicit_binary_pin_required")
    require(pins.get("workflowInputs") == {"release_tag": pins["releaseTag"], "playtest_wallet_no_docker": True, "publish_r2": False, "numeron_operation": "off"}, "build_only_workflow_inputs_required")


def verify(case_dir, pins_path, pins_sha, repo, output):
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
    require(integration.get("schema") == "mir2.comparable.code-integration.v1" and integration.get("canonicalCommit") == pins["integrationCommit"] and integration.get("previousCanonicalHead") == pins["integrationParent"] and set(integration.get("paths", [])) == SOURCE_PATHS | {TOPIC_README} and integration.get("sourceInputManifestSha256") == sha(source_path), "explicit_integration_evidence_mismatch")
    require(pins["sourceNormalization"] != "crlf-to-lf" or integration.get("normalizedLFSourceEqualsFrozenWindowsBuildInputs") is True, "reviewed_crlf_normalization_not_declared")
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
