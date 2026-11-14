import datetime
import hashlib
import json
from pathlib import Path

PHASE = Path('C:/mir2-playtest-releases/20261004-native-r18/dense-network-r18-execution-01')
OUT = Path(__file__).resolve().parent
SOURCE = '321316b791120a6fe549e4006623e63333a5543f'
FINAL_SHA = '86bedc48020ddf1f93320ad036cb343c7d83941e6eb0c359e5fdaacd41019e1a'

def sha(data):
    return hashlib.sha256(data).hexdigest()

def read_json(relative):
    return json.loads((PHASE / relative).read_bytes())

def rows(relative):
    return [json.loads(line) for line in (PHASE / relative).read_text(encoding='utf-8').splitlines() if line.strip()]

def owner(packet, object_id):
    return next(e for e in packet['payload']['entities'] if e['objectId'] == object_id and e['kind'] == 'selfPlayer')

def items(packet):
    return sorted((kind, str(i['uniqueId']), i['key'], i['quantity'])
                  for kind in ('inventoryItems', 'beltItems') for i in packet['payload'][kind])

inventory_bytes = (PHASE / 'FINAL-PUBLIC-INVENTORY-01.json').read_bytes()
assert sha(inventory_bytes) == FINAL_SHA
inventory = json.loads(inventory_bytes)
assert inventory['sourceRevision'] == SOURCE
assert len(inventory['files']) == 93
for entry in inventory['files']:
    path = (PHASE / entry['relativePath']).resolve()
    assert path.is_relative_to(PHASE.resolve()) and path.is_file() and not path.is_symlink()
    data = path.read_bytes()
    assert len(data) == entry['bytes'] and sha(data) == entry['sha256'], entry['relativePath']

