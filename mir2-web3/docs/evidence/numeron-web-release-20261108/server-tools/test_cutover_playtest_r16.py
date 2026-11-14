"""Only synthetic properties and mocked I/O. No systemctl, PostgreSQL or remote use."""
import contextlib
import errno
import hashlib
import importlib.util
import json
import os
import pathlib
import subprocess
import stat
import sys
import tempfile
import types
import unittest
from unittest.mock import Mock, patch

path = pathlib.Path(__file__).with_name("cutover_playtest_r16.py")
spec = importlib.util.spec_from_file_location("cutover_playtest_r16", path)
m = importlib.util.module_from_spec(spec); sys.modules[spec.name] = m; spec.loader.exec_module(m)


def properties(new=False):
    old_drop = "/etc/systemd/system/mir2-playtest.service.d/" + "z" * 12 + "-unified-fixture.conf"
    return {"User": "mir2-playtest", "Group": "mir2-playtest", "WorkingDirectory": m.STATE, "KillSignal": "2",
            "Restart": "on-failure", "TimeoutStopUSec": "2min", "FragmentPath": "/etc/systemd/system/mir2-playtest.service",
            "DropInPaths": old_drop + (" " + m.DROP if new else ""),
            "ExecStart": "{ path=" + (m.NEW if new else m.OLD) + " ; argv[]=fixture ; }",
            "EnvironmentFiles": " ".join(name + " (ignore_errors=no)" for name in (*m.OLD_ENVS, *((m.ENV,) if new else ()))),
            "MainPID": "0" if new else "123", "ActiveState": "inactive" if new else "active", "SubState": "running"}


def multiline_show(new=False, envs=None):
    values = properties(new); lines = []
    for key, value in values.items():
        if key == "EnvironmentFiles":
            for name in envs if envs is not None else (*m.OLD_ENVS, *((m.ENV,) if new else ())):
                lines.append("EnvironmentFiles=" + name + " (ignore_errors=no)")
        else: lines.append(key + "=" + value)
    return ("\n".join(lines) + "\n").encode()


class MultilinePropertiesTests(unittest.TestCase):
    def test_actual_repeated_environmentfiles_shape_preserves_twelve_then_thirteen_in_order(self):
        self.assertEqual(len(m.OLD_ENVS), 12); self.assertEqual(len(set(m.OLD_ENVS)), 12)
        old_raw = multiline_show()
        naive = dict(line.split("=", 1) for line in old_raw.decode().splitlines())
        self.assertEqual(m.envfiles(naive), [m.OLDENV], "the old dict parser lost the first11 records")
        for new, executable, expected in [(False, m.OLD, list(m.OLD_ENVS)), (True, m.NEW, [*m.OLD_ENVS, m.ENV])]:
            with patch.object(m, "command", return_value=multiline_show(new)):
                value = m.properties()
            self.assertEqual(m.envfiles(value), expected)
            self.assertEqual(m.guard(value, executable, expected), value["DropInPaths"].split())
            self.assertEqual(m.parse_properties(multiline_show(new)), value)

    def test_every_other_duplicate_missing_unknown_or_malformed_property_rejects(self):
        raw = multiline_show()
        for key in m.PROPS.split(","):
            if key == "EnvironmentFiles": continue
            with self.subTest(key=key):
                with self.assertRaisesRegex(RuntimeError, "property_duplicate"): m.parse_properties(raw + key.encode() + b"=duplicate\n")
        for changed, code in [(raw + b"Unexpected=fixture\n", "property_unexpected"), (raw + b"no-equals\n", "property_format"),
                              (raw + b"EnvironmentFiles=\n", "property_empty_environment_file"),
                              (b"\n".join(line for line in raw.splitlines() if not line.startswith(b"User=")), "property_missing")]:
            with self.assertRaisesRegex(RuntimeError, code): m.parse_properties(changed)

    def test_missing_reordered_duplicate_and_foreign_env_paths_fail_the_complete_guard(self):
        for names in [m.OLD_ENVS[1:], (*m.OLD_ENVS[:-1], "/foreign.env"), (*m.OLD_ENVS, m.OLDENV), tuple(reversed(m.OLD_ENVS))]:
            value = m.parse_properties(multiline_show(envs=names))
            with self.assertRaisesRegex(RuntimeError, "effective_command_changed"): m.guard(value, m.OLD, list(m.OLD_ENVS))

    def test_preflight_reads_and_returns_all_effective_envfiles_not_just_the_last(self):
        tools = types.SimpleNamespace(open_chain=Mock(return_value=7), open_absolute_file=Mock(return_value=8), digest_fd=Mock())
        provenance = {"sourceSha": m.SOURCE, "artifactSha256": "62c2df7e5b899767331eaca78879e24eee2ff24b9e5557476adb554a7c3123e9",
                      "files": [{"path": "mir2-gateway", "sha256": m.BINARY_SHA, "bytes": 103118328}]}
        seen = []
        def read(path):
            seen.append(path)
            return (json.dumps(provenance).encode() if path.endswith("/provenance.json") else ("synthetic-config:" + path).encode(), None)
        with patch.object(m, "T", tools), patch.object(m, "command", return_value=multiline_show()), patch.object(m, "readfile", side_effect=read), \
             patch.object(m, "proc_bytes", return_value=b"synthetic-proc"), patch.object(m.os, "fstat", return_value=types.SimpleNamespace(st_uid=0, st_gid=0, st_mode=stat.S_IFDIR | 0o700)), \
             patch.object(m.os, "readlink", return_value=m.OLD), patch.object(m.os.path, "lexists", return_value=False), patch.object(m.os, "close"):
            value, configs, _, _ = m.preflight()
        self.assertEqual(m.envfiles(value), list(m.OLD_ENVS))
        self.assertEqual(set(configs), {value["FragmentPath"], *value["DropInPaths"].split(), *m.OLD_ENVS})
        for name in m.OLD_ENVS: self.assertEqual(seen.count(name), 1)


