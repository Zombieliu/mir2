import test from 'node:test';
import assert from 'node:assert/strict';
import { preparationAttackReadiness } from './public_signed_probe.mjs';
test('all four observed early attacks remain inside the real600ms movement lock plus50ms cushion',()=>{
  const ackAt=10000;
  for(const observedAge of [84.023,160.069,42.912,44.691]){
    const r=preparationAttackReadiness({lastMovementAckAt:ackAt,movementKind:'walk'});
    assert.equal(r.movementCooldownMs,600);assert.equal(r.readyAtMs,10650);assert.equal(ackAt+observedAge<r.readyAtMs,true);
  }
});
test('Slow uses1200ms movement cooldown and turns use their separate native delay',()=>{
  assert.equal(preparationAttackReadiness({lastMovementAckAt:100,movementKind:'run',poison:4}).readyAtMs,1350);
  assert.equal(preparationAttackReadiness({lastMovementAckAt:100,movementKind:'turn'}).readyAtMs,500);
});
test('preparation respects both the movement ACK and previous attack clock without changing due-time scheduling',()=>{
  assert.equal(preparationAttackReadiness({lastMovementAckAt:100,lastAttackAt:1000,attackIntervalMs:1600}).readyAtMs,2600);
  assert.equal(preparationAttackReadiness({lastMovementAckAt:3000,lastAttackAt:1000,attackIntervalMs:1600}).readyAtMs,3650);
});
