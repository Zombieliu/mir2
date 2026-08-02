"use client";

import { memo } from "react";

import type { ClientScreen } from "../../lib/original-ui";
import {
  EMPTY_VIEWPORT_OFFSET,
  DEFAULT_VIEWPORT_LAYOUT,
  VIEWPORT_CELL_HEIGHT,
  VIEWPORT_CELL_WIDTH,
  entityMotionOffsetForEntity,
  entityNameplateColor,
  entitySpriteHitBounds,
  ratio,
  viewportDepthForCell,
  type ViewportEntitySprite,
  type ViewportLayout,
  type ViewportOffset,
} from "./original-client-scene-rendering";
import type {
  DisplayDamageFloater,
  DisplayEntity,
  EntityKind,
  EntityMotionSnapshot,
  TranslateFn,
} from "./original-client-types";

// One over-head speech bubble, derived in the shell from recent chat/say log lines and matched to a
// visible speaker by name. `firstSeenAt` is stamped with the shell's motion clock so the shell can
// expire it on the existing render tick — this component never owns timers of its own.
export type SceneChatBubble = {
  objectId: string;
  text: string;
  channel: string;
  firstSeenAt: number;
};

type ViewportEntityEntry = {
  entity: DisplayEntity & { dx: number; dy: number };
  sprite: ViewportEntitySprite | null;
};

const CHAT_BUBBLE_FADE_MS = 600;
// A short white/red brighten over a struck sprite so a hit reads instantly even when the
// monster's atlas lacks struck frames (renderer-independent: it overlays the Bevy canvas too).
const HIT_FLASH_MS = 170;
const EMPTY_ENTITY_MOTION_SNAPSHOTS: Record<string, EntityMotionSnapshot> = {};
type RegisterEntityElement = (key: string, objectId: string) => (el: HTMLElement | null) => void;

// Screen-space top-left of an entity's tile origin, reusing the SAME transform the scene visual
// layers use (origin + tile delta * cell size + camera pan + per-entity motion lerp). Keeping this
// in lockstep with original-client-scene-visual-layers guarantees overlays track sprites exactly.
function entityOriginScreenPosition(
  entry: ViewportEntityEntry,
  isPlayer: boolean,
  cameraOffset: ViewportOffset,
  entityMotionSnapshots: Record<string, EntityMotionSnapshot>,
  motionNow: number,
  viewportLayout: ViewportLayout = DEFAULT_VIEWPORT_LAYOUT,
): { left: number; top: number } {
  const entityMotionOffset = isPlayer
    ? EMPTY_VIEWPORT_OFFSET
    : entityMotionOffsetForEntity(entry.entity, entityMotionSnapshots, motionNow);
  const appliedCamera = isPlayer ? EMPTY_VIEWPORT_OFFSET : cameraOffset;
  return {
    left:
      viewportLayout.entityLeftOrigin +
      entry.entity.dx * VIEWPORT_CELL_WIDTH +
      appliedCamera.x +
      entityMotionOffset.x,
    top:
      viewportLayout.entityTopOrigin +
      entry.entity.dy * VIEWPORT_CELL_HEIGHT +
      appliedCamera.y +
      entityMotionOffset.y,
  };
}

function healthBarColor(healthRatio: number): string {
  if (healthRatio <= 0.25) return "#d8352a";
  if (healthRatio <= 0.5) return "#e0a200";
  return "#00c000";
}

