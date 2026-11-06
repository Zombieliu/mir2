"use client";

import { useEffect, useRef, useState } from "react";
import { ORIGINAL_UI } from "../../lib/original-ui";
import { captureCrystalKeyBinding, clearCrystalKeyBinding, crystalKeyNameFromBrowserEvent,
  defaultCrystalKeyBindings, displayCrystalKeyBinding, type CrystalKeyBinding } from "../../lib/player-ui-preferences";
import { SpriteButton } from "./original-client-overlays";

type TranslateFn = (key: string, params?: Array<string | number>, fallback?: string) => string;
/** Retained for callers supplying explanatory groups; action identity is always Native function. */
export type HotkeyBinding = { id: string; keys: string; labelKey: string; labelFallback: string };
export type HotkeyGroup = { titleKey: string; titleFallback: string; bindings: HotkeyBinding[] };
export type HotkeyWindowProps = {
  t: TranslateFn; groups?: HotkeyGroup[]; onClose: () => void;
  bindings?: readonly CrystalKeyBinding[];
  onBindingsChange?: (bindings: CrystalKeyBinding[]) => void;
  /** Root saves atomically on Close and reports failures in notice. */
  onPersist?: (bindings: readonly CrystalKeyBinding[]) => void;
  /** Host skips gameplay dispatch while active; this listener still receives the event. */
  onCaptureChange?: (active: boolean) => void;
  /** Host held-code state covers Backquote pressed before this window opened. */
  tildePressed?: boolean;
  enforce?: boolean; onEnforceChange?: (enforce: boolean) => void;
  disabled?: boolean; notice?: string;
  /** Omit only when the host really dispatches all Native functions. */
  supportedFunctions?: readonly string[];
};
const DEFAULT_BINDINGS = defaultCrystalKeyBindings();
/** Compatibility reference uses the same Native defaults as the editable table. */
export const DEFAULT_HOTKEY_GROUPS = DEFAULT_BINDINGS.reduce<HotkeyGroup[]>((groups, binding) => {
  let group = groups.find(entry => entry.titleFallback === binding.group);
  if (!group) {
    group = { titleKey: `ui.hotkeyGroup.${binding.group}`, titleFallback: binding.group, bindings: [] };
    groups.push(group);
  }
  group.bindings.push({ id: binding.function, keys: displayCrystalKeyBinding(binding) || "Unbound",
    labelKey: `ui.hotkey.${binding.function}`, labelFallback: binding.description });
  return groups;
}, []);
export function HotkeyWindow({ t, groups, onClose, bindings = DEFAULT_BINDINGS, onBindingsChange, onPersist,
  onCaptureChange, tildePressed, enforce, onEnforceChange, disabled = false, notice, supportedFunctions }: HotkeyWindowProps) {
  const [waiting, setWaiting] = useState<string | null>(null);
  const [localEnforce, setLocalEnforce] = useState(true);
  const shownEnforce = enforce ?? localEnforce;
  const currentRef = useRef({ bindings, onBindingsChange, onCaptureChange, disabled, shownEnforce, tildePressed });
  currentRef.current = { bindings, onBindingsChange, onCaptureChange, disabled, shownEnforce, tildePressed };
  const waitingRef = useRef<string | null>(null);
  const tildeRef = useRef(false);
  const changeWaiting = (functionId: string | null) => {
    waitingRef.current = functionId; setWaiting(functionId);
    currentRef.current.onCaptureChange?.(functionId !== null);
  };
  useEffect(() => {
    const down = (event: KeyboardEvent) => {
      if (event.code === "Backquote") tildeRef.current = true;
      const functionId = waitingRef.current;
      if (!functionId) return;
      event.preventDefault(); event.stopImmediatePropagation();
      if (event.repeat || event.isComposing) return;
      const current = currentRef.current;
      if (current.disabled || !current.onBindingsChange) { changeWaiting(null); return; }
      const key = crystalKeyNameFromBrowserEvent(event);
      if (!key) return;
      const next = captureCrystalKeyBinding(current.bindings, functionId, key,
        { alt: event.altKey, ctrl: event.ctrlKey, shift: event.shiftKey, tilde: current.tildePressed ?? tildeRef.current }, current.shownEnforce);
      if (!next) return; // Modifier-only input stays in capture, as Native does.
      current.onBindingsChange(next); changeWaiting(null);
    };
    const up = (event: KeyboardEvent) => {
      if (event.code === "Backquote") tildeRef.current = false;
      if (waitingRef.current) { event.preventDefault(); event.stopImmediatePropagation(); }
    };
    const blur = () => { tildeRef.current = false; changeWaiting(null); };
    window.addEventListener("keydown", down, true); window.addEventListener("keyup", up, true); window.addEventListener("blur", blur);
    return () => { window.removeEventListener("keydown", down, true); window.removeEventListener("keyup", up, true);
      window.removeEventListener("blur", blur); waitingRef.current = null; currentRef.current.onCaptureChange?.(false); };
    // Stable listeners read the current controlled bindings and callbacks through currentRef.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  useEffect(() => { if (disabled && waitingRef.current) changeWaiting(null); }, [disabled]);
  const rows = [...bindings].sort((a, b) => a.group.localeCompare(b.group) || a.description.toLowerCase().localeCompare(b.description.toLowerCase()));
  const defaultKey = (functionId: string) => { const row = DEFAULT_BINDINGS.find(binding => binding.function === functionId); return row ? displayCrystalKeyBinding(row) : ""; };
  const canEdit = !disabled && Boolean(onBindingsChange);
  const clear = (functionId: string) => {
    if (!canEdit) return;
    const next = clearCrystalKeyBinding(bindings, functionId);
    if (next) { onBindingsChange?.(next); changeWaiting(null); }
  };
  const close = () => { changeWaiting(null); onPersist?.(bindings); onClose(); };
  const legacyLabel = (functionId: string, fallback: string) => {
    const row = groups?.flatMap(group => group.bindings).find(binding => binding.id === functionId);
    return row ? t(row.labelKey, undefined, row.labelFallback) : fallback;
  };
  return <div className="original-hotkey-window" role="dialog" aria-label={t("ui.hotkey", undefined, "Keyboard Settings")}
    data-ui-interactive="true" data-keybinding-capture={waiting ? "true" : "false"}
    style={{ position: "absolute", left: 256, top: 169, width: 512, height: 430, zIndex: 45 }}>
    <img src="/original-ui/Title/119.png" alt="" draggable={false} style={{ position: "absolute", inset: 0, width: 512, height: 430, pointerEvents: "none" }} />
    <div style={{ position: "absolute", left: 135, top: 34, width: 242, textAlign: "center", color: "#eee", fontSize: 13 }}>{t("ui.hotkey", undefined, "Keyboard Settings")}</div>
    <div style={{ position: "absolute", left: 489, top: 3 }}><SpriteButton sprite={ORIGINAL_UI.gameShop.closeButton} label={t("ui.close", undefined, "Close")} onClick={close} /></div>
    <div style={{ position: "absolute", left: 18, top: 73, width: 465, display: "flex", color: "#d8c47c", fontSize: 10 }}>
      <span style={{ width: 230 }}>{t("ui.function", undefined, "Function")}</span><span style={{ width: 110 }}>{t("ui.default", undefined, "Default")}</span><span>{t("ui.binding", undefined, "Binding")}</span>
    </div>
    <div style={{ position: "absolute", left: 18, top: 90, width: 470, height: 290, overflowY: "auto", overscrollBehavior: "contain" }}>
      {rows.map((row, index) => {
        const supported = !supportedFunctions || supportedFunctions.includes(row.function);
        const groupChanged = index === 0 || row.group !== rows[index - 1].group;
        return <div key={row.function}>
          {groupChanged ? <div style={{ height: 30, lineHeight: "30px", color: "#d8c47c", fontSize: 12 }}>{row.group}</div> : null}
          <div style={{ minHeight: 22, display: "flex", alignItems: "center", color: supported ? "#eee" : "#aaa", fontSize: 10 }}>
            <span style={{ width: 230, flexShrink: 0 }} title={supported ? row.description : t("ui.unavailable", undefined, "Unavailable in this client")}>{legacyLabel(row.function, row.description)}</span>
            <span style={{ width: 110, flexShrink: 0 }}>{defaultKey(row.function)}</span>
            <button type="button" disabled={!canEdit || !supported} aria-label={t("ui.rebind", undefined, "Rebind") + " " + row.description}
              aria-pressed={waiting === row.function} onClick={() => changeWaiting(waitingRef.current ? null : row.function)}
              style={{ width: 90, minHeight: 22, padding: "0 2px", fontSize: 10, color: "#eee", background: waiting === row.function ? "#5c4924" : "#231d15", border: "1px solid #66533a" }}>
              {waiting === row.function ? t("ui.pressKey", undefined, "Press a key…") : displayCrystalKeyBinding(row) || t("ui.unbound", undefined, "Unbound")}
            </button>
            <button type="button" disabled={!canEdit || !supported} aria-label={t("ui.clear", undefined, "Clear") + " " + row.description}
              onClick={() => clear(row.function)} style={{ width: 22, minHeight: 22, padding: 0, marginLeft: 2 }}>×</button>
          </div>
        </div>;
      })}
    </div>
    <div style={{ position: "absolute", left: 30, top: 400 }}><SpriteButton sprite={{ base: "/original-ui/Title/120.png", hover: "/original-ui/Title/121.png", pressed: "/original-ui/Title/122.png" }}
      label={t("ui.resetDefaults", undefined, "Reset defaults")} disabled={!canEdit} onClick={() => { onBindingsChange?.(defaultCrystalKeyBindings()); changeWaiting(null); }} /></div>
    <label style={{ position: "absolute", left: 105, top: 402, color: "#eee", fontSize: 10, display: "flex", alignItems: "center", gap: 4 }}>
      <input type="checkbox" checked={shownEnforce} disabled={!canEdit} onChange={event => { setLocalEnforce(event.target.checked); onEnforceChange?.(event.target.checked); }} />
      {t("ui.enforceModifiers", undefined, "Enforce modifiers")}
    </label>
    <div role="status" style={{ position: "absolute", left: 260, top: 390, width: 230, fontSize: 9, color: "#d8c47c" }}>
      {notice || (waiting ? t("ui.keyCaptureHelp", undefined, "Delete clears; Escape can be assigned.") : "")}
    </div>
  </div>;
}
