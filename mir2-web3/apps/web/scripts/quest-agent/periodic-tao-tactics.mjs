import { combatAction } from './protocol-loadout.mjs';
import { observedPlayerHp } from './protocol-observation.mjs';
import { refreshCombatWorldSnapshot } from './protocol-refresh.mjs';
import { moveHeldBeltItemToInventory } from './protocol-supplies.mjs';
import { selfActionBlockMask } from './protocol-status.mjs';

const MATERIALS = Object.freeze({ Amulet: { index: 712, shape: 0 }, GreenPoison: { index: 710, shape: 1 } });
const DIRECTIONS = ['Up', 'UpRight', 'Right', 'DownRight', 'Down', 'DownLeft', 'Left', 'UpLeft'];

/**
 * Periodic journeys only: one receipted support action, or the existing ordinary
 * combat action. This never buys/learns/provisions, awards progress, or changes
 * the generic/newcomer controller. All support waits share one finite budget.
 */
export function createPeriodicTaoCombatAction({
  delegate = combatAction, refresh = refreshCombatWorldSnapshot,
  moveBeltItem = moveHeldBeltItemToInventory,
  checkDeadline = () => {}, onReceipt = () => {}, timeoutMs = 12_000,
} = {}) {
  if (!Number.isSafeInteger(timeoutMs) || timeoutMs < 1 || timeoutMs > 20_000) {
    throw new Error('Periodic Tao support timeout must be 1..20000 ms');
  }
  const clients = new WeakMap();
  return async function periodicTaoCombatAction(client, selectedTarget) {
    assertConnected(client);
    checkDeadline();
    let actor = self(client.snapshot);
    let state = clients.get(client);
    const identity = epoch(client.snapshot);
    if (!state || state.epoch !== identity) {
      state = { epoch: identity, seen: 0, poisoned: new Map() };
      clients.set(client, state);
    }
    observeLifetimes(client, state);
    if (!actor || objectId(actor.objectId) == null || !validPosition(actor) ||
        !String(client.snapshot?.mapFileName ?? '').trim() || actor.dead === true || observedPlayerHp(client.snapshot) <= 0 ||
        normalize(actor.class) !== 'taoist' || selfActionBlockMask(client) ||
        recoveryNeeded(client.snapshot)) return delegate(client, selectedTarget);

    const budget = actionBudget(client, state, checkDeadline, timeoutMs);
    let target = liveEnemy(client.snapshot, selectedTarget?.objectId);
    if (!target) return waitAction(selectedTarget?.objectId, 1);

    // A pre-movement/pre-cast zero cannot release the shared Zone action lock.
    // Positive readiness also needs a new server sample; never age it locally.
    if (needsReadinessRefresh(client) || ['SummonSkeleton', 'Poisoning'].some(spell =>
      cooldown(learned(client.snapshot, spell)) > 0)) {
      await refresh(budget.client, { timeoutMs: budget.remaining() });
      budget.guard();
      observeLifetimes(client, state);
      actor = self(client.snapshot);
      target = liveEnemy(client.snapshot, target.objectId);
      if (!target || recoveryNeeded(client.snapshot) || selfActionBlockMask(client)) {
        return target ? delegate(client, target) : waitAction(selectedTarget?.objectId, 1);
      }
    }

    const amulet = findHeldMaterial(client.snapshot, 'Amulet');
    const summon = affordable(client.snapshot, 'SummonSkeleton');
    if (!ownedPeriodicBoneFamiliar(client) && summon && amulet) {
      if (cooldown(summon) > 0) return waitAction(target.objectId, 650, 'SummonSkeleton');
      if (cooldown(summon) === 0) {
        const material = await equipMaterial(client, amulet, 'Amulet', budget, moveBeltItem);
        budget.guard();
        actor = self(client.snapshot);
        const skill = affordable(client.snapshot, 'SummonSkeleton');
        if (recoveryNeeded(client.snapshot) || selfActionBlockMask(client) || cooldown(skill) !== 0) {
          return delegate(client, liveEnemy(client.snapshot, target.objectId) ?? target);
        }
        const beforeQuantity = quantity(material);
        const after = client.sequence;
        const command = { type: 'magic', objectId: Number(actor.objectId), spell: 'SummonSkeleton',
          direction: DIRECTIONS.includes(actor.direction) ? actor.direction : 'Down',
          targetId: 0, x: Number(actor.x), y: Number(actor.y), spellTargetLock: false };
        budget.client.send(command);
        const accepted = await waitAccepted(client, budget, after, command);
        const spawn = await budget.client.wait(() => received(client, after, 'ObjectMonster').find(event =>
          normalize(event.payload?.name) === 'bonefamiliar' && event.payload?.dead === false &&
          objectId(event.payload?.masterObjectId) === Number(actor.objectId) &&
          objectId(event.payload?.objectId) != null), 'owned BoneFamiliar spawn', budget.remaining());
        await refresh(budget.client, { timeoutMs: budget.remaining() });
        budget.guard();
        const pet = ownedPeriodicBoneFamiliar(client, spawn.payload.objectId);
        if (!pet || !materialDebited(client.snapshot, material.uniqueId, beforeQuantity)) {
          throw new Error('SummonSkeleton lacked owned snapshot and exact Amulet debit');
        }
        const receipt = { kind: 'periodicTaoSupport', spell: command.spell,
          actorObjectId: Number(actor.objectId), petObjectId: Number(pet.objectId),
          ownerName: String(pet.ownerName), masterObjectId: Number(spawn.payload.masterObjectId),
          acceptedSequence: accepted.sequence, effectSequence: spawn.sequence,
          materialUniqueId: Number(material.uniqueId), quantityBefore: beforeQuantity,
          quantityAfter: beforeQuantity - 1, targetId: Number(target.objectId), ownCreditGranted: false };
        onReceipt(receipt);
        return waitAction(target.objectId, 650, command.spell, receipt);
      }
    }

    const poison = affordable(client.snapshot, 'Poisoning');
    const green = findHeldMaterial(client.snapshot, 'GreenPoison');
    target = liveEnemy(client.snapshot, target.objectId);
    if (target && amulet && green && poison && !state.poisoned.has(Number(target.objectId)) &&
        !(finite(target.poison) != null && (Number(target.poison) & 1)) && distance(actor, target) <= 9) {
      if (cooldown(poison) > 0) return waitAction(target.objectId, 650, 'Poisoning');
      if (cooldown(poison) === 0) {
        // Establish a real return slot first. Green poison temporarily occupies
        // slot 9, never a bracelet slot that would sacrifice worn defence.
        const originalAmulet = await equipMaterial(client, amulet, 'Amulet', budget, moveBeltItem);
        let failure;
        let result;
        try {
          const material = await equipMaterial(client, green, 'GreenPoison', budget, moveBeltItem);
          budget.guard();
          actor = self(client.snapshot);
          target = liveEnemy(client.snapshot, target.objectId);
          const skill = affordable(client.snapshot, 'Poisoning');
          if (target && cooldown(skill) === 0 && !recoveryNeeded(client.snapshot) &&
              !selfActionBlockMask(client) && distance(actor, target) <= 9) {
            const beforeQuantity = quantity(material);
            const after = client.sequence;
            const command = { type: 'magic', objectId: Number(actor.objectId), spell: 'Poisoning',
              direction: toward(actor, target), targetId: Number(target.objectId),
              x: Number(target.x), y: Number(target.y), spellTargetLock: true };
            budget.client.send(command);
            const accepted = await waitAccepted(client, budget, after, command);
            const effect = await budget.client.wait(() => received(client, after, 'ObjectPoisoned').find(event =>
              objectId(event.payload?.objectId) === command.targetId &&
              finite(event.payload?.poison) != null && (Number(event.payload.poison) & 1)),
            'green Poisoning target effect', budget.remaining());
            await refresh(budget.client, { timeoutMs: budget.remaining() });
            budget.guard();
            if (!materialDebited(client.snapshot, material.uniqueId, beforeQuantity)) {
              throw new Error('Poisoning lacked exact GreenPoison debit');
            }
            state.poisoned.set(command.targetId, { sequence: effect.sequence });
            result = { kind: 'periodicTaoSupport', spell: command.spell,
              actorObjectId: Number(actor.objectId), targetId: command.targetId,
              acceptedSequence: accepted.sequence, effectSequence: effect.sequence,
              materialUniqueId: Number(material.uniqueId), quantityBefore: beforeQuantity,
              quantityAfter: beforeQuantity - 1, poison: Number(effect.payload.poison), ownCreditGranted: false };
          }
        } catch (error) { failure = error; }
        // Restoration has its own small bounded cleanup budget. Preserve the
        // original failure; reconnect/death must never start further packets.
        try {
          const cleanup = actionBudget(client, state, checkDeadline, Math.min(timeoutMs, 4_000));
          cleanup.guard();
          const held = findHeldMaterial(client.snapshot, 'Amulet', originalAmulet.uniqueId);
          if (!held) throw new Error('Original Amulet missing after Poisoning');
          await equipMaterial(client, held, 'Amulet', cleanup, moveBeltItem);
        } catch (restoreError) {
          if (failure) failure.restoreError = String(restoreError?.message ?? restoreError);
          else failure = restoreError;
        }
        if (failure) throw failure;
        if (result) {
          result.restoredAmuletUniqueId = Number(originalAmulet.uniqueId);
          onReceipt(result);
          return waitAction(result.targetId, 650, 'Poisoning', result);
        }
        return target ? delegate(client, target) : waitAction(selectedTarget?.objectId, 1);
      }
    }
    return delegate(client, target ?? selectedTarget);
  };
}

