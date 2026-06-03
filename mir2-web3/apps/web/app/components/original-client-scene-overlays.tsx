"use client";

import type { ClientScreen } from "../../lib/original-ui";
import {
  EMPTY_VIEWPORT_OFFSET,
  VIEWPORT_CELL_HEIGHT,
  VIEWPORT_CELL_WIDTH,
  VIEWPORT_ENTITY_LEFT_ORIGIN,
  VIEWPORT_ENTITY_TOP_ORIGIN,
  entityMotionOffsetForEntity,
  entityNameplateColor,
  entitySpriteHitBounds,
  ratio,
  viewportDepthForCell,
  type ViewportEntitySprite,
  type ViewportOffset,
} from "./original-client-scene-rendering";
import type {
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

// Screen-space top-left of an entity's tile origin, reusing the SAME transform the scene visual
// layers use (origin + tile delta * cell size + camera pan + per-entity motion lerp). Keeping this
// in lockstep with original-client-scene-visual-layers guarantees overlays track sprites exactly.
function entityOriginScreenPosition(
  entry: ViewportEntityEntry,
  isPlayer: boolean,
  cameraOffset: ViewportOffset,
  entityMotionSnapshots: Record<string, EntityMotionSnapshot>,
  motionNow: number,
): { left: number; top: number } {
  const entityMotionOffset = isPlayer
    ? EMPTY_VIEWPORT_OFFSET
    : entityMotionOffsetForEntity(entry.entity, entityMotionSnapshots, motionNow);
  const appliedCamera = isPlayer ? EMPTY_VIEWPORT_OFFSET : cameraOffset;
  return {
    left:
      VIEWPORT_ENTITY_LEFT_ORIGIN +
      entry.entity.dx * VIEWPORT_CELL_WIDTH +
      appliedCamera.x +
      entityMotionOffset.x,
    top:
      VIEWPORT_ENTITY_TOP_ORIGIN +
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
}: {
  entries: ViewportEntityEntry[];
  player: DisplayEntity | null;
  selectedEntity: DisplayEntity | null;
  playerCameraMotionOffset: ViewportOffset;
  entityMotionSnapshots: Record<string, EntityMotionSnapshot>;
  motionNow: number;
  entityKindClassName: (kind: EntityKind) => string;
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
        );
        const bounds = entitySpriteHitBounds(sprite);
        const centerOffset = (bounds.left + bounds.right) / 2;
        return (
          <div
            key={`overlay-hp-${entity.objectId}`}
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
}: {
  bubbles: SceneChatBubble[];
  entryByObjectId: Map<string, ViewportEntityEntry>;
  player: DisplayEntity | null;
  playerCameraMotionOffset: ViewportOffset;
  entityMotionSnapshots: Record<string, EntityMotionSnapshot>;
  motionNow: number;
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
        );
        const bounds = entitySpriteHitBounds(entry.sprite);
        const centerOffset = (bounds.left + bounds.right) / 2;
        const age = motionNow - bubble.firstSeenAt;
        const opacity = age < CHAT_BUBBLE_FADE_MS ? Math.min(1, age / CHAT_BUBBLE_FADE_MS) : 1;
        return (
          <div
            key={`bubble-${bubble.objectId}`}
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

// Soft selection ring drawn at the selected entity's feet (the sprite stack only tints the body), so
// the active target reads clearly even amid a crowd.
function SelectionHighlight({
  selectedEntity,
  entryByObjectId,
  player,
  playerCameraMotionOffset,
  entityMotionSnapshots,
  motionNow,
}: {
  selectedEntity: DisplayEntity | null;
  entryByObjectId: Map<string, ViewportEntityEntry>;
  player: DisplayEntity | null;
  playerCameraMotionOffset: ViewportOffset;
  entityMotionSnapshots: Record<string, EntityMotionSnapshot>;
  motionNow: number;
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
  );
  const bounds = entitySpriteHitBounds(entry.sprite);
  const centerOffset = (bounds.left + bounds.right) / 2;
  return (
    <div
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
`;

export function OriginalClientSceneOverlays({
  screen,
  t,
  player,
  selectedEntity,
  viewportEntitySprites,
  playerCameraMotionOffset,
  entityMotionSnapshots,
  motionNow,
  chatBubbles,
  targetActionLabel,
  entityKindClassName,
}: {
  screen: ClientScreen;
  t: TranslateFn;
  player: DisplayEntity | null;
  selectedEntity: DisplayEntity | null;
  viewportEntitySprites: ViewportEntityEntry[];
  playerCameraMotionOffset: ViewportOffset;
  entityMotionSnapshots: Record<string, EntityMotionSnapshot>;
  motionNow: number;
  chatBubbles: SceneChatBubble[];
  targetActionLabel: string | null;
  entityKindClassName: (kind: EntityKind) => string;
}) {
  if (screen !== "game" || !player) {
    return null;
  }

  const entryByObjectId = new Map<string, ViewportEntityEntry>();
  for (const entry of viewportEntitySprites) {
    entryByObjectId.set(entry.entity.objectId, entry);
  }

  return (
    <div className="scene-overlay-layer" aria-hidden={chatBubbles.length === 0 && !selectedEntity}>
      <style>{OVERLAY_STYLES}</style>
      <SelectionHighlight
        selectedEntity={selectedEntity}
        entryByObjectId={entryByObjectId}
        player={player}
        playerCameraMotionOffset={playerCameraMotionOffset}
        entityMotionSnapshots={entityMotionSnapshots}
        motionNow={motionNow}
      />
      <EntityHealthBars
        entries={viewportEntitySprites}
        player={player}
        selectedEntity={selectedEntity}
        playerCameraMotionOffset={playerCameraMotionOffset}
        entityMotionSnapshots={entityMotionSnapshots}
        motionNow={motionNow}
        entityKindClassName={entityKindClassName}
      />
      <SceneChatBubbles
        bubbles={chatBubbles}
        entryByObjectId={entryByObjectId}
        player={player}
        playerCameraMotionOffset={playerCameraMotionOffset}
        entityMotionSnapshots={entityMotionSnapshots}
        motionNow={motionNow}
      />
      <SelectedTargetReadout t={t} selectedEntity={selectedEntity} targetActionLabel={targetActionLabel} />
    </div>
  );
}

// Re-export so consumers needing the bubble shape can import it from one place.
export type { ViewportEntityEntry as SceneOverlayEntityEntry };
