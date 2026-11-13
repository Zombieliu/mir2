"""Explicit synthetic melee packet fixtures; no games, sockets or private reads."""
import copy
import json
from pathlib import Path
import tempfile
import types
import unittest
from unittest import mock
import test_verify_report as base
v=base.v

CONTRACT = {'schema': 'mir2.comparable.combat-contract.v1', 'scenario': 'dispersed-melee', 'attackMode': 'all', 'spell': 'none', 'primaryActionKinds': ['attack', 'chat'], 'expectedPrimaryOffersPerActorPerMinute': 52, 'targetBinding': 'planned-adjacent-cell-and-current-shared-object-id', 'attackTerminalEvidence': {'ownerLocationRequired': True, 'allDeclaredPeerAnimationsRequired': True, 'targetOutcomeRequired': True, 'hitRequiresAttackerBoundStruckAndVictimBoundDamageAndExactHpDelta': True, 'explicitMissOrBlockIsValidTerminal': True, 'unpairedHpChangeOrAnimationIsInsufficient': True, 'ambiguousCompetingDamageIsHardFailure': True}, 'potions': {'schema': 'mir2.comparable.reactive-potions.v1', 'itemIndex': 662, 'hpTriggerRatio': 0.7, 'minIntervalMs': 2000, 'maximumInFlightPerActor': 1, 'onlyWhenAttackAndChatIdle': True, 'primaryOfferWhilePotionPending': 'notSent', 'ackDeadlineMs': 2500, 'stockStateDeadlineMs': 2500, 'ordinaryUidAndGridAckRequired': True, 'successfulUseMustConsumeExactlyOne': True, 'laterHpIncreaseAloneIsNotPotionProof': True, 'retryAfterTimeout': False, 'restockAllowed': False, 'includedInPrimaryOfferDenominator': False, 'issuedUseEvidenceFailureIsHardFailure': True}, 'safety': {'deathEndsStage': True, 'debugRespawnAllowed': False, 'damageAiRegenOrDropChangesAllowed': False, 'finalSaveComparedToLastAuthoritativeState': True, 'ordinaryDurabilityAndStockChangesAllowed': True}, 'capacityClaimAllowed': False}


def plan():
    p=base.synthetic_plan(version=2);p['scenario']='dispersed-melee';p['scheduling'].update(primaryIntervalMs=1200,minimumSameActorOfferGapMs=600,chatOffsetWithinPrimaryIntervalMs=600)
    p.update(combatContract=copy.deepcopy(CONTRACT),combatContractSha256=v.COMBAT_SHA,combatContractCanonicalSha256=v.COMBAT_CANONICAL_SHA)
    rows=[]
    for actor in p['actors']:
        i=actor['ordinal'];actor['start']={'x':55+i,'y':51};actor['lane']=[dict(actor['start']),dict(actor['start'])];actor['attackTargetOrdinal']=1-i
        offers=[{'actorOrdinal':i,'kind':'attack','dueMs':due,'deadlineMs':due+2500,'targetActorOrdinal':1-i} for due in range(i*600,60000,1200)]
        offers += [{'actorOrdinal':i,'kind':'chat','dueMs':due,'deadlineMs':due+2500} for due in range(15000+i*600,60000,30000)]
        for ticket,a in enumerate(sorted(offers,key=lambda r:(r['dueMs'],r['kind'])),1):
            a['ticketSequence']=ticket
            if a['kind']=='chat':a['nonce']=f"m2cmp:{p['planId']}:{i}:{ticket}";a['nonceSha256']=v.digest(a['nonce'].encode())
            rows.append(a)
    p['actions']=sorted(rows,key=lambda a:(a['dueMs'],a['actorOrdinal'],a['ticketSequence']))
    return p


def native_items(actor, *, count=20, dura=4000):
    owned={'slots':[{'grid':'equipment','slot':0,'uniqueId':100+actor,'itemIndex':221,'count':1,'currentDura':dura,'maxDura':4000,'originalUserItemSaveSha256':v.digest(f'weapon-{actor}-{dura}'.encode())},
                    {'grid':'inventory','slot':0,'uniqueId':200+actor,'itemIndex':662,'count':count,'currentDura':0,'maxDura':0,'originalUserItemSaveSha256':v.digest(f'potion-{actor}-{count}'.encode())}],
           'gold':0,'credit':0,'capacity':{'inventoryIncludingBelt':46,'equipment':14,'questInventory':0,'belt':6,'bag':40},
           'beltRule':'original Inventory indices0..5; alias of inventory, not duplicated item ownership'}
    return {'owned':owned,'fingerprintSha256':v.digest(v.encoded(owned))}


