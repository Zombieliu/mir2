import { projectBevyBagModel, type BevyBagWorldSource, type BevyInventoryModel } from "./bevy-bag-model";
import type { HudPlayer } from "./bevy-hud-ui";
import type { WorldState } from "./world-model/types";

/** Quest's scene and player proof; Bag/HUD ownership does not supply this read model. */
export type QuestWorldIdentity = Readonly<{
  connectionGeneration: number;
  sessionGeneration: number;
  sceneRevision: number;
  playerObjectId: number;
  mapFileName: string;
}>;

export type QuestWorldStamp = QuestWorldIdentity & Readonly<{
  generation: number;
  revision: number;
}>;

export type QuestWorldDraft = QuestWorldIdentity & Readonly<{
  generation: number;
  inventory: BevyInventoryModel;
  player: QuestSupplyPlayer;
  knownSkills?: readonly unknown[];
  mapIndex: number | null;
  entities: readonly QuestWorldEntity[];
  selectedObjectId: number | null;
  groundDrops: readonly QuestGroundDrop[];
}>;

export type QuestWorldEntity = Readonly<{
  objectId: number;
  kind: "selfPlayer" | "player" | "monster" | "npc";
  name: string;
  x: number;
  y: number;
  direction: string | null;
  level: number | null;
  dead: boolean | null;
  hp: number | null;
  maxHp: number | null;
}>;

export type QuestGroundDrop = Readonly<{
  objectId: number;
  name: string;
  x: number;
  y: number;
  quantity: number;
  sourceMonster: string;
}>;

export type QuestWorldContext = QuestWorldDraft & Readonly<{ revision: number }>;
export type QuestSupplyPlayer = Pick<HudPlayer,
  "level" | "className" | "gender" | "gold" | "weights" | "currentWeight" | "currentWeightKnown" | "maxWeight">;

export type QuestWorldSource = BevyBagWorldSource & Pick<WorldState,
  "connected" | "mapFileName" | "playerObjectId" | "entities" | "currentWeight" | "maxWeight"
  | "selectedObjectId" | "groundDrops">;

/** Old bundles reject unknown Quest DTO fields, so only exact version 2 opts in. */
export function supportsBevyQuestWorldContext(runtime: {
  getMir2QuestWorldContextVersion?: () => unknown;
} | null): boolean {
  try { return runtime?.getMir2QuestWorldContextVersion?.() === 2; }
  catch { return false; }
}

/** Match page.tsx's map identity normalization without inventing map 0 for absence. */
export function normalizeQuestMapFileName(value: WorldState["mapFileName"]): string | null {
  if (typeof value !== "string") return null;
  const normalized = value.trim().replace(/\.map$/i, "").toLowerCase();
  return normalized.length > 0 && normalized.length <= 128 && !normalized.includes("\0")
    ? normalized : null;
}

function positiveSafe(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value > 0;
}

function questText(value: unknown, maxBytes = 256): value is string {
  return typeof value === "string" && !value.includes("\0") && new TextEncoder().encode(value).length <= maxBytes;
}

export function questObjectId(value: unknown): number | null {
  if (typeof value !== "string" || !/^[1-9][0-9]*$/.test(value)) return null;
  const id = Number(value);
  return positiveSafe(id) && id <= 0xffff_ffff && String(id) === value ? id : null;
}

export function questCoordinate(value: unknown): value is number {
  return typeof value === "number" && Number.isInteger(value)
    && value >= -0x8000_0000 && value <= 0x7fff_ffff;
}

export function questMapIndex(value: unknown): value is number {
  return questCoordinate(value) && value >= 0;
}

function projectQuestEntities(world: QuestWorldSource): QuestWorldEntity[] | null {
  if (!Array.isArray(world.entities) || world.entities.length > 512) return null;
  const ids = new Set<number>();
  const rows: QuestWorldEntity[] = [];
  for (const entity of world.entities) {
    const id = questObjectId(entity.objectId);
    if (id === null || ids.has(id) || !["selfPlayer", "player", "monster", "npc"].includes(entity.kind)
      || !questText(entity.name)
      || !questCoordinate(entity.x) || !questCoordinate(entity.y)) return null;
    if (entity.level != null && (!Number.isSafeInteger(entity.level) || entity.level < 0 || entity.level > 0xffff_ffff)
      || entity.direction != null && !questText(entity.direction, 32)
      || entity.dead != null && typeof entity.dead !== "boolean"
      || entity.hp != null && !questCoordinate(entity.hp)
      || entity.maxHp != null && !questCoordinate(entity.maxHp)) return null;
    ids.add(id);
    rows.push({ objectId: id, kind: entity.kind, name: entity.name, x: entity.x, y: entity.y,
      direction: entity.direction ?? null, level: entity.level ?? null,
      dead: entity.dead ?? null, hp: entity.hp ?? null, maxHp: entity.maxHp ?? null });
  }
  return rows;
}

