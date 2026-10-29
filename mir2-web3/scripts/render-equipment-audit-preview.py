#!/usr/bin/env python3
"""Render technical contact sheets of existing source pixels for visual review.

No source art is edited, generated, recolored or installed into game assets.
Requires Pillow in the executing Python environment.
"""
import hashlib
import importlib.util
import json
from functools import lru_cache
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

spec = importlib.util.spec_from_file_location("equipment_audit", Path(__file__).with_name("audit-equipment-level100.py"))
audit = importlib.util.module_from_spec(spec)
spec.loader.exec_module(audit)
OUT = audit.PROJECT / "docs/generated/equipment-level100-20261019"
plan_path = OUT / "candidate-allocation.json"
plan = json.loads(plan_path.read_text(encoding="utf-8"))
inventory = json.loads((OUT / "inventory.json").read_text(encoding="utf-8"))
by_item = {row["itemIndex"]: row for row in inventory}
index = json.loads((audit.PUBLIC / "generated/crystal-packs/full/index.json").read_text(encoding="utf-8"))
assets = audit.Assets(index)
FONT_DIR = Path("C:/Windows/Fonts")
font_path = next((p for p in (FONT_DIR / "segoeui.ttf", FONT_DIR / "arial.ttf") if p.exists()), None)
font = lambda size: ImageFont.truetype(str(font_path), size) if font_path else ImageFont.load_default()
TITLE, LABEL, SMALL = font(25), font(17), font(13)
COLS = [40, 50, 60, 70, 80, 90, 100]
CLS = list(audit.CLASSES)
manifests = []
pixel_samples = []


@lru_cache(maxsize=4)
def page(url):
    path = audit.public_path(url)
    raw = path.read_bytes()
    if hashlib.sha256(raw).hexdigest() != path.stem:
        raise RuntimeError("Contact sheet source PNG hash mismatch")
    with Image.open(path) as image:
        image.load()
        return image.convert("RGBA")


def sprite(frame):
    if frame["status"] != "ok":
        raise RuntimeError(f"Preview requested invalid frame: {frame}")
    rect = frame["rect"]
    return page(frame["imageUrl"]).crop((rect["x"], rect["y"], rect["x"] + rect["width"], rect["y"] + rect["height"]))


def pixel_fingerprint(image):
    """Compare this static sample, not an entire animation/outfit."""
    bounds = image.getchannel("A").getbbox()
    image = image.crop(bounds) if bounds else Image.new("RGBA", (1, 1))
    raw = bytearray(image.tobytes())
    for offset in range(0, len(raw), 4):
        if raw[offset + 3] == 0:
            raw[offset:offset + 3] = b"\0\0\0"
    return hashlib.sha256(f"{image.width}x{image.height}:".encode() + raw).hexdigest()