def actions(engine='modern', terminal='hit', *, not_sent=(), late=None, heal=False):
    p=plan();events=[];seq=[0,0];hp=[1000,1000];epoch=1000
    r={'schema':f'mir2.comparable.{engine}-report.v1','synthetic':True,'planId':p['planId'],'measurementEpochMs':epoch,'measurementStartedAtMs':epoch,
       'observationEndedAtMs':epoch+63000,'measurementEndedAtMs':epoch+60000,'processId':7000,'combatBaselines':[],'combatContractSha256':v.COMBAT_SHA}
    def raw(actor,name,body,at):
        seq[actor]+=1
        e=({'event':'received','receiveSeq':seq[actor],'atMs':at,'payload':body} if engine=='modern' else
           {'kind':'packet','direction':'received','sequence':seq[actor],'monotonicMs':at,'detail':body})
        e.update(actorOrdinal=actor,connection=1,packet=name);events.append(e)
        return {'observerOrdinal':actor,'receiverActorOrdinal':actor,'connection':1,'receiveSeq':seq[actor],'receiveSequence':seq[actor],'atMs':at,'receivedAtMs':at,'packet':name}
    def sent(actor,name,at,correlation=None,command=None):
        e=({'event':'sent','atMs':at,'command':command or {'type':name}} if engine=='modern' else
           {'kind':'packet','direction':'sent','sequence':len(events)+1000,'monotonicMs':at,'packet':name,'correlation':correlation})
        e.update(actorOrdinal=actor,connection=1);events.append(e);return e
    for actor in range(2):
        pos=p['actors'][actor]['start'];uid=1000 if engine=='modern' else 50000+actor
        sent(actor,'Login',700,command={'type':'login'});raw(actor,'LoginSuccess',{},710)
        sent(actor,'StartGame',720,command={'type':'startGame'});raw(actor,'StartGame',{'result':4},730)
        ui=raw(actor,'UserInformation',{'location':pos,'position':pos,'objectId':uid,'hp':1000,'mp':50,'maxHp':1000,'name':f'Hero{actor}','class':'Warrior','gender':'Male','level':40,'realCharacterIndex':actor+1,
              'nameHash':'a'*64,'identityMatchesFixture':True,'magicCount':0,'hasHero':False,'creatureSummoned':False,'intelligentCreatureCount':0,'authoritativeItemState':native_items(actor)},740)
        snapshot=raw(actor,'worldSnapshot',{'playerHp':1000,'playerMp':50,'playerObjectId':uid,'map':'DogYoHyun','owner':{**pos,'level':40},'entities':[]},750) if engine=='modern' else None
        raw(actor,'ObjectPlayer',{'objectId':50000+1-actor,'name':f'Hero{1-actor}','position':p['actors'][1-actor]['start']},760)
        events.append({'event':'admitted','actorOrdinal':actor,'connection':1,'atMs':770} if engine=='modern' else {'kind':'admitted','actorOrdinal':actor,'connection':1,'monotonicMs':770})
        request=sent(actor,'ChangeAMode',800,command={'type':'changeAMode','mode':5});ack=raw(actor,'ChangeAMode',{'mode':5},810)
        if engine=='modern':
            events.append({'event':'combatMode','actorOrdinal':actor,'connection':1,'mode':5,'ack':ack})
            events.append({'event':'combatBaseline','actorOrdinal':actor,'connection':1,'userInformation':ui,'sourceReceipt':snapshot,'knownSkills':[],'activeBuffs':[],'hero':None,'intelligentCreatures':[]})
        else:
            events.append({'kind':'attackModeAll','result':{'actorOrdinal':actor,'connection':1,'mode':5,'sentSequence':request['sequence'],'receiveSequence':ack['receiveSequence']}})
            r['combatBaselines'].append({'actorOrdinal':actor,'connection':1,'initialUserInformationReceiveSequence':ui['receiveSequence'],'initialUserInformationAtMs':740,'maximumHp':1000,'activeOwnedBuffCount':0})
    for a in p['actions']:
        actor,ticket=a['actorOrdinal'],a['ticketSequence'];target=1-actor;when=epoch+a['dueMs']+10+actor*100;deadline=epoch+a['deadlineMs'];floors=dict(enumerate(seq));direction='Right' if actor==0 else 'Left'
        spec={**a,'action':a['kind'],'plannedAtMs':epoch+a['dueMs'],'dueAtMs':epoch+a['dueMs'],'deadlineAtMs':deadline,'expectedPeerOrdinals':[target]}
        if engine=='crystal':events.append({**spec,'kind':'offered'})
        if (actor,ticket) in not_sent:
            value={**spec,'sentAtMs':None,'passed':False,'completedOnTime':False,'timedOut':False,'timeout':False}
            events.append({'event':'notSent',**value} if engine=='modern' else {'kind':'notSent','result':value})
            if engine=='crystal':events.append({'kind':'actionOutcome','result':value})
            continue
        ids=[{'observerOrdinal':other,'connection':1,'objectId':1000 if other==actor else 50000+actor} for other in ([actor,target] if a['kind']=='chat' else [target])]
        common={**spec,'sentAtMs':when,'direction':direction,'expectedSharedIds':ids,'receiveSequenceFloors':[{'actorOrdinal':i,'connection':1,'receiveSeq':s} for i,s in floors.items()]}
        correlation={'ticketSequence':ticket,'targetActorOrdinal':target,'expectedTargetSharedId':50000+target,'spell':0,'direction':2 if actor==0 else 6}
        request=sent(actor,'Attack' if a['kind']=='attack' else 'Chat',when,correlation,{'type':'attackDirection','direction':direction,'spell':0} if a['kind']=='attack' else {'type':'chat','message':a['nonce']})
        if engine=='modern':common['actualTarget']={'actorOrdinal':target,'connection':1,'objectId':50000+target,'ownerObjectId':1000,**p['actors'][target]['start']};events.append({'event':'actionSent',**common})
        else:
            common.update(expectedSharedId=50000+actor,expectedTargetSharedId=50000+target,sentSequence=request['sequence'],receiveSequenceFloors={str(i):s for i,s in floors.items()},origin=p['actors'][actor]['start'],target=p['actors'][target]['start'])
            events.append({**common,'kind':'sent'})
        refs=[]
        if a['kind']=='chat':
            for observer in [actor,target]:
                ref=raw(observer,'ObjectChat',{'objectId':1000 if engine=='modern' and observer==actor else 50000+actor,'text':'synthetic: '+a['nonce'],'nonceHash':a['nonceSha256']},when+20);refs.append(ref)
        else:
            refs.append(raw(actor,'UserLocation',{'position':p['actors'][actor]['start'],'direction':direction},when+20))
            victim=1000 if engine=='modern' else 50000+target
            if terminal=='hit' and engine=='modern':
                if heal:hp[target]+=2;raw(target,'HealthChanged',{'hp':hp[target],'mp':50},when+20)
                hp[target]-=5;refs.append(raw(target,'HealthChanged',{'hp':hp[target],'mp':50},when+21))
            refs.append(raw(target,'ObjectAttack',{'objectId':50000+actor,'position':p['actors'][actor]['start'],'direction':direction,'spell':0},when+25))
            if terminal!='miss':refs.append(raw(target,'ObjectStruck' if engine=='modern' else 'Struck',{'objectId':victim,'attackerId':50000+actor},when+26))
            body=({'object_id':victim,'damage':5 if terminal=='hit' else 0,'damage_type':1 if terminal=='miss' else 0} if engine=='modern' else
                  {'objectId':victim,'damage':-5 if terminal=='hit' else 0,'type':1 if terminal=='miss' else 0})
            refs.append(raw(target,'DamageIndicator',body,when+27))
            if terminal=='hit' and engine=='crystal':
                prior=hp[target];hp[target]-=5;refs.append(raw(target,'HealthChanged',{'hpBefore':prior,'hp':hp[target],'mp':50},when+28))
        timed=(actor,ticket)==late
        if timed:
            for e in events:
                if v.raw_packet(e,engine) and any(v.packet_key(v.raw_packet(e,engine))==v.packet_key(v.receipt(f,engine)) for f in refs):e['atMs' if engine=='modern' else 'monotonicMs']=deadline+1
            for f in refs:f['atMs']=f['receivedAtMs']=deadline+1
            events.append({'event':'actionTimeout',**spec,'atMs':deadline} if engine=='modern' else {'kind':'timeout','actorOrdinal':actor,'ticketSequence':ticket,'observedAtMs':deadline})
        final={**common,'receipts':refs,'passed':not timed,'completedOnTime':not timed,'timedOut':timed,'timeout':timed}
        if engine=='modern':
            if a['kind']=='attack':final['outcome']={'kind':{'hit':'hit','miss':'explicitMiss','block':'explicitZeroBlock'}[terminal]}
            events.append({'event':'actionTerminal',**final})
        else:
            if a['kind']=='attack':
                final['combatEvidence']={'actorOrdinal':actor,'targetActorOrdinal':target,'attackerSharedId':50000+actor,'targetSharedId':50000+target,'sentAtMs':when,'deadlineAtMs':deadline,
                    'expectedObservers':{str(i):{'connection':1,'receiveSequenceFloor':s} for i,s in floors.items()},'invalidReason':None,'facts':refs,'terminalKind':{'hit':'positiveDamage','miss':'miss','block':'explicitZeroDamage'}[terminal]}
            else:
                for f in refs:events.append({'kind':'ownerReceipt' if f['receiverActorOrdinal']==actor else 'peerReceipt','actorOrdinal':actor,'ticketSequence':ticket,'detail':f})
            events.append({'kind':'actionOutcome','result':final})
    return p,r,events


