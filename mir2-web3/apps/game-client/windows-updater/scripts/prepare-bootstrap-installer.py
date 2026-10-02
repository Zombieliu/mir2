#!/usr/bin/env python3
"""Prepare a small installer's literal inputs after the PowerShell CMS gates.

Use build-bootstrap-installer.ps1 as the release entry point. This helper checks
the complete game's byte closure and the updater/seed bindings; its receipt is
supplementary byte evidence, never a replacement for detached CMS verification.
It copies no game EXE, build attestation, configuration, or asset payload.
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import os
from pathlib import Path, PurePosixPath
import re
import sys
import time

sys.dont_write_bytecode = True
SCRIPT_ROOT = Path(__file__).absolute().parent
SPEC = importlib.util.spec_from_file_location(
    "mir2_bootstrap_archive_contract", SCRIPT_ROOT / "archive-update-release.py")
archive = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = archive
SPEC.loader.exec_module(archive)
require = archive.require

LANGUAGES = ("en", "zh-TW", "pt-BR", "ru", "hi", "id", "vi", "th", "ar")
LICENSES = ("NotoSansTC-OFL.txt", "OFL-NotoSans.txt", "OFL-NotoSansArabic.txt",
            "OFL-NotoSansDevanagari.txt", "OFL-NotoSansThai.txt",
            "MULTILINGUAL-SOURCES.json")
PIN = "6C70C777B27D50949370D494B4B25798200FBDD5C171BAEEAA94D7E289900F3E"
EXISTING_MESSAGES = {
    "english": "The online installer needs an empty folder. Existing players should open Mir2Launcher.exe in their installed folder to update. Keep the existing installation and its recovery files, or use the full offline installer to repair it.",
    "chinesetraditional": "線上安裝程式需要空白資料夾。已有遊戲的玩家請開啟安裝資料夾中的 Mir2Launcher.exe 更新。請保留現有安裝與復原檔案；如需修復，請使用完整離線安裝程式。",
    "brazilianportuguese": "O instalador online precisa de uma pasta vazia. Quem já instalou o jogo deve abrir Mir2Launcher.exe na pasta instalada para atualizar. Preserve a instalação e seus arquivos de recuperação ou use o instalador offline completo para reparar.",
    "russian": "Для онлайн-установки нужна пустая папка. Если игра уже установлена, откройте Mir2Launcher.exe в её папке для обновления. Сохраните установку и файлы восстановления; для ремонта используйте полный автономный установщик.",
    "hindi": "ऑनलाइन स्थापना के लिए खाली फ़ोल्डर चाहिए। पहले से स्थापित खेल को अपडेट करने के लिए उसके फ़ोल्डर में Mir2Launcher.exe खोलें। मौजूदा स्थापना और पुनर्प्राप्ति फ़ाइलें सुरक्षित रखें, या मरम्मत के लिए पूर्ण ऑफ़लाइन स्थापना कार्यक्रम का उपयोग करें।",
    "indonesian": "Penginstal online memerlukan folder kosong. Jika game sudah terpasang, buka Mir2Launcher.exe di folder instalasi untuk memperbarui. Simpan instalasi dan berkas pemulihannya, atau gunakan penginstal offline lengkap untuk memperbaikinya.",
    "vietnamese": "Trình cài đặt trực tuyến cần một thư mục trống. Nếu đã cài trò chơi, hãy mở Mir2Launcher.exe trong thư mục cài đặt để cập nhật. Giữ bản cài đặt và các tệp khôi phục; dùng trình cài đặt ngoại tuyến đầy đủ nếu cần sửa chữa.",
    "thai": "โปรแกรมติดตั้งออนไลน์ต้องใช้โฟลเดอร์ว่าง หากติดตั้งเกมแล้ว ให้เปิด Mir2Launcher.exe ในโฟลเดอร์ที่ติดตั้งเพื่ออัปเดต โปรดเก็บการติดตั้งเดิมและไฟล์กู้คืนไว้ หรือใช้โปรแกรมติดตั้งออฟไลน์ฉบับเต็มเพื่อซ่อมแซม",
    "arabic": "يحتاج المثبّت عبر الإنترنت إلى مجلد فارغ. إذا كانت اللعبة مثبتة، فافتح Mir2Launcher.exe في مجلد التثبيت لتحديثها. احتفظ بالتثبيت وملفات الاسترداد، أو استخدم المثبّت الكامل دون اتصال لإصلاحه.",
}


def _safe_inno_path(path: Path) -> None:
    require(not any(c in str(path) for c in '\"{}\r\n'), "unsafe Inno source path")


def scan_tree(root: Path, label: str) -> dict:
    files, _, _ = archive.scan({"releases/" + label: str(root)})
    return files["releases/" + label]


def metadata(entries: dict, name: str) -> dict:
    return archive._metadata(entries, name, limit=32_768)[0]


def verify_bundle(entries: dict, candidate_files: dict, expected_revision: str,
                  expected_candidate: str) -> tuple[dict, dict, dict]:
    statement = metadata(entries, "UPDATER-BUNDLE.json")
    require(set(statement) == {"schema", "sourceRevision", "gameCandidate",
            "gameSourceRevision", "files"}
            and statement["schema"] == "mir2.windows.updater-bundle.v1",
            "unsupported updater bundle contract")
    require(re.fullmatch(r"[0-9a-f]{40}", statement["sourceRevision"]) is not None,
            "invalid updater source revision")
    require(statement["gameCandidate"] == expected_candidate
            and statement["gameSourceRevision"] == expected_revision,
            "updater/game exact identity mismatch")
    require(isinstance(statement["files"], list) and len(statement["files"]) == 7,
            "updater bundle must cover exactly seven payload files")
    declared = {}
    for item in statement["files"]:
        require(isinstance(item, dict) and set(item) == {"path", "size", "sha256"},
                "invalid bundle file entry")
        name = archive.relative_path(item["path"])
        require(name not in declared and name.lower() not in {n.lower() for n in declared},
                "bundle duplicate/case-colliding path")
        require(name in entries
                and archive.uint(item["size"], archive.MAX_FILE, "bundle size") == entries[name].size
                and archive.hash_value(item["sha256"], name) == entries[name].sha256,
                "bundle byte identity mismatch: " + name)
        declared[name] = item
    require(set(entries) == set(declared) | {"UPDATER-BUNDLE.json", "UPDATER-BUNDLE.p7s"},
            "extra/missing updater bundle files")
    require(0 < entries["UPDATER-BUNDLE.p7s"].size <= 32_768,
            "updater bundle signature missing/oversized")
    require("updater/active.txt" in entries, "missing engine pointer")
    with archive._open_entry(entries["updater/active.txt"]) as stream:
        pointer = stream.read(65).decode("ascii")
    archive.hash_value(pointer, "active engine pointer")
    prefix = "updater/engines/" + pointer + "/"
    expected_paths = {"Mir2Launcher.exe", "updater/active.txt", "updater/seed-feed.json",
                      "updater/seed-feed.p7s"} | {prefix + n for n in archive.ENGINE_FILES}
    require(set(declared) == expected_paths, "bundle must contain one active immutable engine")
    engine_files = {n: entries[prefix + n] for n in archive.ENGINE_FILES}
    archive.validate_engine(engine_files)
    engine = metadata(engine_files, "ENGINE.json")
    require(engine["exeSha256"] == pointer
            and engine["launcherSha256"] == entries["Mir2Launcher.exe"].sha256,
            "launcher/engine byte binding mismatch")
    feed = metadata(entries, "updater/seed-feed.json")
    require(set(feed) == {"schema", "channel", "platform", "sequence", "createdUnix", "expiresUnix",
                         "minBootstrap", "protocol", "content", "game", "engine"}
            and feed.get("schema") == "mir2.windows.update-feed.v1"
            and feed.get("channel") == "invited" and feed.get("platform") == "windows-x64"
            and feed.get("protocol") == "crystal-mir2-v1"
            and feed.get("content") == "mir2.windows.package-manifest.v4",
            "unsupported signed seed channel/schema")
    archive.uint(feed.get("sequence"), 9_007_199_254_740_991, "seed sequence", positive=True)
    archive.uint(feed.get("minBootstrap"), 1, "minimum bootstrap")
    created = archive.uint(feed.get("createdUnix"), 2**63 - 1, "seed creation time", positive=True)
    expires = archive.uint(feed.get("expiresUnix"), 2**63 - 1, "seed expiry", positive=True)
    require(0 < expires - created <= 31 * 86400, "seed validity interval")
    now = int(time.time())
    require(created <= now + 300 and expires >= now, "bootstrap needs a current signed seed")
    require(isinstance(feed["game"], dict) and isinstance(feed["engine"], dict),
            "seed component objects required")
    require(feed.get("game", {}).get("identity") == expected_candidate,
            "seed game identity mismatch")
    require(feed.get("engine", {}).get("identity") == str(engine["version"]),
            "seed engine identity mismatch")
    for component, names, files in (("game", archive.META, candidate_files),
                                   ("engine", ("ENGINE.json", "ENGINE.p7s"), engine_files)):
        value = feed[component]
        require(isinstance(value, dict) and set(value) == {"directory", "identity", "metadata"},
                "unsupported seed component contract")
        directory = archive.relative_path(value.get("directory"))
        require(directory.startswith("releases/"), "seed component must use immutable releases/")
        listed = value.get("metadata")
        require(isinstance(listed, list) and len(listed) == len(names), "seed metadata closure")
        seen = set()
        for item in listed:
            require(isinstance(item, dict) and set(item) == {"path", "size", "sha256"},
                    "invalid seed metadata entry")
            name = item["path"]
            require(name in names and name not in seen, "seed metadata duplicate/unknown path")
            seen.add(name)
            require(type(item["size"]) is int and item["size"] == files[name].size
                    and item["sha256"] == files[name].sha256,
                    "seed metadata does not bind included " + component + "/" + name)
        require(seen == set(names), "seed metadata closure")
    require(0 < entries["updater/seed-feed.p7s"].size <= 32_768,
            "seed signature missing/oversized")
    return statement, engine, feed


def copy_checked(entry, destination: Path) -> None:
    destination.parent.mkdir(parents=True, exist_ok=True)
    with archive._open_entry(entry) as source, destination.open("xb") as target:
        reader = archive.CheckedReader(source, entry)
        while data := reader.read(1024 * 1024):
            target.write(data)
        reader.verify()
        target.flush()
        os.fsync(target.fileno())


def literal(source: Path, installed: str) -> str:
    _safe_inno_path(source)
    archive.relative_path(installed)
    parent = PurePosixPath(installed).parent.as_posix()
    destination = "{app}" + ("\\" + parent.replace("/", "\\") if parent != "." else "")
    return f'Source: "{source}"; DestDir: "{destination}"; Flags: ignoreversion'


def bootstrap_guard(files: list[dict]) -> str:
    """Hash every managed seed byte; reject gameplay/history or foreign files.

    The only generated exceptions are Inno's ordinary uninstaller files and the
    held, empty install.lock. Neither exception may be a link or a directory.
    A retry is allowed only with every managed bootstrap file still exact.
    """
    directories = {".update", "uninstall", "game/logs"}
    for entry in files:
        parent = PurePosixPath(entry["path"]).parent
        while parent.as_posix() != ".":
            directories.add(parent.as_posix())
            parent = parent.parent
    hashes = "\n".join(f"  if RelativePath = '{e['path']}' then Result := '{e['sha256']}';"
                       for e in files)
    allowed = " or\n    ".join(f"(RelativePath = '{d}')" for d in sorted(directories))
    return f"""{{ Generated only from the exact verified bootstrap byte closure. }}
