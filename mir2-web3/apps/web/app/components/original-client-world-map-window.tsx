"use client";

import { useMemo, useState, type CSSProperties } from "react";

import { ORIGINAL_UI } from "../../lib/original-ui";
import { SpriteButton } from "./original-client-overlays";

type TranslateFn = (
  key: string,
  params?: Array<string | number>,
  fallback?: string,
) => string;

/** A point of interest plotted on the world map (a map link or teleport). */
export type WorldMapMarker = {
  key: string;
  /** Display name of the destination map. */
  title: string;
  /** Normalised position on the world image, 0..1 on each axis. */
  x: number;
  y: number;
  /** When true the viewer can teleport here directly. */
  teleport?: boolean;
};

export type WorldMapWindowProps = {
  t: TranslateFn;
  /** Current map title shown in the header. */
  currentMap?: string | null;
  /** Player marker position (0..1 each axis); omitted hides the radar dot. */
  playerPosition?: { x: number; y: number } | null;
  /** Known map links / teleport destinations. */
  markers?: WorldMapMarker[];
  /** Travel to a teleport-enabled marker. */
  onTeleport?: (key: string) => void;
  onClose: () => void;
};

const FRAME = ORIGINAL_UI.bigMap;
const MAP_WIDTH = 472;
const MAP_HEIGHT = 432;

export function WorldMapWindow({
  t,
  currentMap,
  playerPosition,
  markers,
  onTeleport,
  onClose,
}: WorldMapWindowProps) {
  const points = useMemo(() => (Array.isArray(markers) ? markers.filter((m) => m && m.key) : []), [markers]);
  const [selectedKey, setSelectedKey] = useState<string | null>(null);
  const selected = points.find((m) => m.key === selectedKey) ?? null;

  const clampedPlayer = playerPosition
    ? { x: clamp01(playerPosition.x), y: clamp01(playerPosition.y) }
    : null;

  return (
    <section
      aria-label={t("ui.worldMap", [], "World Map")}
      data-worldmap-current={currentMap ?? ""}
      data-worldmap-markers={points.length}
      style={style.window}
    >
      <img style={style.frame} src={FRAME.frame} alt="" draggable={false} />
      <div style={style.titleText}>{t("ui.worldMap", [], "World Map")}</div>
      <div style={style.subtitle}>
        {currentMap
          ? t("ui.worldMapHere", [currentMap], `You are in ${currentMap}`)
          : t("ui.worldMapHint", [], "Select a destination to view it.")}
      </div>
      <div style={style.close}>
        <SpriteButton sprite={FRAME.closeButton} label={t("ui.close", [], "Close")} onClick={onClose} />
      </div>

      <div style={style.map}>
        <img style={style.mapImage} src={FRAME.worldMap} alt="" draggable={false} />
        <img style={style.mapBorder} src={FRAME.worldBorder} alt="" draggable={false} />
        {points.map((marker) => {
          const isSelected = marker.key === selectedKey;
          return (
            <button
              key={marker.key}
              type="button"
              data-worldmap-marker={marker.key}
              aria-pressed={isSelected}
              title={marker.title}
              onClick={() => setSelectedKey(marker.key)}
              style={{
                ...style.markerDot,
                left: `${clamp01(marker.x) * 100}%`,
                top: `${clamp01(marker.y) * 100}%`,
                ...(marker.teleport ? style.markerTeleport : null),
                ...(isSelected ? style.markerSelected : null),
              }}
            >
              <span style={style.markerInner} aria-hidden="true" />
            </button>
          );
        })}
        {clampedPlayer ? (
          <img
            style={{ ...style.playerDot, left: `${clampedPlayer.x * 100}%`, top: `${clampedPlayer.y * 100}%` }}
            src={FRAME.radarDot}
            alt=""
            draggable={false}
          />
        ) : null}
      </div>

      <div style={style.sidebar}>
        <div style={style.sidebarTitle}>{t("ui.worldMapDestinations", [], "Destinations")}</div>
        <div style={style.markerList} aria-label={t("ui.worldMapDestinations", [], "Destinations")}>
          {points.length === 0 ? (
            <div style={style.empty}>{t("ui.worldMapEmpty", [], "No known destinations.")}</div>
          ) : (
            points.map((marker) => {
              const isSelected = marker.key === selectedKey;
              return (
                <button
                  key={marker.key}
                  type="button"
                  aria-pressed={isSelected}
                  onClick={() => setSelectedKey(marker.key)}
                  style={{ ...style.markerRow, ...(isSelected ? style.markerRowSelected : null) }}
                >
                  <span
                    style={{ ...style.rowDot, background: marker.teleport ? "#8be07a" : "#caa64a" }}
                    aria-hidden="true"
                  />
                  <span style={style.rowName}>{marker.title}</span>
                </button>
              );
            })
          )}
        </div>
        <button
          type="button"
          disabled={!onTeleport || !selected?.teleport}
          style={{
            ...style.teleportButton,
            ...(!onTeleport || !selected?.teleport ? style.actionButtonDisabled : null),
          }}
          onClick={() => selected && onTeleport?.(selected.key)}
        >
          {selected
            ? selected.teleport
              ? t("ui.worldMapTeleport", [selected.title], `Travel to ${selected.title}`)
              : t("ui.worldMapNoTeleport", [], "No route from here")
            : t("ui.worldMapSelect", [], "Select a destination")}
        </button>
      </div>
    </section>
  );
}

