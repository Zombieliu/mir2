sudo -n python3 -B - <<'MIR2_ORIGIN_PREFLIGHT_EOF'
EXPECTED = {'candidateDirectory': 'releases/game-WN-CANDIDATE-20261004-invited-17', 'sourceFiles': [{'path': 'feed-current/latest.json', 'size': 1159, 'sha256': '7efdddf34fbf58ffa0bf8c4f7fcb1a9ad2bac869cb3f003cea0a0c9c53356c88'}, {'path': 'feed-current/latest.p7s', 'size': 1614, 'sha256': '7dd131dddcef9107ea0c80363cc9ca44746603c586a1e23e00ff6aa178006916'}, {'path': 'releases/game-WN-CANDIDATE-20261004-invited-17/PACKAGE-MANIFEST.json', 'size': 24360174, 'sha256': 'CEAE366BEBCA1C19C3F529EB22F016CCC80DFF5B8655694E76682E31AB12CB18'}, {'path': 'releases/game-WN-CANDIDATE-20261004-invited-17/VERSION.json', 'size': 1221, 'sha256': '1409E21C9F1AE7A04B71603CE1220FE38C37F222D54859AF2592CDD72CE99B16'}, {'path': 'releases/game-WN-CANDIDATE-20261004-invited-17/RELEASE-STATEMENT.json', 'size': 707, 'sha256': 'E64A8EF4FC62E6E9F6FE16625321FC8A88A0C8C29A661AEDA46025B08015DA82'}, {'path': 'releases/game-WN-CANDIDATE-20261004-invited-17/RELEASE-STATEMENT.p7s', 'size': 1614, 'sha256': 'BEE84B574F43F6411EB1BC3AB23A3CE0C389B853FB0F8B63F6AD9BDB1287856E'}, {'path': 'releases/game-WN-CANDIDATE-20261004-invited-17/mir2-platform-windows.exe', 'size': 111013376, 'sha256': 'D0780475EFA9F9C1ECD730DFCFC11E38B9B0659F05B3B1F7D93B4F80C3D734E0'}, {'path': 'releases/game-WN-CANDIDATE-20261003-invited-16/mir2-platform-windows.exe', 'size': 111005696, 'sha256': '9930C82045DF5E6A25EB7201265090E690200698B18EA526CA5E66173E7C6869'}], 'objects': [{'path': 'releases/game-WN-CANDIDATE-20261004-invited-17/DELIVERY.json', 'size': 7272337, 'sha256': 'a72b61448b342656dda4310ceb7ba5aff6608439025775fde2a8303f6ad62de8'}, {'path': 'releases/game-WN-CANDIDATE-20261004-invited-17/DELIVERY.p7s', 'size': 1614, 'sha256': 'c3f245cbff7a2e4545d1723cf56d0498b58d7af6360312d53253d255d60eeb93'}, {'path': 'releases/game-WN-CANDIDATE-20261004-invited-17/delivery/0a685ad8d38e5bb86db4aff061a884408ea390a74170030e77cad7059509d8b0.m2b.gz', 'size': 60629129, 'sha256': '0a685ad8d38e5bb86db4aff061a884408ea390a74170030e77cad7059509d8b0'}, {'path': 'releases/game-WN-CANDIDATE-20261004-invited-17/delivery/112d1f77a20d0da51f62bcbc488827530f5b854f91f7d423d101caedb0109ecc.m2b.gz', 'size': 58054864, 'sha256': '112d1f77a20d0da51f62bcbc488827530f5b854f91f7d423d101caedb0109ecc'}, {'path': 'releases/game-WN-CANDIDATE-20261004-invited-17/delivery/1e4052aa3f158a98b8d33306dda36e788662ea58e625bb7c1f7a8e7ca3153b3e.m2b.gz', 'size': 58094559, 'sha256': '1e4052aa3f158a98b8d33306dda36e788662ea58e625bb7c1f7a8e7ca3153b3e'}, {'path': 'releases/game-WN-CANDIDATE-20261004-invited-17/delivery/25fed6bdb5bba24c651b90ebabe0805dcbd1429149d61241a4d7a0ca48bb8059.m2b.gz', 'size': 42095227, 'sha256': '25fed6bdb5bba24c651b90ebabe0805dcbd1429149d61241a4d7a0ca48bb8059'}, {'path': 'releases/game-WN-CANDIDATE-20261004-invited-17/delivery/3e8b19dfd08f94aa7eac46ef88001d199b6e6be1ba3f9382be2f9d39b5cf6bb3.m2b.gz', 'size': 63353851, 'sha256': '3e8b19dfd08f94aa7eac46ef88001d199b6e6be1ba3f9382be2f9d39b5cf6bb3'}, {'path': 'releases/game-WN-CANDIDATE-20261004-invited-17/delivery/3ff41a0e69a58faaea5b1327ac5900079a043607d94999ffc18ed312bd7f6945.m2b.gz', 'size': 65515198, 'sha256': '3ff41a0e69a58faaea5b1327ac5900079a043607d94999ffc18ed312bd7f6945'}, {'path': 'releases/game-WN-CANDIDATE-20261004-invited-17/delivery/567dfae85cc7e726e99dfe74f7c9b6a3e1ab868f6c6727f5153a1d5abeac92c1.m2b.gz', 'size': 65945319, 'sha256': '567dfae85cc7e726e99dfe74f7c9b6a3e1ab868f6c6727f5153a1d5abeac92c1'}, {'path': 'releases/game-WN-CANDIDATE-20261004-invited-17/delivery/792db75e22e4d9066f2529c3f117655c0ec794eb47b431b49fba6c78760f052e.m2b.gz', 'size': 59469514, 'sha256': '792db75e22e4d9066f2529c3f117655c0ec794eb47b431b49fba6c78760f052e'}, {'path': 'releases/game-WN-CANDIDATE-20261004-invited-17/delivery/ac4d15842886a496f093d585e02f9adcc6c3cf3cc3c763a3021b0c56e01431e3.m2b.gz', 'size': 49934822, 'sha256': 'ac4d15842886a496f093d585e02f9adcc6c3cf3cc3c763a3021b0c56e01431e3'}, {'path': 'releases/game-WN-CANDIDATE-20261004-invited-17/delivery/d528c7149953768ed25b77f5da5d84a4f514ee7ab1e7b8c1684cab25e1823a63.m2d.gz', 'size': 25424169, 'sha256': 'd528c7149953768ed25b77f5da5d84a4f514ee7ab1e7b8c1684cab25e1823a63'}, {'path': 'releases/game-WN-CANDIDATE-20261004-invited-17/delivery/f176d5dba713d8bd9be0de94b7a7fe1fec48493049a0218c2baa144d3312d0b5.m2b.gz', 'size': 36243793, 'sha256': 'f176d5dba713d8bd9be0de94b7a7fe1fec48493049a0218c2baa144d3312d0b5'}, {'path': 'releases/game-WN-CANDIDATE-20261004-invited-17/delivery/f7d89de99e3bc3f565c58e4713870d78d9b446bfee9651d30ba9d7c559a73cfe.m2b.gz', 'size': 60853297, 'sha256': 'f7d89de99e3bc3f565c58e4713870d78d9b446bfee9651d30ba9d7c559a73cfe'}]}
"""Read-only fixed-origin preflight; injected EXPECTED contains public hashes only."""
import hashlib
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import time
import urllib.request
import zlib


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def directory(path):
    path = Path(path)
    require(path.is_absolute(), "absolute path required")
    for part in (Path("/"), *reversed(path.parents[:-1]), path):
        info = part.lstat()
        require(stat.S_ISDIR(info.st_mode) and info.st_uid == 0
                and info.st_mode & 0o022 == 0, "unsafe public directory: " + str(part))
    return path


