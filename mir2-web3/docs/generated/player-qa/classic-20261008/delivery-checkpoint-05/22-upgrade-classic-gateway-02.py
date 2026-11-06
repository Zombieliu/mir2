"""Preflight or upgrade one drained isolated Gateway; default is read-only.

Only --apply stages the pinned artifact, backs up this realm, and changes its
revision/ExecStart. Capacity, authentication, maps, resource limits and the
original realm are preserved. A failed attempt restores configuration only,
never the database or player state.
"""
import argparse
import hashlib
import json
import os
import pathlib
import re
import shutil
import stat
import subprocess
import sys
import time
import urllib.parse
import urllib.request

UNIT = "mir2-playtest.service"
ENV = pathlib.Path("/etc/mir2-playtest/gateway.env")
BASE_UNITFILE = pathlib.Path("/etc/systemd/system") / UNIT
UNITFILE = pathlib.Path("/etc/systemd/system/mir2-playtest.service.d/50-periodic-b71.conf")
BASE_UNIT_SHA256 = "9bf1b374db39732ad7b59e6d468086eea8c4563fbfb3185ef3bf9dfd714551a9"
OLD_DROPIN_SHA256 = "6267365ef2c5c610535d1de865e10d84cdcbec37a89b98326e498f64a78e6814"
ROOT = pathlib.Path("/home/ubuntu/mir2-playtest-preflight-20260929")
RELEASE_PARENT = pathlib.Path("/opt/mir2-playtest/releases")
BACKUP_PARENT = pathlib.Path("/var/backups")
ORIGINAL_CURRENT = pathlib.Path("/opt/mir2/gateway/current")
STATE_PARENT = pathlib.Path("/var/lib/mir2-playtest")
BOOTSTRAP = pathlib.Path("/etc/mir2-playtest/bootstrap.private.json")
CAP_KEYS = {
    "MIR2_GATEWAY_MAX_ACTIVE_SESSIONS": "maxActiveSessions",
    "MIR2_GATEWAY_MAX_WS_CONNECTIONS": "maxWsConnections",
    "MIR2_GATEWAY_MAX_RECONNECT_LEASES": "maxReconnectLeases",
}
RESOURCE_PROPERTIES = (
    "CPUQuotaPerSecUSec", "CPUQuotaPeriodUSec", "CPUWeight", "AllowedCPUs",
    "EffectiveCPUs", "CPUAffinity", "MemoryHigh", "MemoryMax", "MemorySwapMax",
    "MemoryLow", "MemoryMin", "TasksMax", "LimitNOFILE", "DropInPaths",
)
FIXED_FLAGS = {
    "MIR2_GATEWAY_SLOW_STAGE_MS": "25",
    "MIR2_SPECTATOR_ENABLED": "0",
    "MIR2_SPECTATOR_RECORDING_ENABLED": "0",
    "MIR2_SPECTATOR_PUBLIC": "0",
}


class SafetyError(Exception):
    """Only these deliberately non-secret messages may be printed."""


def require(condition, message):
    if not condition:
        raise SafetyError(message)


def limits(active):
    require(type(active) is int and active in (15, 51, 101), "Unsupported capacity tier")
    return {"maxActiveSessions": active, "maxWsConnections": active + 15,
            "maxReconnectLeases": active}


def validate_request(revision, previous, manifest_hash, active):
    require(revision == "a41dfe72d7e5858f11169adf88673dc44ed24200" and
            previous == "c6c32381a646dae1067dafdeec57691f606b1779" and active == 51,
            "Only the attested appearance-to-classic pair at the established capacity is allowed")
    require(all(isinstance(value, str) and re.fullmatch(r"[0-9a-f]{40}", value)
                for value in (revision, previous)) and revision != previous,
            "Two different full lowercase revisions are required")
    require(isinstance(manifest_hash, str) and re.fullmatch(r"[0-9a-f]{64}", manifest_hash),
            "A full lowercase verified-manifest SHA256 is required")
    limits(active)


def run(command, timeout=75):
    return subprocess.run(command, check=True, stdout=subprocess.PIPE,
                          stderr=subprocess.PIPE, timeout=timeout)


def show(properties):
    command = ["systemctl", "show", UNIT]
    for key in properties:
        command.extend(["-p", key])
    return dict(line.split("=", 1) for line in run(command).stdout.decode().splitlines())