class CombatActionTests(unittest.TestCase):
    def verify(self,engine='modern',**kwargs):
        p,r,e=actions(engine,**kwargs);c=v.Checks();result=v.verify_actions(p,r,e,engine,c);return p,r,e,c,result

    def test_exact_fixed_contract_and_midpoint_melee_plan(self):
        p=plan();v.validate_plan(p);self.assertEqual(v.digest(v.encoded(CONTRACT,sorted_keys=True)),v.COMBAT_CANONICAL_SHA)
        for mutate in [lambda x:x['combatContract']['potions'].update(ackDeadlineMs=5000),lambda x:x['actions'][0].update(targetActorOrdinal=0),lambda x:x['scheduling'].update(minimumSameActorOfferGapMs=10)]:
            bad=copy.deepcopy(p);mutate(bad)
            with self.assertRaises(v.EvidenceError):v.validate_plan(bad)

    def test_both_protocols_full_actor_minutes_rebuilt_from_packets(self):
        for engine in ['modern','crystal']:
            with self.subTest(engine=engine):
                p,r,e,c,(windows,derived,_,_)=self.verify(engine)
                self.assertFalse(c.failures);self.assertEqual(len(derived),104);self.assertTrue(all(w['passed'] and w['offered']==52 and w['completed']==52 for w in windows))

    def test_explicit_miss_and_attacker_bound_zero_are_separate_terminals(self):
        for engine in ['modern','crystal']:
            for terminal in ['miss','block']:
                _,_,_,c,(windows,derived,_,_)=self.verify(engine,terminal=terminal)
                self.assertFalse(c.failures);self.assertTrue(all(w['passed'] for w in windows))
                self.assertEqual({d.get('targetOutcome') for d in derived if d['kind']=='attack'},{'explicitMiss' if terminal=='miss' else 'explicitZeroBlock'})

    def test_modern_separate_heal_then_exact_damage_not_net_hp_delta(self):
        _,_,_,c,(windows,_,_,_)=self.verify('modern',heal=True);self.assertFalse(c.failures);self.assertTrue(all(w['passed'] for w in windows))

    def test_one_not_sent_is_not_an_additional_veto_or_timeout(self):
        _,_,_,c,(windows,_,_,_)=self.verify('modern',not_sent={(0,1)});self.assertFalse(c.failures)
        self.assertTrue(windows[0]['passed']);self.assertEqual(windows[0]['notSent'],1);self.assertEqual(windows[0]['timeout'],0)

    def test_two_not_sent_still_use_only_the_declared_ratio_and_three_fail(self):
        for count in [2,3]:
            _,_,_,c,(windows,_,_,_)=self.verify('modern',not_sent={(0,t) for t in range(1,count+1)})
            self.assertFalse(c.failures);self.assertEqual(windows[0]['passed'],count==2)

    def test_original_deadline_failure_never_restored_by_late_terminal(self):
        for engine in ['modern','crystal']:
            _,_,_,c,(windows,derived,_,_)=self.verify(engine,late=(1,52))
            self.assertFalse(c.failures);self.assertFalse(windows[1]['passed'])
            last=next(d for d in derived if d['actorOrdinal']==1 and d['ticketSequence']==52)
            self.assertTrue(last['timeout']);self.assertFalse(last['completed'])

    def test_wrong_attacker_damage_or_owner_response_cannot_be_animation_only_pass(self):
        for packet,key,value in [('ObjectStruck','attackerId',999),('DamageIndicator','damage',7),('UserLocation','direction','Left')]:
            p,r,e=actions();raw=next(x for x in e if x.get('packet')==packet);raw['payload'][key]=value
            with self.assertRaises(v.EvidenceError):v.verify_actions(p,r,e,'modern',v.Checks())

    def test_native_hp_before_is_independently_reconstructed(self):
        p,r,e=actions('crystal');raw=next(x for x in e if x.get('packet')=='HealthChanged');raw['detail']['hpBefore']+=1
        with self.assertRaisesRegex(v.EvidenceError,'prior_packet'):v.verify_actions(p,r,e,'crystal',v.Checks())

    def test_native_fifo_cannot_be_replaced_by_modern_order(self):
        p,r,e=actions('crystal');values=[x for x in e if x.get('kind')=='packet' and x.get('direction')=='received' and x.get('actorOrdinal')==1]
        damage=next(x for x in values if x.get('packet')=='DamageIndicator');health=next(x for x in values if x.get('packet')=='HealthChanged')
        damage['sequence'],health['sequence']=health['sequence'],damage['sequence']
        # Rebind the adapter facts too: raw FIFO, not a self-reported result, must reject.
        for x in e:
            proof=x.get('result',{}).get('combatEvidence',{})
            for f in proof.get('facts',[]):
                if f.get('receiverActorOrdinal')==1 and f.get('receivedAtMs') in [damage['monotonicMs'],health['monotonicMs']]:f['receiveSequence']=damage['sequence'] if f['packet']=='DamageIndicator' else health['sequence']
        with self.assertRaises(v.EvidenceError):v.verify_actions(p,r,e,'crystal',v.Checks())

    def test_external_contract_and_adapter_proof_are_required_even_with_green_summary(self):
        p,r,e=actions();controller,server,injector,meta,_=base.synthetic_resources();r.update(ok=True,runCompleted=True)
        result=v.verify_bundle(p,r,e,controller,server,injector,meta)
        self.assertFalse(result['passed']);self.assertEqual(result['hardFailures'][0]['code'],'external_combat_contract_bytes_not_bound')
        meta['combatContract']={'sha256':v.COMBAT_SHA};result=v.verify_bundle(p,r,e,controller,server,injector,meta,combat_contract=CONTRACT)
        self.assertFalse(result['passed']);self.assertEqual(result['hardFailures'][0]['code'],'melee_adapter_source_artifacts_required')