/** Snapshot ownerName is the public ownership projection; master id, when
 * observed on its ObjectMonster packet, must corroborate rather than conflict.
 * A new summon additionally requires a fresh owned spawn and accepted cast. */
export function ownedPeriodicBoneFamiliar(client, wantedId = null) {
  const actor = self(client?.snapshot);
  const name = String(actor?.name ?? '').trim().toLowerCase();
  if (!name || objectId(actor?.objectId) == null) return null;
  return (client.snapshot.entities ?? []).find(entity => {
    if (normalize(entity.kind) !== 'monster' || normalize(entity.name) !== 'bonefamiliar' ||
        entity.dead === true || !(finite(entity.hp) > 0) || objectId(entity.objectId) == null ||
        (wantedId != null && Number(entity.objectId) !== Number(wantedId)) ||
        String(entity.ownerName ?? '').trim().toLowerCase() !== name) return false;
    if (entity.masterObjectId != null && objectId(entity.masterObjectId) !== Number(actor.objectId)) return false;
    const latest = (client.events ?? []).findLast(event => event.direction === 'received' &&
      event.packet === 'ObjectMonster' && objectId(event.payload?.objectId) === Number(entity.objectId));
    return latest?.payload?.masterObjectId == null || objectId(latest.payload.masterObjectId) === Number(actor.objectId);
  }) ?? null;
}

