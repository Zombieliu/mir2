"""Verify an already strictly verified r4 Candidate and generate literal Inno inputs.

First run the repository CMS/package verifier against this immutable package.
This independent byte/closure check does not replace signature verification.
Both package_root and the full expected_source_revision are required arguments.
No default revision, source checkout mutation, package copy, or game launch.
"""
import argparse
import hashlib
import json
import os
import pathlib
import re
import stat

BASE = pathlib.Path(__file__).resolve().parent
CANDIDATE = "WN-CANDIDATE-20260929-invited-04"
METADATA = {"PACKAGE-MANIFEST.json", "VERSION.json", "RELEASE-STATEMENT.json",
            "RELEASE-STATEMENT.p7s"}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def revision_argument(value):
    if not re.fullmatch(r"[0-9a-fA-F]{40}", value):
        raise argparse.ArgumentTypeError("expected_source_revision must be the full 40-hex commit")
    return value.lower()


def digest(data):
    return hashlib.sha256(data).hexdigest().upper()


def reject_link(path):
    info = path.lstat()
    require(not path.is_symlink() and not path.is_junction(), "linked input is forbidden")
    require(not (getattr(info, "st_file_attributes", 0) &
                 getattr(stat, "FILE_ATTRIBUTE_REPARSE_POINT", 0x400)),
            "reparse input is forbidden")
    return info


def safe_relative(relative):
    require(isinstance(relative, str) and bool(relative), "invalid manifest path")
    path = pathlib.PurePosixPath(relative)
    require(not path.is_absolute() and ".." not in path.parts and
            path.as_posix() == relative, "noncanonical manifest path")
    require(not any(c in relative for c in '\\:\r\n"{}'), "unsafe Inno path")
    require(not any(part.casefold() == "logs" for part in path.parts),
            "runtime logs cannot be installer inputs")
    return path


def prepare(package_root, expected_revision):
    unresolved = pathlib.Path(os.path.abspath(package_root))
    for path in (unresolved, *unresolved.parents):
        reject_link(path)
    root = unresolved.resolve(strict=True)
    require(root.is_dir(), "package_root must be a directory")
    require(not any(c in str(root) for c in '"{}\r\n'), "unsafe source directory")
    output = BASE / "verified-payload-files.iss"
    report_path = BASE / "installer-input-verification.json"
    require(not output.exists() and not report_path.exists(),
            "refusing to overwrite previous verified installer inputs")

    version_bytes = (root / "VERSION.json").read_bytes()
    version = json.loads(version_bytes.decode("utf-8-sig"))
    require(version.get("candidate") == CANDIDATE, "candidate must be " + CANDIDATE)
    require(version.get("gitRevision") == expected_revision, "exact source revision mismatch")
    require(version.get("worktreeDirty") is False, "dirty Candidate is forbidden")
    require(version.get("clientOnly") is True, "installer requires a client-only package")

    manifest_bytes = (root / "PACKAGE-MANIFEST.json").read_bytes()
    manifest = json.loads(manifest_bytes.decode("utf-8-sig"))
    release = json.loads((root / "RELEASE-STATEMENT.json").read_text(encoding="utf-8-sig"))
    attestation_bytes = (root / "BUILD-ATTESTATION.json").read_bytes()
    attestation = json.loads(attestation_bytes.decode("utf-8-sig"))
    require(release.get("candidate") == CANDIDATE, "release candidate mismatch")
    for item in (release, attestation):
        require(item.get("gitRevision") == expected_revision and
                item.get("worktreeDirty") is False, "source provenance mismatch")
    require(digest(manifest_bytes) == version["packageManifestSha256"].upper() ==
            release["packageManifestSha256"].upper(), "manifest metadata hash mismatch")
    require(digest(version_bytes) == release["versionSha256"].upper(),
            "version metadata hash mismatch")
    require(digest(attestation_bytes) == version["buildAttestationSha256"].upper() ==
            release["buildAttestationSha256"].upper(), "attestation metadata hash mismatch")
    require(set(manifest["coverage"]["excludes"]) == METADATA,
            "unexpected recursive-manifest exclusions")
    files = manifest["files"]
    entries = {item["path"]: item for item in files}
    require(len(entries) == len(files) == manifest["fileCount"], "duplicate manifest path/count")
    require(len({p.casefold() for p in entries}) == len(entries), "case-colliding paths")
    for relative in entries:
        safe_relative(relative)
    expected = set(entries) | METADATA
    require(len(expected) == len(entries) + 4, "metadata/payload overlap")

    actual = set()
    for directory, folders, names in os.walk(root, followlinks=False):
        parent = pathlib.Path(directory)
        reject_link(parent)
        for name in folders:
            require(stat.S_ISDIR(reject_link(parent / name).st_mode), "non-directory input")
        for name in names:
            file = parent / name
            info = reject_link(file)
            require(stat.S_ISREG(info.st_mode) and info.st_nlink == 1,
                    "nonregular or hard-linked input")
            actual.add(file.relative_to(root).as_posix())
    require(actual == expected,
            f"file closure mismatch: missing={len(expected-actual)} extra={len(actual-expected)}")

    lines, total, metadata_hashes = [], 0, {}
    for index, relative in enumerate(sorted(expected), 1):
        path = safe_relative(relative)
        file = root.joinpath(*path.parts)
        require(file.resolve().is_relative_to(root), "input escaped package")
        with file.open("rb") as stream:
            sha = hashlib.file_digest(stream, "sha256").hexdigest().upper()
        size = file.stat().st_size
        if relative in entries:
            require(size == entries[relative]["size"] and
                    sha == entries[relative]["sha256"].upper(),
                    "payload size/hash mismatch: " + relative)
        else:
            metadata_hashes[relative] = sha
        destination = "{app}\\game"
        if path.parent.as_posix() != ".":
            destination += "\\" + str(path.parent).replace("/", "\\")
        lines.append(f'Source: "{file}"; DestDir: "{destination}"; Flags: ignoreversion')
        total += size
        if index % 30000 == 0:
            print(f"Installer input verified {index}/{len(expected)}", flush=True)
    require(version["exeName"] in entries, "executable is not covered by manifest")
    require(entries[version["exeName"]]["sha256"].upper() == version["exeSha256"].upper() ==
            release["exeSha256"].upper() == attestation["exeSha256"].upper(),
            "executable provenance hash mismatch")
    report = {
        "passed": True, "candidate": CANDIDATE, "sourceRevision": expected_revision,
        "files": len(expected), "bytes": total, "extraFiles": [], "runtimeLogsIncluded": False,
        "metadataSha256": metadata_hashes, "exeSha256": version["exeSha256"],
        "manifestSha256": digest(manifest_bytes),
        "cmsVerification": "required separately by the repository strict package verifier",
    }
    with output.open("x", encoding="utf-8-sig") as stream:
        stream.write("\n".join(lines) + "\n")
    with report_path.open("x", encoding="utf-8") as stream:
        json.dump(report, stream, indent=2)
        stream.write("\n")
    print(json.dumps(report), flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("package_root", help="immutable r4 package already passing the strict CMS verifier")
    parser.add_argument("expected_source_revision", type=revision_argument,
                        help="required exact 40-hex source commit; no default")
    args = parser.parse_args()
    try:
        prepare(args.package_root, args.expected_source_revision)
    except (OSError, ValueError, KeyError, TypeError) as error:
        parser.exit(1, "installer input rejected: " + str(error) + "\n")


if __name__ == "__main__":
    main()