class PureGuards(unittest.TestCase):
    def test_old_and_new_real_property_shapes_and_twenty_z_order(self):
        self.assertTrue(m.guard(properties(), m.OLD, list(m.OLD_ENVS)))
        self.assertTrue(m.guard(properties(True), m.NEW, [*m.OLD_ENVS, m.ENV]))
        self.assertEqual(pathlib.Path(m.DROP).name.split("-", 1)[0], "z" * 20)
        self.assertGreater(pathlib.Path(m.DROP).name, "z" * 12 + "-unified-fixture.conf")

    def test_each_identity_and_stop_semantic_mismatch_rejects(self):
        for change in [{"User": "root"}, {"Group": "other"}, {"KillSignal": "15"}, {"Restart": "always"},
                       {"TimeoutStopUSec": "5s"}, {"WorkingDirectory": "/var/lib/mir2/state"}, {"ExecStart": "{ path=/opt/mir2/mir2-gateway ; }"},
                       {"DropInPaths": "/etc/systemd/system/mir2-playtest.service.d/" + "z" * 21 + "-foreign.conf"}]:
            with self.assertRaises(RuntimeError): m.guard({**properties(), **change}, m.OLD, list(m.OLD_ENVS))

    def test_effective_env_order_and_unparsed_tail_reject(self):
        for value in [m.OLDENV, m.OLDENV + " (ignore_errors=no) /foreign", "/foreign (ignore_errors=no)",
                      m.ENV + " (ignore_errors=no) " + m.OLDENV + " (ignore_errors=no)"]:
            with self.assertRaises(RuntimeError): m.guard({**properties(), "EnvironmentFiles": value}, m.OLD, list(m.OLD_ENVS))

    def test_new_env_contains_only_four_authorized_changes_and_preserves_origin_bytes(self):
        old = "http://127.0.0.1:3000, https://original.example"
        value = m.render_env({"MIR2_ALLOWED_WEB_ORIGINS": old, "MIR2_PASSKEY_AUTH_SECRET": "synthetic-not-a-real-secret", "MIR2_REDIS_URL": "synthetic-private-url"}).decode()
        self.assertEqual(len(value.splitlines()), 4)
        self.assertEqual({line.split("=", 1)[0] for line in value.splitlines()}, {"MIR2_CONTENT_PROFILE", "MIR2_QUEST_CADENCE", "MIR2_CRYSTAL_MAP_PACK", "MIR2_ALLOWED_WEB_ORIGINS"})
        self.assertIn('MIR2_CONTENT_PROFILE="platinum_fiveclass"', value); self.assertIn('MIR2_QUEST_CADENCE="newcomer-v2"', value)
        self.assertIn(old + "," + ",".join(m.ORIGINS), value)
        self.assertNotIn("SECRET", value); self.assertNotIn("synthetic", value); self.assertNotIn("REDIS", value)
        for origin in m.ORIGINS:
            once = m.render_env({"MIR2_ALLOWED_WEB_ORIGINS": origin}).decode()
            self.assertEqual(once.count(origin), 1)
        with self.assertRaises(RuntimeError): m.render_env({"MIR2_ALLOWED_WEB_ORIGINS": "https://safe\nMIR2_BILLING=bad"})

    def test_only_exact_playtest_systemctl_commands_are_accepted(self):
        with patch.object(m, "END", m.time.monotonic() + 60), patch.object(m.subprocess, "run", return_value=types.SimpleNamespace(returncode=0, stdout=b"", stderr=b"")) as run:
            for tail in [["stop", "--no-block", m.SERVICE], ["start", "--no-block", m.SERVICE], ["daemon-reload"], ["show", m.SERVICE, "--property=" + m.PROPS]]:
                m.command(["/usr/bin/systemctl", *tail])
            self.assertEqual(run.call_count, 4)
            for tail in [["stop", "mir2-gateway.service"], ["start", "--no-block", m.SERVICE, "mir2-gateway.service"], ["stop", "--all"], ["restart", m.SERVICE]]:
                with self.assertRaisesRegex(RuntimeError, "wrong_service"): m.command(["/usr/bin/systemctl", *tail])
            self.assertEqual(run.call_count, 4)
            self.assertNotIn("MIR2", " ".join(run.call_args.kwargs["env"]))

    def test_sql_is_one_readonly_repeatable_transaction_with_all_tables_and_private_raw_hashes(self):
        self.assertEqual(m.SQL.count("BEGIN"), 1); self.assertEqual(m.SQL.count("COMMIT"), 1)
        self.assertIn("REPEATABLE READ READ ONLY", m.SQL); self.assertIn("FROM pg_tables WHERE schemaname='public'", m.SQL)
        self.assertIn("to_jsonb(t)::text", m.SQL); self.assertIn("\\gexec", m.SQL); self.assertIn("%I", m.SQL); self.assertIn("%L", m.SQL)
        for token in ["schema_migrations", "accounts", "characters", "character_saves", "snapshot_json", "raw_json", "stage5_systems_json IS NULL", "character_state"]:
            self.assertIn(token, m.SQL)
        self.assertNotRegex(m.SQL, r"\b(?:INSERT|UPDATE|DELETE|DROP|ALTER|CREATE|TRUNCATE)\b")

    def test_real_stop_polling_returns_pending_without_start_or_forcekill(self):
        times = iter(range(1000))
        with patch.object(m, "END", 100), patch.object(m.time, "monotonic", side_effect=lambda: next(times)), patch.object(m.time, "sleep"), \
             patch.object(m, "properties", return_value={"ActiveState": "deactivating", "MainPID": "123"}), patch.object(m, "command") as command:
            self.assertFalse(m.stop_old())
            command.assert_called_once_with(["/usr/bin/systemctl", "stop", "--no-block", m.SERVICE])
        with patch.object(m, "END", 100), patch.object(m.time, "monotonic", return_value=0), \
             patch.object(m, "properties", return_value={"ActiveState": "inactive", "MainPID": "0"}), patch.object(m, "command"):
            self.assertTrue(m.stop_old())

    def test_partial_write_is_recorded_before_the_first_write_can_fail(self):
        tools = types.SimpleNamespace(open_chain=Mock(return_value=7), CREATE_FLAGS=0, write_all=Mock(side_effect=OSError("fixture-write-failed")))
        owned = {}
        with patch.object(m, "T", tools), patch.object(m.os, "open", return_value=8), patch.object(m.os, "close"), \
             patch.object(m.os, "fstat", return_value=types.SimpleNamespace(st_dev=1, st_ino=2)):
            with self.assertRaises(OSError): m.write_owned(m.ENV, b"complete-expected-config", owned)
        self.assertEqual(owned[m.ENV], {"sha": hashlib.sha256(b"complete-expected-config").hexdigest(), "inode": (1, 2)})


