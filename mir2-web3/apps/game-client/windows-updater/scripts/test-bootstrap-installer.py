#!/usr/bin/env python3
"""Seed-only byte/binding regressions; no CMS keys, installation, or game launch.

Signatures here are deliberately invalid fixture bytes. These tests cover the
helper's supplementary structural gate; release authenticity is independently
required by build-bootstrap-installer.ps1 before it calls the helper.
"""

import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
import unittest

sys.dont_write_bytecode = True
SCRIPT_ROOT = Path(__file__).absolute().parent
SPEC = importlib.util.spec_from_file_location("mir2_bootstrap_input_tests", SCRIPT_ROOT / "prepare-bootstrap-installer.py")
bootstrap = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = bootstrap
SPEC.loader.exec_module(bootstrap)
archive = bootstrap.archive
REVISION = "a" * 40
CANDIDATE = "WN-CANDIDATE-bootstrap-contract-fixture"
POWERSHELL = os.environ.get("MIR2_BOOTSTRAP_TEST_PWSH") or shutil.which("pwsh")


def dumped(value):
    return json.dumps(value, separators=(",", ":")).encode("utf-8")


def put(root, name, data):
    file = root / name
    file.parent.mkdir(parents=True, exist_ok=True)
    file.write_bytes(data)


def entry(name, data):
    return {"path": name, "size": len(data), "sha256": archive.sha256(data)}


def candidate_fixture(root):
    root.mkdir()
    exe = b"MZ NONEXECUTABLE game fixture; never launched"
    status = archive.sha256(b"clean source fixture")
    build = {"schema": "mir2.windows.build-attestation.v2", "exeSha256": archive.sha256(exe),
             "exeSizeBytes": len(exe), "gitRevision": REVISION, "worktreeDirty": False,
             "worktreeStatusSha256": status}
    payload = {"mir2-platform-windows.exe": exe, "BUILD-ATTESTATION.json": dumped(build),
               "mir2-assets/fixture/data.txt": b"An asset that must not be in the installer"}
    payload.update({name: ("license fixture " + name).encode("ascii") for name in bootstrap.LICENSES})
    for name, data in payload.items():
        put(root, name, data)
    files = [entry(name, data) for name, data in sorted(payload.items())]
    canonical = "".join(f"{e['path']}\t{e['size']}\t{e['sha256']}\n" for e in files)
    aggregate = archive.sha256(canonical.encode("ascii"))
    manifest = {"schema": "mir2.windows.package-manifest.v4",
                "coverage": {"excludes": list(archive.META), "rule": "fixture payload hashes"},
                "fileCount": len(files), "totalBytes": sum(f["size"] for f in files),
                "aggregateSha256": aggregate, "files": files}
    manifest_bytes = dumped(manifest)
    version = {"schema": "mir2.windows.candidate-version.v4", "candidate": CANDIDATE,
               "gitRevision": REVISION, "worktreeDirty": False, "clientOnly": True,
               "worktreeStatusSha256": status, "exeName": "mir2-platform-windows.exe",
               "exeSha256": archive.sha256(exe), "exeSizeBytes": len(exe),
               "packageManifestSchema": manifest["schema"],
               "packageManifestSha256": archive.sha256(manifest_bytes),
               "packageManifestAggregateSha256": aggregate,
               "packageManifestFileCount": len(files), "packageFileCount": len(files) + 4,
               "buildAttestationSha256": archive.sha256(payload["BUILD-ATTESTATION.json"]),
               "releaseStatementSchema": "mir2.windows.release-statement.v1",
               "signatureFormat": "CMS/PKCS7-detached"}
    version_bytes = dumped(version)
    statement = {"schema": version["releaseStatementSchema"], "candidate": CANDIDATE,
                 "exeSha256": version["exeSha256"], "packageManifestSha256": version["packageManifestSha256"],
                 "packageManifestAggregateSha256": aggregate, "versionSha256": archive.sha256(version_bytes),
                 "buildAttestationSha256": version["buildAttestationSha256"], "gitRevision": REVISION,
                 "worktreeDirty": False, "worktreeStatusSha256": status}
    for name, data in {"PACKAGE-MANIFEST.json": manifest_bytes, "VERSION.json": version_bytes,
                       "RELEASE-STATEMENT.json": dumped(statement),
                       "RELEASE-STATEMENT.p7s": b"NOT A CMS SIGNATURE; contract fixture only"}.items():
        put(root, name, data)
    return root


def bundle_fixture(root, candidate):
    root.mkdir()
    launcher = b"MZ NONEXECUTABLE launcher fixture"
    exe = b"MZ NONEXECUTABLE engine fixture"
    pointer = archive.sha256(exe)
    prefix = "updater/engines/" + pointer + "/"
    engine = {"schema": "mir2.windows.updater-engine.v1", "version": 2, "minBootstrap": 1,
              "sourceRevision": "b" * 40, "builtUnix": 1_790_907_967,
              "exeSize": len(exe), "exeSha256": pointer,
              "launcherSha256": archive.sha256(launcher), "cargoLockSha256": archive.sha256(b"lock fixture")}
    payload = {"Mir2Launcher.exe": launcher, "updater/active.txt": pointer.encode("ascii"),
               prefix + "ENGINE.json": dumped(engine), prefix + "ENGINE.p7s": b"invalid engine CMS fixture",
               prefix + "Mir2Updater.exe": exe}
    created = int(time.time()) - 60
    feed = {"schema": "mir2.windows.update-feed.v1", "channel": "invited", "platform": "windows-x64",
            "sequence": 7, "createdUnix": created, "expiresUnix": created + 30 * 86400, "minBootstrap": 1,
            "protocol": "crystal-mir2-v1", "content": "mir2.windows.package-manifest.v4",
            "game": {"directory": "releases/game-fixture", "identity": CANDIDATE,
                     "metadata": [entry(n, (candidate / n).read_bytes()) for n in archive.META]},
            "engine": {"directory": "releases/engine-fixture", "identity": "2",
                       "metadata": [entry(n, payload[prefix + n]) for n in ("ENGINE.json", "ENGINE.p7s")]}}
    payload.update({"updater/seed-feed.json": dumped(feed), "updater/seed-feed.p7s": b"invalid feed CMS fixture"})
    for name, data in payload.items():
        put(root, name, data)
    statement = {"schema": "mir2.windows.updater-bundle.v1", "sourceRevision": "b" * 40,
                 "gameCandidate": CANDIDATE, "gameSourceRevision": REVISION,
                 "files": [entry(n, d) for n, d in sorted(payload.items())]}
    put(root, "UPDATER-BUNDLE.json", dumped(statement))
    put(root, "UPDATER-BUNDLE.p7s", b"invalid bundle CMS fixture")
    return root


class BootstrapInputsTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="mir2-bootstrap-contract-")
        self.root = Path(self.temporary.name).absolute()
        self.candidate = candidate_fixture(self.root / "game")
        self.bundle = bundle_fixture(self.root / "bundle", self.candidate)
        self.recipe = self.root / "recipe"
        source_recipe = SCRIPT_ROOT.parents[1] / "platform-windows/scripts/invited-installer"
        shutil.copytree(source_recipe, self.recipe)
        self.guides = self.root / "guides"
        self.guides.mkdir()
        for locale in bootstrap.LANGUAGES:
            put(self.guides, "README." + locale + ".txt", ("guide " + locale + "\n").encode("utf-8"))
        self.output = self.root / "output"

    def tearDown(self):
        assert self.root.parent.resolve() == Path(tempfile.gettempdir()).resolve()
        assert self.root.name.startswith("mir2-bootstrap-contract-")
        self.temporary.cleanup()

    def prepare(self, **changes):
        options = {"candidate_root": self.candidate, "bundle_root": self.bundle,
                   "recipe_root": self.recipe, "guide_root": self.guides,
                   "output": self.output, "expected_revision": REVISION,
                   "expected_candidate": CANDIDATE}
        options.update(changes)
        return bootstrap.prepare(**options)

    def reject(self, **changes):
        with self.assertRaises((ValueError, KeyError, OSError, TypeError)):
            self.prepare(**changes)
        self.assertFalse(self.output.exists(), "validation must precede output creation")

    def edit_bundle(self, name, edit):
        value = json.loads((self.bundle / name).read_bytes())
        edit(value)
        put(self.bundle, name, dumped(value))
        if name != "UPDATER-BUNDLE.json":
            statement = json.loads((self.bundle / "UPDATER-BUNDLE.json").read_bytes())
            statement["files"] = [entry(n["path"], (self.bundle / n["path"]).read_bytes())
                                  for n in statement["files"]]
            put(self.bundle, "UPDATER-BUNDLE.json", dumped(statement))

    def test_exact_seed_closure_excludes_game_binary_attestation_and_assets(self):
        report = self.prepare()
        installed = {e["path"] for e in report["files"]}
        self.assertEqual({n[5:] for n in installed if n.startswith("game/")}, set(archive.META))
        self.assertFalse(any("mir2-assets" in n or n.endswith("BUILD-ATTESTATION.json") or
                             n.endswith("mir2-platform-windows.exe") for n in installed))
        self.assertEqual(report["installedFileCount"], 29)
        self.assertEqual(report["sequence"], 7)
        self.assertEqual(report["engineVersion"], 2)
        self.assertEqual(report["updaterSourceRevision"], "b" * 40)
        self.assertIn("Required separately", report["cmsVerification"])
        for e in report["files"]:
            source = self.output / ("inputs/" + e["path"] if not e["path"].startswith("README.") else e["path"])
            self.assertEqual(archive.sha256(source.read_bytes()), e["sha256"])

    def test_nine_languages_have_existing_install_guidance_and_exact_guard_hashes(self):
        report = self.prepare()
        text = (self.output / "verified-bootstrap-messages.iss").read_text(encoding="utf-8-sig")
        self.assertEqual(text.count(".BootstrapExisting="), 9)
        self.assertEqual(text.count("Mir2Launcher.exe"), 9)
        guard = (self.output / "verified-bootstrap-guard.iss").read_text(encoding="utf-8-sig")
        for e in report["files"]:
            self.assertIn("'" + e["path"] + "'", guard)
            self.assertIn(e["sha256"], guard)
        self.assertNotIn("transaction.json", guard)
        self.assertNotIn("highest.txt", guard)
        self.assertNotIn("game/mir2-platform-windows.exe", guard)

    def test_wrong_source(self):
        self.reject(expected_revision="c" * 40)

    def test_short_source(self):
        self.reject(expected_revision="a" * 7)

    def test_wrong_candidate(self):
        self.reject(expected_candidate="WN-CANDIDATE-other")

    def test_corrupt_payload_not_selected_for_installer_still_rejected(self):
        put(self.candidate, "mir2-assets/fixture/data.txt", b"corrupt asset")
        self.reject()

    def test_extra_unselected_game_file_rejected(self):
        put(self.candidate, "mir2-assets/fixture/extra.txt", b"extra asset")
        self.reject()

    def test_missing_unselected_game_file_rejected(self):
        (self.candidate / "mir2-platform-windows.exe").unlink()
        self.reject()

    def test_missing_license_rejected(self):
        (self.candidate / bootstrap.LICENSES[0]).unlink()
        self.reject()

    def test_missing_guide_rejected(self):
        (self.guides / "README.ar.txt").unlink()
        self.reject()

    def test_extra_updater_file_rejected(self):
        put(self.bundle, "extra.txt", b"extra")
        self.reject()

    def test_bad_engine_pointer_rejected(self):
        put(self.bundle, "updater/active.txt", b"../engine")
        self.reject()

    def test_launcher_hash_rejected(self):
        put(self.bundle, "Mir2Launcher.exe", b"tampered launcher")
        self.reject()

    def test_foreign_seed_game_rejected(self):
        self.edit_bundle("updater/seed-feed.json", lambda x: x["game"].update(identity="WN-CANDIDATE-other"))
        self.reject()

    def test_seed_game_metadata_hash_rejected(self):
        self.edit_bundle("updater/seed-feed.json", lambda x: x["game"]["metadata"][0].update(sha256="0" * 64))
        self.reject()

    def test_seed_engine_metadata_hash_rejected(self):
        self.edit_bundle("updater/seed-feed.json", lambda x: x["engine"]["metadata"][0].update(sha256="0" * 64))
        self.reject()

    def test_seed_duplicate_metadata_rejected(self):
        self.edit_bundle("updater/seed-feed.json", lambda x: x["game"]["metadata"].__setitem__(1, x["game"]["metadata"][0]))
        self.reject()

    def test_seed_non_immutable_directory_rejected(self):
        self.edit_bundle("updater/seed-feed.json", lambda x: x["game"].update(directory="latest/game"))
        self.reject()

    def test_seed_incompatible_bootstrap_rejected(self):
        self.edit_bundle("updater/seed-feed.json", lambda x: x.update(minBootstrap=2))
        self.reject()

    def test_seed_unknown_schema_rejected(self):
        self.edit_bundle("updater/seed-feed.json", lambda x: x.update(schema="mir2.windows.update-feed.v2"))
        self.reject()

    def test_seed_boolean_sequence_rejected(self):
        self.edit_bundle("updater/seed-feed.json", lambda x: x.update(sequence=True))
        self.reject()

    def test_seed_extra_field_rejected(self):
        self.edit_bundle("updater/seed-feed.json", lambda x: x.update(untrusted=True))
        self.reject()

    def test_seed_component_extra_field_rejected(self):
        self.edit_bundle("updater/seed-feed.json", lambda x: x["game"].update(untrusted=True))
        self.reject()

    def test_seed_expired_rejected(self):
        self.edit_bundle("updater/seed-feed.json", lambda x: x.update(
            createdUnix=int(time.time()) - 86400, expiresUnix=int(time.time()) - 1))
        self.reject()

    def test_seed_future_rejected(self):
        self.edit_bundle("updater/seed-feed.json", lambda x: x.update(
            createdUnix=int(time.time()) + 600, expiresUnix=int(time.time()) + 86400))
        self.reject()

    def test_seed_unbounded_validity_rejected(self):
        self.edit_bundle("updater/seed-feed.json", lambda x: x.update(
            expiresUnix=x["createdUnix"] + 32 * 86400))
        self.reject()

    def test_bundle_case_collision_rejected(self):
        self.edit_bundle("UPDATER-BUNDLE.json", lambda x: x["files"].__setitem__(1, dict(x["files"][0], path="mir2launcher.EXE")))
        self.reject()

    def test_bundle_wrong_game_source_rejected(self):
        self.edit_bundle("UPDATER-BUNDLE.json", lambda x: x.update(gameSourceRevision="c" * 40))
        self.reject()

    def test_bundle_extra_field_rejected(self):
        self.edit_bundle("UPDATER-BUNDLE.json", lambda x: x.update(untrusted=True))
        self.reject()

    def test_hardlinked_input_rejected(self):
        os.link(self.candidate / "mir2-platform-windows.exe", self.root / "hardlink.exe")
        self.reject()

    def test_symlink_input_rejected(self):
        linked = self.root / "linked"
        try:
            linked.symlink_to(self.candidate, target_is_directory=True)
        except OSError as error:
            self.skipTest("host does not permit symlinks: " + str(error))
        self.reject(candidate_root=linked)

    @unittest.skipUnless(os.name == "nt", "NTFS data-stream regression")
    def test_ntfs_alternate_data_stream_rejected(self):
        with open(str(self.bundle / "Mir2Launcher.exe") + ":unexpected", "wb") as stream:
            stream.write(b"unexpected stream")
        self.reject()

    def test_output_overwrite_rejected(self):
        self.output.mkdir()
        sentinel = self.output / "sentinel.txt"
        sentinel.write_bytes(b"preserve")
        with self.assertRaises(ValueError):
            self.prepare()
        self.assertEqual(sentinel.read_bytes(), b"preserve")

    def test_output_inside_input_rejected(self):
        self.reject(output=self.candidate / "nested-output")

    def test_unsafe_inno_source_path_rejected(self):
        self.reject(output=self.root / "unsafe{input}")

    def test_cli_requires_full_explicit_identity(self):
        result = subprocess.run([sys.executable, str(SCRIPT_ROOT / "prepare-bootstrap-installer.py"),
                                 str(self.candidate), str(self.bundle), str(self.recipe), str(self.guides),
                                 str(self.output)], capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(self.output.exists())


@unittest.skipUnless(POWERSHELL, "PowerShell is required for the Windows builder regression")
class CanonicalVerifierSourceTests(unittest.TestCase):
    script_names = ("verify-windows-candidate.ps1", "entity-atlas-closure.ps1",
                    "player-sprite-closure.ps1", "candidate-release-profile.ps1",
                    "pet-guild-asset-closure.ps1")
    font_names = ("NotoSansTC.ttf", "NotoSans-Regular.ttf", "NotoSans-Bold.ttf",
                  "NotoSansDevanagari-Regular.ttf", "NotoSansDevanagari-Bold.ttf",
                  "NotoSansThai-Regular.ttf", "NotoSansThai-Bold.ttf",
                  "NotoSansArabic-Regular.ttf", "NotoSansArabic-Bold.ttf")

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="mir2-bootstrap-verifier-")
        self.root = Path(self.temporary.name).absolute()
        self.platform = self.root / "platform-windows"
        self.scripts = self.platform / "scripts"
        self.fonts = self.platform / "assets/fonts"
        self.scripts.mkdir(parents=True)
        self.fonts.mkdir(parents=True)
        source = SCRIPT_ROOT.parents[1] / "platform-windows"
        for name in self.script_names:
            shutil.copyfile(source / "scripts" / name, self.scripts / name)
        for name in self.font_names:
            shutil.copyfile(source / "assets/fonts" / name, self.fonts / name)
        self.output = self.root / "proof"

    def tearDown(self):
        assert self.root.parent.resolve() == Path(tempfile.gettempdir()).resolve()
        assert self.root.name.startswith("mir2-bootstrap-verifier-")
        self.temporary.cleanup()

    def copy_source(self):
        # Load only the actual builder's preparation functions. The installer
        # entry point, CMS/key code and process-launch functions never run.
        command = r"""
$ErrorActionPreference='Stop';Set-StrictMode -Version Latest
$tokens=$null;$errors=$null
$ast=[Management.Automation.Language.Parser]::ParseFile($env:MIR2_BOOTSTRAP_TEST_BUILDER,[ref]$tokens,[ref]$errors)
if($errors.Count-ne0){throw 'Builder parse errors'}
foreach($name in @('NoLinks','FileSha','SaveJson','CopyCanonicalVerifierSource')){
    $definitions=@($ast.FindAll({param($node)
        $node-is[Management.Automation.Language.FunctionDefinitionAst] -and $node.Name-ceq$name
    },$false))
    if($definitions.Count-ne1){throw 'Builder test function missing or ambiguous'}
    . ([scriptblock]::Create($definitions[0].Extent.Text))
}
CopyCanonicalVerifierSource $env:MIR2_BOOTSTRAP_TEST_SCRIPTS $env:MIR2_BOOTSTRAP_TEST_PROOF | Out-Null
"""
        environment = dict(os.environ, MIR2_BOOTSTRAP_TEST_BUILDER=str(SCRIPT_ROOT / "build-bootstrap-installer.ps1"),
                           MIR2_BOOTSTRAP_TEST_SCRIPTS=str(self.scripts),
                           MIR2_BOOTSTRAP_TEST_PROOF=str(self.output))
        return subprocess.run([POWERSHELL, "-NoProfile", "-NonInteractive", "-Command", command],
                              capture_output=True, text=True, encoding="utf-8", env=environment, timeout=30)

    def test_complete_canonical_verifier_sources_are_copied_and_receipted(self):
        result = self.copy_source()
        self.assertEqual(result.returncode, 0, result.stderr)
        closure = json.loads((self.output / "canonical-verifier-source-closure.json").read_text("utf-8"))
        prefix = "apps/game-client/platform-windows/"
        expected = {prefix + "scripts/" + n for n in self.script_names}
        expected.update(prefix + "assets/fonts/" + n for n in self.font_names)
        self.assertEqual(len(closure), 14)
        self.assertEqual({e["path"] for e in closure}, expected)
        for entry in closure:
            relative = entry["path"].removeprefix(prefix)
            source = self.platform / relative
            copied = self.output / "verification-source" / entry["path"]
            self.assertEqual(source.read_bytes(), copied.read_bytes())
            self.assertEqual(entry["sha256"], hashlib.sha256(copied.read_bytes()).hexdigest().upper())
            if relative.startswith("assets/fonts/"):
                self.assertEqual(entry["sizeBytes"], copied.stat().st_size)

    def test_missing_canonical_font_rejected_before_source_receipt(self):
        (self.fonts / "NotoSansThai-Bold.ttf").unlink()
        result = self.copy_source()
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse((self.output / "canonical-verifier-source-closure.json").exists())

    def test_changed_canonical_font_rejected_against_existing_pin(self):
        font = self.fonts / "NotoSansTC.ttf"
        changed = bytearray(font.read_bytes())
        changed[-1] ^= 1
        font.write_bytes(changed)
        result = self.copy_source()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Canonical pinned font source mismatch", result.stderr)
        self.assertFalse((self.output / "canonical-verifier-source-closure.json").exists())



