"use client";

import { memo, useCallback, useEffect, useMemo, useRef, useState } from "react";

import { CRYSTAL_MOVE_INPUT_INTERVAL_MS } from "./original-client-scene-layout";
import type {
  DisplayEntity,
  DisplayItem,
  DisplayKnownSkill,
  DisplayWorld,
  ItemActionRef,
  TranslateFn,
} from "./original-client-types";
import {
  mir2MobileMoveIntentFromVector,
  type Mir2MobileMoveIntent,
  type Mir2MobileMoveMode,
} from "./original-client-mobile-input";

const MIR2_MOBILE_TURN_RETRY_MS = 1_500;

type NippleCollection = {
  on: (event: string, callback: (...args: unknown[]) => void) => void;
  off?: (event?: string, callback?: (...args: unknown[]) => void) => void;
  destroy: () => void;
  reposition?: () => void;
};

type NippleMoveData = {
  vector?: { x?: number; y?: number };
  force?: number;
};

type MobileControlsDebug = {
  active: boolean;
  movementBusy: boolean;
  runLocked: boolean;
  lastIntent: Mir2MobileMoveIntent | null;
  lastSentAt: number | null;
  dispatchDirection: (direction: Mir2MobileMoveIntent["direction"], mode?: Mir2MobileMoveMode) => boolean;
};

type MobileQuickAction =
  | { kind: "skill"; key: string; label: string; title: string; skillKey: string }
  | { kind: "item"; key: string; label: string; title: string; item: ItemActionRef };

type OriginalClientMobileControlsProps = {
  enabled: boolean;
  forceVisible: boolean;
  t: TranslateFn;
  world: DisplayWorld;
  player: DisplayEntity | null;
  selectedEntity: DisplayEntity | null;
  onDirectionIntent: (direction: string, mode: Mir2MobileMoveMode) => void;
  onDirectionStop: () => void;
  onPrimaryTargetAction: () => void;
  onApproachTarget: () => void;
  onPickGroundDrop: (objectId: string) => void;
  onToggleInventory: () => void;
  onToggleCharacter: () => void;
  onCastSkill: (skillKey: string) => void;
  onUseItem: (item: ItemActionRef) => void;
};