class LegacyReadPathTests(unittest.TestCase):
    def setUp(self):
        self.stack = contextlib.ExitStack(); self.addCleanup(self.stack.close)
        self.stats = {10: types.SimpleNamespace(st_uid=0, st_gid=0, st_mode=stat.S_IFDIR | 0o755),
                      11: types.SimpleNamespace(st_uid=0, st_gid=123, st_mode=stat.S_IFDIR | 0o755),
                      12: types.SimpleNamespace(st_uid=0, st_gid=985, st_mode=stat.S_IFDIR | 0o710)}
        self.opens = []
        self.tools = types.SimpleNamespace(DIR_FLAGS=0x12000, READ_FLAGS=0x23000, relative=lambda path: path.split("/"))
        def opened(name, flags, *, dir_fd=None):
            self.opens.append((name, flags, dir_fd))
            return {"/": 10, "etc": 11, "mir2-playtest": 12}.get(name, 13)
        self.stack.enter_context(patch.object(m, "T", self.tools))
        self.stack.enter_context(patch.object(m.os, "open", side_effect=opened))
        self.stack.enter_context(patch.object(m.os, "fstat", side_effect=lambda fd: self.stats[fd]))
        self.close = self.stack.enter_context(patch.object(m.os, "close"))

    def test_root_owned_nonzero_parent_gid_and_service_0710_leaf_directory_are_readable(self):
        self.assertEqual(m.legacy_env_fd(m.OLDENV), 13)
        self.assertEqual(self.opens, [("/", self.tools.DIR_FLAGS, None), ("etc", self.tools.DIR_FLAGS, 10),
                                    ("mir2-playtest", self.tools.DIR_FLAGS, 11), (m.OLDENV.rsplit("/", 1)[1], self.tools.READ_FLAGS, 12)])
        self.assertEqual(self.close.call_count, 3)

    def test_nonroot_owner_is_rejected_at_every_component(self):
        for fd in self.stats:
            with self.subTest(fd=fd):
                self.stats[fd].st_uid = 1000
                with self.assertRaisesRegex(RuntimeError, "legacy_directory_trust"): m.legacy_env_fd(m.OLDENV)
                self.stats[fd].st_uid = 0

    def test_group_or_other_write_is_rejected_at_every_component(self):
        for fd, data in self.stats.items():
            original = data.st_mode
            for bit in (0o020, 0o002):
                data.st_mode = original | bit
                with self.assertRaisesRegex(RuntimeError, "legacy_directory_trust"): m.legacy_env_fd(m.OLDENV)
            data.st_mode = original

    def test_symlink_component_cannot_be_followed(self):
        for component in ("etc", "mir2-playtest", m.OLDENV.rsplit("/", 1)[1]):
            normal = m.os.open.side_effect
            def opened(name, flags, *, dir_fd=None):
                if name == component: raise OSError(errno.ELOOP, "synthetic no-follow rejection")
                return normal(name, flags, dir_fd=dir_fd)
            with patch.object(m.os, "open", side_effect=opened):
                with self.assertRaises(OSError): m.legacy_env_fd(m.OLDENV)
            self.assertTrue(self.close.called)

    def test_only_exact_old_env_bypasses_shared_gid_zero_path_rule(self):
        st = types.SimpleNamespace(st_size=3, st_uid=0, st_gid=985, st_mode=stat.S_IFREG | 0o640)
        self.tools.regular = Mock(return_value=st); self.tools.stable = lambda _: "stable"
        self.tools.open_absolute_file = Mock(return_value=14)
        with patch.object(m, "legacy_env_fd", return_value=13) as old, patch.object(m.os, "read", return_value=b"env"), patch.object(m.os, "fstat", return_value=st):
            self.assertEqual(m.readfile(m.OLDENV)[0], b"env"); old.assert_called_once_with(m.OLDENV)
            self.assertFalse(self.tools.open_absolute_file.called)
            m.readfile(m.ENV)
            self.tools.open_absolute_file.assert_called_once_with(m.ENV, root_owned=True)
            self.assertEqual(old.call_count, 1)
            st.st_uid = 1000
            with self.assertRaisesRegex(RuntimeError, "source_file_identity"): m.readfile(m.OLDENV)
            st.st_uid = 0; st.st_mode |= 0o020
            with self.assertRaisesRegex(RuntimeError, "source_file_identity"): m.readfile(m.OLDENV)

    def test_all_twelve_exact_legacy_paths_use_readonly_reader_and_other_paths_do_not(self):
        st = types.SimpleNamespace(st_size=3, st_uid=0, st_gid=985, st_mode=stat.S_IFREG | 0o640)
        self.tools.regular = Mock(return_value=st); self.tools.stable = lambda _: "stable"; self.tools.open_absolute_file = Mock(return_value=14)
        with patch.object(m, "legacy_env_fd", return_value=13) as legacy, patch.object(m.os, "read", return_value=b"env"), patch.object(m.os, "fstat", return_value=st):
            for name in m.OLD_ENVS: self.assertEqual(m.readfile(name)[0], b"env")
            self.assertEqual([call.args[0] for call in legacy.call_args_list], list(m.OLD_ENVS))
            m.readfile("/etc/mir2-playtest/not-allowlisted.env")
            self.assertEqual(legacy.call_count, 12)
            self.tools.open_absolute_file.assert_called_once_with("/etc/mir2-playtest/not-allowlisted.env", root_owned=True)
        self.opens.clear()
        with self.assertRaisesRegex(RuntimeError, "legacy_path_not_allowlisted"): m.legacy_env_fd(m.ENV)
        self.assertEqual(self.opens, [])

    def test_new_env_parent_is_independent_root_group_zero_0700_and_never_created_by_plan(self):
        self.assertEqual(m.ENV, "/etc/mir2-r16-release/r16-d53f9e3b2-20261108-r4.env")
        self.tools.open_chain = Mock(return_value=7)
        for uid, gid, mode in [(1000, 0, 0o700), (0, 985, 0o700), (0, 0, 0o710), (0, 0, 0o720)]:
            with patch.object(m.os, "fstat", return_value=types.SimpleNamespace(st_uid=uid, st_gid=gid, st_mode=stat.S_IFDIR | mode)), patch.object(m, "properties") as props:
                with self.assertRaisesRegex(RuntimeError, "new_env_parent_trust"): m.preflight()
                self.assertFalse(props.called)
        with patch.object(m.os, "fstat", return_value=types.SimpleNamespace(st_uid=0, st_gid=0, st_mode=stat.S_IFDIR | 0o700)), \
             patch.object(m, "properties", side_effect=RuntimeError("after-parent-fixture")), patch.object(m.os, "mkdir", side_effect=AssertionError("plan wrote directory")):
            with self.assertRaisesRegex(RuntimeError, "after-parent-fixture"): m.preflight()
        self.tools.open_chain.assert_called_with("/etc/mir2-r16-release", root_owned=True)


