// Pure synthetic contract checks. No product import, run read or service.
import assert from 'node:assert/strict';
import {createHpProofCounter,HP_STATUSES} from './hp-proof-contract-v2.mjs';
const dose={type:'nativeHpRecovery',actor:'synthetic',phase:'stage-2',at:1000,uniqueId:7,grid:'inventory',hpBefore:5,maximumHp:18};
const sent={...dose,status:'sent'};
const consumed={...dose,at:1100,status:'verified',acknowledgement:{sequence:11,monotonicMs:100},consumed:true,consumption:{sequence:12,monotonicMs:101,quantity:0},hpIncreaseObserved:false,hpIncrease:null};
const healed={...consumed,consumed:false,consumption:null,hpIncreaseObserved:true,hpIncrease:{sequence:12,monotonicMs:101,hp:12}};
const checks=[];function test(name,fn){fn();checks.push({name,passed:true});}
function verify(row){const c=createHpProofCounter();c.observe(sent);c.observe(row);return c.summary();}
test('verified-consumed requires matched ack and structured quantity proof',()=>{const s=verify(consumed);assert.equal(s.verifiedEventsChecked,1);assert.equal(s.successProofChecked,true);assert.equal(s.noVerifiedDoseObserved,false);assert.equal(s.observedStatuses.verified,1);});
test('verified-healed requires actual HP increase',()=>{assert.equal(verify(healed).verifiedEventsChecked,1);});
test('verified can independently contain both effects',()=>{assert.equal(verify({...consumed,hpIncreaseObserved:true,hpIncrease:healed.hpIncrease}).verifiedEventsChecked,1);});
test('verified without either proof is rejected',()=>assert.throws(()=>verify({...consumed,consumed:false,consumption:null})));
test('consumption Boolean alone does not provide proof',()=>assert.throws(()=>verify({...consumed,consumption:null})));
test('healed proof must actually exceed previous HP',()=>assert.throws(()=>verify({...healed,hpIncrease:{sequence:12,monotonicMs:101,hp:5}})));
test('acknowledgement is mandatory and bounded scalar proof',()=>assert.throws(()=>verify({...consumed,acknowledgement:{sequence:0,monotonicMs:100}})));
test('verified event without actually recorded sent is rejected',()=>assert.throws(()=>createHpProofCounter().observe(consumed)));
test('unknown success status is rejected rather than mistaken for verified',()=>{assert.ok(!HP_STATUSES.includes('success'));assert.throws(()=>verify({...consumed,status:'success'}));});
test('zero events retain dictionary and explicitly no real verified dose',()=>{const s=createHpProofCounter().summary();assert.deepEqual(s.observedStatuses,{});assert.equal(s.successProofChecked,false);assert.equal(s.noVerifiedDoseObserved,true);assert.equal(s.verifiedEventsChecked,0);});
test('ordinary not-sent fixed reason is accepted without dose credit',()=>{const c=createHpProofCounter();c.observe({type:'nativeHpRecovery',actor:'synthetic',phase:'stage-2',at:1,status:'not-sent',reason:'no-held-hp-item'});assert.equal(c.summary().verifiedEventsChecked,0);});
test('failure terminal closes pending without creating success',()=>{const c=createHpProofCounter();c.observe(sent);c.observe({...dose,at:1100,status:'timeout'});assert.equal(c.summary().pendingSentWithoutTerminal,0);assert.equal(c.summary().successProofChecked,false);});
test('proof events inconsistent with a shared2s capture are rejected',()=>assert.throws(()=>verify({...consumed,consumption:{sequence:12,monotonicMs:2101,quantity:0}})));
console.log(JSON.stringify({schemaVersion:1,pureSynthetic:true,productTestsRun:false,runDataRead:false,tests:checks.length,passed:checks.length,failed:0,checks}));
