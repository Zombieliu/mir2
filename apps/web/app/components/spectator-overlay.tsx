"use client";

export type SpectatorTarget = {
  objectId: number;
  name: string;
  hp: number | null;
  maxHp: number | null;
  x: number;
  y: number;
};

export type SpectatorMatch = {
  mapFileName: string;
  mapTitle: string;
  recordingId: string;
  latestSequence: number;
  latestCapturedAtMs: number;
  playerCount: number;
  entityCount: number;
};

export type SpectatorStatus = {
  readOnly: true;
  directorAuthorized: boolean;
  director: boolean;
  delayMs: number;
  map: string;
  target: string | null;
  camera: { x: number; y: number } | null;
  matches: SpectatorMatch[];
  targets: SpectatorTarget[];
  events: Array<{
    kind: string;
    atMs: number;
    objectId: number | null;
    name: string | null;
    payload: Record<string, unknown>;
  }>;
  recordingId: string | null;
  sequence: number | null;
  capturedAtMs: number | null;
  replay: {
    active: boolean;
    playing: boolean;
    speed: number;
    startAtMs: number | null;
    endAtMs: number | null;
    currentAtMs: number | null;
  };
};

type SpectatorOverlayProps = {
  status: SpectatorStatus | null;
  connectionState: string;
  onControl: (command: Record<string, unknown>) => void;
};

const panelStyle = {
  border: "1px solid rgba(105, 226, 255, 0.34)",
  background: "linear-gradient(160deg, rgba(5, 14, 27, 0.96), rgba(10, 18, 35, 0.92))",
  boxShadow: "0 18px 60px rgba(0, 0, 0, 0.5), inset 0 1px rgba(255, 255, 255, 0.04)",
  color: "#e9f8ff",
} as const;

const controlStyle = {
  minHeight: 30,
  border: "1px solid rgba(120, 210, 255, 0.25)",
  borderRadius: 6,
  background: "rgba(16, 37, 62, 0.86)",
  color: "#dff7ff",
  padding: "5px 9px",
  font: "inherit",
} as const;

function formatClock(value: number | null) {
  if (!value) return "--:--:--";
  return new Date(value).toLocaleTimeString([], { hour12: false });
}