@unittest.skipUnless(POWERSHELL, "PowerShell is required for the pure content-gate regression")
class ContentGateFixtureBase(unittest.TestCase):
    """Only actual parsed helper functions run; entry points/keys/Git/Cargo do not.

    The report fixtures deliberately lack real signatures/resources. A passing
    fixture checks binding/control flow, not a release or visual acceptance.
    """
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="mir2-bootstrap-content-gate-")
        self.root = Path(self.temporary.name).absolute()
        self.project = self.root / "source"
        self.candidate = self.root / "candidate"
        self.proof = self.root / "proof"
        for directory in (self.project, self.candidate, self.proof):
            directory.mkdir()
        self.revision = "a" * 40
        self.thumbprint = "1" * 40
        self.scope = "git-status-z+diff+all-untracked-content-v2"
        self.attestation = {"schema": "mir2.windows.build-attestation.v2", "gitRevision": self.revision,
                            "worktreeDirty": False, "worktreeStatusScope": self.scope,
                            "worktreeStatusLineCount": 0, "worktreeStatusSha256": "B" * 64}
        self.worktree = {"revision": self.revision, "dirty": False, "statusScope": self.scope,
                         "statusLineCount": 0, "statusSha256": "B" * 64}
        self.profile_relative = "packages/game-data/data/content_profiles/platinum_fiveclass.json"
        self.verifier_relative = "apps/game-client/platform-windows/scripts/verify-windows-candidate.ps1"
        put(self.project, self.profile_relative, dumped({"profileId": "platinum_fiveclass"}))
        # This is nonexecutable hash input; no fixture verifier or game is run.
        put(self.project, self.verifier_relative, b"nonexecutable verifier identity fixture")
        for kind in ("sprite", "skill"):
            put(self.project, f"apps/web/scripts/asset-pipeline/five-class-{kind}-requirements.json",
                dumped({"kind": kind, "fixture": True}))
        self.index_relative = "original-ui/sound-index.generated.json"
        for directory in (self.project / "apps/web/public", self.candidate / "mir2-assets"):
            put(directory, self.index_relative, dumped({"sounds": {}}))
        put(self.candidate, "BUILD-ATTESTATION.json", dumped(self.attestation))
        put(self.candidate, "PACKAGE-MANIFEST.json", dumped({"fixture": True}))
        put(self.candidate, "mir2-platform-windows.exe", b"nonexecutable file identity fixture")
        self.profile_sha = self.sha(self.project / self.profile_relative)
        self.verifier_sha = self.sha(self.project / self.verifier_relative)
        requirements = [{"kind": kind, "sha256": self.sha(self.project /
            f"apps/web/scripts/asset-pipeline/five-class-{kind}-requirements.json")}
            for kind in ("sprite", "skill")]
        self.report = {
            "schema": "mir2.windows.package-verification.v4", "candidate": CANDIDATE,
            "packageRoot": str(self.candidate), "passed": True, "failures": [],
            "detachedSignatureValid": True, "nonvisual": True, "launchRequested": False,
            "attestationPresent": True, "packageManifestPresent": True,
            "attestationSha256": self.sha(self.candidate / "BUILD-ATTESTATION.json"),
            "packageManifestSha256": self.sha(self.candidate / "PACKAGE-MANIFEST.json"),
            "exeSha256": self.sha(self.candidate / "mir2-platform-windows.exe"),
            "contentProfile": "platinum_fiveclass", "contentProfileId": "platinum_fiveclass",
            "contentProfileSha256": self.profile_sha, "contentProfileSourceAvailable": True,
            "requireCurrentSource": True, "currentSourceValidated": True, "sourceRepoCheck": "checked",
            "currentSourceRevision": self.revision, "currentSourceRoot": str(self.project),
            "trustedSignerThumbprint": self.thumbprint, "verifierSha256": self.verifier_sha,
            "currentSourceProof": {"revision": self.revision, "clean": True, "statusScope": self.scope,
                                   "statusLineCount": 0, "statusSha256": "B" * 64},
            "fiveClassClosure": {"schema": "mir2.windows.five-class-closure-evidence.v1",
                "profileId": "platinum_fiveclass", "profileSha256": self.profile_sha,
                "sourceAssetRoot": str(self.project / "apps/web/public"),
                "stagedAssetRoot": str(self.candidate / "mir2-assets"),
                "sourceSoundValidated": True, "stagedSoundValidated": True, "soundIndexExactCopy": True,
                "sourceSpriteAndSkillValidated": True, "stagedSpriteAndSkillValidated": True,
                "soundIndexRelativePath": self.index_relative,
                "soundIndexSha256": self.sha(self.project / "apps/web/public" / self.index_relative),
                "soundIndexBytes": (self.project / "apps/web/public" / self.index_relative).stat().st_size,
                "availableSoundCount": 51, "absentSoundCount": 6, "requirements": requirements}}

    @staticmethod
    def sha(path):
        return hashlib.sha256(path.read_bytes()).hexdigest().upper()

    def tearDown(self):
        assert self.root.parent.resolve() == Path(tempfile.gettempdir()).resolve()
        assert self.root.name.startswith("mir2-bootstrap-content-gate-")
        self.temporary.cleanup()

    def run_helpers(self, body, *, canonical=(), builder=(), data=None):
        context = {"project": str(self.project), "candidate": str(self.candidate), "proof": str(self.proof),
                   "revision": self.revision, "thumbprint": self.thumbprint, "profileSha": self.profile_sha,
                   "verifierSha": self.verifier_sha, "report": self.report, "attestation": self.attestation,
                   "worktree": self.worktree}
        context.update(data or {})
        context_file = self.root / "context.json"
        context_file.write_bytes(dumped(context))
        command = r"""
$ErrorActionPreference='Stop';Set-StrictMode -Version Latest
function ImportFixtureFunctions([string]$Path,[string[]]$Names) {
    $tokens=$null;$errors=$null
    $ast=[Management.Automation.Language.Parser]::ParseFile($Path,[ref]$tokens,[ref]$errors)
    if($errors.Count-ne0){throw 'Actual production script parse failed'}
    foreach($name in $Names) {
        $defs=@($ast.FindAll({param($node)
            $node-is[Management.Automation.Language.FunctionDefinitionAst]-and$node.Name-ceq$name
        },$false))
        if($defs.Count-ne1){throw 'Missing or ambiguous actual helper'}
        . ([scriptblock]::Create($defs[0].Extent.Text.Replace(('function '+$name),('function global:'+$name))))
    }
}
$ctx=Get-Content -LiteralPath $env:MIR2_CONTENT_TEST_CONTEXT -Raw|ConvertFrom-Json
ImportFixtureFunctions $env:MIR2_CONTENT_TEST_CANONICAL (ConvertFrom-Json $env:MIR2_CONTENT_TEST_CANONICAL_NAMES)
ImportFixtureFunctions $env:MIR2_CONTENT_TEST_BUILDER (ConvertFrom-Json $env:MIR2_CONTENT_TEST_BUILDER_NAMES)
""" + body
        environment = dict(os.environ, MIR2_CONTENT_TEST_CONTEXT=str(context_file),
            MIR2_CONTENT_TEST_CANONICAL=str(SCRIPT_ROOT.parents[1] / "platform-windows/scripts/verify-windows-candidate.ps1"),
            MIR2_CONTENT_TEST_BUILDER=str(SCRIPT_ROOT / "build-bootstrap-installer.ps1"),
            MIR2_CONTENT_TEST_CANONICAL_NAMES=json.dumps(list(canonical)),
            MIR2_CONTENT_TEST_BUILDER_NAMES=json.dumps(list(builder)))
        return subprocess.run([POWERSHELL, "-NoProfile", "-NonInteractive", "-Command", command],
            capture_output=True, text=True, encoding="utf-8", env=environment, timeout=30)

    def assert_report_gate(self, *, accepted):
        result = self.run_helpers(r"""
AssertBootstrapFiveClassReport -Report $ctx.report -ProjectRoot $ctx.project -GameRoot $ctx.candidate `
    -GameCandidate 'WN-CANDIDATE-bootstrap-contract-fixture' -Revision $ctx.revision `
    -SignerThumbprint $ctx.thumbprint -VerifierSha256 $ctx.verifierSha -ProfileSha256 $ctx.profileSha
""", builder=("FileSha", "ReadJson", "AssertBootstrapFiveClassReport"))
        if accepted:
            self.assertEqual(result.returncode, 0, result.stderr)
        else:
            self.assertNotEqual(result.returncode, 0, result.stdout)
        return result