def status():
    return show(("MainPID", "Result", "ExecMainStatus", "ExecMainCode", "NRestarts"))


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, response, code, message, headers, new_url):
        raise SafetyError("Local health endpoint attempted a redirect")


def health(port=7210):
    require(port in (7110, 7210), "Unexpected health port")
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect())
    with opener.open(f"http://127.0.0.1:{port}/health", timeout=5) as response:
        data = response.read(1024 * 1024 + 1)
        require(len(data) <= 1024 * 1024, "Health response exceeds the bound")
        return json.loads(data)


def ready(value):
    return value.get("ok") is True and value.get("http") == "ready" and value.get("ws") == "ready"


def validate_health(value, revision, active):
    require(ready(value) and value.get("revision") == revision, "Expected realm revision is not healthy")
    cap = value.get("capacity", {})
    expected = {**limits(active), "currentWsConnections": 0,
                "currentActiveSessions": 0, "currentReconnectLeases": 0,
                "currentLoginInFlight": 0, "currentNewCharacterInFlight": 0,
                "currentStartGameInFlight": 0}
    require(all(type(cap.get(key)) is int and cap[key] == count for key, count in expected.items()),
            "Realm capacity differs or WebSocket/session/reconnect/login/character/StartGame state is not drained")
    spectator = value.get("spectator", {})
    require(spectator.get("recordingEnabled") is False and spectator.get("bufferedFrames") == 0
            and spectator.get("enabled") is not True, "Spectator state changed")


def validate_tcp_drained():
    # HTTP capacity counts do not include ordinary Crystal TCP connections.
    # Check both IPv4 and IPv6 accepted sockets; listening sockets are excluded.
    connections = run(["ss", "-Htn", "state", "established", "sport = :7200"], timeout=10).stdout
    require(not connections.strip(), "Isolated Gateway TCP 7200 connections are not drained")


def validate_drained(value, revision, active):
    validate_health(value, revision, active)
    validate_tcp_drained()


def validate_rollback_drained(context):
    # Do not interrupt a newly arrived user while recovering a failed candidate.
    # A stopped unit has no in-process sessions; a live one must report all counts.
    pid = status().get("MainPID")
    require(isinstance(pid, str) and pid.isdigit(), "Rollback process identity is unavailable")
    validate_tcp_drained()
    if int(pid) != 0:
        value = health()
        require(value.get("revision") in (context["previous"], context["revision"]),
                "Rollback realm revision differs from the attested pair")
        validate_drained(value, value["revision"], context["active"])
    require(status().get("MainPID") == pid, "Rollback process changed during drain verification")


def listener_pid(port):
    pids = set(re.findall(r"pid=(\d+)", run(["ss", "-ltnpH", f"sport = :{port}"]).stdout.decode()))
    require(len(pids) == 1 and "0" not in pids, "Listener must have exactly one process")
    return pids.pop()


def original_identity():
    require(ready(health(7110)), "Original realm is not healthy")
    return {"pid": listener_pid(7110), "release": str(ORIGINAL_CURRENT.resolve(strict=True))}


def directory(path, root_owned=True):
    require(path.is_absolute() and path.resolve(strict=True) == path and
            stat.S_ISDIR(path.lstat().st_mode), "Refusing a linked or non-directory path")
    info = path.stat()
    require(not root_owned or (info.st_uid == 0 and not info.st_mode & 0o022),
            "Managed directory must be root-owned and not externally writable")


def regular_descriptor(path, maximum, root_owned=True, private=False):
    require(path.is_absolute() and path.parent.resolve(strict=True) == path.parent and
            not path.is_symlink(), "Refusing a linked file or parent")
    descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
    try:
        info = os.fstat(descriptor)
        require(stat.S_ISREG(info.st_mode) and info.st_nlink == 1 and 0 < info.st_size <= maximum,
                "Refusing non-ordinary, hardlinked, empty, or oversized input")
        require(not root_owned or (info.st_uid == 0 and not info.st_mode & 0o022),
                "Managed input must be root-owned and not externally writable")
        require(not private or stat.S_IMODE(info.st_mode) == 0o600, "Private input must have mode 0600")
        return descriptor, info
    except BaseException:
        os.close(descriptor)
        raise


