"""Verify the unmodified, complete bundled SC face and optional display corpus.

Requires fontTools only for font inspection. No downloads, installs or writes.
Usage: python verify-simplified-font.py [--corpus PATH]
"""

import argparse
import hashlib
import json
from pathlib import Path
import unicodedata

from fontTools.ttLib import TTFont


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--corpus", type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parent
    pin = json.loads((root / "SIMPLIFIED-SOURCE.json").read_text(encoding="utf-8-sig"))
    data = (root / pin["file"]).read_bytes()
    assert len(data) == pin["bytes"] == 16_437_364
    assert hashlib.sha256(data).hexdigest() == pin["sha256"]
    assert hashlib.sha1(b"blob " + str(len(data)).encode() + b"\0" + data).hexdigest() == pin["gitBlobSha1"]
    license_bytes = (root / pin["licenseFile"]).read_bytes()
    assert hashlib.sha256(license_bytes).hexdigest() == pin["licenseSha256"]
    assert hashlib.sha1(b"blob " + str(len(license_bytes)).encode() + b"\0" + license_bytes).hexdigest() == pin["licenseGitBlobSha1"]
    font = TTFont(root / pin["file"], lazy=False)
    assert font.sfntVersion == "OTTO" and "CFF " in font and "fvar" not in font
    assert any(n.nameID in (1, 16) and n.toUnicode() == pin["family"] for n in font["name"].names)
    assert font["OS/2"].usWeightClass == pin["weight"] == 400
    cmap = font.getBestCmap()
    assert len(cmap) == pin["unicodeCodepointCount"]
    assert len(font.getGlyphOrder()) == pin["glyphCount"]
    required = set("简体中文法币充值链月卡账号密码登录创建角色购买状态确认点券余额铁剑屠龙刀")
    corpus_characters = set()
    if args.corpus:
        corpus_characters = {c for c in args.corpus.read_text(encoding="utf-8-sig") if not unicodedata.category(c).startswith("C")}
        required |= corpus_characters
    missing = sorted(ord(c) for c in required if ord(c) not in cmap)
    assert not missing, f"Missing codepoints: {[f'U+{cp:04X}' for cp in missing]}"
    print(json.dumps({"passed": True, "family": pin["family"], "bytes": len(data), "sha256": pin["sha256"],
                      "upstreamGitBlobVerified": True, "glyphCount": len(font.getGlyphOrder()),
                      "unicodeCodepointCount": len(cmap), "corpusUniqueCharacters": len(corpus_characters),
                      "missingCodepoints": missing}, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