class CurrentSourceStateFixtureTests(ContentGateFixtureBase):
    def check_state(self, *, clean=True, accepted=False):
        result = self.run_helpers(
            "Assert-CurrentCandidateSourceState -Attestation $ctx.attestation -Worktree $ctx.worktree "
            + ("-RequireClean " if clean else "") + "|ConvertTo-Json -Compress",
            canonical=("Assert-CurrentCandidateSourceState",))
        if accepted:
            self.assertEqual(result.returncode, 0, result.stderr)
            return json.loads(result.stdout.strip())
        self.assertNotEqual(result.returncode, 0, result.stdout)

    def test_clean_current_revision_and_full_attestation_state_accepted(self):
        result = self.check_state(accepted=True)
        self.assertTrue(result["clean"])
        self.assertEqual(result["revision"], self.revision)
        self.assertEqual(result["statusSha256"], self.attestation["worktreeStatusSha256"])

    def test_old_source_revision_rejected(self):
        self.attestation["gitRevision"] = "c" * 40
        self.check_state()

    def test_source_content_digest_drift_rejected(self):
        self.worktree["statusSha256"] = "C" * 64
        self.check_state()

    def test_matching_dirty_state_rejected_when_current_clean_required(self):
        self.attestation.update(worktreeDirty=True, worktreeStatusLineCount=1)
        self.worktree.update(dirty=True, statusLineCount=1)
        self.check_state()

    def test_original_matching_dirty_comparison_preserved_without_require_clean(self):
        self.attestation.update(worktreeDirty=True, worktreeStatusLineCount=1)
        self.worktree.update(dirty=True, statusLineCount=1)
        self.assertFalse(self.check_state(clean=False, accepted=True)["clean"])

    def test_matching_unsupported_status_scope_rejected(self):
        self.attestation["worktreeStatusScope"] = "legacy-scope"
        self.worktree["statusScope"] = "legacy-scope"
        self.check_state()

    def test_non_boolean_source_clean_claim_rejected(self):
        self.worktree["dirty"] = "false"
        self.check_state()

    def test_inconsistent_nonempty_clean_status_rejected(self):
        self.attestation["worktreeStatusLineCount"] = 1
        self.worktree["statusLineCount"] = 1
        self.check_state()


