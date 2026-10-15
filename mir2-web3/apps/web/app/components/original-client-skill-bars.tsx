"use client";

import { useEffect, useRef } from "react";
import nativeMagIcons from "../../public/original-ui/MagIcon/meta.json";
import type { CrystalKeyBinding } from "../../lib/player-ui-preferences";

export type DisplaySkill = {
  key: string; name: string; hotkey?: number; icon?: number; mpCost?: number;
  delayMs?: number; cooldownRemainingTicks?: number;
};
export type SkillBarPositions = readonly [readonly [number, number], readonly [number, number]];
export type SkillBarLease = Readonly<{ slot: number; skillKey: string; sourceKey: string; cursor: readonly [number, number] | null }>;
export type SkillBarsProps = {
  skills: readonly DisplaySkill[]; bindings: readonly CrystalKeyBinding[]; visible: boolean;
  positions: SkillBarPositions; onPositionsChange?: (positions: SkillBarPositions, commit: boolean) => void;
  inputBlocked: boolean; sourceKey: string | null; onCastSlot?: (lease: SkillBarLease) => void;
  onPointerHeldChange?: (held: boolean) => void;
  /** Supply only a verified conversion for the actual carrier; no guessed tick duration. */
  cooldownTickMs?: number;
};
const icons = new Map(nativeMagIcons.frames.map(frame => [frame.index, { width: frame.width, height: frame.height }]));
export function crystalSkillBarHasSkill(skills: readonly DisplaySkill[], bar: 0 | 1): boolean {
  // Native skill_bars.rs:19 preserves the inclusive key9 boundary for both bars.
  return skills.some(skill => skill !== null && typeof skill === "object" && Number.isInteger(skill.hotkey) && skill.hotkey! >= bar * 8 + 1 && skill.hotkey! <= (bar + 1) * 8 + 1);
}
export function crystalSkillBarSkillForSlot(skills: readonly DisplaySkill[], slot: number): DisplaySkill | null {
  if (!Number.isInteger(slot) || slot < 1 || slot > 16) return null;
  const matches = skills.filter(skill => skill !== null && typeof skill === "object" && skill.hotkey === slot);
  if (matches.length !== 1 || typeof matches[0].key !== "string" || !matches[0].key || matches[0].key.length > 128
    || skills.filter(skill => skill !== null && typeof skill === "object" && skill.key === matches[0].key).length !== 1) return null;
  return matches[0];
}
export function clampCrystalSkillBarPositions(positions: SkillBarPositions): SkillBarPositions {
  const point = (value: readonly [number, number]): readonly [number, number] => [
    Number.isFinite(value[0]) ? Math.max(0, Math.min(808, Math.round(value[0]))) : 0,
    Number.isFinite(value[1]) ? Math.max(0, Math.min(740, Math.round(value[1]))) : 0,
  ];
  return [point(positions[0]), point(positions[1])];
}
export function crystalSkillBarCooldownFrame(skill: DisplaySkill, tickMs?: number): number | null {
  const ticks = skill.cooldownRemainingTicks, delay = skill.delayMs;
  if (typeof ticks !== "number" || !Number.isSafeInteger(ticks) || ticks < 0
    || typeof delay !== "number" || !Number.isInteger(delay) || delay < 22 || delay > 0xffffffff
    || typeof tickMs !== "number" || !Number.isFinite(tickMs) || tickMs <= 0) return null;
  const remaining = ticks * tickMs;
  if (!Number.isSafeInteger(remaining) || remaining < 100 || remaining > 0xffffffff) return null;
  return 1260 + 22 - Math.min(22, Math.floor(remaining / Math.floor(delay / 22)));
}
/** A UI lease is not send authority; Root must recheck the current raw model and Combat proof. */
export class CrystalSkillBarGesture {
  private armed: { pointerId: number; lease: SkillBarLease } | null = null;
  arm(pointerId: number, lease: SkillBarLease): boolean {
    if (this.armed || !Number.isSafeInteger(pointerId) || pointerId < -1
      || !Number.isInteger(lease.slot) || lease.slot < 1 || lease.slot > 16
      || typeof lease.skillKey !== "string" || !lease.skillKey || lease.skillKey.length > 128
      || typeof lease.sourceKey !== "string" || !lease.sourceKey || lease.sourceKey.length > 65536
      || lease.cursor !== null && (lease.cursor.length !== 2 || !lease.cursor.every(Number.isFinite))) return false;
    this.armed = { pointerId, lease: Object.freeze({ ...lease,
      cursor: lease.cursor ? Object.freeze([lease.cursor[0], lease.cursor[1]] as const) : null }) };
    return true;
  }
  release(pointerId: number, sourceKey: string | null, slot: number, skillKey: string | null, inside: boolean, blocked: boolean, cursor?: readonly [number, number] | null): SkillBarLease | null {
    const armed = this.armed;
    if (!armed || armed.pointerId !== pointerId) return null;
    this.armed = null;
    if (blocked || !inside || sourceKey !== armed.lease.sourceKey || slot !== armed.lease.slot || skillKey !== armed.lease.skillKey) return null;
    if (cursor === undefined) return armed.lease;
    if (cursor !== null && (cursor.length !== 2 || !cursor.every(Number.isFinite))) return null;
    return Object.freeze({ ...armed.lease, cursor: cursor ? Object.freeze([cursor[0], cursor[1]] as const) : null });
  }
  cancel(): void { this.armed = null; }
  isHeld(): boolean { return this.armed !== null; }
}
export function crystalSkillBarIconForSkill(skill: DisplaySkill | null) {
  if (!skill || !Number.isInteger(skill.icon) || skill.icon! < 0 || skill.icon! > 255) return null;
  const index = skill.icon! * 2, dimensions = icons.get(index);
  return dimensions ? { index, ...dimensions } : null;
}
function stagePoint(element: HTMLElement, x: number, y: number): readonly [number, number] | null {
  const stage = element.closest<HTMLElement>(".client-stage-frame");
  const rect = stage?.getBoundingClientRect();
  return rect && rect.width > 0 && rect.height > 0 ? [(x - rect.left) * 1024 / rect.width, (y - rect.top) * 768 / rect.height] : null;
}
export function OriginalClientSkillBars(props: SkillBarsProps) {
  const latest = useRef(props); latest.current = props;
  const gesture = useRef(new CrystalSkillBarGesture());
  const drag = useRef<{ pointerId: number; bar: 0 | 1; sourceKey: string; x: number; y: number; scale: number; positions: SkillBarPositions } | null>(null);
  const moved = useRef<SkillBarPositions | null>(null);
  const held = useRef(false);
  const setHeld = (value: boolean) => { if (held.current !== value) { held.current = value; latest.current.onPointerHeldChange?.(value); } };
  const cancel = () => { gesture.current.cancel(); drag.current = null; moved.current = null; setHeld(false); };
  useEffect(() => {
    const hidden = () => { if (document.visibilityState !== "visible") cancel(); };
    window.addEventListener("blur", cancel); document.addEventListener("visibilitychange", hidden);
    return () => { window.removeEventListener("blur", cancel); document.removeEventListener("visibilitychange", hidden); cancel(); };
    // Stable listeners read current callbacks through latest.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  useEffect(() => { cancel(); }, [props.visible, props.inputBlocked, props.sourceKey]);
  const positions = clampCrystalSkillBarPositions(props.positions);
  if (!props.visible) return null;
  const canInput = !props.inputBlocked && Boolean(props.sourceKey) && Boolean(props.onCastSlot);
  const release = (pointerId: number, slot: number, inside: boolean, cursor: readonly [number, number] | null) => {
    const current = latest.current, skill = crystalSkillBarSkillForSlot(current.skills, slot);
    const lease = gesture.current.release(pointerId, current.sourceKey, slot, skill?.key ?? null, inside,
      !current.visible || current.inputBlocked || document.visibilityState !== "visible" || !document.hasFocus(), cursor);
    if (!gesture.current.isHeld() && !drag.current) setHeld(false);
    if (lease) current.onCastSlot?.(lease);
  };
  return <>
    {([0, 1] as const).map(bar => {
      if (!crystalSkillBarHasSkill(props.skills, bar)) return null;
      return <div key={bar} className="original-skill-bar" data-ui-interactive="true"
        role="group" aria-label={"Skill bar " + (bar + 1)}
        style={{ position: "absolute", left: positions[bar][0], top: positions[bar][1], width: 216, height: 28, zIndex: 40, touchAction: "none" }}
        onPointerDown={event => {
          const current = latest.current;
          if (drag.current || gesture.current.isHeld() || event.target !== event.currentTarget || event.button !== 0 || current.inputBlocked || !current.visible || !current.sourceKey || !current.onPositionsChange) return;
          event.preventDefault(); event.stopPropagation();
          const rect = event.currentTarget.getBoundingClientRect(); if (rect.width <= 0) return;
          cancel();
          drag.current = { pointerId: event.pointerId, bar, sourceKey: current.sourceKey, x: event.clientX, y: event.clientY,
            scale: rect.width / 216, positions: clampCrystalSkillBarPositions(current.positions) };
          event.currentTarget.setPointerCapture(event.pointerId); setHeld(true);
        }}
        onPointerMove={event => {
          const active = drag.current, current = latest.current;
          if (!active || active.pointerId !== event.pointerId || active.bar !== bar) return;
          if (current.inputBlocked || !current.visible || current.sourceKey !== active.sourceKey) { cancel(); return; }
          event.preventDefault(); event.stopPropagation();
          const next: [[number, number], [number, number]] = [[...active.positions[0]], [...active.positions[1]]];
          next[bar] = [active.positions[bar][0] + (event.clientX - active.x) / active.scale, active.positions[bar][1] + (event.clientY - active.y) / active.scale];
          moved.current = clampCrystalSkillBarPositions(next); current.onPositionsChange?.(moved.current, false);
        }}
        onPointerUp={event => {
          const active = drag.current, current = latest.current;
          if (!active || active.pointerId !== event.pointerId) return;
          event.preventDefault(); event.stopPropagation();
          const next = moved.current ?? clampCrystalSkillBarPositions(current.positions);
          const valid = current.visible && !current.inputBlocked && current.sourceKey === active.sourceKey;
          cancel(); if (valid) current.onPositionsChange?.(next, true);
        }}
        onPointerCancel={cancel} onLostPointerCapture={() => { if (drag.current) cancel(); }}>
        <img src="/original-ui/Prguse/2190.png" alt="" draggable={false} style={{ position: "absolute", inset: 0, width: 216, height: 28, pointerEvents: "none" }} />
        <img src="/original-ui/Prguse/2193.png" alt="" draggable={false} style={{ position: "absolute", left: 12, top: 0, width: 204, height: 28, opacity: 0.5, pointerEvents: "none" }} />
        <button type="button" disabled aria-label="Refresh skill bar unavailable" title="Refresh unavailable"
          style={{ position: "absolute", left: 0, top: 0, width: 16, height: 28, border: 0, padding: 0, background: "transparent" }}>
          <img src="/original-ui/Prguse/2247.png" alt="" draggable={false} style={{ width: 16, height: 28 }} />
        </button>
        <span style={{ position: "absolute", left: 0, top: 1, width: 10, height: 25, color: "white", fontSize: 10.67, pointerEvents: "none" }}>{bar + 1}</span>
        {Array.from({ length: 8 }, (_, cell) => {
          const slot = bar * 8 + cell + 1, skill = crystalSkillBarSkillForSlot(props.skills, slot), icon = crystalSkillBarIconForSkill(skill);
          const key = props.bindings.find(binding => binding.function === "Bar" + (bar + 1) + "Skill" + (cell + 1))?.key ?? "";
          if (!skill || !icon) return <span key={slot} style={{ position: "absolute", left: cell * 25 + 13, top: 0, width: 25, height: 25, fontSize: 10.67, color: "white", pointerEvents: "none" }}>{key}</span>;
          const cooldown = crystalSkillBarCooldownFrame(skill, props.cooldownTickMs);
          const hint = [skill.name, skill.mpCost === undefined ? null : "MP: " + skill.mpCost,
            skill.delayMs === undefined ? null : "Cooldown: " + skill.delayMs + "ms", "Key: " + key].filter(Boolean).join("\n");
          return <button key={slot} type="button" disabled={!canInput} aria-label={skill.name + " (" + key + ")"} title={hint}
            style={{ position: "absolute", left: cell * 25 + 15, top: 3, width: icon.width, height: icon.height, border: 0, padding: 0, background: "transparent", touchAction: "none" }}
            onPointerDown={event => {
              if (event.button !== 0) return;
              const current = latest.current, selected = crystalSkillBarSkillForSlot(current.skills, slot);
              if (drag.current || !selected || !current.visible || current.inputBlocked || !current.sourceKey || !current.onCastSlot || selected.key !== skill.key) return;
              event.preventDefault(); event.stopPropagation();
              if (gesture.current.arm(event.pointerId, { slot, skillKey: selected.key, sourceKey: current.sourceKey,
                cursor: stagePoint(event.currentTarget, event.clientX, event.clientY) })) {
                event.currentTarget.setPointerCapture(event.pointerId); setHeld(true);
              }
            }}
            onPointerUp={event => {
              event.preventDefault(); event.stopPropagation();
              const rect = event.currentTarget.getBoundingClientRect();
              // Native release hit is fixed 24x22, even when decoded icon height is 23.
              release(event.pointerId, slot, event.clientX >= rect.left && event.clientX < rect.left + 24 * rect.width / icon.width
                && event.clientY >= rect.top && event.clientY < rect.top + 22 * rect.height / icon.height,
                stagePoint(event.currentTarget, event.clientX, event.clientY));
            }}
            onPointerCancel={cancel} onLostPointerCapture={cancel}
            onKeyDown={event => {
              if ((event.key !== "Enter" && event.key !== " ") || event.repeat) return;
              event.preventDefault(); event.stopPropagation();
              const current = latest.current, selected = crystalSkillBarSkillForSlot(current.skills, slot);
              if (!drag.current && selected && current.visible && !current.inputBlocked && current.sourceKey && current.onCastSlot && selected.key === skill.key
                && gesture.current.arm(-1, { slot, skillKey: selected.key, sourceKey: current.sourceKey, cursor: null })) setHeld(true);
            }}
            onKeyUp={event => { if (event.key === "Enter" || event.key === " ") { event.preventDefault(); event.stopPropagation(); release(-1, slot, true, null); } }}
            onBlur={cancel} onClick={event => { event.preventDefault(); event.stopPropagation(); }}>
            <img src={"/original-ui/MagIcon/" + icon.index + ".png"} alt="" draggable={false} style={{ width: icon.width, height: icon.height, pointerEvents: "none" }} />
            {cooldown !== null ? <img src={"/original-ui/Prguse2/" + cooldown + ".png"} alt="" draggable={false}
              style={{ position: "absolute", left: 0, top: 0, width: 24, height: 22, opacity: 0.6, pointerEvents: "none" }} /> : null}
          </button>;
        })}
      </div>;
    })}
  </>;
}