def fit(canvas, image, box):
    x, y, w, h = box
    scale = min(w / image.width, h / image.height, 2)
    sized = image.resize((max(1, round(image.width * scale)), max(1, round(image.height * scale))), Image.Resampling.NEAREST)
    canvas.alpha_composite(sized, (x + (w - sized.width) // 2, y + (h - sized.height) // 2))


def ascii_label(text):
    return "".join(c if ord(c) < 128 else "?" for c in text)[:29]


def sheet(slot, gender):
    left, top, cw, ch = 120, 96, 210, 204
    canvas = Image.new("RGBA", (left + cw * len(COLS) + 24, top + ch * len(CLS) + 52), "#111c2b")
    draw = ImageDraw.Draw(canvas)
    draw.text((24, 15), f"1-100 EQUIPMENT RESOURCE AUDIT  /  {slot.upper()}  /  {gender.upper()}", font=TITLE, fill="#f0f4fa")
    draw.text((24, 50), "Original source pixels. Levels are proposed allocations; stats, drops and SLG unlock are not implemented.", font=SMALL, fill="#b9cadb")
    entries = []
    for col, level in enumerate(COLS):
        draw.text((left + col * cw + 70, 75), f"LV {level}", font=LABEL, fill="#8ed7b2")
    for row_num, cls in enumerate(CLS):
        draw.text((14, top + row_num * ch + 75), cls, font=LABEL, fill="#e8eef6")
        for col, level in enumerate(COLS):
            candidate = next(x for x in plan if x["class"] == cls and x["gender"] == gender
                             and x["targetLevel"] == level and x["slot"] == slot)
            x, y = left + col * cw, top + row_num * ch
            draw.rounded_rectangle((x + 4, y + 2, x + cw - 8, y + ch - 8), radius=8, fill="#1c2c40", outline="#34485f")
            if "sourceItemIndex" not in candidate:
                draw.text((x + 14, y + 75), "NO CANDIDATE", font=LABEL, fill="#f7a0a0")
                continue
            source = by_item[candidate["sourceItemIndex"]]
            binding = next(b for b in source["bindings"] if b["class"] == cls and b["gender"] == gender)
            if slot == "weapon":
                selected_frame = binding["paperdoll"]
                sample_weapon_shape = None
            else:
                weapon = next(x for x in plan if x["class"] == cls and x["gender"] == gender
                              and x["targetLevel"] == level and x["slot"] == "weapon")
                sample_weapon_shape = weapon["sourceShape"]
                # Real standing library for the class + same-tier weapon.
                selected_frame = assets.frame(*audit.armour_standing_sample(
                    source["shape"], cls, gender == "Female", sample_weapon_shape))
            source_sprite = sprite(selected_frame)
            fit(canvas, source_sprite, (x + 28, y + 12, 145, 118))
            draw.text((x + 13, y + 133), ascii_label(source["name"]), font=SMALL, fill="#e1ebf5")
            draw.text((x + 13, y + 153), f"Source #{source['itemIndex']} / req {source['requiredAmount']}", font=SMALL, fill="#a8bdd1")
            reused = candidate["status"].startswith("reuse_")
            draw.text((x + 13, y + 173), "WORLD LOOK REUSED" if reused else "SOURCE CANDIDATE", font=SMALL, fill="#f6cd86" if reused else "#8ed7b2")
            entries.append({"class": cls, "gender": gender, "targetLevel": level, "slot": slot,
                            "sourceItemIndex": source["itemIndex"], "sourceName": source["name"], "frame": selected_frame,
                            "sameTierWeaponShapeForBodySample": sample_weapon_shape,
                            "samplePixelSha256": pixel_fingerprint(source_sprite)})
            pixel_samples.append(entries[-1])
    draw.text((24, canvas.height - 30), "Contact sheet checks appearance samples only; eight-direction/action coverage is recorded in motion-coverage.json.", font=SMALL, fill="#b9cadb")
    output = OUT / f"{slot}-{gender.lower()}-40-100.png"
    canvas.convert("RGB").save(output)
    manifests.append({"file": output.name, "sha256": hashlib.sha256(output.read_bytes()).hexdigest(), "entries": entries})
    print(output.name, canvas.size, len(entries))


for slot, gender in (("weapon", "Male"), ("armour", "Male"), ("armour", "Female")):
    sheet(slot, gender)
sample_groups = {}
for sample in pixel_samples:
    sample_groups.setdefault((sample["slot"], sample["gender"], sample["samplePixelSha256"]), []).append(sample)
(OUT / "preview-appearance-groups.json").write_text(json.dumps({
    "scope": "105 static source samples only; identical sample pixels do not prove identical full animation sequences, and different samples do not prove unique complete sets.",
    "samples": len(pixel_samples), "distinctSampleFingerprints": len(sample_groups),
    "duplicateGroups": [{"slot": key[0], "gender": key[1], "samplePixelSha256": key[2],
                         "members": [{k: x[k] for k in ("class", "gender", "targetLevel", "slot", "sourceItemIndex", "sourceName")} for x in group]}
                        for key, group in sample_groups.items() if len(group) > 1]}, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
(OUT / "preview-manifest.json").write_text(json.dumps({"sourceAllocationSha256": hashlib.sha256(plan_path.read_bytes()).hexdigest(),
                                                       "renderMethod": "Technical contact sheet of source sprites, nearest-neighbor scaling, no source changes.",
                                                       "sheets": manifests}, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
