"use client";

import { useState } from "react";

import { ORIGINAL_UI } from "../../lib/original-ui";
import { CRYSTAL_BIG_MAP_NPCS } from "../../lib/generated/crystal-npc-info-data";
import miniMapMeta from "../../public/original-ui/MMap/meta.json";
import { SpriteButton } from "./original-client-overlays";

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
  const bigMapAsset = originalBigMapAssetPath(world.bigMapIndex ?? world.miniMapIndex);
  const mapWidth = Math.max(world.originalMapRegion?.mapWidth ?? player?.x ?? 1, 1);
  const mapHeight = Math.max(world.originalMapRegion?.mapHeight ?? player?.y ?? 1, 1);
  const viewport = bigMapViewport(bigMapAsset);
  const scaleX = viewport.contentWidth / mapWidth;
  const scaleY = viewport.contentHeight / mapHeight;
  const coordinateLabel = player ? `[ ${player.x}, ${player.y} ]` : "[ 0, 0 ]";
  const npcRows = bigMapNpcRowsForWorld(world).slice(0, 18);

  return (
    <section className="big-map-dialog" aria-label={t("client.BigMapKey", ["M"], t("ui.map"))}>
      <img className="big-map-frame" src={ORIGINAL_UI.bigMap.frame} alt="" draggable={false} />
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
            style={{ width: viewport.contentWidth, height: viewport.contentHeight, left: viewport.imageLeft, top: viewport.imageTop }}
          />
        ) : (
          <div className="big-map-fallback" />
        )}
        {world.entities.map((entity) => {
          const left = viewport.imageLeft + entity.x * scaleX - 1;
          const top = viewport.imageTop + entity.y * scaleY - 1;
          return <span key={`big-map-dot-${entity.objectId}`} className={`big-map-dot ${entity.kind}`} style={{ left, top }} />;
        })}
        {player ? (
          <img
            className="big-map-user-dot"
            src={ORIGINAL_UI.bigMap.radarDot}
            alt=""
            draggable={false}
            style={{ left: viewport.imageLeft + player.x * scaleX - 6, top: viewport.imageTop + player.y * scaleY - 5 }}
          />
        ) : null}
      </div>
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
          <img className="big-map-world-image" src={ORIGINAL_UI.bigMap.worldMap} alt="" draggable={false} />
          <img className="big-map-world-clouds" src={ORIGINAL_UI.bigMap.worldClouds} alt="" draggable={false} />
          <img className="big-map-world-border" src={ORIGINAL_UI.bigMap.worldBorder} alt="" draggable={false} />
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
  const panelFrame = hasRasterMiniMap ? ORIGINAL_UI.game.miniMap : ORIGINAL_UI.game.miniMapSmall;

  return (
    <section className={`mini-map-panel ${hasRasterMiniMap ? "large" : "small"}`}>
      <img className="mini-map-bg" src={panelFrame} alt="" draggable={false} />
      <div className={`mini-map-scene-shell ${collapsed || !hasRasterMiniMap ? "hidden" : ""}`}>
        <MiniMapScene world={world} player={player} />
      </div>
      {hasRasterMiniMap ? <div className="mini-map-name">
        <span>{world.mapTitle ?? t("content.scene.starterField.title")}</span>
        {world.inSafeZone ? <>
          {" "}
          <span className="mini-map-safe-zone">{t("ui.safeZone", [], "Safe Zone")}</span>
        </> : null}
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
      <img className="mini-map-light" src={ORIGINAL_UI.game.miniMapIcons.light} alt="" draggable={false} />
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
  const miniMapAssetPath = originalMiniMapAssetPath(world.miniMapIndex);
  const bounds = miniMapBounds(world, player, miniMapAssetPath);

  if (!bounds) {
    return null;
  }

  const radarDot = {
    width: (bounds.width / MINI_MAP_VIEW_WIDTH) * 2,
    height: (bounds.height / MINI_MAP_VIEW_HEIGHT) * 2,
  };

  return (
    <div className="mini-map-scene">
      {miniMapAssetPath && bounds.raster ? (
        <img
          className="mini-map-raster"
          src={miniMapAssetPath.src}
          alt=""
          draggable={false}
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
      <svg className="mini-map-overlay" viewBox={`0 0 ${bounds.width} ${bounds.height}`} preserveAspectRatio="none">
        {world.entities.map((entity) => (
          <rect
            key={`mini-${entity.objectId}`}
            x={entity.x - bounds.minX - radarDot.width / 2}
            y={entity.y - bounds.minY - radarDot.height / 2}
            width={radarDot.width}
            height={radarDot.height}
            fill={miniMapEntityColor(entity.kind)}
          />
        ))}
      </svg>
    </div>
  );
}

function originalMapLinkIconPath(icon: number) {
  return ORIGINAL_UI.bigMap.mapLinkIcon(icon);
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
  asset: { src: string; width: number; height: number } | null,
) {
  if (asset && world.originalMapRegion) {
    const mapWidth = Math.max(world.originalMapRegion.mapWidth, 1);
    const mapHeight = Math.max(world.originalMapRegion.mapHeight, 1);
    const scaleX = asset.width / mapWidth;
    const scaleY = asset.height / mapHeight;
    const viewWidth = Math.min(120, asset.width);
    const viewHeight = Math.min(108, asset.height);
    const center = player ?? { x: mapWidth / 2, y: mapHeight / 2 };
    const rasterLeft = clampNumber(Math.round(center.x * scaleX - viewWidth / 2), 0, Math.max(asset.width - viewWidth, 0));
    const rasterTop = clampNumber(Math.round(center.y * scaleY - viewHeight / 2), 0, Math.max(asset.height - viewHeight, 0));

    return {
      minX: rasterLeft / scaleX,
      minY: rasterTop / scaleY,
      width: viewWidth / scaleX,
      height: viewHeight / scaleY,
      raster: {
        left: rasterLeft,
        top: rasterTop,
        width: viewWidth,
        height: viewHeight,
      },
    };
  }

  if (!player) {
    return { minX: 0, minY: 0, width: 48, height: 48, raster: null };
  }

  return {
    minX: player.x - 12,
    minY: player.y - 12,
    width: 24,
    height: 24,
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

function bigMapViewport(asset: { src: string; width: number; height: number } | null) {
  const contentWidth = asset ? Math.min(568, asset.width) : 568;
  const contentHeight = asset ? Math.min(380, asset.height) : 380;
  return {
    left: 14 + Math.floor((568 - contentWidth) / 2),
    top: 52 + Math.floor((380 - contentHeight) / 2),
    width: contentWidth,
    height: contentHeight,
    contentWidth,
    contentHeight,
    imageLeft: 0,
    imageTop: 0,
  };
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
      return "#ffef5b";
    case "player":
      return "#63d7ff";
    case "npc":
      return "#ff64c8";
    case "monster":
    default:
      return "#ef4444";
  }
}

function clampNumber(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value));
}
