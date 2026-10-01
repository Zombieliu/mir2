import { observedPlayerHp } from './protocol-observation.mjs';
import { refreshCombatWorldSnapshot } from './protocol-refresh.mjs';
import { selfActionBlockMask } from './protocol-status.mjs';
import { startEmergencyHpRecovery, useClassRecovery, useSupplies } from './protocol-loadout.mjs';

const MAX_POST_RETREAT_HEALS = 2;
const POST_RETREAT_WINDOW_MS = 20_000;

/**
 * V2 recovery uses only ordinary held medicine and learned self-Healing.
 * The combat controller supplies `retreat` only after authoritative movement;
 * blocking cooldown/effect waits additionally require a clear live margin.
 */
export function createV2DefensiveRecovery({ sleep = ms => new Promise(resolve => setTimeout(resolve, ms)), now = Date.now } = {}) {
  return {
    async sustain(owner, thresholds = {}, context = {}) {
      if (context.phase !== 'retreat') return useSupplies(owner, thresholds);
      const classRecovery = clearRetreatMargin(owner) && !selfActionBlockMask(owner) &&
        affordableHealing(owner) && healingReady(owner)
        ? await useClassRecovery(owner, { hpThreshold: 0.85 })
        : null;
      // Do not block the next escape step on a gradual restorative's ACK.
      const consumed = startEmergencyHpRecovery(owner, { ...thresholds, hpThreshold: 0.85 });
      return { consumed, classRecovery: classRecovery ? { ...classRecovery, pending: true } : null };
    },

    async recoverAfterUnsafeRetreat(owner) {
      const until = now() + POST_RETREAT_WINDOW_MS;
      const heals = [];
      if (!clearRetreatMargin(owner)) {
        return { consumed: startEmergencyHpRecovery(owner, { hpThreshold: 0.85 }), heals, status: 'unsafeDeferred' };
      }
      if (!learnedTaoistHealing(owner)) {
        return useSupplies(owner, { hpThreshold: 0.85, mpThreshold: 0.35 });
      }
      // Mana doses are ordinary public UseItem operations. Obtain their exact
      // snapshot before measuring the following cast's mana debit.
      const consumed = await useSupplies(owner, { hpThreshold: 0, mpThreshold: 0.35 });
      for (let attempt = 0; attempt < MAX_POST_RETREAT_HEALS; attempt += 1) {
        if (healthRatio(owner) >= 0.85 || now() >= until) break;
        await refreshCombatWorldSnapshot(owner, { timeoutMs: remaining(until, now) });
        while (clearRetreatMargin(owner) && learnedTaoistHealing(owner) &&
               Number(learnedTaoistHealing(owner).cooldownRemainingTicks ?? 0) > 0) {
          if (now() >= until) return { consumed, heals, status: 'cooldownDeferred' };
          await sleep(Math.min(200, remaining(until, now)));
          await refreshCombatWorldSnapshot(owner, { timeoutMs: remaining(until, now) });
        }
        const skill = learnedTaoistHealing(owner);
        if (!clearRetreatMargin(owner) || !skill || selfActionBlockMask(owner) ||
            !affordableHealing(owner) || !healingReady(owner)) break;
        const actor = selfPlayer(owner);
        const after = Number(owner.sequence ?? 0);
        const beforeHp = observedPlayerHp(owner.snapshot);
        const beforeMp = Number(owner.snapshot?.playerMp);
        const recovery = await useClassRecovery(owner, { hpThreshold: 0.85 });
        if (!recovery) break;
        owner.record?.('diagnostic', {
          type: 'v2DefensiveHealingAttempt', phase: 'postRetreat', objectId: Number(actor.objectId),
          afterSequence: after, hp: beforeHp, mp: beforeMp,
        });
        await owner.wait(() => acceptedSelfHealing(owner, after, actor.objectId) ||
          !clearRetreatMargin(owner), 'V2 accepted defensive self-Healing', remaining(until, now));
        if (!acceptedSelfHealing(owner, after, actor.objectId)) break;
        let effectConfirmed = false;
        do {
          await refreshCombatWorldSnapshot(owner, { timeoutMs: remaining(until, now) });
          effectConfirmed = observedPlayerHp(owner.snapshot) > beforeHp &&
            Number(owner.snapshot?.playerMp) <= beforeMp - Number(skill.mpCost);
          if (effectConfirmed || !clearRetreatMargin(owner) || now() >= until) break;
          await sleep(Math.min(200, remaining(until, now)));
        } while (now() < until);
        if (!effectConfirmed) {
          if (!clearRetreatMargin(owner)) break;
          throw new Error('V2 defensive Healing lacked fresh exact HP recovery and MP debit');
        }
        const proof = { afterSequence: after, objectId: Number(actor.objectId), beforeHp, beforeMp,
          hp: observedPlayerHp(owner.snapshot), mp: Number(owner.snapshot.playerMp) };
        heals.push(proof);
        owner.record?.('diagnostic', { type: 'v2DefensiveHealingConfirmed', phase: 'postRetreat', ...proof });
      }
      consumed.push(...clearRetreatMargin(owner)
        ? await useSupplies(owner, { hpThreshold: 0.85, mpThreshold: 0.35 })
        : startEmergencyHpRecovery(owner, { hpThreshold: 0.85 }));
      return { consumed, heals, status: heals.length ? 'healingConfirmed' : 'deferred' };
    },
  };
}

