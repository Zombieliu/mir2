"use client";

import { useEffect, useState } from "react";

import {
  createLinearMiniMapTransform,
  findCrystalMiniMapTransform,
  miniMapImagePointToViewportPoint,
  worldToMiniMapImagePoint,
  type CrystalMiniMapPoint,
  type CrystalMiniMapTransform,
} from "../../lib/crystal-minimap-transform";
import { CRYSTAL_MINI_MAP_TRANSFORMS } from "../../lib/generated/crystal-minimap-transforms";
import { ORIGINAL_UI } from "../../lib/original-ui";
import { CRYSTAL_BIG_MAP_NPCS } from "../../lib/generated/crystal-npc-info-data";
import miniMapMeta from "../../public/original-ui/MMap/meta.json";
import { SpriteButton } from "./original-client-overlays";
import {
  handleSceneAssetImageError,
  handleSceneAssetImageLoad,
} from "./original-client-scene-map-rendering";

type TranslateFn = (
  key: string,
  params?: Array<string | number>,
  fallback?: string,
) => string;

type MiniMapLibraryMeta = {
  frames: Array<{
    index: number;
    width: number;
    height: number;
    path: string;
  }>;
};

const MINI_MAP_ASSETS = new Map(
  (miniMapMeta as MiniMapLibraryMeta).frames.map((frame) => [
    frame.index,
    { src: frame.path, width: frame.width, height: frame.height },
  ]),
);

type MapRasterAsset = {
  src: string;
  width: number;
  height: number;
};

type DisplayEntity = {
  objectId: string;
  kind: "selfPlayer" | "player" | "monster" | "npc";
  name: string;
  x: number;
  y: number;
  bigMapIcon?: number;
  showOnBigMap?: boolean;
  canTeleportTo?: boolean;
};

type DisplayWorld = {
  mapTitle: string | null;
  mapFileName: string | null;
  inSafeZone: boolean;
  miniMapIndex: number | null;
  bigMapIndex?: number | null;
  originalMapRegion: { mapWidth: number; mapHeight: number } | null;
  entities: DisplayEntity[];
  terrainPatches: Array<{
    x: number;
    y: number;
    width: number;
    height: number;
    kind: "grass" | "dirt" | "road" | "water" | "stone";
  }>;
};

type BigMapNpcRowView = {
  key: string;
  name: string;
  icon: number;
  x: number;
  y: number;
  canTeleportTo: boolean;
};

const BIG_MAP_NPC_INDEX = new Map(
  CRYSTAL_BIG_MAP_NPCS.map((npc) => [bigMapNpcKey(npc.map, npc.name, npc.x, npc.y), npc]),
);
const MINI_MAP_VIEW_WIDTH = 120;
const MINI_MAP_VIEW_HEIGHT = 108;