class RollbackTests(unittest.TestCase):
    def setUp(self):
        self.owned = {m.ENV: {"sha": hashlib.sha256(b"env").hexdigest(), "inode": (1, 2)}, m.DROP: {"sha": hashlib.sha256(b"drop").hexdigest(), "inode": (1, 3)}}
        self.data = {m.ENV: (b"env", types.SimpleNamespace(st_dev=1, st_ino=2)), m.DROP: (b"drop", types.SimpleNamespace(st_dev=1, st_ino=3)), "old-config": (b"old", None)}
        self.tools = types.SimpleNamespace(open_chain=Mock(return_value=7), rename_noreplace=Mock())
        self.stack = contextlib.ExitStack(); self.addCleanup(self.stack.close)
        for target, value in [("T", self.tools), ("readfile", Mock(side_effect=lambda path: self.data[path])), ("command", Mock()), ("properties", Mock(return_value=properties()))]:
            self.stack.enter_context(patch.object(m, target, value))
        self.stack.enter_context(patch.object(m.os, "close")); self.stack.enter_context(patch.object(m.os.path, "lexists", return_value=True))

    def test_same_bytes_and_inode_quarantine_both_then_reload_and_start_only_original(self):
        m.rollback(self.owned, {"old-config": b"old"})
        self.assertEqual(self.tools.rename_noreplace.call_count, 2)
        for call in self.tools.rename_noreplace.call_args_list:
            self.assertEqual(call.args[-1], call.args[-2] + ".failed")
        self.assertEqual(m.command.call_args_list[-1].args[0], ["/usr/bin/systemctl", "start", "--no-block", m.SERVICE])
        self.assertEqual(m.properties.call_args.kwargs, {"reserve": 0})

    def test_foreign_content_or_inode_or_old_config_change_never_renames_or_starts(self):
        for bad in [(b"changed", types.SimpleNamespace(st_dev=1, st_ino=3)), (b"drop", types.SimpleNamespace(st_dev=1, st_ino=99))]:
            self.data[m.DROP] = bad
            with self.assertRaisesRegex(RuntimeError, "rollback_not_owned"): m.rollback(self.owned, {"old-config": b"old"})
            self.assertFalse(self.tools.rename_noreplace.called); self.assertFalse(m.command.called)
        self.data[m.DROP] = (b"drop", types.SimpleNamespace(st_dev=1, st_ino=3))
        with self.assertRaisesRegex(RuntimeError, "old_config_changed"): m.rollback(self.owned, {"old-config": b"another"})
        self.assertFalse(self.tools.rename_noreplace.called); self.assertFalse(m.command.called)

    def test_unrecorded_partial_or_concurrent_file_blocks_all_rollback(self):
        with self.assertRaisesRegex(RuntimeError, "rollback_unowned_file"): m.rollback({m.ENV: self.owned[m.ENV]}, {"old-config": b"old"})
        self.assertFalse(self.tools.rename_noreplace.called); self.assertFalse(m.command.called)

    def test_concurrent_effective_command_change_after_reload_never_starts(self):
        m.properties.return_value = {**properties(), "User": "foreign"}
        with self.assertRaisesRegex(RuntimeError, "service_properties_changed"): m.rollback(self.owned, {"old-config": b"old"})
        self.assertEqual(m.command.call_count, 1)


