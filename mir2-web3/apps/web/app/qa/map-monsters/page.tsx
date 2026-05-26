import { notFound } from "next/navigation";
import type { CSSProperties } from "react";

import { loadCrystalSceneBlueprint } from "../../../lib/crystal-map-loader";
import {
  getQaMapMonsterScene,
  normalizeQaMapMonsterLimit,
  type QaMapMonsterRespawn,
} from "../../../lib/qa-map-monster-scenes";
import type { OriginalMapRegion, OriginalMapSpriteFrame } from "../../../lib/scene-types";

export const dynamic = "force-dynamic";
export const runtime = "nodejs";

type QaMapMonstersPageProps = {
  searchParams?: Promise<Record<string, string | string[] | undefined>>;
};

type RenderSprite = {
  key: string;
  path: string;
  left: number;
  top: number;
  width: number;
  height: number;
  zIndex: number;
  kind: string;
};

const SCENE_WIDTH = 1024;
const SCENE_HEIGHT = 768;
const CELL_WIDTH = 48;
const CELL_HEIGHT = 32;
const TILE_LEFT_ORIGIN = 470;
const TILE_TOP_ORIGIN = 352;
const ENTITY_LEFT_ORIGIN = 480;
const ENTITY_TOP_ORIGIN = 352;
const REGION_WIDTH = 32;
const REGION_HEIGHT = 34;

export default async function QaMapMonstersPage({ searchParams }: QaMapMonstersPageProps) {
  const params = (await searchParams) ?? {};
  const sceneKey = paramValue(params.scene);
  const limitPerMap = normalizeQaMapMonsterLimit(paramValue(params.limitPerMap));
  const { list, scene } = getQaMapMonsterScene(sceneKey, limitPerMap);
  if (!scene) notFound();

  const blueprint = await loadCrystalSceneBlueprint({
    mapFileName: scene.mapFileName,
    centerX: scene.center.x,
    centerY: scene.center.y,
    width: REGION_WIDTH,
    height: REGION_HEIGHT,
  });
  const region = blueprint.originalMapRegion;
  const renderCenter = blueprint.sceneView?.center ?? scene.center;
  const sprites = region ? buildRenderSprites(region, renderCenter) : [];

  return (
    <main
      data-qa-map-monster-ready="true"
      data-scene-id={scene.id}
      data-scene-index={scene.sceneIndex}
      data-scene-count={list.sceneCount}
      data-map-file-name={scene.mapFileName}
      data-map-title={scene.mapTitle ?? ""}
      data-center-x={scene.center.x}
      data-center-y={scene.center.y}
      data-render-center-x={renderCenter.x}
      data-render-center-y={renderCenter.y}
      data-visible-respawn-count={scene.visibleRespawns.length}
      style={styles.page}
    >
      <section style={styles.stage} aria-label="Crystal map monster respawn QA scene">
        <div style={styles.mapLayer}>
          {sprites.map((sprite) => (
            <img
              alt=""
              data-map-sprite-kind={sprite.kind}
              decoding="sync"
              key={sprite.key}
              src={sprite.path}
              style={{
                ...styles.sprite,
                left: sprite.left,
                top: sprite.top,
                width: sprite.width,
                height: sprite.height,
                zIndex: sprite.zIndex,
              }}
            />
          ))}
        </div>
        <div style={styles.markerLayer}>
          {scene.visibleRespawns.map((respawn, index) => (
            <RespawnMarker
              renderCenter={renderCenter}
              isSelected={respawn.respawnIndex === scene.selectedRespawn.respawnIndex}
              key={`${respawn.respawnIndex}-${respawn.location.x}-${respawn.location.y}-${index}`}
              respawn={respawn}
            />
          ))}
        </div>
        <div style={styles.hud}>
          <div style={styles.hudTitle}>
            {scene.mapTitle ?? scene.mapFileName} / {scene.mapFileName}
          </div>
          <div>
            scene {scene.sceneIndex + 1}/{list.sceneCount} · center {scene.center.x}:{scene.center.y} · render{" "}
            {renderCenter.x}:{renderCenter.y}
          </div>
          <div>
            selected {scene.selectedRespawn.monsterName} · count {scene.selectedRespawn.count} · spread{" "}
            {scene.selectedRespawn.spread} · respawn #{scene.selectedRespawn.respawnIndex}
          </div>
          <div>
            source {list.respawnMapCount}/{list.sourceMapCount} maps · {list.positiveRespawnRowCount} respawn rows ·{" "}
            limit {list.limitPerMap}/map
          </div>
          <div style={styles.hudMonsters}>{scene.dominantMonsters.join(", ")}</div>
        </div>
      </section>
    </main>
  );
}

