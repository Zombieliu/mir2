"""Recompute audit inventory and source hashes. Does not run the game or mutate stores."""
import csv, datetime, hashlib, io, json, pathlib, re, subprocess
HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parents[2]
def read_json(path): return json.loads(pathlib.Path(path).read_text(encoding="utf-8-sig"))
def digest(path):
    h=hashlib.sha256()
    with pathlib.Path(path).open("rb") as f:
        for chunk in iter(lambda:f.read(1024*1024),b""): h.update(chunk)
    return h.hexdigest()
def git(*args,cwd=ROOT):
    return subprocess.check_output(["git",*args],cwd=cwd).decode("utf-8",errors="replace").strip()
def write(name,obj):
    (HERE/name).write_text(json.dumps(obj,ensure_ascii=False,indent=2)+"\n",encoding="utf-8")
audit=read_json(HERE/"audit.json")
checks=[]
snap=[]
for src in audit["evidenceSources"]:
    p=pathlib.Path(src["path"])
    if not p.is_absolute(): p=ROOT/p
    assert p.is_file(),f"Missing source {src['id']}: {p}"
    lines=sum(1 for _ in p.open(encoding="utf-8-sig"))
    assert src["line"]<=lines,f"Invalid line {src['id']}: {src['line']} > {lines}"
    snap.append({**src,"absolutePath":p.as_posix(),"sha256":digest(p),"bytes":p.stat().st_size,"lineCount":lines})
checks.append("49 evidence references exist and line anchors are valid")
data=ROOT/"packages/game-data/data/generated"
names={
"maps":("crystal_respawn_manifest.json","maps"),
"monsters":("crystal_monster_manifest.json","monsters"),
"classes":("crystal_base_stats_packet_manifest.json","classes"),
"magics":("crystal_magic_manifest.json","magics"),
"buffs":("crystal_buff_manifest.json","buffs"),
"items":("crystal_item_manifest.json","items"),
"recipes":("crystal_recipe_packet_manifest.json","recipes"),
"gameShop":("crystal_game_shop_packet_manifest.json","items"),
"quests":("crystal_quest_packet_manifest.json","quests"),
"npcs":("crystal_npc_info_manifest.json","npcs"),
"npcScripts":("crystal_npc_manifest.json","scripts"),
"drops":("crystal_drop_manifest.json","tables")}
loaded={}
inventory={}
for name,(filename,key) in names.items():
    p=data/filename;d=read_json(p);loaded[name]=d
    inventory[name]={"path":p.relative_to(ROOT).as_posix(),"generatedAt":d.get("generated_at"),
        "source":d.get("source_file",d.get("source_dir",d.get("source_configs_dir"))),
        "sha256":digest(p),"array":key,"arrayCount":len(d[key])}
maps=loaded["maps"]["maps"]
monsters=loaded["monsters"]["monsters"]
inventory["maps"].update({
    "nonemptyMapNames":sum(bool(str(m["map_file_name"]).strip()) for m in maps),
    "movementRows":sum(len(m.get("movements",[])) for m in maps),
    "respawnRules":sum(len(m.get("respawns",[])) for m in maps),
    "configuredSpawnSlots":sum(r["count"] for m in maps for r in m.get("respawns",[])),
    "countingRule":"sum movements.length; sum respawns.length; sum respawn.count, not activated entities"})
inventory["monsters"].update({"distinctAiValues":len({m["ai"] for m in monsters}),"distinctImageValues":len({m["image"] for m in monsters})})
inventory["npcScripts"]["reportedLabels"]=loaded["npcScripts"]["total_labels"]
inventory["drops"]["reportedEntries"]=loaded["drops"]["total_entries"]
assert inventory["maps"]["nonemptyMapNames"]==463
assert inventory["maps"]["movementRows"]==1999
assert inventory["maps"]["respawnRules"]==6341
assert inventory["maps"]["configuredSpawnSlots"]==76181
assert (inventory["monsters"]["arrayCount"],inventory["monsters"]["distinctAiValues"],inventory["monsters"]["distinctImageValues"])==(555,116,296)
expected={"classes":5,"magics":109,"buffs":59,"items":1628,"recipes":79,"gameShop":105,"quests":154,"npcs":375,"npcScripts":634,"drops":1640}
for k,v in expected.items(): assert inventory[k]["arrayCount"]==v,(k,inventory[k])
checks.append("content array counts, respawn sums and distinct monster classifications recomputed")
fs=ROOT/"apps/web/public/original-ui/frame-sets.generated.json";fd=read_json(fs)
inventory["frameSets"]={"path":fs.relative_to(ROOT).as_posix(),"sha256":digest(fs),"libraryCount":fd["libraryCount"],"actionCount":fd["actionCount"]}
assert (fd["libraryCount"],fd["actionCount"])==(703,3643)
asset=pathlib.Path("C:/mir2-ground-label-20260908/mir2-assets")
atlas=read_json(asset/"bevy-entity-atlases/manifest.json")
inventory["installedEntityAtlas"]={"path":str(asset/"bevy-entity-atlases/manifest.json"),"sha256":digest(asset/"bevy-entity-atlases/manifest.json"),
"atlasCount":len(atlas["atlases"]),"stats":atlas["stats"],"monsterLibraries":[r for r in atlas["stats"]["roots"] if r.startswith("Monster/")]}
assert len(inventory["installedEntityAtlas"]["monsterLibraries"])==8
for key,rel in [("installedKeyedMap","generated/native-map-keyed/manifest.json"),("installedMapAtlas","generated/map-atlas/manifest.json")]:
    d=read_json(asset/rel)
    inventory[key]={"path":str(asset/rel),"sha256":digest(asset/rel),"stats":d["stats"]}
    if "mapFileNames" in d: inventory[key]["mapFileNames"]=d["mapFileNames"]
