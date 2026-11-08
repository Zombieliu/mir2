#!/usr/bin/env python3
"""One TEST cutover; no health claim, database restore, deletion or formal-service control."""
import sys; sys.dont_write_bytecode = True
import argparse, contextlib, hashlib, importlib.util, itertools, json, os, pathlib, re, shlex, stat, subprocess, time

SERVICE = "mir2-playtest.service"
SOURCE = "d53f9e3b22e2b92c2dc86644b6ae370fc1acab53"
OLD = "/opt/mir2-playtest/releases/e9117b6691e3f7ec238ff1c1eb77b699b0646c2f/mir2-gateway"
NEWROOT = "/opt/mir2-playtest/releases/" + SOURCE + "-r16-qa"
NEW = NEWROOT + "/mir2-gateway"
BINARY_SHA = "697696b221e1c5d98f006f96217d9ddddd246884b90e885a122d2fd03419d181"
OLDENV = "/etc/mir2-playtest/unified-e9117b6691e3f7ec238ff1c1eb77b699b0646c2f-e9117b6620261101.env"
OLD_ENVS = (
    "/etc/mir2-playtest/gateway.env",
    "/var/lib/mir2-playtest-spectator-stage/spectator-feature.env",
    "/etc/mir2-playtest/wallet-rollout-00ca0f93f0c7e367afec495711e0e04334f4deb8.env",
    "/etc/mir2-playtest-testnet/private/test-chain-keys.env",
    "/etc/mir2-playtest-testnet/playtest-testnet.env",
    "/etc/mir2-playtest/deployment-e824.env",
    "/etc/mir2-playtest/wallet-account-fix-2decb491856dcd8d9b91aa40c7fca32dcf18101a.env",
    "/etc/mir2-playtest/promotion-bridge-050950f5b9773a36901861a1a95c18b8058793a5.env",
    "/etc/mir2-playtest/usdc-native-868ef2d2b24de2ab2c577bbe2f3b1667829c9bc3.env",
    "/etc/mir2-playtest/promotion-migration-173332a004c01312802a67042ff7800a04893134.env",
    "/etc/mir2-playtest/full-monthly-13fc2e9f60203e65df152792d9ee63fb059065db-36398409984517bc.env",
    OLDENV,
)
ENV = "/etc/mir2-r16-release/r16-d53f9e3b2-20261108-r4.env"
DROP = "/etc/systemd/system/" + SERVICE + ".d/" + "z" * 20 + "-r16-d53f9e3b2-r4.conf"
BACKUP = "/root/r16-playtest-cutover-d53f9e3b2-20261108-r4"
STATE = "/var/lib/mir2-playtest/state"
ORIGINS = ("https://numeron.obelisk.build", "https://numeron-r16-web.vercel.app", "https://numeron-r16-web-obelisk-labs.vercel.app")
PROPS = "FragmentPath,DropInPaths,EnvironmentFiles,ExecStart,User,Group,WorkingDirectory,KillSignal,Restart,TimeoutStopUSec,ActiveState,SubState,MainPID"
TOOLS_SHA = "03c079ee51a6297dac1728df004c57df58bee1ddcdd65d74308b5cc4b6a5878b"
T, END, BKP = None, 0, None
def need(ok, code):
    if not ok: raise RuntimeError(code)

def command(argv, *, data=None, output=None, timeout=4, reserve=5):
    if argv[0] == "/usr/bin/systemctl": need(argv[1:] in (["daemon-reload"], ["show", SERVICE, "--property=" + PROPS], ["stop", "--no-block", SERVICE], ["start", "--no-block", SERVICE]), "wrong_service")
    remaining = END - time.monotonic() - reserve; need(remaining > 0, "deadline")
    result = subprocess.run(argv, input=data, stdout=output or subprocess.PIPE, stderr=subprocess.PIPE, timeout=min(timeout, remaining),
                            env={"PATH": "/usr/sbin:/usr/bin:/sbin:/bin", "LC_ALL": "C", "PGCONNECT_TIMEOUT": "3", "PGOPTIONS": "-c default_transaction_read_only=on -c statement_timeout=8000"})
    need(result.returncode == 0, "command_failed")
    need(output is not None or len(result.stdout) <= 1048576, "command_output_bound")
    return result.stdout

def parse_properties(raw):
    result, files = {}, []
    for line in raw.decode().splitlines():
        need("=" in line, "property_format"); key, value = line.split("=", 1)
        need(key in PROPS.split(","), "property_unexpected")
        if key == "EnvironmentFiles": need(bool(value.strip()), "property_empty_environment_file"); files.append(value)
        else:
            need(key not in result, "property_duplicate"); result[key] = value
    need(files and set(result) == set(PROPS.split(",")) - {"EnvironmentFiles"}, "property_missing")
    result["EnvironmentFiles"] = " ".join(files)
    return result