function OriginalClientMobileControlsInner({
  enabled,
  forceVisible,
  t,
  world,
  player,
  selectedEntity,
  onDirectionIntent,
  onDirectionStop,
  onPrimaryTargetAction,
  onApproachTarget,
  onPickGroundDrop,
  onToggleInventory,
  onToggleCharacter,
  onCastSkill,
  onUseItem,
}: OriginalClientMobileControlsProps) {
  const joystickZoneRef = useRef<HTMLDivElement | null>(null);
  const activeIntentRef = useRef<Mir2MobileMoveIntent | null>(null);
  const lastSentRef = useRef<{ direction: string; mode: Mir2MobileMoveMode; at: number } | null>(null);
  const [runLocked, setRunLocked] = useState(true);
  const [activeIntent, setActiveIntent] = useState<Mir2MobileMoveIntent | null>(null);
  const runLockedRef = useRef(runLocked);
  const enabledRef = useRef(enabled);
  const onDirectionIntentRef = useRef(onDirectionIntent);
  const onDirectionStopRef = useRef(onDirectionStop);

  const nearestDrop = useMemo(() => nearestGroundDrop(world, player), [world, player]);
  const quickBeltItems = useMemo(() => mobileBeltItems(world.beltItems), [world.beltItems]);
  const quickSkills = useMemo(() => mobileKnownSkills(world.knownSkills), [world.knownSkills]);
  const wheelActions = useMemo(
    () => mobileWheelActions(quickSkills, quickBeltItems),
    [quickBeltItems, quickSkills],
  );

  useEffect(() => {
    runLockedRef.current = runLocked;
  }, [runLocked]);

  useEffect(() => {
    enabledRef.current = enabled;
  }, [enabled]);

  useEffect(() => {
    onDirectionIntentRef.current = onDirectionIntent;
  }, [onDirectionIntent]);

  useEffect(() => {
    onDirectionStopRef.current = onDirectionStop;
  }, [onDirectionStop]);

  const publishDebugState = useCallback((nextIntent: Mir2MobileMoveIntent | null = activeIntentRef.current) => {
    const debugWindow = window as typeof window & { __mir2MobileControls?: MobileControlsDebug };
    debugWindow.__mir2MobileControls = {
      active: Boolean(nextIntent),
      movementBusy: mobileMovementTransportBusy(Date.now(), nextIntent),
      runLocked: runLockedRef.current,
      lastIntent: nextIntent,
      lastSentAt: lastSentRef.current?.at ?? null,
      dispatchDirection: (direction, mode = runLockedRef.current ? "run" : "walk") => {
        if (!enabledRef.current) return false;
        const intent = { direction, mode, force: 1 };
        activeIntentRef.current = intent;
        setActiveIntent(intent);
        const now = Date.now();
        if (mobileMovementTransportBusy(now, intent)) {
          publishDebugState(intent);
          return false;
        }
        lastSentRef.current = { direction, mode, at: now };
        onDirectionIntentRef.current(direction, mode);
        publishDebugState(intent);
        return true;
      },
    };
  }, []);

  const dispatchLatestIntent = useCallback((immediate = false) => {
    if (!enabledRef.current) return false;
    const intent = activeIntentRef.current;
    if (!intent) return false;

    const now = Date.now();
    if (mobileMovementTransportBusy(now, intent)) {
      publishDebugState(intent);
      return false;
    }

    const previous = lastSentRef.current;
    if (
      !immediate &&
      previous &&
      previous.direction === intent.direction &&
      previous.mode === intent.mode &&
      now - previous.at < CRYSTAL_MOVE_INPUT_INTERVAL_MS
    ) {
      return false;
    }

    lastSentRef.current = { direction: intent.direction, mode: intent.mode, at: now };
    onDirectionIntentRef.current(intent.direction, intent.mode);
    publishDebugState(intent);
    return true;
  }, [publishDebugState]);

  useEffect(() => {
    publishDebugState(null);
    return () => {
      const debugWindow = window as typeof window & { __mir2MobileControls?: MobileControlsDebug };
      delete debugWindow.__mir2MobileControls;
    };
  }, [publishDebugState]);

  useEffect(() => {
    if (!enabled) {
      activeIntentRef.current = null;
      setActiveIntent(null);
      onDirectionStopRef.current();
      return;
    }

    const timer = window.setInterval(() => dispatchLatestIntent(false), CRYSTAL_MOVE_INPUT_INTERVAL_MS);
    return () => {
      window.clearInterval(timer);
      activeIntentRef.current = null;
      setActiveIntent(null);
      onDirectionStopRef.current();
    };
  }, [dispatchLatestIntent, enabled]);

  useEffect(() => {
    if (!enabled || !joystickZoneRef.current) {
      activeIntentRef.current = null;
      setActiveIntent(null);
      onDirectionStopRef.current();
      return;
    }

    let disposed = false;
    let manager: NippleCollection | null = null;
    let repositionTimer = 0;
    let moveHandler: ((...args: unknown[]) => void) | null = null;
    let endHandler: (() => void) | null = null;

    import("nipplejs")
      .then((module) => {
        if (disposed || !joystickZoneRef.current) return;
        module.default.setLogLevel("none");
        manager = module.default.create({
          zone: joystickZoneRef.current,
          mode: "static",
          position: { left: "50%", top: "50%" },
          color: {
            back: "rgba(17, 12, 8, 0.72)",
            front: "rgba(226, 195, 112, 0.88)",
          },
          size: 104,
          threshold: 0.08,
          restJoystick: true,
          restOpacity: 0.72,
        }) as NippleCollection;

        const handleMove = (...args: unknown[]) => {
          const data = readNippleMoveData(args);
          const vector = data?.vector;
          if (!vector) return;
          const nextIntent = mir2MobileMoveIntentFromVector(
            {
              x: Number(vector.x ?? 0),
              y: Number(vector.y ?? 0),
              force: Number(data?.force ?? 0),
            },
            runLockedRef.current,
            activeIntentRef.current?.direction ?? null,
          );
          activeIntentRef.current = nextIntent;
          setActiveIntent(nextIntent);
          publishDebugState(nextIntent);

          const previous = lastSentRef.current;
          if (
            nextIntent &&
            (!previous || previous.direction !== nextIntent.direction || previous.mode !== nextIntent.mode)
          ) {
            dispatchLatestIntent(true);
          }
        };

        const handleEnd = () => {
          activeIntentRef.current = null;
          setActiveIntent(null);
          onDirectionStopRef.current();
          publishDebugState(null);
        };

        moveHandler = handleMove;
        endHandler = handleEnd;
        manager.on("move", handleMove);
        manager.on("end", handleEnd);
        repositionTimer = window.setTimeout(() => manager?.reposition?.(), 0);
      })
      .catch((error: unknown) => {
        // Joystick init failing must not crash mobile gameplay; the page stays
        // usable (tap-to-move still works) and the chunk-reload guard handles
        // genuine ChunkLoadErrors.
        console.error("Failed to initialize mobile joystick", error);
      });

    return () => {
      disposed = true;
      if (repositionTimer) window.clearTimeout(repositionTimer);
      if (manager) {
        if (moveHandler) manager.off?.("move", moveHandler);
        if (endHandler) manager.off?.("end", endHandler);
        manager.destroy();
      }
      manager = null;
      activeIntentRef.current = null;
      setActiveIntent(null);
      onDirectionStopRef.current();
    };
  }, [dispatchLatestIntent, enabled, publishDebugState]);

  if (!enabled) {
    if (!forceVisible) {
      return null;
    }

    return (
      <div className="mir-mobile-rotate-overlay force-mobile-controls">
        <div className="mir-mobile-rotate-card">
          <span className="mir-mobile-rotate-icon" aria-hidden="true">[]</span>
          <span>{t("ui.mobileRotateLandscape", [], "Rotate to landscape")}</span>
        </div>
      </div>
    );
  }

  return (
    <>
      <div className={`mir-mobile-controls ${forceVisible ? "force-mobile-controls" : ""}`} data-ui-interactive="true">
        <div className="mir-mobile-stick-shell" aria-label={t("ui.mobileMove", [], "Move")}>
          <div ref={joystickZoneRef} className="mir-mobile-stick-zone" data-mobile-joystick="true" />
          <div className="mir-mobile-stick-state" data-active={activeIntent ? "true" : "false"}>
            {activeIntent
              ? activeIntent.direction.replace(/([A-Z])/g, " $1").trim()
              : t("ui.mobileMove", [], "Move")}
          </div>
        </div>
        <div className="mir-mobile-action-pad" aria-label={t("ui.mobileActions", [], "Actions")}>
          <div className="mir-mobile-panel-row">
            <button
              type="button"
              className="mir-mobile-panel-button"
              aria-label={t("ui.character", [], "Character")}
              onClick={onToggleCharacter}
            >
              {mobileCompactLabel(t("ui.character", [], "Character"))}
            </button>
            <button
              type="button"
              className="mir-mobile-panel-button"
              aria-label={t("ui.inventory", [], "Inventory")}
              onClick={onToggleInventory}
            >
              {mobileCompactLabel(t("ui.inventory", [], "Inventory"))}
            </button>
          </div>
          {Array.from({ length: 5 }).map((_, index) => {
            const action = wheelActions[index];
            if (!action) {
              return (
                <button
                  key={`empty-${index}`}
                  type="button"
                  className={`mir-mobile-action wheel quick quick-${index}`}
                  disabled
                  aria-label={t("ui.emptyQuickSlot", [], "Empty quick slot")}
                >
                  +
                </button>
              );
            }
            return (
              <button
                key={action.key}
                type="button"
                className={`mir-mobile-action wheel quick quick-${index} ${action.kind}`}
                onClick={() => {
                  if (action.kind === "skill") {
                    onCastSkill(action.skillKey);
                  } else {
                    onUseItem(action.item);
                  }
                }}
                title={action.title}
              >
                {action.label}
              </button>
            );
          })}
          <button
            type="button"
            className={`mir-mobile-action wheel run ${runLocked ? "active" : ""}`}
            aria-label={t("ui.mobileRun", [], "Run")}
            aria-pressed={runLocked}
            onClick={() => {
              setRunLocked((current) => !current);
              window.setTimeout(() => publishDebugState(), 0);
            }}
          >
            {t("ui.mobileRun", [], "Run")}
          </button>
          <button
            type="button"
            className="mir-mobile-action wheel approach"
            aria-label={t("ui.approach", [], "Approach")}
            disabled={!selectedEntity}
            onClick={onApproachTarget}
          >
            {mobileCompactLabel(t("ui.approach", [], "Approach"))}
          </button>
          <button
            type="button"
            className="mir-mobile-action wheel pick"
            aria-label={t("ui.mobilePickUp", [], "Pick up")}
            disabled={!nearestDrop}
            onClick={() => nearestDrop && onPickGroundDrop(nearestDrop.objectId)}
          >
            {t("ui.mobilePickUp", [], "Pick")}
          </button>
          <button
            type="button"
            className="mir-mobile-action wheel primary"
            aria-label={t("ui.attack", [], "Attack")}
            disabled={!selectedEntity}
            onClick={onPrimaryTargetAction}
          >
            {mobileCompactLabel(t("ui.attack", [], "Attack"))}
          </button>
        </div>
      </div>
      <div className={`mir-mobile-rotate-overlay ${forceVisible ? "force-mobile-controls" : ""}`}>
        <div className="mir-mobile-rotate-card">
          <span className="mir-mobile-rotate-icon" aria-hidden="true">[]</span>
          <span>{t("ui.mobileRotateLandscape", [], "Rotate to landscape")}</span>
        </div>
      </div>
    </>
  );
}

