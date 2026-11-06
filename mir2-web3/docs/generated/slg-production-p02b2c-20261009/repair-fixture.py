from pathlib import Path
import hashlib
import json
from datetime import datetime, timezone

work = Path(__file__).resolve().parent
repo = work.parent.parent
relative = 'mir2-web3/apps/simulation/src/runtime/tests.rs'
path = repo / relative
before = path.read_bytes()
before_sha = hashlib.sha256(before).hexdigest()
tested = json.loads((work / 'use-item-regression-01/source-snapshot.json').read_text())
row = next(item for item in tested['files'] if item['path'] == relative)
assert row['sha256'] == before_sha, 'Protected source drift before fixture repair'
archive = work / 'authored-bytes-01' / relative
assert not archive.exists(), 'Preserve prior bytes'
archive.parent.mkdir(parents=True, exist_ok=True)
archive.write_bytes(before)
text = before.decode('utf-8')
start = text.index('fn use_item_packet_dynamic_crystal_town_teleport_routes_through_template_scroll() {')
end = text.index('\n#[test]', start)
original = text[start:end]
marker = '    let packets = session.handle_packet(ClientPacket::UseItem {'
assert original.count(marker) == 1
line_end = '\r\n' if '\r\n' in original else '\n'
insertion = '''    // Pin this scroll-routing fixture to its configured destination. StartGame
    // can import a Crystal safe-zone bind; that selection is tested separately.
    let bind_map = session
        .app
        .world()
        .resource::<RuntimeConfigResource>()
        .config
        .map
        .file_name
        .clone();
    session.app.world_mut().resource_mut::<PlayerRuntimeResource>().bind_point =
        Some(crate::config::CharacterBindPoint {
            map_file_name: bind_map,
            position: spawn.clone(),
        });

'''.replace('\n', line_end)
repaired = original.replace(marker, insertion + marker)
assert repaired.replace(insertion, '') == original, 'Original fixture/assertions changed'
after = (text[:start] + repaired + text[end:]).encode('utf-8')
assert path.read_bytes() == before, 'Concurrent source change'
path.write_bytes(after)
receipt = {
    'schema': 'mir2.production-creature-box-fixture-repair.v1',
    'createdUtc': datetime.now(timezone.utc).isoformat(),
    'path': relative,
    'beforeSha256': before_sha,
    'afterSha256': hashlib.sha256(after).hexdigest(),
    'beforeLength': len(before),
    'afterLength': len(after),
    'originalFunctionRetainedAfterRemovingOnlyInsertion': True,
    'originalAssertionsUnchanged': True,
    'productionTeleportRuleChanged': False,
    'note': 'Existing scroll routing fixture pins the bind point it already asserted; actual Source01 46pass/1fail retained. Normal latest-safe-zone rule unchanged.'
}
(work / 'fixture-repair.json').write_text(json.dumps(receipt, indent=2) + '\n', encoding='utf-8')
source01 = json.loads((work / 'authored-source-01.json').read_text())
source02 = dict(source01)
source02['createdUtc'] = datetime.now(timezone.utc).isoformat()
source02['files'] = source01['files'] + [{
    'path': relative, 'sha256': receipt['afterSha256'], 'length': len(after)
}]
output = work / 'authored-source-02.json'
assert not output.exists()
output.write_text(json.dumps(source02, indent=2) + '\n', encoding='utf-8')
for item in source02['files']:
    data = (repo / item['path']).read_bytes()
    assert hashlib.sha256(data).hexdigest() == item['sha256']
    archived = work / 'authored-bytes-02' / item['path']
    archived.parent.mkdir(parents=True, exist_ok=True)
    archived.write_bytes(data)
runner = work / 'run-check.ps1'
prior = work / 'run-check-01.ps1'
assert not prior.exists()
prior.write_bytes(runner.read_bytes())
script = runner.read_text(encoding='utf-8')
assert "'independent-source-review-01.json'" in script
runner.write_text(script.replace("'independent-source-review-01.json'", "'independent-source-review-02.json'"), encoding='utf-8', newline='\n')
print(json.dumps({'authoredSource': str(output), 'sha256': hashlib.sha256(output.read_bytes()).hexdigest(), 'authoredFiles': len(source02['files']), 'originalAssertionsUnchanged': True}))