// Per-entity health bars. The shell already renders the *self* player's bar inside its sprite stack,
// so here we cover every OTHER damaged combat entity (players / monsters / heroes) whose hp is known.
function EntityHealthBars({
  entries,
  player,
  selectedEntity,
  playerCameraMotionOffset,
  entityMotionSnapshots,
  motionNow,
  entityKindClassName,
  registerEntityEl,
  viewportLayout = DEFAULT_VIEWPORT_LAYOUT,
}: {
  entries: ViewportEntityEntry[];
  player: DisplayEntity | null;
  selectedEntity: DisplayEntity | null;
  playerCameraMotionOffset: ViewportOffset;
  entityMotionSnapshots: Record<string, EntityMotionSnapshot>;
  motionNow: number;
  entityKindClassName: (kind: EntityKind) => string;
  registerEntityEl?: RegisterEntityElement;
  viewportLayout?: ViewportLayout;
}) {
  return (
    <>
      {entries.map((entry) => {
        const { entity, sprite } = entry;
        const isPlayer = player?.objectId === entity.objectId;
        // Self bar lives in the sprite stack already; NPCs are non-combat. Only draw where useful.
        if (isPlayer || entity.kind === "npc") {
          return null;
        }
        if (entity.dead || entity.hp === undefined || !entity.maxHp || entity.maxHp <= 0) {
          return null;
        }
        const healthRatio = ratio(entity.hp, entity.maxHp);
        // A full-health bystander is visual noise; reveal the bar once it has taken a hit or is the
        // active target so the selection always shows its condition.
        const isTarget = entity.objectId === selectedEntity?.objectId;
        if (healthRatio >= 1 && !isTarget) {
          return null;
        }
        const origin = entityOriginScreenPosition(
          entry,
          isPlayer,
          playerCameraMotionOffset,
          entityMotionSnapshots,
          motionNow,
          viewportLayout,
        );
        const bounds = entitySpriteHitBounds(sprite);
        const centerOffset = (bounds.left + bounds.right) / 2;
        return (
          <div
            key={`overlay-hp-${entity.objectId}`}
            ref={registerEntityEl?.(`overlay-hp:${entity.objectId}`, entity.objectId)}
            className={`scene-overlay-health ${entityKindClassName(entity.kind)} ${isTarget ? "is-target" : ""}`}
            style={{
              left: `${origin.left + centerOffset}px`,
              top: `${origin.top + bounds.top - 8}px`,
              zIndex: viewportDepthForCell(entity.x, entity.y, player ?? entity, 70),
            }}
          >
            <span style={{ width: `${healthRatio * 100}%`, background: healthBarColor(healthRatio) }} />
          </div>
        );
      })}
    </>
  );
}

// Over-head chat bubbles. Each bubble is anchored above its speaker's sprite and tracks the sprite
// while it lives; expiry is owned by the shell (it stops passing a bubble once it ages out).
function SceneChatBubbles({
  bubbles,
  entryByObjectId,
  player,
  playerCameraMotionOffset,
  entityMotionSnapshots,
  motionNow,
  registerEntityEl,
  viewportLayout = DEFAULT_VIEWPORT_LAYOUT,
}: {
  bubbles: SceneChatBubble[];
  entryByObjectId: Map<string, ViewportEntityEntry>;
  player: DisplayEntity | null;
  playerCameraMotionOffset: ViewportOffset;
  entityMotionSnapshots: Record<string, EntityMotionSnapshot>;
  motionNow: number;
  registerEntityEl?: RegisterEntityElement;
  viewportLayout?: ViewportLayout;
}) {
  return (
    <>
      {bubbles.map((bubble) => {
        const entry = entryByObjectId.get(bubble.objectId);
        if (!entry) {
          return null;
        }
        const isPlayer = player?.objectId === bubble.objectId;
        const origin = entityOriginScreenPosition(
          entry,
          isPlayer,
          playerCameraMotionOffset,
          entityMotionSnapshots,
          motionNow,
          viewportLayout,
        );
        const bounds = entitySpriteHitBounds(entry.sprite);
        const centerOffset = (bounds.left + bounds.right) / 2;
        const age = motionNow - bubble.firstSeenAt;
        const opacity = age < CHAT_BUBBLE_FADE_MS ? Math.min(1, age / CHAT_BUBBLE_FADE_MS) : 1;
        return (
          <div
            key={`bubble-${bubble.objectId}`}
            ref={registerEntityEl?.(`overlay-bubble:${bubble.objectId}`, bubble.objectId)}
            className={`scene-chat-bubble channel-${bubble.channel}`}
            style={{
              left: `${origin.left + centerOffset}px`,
              top: `${origin.top + bounds.top - 22}px`,
              opacity,
              zIndex: viewportDepthForCell(entry.entity.x, entry.entity.y, player ?? entry.entity, 96),
            }}
          >
            <span className="scene-chat-bubble-text">{bubble.text}</span>
          </div>
        );
      })}
    </>
  );
}