class FreshReportPathFixtureTests(ContentGateFixtureBase):
    helper_names = ("Test-PathWithin", "Test-IsReparsePoint", "Assert-NoReparseAncestors",
                    "Resolve-FreshCandidateVerificationReportPath", "Write-FreshCandidateVerificationReport")

    def write_report(self, path, *, accepted=False):
        result = self.run_helpers(r"""
Write-FreshCandidateVerificationReport -Path $ctx.reportPath -PackageRoot $ctx.candidate -Text '{"fixture":true}'
""", canonical=self.helper_names, data={"reportPath": str(path)})
        if accepted:
            self.assertEqual(result.returncode, 0, result.stderr)
        else:
            self.assertNotEqual(result.returncode, 0, result.stdout)
        return result

    def test_fresh_absolute_external_report_is_written(self):
        path = self.proof / "result.json"
        self.write_report(path, accepted=True)
        self.assertEqual(path.read_bytes(), b'{"fixture":true}')

    def test_existing_report_cannot_be_overwritten(self):
        path = self.proof / "result.json"
        path.write_bytes(b"existing exact evidence")
        self.write_report(path)
        self.assertEqual(path.read_bytes(), b"existing exact evidence")

    def test_report_inside_candidate_rejected_before_write(self):
        path = self.candidate / "result.json"
        self.write_report(path)
        self.assertFalse(path.exists())

    def test_relative_report_path_rejected(self):
        self.write_report("relative-report.json")

    def test_report_data_stream_rejected(self):
        self.write_report(str(self.proof / "result.json") + ":secret")
        self.assertFalse((self.proof / "result.json").exists())

    def test_reparse_report_parent_rejected(self):
        target = self.root / "target"
        target.mkdir()
        link = self.root / "linked-proof"
        try:
            os.symlink(target, link, target_is_directory=True)
        except OSError as error:
            self.skipTest(f"Directory reparse fixture unavailable: {error}")
        self.write_report(link / "result.json")
        self.assertFalse((target / "result.json").exists())

    def test_second_write_preserves_first_new_report(self):
        path = self.proof / "result.json"
        self.write_report(path, accepted=True)
        self.write_report(path)
        self.assertEqual(path.read_bytes(), b'{"fixture":true}')


