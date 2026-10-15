"use client";

import { memo, useEffect, useRef, useState, type MouseEvent } from "react";

import {
  mapRouteSourceMatchesWorld,
  nativeBigMapImagePointToTile,
  nativeBigMapImageRect,
  nativeMiniMapCrop,
  nativeMiniMapViewportPointToTile,
  sameMapRouteSource,
  type MapImageRouteIntent,
  type MapImageRouteSource,
} from "../../lib/client-map-input";

import {
  crystalMiniMapRadarColor,
  createLinearMiniMapTransform,
  findCrystalMiniMapTransform,
  miniMapImagePointToViewportPoint,
  worldToCrystalMiniMapRadarPoint,
  worldToMiniMapImagePoint,
  type CrystalMiniMapPoint,
  type CrystalMiniMapTransform,
} from "../../lib/crystal-minimap-transform";
import { CRYSTAL_MINI_MAP_TRANSFORMS } from "../../lib/generated/crystal-minimap-transforms";
import { ORIGINAL_UI } from "../../lib/original-ui";
import { CRYSTAL_BIG_MAP_NPCS } from "../../lib/generated/crystal-npc-info-data";
import {
  localizeCrystalEntityName,
  localizeCrystalMapTitle,
} from "../../lib/crystal-content-localization";
import miniMapMeta from "../../public/original-ui/MMap/meta.json";
import { SpriteButton } from "./original-client-overlays";
import { CrystalGdiTextImage, findCrystalGdiTextAsset } from "./crystal-gdi-text";
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
  ownerName?: string;
  ai?: number;
  x: number;
  y: number;
  dead?: boolean;
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
  lightSetting?: number | null;
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

type LoadedMapRaster = Readonly<{ src: string; width: number; height: number }>;

function loadedRasterFor(asset: MapRasterAsset | null, loaded: LoadedMapRaster | null): MapRasterAsset | null {
  return asset && loaded && loaded.src === asset.src && loaded.width > 0 && loaded.height > 0
    ? { src: asset.src, width: loaded.width, height: loaded.height } : asset;
}

function consumeMapImageClick<T extends HTMLElement>(event: MouseEvent<T>) {
  event.preventDefault();
  event.stopPropagation();
}