function clamp01(value: number) {
  if (!Number.isFinite(value)) return 0;
  return Math.max(0, Math.min(1, value));
}

const style: Record<string, CSSProperties> = {
  window: {
    position: "absolute",
    left: 132,
    top: 134,
    width: FRAME.width,
    height: FRAME.height,
    zIndex: 34,
    color: "#f0eee8",
    fontSize: 12,
    textShadow: "1px 1px 0 #000",
    fontFamily: "inherit",
  },
  frame: { position: "absolute", inset: 0, width: FRAME.width, height: FRAME.height, pointerEvents: "none" },
  titleText: {
    position: "absolute",
    left: 24,
    top: 12,
    fontSize: 14,
    fontWeight: 700,
    color: "#f4dcaf",
    letterSpacing: 0.5,
  },
  subtitle: { position: "absolute", left: 24, top: 32, fontSize: 11, color: "#cbb38a" },
  close: { position: "absolute", left: 724, top: 8 },
  map: {
    position: "absolute",
    left: 24,
    top: 56,
    width: MAP_WIDTH,
    height: MAP_HEIGHT,
    overflow: "hidden",
    border: "1px solid rgba(190, 157, 99, 0.4)",
    background: "#1a130a",
  },
  mapImage: { position: "absolute", inset: 0, width: "100%", height: "100%", objectFit: "cover" },
  mapBorder: { position: "absolute", inset: 0, width: "100%", height: "100%", pointerEvents: "none" },
  markerDot: {
    position: "absolute",
    width: 14,
    height: 14,
    transform: "translate(-50%, -50%)",
    border: "none",
    background: "transparent",
    padding: 0,
    cursor: "pointer",
  },
  markerInner: {
    display: "block",
    width: 8,
    height: 8,
    margin: "3px auto",
    borderRadius: "50%",
    background: "#caa64a",
    boxShadow: "0 0 0 1px rgba(0,0,0,0.7)",
  },
  markerTeleport: {},
  markerSelected: {
    outline: "2px solid rgba(248, 230, 187, 0.9)",
    borderRadius: "50%",
  },
  playerDot: {
    position: "absolute",
    width: 12,
    height: 12,
    transform: "translate(-50%, -50%)",
    pointerEvents: "none",
  },
  sidebar: {
    position: "absolute",
    left: 508,
    top: 56,
    width: 228,
    height: MAP_HEIGHT,
    display: "flex",
    flexDirection: "column",
    border: "1px solid rgba(190, 157, 99, 0.32)",
    background: "rgba(11, 8, 5, 0.45)",
    padding: 8,
  },
  sidebarTitle: {
    fontSize: 12,
    fontWeight: 700,
    color: "#f4dcaf",
    borderBottom: "1px solid rgba(190, 157, 99, 0.24)",
    paddingBottom: 5,
    marginBottom: 6,
  },
  markerList: { flex: 1, overflowY: "auto", display: "flex", flexDirection: "column", gap: 2 },
  empty: { color: "#cbb38a", padding: "8px 4px", fontSize: 11 },
  markerRow: {
    display: "flex",
    alignItems: "center",
    gap: 7,
    width: "100%",
    height: 22,
    padding: "0 7px",
    border: "1px solid transparent",
    background: "rgba(20, 13, 7, 0.4)",
    color: "#e3d3af",
    textAlign: "left",
    cursor: "pointer",
  },
  markerRowSelected: { background: "rgba(95, 53, 24, 0.5)", borderColor: "rgba(214, 180, 110, 0.7)" },
  rowDot: { width: 8, height: 8, borderRadius: "50%", flex: "0 0 auto" },
  rowName: { flex: 1, minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" },
  teleportButton: {
    marginTop: 8,
    border: "1px solid rgba(190, 157, 99, 0.56)",
    background: "linear-gradient(180deg, rgba(95, 53, 24, 0.95), rgba(45, 23, 12, 0.95))",
    color: "#f4dcaf",
    padding: "6px 0",
    fontSize: 11,
    cursor: "pointer",
  },
  actionButtonDisabled: { opacity: 0.45, cursor: "default" },
};