def properties(reserve=5):
    return parse_properties(command(["/usr/bin/systemctl", "show", SERVICE, "--property=" + PROPS], reserve=reserve))

def envfiles(props):
    need(re.fullmatch(r"(?:/\S+ \(ignore_errors=(?:no|yes)\)(?: |$))+", props["EnvironmentFiles"].strip()), "environment_files_format")
    return re.findall(r"(\S+) \(ignore_errors=(?:no|yes)\)", props["EnvironmentFiles"])

def guard(props, executable, envs):
    for key, expected in {"User": "mir2-playtest", "Group": "mir2-playtest", "WorkingDirectory": STATE, "KillSignal": "2", "Restart": "on-failure", "TimeoutStopUSec": "2min"}.items(): need(props.get(key) == expected, "service_properties_changed")
    need(envfiles(props) == envs and re.search(r"\bpath=" + re.escape(executable) + r"(?:\s|;)", props["ExecStart"]), "effective_command_changed")
    drops = shlex.split(props["DropInPaths"])
    need(drops and pathlib.Path(DROP).name > max(pathlib.Path(p).name for p in drops if p != DROP), "dropin_order")
    return drops

def legacy_env_fd(path):
    # Only these exact existing read-only paths permit nonzero directory GIDs.
    need(path in OLD_ENVS, "legacy_path_not_allowlisted")
    parts = T.relative(path[1:]); fd = os.open("/", T.DIR_FLAGS)
    try:
        for component in ["", *parts[:-1]]:
            if component:
                child = os.open(component, T.DIR_FLAGS, dir_fd=fd); os.close(fd); fd = child
            st = os.fstat(fd); need(stat.S_ISDIR(st.st_mode) and st.st_uid == 0 and not stat.S_IMODE(st.st_mode) & 0o022, "legacy_directory_trust")
        return os.open(parts[-1], T.READ_FLAGS, dir_fd=fd)
    finally: os.close(fd)

def readfile(path, limit=1048576, service=False, directory_fd=None):
    if service: import pwd
    fd = os.open(path, T.READ_FLAGS, dir_fd=directory_fd) if directory_fd is not None else legacy_env_fd(path) if path in OLD_ENVS else T.open_absolute_file(path, root_owned=not service)
    try:
        before = T.regular(fd); need(before.st_size <= limit and before.st_uid in ((0, pwd.getpwnam("mir2-playtest").pw_uid) if service and path not in OLD_ENVS else (0,)) and not stat.S_IMODE(before.st_mode) & 0o022, "source_file_identity")
        data = os.read(fd, limit + 1); need(len(data) == before.st_size and T.stable(before) == T.stable(os.fstat(fd)), "source_file_changed")
        return data, before
    finally: os.close(fd)

def proc_bytes(pid, leaf):
    fd = os.open(f"/proc/{pid}/{leaf}", T.READ_FLAGS)
    try:
        value = os.read(fd, 262145); need(len(value) <= 262144, "proc_bound"); return value
    finally: os.close(fd)

def environment(raw):
    rows = [entry.split(b"=", 1) for entry in raw.split(b"\0") if entry]
    need(all(len(row) == 2 for row in rows), "proc_environment_invalid")
    result = {k.decode(): v.decode() for k, v in rows}; need(len(result) == len(rows), "duplicate_environment"); return result

def render_env(values):
    original = values["MIR2_ALLOWED_WEB_ORIGINS"]; need(not any(ord(c) < 32 for c in original), "origin_control_character")
    combined = original + "".join("," + origin for origin in ORIGINS if origin not in [part.strip() for part in original.split(",")])
    if not original: combined = combined.lstrip(",")
    updates = {"MIR2_CONTENT_PROFILE": "platinum_fiveclass", "MIR2_QUEST_CADENCE": "newcomer-v2", "MIR2_CRYSTAL_MAP_PACK": NEWROOT + "/crystal-map-pack", "MIR2_ALLOWED_WEB_ORIGINS": combined}
    return "".join(key + "=" + json.dumps(value, ensure_ascii=False) + "\n" for key, value in updates.items()).encode()

