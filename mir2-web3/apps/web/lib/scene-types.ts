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
};

export type OriginalMapSprite = {
  kind: "back" | "middle" | "front" | "tileAnimation";
  drawMode: "floor" | "object";
  frames: OriginalMapSpriteFrame[];
};

export type OriginalMapCell = {
  x: number;
  y: number;
  back?: string | null;
  middle?: string | null;
  front?: string | null;
  tileAnimation?: string | null;
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
};

export type SceneBlueprint = {
  mapTitle: string | null;
  miniMapIndex: number | null;
  sceneView: SceneView | null;
  terrainPatches: TerrainPatch[];
  decorObjects: DecorObject[];
  originalMapRegion: OriginalMapRegion | null;
};
