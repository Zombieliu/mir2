#!/usr/bin/env python3
"""Prepare a bounded immutable object closure or exact R2 publisher plan.

No CMS, signing, secrets, cloud access or publishing. Root's external CMS gate
first consumes objects-only output, then writes a bound verification attestation.
Candidate game bytes and the signed feed are preserved, including directory URLs.
"""
from __future__ import annotations

import argparse
import importlib.util
import json
import os
from pathlib import Path
import sys
import time

sys.dont_write_bytecode = True
def load(name, filename):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).absolute().with_name(filename))
    result = importlib.util.module_from_spec(spec)
    sys.modules[name] = result
    spec.loader.exec_module(result)
    return result

verify = load("mir2_publication_delivery_verify", "verify-native-delivery.py")
publisher = load("mir2_publication_publisher", "publish-native-r2.py")
archive = verify.archive
require = archive.require
CLOSURE_SCHEMA = "mir2.windows.r2-publication-closure.v1"


def entry_dict(entry, path):
    publisher.relative_path(path)
    require(entry.size <= publisher.object_limit(path), "publication object exceeds delivery limit")
    return {"path": path, "source": str(entry.source), "size": entry.size, "sha256": entry.sha256.lower()}


def check_metadata(component, expected):
    verify.exact(component, {"directory", "identity", "metadata"}, "feed component fields")
    archive.relative_path(component["directory"])
    require(component["directory"].startswith("releases/"), "feed release directory")
    publisher.relative_path(component["directory"] + "/placeholder")
    require(isinstance(component["metadata"], list) and len(component["metadata"]) == len(expected),
            "feed metadata count")
    names = set()
    for item in component["metadata"]:
        verify.exact(item, {"path", "size", "sha256"}, "feed metadata fields")
        name = item["path"]
        require(isinstance(name, str) and name in expected and name not in names, "feed metadata set")
        names.add(name)
        require(type(item["size"]) is int and item["size"] == expected[name].size
                and item["sha256"] == expected[name].sha256, "feed metadata byte binding")


def check_feed(data, game_files, game_identity, engine_files, engine_identity):
    value = archive.json_bytes(data, "latest.json", 32768)
    verify.exact(value, {"schema", "channel", "platform", "sequence", "createdUnix", "expiresUnix",
                        "minBootstrap", "protocol", "content", "game", "engine"}, "feed fields")
    require(value["schema"] == "mir2.windows.update-feed.v1" and value["channel"] == "invited"
            and value["platform"] == "windows-x64" and value["protocol"] == "crystal-mir2-v1"
            and value["content"] == "mir2.windows.package-manifest.v4", "feed compatibility")
    archive.uint(value["sequence"], 9007199254740991, "feed sequence", positive=True)
    archive.uint(value["minBootstrap"], 1, "minimum bootstrap")
    now = int(time.time())
    archive.uint(value["createdUnix"], now + 300, "feed creation", positive=True)
    archive.uint(value["expiresUnix"], 2**63 - 1, "feed expiry", positive=True)
    require(value["expiresUnix"] >= now and 0 < value["expiresUnix"] - value["createdUnix"] <= 31 * 86400,
            "feed validity")
    check_metadata(value["game"], {name: game_files[name] for name in archive.META})
    check_metadata(value["engine"], {name: engine_files[name] for name in ("ENGINE.json", "ENGINE.p7s")})
    require(value["game"]["identity"] == game_identity["identity"]
            and value["engine"]["identity"] == engine_identity["identity"], "feed component identity")
    a, b = value["game"]["directory"].lower(), value["engine"]["directory"].lower()
    require(a != b and not a.startswith(b + "/") and not b.startswith(a + "/"), "overlapping component directories")
    return value


