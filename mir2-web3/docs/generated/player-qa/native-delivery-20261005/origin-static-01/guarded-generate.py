"""Call unchanged frozen producer; stop at the first signed-byte mismatch."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import stat
import sys
import time

sys.dont_write_bytecode = True
STAGE = Path("/srv/.mir2-origin-r17-acceleration-20261005-01")
ORIGIN = Path("/srv/mir2-client-updates")


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def pinned(path, item):
    info = path.lstat()
    require(stat.S_ISREG(info.st_mode) and info.st_nlink == 1 and info.st_uid == 0
            and info.st_mode & 0o022 == 0, "unsafe root input: " + str(path))
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    with os.fdopen(fd, "rb") as stream:
        initial = os.fstat(stream.fileno())
        require((initial.st_dev, initial.st_ino, initial.st_size, initial.st_mtime_ns)
                == (info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns), "input raced opening")
        data = stream.read(item["size"] + 1)
        final = os.fstat(stream.fileno())
    require((final.st_dev, final.st_ino, final.st_size, final.st_mtime_ns)
            == (info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns), "input changed reading")
    require(len(data) == item["size"] and hashlib.sha256(data).hexdigest() == item["sha256"].lower(),
            "input hash mismatch: " + str(path))
    return data


def import_tool(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


def put(path, value):
    data = json.dumps(value, sort_keys=True, indent=2).encode() + b"\n"
    with path.open("xb") as stream:
        stream.write(data)
        stream.flush()
        os.fsync(stream.fileno())


def main():
    require(os.geteuid() == 0 and os.getpriority(os.PRIO_PROCESS, 0) >= 10,
            "one low-priority root worker required")
    os.umask(0o077)
    private = STAGE.lstat()
    require(stat.S_ISDIR(private.st_mode) and private.st_uid == 0
            and stat.S_IMODE(private.st_mode) == 0o700, "fresh private root staging required")
    uploaded = json.loads((STAGE / "UPLOAD-INPUTS.json").read_bytes())
    for item in uploaded["files"]:
        pinned(STAGE / item["path"], item)
    expected = json.loads((STAGE / "EXPECTED.json").read_bytes())
    preflight = json.loads((STAGE / "BASELINE.json").read_bytes())
    source_records = []
    for item in expected["sourceFiles"]:
        data = pinned(ORIGIN / item["path"], item)
        source_records.append({"path": item["path"], "size": len(data), "sha256": hashlib.sha256(data).hexdigest()})
    observation = import_tool("origin_observation", STAGE / "tools/remote-preflight.py")
    baseline_units = {item["unit"]: item for item in preflight["services"]}

    def health_guard():
        memory = {line.split(":", 1)[0]: line.split(":", 1)[1].strip()
                  for line in Path("/proc/meminfo").read_text().splitlines()}
        available = int(memory["MemAvailable"].split()[0]) * 1024
        require(available >= 512 * 1024**2, "resource guard: available memory below512MiB")
        require(os.getloadavg()[0] < 3.9, "resource guard: load above reviewed single-worker bound")
        current_units = observation.services()
        for unit in current_units:
            old = baseline_units[unit["unit"]]
            require(unit["MainPID"] == old["MainPID"] and unit["NRestarts"] == old["NRestarts"]
                    and unit["ActiveState"] == "active" and unit["SubState"] == "running",
                    "existing service changed during compression")
        # Static generation is permitted while clients remain connected.
        states = [observation.health(7110), observation.health(7210)]
        for item in expected["sourceFiles"][:2]:
            pinned(ORIGIN / item["path"], item)
        return {"memoryAvailableBytes": available, "health": states, "services": current_units,
                "loadAverage": list(os.getloadavg())}

    producer = import_tool("mir2_frozen_delivery_producer", STAGE / "tools/prepare-native-delivery.py")
    original_scan = producer.archive.scan

    def root_owned_scan(*args, **kwargs):
        result = original_scan(*args, **kwargs)
        for entries in result[0].values():
            for entry in entries.values():
                info = entry.source.lstat()
                require(info.st_uid == 0 and info.st_mode & 0o022 == 0,
                        "non-root or shared-writable Candidate source")
        return result

    producer.archive.scan = root_owned_scan
    descriptor = json.loads((STAGE / "inputs/DELIVERY.json").read_bytes())
    expected_bundle = descriptor["bundles"]
    original_write_bundle = producer.write_bundle
    results = []

    def signed_write_bundle(path, files):
        index = len(results)
        require(index < len(expected_bundle), "producer emitted extra bundle")
        wanted = expected_bundle[index]
        require([entry.path for entry in files] == wanted["files"], "signed bundle partition differs")
        before = health_guard()
        proof = original_write_bundle(path, files)
        actual = producer.entry(path, wanted["archive"]["path"])
        require(actual == wanted["archive"], "compressed bytes differ from original signed bundle")
        result = {"index": index, "archive": actual, "fileCount": len(files),
                  "expandedBytes": proof["expandedBytes"], "healthBefore": before,
                  "healthAfter": health_guard()}
        results.append(result)
        put(STAGE / ("SIGNED-BUNDLE-%02d.json" % index), result)
        print(json.dumps({"stage": "signed-bundle-match", "index": index,
                          "sha256": actual["sha256"], "size": actual["size"]}), flush=True)
        return proof

    producer.write_bundle = signed_write_bundle
    health_before = health_guard()
    start = time.monotonic()
    generated = STAGE / "generated"
    receipt = producer.build(str(ORIGIN / expected["candidateDirectory"]), generated,
                             [ORIGIN / "releases/game-WN-CANDIDATE-20261003-invited-16/mir2-platform-windows.exe"],
                             64 * 1024**2)
    result_descriptor = json.loads((generated / "DELIVERY.json").read_bytes())
    comparison = lambda value: {key: item for key, item in value.items() if key != "builtUnix"}
    require(comparison(result_descriptor) == comparison(descriptor), "generated artifact descriptor differs")
    for item in expected["sourceFiles"]:
        pinned(ORIGIN / item["path"], item)
    proof = {"schema": "mir2.origin.signed-regeneration.v1", "passed": True,
             "elapsedSeconds": round(time.monotonic() - start, 3),
             "originalSignedDescriptorUsedForPublication": True,
             "unsignedGeneratedDescriptorNotPublishable": True,
             "firstBundleGatePassed": bool(results), "allBundlesMatched": len(results),
             "allArtifactTablesMatched": True, "sources": source_records,
             "healthBefore": health_before, "healthAfter": health_guard(), "producerReceipt": receipt,
             "publicFilesWritten": False, "gatewayOrCaddyRestarted": False}
    put(STAGE / "GENERATION-RESULT.json", proof)
    print(json.dumps({"stage": "signed-regeneration-complete", "passed": True,
                      "bundles": len(results), "patches": len(receipt["patches"]),
                      "elapsedSeconds": proof["elapsedSeconds"]}), flush=True)


if __name__ == "__main__":
    main()
