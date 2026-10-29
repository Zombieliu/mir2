import hashlib
import json
import sys
from pathlib import Path

root = Path(__file__).parent
output = Path(sys.argv[1])
output.mkdir(parents=True, exist_ok=True)

def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()

def safe_event(event):
    selected = {key: event[key] for key in ('at', 'sequence', 'direction', 'packet', 'type') if key in event}
    if event.get('type') == 'worldSnapshot':
        payload = event.get('payload', {})
        selected['payload'] = {key: payload[key] for key in (
            'mapFileName', 'playerObjectId', 'playerHp', 'playerMaxHp', 'playerMp', 'playerMaxMp',
            'gold', 'knownSkills', 'inventoryItems', 'beltItems', 'equipmentItems', 'questLog',
            'freeBagSlots', 'currentWeight', 'maxWeight', 'playerWeights', 'inSafeZone') if key in payload}
        selected['payload']['entities'] = [{key: entity[key] for key in (
            'objectId', 'kind', 'name', 'class', 'gender', 'level', 'x', 'y', 'hp', 'maxHp', 'dead', 'direction', 'poison')
            if key in entity} for entity in payload.get('entities', [])]
    elif event.get('direction') == 'sent':
        assert event.get('type') in ('buyItem', 'useItem', 'magic', 'walk', 'run', 'clientVersion', 'acceptQuest', 'finishQuest', 'logOut')
        selected.update({key: event[key] for key in (
            'uniqueId', 'itemIndex', 'count', 'quantity', 'spell', 'objectId', 'targetId', 'spellTargetLock',
            'movementDirection', 'grid', 'direction', 'location', 'x', 'y', 'questId') if key in event})
    else:
        assert event.get('packet') in ('LoseGold', 'GainedItem', 'UseItem', 'Magic', 'ObjectDied', 'DamageIndicator',
            'UserLocation', 'MapInformation', 'ChangeQuest', 'CompleteQuest', 'LogOutSuccess', 'ClientVersion',
            'ObjectSpell', 'ObjectWalk', 'ObjectMagic', 'ObjectStruck', 'ObjectHealth', 'ObjectMonster', 'ObjectPoisoned')
        selected['payload'] = event.get('payload', {})
    return selected

cases = [
    ('Wizard', '2026-10-01T09-12-39-083Z', 'level9-hp-real-purchase', [2958,2959,2960,2961,2963]),
    ('Taoist', '2026-10-01T09-12-39-082Z', 'level9-hp-real-purchase', [3280,3281,3282,3283,3285]),
    ('Wizard', '2026-10-01T09-12-39-083Z', 'level15-hp-real-restock', [6687,6688,6689,6690,6691,6698,6699,6700,6701,6702]),
    ('Wizard', '2026-10-01T10-16-16-279Z', 'level24-town-real-restock', [5221,5222,5223,5224,5225]),
    ('Taoist', '2026-10-01T09-49-42-314Z', 'level19-hp-real-restock', [3301,3302,3303,3304,3305,3314,3315,3316,3317,3318]),
    ('Taoist', '2026-10-01T10-03-04-666Z', 'level22-amulet-real-restock', [8438,8439,8440,8441,8442,8449,8450,8451,8452,8453]),
    ('Taoist', '2026-10-01T10-27-25-529Z', 'level22-hp-real-restock', [758,759,760,761,762,769,770,771,772,773]),
    ('Wizard', '2026-10-01T09-12-39-083Z', 'mp-consumption', [4136,4137,4138,4139]),
    ('Wizard', '2026-10-01T09-12-39-083Z', 'random-consumption-transfer', [4803,4823,4849,4850,4891]),
    ('Wizard', '2026-10-01T10-16-16-279Z', 'town-consumption-transfer', [1445,1452,1460,1461,1476]),
    ('Taoist', '2026-10-01T09-49-42-314Z', 'mp-consumption', [1954,1955,1956,1957]),
    ('Taoist', '2026-10-01T09-49-42-314Z', 'soulfire-amulet-consumption', [571,572,574,581]),
    ('Taoist', '2026-10-01T10-03-04-666Z', 'poison-green-consumption', [9154,9155,9157,9162]),
    ('Taoist', '2026-10-01T09-12-39-082Z', 'healing-learned-practice-and-real-healing', [1554,1555,1556,1557,1560,2270,2271,2272,2273,2276,2285]),
    ('Taoist', '2026-10-01T09-49-42-314Z', 'soulfire-single-target-practice', [571,572,574,575,581,583,584,587,588]),
    ('Taoist', '2026-10-01T10-03-04-666Z', 'owned-skeleton-pet-practice', [570,571,573,574,579,583,593,601,602,604,605]),
    ('Taoist', '2026-10-01T10-03-04-666Z', 'poison-practice-and-following-same-target-tick', [9154,9155,9157,9158,9160,9161,9162,9180]),
]
grouped = {}
for klass, run, label, sequences in cases:
    grouped.setdefault((klass, run), set()).update(sequences)
