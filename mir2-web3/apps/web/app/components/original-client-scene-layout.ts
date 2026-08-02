import { ORIGINAL_UI } from "../../lib/original-ui";
import type { DisplayEntity } from "./original-client-types";

export const VIEWPORT_CELL_WIDTH = 48;
export const VIEWPORT_CELL_HEIGHT = 32;
export const VIEWPORT_OFFSET_X = Math.floor(ORIGINAL_UI.game.sceneWidth / 2 / VIEWPORT_CELL_WIDTH);
export const VIEWPORT_OFFSET_Y = Math.floor(ORIGINAL_UI.game.sceneHeight / 2 / VIEWPORT_CELL_HEIGHT) - 1;
export const VIEWPORT_RANGE_X = VIEWPORT_OFFSET_X + 6;
export const VIEWPORT_RANGE_Y = VIEWPORT_OFFSET_Y + 6;
export const VIEWPORT_TILE_LEFT_ORIGIN = VIEWPORT_OFFSET_X * VIEWPORT_CELL_WIDTH - VIEWPORT_OFFSET_X;
export const VIEWPORT_TILE_TOP_ORIGIN = VIEWPORT_OFFSET_Y * VIEWPORT_CELL_HEIGHT;
export const VIEWPORT_TILE_CENTER_X = VIEWPORT_TILE_LEFT_ORIGIN + VIEWPORT_CELL_WIDTH / 2;
export const VIEWPORT_TILE_CENTER_Y = VIEWPORT_TILE_TOP_ORIGIN + VIEWPORT_CELL_HEIGHT / 2;
export const VIEWPORT_ENTITY_LEFT_ORIGIN = VIEWPORT_OFFSET_X * VIEWPORT_CELL_WIDTH;
export const VIEWPORT_ENTITY_TOP_ORIGIN = VIEWPORT_OFFSET_Y * VIEWPORT_CELL_HEIGHT;
export const VIEWPORT_MOUSE_TILE_CENTER_X = VIEWPORT_ENTITY_LEFT_ORIGIN + VIEWPORT_CELL_WIDTH / 2;
export const VIEWPORT_MOUSE_TILE_CENTER_Y = VIEWPORT_ENTITY_TOP_ORIGIN + VIEWPORT_CELL_HEIGHT / 2;

/**
 * Screen-space geometry for one rendered world stage. The original client was
 * authored around a 1024x768 stage, but the opt-in wide-mobile shell can expose
 * additional columns without changing tile size. Keeping these values together
 * prevents DOM, WebGL/Bevy, pointer hit-testing, and prefetch from drifting.
 */
export type ViewportLayout = {
  stageWidth: number;
  stageHeight: number;
  offsetX: number;
  offsetY: number;
  rangeX: number;
  rangeY: number;
  tileLeftOrigin: number;
  tileTopOrigin: number;
  tileCenterX: number;
  tileCenterY: number;
  entityLeftOrigin: number;
  entityTopOrigin: number;
  mouseTileCenterX: number;
  mouseTileCenterY: number;
};

export const DEFAULT_VIEWPORT_LAYOUT: ViewportLayout = {
  stageWidth: ORIGINAL_UI.game.sceneWidth,
  stageHeight: ORIGINAL_UI.game.sceneHeight,
  offsetX: VIEWPORT_OFFSET_X,
  offsetY: VIEWPORT_OFFSET_Y,
  rangeX: VIEWPORT_RANGE_X,
  rangeY: VIEWPORT_RANGE_Y,
  tileLeftOrigin: VIEWPORT_TILE_LEFT_ORIGIN,
  tileTopOrigin: VIEWPORT_TILE_TOP_ORIGIN,
  tileCenterX: VIEWPORT_TILE_CENTER_X,
  tileCenterY: VIEWPORT_TILE_CENTER_Y,
  entityLeftOrigin: VIEWPORT_ENTITY_LEFT_ORIGIN,
  entityTopOrigin: VIEWPORT_ENTITY_TOP_ORIGIN,
  mouseTileCenterX: VIEWPORT_MOUSE_TILE_CENTER_X,
  mouseTileCenterY: VIEWPORT_MOUSE_TILE_CENTER_Y,
};

