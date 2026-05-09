"use client";

import { useCallback, useEffect, useMemo, useRef, useState, type MouseEvent } from "react";

import {
  ORIGINAL_UI,
  type ClientScreen,
  type CharacterTabKey,
  type InventoryTabKey,
} from "../lib/original-ui";
import {
  loadOriginalSceneSpriteLibrary,
  normalizeSceneSpriteLibraryKey,
  originalSceneSpriteLibraryExists,
  type OriginalSceneSpriteLibraryMeta,
} from "../lib/original-scene-sprite-meta";
import {
  buildTranslator,
  formatRuntimeMessage,
  formatRuntimePhase,
  languageLocale,
  type Mir2Language,
} from "../lib/localization";
import {
  LanguageSelector,
  LoginOverlay,
  SelectOverlay,
} from "./components/original-client-overlays";
import { GameUiScene } from "./components/original-client-game-ui-scene";
import type { OriginalClientShellProps } from "./components/original-client-shell-types";
import {
  LOGIN_STATIC_BACKGROUND_FRAME,
  LOGIN_TRANSITION_FRAME_MS,
  ORIGINAL_AUDIO,
  ORIGINAL_EFFECT_VOLUME,
  ORIGINAL_MUSIC_VOLUME,
  desiredMusicForScreen,
  entityKindLabelKey,
  selectedTargetActionLabel,
} from "./components/original-client-shell-flow";
import type {
  DisplayEntity,
  DisplayLogLine,
  DisplayWorld,
  EntityKind,
  EntityMotionSnapshot,
  EquipmentActionRef,
  EquipmentSlot,
  ItemActionRef,
  MergeItemRef,
  MoveItemRef,
  PredictedPlayerMotion,
  SelectCharacterEntry,
  TranslateFn,
} from "./components/original-client-types";
import {
  CRYSTAL_MOVE_INPUT_INTERVAL_MS,
  EMPTY_VIEWPORT_MAP_SPRITES,
  EMPTY_VIEWPORT_OFFSET,
  GameSceneBackdrop,
  MAX_PREDICTED_PLAYER_LEAD_TILES,
  VIEWPORT_CELL_HEIGHT,
  VIEWPORT_CELL_WIDTH,
  VIEWPORT_MOUSE_TILE_CENTER_X,
  VIEWPORT_MOUSE_TILE_CENTER_Y,
  VIEWPORT_OFFSET_X,
  VIEWPORT_OFFSET_Y,
  VIEWPORT_RANGE_X,
  VIEWPORT_RANGE_Y,
  buildViewportEntitySprite,
  buildViewportMapSprites,
  cameraMotionOffsetForEntity,
  entityAnimationStateForEntity,
  portraitFramesForCharacter,
  projectileProgress,
  refreshEntityMotionSnapshots,
  type ViewportOffset,
} from "./components/original-client-scene-rendering";
import { OriginalClientSceneVisualLayers } from "./components/original-client-scene-visual-layers";

type HeldScenePointer = {
  button: 0 | 2;
  sceneX: number;
  sceneY: number;
  startedAt: number;
  dispatched: boolean;
  tileX?: number;
  tileY?: number;
};

