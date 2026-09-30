"""Offline installer language/provenance validation; never installs or launches.

Pass the trusted Inno 7 compiler directory. By default player guides live beside
an external recipe, or in the repository's sibling player-readme directory.
This validates text inputs only, not a Candidate, publisher signature or GUI.
"""
import argparse
import hashlib
import json
import pathlib
import re

BASE = pathlib.Path(__file__).resolve().parent
LANGUAGES = {
    "english": "en", "chinesetraditional": "zh-TW",
    "brazilianportuguese": "pt-BR", "russian": "ru", "hindi": "hi",
    "indonesian": "id", "vietnamese": "vi", "thai": "th", "arabic": "ar",
}
TOKENS = re.compile(r"%[1-9]|\[(?:name(?:/ver)?|mb|gb)\]")


def require(condition, message):
    if not condition:
        raise ValueError(message)


def records(path):
    result, section = {}, ""
    for line in path.read_text(encoding="utf-8-sig").splitlines():
        if line.startswith("["):
            section = line
        elif "=" in line and not line.lstrip().startswith(";"):
            name, value = line.split("=", 1)
            key = (section, name)
            require(key not in result, f"duplicate localized key in {path.name}: {name}")
            result[key] = value
    return result


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def validate(compiler, readme):
    reference = records(compiler / "Default.isl")
    required = {k: v for k, v in reference.items() if k[0] == "[Messages]"}
    recipe = (BASE / "Mir2-Invite.iss").read_text(encoding="utf-8-sig")
    section = recipe.split("[Languages]", 1)[1].split("[CustomMessages]", 1)[0]
    entries = re.findall(r'Name: "([a-z]+)"; MessagesFile: "([^"]+)"; InfoBeforeFile: "([^"]+)"', section)
    require(len(entries) == 9 and {row[0] for row in entries} == set(LANGUAGES), "nine unique installer languages required")
    rows = []
    for name, source, guide in entries:
        code = LANGUAGES[name]
        require(guide == f"README.{code}.txt", f"wrong guide for {name}")
        require((readme / guide).is_file(), f"missing guide: {guide}")
        path = (compiler / source[len("compiler:"):].replace("\\", "/")) if source.startswith("compiler:") else BASE / source.replace("\\", "/")
        values = records(path)
        actual = {k: v for k, v in values.items() if k[0] == "[Messages]"}
        require(actual.keys() == required.keys(), f"current message closure mismatch: {name}")
        for key, text in required.items():
            require(sorted(TOKENS.findall(text)) == sorted(TOKENS.findall(actual[key])), f"message placeholder mismatch: {name}/{key[1]}")
        if code == "ar":
            require(values.get(("[LangOptions]", "RightToLeft")) == "yes", "Arabic installer must use RTL")
        if code != "en":
            require(f"if ActiveLanguage = '{name}' then Code := '{code}';" in recipe, f"locale seed missing: {code}")
        rows.append({"locale": code, "messages": len(actual), "languageSha256": sha(path), "guideSha256": sha(readme / guide)})

    custom_text = recipe.split("[CustomMessages]", 1)[1].split("[Tasks]", 1)[0]
    custom = {}
    for line in custom_text.splitlines():
        if not line or line.startswith(";"):
            continue
        key, value = line.split("=", 1)
        name, key = key.split(".", 1)
        require((name, key) not in custom, "duplicate custom message")
        custom[(name, key)] = value
    english = {key: value for (name, key), value in custom.items() if name == "english"}
    require(len(english) == 15, "unexpected installer message closure")
    for name in LANGUAGES:
        translations = {key: value for (language, key), value in custom.items() if language == name}
        require(translations.keys() == english.keys(), f"missing custom message: {name}")
        for key, value in translations.items():
            require(sorted(TOKENS.findall(value)) == sorted(TOKENS.findall(english[key])), f"custom message placeholder mismatch: {name}/{key}")

    sources = json.loads((BASE / "languages/SOURCES.json").read_text(encoding="utf-8"))
    for source in sources["upstream"]:
        file = BASE / "languages" / source.get("preservedAs", source["file"])
        require(sha(file) == source["sha256"], "pinned upstream translation changed")
    for name in ["HINDI-UPDATE.json", "VIETNAMESE-UPDATE.json"]:
        change = json.loads((BASE / "languages" / name).read_text(encoding="utf-8"))
        require(sha(BASE / "languages" / change["derived"]) == change["sha256"], "derived translation provenance changed")
    return {"passed": True, "languages": rows, "customMessagesPerLocale": len(english), "recipeSha256": sha(BASE / "Mir2-Invite.iss"), "guiOrInstallationTested": False, "signatureVerified": False}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("compiler_root", type=pathlib.Path)
    parser.add_argument("--readme-root", type=pathlib.Path)
    args = parser.parse_args()
    readme = args.readme_root or (BASE if (BASE / "README.en.txt").exists() else BASE.parent / "player-readme")
    result = validate(args.compiler_root.resolve(strict=True), readme.resolve(strict=True))
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
