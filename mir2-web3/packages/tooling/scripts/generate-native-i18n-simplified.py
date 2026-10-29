"""Generate a complete native zh-CN keyed overlay, never convert player text.

Development-only official OpenCC 1.4.2 (Apache-2.0), configuration tw2sp.
The generated data is committed; OpenCC is NOT a game/runtime dependency.
Existing shared zh-CN keys take precedence over offline conversion. Only
catalog-owned templates are converted, before opaque arguments are inserted.

Use --opencc-path with an explicitly isolated unpacked official wheel, or an
existing OpenCC environment. --check verifies byte-equivalent generated data.
No package installation, network request, game data or preference edit occurs.
"""

from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import importlib.metadata
import json
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[3]
DATA = ROOT / "packages/game-data/data/native-i18n"
NATIVE = ROOT / "apps/game-client/client-bevy/src/native_i18n.rs"
SHARED = ROOT / "packages/game-data/data/generated/localization_bundle.json"
OUTPUT = DATA / "extra/zh-CN.json"
PROVENANCE = DATA / "extra/zh-CN-provenance.json"
CATALOGUES = ["common", "content", "shell", "quest", "game", "help",
              "npc-menus", "npc-prose", "periodic", "conquest"]
OPENCC_VERSION = "1.4.2"
OPENCC_CONFIG = "tw2sp"
TOOL_PIN = {
    "project": "OpenCC", "version": OPENCC_VERSION,
    "upstream": "https://github.com/BYVoid/OpenCC", "license": "Apache-2.0",
    "metadataUrl": "https://pypi.org/pypi/OpenCC/1.4.2/json",
    "windowsWheel": "opencc-1.4.2-cp313-cp313-win_amd64.whl",
    "windowsWheelUrl": "https://files.pythonhosted.org/packages/cc/15/7bc47cc20436d1eb7163f15b200531bb4e18a6ee261784465019f87b049e/opencc-1.4.2-cp313-cp313-win_amd64.whl",
    "windowsWheelSha256": "4338dc5c7c6c7b42a847f3a8ecbbdfb2543e39d63bebcbe25f10e3c81b1c76fb",
}
OVERRIDES = {
    "ui.languageChinese": "简体中文",
    "billing.fiatTab": "法币充值",
    "billing.usdcTab": "USDC链上充值",
    "billing.monthlyTab": "月卡",
}
SLOTS = re.compile(r"\{[^{}]+\}")
COMMANDS = re.compile(r"(?<![\w.])@[A-Za-z][A-Za-z0-9_]*(?:\([^\r\n)]*\))?")


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def text_sha(path: Path) -> str:
    return sha(path.read_text(encoding="utf-8-sig").replace("\r\n", "\n").encode())


def encoded(value) -> bytes:
    return (json.dumps(value, ensure_ascii=False, indent=2) + "\n").encode("utf-8")


class RustVocabulary:
    """Read literal keyed arrays without depending on Rust source formatting."""

    def __init__(self, source: str):
        self.source = source
        self.at = 0

    def skip(self):
        while self.at < len(self.source):
            if self.source[self.at].isspace():
                self.at += 1
            elif self.source.startswith("//", self.at):
                self.at = self.source.find("\n", self.at) + 1
                if self.at == 0:
                    self.at = len(self.source)
            else:
                break

    def expect(self, literal: str):
        self.skip()
        if not self.source.startswith(literal, self.at):
            raise ValueError(f"Expected {literal!r} near {self.source[self.at:self.at + 60]!r}")
        self.at += len(literal)

    def string(self) -> str:
        self.expect('"')
        output = []
        escapes = {"n": "\n", "r": "\r", "t": "\t", "0": "\0", '"': '"', "\\": "\\"}
        while self.at < len(self.source):
            char = self.source[self.at]
            self.at += 1
            if char == '"':
                return "".join(output)
            if char != "\\":
                output.append(char)
                continue
            char = self.source[self.at]
            self.at += 1
            if char in escapes:
                output.append(escapes[char])
            elif char == "u":
                self.expect("{")
                end = self.source.index("}", self.at)
                output.append(chr(int(self.source[self.at:end].replace("_", ""), 16)))
                self.at = end + 1
            elif char == "x":
                output.append(chr(int(self.source[self.at:self.at + 2], 16)))
                self.at += 2
            else:
                raise ValueError(f"Unsupported Rust string escape {char!r}")
        raise ValueError("Unterminated Rust string")

    def literal_or_constant(self) -> str:
        self.skip()
        if self.source[self.at] == '"':
            return self.string()
        match = re.match(r"[A-Z][A-Z0-9_]*", self.source[self.at:])
        if match is None:
            raise ValueError("Expected embedded literal or string constant")
        name = match.group()
        self.at += len(name)
        definition = re.search(r"const\s+" + re.escape(name) + r"\s*:\s*&str\s*=", self.source)
        if definition is None:
            raise ValueError(f"Unknown literal constant {name}")
        reader = RustVocabulary(self.source)
        reader.at = definition.end()
        return reader.string()

    def entries(self) -> list[dict]:
        start = self.source.index("const BILLING_TRANSLATIONS:")
        self.at = self.source.index("= &[", start) + len("= &[")
        rows = []
        while True:
            self.skip()
            if self.source[self.at] == "]":
                break
            self.expect("(")
            key = self.string()
            self.expect(",")
            self.expect("[")
            values = []
            while True:
                self.skip()
                if self.source[self.at] == "]":
                    break
                values.append(self.literal_or_constant())
                self.skip()
                if self.source[self.at] == ",":
                    self.at += 1
                elif self.source[self.at] != "]":
                    raise ValueError(f"Invalid locale array for {key}")
            self.expect("]")
            self.skip()
            if self.source[self.at] == ",":
                self.at += 1
            self.expect(")")
            self.expect(",")
            if len(values) != 9:
                raise ValueError(f"Expected the nine existing locale inputs for {key}")
            rows.append({"key": key, "en": values[0], "zh-TW": values[1]})
        return rows