function actionBudget(client, state, checkDeadline, timeoutMs) {
  const end = Date.now() + timeoutMs;
  const guard = () => {
    assertConnected(client); checkDeadline();
    if (epoch(client.snapshot) !== state.epoch) throw new Error('Periodic Tao support player/map changed');
    const actor = self(client.snapshot);
    if (!actor || actor.dead === true || observedPlayerHp(client.snapshot) <= 0) throw new Error('Player died during periodic Tao support');
    if (normalize(actor.class) !== 'taoist' || !validPosition(actor) ||
        !Number.isSafeInteger(client.sequence) || client.sequence < 0) throw new Error('Periodic Tao support lacks authoritative actor/sequence');
    if (Date.now() >= end) throw new Error('Periodic Tao support deadline exceeded');
  };
  const remaining = () => { guard(); return Math.max(1, end - Date.now()); };
  const facade = {
    get snapshot() { return client.snapshot; }, get events() { return client.events; },
    get sequence() { return client.sequence; }, get failure() { return client.failure; }, get closed() { return client.closed; },
    send(command) { guard(); return client.send(command); },
    wait(predicate, label, requested) {
      return client.wait(() => { guard(); return predicate(); }, label,
        Math.min(remaining(), requested ?? remaining()));
    },
    request(command, packet, requested) {
      const after = client.sequence;
      facade.send(command);
      return facade.wait(() => received(client, after, packet)[0], packet, requested);
    },
  };
  return { guard, remaining, client: facade };
}

