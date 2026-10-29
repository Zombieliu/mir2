export const ACTION_BLOCKING_POISON_MASK = 8 | 16 | 32 | 256;

/** Return the authoritative poison/status bits that block movement and attacks. */
export function selfActionBlockMask(client) {
  const snapshot = client?.snapshot;
  const actor = (snapshot?.entities ?? []).find(entity =>
    Number(entity?.objectId) === Number(snapshot?.playerObjectId));
  return Number(actor?.poison ?? snapshot?.playerPoison ?? 0) & ACTION_BLOCKING_POISON_MASK;
}