def file_record(path, expected=None):
    directory(path.parent)
    info = path.lstat()
    require(stat.S_ISREG(info.st_mode) and info.st_nlink == 1 and info.st_uid == 0
            and info.st_mode & 0o022 == 0, "unsafe source file: " + str(path))
    with path.open("rb") as stream:
        opened = os.fstat(stream.fileno())
        require((opened.st_dev, opened.st_ino, opened.st_size, opened.st_mtime_ns)
                == (info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns), "file changed opening")
        digest = hashlib.file_digest(stream, "sha256").hexdigest()
        finished = os.fstat(stream.fileno())
    require((finished.st_dev, finished.st_ino, finished.st_size, finished.st_mtime_ns)
            == (info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns), "file changed hashing")
    result = {"path": str(path), "size": info.st_size, "sha256": digest,
              "uid": info.st_uid, "mode": oct(stat.S_IMODE(info.st_mode)), "nlink": info.st_nlink,
              "device": info.st_dev, "inode": info.st_ino, "mtimeNs": info.st_mtime_ns}
    if expected is not None:
        require(result["size"] == expected["size"] and digest == expected["sha256"].lower(),
                "source bytes do not match: " + str(path))
    return result


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *args, **kwargs):
        raise RuntimeError("unexpected local health redirect")