def checked(path, maximum, expected=None, root_owned=True, private=False):
    descriptor, info = regular_descriptor(path, maximum, root_owned, private)
    with os.fdopen(descriptor, "rb") as stream:
        data = stream.read(maximum + 1)
    require(len(data) == info.st_size, "Input changed size while being read")
    require(expected is None or hashlib.sha256(data).hexdigest() == expected, "Input hash differs")
    return data


def hash_descriptor(descriptor, maximum):
    with os.fdopen(descriptor, "rb") as stream:
        info = os.fstat(stream.fileno())
        require(stat.S_ISREG(info.st_mode) and info.st_uid == 0 and
                not info.st_mode & 0o022 and 0 < info.st_size <= maximum,
                "Executable or backup is not a bounded root-owned regular file")
        digest, count = hashlib.sha256(), 0
        while block := stream.read(1024 * 1024):
            count += len(block)
            require(count <= maximum, "Input grew beyond its allowed bound")
            digest.update(block)
    require(count == info.st_size, "Input changed size while being hashed")
    return digest.hexdigest(), count


def file_hash(path, maximum=268435456):
    descriptor, _ = regular_descriptor(path, maximum)
    return hash_descriptor(descriptor, maximum)


def process_binary(pid):
    require(isinstance(pid, str) and pid.isdigit() and int(pid) > 0, "Invalid process identity")
    link = pathlib.Path(f"/proc/{pid}/exe")
    resolved = link.resolve(strict=True)
    # This is the one intentionally followed link: hash the executable inode
    # actually held by this verified PID, not merely its pathname on disk.
    fingerprint = hash_descriptor(os.open(link, os.O_RDONLY), 268435456)
    return resolved, fingerprint


def parse_env(data):
    result = {}
    for line in data.decode("utf-8").splitlines():
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        key, separator, value = line.partition("=")
        require(separator == "=" and re.fullmatch(r"[A-Z][A-Z0-9_]*", key),
                "Unsupported environment assignment")
        require(key not in result, "Duplicate environment assignment")
        result[key] = value
    return result


def rewrite_env(data, previous, revision, active):
    config = parse_env(data)
    require(hashlib.sha256(data).hexdigest() == "77f12d974ca32103673181669803c89283c68cf34a952c0a177b721c1d013729",
            "Pinned established environment bytes changed")
    require(config.get("MIR2_DEPLOY_REVISION") == "3f5e61533235921369bc13a7760b4a56b0e467e5",
            "Established file revision changed")
    require(all(config.get(key) == str(limits(active)[field]) for key, field in CAP_KEYS.items()),
            "Environment capacity differs from the expected tier")
    require(all(config.get(key) == value for key, value in FIXED_FLAGS.items()),
            "Established spectator or diagnostic settings changed")
    # The current service deliberately overrides this field in ExecStart.
    # Preserve the actual file byte-for-byte, including all private settings.
    return data


def validate_unit(data, release):
    lines = [line.strip() for line in data.decode().splitlines() if line.strip().startswith("ExecStart=")]
    expected = f"ExecStart=/usr/bin/env MIR2_DEPLOY_REVISION={release.name} {release}/mir2-gateway"
    require(lines == ["ExecStart=", expected], "Drop-in must have one reset and one exact wrapped ExecStart")


def rewrite_unit(data, previous, revision):
    old, new = RELEASE_PARENT / previous, RELEASE_PARENT / revision
    validate_unit(data, old)
    old_line = f"ExecStart=/usr/bin/env MIR2_DEPLOY_REVISION={previous} {old}/mir2-gateway"
    new_line = f"ExecStart=/usr/bin/env MIR2_DEPLOY_REVISION={revision} {new}/mir2-gateway"
    result = []
    for line in data.decode().splitlines(keepends=True):
        result.append(line.replace(old_line, new_line) if line.strip().startswith("ExecStart=") else line)
    output = "".join(result).encode()
    validate_unit(output, new)
    return output