// Floating combat numbers (Crystal `GameScene.DamageIndicator`). Each number is anchored over its
// object's sprite-top and rises+fades via CSS over its lifetime; this component only positions it so
// it tracks a moving target. White over monsters, red over players, plus Miss/Crit/heal styling.
function DamageFloaters({
  floaters,
  entryByObjectId,
  player,
  playerCameraMotionOffset,
  entityMotionSnapshots,
  motionNow,
  registerEntityEl,
  viewportLayout = DEFAULT_VIEWPORT_LAYOUT,
}: {
  floaters: DisplayDamageFloater[];
  entryByObjectId: Map<string, ViewportEntityEntry>;
  player: DisplayEntity | null;
  playerCameraMotionOffset: ViewportOffset;
  entityMotionSnapshots: Record<string, EntityMotionSnapshot>;
  motionNow: number;
  registerEntityEl?: RegisterEntityElement;
  viewportLayout?: ViewportLayout;
}) {
  return (
    <>
      {floaters.map((floater) => {
        if (floater.expiresAt <= motionNow) {
          return null;
        }
        const entry = entryByObjectId.get(floater.objectId);
        if (!entry) {
          return null;
        }
        const isPlayer = player?.objectId === floater.objectId;
        const origin = entityOriginScreenPosition(
          entry,
          isPlayer,
          playerCameraMotionOffset,
          entityMotionSnapshots,
          motionNow,
          viewportLayout,
        );
        const bounds = entitySpriteHitBounds(entry.sprite);
        const centerOffset = (bounds.left + bounds.right) / 2;
        const lifeMs = Math.max(1, floater.expiresAt - floater.startedAt);
        return (
          <div
            key={floater.key}
            ref={registerEntityEl?.(`overlay-damage:${floater.key}`, floater.objectId)}
            className={`scene-damage-floater variant-${floater.variant} ${floater.isPlayerTarget ? "is-player" : "is-monster"}`}
            style={{
              left: `${origin.left + centerOffset}px`,
              top: `${origin.top + bounds.top - 6}px`,
              animationDuration: `${lifeMs}ms`,
              zIndex: viewportDepthForCell(entry.entity.x, entry.entity.y, player ?? entry.entity, 98),
            }}
          >
            {floater.text}
          </div>
        );
      })}
    </>
  );
}

// Brief brighten over a just-struck sprite. Driven by the entity's `struckStartedAt` (set for both
// monsters and the player), so the target visibly reacts on every landed hit regardless of which
// entity renderer (DOM / WebGL / Bevy) is active.
function HitFlashes({
  entries,
  player,
  playerCameraMotionOffset,
  entityMotionSnapshots,
  motionNow,
  registerEntityEl,
  viewportLayout = DEFAULT_VIEWPORT_LAYOUT,
}: {
  entries: ViewportEntityEntry[];
  player: DisplayEntity | null;
  playerCameraMotionOffset: ViewportOffset;
  entityMotionSnapshots: Record<string, EntityMotionSnapshot>;
  motionNow: number;
  registerEntityEl?: RegisterEntityElement;
  viewportLayout?: ViewportLayout;
}) {
  return (
    <>
      {entries.map((entry) => {
        const { entity, sprite } = entry;
        const struckAt = entity.struckStartedAt;
        if (typeof struckAt !== "number" || entity.dead) {
          return null;
        }
        const age = motionNow - struckAt;
        if (age < 0 || age > HIT_FLASH_MS) {
          return null;
        }
        const isPlayer = player?.objectId === entity.objectId;
        const origin = entityOriginScreenPosition(
          entry,
          isPlayer,
          playerCameraMotionOffset,
          entityMotionSnapshots,
          motionNow,
          viewportLayout,
        );
        const bounds = entitySpriteHitBounds(sprite);
        const opacity = Math.max(0, 1 - age / HIT_FLASH_MS) * 0.7;
        return (
          <div
            key={`hit-flash-${entity.objectId}`}
            ref={registerEntityEl?.(`overlay-hit:${entity.objectId}`, entity.objectId)}
            className={`scene-hit-flash ${isPlayer ? "is-player" : "is-monster"}`}
            style={{
              left: `${origin.left + bounds.left}px`,
              top: `${origin.top + bounds.top}px`,
              width: `${Math.max(0, bounds.right - bounds.left)}px`,
              height: `${Math.max(0, bounds.bottom - bounds.top)}px`,
              opacity,
              zIndex: viewportDepthForCell(entity.x, entity.y, player ?? entity, 97),
            }}
            aria-hidden="true"
          />
        );
      })}
    </>
  );
}

