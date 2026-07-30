"use client";

import { memo, useEffect, useRef, type RefObject } from "react";

import type { ClientScreen } from "../../lib/original-ui";
import { TUTORIAL_CONTROL_EVENT } from "../../lib/tutorial-steps";
import {
  MIR2_GAMEPAD_BUTTON,
  mir2GamepadButtonPressed,
  mir2GamepadButtons,
  mir2GamepadSpatialDirection,
  mir2GamepadVector,
} from "./original-client-gamepad-input";
import {
  activateMir2GamepadFocus,
  closeMir2GamepadSurface,
  moveMir2GamepadFocus,
  type Mir2SpatialDirection,
} from "./original-client-gamepad-navigation";
import {
  mir2MobileMoveIntentFromVector,
  type Mir2MobileMoveIntent,
  type Mir2MobileMoveMode,
} from "./original-client-mobile-input";
import { CRYSTAL_MOVE_INPUT_INTERVAL_MS } from "./original-client-scene-layout";
import type { DisplayEntity, DisplayWorld, ItemActionRef } from "./original-client-types";

type OriginalClientGamepadControlsProps = {
  enabled: boolean;
  screen: ClientScreen;
  gameplayReady: boolean;
  stageRootRef: RefObject<HTMLDivElement | null>;
  world: DisplayWorld;
  player: DisplayEntity | null;
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

type Mir2GamepadDebugState = {
  connected: boolean;
  id: string | null;
  uiMode: boolean;
  movement: Mir2MobileMoveIntent | null;
};

const UI_INITIAL_REPEAT_MS = 360;
const UI_REPEAT_MS = 115;

function OriginalClientGamepadControlsInner({
  enabled,
  screen,
  gameplayReady,
  stageRootRef,
  world,
  player,
  onDirectionIntent,
  onDirectionStop,
  onPrimaryTargetAction,
  onApproachTarget,
  onPickGroundDrop,
  onToggleInventory,
  onToggleCharacter,
  onCastSkill,
  onUseItem,
}: OriginalClientGamepadControlsProps) {
  const activeMovementRef = useRef<Mir2MobileMoveIntent | null>(null);

  useEffect(() => {
    if (!enabled) {
      if (activeMovementRef.current) {
        activeMovementRef.current = null;
        onDirectionStop();
      }
      publishDebug({ connected: false, id: null, uiMode: false, movement: null });
      return;
    }

    let animationFrame = 0;
    let previousButtons: boolean[] = [];
    let lastMoveSentAt = 0;
    let lastUiDirection: Mir2SpatialDirection | null = null;
    let nextUiRepeatAt = 0;

    const stopMovement = () => {
      if (!activeMovementRef.current) return;
      activeMovementRef.current = null;
      onDirectionStop();
    };

    const tick = (now: number) => {
      const gamepad = firstConnectedGamepad();
      if (!gamepad) {
        stopMovement();
        previousButtons = [];
        lastUiDirection = null;
        publishDebug({ connected: false, id: null, uiMode: false, movement: null });
        animationFrame = window.requestAnimationFrame(tick);
        return;
      }

      const vector = mir2GamepadVector(gamepad);
      const stageRoot = stageRootRef.current;
      const uiMode =
        screen !== "game" ||
        Boolean(stageRoot?.querySelector('.game-ui-scene[data-gamepad-ui-open="true"]'));

      if (uiMode) {
        stopMovement();
        const direction = mir2GamepadSpatialDirection(vector);
        if (direction) {
          if (direction !== lastUiDirection || now >= nextUiRepeatAt) {
            if (stageRoot) moveMir2GamepadFocus(stageRoot, direction);
            nextUiRepeatAt = now + (direction === lastUiDirection ? UI_REPEAT_MS : UI_INITIAL_REPEAT_MS);
          }
        } else {
          nextUiRepeatAt = 0;
        }
        lastUiDirection = direction;

        if (
          stageRoot &&
          mir2GamepadButtonPressed(gamepad, previousButtons, MIR2_GAMEPAD_BUTTON.primary)
        ) {
          activateMir2GamepadFocus(stageRoot);
        }
        if (
          stageRoot &&
          mir2GamepadButtonPressed(gamepad, previousButtons, MIR2_GAMEPAD_BUTTON.cancel)
        ) {
          closeMir2GamepadSurface(stageRoot);
        }
      } else if (gameplayReady) {
        lastUiDirection = null;
        const movement = mir2MobileMoveIntentFromVector(
          vector,
          false,
          activeMovementRef.current?.direction ?? null,
        );
        const previousMovement = activeMovementRef.current;
        activeMovementRef.current = movement;
        if (movement && !previousMovement) {
          publishTutorialControl("gamepad:move");
        }
        if (!movement) {
          if (previousMovement) onDirectionStop();
        } else if (
          !previousMovement ||
          previousMovement.direction !== movement.direction ||
          previousMovement.mode !== movement.mode ||
          now - lastMoveSentAt >= CRYSTAL_MOVE_INPUT_INTERVAL_MS
        ) {
          lastMoveSentAt = now;
          onDirectionIntent(movement.direction, movement.mode);
        }

        if (mir2GamepadButtonPressed(gamepad, previousButtons, MIR2_GAMEPAD_BUTTON.primary)) {
          publishTutorialControl("gamepad:primary");
          onPrimaryTargetAction();
        }
        if (mir2GamepadButtonPressed(gamepad, previousButtons, MIR2_GAMEPAD_BUTTON.approach)) {
          publishTutorialControl("gamepad:approach");
          onApproachTarget();
        }
        if (mir2GamepadButtonPressed(gamepad, previousButtons, MIR2_GAMEPAD_BUTTON.pick)) {
          publishTutorialControl("gamepad:pick");
          const drop = nearestGroundDrop(world, player);
          if (drop) onPickGroundDrop(drop.objectId);
        }
        if (mir2GamepadButtonPressed(gamepad, previousButtons, MIR2_GAMEPAD_BUTTON.view)) {
          publishTutorialControl("gamepad:panel");
          onToggleCharacter();
        }
        if (mir2GamepadButtonPressed(gamepad, previousButtons, MIR2_GAMEPAD_BUTTON.menu)) {
          publishTutorialControl("gamepad:panel");
          onToggleInventory();
        }
        if (mir2GamepadButtonPressed(gamepad, previousButtons, MIR2_GAMEPAD_BUTTON.leftTrigger)) {
          publishTutorialControl("gamepad:quick");
          const skill = world.knownSkills[0];
          if (skill) onCastSkill(skill.key);
        }
        if (mir2GamepadButtonPressed(gamepad, previousButtons, MIR2_GAMEPAD_BUTTON.rightTrigger)) {
          publishTutorialControl("gamepad:quick");
          const skill = world.knownSkills[1];
          if (skill) onCastSkill(skill.key);
        }
        if (mir2GamepadButtonPressed(gamepad, previousButtons, MIR2_GAMEPAD_BUTTON.leftBumper)) {
          publishTutorialControl("gamepad:quick");
          const item = [...world.beltItems].sort((a, b) => a.slot - b.slot)[0];
          if (item) onUseItem(item);
        }
        if (mir2GamepadButtonPressed(gamepad, previousButtons, MIR2_GAMEPAD_BUTTON.rightBumper)) {
          publishTutorialControl("gamepad:quick");
          const item = [...world.beltItems].sort((a, b) => a.slot - b.slot)[1];
          if (item) onUseItem(item);
        }
      } else {
        stopMovement();
      }

      previousButtons = mir2GamepadButtons(gamepad);
      publishDebug({
        connected: true,
        id: gamepad.id,
        uiMode,
        movement: activeMovementRef.current,
      });
      animationFrame = window.requestAnimationFrame(tick);
    };

    animationFrame = window.requestAnimationFrame(tick);
    return () => {
      window.cancelAnimationFrame(animationFrame);
      stopMovement();
      publishDebug({ connected: false, id: null, uiMode: false, movement: null });
    };
  }, [
    enabled,
    gameplayReady,
    onApproachTarget,
    onCastSkill,
    onDirectionIntent,
    onDirectionStop,
    onPickGroundDrop,
    onPrimaryTargetAction,
    onToggleCharacter,
    onToggleInventory,
    onUseItem,
    player,
    screen,
    stageRootRef,
    world,
  ]);

  return null;
}

export const OriginalClientGamepadControls = memo(OriginalClientGamepadControlsInner);

function firstConnectedGamepad() {
  return Array.from(navigator.getGamepads?.() ?? []).find(
    (gamepad): gamepad is Gamepad => Boolean(gamepad?.connected),
  ) ?? null;
}

function nearestGroundDrop(world: DisplayWorld, player: DisplayEntity | null) {
  if (!player || !world.groundDrops.length) return null;
  return [...world.groundDrops].sort(
    (a, b) =>
      tileDistance(player.x, player.y, a.x, a.y) -
      tileDistance(player.x, player.y, b.x, b.y),
  )[0] ?? null;
}

function tileDistance(ax: number, ay: number, bx: number, by: number) {
  return Math.max(Math.abs(ax - bx), Math.abs(ay - by));
}

function publishDebug(state: Mir2GamepadDebugState) {
  const debugWindow = window as typeof window & {
    __mir2GamepadControls?: Mir2GamepadDebugState;
  };
  debugWindow.__mir2GamepadControls = state;
}

function publishTutorialControl(action: string) {
  window.dispatchEvent(new CustomEvent(TUTORIAL_CONTROL_EVENT, { detail: { action } }));
}