function GetFileSizeEx(Handle: NativeUInt; var Size: Int64): Boolean;
  external 'GetFileSizeEx@kernel32.dll stdcall';

function ExpectedBootstrapHash(RelativePath: String): String;
begin
  Result := '';
{hashes}
end;

function BootstrapDirectoryAllowed(RelativePath: String): Boolean;
begin
  Result := {allowed};
end;

function InspectBootstrapDirectory(Directory, Prefix: String;
  var ManagedCount: Integer; var HasUninstaller: Boolean): Boolean;
var
  Found: TFindRec;
  FullPath, RelativePath, Expected: String;
  LockSize: Int64;
begin
  Result := False;
  if not SafeLocalePath(Directory) then exit;
  if not FindFirst(AddBackslash(Directory) + '*', Found) then exit;
  try
      repeat
        if (Found.Name <> '.') and (Found.Name <> '..') then
        begin
          RelativePath := Prefix + Found.Name;
          FullPath := AddBackslash(Directory) + Found.Name;
          if (Found.Attributes and $400) <> 0 then exit;
          if (Found.Attributes and $10) <> 0 then
          begin
            if not BootstrapDirectoryAllowed(RelativePath) then exit;
            if RelativePath = 'uninstall' then HasUninstaller := True;
            if not InspectBootstrapDirectory(FullPath, RelativePath + '/', ManagedCount,
              HasUninstaller) then exit;
          end
          else if RelativePath = '.update/install.lock' then
          begin
            if (InstallLockHandle = 0) or
               not GetFileSizeEx(InstallLockHandle, LockSize) or (LockSize <> 0) then exit;
          end
          else if (RelativePath = 'uninstall/unins000.exe') or
                  (RelativePath = 'uninstall/unins000.dat') or
                  (RelativePath = 'uninstall/unins000.msg') then
            HasUninstaller := True
          else
          begin
            Expected := ExpectedBootstrapHash(RelativePath);
            if (Expected = '') or (Uppercase(GetSHA256OfFile(FullPath)) <> Expected) then exit;
            ManagedCount := ManagedCount + 1;
          end;
        end;
      until not FindNext(Found);
  finally
    FindClose(Found);
  end;
  Result := True;