def potion_fixture(engine='modern'):
    p,r,all_events=actions(engine)
    clock='atMs' if engine=='modern' else 'monotonicMs'
    e=[x for x in all_events if not (v.raw_packet(x,engine) and v.raw_packet(x,engine)['at']>=1000)
       and (x.get('event') not in ['sent','actionSent','actionTerminal','actionTimeout','notSent'] or x.get('atMs',0)<1000)
       and (x.get('kind') not in ['packet','offered','sent','actionOutcome','ownerReceipt','peerReceipt','timeout','notSent'] or x.get('kind')=='packet' and x.get('monotonicMs',0)<1000)]
    cursor=max(v.raw_packet(x,engine)['sequence'] for x in e if v.raw_packet(x,engine) and x['actorOrdinal']==0)
    def raw(name,body,at):
        nonlocal cursor
        cursor+=1;row={'actorOrdinal':0,'connection':1,'packet':name}
        row.update({'event':'received','receiveSeq':cursor,'atMs':at,'payload':body} if engine=='modern' else {'kind':'packet','direction':'received','sequence':cursor,'monotonicMs':at,'detail':body})
        e.append(row);return {'observerOrdinal':0,'receiverActorOrdinal':0,'connection':1,'receiveSeq':cursor,'receiveSequence':cursor,'atMs':at,'receivedAtMs':at,'packet':name}
    raw('HealthChanged',{'hp':650,'mp':50,**({'hpBefore':1000} if engine=='crystal' else {})},3000)
    if engine=='modern':
        def state(count):return {'inventoryItems':[{'uniqueId':200,'key':'crystal-item-662','slot':0,'quantity':count}],
            'equipmentItems':[{'uniqueId':100,'key':'crystal-item-221','slot':'weapon','quantity':1,'durabilityCurrent':4000,'durabilityMax':4000,'tooltipSource':{'userItem':{'current_dura':4000}}}],
            'beltItems':[],'gold':0,'inventoryCapacity':46,'maxBagSlots':40}
        before,after=state(20),state(19)
        def snapshot(carried,hp):return {'playerObjectId':1000,'map':'DogYoHyun','playerHp':hp,'playerMp':50,'carriedFingerprint':v.carried(carried,engine)['fingerprint'],
             'owner':{'x':55,'y':51,'direction':'Right','level':40,'name':'Hero0','class':'Warrior','gender':'Male'}}
        source=raw('worldSnapshot',snapshot(before,650),3001)
        e.append({'event':'potionSourceState','actorOrdinal':0,'connection':1,'potionSequence':1,'state':before,'sourceReceipt':source})
        e.append({'event':'sent','actorOrdinal':0,'connection':1,'atMs':3010,'command':{'type':'useItem','uniqueId':200,'grid':'inventory'}})
        e.append({'event':'potionSent','actorOrdinal':0,'connection':1,'potionSequence':1,'sentAtMs':3010,'deadlineAtMs':5510,'uniqueId':200,'hpAtIssue':650})
        ack=raw('UseItem',{'uniqueId':200,'grid':'Inventory','success':True},3020)
        source_after=raw('worldSnapshot',snapshot(after,760),3030)
        e.append({'event':'potionStateEvidence','actorOrdinal':0,'connection':1,'potionSequence':1,'state':after,'sourceReceipt':source_after})
        e.append({'event':'potionTerminal','actorOrdinal':0,'connection':1,'potionSequence':1,'passed':True,'timedOut':False,'proof':{'ack':ack,'stock':source_after}})
    else:
        e.append({'kind':'packet','direction':'sent','actorOrdinal':0,'connection':1,'sequence':999,'monotonicMs':3010,'packet':'UseItem'})
        ack=raw('UseItem',{'uniqueID':200,'grid':1,'success':True,'remainingCountPresentInNativePacket':False},3020)
        e.append({'kind':'potion','result':{'actorOrdinal':0,'connection':1,'uniqueId':200,'sentSequence':999,'receiveSequence':ack['receiveSequence'],
            'sentAtMs':3010,'receivedAtMs':3020,'deadlineAtMs':5510,'hpAtDecision':650,'before':native_items(0),'after':native_items(0,count=19),
            'freshStockSnapshotClaimed':False,'finalFreshAndAllDiskReadbackRequired':True,'laterHpIncreaseIsUniquePotionProof':False}})
    return p,r,e,raw


