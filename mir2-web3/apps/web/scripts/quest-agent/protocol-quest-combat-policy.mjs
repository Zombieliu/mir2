import { combatAction, combatApproachRange, meleeCombatAction } from './protocol-loadout.mjs';

// These mixed Wooma expeditions are long enough that spending SoulFireBall
// amulets on incidental doorway blockers can strand a Taoist before the final
// objective. Preserve the finite ammunition while retaining ordinary melee.
const TAOIST_OBJECTIVE_AMMO_QUESTS = new Set([98, 99]);

export function shouldPreserveTaoistObjectiveAmmo(client, routeQuest, target) {
  const snapshot = client?.snapshot;
  const actor = (snapshot?.entities ?? []).find(entity =>
    Number(entity?.objectId) === Number(snapshot?.playerObjectId));
  if (String(actor?.class ?? '').toLowerCase() !== 'taoist') return false;
  if (!TAOIST_OBJECTIVE_AMMO_QUESTS.has(Number(routeQuest?.questId))) return false;
  const targetName = normalized(target?.name);
  return Boolean(targetName) && !objectiveMonsterNames(routeQuest).has(targetName);
}

export function questCombatApproachRange(client, routeQuest, target) {
  const ordinaryRange = combatApproachRange(client, target);
  if (shouldPreserveTaoistObjectiveAmmo(client, routeQuest, target)) return 1;
  return Number(routeQuest?.questId) === 113 && isTaoist(client) && ordinaryRange > 1
    ? 9
    : ordinaryRange;
}

export async function questCombatAction(client, routeQuest, target) {
  return shouldPreserveTaoistObjectiveAmmo(client, routeQuest, target)
    ? meleeCombatAction(client, target)
    : combatAction(client, target);
}

function objectiveMonsterNames(routeQuest) {
  const names = new Set();
  for (const objective of routeQuest?.objectives?.kill ?? []) names.add(normalized(objective?.monsterName));
  for (const objective of routeQuest?.objectives?.item ?? []) {
    for (const source of objective?.sources ?? []) names.add(normalized(source?.monsterName));
  }
  names.delete('');
  return names;
}

function normalized(value) {
  return String(value ?? '').trim().toLowerCase();
}

function isTaoist(client) {
  const snapshot = client?.snapshot;
  const actor = (snapshot?.entities ?? []).find(entity =>
    Number(entity?.objectId) === Number(snapshot?.playerObjectId));
  return normalized(actor?.class) === 'taoist';
}
