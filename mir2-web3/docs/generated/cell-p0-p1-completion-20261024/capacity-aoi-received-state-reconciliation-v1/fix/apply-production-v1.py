from pathlib import Path
import json,hashlib
N=Path(__file__).parent;P=Path('C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey/mir2-web3/apps/web/scripts')
def pin(p):
 b=p.read_bytes();return dict(file=str(p.resolve()),bytes=len(b),sha256=hashlib.sha256(b).hexdigest())
method='''  reconcileReceived(clients, changedActor) {
    if (!this.missingSince.size && !this.extraSince.size) return;
    const pending = [];
    for (const clocks of [this.missingSince, this.extraSince]) {
      for (const key of clocks.keys()) {
        const [receiverName, ownerName] = key.split(':');
        if (receiverName === changedActor || ownerName === changedActor) pending.push({ clocks, key, receiverName, ownerName });
      }
    }
    if (!pending.length) return;
    const cohort = new Map(clients.map(client => [client.label, client])), entriesByReceiver = new Map();
    const live = client => client?.inGame && !client.closed && !client.paused && self(client);
    for (const { clocks, key, receiverName, ownerName } of pending) {
      const receiver = cohort.get(receiverName), owner = cohort.get(ownerName);
      if (!live(receiver) || !live(owner)) { clocks.delete(key); continue; }
      const shouldSee = receiver.snapshot.mapFileName === owner.snapshot.mapFileName && distance(self(receiver), self(owner)) <= 16;
      const missing = clocks === this.missingSince;
      if (missing ? !shouldSee : shouldSee) { clocks.delete(key); continue; }
      let counts = entriesByReceiver.get(receiverName);
      if (!counts) {
        counts = new Map();
        for (const entity of receiver.snapshot.entities) {
          if (['player', 'remoteplayer', 'selfplayer'].includes(String(entity.kind).toLowerCase())) counts.set(entity.name, (counts.get(entity.name) ?? 0) + 1);
        }
        entriesByReceiver.set(receiverName, counts);
      }
      const count = counts.get(ownerName) ?? 0;
      if (missing ? count === 1 : count === 0) clocks.delete(key);
    }
  }
'''
rows=[]
for name,before,after in [
 ('playtest-capacity-soak-core.mjs','  sample(clients, now = performance.now()) {',method+'  sample(clients, now = performance.now()) {'),
 ('playtest-capacity-soak.mjs','    super.observe(message);','    super.observe(message);\n    this.context.aoi?.reconcileReceived(this.context.clients ?? [], this.label);'),
]:
 p=P/name;old=p.read_bytes();a=before.encode();b=after.encode();assert old.count(a)==1;new=old.replace(a,b);assert new.replace(b,a)==old;p.write_bytes(new);rows.append(dict(file=name,before=before,after=after,preimage=pin(N/'preimages'/(name+'.txt')),current=pin(p),inverseRawExact=True))
# This newly appended assertion checks no statistical credit, independent of prior poll accumulation.
p=P/'playtest-capacity-soak.test.mjs';s=p.read_bytes();a=b"  b.snapshot.entities[0].x = 17; coverage.reconcileReceived?.(clients, 'b'); assert.equal(coverage.missingSince.has('a:b'), false);\n  assert.equal(coverage.stats.observedPairs, 1); // Reconciliation gives no visibility credit or sample."
b=b"  const priorStats = { ...coverage.stats };\n  b.snapshot.entities[0].x = 17; coverage.reconcileReceived?.(clients, 'b'); assert.equal(coverage.missingSince.has('a:b'), false);\n  assert.deepEqual(coverage.stats, priorStats); // Reconciliation gives no visibility credit or sample."
assert s.count(a)==1;s=s.replace(a,b);p.write_bytes(s)
with (N/'production-mutation-v1.json').open('x') as f:json.dump(dict(schemaVersion=1,rows=rows,onlyClearExistingClocks=True,range16Unchanged=True,grace1000AndStrictGreaterUnchanged=True,noStatsOrVisibilityCredit=True),f,indent=2)
print(json.dumps(rows,indent=2))