export function OriginalClientShell({
  language,
  screen,
  runtimePhase,
  runtimeMessage,
  wsState,
  world,
  player,
  predictedPlayerPosition,
  selectedEntity,
  sortedEntities,
  viewportEntities,
  viewportTiles,
  logs,
  accountId,
  password,
  chatMessage,
  loginBusy,
  loginError,
  characters,
  selectedCharacterIndex,
  showInventory,
  showCharacter,
  activeInventoryTab,
  activeCharacterTab,
  storageServiceOpenVersion,
  onLanguageChange,
  onAccountIdChange,
  onPasswordChange,
  onChatMessageChange,
  onCreateAccount,
  onSubmitLogin,
  onQuickEnter,
  onResetClient,
  onExitSelect,
  onSendChat,
  onRentExpandedStorage,
  onLogout,
  onCreateCharacter,
  onDeleteCharacter,
  onUseItem,
  onDropItem,
  onEquipItem,
  onRemoveItem,
  onMoveItem,
  onMergeItem,
  onSplitItem,
  onStoreItem,
  onTakeBackItem,
  onUnlockStorage,
  onSetStoragePassword,
  onRemoveStoragePassword,
  onSellItem,
  onDropGold,
  onRepairItem,
  onSpecialRepairItem,
  onCastSkill,
  onTransferMap,
  onClaimMail,
  onDeleteMail,
  onBuyGameShopItem,
  onRunStage5Command,
  onSendClientCommand,
  transferOptions,
  onToggleCharacter,
  onToggleInventory,
  onCloseCharacter,
  onCloseInventory,
  onOpenCharacterTab,
  onOpenInventoryTab,
  onViewportTileClick,
  onViewportTileSecondaryAction,
  onViewportTileStepClick,
  onViewportTileStepSecondaryAction,
  onViewportDirectionStep,
  onPickGroundDrop,
  onSelectEntity,
  onActivateEntity,
  onApproachTarget,
  onPrimaryTargetAction,
  onSelectNpcDialogTarget,
  onSubmitNpcInput,
  onSelectCharacter,
  onEnterWorld,
  targetDistance,
  entityKindClassName,
}: OriginalClientShellProps) {
  const t = buildTranslator(language);
  const locale = languageLocale(language);
  const runtimePhaseLabel = formatRuntimePhase(language, runtimePhase);
  const runtimeMessageLabel = formatRuntimeMessage(language, runtimeMessage);
  const [loginTransitionFrame, setLoginTransitionFrame] = useState<number | null>(null);
  const [selectPortraitFrameIndex, setSelectPortraitFrameIndex] = useState(0);
  const [sceneSpriteFrameIndex, setSceneSpriteFrameIndex] = useState(0);
  const [motionNow, setMotionNow] = useState(() => Date.now());
  const [sceneSpriteLibraries, setSceneSpriteLibraries] = useState<Record<string, OriginalSceneSpriteLibraryMeta>>({});
  const previousScreenRef = useRef<ClientScreen>(screen);
  const musicAudioRef = useRef<HTMLAudioElement | null>(null);
  const loginEffectAudioRef = useRef<HTMLAudioElement | null>(null);
  const activeMusicSrcRef = useRef<string | null>(null);
  const pendingMusicSrcRef = useRef<string | null>(null);
  const missingSceneSpriteLibrariesRef = useRef<Set<string>>(new Set());
  const entityMotionSnapshotsRef = useRef<Record<string, EntityMotionSnapshot>>({});
  const stageFrameRef = useRef<HTMLDivElement | null>(null);
  const heldScenePointerRef = useRef<HeldScenePointer | null>(null);
  const latestMoveInputRef = useRef<{
    screen: ClientScreen;
    player: DisplayEntity | null;
    renderPlayer: DisplayEntity | null;
    playerCameraMotionOffset: ViewportOffset;
  }>({
    screen,
    player,
    renderPlayer: player,
    playerCameraMotionOffset: EMPTY_VIEWPORT_OFFSET,
  });

  const selectedCharacter = characters[selectedCharacterIndex] ?? null;
  const selectedPortraitFrames = selectedCharacter ? portraitFramesForCharacter(selectedCharacter) : [];
  const activeSelectPortraitFrame =
    selectedPortraitFrames[selectPortraitFrameIndex % Math.max(selectedPortraitFrames.length, 1)] ?? null;
  const loginBackgroundFrame =
    ORIGINAL_UI.login.backgroundFrames[LOGIN_STATIC_BACKGROUND_FRAME] ?? ORIGINAL_UI.login.backgroundFrames[0];
  const loginTransitionBackground =
    screen !== "select" || loginTransitionFrame === null
      ? null
      : ORIGINAL_UI.login.backgroundFrames[
          Math.min(loginTransitionFrame, ORIGINAL_UI.login.backgroundFrames.length - 1)
        ] ?? loginBackgroundFrame;
  const loginTransitionAudioActive = screen === "select" && loginTransitionFrame !== null;

  const syncMusic = useCallback((src: string | null) => {
    pendingMusicSrcRef.current = src;

    if (!src) {
      musicAudioRef.current?.pause();
      activeMusicSrcRef.current = null;
      return;
    }

    const audio = musicAudioRef.current ?? new Audio();
    musicAudioRef.current = audio;

    if (activeMusicSrcRef.current !== src) {
      audio.pause();
      audio.src = src;
      audio.currentTime = 0;
      activeMusicSrcRef.current = src;
    }

    audio.loop = true;
    audio.volume = ORIGINAL_MUSIC_VOLUME;
    void audio.play().catch(() => undefined);
  }, []);

  const playLoginEffect = useCallback(() => {
    const audio = loginEffectAudioRef.current ?? new Audio();
    loginEffectAudioRef.current = audio;
    audio.src = ORIGINAL_AUDIO.loginEffect;
    audio.currentTime = 0;
    audio.volume = ORIGINAL_EFFECT_VOLUME;
    void audio.play().catch(() => undefined);
  }, []);

  useEffect(() => {
    const handleUserAudioGesture = () => syncMusic(pendingMusicSrcRef.current);

    window.addEventListener("pointerdown", handleUserAudioGesture, true);
    window.addEventListener("keydown", handleUserAudioGesture, true);

    return () => {
      window.removeEventListener("pointerdown", handleUserAudioGesture, true);
      window.removeEventListener("keydown", handleUserAudioGesture, true);
      musicAudioRef.current?.pause();
      loginEffectAudioRef.current?.pause();
    };
  }, [syncMusic]);

  useEffect(() => {
    const previousScreen = previousScreenRef.current;
    previousScreenRef.current = screen;

    if (previousScreen === "login" && screen === "select") {
      setLoginTransitionFrame(0);
      return;
    }

    if (screen !== "select") {
      setLoginTransitionFrame(null);
    }
  }, [screen]);

  useEffect(() => {
    if (loginTransitionFrame === null) {
      return;
    }

    const timer = window.setTimeout(() => {
      setLoginTransitionFrame((current) => {
        if (current === null) {
          return null;
        }

        const nextFrame = current + 1;
        return nextFrame >= ORIGINAL_UI.login.backgroundFrames.length ? null : nextFrame;
      });
    }, LOGIN_TRANSITION_FRAME_MS);

    return () => window.clearTimeout(timer);
  }, [loginTransitionFrame]);

  useEffect(() => {
    syncMusic(desiredMusicForScreen(screen, loginTransitionAudioActive));
  }, [loginTransitionAudioActive, screen, syncMusic]);

  useEffect(() => {
    if (loginTransitionFrame === 0) {
      playLoginEffect();
    }
  }, [loginTransitionFrame, playLoginEffect]);

  useEffect(() => {
    setSelectPortraitFrameIndex(0);
  }, [screen, selectedCharacterIndex, selectedCharacter?.classKey, selectedCharacter?.gender]);

  useEffect(() => {
    if (screen !== "select" || selectedPortraitFrames.length <= 1) {
      return;
    }

    const timer = window.setInterval(() => {
      setSelectPortraitFrameIndex((current) => (current + 1) % selectedPortraitFrames.length);
    }, 120);

    return () => window.clearInterval(timer);
  }, [screen, selectedPortraitFrames.length]);

  useEffect(() => {
    if (screen !== "game") {
      return;
    }

    const timer = window.setInterval(() => {
      setSceneSpriteFrameIndex((current) => current + 1);
    }, 120);

    return () => window.clearInterval(timer);
  }, [screen]);

  useEffect(() => {
    if (screen !== "game") {
      return;
    }

    function suppressBrowserContextMenu(event: globalThis.MouseEvent) {
      const target = event.target;
      if (target instanceof Element && target.closest(".client-stage-frame")) {
        event.preventDefault();
      }
    }

    function suppressRightMouseDefault(event: globalThis.MouseEvent) {
      if (event.button !== 2) {
        return;
      }

      const target = event.target;
      if (target instanceof Element && target.closest(".client-stage-frame")) {
        event.preventDefault();
      }
    }

    window.addEventListener("contextmenu", suppressBrowserContextMenu, true);
    window.addEventListener("mousedown", suppressRightMouseDefault, true);

    return () => {
      window.removeEventListener("contextmenu", suppressBrowserContextMenu, true);
      window.removeEventListener("mousedown", suppressRightMouseDefault, true);
    };
  }, [screen]);

  useEffect(() => {
    if (screen !== "game") {
      return;
    }

    function handleShortcutKey(event: KeyboardEvent) {
      if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) {
        return;
      }

      const target = event.target;
      if (
        target instanceof HTMLInputElement ||
        target instanceof HTMLTextAreaElement ||
        target instanceof HTMLSelectElement ||
        (target instanceof HTMLElement && target.isContentEditable)
      ) {
        return;
      }

      if (selectedEntity && !selectedEntity.dead) {
        if (event.key === " " || event.key === "Enter") {
          event.preventDefault();
          onPrimaryTargetAction();
          return;
        }

        if (event.key.toLowerCase() === "a") {
          event.preventDefault();
          onApproachTarget();
          return;
        }
      }

      const slotIndex = Number.parseInt(event.key, 10);
      if (!Number.isFinite(slotIndex) || slotIndex < 1 || slotIndex > 6) {
        return;
      }

      const item = world.beltItems.find((entry) => entry.slot === slotIndex - 1);
      if (!item) {
        return;
      }

      event.preventDefault();
      onUseItem({
        key: item.key,
        uniqueId: item.uniqueId,
        slot: item.slot,
        container: item.container,
      });
    }

    window.addEventListener("keydown", handleShortcutKey);
    return () => window.removeEventListener("keydown", handleShortcutKey);
  }, [screen, selectedEntity, world.beltItems, onApproachTarget, onPrimaryTargetAction, onUseItem]);

  useEffect(() => {
    if (screen !== "game") {
      return;
    }

    let animationFrame = 0;
    const updateMotionClock = () => {
      setMotionNow(Date.now());
      animationFrame = window.requestAnimationFrame(updateMotionClock);
    };
    animationFrame = window.requestAnimationFrame(updateMotionClock);

    return () => window.cancelAnimationFrame(animationFrame);
  }, [screen]);

  const renderPlayer = useMemo(() => (
    player &&
    predictedPlayerPosition &&
    Math.max(Math.abs(player.x - predictedPlayerPosition.x), Math.abs(player.y - predictedPlayerPosition.y)) <=
      MAX_PREDICTED_PLAYER_LEAD_TILES
      ? {
          ...player,
          x: predictedPlayerPosition.x,
          y: predictedPlayerPosition.y,
          direction: predictedPlayerPosition.direction ?? player.direction,
        }
      : player
  ), [player, predictedPlayerPosition]);

  useEffect(() => {
    if (screen !== "game") {
      return;
    }

    const libraries = new Set<string>();
    for (const entity of world.entities) {
      if (entity.sprite?.bodyLibrary) {
        libraries.add(normalizeSceneSpriteLibraryKey(entity.sprite.bodyLibrary));
      }
      if (entity.sprite?.hairLibrary) {
        libraries.add(normalizeSceneSpriteLibraryKey(entity.sprite.hairLibrary));
      }
      if (entity.sprite?.weaponLibrary) {
        libraries.add(normalizeSceneSpriteLibraryKey(entity.sprite.weaponLibrary));
      }
      if (entity.sprite?.altBodyLibrary) {
        libraries.add(normalizeSceneSpriteLibraryKey(entity.sprite.altBodyLibrary));
      }
      if (entity.sprite?.altHairLibrary) {
        libraries.add(normalizeSceneSpriteLibraryKey(entity.sprite.altHairLibrary));
      }
      if (entity.sprite?.altWeaponLibrary) {
        libraries.add(normalizeSceneSpriteLibraryKey(entity.sprite.altWeaponLibrary));
      }
      if (entity.sprite?.altWeaponLibrarySecondary) {
        libraries.add(normalizeSceneSpriteLibraryKey(entity.sprite.altWeaponLibrarySecondary));
      }
      if (entity.kind === "selfPlayer" || entity.kind === "player") {
        libraries.add("CArmour/00");
        libraries.add("CHair/00");
        libraries.add("CWeapon/00");
        if (entity.classKey === "archer") {
          libraries.add("ARArmour/00");
          libraries.add("ARHair/00");
          libraries.add("ARWeapon/00");
          libraries.add("ARWeapon/00 S");
        }
        if (entity.classKey === "assassin") {
          libraries.add("AArmour/00");
          libraries.add("AHair/00");
          libraries.add("AWeapon/00 L");
          libraries.add("AWeapon/00 R");
        }
      }
    }

    const missingLibraries = [...libraries].filter((libraryKey) => !(libraryKey in sceneSpriteLibraries));
    for (const libraryKey of missingLibraries) {
      if (!originalSceneSpriteLibraryExists(libraryKey)) {
        missingSceneSpriteLibrariesRef.current.add(libraryKey);
      }
    }
    const pendingLibraries = missingLibraries.filter(
      (libraryKey) =>
        originalSceneSpriteLibraryExists(libraryKey) &&
        !missingSceneSpriteLibrariesRef.current.has(libraryKey),
    );
    if (!pendingLibraries.length) {
      return;
    }

    let disposed = false;
    void Promise.all(
      pendingLibraries.map(async (libraryKey) => {
        try {
          return [libraryKey, await loadOriginalSceneSpriteLibrary(libraryKey)] as const;
        } catch {
          missingSceneSpriteLibrariesRef.current.add(libraryKey);
          return null;
        }
      }),
    )
      .then((loadedLibraries) => {
        if (disposed) {
          return;
        }

        setSceneSpriteLibraries((current) => {
          const next = { ...current };
          for (const entry of loadedLibraries) {
            if (!entry) {
              continue;
            }
            const [libraryKey, libraryMeta] = entry;
            next[libraryKey] = libraryMeta;
          }
          return next;
        });
      })
      .catch(() => undefined);

    return () => {
      disposed = true;
    };
  }, [sceneSpriteLibraries, screen, world.entities]);

  const sceneNow = motionNow;
  entityMotionSnapshotsRef.current = refreshEntityMotionSnapshots(
    screen,
    world.entities,
    renderPlayer,
    entityMotionSnapshotsRef.current,
    sceneNow,
  );
  const playerCameraMotionOffset = renderPlayer
    ? cameraMotionOffsetForEntity(renderPlayer, entityMotionSnapshotsRef.current, motionNow)
    : EMPTY_VIEWPORT_OFFSET;
  latestMoveInputRef.current = {
    screen,
    player,
    renderPlayer,
    playerCameraMotionOffset,
  };
  const viewportEntitySprites = player
    ? viewportEntities.map((entity) => {
        const motionSnapshot = entityMotionSnapshotsRef.current[entity.objectId];
        const animationState = entityAnimationStateForEntity(entity, entityMotionSnapshotsRef.current, sceneNow);
        return {
          entity,
          sprite: buildViewportEntitySprite(
            entity,
            sceneSpriteLibraries,
            sceneSpriteFrameIndex,
            sceneNow,
            animationState,
            motionSnapshot,
          ),
        };
      })
    : [];
  const viewportGroundDrops = player
    ? world.groundDrops
        .filter(
          (drop) =>
            Math.abs(drop.x - (renderPlayer ?? player).x) <= VIEWPORT_RANGE_X &&
            Math.abs(drop.y - (renderPlayer ?? player).y) <= VIEWPORT_RANGE_Y,
        )
        .map((drop) => ({
          ...drop,
          dx: drop.x - (renderPlayer ?? player).x,
          dy: drop.y - (renderPlayer ?? player).y,
        }))
    : [];
  const viewportMapSprites = renderPlayer
    ? buildViewportMapSprites(world, renderPlayer, sceneSpriteFrameIndex)
    : EMPTY_VIEWPORT_MAP_SPRITES;
  const viewportProjectiles = renderPlayer
    ? world.projectiles
        .filter((projectile) => projectile.expiresAt > motionNow)
        .map((projectile) => ({
          ...projectile,
          fromDx: projectile.fromX - renderPlayer.x,
          fromDy: projectile.fromY - renderPlayer.y,
          toDx: projectile.toX - renderPlayer.x,
          toDy: projectile.toY - renderPlayer.y,
          progress: projectileProgress(projectile, motionNow),
        }))
    : [];
  const viewportDepthPlayer = renderPlayer ?? player ?? { x: 0, y: 0 };
  const showSyntheticScene =
    screen === "game" && !viewportMapSprites.floor.length && Boolean(world.originalMapRegion);

  function scenePointFromMouseEvent(event: MouseEvent<HTMLElement>) {
    const rect = stageFrameRef.current?.getBoundingClientRect() ?? event.currentTarget.getBoundingClientRect();
    const scaleX = ORIGINAL_UI.game.sceneWidth / Math.max(rect.width, 1);
    const scaleY = ORIGINAL_UI.game.sceneHeight / Math.max(rect.height, 1);
    return {
      sceneX: (event.clientX - rect.left) * scaleX,
      sceneY: (event.clientY - rect.top) * scaleY,
    };
  }

  function tileFromScenePoint(sceneX: number, sceneY: number) {
    const latest = latestMoveInputRef.current;
    const basePlayer = latest.renderPlayer ?? latest.player;
    if (!basePlayer) return null;
    return {
      x: basePlayer.x + Math.floor(sceneX / VIEWPORT_CELL_WIDTH) - VIEWPORT_OFFSET_X,
      y: basePlayer.y + Math.floor(sceneY / VIEWPORT_CELL_HEIGHT) - VIEWPORT_OFFSET_Y,
    };
  }

  function dispatchSceneMoveInput(pointer: HeldScenePointer) {
    if (latestMoveInputRef.current.screen !== "game") return;
    const tile = tileFromScenePoint(pointer.sceneX, pointer.sceneY);
    if (!tile) return;

    if (pointer.button === 2) {
      onViewportDirectionStep(tile.x, tile.y, "run");
    } else {
      onViewportDirectionStep(tile.x, tile.y, "walk");
    }
  }

  function dispatchSceneClickInput(pointer: HeldScenePointer) {
    if (latestMoveInputRef.current.screen !== "game") return;
    const tile =
      pointer.tileX !== undefined && pointer.tileY !== undefined
        ? { x: pointer.tileX, y: pointer.tileY }
        : tileFromScenePoint(pointer.sceneX, pointer.sceneY);
    if (!tile) return;

    if (pointer.button === 2) {
      onViewportTileSecondaryAction(tile.x, tile.y);
    } else {
      onViewportTileClick(tile.x, tile.y);
    }
  }

  function handleScenePointerAction(event: MouseEvent<HTMLDivElement>) {
    if (screen !== "game" || !player) {
      return;
    }

    const target = event.target;
    if (
      target instanceof HTMLElement &&
      target.closest("[data-ui-interactive='true'], .game-ui-scene, .login-overlay, .select-overlay")
    ) {
      return;
    }

    if (event.button !== 0 && event.button !== 2) {
      return;
    }

    event.preventDefault();
    const point = scenePointFromMouseEvent(event);
    const pointer: HeldScenePointer = {
      button: event.button,
      sceneX: point.sceneX,
      sceneY: point.sceneY,
      startedAt: Date.now(),
      dispatched: false,
    };
    heldScenePointerRef.current = pointer;
  }

  function handleScenePointerMove(event: MouseEvent<HTMLDivElement>) {
    const held = heldScenePointerRef.current;
    if (!held || screen !== "game") {
      return;
    }

    const point = scenePointFromMouseEvent(event);
    heldScenePointerRef.current = {
      ...held,
      sceneX: point.sceneX,
      sceneY: point.sceneY,
    };
  }

  function stopHeldScenePointer() {
    const held = heldScenePointerRef.current;
    heldScenePointerRef.current = null;
    if (!held || held.dispatched) return;
    dispatchSceneClickInput(held);
  }

  useEffect(() => {
    if (screen !== "game") {
      heldScenePointerRef.current = null;
      return;
    }

    const timer = window.setInterval(() => {
      const held = heldScenePointerRef.current;
      if (!held) return;
      if (!held.dispatched && Date.now() - held.startedAt < CRYSTAL_MOVE_INPUT_INTERVAL_MS) {
        return;
      }
      held.dispatched = true;
      dispatchSceneMoveInput(held);
    }, CRYSTAL_MOVE_INPUT_INTERVAL_MS);

    const stop = () => {
      heldScenePointerRef.current = null;
    };
    window.addEventListener("mouseup", stop);
    window.addEventListener("blur", stop);

    return () => {
      window.clearInterval(timer);
      window.removeEventListener("mouseup", stop);
      window.removeEventListener("blur", stop);
    };
  }, [screen, onViewportDirectionStep]);

  return (
    <main className="mir-client-page">
      <section className="mir-stage">
        <div
          ref={stageFrameRef}
          className="client-stage-frame"
          onMouseDownCapture={(event) => {
            if (screen === "game" && event.button === 2) {
              event.preventDefault();
            }
          }}
          onMouseDown={handleScenePointerAction}
          onMouseMove={handleScenePointerMove}
          onMouseUp={stopHeldScenePointer}
          onContextMenuCapture={(event) => {
            if (screen === "game") {
              event.preventDefault();
            }
          }}
          onContextMenu={(event) => {
            if (screen === "game") {
              event.preventDefault();
            }
          }}
        >
          {screen === "login" ? (
            <div className="client-scene-overlay">
              <img
                className="client-scene-background"
                src={loginBackgroundFrame}
                alt=""
                draggable={false}
              />
            </div>
          ) : null}
          {loginTransitionBackground ? (
            <div className="client-scene-overlay login-transition-overlay" aria-hidden="true">
              <img
                className="client-scene-background"
                src={loginTransitionBackground}
                alt=""
                draggable={false}
              />
            </div>
          ) : null}
          {showSyntheticScene ? <div className="game-scene-underlay" /> : null}
          <canvas id="mir2-web3-canvas" />
          {screen === "game" ? (
            <GameSceneBackdrop
              world={world}
              player={player}
              floorSprites={viewportMapSprites.floor}
              cameraOffset={playerCameraMotionOffset}
            />
          ) : null}

          <div className={`viewport-grid-overlay ${screen !== "game" ? "hidden" : ""}`}>
            {viewportTiles.map((tile) => (
              <button
                key={`tile-${tile.x}-${tile.y}`}
                type="button"
                className="tile-hit"
                style={{
                  left: `${VIEWPORT_MOUSE_TILE_CENTER_X + tile.dx * VIEWPORT_CELL_WIDTH}px`,
                  top: `${VIEWPORT_MOUSE_TILE_CENTER_Y + tile.dy * VIEWPORT_CELL_HEIGHT}px`,
                }}
                data-ui-interactive="true"
                onMouseDown={(event) => {
                  if (event.button !== 0 && event.button !== 2) {
                    return;
                  }

                  event.stopPropagation();
                  const point = scenePointFromMouseEvent(event);
                  const pointer: HeldScenePointer = {
                    button: event.button,
                    sceneX: point.sceneX,
                    sceneY: point.sceneY,
                    startedAt: Date.now(),
                    dispatched: false,
                    tileX: tile.x,
                    tileY: tile.y,
                  };
                  heldScenePointerRef.current = pointer;
                  if (event.button === 2) {
                    event.preventDefault();
                  }
                }}
                onMouseMove={(event) => {
                  const held = heldScenePointerRef.current;
                  if (!held) return;
                  const point = scenePointFromMouseEvent(event);
                  heldScenePointerRef.current = {
                    ...held,
                    sceneX: point.sceneX,
                    sceneY: point.sceneY,
                  };
                }}
                onMouseUp={stopHeldScenePointer}
                onClick={(event) => {
                  event.preventDefault();
                  event.stopPropagation();
                }}
                onContextMenu={(event) => {
                  event.preventDefault();
                  event.stopPropagation();
                }}
                aria-label={`tile ${tile.x}, ${tile.y}`}
              />
            ))}
          </div>

          <OriginalClientSceneVisualLayers
            screen={screen}
            t={t}
            world={world}
            player={player}
            selectedEntity={selectedEntity}
            viewportGroundDrops={viewportGroundDrops}
            viewportMapSprites={viewportMapSprites}
            viewportEntitySprites={viewportEntitySprites}
            viewportProjectiles={viewportProjectiles}
            viewportDepthPlayer={viewportDepthPlayer}
            playerCameraMotionOffset={playerCameraMotionOffset}
            entityMotionSnapshots={entityMotionSnapshotsRef.current}
            motionNow={motionNow}
            sceneSpriteFrameIndex={sceneSpriteFrameIndex}
            entityKindClassName={entityKindClassName}
            onPickGroundDrop={onPickGroundDrop}
            onActivateEntity={onActivateEntity}
          />
          {screen === "login" ? (
            <LoginOverlay
              language={language}
              t={t}
              runtimePhase={runtimePhaseLabel}
              runtimeMessage={runtimeMessageLabel}
              wsState={wsState}
              accountId={accountId}
              password={password}
              loginBusy={loginBusy}
              loginError={loginError}
              onLanguageChange={onLanguageChange}
              onAccountIdChange={onAccountIdChange}
              onPasswordChange={onPasswordChange}
              onCreateAccount={onCreateAccount}
              onSubmitLogin={onSubmitLogin}
              onQuickEnter={onQuickEnter}
              onResetClient={onResetClient}
            />
          ) : null}
          {screen === "select" ? (
            <SelectOverlay
              language={language}
              t={t}
              characters={characters}
              selectedCharacterIndex={selectedCharacterIndex}
              accountId={accountId}
              selectedPortraitFrame={activeSelectPortraitFrame}
              onLanguageChange={onLanguageChange}
              onSelectCharacter={onSelectCharacter}
              onEnterWorld={onEnterWorld}
              onCreateCharacter={onCreateCharacter}
              onDeleteCharacter={onDeleteCharacter}
              onExit={onExitSelect}
            />
          ) : null}
          {screen === "game" ? (
            <GameUiScene
              t={t}
              locale={locale}
              runtimeMessage={runtimeMessageLabel}
              world={world}
              player={player}
              logs={logs}
              chatMessage={chatMessage}
              showInventory={showInventory}
              showCharacter={showCharacter}
              activeInventoryTab={activeInventoryTab}
              activeCharacterTab={activeCharacterTab}
              storageServiceOpenVersion={storageServiceOpenVersion}
              onChatMessageChange={onChatMessageChange}
              onSendChat={onSendChat}
              onRentExpandedStorage={onRentExpandedStorage}
              onLogout={onLogout}
              onToggleCharacter={onToggleCharacter}
              onToggleInventory={onToggleInventory}
              onCloseCharacter={onCloseCharacter}
              onCloseInventory={onCloseInventory}
              onOpenCharacterTab={onOpenCharacterTab}
              onOpenInventoryTab={onOpenInventoryTab}
              onSelectNpcDialogTarget={onSelectNpcDialogTarget}
              onSubmitNpcInput={onSubmitNpcInput}
              onUseItem={onUseItem}
              onDropItem={onDropItem}
              onEquipItem={onEquipItem}
              onRemoveItem={onRemoveItem}
              onMoveItem={onMoveItem}
              onMergeItem={onMergeItem}
              onSplitItem={onSplitItem}
              onStoreItem={onStoreItem}
              onTakeBackItem={onTakeBackItem}
              onUnlockStorage={onUnlockStorage}
              onSetStoragePassword={onSetStoragePassword}
              onRemoveStoragePassword={onRemoveStoragePassword}
              onSellItem={onSellItem}
              onDropGold={onDropGold}
              onRepairItem={onRepairItem}
              onSpecialRepairItem={onSpecialRepairItem}
              onCastSkill={onCastSkill}
              onTransferMap={onTransferMap}
              onClaimMail={onClaimMail}
              onDeleteMail={onDeleteMail}
              onBuyGameShopItem={onBuyGameShopItem}
              onRunStage5Command={onRunStage5Command}
              onSendClientCommand={onSendClientCommand}
              transferOptions={transferOptions}
            />
          ) : null}
        </div>
      </section>
    </main>
  );
}