// Soft selection ring drawn at the selected entity's feet (the sprite stack only tints the body), so
// the active target reads clearly even amid a crowd.
function SelectionHighlight({
  selectedEntity,
  entryByObjectId,
  player,
  playerCameraMotionOffset,
  entityMotionSnapshots,
  motionNow,
  registerEntityEl,
  viewportLayout = DEFAULT_VIEWPORT_LAYOUT,
}: {
  selectedEntity: DisplayEntity | null;
  entryByObjectId: Map<string, ViewportEntityEntry>;
  player: DisplayEntity | null;
  playerCameraMotionOffset: ViewportOffset;
  entityMotionSnapshots: Record<string, EntityMotionSnapshot>;
  motionNow: number;
  registerEntityEl?: RegisterEntityElement;
  viewportLayout?: ViewportLayout;
}) {
  if (!selectedEntity) {
    return null;
  }
  const entry = entryByObjectId.get(selectedEntity.objectId);
  if (!entry) {
    return null;
  }
  const isPlayer = player?.objectId === selectedEntity.objectId;
  const origin = entityOriginScreenPosition(
    entry,
    isPlayer,
    playerCameraMotionOffset,
    entityMotionSnapshots,
    motionNow,
    viewportLayout,
  );
  const bounds = entitySpriteHitBounds(entry.sprite);
  const centerOffset = (bounds.left + bounds.right) / 2;
  return (
    <div
      ref={registerEntityEl?.(`overlay-selection:${selectedEntity.objectId}`, selectedEntity.objectId)}
      className={`scene-selection-ring ${selectedEntity.kind === "monster" ? "hostile" : "neutral"}`}
      style={{
        left: `${origin.left + centerOffset}px`,
        top: `${origin.top}px`,
        zIndex: viewportDepthForCell(entry.entity.x, entry.entity.y, player ?? entry.entity, 2),
      }}
      aria-hidden="true"
    />
  );
}

// Bottom-left readout for the currently selected target (name / level / hp / action+distance). The
// shell already holds selectedEntity + targetDistance + the action-label helper but never surfaced
// them on screen, so this fills an obviously-incomplete HUD slot without new data plumbing.
function SelectedTargetReadout({
  t,
  selectedEntity,
  targetActionLabel,
}: {
  t: TranslateFn;
  selectedEntity: DisplayEntity | null;
  targetActionLabel: string | null;
}) {
  if (!selectedEntity) {
    return null;
  }
  const nameColor = entityNameplateColor(selectedEntity);
  const hasHealth =
    selectedEntity.hp !== undefined && selectedEntity.maxHp !== undefined && selectedEntity.maxHp > 0;
  const healthRatio = hasHealth ? ratio(selectedEntity.hp, selectedEntity.maxHp) : 0;
  return (
    <div className="scene-target-readout" role="status" aria-live="polite">
      <div className="scene-target-readout-head">
        <strong style={{ color: nameColor }}>{selectedEntity.name.replace(/_/g, " ")}</strong>
        {selectedEntity.level !== undefined ? (
          <span className="scene-target-readout-level">
            {t("ui.level", [selectedEntity.level], `Lv ${selectedEntity.level}`)}
          </span>
        ) : null}
      </div>
      {hasHealth ? (
        <div className="scene-target-readout-health">
          <span style={{ width: `${healthRatio * 100}%`, background: healthBarColor(healthRatio) }} />
          {selectedEntity.maxHp ? (
            <em>
              {Math.max(0, Math.round(selectedEntity.hp ?? 0))}/{Math.round(selectedEntity.maxHp)}
            </em>
          ) : null}
        </div>
      ) : null}
      <span className="scene-target-readout-action">
        {selectedEntity.dead ? t("ui.dead", [], "Dead") : targetActionLabel ?? ""}
      </span>
    </div>
  );
}

