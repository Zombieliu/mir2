"""Read-only source extraction into this external fixture directory."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil

ROOT = Path(__file__).resolve().parent
SOURCE = "6032ef8b3e27dd97bad0b20c8676ef9f185db64b"


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", type=Path, required=True)
    args = parser.parse_args()
    repo = args.repo.resolve(strict=True)
    respawn_file = repo / "apps/web/lib/generated/crystal_respawn_manifest.json"
    npc_file = repo / "packages/game-data/data/generated/crystal_npc_info_manifest.json"
    profile_file = repo / "packages/game-data/data/content_profiles/platinum_176.json"
    maps = json.loads(respawn_file.read_text(encoding="utf-8"))["maps"]
    by_index = {m["map_index"]: m for m in maps}
    wanted = {"0", "1", "0132", "D021"}
    selected = []
    for m in maps:
        if m["map_file_name"] not in wanted:
            continue
        selected.append({
            "map": m["map_file_name"], "title": m["map_title"],
            "index": m["map_index"], "safeZones": m["safe_zones"],
            "fire": m["fire"], "lightning": m["lightning"],
            "transfers": [{**e, "targetMap": by_index[e["map_index"]]["map_file_name"]}
                          for e in m["movements"] if e["map_index"] in by_index],
            "respawns": m["respawns"] if m["map_file_name"] == "D021" else [],
        })
        source = repo / "apps/web/lib/generated/crystal-map-pack" / (m["map_file_name"].lower() + ".map.gz")
        shutil.copyfile(source, ROOT / "maps" / source.name)
    npcs = json.loads(npc_file.read_text(encoding="utf-8"))["npcs"]
    metadata = {
        "schema": "mir2.dense-fixture-original-world-inputs.v1", "sourceRevision": SOURCE,
        "sourceFiles": [{"relativePath": str(p.relative_to(repo)).replace("\\", "/"), "sha256": digest(p)}
                        for p in (respawn_file, npc_file, profile_file)],
        "maps": selected, "legalDenseRoute": ["0", "1", "D021"],
        "legalBookRoute": ["0", "0132", "0"],
        "npcs": [{"map": n["map_file_name"], "name": n["name"], **n["location"]}
                 for n in npcs if n["name"] in {"Merchant_Ruben", "Librarian_Brian"}],
        "acceptedHostileFamily": {"name": "CaveBat", "ai": 0, "level": 20, "hp": 25,
                                  "minDC": 4, "maxDC": 6, "moveMs": 1200, "attackMs": 2500,
                                  "viewRange": 7, "AC": 2, "MAC": 0, "accuracy": 11},
        "canPinMonsterRing": False, "unsafeQaRequired": False,
    }
    (ROOT / "WORLD-INPUTS.json").write_text(json.dumps(metadata, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"readOnlySourceExtraction": True, "mapCount": len(selected), "output": str(ROOT)}))


if __name__ == "__main__":
    main()
