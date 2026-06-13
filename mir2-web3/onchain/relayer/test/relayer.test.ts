import test from 'node:test';
import assert from 'node:assert/strict';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { rmSync } from 'node:fs';
import { normalize } from '../src/normalize';
import { DedupStore } from '../src/dedup';
import type { ChainEvent } from '../src/types';

// SK1: plain Sui events — the event NAME is in `type` and `parsedJson` is the struct fields.
function event(name: string, fields: Record<string, unknown>, id = { txDigest: 'TX1', eventSeq: '4' }): ChainEvent {
  return {
    id,
    type: `0xpkg::mine_system::${name}`,
    parsedJson: fields,
  };
}

const settledFields = {
  miner: '0xabc',
  mine_id: '1',
  swings: '5',
  ore_kind: { variant: 'BlackIron', fields: {}, type: '0xpkg::ore_kind::OreKind' },
  ore_amount: '5',
  stones_left: 5,
  fee_paid: '0',
  nonce: '1',
};

test('MineSettledEvent -> GrantOnchainOre with sui: account + idempotency key', () => {
  const cmd = normalize(event('MineSettledEvent', settledFields));
  assert.equal(cmd?.kind, 'GrantOnchainOre');
  assert.ok(cmd && cmd.kind === 'GrantOnchainOre');
  assert.equal(cmd.account, 'sui:0xabc');
  assert.equal(cmd.oreKind, 'BlackIron');
  assert.equal(cmd.amount, '5');
  assert.equal(cmd.mineId, '1');
  assert.equal(cmd.stonesLeft, 5);
  assert.equal(cmd.nonce, '1');
  assert.equal(cmd.idempotencyKey, 'TX1:4');
});

test('dry mine_settled (ore_amount 0) is STILL forwarded (the client settlement signal)', () => {
  // M4: the sim re-broadcasts the vein from stones_left even for a zero-ore settlement,
  // and the client closes its in-flight batch on that packet — dropping dry batches here
  // would wedge the client's batcher.
  const cmd = normalize(
    event('MineSettledEvent', { ...settledFields, ore_amount: '0', stones_left: 0 }),
  );
  assert.ok(cmd && cmd.kind === 'GrantOnchainOre');
  assert.equal(cmd.amount, '0');
  assert.equal(cmd.stonesLeft, 0);
});

test('ore_kind variant forms: plain string and single-key object both parse', () => {
  const a = normalize(event('MineSettledEvent', { ...settledFields, ore_kind: 'Gold' }));
  assert.ok(a && a.kind === 'GrantOnchainOre');
  assert.equal(a.oreKind, 'Gold');
  const b = normalize(event('MineSettledEvent', { ...settledFields, ore_kind: { Silver: {} } }));
  assert.ok(b && b.kind === 'GrantOnchainOre');
  assert.equal(b.oreKind, 'Silver');
});

test('MineDepletedEvent -> MineDepleted', () => {
  const cmd = normalize(event('MineDepletedEvent', { mine_id: '1' }));
  assert.ok(cmd && cmd.kind === 'MineDepleted');
  assert.equal(cmd.mineId, '1');
});

test('OreRedeemedEvent -> CreditGoldFromOre (rate applied downstream at M5)', () => {
  const cmd = normalize(event('OreRedeemedEvent', { miner: '0xdef', ore_kind: { variant: 'BlackIron' }, amount: '3' }));
  assert.ok(cmd && cmd.kind === 'CreditGoldFromOre');
  assert.equal(cmd.account, 'sui:0xdef');
  assert.equal(cmd.oreKind, 'BlackIron');
  assert.equal(cmd.oreAmount, '3');
});

test('non-bridged event (MineRegenedEvent) -> null', () => {
  assert.equal(normalize(event('MineRegenedEvent', { mine_id: '1', stones_left: 10 })), null);
});

test('event without parsedJson -> null', () => {
  assert.equal(normalize({ id: { txDigest: 'T', eventSeq: '0' }, type: 't', parsedJson: null }), null);
});

test('dedup: claim is idempotent (first true, then false)', () => {
  const dedup = new DedupStore();
  assert.equal(dedup.claim('TX1:4'), true);
  assert.equal(dedup.claim('TX1:4'), false);
  assert.equal(dedup.isNew('TX1:4'), false);
  assert.equal(dedup.isNew('TX1:5'), true);
  assert.equal(dedup.size(), 1);
});

test('dedup: survives persist + reload (no-dup across restart)', () => {
  const path = join(tmpdir(), `mir2-relayer-dedup-${process.pid}.json`);
  try {
    const a = new DedupStore(path);
    assert.equal(a.claim('TXa:1'), true);
    assert.equal(a.claim('TXb:2'), true);
    a.persist();
    const b = new DedupStore(path); // simulate restart
    assert.equal(b.claim('TXa:1'), false); // already seen
    assert.equal(b.claim('TXb:2'), false);
    assert.equal(b.claim('TXc:3'), true);
    assert.equal(b.size(), 3);
  } finally {
    rmSync(path, { force: true });
  }
});