// Memoized: OriginalClientMobileControls does not receive motionNow and its props
// (world, player, selectedEntity, callbacks) only change on server/user events, not
// on every 30 Hz motion tick. Memo prevents spurious re-renders during gameplay.
export const OriginalClientMobileControls = memo(OriginalClientMobileControlsInner);

function mobileCompactLabel(label: string): string {
  const glyphs = Array.from(label.trim());
  return glyphs.length <= 4 ? glyphs.join("") : glyphs.slice(0, 4).join("");
}

function readNippleMoveData(args: unknown[]): NippleMoveData | null {
  for (const arg of args) {
    if (!arg || typeof arg !== "object") continue;
    const maybeData = arg as NippleMoveData & { data?: NippleMoveData };
    if (maybeData.vector) return maybeData;
    if (maybeData.data?.vector) return maybeData.data;
  }
  return null;
}

function mobileMovementTransportBusy(now = Date.now(), intent?: Mir2MobileMoveIntent | null) {
  const stageWindow = window as typeof window & {
    __mir2Stage5?: {
      state?: {
        screen?: string;
        movementPlan?: unknown;
        directionStepPending?: unknown;
        directionStepPendingQueue?: unknown[];
        outstandingSelfMovementActions?: Array<{ visualUntil?: number }>;
        movementInputBlockedUntil?: number;
      };
    };
    __mir2CommandHistory?: Array<{ type?: string; direction?: string; at?: number }>;
  };
  const state = stageWindow.__mir2Stage5?.state;
  if (!state || state.screen !== "game") return false;
  if (typeof state.movementInputBlockedUntil === "number" && now < state.movementInputBlockedUntil) return true;
  if (state.movementPlan) return true;
  if (state.directionStepPending) return true;
  if (Array.isArray(state.directionStepPendingQueue) && state.directionStepPendingQueue.length > 0) return true;
  if (
    Array.isArray(state.outstandingSelfMovementActions) &&
      state.outstandingSelfMovementActions.some((action) => typeof action.visualUntil !== "number" || action.visualUntil > now)
  ) {
    return true;
  }
  if (intent) {
    const recentCommand = stageWindow.__mir2CommandHistory?.find((entry) => entry?.type === "turn");
    if (
      recentCommand?.direction === intent.direction &&
      typeof recentCommand.at === "number" &&
      now - recentCommand.at < MIR2_MOBILE_TURN_RETRY_MS
    ) {
      return true;
    }
  }
  return false;
}

