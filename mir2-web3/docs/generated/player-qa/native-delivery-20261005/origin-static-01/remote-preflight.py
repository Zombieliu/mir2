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
