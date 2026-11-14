#!/usr/bin/env python3
"""Supplementary byte/closure gate for build-packed-installer.ps1.

The release entry point first runs the real Candidate, bundle and full packed
CMS/verifier gates. This helper does not authenticate signatures, sign, activate,
download or launch anything. It emits only exact, locally verified Inno inputs.
"""
from __future__ import annotations

import argparse
import importlib.util
import json
from pathlib import Path, PurePosixPath
import re
import sys

sys.dont_write_bytecode = True
BASE = Path(__file__).absolute().parent
UPDATER = BASE.parents[2] / "windows-updater/scripts"


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


bootstrap = load("packed_installer_bootstrap", UPDATER / "prepare-bootstrap-installer.py")
packed = load("packed_installer_generation", UPDATER / "prepare-packed-generation.py")
a = bootstrap.archive
require = a.require


def keep_existing(path):
    return (path.startswith(("game/", "generations/", "objects/packs/")) or path in
            {"updater/seed-feed.json", "updater/seed-feed.p7s", "updater/packed-install.json"})


def literal(source, installed):
    line = bootstrap.literal(source, installed)
    return line + (" onlyifdoesntexist" if keep_existing(installed) else "")


def packed_guard(files, progressive_bootstrap=False):
    """Only fresh/exact managed retries are admitted; unknown history is retained."""
    guard = bootstrap.bootstrap_guard(files)
    guard = guard.replace("Bootstrap", "PackedInstall").replace("bootstrap", "packed install")
    for anchor in ("if (Expected = '') or (Uppercase(GetSHA256OfFile(FullPath)) <> Expected) then exit;",
                   "LockSize: Int64;", "not GetFileSizeEx(InstallLockHandle, LockSize) or (LockSize <> 0)",
                   "HasUninstaller := True\n          else", "    Result := (ManagedCount = "):
        require(guard.count(anchor) == 1, "canonical guard template changed; review required")
    guard = guard.replace("if (Expected = '') or (Uppercase(GetSHA256OfFile(FullPath)) <> Expected) then exit;",
                          "if (Expected = '') or not PackedInstallOrdinaryFile(FullPath) or\n"
                          "               (Uppercase(GetSHA256OfFile(FullPath)) <> Expected) then exit;")
    guard = guard.replace("LockSize: Int64;", "LockSize: Int64;\n  LockInfo: TPackedFileInfo;")
    guard = guard.replace("not GetFileSizeEx(InstallLockHandle, LockSize) or (LockSize <> 0)",
                          "not PackedGetFileInformation(InstallLockHandle, LockInfo) or\n"
                          "               (LockInfo.Links <> 1) or\n"
                          "               not GetFileSizeEx(InstallLockHandle, LockSize) or (LockSize <> 0)")
    guard = guard.replace("HasUninstaller := True\n          else", "begin\n"
                          "            if not PackedInstallOrdinaryFile(FullPath) then exit;\n"
                          "            HasUninstaller := True;\n          end\n          else")
    # Empty/subset-exact retry is safe: immutable inputs are never overwritten.
    # Any state, pack-game lease, recovery file or unlisted legacy byte is denied
    # by the complete path allowlist before the installer writes payload bytes.
    start = guard.index("    Result := (ManagedCount = ")
    end = guard.index(";\n  except", start)
    guard = guard[:start] + f"    Result := ManagedCount <= {len(files)}" + guard[end:]
    header = ("{ Generated packed-progressive guard: no active/highest/health authority. }"
              if progressive_bootstrap else "{ Generated packed-full guard: no active/highest/health authority. }")
    return header + """
type
  TPackedFileInfo = record
    Attributes, CreationLow, CreationHigh, AccessLow, AccessHigh: Cardinal;
    WriteLow, WriteHigh, Volume, SizeHigh, SizeLow, Links, IndexHigh, IndexLow: Cardinal;
  end;
  TPackedStreamInfo = record
    Size: Int64;
    Name: array[0..295] of Char;
  end;

function PackedGetFileInformation(Handle: NativeUInt; var Info: TPackedFileInfo): Boolean;
  external 'GetFileInformationByHandle@kernel32.dll stdcall';
function PackedFindFirstStream(Name: String; Level: Cardinal; var Info: TPackedStreamInfo;
  Flags: Cardinal): NativeUInt;
  external 'FindFirstStreamW@kernel32.dll stdcall';
function PackedFindNextStream(Handle: NativeUInt; var Info: TPackedStreamInfo): Boolean;
  external 'FindNextStreamW@kernel32.dll stdcall';
function PackedFindClose(Handle: NativeUInt): Boolean;
  external 'FindClose@kernel32.dll stdcall';
function PackedLastError: Cardinal;
  external 'GetLastError@kernel32.dll stdcall';

function PackedInstallOrdinaryFile(Path: String): Boolean;
var
  Handle, Streams: NativeUInt;
  Info: TPackedFileInfo;
  Stream: TPackedStreamInfo;
  I: Integer;
  DefaultStream: String;
begin
  Result := False;
  if not SafeLocalePath(Path) then exit;
  Handle := CreateFileW(Path, $80000000, 1, 0, 3, $200000, 0);
  if Handle = NativeUInt(-1) then exit;
  try
    if not PackedGetFileInformation(Handle, Info) or (Info.Links <> 1) or
       ((Info.Attributes and $410) <> 0) then exit;
    Streams := PackedFindFirstStream(Path, 0, Stream, 0);
    if Streams = NativeUInt(-1) then exit;
    try
      DefaultStream := '::$DATA';
      for I := 0 to Length(DefaultStream) - 1 do
        if Stream.Name[I] <> DefaultStream[I + 1] then exit;
      if Stream.Name[Length(DefaultStream)] <> #0 then exit;
      if PackedFindNextStream(Streams, Stream) or (PackedLastError <> 38) then exit;
    finally
      PackedFindClose(Streams);
    end;
    Result := True;
  finally
    CloseHandle(Handle);
  end;
end;

""" + guard