def validate_running(revision, fingerprint):
    release = RELEASE_PARENT / revision
    require(file_hash(release / "mir2-gateway") == fingerprint, "On-disk executable hash or size changed")
    effective = show(("FragmentPath", "ExecStart"))
    require(effective.get("FragmentPath") == str(BASE_UNITFILE), "Effective base unit fragment changed")
    require(show(("DropInPaths",)).get("DropInPaths") == str(UNITFILE), "Effective drop-in set changed")
    starts = effective.get("ExecStart", "")
    require(re.findall(r"(?:^|\s)path=([^ ;}]+)", starts) == ["/usr/bin/env"] and
            [value.strip() for value in re.findall(r"argv\[\]=([^;]+)", starts)] ==
            [f"/usr/bin/env MIR2_DEPLOY_REVISION={revision} {release}/mir2-gateway"],
            "Effective systemd ExecStart differs from the intended executable")
    pid = status().get("MainPID")
    require(listener_pid(7210) == pid, "Test listener differs from the unit MainPID")
    actual, observed = process_binary(pid)
    require(actual == release / "mir2-gateway" and observed == fingerprint,
            "Running executable path or inode hash differs from the verified release")
    require(status().get("MainPID") == pid, "Process changed during identity verification")
    return pid


def validate_backup_tree(source):
    directory(source, root_owned=False)
    for parent, directories, files in os.walk(source, followlinks=False):
        for name in directories + files:
            child = pathlib.Path(parent) / name
            info = child.lstat()
            require(not stat.S_ISLNK(info.st_mode) and child.resolve(strict=True).is_relative_to(source),
                    "Refusing a linked backup entry")
            require(stat.S_ISDIR(info.st_mode) or (stat.S_ISREG(info.st_mode) and info.st_nlink == 1),
                    "Refusing non-ordinary or hardlinked backup data")


def newfile(path, data, mode=0o600):
    directory(path.parent)
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, mode)
    with os.fdopen(descriptor, "wb") as stream:
        stream.write(data)
        stream.flush()
        os.fsync(stream.fileno())
    os.chmod(path, mode)


def write_json(path, value):
    newfile(path, (json.dumps(value, indent=2) + "\n").encode())


def replace(path, data, mode, revision, label):
    directory(path.parent)
    temporary = path.with_name(f"{path.name}.{revision[:12]}.{label}.tmp")
    newfile(temporary, data, mode)
    os.replace(temporary, path)
    descriptor = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def wait_ready(revision, active):
    for _ in range(45):
        try:
            value = health()
            validate_health(value, revision, active)
            return value
        except Exception:
            time.sleep(1)
    raise SafetyError("Expected healthy, drained realm capacity did not become ready")


def release_fingerprint(release, revision):
    directory(release)
    # The established B71 rollout recorded its exact binary without RELEASE.json.
    # Use those preserved cutover pins for this one baseline, never for candidates.
    if revision == "b71fac99df2730cf056d8ee73fa8a224365d55c2":
        require(release == RELEASE_PARENT / revision, "Baseline release path changed")
        fingerprint = file_hash(release / "mir2-gateway")
        require(fingerprint == ("b7c8fd545724ff559ac75ebb7ecc6d6b2a0461e847792015c2aac89158fe8b67", 79776472),
                "Established B71 binary differs from the recorded cutover")
        return fingerprint
    metadata = json.loads(checked(release / "RELEASE.json", 65536))
    require(metadata.get("gitRevision") == revision and metadata.get("gitDirty") is False and
            metadata.get("name") == "mir2-gateway", "Release metadata is not the expected clean Gateway")
    fingerprint = file_hash(release / "mir2-gateway")
    require(fingerprint == (metadata.get("binarySha256", "").lower(), metadata.get("binarySizeBytes")),
            "Release metadata and executable disagree")
    return fingerprint


