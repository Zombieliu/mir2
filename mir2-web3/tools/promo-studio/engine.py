"""Local, single-worker promo studio. Python stdlib + FFmpeg; no spend by default."""
from __future__ import annotations

import copy
import functools
import hashlib
import json
import math
import os
from pathlib import Path
import queue
import re
import shutil
import stat
import subprocess
import threading
import time
import unicodedata
import urllib.error
import urllib.parse
import urllib.request
import uuid

BASE = Path(__file__).resolve().parent
FORMATS = {"vertical": (720, 1280), "landscape": (1280, 720)}
IMAGE_EXTENSIONS = {".png", ".jpg", ".jpeg", ".webp", ".bmp", ".gif"}
VIDEO_EXTENSIONS = {".mp4", ".mov", ".mkv", ".webm", ".avi", ".m4v"}
PROVENANCE_KINDS = {"ai_creative", "gameplay", "game_screenshot"}
MAX_ASSET_BYTES = 2 * 1024 ** 3
ID_RE = re.compile(r"^[a-f0-9]{32}$")
TAG_RE = re.compile(r"^[a-z][a-z0-9_-]{0,39}$")
SHOT_ID_RE = re.compile(r"^[a-zA-Z0-9][a-zA-Z0-9_-]{0,39}$")
DEMUXERS = {".png": "png_pipe", ".jpg": "jpeg_pipe", ".jpeg": "jpeg_pipe", ".webp": "webp_pipe", ".bmp": "bmp_pipe", ".gif": "gif", ".mp4": "mov", ".mov": "mov", ".m4v": "mov", ".avi": "avi", ".mkv": "matroska", ".webm": "matroska"}
CREATIVE_TOKENS = {
    "zh-TW": ["熟悉的冒險", "新的旅程", "回到瑪法", "再次出發", "你的傳奇", "等你續寫", "一起出發", "重拾初心", "經典記憶", "現在開始"],
    "en": ["A familiar adventure", "A new journey", "Return to Mir", "Start again", "Your legend", "Write the next chapter", "Let's go", "Remember the beginning", "Classic memories", "Begin today"],
}


class StudioError(ValueError):
    def __init__(self, message: str, status: int = 400):
        super().__init__(message)
        self.status = status


class PersistenceError(StudioError):
    def __init__(self):
        super().__init__("Persistent storage is unavailable. Repair storage and restart the studio before submitting work.", 503)


class NoProviderRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        # An Authorization header must never follow a provider redirect.
        return None


@functools.lru_cache(maxsize=1)
def system_voices() -> list[dict]:
    if os.name != "nt":
        return []
    code = "[Console]::OutputEncoding = New-Object Text.UTF8Encoding($false); Add-Type -AssemblyName System.Speech; $promoInventory = New-Object System.Speech.Synthesis.SpeechSynthesizer; @($promoInventory.GetInstalledVoices() | Where-Object { $_.Enabled } | ForEach-Object { @{name=$_.VoiceInfo.Name;culture=$_.VoiceInfo.Culture.Name} }) | ConvertTo-Json -Compress; $promoInventory.Dispose()"
    try:
        result = subprocess.run(["powershell.exe", "-NoProfile", "-NonInteractive", "-Command", code], capture_output=True, timeout=15, check=False, creationflags=subprocess.CREATE_NO_WINDOW)
        if result.returncode:
            return []
        data = json.loads(result.stdout.decode("utf-8-sig"))
        return data if isinstance(data, list) else [data] if isinstance(data, dict) else []
    except (OSError, ValueError, subprocess.TimeoutExpired):
        return []


def clean_text(value, maximum: int, field: str, allow_empty=False) -> str:
    if not isinstance(value, str):
        raise StudioError(f"{field} must be text")
    value = value.strip()
    if len(value) > maximum or (not value and not allow_empty):
        raise StudioError(f"{field} must contain {'0' if allow_empty else '1'}–{maximum} characters")
    if any(ord(c) < 32 and c not in "\n\t" for c in value):
        raise StudioError(f"{field} contains control characters")
    return value


def ass_text(value: str) -> str:
    """ASS override tags are never accepted from scripts or asset labels."""
    return value.replace("\\", "＼").replace("{", "｛").replace("}", "｝").replace("\r", "").replace("\n", "\\N")


def timecode(seconds: float, ass=False) -> str:
    units = max(0, round(seconds * (100 if ass else 1000)))
    denominator = 100 if ass else 1000
    whole, fraction = divmod(units, denominator)
    minutes, second = divmod(whole, 60)
    hour, minute = divmod(minutes, 60)
    return f"{hour}:{minute:02d}:{second:02d}.{fraction:02d}" if ass else f"{hour:02d}:{minute:02d}:{second:02d},{fraction:03d}"


def executable(name: str) -> str | None:
    supplied = os.environ.get(f"PROMO_STUDIO_{name.upper()}")
    if supplied:
        p = Path(supplied)
        return str(p.resolve()) if p.is_file() else None
    found = shutil.which(name)
    if found:
        return found
    if os.name == "nt":
        p = Path(os.environ.get("LOCALAPPDATA", "")) / "Microsoft/WinGet/Links" / f"{name}.exe"
        if p.is_file():
            return str(p)
    return None


def media_input_options(path: Path) -> list[str]:
    """Validate bytes before any decoder; never let content choose a demuxer."""
    extension = path.suffix.lower()
    demuxer = DEMUXERS.get(extension)
    if not demuxer:
        raise StudioError("Unsupported media extension")
    try:
        with path.open("rb") as handle:
            magic = handle.read(32)
    except OSError as exc:
        raise StudioError("The local media file cannot be read") from exc
    matches = {
        ".png": magic.startswith(b"\x89PNG\r\n\x1a\n"),
        ".jpg": magic.startswith(b"\xff\xd8\xff"),
        ".jpeg": magic.startswith(b"\xff\xd8\xff"),
        ".webp": magic.startswith(b"RIFF") and magic[8:12] == b"WEBP",
        ".bmp": magic.startswith(b"BM"),
        ".gif": magic.startswith((b"GIF87a", b"GIF89a")),
        ".avi": magic.startswith(b"RIFF") and magic[8:12] == b"AVI ",
        ".mkv": magic.startswith(b"\x1a\x45\xdf\xa3"),
        ".webm": magic.startswith(b"\x1a\x45\xdf\xa3"),
    }
    if demuxer == "mov":
        valid = len(magic) >= 16 and magic[4:8] == b"ftyp" and int.from_bytes(magic[:4], "big") >= 16
    else:
        valid = matches.get(extension, False)
    if not valid:
        raise StudioError("File bytes do not match the selected media extension; playlists and external references are not media imports")
    whitelist = "mov,mp4,m4a,3gp,3g2,mj2" if demuxer == "mov" else "matroska,webm" if demuxer == "matroska" else demuxer
    options = ["-protocol_whitelist", "file,pipe", "-format_whitelist", whitelist, "-f", demuxer]
    if demuxer == "mov":
        options += ["-enable_drefs", "0", "-use_absolute_path", "0"]
    return options


