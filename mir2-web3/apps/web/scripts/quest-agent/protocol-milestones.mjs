import fs from 'node:fs/promises';
import { interactQuest } from './protocol-quest-actions.mjs';
import { verifyItemRewards } from './protocol-rewards.mjs';

export const JOURNEY_MILESTONES = [15, 20, 25, 30];
const GOLD = { 15: 5000, 20: 10000, 25: 15000, 30: 25000 };
let catalogPromise;
const stage = value => String(value ?? '').replace(/[^a-z]/gi, '').toLowerCase();
const player = client => client.snapshot.entities.find(e => e.objectId === client.snapshot.playerObjectId);

export function milestoneRewardsFor(profile, level, actor, items) {
  if (!['Warrior', 'Wizard', 'Taoist'].includes(actor.class)) throw new Error('Unknown milestone class');
  if (!['Male', 'Female'].includes(actor.gender)) throw new Error('Unknown milestone gender');
  const rule = profile.milestoneRewards?.find(entry => entry.level === level);
  if (!rule && level !== 30) throw new Error(`Missing level ${level} milestone reward configuration`);
  if (rule && !rule.classRewards?.[actor.class]?.length) throw new Error(`Missing ${actor.class} milestone rewards`);
  return (rule?.classRewards?.[actor.class] ?? []).map(reward => {
    const name = reward.item ?? (actor.gender === 'Female' ? reward.femaleItem : reward.maleItem);
    const item = items.find(entry => entry.name === name);
    if (!item || !Number.isSafeInteger(item.item_index) || !Number.isSafeInteger(reward.count) || reward.count <= 0) throw new Error(`Invalid milestone item ${name}`);
    return { itemIndex: item.item_index, itemName: name, count: reward.count };
  });
}

export function verifyMilestoneOffer(client, questId, expected, gold, boardId) {
  const cached = typeof client.questDefinition === 'function'
    ? client.questDefinition(questId)
    : client.questDefinitions?.get?.(questId);
  const event = cached ? null : [...client.events].reverse().find(e => e.direction === 'received' && e.packet === 'NewQuestInfo' && Number(e.payload?.info?.index) === questId);
  const info = cached ?? event?.payload?.info;
  if (!info || info.npc_index !== boardId || info.finish_npc_index !== boardId || info.reward_gold !== gold) throw new Error(`q${questId} milestone definition is absent or mismatched`);
  const normalized = rows => rows.map(row => `${row.itemIndex}:${row.count}`).sort();
  const offered = (info.rewards_fixed_item ?? []).filter(row => row.count > 0).map(row => ({ itemIndex: row.item.index, count: row.count }));
  if ((info.rewards_select_item ?? []).some(row => row.count > 0) || JSON.stringify(normalized(offered)) !== JSON.stringify(normalized(expected))) throw new Error(`q${questId} server milestone rewards differ from the class progression profile`);
}

/** Claim only reached, one-time growth rewards through the ordinary Board UI protocol. */
export async function claimAvailableMilestones(client, { profile, route, travel, navigate, items, act = interactQuest }) {
  if (profile.profile !== 'newcomer-v1') return [];
  const due = JOURNEY_MILESTONES.filter(level => level <= Number(player(client)?.level) && !client.snapshot.questLog.some(q => q.questId === 2100000 + level && stage(q.stage) === 'completed'));
  if (!due.length) return [];
  const board = route.quests.map(q => q.startNpc).find(npc => npc?.scriptKey === 'BichonProvince/BichonWall/Board');
  if (!board) throw new Error('Milestone Board is missing from the source-backed route');
  if (!items) {
    catalogPromise ??= fs.readFile(new URL('../../../../packages/game-data/data/generated/crystal_item_manifest.json', import.meta.url), 'utf8').then(text => JSON.parse(text).items);
    items = await catalogPromise;
  }
  await travel(board.mapFileName);
  const results = [];
  for (const level of due) {
    const questId = 2100000 + level;
    const expected = milestoneRewardsFor(profile, level, player(client), items);
    const quest = { questId, startNpc: board, finishNpc: board };
    const verifyDefinition = owner => verifyMilestoneOffer(owner, questId, expected, GOLD[level], board.objectId);
    const current = client.snapshot.questLog.find(q => q.questId === questId);
    if (!['inprogress', 'readytoturnin'].includes(stage(current?.stage))) await act(client, quest, { type: 'accept', verifyDefinition }, navigate);
    const before = structuredClone(client.snapshot);
    await act(client, quest, { type: 'finish', selectedItemIndex: -1, verifyDefinition }, navigate);
    if (!client.snapshot.questLog.some(q => q.questId === questId && stage(q.stage) === 'completed')) throw new Error(`q${questId} milestone completion not confirmed`);
    if (Number(client.snapshot.gold) - Number(before.gold) < GOLD[level]) throw new Error(`q${questId} milestone gold missing`);
    const rewards = verifyItemRewards(before, client.snapshot, expected);
    const result = { questId, level, rewards, gold: Number(client.snapshot.gold) - Number(before.gold) };
    results.push(result);
    client.record?.('diagnostic', { type: 'milestoneClaimed', ...result });
  }
  return results;
}