def inputs():
    rows = []
    hashes = {}
    counts = {}
    for name in CATALOGUES:
        path = DATA / f"{name}.json"
        entries = json.loads(path.read_text(encoding="utf-8"))["entries"]
        rows.extend(entries)
        counts[name] = len(entries)
        hashes[path.relative_to(ROOT).as_posix()] = text_sha(path)
    embedded = RustVocabulary(NATIVE.read_text(encoding="utf-8")).entries()
    rows.extend(embedded)
    if len({entry["key"] for entry in rows}) != len(rows):
        raise ValueError("Duplicate native key")
    hashes["native_i18n.rs::BILLING_TRANSLATIONS(key,en,zh-TW)"] = sha(encoded(embedded))
    hashes[SHARED.relative_to(ROOT).as_posix()] = text_sha(SHARED)
    shared = json.loads(SHARED.read_text(encoding="utf-8"))["languages"]["zh-CN"]["texts"]
    return rows, shared, hashes, dict(counts, embeddedVocabulary=len(embedded))


def tokens(source: str):
    return {
        "placeholders": sorted(SLOTS.findall(source)),
        "commands": sorted(COMMANDS.findall(source)),
        "numbers": sorted(re.findall(r"\d+", SLOTS.sub("", source))),
    }


def generate(opencc_path: Path | None = None):
    if opencc_path is not None:
        sys.path.insert(0, str(opencc_path.resolve()))
    if importlib.metadata.version("opencc") != OPENCC_VERSION:
        raise ValueError("Reproducible conversion requires official OpenCC 1.4.2")
    import opencc
    from opencc import OpenCC

    convert = OpenCC(OPENCC_CONFIG).convert
    rows, shared, hashes, counts = inputs()
    output = {}
    methods = Counter()
    override_keys = []
    for entry in rows:
        key = entry["key"]
        if key in shared:
            source = shared[key]
            value = source
            method = "existing-shared-zh-CN"
        else:
            source = entry["zh-TW"]
            value = convert(source)
            method = "OpenCC-1.4.2-tw2sp"
        if key.startswith("billing."):
            value = value.replace("点数", "点券")
        if key in OVERRIDES:
            value = OVERRIDES[key]
            override_keys.append(key)
        if not value.strip():
            raise ValueError(f"Empty Simplified Chinese value: {key}")
        if tokens(source) != tokens(value):
            raise ValueError(f"Conversion changed interpolation/numbers/commands: {key}")
        output[key] = value
        methods[method] += 1
    if set(OVERRIDES) - output.keys():
        raise ValueError("Unknown reviewed override key")
    output = dict(sorted(output.items()))
    package = Path(opencc.__file__).resolve().parent
    dictionaries = {}
    for pattern in ["*.json", "*.ocd2"]:
        for path in sorted(package.rglob(pattern)):
            dictionaries[path.relative_to(package).as_posix()] = sha(path.read_bytes())
    provenance = {
        "schema": 1, "locale": "zh-CN", "nativeName": "简体中文",
        "generatedBy": "packages/tooling/scripts/generate-native-i18n-simplified.py",
        "counts": dict(counts, totalKeys=len(output)),
        "methods": dict(sorted(methods.items())),
        "converter": dict(TOOL_PIN, configuration=OPENCC_CONFIG, dictionaryAndConfigSha256=dictionaries),
        "sourceTextSha256": hashes,
        "reviewedOverrides": {key: OVERRIDES[key] for key in sorted(override_keys)},
        "productTerminology": "billing.* uses 点券 for the existing Credits balance; no rates, prices, identity or gameplay data change.",
        "overlaySha256": sha(encoded(output)),
        "scope": "All native base, item/monster/quest/NPC/help, periodic/conquest and embedded system-role/group/shop/mail/billing keys. Known system templates only; opaque player arguments, names, chat, notices and mail bodies are never converted at runtime.",
        "acceptance": "Offline coverage and placeholder/number/command preservation; native layout/glyph tests and human linguistic acceptance are separate.",
    }
    return output, provenance


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--opencc-path", type=Path)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--glyph-corpus", type=Path,
                        help="Optional external font-validation corpus; never changes game assets")
    args = parser.parse_args()
    overlay, provenance = generate(args.opencc_path)
    for path, value in [(OUTPUT, overlay), (PROVENANCE, provenance)]:
        expected = encoded(value)
        if args.check:
            actual = path.read_text(encoding="utf-8").replace("\r\n", "\n").encode()
            if actual != expected:
                raise SystemExit(f"Generated file is stale: {path.relative_to(ROOT)}")
        else:
            path.write_bytes(expected)
    if args.glyph_corpus is not None:
        args.glyph_corpus.parent.mkdir(parents=True, exist_ok=True)
        args.glyph_corpus.write_text("\n".join(overlay.values()) + "\n简体中文\n", encoding="utf-8")
    print(json.dumps({"passed": True, "check": args.check, "keys": len(overlay),
                      "methods": provenance["methods"], "overlaySha256": provenance["overlaySha256"],
                      "glyphCorpus": str(args.glyph_corpus) if args.glyph_corpus else None}, indent=2))


if __name__ == "__main__":
    main()