async function equipMaterial(client, candidate, name, budget, moveBeltItem) {
  budget.guard();
  let held = findHeldMaterial(client.snapshot, name, candidate.uniqueId);
  if (!held) throw new Error(`No eligible held ${name}`);
  if ((client.snapshot.equipmentItems ?? []).includes(held) && normalize(held.slot ?? held.equipSlot) === 'amulet') return held;
  if ((client.snapshot.beltItems ?? []).includes(held)) {
    await moveBeltItem(budget.client, held, name);
    held = (client.snapshot.inventoryItems ?? []).find(item => Number(item.uniqueId) === Number(candidate.uniqueId));
    if (!eligibleMaterial(client.snapshot, held, name)) throw new Error(`Belt ${name} move lacked exact item receipt`);
  }
  if (!(client.snapshot.inventoryItems ?? []).includes(held)) throw new Error(`${name} is not in the normal inventory`);
  const expectedQuantity = quantity(held);
  const ack = await budget.client.request({ type: 'equipItem', grid: 'inventory', uniqueId: Number(held.uniqueId), to: 9 }, 'EquipItem');
  if (ack?.payload?.success !== true) throw new Error(`Equip rejected for ${name}`);
  await budget.client.wait(() => (client.snapshot.equipmentItems ?? []).find(item =>
    Number(item.uniqueId) === Number(held.uniqueId) && normalize(item.slot ?? item.equipSlot) === 'amulet' &&
    quantity(item) === expectedQuantity && eligibleMaterial(client.snapshot, item, name)),
  `exact equipped ${name}`, budget.remaining());
  return (client.snapshot.equipmentItems ?? []).find(item => Number(item.uniqueId) === Number(held.uniqueId));
}

async function waitAccepted(client, budget, after, command) {
  return budget.client.wait(() => {
    const matches = (client.events ?? []).filter(event => Number(event.sequence) > after && event.direction === 'received' &&
      (event.packet === 'Magic' && (event.payload?.objectId == null || Number(event.payload.objectId) === command.objectId) ||
       event.packet === 'ObjectMagic' && objectId(event.payload?.objectId) === command.objectId) &&
      event.payload?.spell === command.spell && objectId(event.payload?.targetId, true) === command.targetId);
    if (matches.some(event => event.payload.cast === false)) throw new Error(`${command.spell} rejected by server`);
    return matches.find(event => event.payload.cast === true);
  }, `${command.spell} accepted own cast`, budget.remaining());
}

