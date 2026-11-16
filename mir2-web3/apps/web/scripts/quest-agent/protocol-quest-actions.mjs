let requestSequence = 0;
const NPC_INTERACTION_RANGE = 16;

const normalizedStage = value => String(value ?? '').replace(/[^a-z]/gi, '').toLowerCase();

function actionDetails(action, routeQuest) {
  const kind = typeof action === 'string' ? action : action?.type ?? action?.action;
  if (!['accept', 'finish'].includes(kind)) throw new Error(`Unsupported quest action: ${kind}`);
  const selectedItemIndex = kind === 'finish'
    ? Number(action?.selectedItemIndex ?? routeQuest?.selectedItemIndex ?? -1)
    : -1;
  if (!Number.isSafeInteger(selectedItemIndex) || selectedItemIndex < -1) {
    throw new Error(`Invalid selected quest reward index: ${selectedItemIndex}`);
  }
  return { kind, selectedItemIndex };
}

function questIdOf(routeQuest) {
  const questId = Number(routeQuest?.questId);
  if (!Number.isSafeInteger(questId) || questId <= 0) throw new Error('routeQuest.questId must be a positive integer');
  return questId;
}

function questEntry(snapshot, questId) {
  return (snapshot?.questLog ?? []).find(entry => Number(entry?.questId) === questId) ?? null;
}

function stageProvesOperation(snapshot, questId, kind) {
  const stage = normalizedStage(questEntry(snapshot, questId)?.stage);
  return kind === 'accept'
    ? stage === 'inprogress' || stage === 'readytoturnin'
    : stage === 'completed';
}

function diaryOperationAllowed(routeQuest, kind) {
  if (kind === 'accept' && routeQuest?.startInDiary === true) return true;
  if (kind === 'finish' && routeQuest?.finishInDiary === true) return true;
  return (routeQuest?.specialHandlers ?? []).includes(`quest-diary-${kind}`);
}

function routeNpcFor(routeQuest, kind) {
  return kind === 'accept' ? routeQuest?.startNpc ?? null : routeQuest?.finishNpc ?? null;
}

function sameNpc(entity, routeNpc) {
  if (!entity || String(entity.kind ?? '').toLowerCase() !== 'npc') return false;
  if (routeNpc.objectId != null && String(entity.objectId) === String(routeNpc.objectId)) return true;
  const expectedName = String(routeNpc.name ?? '').replaceAll('_', ' ').trim().toLowerCase();
  const actualName = String(entity.name ?? '').replaceAll('_', ' ').trim().toLowerCase();
  const point = routeNpc.position ?? routeNpc;
  return expectedName !== '' && actualName === expectedName &&
    Number(entity.x) === Number(point?.x) && Number(entity.y) === Number(point?.y);
}

function liveNpc(snapshot, routeNpc) {
  return (snapshot?.entities ?? []).find(entity => sameNpc(entity, routeNpc)) ?? null;
}

function operationTarget(link, questId, kind, selectedItemIndex) {
  const target = String(link?.target ?? '').trim().toLowerCase();
  const expected = kind === 'accept'
    ? [`@acceptquest:${questId}`, `@quest:accept:${questId}`]
    : [
        `@finishquest:${questId}`,
        `@quest:finish:${questId}`,
        ...(selectedItemIndex >= 0 ? [`@quest:finish:${questId}:${selectedItemIndex}`] : []),
      ];
  return expected.includes(target);
}

function showTarget(link, questId) {
  return String(link?.target ?? '').trim().toLowerCase() === `@quest:show:${questId}`;
}

function dialogOperationLink(snapshot, questId, kind, selectedItemIndex) {
  return (snapshot?.activeNpcDialog?.links ?? []).find(link =>
    operationTarget(link, questId, kind, selectedItemIndex)
  ) ?? null;
}

function matchingAck(event, afterSequence, requestId, questId, kind, selectedItemIndex) {
  if (event?.sequence <= afterSequence || event?.direction !== 'received' || event?.type !== 'worldSnapshot') return false;
  const ack = event?.payload?.questOperationAck;
  if (!ack || ack.operation !== `${kind}Quest` || ack.requestId !== requestId ||
      Number(ack.questIndex) !== questId || ack.success !== true) return false;
  return kind !== 'finish' || Number(ack.selectedItemIndex) === selectedItemIndex;
}

