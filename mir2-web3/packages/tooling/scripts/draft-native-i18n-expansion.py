"""Reproducible offline translation drafts, never part of the game runtime.

Requires Python 3.13, ctranslate2==4.6.3 and sentencepiece==0.2.1 in a
separate tooling environment. Models are downloaded from the official Argos
index; hashes and metadata are recorded. Curated, keyed overrides are applied
by the expansion generator, not by this draft tool.
"""
from __future__ import annotations

import argparse
import concurrent.futures
import hashlib
import json
import os
from pathlib import Path
import re
import time
import unicodedata
import urllib.request
import zipfile

INDEX = "https://raw.githubusercontent.com/argosopentech/argospm-index/main/index.json"
LANGUAGES = ("ru", "hi", "id", "vi", "th", "ar")
CATALOGUES = ("common", "content", "shell", "quest", "game", "help", "npc-menus", "npc-prose")
PROJECT = Path(__file__).resolve().parents[3]
DATA = PROJECT / "packages/game-data/data/native-i18n"
PLACEHOLDER = r"\{[A-Za-z0-9_]+\}|\{\d+:[0-9#,.*%+; PpFfNnDdXxGgEeCcRr\-]+\}"
PROTECTED = re.compile(
    PLACEHOLDER
    + r"|(?<![\w.])@[A-Za-z][A-Za-z0-9_]*(?:\([^\r\n)]*\))?"
    + r"|\b(?:F(?:1[0-2]|[1-9])|Ctrl|Alt|Shift|Esc|Tab|Enter|Space|Backspace|Delete|Insert|Home|End|PageUp|PageDown|HP|MP|DC|MC|SC|AC|MAC|EXP|FPS|NPC|WS)\b"
    + r"|\d+(?:[.,]\d+)*(?:%|x\d+)?"
)


def source_texts() -> list[str]:
    return list(dict.fromkeys(entry["en"] for name in CATALOGUES
                             for entry in json.loads((DATA / f"{name}.json").read_text(encoding="utf-8"))["entries"]))


def prepare(text: str, glossary: dict[str, str] | None = None, marker_style: str = "numeric") -> tuple[str, list[str]]:
    # Script colour markup is presentation only. Retain interpolation slots;
    # the localized native NPC renderer supplies its own text styling.
    text = re.sub(r"\{(\{[^{}]+\})/[A-Za-z]+\}", r"\1", text)
    text = re.sub(r"\{([^{}]+)/[A-Za-z]+\}", r"\1", text)
    protected: list[str] = []
    def replace(match: re.Match) -> str:
        original = match.group()
        protected.append((glossary or {}).get(original, original))
        return f"[X{len(protected)}]" if marker_style == "bracket" else str(9875 + len(protected))
    # Adjacent interpolation slots form a single opaque value so their boundary
    # is not guessed by an MT model (or lost by concatenated decimal markers).
    extras = "|".join(re.escape(term) for term in sorted(glossary or {}, key=len, reverse=True))
    pattern = re.compile(r"(?:" + PLACEHOLDER + r")+|" + PROTECTED.pattern + (r"|(?<!\w)(?:" + extras + r")(?!\w)" if extras else ""))
    return pattern.sub(replace, text), protected


def restore(text: str, protected: list[str], marker_style: str = "numeric") -> tuple[str, str | None]:
    # A model may change ASCII to the locale's decimal glyphs or add thousands
    # separators. Normalize only each synthetic number, not arbitrary prose.
    spans = []
    for index, value in enumerate(protected, 1):
        number = str(9875 + index)
        pattern = r"(?<!\d)" + r"[\s,，٬]*".join(
            "[" + digit + chr(0x660 + int(digit)) + chr(0x6f0 + int(digit)) + chr(0x966 + int(digit)) + "]"
            for digit in number) + r"(?!\d)"
        if marker_style == "bracket":
            pattern = r"[\[［]\s*[Xx]\s*" + str(index) + r"\s*[\]］]"
        matches = list(re.finditer(pattern, text))
        if len(matches) != 1:
            return text, f"protected token {index} missing or duplicated"
        spans.append((matches[0].start(), matches[0].end(), value))
    # Inspect the still-synthetic string, before real values (which may legally
    # contain 9876 or [X1]) are restored. Never rewrite guessed partial markers.
    remainder = text
    for start, end, _ in sorted(spans, reverse=True):
        remainder = remainder[:start] + remainder[end:]
    residual = r"\[[Xx]\s*\d+\]" if marker_style == "bracket" else r"(?:987[6-9]|98[89]\d|99\d\d)"
    if re.search(residual, remainder):
        return text, "unconsumed synthetic token"
    for start, end, value in sorted(spans, reverse=True):
        text = text[:start] + value + text[end:]
    return text, None