function observeLifetimes(client, state) {
  for (const event of client.events ?? []) {
    if (!(Number(event.sequence) > state.seen)) continue;
    if (event.direction === 'lifecycle' || event.direction === 'sent' && ['startGame', 'logOut', 'townRevive'].includes(event.type) ||
        event.direction === 'received' && ['MapInformation', 'MapChanged', 'UserInformation'].includes(event.packet)) state.poisoned.clear();
    if (event.direction === 'received') {
      const id = objectId(event.payload?.objectId);
      const entry = state.poisoned.get(id);
      if (entry && Number(event.sequence) > entry.sequence && (
        ['ObjectDied', 'ObjectRemove', 'ObjectMonster'].includes(event.packet) ||
        event.packet === 'ObjectPoisoned' && finite(event.payload?.poison) != null && !(Number(event.payload.poison) & 1))) {
        state.poisoned.delete(id);
      }
      if (event.packet === 'ObjectDied' && id === Number(client.snapshot?.playerObjectId)) state.poisoned.clear();
    }
    state.seen = Math.max(state.seen, Number(event.sequence) || 0);
  }
  for (const [id] of state.poisoned) {
    const entity = (client.snapshot?.entities ?? []).find(entry => Number(entry.objectId) === id);
    if (entity?.dead === true || entity && finite(entity.hp) != null && Number(entity.hp) <= 0) state.poisoned.delete(id);
  }
}