def preflight():
    fd = T.open_chain(os.path.dirname(ENV), root_owned=True)
    try:
        st = os.fstat(fd); need(st.st_uid == st.st_gid == 0 and stat.S_IMODE(st.st_mode) == 0o700, "new_env_parent_trust")
    finally: os.close(fd)
    props = properties(); paths = [props["FragmentPath"], *guard(props, OLD, list(OLD_ENVS)), *OLD_ENVS]
    need(props["ActiveState"] == "active" and props["MainPID"].isdigit() and int(props["MainPID"]) > 1, "old_service_not_active")
    need(os.readlink("/proc/" + props["MainPID"] + "/exe") == OLD, "old_process_changed")
    for path in (ENV, DROP, ENV + ".failed", DROP + ".failed", BACKUP): need(not os.path.lexists(path), "target_not_fresh")
    provenance = json.loads(readfile(NEWROOT + "/provenance.json")[0]); need(provenance["sourceSha"] == SOURCE and provenance["artifactSha256"] == "62c2df7e5b899767331eaca78879e24eee2ff24b9e5557476adb554a7c3123e9", "new_provenance")
    pin = next(row for row in provenance["files"] if row["path"] == "mir2-gateway"); need(pin["sha256"] == BINARY_SHA and pin["bytes"] == 103118328, "new_binary_pin")
    fd = T.open_absolute_file(NEW, root_owned=True)
    try: T.digest_fd(fd, pin["bytes"], BINARY_SHA, uid=0, mode=0o755)
    finally: os.close(fd)
    configs = {path: readfile(path)[0] for path in paths}; raw = proc_bytes(props["MainPID"], "environ")
    return props, configs, raw, proc_bytes(props["MainPID"], "stat")

def copy_small(directory, target, optional=False):
    import pwd
    allowed = {0, pwd.getpwnam("mir2-playtest").pw_uid}
    src = T.open_chain(directory)
    try: dst = T.child_dir(BKP, target, 0, create=True)
    except BaseException: os.close(src); raise
    rows = []; totals = {"files": 0, "dirs": 0, "bytes": 0}; seen = set()
    def scan(fd):
        with os.scandir(fd) as entries: names = [entry.name for entry in itertools.islice(entries, 257 if optional else 385)]
        need(len(names) <= (256 if optional else 384), "backup_member_bound")
        return sorted(names)
    def safe_dir(fd):
        value = os.fstat(fd)
        need(stat.S_ISDIR(value.st_mode) and value.st_uid in allowed and not stat.S_IMODE(value.st_mode) & 0o022, "backup_directory_trust")
        return value
    def walk(source, destination, prefix="", depth=0):
        initial = safe_dir(source); identity = (initial.st_dev, initial.st_ino)
        need(depth <= 8 and totals["dirs"] < 128 and identity not in seen, "backup_directory_bound")
        seen.add(identity); totals["dirs"] += 1; names = scan(source)
        for name in names:
            before = os.stat(name, dir_fd=source, follow_symlinks=False); relative_name = prefix + name
            if stat.S_ISDIR(before.st_mode) and not optional:
                need(depth < 8 and totals["dirs"] < 128, "backup_directory_bound")
                child = os.open(name, T.DIR_FLAGS, dir_fd=source)
                try:
                    need(T.stable(before) == T.stable(safe_dir(child)), "backup_path_changed")
                    os.mkdir(name, 0o700, dir_fd=destination); out = os.open(name, T.DIR_FLAGS, dir_fd=destination)
                    try: walk(child, out, relative_name + "/", depth + 1)
                    finally: os.close(out)
                    need(T.stable(before) == T.stable(os.stat(name, dir_fd=source, follow_symlinks=False)), "backup_path_changed")
                    rows.append({"name": relative_name, "type": "directory", "uid": before.st_uid, "gid": before.st_gid, "mode": stat.S_IMODE(before.st_mode)})
                finally: os.close(child)
                continue
            if optional and (not stat.S_ISREG(before.st_mode) or before.st_size > 1048576): continue
            need(stat.S_ISREG(before.st_mode) and before.st_nlink == 1 and before.st_uid in allowed and not stat.S_IMODE(before.st_mode) & 0o022
                 and before.st_size <= 1048576 and totals["files"] < 256 and totals["bytes"] + before.st_size <= 8388608, "backup_tree_bound")
            data, after = readfile(name, service=True, directory_fd=source)
            need(T.stable(before) == T.stable(after) == T.stable(os.stat(name, dir_fd=source, follow_symlinks=False)), "backup_path_changed")
            T.write_new(destination, name, data); totals["files"] += 1; totals["bytes"] += len(data)
            rows.append({"name": relative_name, "bytes": len(data), "sha256": hashlib.sha256(data).hexdigest(), "uid": after.st_uid, "gid": after.st_gid, "mode": stat.S_IMODE(after.st_mode)})
        need(names == scan(source) and T.stable(initial) == T.stable(safe_dir(source)), "backup_directory_changed")
        os.fsync(destination)
    try:
        initial = safe_dir(src); walk(src, dst)
        current = T.open_chain(directory)
        try: need(T.stable(initial) == T.stable(safe_dir(current)), "backup_root_changed")
        finally: os.close(current)
        return rows
    finally: os.close(src); os.close(dst)
