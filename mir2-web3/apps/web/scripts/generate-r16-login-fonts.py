"""Build the small Web login subsets from the packaged R16 fonts and UI text."""

import argparse
import json
from pathlib import Path

from fontTools import subset
from fontTools.ttLib import TTFont


WEB = Path(__file__).resolve().parents[1]
FONT_SOURCES = (
    ("zh-CN", "NotoSansCJKsc-Regular.otf", "SC"),
    ("zh-TW", "NotoSansTC.ttf", "TC"),
)
LOCALIZED_WEB_FILES = (
    "app/components/original-client-overlays.tsx",
    "app/components/dubhe-wallet-qr-login.tsx",
    "lib/r16-auth-layout.ts",
    "lib/r16-browser-localization.ts",
)
AUTH_TERMS = ("account", "login", "password", "passkey", "safekey", "wallet", "register", "recover", "security")


def is_auth_text(key):
    return key.startswith("ui.") or any(term in key.lower() for term in AUTH_TERMS)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--font-dir", type=Path, default=WEB / "public/original-ui/fonts")
    font_dir = parser.parse_args().font_dir
    native = json.loads((WEB / "lib/generated/r16-native-localization.json").read_text(encoding="utf8"))
    legacy = json.loads((WEB / "lib/generated/localization_bundle.json").read_text(encoding="utf8"))
    web_text = "".join((WEB / name).read_text(encoding="utf8") for name in LOCALIZED_WEB_FILES)
    output = WEB / "app/login-fonts"
    output.mkdir(exist_ok=True)

    for language, source, region in FONT_SOURCES:
        index = next(i for i, entry in enumerate(native["languages"]) if entry["code"] == language)
        text = web_text + "".join(
            value for key, value in legacy["languages"][language]["texts"].items() if is_auth_text(key)
        )
        text += "".join(
            entry["values"][index] or ""
            for entry in native["entries"]
            if entry["domain"] == "keyed" or is_auth_text(entry["key"])
        )
        text += "".join(entry["nativeName"] for entry in native["languages"])
        font = TTFont(font_dir / source, recalcTimestamp=False)
        characters = ({ord(char) for char in text} | set(range(32, 127))) & font.getBestCmap().keys()
        options = subset.Options()
        options.name_IDs = ["*"]
        options.name_languages = ["*"]
        options.layout_features = ["*"]
        subsetter = subset.Subsetter(options=options)
        subsetter.populate(unicodes=characters)
        subsetter.subset(font)

        family = f"Numeron Login {region}"
        postscript = f"NumeronLogin{region}-Regular"
        for name in font["name"].names:
            if name.nameID in {1, 16}:
                value = family
            elif name.nameID in {3, 4, 18}:
                value = f"{family} Regular"
            elif name.nameID == 6:
                value = postscript
            else:
                continue
            name.string = value.encode(name.getEncoding())
        if "CFF " in font:
            cff = font["CFF "].cff
            cff.fontNames[0] = postscript
            cff.topDictIndex[0].FamilyName = family
            cff.topDictIndex[0].FullName = f"{family} Regular"
        font.flavor = "woff2"
        destination = output / f"r16-login-{region.lower()}.woff2"
        font.save(destination)
        with TTFont(destination) as exported:
            assert characters <= exported.getBestCmap().keys()
        print(f"{destination.name} {destination.stat().st_size} bytes, {len(characters)} mapped characters")


if __name__ == "__main__":
    main()