def probe(path: Path, ffprobe: str | None) -> dict:
    if not ffprobe:
        raise StudioError("ffprobe is unavailable; install FFmpeg or set PROMO_STUDIO_FFPROBE", 503)
    try:
        result = subprocess.run(
            [ffprobe, "-v", "error", *media_input_options(path), "-show_entries", "format=duration,format_name:stream=codec_type,codec_name,width,height,duration", "-of", "json", "-i", str(path)],
            stdin=subprocess.DEVNULL, capture_output=True, timeout=30, check=False, creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise StudioError("Media inspection failed or timed out") from exc
    if result.returncode or len(result.stdout) > 1024 * 1024:
        raise StudioError("The selected file is not valid readable image/video media")
    try:
        data = json.loads(result.stdout)
        stream = next(s for s in data["streams"] if s.get("codec_type") == "video")
        width, height = int(stream["width"]), int(stream["height"])
        duration = float(data.get("format", {}).get("duration") or stream.get("duration") or 0)
    except (ValueError, KeyError, TypeError, StopIteration) as exc:
        raise StudioError("The selected file has no valid video/image stream") from exc
    if not (16 <= width <= 16384 and 16 <= height <= 16384 and width * height <= 100_000_000):
        raise StudioError("Media dimensions are outside the supported range")
    if not math.isfinite(duration) or duration < 0:
        raise StudioError("Invalid media duration")
    return {"width": width, "height": height, "durationSeconds": round(duration, 3), "codec": stream.get("codec_name", "unknown"), "formatName": data.get("format", {}).get("format_name", "")}


def local_regular_file(raw_path: str) -> Path:
    if not isinstance(raw_path, str) or len(raw_path) > 4096 or "\x00" in raw_path:
        raise StudioError("A local absolute file path is required")
    if raw_path.startswith(("\\\\", "//")) or re.match(r"^[a-z]+://", raw_path, re.I):
        raise StudioError("Network paths and remote URLs cannot be imported")
    p = Path(raw_path)
    if not p.is_absolute() or any(part == ".." for part in p.parts):
        raise StudioError("Use a local absolute path without traversal")
    if os.name == "nt":
        import ctypes
        if ctypes.windll.kernel32.GetDriveTypeW(str(p.anchor)) == 4:
            raise StudioError("Mapped network drives cannot be imported")
        if any(":" in part for part in p.parts[1:]):
            raise StudioError("Alternate data streams cannot be imported")
    try:
        for component in (p, *p.parents):
            info = component.lstat()
            if component.is_symlink() or getattr(info, "st_file_attributes", 0) & getattr(stat, "FILE_ATTRIBUTE_REPARSE_POINT", 1024):
                raise StudioError("Symlinks and reparse points cannot be imported")
        info = p.stat()
    except OSError as exc:
        raise StudioError("The local file cannot be read") from exc
    if not stat.S_ISREG(info.st_mode) or not 0 < info.st_size <= MAX_ASSET_BYTES:
        raise StudioError("Select a regular local file between 1 byte and 2 GB")
    return p.resolve()


def template_script(campaign: dict, language: str) -> dict:
    base = {k: copy.deepcopy(v) for k, v in campaign.items() if k != "locales"}
    locales = campaign.get("locales", {})
    if isinstance(locales, dict) and isinstance(locales.get(language), dict):
        base.update(copy.deepcopy(locales[language]))
        # Translations inherit structural constraints from the trusted base shot.
        trusted_shots = campaign.get("shots", [])
        if isinstance(trusted_shots, list) and isinstance(base.get("shots"), list):
            by_id = {shot_id(s.get("shotId", str(i+1))): s for i, s in enumerate(trusted_shots) if isinstance(s, dict)}
            merged = []
            for i, translated in enumerate(base["shots"]):
                if not isinstance(translated, dict):
                    raise StudioError("Campaign locale shot must be an object")
                fallback_id = trusted_shots[i].get("shotId", str(i+1)) if i < len(trusted_shots) and isinstance(trusted_shots[i], dict) else str(i+1)
                sid = shot_id(translated.get("shotId", fallback_id))
                original = by_id.get(sid)
                if original is None:
                    raise StudioError("Localized shotId must reference a trusted base shot")
                required = shot_tags(original)
                if "assetRole" in translated and translated["assetRole"] != original.get("assetRole", "gameplay"):
                    raise StudioError("A localized shot cannot change the trusted asset role")
                if ("assetTags" in translated or "assetsTags" in translated) and shot_tags(translated) != required:
                    raise StudioError("A localized shot cannot change trusted asset tags")
                merged.append({**copy.deepcopy(original), **translated, "shotId": sid, "assetRole": original.get("assetRole", "gameplay"), "assetTags": required})
            base["shots"] = merged
    script = {k: base.get(k) for k in ("title", "theme", "description", "claims", "shots", "cta", "hashtags")}
    script["theme"] = script["theme"] or script["title"]
    script["description"] = script["description"] or ""
    script["claims"] = script["claims"] or []
    script["hashtags"] = script["hashtags"] or []
    script["creativeTokens"] = base.get("creativeTokens", CREATIVE_TOKENS[language])
    return validate_script(script)


def shot_id(value) -> str:
    if isinstance(value, int) and not isinstance(value, bool):
        value = str(value)
    if not isinstance(value, str) or not SHOT_ID_RE.fullmatch(value):
        raise StudioError("shotId must be a stable alphanumeric identifier up to 40 characters")
    return value


def shot_tags(shot: dict) -> list[str]:
    tags = shot.get("assetTags", shot.get("assetsTags", []))
    if not isinstance(tags, list) or len(tags) > 20 or any(not isinstance(t, str) or not TAG_RE.fullmatch(t) for t in tags):
        raise StudioError("assetTags must contain at most 20 lowercase topic identifiers")
    if "assetTags" in shot and "assetsTags" in shot:
        alias = shot["assetsTags"]
        if not isinstance(alias, list) or len(alias) > 20 or any(not isinstance(t, str) or not TAG_RE.fullmatch(t) for t in alias) or sorted(set(tags)) != sorted(set(alias)):
            raise StudioError("assetTags and assetsTags cannot disagree")
    return sorted(set(tags))


def creative_headline(text: str, approved_tokens: list[str]) -> bool:
    """New hooks may combine vetted emotional phrases, never new feature facts."""
    compact = re.sub(r"[\s，。！？、：；,.!?;:·—–\-]+", "", text).casefold()
    tokens = sorted({re.sub(r"[\s，。！？、：；,.!?;:·—–\-]+", "", t).casefold() for t in approved_tokens}, key=len, reverse=True)
    if not compact:
        return False
    count = 0
    while compact:
        matched = next((t for t in tokens if t and compact.startswith(t)), None)
        if not matched:
            return False
        compact = compact[len(matched):]
        count += 1
        if count > 4:
            return False
    return True


def validate_script(script, approved: dict | None = None, provider=False) -> dict:
    if not isinstance(script, dict):
        raise StudioError("script must be a structured object")
    result = {}
    for field, limit in (("title", 80), ("theme", 80), ("description", 1200), ("cta", 160)):
        result[field] = clean_text(script.get(field, ""), limit, field, allow_empty=field == "description")
    claims = script.get("claims", [])
    if not isinstance(claims, list) or len(claims) > 30:
        raise StudioError("claims must be a list of at most 30 verified facts")
    result["claims"] = [clean_text(c, 300, "claim") for c in claims]
    tags = script.get("hashtags", [])
    if not isinstance(tags, list) or len(tags) > 12:
        raise StudioError("hashtags must be a list of at most 12 tags")
    result["hashtags"] = [clean_text(t, 60, "hashtag") for t in tags]
    creative_tokens = script.get("creativeTokens", [])
    if not isinstance(creative_tokens, list) or len(creative_tokens) > 30:
        raise StudioError("creativeTokens must contain at most 30 authored phrases")
    result["creativeTokens"] = [clean_text(t, 80, "creative token") for t in creative_tokens]
    shots = script.get("shots")
    if not isinstance(shots, list) or not 1 <= len(shots) <= 12:
        raise StudioError("A script needs 1–12 shots")
    result["shots"] = []
    seen_shots = set()
    approved_by_id = {s["shotId"]: s for s in approved["shots"]} if approved is not None else {}
    for i, shot in enumerate(shots):
        if not isinstance(shot, dict):
            raise StudioError("Every shot must be an object")
        seconds = shot.get("seconds")
        if isinstance(seconds, bool) or not isinstance(seconds, (float, int)) or not math.isfinite(seconds) or not 1 <= seconds <= 15:
            raise StudioError("Shot durations must be between 1 and 15 seconds")
        role = shot.get("assetRole", "gameplay")
        if role not in ("creative", "gameplay"):
            raise StudioError("assetRole must be creative or gameplay")
        sid = shot_id(shot.get("shotId", str(i+1)))
        if sid in seen_shots:
            raise StudioError("shotId must be unique within a script")
        seen_shots.add(sid)
        required_tags = shot_tags(shot)
        if approved is not None:
            trusted = approved_by_id.get(sid)
            if trusted is None:
                raise StudioError("shotId is outside the trusted campaign")
            if role != trusted["assetRole"]:
                raise StudioError("A script cannot change a trusted shot's assetRole")
            if ("assetTags" in shot or "assetsTags" in shot) and required_tags != trusted["assetTags"]:
                raise StudioError("A script cannot change a trusted shot's assetTags")
            required_tags = copy.deepcopy(trusted["assetTags"])
        normalized = {"shotId": sid, "seconds": float(seconds), "assetRole": role, "assetTags": required_tags}
        for field, limit in (("headline", 60), ("caption", 180), ("narration", 260)):
            normalized[field] = clean_text(shot.get(field, ""), limit, f"shots[{i}].{field}", allow_empty=field != "headline")
        facts = shot.get("factIndexes", [])
        if not isinstance(facts, list) or any(isinstance(n, bool) or not isinstance(n, int) or not 0 <= n < len(result["claims"]) for n in facts):
            raise StudioError("factIndexes must reference supplied verified claims")
        if approved is not None:
            trusted_facts = trusted.get("factIndexes", [])
            if "factIndexes" in shot and facts != trusted_facts:
                raise StudioError("A script cannot change, clear or reorder a trusted shotId's factIndexes")
            facts = trusted_facts
        normalized["factIndexes"] = copy.deepcopy(facts)
        result["shots"].append(normalized)
    if sum(s["seconds"] for s in result["shots"]) > 120:
        raise StudioError("The total video length is limited to 120 seconds")
    if approved is not None:
        # Automatic text may select/reorder authored verified wording; it may not
        # invent rewards, population counts, Boss events or product capabilities.
        if result["claims"] != approved["claims"]:
            raise StudioError("Keep the complete verified claims list in its original order so factIndexes remain stable")
        for field in ("title", "theme", "description", "cta"):
            if result[field] != approved[field]:
                raise StudioError(f"Unapproved {field}; keep this field's supplied campaign wording")
        for shot in result["shots"]:
            trusted = approved_by_id[shot["shotId"]]
            shot_wording = {trusted[field] for field in ("headline", "caption", "narration")}
            for field in ("headline", "caption", "narration"):
                if shot[field] and shot[field] not in shot_wording and not (field == "headline" and creative_headline(shot[field], approved.get("creativeTokens", []))):
                    raise StudioError(f"Unapproved shot {field}; keep wording bound to its trusted shotId")
        if not set(result["hashtags"]).issubset(set(approved["hashtags"])):
            raise StudioError("Use campaign-approved hashtags")
        # Creative phrase authority always comes from the campaign, never POST.
        result["creativeTokens"] = copy.deepcopy(approved.get("creativeTokens", []))
    return result


def _wrap(text: str, units: int) -> list[str]:
    lines, current, used = [], "", 0
    for char in text:
        cost = 2 if unicodedata.east_asian_width(char) in ("W", "F") else 1
        if char == "\n" or (used + cost > units and current):
            lines.append(current.strip())
            current, used = "", 0
        if char != "\n":
            current += char
            used += cost
    if current:
        lines.append(current.strip())
    return lines or [""]


def _fitted(text: str, width: int, base_size: int, max_lines=2) -> tuple[str, int]:
    for size in range(base_size, 15, -1):
        lines = _wrap(text, max(10, int(width / (size * .57))))
        if len(lines) <= max_lines:
            return ass_text("\n".join(lines)), size
    raise StudioError("Script text is too long to fit its video layout; shorten the campaign text")


def source_timing(shots: list[dict], selected: list[dict]) -> list[dict]:
    """Continue each selected video from where its previous shot ended."""
    cursors, timings = {}, []
    for shot, asset in zip(shots, selected):
        if asset["kind"] != "video":
            timings.append({"inputOffsetSeconds": 0.0, "sourceDurationSeconds": 0.0, "sourceLooped": False})
            continue
        duration = float(asset.get("durationSeconds", 0))
        if not math.isfinite(duration) or duration <= 0:
            raise StudioError("An imported video has no valid duration")
        requested = cursors.get(asset["id"], 0.0)
        # Floor rather than round: a seek must remain strictly below the source
        # duration even near the last frame of a short imported recording.
        offset = math.floor((requested % duration) * 1_000_000) / 1_000_000
        timings.append({"inputOffsetSeconds": offset, "sourceDurationSeconds": duration, "sourceLooped": requested >= duration or offset + shot["seconds"] > duration + .000001, "sourcePositionRequestedSeconds": round(requested, 6)})
        cursors[asset["id"]] = requested + shot["seconds"]
    return timings


def choose_shot_assets(shots: list[dict], assets: list[dict]) -> list[dict]:
    selected, counters = [], {}
    for i, shot in enumerate(shots):
        role = shot["assetRole"]
        required_tags = shot_tags(shot)
        key = (role, tuple(required_tags))
        pool = [a for a in assets if ((a["provenance"]["kind"] == "ai_creative") == (role == "creative")) and set(required_tags).issubset(set(a.get("tags", [])))]
        if not pool:
            suffix = " tagged: " + ", ".join(required_tags) if required_tags else ""
            raise StudioError(f"Shot {shot.get('shotId', str(i+1))} requires selected {role} material{suffix}")
        counter = counters.get(key, 0)
        selected.append(pool[counter % len(pool)])
        counters[key] = counter + 1
    return selected


def _ass(script: dict, shot: dict, asset: dict, language: str, width: int, height: int) -> str:
    vertical = height > width
    full_creative = vertical and asset["kind"] == "image" and asset["provenance"]["kind"] == "ai_creative"
    seconds = shot["seconds"]
    rows = ["[Script Info]", "ScriptType: v4.00+", f"PlayResX: {width}", f"PlayResY: {height}", "WrapStyle: 2", "ScaledBorderAndShadow: yes", "", "[V4+ Styles]", "Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding", "Style: Main,Microsoft JhengHei,30,&H00F8F5EE,&H000000FF,&H00101920,&H90101920,-1,0,0,0,100,100,0,0,1,2,0,8,30,30,0,1", "", "[Events]", "Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text"]
    end = timecode(seconds, ass=True)
    def add(text, y, font, color="F8F5EE", alignment=8, x=None, max_lines=2):
        if not text:
            return
        escaped, size = _fitted(text, width - 80, font, max_lines)
        x = width // 2 if x is None else x
        # Override tags below are authored code; escaped content cannot add tags.
        rows.append(f"Dialogue: 0,0:00:00.00,{end},Main,,0,0,0,,{{\\an{alignment}\\pos({x},{y})\\fs{size}\\c&H{color[4:6]}{color[2:4]}{color[0:2]}&}}{escaped}")
    add(script["title"], 44 if full_creative else 50 if vertical else 28, 42 if vertical else 36, "F1D692")
    add(script["theme"], 111 if full_creative else 126 if vertical else 78, 23 if vertical else 20, "8EBBBF")
    add(shot["headline"], 156 if full_creative else 186 if vertical else 111, 42 if vertical else 31)
    disclosures = {
        "ai_creative": "AI 創意視覺 · 非實機畫面" if language == "zh-TW" else "AI CREATIVE · NOT GAMEPLAY",
        "game_screenshot": "截圖演示 · 非連續實機錄影" if language == "zh-TW" else "SCREENSHOT DEMO · STILL IMAGE",
        "gameplay": "實機錄影" if language == "zh-TW" else "GAMEPLAY FOOTAGE",
    }
    add(disclosures[asset["provenance"]["kind"]], 246 if full_creative else 287 if vertical else 167, 20 if full_creative else 17 if vertical else 16, "F1D692", alignment=7, x=36)
    add(shot["caption"], 1024 if vertical else 603, 32 if full_creative else 29 if vertical else 24, max_lines=2)
    narration = shot["narration"] if shot["narration"] != shot["caption"] else ""
    add(narration, 1110 if vertical else 648, 28 if full_creative else 26 if vertical else 20, "A7C9CD", max_lines=3 if vertical else 2)
    add(script["cta"], 1223 if vertical else 694, 22 if full_creative else 19 if vertical else 17, "F1D692", max_lines=1)
    return "\n".join(rows) + "\n"


class Studio:
    def __init__(self, data_dir=None, campaigns_path=None, start_worker=True):
        self.data_dir = Path(data_dir or os.environ.get("PROMO_STUDIO_DATA") or BASE / "artifacts/promo-studio").resolve()
        self.campaigns_path = Path(campaigns_path or BASE / "campaigns.json")
        self.data_dir.mkdir(parents=True, exist_ok=True)
        (self.data_dir / "assets").mkdir(exist_ok=True)
        (self.data_dir / "jobs").mkdir(exist_ok=True)
        self.state_path = self.data_dir / "state.json"
        self.ffmpeg, self.ffprobe = executable("ffmpeg"), executable("ffprobe")
        self.lock = threading.RLock()
        self.queue = queue.Queue()
        self.assets, self.jobs = {}, {}
        self.stopping = threading.Event()
        self.worker = None
        self.storage_error = None
        self._lease = None
        if start_worker:
            self._acquire_lease()
        if self.state_path.exists():
            try:
                state = json.loads(self.state_path.read_text(encoding="utf-8"))
                self.assets = {a["id"]: a for a in state.get("assets", []) if ID_RE.fullmatch(a["id"])}
                self.jobs = {j["id"]: j for j in state.get("jobs", []) if ID_RE.fullmatch(j["id"])}
            except (ValueError, KeyError, TypeError, OSError) as exc:
                self._release_lease()
                raise StudioError("Cannot read persistent state; preserve state.json and repair it before restarting", 500) from exc
            for job in self.jobs.values():
                if job["status"] == "running":
                    job.update(status="error", error="Rendering was interrupted by a server restart. Submit a new job to retry.")
            try:
                self._save_locked()
            except PersistenceError:
                self._release_lease()
                raise
        if start_worker:
            self.start_worker()

    def _save_locked(self):
        self._ensure_storage_healthy()
        temporary = self.data_dir / f".state-{uuid.uuid4().hex}.tmp"
        payload = {"version": 1, "assets": list(self.assets.values()), "jobs": list(self.jobs.values())}
        try:
            with temporary.open("w", encoding="utf-8", newline="\n") as handle:
                json.dump(payload, handle, ensure_ascii=False, indent=2, allow_nan=False)
                handle.flush()
                os.fsync(handle.fileno())
            os.replace(temporary, self.state_path)
        except (OSError, ValueError, TypeError) as exc:
            self.storage_error = f"Persistent state save failed ({type(exc).__name__}). Repair storage and restart the studio."
            raise PersistenceError() from exc
        finally:
            try:
                temporary.unlink(missing_ok=True)
            except OSError:
                pass

    def _ensure_storage_healthy(self):
        if self.storage_error is not None:
            raise PersistenceError()

    def _acquire_lease(self):
        if self._lease is not None:
            return
        handle = (self.data_dir / ".studio.lock").open("a+b")
        if handle.seek(0, os.SEEK_END) == 0:
            handle.write(b"0")
            handle.flush()
        handle.seek(0)
        try:
            if os.name == "nt":
                import msvcrt
                msvcrt.locking(handle.fileno(), msvcrt.LK_NBLCK, 1)
            else:
                import fcntl
                fcntl.flock(handle.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
        except OSError as exc:
            handle.close()
            raise StudioError("This data directory already has an active promo studio worker", 409) from exc
        self._lease = handle

    def _release_lease(self):
        if self._lease is not None:
            if os.name == "nt":
                import msvcrt
                self._lease.seek(0)
                msvcrt.locking(self._lease.fileno(), msvcrt.LK_UNLCK, 1)
            else:
                import fcntl
                fcntl.flock(self._lease.fileno(), fcntl.LOCK_UN)
            self._lease.close()
            self._lease = None

    def campaigns(self):
        try:
            if not self.campaigns_path.exists():
                return []
            data = json.loads(self.campaigns_path.read_text(encoding="utf-8-sig"))
            data = data.get("campaigns", []) if isinstance(data, dict) else data
            if not isinstance(data, list) or len(data) > 100:
                raise ValueError("invalid campaigns")
            return data
        except (OSError, ValueError) as exc:
            raise StudioError("campaigns.json is not a valid campaign list", 500) from exc

    def campaign(self, campaign_id):
        if not isinstance(campaign_id, str) or len(campaign_id) > 100:
            raise StudioError("A campaignId is required")
        for campaign in self.campaigns():
            if isinstance(campaign, dict) and campaign.get("id") == campaign_id:
                return campaign
        raise StudioError("Unknown campaignId", 404)

    def provider_status(self):
        configured = all(os.environ.get(k, "").strip() for k in ("PROMO_LLM_BASE_URL", "PROMO_LLM_API_KEY", "PROMO_LLM_MODEL"))
        voices = system_voices()
        return {"mode": "provider" if configured else "template", "configured": configured, "systemTtsAvailable": bool(voices), "systemVoices": voices, "notice": "Text generation uses only campaign-approved wording; system narration uses installed Windows speech voices."}

    def status(self):
        with self.lock:
            assets = [{k: v for k, v in a.items() if k not in ("sourcePath", "sha256")} for a in self.assets.values()]
            alive = bool(self.worker and self.worker.is_alive())
            worker_state = "degraded" if self.storage_error else "rendering" if alive and any(j["status"] == "running" for j in self.jobs.values()) else "ready" if alive else "stopped" if self.worker else "not_started"
            health = {"worker": worker_state, "workerAlive": alive, "storage": "error" if self.storage_error else "ok"}
            if self.storage_error:
                health["error"] = self.storage_error
            snapshot = copy.deepcopy({"assets": assets, "jobs": list(self.jobs.values()), "health": health})
        campaigns = self.campaigns()
        for c in campaigns:
            if isinstance(c, dict):
                c["languages"] = ["zh-TW"] + [language for language in c.get("locales", {}) if language in ("en",) ]
        snapshot.update(campaigns=campaigns, provider=self.provider_status(), tooling={"ffmpegAvailable": bool(self.ffmpeg), "ffprobeAvailable": bool(self.ffprobe)})
        return snapshot

    def _run(self, args, log_path: Path, timeout=300, cwd=None):
        if not self.ffmpeg:
            raise StudioError("FFmpeg is unavailable; install it or set PROMO_STUDIO_FFMPEG", 503)
        try:
            with log_path.open("ab") as log:
                result = subprocess.run([self.ffmpeg, "-hide_banner", "-v", "error", "-nostdin", "-y", *args], stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=log, timeout=timeout, cwd=cwd, check=False, creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0)
            if log_path.stat().st_size > 2 * 1024 * 1024:
                with log_path.open("rb") as bounded:
                    bounded.seek(-2 * 1024 * 1024, os.SEEK_END)
                    tail = bounded.read(2 * 1024 * 1024)
                log_path.write_bytes(b"[Earlier FFmpeg diagnostics truncated to bound local log size]\n" + tail)
            if result.returncode:
                with log_path.open("rb") as bounded:
                    bounded.seek(max(0, log_path.stat().st_size - 3500))
                    tail = bounded.read(3500).decode("utf-8", errors="replace")
                raise StudioError("FFmpeg failed: " + tail.strip(), 500)
        except subprocess.TimeoutExpired as exc:
            raise StudioError("FFmpeg render exceeded the bounded time limit", 500) from exc
        except OSError as exc:
            raise StudioError("FFmpeg could not be started", 503) from exc

    def import_asset(self, payload, original_filename=None):
        if not isinstance(payload, dict):
            raise StudioError("Import body must be an object")
        source = local_regular_file(payload.get("path"))
        if original_filename is not None:
            original_filename = clean_text(original_filename, 240, "filename")
            if Path(original_filename).name != original_filename or any(c in original_filename for c in ("/", "\\", ":")):
                raise StudioError("The original upload filename must be a simple filename")
        extension = source.suffix.lower()
        if extension not in IMAGE_EXTENSIONS | VIDEO_EXTENSIONS:
            raise StudioError("Supported imports: PNG/JPEG/WebP/BMP/GIF and MP4/MOV/MKV/WebM/AVI/M4V")
        label = clean_text(payload.get("label") or source.stem, 100, "label")
        provenance = payload.get("provenance")
        if not isinstance(provenance, dict) or provenance.get("kind") not in PROVENANCE_KINDS:
            raise StudioError("Supply provenance.kind: ai_creative, gameplay or game_screenshot")
        origin = clean_text(provenance.get("source"), 600, "provenance.source")
        tags = payload.get("tags", [])
        if not isinstance(tags, list) or len(tags) > 20 or any(not isinstance(t, str) or not TAG_RE.fullmatch(t) for t in tags):
            raise StudioError("tags must contain at most 20 lowercase topic identifiers")
        tags = sorted(set(tags))
        kind = "image" if extension in IMAGE_EXTENSIONS else "video"
        if provenance["kind"] == "gameplay" and kind != "video":
            raise StudioError("Gameplay provenance requires a real video; import screenshots as game_screenshot")
        if provenance["kind"] == "game_screenshot" and kind != "image":
            raise StudioError("game_screenshot provenance requires an image")
        if not self.ffmpeg or not self.ffprobe:
            raise StudioError("FFmpeg and ffprobe are required before importing media", 503)
        with self.lock:
            self._ensure_storage_healthy()
            if len(self.assets) >= 500:
                raise StudioError("This studio is limited to 500 imported assets", 409)
        # Reject playlists/disguised media before copying or invoking a decoder.
        media_input_options(source)
        asset_id = uuid.uuid4().hex
        directory = self.data_dir / "assets" / asset_id
        directory.mkdir()
        try:
            copied = directory / ("source" + extension)
            before = source.stat()
            digest = hashlib.sha256()
            with source.open("rb") as src, copied.open("xb") as dst:
                copied_bytes = 0
                while block := src.read(1024 * 1024):
                    copied_bytes += len(block)
                    if copied_bytes > MAX_ASSET_BYTES:
                        raise StudioError("The file grew beyond the 2 GB import limit")
                    digest.update(block)
                    dst.write(block)
            after = source.stat()
            if before.st_size != after.st_size or before.st_mtime_ns != after.st_mtime_ns:
                raise StudioError("Source changed during import; retry with a finished recording")
            info = probe(copied, self.ffprobe)
            container = info["formatName"]
            if kind == "video" and not any(token in container for token in {".mp4": ("mov", "mp4"), ".mov": ("mov",), ".m4v": ("mov", "mp4"), ".mkv": ("matroska",), ".webm": ("webm",), ".avi": ("avi",)}[extension]):
                raise StudioError("File bytes do not match the selected video container")
            if kind == "video" and info["durationSeconds"] <= 0:
                raise StudioError("Video imports need a positive readable duration")
            self._run([*media_input_options(copied), "-i", str(copied), "-vf", "scale=480:320:force_original_aspect_ratio=decrease", "-frames:v", "1", "-q:v", "3", str(directory / "thumbnail.jpg")], directory / "import.log", timeout=45)
            asset = {"id": asset_id, "label": label, "kind": kind, "tags": tags, "path": str(copied), "sourcePath": str(source), "sourceFileName": original_filename or source.name, "provenance": {"kind": provenance["kind"], "source": origin}, "thumbnailUrl": f"/media/{asset_id}/thumbnail.jpg", "mediaUrl": f"/media/{asset_id}/{copied.name}", "sha256": digest.hexdigest(), "createdAt": int(time.time() * 1000), **info}
            with self.lock:
                self._ensure_storage_healthy()
                if len(self.assets) >= 500:
                    raise StudioError("This studio is limited to 500 imported assets", 409)
                self.assets[asset_id] = asset
                try:
                    self._save_locked()
                except BaseException:
                    self.assets.pop(asset_id, None)
                    raise
            return {k: v for k, v in asset.items() if k not in ("sourcePath", "sha256")}
        except BaseException:
            try:
                self._cleanup_failed_import(directory, asset_id)
            except OSError as exc:
                with self.lock:
                    self.storage_error = "Owned failed-import cleanup could not finish. Repair storage and restart the studio."
                raise PersistenceError() from exc
            raise

    def _cleanup_failed_import(self, directory: Path, asset_id: str):
        assets_root = self.data_dir / "assets"
        with self.lock:
            if asset_id in self.assets:
                raise OSError("Refusing cleanup of a registered asset")
        if not ID_RE.fullmatch(asset_id) or directory.name != asset_id or directory.is_symlink() or directory.parent.resolve() != assets_root.resolve() or directory.resolve().parent != assets_root.resolve() or not assets_root.resolve().is_relative_to(self.data_dir):
            raise OSError("Refusing cleanup outside this request's managed asset directory")
        # Import creates only direct files. Never recurse into unexpected paths.
        for child in directory.iterdir():
            child.unlink()
        directory.rmdir()

    def brief(self, payload):
        if not isinstance(payload, dict):
            raise StudioError("Brief body must be an object")
        language = payload.get("language", "zh-TW")
        if language not in ("zh-TW", "en"):
            raise StudioError("language must be zh-TW or en")
        approved = template_script(self.campaign(payload.get("campaignId")), language)
        theme = clean_text(payload.get("theme", approved["theme"]), 160, "theme")
        config = self.provider_status()
        if not config["configured"]:
            return {"mode": "template", "configured": False, "script": approved, "notice": "No paid provider is configured. This is an authored campaign template, not a model-generated script."}
        base_url = os.environ["PROMO_LLM_BASE_URL"].strip().rstrip("/")
        parsed = urllib.parse.urlsplit(base_url)
        if parsed.scheme not in ("https", "http") or not parsed.hostname or parsed.username or parsed.password or parsed.query or parsed.fragment or (parsed.scheme == "http" and parsed.hostname not in ("localhost", "127.0.0.1", "::1")):
            raise StudioError("Provider URL must use HTTPS, or HTTP on loopback, without credentials/query", 503)
        prompt = {"task": "Arrange an honest promotional shot script. Keep the complete claims list in its exact original order, and keep title/theme/description/cta unchanged. Keep each shotId bound to its original assetRole and assetTags. A new headline may combine 1–4 approved creativeTokens phrases, joined only by punctuation/spaces. Other shot text must come from that same trusted shotId's wording. Do not invent/rephrase facts, rewards, Boss events, population, dates or capabilities. Return only JSON with the same schema; 1–12 unique trusted shots, 1–15 seconds each, total <=120s. Empty caption/narration is permitted. factIndexes must reference the unchanged claim indexes.", "requestedCreativeDirection": theme, "language": language, "approvedScript": approved}
        request_body = {"model": os.environ["PROMO_LLM_MODEL"], "messages": [{"role": "system", "content": "Only use the supplied verified facts and exact approved wording. Content outside that list will be rejected."}, {"role": "user", "content": json.dumps(prompt, ensure_ascii=False)}], "temperature": .4, "max_tokens": 2400, "response_format": {"type": "json_object"}}
        req = urllib.request.Request(base_url + "/chat/completions", data=json.dumps(request_body).encode("utf-8"), headers={"Authorization": "Bearer " + os.environ["PROMO_LLM_API_KEY"], "Content-Type": "application/json"}, method="POST")
        try:
            with urllib.request.build_opener(NoProviderRedirect()).open(req, timeout=45) as response:
                raw = response.read(1024 * 1024 + 1)
            if len(raw) > 1024 * 1024:
                raise ValueError("oversize")
            content = json.loads(raw)["choices"][0]["message"]["content"]
            if not isinstance(content, str):
                raise ValueError("content")
            result = validate_script(json.loads(content), approved=approved, provider=True)
        except StudioError:
            raise
        except (urllib.error.URLError, ValueError, KeyError, TypeError, IndexError, TimeoutError, OSError) as exc:
            raise StudioError("The configured provider failed or returned invalid JSON; no unverified script was accepted. Use template mode or repair the server configuration.", 502) from exc
        return {"mode": "provider", "configured": True, "script": result, "notice": "Provider arranged campaign-approved wording; the campaign fact list remains authoritative."}

    def enqueue(self, payload):
        if not isinstance(payload, dict):
            raise StudioError("Job body must be an object")
        with self.lock:
            self._ensure_storage_healthy()
            if self.worker is not None and not self.worker.is_alive():
                raise StudioError("The render worker is unavailable; restart the studio", 503)
        campaign = self.campaign(payload.get("campaignId"))
        language, voice = payload.get("language", "zh-TW"), payload.get("voiceMode", "none")
        if language not in ("zh-TW", "en") or voice not in ("none", "system"):
            raise StudioError("Use language zh-TW/en and voiceMode none/system")
        if voice == "system" and os.name != "nt":
            raise StudioError("System narration requires Windows installed speech voices")
        formats = payload.get("formats", ["vertical"])
        if not isinstance(formats, list) or not 1 <= len(formats) <= 2 or any(f not in FORMATS for f in formats) or len(set(formats)) != len(formats):
            raise StudioError("formats must contain vertical and/or landscape, without duplicates")
        selected = payload.get("assetIds")
        if not isinstance(selected, list) or not 1 <= len(selected) <= 12 or any(not isinstance(a, str) or not ID_RE.fullmatch(a) for a in selected) or len(set(selected)) != len(selected):
            raise StudioError("Select 1–12 distinct imported assetIds")
        approved = template_script(campaign, language)
        script = validate_script(payload["script"], approved=approved) if payload.get("script") is not None else approved
        if voice == "system" and not any(s["narration"] for s in script["shots"]):
            raise StudioError("This script has no narration. Choose voiceMode none, or supply approved narration text.")
        if not self.ffmpeg or not self.ffprobe:
            raise StudioError("FFmpeg and ffprobe are required to render", 503)
        with self.lock:
            self._ensure_storage_healthy()
            if any(a not in self.assets for a in selected):
                raise StudioError("An assetId is not imported in this studio")
            selected_assets = [self.assets[a] for a in selected]
            required_tags = campaign.get("requiredTags", [])
            if not isinstance(required_tags, list) or any(not isinstance(t, str) or not TAG_RE.fullmatch(t) for t in required_tags):
                raise StudioError("Campaign requiredTags is invalid", 500)
            present_tags = {t for asset in selected_assets for t in asset.get("tags", [])}
            missing_tags = sorted(set(required_tags) - present_tags)
            if missing_tags:
                raise StudioError("Import and select campaign material tagged: " + ", ".join(missing_tags))
            choose_shot_assets(script["shots"], selected_assets)
            if sum(j["status"] in ("queued", "running") for j in self.jobs.values()) >= 20 or len(self.jobs) >= 1000:
                raise StudioError("The render queue is full", 409)
            job = {"id": uuid.uuid4().hex, "campaignId": campaign["id"], "status": "queued", "progress": 0, "outputs": [], "language": language, "voiceMode": voice, "formats": formats, "assetIds": selected, "script": script, "createdAt": int(time.time() * 1000)}
            self.jobs[job["id"]] = job
            try:
                self._save_locked()
            except BaseException:
                self.jobs.pop(job["id"], None)
                raise
            self.queue.put(job["id"])
            return copy.deepcopy(job)

    def start_worker(self):
        with self.lock:
            if self.worker:
                return
            self._ensure_storage_healthy()
            self._acquire_lease()
            # Jobs submitted before the worker starts are already in the queue.
            queued = set(self.queue.queue)
            for job in self.jobs.values():
                if job["status"] == "queued" and job["id"] not in queued:
                    self.queue.put(job["id"])
            self.worker = threading.Thread(target=self._work, name="promo-ffmpeg-worker", daemon=True)
            self.worker.start()

    def close(self):
        self.stopping.set()
        self.queue.put(None)
        if self.worker:
            self.worker.join(timeout=2)
        if not self.worker or not self.worker.is_alive():
            self._release_lease()

    def _update(self, job_id, **changes):
        with self.lock:
            previous = copy.deepcopy(self.jobs[job_id])
            self.jobs[job_id] = {**previous, **changes}
            try:
                self._save_locked()
            except BaseException:
                self.jobs[job_id] = previous
                raise

    def _record_storage_failure(self, job_id):
        with self.lock:
            self.storage_error = self.storage_error or "Persistent state could not be committed. Repair storage and restart the studio."
            job = self.jobs[job_id]
            job.update(status="error", progress=min(job.get("progress", 0), 99), outputs=[], error="Render state was not committed because storage failed. Repair storage and restart the studio; no completed result is claimed.", completedAt=int(time.time() * 1000))
            for pending in self.jobs.values():
                if pending["status"] == "queued":
                    pending["blockedReason"] = "Storage failed; this persisted queued job awaits repair and studio restart."

    def _work(self):
        while not self.stopping.is_set():
            if self.storage_error is not None:
                self.stopping.wait(.25)
                continue
            job_id = self.queue.get()
            if job_id is None:
                self.queue.task_done()
                return
            try:
                with self.lock:
                    if self.jobs[job_id]["status"] != "queued":
                        continue
                    job = copy.deepcopy(self.jobs[job_id])
                    assets = [copy.deepcopy(self.assets[a]) for a in job["assetIds"]]
                approved = template_script(self.campaign(job["campaignId"]), job["language"])
                job["script"] = validate_script(job["script"], approved=approved)
                self._update(job_id, status="running", progress=2, startedAt=int(time.time() * 1000))
                outputs = self.render(job, assets, lambda n: self._update(job_id, progress=n))
                self._update(job_id, status="completed", progress=100, outputs=outputs, completedAt=int(time.time() * 1000))
            except Exception as exc:
                # Bound user-facing diagnostics and never expose provider secrets.
                message = str(exc)[:4000] if isinstance(exc, StudioError) else f"Local render failed ({type(exc).__name__}); see the job render.log on disk."
                if isinstance(exc, PersistenceError):
                    self._record_storage_failure(job_id)
                else:
                    try:
                        self._update(job_id, status="error", error=message, completedAt=int(time.time() * 1000))
                    except Exception:
                        # A second failed state write must not kill the worker or
                        # silently leave later jobs looking runnable forever.
                        self._record_storage_failure(job_id)
            finally:
                self.queue.task_done()

    def _font(self):
        candidates = [os.environ.get("PROMO_STUDIO_FONT", ""), "C:/Windows/Fonts/msjh.ttc", "C:/Windows/Fonts/simhei.ttf", "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc", "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"]
        source = next((Path(p) for p in candidates if p and Path(p).is_file()), None)
        if not source:
            raise StudioError("A Unicode font is required; set PROMO_STUDIO_FONT")
        directory = self.data_dir / "fonts"
        directory.mkdir(exist_ok=True)
        target = directory / ("unicode" + source.suffix)
        if not target.exists():
            shutil.copyfile(source, target)
        return target

    def _tts(self, text: str, language: str, directory: Path, index: int):
        text_path, wav_path = directory / f"narration-{index}.txt", directory / f"narration-{index}.wav"
        text_path.write_text(text, encoding="utf-8")
        code = "[Console]::OutputEncoding = New-Object Text.UTF8Encoding($false); Add-Type -AssemblyName System.Speech; $promoSpeaker = New-Object System.Speech.Synthesis.SpeechSynthesizer; $promoVoices = @($promoSpeaker.GetInstalledVoices() | Where-Object { $_.Enabled -and $_.VoiceInfo.Culture.Name.StartsWith($env:PROMO_TTS_LANGUAGE) }); if ($promoVoices.Count -eq 0) { throw 'No matching installed speech voice' }; $promoSpeaker.SelectVoice($promoVoices[0].VoiceInfo.Name); $promoSpeaker.Rate = 0; $promoSpeaker.SetOutputToWaveFile($env:PROMO_TTS_OUTPUT); $promoSpeaker.Speak([IO.File]::ReadAllText($env:PROMO_TTS_TEXT, [Text.Encoding]::UTF8)); $promoSpeaker.Dispose(); Write-Output $promoVoices[0].VoiceInfo.Name"
        env = dict(os.environ, PROMO_TTS_TEXT=str(text_path), PROMO_TTS_OUTPUT=str(wav_path), PROMO_TTS_LANGUAGE="zh" if language == "zh-TW" else "en")
        try:
            result = subprocess.run(["powershell.exe", "-NoProfile", "-NonInteractive", "-Command", code], capture_output=True, env=env, timeout=60, check=False, creationflags=subprocess.CREATE_NO_WINDOW)
        except (OSError, subprocess.TimeoutExpired) as exc:
            raise StudioError("Windows system narration failed; select no narration or install a matching system voice") from exc
        if result.returncode or not wav_path.is_file():
            raise StudioError("No usable installed system voice for this language; select no narration or install one")
        try:
            import wave
            with wave.open(str(wav_path), "rb") as audio:
                duration = audio.getnframes() / audio.getframerate()
        except (ValueError, EOFError, wave.Error) as exc:
            raise StudioError("System narration did not produce a valid WAV") from exc
        return wav_path, duration, result.stdout.decode("utf-8", errors="replace").strip()[:200]

    def render(self, job: dict, assets: list[dict], progress=lambda n: None):
        directory = self.data_dir / "jobs" / job["id"]
        directory.mkdir(exist_ok=True)
        log = directory / "render.log"
        self._font()
        script = copy.deepcopy(job["script"])
        narrated, voices = {}, set()
        if job["voiceMode"] == "system":
            for i, shot in enumerate(script["shots"]):
                if shot["narration"]:
                    wav, duration, voice_name = self._tts(shot["narration"], job["language"], directory, i)
                    shot["seconds"] = max(shot["seconds"], round(duration + .4, 2))
                    if shot["seconds"] > 15:
                        raise StudioError("System narration exceeds 15 seconds in one shot; shorten the campaign narration")
                    narrated[i], voices = wav, voices | {voice_name}
            if sum(s["seconds"] for s in script["shots"]) > 120:
                raise StudioError("System narration extends this script beyond 120 seconds")
        selected = choose_shot_assets(script["shots"], assets)
        timings = source_timing(script["shots"], selected)
        outputs = []
        total_segments = len(job["formats"]) * len(script["shots"])
        completed = 0
        for video_format in job["formats"]:
            width, height = FORMATS[video_format]
            segments = directory / ("segments-" + video_format)
            segments.mkdir(exist_ok=True)
            vertical = height > width
            frame_width, frame_height = width - 48, 720 if vertical else 424
            frame_y = 274 if vertical else 155
            for i, (shot, asset) in enumerate(zip(script["shots"], selected)):
                source = self.media_path(asset["id"], Path(asset["path"]).name)
                if not source:
                    raise StudioError("An imported asset is missing from managed storage")
                ass_path = segments / f"scene-{i}.ass"
                ass_path.write_text(_ass(script, shot, asset, job["language"], width, height), encoding="utf-8")
                duration = shot["seconds"]
                source_args = ["-loop", "1", "-framerate", "30"] if asset["kind"] == "image" and source.suffix.lower() != ".gif" else ["-stream_loop", "-1"]
                if asset["kind"] == "video":
                    source_args += ["-ss", f"{timings[i]['inputOffsetSeconds']:.6f}"]
                audio_args = ["-protocol_whitelist", "file,pipe", "-format_whitelist", "wav", "-f", "wav", "-i", str(narrated[i])] if i in narrated else ["-f", "lavfi", "-i", "anullsrc=r=48000:cl=stereo"]
                full_creative = vertical and asset["kind"] == "image" and asset["provenance"]["kind"] == "ai_creative"
                if full_creative:
                    frames = max(2, math.ceil(duration * 30))
                    denominator = frames - 1
                    # Only creative stills may fill/crop/animate. The double-size
                    # input gives the small Ken Burns movement smooth sampling.
                    layout_filter = f"scale={width*2}:{height*2}:force_original_aspect_ratio=increase:flags=lanczos,crop={width*2}:{height*2},zoompan=z='min(1.03,1+0.03*on/{denominator})':x='(iw-iw/zoom)*(0.4+0.2*min(on/{denominator},1))':y='(ih-ih/zoom)/2':d={frames}:s={width}x{height}:fps=30,drawbox=x=0:y=0:w=iw:h=280:color=0x07141d@0.58:t=fill,drawbox=x=0:y=980:w=iw:h=300:color=0x07141d@0.72:t=fill"
                else:
                    layout_filter = f"scale={frame_width}:{frame_height}:force_original_aspect_ratio=decrease:flags=lanczos,pad={frame_width}:{frame_height}:(ow-iw)/2:(oh-ih)/2:color=0x0b202b,pad={width}:{height}:24:{frame_y}:color=0x07141d"
                video_filter = f"{layout_filter},setsar=1,subtitles=filename=scene-{i}.ass:fontsdir=../../../fonts,fade=t=in:st=0:d=0.18,fade=t=out:st={max(0,duration-.18):.3f}:d=0.18"
                args = [*source_args, *media_input_options(source), "-i", str(source), *audio_args, "-t", f"{duration:.3f}", "-map", "0:v:0", "-map", "1:a:0", "-filter_threads", "2", "-vf", video_filter, "-af", "apad,atrim=0:" + str(duration), "-r", "30", "-c:v", "libx264", "-preset", "veryfast", "-crf", "21", "-pix_fmt", "yuv420p", "-threads", "2", "-c:a", "aac", "-b:a", "128k", "-ar", "48000", "-ac", "2", "-movflags", "+faststart", f"scene-{i}.mp4"]
                self._run(args, log, timeout=max(120, int(duration * 30)), cwd=segments)
                completed += 1
                progress(round(5 + 80 * completed / total_segments))
            (segments / "concat.txt").write_text("\n".join(f"file 'scene-{i}.mp4'" for i in range(len(script["shots"]))) + "\n", encoding="ascii")
            video = directory / f"{video_format}.mp4"
            self._run(["-protocol_whitelist", "file,pipe", "-format_whitelist", "concat,mov,mp4,m4a,3gp,3g2,mj2", "-f", "concat", "-safe", "1", "-i", "concat.txt", "-c", "copy", "-movflags", "+faststart", str(video)], log, timeout=120, cwd=segments)
            info = probe(video, self.ffprobe)
            if (info["width"], info["height"]) != (width, height):
                raise StudioError("Rendered MP4 dimensions failed verification", 500)
            cover = directory / f"{video_format}-cover.jpg"
            self._run(["-ss", "0.5", *media_input_options(video), "-i", str(video), "-frames:v", "1", "-q:v", "2", str(cover)], log, timeout=45)
            cursor, srt_lines = 0.0, []
            for i, shot in enumerate(script["shots"]):
                end = cursor + shot["seconds"]
                subtitle = shot["narration"] or shot["caption"] or shot["headline"]
                srt_lines.append(f"{i+1}\n{timecode(cursor)} --> {timecode(end)}\n{subtitle}\n")
                cursor = end
            (directory / f"{video_format}.srt").write_text("\n".join(srt_lines), encoding="utf-8")
            provenance = []
            for i, asset in enumerate(selected):
                full_creative = vertical and asset["kind"] == "image" and asset["provenance"]["kind"] == "ai_creative"
                presentation = {"fit": "creative_full_frame" if full_creative else "complete_source_frame", "motion": "ken_burns_1.00_to_1.03" if full_creative else "source_video" if asset["kind"] == "video" else "still", "sourceCropApplied": full_creative, "gameplayCropApplied": False}
                provenance.append({"shot": i + 1, "shotId": script["shots"][i].get("shotId", str(i+1)), "assetTags": shot_tags(script["shots"][i]), "assetId": asset["id"], "label": asset["label"], "provenance": asset["provenance"], "sourceFileName": asset["sourceFileName"], "kind": asset["kind"], "tags": asset.get("tags", []), "sha256": asset["sha256"], "presentation": presentation, **timings[i]})
            disclosures = sorted({a["provenance"]["kind"] for a in selected})
            disclosure_text = "AI 創意畫面及截圖均在畫面標示；截圖不代表連續實機錄影。" if job["language"] == "zh-TW" else "AI creative visuals and still screenshots are labelled on screen; screenshots are not continuous gameplay footage."
            metadata = {"version": 1, "jobId": job["id"], "campaignId": job["campaignId"], "format": video_format, "width": width, "height": height, "fps": 30, "codec": "h264", "pixelFormat": "yuv420p", "durationSeconds": info["durationSeconds"], "language": job["language"], "voice": {"mode": job["voiceMode"], "engine": "Windows system TTS (SAPI/System.Speech), not neural AI" if narrated else "none", "installedVoices": sorted(voices)}, "originalAudioIncluded": False, "disclosures": disclosures, "script": script, "assetProvenance": provenance, "posting": {"title": script["title"], "description": script["description"] + "\n\n" + disclosure_text + "\n" + script["cta"], "hashtags": script["hashtags"], "verifiedClaims": script["claims"], "note": "Review this package before manual publication. This tool does not publish to any platform."}, "generatedAt": int(time.time() * 1000)}
            (directory / f"{video_format}-metadata.json").write_text(json.dumps(metadata, ensure_ascii=False, indent=2), encoding="utf-8")
            prefix = f"/media/{job['id']}/"
            outputs.append({"format": video_format, "videoUrl": prefix + video.name, "coverUrl": prefix + cover.name, "subtitleUrl": prefix + f"{video_format}.srt", "metadataUrl": prefix + f"{video_format}-metadata.json", "durationSeconds": info["durationSeconds"], "width": width, "height": height})
        progress(98)
        return outputs

    def media_path(self, object_id: str, filename: str) -> Path | None:
        if not ID_RE.fullmatch(object_id) or not filename or Path(filename).name != filename or any(c in filename for c in ("/", "\\", "\x00", ":")):
            return None
        with self.lock:
            if object_id in self.assets:
                asset = self.assets[object_id]
                if filename not in ("thumbnail.jpg", Path(asset["path"]).name):
                    return None
                target = self.data_dir / "assets" / object_id / filename
            elif object_id in self.jobs:
                permitted = {Path(url).name for out in self.jobs[object_id].get("outputs", []) for key, url in out.items() if key.endswith("Url")}
                if filename not in permitted:
                    return None
                target = self.data_dir / "jobs" / object_id / filename
            else:
                return None
        try:
            if target.is_symlink() or not target.is_file() or not target.resolve().is_relative_to(self.data_dir):
                return None
            return target
        except OSError:
            return None