export function BigMapDialog({
  t,
  world,
  player,
  onClose,
}: {
  t: TranslateFn;
  world: DisplayWorld;
  player: DisplayEntity | null;
  onClose: () => void;
}) {
  const [showWorldMap, setShowWorldMap] = useState(false);
  const mapDebug = useMapDebugEnabled();
  const bigMapAsset =
    originalBigMapAssetPath(world.bigMapIndex) ?? originalMiniMapAssetPath(world.miniMapIndex);
  const viewport = bigMapViewport(bigMapAsset);
  const transform = mapPanelTransformForWorld(world, bigMapAsset, "big");
  const playerImagePoint = transform && player ? worldToMiniMapImagePoint(transform, player) : null;
  const playerViewportPoint = playerImagePoint
    ? bigMapImagePointToViewportPoint(playerImagePoint, viewport, bigMapAsset)
    : null;
  const coordinateLabel = player ? `[ ${player.x}, ${player.y} ]` : "[ 0, 0 ]";
  const npcRows = bigMapNpcRowsForWorld(world).slice(0, 18);
  const mapDebugNpcRows = mapDebug ? bigMapNpcRowsForWorld(world) : [];

  return (
    <section className="big-map-dialog" aria-label={t("client.BigMapKey", ["M"], t("ui.map"))}>
      <img
        className="big-map-frame"
        src={ORIGINAL_UI.bigMap.frame}
        alt=""
        draggable={false}
        data-mir2-original-src={ORIGINAL_UI.bigMap.frame}
        onError={handleSceneAssetImageError}
        onLoad={handleSceneAssetImageLoad}
      />
      <div className="big-map-title">{world.mapTitle ?? world.mapFileName ?? ""}</div>
      <div className="big-map-close"><SpriteButton sprite={ORIGINAL_UI.bigMap.closeButton} label={t("ui.close")} onClick={onClose} /></div>
      <div className="big-map-scroll up"><SpriteButton sprite={ORIGINAL_UI.bigMap.upButton} label={t("ui.up", [], "Up")} onClick={() => undefined} /></div>
      <div className="big-map-scroll thumb"><SpriteButton sprite={ORIGINAL_UI.bigMap.positionBar} label={t("ui.scroll", [], "Scroll")} onClick={() => undefined} /></div>
      <div className="big-map-scroll down"><SpriteButton sprite={ORIGINAL_UI.bigMap.downButton} label={t("ui.down", [], "Down")} onClick={() => undefined} /></div>
      <div className="big-map-viewport" style={{ left: viewport.left, top: viewport.top, width: viewport.width, height: viewport.height }}>
        {bigMapAsset ? (
          <img
            className="big-map-raster"
            src={bigMapAsset.src}
            alt=""
            draggable={false}
            data-mir2-original-src={bigMapAsset.src}
            onError={handleSceneAssetImageError}
            onLoad={handleSceneAssetImageLoad}
            style={{ width: viewport.contentWidth, height: viewport.contentHeight, left: viewport.imageLeft, top: viewport.imageTop }}
          />
        ) : (
          <div className="big-map-fallback" />
        )}
        {world.entities.map((entity) => {
          const point = bigMapImagePointToViewportPoint(
            worldToMiniMapImagePoint(transform, entity),
            viewport,
            bigMapAsset,
          );
          const left = point.x - 1;
          const top = point.y - 1;
          return <span key={`big-map-dot-${entity.objectId}`} className={`big-map-dot ${entity.kind}`} style={{ left, top }} />;
        })}
        {player ? (
          <img
            className="big-map-user-dot"
            src={ORIGINAL_UI.bigMap.radarDot}
            alt=""
            draggable={false}
            data-mir2-original-src={ORIGINAL_UI.bigMap.radarDot}
            onError={handleSceneAssetImageError}
            onLoad={handleSceneAssetImageLoad}
            style={{
              left: (playerViewportPoint?.x ?? 0) - 6,
              top: (playerViewportPoint?.y ?? 0) - 5,
            }}
          />
        ) : null}
        {mapDebug && transform ? (
          <svg className="big-map-debug-overlay" viewBox={`0 0 ${viewport.width} ${viewport.height}`} preserveAspectRatio="none">
            {mapDebugNpcRows.map((npc) => {
              const point = bigMapImagePointToViewportPoint(
                worldToMiniMapImagePoint(transform, npc),
                viewport,
                bigMapAsset,
              );
              return (
                <circle
                  key={`big-map-debug-npc-${npc.key}`}
                  cx={point.x}
                  cy={point.y}
                  r={2.5}
                  fill="none"
                  stroke="#00ff32"
                  strokeWidth={1}
                />
              );
            })}
          </svg>
        ) : null}
      </div>
      {mapDebug ? (
        <MapDebugReadout
          className="big-map-debug-readout"
          world={world}
          asset={bigMapAsset}
          transform={transform}
          player={player}
          playerImagePoint={playerImagePoint}
          mapKind="big"
        />
      ) : null}
      <div className="big-map-coordinate">{coordinateLabel}</div>
      <div className="big-map-npc-list">
        {npcRows.map((entity, index) => (
          <button
            key={`big-map-npc-${entity.key}`}
            type="button"
            className="big-map-npc-row"
            style={{ top: index * 21 }}
          >
            <img
              className="big-map-npc-icon"
              src={originalMapLinkIconPath(entity.icon)}
              alt=""
              draggable={false}
              data-mir2-original-src={originalMapLinkIconPath(entity.icon)}
              onError={handleSceneAssetImageError}
              onLoad={handleSceneAssetImageLoad}
            />
            <span className="big-map-npc-name">{bigMapNpcDisplayName(entity.name)}</span>
          </button>
        ))}
      </div>
      <div className="big-map-world-button"><SpriteButton sprite={ORIGINAL_UI.bigMap.worldButton} label={t("ui.world", [], "World")} onClick={() => setShowWorldMap(true)} /></div>
      <div className="big-map-my-location-button"><SpriteButton sprite={ORIGINAL_UI.bigMap.myLocationButton} label={t("ui.myLocation", [], "My Location")} onClick={() => setShowWorldMap(false)} /></div>
      <div className="big-map-teleport-button disabled"><SpriteButton sprite={ORIGINAL_UI.bigMap.teleportButton} label={t("ui.teleport", [], "Teleport")} onClick={() => undefined} active /></div>
      <div className="big-map-search-button"><SpriteButton sprite={ORIGINAL_UI.bigMap.searchButton} label={t("ui.search", [], "Search")} onClick={() => undefined} /></div>
      <input className="big-map-search-input" aria-label={t("ui.search", [], "Search")} readOnly />
      {showWorldMap ? (
        <div className="big-map-world-overlay">
          <img
            className="big-map-world-image"
            src={ORIGINAL_UI.bigMap.worldMap}
            alt=""
            draggable={false}
            data-mir2-original-src={ORIGINAL_UI.bigMap.worldMap}
            onError={handleSceneAssetImageError}
            onLoad={handleSceneAssetImageLoad}
          />
          <img
            className="big-map-world-clouds"
            src={ORIGINAL_UI.bigMap.worldClouds}
            alt=""
            draggable={false}
            data-mir2-original-src={ORIGINAL_UI.bigMap.worldClouds}
            onError={handleSceneAssetImageError}
            onLoad={handleSceneAssetImageLoad}
          />
          <img
            className="big-map-world-border"
            src={ORIGINAL_UI.bigMap.worldBorder}
            alt=""
            draggable={false}
            data-mir2-original-src={ORIGINAL_UI.bigMap.worldBorder}
            onError={handleSceneAssetImageError}
            onLoad={handleSceneAssetImageLoad}
          />
        </div>
      ) : null}
    </section>
  );
}