report = read_json('downloaded-public/public-receipts/death-receipts/report.json')
assert report['sourceRevision'] == SOURCE and report['unsafeQaEnabled'] is False
reviewed = []
for case in report['classes']:
    name = case['className']
    if name == 'Warrior':
        assert case['death']['status'] == 'inconclusive' and case['freshLoad']['status'] == 'not-applicable'
        continue
    trace = rows(f'downloaded-public/public-receipts/death-receipts/{name}.jsonl')
    by_seq = {r['sequence']: r for r in trace}
    assert len(by_seq) == len(trace)
    proof = case['death']['deadActions']
    cast = proof['castBarrier']
    baseline = by_seq[cast['baselineSnapshotSequence']]
    magic = by_seq[cast['actualMagicInputSequence']]
    owner_id = proof['window']['ownerId']
    connection = baseline['connectionId']
    assert baseline['type'] == 'worldSnapshot' and owner(baseline, owner_id)['dead'] is True
    assert baseline['payload']['playerHp'] == 0 and baseline['payload']['playerMp'] > 0
    learned = next(s for s in baseline['payload']['knownSkills'] if s['spell'] == magic['spell'])
    assert learned['mpCost'] == 3 and learned['cooldownRemainingMs'] == 0
    assert magic['type'] == 'magic' and magic['direction'] == 'sent'
    assert magic['connectionId'] == connection and magic['objectId'] == owner_id
    assert magic['spell'] == ('FireBall' if name == 'Wizard' else 'Healing')
    assert baseline['sequence'] < magic['sequence'] and magic['tsMs'] <= proof['window']['endMs']
    target = next(e for e in baseline['payload']['entities'] if e['objectId'] == magic['targetId'])
    assert (target['x'], target['y']) == (magic['x'], magic['y'])
    if name == 'Wizard':
        assert target['kind'] == 'monster' and target['disposition'] == 'hostile' and target['dead'] is False
    else:
        assert target['objectId'] == owner_id
    death = by_seq[case['death']['death']['deathSequence']]
    assert death['packet'] == 'Death' and death['connectionId'] == connection
    assert death['state']['hp'] == 0 and death['state']['dead'] is True
    closed = [r for r in trace if r['connectionId'] == connection and magic['sequence'] < r['sequence']
              and r['tsMs'] <= proof['window']['endMs']]
    absolute = [r for r in closed if r['type'] == 'worldSnapshot']
    assert len(absolute) == (11 if name == 'Wizard' else 12)
    assert [r['sequence'] for r in absolute] == cast['absoluteMpObservationSequences']
    base_life = owner(baseline, owner_id)
    base_items = items(baseline)
    for r in absolute:
        assert r['payload']['playerObjectId'] == owner_id
        assert r['payload']['playerMp'] == baseline['payload']['playerMp']
        assert r['payload']['playerHp'] == 0 and owner(r, owner_id)['dead'] is True
        assert owner(r, owner_id)['direction'] == base_life['direction']
        assert (owner(r, owner_id)['x'], owner(r, owner_id)['y']) == (base_life['x'], base_life['y'])
        assert items(r) == base_items
    assert not any(r.get('packet') in ('Magic', 'ObjectMagic') for r in closed)
    logout = case['freshLoad']['normalLogout']
    assert by_seq[logout['sentSequence']]['type'] == 'logOut'
    assert by_seq[logout['ackSequence']]['packet'] == 'LogOutSuccess'
    assert proof['window']['endMs'] < by_seq[logout['sentSequence']]['tsMs']
    assert logout['normalCloseConfirmed'] is True and logout['closedMs'] >= logout['ackMs']
    fresh_trace = rows(f'downloaded-public/public-receipts/death-receipts/{name}-fresh-load.jsonl')
    fresh = case['freshLoad']['fresh']
    assert fresh['connectionId'] != connection
    fresh_packet = next(r for r in fresh_trace if r['sequence'] == fresh['receiptSequence'])
    assert fresh_packet['type'] == 'worldSnapshot' and fresh_packet['connectionId'] == fresh['connectionId']
    assert owner(fresh_packet, owner_id)['dead'] is False
    assert fresh_packet['payload']['playerHp'] == fresh_packet['payload']['playerMaxHp'] == fresh['hp']
    assert fresh_packet['payload']['playerMp'] == fresh_packet['payload']['playerMaxMp'] == fresh['mp']
    assert fresh_packet['payload']['mapFileName'] == '0'
    assert (owner(fresh_packet, owner_id)['x'], owner(fresh_packet, owner_id)['y']) == (288, 616)
    assert case['freshQuiet']['status'] == case['alivePersistence']['status'] == 'passed'
    assert case['freshQuiet']['drift']['durationMs'] >= 5000
    reviewed.append({'class': name, 'naturalDeathSequence': death['sequence'], 'baselineSequence': baseline['sequence'],
                     'actualMagicSequence': magic['sequence'], 'spell': magic['spell'], 'mpCost': learned['mpCost'],
                     'absoluteMp': baseline['payload']['playerMp'], 'absoluteObservations': len(absolute),
                     'closedBeforeLogout': True, 'logoutAckSequence': logout['ackSequence'],
                     'freshConnectionDifferent': True, 'freshHp': fresh['hp'], 'freshMp': fresh['mp'],
                     'quietDurationMs': case['freshQuiet']['drift']['durationMs'],
                     'directDurableHp0Read': False})

live = read_json('LIVE-COMPARISON-01.json')
assert all(live[k] is True for k in ('realmsUnchanged', 'caddyUnchanged', 'configHashesUnchanged', 'feedUnchanged'))
assert live['beforeSixCounters'] == live['afterSixCounters'] and live['beforeTcpCounts'] == live['afterTcpCounts']
assert all(inventory[k] is False for k in ('fullP1Accepted', 'humanAccepted', 'nativeInputAccepted', 'nativeVisualAccepted',
                                         'ordinaryProgressionAccepted', 'loadAccepted', 'capacityAccepted', 'sevenMonsterMechanicsAccepted'))
result = {'schema': 'mir2.r18.root-independent-closed-window-review.v1',
          'createdUtc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
          'sourceRevision': SOURCE, 'finalPublicInventorySha256': FINAL_SHA, 'frozenPublicFilesVerified': 93,
          'classes': reviewed, 'liveComparisonVerified': True, 'newNetworkRun': False,
          'privateFilesRead': False, 'sevenMonsterAccepted': False, 'fullP1Accepted': False,
          'nativeOrHumanAccepted': False, 'reviewScope': 'raw eligible input, absolute life/MP/items, closed window, normal logout and fresh Load; existing quiet/persistence proofs read'}
(OUT / 'ROOT-RAW-REVIEW-01.json').write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
print(json.dumps({'frozenPublicFilesVerified': 93, 'classes': reviewed, 'newNetworkRun': False, 'fullP1Accepted': False}))