def build_closure(candidate_root, delivery_root, delivery_verification, engine_root, feed_root,
                  expected_current=None, bootstrap_exe=None, include_root_files=False):
    game_root, game_files, identity, version, manifest_bytes, version_bytes = verify.candidate_binding(candidate_root)
    delivery_root = archive.absolute_directory(str(delivery_root))
    descriptor = verify.file_entry(delivery_root / "DELIVERY.json", "DELIVERY.json", archive.MAX_META)
    descriptor_bytes = verify.read_entry(descriptor, archive.MAX_META)
    delivery = verify.descriptor_binding(descriptor_bytes, game_files, identity, manifest_bytes, version_bytes)
    signature = verify.file_entry(delivery_root / "DELIVERY.p7s", "DELIVERY.p7s", archive.MAX_META)
    proof_entry = verify.file_entry(delivery_verification, "verification", archive.MAX_META)
    proof = archive.json_bytes(verify.read_entry(proof_entry, archive.MAX_META), "delivery verification")
    declared = [b["archive"] for b in delivery["bundles"]] + [p["patch"] for p in delivery["patches"]]
    require(proof.get("schema") == verify.SCHEMA and proof.get("passed") is True
            and proof.get("cmsVerified") is False and proof.get("requiresExternalSignatureVerification") is True
            and proof.get("candidate") == identity["identity"] and proof.get("sourceRevision") == version["gitRevision"]
            and proof.get("manifestSha256") == archive.sha256(manifest_bytes)
            and proof.get("versionSha256") == archive.sha256(version_bytes)
            and proof.get("deliverySha256") == descriptor.sha256
            and proof.get("checkedArtifacts") == declared
            and proof.get("metadata") == [{"path": name, "size": game_files[name].size,
                                          "sha256": game_files[name].sha256} for name in archive.META],
            "passed exact delivery byte proof required")
    engine_root = archive.absolute_directory(str(engine_root))
    require({child.name for child in engine_root.iterdir()} == archive.ENGINE_FILES,
            "engine directory must contain exactly three signed release files")
    engine_files = {name: verify.file_entry(engine_root / name, name,
                    publisher.MAX_ARTIFACT if name.endswith(".exe") else archive.MAX_META) for name in archive.ENGINE_FILES}
    engine_identity = archive.validate_engine(engine_files)
    engine = archive.json_bytes(verify.read_entry(engine_files["ENGINE.json"], 32768), "ENGINE.json", 32768)
    require(engine["builtUnix"] <= int(time.time()) + 300, "future engine build")
    feed_root = archive.absolute_directory(str(feed_root))
    feed_entry = verify.file_entry(feed_root / "latest.json", "latest.json", 32768)
    feed_signature = verify.file_entry(feed_root / "latest.p7s", "latest.p7s", archive.MAX_META)
    feed = check_feed(verify.read_entry(feed_entry, 32768), game_files, identity, engine_files, engine_identity)
    candidate = publisher.validate_pointer({"schema": publisher.POINTER_SCHEMA, "channel": "invited",
        "platform": "windows-x64", "sequence": feed["sequence"], "sourceRevision": engine["sourceRevision"].lower(),
        "feedSha256": feed_entry.sha256.lower(), "signatureSha256": feed_signature.sha256.lower()})
    prior = None
    if expected_current is not None:
        previous = verify.file_entry(expected_current, "expected channel", publisher.MAX_POINTER)
        prior = publisher.validate_pointer(publisher.parse_json(verify.read_entry(previous, publisher.MAX_POINTER)))
        require(candidate["sequence"] > prior["sequence"], "publication sequence downgrade or reuse")
    game_directory, engine_directory = feed["game"]["directory"], feed["engine"]["directory"]
    objects = []
    def add(entry, path):
        objects.append(entry_dict(entry, path))
    for name in archive.META:
        add(game_files[name], game_directory + "/" + name)
    root_names = {"mir2-platform-windows.exe", "BUILD-ATTESTATION.json", "mir2-client.toml"}
    if include_root_files:
        root_names |= {name for name in game_files if "/" not in name and name not in archive.META}
    require(root_names <= set(game_files), "required game manual-repair files absent from manifest")
    for name in sorted(root_names):
        expected = game_files[name]
        actual = verify.file_entry(game_root / name, name,
                  publisher.MAX_ARTIFACT if name.endswith(".exe") else archive.MAX_META)
        require(actual.size == expected.size and actual.sha256 == expected.sha256,
                "game root payload differs from signed manifest")
        add(actual, game_directory + "/" + name)
    add(descriptor, game_directory + "/DELIVERY.json")
    add(signature, game_directory + "/DELIVERY.p7s")
    for item in declared:
        actual = verify.file_entry(delivery_root / item["path"], item["path"], publisher.MAX_ARTIFACT)
        require(actual.size == item["size"] and actual.sha256 == item["sha256"], "delivery artifact changed since byte proof")
        add(actual, game_directory + "/" + item["path"])
    for name in sorted(archive.ENGINE_FILES):
        add(engine_files[name], engine_directory + "/" + name)
    directory = publisher.feed_directory(candidate)
    add(feed_entry, directory + "latest.json")
    add(feed_signature, directory + "latest.p7s")
    if bootstrap_exe is not None:
        bootstrap = verify.file_entry(bootstrap_exe, "bootstrap", publisher.MAX_ARTIFACT)
        with archive._open_entry(bootstrap) as stream:
            require(stream.read(2) == b"MZ", "bootstrap must be an EXE")
        add(bootstrap, "installers/" + bootstrap.sha256.lower() + "/Mir2Setup.exe")
    objects.sort(key=lambda item: item["path"])
    require(len(objects) <= publisher.MAX_OBJECTS
            and len({item["path"].lower() for item in objects}) == len(objects), "publication object closure duplicates/bound")
    bindings = {
        "gameCandidate": identity["identity"], "gameSourceRevision": version["gitRevision"], "gameDirectory": game_directory,
        "engineSourceRevision": engine["sourceRevision"].lower(), "engineDirectory": engine_directory,
        "manifestSha256": archive.sha256(manifest_bytes).lower(), "versionSha256": archive.sha256(version_bytes).lower(),
        "feedSha256": feed_entry.sha256.lower(), "feedSignatureSha256": feed_signature.sha256.lower(),
        "engineSha256": engine_files["ENGINE.json"].sha256.lower(), "engineSignatureSha256": engine_files["ENGINE.p7s"].sha256.lower(),
        "deliverySha256": descriptor.sha256.lower(), "deliverySignatureSha256": signature.sha256.lower(),
    }
    return {"schema": CLOSURE_SCHEMA, "candidate": candidate, "expectedCurrent": prior, "bindings": bindings,
            "objectsSha256": publisher.sha256(publisher.canonical_json(publisher.object_closure(objects))), "objects": objects}