class PotionAndFinalStateTests(unittest.TestCase):
    def evaluate(self,engine,change=None):
        p,r,e,_=potion_fixture(engine)
        if change:change(p,r,e)
        packets,admissions,_=v.index_evidence(e,engine,v.Checks())
        return v.verify_potions(p,r,e,packets,admissions,[],engine)

    def test_both_ordinary_potion_boundaries_and_exact_stock_one(self):
        for engine in ['modern','crystal']:
            result=self.evaluate(engine);self.assertEqual(result['issued'],1);self.assertFalse(result['hpIncreaseUsedAsPotionProof'])
            self.assertEqual(result['nativeImmediateStockIsAckDerived'],engine=='crystal')

    def test_hp_heal_cannot_replace_ordinary_UID_grid_ACK(self):
        def change(p,r,e):
            ack=next(x for x in e if x.get('event')=='received' and x.get('packet')=='UseItem');ack.update(packet='HealthChanged',payload={'hp':760,'mp':50})
        with self.assertRaises(v.EvidenceError):self.evaluate('modern',change)

    def test_modern_hash_only_or_unchanged_stock_is_insufficient(self):
        def change(p,r,e):
            proof=next(x for x in e if x.get('event')=='potionStateEvidence');proof['state']['inventoryItems'][0]['quantity']=20
            raw=next(x for x in e if x.get('receiveSeq')==proof['sourceReceipt']['receiveSeq'] and x.get('event')=='received')
            raw['payload']['carriedFingerprint']=v.carried(proof['state'],'modern')['fingerprint']
        with self.assertRaisesRegex(v.EvidenceError,'stock_not_exactly_one|roster_has_unexplained_change'):self.evaluate('modern',change)

    def test_native_count_boundary_cannot_be_claimed_independent_snapshot(self):
        def change(p,r,e):next(x for x in e if x.get('kind')=='potion')['result']['freshStockSnapshotClaimed']=True
        with self.assertRaisesRegex(v.EvidenceError,'misrepresented'):self.evaluate('crystal',change)

    def test_native_potion_before_full_state_must_match_prior_raw(self):
        def change(p,r,e):
            before=next(x for x in e if x.get('kind')=='potion')['result']['before'];before['owned']['slots'][1]['count']=21;before['fingerprintSha256']=v.digest(v.encoded(before['owned']))
        with self.assertRaisesRegex(v.EvidenceError,'prior_stock'):self.evaluate('crystal',change)

    def test_issued_late_or_unaccounted_potion_never_passes(self):
        def late(p,r,e):next(x for x in e if x.get('event')=='potionSent')['deadlineAtMs']=3015
        with self.assertRaises(v.EvidenceError):self.evaluate('modern',late)
        def extra(p,r,e):e.append({'event':'sent','actorOrdinal':0,'connection':1,'atMs':3500,'command':{'type':'useItem','uniqueId':200,'grid':'inventory'}})
        with self.assertRaisesRegex(v.EvidenceError,'unaccounted'):self.evaluate('modern',extra)

    def test_modern_logout_reconstructs_last_snapshot_plus_all_exact_patches(self):
        p,r,e,raw=potion_fixture();full=copy.deepcopy(next(x for x in e if x.get('event')=='potionStateEvidence')['state'])
        source=next(x for x in e if x.get('event')=='potionStateEvidence')['sourceReceipt']
        health=raw('HealthChanged',{'hp':762,'mp':49},70000);dura=raw('DuraChanged',{'uniqueId':100,'currentDura':3999},70001)
        full['equipmentItems'][0]['durabilityCurrent']=3999;full['equipmentItems'][0]['tooltipSource']['userItem']['current_dura']=3999
        ack=raw('LogOutSuccess',{},70002);owner={'map':'DogYoHyun','x':55,'y':51,'direction':'Right','name':'Hero0','class':'Warrior','gender':'Male','level':40,'hp':762,'mp':49,'items':v.carried(full,'modern')['fingerprint']}
        e.append({'event':'logoutOwnedState','actorOrdinal':0,'connection':1,'state':owner,'carried':full,'source':{'snapshot':source,'patches':[health,dura],'logOutSuccess':ack}})
        packets,_,_=v.index_evidence(e,'modern',v.Checks());life=v.Lifecycle('modern',e,packets,v.Checks())
        value,state=v.modern_logout_state(0,1,packets[v.packet_key(v.receipt(ack,'modern'))],e,packets,life)
        self.assertEqual(value['hp'],762);self.assertEqual(value['mp'],49);self.assertEqual(state['items'][100]['payload']['durabilityCurrent'],3999)
        e[-1]['source']['patches'].pop()
        with self.assertRaisesRegex(v.EvidenceError,'patch_list'):v.modern_logout_state(0,1,packets[v.packet_key(v.receipt(ack,'modern'))],e,packets,life)