def preflight(revision, previous, manifest_hash, active):
    require(sys.platform == "linux" and os.geteuid() == 0, "Preflight requires root on the isolated Linux host")
    validate_request(revision, previous, manifest_hash, active)
    for parent in (RELEASE_PARENT, BACKUP_PARENT, ENV.parent, UNITFILE.parent):
        directory(parent)
    source = ROOT / ("capacity-verified-" + revision)
    directory(source, root_owned=False)
    dest = RELEASE_PARENT / revision
    backup = BACKUP_PARENT / ("mir2-playtest-before-" + revision)
    require(all(not path.exists() and not path.is_symlink() for path in (dest, backup)),
            "Existing release or backup requires inspection; refusing repeated mutation")
    before = health()
    validate_drained(before, previous, active)
    original = original_identity()
    checked(BASE_UNITFILE, 32768, BASE_UNIT_SHA256)
    env, unit = checked(ENV, 65536, private=True), checked(UNITFILE, 32768, OLD_DROPIN_SHA256)
    new_env, new_unit = rewrite_env(env, previous, revision, active), rewrite_unit(unit, previous, revision)
    config = parse_env(env)
    require(config.get("MIR2_GATEWAY_WEB_ADDR") == "127.0.0.1:7210" and
            config.get("MIR2_GATEWAY_REDIS_CACHE_URL") == "redis://127.0.0.1:6381", "Realm isolation changed")
    database = urllib.parse.urlsplit(config.get("MIR2_ACCOUNT_STORE_DATABASE_URL", ""))
    require(database.scheme in ("postgres", "postgresql") and database.hostname == "127.0.0.1" and
            database.port == 5432 and database.username == "mir2_playtest" and database.path == "/mir2_playtest" and
            not database.query and not database.fragment, "Database is not the expected isolated database")
    require(config.get("MIR2_CHANNEL_IDENTITY_DATABASE_URL") == config.get("MIR2_ACCOUNT_STORE_DATABASE_URL"),
            "Channel identity database differs")
    maps = pathlib.Path(config.get("MIR2_CRYSTAL_MAP_PACK", ""))
    directory(maps)
    require(maps.parent.parent == RELEASE_PARENT and re.fullmatch(r"[0-9a-f]{40}", maps.parent.name) and
            maps.name == "crystal-map-pack" and len(list(maps.iterdir())) == 1620 and (maps / "0.map.gz").is_file(),
            "Verified map resource root changed")
    validate_backup_tree(maps)
    for name in ("state", "recovery"):
        validate_backup_tree(STATE_PARENT / name)
    checked(BOOTSTRAP, 65536, private=True)
    manifest_bytes = checked(source / "VERIFIED-FILES.json", 65536, manifest_hash, root_owned=False)
    manifest = json.loads(manifest_bytes)
    entries = manifest.get("files", [])
    require(manifest.get("revision") == revision and len(entries) == 2 and
            {entry.get("path") for entry in entries} == {"mir2-gateway", "RELEASE.json"}, "Artifact inventory differs")
    content = {}
    for entry in entries:
        name = entry["path"]
        mode = 0o755 if name == "mir2-gateway" else 0o644
        require(entry.get("mode") == mode and re.fullmatch(r"[0-9a-f]{64}", entry.get("sha256", "")),
                "Artifact entry mode or digest differs")
        data = checked(source / name, 268435456, entry["sha256"], root_owned=False)
        require(len(data) == entry.get("size"), "Artifact entry size differs")
        content[name] = (data, mode)
    metadata = json.loads(content["RELEASE.json"][0])
    binary = content["mir2-gateway"][0]
    fingerprint = (hashlib.sha256(binary).hexdigest(), len(binary))
    require(metadata.get("gitRevision") == revision and metadata.get("gitDirty") is False and
            metadata.get("name") == "mir2-gateway" and
            metadata.get("target") in ("linux-x64", "x86_64-unknown-linux-gnu") and binary[:4] == b"\x7fELF" and
            fingerprint == (metadata.get("binarySha256", "").lower(), metadata.get("binarySizeBytes")),
            "Artifact metadata or executable disagrees with the verified inventory")
    old_fingerprint = release_fingerprint(RELEASE_PARENT / previous, previous)
    pid = validate_running(previous, old_fingerprint)
    return {"revision": revision, "previous": previous, "active": active, "dest": dest, "backup": backup,
            "before": before, "original": original, "env": env, "unit": unit, "newEnv": new_env, "newUnit": new_unit,
            "manifest": manifest_bytes, "content": content, "fingerprint": fingerprint, "oldFingerprint": old_fingerprint,
            "resources": show(RESOURCE_PROPERTIES), "pid": pid, "mapRoot": str(maps)}


def unchanged(context, upgraded):
    checked(BASE_UNITFILE, 32768, BASE_UNIT_SHA256)
    require(original_identity() == context["original"], "Original realm identity or health changed")
    require(show(RESOURCE_PROPERTIES) == context["resources"], "Resource limits or drop-ins changed")
    require(checked(ENV, 65536, private=True) == context["newEnv" if upgraded else "env"],
            "Environment differs from the bounded rewrite")
    require(checked(UNITFILE, 32768) == context["newUnit" if upgraded else "unit"],
            "Unit differs from its sole ExecStart rewrite")


