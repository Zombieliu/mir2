"""Generate a complete native zh-CN keyed overlay, never convert player text.

Development-only official OpenCC 1.4.2 (Apache-2.0), configuration tw2sp.
The generated data is committed; OpenCC is NOT a game/runtime dependency.
Existing shared zh-CN keys take precedence over offline conversion. Only
catalog-owned templates are converted, before opaque arguments are inserted.
Versioned, keyed editorial overrides then take precedence over that baseline.

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
EDITORIAL = DATA / "extra/zh-CN-editorial-overrides.json"
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


def editorial_overrides(document: dict, known_keys: set[str]) -> dict[str, dict]:
    """Reject stale or unsafe copy edits instead of silently changing a template."""
    if document.get("schema") != 1 or document.get("locale") != "zh-CN":
        raise ValueError("Unsupported Simplified editorial document")
    if not isinstance(document.get("revision"), str) or not document["revision"].strip():
        raise ValueError("Editorial revision is required")
    if not isinstance(document.get("reviewMethod"), str) or not document["reviewMethod"].strip():
        raise ValueError("Editorial review method is required")
    if document.get("humanAccepted") is not False:
        raise ValueError("Editorial AI review must not claim human acceptance")
    entries = document.get("entries")
    if not isinstance(entries, list):
        raise ValueError("Editorial entries must be an explicit keyed list")
    output = {}
    for entry in entries:
        if not isinstance(entry, dict):
            raise ValueError("Editorial entries must be keyed objects")
        key = entry.get("key")
        if key not in known_keys or key in output:
            raise ValueError(f"Unknown or duplicate editorial key: {key}")
        for field in ("expectedBase", "value", "category", "reason"):
            if not isinstance(entry.get(field), str) or not entry[field].strip():
                raise ValueError(f"Missing editorial {field}: {key}")
        if entry["expectedBase"] == entry["value"]:
            raise ValueError(f"Redundant editorial override: {key}")
        if tokens(entry["expectedBase"]) != tokens(entry["value"]):
            raise ValueError(f"Editorial changed interpolation/numbers/commands: {key}")
        output[key] = entry
    return output


def apply_editorial(key: str, baseline: str, reviewed: dict[str, dict]) -> str:
    entry = reviewed.get(key)
    if entry is None:
        return baseline
    if baseline != entry["expectedBase"]:
        raise ValueError(f"Editorial baseline is stale; re-review the source: {key}")
    return entry["value"]


def self_test():
    """Focused negative checks need neither OpenCC nor a changed game asset."""
    entry = {"key": "test.copy", "expectedBase": "帐单 {order}：500 金币 @open",
             "value": "账单 {order}：500 金币 @open", "category": "terminology",
             "reason": "Use the Simplified ledger spelling without changing terms."}
    document = {"schema": 1, "locale": "zh-CN", "revision": "test-v1",
                "reviewMethod": "test fixture, no human acceptance",
                "humanAccepted": False, "entries": [entry]}
    reviewed = editorial_overrides(document, {"test.copy"})
    assert apply_editorial("test.copy", entry["expectedBase"], reviewed) == entry["value"]
    assert apply_editorial("opaque", "龍城{Gold}", reviewed) == "龍城{Gold}"
    cases = []
    for name, changed in [
        ("duplicate", dict(document, entries=[entry, entry])),
        ("unknown-key", dict(document, entries=[dict(entry, key="unknown")])),
        ("empty-copy", dict(document, entries=[dict(entry, value=" ")])),
        ("changed-placeholder", dict(document, entries=[dict(entry, value="账单 {other}：500 金币 @open")])),
        ("changed-amount", dict(document, entries=[dict(entry, value="账单 {order}：50 金币 @open")])),
        ("changed-command", dict(document, entries=[dict(entry, value="账单 {order}：500 金币 @close")])),
        ("human-acceptance-claim", dict(document, humanAccepted=True)),
        ("missing-review-method", dict(document, reviewMethod="")),
        ("malformed-entry", dict(document, entries=[None])),
    ]:
        try:
            editorial_overrides(changed, {"test.copy"})
        except ValueError:
            cases.append(name)
        else:
            raise AssertionError(f"Editorial accepted invalid {name}")
    try:
        apply_editorial("test.copy", "上游改动", reviewed)
    except ValueError:
        cases.append("stale-baseline")
    else:
        raise AssertionError("Editorial silently applied to changed source")
    print(json.dumps({"passed": True, "positiveCases": 2, "rejectedCases": cases,
                      "runtimePlayerConversion": False}, indent=2))


def generate(opencc_path: Path | None = None):
    if opencc_path is not None:
        sys.path.insert(0, str(opencc_path.resolve()))
    if importlib.metadata.version("opencc") != OPENCC_VERSION:
        raise ValueError("Reproducible conversion requires official OpenCC 1.4.2")
    import opencc
    from opencc import OpenCC

    convert = OpenCC(OPENCC_CONFIG).convert
    rows, shared, hashes, counts = inputs()
    editorial_document = json.loads(EDITORIAL.read_text(encoding="utf-8"))
    reviewed = editorial_overrides(editorial_document, {row["key"] for row in rows})
    hashes[EDITORIAL.relative_to(ROOT).as_posix()] = text_sha(EDITORIAL)
    for relative, expected in editorial_document.get("sourceReviewSha256", {}).items():
        path = (ROOT / relative).resolve()
        if not path.is_relative_to(ROOT) or path == ROOT:
            raise ValueError(f"Editorial source escaped the project: {relative}")
        actual = text_sha(path)
        if actual != expected:
            raise ValueError(f"Reviewed naming source changed; re-review required: {relative}")
        hashes[relative] = actual
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
        value = apply_editorial(key, value, reviewed)
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
        "methodsScope": "Baseline generation before keyed editorial overrides; final values include the separately counted editorial edits.",
        "converter": dict(TOOL_PIN, configuration=OPENCC_CONFIG, dictionaryAndConfigSha256=dictionaries),
        "sourceTextSha256": hashes,
        "reviewedOverrides": {key: OVERRIDES[key] for key in sorted(override_keys)},
        "editorialReview": {
            "path": EDITORIAL.relative_to(ROOT).as_posix(),
            "revision": editorial_document["revision"],
            "sha256": text_sha(EDITORIAL),
            "reviewMethod": editorial_document["reviewMethod"],
            "reviewCoverage": editorial_document["reviewCoverage"],
            "overrideCount": len(reviewed),
            "categories": dict(sorted(Counter(row["category"] for row in reviewed.values()).items())),
            "precedence": "Pinned OpenCC/shared baseline, four locale/navigation identity labels, then exact-key versioned editorial values.",
            "staleBasePolicy": "Every edited key pins the exact pre-editorial value; upstream changes require re-review.",
            "humanAccepted": False,
        },
        "productTerminology": "Credits uses 点券; guild buff points remain 点数. 账号/账单/到账, 行会 and system labels use consistent Simplified copy. No rates, prices, identity or gameplay data change.",
        "overlaySha256": sha(encoded(output)),
        "scope": "All native base, item/monster/quest/NPC/help, periodic/conquest and embedded system-role/group/shop/mail/billing keys. Known system templates only; opaque player arguments, names, chat, notices and mail bodies are never converted at runtime.",
        "acceptance": "Offline coverage and placeholder/number/command preservation; native layout/glyph tests and human linguistic acceptance are separate.",
    }
    return output, provenance


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--opencc-path", type=Path)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--self-test", action="store_true",
                        help="Run focused editorial validation checks without OpenCC or writes")
    parser.add_argument("--glyph-corpus", type=Path,
                        help="Optional external font-validation corpus; never changes game assets")
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return
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