def native_lifecycle():
    _,r,original,disk=base.synthetic_native_lifecycle();p=plan()
    e=[x for x in original if x.get('kind') not in ['offered','sent','ownerReceipt','peerReceipt','actionOutcome','timeout']
       and not(x.get('kind')=='packet' and 100000<=x.get('monotonicMs',0)<160000)]
    for row in e:
        body=row.get('detail',{});actor=row.get('actorOrdinal')
        if row.get('kind')=='packet' and row.get('packet')=='UserInformation':
            body.update(objectId=50000+actor,position=p['actors'][actor]['start'],hp=1000,mp=50)
            state=body['authoritativeItemState'];state['owned']['slots'].append(copy.deepcopy(native_items(actor)['owned']['slots'][1]));state['fingerprintSha256']=v.digest(v.encoded(state['owned']))
        if row.get('kind')=='packet' and row.get('packet')=='LogOutSuccess' and row.get('connection')==5:
            final=native_items(actor,count=19,dura=3900)
            body.update(lastAuthoritativeOwnerState={'actorOrdinal':actor,'connection':5,'objectId':50000+actor,'mapIndex':440,'position':p['actors'][actor]['start'],'hp':900,'mp':49,'itemState':final})
        if row.get('kind')=='packet' and row.get('packet')=='UserInformation' and row.get('connection')==6:
            body.update(hp=900,mp=49,authoritativeItemState=native_items(actor,count=19,dura=3900))
    for actor in range(2):
        for seq,name,body in [(6,'HealthChanged',{'hpBefore':1000,'hp':900,'mp':49}),(7,'DuraChanged',{'uniqueID':100+actor,'currentDura':3900}),(8,'UseItem',{'uniqueID':200+actor,'grid':1,'success':True,'remainingCountPresentInNativePacket':False})]:
            e.append({'kind':'packet','direction':'received','actorOrdinal':actor,'connection':5,'sequence':seq,'monotonicMs':190000+seq,'packet':name,'detail':body})
    # Raw projections are ordered by their real per-process monotonic clock.
    e.sort(key=lambda x:x.get('monotonicMs',x.get('result',{}).get('startedAtMs',0)))
    r['cleanup']=[{'actorOrdinal':i} for i in range(2)]
    for s in disk['samples']:s.update(position=p['actors'][s['actorOrdinal']]['start'],hp=900,mp=49,itemState=native_items(s['actorOrdinal'],count=19,dura=3900))
    return p,r,e,disk


class NativeFinalTests(unittest.TestCase):
    def test_native_final_fresh_and_all_disk_match_actual_ack_not_initial_hp_or_dura(self):
        p,r,e,d=native_lifecycle();packets,admissions,_=v.index_evidence(e,'crystal',v.Checks())
        result=v.verify_crystal_lifecycle(p,r,e,packets,admissions,d);self.assertTrue(result['valid']);self.assertEqual(result['equipmentStrongChecks'],8)

    def test_native_fresh_hp_and_all_actor_disk_hp_durability_stock_are_exact(self):
        for what in ['fresh','diskHp','diskDura','diskStock']:
            p,r,e,d=native_lifecycle()
            if what=='fresh':next(x for x in e if x.get('packet')=='UserInformation' and x.get('actorOrdinal')==0 and x.get('connection')==6)['detail']['hp']+=1
            elif what=='diskHp':d['samples'][0]['hp']+=1
            else:
                state=d['samples'][0]['itemState'];state['owned']['slots'][0 if what=='diskDura' else 1]['currentDura' if what=='diskDura' else 'count']+=1;state['fingerprintSha256']=v.digest(v.encoded(state['owned']))
            packets,admissions,_=v.index_evidence(e,'crystal',v.Checks())
            with self.assertRaises(v.EvidenceError):v.verify_crystal_lifecycle(p,r,e,packets,admissions,d)