def verify_live(context, upgraded):
    revision = context["revision" if upgraded else "previous"]
    value = wait_ready(revision, context["active"])
    validate_drained(value, revision, context["active"])
    unchanged(context, upgraded)
    pid = validate_running(revision, context["fingerprint" if upgraded else "oldFingerprint"])
    return value, pid


def normal_stop(pid):
    run(["systemctl", "stop", UNIT])
    stopped = status()
    require(stopped.get("MainPID") == "0" and stopped.get("Result") == "success" and
            stopped.get("ExecMainStatus") == "0", "Gateway did not stop normally")
    journal = run(["journalctl", "-b", "-u", UNIT, "_PID=" + pid, "--no-pager", "-o", "cat"],
                  timeout=20).stdout.decode(errors="replace")
    require(not any(value in journal for value in ("panicked at", "Cannot start a runtime", "panic in a destructor")),
            "Gateway shutdown recorded a panic")
    return stopped


def stage_and_backup_config(context):
    context["dest"].mkdir(mode=0o755)
    for name, (data, mode) in context["content"].items():
        newfile(context["dest"] / name, data, mode)
    newfile(context["dest"] / "VERIFIED-FILES.json", context["manifest"], 0o644)
    require(release_fingerprint(context["dest"], context["revision"]) == context["fingerprint"],
            "Staged candidate bytes changed")
    backup = context["backup"]
    backup.mkdir(mode=0o700)
    newfile(backup / "gateway.env", context["env"])
    newfile(backup / UNITFILE.name, context["unit"])
    newfile(backup / ("base-" + UNIT), checked(BASE_UNITFILE, 32768, BASE_UNIT_SHA256))
    write_json(backup / "before.json", {"health": context["before"], "original": context["original"],
               "resources": context["resources"], "revision": context["previous"]})