function nearestGroundDrop(world: DisplayWorld, player: DisplayEntity | null) {
  if (!player || !world.groundDrops.length) return null;
  return [...world.groundDrops].sort(
    (a, b) => tileDistance(player.x, player.y, a.x, a.y) - tileDistance(player.x, player.y, b.x, b.y),
  )[0] ?? null;
}

function tileDistance(ax: number, ay: number, bx: number, by: number) {
  return Math.max(Math.abs(ax - bx), Math.abs(ay - by));
}

function mobileBeltItems(items: DisplayItem[]) {
  return [...items].sort((a, b) => a.slot - b.slot).slice(0, 4);
}

function mobileKnownSkills(skills: DisplayKnownSkill[]) {
  return [...skills].slice(0, 4);
}

function mobileWheelActions(skills: DisplayKnownSkill[], items: DisplayItem[]): MobileQuickAction[] {
  const skillActions: MobileQuickAction[] = skills.slice(0, 3).map((skill, index) => ({
    kind: "skill",
    key: `skill-${skill.key}`,
    label: `S${index + 1}`,
    title: skill.name,
    skillKey: skill.key,
  }));
  const itemActions: MobileQuickAction[] = items.slice(0, 2).map((item, index) => ({
    kind: "item",
    key: `item-${item.slot}-${item.uniqueId}`,
    label: String(index + 1),
    title: item.name,
    item,
  }));
  return [...skillActions, ...itemActions].slice(0, 5);
}
