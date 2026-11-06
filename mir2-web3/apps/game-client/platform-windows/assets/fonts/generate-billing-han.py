"""Reproduce the bundled two-Han-glyph OFL font using fontTools 4.68.0.

Use an existing fontTools environment, or put the unpacked pinned wheel on a
temporary PYTHONPATH. No installation, download or system change is performed.
The original official Google Fonts EOT is included for offline reproduction.
"""

from __future__ import annotations

import argparse
import hashlib
import io
import json
from pathlib import Path
import struct

SOURCE_SHA256 = "0ca2976cbf7b560ca3a5f565b40b9f17fa648b0966cece2b08a1e39cad4b9917"
SFNT_SHA256 = "3da569a39e0173d80d6e56b5931e0b9c71b78da4a6195ed3da6cb36e1148161f"
TOOL_VERSION = "4.68.0"
UNICODES = [0x20, 0x5E01, 0x94FE]
FAMILY = "MIR2 Billing Han"
POSTSCRIPT_NAME = "MIR2BillingHan-Regular"


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def generate(source: Path, output: Path) -> dict:
    import fontTools
    from fontTools import subset
    from fontTools.ttLib import TTFont

    if fontTools.__version__ != TOOL_VERSION:
        raise SystemExit(f"Expected fontTools {TOOL_VERSION}, found {fontTools.__version__}")
    eot = source.read_bytes()
    if digest(eot) != SOURCE_SHA256:
        raise SystemExit("Official source EOT SHA256 mismatch")
    total_size, sfnt_size, eot_version, flags = struct.unpack_from("<LLLL", eot)
    if total_size != len(eot) or eot_version != 0x00020002 or flags != 0:
        raise SystemExit("Expected pinned uncompressed EOT v2.0002")
    sfnt = eot[-sfnt_size:]
    if digest(sfnt) != SFNT_SHA256 or sfnt[:4] != b"\x00\x01\x00\x00":
        raise SystemExit("Pinned embedded TrueType source mismatch")
    font = TTFont(io.BytesIO(sfnt), recalcTimestamp=False)
    assert "fvar" not in font and font["OS/2"].usWeightClass == 400
    assert sorted(font.getBestCmap()) == UNICODES
    source_copyright = font["name"].getDebugName(0)
    options = subset.Options()
    options.notdef_glyph = True
    options.notdef_outline = True
    options.recommended_glyphs = False
    options.layout_features = []
    options.name_IDs = ["*"]
    options.name_languages = ["*"]
    options.name_legacy = True
    options.glyph_names = True
    worker = subset.Subsetter(options=options)
    worker.populate(unicodes=UNICODES)
    worker.subset(font)
    replacements = {
        1: FAMILY, 2: "Regular", 3: f"{POSTSCRIPT_NAME}-1.000",
        4: f"{FAMILY} Regular",
        5: "Version 1.000; MIR2 two-Han-glyph OFL derivative",
        6: POSTSCRIPT_NAME, 16: FAMILY, 17: "Regular",
        21: FAMILY, 22: "Regular",
    }
    names = font["name"]
    for record in list(names.names):
        if record.nameID in replacements:
            names.setName(replacements[record.nameID], record.nameID,
                          record.platformID, record.platEncID, record.langID)
    for name_id, value in replacements.items():
        names.setName(value, name_id, 3, 1, 0x409)
    names.setName("Licensed under the SIL Open Font License, Version 1.1.", 13, 3, 1, 0x409)
    font["OS/2"].usWeightClass = 400
    font["OS/2"].fsSelection = (font["OS/2"].fsSelection & ~((1 << 0) | (1 << 5))) | (1 << 6)
    font["head"].macStyle &= ~3
    font.save(output, reorderTables=True)
    verified = TTFont(output, recalcTimestamp=False, checkChecksums=2)
    cmap = verified.getBestCmap()
    assert sorted(cmap) == UNICODES and len(verified.getGlyphOrder()) == 4
    assert verified["OS/2"].usWeightClass == 400 and "fvar" not in verified
    assert verified["name"].getDebugName(1) == FAMILY
    assert verified["name"].getDebugName(0) == source_copyright
    return {
        "sha256": digest(output.read_bytes()), "bytes": output.stat().st_size,
        "family": FAMILY, "postScriptName": POSTSCRIPT_NAME,
        "weight": 400, "static": True, "format": "TrueType",
        "unicodeMap": {f"U+{cp:04X}": cmap[cp] for cp in UNICODES},
        "glyphOrder": verified.getGlyphOrder(),
        "copyright": source_copyright,
        "fontToolsVersion": fontTools.__version__,
    }


def main() -> None:
    root = Path(__file__).resolve().parent
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path,
                        default=root / "billing-han-source/NotoSansSC-Text400.eot")
    parser.add_argument("--output", type=Path, default=root / "MIR2BillingHan-Regular.ttf")
    args = parser.parse_args()
    print(json.dumps(generate(args.source, args.output), indent=2))


if __name__ == "__main__":
    main()