def health(port):
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect())
    with opener.open("http://127.0.0.1:" + str(port) + "/health", timeout=5) as response:
        data = response.read(1024 * 1024 + 1)
        require(len(data) <= 1024 * 1024, "health response exceeds bound")
    value = json.loads(data)
    keys = ("currentWsConnections", "currentActiveSessions", "currentReconnectLeases",
            "currentLoginInFlight", "currentNewCharacterInFlight", "currentStartGameInFlight")
    counters = {key: value.get("capacity", {}).get(key) for key in keys}
    require(all(type(count) is int and count >= 0 for count in counters.values()), "invalid health counters")
    ready = value.get("ok") is True and value.get("http") == "ready" and value.get("ws") == "ready"
    require(ready, "existing realm not ready")
    return {"port": port, "ready": ready, "revision": value.get("revision"), "capacity": counters}


def services():
    result = []
    for name in ("mir2-playtest.service", "mir2-gateway.service", "caddy.service"):
        proc = subprocess.run(["systemctl", "show", name, "-p", "MainPID", "-p", "NRestarts",
                               "-p", "Result", "-p", "ActiveState", "-p", "SubState"],
                              check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=10)
        values = dict(line.split("=", 1) for line in proc.stdout.decode().splitlines())
        result.append({"unit": name, **values})
    return result


def main():
    expected = EXPECTED
    require(os.geteuid() == 0, "root read-only preflight required")
    root = directory("/srv/mir2-client-updates")
    candidate = directory(root / expected["candidateDirectory"])
    records = []
    for item in expected["sourceFiles"]:
        records.append(file_record(root / item["path"], item))
    version = json.loads((candidate / "VERSION.json").read_bytes())
    require(version["gitRevision"] == "6032ef8b3e27dd97bad0b20c8676ef9f185db64b",
            "candidate source revision differs")
    feed = json.loads((root / "feed-current/latest.json").read_bytes())
    require(feed["sequence"] == 12 and feed["game"]["directory"] == expected["candidateDirectory"],
            "feed advanced or candidate changed")
    additions = []
    for item in expected["objects"]:
        target = root / item["path"]
        try:
            target.lstat()
        except FileNotFoundError:
            # delivery/ itself may not exist, but it must never resolve via a link.
            parent = target.parent
            if parent.exists() or parent.is_symlink():
                directory(parent)
            else:
                require(parent == candidate / "delivery", "unexpected missing parent")
            additions.append({"path": item["path"], "state": "absent"})
        else:
            additions.append({"state": "exact-existing", **file_record(target, item)})
    stage_names = ("/tmp/mir2-origin-input-20261005-01", "/var/tmp/mir2-origin-acceleration-20261005-01")
    for name in stage_names:
        require(not os.path.lexists(name), "fresh staging path already exists: " + name)
    disk = os.statvfs("/var/tmp")
    available = disk.f_bavail * disk.f_frsize
    memory = {line.split(":", 1)[0]: line.split(":", 1)[1].strip()
              for line in Path("/proc/meminfo").read_text().splitlines()}
    memory_bytes = int(memory["MemAvailable"].split()[0]) * 1024
    require(available >= 3 * 1024**3, "less than3GiB disk headroom")
    require(memory_bytes >= 512 * 1024**2, "less than512MiB memory headroom")
    tcp = {}
    for port in (7100, 7200):
        proc = subprocess.run(["ss", "-Htn", "state", "established", "sport = :" + str(port)],
                              check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=10)
        tcp[str(port)] = len(proc.stdout.splitlines())
    result = {"schema": "mir2.origin.acceleration-preflight.v1", "passed": True,
              "timestampUnix": int(time.time()), "remoteFilesystemWritten": False,
              "feedSequence": feed["sequence"], "candidateDirectory": expected["candidateDirectory"],
              "sources": records, "additions": additions, "stageNamesAbsent": list(stage_names),
              "resources": {"diskAvailableBytes": available, "memoryAvailableBytes": memory_bytes,
                            "cpuCount": os.cpu_count(), "loadAverage": list(os.getloadavg())},
              "compressionRuntime": {"python": sys.version, "zlibCompiled": zlib.ZLIB_VERSION,
                                     "zlibRuntime": zlib.ZLIB_RUNTIME_VERSION},
              "health": [health(7110), health(7210)], "tcpEstablishedCounts": tcp,
              "services": services(), "zeroSessionsRequiredForStaticAppend": False,
              "gatewayOrCaddyRestarted": False}
    print(json.dumps(result, sort_keys=True, indent=2))


if __name__ == "__main__":
    main()

MIR2_ORIGIN_PREFLIGHT_EOF