inventory["installedSound"]={"directory":str(asset/"original-ui/Sound"),"fileCount":sum(p.is_file() for p in (asset/"original-ui/Sound").rglob("*"))}
inventory["installedFullPackIndexExists"]=(asset/"generated/crystal-packs/full/index.json").is_file()
assert inventory["installedSound"]["fileCount"]==49
assert inventory["installedKeyedMap"]["stats"]["missingSourceCount"]==2969
checks.append("current installed asset package inventory read and boundary counts confirmed")
backlog=ROOT/"docs/generated/player-qa/windows-visual-parity/VIS-03-USER-OBSERVED-UI-RENDER-BACKLOG-20260829.md"
ids=re.findall(r"^\| (WN-[A-Z]+-\d+) \|",backlog.read_text(encoding="utf-8"),re.M)
assert len(ids)==len(set(ids))==33
inventory["nativeBacklog"]={"path":str(backlog),"count":len(ids),"ids":ids,"meaning":"retained backlog rows, not wholly unimplemented features"}
logs={}
for name,rel in [
("windowsEntry","docs/generated/player-qa/windows-reenter-world-20260908/tests.log"),
("gatewayOwnerAttack","docs/generated/player-qa/windows-owner-swing-20260908/mir2-owner-swing-full.log")]:
    p=ROOT/rel
    if p.is_file():
        summaries=[l for l in p.read_text(encoding="utf-8",errors="replace").splitlines() if "test result:" in l]
        logs[name]={"path":rel,"sha256":digest(p),"summaries":summaries,"rerunByThisAudit":False}
    else:
        logs[name]={"requestedPath":rel,"found":False,"rerunByThisAudit":False}
binary=pathlib.Path("C:/mir2-ground-label-20260908/mir2-platform-windows-entry-fix.exe")
binary_record={"path":str(binary),"sha256":digest(binary),"bytes":binary.stat().st_size}
assert binary_record["sha256"].upper()=="082D5C6229931BAFCFB84BC8E679EE9F3430F5754A1E255D801E98EBAD1D1B85"
checks.append("current development EXE SHA-256 matches entry-fix record")
csv_rows=list(csv.DictReader((HERE/"domains.csv").open(encoding="utf-8-sig",newline="")))
assert len(csv_rows)==len(audit["domains"])==36
assert len(audit["findings"])==12
assert audit["globalParityPercent"] is None and audit["inventoryComplete"] is False
report=ROOT/"docs/WHOLE-GAME-COMPLETENESS-AUDIT-20260908.zh-CN.md"
body=report.read_text(encoding="utf-8")
used=set(re.findall(r"\[(S\d+)\]",body))
assert used=={x["id"] for x in audit["evidenceSources"]}
checks.append("report reference IDs and CSV/JSON domain/finding totals agree; no global percentage claimed")
write("inventory.json",{"schemaVersion":"mir2.audit-content-inventory.v1","observedAtUtc":datetime.datetime.now(datetime.timezone.utc).isoformat(),"countsAreNotParity":True,"inventory":inventory})
write("source-snapshot.json",{"schemaVersion":"mir2.audit-source-snapshot.v1","observedAtUtc":datetime.datetime.now(datetime.timezone.utc).isoformat(),"projectRoot":ROOT.as_posix(),
"head":git("rev-parse","HEAD"),"branch":git("branch","--show-current"),"gitStatusAtCapture":git("status","--porcelain").splitlines(),
"crystalHead":git("rev-parse","HEAD",cwd=pathlib.Path("E:/mir2/Crystal")),
"crystalControlledSourceStatus":git("status","--porcelain","--","Client","Server","Shared",cwd=pathlib.Path("E:/mir2/Crystal")),
"evidence":snap,"binary":binary_record,"priorTestLogs":logs,
"reportSha256":digest(report),"auditSha256":digest(HERE/"audit.json")})
write("validation.json",{"schemaVersion":"mir2.audit-validation.v1","passed":True,"checks":checks,"gameAcceptance":False,"freshGameTestsRun":False})
print(json.dumps({"passed":True,"domainCount":len(csv_rows),"findingGroups":len(audit["findings"]),"evidenceReferences":len(snap),"backlogRows":len(ids),"checks":checks,"logs":logs},ensure_ascii=False,indent=2))