export type MiniMapPanelProps = {
  t: TranslateFn;
  world: DisplayWorld;
  player: DisplayEntity | null;
  showMailPanel: boolean;
  showBigMap: boolean;
  onToggleMail: () => void;
  onToggleBigMap: () => void;
};

export function MiniMapPanel({ t, world, player, showMailPanel, showBigMap, onToggleMail, onToggleBigMap }: MiniMapPanelProps) {
  const [collapsed, setCollapsed] = useState(false);
  const miniMapAsset = originalMiniMapAssetPath(world.miniMapIndex);
  const hasRasterMiniMap = Boolean(miniMapAsset);
  const smallMode = collapsed || !hasRasterMiniMap;
  const panelFrame = smallMode ? ORIGINAL_UI.game.miniMapSmall : ORIGINAL_UI.game.miniMap;
  const mapTitle = crystalMiniMapTitle(world.mapTitle, t);

  useEffect(() => {
    if (hasRasterMiniMap) {
      setCollapsed(false);
    }
  }, [world.miniMapIndex, hasRasterMiniMap]);

  return (
    <section className={`mini-map-panel ${smallMode ? "small" : "large"}`}>
      <img
        className="mini-map-bg"
        src={panelFrame}
        alt=""
        draggable={false}
        data-mir2-original-src={panelFrame}
        onError={handleSceneAssetImageError}
        onLoad={handleSceneAssetImageLoad}
      />
      <div className={`mini-map-scene-shell ${smallMode ? "hidden" : ""}`}>
        <MiniMapScene world={world} player={player} />
      </div>
      {!smallMode ? <div className="mini-map-name">
        <span>{mapTitle}</span>
      </div> : null}
      <div className="mini-map-coords">{player ? `${player.x}:${player.y}` : "--:--"}</div>
      <div className="mini-map-button mail">
        <SpriteButton
          sprite={ORIGINAL_UI.game.miniMapButtons.mail}
          label={t("client.Mail", [], "Mail")}
          onClick={onToggleMail}
          active={showMailPanel}
        />
      </div>
      <div className="mini-map-button bigmap">
        <SpriteButton sprite={ORIGINAL_UI.game.miniMapButtons.bigMap} label={t("client.BigMapKey", ["M"], t("ui.map"))} onClick={onToggleBigMap} active={showBigMap} />
      </div>
      {hasRasterMiniMap ? <div className="mini-map-button toggle">
        <SpriteButton sprite={ORIGINAL_UI.game.miniMapButtons.toggle} label={t("ui.toggleMiniMap")} onClick={() => setCollapsed((current) => !current)} />
      </div> : null}
      <img
        className="mini-map-light"
        src={ORIGINAL_UI.game.miniMapIcons.light}
        alt=""
        draggable={false}
        data-mir2-original-src={ORIGINAL_UI.game.miniMapIcons.light}
        onError={handleSceneAssetImageError}
        onLoad={handleSceneAssetImageLoad}
      />
    </section>
  );
}

