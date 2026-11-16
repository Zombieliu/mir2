"use client";

import { useEffect, useId, useRef, useState, type CSSProperties, type ReactNode } from "react";
import { R16_GUILD_LAYOUT as G, type R16GuildRect } from "../../lib/r16-guild-layout";
import { SpriteButton } from "./original-client-overlays";

export const guildRect = (rect: R16GuildRect): CSSProperties => ({ position: "absolute", ...rect });
type Art = Readonly<{ normal: string; hover: string; active: string }>;

/** Keep source art and its hit box; translated captions cover only the original ink. */
export function GuildSourceButton({ art, rect, label, english, onClick, disabled, dataAction }: {
  art: Art; rect: R16GuildRect; label: string; english?: string; onClick: () => void;
  disabled?: boolean; dataAction?: string;
}) {
  return <div style={guildRect(rect)} data-guild-action={dataAction}>
    <SpriteButton sprite={{ base: art.normal, hover: art.hover, pressed: art.active }}
      label={label} disabled={disabled} onClick={onClick} />
    {english && label !== english ? <span aria-hidden="true" style={{ position: "absolute", inset: "3px 3px 4px",
      pointerEvents: "none", display: "grid", placeItems: "center", background: "#211b10", color: "#f0eee8",
      fontSize: 10, lineHeight: "12px", overflow: "hidden", whiteSpace: "nowrap" }}>{label}</span> : null}
  </div>;
}

export type GuildChoice = Readonly<{ index: number; name: string; create?: boolean; disabled?: boolean }>;

/** The R16 five-row list is local presentation. Only a real rank index reaches the host. */
export function GuildDropdown({ rect, choices, selected, label, onSelect, disabled = false }: {
  rect: R16GuildRect; choices: readonly GuildChoice[]; selected: number | null;
  label: string; onSelect: (choice: GuildChoice) => void; disabled?: boolean;
}) {
  const [open, setOpen] = useState(false);
  const [start, setStart] = useState(0);
  const [focused, setFocused] = useState(0);
  const root = useRef<HTMLDivElement>(null);
  const toggle = useRef<HTMLButtonElement>(null);
  const thumbDrag = useRef<{ pointer: number; y: number; start: number; scale: number } | null>(null);
  const id = useId();
  const count = G.ranks.dropdownMaxRows;
  const max = Math.max(0, choices.length - count);
  const offset = Math.min(start, max);
  const selectedChoice = choices.find(choice => choice.index === selected && !choice.create);
  useEffect(() => {
    if (!open) return;
    const outside = (event: PointerEvent) => { if (!root.current?.contains(event.target as Node)) setOpen(false); };
    document.addEventListener("pointerdown", outside, true);
    return () => document.removeEventListener("pointerdown", outside, true);
  }, [open]);
  useEffect(() => { if (disabled) setOpen(false); }, [disabled]);
  const focusRow = (index: number) => {
    const next = Math.max(0, Math.min(choices.length - 1, index));
    setFocused(next);
    setStart(current => next < current ? next : next >= current + count ? next - count + 1 : Math.min(current, max));
  };
  const expand = () => {
    const at = Math.max(0, choices.findIndex(choice => choice.index === selected && !choice.create));
    setFocused(at); setStart(Math.min(at, max)); setOpen(true);
  };
  const choose = (choice: GuildChoice | undefined) => {
    if (!choice || disabled || choice.disabled) return;
    setOpen(false); onSelect(choice); toggle.current?.focus();
  };
  const scroll = (delta: number) => setStart(current => Math.max(0, Math.min(max, current + delta)));
  const arrowRect = { left: rect.width - 18, top: 2, width: 12, height: 12 };
  return <div ref={root} data-guild-keyboard-owner="dropdown" style={{ ...guildRect(rect), zIndex: open ? 8 : 1 }}
    onBlur={event => { if (!event.currentTarget.contains(event.relatedTarget as Node | null)) setOpen(false); }}
    onWheel={event => { if (!open || !event.deltaY) return; event.preventDefault(); event.stopPropagation(); scroll(event.deltaY > 0 ? 1 : -1); }}
    onKeyDown={event => {
      if (event.nativeEvent.isComposing || event.keyCode === 229) return;
      if (event.key === "Escape" && open) { event.preventDefault(); event.stopPropagation(); setOpen(false); return; }
      if (["ArrowDown", "ArrowUp", "Home", "End", "PageDown", "PageUp"].includes(event.key)) {
        event.preventDefault(); event.stopPropagation();
        if (!open) { expand(); return; }
        focusRow(event.key === "Home" ? 0 : event.key === "End" ? choices.length - 1
          : focused + (event.key === "ArrowUp" ? -1 : event.key === "PageUp" ? -count : event.key === "PageDown" ? count : 1));
      } else if (open && (event.key === "Enter" || event.key === " ")) {
        event.preventDefault(); event.stopPropagation(); choose(choices[focused]);
      }
    }}>
    <button ref={toggle} type="button" role="combobox" aria-label={label} aria-expanded={open}
      aria-controls={id} aria-haspopup="listbox" aria-activedescendant={open ? `${id}-${focused}` : undefined}
      disabled={disabled} onClick={() => open ? setOpen(false) : expand()}
      style={{ ...control, width: "100%", height: "100%", background: "#191919", border: "1px solid #303030",
        textAlign: "left", padding: "0 18px 0 2px", overflow: "hidden", whiteSpace: "nowrap" }}>
      {selectedChoice?.name ?? ""}
      <img src={G.assets.dropdownArrow.normal} alt="" draggable={false} style={{ ...guildRect(arrowRect), pointerEvents: "none" }} />
    </button>
    {open ? <div id={id} role="listbox" aria-label={label}
      style={{ position: "absolute", left: 0, top: 15, width: rect.width, height: 67, background: "#141414", border: "1px solid #303030" }}>
      {choices.slice(offset, offset + count).map((choice, row) => {
        const actual = row + offset;
        return <button key={`${choice.create ? "new" : "rank"}-${choice.index}`} type="button" role="option"
          id={`${id}-${actual}`} aria-selected={!choice.create && choice.index === selected} aria-disabled={choice.disabled || undefined}
          data-guild-rank-choice={choice.create ? "create" : choice.index} tabIndex={-1}
          onPointerMove={() => setFocused(actual)} onClick={() => choose(choice)}
          style={{ ...control, position: "absolute", left: 0, top: row * G.ranks.dropdownRowStep,
            width: 114, height: 16, textAlign: "left", padding: "0 2px", overflow: "hidden", whiteSpace: "nowrap",
            background: actual === focused ? "rgb(140,70,0)" : "#141414" }}>{choice.name}</button>;
      })}
      <GuildSourceButton art={G.assets.dropdownUp} rect={{ left: 117, top: -1, width: 12, height: 6 }}
        label="↑" disabled={offset === 0} onClick={() => scroll(-1)} />
      <GuildSourceButton art={G.assets.dropdownDown} rect={{ left: 117, top: 59, width: 12, height: 6 }}
        label="↓" disabled={offset >= max} onClick={() => scroll(1)} />
      <div role="scrollbar" aria-label={label} aria-valuemin={0} aria-valuemax={max} aria-valuenow={offset}
        aria-controls={id} aria-orientation="vertical" tabIndex={-1}
        style={{ position: "absolute", left: 118, top: 7 + offset * G.ranks.thumbTravel / Math.max(1, choices.length), width: 8, height: 14, touchAction: "none" }}
        onPointerDown={event => {
          if (event.button !== 0) return;
          event.preventDefault(); event.stopPropagation(); event.currentTarget.setPointerCapture(event.pointerId);
          thumbDrag.current = { pointer: event.pointerId, y: event.clientY, start: offset,
            scale: rect.width / Math.max(1, root.current!.getBoundingClientRect().width) };
        }} onPointerMove={event => {
          const drag = thumbDrag.current; if (!drag || drag.pointer !== event.pointerId) return;
          event.stopPropagation(); setStart(Math.max(0, Math.min(max,
            drag.start + Math.round((event.clientY - drag.y) * drag.scale * Math.max(1, choices.length) / G.ranks.thumbTravel))));
        }} onPointerUp={() => { thumbDrag.current = null; }} onLostPointerCapture={() => { thumbDrag.current = null; }}>
        <img src={G.assets.dropdownThumb.normal} alt="" draggable={false} style={{ width: 8, height: 14, pointerEvents: "none" }} />
      </div>
    </div> : null}
  </div>;
}