function projectQuestGroundDrops(world: QuestWorldSource, entities: readonly QuestWorldEntity[]): QuestGroundDrop[] | null {
  if (!Array.isArray(world.groundDrops) || world.groundDrops.length > 512) return null;
  const ids = new Set(entities.map(entity => entity.objectId));
  const rows: QuestGroundDrop[] = [];
  for (const drop of world.groundDrops) {
    const id = questObjectId(drop.objectId);
    if (id === null || ids.has(id) || !questText(drop.name)
      || !questCoordinate(drop.x) || !questCoordinate(drop.y)
      || !Number.isSafeInteger(drop.quantity) || drop.quantity <= 0 || drop.quantity > 0xffff_ffff
      || !questText(drop.sourceMonster)) return null;
    ids.add(id);
    rows.push({ objectId: id, name: drop.name, x: drop.x, y: drop.y,
      quantity: drop.quantity, sourceMonster: drop.sourceMonster });
  }
  return rows;
}

function completeInventory(value: unknown): value is BevyInventoryModel {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return false;
  const model = value as Partial<BevyInventoryModel>;
  return Object.hasOwn(value, "capacity") && typeof model.capacity === "number"
    && Number.isSafeInteger(model.capacity) && model.capacity >= 0
    && Object.hasOwn(value, "gold") && typeof model.gold === "number"
    && Number.isSafeInteger(model.gold) && model.gold >= 0 && model.gold <= 0xffff_ffff
    && Object.hasOwn(value, "items") && Array.isArray(model.items);
}

function completePlayer(value: unknown): value is QuestSupplyPlayer {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return false;
  const player = value as Partial<QuestSupplyPlayer>;
  return typeof player.className === "string" && player.className.length > 0
    && (player.gender === "male" || player.gender === "female")
    && typeof player.level === "number" && Number.isSafeInteger(player.level) && player.level > 0
    && typeof player.gold === "number" && Number.isSafeInteger(player.gold)
    && player.gold >= 0 && player.gold <= 0xffff_ffff
    && typeof player.currentWeight === "number" && Number.isSafeInteger(player.currentWeight)
    && player.currentWeight >= 0 && player.currentWeight <= 0xffff
    && typeof player.maxWeight === "number" && Number.isSafeInteger(player.maxWeight)
    && player.maxWeight >= 0 && player.maxWeight <= 0xffff
    && typeof player.currentWeightKnown === "boolean"
    && (player.weights === undefined || player.weights !== null
      && [player.weights.bag, player.weights.wear, player.weights.hand].every((v) => Number.isSafeInteger(v) && v >= 0 && v <= 0xffff_ffff));
}

function validIdentity(value: Partial<QuestWorldIdentity> | null): value is QuestWorldIdentity {
  return value !== null && typeof value === "object"
    && positiveSafe(value.connectionGeneration)
    && positiveSafe(value.sessionGeneration)
    && positiveSafe(value.sceneRevision)
    && positiveSafe(value.playerObjectId) && value.playerObjectId <= 0xffff_ffff
    && typeof value.mapFileName === "string" && value.mapFileName.length > 0
    && normalizeQuestMapFileName(value.mapFileName) === value.mapFileName;
}

export function sameQuestWorldIdentity(a: Partial<QuestWorldIdentity> | null, b: Partial<QuestWorldIdentity> | null): boolean {
  return validIdentity(a) && validIdentity(b)
    && a.connectionGeneration === b.connectionGeneration
    && a.sessionGeneration === b.sessionGeneration
    && a.sceneRevision === b.sceneRevision
    && a.playerObjectId === b.playerObjectId
    && a.mapFileName === b.mapFileName;
}

function completeWorldRows(value: QuestWorldDraft): boolean {
  if (!Object.hasOwn(value, "mapIndex") || !(value.mapIndex === null || questMapIndex(value.mapIndex))
    || !Object.hasOwn(value, "selectedObjectId")
    || !(value.selectedObjectId === null || positiveSafe(value.selectedObjectId) && value.selectedObjectId <= 0xffff_ffff)
    || !Array.isArray(value.entities) || value.entities.length > 512
    || !Array.isArray(value.groundDrops) || value.groundDrops.length > 512) return false;
  const ids = new Set<number>();
  let selfCount = 0;
  for (const row of value.entities) {
    if (!row || !positiveSafe(row.objectId) || row.objectId > 0xffff_ffff || ids.has(row.objectId)
      || !["selfPlayer", "player", "monster", "npc"].includes(row.kind) || !questText(row.name)
      || !questCoordinate(row.x) || !questCoordinate(row.y)
      || !(row.direction === null || questText(row.direction, 32))
      || !(row.level === null || typeof row.level === "number" && Number.isSafeInteger(row.level) && row.level >= 0 && row.level <= 0xffff_ffff)
      || !(row.dead === null || typeof row.dead === "boolean")
      || !(row.hp === null || questCoordinate(row.hp)) || !(row.maxHp === null || questCoordinate(row.maxHp))) return false;
    if (row.kind === "selfPlayer") { selfCount++; if (row.objectId !== value.playerObjectId) return false; }
    else if (row.objectId === value.playerObjectId) return false;
    ids.add(row.objectId);
  }
  if (selfCount !== 1) return false;
  for (const row of value.groundDrops) {
    if (!row || !positiveSafe(row.objectId) || row.objectId > 0xffff_ffff || ids.has(row.objectId)
      || !questCoordinate(row.x) || !questCoordinate(row.y) || !questText(row.name) || !questText(row.sourceMonster)
      || !positiveSafe(row.quantity) || row.quantity > 0xffff_ffff) return false;
    ids.add(row.objectId);
  }
  return true;
}