end;

function BootstrapCanInstall: Boolean;
var
  ManagedCount: Integer;
  HasUninstaller: Boolean;
begin
  Result := False;
  ManagedCount := 0;
  HasUninstaller := False;
  try
    if not InspectBootstrapDirectory(ExpandConstant('{{app}}'), '', ManagedCount,
      HasUninstaller) then exit;
    Result := (ManagedCount = {len(files)}) or
      ((ManagedCount = 0) and not HasUninstaller and
       not DirExists(ExpandConstant('{{app}}\\game')) and
       not DirExists(ExpandConstant('{{app}}\\updater')));
  except
    Result := False;
  end;
end;
"""


def prepare(candidate_root: Path, bundle_root: Path, recipe_root: Path, guide_root: Path,
            output: Path, expected_revision: str, expected_candidate: str) -> dict:
    require(re.fullmatch(r"[0-9a-f]{40}", expected_revision) is not None,
            "expected source must be exact lowercase 40-hex revision")
    require(re.fullmatch(r"WN-CANDIDATE-[A-Za-z0-9._-]+", expected_candidate) is not None,
            "expected Candidate identity required")
    roots = [archive.absolute_directory(str(p)) for p in
             (candidate_root, bundle_root, recipe_root, guide_root)]
    candidate_root, bundle_root, recipe_root, guide_root = roots
    require(output.is_absolute() and not os.path.lexists(output), "fresh absolute output required")
    archive.absolute_directory(str(output.parent))
    for root in roots:
        require(not output.is_relative_to(root) and not root.is_relative_to(output),
                "installer output overlaps an input tree")
    _safe_inno_path(output)
    candidate_files = scan_tree(candidate_root, "game")
    identity = archive.validate_candidate(candidate_files)
    version = archive._metadata(candidate_files, "VERSION.json")[0]
    require(identity["identity"] == expected_candidate and version["gitRevision"] == expected_revision,
            "exact game Candidate/source mismatch")
    bundle_files = scan_tree(bundle_root, "bundle")
    bundle, engine, feed = verify_bundle(bundle_files, candidate_files, expected_revision,
                                         expected_candidate)
    documentation = {n: candidate_files[n] for n in LICENSES}
    guide_files = scan_tree(guide_root, "guides")
    guides = {"README." + code + ".txt": guide_files["README." + code + ".txt"]
              for code in LANGUAGES}
    recipe_files = scan_tree(recipe_root, "recipe")
    required_recipe = {n: e for n, e in recipe_files.items()
                       if n == "Mir2-Invite.iss" or n == "validate-recipe.py" or n.startswith("languages/")}
    require({"Mir2-Invite.iss", "validate-recipe.py", "languages/Inno-Setup-LICENSE.txt"}
            <= set(required_recipe), "installer recipe/license closure incomplete")
    require(all(n in candidate_files for n in archive.META), "game seed incomplete")
    # All byte and release-binding validation precedes creation of literal inputs.
    output.mkdir()
    inputs = output / "inputs"
    package_lines, updater_lines, installed_files = [], [], []
    selected = [(candidate_files[n], "game/" + n, package_lines) for n in archive.META]
    selected += [(e, "licenses/" + n, package_lines) for n, e in documentation.items()]
    selected += [(required_recipe["languages/Inno-Setup-LICENSE.txt"],
                  "licenses/Inno-Setup-LICENSE.txt", package_lines)]
    selected += [(e, n, updater_lines) for n, e in bundle_files.items()]
    for entry, installed, lines in sorted(selected, key=lambda row: row[1]):
        destination = inputs / installed
        copy_checked(entry, destination)
        lines.append(literal(destination, installed))
        installed_files.append({"path": installed, "size": entry.size, "sha256": entry.sha256})
    for name, entry in sorted(required_recipe.items()):
        copy_checked(entry, output / name)
    for name, entry in sorted(guides.items()):
        copy_checked(entry, output / name)
        installed_files.append({"path": name, "size": entry.size, "sha256": entry.sha256})
    for name, lines in (("verified-payload-files.iss", package_lines),
                        ("verified-updater-files.iss", updater_lines)):
        with (output / name).open("x", encoding="utf-8-sig") as stream:
            stream.write("\n".join(lines) + "\n")
    with (output / "verified-bootstrap-guard.iss").open("x", encoding="utf-8-sig") as stream:
        stream.write(bootstrap_guard(sorted(installed_files, key=lambda e: e["path"])))
    with (output / "verified-bootstrap-messages.iss").open("x", encoding="utf-8-sig") as stream:
        stream.write("[CustomMessages]\n" + "\n".join(
            name + ".BootstrapExisting=" + message for name, message in EXISTING_MESSAGES.items()) + "\n")
    report = {"schema": "mir2.windows.bootstrap-inputs.v1", "passed": True,
              "candidate": expected_candidate, "gameSourceRevision": expected_revision,
              "updaterSourceRevision": bundle["sourceRevision"], "sequence": feed["sequence"],
              "engineVersion": engine["version"], "engineSha256": engine["exeSha256"],
              "launcherSha256": engine["launcherSha256"], "signingKeySha256": PIN,
              "fullPackageFileCount": len(candidate_files),
              "fullPackageBytes": sum(e.size for e in candidate_files.values()),
              "manifestSha256": candidate_files["PACKAGE-MANIFEST.json"].sha256,
              "installedFileCount": len(installed_files),
              "installedBytes": sum(e["size"] for e in installed_files),
              "gamePayloadIncluded": False, "gameAssetsIncluded": False,
              "gameBuildAttestationIncluded": False, "languages": list(LANGUAGES),
              "recipeInputs": [{"path": n, "size": e.size, "sha256": e.sha256}
                               for n, e in sorted(required_recipe.items())],
              "cmsVerification": "Required separately by build-bootstrap-installer.ps1 before this helper",
              "files": sorted(installed_files, key=lambda e: e["path"])}
    with (output / "BOOTSTRAP-INPUTS.json").open("x", encoding="utf-8") as stream:
        json.dump(report, stream, indent=2)
        stream.write("\n")
    return report


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("candidate_root", "bundle_root", "recipe_root", "guide_root", "output"):
        parser.add_argument(name, type=Path)
    parser.add_argument("--expected-source", required=True)
    parser.add_argument("--expected-candidate", required=True)
    args = parser.parse_args()
    try:
        report = prepare(args.candidate_root, args.bundle_root, args.recipe_root, args.guide_root,
                         args.output, args.expected_source, args.expected_candidate)
    except (OSError, ValueError, KeyError, TypeError) as error:
        parser.exit(1, "bootstrap input rejected: " + str(error) + "\n")
    print(json.dumps({k: v for k, v in report.items() if k != "files"}), flush=True)


if __name__ == "__main__":
    main()