def prepare(candidate_root, bundle_root, packed_root, recipe_root, guide_root,
            output, game_revision, engine_revision, candidate, manifest_sha, progressive_bootstrap=False):
    require(type(progressive_bootstrap) is bool, "explicit progressive bootstrap boolean required")
    for value in (game_revision, engine_revision):
        require(re.fullmatch(r"[0-9a-f]{40}", value) is not None, "exact lowercase source required")
    manifest_sha = a.hash_value(manifest_sha.upper(), "reviewed manifest")
    roots = [a.absolute_directory(str(root)) for root in
             (candidate_root, bundle_root, packed_root, recipe_root, guide_root)]
    candidate_root, bundle_root, packed_root, recipe_root, guide_root = roots
    output = Path(output)
    require(output.is_absolute() and not output.exists(), "fresh absolute output required")
    a.absolute_directory(str(output.parent))
    for root in roots:
        require(not output.is_relative_to(root) and not root.is_relative_to(output), "output/input overlap")
    bootstrap._safe_inno_path(output)
    game = bootstrap.scan_tree(candidate_root, "game")
    identity = a.validate_candidate(game)
    version = bootstrap.metadata(game, "VERSION.json")
    require(identity["identity"] == candidate and version["gitRevision"] == game_revision,
            "exact Candidate/game source mismatch")
    bundle_files = bootstrap.scan_tree(bundle_root, "bundle")
    bundle, engine, _ = bootstrap.verify_bundle(bundle_files, game, game_revision, candidate)
    require(bundle["sourceRevision"] == engine_revision == engine["sourceRevision"],
            "exact engine source mismatch")
    proof = packed.verify_release(packed_root, manifest_sha)
    require(proof["sourceClean"] is True and proof["metadataOnly"] is False and
            proof["gameSourceRevision"] == game_revision and
            proof["engineSourceRevision"] == engine_revision, "packed source/full proof mismatch")
    generation_dir = proof["generationDirectory"]
    generation = bootstrap.scan_tree(packed_root / generation_dir, "generation")
    manifest_entry = packed.file_entry(packed_root / generation_dir / "GENERATION.json", "GENERATION.json", 4 * 1024**2)
    require(manifest_entry.sha256 == manifest_sha == generation["GENERATION.json"].sha256,
            "reviewed generation changed after full verification")
    manifest = packed.decoded(packed.read(manifest_entry, 4 * 1024**2))
    packed.validate_manifest(manifest)
    progressive_generation = manifest["schema"] == "mir2.windows.generation.v2"
    require(not progressive_bootstrap or progressive_generation,
            "progressive bootstrap requires signed generation v2 and required scope")
    controls = {"GENERATION.json", "GENERATION.p7s", "catalog.json", "mir2-platform-windows.exe", "mir2-client.toml"}
    if progressive_generation:
        controls.add("required-scope.json")
    require(set(generation) == controls,
            "generation control closure")
    for field in ("exe", "signedConfig", "catalog") + (("requiredScope",) if progressive_generation else ()):
        control = manifest[field]
        require(generation[control["path"]].size == control["size"] and
                generation[control["path"]].sha256 == control["sha256"], "generation control byte binding")
    require(engine["version"] >= manifest["minEngine"], "incompatible engine capability")
    for field in ("exe", "signedConfig"):
        entry = manifest[field]
        require(entry["path"] in game and game[entry["path"]].sha256 == entry["sha256"] and
                game[entry["path"]].size == entry["size"], "Candidate/packed control mismatch")
    catalog = packed.decoded(packed.read(packed.file_entry(
        packed_root / generation_dir / "catalog.json", "catalog.json", packed.MAX_CATALOG), packed.MAX_CATALOG))
    expected_resources = {name[len("mir2-assets/"):]: (entry.size, entry.sha256)
                          for name, entry in game.items() if name.startswith("mir2-assets/")}
    found_resources = {name: (entry["size"], entry["sha256"])
                       for name, entry in catalog["resources"].items()}
    require(expected_resources == found_resources, "Candidate/packed resource closure mismatch")
    required_ids = {entry["sha256"] for entry in manifest["packs"]}
    if progressive_generation:
        scope_entry = manifest["requiredScope"]
        scope_data = packed.read(packed.file_entry(packed_root / generation_dir / scope_entry["path"],
                                                  scope_entry["path"], packed.MAX_REQUIRED_SCOPE), packed.MAX_REQUIRED_SCOPE)
        scope = packed.validate_required_scope(scope_data, scope_entry["sha256"], catalog, manifest["catalog"]["sha256"])
        required_ids = {catalog["resources"][path]["packSha256"] for path in scope["requiredPaths"]}
    objects = bootstrap.scan_tree(packed_root / "objects/packs", "packs")
    expected_packs = {entry["sha256"].lower() + ".m2p": entry for entry in manifest["packs"]}
    require(set(objects) == set(expected_packs), "pack directory closure")
    for name, entry in objects.items():
        require(entry.sha256 == expected_packs[name]["sha256"] and
                entry.size == expected_packs[name]["size"], "pack byte binding")
    required_names = {value.lower() + ".m2p" for value in required_ids}
    require(required_names and required_names <= set(expected_packs), "required pack outside full closure")
    selected_objects = {name: entry for name, entry in objects.items()
                        if not progressive_bootstrap or name in required_names}
    # Feed aliases are input artifacts only; local runtime names are explicit.
    feed_entries = {name: packed.file_entry(packed_root / name, name, 32768)
                    for name in ("latest.json", "latest.p7s")}
    require(feed_entries["latest.json"].sha256 == proof["feed"]["sha256"] and
            feed_entries["latest.p7s"].sha256 == proof["feedSignature"]["sha256"],
            "feed changed after verification")
    guides = bootstrap.scan_tree(guide_root, "guides")
    recipe = bootstrap.scan_tree(recipe_root, "recipe")
    recipe_files = {n: e for n, e in recipe.items() if n in {"Mir2-Invite.iss", "validate-recipe.py"}
                    or n.startswith("languages/")}
    require({"Mir2-Invite.iss", "validate-recipe.py", "languages/Inno-Setup-LICENSE.txt"}
            <= set(recipe_files), "recipe/license closure")
    selected = [(entry, generation_dir + "/" + name) for name, entry in generation.items()]
    selected += [(entry, "objects/packs/" + name) for name, entry in selected_objects.items()]
    selected += [(entry, generation_dir + "/PACK-FEED." + name.split(".")[-1])
                 for name, entry in feed_entries.items()]
    selected += [(game[name], "game/" + name) for name in a.META]
    selected += [(entry, name) for name, entry in bundle_files.items()]
    selected += [(game[name], "licenses/" + name) for name in bootstrap.LICENSES]
    for name in bootstrap.SIMPLIFIED_LICENSES:
        matches = [key for key in (name, "mir2-assets/legal/" + name) if key in game]
        require(len(matches) <= 1, "duplicate simplified license")
        selected += [(game[key], "licenses/" + name) for key in matches]
    selected.append((recipe_files["languages/Inno-Setup-LICENSE.txt"], "licenses/Inno-Setup-LICENSE.txt"))
    for locale in bootstrap.LANGUAGES:
        name = "README." + locale + ".txt"
        require(name in guides, "missing locale guide")
    names = [installed.casefold() for _, installed in selected]
    require(len(names) == len(set(names)), "colliding install paths")
    locator = packed.compact({"schema": "mir2.windows.packed-install.v1",
                              "manifestSha256": manifest_sha.lower()})
    planned = [{"path": name, "size": entry.size, "sha256": entry.sha256}
               for entry, name in sorted(selected, key=lambda pair: pair[1])]
    planned.append({"path": "updater/packed-install.json", "size": len(locator), "sha256": a.sha256(locator)})
    planned += [{"path": "README." + locale + ".txt", "size": guides["README." + locale + ".txt"].size,
                 "sha256": guides["README." + locale + ".txt"].sha256} for locale in bootstrap.LANGUAGES]
    guard = packed_guard(planned, progressive_bootstrap)
    # All byte/guard checks precede literal output creation. No authority/state
    # files are installed: the unsigned locator is only a signed-data location.
    output.mkdir()
    lines, installed = [], []
    for entry, name in sorted(selected, key=lambda pair: pair[1]):
        destination = output / "inputs" / name
        bootstrap.copy_checked(entry, destination)
        lines.append(literal(destination, name))
        installed.append({"path": name, "size": entry.size, "sha256": entry.sha256})
    locator_file = output / "inputs/updater/packed-install.json"
    locator_file.write_bytes(locator)
    lines.append(literal(locator_file, "updater/packed-install.json"))
    installed.append({"path": "updater/packed-install.json", "size": len(locator),
                      "sha256": a.sha256(locator)})
    for name, entry in recipe_files.items():
        bootstrap.copy_checked(entry, output / name)
    for locale in bootstrap.LANGUAGES:
        name = "README." + locale + ".txt"
        bootstrap.copy_checked(guides[name], output / name)
        installed.append({"path": name, "size": guides[name].size, "sha256": guides[name].sha256})
    (output / "verified-payload-files.iss").write_text("\n".join(lines) + "\n", encoding="utf-8-sig")
    (output / "verified-updater-files.iss").write_text("; All nine updater inputs are in the exact packed payload list.\n", encoding="utf-8-sig")
    require(installed == planned, "literal/guard closure changed")
    (output / "verified-packed-guard.iss").write_text(guard, encoding="utf-8-sig")
    report = {"schema": "mir2.windows.packed-installer-inputs.v1", "passed": True,
              "cmsVerified": False, "cmsVerification": "required by build-packed-installer.ps1 before this supplementary helper",
              "candidate": candidate, "gameSourceRevision": game_revision, "engineSourceRevision": engine_revision,
              "manifestSha256": manifest_sha, "sequence": proof["sequence"],
              "packCount": len(selected_objects), "resourceCount": proof["resourceCount"],
              "fullCandidateFileCount": len(game), "installedFiles": installed,
              "installedFileCount": len(installed), "installedBytes": sum(e["size"] for e in installed),
              "gameLooseExeIncluded": False, "gameLooseAssetsIncluded": False,
              "selectorOrHealthIncluded": False, "existingInstallPolicy": "fresh-or-exact-managed-retry-only"}
    if progressive_generation:
        full_bytes = sum(entry.size for entry in objects.values())
        required_bytes = sum(objects[name].size for name in required_names)
        installed_bytes = sum(entry.size for entry in selected_objects.values())
        report.update(generationSchema=manifest["schema"], requiredScope=manifest["requiredScope"],
                      fullResourceCount=proof["resourceCount"], requiredResourceCount=len(scope["requiredPaths"]),
                      progressiveBootstrap=progressive_bootstrap,
                      installationMode="progressive-bootstrap" if progressive_bootstrap else "full",
                      fullPackCount=len(objects), fullPackBytes=full_bytes,
                      requiredPackCount=len(required_names), requiredPackBytes=required_bytes,
                      installedPackCount=len(selected_objects), installedPackBytes=installed_bytes,
                      deferredPackCount=len(objects) - len(selected_objects), deferredPackBytes=full_bytes - installed_bytes,
                      sourceFullResourceByteVerified=True, allResourcePacksIncluded=len(selected_objects) == len(objects),
                      backgroundCompletionRequired=len(selected_objects) < len(objects), gameplayVerified=False)
    (output / "PACKED-INSTALLER-INPUTS.json").write_bytes(packed.compact(report))
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("candidate_root", "bundle_root", "packed_root", "recipe_root", "guide_root", "output"):
        parser.add_argument(name, type=Path)
    for name in ("game-source", "engine-source", "candidate", "manifest-sha"):
        parser.add_argument("--" + name, required=True)
    parser.add_argument("--progressive-bootstrap", action="store_true")
    args = parser.parse_args()
    try:
        result = prepare(args.candidate_root, args.bundle_root, args.packed_root, args.recipe_root,
                         args.guide_root, args.output, args.game_source, args.engine_source,
                         args.candidate, args.manifest_sha, args.progressive_bootstrap)
    except (OSError, ValueError, KeyError, TypeError) as error:
        parser.exit(1, "packed installer inputs rejected: " + str(error) + "\n")
    print(json.dumps(result))


if __name__ == "__main__":
    main()
