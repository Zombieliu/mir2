"""Bind the read-only SSH preflight to exact frozen public evidence."""
import hashlib
import json
import os
from pathlib import Path
import sys
import zlib

root = Path(__file__).absolute().parent
plan = root.parent / "origin-download-compatibility-plan-01"
acceleration = root.parent / "r17-download-acceleration-02"


def bound(path, item):
    data = path.read_bytes()
    assert len(data) == item["size"] and hashlib.sha256(data).hexdigest() == item["sha256"].lower(), path
    return data


freeze = json.loads((plan / "FROZEN-RECEIPT.json").read_bytes())
assert hashlib.sha256((plan / "FROZEN-RECEIPT.json").read_bytes()).hexdigest() == \
    "dc2de5ac2df3a6abcb6db00950d7a55821ae6d58925e66734dcdab5e3ff6aa7f"
for item in freeze["files"]:
    bound(plan / item["path"], item)
assert hashlib.sha256((plan / "PLAN.md").read_bytes()).hexdigest() == \
    "53f2e0d11e47af2767ad420d1b50ce8ed3c7b6edd07b8a7893b764d8032ae4bb"
additions = json.loads((plan / "ORIGIN-ADDITIONS.json").read_bytes())["objects"]
for item in additions:
    bound(Path(item["source"]), item)
feed_data = (plan / "public-metadata/00-165.154.65.136.sslip.io-latest.json").read_bytes()
feed = json.loads(feed_data)
signature = (plan / "public-metadata/01-165.154.65.136.sslip.io-latest.p7s").read_bytes()
descriptor = json.loads((acceleration / "DELIVERY.json").read_bytes())
base = descriptor["patches"][0]
manifest_bound = next(item for item in feed["game"]["metadata"] if item["path"] == "PACKAGE-MANIFEST.json")
candidate_input = json.loads((acceleration / "INPUT.json").read_bytes())
manifest_path = Path(candidate_input["candidateRoot"]) / "PACKAGE-MANIFEST.json"
manifest = json.loads(bound(manifest_path, manifest_bound))
exe = next(item for item in manifest["files"] if item["path"] == "mir2-platform-windows.exe")
directory = feed["game"]["directory"]
sources = [{"path": "feed-current/latest.json", "size": len(feed_data), "sha256": hashlib.sha256(feed_data).hexdigest()},
           {"path": "feed-current/latest.p7s", "size": len(signature), "sha256": hashlib.sha256(signature).hexdigest()}]
sources.extend({**item, "path": directory + "/" + item["path"]} for item in feed["game"]["metadata"])
sources.append({**exe, "path": directory + "/mir2-platform-windows.exe"})
sources.append({"path": "releases/game-WN-CANDIDATE-20261003-invited-16/mir2-platform-windows.exe",
                "size": base["baseSize"], "sha256": base["baseSha256"]})
expected = {"candidateDirectory": directory, "sourceFiles": sources,
            "objects": [{key: item[key] for key in ("path", "size", "sha256")} for item in additions]}
assert len(expected["objects"]) == 14 and feed["sequence"] == 12
assert descriptor["packageManifestSha256"] == manifest_bound["sha256"]
expected_data = json.dumps(expected, sort_keys=True, indent=2).encode("ascii") + b"\n"
with (root / "EXPECTED-02.json").open("xb") as stream:
    stream.write(expected_data)
script = (root / "remote-preflight.py").read_text()
code = "EXPECTED = " + repr(expected) + "\n" + script
assert "MIR2_ORIGIN_PREFLIGHT_EOF" not in code
with (root / "preflight-02.command").open("x", newline="\n") as stream:
    stream.write("sudo -n python3 -B - <<'MIR2_ORIGIN_PREFLIGHT_EOF'\n" + code + "\nMIR2_ORIGIN_PREFLIGHT_EOF\n")
receipt = {"passed": True, "planFilesVerified": len(freeze["files"]), "signedArtifactsVerified": len(additions),
           "expectedSha256": hashlib.sha256(expected_data).hexdigest(), "privateCredentialAccess": False,
           "localRuntime": {"python": sys.version, "zlibCompiled": zlib.ZLIB_VERSION,
                            "zlibRuntime": zlib.ZLIB_RUNTIME_VERSION}}
with (root / "LOCAL-PREFLIGHT-02.json").open("x", newline="\n") as stream:
    json.dump(receipt, stream, sort_keys=True, indent=2)
print(json.dumps(receipt))