function RespawnMarker({
  renderCenter,
  isSelected,
  respawn,
}: {
  renderCenter: { x: number; y: number };
  isSelected: boolean;
  respawn: QaMapMonsterRespawn;
}) {
  const originalLeft = ENTITY_LEFT_ORIGIN + (respawn.location.x - renderCenter.x) * CELL_WIDTH;
  const originalTop = ENTITY_TOP_ORIGIN + (respawn.location.y - renderCenter.y) * CELL_HEIGHT;
  const left = clamp(originalLeft, 24, SCENE_WIDTH - 24);
  const top = clamp(originalTop, 36, SCENE_HEIGHT - 36);
  const clamped = left !== originalLeft || top !== originalTop;
  const spreadDiameter = Math.max(18, Math.min(220, respawn.spread * 4));
  return (
    <div
      data-monster-name={respawn.monsterName}
      data-respawn-index={respawn.respawnIndex}
      data-respawn-count={respawn.count}
      data-respawn-spread={respawn.spread}
      data-marker-clamped={clamped ? "true" : "false"}
      style={{
        ...styles.marker,
        left,
        top,
        zIndex: 30000 + respawn.location.y * 16 + respawn.location.x + (isSelected ? 100000 : 0),
      }}
    >
      <span
        aria-hidden="true"
        style={{
          ...styles.spreadRing,
          borderColor: isSelected ? "rgba(255, 225, 92, 0.9)" : "rgba(81, 224, 255, 0.65)",
          height: spreadDiameter,
          width: spreadDiameter,
        }}
      />
      <span
        aria-hidden="true"
        style={{
          ...styles.markerDot,
          background: isSelected ? "#ffd95c" : "#51e0ff",
          boxShadow: isSelected
            ? "0 0 0 2px #1a0b00, 0 0 16px rgba(255, 217, 92, 0.9)"
            : "0 0 0 2px #00151a, 0 0 12px rgba(81, 224, 255, 0.75)",
        }}
      />
      <span style={styles.markerLabel}>
        {respawn.monsterName} x{respawn.count}
        {clamped ? ` @ ${respawn.location.x}:${respawn.location.y}` : ""}
      </span>
    </div>
  );
}

function buildRenderSprites(region: OriginalMapRegion, center: { x: number; y: number }): RenderSprite[] {
  const sprites: RenderSprite[] = [];
  for (const cell of region.cells) {
    const cellLeft = TILE_LEFT_ORIGIN + (cell.x - center.x) * CELL_WIDTH;
    const cellTop = TILE_TOP_ORIGIN + (cell.y - center.y) * CELL_HEIGHT;
    addCellSprite(sprites, region, cell.back, cellLeft, cellTop, cell.x, cell.y, "back", 0);
    addCellSprite(sprites, region, cell.middle, cellLeft, cellTop, cell.x, cell.y, "middle", 10000);
    addCellSprite(sprites, region, cell.front, cellLeft, cellTop, cell.x, cell.y, "front", 20000);
    addCellSprite(sprites, region, cell.tileAnimation, cellLeft, cellTop, cell.x, cell.y, "tileAnimation", 25000);
  }
  return sprites;
}