class FiveClassActualClosureFlowFixtureTests(ContentGateFixtureBase):
    def closure(self, *, failing="", accepted=False):
        body = r"""
$script:soundCalls=@();$script:assetCalls=@()
function Assert-FiveClassPackageSoundAssets([string]$AssetRoot,[object]$Policy) {
    $script:soundCalls+=,$AssetRoot
    $path=Join-Path $AssetRoot $Policy.soundIndexRelativePath
    return [pscustomobject]@{bytes=(Get-Item -LiteralPath $path).Length;sha256=(Get-FileHash -LiteralPath $path).Hash}
}
function Assert-FiveClassAssetClosure([string]$ProjectRoot,[string]$AssetRoot,[string]$ProfilePath) {
    $script:assetCalls+=,$AssetRoot
    if($ctx.failing-ceq'staged'-and$script:assetCalls.Count-eq2){throw 'Actual staged validator fixture rejection'}
    if($ctx.failing-ceq'profile-change'-and$script:assetCalls.Count-eq2){
        [IO.File]::WriteAllText($ProfilePath,'{"profileId":"platinum_fiveclass","changed":true}')
    }
    'native validator stdout fixture; must not become success evidence'
}
$evidence=$null;$failure=''
try {
    $evidence=Invoke-FiveClassCandidateClosureEvidence -ProjectRoot $ctx.project -PackageRoot $ctx.candidate `
        -ProfilePath (Join-Path $ctx.project 'packages/game-data/data/content_profiles/platinum_fiveclass.json') `
        -SoundPolicy ([pscustomobject]@{available=@(1);absent=@();soundIndexRelativePath='original-ui/sound-index.generated.json'})
} catch {$failure=$_.Exception.Message}
[pscustomobject]@{evidence=$evidence;failure=$failure;soundCalls=$script:soundCalls;assetCalls=$script:assetCalls}|ConvertTo-Json -Depth 8 -Compress
"""
        result = self.run_helpers(body, canonical=("ConvertFrom-JsonPreservingDateStrings",
            "Invoke-FiveClassCandidateClosureEvidence"), data={"failing": failing})
        self.assertEqual(result.returncode, 0, result.stderr)
        output = json.loads(result.stdout.strip().splitlines()[-1])
        if accepted:
            self.assertEqual(output["failure"], "")
            self.assertIsInstance(output["evidence"], dict)
        else:
            self.assertIsNone(output["evidence"])
            self.assertTrue(output["failure"])
        return output

    def test_both_actual_source_and_staged_callbacks_required_for_evidence(self):
        result = self.closure(accepted=True)
        self.assertEqual(result["soundCalls"], [str(self.project / "apps/web/public"),
                                                str(self.candidate / "mir2-assets")])
        self.assertEqual(result["assetCalls"], result["soundCalls"])
        evidence = result["evidence"]
        self.assertTrue(evidence["sourceSpriteAndSkillValidated"])
        self.assertTrue(evidence["stagedSpriteAndSkillValidated"])
        self.assertEqual(evidence["profileSha256"], self.profile_sha)

    def test_staged_validator_failure_never_returns_pass_evidence(self):
        result = self.closure(failing="staged")
        self.assertEqual(len(result["assetCalls"]), 2)

    def test_sound_index_difference_fails_before_sprite_callbacks(self):
        put(self.candidate / "mir2-assets", self.index_relative, b"different staged index")
        result = self.closure()
        self.assertEqual(result["assetCalls"], [])

    def test_changed_actual_profile_cannot_get_success_evidence(self):
        self.closure(failing="profile-change")

    def test_wrong_source_profile_rejected_without_callbacks(self):
        put(self.project, self.profile_relative, dumped({"profileId": "platinum_176"}))
        result = self.closure()
        self.assertEqual(result["soundCalls"], [])
        self.assertEqual(result["assetCalls"], [])