export function SpectatorOverlay({ status, connectionState, onControl }: SpectatorOverlayProps) {
  const replay = status?.replay;
  const start = replay?.startAtMs ?? 0;
  const end = replay?.endAtMs ?? start;
  const cursor = replay?.currentAtMs ?? start;
  const camera = status?.camera;

  return (
    <aside
      data-testid="spectator-overlay"
      aria-label="观战控制台"
      style={{
        position: "fixed",
        zIndex: 3500,
        inset: "18px 18px auto auto",
        width: "min(380px, calc(100vw - 36px))",
        maxHeight: "calc(100vh - 36px)",
        overflow: "auto",
        pointerEvents: "auto",
        borderRadius: 12,
        padding: 16,
        fontFamily: '"Inter", "PingFang SC", "Microsoft YaHei", sans-serif',
        fontSize: 12,
        ...panelStyle,
      }}
    >
      <header style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
        <div>
          <div style={{ color: "#6ee7ff", fontSize: 10, letterSpacing: "0.22em", textTransform: "uppercase" }}>
            Dubhe Live Observer
          </div>
          <strong style={{ display: "block", marginTop: 4, fontSize: 18 }}>世界观战台</strong>
        </div>
        <span
          data-testid="spectator-read-only"
          style={{
            borderRadius: 999,
            padding: "6px 9px",
            background: status ? "rgba(44, 210, 156, 0.14)" : "rgba(255, 189, 89, 0.12)",
            color: status ? "#6ff0bd" : "#ffc56f",
          }}
        >
          {status ? "● 只读安全" : `● ${connectionState}`}
        </span>
      </header>

      <div
        style={{
          display: "grid",
          gridTemplateColumns: "repeat(3, minmax(0, 1fr))",
          gap: 8,
          marginTop: 14,
        }}
      >
        {[
          ["延迟", status ? `${Math.round(status.delayMs / 1000)}s` : "--"],
          ["地图", status?.map ?? "--"],
          ["时间", formatClock(status?.capturedAtMs ?? null)],
        ].map(([label, value]) => (
          <div key={label} style={{ borderRadius: 8, padding: "9px 10px", background: "rgba(255,255,255,0.045)" }}>
            <span style={{ display: "block", color: "#71869d", fontSize: 10 }}>{label}</span>
            <strong style={{ display: "block", overflow: "hidden", textOverflow: "ellipsis", marginTop: 3 }}>
              {value}
            </strong>
          </div>
        ))}
      </div>

      <label style={{ display: "grid", gap: 5, marginTop: 14, color: "#89a0b7" }}>
        赛事 / 地图
        <select
          data-testid="spectator-map"
          value={status?.map ?? ""}
          disabled={!status}
          onChange={(event) => onControl({ type: "map", map: event.target.value })}
          style={controlStyle}
        >
          {!status?.matches.some((match) => match.mapFileName === status.map) ? (
            <option value={status?.map ?? ""}>{status?.map ?? "等待赛事"}</option>
          ) : null}
          {status?.matches.map((match) => (
            <option key={match.mapFileName} value={match.mapFileName}>
              {match.mapTitle} · {match.playerCount} 玩家
            </option>
          ))}
        </select>
      </label>

      <label style={{ display: "grid", gap: 5, marginTop: 10, color: "#89a0b7" }}>
        跟随玩家
        <select
          data-testid="spectator-target"
          value={status?.target ?? ""}
          disabled={!status}
          onChange={(event) => onControl({ type: "follow", target: event.target.value || null })}
          style={controlStyle}
        >
          <option value="">自动选择</option>
          {status?.targets.map((target) => (
            <option key={target.objectId} value={target.name}>
              {target.name} · HP {target.hp ?? "?"}/{target.maxHp ?? "?"}
            </option>
          ))}
        </select>
      </label>

      {status?.directorAuthorized ? (
        <section style={{ marginTop: 14, borderTop: "1px solid rgba(255,255,255,0.08)", paddingTop: 12 }}>
          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
            <strong>导播镜头</strong>
            <button
              type="button"
              data-testid="spectator-director"
              onClick={() => onControl({ type: "director", enabled: !status.director })}
              style={{ ...controlStyle, color: status.director ? "#62edc1" : "#dff7ff" }}
            >
              {status.director ? "自动导播：开" : "开启自动导播"}
            </button>
          </div>
          <div style={{ display: "grid", gridTemplateColumns: "repeat(3, 34px)", gap: 4, justifyContent: "center", marginTop: 10 }}>
            <span />
            <button type="button" aria-label="镜头向上" style={controlStyle} onClick={() => onControl({ type: "camera", x: camera?.x ?? 330, y: (camera?.y ?? 270) - 4 })}>↑</button>
            <span />
            <button type="button" aria-label="镜头向左" style={controlStyle} onClick={() => onControl({ type: "camera", x: (camera?.x ?? 330) - 4, y: camera?.y ?? 270 })}>←</button>
            <button type="button" aria-label="重置镜头" style={controlStyle} onClick={() => onControl({ type: "cameraClear" })}>◎</button>
            <button type="button" aria-label="镜头向右" style={controlStyle} onClick={() => onControl({ type: "camera", x: (camera?.x ?? 330) + 4, y: camera?.y ?? 270 })}>→</button>
            <span />
            <button type="button" aria-label="镜头向下" style={controlStyle} onClick={() => onControl({ type: "camera", x: camera?.x ?? 330, y: (camera?.y ?? 270) + 4 })}>↓</button>
            <span />
          </div>
        </section>
      ) : null}

      {replay?.active ? (
        <section data-testid="spectator-replay" style={{ marginTop: 14, borderTop: "1px solid rgba(255,255,255,0.08)", paddingTop: 12 }}>
          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
            <button
              type="button"
              onClick={() => onControl({ type: replay.playing ? "replayPause" : "replayPlay" })}
              style={controlStyle}
            >
              {replay.playing ? "暂停" : "播放"}
            </button>
            <select
              aria-label="回放速度"
              value={replay.speed}
              onChange={(event) => onControl({ type: "replaySpeed", speed: Number(event.target.value) })}
              style={controlStyle}
            >
              {[0.25, 0.5, 1, 2, 4, 8].map((speed) => <option key={speed} value={speed}>{speed}×</option>)}
            </select>
            <span style={{ marginLeft: "auto", color: "#7f94aa" }}>{formatClock(cursor)}</span>
          </div>
          <input
            aria-label="回放时间轴"
            type="range"
            min={start}
            max={Math.max(start + 1, end)}
            value={Math.min(Math.max(cursor, start), Math.max(start + 1, end))}
            onChange={(event) => onControl({ type: "replaySeek", capturedAtMs: Number(event.target.value) })}
            style={{ width: "100%", marginTop: 10, accentColor: "#5bddf2" }}
          />
        </section>
      ) : null}

      {status?.events?.length ? (
        <section style={{ marginTop: 14, borderTop: "1px solid rgba(255,255,255,0.08)", paddingTop: 12 }}>
          <strong>赛事事件</strong>
          <div data-testid="spectator-events" style={{ display: "grid", gap: 5, marginTop: 8 }}>
            {status.events.slice(-5).reverse().map((event, index) => (
              <div
                key={`${event.atMs}-${event.objectId ?? "world"}-${event.kind}-${index}`}
                style={{
                  display: "grid",
                  gridTemplateColumns: "62px 1fr auto",
                  gap: 8,
                  color: "#a9bfd0",
                  padding: "5px 7px",
                  borderRadius: 5,
                  background: "rgba(255,255,255,0.035)",
                }}
              >
                <span style={{ color: event.kind === "death" ? "#ff8585" : "#63dff4" }}>{event.kind}</span>
                <span style={{ overflow: "hidden", textOverflow: "ellipsis" }}>{event.name ?? `#${event.objectId}`}</span>
                <time style={{ color: "#637b91" }}>{formatClock(event.atMs)}</time>
              </div>
            ))}
          </div>
        </section>
      ) : null}

      <footer style={{ marginTop: 12, color: "#668096", lineHeight: 1.5 }}>
        观众连接与玩家 Session 完全隔离，无法发送移动、战斗、交易或聊天指令。
      </footer>
    </aside>
  );
}