SQL = """BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY; SET LOCAL statement_timeout='8s';
SELECT json_build_object('counts',json_build_object('schema_migrations',(SELECT count(*) FROM schema_migrations),'accounts',(SELECT count(*) FROM accounts),'characters',(SELECT count(*) FROM characters),'character_saves',(SELECT count(*) FROM character_saves)));
SELECT format('SELECT json_build_object(''table'',%L,''count'',count(*),''md5'',md5(coalesce(string_agg(to_jsonb(t)::text,E''\\n'' ORDER BY to_jsonb(t)::text),''''))) FROM public.%I t;',tablename,tablename) FROM pg_tables WHERE schemaname='public' ORDER BY tablename
\\gexec
SELECT json_build_object('raw','accounts','rows',json_agg(json_build_array(md5(account_id),md5(raw_json::text)) ORDER BY account_id)) FROM accounts;
SELECT json_build_object('raw','characters','rows',json_agg(json_build_array(md5(jsonb_build_array(account_id,character_index)::text),md5(raw_json::text)) ORDER BY account_id,character_index)) FROM characters;
SELECT json_build_object('raw','character_saves','rows',json_agg(json_build_array(md5(jsonb_build_array(account_id,character_index)::text),md5(snapshot_json::text),stage5_systems_json IS NULL,md5(stage5_systems_json::text)) ORDER BY account_id,character_index)) FROM character_saves;
SELECT json_build_object('projection','character_state','rows',json_agg(json_build_array(md5(jsonb_build_array(account_id,character_index)::text),md5(to_jsonb(t)::text)) ORDER BY account_id,character_index)) FROM character_state t;
COMMIT;"""
def stop_old():
    command(["/usr/bin/systemctl", "stop", "--no-block", SERVICE]); until = min(time.monotonic() + 45, END - 13)
    while time.monotonic() < until:
        state = properties()
        if state["ActiveState"] == "inactive" and state["MainPID"] == "0": return True
        need(state["ActiveState"] in ("active", "deactivating"), "stop_failed"); time.sleep(0.25)
    return False

def write_owned(path, data, owned):
    parent, name = os.path.split(path); parent_fd = T.open_chain(parent, root_owned=True)
    try:
        fd = os.open(name, T.CREATE_FLAGS, 0o600, dir_fd=parent_fd); st = os.fstat(fd)
        owned[path] = {"sha": hashlib.sha256(data).hexdigest(), "inode": (st.st_dev, st.st_ino)}
        try: T.write_all(fd, data); os.fsync(fd); T.regular(fd, len(data), uid=0, mode=0o600)
        finally: os.close(fd)
        os.fsync(parent_fd)
    finally: os.close(parent_fd)

def rollback(owned, configs):
    need(all(path in owned or not os.path.lexists(path) for path in (ENV, DROP)), "rollback_unowned_file")
    for path, expected in owned.items():
        content, st = readfile(path); need(hashlib.sha256(content).hexdigest() == expected["sha"] and (st.st_dev, st.st_ino) == expected["inode"], "rollback_not_owned")
    for path, original in configs.items(): need(readfile(path)[0] == original, "old_config_changed")
    for path in owned:
        parent, name = os.path.split(path); fd = T.open_chain(parent, root_owned=True)
        try: T.rename_noreplace(fd, fd, name, name + ".failed")
        finally: os.close(fd)
    command(["/usr/bin/systemctl", "daemon-reload"], reserve=0); guard(properties(reserve=0), OLD, list(OLD_ENVS))
    command(["/usr/bin/systemctl", "start", "--no-block", SERVICE], reserve=0)

