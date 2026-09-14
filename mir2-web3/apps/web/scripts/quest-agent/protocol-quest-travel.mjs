const SABUK_MERCHANT_QUEST_IDS = new Set([110, 111, 112]);
const SABUK_MAP = '3';
const SABUK_SECRET_GATE = 'D701';
const SABUK_SAFE_OUTER_ENTRANCE = Object.freeze({ x: 564, y: 287 });
const SABUK_INNER_EXIT = Object.freeze({ x: 171, y: 132 });

function player(snapshot) {
  const objectId = Number(snapshot?.playerObjectId);
  return (snapshot?.entities ?? []).find(entity =>
    Number(entity?.objectId) === objectId ||
    String(entity?.kind ?? '').toLowerCase() === 'selfplayer'
  ) ?? null;
}

function isInsideSabukMerchantGate(snapshot) {
  if (String(snapshot?.mapFileName ?? '') !== SABUK_MAP) return false;
  const actor = player(snapshot);
  return actor != null && Number(actor.x) >= 640 && Number(actor.y) <= 330;
}

function needsSabukMerchantRoute(quest, phase) {
  const questId = Number(quest?.questId);
  if (!SABUK_MERCHANT_QUEST_IDS.has(questId)) return false;
  const npc = phase === 'start' ? quest?.startNpc : quest?.finishNpc;
  return String(npc?.mapFileName ?? '') === SABUK_MAP;
}

/**
 * Reach a quest NPC with ordinary Crystal walking transfers. Sabuk's southern
 * approach is guarded by invulnerable ArcherGuards, while its D701 secret-gate
 * exit lands inside the merchant quarter. The detour therefore enters D701,
 * walks to its north-east return portal, and comes back inside the wall.
 */
export async function travelToQuestNpc(client, quest, phase, { travel, navigate } = {}) {
  if (!client?.snapshot) throw new TypeError('client must provide an authoritative snapshot');
  if (!['start', 'finish'].includes(phase)) throw new TypeError('phase must be start or finish');
  if (typeof travel !== 'function') throw new TypeError('travel must be a function');
  if (typeof navigate !== 'function') throw new TypeError('navigate must be a function');

  const npc = phase === 'start' ? quest?.startNpc : quest?.finishNpc;
  if (!npc?.mapFileName) return { status: 'noNpc', questId: Number(quest?.questId), phase };
  if (!needsSabukMerchantRoute(quest, phase) || isInsideSabukMerchantGate(client.snapshot)) {
    await travel(String(npc.mapFileName));
    return { status: 'direct', questId: Number(quest?.questId), phase };
  }

  // D701 has three entrances on map 3. Choosing only by tile distance can
  // select the eastern entrance behind Sabuk's ArcherGuard line. Pin the
  // ordinary western secret-gate tile. When arriving from another province,
  // use Crystal's paid transporter first: it lands at 361,342 and avoids the
  // southern walking entrance and the ArcherGuard corridor.
  const currentMap = String(client.snapshot.mapFileName ?? '');
  if (currentMap === SABUK_SECRET_GATE) {
    await travel(SABUK_MAP, { preferredTransferSource: SABUK_INNER_EXIT });
  } else {
    if (currentMap !== SABUK_MAP) {
      await travel(SABUK_MAP, { preferDirectScriptedEdge: true });
    }
    await travel(SABUK_SECRET_GATE, { preferredTransferSource: SABUK_SAFE_OUTER_ENTRANCE });
    // Let the map traveler own the whole inner crossing as well. D701 contains
    // ordinary Zombies that can seal its narrow corridor; the traveler can
    // clear a proven blocker and retry, whereas a bare navigation call cannot.
    await travel(SABUK_MAP, { preferredTransferSource: SABUK_INNER_EXIT });
  }
  if (!isInsideSabukMerchantGate(client.snapshot)) {
    throw new Error(
      `q${Number(quest?.questId)} ${phase} secret-gate route did not enter the Sabuk merchant quarter`,
    );
  }
  return {
    status: 'sabukSecretGate',
    questId: Number(quest?.questId),
    phase,
    via: SABUK_SECRET_GATE,
  };
}

export const SABUK_QUEST_TRAVEL = Object.freeze({
  mapFileName: SABUK_MAP,
  secretGateMapFileName: SABUK_SECRET_GATE,
  safeOuterEntrance: SABUK_SAFE_OUTER_ENTRANCE,
  innerExit: SABUK_INNER_EXIT,
});
