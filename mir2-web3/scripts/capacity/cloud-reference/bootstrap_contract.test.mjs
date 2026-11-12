import test from 'node:test';
import assert from 'node:assert/strict';
import { bootstrapEvidence } from './public_signed_probe.mjs';
const name = 'SyntheticSaved';
const packet = (sequence, packet, payload) => ({ sequence, direction: 'received', packet, payload,monotonicMs:sequence*100 });
function complete() {
  return { events: [packet(17, 'StartGame', { result: 4 }), packet(24, 'MapInformation', { fileName: '0' }),
    packet(25, 'UserInformation', { objectId: 1000, name, hp: 18, location: { x: 290, y: 616 } }),
    { sequence: 30, direction: 'received', type: 'worldSnapshot', ownerNameMatches: true,monotonicMs:3000 }],
    snapshot: { mapFileName: '0', playerObjectId: 1000, playerHp: 18, entities: [{ objectId: 1000, name, x: 290, y: 616, dead: false }] } };
}
test('saved owner requires fresh UI, map and complete snapshot in addition to successful StartGame', () => {
  const c = complete(), proof = bootstrapEvidence(c, 16, name);
  assert.equal(proof.ready, true); assert.equal(proof.userSequence, 25); assert.equal(proof.snapshotSequence, 30);
  for (const missing of ['UserInformation', 'MapInformation']) {
    const bad = complete(); bad.events = bad.events.filter(e => e.packet !== missing);
    assert.equal(bootstrapEvidence(bad, 16, name).ready, false);
  }
});
test('pre-start or pre-UI snapshot is never reused as a complete bootstrap', () => {
  const c = complete(); c.events.at(-1).sequence = 12;
  assert.equal(bootstrapEvidence(c, 16, name).ready, false);
  c.events.at(-1).sequence = 20;
  assert.equal(bootstrapEvidence(c, 16, name).ready, false);
  c.events.push({ sequence: 31, direction: 'received', type: 'worldSnapshot', ownerNameMatches: true,monotonicMs:3100 });
  assert.equal(bootstrapEvidence(c, 16, name).ready, true);
});
test('wrong wire owner name, HP, position or map fails even with a success packet and snapshot', () => {
  const mutations = [c => { c.events[2].payload.name = 'OtherOwner'; }, c => { c.snapshot.playerHp = 0; },
    c => { c.snapshot.entities[0].x = 291; }, c => { c.snapshot.mapFileName = '1'; }];
  for (const mutate of mutations) { const c = complete(); mutate(c); assert.equal(bootstrapEvidence(c, 16, name).ready, false); }
});
test('legitimate shared-Zone Join correction supersedes the personal initial position with explicit fresh proof', () => {
  const c = complete();
  c.snapshot.entities[0].x = 287; c.snapshot.entities[0].y = 617;
  c.events.splice(3, 0, packet(29, 'UserLocation', { location: { x: 287, y: 617 } }));
  const proof = bootstrapEvidence(c, 16, name);
  assert.equal(proof.ready, true); assert.equal(proof.authoritativePositionAdjusted, true);
  assert.equal(proof.positionCorrectionSequence, 29); assert.equal(proof.positionDeltaX, -3); assert.equal(proof.positionDeltaY, 1);
});
test('position differences without a matching fresh Zone correction or with changed owner ID still fail', () => {
  for (const correction of [packet(20, 'UserLocation', { location: { x: 287, y: 617 } }),
    packet(29, 'UserLocation', { location: { x: 286, y: 617 } })]) {
    const c = complete(); c.snapshot.entities[0].x = 287; c.snapshot.entities[0].y = 617; c.events.splice(3, 0, correction);
    assert.equal(bootstrapEvidence(c, 16, name).ready, false);
  }
  const c=complete();c.snapshot.playerObjectId=1001;c.snapshot.entities[0].objectId=1001;
  assert.equal(bootstrapEvidence(c,16,name).ready,false);
});
test('partial bootstrap with UI but no owned entity remains pending without inventing a correction', () => {
  const c = complete();
  c.snapshot.entities = [];
  c.events = c.events.filter(e => e.type !== 'worldSnapshot');
  const proof = bootstrapEvidence(c, 16, name);
  assert.equal(proof.ready, false);
  assert.equal(proof.authoritativePositionAdjusted, false);
  assert.equal(proof.positionCorrectionSequence, 0);
  c.snapshot = null;
  assert.equal(bootstrapEvidence(c, 16, name).ready, false);
});

test('complete bootstrap proof carries the last required fresh receipt time and never invents a missing receipt timestamp',()=>{
  const c=complete();assert.equal(bootstrapEvidence(c,16,name).monotonicMs,3000);
  c.snapshot.entities[0].x=287;c.snapshot.entities[0].y=617;
  c.events.splice(3,0,packet(29,'UserLocation',{location:{x:287,y:617}}));
  let proof=bootstrapEvidence(c,16,name);assert.equal(proof.ready,true);assert.equal(proof.monotonicMs,3000);
  delete c.events[3].monotonicMs;proof=bootstrapEvidence(c,16,name);
  assert.equal(proof.ready,true);assert.equal(proof.monotonicMs,null);
});
