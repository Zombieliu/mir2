#!/usr/bin/env python3
"""Actual offline byte/closure regressions with small non-executable fixtures.

Fixture CMS bytes are deliberately invalid. They never stand in for the real
three CMS gates in the release wrapper; a separate test verifies their rejection.
No installation, keys, game, network or publication is used by these tests.
"""
import copy
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

sys.dont_write_bytecode = True
BASE = Path(__file__).absolute().parent
import importlib.util


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


p = load("packed_installer_test_subject", BASE / "prepare-packed-installer-input.py")
b = load("packed_installer_seed_fixtures", p.UPDATER / "test-bootstrap-installer.py")
f = load("packed_installer_pack_fixtures", p.UPDATER / "test-prepare-packed-generation.py")


class PackedInstallerInputs(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="mir2-packed-installer-test-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).absolute()
        self.pack = f.Fixture(self.root)
        b.REVISION = self.pack.rev
        self.candidate = b.candidate_fixture(self.root / "candidate")
        old = json.loads((self.candidate / "PACKAGE-MANIFEST.json").read_bytes())
        payload = {e["path"]: (self.candidate / e["path"]).read_bytes() for e in old["files"]}
        del payload["mir2-assets/fixture/data.txt"]
        (self.candidate / "mir2-assets/fixture/data.txt").unlink()
        payload.update({"mir2-assets/" + key: data for key, data in self.pack.contents.items()})
        payload["mir2-platform-windows.exe"] = self.pack.exe.read_bytes()
        payload["mir2-client.toml"] = self.pack.config.read_bytes()
        attest = json.loads(payload["BUILD-ATTESTATION.json"])
        attest.update(exeSha256=b.archive.sha256(payload["mir2-platform-windows.exe"]),
                      exeSizeBytes=len(payload["mir2-platform-windows.exe"]))
        payload["BUILD-ATTESTATION.json"] = b.dumped(attest)
        for name, data in payload.items():
            b.put(self.candidate, name, data)
        self.rebind_candidate(payload)
        self.bundle = b.bundle_fixture(self.root / "bundle", self.candidate)
        self.set_bundle_revision(self.pack.rev)
        # A real synthetic binary pack/catalog is verified, not a mocked pass flag.
        self.pack.manifest = self.candidate / "PACKAGE-MANIFEST.json"
        self.pack.manifest_value = json.loads(self.pack.manifest.read_bytes())
        self.pack.update_resource_proof()
        self.pack.prepare()
        self.packed = self.root / "packed"
        prepared = json.loads((self.packed / "PREPARED.json").read_bytes())
        self.digest = prepared["manifestSha256"]
        self.generation = prepared["generationDirectory"]
        self.seal_fixture(self.packed, prepared)
        self.guides = self.root / "guides"
        self.guides.mkdir()
        for locale in b.bootstrap.LANGUAGES:
            b.put(self.guides, "README." + locale + ".txt", ("guide " + locale).encode())
        self.output = self.root / "output"

    def set_bundle_revision(self, revision):
        bundle = json.loads((self.bundle / "UPDATER-BUNDLE.json").read_bytes())
        engine_name = next(e["path"] for e in bundle["files"] if e["path"].endswith("/ENGINE.json"))
        engine = json.loads((self.bundle / engine_name).read_bytes())
        engine["sourceRevision"] = revision
        b.put(self.bundle, engine_name, b.dumped(engine))
        seed = json.loads((self.bundle / "updater/seed-feed.json").read_bytes())
        engine_dir = str(Path(engine_name).parent).replace("\\", "/")
        seed["engine"]["metadata"] = [b.entry(n, (self.bundle / engine_dir / n).read_bytes())
                                        for n in ("ENGINE.json", "ENGINE.p7s")]
        b.put(self.bundle, "updater/seed-feed.json", b.dumped(seed))
        bundle["sourceRevision"] = revision
        bundle["files"] = [b.entry(e["path"], (self.bundle / e["path"]).read_bytes())
                           for e in bundle["files"]]
        b.put(self.bundle, "UPDATER-BUNDLE.json", b.dumped(bundle))

    def seal_fixture(self, root, prepared):
        b.put(root, prepared["generationDirectory"] + "/GENERATION.p7s", b"INVALID FIXTURE CMS; never authenticated")
        f.p.finalize_feed(root, prepared["manifestSha256"])
        b.put(root, "latest.p7s", b"INVALID FIXTURE CMS; never authenticated")

    def rebind_candidate(self, payload):
        entries = [b.entry(name, data) for name, data in sorted(payload.items())]
        aggregate = b.archive.sha256("".join(f"{e['path']}\t{e['size']}\t{e['sha256']}\n" for e in entries).encode())
        manifest = {"schema": "mir2.windows.package-manifest.v4",
                    "coverage": {"excludes": list(b.archive.META), "rule": "synthetic complete closure"},
                    "fileCount": len(entries), "totalBytes": sum(e["size"] for e in entries),
                    "aggregateSha256": aggregate, "files": entries}
        b.put(self.candidate, "PACKAGE-MANIFEST.json", b.dumped(manifest))
        version = json.loads((self.candidate / "VERSION.json").read_bytes())
        version.update(exeSha256=b.archive.sha256(payload["mir2-platform-windows.exe"]),
                       exeSizeBytes=len(payload["mir2-platform-windows.exe"]),
                       packageManifestSha256=b.archive.sha256(b.dumped(manifest)),
                       packageManifestAggregateSha256=aggregate, packageManifestFileCount=len(entries),
                       packageFileCount=len(entries) + 4,
                       buildAttestationSha256=b.archive.sha256(payload["BUILD-ATTESTATION.json"]))
        b.put(self.candidate, "VERSION.json", b.dumped(version))
        release = json.loads((self.candidate / "RELEASE-STATEMENT.json").read_bytes())
        release.update(exeSha256=version["exeSha256"], packageManifestSha256=version["packageManifestSha256"],
                       packageManifestAggregateSha256=aggregate, versionSha256=b.archive.sha256(b.dumped(version)),
                       buildAttestationSha256=version["buildAttestationSha256"])
        b.put(self.candidate, "RELEASE-STATEMENT.json", b.dumped(release))

    def run_prepare(self, **changes):
        args = dict(candidate_root=self.candidate, bundle_root=self.bundle, packed_root=self.packed,
                    recipe_root=BASE, guide_root=self.guides, output=self.output,
                    game_revision=self.pack.rev, engine_revision=self.pack.rev,
                    candidate=b.CANDIDATE, manifest_sha=self.digest)
        args.update(changes)
        return p.prepare(**args)

    def rejected(self, **changes):
        with self.assertRaises((ValueError, OSError)):
            self.run_prepare(**changes)
        self.assertFalse(self.output.exists(), "rejection must precede literal output")

    def enable_v2(self):
        data = b"synthetic deferred PNG fixture, never executed"
        item_sha = f.sha(data)
        index = struct.pack("<32sQQQ8s", bytes.fromhex(item_sha), 96, len(data), len(data), b"\0" * 8)
        blob = struct.pack("<8sIIQQ", b"M2PACK01", 1, 1, 64, len(data)) + index + data
        pack_sha = f.sha(blob)
        (self.pack.objects / (pack_sha.lower() + ".m2p")).write_bytes(blob)
        self.pack.catalog_value["packs"].append({"sha256": pack_sha, "size": len(blob)})
        self.pack.catalog_value["resources"]["later/deferred.png"] = {
            "sha256": item_sha, "size": len(data), "packSha256": pack_sha,
            "offset": 96, "storedSize": len(data), "codec": "stored"}
        f.write_json(self.pack.catalog, self.pack.catalog_value)
        old = json.loads((self.candidate / "PACKAGE-MANIFEST.json").read_bytes())
        payload = {e["path"]: (self.candidate / e["path"]).read_bytes() for e in old["files"]}
        payload["mir2-assets/later/deferred.png"] = data
        b.put(self.candidate, "mir2-assets/later/deferred.png", data)
        self.rebind_candidate(payload)
        self.pack.manifest_value = json.loads(self.pack.manifest.read_bytes())
        self.pack.update_resource_proof()
        self.bundle = b.bundle_fixture(self.root / "bundle-v2", self.candidate)
        self.set_bundle_revision(self.pack.rev)
        self.scope = self.root / "scope.json"
        f.write_json(self.scope, {"schema": "mir2.native.required-resource-scope.v1",
                                "catalogSha256": f.sha(self.pack.catalog.read_bytes()), "id": "synthetic-bootstrap",
                                "requiredPaths": ["images/a.png"]})
        self.pack.prepare("packed-v2", required_scope_path=self.scope, required_scope_sha256=f.sha(self.scope.read_bytes()))
        self.packed = self.root / "packed-v2"
        prepared = json.loads((self.packed / "PREPARED.json").read_bytes())
        self.digest, self.generation = prepared["manifestSha256"], prepared["generationDirectory"]
        self.seal_fixture(self.packed, prepared)
        self.deferred_name = pack_sha.lower() + ".m2p"

    def test_v1_progressive_flag_rejected_and_default_stays_full(self):
        self.rejected(progressive_bootstrap=True)
        report = self.run_prepare()
        self.assertNotIn("progressiveBootstrap", report)
        self.assertEqual(report["packCount"], 1)

    def test_v2_full_and_explicit_bootstrap_differ_only_in_selected_pack_payload(self):
        self.enable_v2()
        full = self.run_prepare(output=self.root / "full-inputs")
        partial = self.run_prepare(progressive_bootstrap=True)
        self.assertEqual(full["installationMode"], "full")
        self.assertEqual(full["packCount"], 2)
        self.assertTrue(full["allResourcePacksIncluded"])
        self.assertFalse(full["backgroundCompletionRequired"])
        self.assertEqual(partial["installationMode"], "progressive-bootstrap")
        self.assertEqual(partial["packCount"], partial["requiredPackCount"])
        self.assertEqual(partial["packCount"], 1)
        self.assertEqual(partial["fullPackCount"], 2)
        self.assertEqual(partial["resourceCount"], partial["fullResourceCount"])
        self.assertEqual(partial["fullResourceCount"], 4)
        self.assertFalse(partial["allResourcePacksIncluded"])
        self.assertTrue(partial["backgroundCompletionRequired"])
        self.assertTrue(partial["sourceFullResourceByteVerified"])
        self.assertFalse(partial["cmsVerified"])
        self.assertFalse(partial["gameplayVerified"])
        self.assertEqual(partial["fullPackBytes"], partial["installedPackBytes"] + partial["deferredPackBytes"])
        self.assertEqual(partial["deferredPackCount"], 1)
        def controls(report):
            return {e["path"]: (e["size"], e["sha256"]) for e in report["installedFiles"] if not e["path"].startswith("objects/packs/")}
        self.assertEqual(controls(full), controls(partial))
        self.assertIn(self.generation + "/required-scope.json", controls(partial))
        self.assertFalse((self.output / "inputs/objects/packs" / self.deferred_name).exists())
        self.assertTrue((self.packed / "objects/packs" / self.deferred_name).exists())
        guard = (self.output / "verified-packed-guard.iss").read_text(encoding="utf-8-sig")
        self.assertIn("Generated packed-progressive guard", guard)
        locator = json.loads((self.output / "inputs/updater/packed-install.json").read_bytes())
        self.assertEqual(locator, {"schema": "mir2.windows.packed-install.v1", "manifestSha256": self.digest.lower()})

    def test_bootstrap_still_rejects_corrupt_deferred_source_and_extra_packs(self):
        self.enable_v2()
        deferred = self.packed / "objects/packs" / self.deferred_name
        original = deferred.read_bytes(); deferred.write_bytes(b"corrupt deferred source")
        self.rejected(progressive_bootstrap=True)
        deferred.write_bytes(original)
        b.put(self.packed, "objects/packs/" + "a" * 64 + ".m2p", b"unexpected")
        self.rejected(progressive_bootstrap=True)

    def test_bootstrap_missing_scope_and_post_verification_binding_change_rejected(self):
        self.enable_v2()
        scope = self.packed / self.generation / "required-scope.json"
        original = scope.read_bytes(); scope.unlink()
        self.rejected(progressive_bootstrap=True)
        scope.write_bytes(original)
        real_verify = p.packed.verify_release
        def change_after_real_verification(*args, **kwargs):
            result = real_verify(*args, **kwargs)
            scope.write_bytes(b"X" + original[1:])
            return result
        with mock.patch.object(p.packed, "verify_release", side_effect=change_after_real_verification):
            self.rejected(progressive_bootstrap=True)

    def test_progressive_cli_is_explicit_and_preserves_source_gate_order(self):
        self.enable_v2()
        args = [sys.executable, "-I", "-S", "-B", str(BASE / "prepare-packed-installer-input.py"),
                str(self.candidate), str(self.bundle), str(self.packed), str(BASE), str(self.guides), str(self.output),
                "--game-source", self.pack.rev, "--engine-source", self.pack.rev, "--candidate", b.CANDIDATE,
                "--manifest-sha", self.digest, "--progressive-bootstrap"]
        result = subprocess.run(args, capture_output=True, timeout=60)
        self.assertEqual(result.returncode, 0, result.stdout.decode(errors="replace") + result.stderr.decode(errors="replace"))
        self.assertTrue(json.loads(result.stdout)["progressiveBootstrap"])
        wrapper = (BASE / "build-packed-installer.ps1").read_text(encoding="utf-8-sig")
        self.assertIn("[switch]$ProgressiveBootstrap", wrapper)
        self.assertIn("--progressive-bootstrap", wrapper)
        self.assertLess(wrapper.index("'full-packed-cms'"), wrapper.index("Gate $PythonPath $inputArgs"))

    @unittest.skipUnless(os.name == "nt", "PowerShell copied-scope byte gate")
    def test_copied_scope_gate_rejects_tampering_without_claiming_cms(self):
        self.enable_v2(); self.run_prepare(progressive_bootstrap=True)
        pwsh = shutil.which("pwsh"); self.assertIsNotNone(pwsh)
        script = r'''
$ErrorActionPreference='Stop'
$tokens=$null;$errors=$null
$ast=[Management.Automation.Language.Parser]::ParseFile($env:PACKED_TEST_WRAPPER,[ref]$tokens,[ref]$errors)
if($errors.Count){throw 'Wrapper parse failed'}
foreach($name in @('NoLinks','Sha','ReadJson','Ordinary','VerifyCopiedScopeControl')){
  $definition=@($ast.FindAll({param($node) $node-is[Management.Automation.Language.FunctionDefinitionAst]-and$node.Name-ceq$name},$false))
  if($definition.Count-ne1){throw 'Exact scope byte gate function required'}
  . ([scriptblock]::Create($definition[0].Extent.Text))
}
if((VerifyCopiedScopeControl $env:PACKED_TEST_GENERATION)-ne1){throw 'V2 scope binding failed'}
$path=Join-Path $env:PACKED_TEST_GENERATION 'required-scope.json'
$original=[IO.File]::ReadAllBytes($path)
[IO.File]::WriteAllBytes($path,[byte[]]@(0))
$rejected=$false
try { VerifyCopiedScopeControl $env:PACKED_TEST_GENERATION } catch { $rejected=$true }
if(!$rejected){throw 'Tampered copied scope was accepted'}
[IO.File]::WriteAllBytes($path,$original)
'copied scope byte binding verified; no CMS claim'
'''
        env = dict(os.environ, PACKED_TEST_WRAPPER=str(BASE / "build-packed-installer.ps1"),
                   PACKED_TEST_GENERATION=str(self.output / "inputs" / self.generation))
        result = subprocess.run([pwsh, "-NoProfile", "-Command", script], env=env, capture_output=True, timeout=30)
        self.assertEqual(result.returncode, 0, result.stdout.decode(errors="replace") + result.stderr.decode(errors="replace"))
        self.assertIn(b"no CMS claim", result.stdout)

    def test_exact_packed_layout_locator_and_preserve_flags(self):
        report = self.run_prepare()
        self.assertTrue(report["passed"])
        self.assertFalse(report["cmsVerified"])
        self.assertFalse(report["selectorOrHealthIncluded"])
        self.assertEqual(report["packCount"], 1)
        inputs = self.output / "inputs"
        paths = {entry["path"] for entry in report["installedFiles"]}
        self.assertFalse(any(name.startswith("game/mir2-assets/") for name in paths))
        self.assertNotIn("game/mir2-platform-windows.exe", paths)
        self.assertFalse(any("state/" in name or name.endswith("PREPARED.json") for name in paths))
        self.assertEqual({name for name in paths if name.startswith("game/")}, {"game/" + n for n in b.archive.META})
        locator = json.loads((inputs / "updater/packed-install.json").read_bytes())
        self.assertEqual(locator, {"schema": "mir2.windows.packed-install.v1", "manifestSha256": self.digest.lower()})
        for name in ("json", "p7s"):
            self.assertEqual((inputs / self.generation / ("PACK-FEED." + name)).read_bytes(),
                             (self.packed / ("latest." + name)).read_bytes())
        lines = (self.output / "verified-payload-files.iss").read_text(encoding="utf-8-sig").splitlines()
        for line in lines:
            if any(part in line for part in ("\\game", "/game/", "/generations/", "/objects/packs/", "seed-feed.", "packed-install.json")):
                self.assertIn("onlyifdoesntexist", line)
        self.assertEqual(len(paths), report["installedFileCount"])
        self.assertEqual(report["installedBytes"], sum(e["size"] for e in report["installedFiles"]))
        guard = (self.output / "verified-packed-guard.iss").read_text(encoding="utf-8-sig")
        self.assertIn("Info.Links <> 1", guard)
        self.assertIn("PackedFindFirstStream", guard)
        self.assertNotIn("active-generation.json", guard)
        self.assertNotIn("pack-game.lock", guard)
        self.assertNotIn("transaction.json", guard)
        self.assertNotIn("DeleteFile", guard)

    def test_wrong_identity_source_and_reviewed_manifest_fail_before_output(self):
        for changes in ({"candidate": "WN-CANDIDATE-wrong"}, {"game_revision": "a" * 40},
                        {"engine_revision": "b" * 40}, {"manifest_sha": "C" * 64}):
            with self.subTest(changes=changes):
                self.rejected(**changes)

    def test_distinct_clean_game_and_engine_revisions_are_bound_separately(self):
        engine_root = self.root / "engine-fixture"
        engine_root.mkdir()
        engine = f.Fixture(engine_root)
        (engine.source / "src/lib.rs").write_text("pub fn different_engine() {}\n", encoding="utf-8")
        engine.git("add", ".")
        engine.git("-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
                   "commit", "--quiet", "-m", "different engine fixture")
        revision = engine.git("rev-parse", "HEAD").decode().strip()
        self.assertNotEqual(revision, self.pack.rev)
        args = self.pack.arguments("mixed-packed")
        args.update(engine_source_root=engine.source, engine_revision=revision)
        f.p.prepare(**args)
        mixed = self.root / "mixed-packed"
        prepared = json.loads((mixed / "PREPARED.json").read_bytes())
        self.seal_fixture(mixed, prepared)
        self.set_bundle_revision(revision)
        report = self.run_prepare(packed_root=mixed, engine_revision=revision,
                                  manifest_sha=prepared["manifestSha256"])
        self.assertEqual(report["gameSourceRevision"], self.pack.rev)
        self.assertEqual(report["engineSourceRevision"], revision)

    def test_candidate_extra_bytes_fail_before_output(self):
        b.put(self.candidate, "extra.bin", b"unknown")
        self.rejected()

    def test_candidate_corruption_fails_before_output(self):
        b.put(self.candidate, "mir2-client.toml", b"corrupt")
        self.rejected()

    def test_pack_corruption_fails_before_output(self):
        pack_file = next((self.packed / "objects/packs").iterdir())
        pack_file.write_bytes(b"corrupt")
        self.rejected()

    def test_extra_generation_control_fails_before_output(self):
        b.put(self.packed, self.generation + "/provenance.json", b"private")
        self.rejected()

    def test_extra_pack_fails_before_output(self):
        b.put(self.packed, "objects/packs/" + "a" * 64 + ".m2p", b"extra")
        self.rejected()

    def test_unlisted_bundle_file_fails_before_output(self):
        b.put(self.bundle, "unexpected.exe", b"not executable")
        self.rejected()

    def test_feed_changed_after_draft_is_rejected(self):
        feed = json.loads((self.packed / "latest.json").read_bytes())
        feed["sequence"] += 1
        b.put(self.packed, "latest.json", b.dumped(feed))
        self.rejected()

    def test_dirty_packed_source_is_rejected(self):
        (self.pack.source / "src/lib.rs").write_text("dirty fixture", encoding="utf-8")
        self.rejected()

    def test_required_locale_is_not_silently_dropped(self):
        (self.guides / "README.zh-CN.txt").unlink()
        self.rejected()

    def test_changed_guard_template_fails_before_creating_literal_outputs(self):
        with mock.patch.object(p.bootstrap, "bootstrap_guard", return_value="unexpected future template"):
            self.rejected()

    def test_output_overlap_and_overwrite_are_rejected(self):
        self.rejected(output=self.candidate / "overlap")
        self.run_prepare()
        with self.assertRaises(ValueError):
            self.run_prepare()

    def test_hard_link_and_ads_are_rejected(self):
        target = next((self.packed / "objects/packs").iterdir())
        os.link(target, self.root / "foreign-hard-link")
        self.rejected()

    @unittest.skipUnless(os.name == "nt", "NTFS ADS requires Windows")
    def test_alternate_stream_is_rejected(self):
        with open(str(self.candidate / "mir2-platform-windows.exe") + ":extra", "wb") as stream:
            stream.write(b"named stream")
        self.rejected()

    def test_real_bundle_cms_gate_rejects_synthetic_signature(self):
        pwsh = shutil.which("pwsh")
        self.assertIsNotNone(pwsh, "Windows release tests require PowerShell")
        result = subprocess.run([pwsh, "-NoProfile", "-File", str(p.UPDATER / "verify-updater-bundle.ps1"),
                                 "-BundleRoot", str(self.bundle)], capture_output=True, timeout=30)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(self.output.exists())

    def test_recipe_opt_in_keeps_defaults_and_recovery(self):
        recipe = (BASE / "Mir2-Invite.iss").read_text(encoding="utf-8-sig")
        self.assertIn('AppId={{DC4E7701-25AD-4E48-9970-5C1983CBC77F}', recipe)
        self.assertIn('#include "verified-packed-guard.iss"', recipe)
        self.assertIn('#ifndef Mir2Bootstrap\n#ifndef Mir2Packed\n    RetirePreviousUpdateTransaction;', recipe)
        self.assertIn("Mir2Packed and Mir2Bootstrap are separate install layouts", recipe)
        wrapper = (BASE / "build-packed-installer.ps1").read_text(encoding="utf-8-sig")
        self.assertLess(wrapper.index("verify-windows-candidate.ps1"), wrapper.index("prepare-packed-installer-input.py"))
        self.assertLess(wrapper.index("verify-updater-bundle.ps1"), wrapper.index("prepare-packed-installer-input.py"))
        self.assertLess(wrapper.index("verify-packed-generation.ps1"), wrapper.index("prepare-packed-installer-input.py"))
        self.assertIn("-RequireCurrentSource", wrapper)
        self.assertNotIn("sign-packed-generation.ps1", wrapper)
        self.assertNotIn("AllowDirtyWorktree", wrapper)

    def test_actual_inno_compiles_generated_guard_and_packed_validation_recipe(self):
        compiler = Path(os.environ.get("MIR2_PACKED_TEST_ISCC",
                        "C:/mir2-build/native-zh-cn-finish-20261029/tools/InnoSetup7/ISCC.exe"))
        if not compiler.is_file():
            self.skipTest("actual Inno 7 compiler is not available")
        self.run_prepare()
        b.put(self.output, "tools/vc_redist.x64.exe", b"NONEXECUTABLE compiler fixture; never installed")
        icon = BASE.parents[1] / "resources/numeron.ico"
        result = subprocess.run([str(compiler), "/Qp", "/DMir2Packed=1", "/DMir2ValidationInstall=1",
                                 "/DMir2ValidationAppGuid=5D882E2E-C6CB-46E4-A01F-C10487DF4783",
                                 "/DAppVersion=2026.11.1.17", "/DMir2ReleaseBaseName=Packed-Compiler-Fixture",
                                 "/DMir2IconFile=" + str(icon), "/O" + str(self.root / "compile-output"),
                                 str(self.output / "Mir2-Invite.iss")], capture_output=True, timeout=60)
        self.assertEqual(result.returncode, 0, result.stdout.decode(errors="replace") + result.stderr.decode(errors="replace"))
        self.assertTrue((self.root / "compile-output/Packed-Compiler-Fixture-LocalValidation-Setup.exe").is_file())

    def test_copied_cms_gate_rejects_fixture_without_changing_output_scope(self):
        self.run_prepare()
        pwsh = shutil.which("pwsh")
        self.assertIsNotNone(pwsh)
        # Import only the fixed repository function, never the build entry point.
        # The real canonical CMS helper must reject these deliberately invalid
        # copied signatures, and its parameter scope must not change OutputRoot.
        script = r'''
$ErrorActionPreference='Stop'
$tokens=$null;$errors=$null
$ast=[Management.Automation.Language.Parser]::ParseFile($env:PACKED_TEST_WRAPPER,[ref]$tokens,[ref]$errors)
if($errors.Count){throw 'Wrapper parse failed'}
$definition=@($ast.FindAll({param($node) $node-is[Management.Automation.Language.FunctionDefinitionAst]-and$node.Name-ceq'VerifyCopiedCms'},$false))
if($definition.Count-ne1){throw 'Exact copied-CMS function required'}
. ([scriptblock]::Create($definition[0].Extent.Text))
function ReadJson([string]$Path){Get-Content -LiteralPath $Path -Raw|ConvertFrom-Json}
$OutputRoot='UNCHANGED-CALLER-SCOPE'
$updaterScripts=$env:PACKED_TEST_SCRIPTS
$PackedRoot=$env:PACKED_TEST_RELEASE
$ManifestSha256=$env:PACKED_TEST_SHA
$PackVerifierPath=$env:PACKED_TEST_DUMMY_EXE
$PackVerifierSha256='A'*64
$rejected=$false
try { VerifyCopiedCms $env:PACKED_TEST_INSTALLER } catch {
  if($_.Exception.ToString()-notmatch'CryptographicException'){throw}
  $rejected=$true
}
if(!$rejected-or$OutputRoot-cne'UNCHANGED-CALLER-SCOPE'){throw 'CMS rejection/scope boundary failed'}
'copied CMS rejected; caller OutputRoot unchanged'
'''
        env = dict(os.environ, PACKED_TEST_WRAPPER=str(BASE / "build-packed-installer.ps1"),
                   PACKED_TEST_SCRIPTS=str(p.UPDATER), PACKED_TEST_RELEASE=str(self.packed),
                   PACKED_TEST_SHA=self.digest, PACKED_TEST_DUMMY_EXE=str(self.pack.exe),
                   PACKED_TEST_INSTALLER=str(self.output))
        result = subprocess.run([pwsh, "-NoProfile", "-Command", script], env=env,
                                capture_output=True, timeout=30)
        self.assertEqual(result.returncode, 0, result.stdout.decode(errors="replace") + result.stderr.decode(errors="replace"))
        self.assertIn(b"caller OutputRoot unchanged", result.stdout)


if __name__ == "__main__":
    unittest.main()