@unittest.skipUnless(sys.platform == "linux" and getattr(os, "geteuid", lambda: -1)() == 0,
                     "real Linux root dirfd/tree/ownership fixture; must run before cutover apply")
class RecoveryTreeTests(unittest.TestCase):
    def setUp(self):
        self.stack = contextlib.ExitStack(); self.addCleanup(self.stack.close)
        temporary = self.stack.enter_context(tempfile.TemporaryDirectory(prefix="r16-recovery-fixture-"))
        self.root = pathlib.Path(temporary).resolve()
        self.assertEqual(self.root.parent, pathlib.Path(tempfile.gettempdir()).resolve())
        self.assertTrue(self.root.name.startswith("r16-recovery-fixture-"))
        self.src, self.backup = self.root / "source", self.root / "private-backup"
        self.src.mkdir(mode=0o700); self.backup.mkdir(mode=0o700)
        spec = importlib.util.spec_from_file_location("r16_recovery_fixture_tools", path.with_name("r16_candidate_install.py"))
        self.tools = importlib.util.module_from_spec(spec); sys.modules[spec.name] = self.tools; spec.loader.exec_module(self.tools)
        self.stack.enter_context(patch.object(m, "T", self.tools))
        fd = self.tools.open_chain(str(self.backup)); self.stack.callback(os.close, fd)
        self.stack.enter_context(patch.object(m, "BKP", fd))

    def file(self, relative, data=b"synthetic-fixture"):
        target = self.src / relative; target.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
        target.write_bytes(data); target.chmod(0o600)
        return target

    def test_real_twelve_allowlisted_env_files_with_service_gid_directory_and_final_nlink(self):
        temporary = self.stack.enter_context(tempfile.TemporaryDirectory(prefix="r16-env-list-fixture-", dir="/root"))
        base = pathlib.Path(temporary).resolve()
        self.assertEqual(base.parent, pathlib.Path("/root").resolve()); self.assertTrue(base.name.startswith("r16-env-list-fixture-"))
        parent = base / "readonly-parent"; parent.mkdir(mode=0o750); os.chown(parent, 0, 123)
        legacy = parent / "service-directory"; legacy.mkdir(mode=0o710); os.chown(legacy, 0, 985)
        paths = []
        for index in range(12):
            leaf = legacy / ("config-" + str(index) + ".env"); leaf.write_bytes(("synthetic-private-config-" + str(index)).encode())
            leaf.chmod(0o640); os.chown(leaf, 0, 985); paths.append(str(leaf))
        with patch.object(m, "OLD_ENVS", tuple(paths)):
            for index, filename in enumerate(paths): self.assertEqual(m.readfile(filename)[0], ("synthetic-private-config-" + str(index)).encode())
            os.link(paths[0], legacy / "hardlink")
            with self.assertRaises(self.tools.SafetyError): m.readfile(paths[0])
            (legacy / "hardlink").unlink()
            leaf = pathlib.Path(paths[1]); leaf.unlink(); leaf.symlink_to(paths[0])
            with self.assertRaises(OSError): m.readfile(paths[1])

    def test_three_real_nested_levels_nonempty_and_empty_dirs_and_zero_byte_file(self):
        self.file("a/b/c/payload.bin", b"K" * 340); self.file("zero.bin", b""); (self.src / "empty").mkdir(mode=0o700)
        before = self.src.stat()
        self.assertGreater(before.st_nlink, 1, "a directory's normal nlink is not the regular-file contract")
        rows = m.copy_small(str(self.src), "recovery")
        target = self.backup / "recovery"
        self.assertEqual((target / "a/b/c/payload.bin").read_bytes(), b"K" * 340)
        self.assertEqual((target / "zero.bin").stat().st_size, 0)
        self.assertTrue((target / "empty").is_dir()); self.assertEqual(list((target / "empty").iterdir()), [])
        files = [row for row in rows if row.get("type") != "directory"]
        self.assertEqual(len(files), 2); self.assertEqual(sum(row["bytes"] for row in files), 340)
        self.assertIn("a/b/c", {row["name"] for row in rows if row.get("type") == "directory"})
        for actual in target.rglob("*"):
            self.assertFalse(actual.is_symlink()); self.assertEqual(actual.stat().st_uid, 0)
            self.assertEqual(actual.stat().st_mode & 0o777, 0o700 if actual.is_dir() else 0o600)
        self.assertEqual(self.tools.stable(before), self.tools.stable(self.src.stat()))

    def test_observed_shape_two_top_files_zero_and_340_bytes_plus_empty_child(self):
        self.file("zero.bin", b""); self.file("payload.bin", b"K" * 340); (self.src / "empty").mkdir(mode=0o700)
        self.assertEqual(self.src.stat().st_nlink, 3)
        rows = m.copy_small(str(self.src), "observed-shape")
        self.assertEqual(sum(row.get("bytes", 0) for row in rows), 340)
        self.assertEqual(sum(row.get("type") == "directory" for row in rows), 1)
        self.assertEqual((self.backup / "observed-shape/payload.bin").read_bytes(), b"K" * 340)
        self.assertTrue((self.backup / "observed-shape/empty").is_dir())

    def test_state_scope_still_copies_only_top_level_small_regular_files(self):
        self.file("one.bin", b"one"); self.file("two.bin", b"two"); self.file("nested/not-in-scope.bin", b"child")
        rows = m.copy_small(str(self.src), "state", True)
        self.assertEqual({row["name"] for row in rows}, {"one.bin", "two.bin"})
        self.assertEqual({p.name for p in (self.backup / "state").iterdir()}, {"one.bin", "two.bin"})

    def test_symbolic_file_directory_and_hardlink_are_rejected_by_actual_copy(self):
        target = self.file("payload.bin"); (self.src / "empty").mkdir(mode=0o700)
        for kind in ("file", "directory", "hard"):
            link = self.src / "bad-link"
            if kind == "hard": os.link(target, link)
            else: link.symlink_to(target if kind == "file" else self.src / "empty", target_is_directory=kind == "directory")
            try:
                with self.assertRaises((RuntimeError, self.tools.SafetyError, OSError)): m.copy_small(str(self.src), "reject-" + kind)
            finally: link.unlink()

    def test_depth_eight_is_accepted_but_nine_is_rejected(self):
        deep = "/".join("d" + str(index) for index in range(8)); self.file(deep + "/payload.bin")
        m.copy_small(str(self.src), "depth-eight")
        (self.src / deep / "extra").mkdir(mode=0o700)
        with self.assertRaisesRegex(RuntimeError, "backup_directory_bound"): m.copy_small(str(self.src), "depth-nine")

    def test_directory_count_includes_root_and_caps_at_128(self):
        for index in range(128): (self.src / ("dir-" + str(index))).mkdir(mode=0o700)
        with self.assertRaisesRegex(RuntimeError, "backup_directory_bound"): m.copy_small(str(self.src), "too-many-directories")

    def test_file_count_and_individual_and_total_byte_caps_are_real(self):
        for index in range(257): self.file("zero-" + str(index), b"")
        with self.assertRaisesRegex(RuntimeError, "backup_tree_bound"): m.copy_small(str(self.src), "too-many-files")
        for item in self.src.iterdir(): item.unlink()
        self.file("oversized.bin", b"x" * (1048576 + 1))
        with self.assertRaisesRegex(RuntimeError, "backup_tree_bound"): m.copy_small(str(self.src), "oversized")
        (self.src / "oversized.bin").unlink()
        for index in range(9): self.file("chunk-" + str(index), b"x" * 1048576)
        with self.assertRaisesRegex(RuntimeError, "backup_tree_bound"): m.copy_small(str(self.src), "too-many-bytes")

    def test_real_source_directory_group_write_and_foreign_uid_reject(self):
        self.file("child/payload.bin"); child = self.src / "child"
        child.chmod(0o720)
        with self.assertRaisesRegex(RuntimeError, "backup_directory_trust"): m.copy_small(str(self.src), "directory-writable")
        child.chmod(0o700); os.chown(child, 65534, -1)
        with self.assertRaisesRegex(RuntimeError, "backup_directory_trust"): m.copy_small(str(self.src), "directory-foreign")

    def test_real_child_directory_rename_and_replacement_during_copy_is_rejected(self):
        self.file("a/b/c/payload.bin"); original = self.tools.write_new; changed = False
        def mutate(destination, name, data):
            nonlocal changed
            original(destination, name, data)
            if name == "payload.bin" and not changed:
                changed = True; (self.src / "a/b").rename(self.src / "a/moved"); (self.src / "a/b").mkdir(mode=0o700)
        with patch.object(self.tools, "write_new", side_effect=mutate):
            with self.assertRaisesRegex(RuntimeError, "backup_directory_changed|backup_path_changed"): m.copy_small(str(self.src), "renamed-child")
        self.assertTrue(changed)

    def test_real_root_directory_rename_and_replacement_during_copy_is_rejected(self):
        self.file("payload.bin"); original = self.tools.write_new; changed = False
        def mutate(destination, name, data):
            nonlocal changed
            original(destination, name, data)
            if name == "payload.bin" and not changed:
                changed = True; self.src.rename(self.root / "moved-source"); self.src.mkdir(mode=0o700)
        with patch.object(self.tools, "write_new", side_effect=mutate):
            with self.assertRaisesRegex(RuntimeError, "backup_directory_changed|backup_root_changed"): m.copy_small(str(self.src), "renamed-root")
        self.assertTrue(changed)