class SourceAndStreamTests(unittest.TestCase):
    def source_fixture(self,directory):
        root=Path(directory);files=[]
        for name in ['driver.mjs','contract.mjs','combat.mjs','melee-driver.mjs']:
            payload=('// synthetic offline source '+name+'\n').encode();(root/name).write_bytes(payload);files.append({'path':name,'bytes':len(payload),'sha256':v.digest(payload)})
        ws=root/'ws.cjs';ws.write_text('// synthetic no network constructor\n')
        manifest={'schema':'mir2.comparable.modern-adapter-sources.v1','entrypoint':'melee-driver.mjs','files':files,'wsModule':{'path':str(ws),'bytes':ws.stat().st_size,'sha256':v.digest(ws.read_bytes())}}
        manifest_path=root/'adapter.json';manifest_path.write_text(json.dumps(manifest));source_sha=v.digest(manifest_path.read_bytes());entry=files[-1]['sha256']
        runtime={'schema':'mir2.comparable.modern-runtime.v1','sourceRevision':'a'*40,'binarySha256':'b'*64,'backend':'file','sessionCache':'in-memory','planSha256':'1'*64,
                 'adapterSourceManifestSha256':source_sha,'adapterEntrypointSha256':entry}
        runtime_path=root/'runtime.json';runtime_path.write_text(json.dumps(runtime));meta={'plan':{'sha256':'1'*64}}
        args=types.SimpleNamespace(adapter_sources=manifest_path,runtime_receipt=runtime_path,native_build=None,server_ready=None,build_manifest=None)
        artifacts=v.read_combat_sources(args,'modern',meta)
        report={'sourceRevision':'a'*40,'binarySha256':'b'*64,'sourceProof':'owned-controller-receipt','sourceRevisionIsBaseOnly':False,'additionalSource':None,
                'adapterSourceManifestSha256':source_sha,'adapterEntrypointSha256':entry,'adapterSource':{k:manifest[k] for k in ['entrypoint','files','wsModule']}}
        report['adapterSource']['dependencyScope']='static-esm-closure-plus-explicit-websocket-module'
        controller={'adapterSourceManifestSha256':source_sha,'driverEntrypointSha256':entry,'adapterSourceBindingStable':True,'adapterSourceProof':{**manifest,'actualCommandEntrypointMatched':True},
                    'runtimeReceiptSha256':meta['runtimeReceipt']['sha256'],'serverBinarySha256':'b'*64}
        return args,report,controller,meta,artifacts

    def test_actual_source_hashes_and_predeclared_launch_binding_required(self):
        with tempfile.TemporaryDirectory() as directory:
            args,report,controller,meta,artifacts=self.source_fixture(directory)
            self.assertTrue(v.verify_combat_sources(report,controller,meta,artifacts,'modern')['valid'])
            for field in ['adapterSourceManifestSha256','driverEntrypointSha256','runtimeReceiptSha256','serverBinarySha256','adapterSourceBindingStable']:
                bad=copy.deepcopy(controller);bad.pop(field)
                with self.assertRaises(v.EvidenceError):v.verify_combat_sources(report,bad,meta,artifacts,'modern')
            (Path(directory)/'combat.mjs').write_text('// changed after freeze')
            with self.assertRaisesRegex(v.EvidenceError,'pin_mismatch'):v.read_combat_sources(args,'modern',{})

    def test_source_escape_static_import_and_ws_digest_fail_closed(self):
        for change in ['escape','import','ws']:
            with tempfile.TemporaryDirectory() as directory:
                args,report,controller,meta,artifacts=self.source_fixture(directory);manifest=json.loads(args.adapter_sources.read_text())
                if change=='escape':manifest['files'][0]['path']='../driver.mjs'
                elif change=='import':
                    path=Path(directory)/'driver.mjs';path.write_text("import './unfrozen.mjs';\n");manifest['files'][0].update(bytes=path.stat().st_size,sha256=v.digest(path.read_bytes()))
                else:manifest['wsModule']['sha256']='0'*64
                args.adapter_sources.write_text(json.dumps(manifest))
                with self.assertRaises(v.EvidenceError):v.read_combat_sources(args,'modern',{})

    def test_overlay_requires_exact_manifest_and_cannot_claim_clean_base(self):
        for schema,kind,timing,proof in [('mir2.comparable.observer-timing-build.v1','observer-timing-only',True,'base-plus-observer-timing-overlay'),
                                       ('mir2.comparable.spectator-scene-build.v1','spectator-scene-projection',False,'base-plus-spectator-scene-projection')]:
            with tempfile.TemporaryDirectory() as directory:
                _,report,controller,meta,artifacts=self.source_fixture(directory)
                build={'schema':schema,'changesKind':kind,'observerTimingOnly':timing,'baseRevision':'31e9b6a74661c31d6515dd7322b466285f975d71','binarySha256':'b'*64,'sourceInputManifestSha256':'d'*64,'buildActualExitCode':0}
                meta['buildManifest']={'sha256':'e'*64};artifacts['buildManifest']=build;controller['buildManifestSha256']='e'*64
                artifacts['runtime'].update(sourceRevision=build['baseRevision'],buildManifestSha256='e'*64,changesKind=kind,sourceInputManifestSha256='d'*64,buildActualExitCode=0)
                report.update(sourceRevision=build['baseRevision'],sourceRevisionIsBaseOnly=True,sourceProof=proof,additionalSource={**{k:build[k] for k in ['changesKind','observerTimingOnly','baseRevision','binarySha256','sourceInputManifestSha256','buildActualExitCode']},
                    'buildManifestSchema':schema,'buildManifestSha256':'e'*64,'cleanBaseBuild':False,'behaviorFixClaimed':False,'capacityClaimAllowed':False})
                self.assertTrue(v.verify_combat_sources(report,controller,meta,artifacts,'modern')['valid'])
                report['additionalSource']['cleanBaseBuild']=True
                with self.assertRaises(v.EvidenceError):v.verify_combat_sources(report,controller,meta,artifacts,'modern')

    def test_combat_stream_retains_unreferenced_conflict_health_stock_and_durability(self):
        p,r,e=actions('modern')
        e.append({'event':'received','actorOrdinal':0,'connection':1,'receiveSeq':99999,'atMs':64001,'packet':'ObjectRangeAttack','payload':{'objectId':999}})
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory)/'events.jsonl';path.write_bytes(b''.join(v.encoded(row)+b'\n' for row in e))
            selected,meta=v.read_events(path,'modern',2)
            self.assertEqual(meta['rows'],len(e));self.assertEqual(meta['sha256'],v.digest(path.read_bytes()))
            self.assertTrue(any(x.get('packet')=='ObjectRangeAttack' for x in selected));self.assertTrue(any(x.get('packet')=='HealthChanged' for x in selected))
            checks=v.Checks();windows,*_=v.verify_actions(p,r,selected,'modern',checks);self.assertFalse(checks.failures);self.assertTrue(all(w['passed'] for w in windows))

    def test_anonymous_prelogin_snapshot_is_not_an_owner_death_or_baseline(self):
        packets={(0,1,1):{'actor':0,'connection':1,'sequence':1,'at':0,'packet':'worldSnapshot','body':{'playerHp':0}},
                 (0,1,2):{'actor':0,'connection':1,'sequence':2,'at':1,'packet':'UserInformation','body':{'objectId':1000,'hp':1000,'mp':50}},
                 (0,1,3):{'actor':0,'connection':1,'sequence':3,'at':2,'packet':'LogOutSuccess','body':{}},
                 (0,1,4):{'actor':0,'connection':1,'sequence':4,'at':3,'packet':'worldSnapshot','body':{'playerHp':0,'playerObjectId':0}}}
        context=v.combat_context(packets)
        self.assertTrue(context['sessions'])

    def test_real_sdk_synthetic_schema_recomputes_both_minutes_but_is_not_live(self):
        directory=Path(tempfile.gettempdir())/'mir2-comparable-capacity-20261113/modern-audit/synthetic-melee-v2'
        p,pm=v.read_file(directory/'sample.plan.json');r,rm=v.read_file(directory/'sample.report.json');e,em=v.read_events(directory/'sample.events.jsonl','modern',2)
        self.assertEqual(pm['sha256'],'8f4d4cab28b5d27a3bb1a177fdbf9b717dce63e57e0eee115243e8c98d4200bd')
        self.assertEqual(rm['sha256'],'d9c1b8945162758604cfa18327adb8f76443ba008936bbf5cd5cd8d24fc954d9')
        self.assertEqual(em['sha256'],'f55af6c165cc02bbdcafba2f4a2dd02202b1e47e35978f483eab22a5747f36cb')
        c=v.Checks();windows,derived,*_=v.verify_actions(p,r,e,'modern',c)
        self.assertFalse(c.failures);self.assertEqual(len(derived),104);self.assertTrue(all(w['passed'] for w in windows))
        self.assertTrue(r['synthetic']);self.assertFalse(r['liveEvidence']);self.assertTrue(e.any_synthetic)

    def test_frozen_A8_actual_source_lock_and_build_schema_can_be_read_without_execution(self):
        root=Path(tempfile.gettempdir())/'mir2-comparable-capacity-20261113/crystal-adapter-final-v8'
        with tempfile.TemporaryDirectory() as directory:
            ready=Path(directory)/'ready.json';ready.write_text('{"synthetic":true}')
            args=types.SimpleNamespace(adapter_sources=root/'source-lock.json',native_build=root/'build-receipt.json',server_ready=ready,runtime_receipt=None,build_manifest=None)
            metadata={};artifacts=v.read_combat_sources(args,'crystal',metadata)
            self.assertEqual(metadata['adapterSources']['sha256'],'abbe839c38b9a8b923e0c59e03bfa203779a13ec24b3eb2c47722cd7452479b9')
            report={'sourceLockSha256':metadata['adapterSources']['sha256'],'adapterSha256':artifacts['build']['adapterSha256'],
                    'frozenSharedSha256':artifacts['verifiedRuntime']['Shared.dll']['sha256'],'serverReadySha256':metadata['serverReady']['sha256']}
            self.assertTrue(v.verify_combat_sources(report,{},metadata,artifacts,'crystal')['valid'])
            report['adapterSha256']='0'*64
            with self.assertRaises(v.EvidenceError):v.verify_combat_sources(report,{},metadata,artifacts,'crystal')

    def test_actual_frozen_modern_source_manifest_and_import_graph_schema(self):
        root=Path(tempfile.gettempdir())/'mir2-comparable-capacity-20261113/modern-adapter-v5';source=root/'adapter-sources.melee.json'
        manifest,source_meta=v.read_file(source)
        self.assertEqual(source_meta['sha256'],'66a546f3acfc9ae2a6645a2da8b3ce0bb9590a3583a4e264d22421155a1372d9')
        with tempfile.TemporaryDirectory() as directory:
            runtime=Path(directory)/'runtime.json';runtime.write_text('{}')
            args=types.SimpleNamespace(adapter_sources=source,native_build=None,server_ready=None,runtime_receipt=runtime,build_manifest=None)
            metadata={};result=v.read_combat_sources(args,'modern',metadata)
            self.assertEqual(result['verifiedFiles'],manifest['files']);self.assertEqual(len(result['verifiedFiles']),4)

    def test_unreferenced_competing_packet_is_still_audited_and_rejected(self):
        p,r,e=actions('modern');health=next(x for x in e if x.get('event')=='received' and x.get('packet')=='HealthChanged')
        boundary=health['receiveSeq']+1;visited=set()
        def shift(value):
            if type(value)is dict:
                if id(value) in visited:return
                visited.add(id(value));actor=value.get('observerOrdinal',value.get('receiverActorOrdinal',value.get('actorOrdinal')))
                if actor==1 and value.get('connection')==1:
                    for key in ['receiveSeq','receiveSequence']:
                        if type(value.get(key))is int and value[key]>=boundary:value[key]+=1
                for child in value.values():shift(child)
            elif type(value)is list:
                for child in value:shift(child)
        shift(e)
        extra={'event':'received','actorOrdinal':1,'connection':1,'receiveSeq':boundary,'atMs':health['atMs']+.1,'packet':'ObjectRangeAttack','payload':{'objectId':999}}
        e.insert(e.index(health)+1,extra)
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory)/'events.jsonl';path.write_bytes(b''.join(v.encoded(row)+b'\n' for row in e))
            selected,_=v.read_events(path,'modern',2)
            with self.assertRaisesRegex(v.EvidenceError,'competing_range_attack'):v.verify_actions(p,r,selected,'modern',v.Checks())


if __name__=='__main__':unittest.main()


if __name__=='__main__':unittest.main()