export function viewportLayoutForStage(
  stageWidth: number = ORIGINAL_UI.game.sceneWidth,
  stageHeight: number = ORIGINAL_UI.game.sceneHeight,
): ViewportLayout {
  const safeWidth = Math.max(VIEWPORT_CELL_WIDTH, Math.floor(stageWidth));
  const safeHeight = Math.max(VIEWPORT_CELL_HEIGHT, Math.floor(stageHeight));
  const offsetX = Math.floor(safeWidth / 2 / VIEWPORT_CELL_WIDTH);
  const offsetY = Math.floor(safeHeight / 2 / VIEWPORT_CELL_HEIGHT) - 1;
  const tileLeftOrigin = offsetX * VIEWPORT_CELL_WIDTH - offsetX;
  const tileTopOrigin = offsetY * VIEWPORT_CELL_HEIGHT;
  const tileCenterX = tileLeftOrigin + VIEWPORT_CELL_WIDTH / 2;
  const tileCenterY = tileTopOrigin + VIEWPORT_CELL_HEIGHT / 2;
  const entityLeftOrigin = offsetX * VIEWPORT_CELL_WIDTH;
  const entityTopOrigin = offsetY * VIEWPORT_CELL_HEIGHT;
  return {
    stageWidth: safeWidth,
    stageHeight: safeHeight,
    offsetX,
    offsetY,
    rangeX: offsetX + 6,
    rangeY: offsetY + 6,
    tileLeftOrigin,
    tileTopOrigin,
    tileCenterX,
    tileCenterY,
    entityLeftOrigin,
    entityTopOrigin,
    mouseTileCenterX: entityLeftOrigin + VIEWPORT_CELL_WIDTH / 2,
    mouseTileCenterY: entityTopOrigin + VIEWPORT_CELL_HEIGHT / 2,
  };
}
export const CRYSTAL_MOVE_INPUT_INTERVAL_MS = 100;
export const CRYSTAL_MOVE_FRAME_COUNT = 6;
export const CRYSTAL_MOVE_FRAME_INTERVAL_MS = 100;
export const MAX_PREDICTED_PLAYER_LEAD_TILES = 2;

export type SceneBackdropTile = {
  key: string;
  left: number;
  top: number;
  texture: string;
  tint: string;
};

export type ViewportMapSprite = {
  key: string;
  path: string;
  // All source frames for the visible map animation. The standalone renderer
  // uses this to make additive-family handoff to Bevy atomic.
  animationFramePaths?: string[];
  kind: "back" | "middle" | "front" | "tileAnimation";
  blendMode?: "normal" | "additive";
  cellX: number;
  cellY: number;
  left: number;
  top: number;
  width: number;
  height: number;
  zIndex: number;
};

export type ViewportMapSprites = {
  floor: ViewportMapSprite[];
  objects: ViewportMapSprite[];
};

export type ViewportOffset = {
  x: number;
  y: number;
};

export const EMPTY_VIEWPORT_MAP_SPRITES: ViewportMapSprites = {
  floor: [],
  objects: [],
};

export const EMPTY_VIEWPORT_OFFSET: ViewportOffset = {
  x: 0,
  y: 0,
};

const VIEWPORT_ROW_Z_STRIDE = 128;
const VIEWPORT_BASE_Z = 4096;
const VIEWPORT_FLOOR_LAYER_Z_STRIDE = 256;
const VIEWPORT_FLOOR_ROW_Z_STRIDE = 4;

export function ratio(value?: number, max?: number) {
  if (value === undefined || max === undefined || max <= 0) {
    return 0;
  }

  return Math.max(0, Math.min(1, value / max));
}

export function viewportDepthForCell(
  x: number,
  y: number,
  player: Pick<DisplayEntity, "x" | "y">,
  layerOffset = 0,
) {
  return VIEWPORT_BASE_Z + (y - player.y) * VIEWPORT_ROW_Z_STRIDE + (x - player.x) * 2 + layerOffset;
}

export function viewportFloorDepthForCell(
  x: number,
  y: number,
  player: Pick<DisplayEntity, "x" | "y">,
  layerOrder: number,
  viewportLayout: ViewportLayout = DEFAULT_VIEWPORT_LAYOUT,
) {
  const localRow = y - player.y + viewportLayout.rangeY;
  const localColumn = x - player.x + viewportLayout.rangeX;
  return (
    layerOrder * VIEWPORT_FLOOR_LAYER_Z_STRIDE +
    localRow * VIEWPORT_FLOOR_ROW_Z_STRIDE +
    localColumn * 0.01
  );
}

export function argbToCssColor(value: number | undefined) {
  if (value === undefined || value === -1) {
    return undefined;
  }

  const argb = value >>> 0;
  const alpha = (argb >>> 24) & 0xff;
  if (alpha === 0) {
    return undefined;
  }

  const red = (argb >>> 16) & 0xff;
  const green = (argb >>> 8) & 0xff;
  const blue = argb & 0xff;
  if (alpha === 0xff) {
    return `#${red.toString(16).padStart(2, "0")}${green.toString(16).padStart(2, "0")}${blue
      .toString(16)
      .padStart(2, "0")}`;
  }

  return `rgba(${red}, ${green}, ${blue}, ${(alpha / 255).toFixed(3)})`;
}