def execute(apply):
    global BKP
    props, configs, raw, proc_stat = preflight(); values = environment(raw); update = render_env(values)
    recovery_fd = T.open_chain(values["MIR2_SAVE_RECOVERY_DIR"]); os.close(recovery_fd)
    if not apply: return {"phase": "plan-ready", "healthy": False}
    os.mkdir(BACKUP, 0o700); BKP = T.open_chain(BACKUP, root_owned=True); owned = {}; stopped = attempted = False
    try:
        for index, data in enumerate(configs.values()): T.write_new(BKP, f"config-{index:03}.private", data)
        T.write_new(BKP, "proc-environment.private", raw); T.write_new(BKP, "proc-stat.private", proc_stat)
        T.write_new(BKP, "before.private.json", (json.dumps({"properties": props, "configs": [{"path": p, "bytes": len(d), "sha256": hashlib.sha256(d).hexdigest(), "stat": list(os.stat(p, follow_symlinks=False))} for p, d in configs.items()]}) + "\n").encode()); os.fsync(BKP)
        need(properties() == props and all(readfile(p)[0] == d for p, d in configs.items()) and os.readlink("/proc/" + props["MainPID"] + "/exe") == OLD and proc_bytes(props["MainPID"], "stat").rsplit(b")", 1)[1].split()[19] == proc_stat.rsplit(b")", 1)[1].split()[19], "prestop_changed")
        if not stop_old(): return {"phase": "stop-pending", "healthy": False, "manualReconcile": True}
        stopped = True
        with T.FileSink(BKP, "final.pg_dump") as sink: command(["/usr/sbin/runuser", "-u", "postgres", "--", "/usr/bin/pg_dump", "--no-password", "-Fc", "--dbname=mir2_playtest"], output=sink.fd, timeout=8)
        snapshot = command(["/usr/sbin/runuser", "-u", "postgres", "--", "/usr/bin/psql", "-XqAt", "--no-password", "-v", "ON_ERROR_STOP=1", "--dbname=mir2_playtest"], data=SQL.encode(), timeout=8); T.write_new(BKP, "sql-hashes.private.jsonl", snapshot)
        T.write_new(BKP, "filesystem.private.json", (json.dumps({"recovery": copy_small(values["MIR2_SAVE_RECOVERY_DIR"], "recovery"), "stateTopLevelSmallOnly": copy_small(STATE, "state", True)}) + "\n").encode())
        write_owned(ENV, update, owned); write_owned(DROP, ("[Service]\nExecStart=\nExecStart=" + NEW + "\nEnvironmentFile=" + ENV + "\n").encode(), owned)
        command(["/usr/bin/systemctl", "daemon-reload"]); guard(properties(), NEW, [*OLD_ENVS, ENV])
        for path, expected in owned.items():
            content, st = readfile(path); need(hashlib.sha256(content).hexdigest() == expected["sha"] and (st.st_dev, st.st_ino) == expected["inode"], "new_config_changed")
        attempted = True
        command(["/usr/bin/systemctl", "start", "--no-block", SERVICE]); return {"phase": "new-start-requested", "healthy": False}
    except Exception as error:
        with contextlib.suppress(Exception): T.write_new(BKP, "failure.private.json", json.dumps({"type": type(error).__name__, "reason": str(error)}).encode())
        if stopped and not attempted:
            try: rollback(owned, configs); return {"phase": "old-restart-requested", "healthy": False, "manualReconcile": False}
            except Exception: pass
        return {"phase": "manual-reconcile", "healthy": False, "manualReconcile": True}
    finally: os.close(BKP); BKP = None
def main():
    global T, END
    parser = argparse.ArgumentParser(description=__doc__); modes = parser.add_mutually_exclusive_group(required=True); modes.add_argument("--plan", action="store_true"); modes.add_argument("--apply", action="store_true"); args = parser.parse_args()
    need(sys.platform == "linux" and os.geteuid() == 0 and sys.flags.isolated, "isolated_root_linux_required"); END = time.monotonic() + 59
    path = pathlib.Path(__file__).absolute().with_name("r16_candidate_install.py")
    for trusted in (pathlib.Path(__file__).absolute(), path, *path.parents):
        st = trusted.lstat(); need(st.st_uid == st.st_gid == 0 and not stat.S_ISLNK(st.st_mode) and not stat.S_IMODE(st.st_mode) & 0o022 and (stat.S_ISDIR(st.st_mode) or stat.S_ISREG(st.st_mode) and st.st_nlink == 1), "untrusted_tools_path")
    payload = path.read_bytes(); need(hashlib.sha256(payload).hexdigest() == TOOLS_SHA, "reviewed_tools_changed")
    spec = importlib.util.spec_from_file_location("r16_candidate_install", path); T = importlib.util.module_from_spec(spec); sys.modules[spec.name] = T; exec(compile(payload, str(path), "exec"), T.__dict__); T.linux_context("publish")
    result = execute(args.apply); print(json.dumps(result)); return 0 if result["phase"] in ("plan-ready", "new-start-requested") else 2
if __name__ == "__main__":
    try: raise SystemExit(main())
    except Exception: print(json.dumps({"phase": "preflight-failed", "healthy": False, "manualReconcile": True})); raise SystemExit(1)