def translate_drafts(root: Path, output: Path, languages: list[str], device: str, retry_errors: bool = False) -> None:
    import ctranslate2
    import sentencepiece
    if device == "cuda":
        import site
        for directory in site.getsitepackages():
            for dll in Path(directory).glob("nvidia/*/bin"):
                os.add_dll_directory(str(dll))
    sources = source_texts()
    # These models are trained for sentences, not entire help-page paragraphs.
    # Keep every original separator while translating sentence units separately.
    def parts(source: str) -> list[str]:
        return re.split(r"(\n|(?<=[.!?]) +(?=[A-Z]))", source)
    units = list(dict.fromkeys(part.strip() for source in sources for part in parts(source) if part.strip()))
    all_entries = [entry for name in CATALOGUES for entry in json.loads((DATA / f"{name}.json").read_text(encoding="utf-8"))["entries"]]
    overrides = json.loads((PROJECT / "packages/tooling/data/native-i18n/expansion/content-overrides.json").read_text(encoding="utf-8"))
    input_hash = hashlib.sha256(json.dumps(sources, ensure_ascii=False).encode()).hexdigest()
    for language in languages:
        started = time.monotonic()
        glossary = {entry["en"]: overrides[entry["key"]][language] for entry in all_entries
                    if entry["key"] in overrides and entry["key"].startswith("content.") and entry["key"].endswith(".name") and len(entry["en"]) < 60}
        model = next((root / language).rglob("model.bin")).parent
        tokenizer = load_tokenizer(model, language)
        translator = ctranslate2.Translator(str(model), device=device,
            compute_type="float32" if device == "cuda" else "int8", intra_threads=6)
        cache_path = output / f"{language}-units.json"
        cache = json.loads(cache_path.read_text(encoding="utf-8")) if cache_path.exists() else {}
        pending = [unit for unit in units if unit not in cache or (retry_errors and cache[unit]["error"])]
        print(f"{language}: {len(pending)} units to draft; {len(cache)} cached", flush=True)
        for start in range(0, len(pending), 64):
            batch = pending[start:start + 64]
            marker_style = "bracket" if retry_errors else "numeric"
            prepared = [prepare(unit, glossary, marker_style) for unit in batch]
            tokens = [tokenizer.encode(text, out_type=str) for text, _ in prepared]
            results = translator.translate_batch(tokens, beam_size=4,
                max_batch_size=1024, batch_type="tokens", max_input_length=1024,
                max_decoding_length=768, repetition_penalty=1.1, disable_unk=True)
            for source, (_, protected), result in zip(batch, prepared, results):
                translated, error = restore(tokenizer.decode(result.hypotheses[0]).replace("▁", " ").replace("_", " "), protected, marker_style)
                cache[source] = {"text": translated, "error": error, "marker_style": marker_style}
            write_json(cache_path, cache)
            if start % 512 == 0:
                print(f"{language}: {min(start + 64, len(pending))}/{len(pending)} ({time.monotonic()-started:.1f}s)", flush=True)
        entries = {}
        problems = []
        for source in sources:
            lines = []
            for line in parts(source):
                if not line.strip():
                    lines.append(line)
                    continue
                unit = line.strip()
                item = cache[unit]
                if item["error"]:
                    problems.append({"source": source, "unit": unit, **item})
                leading = line[:len(line) - len(line.lstrip())]
                trailing = line[len(line.rstrip()):]
                lines.append(leading + item["text"] + trailing)
            entries[source] = "".join(lines)
        write_json(output / f"{language}.json", {"source_sha256": input_hash,
            "method": "Argos/CTranslate2 local draft; curated overrides and review required",
            "tool_versions": {"ctranslate2": ctranslate2.__version__, "sentencepiece": sentencepiece.__version__},
            "device": device, "compute_type": "float32" if device == "cuda" else "int8",
            "entries": entries, "problems": problems})
        print(f"{language}: done, {len(entries)} sources, {len(problems)} protected-token problems, {time.monotonic()-started:.1f}s", flush=True)
        del translator


def load_tokenizer(model: Path, language: str):
    import sentencepiece
    if (model.parent / "sentencepiece.model").exists():
        return sentencepiece.SentencePieceProcessor(model_file=str(model.parent / "sentencepiece.model"))
    # Argos' OPUS Indonesian package uses Moses + subword-nmt BPE.
    from sacremoses import MosesTokenizer, MosesDetokenizer, MosesPunctNormalizer
    from subword_nmt.apply_bpe import BPE
    class BpeTokenizer:
        def __init__(self):
            self.tokenizer = MosesTokenizer("en")
            self.detokenizer = MosesDetokenizer(language)
            self.normalizer = MosesPunctNormalizer("en")
            with (model.parent / "bpe.model").open(encoding="utf-8") as codes:
                self.bpe = BPE(codes)
        def encode(self, text, out_type=str):
            return self.bpe.segment_tokens(self.tokenizer.tokenize(self.normalizer.normalize(text)))
        def decode(self, tokens):
            return self.detokenizer.detokenize(" ".join(tokens).replace("@@ ", "").split(" "))
    return BpeTokenizer()