class BootstrapFiveClassReportFixtureTests(ContentGateFixtureBase):
    def test_bound_five_class_structural_report_accepted(self):
        self.assert_report_gate(accepted=True)

    def test_wrong_selected_classic_profile_rejected(self):
        self.report["contentProfile"] = "platinum_176"
        self.assert_report_gate(accepted=False)

    def test_old_passed_report_without_real_source_and_closure_fields_rejected(self):
        self.report = {"passed": True, "detachedSignatureValid": True, "nonvisual": True, "launchRequested": False}
        self.assert_report_gate(accepted=False)

    def test_wrong_current_source_revision_rejected(self):
        self.report["currentSourceRevision"] = "c" * 40
        self.assert_report_gate(accepted=False)

    def test_another_source_root_rejected(self):
        self.report["currentSourceRoot"] = str(self.root / "marker-source")
        self.assert_report_gate(accepted=False)

    def test_missing_real_closure_rejected(self):
        self.report.pop("fiveClassClosure")
        self.assert_report_gate(accepted=False)

    def test_incomplete_staged_closure_rejected(self):
        self.report["fiveClassClosure"]["stagedSpriteAndSkillValidated"] = False
        self.assert_report_gate(accepted=False)

    def test_string_pass_flag_rejected(self):
        self.report["passed"] = "true"
        self.assert_report_gate(accepted=False)

    def test_changed_actual_attestation_rejected(self):
        put(self.candidate, "BUILD-ATTESTATION.json", dumped(dict(self.attestation, gitRevision="c" * 40)))
        self.assert_report_gate(accepted=False)

    def test_changed_actual_source_profile_rejected(self):
        put(self.project, self.profile_relative, dumped({"profileId": "platinum_fiveclass", "changed": True}))
        self.assert_report_gate(accepted=False)

    def test_changed_staged_sound_index_rejected(self):
        put(self.candidate / "mir2-assets", self.index_relative, b"different index")
        self.assert_report_gate(accepted=False)

    def test_missing_skill_requirements_proof_rejected(self):
        self.report["fiveClassClosure"]["requirements"].pop()
        self.assert_report_gate(accepted=False)

    def test_launch_claim_rejected_for_nonvisual_installer_build(self):
        self.report["launchRequested"] = True
        self.assert_report_gate(accepted=False)

    def test_matching_dirty_attestation_cannot_substitute_clean_source_proof(self):
        self.attestation.update(worktreeDirty=True, worktreeStatusLineCount=1)
        put(self.candidate, "BUILD-ATTESTATION.json", dumped(self.attestation))
        self.report["attestationSha256"] = self.sha(self.candidate / "BUILD-ATTESTATION.json")
        self.assert_report_gate(accepted=False)