function needsReadinessRefresh(client) {
  let snapshotSequence = -1, actionSequence = -1;
  const id = objectId(client.snapshot?.playerObjectId);
  for (const event of client.events ?? []) {
    if (event.direction !== 'received') continue;
    if (event.type === 'worldSnapshot') snapshotSequence = Math.max(snapshotSequence, Number(event.sequence));
    if (event.packet === 'UserLocation' && (event.payload?.objectId == null || objectId(event.payload.objectId) === id) ||
        event.packet === 'Magic' || event.packet === 'ObjectMagic' && objectId(event.payload?.objectId) === id) {
      actionSequence = Math.max(actionSequence, Number(event.sequence));
    }
  }
  return actionSequence > snapshotSequence;
}
function findHeldMaterial(snapshot, name, uniqueId = null) {
  return [...(snapshot?.equipmentItems ?? []).filter(item => normalize(item.slot ?? item.equipSlot) === 'amulet'),
    ...(snapshot?.inventoryItems ?? []), ...(snapshot?.beltItems ?? [])].find(item =>
    (uniqueId == null || Number(item.uniqueId) === Number(uniqueId)) && eligibleMaterial(snapshot, item, name)) ?? null;
}
function eligibleMaterial(snapshot, item, name) {
  const spec = MATERIALS[name], info = item?.tooltipSource?.realInfo ?? item?.tooltipSource?.real_info ?? item?.tooltipSource?.info;
  const user = item?.tooltipSource?.userItem ?? item?.tooltipSource?.user_item;
  const index = finite(item?.itemIndex ?? item?.item_index ?? info?.item_index ?? info?.itemIndex);
  const type = finite(info?.item_type ?? info?.itemType), shape = finite(item?.shape ?? info?.shape);
  if (!spec || objectId(item?.uniqueId) == null || quantity(item) == null || index !== spec.index || type !== 8 || shape !== spec.shape ||
      item?.expired === true || item?.isBroken === true || item?.broken === true ||
      user?.expire_info != null || user?.expireInfo != null || user?.rental_information != null || user?.rentalInformation != null) return false;
  const max = finite(item?.durabilityMax ?? item?.durability_max ?? user?.max_dura ?? user?.maxDura);
  const current = finite(item?.durabilityCurrent ?? item?.durability_current ?? user?.current_dura ?? user?.currentDura);
  if (max == null || max < 0 || current == null || current < 0 || max > 0 && current <= 0) return false;
  const requiredClass = finite(info?.required_class ?? info?.requiredClass);
  const requiredType = finite(info?.required_type ?? info?.requiredType);
  const requiredAmount = finite(info?.required_amount ?? info?.requiredAmount);
  const level = finite(self(snapshot)?.level);
  return requiredClass != null && (requiredClass === 0 || (requiredClass & 4) !== 0) && requiredType === 0 &&
    requiredAmount != null && level != null && level >= requiredAmount;
}
function materialDebited(snapshot, uniqueId, before) {
  const matches = [...(snapshot.equipmentItems ?? []), ...(snapshot.inventoryItems ?? []), ...(snapshot.beltItems ?? [])]
    .filter(item => Number(item.uniqueId) === Number(uniqueId));
  if (before === 1) return matches.length === 0;
  return matches.length === 1 && quantity(matches[0]) === before - 1;
}
function affordable(snapshot, spell) {
  const skill = learned(snapshot, spell), mp = finite(snapshot?.playerMp), cost = finite(skill?.mpCost);
  return skill && mp != null && cost != null && cost > 0 && mp >= cost ? skill : null;
}
function cooldown(skill) {
  if (!skill) return null;
  const ms = finite(skill.cooldownRemainingMs), ticks = finite(skill.cooldownRemainingTicks);
  if (skill.cooldownRemainingMs != null && (ms == null || ms < 0) ||
      skill.cooldownRemainingTicks != null && (ticks == null || ticks < 0)) return null;
  if (ms == null && ticks == null) return null;
  return Math.max(ms ?? 0, ticks ?? 0);
}
function liveEnemy(snapshot, id) {
  const entity = (snapshot?.entities ?? []).find(entry => objectId(entry?.objectId) === objectId(id));
  return entity && normalize(entity.kind) === 'monster' && entity.dead !== true && finite(entity.hp) > 0 &&
    entity.hidden !== true && !String(entity.ownerName ?? '').trim() &&
    !['friendly', 'neutral'].includes(normalize(entity.disposition)) &&
    [entity.x, entity.y].every(value => Number.isSafeInteger(finite(value))) ? entity : null;
}
function recoveryNeeded(snapshot) {
  const max = finite(snapshot?.playerMaxHp);
  return max != null && max > 0 && observedPlayerHp(snapshot) / max <= 0.7;
}
function assertConnected(client) { if (client?.failure) throw client.failure; if (client?.closed) throw new Error('Connection closed during periodic Tao support'); }
function learned(snapshot, spell) { return (snapshot?.knownSkills ?? []).find(skill => skill.spell === spell); }
function self(snapshot) { return (snapshot?.entities ?? []).find(entity => objectId(entity.objectId) === objectId(snapshot?.playerObjectId)); }
function epoch(snapshot) { const actor = self(snapshot); return `${snapshot?.mapFileName ?? ''}|${snapshot?.playerObjectId ?? ''}|${actor?.name ?? ''}`; }
function quantity(item) { const value = finite(item?.quantity); return Number.isSafeInteger(value) && value > 0 ? value : null; }
function finite(value) { if (value == null || typeof value === 'boolean' || typeof value === 'string' && !value.trim()) return null; const parsed = Number(value); return Number.isFinite(parsed) ? parsed : null; }
function objectId(value, zero = false) { const parsed = finite(value); return Number.isSafeInteger(parsed) && parsed >= (zero ? 0 : 1) ? parsed : null; }
function normalize(value) { return String(value ?? '').replace(/[^a-z0-9]/gi, '').toLowerCase(); }
function received(client, after, packet) { return (client.events ?? []).filter(event => event.direction === 'received' && Number(event.sequence) > after && event.packet === packet); }
function distance(a, b) { return Math.max(Math.abs(Number(a?.x) - Number(b?.x)), Math.abs(Number(a?.y) - Number(b?.y))); }
function validPosition(entity) { return [entity?.x, entity?.y].every(value => Number.isSafeInteger(finite(value))); }
function toward(a, b) { const dx = Math.sign(Number(b.x) - Number(a.x)), dy = Math.sign(Number(b.y) - Number(a.y)); return new Map([['0,-1', 'Up'], ['1,-1', 'UpRight'], ['1,0', 'Right'], ['1,1', 'DownRight'], ['0,1', 'Down'], ['-1,1', 'DownLeft'], ['-1,0', 'Left'], ['-1,-1', 'UpLeft']]).get(`${dx},${dy}`) ?? 'Down'; }
function waitAction(targetId, delayMs, spell, supportReceipt) { return { kind: 'wait', targetId: Number(targetId), delayMs, ...(spell ? { spell } : {}), ...(supportReceipt ? { supportReceipt } : {}) }; }