class FlowTests(unittest.TestCase):
    def setUp(self):
        self.stack = contextlib.ExitStack(); self.addCleanup(self.stack.close)
        self.old, self.new = properties(), properties(True)
        self.raw = b"MIR2_ALLOWED_WEB_ORIGINS=https://old.example\0MIR2_SAVE_RECOVERY_DIR=/var/lib/mir2-playtest/recovery\0"
        self.proc = b"123 (fixture) " + b" ".join([b"S"] + [b"0"] * 18 + [b"12345"])
        @contextlib.contextmanager
        def sink(*_): yield types.SimpleNamespace(fd=44)
        self.tools = types.SimpleNamespace(open_chain=Mock(return_value=7), write_new=Mock(), FileSink=sink)
        data = {"cfg": (b"original", None)}
        def written(path, content, owned):
            owned[path] = {"sha": hashlib.sha256(content).hexdigest(), "inode": (1, 2)}
            data[path] = (content, types.SimpleNamespace(st_dev=1, st_ino=2))
        mocks = {"T": self.tools, "preflight": Mock(return_value=(self.old, {"cfg": b"original"}, self.raw, self.proc)),
                 "properties": Mock(side_effect=[self.old, self.new]), "readfile": Mock(side_effect=lambda path: data[path]),
                 "proc_bytes": Mock(return_value=self.proc), "copy_small": Mock(return_value=[]), "stop_old": Mock(return_value=True),
                 "write_owned": Mock(side_effect=written),
                 "command": Mock(return_value=b"{}\n"), "rollback": Mock()}
        for name, value in mocks.items(): self.stack.enter_context(patch.object(m, name, value))
        for name in ["mkdir", "close", "fsync"]: self.stack.enter_context(patch.object(m.os, name))
        self.stack.enter_context(patch.object(m.os, "readlink", return_value=m.OLD))
        self.stack.enter_context(patch.object(m.os, "stat", return_value=(0o100600, 2, 1, 1, 0, 0, 8, 0, 0, 0)))

    def test_plan_does_not_write_stop_reload_or_start(self):
        result = m.execute(False)
        self.assertEqual(result, {"phase": "plan-ready", "healthy": False})
        self.assertFalse(m.os.mkdir.called); self.assertFalse(self.tools.write_new.called)
        self.assertFalse(m.stop_old.called); self.assertFalse(m.command.called)
        self.assertEqual(m.BACKUP, "/root/r16-playtest-cutover-d53f9e3b2-20261108-r4")

    def test_pending_stop_does_not_dump_mutate_config_or_start(self):
        m.stop_old.return_value = False
        result = m.execute(True)
        self.assertEqual(result["phase"], "stop-pending"); self.assertTrue(result["manualReconcile"])
        self.assertFalse(m.command.called); self.assertFalse(m.write_owned.called); self.assertFalse(m.rollback.called)

    def test_success_only_requests_start_and_keeps_private_backup(self):
        result = m.execute(True)
        self.assertEqual(result, {"phase": "new-start-requested", "healthy": False})
        self.assertEqual(m.command.call_args.args[0], ["/usr/bin/systemctl", "start", "--no-block", m.SERVICE])
        self.assertTrue(any(call.args[1] == "proc-environment.private" for call in self.tools.write_new.call_args_list))
        self.assertTrue(any(call.args[1] == "sql-hashes.private.jsonl" for call in self.tools.write_new.call_args_list))
        self.assertEqual(m.write_owned.call_args_list[0].args[0], m.ENV)
        self.assertIn(b"ExecStart=\nExecStart=" + m.NEW.encode(), m.write_owned.call_args_list[1].args[1])
        self.assertNotIn(b"EnvironmentFile=\n", m.write_owned.call_args_list[1].args[1])
        self.assertFalse(m.rollback.called)

    def test_all_twelve_old_env_byte_backups_are_written_before_stop(self):
        configs = {"cfg": b"original", **{name: ("synthetic-old-env-" + str(index)).encode() for index, name in enumerate(m.OLD_ENVS)}}
        m.preflight.return_value = self.old, configs, self.raw, self.proc
        base_read = m.readfile.side_effect
        m.readfile.side_effect = lambda path: (configs[path], None) if path in configs else base_read(path)
        result = m.execute(True)
        self.assertEqual(result["phase"], "new-start-requested")
        backups = [call.args[2] for call in self.tools.write_new.call_args_list if call.args[1].startswith("config-")]
        self.assertEqual(backups, list(configs.values()))
        for name in m.OLD_ENVS: self.assertIn(configs[name], backups)

    def test_failure_before_start_requests_old_only_after_owned_rollback(self):
        m.write_owned.side_effect = RuntimeError("fixture-write-failure")
        result = m.execute(True)
        self.assertEqual(result["phase"], "old-restart-requested"); self.assertFalse(result["healthy"])
        self.assertTrue(m.rollback.called)
        self.assertFalse(any("start" in call.args[0] for call in m.command.call_args_list))

    def test_start_timeout_is_unknown_and_never_blindly_rolls_back(self):
        def command(argv, **_):
            if "start" in argv: raise subprocess.TimeoutExpired(argv, 1)
            return b"{}\n"
        m.command.side_effect = command
        result = m.execute(True)
        self.assertEqual(result["phase"], "manual-reconcile"); self.assertFalse(m.rollback.called)

    def test_changed_new_config_before_start_remains_manual_when_ownership_is_lost(self):
        original = m.readfile.side_effect
        m.readfile.side_effect = lambda path: (b"foreign", types.SimpleNamespace(st_dev=1, st_ino=2)) if path == m.ENV else original(path)
        m.rollback.side_effect = RuntimeError("rollback_not_owned")
        result = m.execute(True)
        self.assertEqual(result["phase"], "manual-reconcile"); self.assertTrue(m.rollback.called)
        self.assertFalse(any("start" in call.args[0] for call in m.command.call_args_list))


if __name__ == "__main__": unittest.main(verbosity=2)