def backup_private(context):
    backup = context["backup"]
    dump = backup / "mir2_playtest.dump"
    descriptor = os.open(dump, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    with os.fdopen(descriptor, "wb") as stream:
        subprocess.run(["runuser", "-u", "postgres", "--", "pg_dump", "-Fc", "mir2_playtest"],
                       stdout=stream, stderr=subprocess.PIPE, check=True, timeout=120)
        stream.flush()
        os.fsync(stream.fileno())
    run(["pg_restore", "--list", str(dump)], timeout=30)
    for name in ("state", "recovery"):
        source = STATE_PARENT / name
        validate_backup_tree(source)
        destination = backup / name
        shutil.copytree(source, destination)
        for parent, directories, files in os.walk(destination, followlinks=False):
            os.chmod(parent, 0o700)
            for file in files:
                os.chmod(pathlib.Path(parent) / file, 0o600)
    newfile(backup / "bootstrap.private.json", checked(BOOTSTRAP, 65536, private=True))
    return file_hash(dump, 4 * 1024**3)[0]


def rollback(context, error):
    # A started candidate may already have written state. Do not automatically
    # change its schema interpreter or restore possible new player writes.
    if context.get("candidateStarted") is True:
        result = {"ok": False, "applied": False, "rolledBack": False,
                  "manualRecoveryRequired": True, "databaseRestored": False,
                  "configurationRetainedForV7": True, "backup": str(context["backup"]),
                  "error": str(error) if isinstance(error, SafetyError) else type(error).__name__}
        write_json(context["backup"] / "failure.json", result)
        return result
    failures = []
    try:
        validate_rollback_drained(context)
    except BaseException:
        failures.append("drain")
    if not failures:
        try:
            run(["systemctl", "stop", UNIT])
            require(status().get("MainPID") == "0", "Rollback process did not stop")
        except BaseException:
            failures.append("stop")
    if not failures:
        for label, action in (
            ("restore-env", lambda: None if context["newEnv"] == context["env"] else replace(ENV, context["env"], 0o600, context["revision"], "rollback")),
            ("restore-unit", lambda: replace(UNITFILE, context["unit"], 0o644, context["revision"], "rollback")),
            ("reload", lambda: run(["systemctl", "daemon-reload"])),
            ("reset-failed", lambda: run(["systemctl", "reset-failed", UNIT])),
            ("start", lambda: run(["systemctl", "start", UNIT])),
        ):
            try:
                action()
            except BaseException:
                failures.append(label)
    confirmed = False
    if "drain" not in failures and "stop" not in failures:
        try:
            verify_live(context, False)
            confirmed = True
        except BaseException:
            failures.append("verify")
    result = {"ok": False, "applied": False, "rolledBack": confirmed, "rollbackFailed": not confirmed,
              "rollbackErrors": failures, "backup": str(context["backup"]), "databaseRestored": False,
              "error": str(error) if isinstance(error, SafetyError) else type(error).__name__}
    try:
        write_json(context["backup"] / "failure.json", result)
    except BaseException:
        result["failureReceiptWriteFailed"] = True
    return result


def apply_change(context):
    # Recheck drain before writing any candidate or backup, then recheck after
    # staging. Retain partial output for inspection and refuse blind reruns.
    before, pid = verify_live(context, False)
    require(pid == context["pid"], "Test process changed after preflight")
    validate_drained(health(), context["previous"], context["active"])
    stage_and_backup_config(context)
    before, pid = verify_live(context, False)
    require(pid == context["pid"], "Test process changed during staging")
    # Refuse newly arrived users before entering the mutating/rollback region.
    validate_drained(health(), context["previous"], context["active"])
    try:
        prior_stop = normal_stop(pid)
        dump_hash = backup_private(context)
        unchanged(context, False)
        if context["newEnv"] != context["env"]:
            replace(ENV, context["newEnv"], 0o600, context["revision"], "upgrade")
        replace(UNITFILE, context["newUnit"], 0o644, context["revision"], "upgrade")
        run(["systemd-analyze", "verify", str(BASE_UNITFILE)])
        run(["systemctl", "daemon-reload"])
        run(["systemctl", "reset-failed", UNIT])
        context["candidateStarted"] = True
        run(["systemctl", "start", UNIT])
        first, candidate_pid = verify_live(context, True)
        validate_drained(health(), context["revision"], context["active"])
        candidate_stop = normal_stop(candidate_pid)
        run(["systemctl", "start", UNIT])
        after, candidate_pid = verify_live(context, True)
        result = {"ok": True, "applied": True, "revision": context["revision"],
                  "previousRevision": context["previous"], "binarySha256": context["fingerprint"][0],
                  "mapRoot": context["mapRoot"], "backup": str(context["backup"]), "databaseDumpSha256": dump_hash,
                  "priorStop": prior_stop, "candidateStop": candidate_stop, "candidateShutdownPanic": False,
                  "originalPid": context["original"]["pid"], "originalRelease": context["original"]["release"],
                  "capacity": after["capacity"], "pid": candidate_pid, "resourceLimitsUnchanged": context["resources"],
                  "envBeforeSha256": hashlib.sha256(context["env"]).hexdigest(),
                  "envAfterSha256": hashlib.sha256(context["newEnv"]).hexdigest(),
                  "modifiedDropInPath": str(UNITFILE), "baseUnitSha256Unchanged": BASE_UNIT_SHA256}
        write_json(context["backup"] / "result.json", result)
        return result
    except BaseException as error:
        return rollback(context, error)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--revision", required=True)
    parser.add_argument("--expected-current", required=True)
    parser.add_argument("--manifest-sha256", required=True)
    parser.add_argument("--expected-active", type=int, choices=(15, 51, 101), default=15)
    parser.add_argument("--apply", action="store_true", help="Execute after strict preflight; default is read-only")
    args = parser.parse_args(argv)
    try:
        context = preflight(args.revision, args.expected_current, args.manifest_sha256, args.expected_active)
        result = apply_change(context) if args.apply else {
            "ok": True, "applied": False, "revision": args.revision, "previousRevision": args.expected_current,
            "preservedCapacity": limits(args.expected_active), "backup": str(context["backup"]),
            "original": context["original"], "resourceLimitsUnchanged": context["resources"],
            "mapRoot": context["mapRoot"], "binarySha256": context["fingerprint"][0],
            "modifiedEnvironmentKeys": [], "modifiedUnitKeys": ["ExecStart"],
            "modifiedDropInPath": str(UNITFILE), "baseUnitSha256Unchanged": BASE_UNIT_SHA256}
    except BaseException as error:
        result = {"ok": False, "applied": False,
                  "error": str(error) if isinstance(error, SafetyError) else type(error).__name__}
    print(json.dumps(result))
    return 0 if result["ok"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
