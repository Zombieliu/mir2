function entitiesOf(snapshot) {
  if (!Array.isArray(snapshot.entities)) snapshot.entities = [];
  return snapshot.entities;
}

function objectIdOf(value) {
  const objectId = Number(value);
  return Number.isFinite(objectId) ? objectId : null;
}

function entityById(snapshot, objectId) {
  const wanted = objectIdOf(objectId);
  if (wanted == null) return null;
  return entitiesOf(snapshot).find(entity => objectIdOf(entity?.objectId) === wanted) ?? null;
}

function selfEntity(snapshot) {
  return entityById(snapshot, snapshot.playerObjectId);
}

function positionFields(payload) {
  const location = payload?.location;
  const fields = {};
  if (Number.isFinite(Number(payload?.x))) fields.x = Number(payload.x);
  if (Number.isFinite(Number(payload?.y))) fields.y = Number(payload.y);
  if (Number.isFinite(Number(location?.x))) fields.x = Number(location.x);
  if (Number.isFinite(Number(location?.y))) fields.y = Number(location.y);
  if (payload?.direction != null) fields.direction = payload.direction;
  return fields;
}

function updateEntity(snapshot, objectId, payload) {
  const entity = entityById(snapshot, objectId);
  if (entity) Object.assign(entity, positionFields(payload));
  return entity;
}

function upsertEntity(snapshot, payload, kind) {
  const objectId = objectIdOf(payload?.objectId);
  if (objectId == null) return null;
  let entity = entityById(snapshot, objectId);
  if (!entity) {
    entity = { objectId, kind };
    entitiesOf(snapshot).push(entity);
  }
  const location = payload?.location;
  const observed = { ...payload, ...positionFields(payload), kind: entity.kind ?? kind };
  delete observed.location;
  Object.assign(entity, observed);
  if (location && entity.location) delete entity.location;
  return entity;
}

// Crystal's incremental monster packets do not carry the disposition exposed
// by a worldSnapshot. Preserve that semantic for families which are
// unconditionally neutral in the imported Crystal AI table; otherwise the
// navigator treats their moving trail as a hostile clearance wall until the
// next full snapshot arrives.
const NEUTRAL_MONSTER_AI = new Set([1, 2, 3, 6, 56, 57, 58]);

function observedMonsterPayload(snapshot, payload) {
  const existing = entityById(snapshot, payload?.objectId);
  if (payload?.disposition != null || existing?.disposition != null ||
      !NEUTRAL_MONSTER_AI.has(Number(payload?.ai))) {
    return payload;
  }
  return { ...payload, disposition: 'neutral' };
}

function removeEntity(snapshot, objectId) {
  const wanted = objectIdOf(objectId);
  if (wanted == null) return;
  snapshot.entities = entitiesOf(snapshot).filter(entity => objectIdOf(entity?.objectId) !== wanted);
}

function clearOldMapObservations(snapshot, self) {
  snapshot.entities = self ? [self] : [];
  snapshot.mapSnapshotPending = true;
  snapshot.mapTransfers = [];
  snapshot.groundDrops = [];
  snapshot.activeNpcDialog = null;
}

function applyMapChange(snapshot, payload) {
  const self = selfEntity(snapshot);
  if (self) {
    Object.assign(self, positionFields(payload));
  }
  if (payload?.mapIndex != null) snapshot.mapIndex = payload.mapIndex;
  if (payload?.fileName != null) snapshot.mapFileName = payload.fileName;
  if (payload?.title != null) snapshot.mapTitle = payload.title;
  clearOldMapObservations(snapshot, self);
}

function applyMapInformation(snapshot, payload) {
  const self = selfEntity(snapshot);
  if (self) {
    // The live wire packet identifies the new map but carries no landing
    // position. Do not carry old-map coordinates across that boundary; the
    // following UserLocation packet supplies the authoritative landing tile.
    delete self.x;
    delete self.y;
  }
  if (payload?.mapIndex != null) snapshot.mapIndex = payload.mapIndex;
  if (payload?.fileName != null) snapshot.mapFileName = payload.fileName;
  if (payload?.title != null) snapshot.mapTitle = payload.title;
  clearOldMapObservations(snapshot, self);
}

function finiteNumber(value) {
  const number = Number(value);
  return Number.isFinite(number) ? number : null;
}

function applySelfVitals(snapshot, payload) {
  const player = selfEntity(snapshot);
  const hp = finiteNumber(payload?.hp);
  const mp = finiteNumber(payload?.mp);
  const maxHp = finiteNumber(payload?.maxHp);
  const maxMp = finiteNumber(payload?.maxMp);
  if (hp != null) {
    const current = Math.max(0, hp);
    snapshot.playerHp = current;
    if (player) {
      player.hp = current;
      if (current > 0) player.dead = false;
    }
  }
  if (mp != null) snapshot.playerMp = Math.max(0, mp);
  if (maxHp != null) {
    snapshot.playerMaxHp = Math.max(0, maxHp);
    if (player) player.maxHp = Math.max(0, maxHp);
  }
  if (maxMp != null) snapshot.playerMaxMp = Math.max(0, maxMp);
  const knownMaxHp = maxHp ?? finiteNumber(snapshot.playerMaxHp);
  if (hp != null && knownMaxHp != null && knownMaxHp > 0) {
    const percent = Math.max(0, Math.min(100, Math.round((Math.max(0, hp) * 100) / knownMaxHp)));
    snapshot.playerHealthPercent = percent;
    if (player) player.healthPercent = percent;
  }
}