function addCellSprite(
  sprites: RenderSprite[],
  region: OriginalMapRegion,
  spriteKey: string | null | undefined,
  cellLeft: number,
  cellTop: number,
  cellX: number,
  cellY: number,
  kind: string,
  zOffset: number,
) {
  if (!spriteKey) return;
  const sprite = region.sprites[spriteKey];
  const frame = sprite?.frames?.[0];
  if (!sprite || !frame) return;

  const placement = placeFrame(frame, sprite.drawMode, cellLeft, cellTop);
  if (!intersectsViewport(placement.left, placement.top, frame.width, frame.height)) return;
  sprites.push({
    key: `${spriteKey}-${cellX}-${cellY}-${kind}`,
    path: frame.path,
    left: placement.left,
    top: placement.top,
    width: frame.width,
    height: frame.height,
    zIndex: zOffset + cellY * 128 + cellX * 2,
    kind,
  });
}

function placeFrame(
  frame: OriginalMapSpriteFrame,
  drawMode: "floor" | "object",
  cellLeft: number,
  cellTop: number,
) {
  if (drawMode === "floor") {
    return { left: cellLeft, top: cellTop };
  }
  return {
    left: cellLeft + (frame.offsetX ?? 0),
    top: cellTop + CELL_HEIGHT - frame.height + (frame.offsetY ?? 0),
  };
}

function intersectsViewport(left: number, top: number, width: number, height: number): boolean {
  return left < SCENE_WIDTH + 160 && left + width > -160 && top < SCENE_HEIGHT + 160 && top + height > -160;
}

function clamp(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, value));
}

function paramValue(value: string | string[] | undefined): string | null {
  if (Array.isArray(value)) return value[0] ?? null;
  return value ?? null;
}

const styles = {
  page: {
    alignItems: "center",
    background: "#050505",
    color: "#f6ead3",
    display: "flex",
    fontFamily: "Georgia, 'Times New Roman', serif",
    height: "100vh",
    justifyContent: "center",
    margin: 0,
    overflow: "hidden",
    width: "100vw",
  },
  stage: {
    background: "#10100d",
    height: SCENE_HEIGHT,
    overflow: "hidden",
    position: "relative",
    width: SCENE_WIDTH,
  },
  mapLayer: {
    inset: 0,
    position: "absolute",
    zIndex: 0,
  },
  markerLayer: {
    inset: 0,
    pointerEvents: "none",
    position: "absolute",
    zIndex: 1000000,
  },
  sprite: {
    imageRendering: "pixelated",
    position: "absolute",
  },
  marker: {
    position: "absolute",
    transform: "translate(-50%, -100%)",
  },
  spreadRing: {
    border: "2px solid rgba(255, 225, 92, 0.8)",
    borderRadius: "999px",
    left: "50%",
    opacity: 0.7,
    position: "absolute",
    top: "100%",
    transform: "translate(-50%, -50%)",
  },
  markerDot: {
    borderRadius: "999px",
    display: "block",
    height: 12,
    left: "50%",
    position: "absolute",
    top: "100%",
    transform: "translate(-50%, -50%)",
    width: 12,
  },
  markerLabel: {
    background: "rgba(12, 8, 4, 0.82)",
    border: "1px solid rgba(255, 232, 170, 0.55)",
    color: "#fff6df",
    display: "block",
    fontSize: 16,
    fontWeight: 700,
    lineHeight: "18px",
    maxWidth: 180,
    padding: "2px 6px",
    textShadow: "0 1px 1px #000, 1px 0 1px #000",
    transform: "translate(-50%, -8px)",
    whiteSpace: "nowrap",
  },
  hud: {
    background: "rgba(8, 6, 3, 0.78)",
    border: "1px solid rgba(255, 232, 170, 0.45)",
    bottom: 12,
    boxShadow: "0 8px 28px rgba(0, 0, 0, 0.45)",
    fontSize: 15,
    left: 12,
    lineHeight: "20px",
    maxWidth: 720,
    padding: "10px 12px",
    position: "absolute",
    textShadow: "0 1px 1px #000",
    zIndex: 1000001,
  },
  hudTitle: {
    color: "#ffe28c",
    fontSize: 18,
    fontWeight: 700,
    lineHeight: "22px",
  },
  hudMonsters: {
    color: "#9fffd1",
    maxWidth: 690,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
} satisfies Record<string, CSSProperties>;