sources = {}
records = {}
for (klass, run), required in grouped.items():
    path = root / klass.lower() / f'{klass}.{run}.trace.jsonl'
    sources[path.name] = sha(path)
    found = {}
    with path.open('rb') as stream:
        for line in stream:
            event = json.loads(line)
            if event.get('sequence') in required:
                found[event['sequence']] = safe_event(event)
    assert set(found) == required, (path.name, required - set(found))
    records[(klass, run)] = found
document = {'schema':1, 'scope':'selected-normal-protocol-supply-and-class-practice-receipts', 'sanitized':True,
    'visualAccepted':False, 'sources':sources, 'cases':[]}
for klass, run, label, sequences in cases:
    document['cases'].append({'className':klass, 'case':label, 'trace':f'{klass}.{run}.trace.jsonl',
        'events':[records[(klass, run)][sequence] for sequence in sorted(sequences)]})
(output / 'cohort-r2-supply-receipts.json').write_text(json.dumps(document, ensure_ascii=False, indent=2)+'\n', encoding='utf-8')

path = root / 'wizard/Wizard.2026-10-01T11-01-08-172Z.trace.jsonl'
required = {613,623,624,625,820,834,835,836,854,869,870,873}
events = []
all_casts = []
all_acks = []
damage = []
deaths = []
with path.open('rb') as stream:
    for line in stream:
        event = json.loads(line)
        if event.get('sequence') in required:
            events.append(safe_event(event))
        if event.get('direction') == 'sent' and event.get('type') == 'magic': all_casts.append(safe_event(event))
        if event.get('packet') == 'Magic': all_acks.append(safe_event(event))
        if event.get('packet') == 'DamageIndicator': damage.append(safe_event(event))
        if event.get('packet') == 'ObjectDied': deaths.append(safe_event(event))
assert {event['sequence'] for event in events} == required
document = {'schema':1, 'scope':'selected-ordinary-readiness-after-receipts', 'sanitized':True, 'visualAccepted':False,
    'trace':path.name, 'rawTraceSha256':sha(path), 'ownerMovementFreshReadinessAndMagic':events,
    'allSentMagic':all_casts, 'allMagicReceipts':all_acks, 'allDamageIndicators':damage, 'allObjectDeaths':deaths,
    'notEveryDamageIndicatorIsPlayerOffense':True}
(output / 'cohort-r2-ordinary-readiness-after.json').write_text(json.dumps(document, ensure_ascii=False, indent=2)+'\n',encoding='utf-8')

path = root / 'wizard/Wizard.2026-10-01T11-44-21-633Z.trace.jsonl'
allowed = {'Magic','ObjectSpell','ObjectWalk','ObjectMagic','ObjectStruck','ObjectHealth','DamageIndicator'}
events = []
with path.open('rb') as stream:
    for line in stream:
        event = json.loads(line)
        sequence = event.get('sequence', 0)
        if 4165 <= sequence <= 4237 and (event.get('type') == 'worldSnapshot' or
            event.get('packet') in allowed or (event.get('direction') == 'sent' and event.get('type') == 'magic')):
            events.append(safe_event(event))
document = {'schema':1,'scope':'accepted-ground-cast-moving-target-miss','sanitized':True,'visualAccepted':False,
    'trace':path.name,'rawTraceSha256':sha(path),'questId':2110021,'ownerObjectId':1000,'selectedTargetId':1202102,
    'groundAim':[301,382],'castAccepted':True,'selectedTargetDiagonalAfterCast':[302,381],
    'noPositiveTargetDamageClaimed':True,'events':events}
(output / 'cohort-r2-firewall-miss-before.json').write_text(json.dumps(document,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
print(json.dumps({'supplyCases':len(cases), 'wizardSentMagic':len(all_casts), 'wizardMagicReceipts':len(all_acks), 'output':str(output)}))