function MiniMapScene({
  world,
  player,
}: {
  world: DisplayWorld;
  player: DisplayEntity | null;
}) {
  const mapDebug = useMapDebugEnabled();
  const miniMapAssetPath = originalMiniMapAssetPath(world.miniMapIndex);
  const bounds = miniMapBounds(world, player, miniMapAssetPath);

  if (!bounds) {
    return null;
  }

  const rasterMode = Boolean(miniMapAssetPath && bounds.raster && bounds.imageViewport && bounds.transform);
  const radarDot = rasterMode
    ? { width: 2, height: 2 }
    : {
        width: (bounds.width / MINI_MAP_VIEW_WIDTH) * 2,
        height: (bounds.height / MINI_MAP_VIEW_HEIGHT) * 2,
      };
  const overlayViewBox = rasterMode
    ? `0 0 ${bounds.imageViewport?.width ?? MINI_MAP_VIEW_WIDTH} ${bounds.imageViewport?.height ?? MINI_MAP_VIEW_HEIGHT}`
    : `0 0 ${bounds.width} ${bounds.height}`;
  const playerImagePoint = bounds.transform && player ? worldToMiniMapImagePoint(bounds.transform, player) : null;
  const mapDebugNpcRows = mapDebug ? bigMapNpcRowsForWorld(world) : [];

  return (
    <div className="mini-map-scene">
      {miniMapAssetPath && bounds.raster ? (
        <img
          className="mini-map-raster"
          src={miniMapAssetPath.src}
          alt=""
          draggable={false}
          data-mir2-original-src={miniMapAssetPath.src}
          onError={handleSceneAssetImageError}
          onLoad={handleSceneAssetImageLoad}
          style={miniMapRasterStyle(bounds.raster)}
        />
      ) : (
        <svg className="mini-map-patch-fallback" viewBox={`0 0 ${bounds.width} ${bounds.height}`} preserveAspectRatio="none">
          <rect x="0" y="0" width={bounds.width} height={bounds.height} fill="#090603" />
          {world.terrainPatches.map((patch) => (
            <rect
              key={`patch-${patch.x}-${patch.y}-${patch.kind}`}
              x={patch.x - bounds.minX}
              y={patch.y - bounds.minY}
              width={patch.width}
              height={patch.height}
              fill={miniMapTerrainColor(patch.kind)}
            />
          ))}
        </svg>
      )}
      <svg className="mini-map-overlay" viewBox={overlayViewBox} preserveAspectRatio="none">
        {world.entities.map((entity) => {
          const point = miniMapViewportPointForWorldPoint(bounds, entity);
          return (
            <rect
              key={`mini-${entity.objectId}`}
              x={point.x - radarDot.width / 2}
              y={point.y - radarDot.height / 2}
              width={radarDot.width}
              height={radarDot.height}
              fill={miniMapEntityColor(entity.kind)}
            />
          );
        })}
        {mapDebug && bounds.transform && bounds.imageViewport ? mapDebugNpcRows.map((npc) => {
          const point = miniMapImagePointToViewportPoint(
            worldToMiniMapImagePoint(bounds.transform as CrystalMiniMapTransform, npc),
            bounds.imageViewport,
          );
          return (
            <circle
              key={`mini-map-debug-npc-${npc.key}`}
              cx={point.x}
              cy={point.y}
              r={2.5}
              fill="none"
              stroke="#00ff32"
              strokeWidth={1}
            />
          );
        }) : null}
      </svg>
      {mapDebug ? (
        <MapDebugReadout
          className="mini-map-debug-readout"
          world={world}
          asset={miniMapAssetPath}
          transform={bounds.transform ?? null}
          player={player}
          playerImagePoint={playerImagePoint}
          mapKind="mini"
        />
      ) : null}
    </div>
  );
}