def draft_names(models: Path, output: Path, review_path: Path) -> None:
    """Retry untranslated content titles as lexical phrases, not proper names.

    Output remains machine draft data; the keyed review layer always wins.
    The expansion validator independently rejects damaged parameters/numbers.
    """
    import ctranslate2
    report = json.loads(review_path.read_text(encoding="utf-8"))
    values, review = {}, {}
    for language in LANGUAGES:
        sources = sorted({row["source"] for row in report["identicalNeedsReview"]
                          if row["locale"] == language and row["key"].startswith("content.")})
        model = next((models / language).rglob("model.bin")).parent
        tokenizer = load_tokenizer(model, language)
        translator = ctranslate2.Translator(str(model), device="cpu", compute_type="int8", intra_threads=6)
        translations, errors = {}, []
        for start in range(0, len(sources), 64):
            batch, prepared = sources[start:start + 64], []
            for source in batch:
                normalized = re.sub(r"(?<=[a-z])(?=[A-Z])", " ", source).replace("_", " ").lower()
                codes = {token.lower(): token for token in re.findall(
                    r"\b(?:\d+F|[BWNS]|ARCH|ASSA|TAO|WAR|WIZ|HP|MP|EXP|DC|MC|SC|AC|MAC|ACC|[XSML]{1,3})\b", source)}
                prepared.append(prepare(normalized, codes, "bracket"))
            results = translator.translate_batch([tokenizer.encode(text, out_type=str) for text, _ in prepared],
                                                 beam_size=4, max_decoding_length=256, disable_unk=True)
            for source, (_, protected), result in zip(batch, prepared, results):
                text, error = restore(tokenizer.decode(result.hypotheses[0]).replace("▁", " ").replace("_", " ").strip(), protected, "bracket")
                if error or not text or text.casefold() == source.casefold():
                    errors.append({"source": source, "text": text, "error": error or "unchanged"})
                else:
                    translations[source] = text
        for row in report["identicalNeedsReview"]:
            if row["locale"] == language and row["key"].startswith("content.") and row["source"] in translations:
                values.setdefault(row["key"], {})[language] = translations[row["source"]]
        review[language] = {"sources": len(sources), "translated": len(translations), "unresolved": errors}
        print(f"{language}: {len(sources)} titles, {len(translations)} drafts, {len(errors)} unresolved", flush=True)
        del translator
    write_json(output / "name-drafts.json", values)
    write_json(output / "name-draft-review.json", review)


def write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def download(url: str, path: Path) -> None:
    if path.exists():
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + ".part")
    request = urllib.request.Request(url, headers={"User-Agent": "mir2-localization-tool/1.0"})
    with urllib.request.urlopen(request, timeout=90) as response, temporary.open("wb") as output:
        while chunk := response.read(1024 * 1024):
            output.write(chunk)
    temporary.replace(path)


def fetch_models(root: Path) -> None:
    index_path = root / "index.json"
    download(INDEX, index_path)
    packages = json.loads(index_path.read_text(encoding="utf-8"))
    selected = [p for p in packages if p["from_code"] == "en" and p["to_code"] in LANGUAGES]
    if len(selected) != len(LANGUAGES):
        raise ValueError("Expected exactly one English source model per requested language")

    def fetch(package: dict) -> dict:
        code = package["to_code"]
        url = next(link for link in package["links"] if link.startswith("https://"))
        archive = root / url.rsplit("/", 1)[1]
        download(url, archive)
        destination = root / code
        if not (destination / "extracted.json").exists():
            destination.mkdir(parents=True, exist_ok=True)
            with zipfile.ZipFile(archive) as model:
                for entry in model.infolist():
                    target = (destination / entry.filename).resolve()
                    if not target.is_relative_to(destination.resolve()):
                        raise ValueError("Model archive contains a path outside the model directory")
                model.extractall(destination)
            write_json(destination / "extracted.json", {"archive": archive.name})
        metadata = {
            "language": code,
            "url": url,
            "package": package,
            "bytes": archive.stat().st_size,
            "sha256": hashlib.file_digest(archive.open("rb"), "sha256").hexdigest(),
        }
        print(json.dumps(metadata), flush=True)
        return metadata

    with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:
        models = list(pool.map(fetch, selected))
    write_json(root / "model-provenance.json", {
        "index_url": INDEX,
        "index_sha256": hashlib.sha256(index_path.read_bytes()).hexdigest(),
        "license": "MIT / CC0 (Argos maintainer clarification, issue 533)",
        "license_source": "https://github.com/argosopentech/argos-translate/issues/533",
        "models": models,
    })


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("fetch", "translate", "names"))
    parser.add_argument("--models", type=Path, required=True)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--languages", nargs="+", choices=LANGUAGES, default=list(LANGUAGES))
    parser.add_argument("--device", choices=("cpu", "cuda"), default="cpu")
    parser.add_argument("--retry-errors", action="store_true", help="Retry failed units with bracketed opaque markers; never guess missing parameters")
    parser.add_argument("--review", type=Path, help="Keyed expansion review report for the names pass")
    args = parser.parse_args()
    if args.action == "fetch":
        fetch_models(args.models)
    elif args.action == "names" and args.output and args.review:
        draft_names(args.models, args.output, args.review)
    elif args.action == "translate" and args.output:
        translate_drafts(args.models, args.output, args.languages, args.device, args.retry_errors)
    else:
        parser.error("translate requires --output; names requires --output and --review")


if __name__ == "__main__":
    main()