async function prepareNpcDialog(client, routeNpc, questId, kind, selectedItemIndex, navigateNear) {
  const point = routeNpc.position ?? { x: routeNpc.x, y: routeNpc.y };
  if (!Number.isFinite(Number(point?.x)) || !Number.isFinite(Number(point?.y))) {
    throw new Error(`q${questId} ${kind} NPC has no usable position`);
  }
  await navigateNear(point, NPC_INTERACTION_RANGE);
  const npc = await client.wait(
    () => liveNpc(client.snapshot, routeNpc),
    `q${questId} live ${kind} NPC`,
    20_000,
  );
  const afterInteract = client.sequence;
  client.send({ type: 'interact', objectId: Number(npc.objectId) });
  await client.wait(
    () => client.sequence > afterInteract &&
      String(client.snapshot?.activeNpcDialog?.npcObjectId) === String(npc.objectId),
    `q${questId} ${kind} NPC dialog`,
    20_000,
  );

  if (!dialogOperationLink(client.snapshot, questId, kind, selectedItemIndex)) {
    const link = (client.snapshot?.activeNpcDialog?.links ?? []).find(entry => showTarget(entry, questId));
    if (!link) throw new Error(`q${questId} ${kind} link is absent from the active NPC dialog`);
    const beforePage = client.sequence;
    client.send({ type: 'selectNpcDialog', target: link.target });
    await client.wait(
      () => client.sequence > beforePage &&
        dialogOperationLink(client.snapshot, questId, kind, selectedItemIndex),
      `q${questId} ${kind} quest page`,
      20_000,
    );
  }

  if (!dialogOperationLink(client.snapshot, questId, kind, selectedItemIndex)) {
    throw new Error(`q${questId} ${kind} was not authorized by the active NPC dialog`);
  }
  return Number(client.snapshot.activeNpcDialog.npcObjectId);
}

/**
 * Complete one ordinary quest accept/finish operation through the public
 * Gateway protocol. NPC quests physically navigate and interact first; quests
 * explicitly marked as Diary operations omit only that NPC interaction.
 */
export async function interactQuest(client, routeQuest, action, navigateNear) {
  const questId = questIdOf(routeQuest);
  const { kind, selectedItemIndex } = actionDetails(action, routeQuest);
  const routeNpc = routeNpcFor(routeQuest, kind);
  const diary = routeNpc == null && diaryOperationAllowed(routeQuest, kind);
  if (routeNpc == null && !diary) throw new Error(`q${questId} ${kind} has neither NPC nor Diary metadata`);
  if (routeNpc != null && typeof navigateNear !== 'function') throw new Error('navigateNear is required for NPC quests');

  const npcIndex = routeNpc == null
    ? 0
    : await prepareNpcDialog(client, routeNpc, questId, kind, selectedItemIndex, navigateNear);
  if (typeof action?.verifyDefinition === 'function') await action.verifyDefinition(client, questId, kind);
  const requestId = `pq-${Date.now().toString(36)}-${++requestSequence}-${kind}-${questId}`;
  const command = kind === 'accept'
    ? { type: 'acceptQuest', requestId, npcIndex, questIndex: questId }
    : { type: 'finishQuest', requestId, questIndex: questId, selectedItemIndex };
  const beforeOperation = client.sequence;
  client.send(command);

  const ackEvent = await client.wait(
    () => client.events.find(event =>
      matchingAck(event, beforeOperation, requestId, questId, kind, selectedItemIndex)
    ),
    `q${questId} ${kind} acknowledgement`,
    20_000,
  );
  await client.wait(
    () => stageProvesOperation(client.snapshot, questId, kind),
    `q${questId} ${kind} quest stage`,
    20_000,
  );
  return { command, acknowledgement: ackEvent.payload.questOperationAck, quest: questEntry(client.snapshot, questId) };
}