function originalMapLinkIconPath(icon: number) {
  return ORIGINAL_UI.bigMap.mapLinkIcon(icon);
}

function crystalMiniMapTitle(mapTitle: string | null, t: TranslateFn) {
  const fallback = t("content.scene.starterField.title");
  const safeZoneText = t("ui.safeZone", [], "Safe Zone");
  const safeZoneLabels = [safeZoneText, "Safe Zone"].filter(Boolean);
  const normalized = safeZoneLabels.reduce((title, label) => {
    const escaped = label.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    return title
      .replace(new RegExp(`\\s*${escaped}\\s*$`, "i"), "")
      .replace(new RegExp(`^\\s*${escaped}\\s*`, "i"), "");
  }, mapTitle ?? fallback)
    .split(/\r?\n/)
    .map((part) => part.trim())
    .filter(Boolean)
    .filter((part) => !safeZoneLabels.some((label) => part.toLocaleLowerCase() === label.toLocaleLowerCase()));
  return normalized[0] ?? fallback;
}

function bigMapNpcKey(mapFileName: string | null | undefined, name: string, x: number, y: number) {
  return `${mapFileName ?? ""}|${name}|${x}|${y}`;
}

function withBigMapNpcInfo(mapFileName: string | null | undefined, entity: DisplayEntity): DisplayEntity {
  if (entity.kind !== "npc") return entity;

  const info = BIG_MAP_NPC_INDEX.get(bigMapNpcKey(mapFileName, entity.name, entity.x, entity.y));
  if (!info) {
    return entity;
  }

  return {
    ...entity,
    bigMapIcon: info.icon,
    showOnBigMap: true,
    canTeleportTo: info.teleport,
  };
}

function bigMapNpcRowsForWorld(world: DisplayWorld): BigMapNpcRowView[] {
  const mapFileName = world.mapFileName ?? "";
  const manifestRows = CRYSTAL_BIG_MAP_NPCS.filter((npc) => npc.map === mapFileName);
  if (manifestRows.length > 0) {
    return manifestRows.map((npc, index) => ({
      key: `${npc.map}-${npc.name}-${npc.x}-${npc.y}-${index}`,
      name: npc.name,
      icon: npc.icon,
      x: npc.x,
      y: npc.y,
      canTeleportTo: npc.teleport,
    }));
  }

  return world.entities
    .filter((entity) => entity.kind === "npc")
    .map((entity) => withBigMapNpcInfo(world.mapFileName, entity))
    .filter((entity) => entity.showOnBigMap !== false)
    .map((entity) => ({
      key: entity.objectId,
      name: entity.name,
      icon: entity.bigMapIcon ?? 120,
      x: entity.x,
      y: entity.y,
      canTeleportTo: entity.canTeleportTo === true,
    }));
}

function bigMapNpcDisplayName(name: string) {
  if (!name.includes("_")) {
    return name;
  }

  const parts = name.split("_").filter(Boolean);
  if (parts.length <= 1) {
    return name.replace(/_/g, "");
  }

  return `${parts.slice(0, -1).map((part) => `(${part})`).join("")}${parts.at(-1) ?? ""}`;
}

