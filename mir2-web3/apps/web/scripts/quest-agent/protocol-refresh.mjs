import { observedPlayerHp } from './protocol-observation.mjs';

/** One snapshot probe, bounded to cover the measured 10.61-second save stall. */
export async function refreshCombatWorldSnapshot(client, { timeoutMs = 20_000 } = {}) {
  const afterSequence = Number(client.sequence ?? 0);
  client.send({ type: 'clientVersion' });
  const died = () => (client.snapshot?.entities ?? []).some(entity =>
    Number(entity.objectId) === Number(client.snapshot?.playerObjectId) && entity.dead === true) ||
    observedPlayerHp(client.snapshot) <= 0;
  await client.wait(() => died() || (client.events ?? []).some(event =>
    Number(event.sequence) > afterSequence && event.direction === 'received' && event.type === 'worldSnapshot'),
  'combat cooldown worldSnapshot', timeoutMs);
  if (died()) throw new Error('Player died during quest combat');
}
