// Barrel exports for lib/game-events/.
//
// Import the bus factory and subscriber wiring from here; individual type-only
// imports can come from the sub-modules directly.

export type {
  GameEvent,
  GameEventType,
  GameEventOf,
  CrystalDamageType,
  SoundEntityRef,
  EntityAttackEvent,
  EntityStruckEvent,
  EntityDiedEvent,
  MagicCastEvent,
  PlaySoundEvent,
  GainedGoldEvent,
  MapMusicChangedEvent,
  DamageDealtEvent,
} from "./events";

export type {
  GameEventHandler,
  AnyGameEventHandler,
  Unsubscribe,
  GameEventBus,
} from "./bus";
export { createGameEventBus } from "./bus";

export type { AudioSink } from "./sound-subscriber";
export {
  registerSoundSubscriber,
  makeRealAudioSink,
  registerDefaultSoundSubscriber,
} from "./sound-subscriber";

export type { DamageFloaterPayload, VfxSink } from "./vfx-subscriber";
export { registerVfxSubscriber } from "./vfx-subscriber";