export function GuildConfirmation({ title, children, confirm, cancel, disabled, onConfirm, onCancel }: {
  title: string; children?: ReactNode; confirm: string; cancel: string; disabled?: boolean;
  onConfirm: () => void; onCancel: () => void;
}) {
  const dialog = useRef<HTMLDivElement>(null);
  const confirmRef = useRef(onConfirm); confirmRef.current = onConfirm;
  useEffect(() => {
    const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    dialog.current?.querySelector<HTMLButtonElement>("button:not(:disabled)")?.focus();
    return () => { if (previous?.isConnected) previous.focus(); };
  }, []);
  return <div style={{ position: "absolute", inset: 0, zIndex: 15, display: "grid", placeItems: "center", background: "#0007" }}
    onPointerDown={event => event.stopPropagation()} onPointerUp={event => event.stopPropagation()}>
    <div ref={dialog} data-guild-keyboard-owner="confirmation" role="alertdialog" aria-modal="true" aria-label={title} style={{ width: 320, padding: 20,
      background: "#17120a", border: "1px solid #bba16b", color: "white" }} onKeyDown={event => {
      event.stopPropagation();
      if (event.nativeEvent.isComposing || event.keyCode === 229) return;
      if (event.key === "Escape") { event.preventDefault(); onCancel(); }
      if (event.key === "Enter" && !(event.target instanceof HTMLButtonElement)) { event.preventDefault(); if (!disabled) confirmRef.current(); }
      if (event.key === "Tab") {
        const buttons = dialog.current?.querySelectorAll<HTMLButtonElement>("button:not(:disabled)");
        if (!buttons?.length) { event.preventDefault(); return; }
        const first = buttons[0], last = buttons[buttons.length - 1];
        if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last.focus(); }
        else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first.focus(); }
      }
    }}>
      <p style={{ margin: "0 0 12px", whiteSpace: "pre-wrap" }}>{title}</p>{children}
      <div style={{ display: "flex", gap: 12, justifyContent: "center" }}>
        <button type="button" disabled={disabled} onClick={onConfirm} style={dialogButton}>{confirm}</button>
        <button type="button" onClick={onCancel} style={dialogButton}>{cancel}</button>
      </div>
    </div>
  </div>;
}

const control: CSSProperties = { border: 0, color: "white", background: "transparent", font: "inherit",
  fontSize: 10.6667, lineHeight: "14px", boxSizing: "border-box", cursor: "pointer" };
const dialogButton: CSSProperties = { color: "#f4dcaf", background: "#2b2112", border: "1px solid #9c8d6f", minWidth: 70,
  padding: "5px 8px", font: "inherit", cursor: "pointer" };