class BootstrapVerifierInvocationFixtureTests(ContentGateFixtureBase):
    def invoke(self, *, profile=None):
        body = r"""
$script:copyCalls=0;$script:fixtureArgs=@()
function CopyCanonicalVerifierSource([string]$PlatformScripts,[string]$ProofRoot) {
    $script:copyCalls++
    $root=Join-Path $ProofRoot 'verification-source'
    $scripts=Join-Path $root 'apps/game-client/platform-windows/scripts'
    [IO.Directory]::CreateDirectory($scripts)|Out-Null
    [IO.File]::WriteAllText((Join-Path $scripts 'verify-windows-candidate.ps1'),'nonexecutable copied classic verifier fixture')
    return $root
}
function InvokeLogged([string]$Executable,[string[]]$Arguments,[string]$Name) {
    # No process executes. This spy proves real helper argument/path selection;
    # the complete full-resource/CMS verifier is not claimed to run in fixtures.
    $script:fixtureArgs=$Arguments
    if($Arguments-contains'-ReportPath') {
        $path=$Arguments[[Array]::IndexOf($Arguments,'-ReportPath')+1]
        $ctx.report|ConvertTo-Json -Depth 12|Set-Content -LiteralPath $path -Encoding utf8
    } else {
        $root=Join-Path $ctx.proof 'verification-source/docs/generated/player-qa/windows-package-preflight'
        [IO.Directory]::CreateDirectory($root)|Out-Null
        [pscustomobject]@{passed=$true;detachedSignatureValid=$true;nonvisual=$true;launchRequested=$false;sourceRepoCheck='unavailable'}|
            ConvertTo-Json|Set-Content -LiteralPath (Join-Path $root 'WN-CANDIDATE-bootstrap-contract-fixture-verification.json') -Encoding utf8
    }
}
$params=@{ProjectRoot=$ctx.project;GameRoot=$ctx.candidate;ProofRoot=$ctx.proof;
    GameCandidate='WN-CANDIDATE-bootstrap-contract-fixture';Revision=$ctx.revision;
    SignerThumbprint=$ctx.thumbprint;PowerShellPath='fixture-never-run'}
if($null-ne$ctx.selectedProfile){$params.Profile=$ctx.selectedProfile}
$verified=InvokeBootstrapGameVerification @params
[pscustomobject]@{copyCalls=$script:copyCalls;args=$script:fixtureArgs;profile=$verified.contentProfile;
    profileSha=$verified.contentProfileSha256;reportPath=$verified.reportPath}|ConvertTo-Json -Compress
"""
        result = self.run_helpers(body, builder=("NoLinks", "FileSha", "ReadJson",
            "AssertBootstrapFiveClassReport", "InvokeBootstrapGameVerification"), data={"selectedProfile": profile})
        self.assertEqual(result.returncode, 0, result.stderr)
        return json.loads(result.stdout.strip())

    def test_five_class_invokes_actual_source_with_current_source_and_fresh_report(self):
        result = self.invoke(profile="platinum_fiveclass")
        self.assertEqual(result["copyCalls"], 0)
        self.assertEqual(result["args"][2], str(self.project / self.verifier_relative))
        self.assertIn("-RequireCurrentSource", result["args"])
        self.assertEqual(result["reportPath"], str(self.proof / "canonical-five-class-verification.json"))
        self.assertEqual(result["profileSha"], self.profile_sha)
        self.assertFalse((self.proof / "verification-source").exists())

    def test_mixed_case_five_class_selection_cannot_fall_back_to_classic(self):
        result = self.invoke(profile="PLATINUM_FIVECLASS")
        self.assertEqual(result["copyCalls"], 0)
        self.assertEqual(result["profile"], "platinum_fiveclass")
        self.assertIn("-RequireCurrentSource", result["args"])

    def test_default_classic_invocation_preserves_original_marker_branch(self):
        result = self.invoke()
        self.assertEqual(result["copyCalls"], 1)
        self.assertEqual(result["profile"], "platinum_176_v27")
        self.assertNotIn("-RequireCurrentSource", result["args"])
        self.assertNotIn("-ReportPath", result["args"])
        self.assertEqual(result["profileSha"], "")

    def test_explicit_classic_profile_uses_original_marker_branch(self):
        result = self.invoke(profile="platinum_176")
        self.assertEqual(result["copyCalls"], 1)
        self.assertIn("platinum_176", result["args"])
        self.assertNotIn("-RequireCurrentSource", result["args"])

if __name__ == "__main__":
    unittest.main()