export function BigMapDialog({
  t,
  world,
  player,
  onClose,
  routeSource = null,
  onImageRoute,
  onImageRoutePress,
}: {
  t: TranslateFn;
  world: DisplayWorld;
  player: DisplayEntity | null;
  onClose: () => void;
  routeSource?: MapImageRouteSource | null;
  onImageRoute?: (intent: MapImageRouteIntent) => void;
  onImageRoutePress?: (source: MapImageRouteSource | null) => void;
}) {
  const [showWorldMap, setShowWorldMap] = useState(false);
  const [search, setSearch] = useState("");
  const [loadedRaster, setLoadedRaster] = useState<LoadedMapRaster | null>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const mapDebug = useMapDebugEnabled();
  const routeAsset = originalBigMapAssetPath(world.bigMapIndex);
  const sourceAsset = routeAsset ?? originalMiniMapAssetPath(world.miniMapIndex);
  const bigMapAsset = loadedRasterFor(sourceAsset, loadedRaster);
  const viewport = bigMapViewport(bigMapAsset);
  const transform = mapPanelTransformForWorld(world, bigMapAsset, "big");
  const playerImagePoint = player ? nativeMapTileImagePoint(world, bigMapAsset, "big", player) : null;
  const playerViewportPoint = playerImagePoint
    ? bigMapImagePointToViewportPoint(playerImagePoint, viewport, bigMapAsset)
    : null;
  const coordinateLabel = player ? `[ ${player.x}, ${player.y} ]` : "[ 0, 0 ]";
  const searchQuery = search.trim().toLowerCase();
  const npcRows = bigMapNpcRowsForWorld(world)
    .filter((entity) => !searchQuery || entity.name.toLowerCase().includes(searchQuery))
    .slice(0, 18);
  const mapDebugNpcRows = mapDebug ? bigMapNpcRowsForWorld(world) : [];
  const routeSourceValid = !showWorldMap && Boolean(routeAsset && bigMapAsset?.src === routeAsset.src
    && loadedRaster?.src === routeAsset.src
    && loadedRaster.width === routeAsset.width && loadedRaster.height === routeAsset.height
    && mapRouteSourceMatchesWorld(routeSource, world, player?.objectId, "big"));
  const routeBigMapClick = (event: MouseEvent<HTMLDivElement>) => {
    consumeMapImageClick(event);
    if (showWorldMap || !bigMapAsset || typeof document === "undefined"
      || document.visibilityState !== "visible" || !document.hasFocus()) return;
    const rect = event.currentTarget.getBoundingClientRect();
    if (rect.width <= 0 || rect.height <= 0) return;
    const pointX = (event.clientX - rect.left) * viewport.width / rect.width - viewport.imageLeft;
    const pointY = (event.clientY - rect.top) * viewport.height / rect.height - viewport.imageTop;
    if (!Number.isFinite(pointX) || !Number.isFinite(pointY) || pointX < 0 || pointY < 0
      || pointX >= viewport.contentWidth || pointY >= viewport.contentHeight) return;
    onImageRoutePress?.(routeSource);
    if (!routeSourceValid || !routeSource || !onImageRoute || !routeAsset || !loadedRaster) return;
    const tile = nativeBigMapImagePointToTile(pointX, pointY,
      routeSource.mapWidth, routeSource.mapHeight, loadedRaster.width, loadedRaster.height);
    if (!tile) return;
    onImageRoute({ source: routeSource, kind: "big", ...tile,
      imageSrc: routeAsset.src, imageWidth: loadedRaster.width, imageHeight: loadedRaster.height });
  };

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
      <div className="big-map-title">
        {localizeCrystalMapTitle(world.mapTitle, t) ?? world.mapFileName ?? ""}
      </div>
      <div className="big-map-close"><SpriteButton sprite={ORIGINAL_UI.bigMap.closeButton} label={t("ui.close")} onClick={onClose} /></div>
      <div className="big-map-scroll up"><SpriteButton sprite={ORIGINAL_UI.bigMap.upButton} label={t("ui.up", [], "Up")} disabled /></div>
      <div className="big-map-scroll thumb"><SpriteButton sprite={ORIGINAL_UI.bigMap.positionBar} label={t("ui.scroll", [], "Scroll")} disabled /></div>
      <div className="big-map-scroll down"><SpriteButton sprite={ORIGINAL_UI.bigMap.downButton} label={t("ui.down", [], "Down")} disabled /></div>
      <div className="big-map-viewport" style={{ left: viewport.left, top: viewport.top, width: viewport.width, height: viewport.height }}
        onClick={routeBigMapClick} onContextMenu={routeBigMapClick}>
        {bigMapAsset ? (
          <img
            className="big-map-raster"
            src={bigMapAsset.src}
            alt=""
            draggable={false}
            data-mir2-original-src={bigMapAsset.src}
            onError={(event) => {
              handleSceneAssetImageError(event);
              setLoadedRaster((current) => current?.src === sourceAsset?.src ? null : current);
            }}
            onLoad={(event) => {
              handleSceneAssetImageLoad(event);
              const image = event.currentTarget;
               const src = image.dataset.mir2OriginalSrc;
               if (src && src === sourceAsset?.src
                && image.naturalWidth > 0 && image.naturalHeight > 0) {
                 setLoadedRaster({ src, width: image.naturalWidth, height: image.naturalHeight });
              }
            }}
            style={{ width: viewport.contentWidth, height: viewport.contentHeight, left: viewport.imageLeft, top: viewport.imageTop }}
          />
        ) : (
          <div className="big-map-fallback" />
        )}
        {world.entities.map((entity) => {
          const point = bigMapImagePointToViewportPoint(
            nativeMapTileImagePoint(world, bigMapAsset, "big", entity),
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
            disabled
            aria-label={bigMapNpcDisplayName(localizeCrystalEntityName(entity.name, t))}
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
            <span className="big-map-npc-name">
              {bigMapNpcDisplayName(localizeCrystalEntityName(entity.name, t))}
            </span>
          </button>
        ))}
      </div>
      <div className="big-map-world-button"><SpriteButton sprite={ORIGINAL_UI.bigMap.worldButton} label={t("ui.world", [], "World")} onClick={() => setShowWorldMap(true)} /></div>
      <div className="big-map-my-location-button"><SpriteButton sprite={ORIGINAL_UI.bigMap.myLocationButton} label={t("ui.myLocation", [], "My Location")} onClick={() => setShowWorldMap(false)} /></div>
      <div className="big-map-teleport-button disabled"><SpriteButton sprite={ORIGINAL_UI.bigMap.teleportButton} label={t("ui.teleport", [], "Teleport")} disabled active /></div>
      <div className="big-map-search-button"><SpriteButton sprite={ORIGINAL_UI.bigMap.searchButton} label={t("ui.search", [], "Search")} onClick={() => searchRef.current?.focus()} /></div>
      <input
        ref={searchRef}
        className="big-map-search-input"
        aria-label={t("ui.search", [], "Search")}
        value={search}
        onChange={(event) => setSearch(event.target.value)}
        spellCheck={false}
      />
      {showWorldMap ? (
        <div className="big-map-world-overlay" onClick={consumeMapImageClick} onContextMenu={consumeMapImageClick}>
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
  showMailAction?: boolean;
  routeSource?: MapImageRouteSource | null;
  onImageRoute?: (intent: MapImageRouteIntent) => void;
  onImageRoutePress?: (source: MapImageRouteSource | null) => void;
};

export function MiniMapPanel({
  t,
  world,
  player,
  showMailPanel,
  showBigMap,
  onToggleMail,
  onToggleBigMap,
  showMailAction = true,
  routeSource = null,
  onImageRoute,
  onImageRoutePress,
}: MiniMapPanelProps) {
  const [collapsed, setCollapsed] = useState(false);
  const [loadedRaster, setLoadedRaster] = useState<LoadedMapRaster | null>(null);
  const miniMapAsset = originalMiniMapAssetPath(world.miniMapIndex);
  const hasRasterMiniMap = Boolean(miniMapAsset);
  const smallMode = collapsed || !hasRasterMiniMap;
  const panelFrame = smallMode ? ORIGINAL_UI.game.miniMapSmall : ORIGINAL_UI.game.miniMap;
  const mapTitle = crystalMiniMapTitle(world.mapTitle, t);
  const lightIcon = crystalMiniMapLightIcon(world.lightSetting);
  const coordinateText = player ? `${player.x}, ${player.y}` : "0, 0";
  const coordinateGdiText = findCrystalGdiTextAsset({
    text: coordinateText,
    foreground: "#ffffff",
    outline: true,
    width: 55,
    height: 15,
  });

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
        {/* Unmount (return null) when collapsed/small instead of CSS-hiding: a hidden scene still
            reconciles one <rect> per entity every flush. Skipping the mount removes that cost. */}
        {smallMode ? null : <MiniMapScene world={world} player={player} loadedRaster={loadedRaster}
          routeSource={routeSource} onImageRoute={onImageRoute}
          onImageRoutePress={onImageRoutePress}
          onRasterLoad={(src, width, height) => setLoadedRaster({ src, width, height })}
          onRasterError={(src) => setLoadedRaster((current) => current?.src === src ? null : current)} />}
      </div>
      {!smallMode ? <div className="mini-map-name">
        <span>{mapTitle}</span>
      </div> : null}
      <div className="mini-map-coords">
        {coordinateGdiText ? (
          <CrystalGdiTextImage asset={coordinateGdiText} accessibleText={coordinateText} />
        ) : coordinateText}
      </div>
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
        src={lightIcon}
        alt=""
        draggable={false}
        data-mir2-original-src={lightIcon}
        onError={handleSceneAssetImageError}
        onLoad={handleSceneAssetImageLoad}
      />
    </section>
  );
}

type MiniMapSceneProps = {
  world: DisplayWorld;
  player: DisplayEntity | null;
  loadedRaster: LoadedMapRaster | null;
  routeSource: MapImageRouteSource | null;
  onImageRoute?: (intent: MapImageRouteIntent) => void;
  onImageRoutePress?: (source: MapImageRouteSource | null) => void;
  onRasterLoad: (src: string, width: number, height: number) => void;
  onRasterError: (src: string) => void;
};

// Re-render only when an input the scene actually reads changes: the radar dots key on
// `world.entities`, and `bounds` derives from `world.miniMapIndex` / `world.originalMapRegion` /
// `player`; the fallback SVG keys on `world.terrainPatches`; the transform + debug NPC rows key on
// `world.mapFileName` / `world.bigMapIndex`. Everything else on `world` (combat fields, the 30Hz
// motion tick, etc.) is irrelevant here, so a fresh `world` identity alone must NOT re-render.
function areMiniMapScenePropsEqual(prev: MiniMapSceneProps, next: MiniMapSceneProps): boolean {
  if (prev.player !== next.player || prev.loadedRaster !== next.loadedRaster
    || !sameMapRouteSource(prev.routeSource, next.routeSource)
    || prev.onImageRoute !== next.onImageRoute || prev.onImageRoutePress !== next.onImageRoutePress || prev.onRasterLoad !== next.onRasterLoad
    || prev.onRasterError !== next.onRasterError) return false;
  const a = prev.world;
  const b = next.world;
  return (
    a.entities === b.entities &&
    a.terrainPatches === b.terrainPatches &&
    a.originalMapRegion === b.originalMapRegion &&
    a.miniMapIndex === b.miniMapIndex &&
    a.bigMapIndex === b.bigMapIndex &&
    a.mapFileName === b.mapFileName
  );
}

const MiniMapScene = memo(function MiniMapScene({ world, player, loadedRaster, routeSource,
  onImageRoute, onImageRoutePress, onRasterLoad, onRasterError }: MiniMapSceneProps) {
  const mapDebug = useMapDebugEnabled();
  const miniMapAssetPath = originalMiniMapAssetPath(world.miniMapIndex);
  const miniMapAsset = loadedRasterFor(miniMapAssetPath, loadedRaster);
  const bounds = miniMapBounds(world, player, miniMapAsset);

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
  const routeSourceValid = Boolean(miniMapAssetPath && miniMapAsset && loadedRaster
    && loadedRaster.src === miniMapAssetPath.src && loadedRaster.width === miniMapAssetPath.width
    && loadedRaster.height === miniMapAssetPath.height
    && mapRouteSourceMatchesWorld(routeSource, world, player?.objectId, "mini"));
  const routeMiniMapClick = (event: MouseEvent<HTMLDivElement>) => {
    consumeMapImageClick(event);
    if (document.visibilityState === "visible" && document.hasFocus()) onImageRoutePress?.(routeSource);
    if (!routeSourceValid || !routeSource || !onImageRoute || !miniMapAssetPath || !loadedRaster
      || typeof document === "undefined" || !document.hasFocus()) return;
    const rect = event.currentTarget.getBoundingClientRect();
    if (rect.width <= 0 || rect.height <= 0) return;
    const viewportX = (event.clientX - rect.left) * MINI_MAP_VIEW_WIDTH / rect.width;
    const viewportY = (event.clientY - rect.top) * MINI_MAP_VIEW_HEIGHT / rect.height;
    const imageViewport = bounds.imageViewport;
    if (!imageViewport) return;
    const crop = { left: imageViewport.imageLeft, top: imageViewport.imageTop,
      width: imageViewport.width, height: imageViewport.height };
    const tile = nativeMiniMapViewportPointToTile(viewportX, viewportY, crop,
      routeSource.mapWidth, routeSource.mapHeight, loadedRaster.width, loadedRaster.height);
    if (!tile) return;
    onImageRoute({ source: routeSource, kind: "mini", ...tile,
      imageSrc: miniMapAssetPath.src, imageWidth: loadedRaster.width, imageHeight: loadedRaster.height });
  };

  return (
    <div className="mini-map-scene" onClick={routeMiniMapClick} onContextMenu={routeMiniMapClick}>
      {miniMapAssetPath && bounds.raster ? (
        <img
          className="mini-map-raster"
          src={miniMapAssetPath.src}
          alt=""
          draggable={false}
          data-mir2-original-src={miniMapAssetPath.src}
          onError={(event) => { handleSceneAssetImageError(event); onRasterError(miniMapAssetPath.src); }}
          onLoad={(event) => {
            handleSceneAssetImageLoad(event);
            const image = event.currentTarget;
            if (image.dataset.mir2OriginalSrc === miniMapAssetPath.src
              && image.naturalWidth > 0 && image.naturalHeight > 0) {
              onRasterLoad(miniMapAssetPath.src, image.naturalWidth, image.naturalHeight);
            }
          }}
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
        {world.entities.filter(isMiniMapRadarEntity).map((entity) => {
          const point = miniMapViewportPointForWorldPoint(bounds, entity);
          const rect = miniMapRadarRect(point, radarDot, rasterMode);
          return (
            <rect
              key={`mini-${entity.objectId}`}
              x={rect.x}
              y={rect.y}
              width={radarDot.width}
              height={radarDot.height}
              fill={miniMapEntityColor(entity, player)}
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
}, areMiniMapScenePropsEqual);

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
  return localizeCrystalMapTitle(normalized[0] ?? fallback, t) ?? fallback;
}

function crystalMiniMapLightIcon(lightSetting: number | null | undefined) {
  switch (lightSetting) {
    case 1:
      return ORIGINAL_UI.game.miniMapIcons.lightDawn;
    case 3:
      return ORIGINAL_UI.game.miniMapIcons.lightEvening;
    case 4:
      return ORIGINAL_UI.game.miniMapIcons.lightNight;
    case 0:
    case 2:
    default:
      return ORIGINAL_UI.game.miniMapIcons.light;
  }
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
    const crop = nativeMiniMapCrop(Math.trunc(player?.x ?? mapWidth / 2), Math.trunc(player?.y ?? mapHeight / 2),
      mapWidth, mapHeight, asset.width, asset.height);
    if (!crop) return null;
    const scaleX = MINI_MAP_VIEW_WIDTH / crop.width;
    const scaleY = MINI_MAP_VIEW_HEIGHT / crop.height;

    return {
      minX: player ? player.x - 12 : mapWidth / 2 - 12,
      minY: player ? player.y - 12 : mapHeight / 2 - 12,
      width: 24,
      height: 24,
      mapWidth,
      mapHeight,
      imageWidth: asset.width,
      imageHeight: asset.height,
      imageViewport: {
        imageLeft: crop.left,
        imageTop: crop.top,
        width: crop.width,
        height: crop.height,
      },
      transform,
      raster: {
        left: -crop.left * scaleX,
        top: -crop.top * scaleY,
        width: asset.width * scaleX,
        height: asset.height * scaleY,
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
  const rect = nativeBigMapImageRect(asset.width, asset.height);
  if (!rect) {
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

  return {
    left: 14,
    top: 52,
    width: maxViewportWidth,
    height: maxViewportHeight,
    contentWidth: rect.width,
    contentHeight: rect.height,
    imageLeft: rect.left - 14,
    imageTop: rect.top - 52,
    contentScale: 1,
  };
}

function nativeMapTileImagePoint(
  world: DisplayWorld,
  asset: MapRasterAsset | null,
  kind: "mini" | "big",
  tile: Pick<DisplayEntity, "x" | "y">,
): CrystalMiniMapPoint {
  const mapWidth = world.originalMapRegion?.mapWidth ?? 0;
  const mapHeight = world.originalMapRegion?.mapHeight ?? 0;
  if (!asset || mapWidth <= 0 || mapHeight <= 0) return { x: 0, y: 0 };
  if (kind === "big") {
    const rect = nativeBigMapImageRect(asset.width, asset.height);
    if (!rect) return { x: 0, y: 0 };
    const fractionX = Math.max(0, Math.min(1, Math.fround(Math.fround(tile.x) / Math.fround(mapWidth))));
    const fractionY = Math.max(0, Math.min(1, Math.fround(Math.fround(tile.y) / Math.fround(mapHeight))));
    return {
      x: Math.fround(fractionX * Math.fround(rect.width)),
      y: Math.fround(fractionY * Math.fround(rect.height)),
    };
  }
  return {
    x: Math.fround(Math.fround(Math.fround(tile.x) * Math.fround(asset.width)) / Math.fround(mapWidth)),
    y: Math.fround(Math.fround(Math.fround(tile.y) * Math.fround(asset.height)) / Math.fround(mapHeight)),
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
  if (bounds.imageViewport && "mapWidth" in bounds && "mapHeight" in bounds
    && "imageWidth" in bounds && "imageHeight" in bounds) {
    const sourceX = Math.fround(Math.fround(Math.fround(point.x) * Math.fround(bounds.imageWidth)) / Math.fround(bounds.mapWidth));
    const sourceY = Math.fround(Math.fround(Math.fround(point.y) * Math.fround(bounds.imageHeight)) / Math.fround(bounds.mapHeight));
    return {
      x: Math.fround(Math.fround(Math.fround(sourceX - Math.fround(bounds.imageViewport.imageLeft))
        * MINI_MAP_VIEW_WIDTH) / Math.fround(bounds.imageViewport.width)),
      y: Math.fround(Math.fround(Math.fround(sourceY - Math.fround(bounds.imageViewport.imageTop))
        * MINI_MAP_VIEW_HEIGHT) / Math.fround(bounds.imageViewport.height)),
    };
  }
  return {
    x: point.x - bounds.minX,
    y: point.y - bounds.minY,
  };
}

function isMiniMapRadarEntity(entity: DisplayEntity) {
  return !entity.dead;
}

function miniMapRadarRect(
  point: CrystalMiniMapPoint,
  radarDot: { width: number; height: number },
  rasterMode: boolean,
) {
  if (rasterMode) {
    return { x: point.x - 0.5, y: point.y - 0.5 };
  }
  return {
    x: point.x - radarDot.width / 2,
    y: point.y - radarDot.height / 2,
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

function miniMapEntityColor(entity: DisplayEntity, player: DisplayEntity | null) {
  let ownedByPlayer = false;
  if (player && entity.objectId !== player.objectId) {
    const ownerName = entity.ownerName?.trim();
    if ((ownerName && ownerName === player.name) || entity.name.endsWith(`(${player.name})`)) {
      ownedByPlayer = true;
    }
  }
  return crystalMiniMapRadarColor({ kind: entity.kind, ai: entity.ai, ownedByPlayer });
}

function clampNumber(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value));
}