/** A complete same-world projection; invalid or partial bag data stays unavailable. */
export function projectBevyQuestWorldDraft(
  world: QuestWorldSource,
  captured: QuestWorldIdentity | null,
  current: QuestWorldIdentity | null,
  generation: number,
  player: HudPlayer | null,
  knownSkills?: readonly unknown[] | null,
  mapIndex: number | null = null,
): QuestWorldDraft | null {
  if (!positiveSafe(generation) || !captured || !current || !sameQuestWorldIdentity(captured, current)
    || world.connected !== true || normalizeQuestMapFileName(world.mapFileName) !== current.mapFileName
    || world.playerObjectId !== String(current.playerObjectId)
    || !Array.isArray(world.entities)) return null;
  try {
    const self = world.entities.filter((entity) => entity.objectId === world.playerObjectId);
    if (self.length !== 1 || self[0]?.kind !== "selfPlayer"
      || world.entities.some(entity => entity.kind === "selfPlayer" && entity.objectId !== world.playerObjectId)) return null;
    if (mapIndex !== null && !questMapIndex(mapIndex)) return null;
    const entities = projectQuestEntities(world);
    const groundDrops = entities && projectQuestGroundDrops(world, entities);
    const selectedObjectId = world.selectedObjectId === null ? null : questObjectId(world.selectedObjectId);
    if (!entities || !groundDrops || world.selectedObjectId !== null && selectedObjectId === null) return null;
    const projected = projectBevyBagModel(world);
    if (!Number.isSafeInteger(world.currentWeight) || world.currentWeight < 0 || world.currentWeight > 0xffff
      || !Number.isSafeInteger(world.maxWeight) || world.maxWeight < 0 || world.maxWeight > 0xffff) return null;
    if (!projected.ok || !completePlayer(player) || player.gold !== projected.model.gold
      || player.level !== self[0].level || player.className !== self[0].classKey
      || player.gender !== self[0].genderKey) return null;
    const supplyPlayer: QuestSupplyPlayer = { level: player.level, className: player.className,
      gender: player.gender, gold: player.gold, currentWeight: player.currentWeight,
      currentWeightKnown: player.currentWeightKnown, maxWeight: player.maxWeight,
      ...(player.weights ? { weights: { ...player.weights } } : {}) };
    return { ...current, generation, inventory: projected.model, player: supplyPlayer,
      mapIndex, entities, selectedObjectId, groundDrops,
      ...(Array.isArray(knownSkills) && knownSkills.length <= 512 ? { knownSkills: [...knownSkills] } : {}) };
  } catch { return null; }
}

/** The Quest hook assigns the revision only when it sends this exact snapshot. */
export function stampBevyQuestWorldContext(draft: QuestWorldDraft | null, revision: number): QuestWorldContext | null {
  return draft && validIdentity(draft) && positiveSafe(draft.generation) && positiveSafe(revision)
    && completeInventory(draft.inventory) && completePlayer(draft.player) && completeWorldRows(draft)
    ? { ...draft, revision } : null;
}

export function sameQuestWorldStamp(actual: Partial<QuestWorldStamp> | null, expected: QuestWorldStamp | null): boolean {
  return actual !== null && expected !== null
    && positiveSafe(actual.generation) && positiveSafe(expected.generation)
    && positiveSafe(actual.revision) && positiveSafe(expected.revision)
    && actual.generation === expected.generation && actual.revision === expected.revision
    && sameQuestWorldIdentity(actual, expected);
}

/** Reproject live input before an action: an inventory mutation cannot reuse the prior UI proof. */
export function sameQuestWorldDraft(actual: QuestWorldDraft | null, expected: QuestWorldDraft | null): boolean {
  if (!actual || !expected || !positiveSafe(actual.generation)
    || actual.generation !== expected.generation || !sameQuestWorldIdentity(actual, expected)
    || !completeInventory(actual.inventory) || !completeInventory(expected.inventory)
    || !completePlayer(actual.player) || !completePlayer(expected.player)
    || !completeWorldRows(actual) || !completeWorldRows(expected)) return false;
  try { return JSON.stringify(actual.inventory) === JSON.stringify(expected.inventory)
    && JSON.stringify(actual.player) === JSON.stringify(expected.player)
    && JSON.stringify(actual.knownSkills) === JSON.stringify(expected.knownSkills)
    && actual.mapIndex === expected.mapIndex && actual.selectedObjectId === expected.selectedObjectId
    && JSON.stringify(actual.entities) === JSON.stringify(expected.entities)
    && JSON.stringify(actual.groundDrops) === JSON.stringify(expected.groundDrops); }
  catch { return false; }
}