function selfPlayer(owner) {
  return (owner?.snapshot?.entities ?? []).find(entity =>
    Number(entity?.objectId) === Number(owner.snapshot?.playerObjectId));
}

function learnedTaoistHealing(owner) {
  const actor = selfPlayer(owner);
  if (String(actor?.class ?? '').toLowerCase() !== 'taoist') return null;
  return (owner?.snapshot?.knownSkills ?? []).find(skill => String(skill?.spell) === 'Healing') ?? null;
}

function affordableHealing(owner) {
  const skill = learnedTaoistHealing(owner);
  const mp = owner?.snapshot?.playerMp;
  const cost = skill?.mpCost;
  return skill && mp != null && cost != null && Number.isFinite(Number(mp)) &&
    Number.isFinite(Number(cost)) && Number(cost) >= 0 && Number(mp) >= Number(cost);
}

function healingReady(owner) {
  const ticks = learnedTaoistHealing(owner)?.cooldownRemainingTicks;
  return ticks != null && Number.isFinite(Number(ticks)) && Number(ticks) <= 0;
}

function clearRetreatMargin(owner) {
  const actor = selfPlayer(owner);
  if (!actor || actor.dead === true || observedPlayerHp(owner.snapshot) <= 0 ||
      !Number.isFinite(Number(actor.x)) || !Number.isFinite(Number(actor.y))) return false;
  return !(owner.snapshot.entities ?? []).some(entity =>
    entity?.kind === 'monster' && entity?.dead !== true && Number(entity?.hp ?? 1) > 0 &&
    String(entity?.disposition ?? '').toLowerCase() === 'hostile' &&
    Math.max(Math.abs(Number(entity.x) - Number(actor.x)), Math.abs(Number(entity.y) - Number(actor.y))) <= 3);
}

function acceptedSelfHealing(owner, after, objectId) {
  return (owner.events ?? []).some(event => Number(event.sequence) > after && event.direction === 'received' &&
    (event.packet === 'Magic' || (event.packet === 'ObjectMagic' && Number(event.payload?.objectId) === Number(objectId))) &&
    event.payload?.spell === 'Healing' && event.payload?.cast === true && Number(event.payload?.targetId) === Number(objectId));
}

function healthRatio(owner) {
  return observedPlayerHp(owner.snapshot) / Math.max(1, Number(owner.snapshot?.playerMaxHp ?? 1));
}

function remaining(until, now) {
  return Math.max(1, until - now());
}
