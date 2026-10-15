"""Independent raw public packet review; no network, Gateway or private file reader."""
from pathlib import Path
from datetime import datetime, timezone
import hashlib
import json
import math

ROOT = Path(__file__).resolve().parent
PUBLIC = ROOT / 'downloaded-public'
INPUTS = ROOT.parent / 'dense-network-r18-inputs-01'
SOURCE = '321316b791120a6fe549e4006623e63333a5543f'
BINARY = '4064f6bce2337bbd1d251b37e7219163fd524828ddb83d90031232a0b1e25ca8'
FROZEN = 'b7ac4b4f33cc746248a6e6c0f2d9d97a5822fc4be5e953e5f1e08aad34dfc0c0'

def sha(path):
    return hashlib.file_digest(path.open('rb'), 'sha256').hexdigest()

def read(path):
    return json.loads(path.read_text(encoding='utf-8'))

def regular(path):
    assert path.is_file() and not path.is_symlink() and path.stat().st_nlink == 1, str(path)

def trace(phase, name):
    return [json.loads(line) for line in (PUBLIC / 'public-receipts' / (phase+'-receipts') / (name+'.jsonl')).read_text(encoding='utf-8').splitlines()]

def row_by_sequence(rows, sequence):
    matched = [r for r in rows if r['sequence'] == sequence]
    assert len(matched) == 1
    return matched[0]

def finite(n):
    return not isinstance(n, bool) and isinstance(n, (int, float)) and math.isfinite(n) and n >= 0

def owner_entity(payload, owner):
    matches = [e for e in payload['entities'] if e['objectId'] == owner]
    assert len(matches) == 1
    return matches[0]

def raw_world(row, owner, from_ms):
    assert row['direction'] == 'received' and row['type'] == 'worldSnapshot'
    p, s = row['payload'], row['state']
    assert p['playerObjectId'] == s['ownerId'] == owner
    for field, flag in [('hp', 'exactHp'), ('mp', 'exactMp')]:
        v = p['player' + field.capitalize()]
        prov = s[field + 'Provenance']
        assert finite(v) and v == s[field] and s[flag] is True
        assert prov['absolute'] is True and prov['ownerId'] == owner and prov['source'] == 'worldSnapshot'
        assert from_ms <= prov['observedMs'] <= row['tsMs']
    return p, owner_entity(p, owner)

def ledger(payload):
    result = {}
    for key in ['inventoryItems', 'beltItems', 'equipmentItems']:
        assert isinstance(payload[key], list)
        for item in payload[key]:
            uid, qty = item['uniqueId'], item['quantity']
            assert type(uid) is int and 0 <= uid < 2**53 and uid not in result
            assert type(qty) is int and qty >= 0
            result[uid] = qty
    return result

def plain_location(payload, entity):
    return {'map': payload['mapFileName'], 'x': entity['x'], 'y': entity['y'], 'direction': entity['direction']}

def fingerprint(payload):
    me = owner_entity(payload, payload['playerObjectId'])
    return {**plain_location(payload, me), 'class': me['class'], 'level': me['level'], 'gold': payload['gold'], 'dead': me['dead'],
            'spells': sorted(s['spell'] for s in payload['knownSkills']), 'items': ledger(payload),
            'equipment': sorted((i['uniqueId'], i['key'], i['slot'], i['durabilityCurrent']) for i in payload['equipmentItems'])}

