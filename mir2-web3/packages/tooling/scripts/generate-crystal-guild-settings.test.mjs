import { test } from 'node:test';
import assert from 'node:assert/strict';
import { parseGuildSettings, serializeGuildSettings } from './generate-crystal-guild-settings.mjs';

test('Int64 experience remains exact and source sentinels stop arrays', () => {
  const rules = parseGuildSettings(Buffer.from(`[Guilds]
ExpRate=0.01
[Exp]
Level-0=999999999999999999
Level-1=-1
Level-2=99
[Cap]
Level-0=5
Level-1=-1
[Required-0]
ItemName=
Amount=1000000
[Required-1]
ItemName=WoomaHorn
Amount=1
[Required-2]
Amount=0
[Required-3]
Amount=10
`));
  assert.deepEqual(rules.experienceLevels, [999999999999999999n]);
  assert.deepEqual(rules.memberCaps, [5]);
  assert.equal(rules.creationCosts.length, 2);
  assert.equal(rules.creationCosts[0].itemName, '');
  assert.match(serializeGuildSettings(rules), /999999999999999999/);
  assert.doesNotMatch(serializeGuildSettings(rules), /1000000000000000000/);
});

test('malformed or out of range server rules are rejected', () => {
  for (const text of ['[Exp]\nLevel-0=9223372036854775808',
    '[Guilds]\nExpRate=NaN', '[Guilds]\nExpRate=1e39', '[Guilds]\nMinimumLevel=256',
    '[Required-0]\nAmount=-1', '[Cap]\nLevel-0=-2']) {
    assert.throws(() => parseGuildSettings(Buffer.from(text)));
  }
  assert.throws(() => serializeGuildSettings({ itemName: '__I64_100__' }));
});

test('missing optional keys retain Crystal field defaults', () => {
  const rules = parseGuildSettings(Buffer.from('[Guilds]\n'));
  assert.equal(rules.pointsPerLevel, 0);
  assert.equal(rules.warCost, 3000);
  assert.equal(rules.minimumLevel, 22);
  assert.equal(rules.experienceRate, 0.01);
  assert.deepEqual(rules.creationCosts, []);
});