function miniMapBounds(
  world: DisplayWorld,
  player: DisplayEntity | null,
  asset: MapRasterAsset | null,
) {
  if (asset && world.originalMapRegion) {
    const mapWidth = Math.max(world.originalMapRegion.mapWidth, 1);
    const mapHeight = Math.max(world.originalMapRegion.mapHeight, 1);
    const transform = mapPanelTransformForWorld(world, asset, "mini");
    const viewWidth = Math.min(120, asset.width);
    const viewHeight = Math.min(108, asset.height);
    const center = player
      ? worldToMiniMapImagePoint(transform, player)
      : {
          x: (transform.imageMinX + transform.imageMaxX) / 2,
          y: (transform.imageMinY + transform.imageMaxY) / 2,
        };
    const rasterLeft = clampNumber(Math.round(center.x - viewWidth / 2), 0, Math.max(asset.width - viewWidth, 0));
    const rasterTop = clampNumber(Math.round(center.y - viewHeight / 2), 0, Math.max(asset.height - viewHeight, 0));

    return {
      minX: player ? player.x - 12 : mapWidth / 2 - 12,
      minY: player ? player.y - 12 : mapHeight / 2 - 12,
      width: 24,
      height: 24,
      imageViewport: {
        imageLeft: rasterLeft,
        imageTop: rasterTop,
        width: viewWidth,
        height: viewHeight,
      },
      transform,
      raster: {
        left: -rasterLeft,
        top: -rasterTop,
        width: asset.width,
        height: asset.height,
      },
    };
  }

  if (!player) {
    return { minX: 0, minY: 0, width: 48, height: 48, imageViewport: null, transform: null, raster: null };
  }

  return {
    minX: player.x - 12,
    minY: player.y - 12,
    width: 24,
    height: 24,
    imageViewport: null,
    transform: null,
    raster: null,
  };
}

export function hasOriginalMiniMapAsset(miniMapIndex: number | null) {
  return Boolean(originalMiniMapAssetPath(miniMapIndex));
}

function originalMiniMapAssetPath(miniMapIndex: number | null) {
  if (!miniMapIndex || miniMapIndex <= 0) {
    return null;
  }

  return MINI_MAP_ASSETS.get(miniMapIndex) ?? null;
}

function originalBigMapAssetPath(bigMapIndex: number | null | undefined) {
  if (!bigMapIndex || bigMapIndex <= 0) {
    return null;
  }

  return MINI_MAP_ASSETS.get(bigMapIndex) ?? null;
}

function bigMapViewport(asset: MapRasterAsset | null) {
  const maxViewportWidth = 568;
  const maxViewportHeight = 380;
  if (!asset) {
    return {
      left: 14,
      top: 52,
      width: maxViewportWidth,
      height: maxViewportHeight,
      contentWidth: maxViewportWidth,
      contentHeight: maxViewportHeight,
      imageLeft: 0,
      imageTop: 0,
      contentScale: 1,
    };
  }

  const contentScale = Math.min(
    maxViewportWidth / Math.max(asset.width, 1),
    maxViewportHeight / Math.max(asset.height, 1),
  );
  const contentWidth = Math.max(1, Math.round(asset.width * contentScale));
  const contentHeight = Math.max(1, Math.round(asset.height * contentScale));

  return {
    left: 14 + Math.round((maxViewportWidth - contentWidth) / 2),
    top: 52 + Math.round((maxViewportHeight - contentHeight) / 2),
    width: maxViewportWidth,
    height: maxViewportHeight,
    contentWidth,
    contentHeight,
    imageLeft: 0,
    imageTop: 0,
    contentScale,
  };
}

