import assert from 'node:assert/strict';

export const HP_STATUSES=Object.freeze(['not-sent','sent','send-failed','verified','cancelled','owner-changed','owner-dead','public-error','rejected','timeout']);
const REASONS=new Set(['unknown-exact-health','no-held-hp-item','command-budget','helper-declined']);
const COMMON=['type','actor','phase','at','status'];
const DOSE=['uniqueId','grid','hpBefore','maximumHp'];
const VERIFIED=['acknowledgement','consumed','consumption','hpIncreaseObserved','hpIncrease'];
function keys(value,required){assert.ok(value&&typeof value==='object'&&!Array.isArray(value));assert.deepEqual(Object.keys(value).sort(),[...required].sort());}
function proof(value,extra){keys(value,['sequence','monotonicMs',...extra]);assert.ok(Number.isSafeInteger(value.sequence)&&value.sequence>0);assert.ok(Number.isFinite(value.monotonicMs)&&value.monotonicMs>=0);}
const key=r=>JSON.stringify([r.actor,r.uniqueId,r.grid]);
export function createHpProofCounter(){
 const observedStatuses={},pending=new Map();let verifiedEventsChecked=0,totalRows=0;
 return{
  observe(row){
   assert.equal(row.type,'nativeHpRecovery');assert.ok(HP_STATUSES.includes(row.status),'Unknown HP maintenance status');
   assert.ok(typeof row.actor==='string'&&row.actor.length>0&&row.actor.length<=80&&typeof row.phase==='string'&&row.phase.length<=80);
   assert.ok(Number.isSafeInteger(row.at)&&row.at>=0);
   if(row.status==='not-sent'){
    keys(row,[...COMMON,'reason']);assert.ok(REASONS.has(row.reason),'Unknown non-send reason');
   }else{
    keys(row,[...COMMON,...DOSE,...(row.status==='verified'?VERIFIED:[])]);
    assert.ok(Number.isSafeInteger(row.uniqueId)&&row.uniqueId>=0&&['belt','inventory'].includes(row.grid));
    assert.ok(Number.isFinite(row.hpBefore)&&row.hpBefore>0&&Number.isFinite(row.maximumHp)&&row.maximumHp>0&&row.hpBefore<=row.maximumHp);
    if(row.status==='sent'){
     assert.ok(!pending.has(key(row)),'Duplicate HP sent row before terminal');assert.ok(pending.size<100);pending.set(key(row),{at:row.at,hpBefore:row.hpBefore,maximumHp:row.maximumHp});
    }else if(row.status==='verified'){
     const sent=pending.get(key(row));assert.ok(sent,'Verified HP row has no matching actually recorded sent row');
     assert.equal(sent.hpBefore,row.hpBefore);assert.equal(sent.maximumHp,row.maximumHp);assert.ok(row.at>=sent.at);
     proof(row.acknowledgement,[]);
     assert.equal(typeof row.consumed,'boolean');assert.equal(typeof row.hpIncreaseObserved,'boolean');
     assert.equal(row.consumed,row.consumption!==null);assert.equal(row.hpIncreaseObserved,row.hpIncrease!==null);
     assert.ok(row.consumed||row.hpIncreaseObserved,'Verified HP row has no consumption or HP proof');
     const proofs=[row.acknowledgement];
     if(row.consumed){proof(row.consumption,['quantity']);assert.ok(Number.isSafeInteger(row.consumption.quantity)&&row.consumption.quantity>=0);proofs.push(row.consumption);}
     if(row.hpIncreaseObserved){proof(row.hpIncrease,['hp']);assert.ok(Number.isFinite(row.hpIncrease.hp)&&row.hpIncrease.hp>row.hpBefore);proofs.push(row.hpIncrease);}
     // Necessary consistency only: source captures each proof inside its own
     // 2s send deadline, but sendMono/cursor and quantityBefore are not logged.
     const times=proofs.map(p=>p.monotonicMs);assert.ok(Math.max(...times)-Math.min(...times)<=2000,'Proofs exceed a common possible two-second capture window');
     pending.delete(key(row));verifiedEventsChecked++;
    }else if(row.status!=='send-failed'){
     assert.ok(pending.has(key(row)),'HP failure terminal has no recorded sent row');pending.delete(key(row));
    }
   }
   totalRows++;observedStatuses[row.status]=(observedStatuses[row.status]??0)+1;
  },
  summary(){return{observedStatuses:{...observedStatuses},statusValidationChecked:true,totalRows,verifiedEventsChecked,successProofChecked:verifiedEventsChecked>0,noVerifiedDoseObserved:verifiedEventsChecked===0,pendingSentWithoutTerminal:pending.size,notContinuousHpCoverage:true,claim:'Only actual verified rows with matching sent/acknowledgement and effect proof are checked. Zero verified events claim neither a real dose nor survival.',proofLimitations:['The raw producer does not persist send monotonic time, receipt cursor or quantityBefore. This verifier cannot independently reconstruct the original freshness deadline or the numeric inventory decrease.','Consumed and increased HP remain distinct observations. A consumption proof alone does not claim healing.','No HP event or diagnostic proof contributes to native opportunity, launched attack, dual hit, ACK or original gameplay acceptance.']};}
 };
}