// Inline (no globals.css) styling for every overlay this module owns. Pixel-art aesthetic matches the
// existing nameplate / health-bar styling in globals.css.
const OVERLAY_STYLES = `
.scene-overlay-layer {
  position: absolute;
  inset: 0;
  /* Above the sprite/nameplate overlays (z 5-6), below the HUD windows (z 10). */
  z-index: 7;
  pointer-events: none;
}
.scene-overlay-health {
  position: absolute;
  width: 34px;
  height: 4px;
  transform: translateX(-50%);
  border: 1px solid #101010;
  background: #270000;
  box-shadow: 1px 1px 0 #000;
}
.scene-overlay-health.is-target {
  width: 40px;
  height: 5px;
  box-shadow: 0 0 4px rgba(255, 226, 150, 0.55), 1px 1px 0 #000;
}
.scene-overlay-health > span {
  display: block;
  height: 100%;
  background: #00c000;
  transition: width 120ms linear;
}
.scene-chat-bubble {
  position: absolute;
  max-width: 220px;
  transform: translate(-50%, -100%);
  padding: 3px 7px;
  border: 1px solid rgba(20, 20, 20, 0.92);
  border-radius: 7px;
  background: rgba(252, 250, 240, 0.95);
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.55);
  text-align: center;
}
.scene-chat-bubble::after {
  content: "";
  position: absolute;
  left: 50%;
  bottom: -5px;
  width: 8px;
  height: 8px;
  transform: translateX(-50%) rotate(45deg);
  background: inherit;
  border-right: 1px solid rgba(20, 20, 20, 0.92);
  border-bottom: 1px solid rgba(20, 20, 20, 0.92);
}
.scene-chat-bubble-text {
  display: block;
  font-size: 12px;
  line-height: 1.2;
  color: #1a1a1a;
  white-space: pre-wrap;
  word-break: break-word;
}
.scene-chat-bubble.channel-shout {
  background: rgba(255, 236, 210, 0.96);
  border-color: #8a3a12;
}
.scene-chat-bubble.channel-shout .scene-chat-bubble-text {
  color: #7a2e08;
  font-weight: 700;
}
.scene-chat-bubble.channel-whisper {
  background: rgba(228, 240, 255, 0.96);
  border-color: #2a5a9a;
}
.scene-chat-bubble.channel-whisper .scene-chat-bubble-text {
  color: #1d3f72;
  font-style: italic;
}
.scene-chat-bubble.channel-group .scene-chat-bubble-text {
  color: #155724;
}
.scene-chat-bubble.channel-guild .scene-chat-bubble-text {
  color: #5a3d8a;
}
.scene-selection-ring {
  position: absolute;
  width: 46px;
  height: 22px;
  transform: translate(-50%, -50%);
  border-radius: 50%;
  border: 2px solid rgba(255, 228, 152, 0.85);
  box-shadow: 0 0 8px rgba(255, 228, 152, 0.45), inset 0 0 6px rgba(255, 228, 152, 0.4);
  background: radial-gradient(closest-side, rgba(255, 228, 152, 0.12), transparent);
  animation: scene-selection-pulse 1.4s ease-in-out infinite;
}
.scene-selection-ring.hostile {
  border-color: rgba(255, 150, 130, 0.9);
  box-shadow: 0 0 8px rgba(255, 120, 96, 0.5), inset 0 0 6px rgba(255, 120, 96, 0.45);
  background: radial-gradient(closest-side, rgba(255, 120, 96, 0.14), transparent);
}
@keyframes scene-selection-pulse {
  0%, 100% { opacity: 0.55; }
  50% { opacity: 1; }
}
.scene-target-readout {
  position: absolute;
  left: 16px;
  bottom: 132px;
  min-width: 168px;
  max-width: 240px;
  padding: 6px 9px;
  border: 1px solid rgba(110, 86, 40, 0.85);
  border-radius: 4px;
  background: linear-gradient(180deg, rgba(28, 22, 12, 0.92), rgba(16, 12, 6, 0.92));
  box-shadow: 0 2px 6px rgba(0, 0, 0, 0.55);
  display: grid;
  gap: 4px;
  pointer-events: none;
}
.scene-target-readout-head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 8px;
}
.scene-target-readout-head strong {
  font-size: 13px;
  font-weight: 600;
  text-shadow: 1px 1px 0 #000;
}
.scene-target-readout-level {
  font-size: 11px;
  color: #e7d7a4;
  white-space: nowrap;
}
.scene-target-readout-health {
  position: relative;
  height: 9px;
  border: 1px solid #0e0e0e;
  background: #270000;
  overflow: hidden;
}
.scene-target-readout-health > span {
  display: block;
  height: 100%;
  background: #00c000;
  transition: width 120ms linear;
}
.scene-target-readout-health > em {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 9px;
  font-style: normal;
  color: #fff;
  text-shadow: 1px 1px 0 #000;
}
.scene-target-readout-action {
  font-size: 11px;
  color: #f0dc9a;
}
.scene-damage-floater {
  position: absolute;
  font-size: 15px;
  font-weight: 700;
  line-height: 1;
  white-space: nowrap;
  color: #f4f4f4;
  text-shadow: 1px 1px 0 #000, -1px 1px 0 #000, 1px -1px 0 #000, -1px -1px 0 #000;
  will-change: transform, opacity;
  animation-name: scene-damage-rise;
  animation-timing-function: ease-out;
  animation-fill-mode: forwards;
}
/* Crystal: Hit = white over monsters / red over players; Miss = grey-ish; Crit = dark red. */
.scene-damage-floater.is-player {
  color: #ff5a4d;
}
.scene-damage-floater.variant-miss {
  color: #cfcfcf;
  font-weight: 600;
  font-size: 13px;
}
.scene-damage-floater.variant-miss.is-player {
  color: #ff9d92;
}
.scene-damage-floater.variant-crit {
  color: #ff3b2f;
  font-size: 18px;
}
.scene-damage-floater.variant-heal {
  color: #6bff7a;
}
@keyframes scene-damage-rise {
  0% { opacity: 0; transform: translate(-50%, 6px) scale(0.8); }
  16% { opacity: 1; transform: translate(-50%, -3px) scale(1.08); }
  30% { transform: translate(-50%, -7px) scale(1); }
  100% { opacity: 0; transform: translate(-50%, -30px) scale(1); }
}
.scene-hit-flash {
  position: absolute;
  border-radius: 42% 42% 38% 38%;
  background: radial-gradient(closest-side, rgba(255, 255, 255, 0.95), rgba(255, 255, 255, 0.3) 65%, transparent);
  mix-blend-mode: screen;
}
.scene-hit-flash.is-player {
  background: radial-gradient(closest-side, rgba(255, 96, 84, 0.95), rgba(255, 64, 52, 0.28) 65%, transparent);
}
`;

