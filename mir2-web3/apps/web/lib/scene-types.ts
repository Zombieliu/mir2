export type TilePoint = {
  x: number;
  y: number;
};

export type TerrainKind = "grass" | "dirt" | "road" | "water" | "stone";

export type DecorKind =
  | "lantern"
  | "banner"
  | "tree"
  | "rock"
  | "campfire"
  | "stump";

export type SceneView = {
  center: TilePoint;
  width: number;
  height: number;
};

export type TerrainPatch = {
  x: number;
  y: number;
  width: number;
  height: number;
  kind: TerrainKind;
};

export type DecorObject = {
  id: string;
  x: number;
  y: number;
  kind: DecorKind;
};

export type OriginalMapSpriteFrame = {
  path: string;
  width: number;
  height: number;
  offsetX?: number;
  offsetY?: number;
};

export type OriginalMapSprite = {
  kind: "back" | "middle" | "front" | "tileAnimation";
  drawMode: "floor" | "object";
  blendMode?: "normal" | "additive";
  frames: OriginalMapSpriteFrame[];
};

export type OriginalMapCell = {
  x: number;
  y: number;
  back?: string | null;
  middle?: string | null;
  front?: string | null;
  tileAnimation?: string | null;
  light?: number;
  lightOffsetX?: number;
  lightOffsetY?: number;
  blocked?: boolean;
  /** Crystal map door group for this cell. Zero/absent means no door. */
  doorIndex?: number;
  closedDoor?: boolean;
};

export type OriginalMapMissingAsset = {
  resourceType: "map" | "library" | "frame" | "png";
  path: string;
  libraryKey?: string;
  frameIndex?: number;
};

export type OriginalMapRegion = {
  mapFileName: string;
  mapWidth: number;
  mapHeight: number;
  cellWidth: number;
  cellHeight: number;
  regionBounds: {
    minX: number;
    maxX: number;
    minY: number;
    maxY: number;
  };
  playBounds: {
    minX: number;
    maxX: number;
    minY: number;
    maxY: number;
  };
  sprites: Record<string, OriginalMapSprite>;
  cells: OriginalMapCell[];
  /**
   * Assets that could not be resolved while building this region. Populated when the loader
   * runs in graceful mode (the default at runtime): a missing sprite frame or library is
   * skipped and recorded here instead of failing the whole scene with HTTP 424. Omitted when
   * every referenced asset resolved. Strict mode (`MIR2_STRICT_ASSET_RESOLUTION=1`) restores
   * hard-fail behaviour for CI/release gating.
   */
  missingAssets?: OriginalMapMissingAsset[];
};

export type SceneBlueprint = {
  mapTitle: string | null;
  miniMapIndex: number | null;
  bigMapIndex: number | null;
  sceneView: SceneView | null;
  terrainPatches: TerrainPatch[];
  decorObjects: DecorObject[];
  originalMapRegion: OriginalMapRegion | null;
};