def audit_caster(c):
    name = c['className']
    rows = trace('death', name)
    d, proof = c['death'], c['death']['deadActions']
    owner, start_ms, end_ms = proof['window']['ownerId'], proof['window']['fromMs'], proof['window']['endMs']
    conn = c['normalLogout']['connectionId']
    assert all(r['connectionId'] == conn for r in rows)
    death = row_by_sequence(rows, d['death']['deathSequence'])
    assert death['packet'] == 'Death' and death['direction'] == 'received' and death['state']['hp'] == 0 and death['state']['dead'] is True
    assert death['state']['hpProvenance']['absolute'] is True and death['state']['hpProvenance']['ownerId'] == owner
    assert death['tsMs'] == start_ms
    initial_seq = c['bootstrap']['sourceBootstrap']['receiptSequence']
    positives = [r for r in rows if r['sequence'] > initial_seq and r['direction'] == 'received' and r.get('packet') == 'DamageIndicator'
                 and r['payload'].get('objectId') == owner and r['payload'].get('damageType') == 0 and r['payload'].get('damage', 0) > 0 and r['tsMs'] <= death['tsMs']]
    assert len(positives) == d['death']['positiveOwnerHitCount'] > 0
    magic = row_by_sequence(rows, proof['castBarrier']['actualMagicInputSequence'])
    baseline = row_by_sequence(rows, proof['castBarrier']['baselineSnapshotSequence'])
    p, me = raw_world(baseline, owner, start_ms)
    assert p['playerHp'] == 0 and me['dead'] is True
    spell = magic['spell']
    assert magic['direction'] == 'sent' and magic['type'] == 'magic' and magic['objectId'] == owner
    assert baseline['sequence'] < magic['sequence'] <= d['commandsEndSequence'] and start_ms <= magic['tsMs'] <= end_ms
    skill = next(s for s in p['knownSkills'] if s['spell'] == spell)
    assert finite(skill['mpCost']) and p['playerMp'] >= skill['mpCost'] == 3
    assert not (magic['state']['poison'] & (8 | 16 | 32 | 1024))
    target = next(e for e in p['entities'] if e['objectId'] == magic['targetId'])
    assert magic['x'] == target['x'] and magic['y'] == target['y']
    if spell == 'FireBall':
        assert target['kind'] == 'monster' and target['disposition'] == 'hostile' and target['dead'] is False
        assert max(abs(target['x']-me['x']), abs(target['y']-me['y'])) == 1
    else:
        assert spell == 'Healing' and target['objectId'] == owner and target['kind'] == 'selfPlayer'
    closed = [r for r in rows if r['sequence'] > death['sequence'] and start_ms <= r['tsMs'] <= end_ms and r['state']['ownerId'] == owner]
    sent = [r for r in closed if r['direction'] == 'sent' and r['type'] in ['walk', 'run', 'attack', 'magic']]
    assert [r['type'] for r in sent] == ['walk', 'run', 'attack', 'magic']
    worlds = [r for r in closed if r['direction'] == 'received' and r['type'] == 'worldSnapshot' and r['sequence'] > magic['sequence']]
    assert [r['sequence'] for r in worlds] == proof['castBarrier']['absoluteMpObservationSequences']
    origin, materials, baseline_mp = plain_location(p, me), ledger(p), p['playerMp']
    mp_rows = []
    for row in worlds:
        pp, mm = raw_world(row, owner, magic['tsMs'])
        assert pp['playerHp'] == 0 and pp['playerMp'] == baseline_mp and mm['dead'] is True
        assert plain_location(pp, mm) == origin and ledger(pp) == materials
        mp_rows.append({'sequence': row['sequence'], 'tsMs': row['tsMs'], 'mp': pp['playerMp'], 'source': 'worldSnapshot', 'absolute': True})
    assert len(worlds) >= 8 and worlds[-1]['tsMs']-magic['tsMs'] >= 5000
    for row in closed:
        if row['direction'] != 'received':
            continue
        packet, pp = row.get('packet'), row.get('payload', {})
        if packet in ['HealthChanged', 'UserInformation'] and pp.get('objectId', owner) == owner and finite(pp.get('mp')):
            assert pp['mp'] == baseline_mp
        assert packet not in ['Revived', 'MapChanged', 'MapInformation']
        assert not (packet == 'ObjectAttack' and pp.get('objectId') == owner)
        assert not (packet == 'MagicCast' and pp.get('spell') == spell)
        assert not (packet == 'ObjectMagic' and pp.get('objectId') == owner and pp.get('cast') is True)
    terminal = row_by_sequence(rows, d['terminalOnlineObservation']['receiptSequence'])
    tp, tm = raw_world(terminal, owner, start_ms)
    assert terminal['tsMs'] > end_ms and tp['playerHp'] == 0 and tm['dead'] is True and plain_location(tp, tm) == origin
    logout = row_by_sequence(rows, c['normalLogout']['sentSequence'])
    ack = row_by_sequence(rows, c['normalLogout']['ackSequence'])
    assert logout['type'] == 'logOut' and logout['direction'] == 'sent' and ack['packet'] == 'LogOutSuccess' and ack['direction'] == 'received'
    assert terminal['sequence'] < logout['sequence'] < ack['sequence'] and terminal['tsMs'] <= logout['tsMs'] <= ack['tsMs'] <= c['normalLogout']['closedMs']
    assert c['normalLogout']['normalCloseConfirmed'] is True
    fresh = trace('death', name+'-fresh-load')
    fresh_info = c['freshLogin']['sourceBootstrap']
    new_conn = fresh_info['connectionId']
    assert new_conn != conn and all(r['connectionId'] == new_conn for r in fresh)
    assert fresh[0]['tsMs'] > c['normalLogout']['closedMs']
    login_inputs = [r for r in fresh if r['direction'] == 'sent' and r['type'] == 'login']
    start_inputs = [r for r in fresh if r['direction'] == 'sent' and r['type'] == 'startGame']
    assert len(login_inputs) == len(start_inputs) == 1 and login_inputs[0]['sequence'] < start_inputs[0]['sequence']
    start_ack = row_by_sequence(fresh, c['freshLogin']['startAckSequence'])
    assert start_ack['packet'] == 'StartGame' and start_ack['payload']['result'] == 4
    fresh_row = row_by_sequence(fresh, fresh_info['receiptSequence'])
    fp, fm = raw_world(fresh_row, owner, start_ack['tsMs'])
    assert fm['dead'] is False and fp['playerHp'] == fp['playerMaxHp'] > 0 and fp['playerMp'] == fp['playerMaxMp']
    bind = c['freshLoad']['expectedBindPoint']
    assert {k:v for k,v in plain_location(fp,fm).items() if k != 'direction'} == bind == {'map':'0','x':288,'y':616}
    assert not any(r['direction'] == 'sent' and r['type'] in ['townRevive','revive','resume','passkeyLogin'] for r in rows+fresh)
    assert sum(r.get('packet') == 'Death' for r in rows+fresh) == 1
    q = c['freshQuiet']['window']
    quiet = [r for r in fresh if r['sequence'] > q['afterSequence'] and q['fromMs'] <= r['tsMs'] <= q['endMs'] and r['state']['ownerId'] == owner]
    quiet_worlds = [r for r in quiet if r['direction'] == 'received' and r['type'] == 'worldSnapshot']
    assert q['endMs']-q['fromMs'] >= 5000 and len(quiet_worlds) >= 4 and quiet_worlds[-1]['tsMs'] >= q['fromMs']+4500
    assert not [r for r in quiet if r['direction'] == 'sent' and r['type'] not in ['clientVersion','keepAlive']]
    for r in quiet_worlds:
        pp, mm = raw_world(r, owner, q['fromMs'])
        assert mm['dead'] is False and pp['playerMp'] == fp['playerMp'] and plain_location(pp,mm) == plain_location(fp,fm)
    alive = trace('death', name+'-alive-relogin')
    ar = row_by_sequence(alive, c['aliveRelogin']['sourceBootstrap']['receiptSequence'])
    ap, am = raw_world(ar, owner, c['aliveRelogin']['startAckMs'])
    alive_save = row_by_sequence(fresh, c['freshAliveLogoutAckSequence'])
    assert alive_save['packet'] == 'LogOutSuccess' and alive[0]['tsMs'] > alive_save['tsMs'] and am['dead'] is False
    last_fresh_world = next(r for r in reversed(fresh) if r['type'] == 'worldSnapshot' and r['sequence'] < alive_save['sequence'] and r['payload'].get('playerObjectId') == owner)
    assert fingerprint(last_fresh_world['payload']) == fingerprint(ap)
    assert c['retainedResume']['executed'] is False and c['onlineTownRevive']['executed'] is False
    return {'className':name, 'independentRawReview':'passed', 'actualNaturalDeathSequence':death['sequence'], 'positiveOwnerHitCount':len(positives),
        'closedWindow':proof['window'], 'connectionId':conn, 'baselineSequence':baseline['sequence'], 'actualMagicInputSequence':magic['sequence'],
        'spell':spell, 'mpCost':skill['mpCost'], 'validTargetObserved':True, 'absoluteMpConstant':baseline_mp, 'absoluteMpReceipts':mp_rows,
        'itemQuantityLedgerStable':True, 'quantityLedger':materials, 'terminalOnlineDeadHp0':True,
        'normalLogoutSentSequence':logout['sequence'], 'normalLogoutAckSequence':ack['sequence'],
        'normalCloseEvidence':'frozen owned protocol close result in public report; no private transport log read',
        'newOrdinaryLoginStartGameConnectionId':new_conn, 'freshLoadSequence':fresh_row['sequence'], 'freshAliveFullCurrentCaps':[fp['playerHp'],fp['playerMp']],
        'freshLocation':plain_location(fp,fm), 'quietDurationMs':q['endMs']-q['fromMs'], 'quietWorldSamples':len(quiet_worlds),
        'alivePersistenceRawFingerprintMatched':True, 'directPrivateStoreInspection':False, 'directDurableHp0Verified':False,
        'positiveHpCorpseLevelUpCaseActuallyObserved':False, 'retainedResumeExecuted':False, 'onlineTownReviveExecuted':False}

