"use client";

import { useId, useState, type CSSProperties, type SyntheticEvent } from "react";

export type CombatModeControls = {
  mode: number | null;
  shortcut: string;
  enabled: boolean;
  onSelectMode: (mode: number) => void;
};

const modes = [
  { name: "和平", description: "不攻击玩家" },
  { name: "组队", description: "不攻击同队玩家" },
  { name: "行会", description: "不攻击同行会玩家" },
  { name: "敌对行会", description: "只攻击敌对行会玩家" },
  { name: "善恶", description: "只攻击红名、褐名玩家" },
  { name: "全体", description: "可攻击其他玩家" },
] as const;

const panelStyle: CSSProperties = {
  position: "absolute", left: 8, top: 8, width: 224, zIndex: 10,
  padding: 6, boxSizing: "border-box", border: "1px solid #655139",
  borderRadius: 3, background: "rgba(18, 14, 9, .88)", color: "#dcc6a0",
  font: "12px/1.4 Arial, sans-serif", textShadow: "0 1px #000", pointerEvents: "auto",
};
const buttonStyle: CSSProperties = {
  border: "1px solid #796344", borderRadius: 2, padding: "3px 6px", minHeight: 28,
  color: "#f0dcaf", background: "#302519", font: "inherit", cursor: "pointer",
};

function stopSceneEvent(event: SyntheticEvent) { event.stopPropagation(); }

export function OriginalClientCombatModeControls({
  mode, shortcut, enabled, onSelectMode, inSafeZone,
}: CombatModeControls & { inSafeZone: boolean }) {
  const [expanded, setExpanded] = useState(false);
  const optionsId = useId();
  const currentMode = Number.isInteger(mode) && mode !== null ? modes[mode] : undefined;
  const ready = enabled && Boolean(currentMode);

  return (
    <div className="original-combat-mode-controls" data-ui-interactive="true"
      style={panelStyle}
      onPointerDown={stopSceneEvent} onPointerUp={stopSceneEvent}
      onMouseDown={stopSceneEvent} onMouseUp={stopSceneEvent} onDoubleClick={stopSceneEvent}
      onClick={stopSceneEvent}
      onContextMenu={event => { event.preventDefault(); event.stopPropagation(); }}
      onKeyDown={event => {
        if (["Enter", " ", "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"].includes(event.key)) event.stopPropagation();
        if (event.key === "Escape" && expanded) { event.stopPropagation(); setExpanded(false); }
      }}
      onKeyUp={event => { if (event.key === "Enter" || event.key === " ") event.stopPropagation(); }}>
      <button type="button" data-ui-interactive="true" aria-expanded={expanded}
        aria-controls={optionsId} aria-label={`攻击模式：${currentMode?.name ?? "同步中"}`}
        disabled={!ready} onClick={() => setExpanded(value => !value)}
        style={{ ...buttonStyle, width: "100%", display: "flex", justifyContent: "space-between", alignItems: "center",
          opacity: ready ? 1 : .65, cursor: ready ? "pointer" : "default" }}>
        <span>攻击：{currentMode?.name ?? "同步中"} {expanded ? "▴" : "▾"}</span>
        {shortcut ? <span style={{ fontSize: 11, color: "#c9b793" }}>{shortcut}</span> : null}
      </button>
      <div style={{ marginTop: 4, fontSize: 11 }}>Shift + 左键点击玩家攻击</div>
      <div role="status" aria-live="polite" style={{ color: inSafeZone ? "#dbb66e" : "#bfae91", fontSize: 11 }}>
        {inSafeZone ? "安全区内禁止攻击玩家" : currentMode?.description ?? "正在同步攻击模式…"}
      </div>
      {expanded ? (
        <div id={optionsId} role="group" aria-label="选择攻击模式"
          style={{ display: "grid", gridTemplateColumns: "repeat(3, 1fr)", gap: 4, marginTop: 6 }}>
          {modes.map((entry, value) => <button key={value} type="button" data-ui-interactive="true"
            disabled={!ready} aria-pressed={mode === value} title={entry.description}
            style={{ ...buttonStyle, padding: "3px 2px", color: mode === value ? "#ffe5a0" : "#dcc6a0",
              background: mode === value ? "#564326" : "#302519", opacity: ready ? 1 : .65 }}
            onClick={() => {
              if (!ready) return;
              onSelectMode(value);
              setExpanded(false);
            }}>{entry.name}</button>)}
        </div>
      ) : null}
    </div>
  );
}