function OriginalClientSceneOverlaysInner({
  screen,
  t,
  player,
  selectedEntity,
  viewportEntitySprites,
  playerCameraMotionOffset,
  entityMotionSnapshots,
  motionNow,
  imperativeCamera,
  registerCameraSurface,
  registerEntityEl,
  chatBubbles,
  damageFloaters,
  targetActionLabel,
  entityKindClassName,
  viewportLayout = DEFAULT_VIEWPORT_LAYOUT,
}: {
  screen: ClientScreen;
  t: TranslateFn;
  player: DisplayEntity | null;
  selectedEntity: DisplayEntity | null;
  viewportEntitySprites: ViewportEntityEntry[];
  playerCameraMotionOffset: ViewportOffset;
  entityMotionSnapshots: Record<string, EntityMotionSnapshot>;
  motionNow: number;
  imperativeCamera: boolean;
  registerCameraSurface: (key: string) => (el: HTMLElement | null) => void;
  registerEntityEl: RegisterEntityElement;
  chatBubbles: SceneChatBubble[];
  damageFloaters: DisplayDamageFloater[];
  targetActionLabel: string | null;
  entityKindClassName: (kind: EntityKind) => string;
  viewportLayout?: ViewportLayout;
}) {
  if (screen !== "game" || !player) {
    return null;
  }

  const entryByObjectId = new Map<string, ViewportEntityEntry>();
  for (const entry of viewportEntitySprites) {
    entryByObjectId.set(entry.entity.objectId, entry);
  }
  const overlayMotionSnapshots = imperativeCamera
    ? EMPTY_ENTITY_MOTION_SNAPSHOTS
    : entityMotionSnapshots;
  const overlayEntityRegistration = imperativeCamera ? registerEntityEl : undefined;

  return (
    <div
      className="scene-overlay-layer"
      aria-hidden={chatBubbles.length === 0 && damageFloaters.length === 0 && !selectedEntity}
    >
      <style>{OVERLAY_STYLES}</style>
      {/* World-positioned overlays pan with the camera. In the imperative path the
          motion driver writes this surface's transform at display Hz; the fixed HUD
          readout below stays OUTSIDE it so it does not pan. */}
      <div
        ref={imperativeCamera ? registerCameraSurface("overlays") : undefined}
        style={
          imperativeCamera
            ? { position: "absolute", inset: 0, pointerEvents: "none", willChange: "transform" }
            : { display: "contents" }
        }
      >
      <HitFlashes
        entries={viewportEntitySprites}
        player={player}
        playerCameraMotionOffset={playerCameraMotionOffset}
        entityMotionSnapshots={overlayMotionSnapshots}
        motionNow={motionNow}
        registerEntityEl={overlayEntityRegistration}
        viewportLayout={viewportLayout}
      />
      <SelectionHighlight
        selectedEntity={selectedEntity}
        entryByObjectId={entryByObjectId}
        player={player}
        playerCameraMotionOffset={playerCameraMotionOffset}
        entityMotionSnapshots={overlayMotionSnapshots}
        motionNow={motionNow}
        registerEntityEl={overlayEntityRegistration}
        viewportLayout={viewportLayout}
      />
      <EntityHealthBars
        entries={viewportEntitySprites}
        player={player}
        selectedEntity={selectedEntity}
        playerCameraMotionOffset={playerCameraMotionOffset}
        entityMotionSnapshots={overlayMotionSnapshots}
        motionNow={motionNow}
        entityKindClassName={entityKindClassName}
        registerEntityEl={overlayEntityRegistration}
        viewportLayout={viewportLayout}
      />
      <SceneChatBubbles
        bubbles={chatBubbles}
        entryByObjectId={entryByObjectId}
        player={player}
        playerCameraMotionOffset={playerCameraMotionOffset}
        entityMotionSnapshots={overlayMotionSnapshots}
        motionNow={motionNow}
        registerEntityEl={overlayEntityRegistration}
        viewportLayout={viewportLayout}
      />
      <DamageFloaters
        floaters={damageFloaters}
        entryByObjectId={entryByObjectId}
        player={player}
        playerCameraMotionOffset={playerCameraMotionOffset}
        entityMotionSnapshots={overlayMotionSnapshots}
        motionNow={motionNow}
        registerEntityEl={overlayEntityRegistration}
        viewportLayout={viewportLayout}
      />
      </div>
      <SelectedTargetReadout t={t} selectedEntity={selectedEntity} targetActionLabel={targetActionLabel} />
    </div>
  );
}

// Memoised so the 30Hz motion tick skips the overlay layer (hit flashes / floaters / chat bubbles /
// selection / target readout) when none of its shell-memoised inputs changed.
export const OriginalClientSceneOverlays = memo(OriginalClientSceneOverlaysInner);

// Re-export so consumers needing the bubble shape can import it from one place.
export type { ViewportEntityEntry as SceneOverlayEntityEntry };
