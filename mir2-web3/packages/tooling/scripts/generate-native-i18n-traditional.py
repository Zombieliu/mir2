"""Generate the zh-TW shared catalogue with official OpenCC and reviewed overrides.

Development-only dependency: python -m pip install opencc==1.4.2
Upstream: https://github.com/BYVoid/OpenCC (Apache-2.0); configuration s2twp.
Run from any working directory. Does not edit gameplay data or user preferences.
The generated outputs are committed; OpenCC is not a runtime game dependency.
"""

import importlib.metadata
import json
from pathlib import Path

from opencc import OpenCC

ROOT = Path(__file__).resolve().parents[3]
BUNDLE = ROOT / "packages/game-data/data/generated/localization_bundle.json"
MIRROR = ROOT / "apps/web/lib/generated/localization_bundle.json"
OVERRIDES = ROOT / "packages/game-data/data/i18n-overrides.json"


def main():
    if importlib.metadata.version("opencc") != "1.4.2":
        raise SystemExit("Reproducible conversion requires official opencc==1.4.2")
    bundle = json.loads(BUNDLE.read_text(encoding="utf-8"))
    overrides = json.loads(OVERRIDES.read_text(encoding="utf-8"))
    convert = OpenCC("s2twp").convert
    english = bundle["languages"]["en"]["texts"]
    simplified = bundle["languages"]["zh-CN"]["texts"]
    traditional = {key: convert(simplified.get(key, value)) for key, value in english.items()}
    # Product terminology, not a claim that every legacy translation was reviewed.
    traditional.update({
        "ui.languageEnglish": "English",
        "ui.languageChinese": "繁體中文",
        "ui.selectLanguage": "語言",
        "ui.languageSettings": "語言設定",
        "ui.languageDescription": "選擇介面顯示語言。",
        "client.Warrior": "戰士",
        "client.Wizard": "法師",
        "client.Taoist": "道士",
        "client.Assassin": "刺客",
        "client.Archer": "弓箭手",
    })
    overrides["zhTW"] = dict(sorted(traditional.items()))
    overrides.setdefault("_meta", {}).setdefault("languages", {})["zhTW"] = {
        "nativeName": "繁體中文", "locale": "zh-TW",
        "source": "zh-CN with official OpenCC 1.4.2 s2twp plus reviewed terminology",
    }
    bundle["languages"]["zh-TW"] = {
        "nativeName": "繁體中文", "locale": "zh-TW", "texts": dict(sorted(traditional.items())),
    }
    # Preserve en/zh-CN/es/pt-BR exactly; additions do not alter their selectors.
    for path, value in [(OVERRIDES, overrides), (BUNDLE, bundle), (MIRROR, bundle)]:
        path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"converter": "OpenCC 1.4.2 s2twp", "zh-TW": len(traditional)}))


if __name__ == "__main__":
    main()