/**
 * Fold public Gateway packets into a local observed world view. This state is
 * only a runner-side cache; a worldSnapshot always replaces it authoritatively.
 */
export function applyProtocolObservation(snapshot, message) {
  if (message?.type === 'worldSnapshot') {
    const authoritative = structuredClone(message.payload);
    if (authoritative && typeof authoritative === 'object') authoritative.mapSnapshotPending = false;
    return authoritative;
  }
  if (!snapshot || typeof snapshot !== 'object' || !message?.packet) return snapshot;

  // Never retain references into the recorded packet. The client appends its
  // raw messages asynchronously, so observation updates must not rewrite the
  // historical evidence that caused them.
  const payload = structuredClone(message.payload ?? {});
  switch (message.packet) {
    case 'UserInformation': {
      const objectId = objectIdOf(payload.objectId);
      if (objectId != null && objectIdOf(snapshot.playerObjectId) == null) snapshot.playerObjectId = objectId;
      upsertEntity(snapshot, payload, 'player');
      applySelfVitals(snapshot, payload);
      break;
    }
    case 'HealthChanged':
      applySelfVitals(snapshot, payload);
      break;
    case 'UserLocation':
      updateEntity(snapshot, snapshot.playerObjectId, payload);
      break;
    case 'Pushed':
    case 'UserDash':
    case 'UserDashFail':
      updateEntity(snapshot, snapshot.playerObjectId, payload);
      break;
    case 'ObjectTurn':
    case 'ObjectWalk':
    case 'ObjectRun':
    case 'ObjectBackStep':
    case 'ObjectPushed':
    case 'ObjectDash':
    case 'ObjectDashFail':
    case 'ObjectAttack':
    case 'ObjectRangeAttack':
    case 'ObjectMagic':
    case 'ObjectStruck':
      updateEntity(snapshot, payload.objectId, payload);
      break;
    case 'ObjectMonster':
    case 'NewMonsterInfo':
      upsertEntity(snapshot, observedMonsterPayload(snapshot, payload), 'monster');
      break;
    case 'ObjectPlayer':
      upsertEntity(snapshot, payload, 'player');
      break;
    case 'ObjectHero':
      upsertEntity(snapshot, payload, 'hero');
      break;
    case 'ObjectNpc':
    case 'NewNpcInfo':
      upsertEntity(snapshot, payload, 'npc');
      break;
    case 'ObjectHealth': {
      const entity = entityById(snapshot, payload.objectId);
      if (entity) {
        entity.healthPercent = payload.percent;
        entity.healthExpire = payload.expire;
      }
      if (objectIdOf(payload.objectId) === objectIdOf(snapshot.playerObjectId)) {
        snapshot.playerHealthPercent = payload.percent;
        snapshot.playerHealthExpire = payload.expire;
      }
      break;
    }
    case 'ObjectDied': {
      const entity = updateEntity(snapshot, payload.objectId, payload);
      if (entity) Object.assign(entity, { dead: true, hp: 0, healthPercent: 0 });
      break;
    }
    case 'Death': {
      const entity = selfEntity(snapshot);
      if (entity) Object.assign(entity, positionFields(payload), { dead: true, hp: 0, healthPercent: 0 });
      snapshot.playerHp = 0;
      snapshot.playerHealthPercent = 0;
      break;
    }
    case 'ObjectRevived': {
      const entity = entityById(snapshot, payload.objectId);
      if (entity) {
        Object.assign(entity, { dead: false, effect: payload.effect });
        delete entity.hp;
        delete entity.healthPercent;
        delete entity.healthExpire;
      }
      break;
    }
    case 'Revived': {
      const entity = selfEntity(snapshot);
      if (entity) {
        entity.dead = false;
        delete entity.hp;
        delete entity.healthPercent;
        delete entity.healthExpire;
      }
      delete snapshot.playerHp;
      delete snapshot.playerHealthPercent;
      delete snapshot.playerHealthExpire;
      break;
    }
    case 'ObjectHide': {
      const entity = entityById(snapshot, payload.objectId);
      if (entity) entity.hidden = true;
      break;
    }
    case 'ObjectShow': {
      const entity = entityById(snapshot, payload.objectId);
      if (entity) entity.hidden = false;
      break;
    }
    case 'ObjectHidden': {
      const entity = entityById(snapshot, payload.objectId);
      if (entity) entity.hidden = Boolean(payload.hidden);
      break;
    }
    case 'ObjectTeleportIn': {
      const entity = entityById(snapshot, payload.objectId);
      if (entity) entity.hidden = false;
      break;
    }
    case 'ObjectRemove':
    case 'ObjectTeleportOut':
      removeEntity(snapshot, payload.objectId);
      break;
    case 'MapChanged':
      applyMapChange(snapshot, payload);
      break;
    case 'MapInformation':
      applyMapInformation(snapshot, payload);
      break;
    default:
      break;
  }
  return snapshot;
}
