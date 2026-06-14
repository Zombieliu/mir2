/**
 * Public API barrel for the world-model module.
 *
 * Import from this file in consuming code:
 *   import { createWorldStore, createSnapshotEmitter } from "../lib/world-model";
 */

export type {
  // Types
  WorldState,
  WorldEntity,
  WorldEntitySprite,
  WorldItem,
  EquipmentItem,
  GroundDrop,
  DamageFloater,
  ProjectileState,
  QuestEntry,
  NpcDialog,
  KnownSkill,
  ActiveBuff,
  MapTransferArea,
  RankingEntry,
  RankingState,
  Stage5SystemsState,
  EntityKind,
  EntityDisposition,
  ItemContainer,
  EquipmentSlot,
  QuestStage,
} from "./types";
export { DEFAULT_WORLD_STATE } from "./types";

export type { WorldStore } from "./store";
export { createWorldStore } from "./store";

export type { SnapshotEmitter, SnapshotEmitterOptions } from "./snapshot-emitter";
export { createSnapshotEmitter } from "./snapshot-emitter";
