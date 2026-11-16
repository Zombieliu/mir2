"""Literal supplemental installer include after verify-updater-bundle.ps1 passes."""
import argparse, hashlib, json, pathlib, re, stat

def prepare(root, out):
    root = pathlib.Path(root).absolute()
    for p in [root, *root.parents]:
        m = p.lstat()
        if p.is_symlink() or p.is_junction() or m.st_file_attributes & 0x400:
            raise ValueError("reparse input")
    receipt = json.loads((root / "UPDATER-BUNDLE.json").read_text(encoding="utf-8"))
    if receipt["schema"] != "mir2.windows.updater-bundle.v1":
        raise ValueError("bundle schema")
    files = receipt["files"]
    expected = {f["path"]: f for f in files}
    if len(expected) != 7:
        raise ValueError("bundle file count")
    expected.update({"UPDATER-BUNDLE.json": None, "UPDATER-BUNDLE.p7s": None})
    actual = {p.relative_to(root).as_posix() for p in root.rglob("*") if p.is_file()}
    if actual != expected.keys():
        raise ValueError("bundle closure")
    lines = []
    for name, entry in sorted(expected.items()):
        if not re.fullmatch(r"[A-Za-z0-9._/-]+", name) or ".." in name or name.startswith("/"):
            raise ValueError("unsafe input")
        p = root / name
        m = p.lstat()
        if p.is_symlink() or p.is_junction() or m.st_file_attributes & 0x400 or m.st_nlink != 1 or not stat.S_ISREG(m.st_mode):
            raise ValueError("nonregular input")
        sha = hashlib.file_digest(p.open("rb"), "sha256").hexdigest().upper()
        if entry and (sha != entry["sha256"] or m.st_size != entry["size"]):
            raise ValueError("input hash mismatch")
        parent = pathlib.PurePosixPath(name).parent
        dest = "{app}" + (("\\" + str(parent).replace("/", "\\")) if str(parent) != "." else "")
        if any(c in str(p) for c in '"{}\r\n'):
            raise ValueError("unsafe Inno source path")
        lines.append(f'Source: "{p}"; DestDir: "{dest}"; Flags: ignoreversion')
    with pathlib.Path(out).open("x", encoding="utf-8-sig") as stream:
        stream.write("\n".join(lines)+"\n")
    print(json.dumps({"passed":True,"supplementalFiles":len(lines),"cms":"required separately"}))

if __name__ == "__main__":
    a=argparse.ArgumentParser(description=__doc__)
    a.add_argument("bundle_root");a.add_argument("include_output")
    args=a.parse_args()
    prepare(args.bundle_root,args.include_output)