function mapPanelTransformForWorld(
  world: DisplayWorld,
  asset: MapRasterAsset | null,
  kind: "mini" | "big",
) {
  const mapWidth = Math.max(world.originalMapRegion?.mapWidth ?? 1, 1);
  const mapHeight = Math.max(world.originalMapRegion?.mapHeight ?? 1, 1);
  const assetWidth = Math.max(asset?.width ?? (kind === "big" ? 568 : MINI_MAP_VIEW_WIDTH), 1);
  const assetHeight = Math.max(asset?.height ?? (kind === "big" ? 380 : MINI_MAP_VIEW_HEIGHT), 1);
  const explicit = findCrystalMiniMapTransform(CRYSTAL_MINI_MAP_TRANSFORMS, {
    mapFileName: world.mapFileName,
    miniMapIndex: world.miniMapIndex,
    bigMapIndex: world.bigMapIndex,
    kind,
  });

  return explicit ?? createLinearMiniMapTransform({
    mapFileName: world.mapFileName,
    miniMapIndex: world.miniMapIndex,
    bigMapIndex: world.bigMapIndex,
    worldWidth: mapWidth,
    worldHeight: mapHeight,
    imageWidth: assetWidth,
    imageHeight: assetHeight,
  });
}

function bigMapImagePointToViewportPoint(
  imagePoint: CrystalMiniMapPoint,
  viewport: ReturnType<typeof bigMapViewport>,
  asset: MapRasterAsset | null,
) {
  if (!asset) {
    return {
      x: viewport.imageLeft + imagePoint.x,
      y: viewport.imageTop + imagePoint.y,
    };
  }

  const scaleX = viewport.contentScale;
  const scaleY = viewport.contentScale;
  return {
    x: viewport.imageLeft + imagePoint.x * scaleX,
    y: viewport.imageTop + imagePoint.y * scaleY,
  };
}

function miniMapViewportPointForWorldPoint(
  bounds: NonNullable<ReturnType<typeof miniMapBounds>>,
  point: CrystalMiniMapPoint,
) {
  if (bounds.transform && bounds.imageViewport) {
    return miniMapImagePointToViewportPoint(
      worldToMiniMapImagePoint(bounds.transform, point),
      bounds.imageViewport,
    );
  }
  return {
    x: point.x - bounds.minX,
    y: point.y - bounds.minY,
  };
}

function useMapDebugEnabled() {
  const [enabled, setEnabled] = useState(false);
  useEffect(() => {
    setEnabled(new URLSearchParams(window.location.search).get("mapDebug") === "1");
  }, []);
  return enabled;
}

function MapDebugReadout({
  className,
  world,
  asset,
  transform,
  player,
  playerImagePoint,
  mapKind,
}: {
  className: string;
  world: DisplayWorld;
  asset: MapRasterAsset | null;
  transform: CrystalMiniMapTransform | null;
  player: DisplayEntity | null;
  playerImagePoint: CrystalMiniMapPoint | null;
  mapKind: "mini" | "big";
}) {
  return (
    <div className={className}>
      <div>{mapKind} map</div>
      <div>map {world.mapFileName ?? "-"} mini {world.miniMapIndex ?? "-"} big {world.bigMapIndex ?? "-"}</div>
      <div>world {world.originalMapRegion?.mapWidth ?? "-"}x{world.originalMapRegion?.mapHeight ?? "-"}</div>
      <div>asset {asset?.width ?? "-"}x{asset?.height ?? "-"}</div>
      <div>projection {transform?.projection ?? "linear"}</div>
      <div>player {player ? `${player.x},${player.y}` : "-"}</div>
      <div>image {playerImagePoint ? `${Math.round(playerImagePoint.x)},${Math.round(playerImagePoint.y)}` : "-"}</div>
    </div>
  );
}

function miniMapRasterStyle(raster: { left: number; top: number; width: number; height: number }) {
  return {
    width: `${raster.width}px`,
    height: `${raster.height}px`,
    left: `${raster.left}px`,
    top: `${raster.top}px`,
  };
}

function miniMapTerrainColor(kind: string) {
  switch (kind) {
    case "water":
      return "#4f7ca2";
    case "road":
      return "#ac905f";
    case "stone":
      return "#8d8878";
    case "dirt":
      return "#7d5a33";
    case "grass":
    default:
      return "#4e7d3a";
  }
}

function miniMapEntityColor(kind: string) {
  switch (kind) {
    case "selfPlayer":
    case "player":
      return "#ffffff";
    case "npc":
      return "#00ff32";
    case "monster":
    default:
      return "#ff0000";
  }
}

function clampNumber(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value));
}