def main():
    frozen = read(INPUTS / 'FROZEN-INPUTS.json')
    assert sha(INPUTS / 'FROZEN-INPUTS.json') == FROZEN
    for f in frozen['files']:
        p = INPUTS / f['relativePath']; regular(p)
        assert p.stat().st_size == f['bytes'] and sha(p) == f['sha256']
    export = read(ROOT / 'PUBLIC-EXPORT-01.json')
    for f in export['files']:
        p = PUBLIC / f['relativePath']; regular(p)
        assert p.stat().st_size == f['bytes'] and sha(p) == f['sha256']
    wrapper, launch = read(PUBLIC/'wrapper-report.json'), read(PUBLIC/'EXECUTION-LAUNCH-01.json')
    assert wrapper['sourceRevision'] == launch['sourceRevision'] == SOURCE
    assert wrapper['binarySha256'] == launch['binarySha256'] == BINARY and launch['wrapperExitCode'] == 0
    assert wrapper['frozenInputsSha256'] == FROZEN and launch['singleAttemptNoRetry'] is True
    counters = ['currentWsConnections','currentActiveSessions','currentReconnectLeases','currentLoginInFlight','currentNewCharacterInFlight','currentStartGameInFlight']
    stages = []
    for s in wrapper['stages']:
        assert s['normalLogoutConfirmed'] is True and s['ownedGatewayNormalExitCode'] == 0 and s['qaEnabled'] is False
        assert all(s['settledHealth']['capacity'][key] == 0 for key in counters)
        stages.append({'phase':s['phase'],'ownedGatewayPid':s['ownedGatewayPid'],'normalExitCode':0,'sixSettledCounters':[0]*6,'normalLogoutConfirmed':True})
    death_report = read(PUBLIC / 'public-receipts/death-receipts/report.json')
    classes = [audit_caster(c) for c in death_report['classes'] if c['className'] != 'Warrior']
    warrior = next(c for c in death_report['classes'] if c['className'] == 'Warrior')
    wr = trace('death','Warrior')
    positives = [r for r in wr if r.get('packet') == 'DamageIndicator' and r['direction'] == 'received' and r['payload'].get('objectId') == 1000 and r['payload'].get('damageType') == 0 and r['payload'].get('damage',0)>0]
    assert not any(r.get('packet') == 'Death' for r in wr) and len(positives) == 122
    assert warrior['death']['status'] == 'inconclusive' and warrior['naturalDeathAttempts'] == 1
    formation = []
    for c in read(PUBLIC / 'public-receipts/dense-receipts/report.json')['classes']:
        for case in c['cases']:
            ts = case.get('formation',{}).get('trials',[])
            if not ts:
                assert case['status'] == 'not-applicable'
                continue
            assert case['status'] == 'inconclusive' and all(t['qualification']['status'] == 'inconclusive' for t in ts)
            formation.append({'className':c['className'],'kind':case['kind'],'status':case['status'],'sampleCount':len(ts),
                'elapsedMs':case['formation']['elapsedMs'],'maxAdjacent':max(t['adjacent'] for t in ts),
                'maxDistinctQualifiedAttackerIds':max(len(t['qualification']['attackerIds']) for t in ts),
                'maxPositiveOwnerHitsIn6200ms':max(t['qualification']['positiveOwnerHitCount'] for t in ts),
                'samplesWithLegalExit':sum(t['qualification']['exit'] is not None for t in ts),
                'qualifiedSamples':0,'lastReasons':ts[-1]['qualification']['reasons'],'escapeTrialExecuted':False})
    assert len(formation) == 10 and max(f['maxDistinctQualifiedAttackerIds'] for f in formation) == 4
    purchases = []
    for c in read(PUBLIC / 'public-receipts/purchases-receipts/report.json')['classes']:
        p = c['purchases']
        purchases.append({'className':c['className'],'medicinePurchases':p['medicines'],'medicineUseAckSequence':p['medicineUse']['ackSequence'],
            'skillBookPurchase':p['skillBook'],'bookUseAckSequence':p.get('bookUse',{}).get('ackSequence'),'learnedSpell':p.get('learnedSpell'),
            'bookEntryStatus':p.get('bookEntry',{}).get('status'),'bookExitStatus':p.get('bookExit',{}).get('status')})
    live = read(ROOT/'LIVE-COMPARISON-01.json')
    assert all(live[k] is True for k in ['realmsUnchanged','caddyUnchanged','configHashesUnchanged','feedUnchanged'])
    assert live['beforeSixCounters'] == live['afterSixCounters'] and live['beforeTcpCounts'] == live['afterTcpCounts']
    receipt = {'schema':'mir2.r18-independent-public-raw-review.v1','createdUtc':datetime.now(timezone.utc).isoformat(),
        'sourceRevision':SOURCE,'binarySha256':BINARY,'frozenInputsSha256':FROZEN,'immutableInputCountVerified':len(frozen['files']),
        'publicFileCountVerified':len(export['files']),'offlineOnly':True,'privateStoresKeysRawGatewayLogsRead':False,
        'earlierFilesChanged':False,'verificationComplete':True,'singleAttemptNoRetry':True,'stages':stages,'classes':classes,
        'warrior':{'status':'inconclusive','positiveOwnerHits':122,'ownerDeathCount':0,'singleNaturalDeathBudgetSeconds':180,'castAndFreshLoadNotExecuted':True},
        'denseCases':formation,'applicableDenseCases':10,'qualifiedDenseCases':0,'purchases':purchases,
        'liveBeforeAfterUnchanged':True,'r16ComparisonExecuted':False,'historicalR17IndependentMpFailuresRemainPreserved':True,
        'sevenMonsterMechanicsAccepted':False,'fullP1Accepted':False,'nativeInputAccepted':False,'nativeVisualAccepted':False,
        'humanAccepted':False,'ordinaryProgressionAccepted':False,'loadAccepted':False,'capacityAccepted':False,
        'limitations':['Prepared level-30 field staging is not ordinary long-play progression','TownRevive and optional retained resume were not executed',
            'No natural positiveHP corpse LevelUp case occurred; it cannot be inferred from HP alone',
            'Normal transport close is owned driver public metadata; private raw logs and durable stores were not inspected',
            'No seven-attacker escape case qualified; wrapper exit 0 is process completion only']}
    out = ROOT/'INDEPENDENT-RAW-REVIEW-01.json'
    with out.open('x',encoding='utf-8',newline='\n') as f:
        json.dump(receipt,f,indent=2,ensure_ascii=False); f.write('\n')
    print(json.dumps({'output':str(out),'sha256':sha(out),'verificationComplete':True,'casters':[c['className'] for c in classes],'qualifiedDenseCases':0}))

if __name__ == '__main__':
    main()