def make_plan(closure, verification_receipt):
    verification = verify.file_entry(verification_receipt, "root verification", publisher.MAX_METADATA)
    data = verify.read_entry(verification, publisher.MAX_METADATA)
    attestation = publisher.parse_json(data)
    publisher.exact_keys(attestation, {"schema", "passed", "candidateVerified", "cmsVerified", "sourceRevision",
        "sequence", "feedSha256", "signatureSha256", "objectsSha256"}, "invalid_verification_receipt")
    require(attestation["schema"] == publisher.VERIFICATION_SCHEMA and attestation["passed"] is True
            and attestation["candidateVerified"] is True and attestation["cmsVerified"] is True,
            "root external verification attestation required")
    for name in ("sourceRevision", "sequence", "feedSha256", "signatureSha256"):
        require(attestation[name] == closure["candidate"][name]
                and type(attestation[name]) is type(closure["candidate"][name]), "root attestation Candidate binding")
    require(attestation["objectsSha256"] == closure["objectsSha256"], "root attestation object closure mismatch")
    return {"schema": publisher.PLAN_SCHEMA, "expectedCurrent": closure["expectedCurrent"],
            "candidate": closure["candidate"], "verificationReceipt": {"path": str(verification.source),
            "sha256": verification.sha256.lower()}, "objects": closure["objects"]}


def run(output_path, *, verification_receipt=None, **inputs):
    output_path = Path(output_path)
    require(output_path.is_absolute() and not os.path.lexists(output_path), "fresh absolute preparation output required")
    archive.absolute_directory(str(output_path.parent))
    result = {"schema": "mir2.windows.r2-preparation-attempt.v1", "passed": False}
    with output_path.open("x+", encoding="utf-8", newline="\n") as output:
        def journal():
            output.seek(0)
            output.write(json.dumps(result, indent=2) + "\n")
            output.truncate()
            output.flush()
            os.fsync(output.fileno())
        journal()
        try:
            closure = build_closure(**inputs)
            result = make_plan(closure, verification_receipt) if verification_receipt is not None else closure
        except (archive.ValidationError, publisher.PublishError) as error:
            result["failure"] = str(error)
        except Exception:
            result["failure"] = "publication input guard failed"
        journal()
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("candidate-root", "delivery-root", "delivery-verification", "engine-root", "feed-root"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--expected-current", type=Path)
    parser.add_argument("--bootstrap-exe", type=Path)
    parser.add_argument("--include-root-files", action="store_true")
    parser.add_argument("--verification-receipt", type=Path)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--objects-only", type=Path, help="Fresh closure for root CMS approval.")
    mode.add_argument("--output", type=Path, help="Fresh exact publisher plan after root approval.")
    args = vars(parser.parse_args())
    output = args.pop("objects_only")
    plan_output = args.pop("output")
    verification_receipt = args.pop("verification_receipt")
    require((output is not None and verification_receipt is None)
            or (plan_output is not None and verification_receipt is not None), "separate closure/attestation/plan steps required")
    try:
        result = run(output or plan_output, verification_receipt=verification_receipt, **args)
        passed = result.get("schema") in (CLOSURE_SCHEMA, publisher.PLAN_SCHEMA)
        print(json.dumps({"passed": passed, "output": str(output or plan_output),
                          "objects": len(result.get("objects", [])), "failure": result.get("failure")}))
        return 0 if passed else 1
    except Exception:
        print(json.dumps({"passed": False, "failure": "fresh output or local preparation guard failed"}))
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
