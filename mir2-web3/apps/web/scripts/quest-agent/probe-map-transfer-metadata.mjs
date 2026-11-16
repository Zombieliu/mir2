// Ordinary local protocol smoke on an isolated, unmodified copy of a saved
// character near the real Bichon/DeadMine entrance. Never writes the source store.
import fs from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
import { ProtocolClient, delay, startGameBootstrapEvidence } from './protocol-client.mjs';
import { findProtocolWalkPath, loadProtocolCollisionMap, protocolWalkSteps } from './protocol-navigation.mjs';

const [endpoint, output, characterIndex] = process.argv.slice(2);
if (!endpoint || !output || !Number.isSafeInteger(Number(characterIndex))) {
  throw new Error('Usage: probe-map-transfer-metadata.mjs LOCAL_WS OUTPUT CHARACTER_INDEX');
}
const accountId = process.env.MIR2_PROBE_ACCOUNT;
const password = process.env.MIR2_PROBE_PASSWORD;
if (!accountId || !password) throw new Error('MIR2_PROBE_ACCOUNT and MIR2_PROBE_PASSWORD are required');
await fs.mkdir(output, { recursive: true });
const manifest = JSON.parse(await fs.readFile(new URL('../../lib/generated/crystal_respawn_manifest.json', import.meta.url)));
const client = new ProtocolClient(endpoint, path.join(output, 'protocol.jsonl'));
const report = { status: 'running', ordinaryProtocol: true, isolatedStoreCopy: true,
  liveUserStoreWritten: false, visualAccepted: false, transfers: [], commands: 0, logoutSuccess: false };
const self = () => client.snapshot?.entities?.find(e => String(e.objectId) === String(client.snapshot?.playerObjectId));
const deadline = Date.now() + 90_000;
let firstInGameSequence = 0;
try {
  await client.connect();
  await client.request({ type: 'login', accountId, password }, 'LoginSuccess');
  const beforeStart = client.sequence;
  client.send({ type: 'startGame', characterIndex: Number(characterIndex) });
  await client.wait(() => startGameBootstrapEvidence(client, beforeStart) && self()?.x != null, 'game bootstrap');
  firstInGameSequence = beforeStart;
  report.initial = { fileName: client.snapshot.mapFileName, x: self().x, y: self().y };
  const routes = report.initial.fileName === 'D401' ? ['0', 'D401'] : ['D401', '0'];
  assert.ok(['0', 'D401'].includes(report.initial.fileName), 'copy must be near this ordinary entrance');
  for (const destination of routes) {
    const source = client.snapshot.mapFileName;
    const target = source === 'D401' ? { x: 24, y: 182 } : { x: 663, y: 215 };
    const map = await loadProtocolCollisionMap(source);
    const beforeTransfer = client.sequence;
    while (client.snapshot.mapFileName === source) {
      assert.ok(Date.now() < deadline && report.commands < 40, 'ordinary smoke budget exhausted');
      assert.ok(self() && !self().dead && self().hp !== 0, 'player must remain alive');
      const from = { x: self().x, y: self().y };
      const obstacles = (client.snapshot.entities ?? []).filter(e =>
        String(e.objectId) !== String(client.snapshot.playerObjectId) && !e.dead &&
        Number.isFinite(e.x) && Number.isFinite(e.y));
      const route = findProtocolWalkPath({ map, start: from, target,
        dynamicObstacles: obstacles, staticWalkableOverrides: [target] });
      assert.ok(route?.length > 1, 'real entrance must have a collision-valid next step');
      const [step] = protocolWalkSteps(route);
      await delay(650);
      await client.request({ type: 'walk', direction: step.direction.replace('+', '') }, 'UserLocation', 5000);
      report.commands += 1;
      await delay(50);
    }
    assert.equal(client.snapshot.mapFileName, destination);
    const packet = client.events.find(e => e.sequence > beforeTransfer && e.packet === 'MapInformation');
    assert.ok(packet, 'ordinary entrance must emit a map packet');
    const expected = manifest.maps.find(m => m.map_file_name === destination);
    const actual = packet.payload;
    const expectedFields = { mapIndex: expected.map_index, miniMapIndex: expected.mini_map,
      bigMapIndex: expected.big_map, lights: expected.light, music: expected.music };
    const mismatches = Object.entries(expectedFields).filter(([key, value]) => actual[key] !== value)
      .map(([field, expected]) => ({ field, expected, actual: actual[field] }));
    client.send({ type: 'clientVersion' });
    await client.wait(() => client.events.find(e => e.sequence > packet.sequence &&
      e.type === 'worldSnapshot' && e.payload?.mapFileName === destination), 'destination snapshot');
    report.transfers.push({ source, destination, sequence: packet.sequence,
      actual: Object.fromEntries(['mapIndex', 'fileName', 'title', 'miniMapIndex', 'bigMapIndex', 'lights', 'music'].map(k => [k, actual[k]])),
      expected: expectedFields, mismatches });
  }
  report.status = report.transfers.every(t => t.mismatches.length === 0) ? 'passed' : 'metadata_mismatch';
} catch (error) {
  report.status = 'stopped'; report.error = String(error?.stack ?? error);
} finally {
  await client.close();
  report.logoutSuccess = client.events.some(e => e.sequence > firstInGameSequence && e.packet === 'LogOutSuccess');
  await fs.writeFile(path.join(output, 'report.json'), JSON.stringify(report, null, 2));
  console.log(JSON.stringify(report, null, 2));
  if (report.status !== 'passed' || !report.logoutSuccess) process.exitCode = 1;
}
