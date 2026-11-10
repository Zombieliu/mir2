"use client";

import { useEffect, useRef, useState, type ButtonHTMLAttributes, type CSSProperties, type KeyboardEvent, type PointerEvent, type ReactNode } from "react";
import { r16ClampSocialWindow, R16_SOCIAL_LAYOUT, type R16SocialRect } from "../../lib/r16-social-layout";

export function R16SocialButton({ library = "Title", frame, hoverFrame = frame + 1, pressedFrame = frame + 2, rect, label, locked = false, ...props }:
  ButtonHTMLAttributes<HTMLButtonElement> & { library?: string; frame: number; hoverFrame?: number; pressedFrame?: number; rect: R16SocialRect; label: string; locked?: boolean }) {
  const [hovered, setHovered] = useState(false);
  const [pressed, setPressed] = useState(false);
  const source = `/original-ui/${library}/${!props.disabled && pressed ? pressedFrame : locked || !props.disabled && hovered ? hoverFrame : frame}.png`;
  return <button {...props} type={props.type ?? "button"} aria-label={label} title={label}
    aria-pressed={props["aria-pressed"] ?? (locked || undefined)}
    style={{ position: "absolute", ...rect, padding: 0, border: 0, background: "transparent", cursor: props.disabled ? "default" : "pointer", opacity: props.disabled ? 0.45 : 1, ...props.style }}
    onMouseEnter={() => setHovered(true)} onMouseLeave={() => { setHovered(false); setPressed(false); }}
    onMouseDown={() => { if (!props.disabled) setPressed(true); }} onMouseUp={() => setPressed(false)} onBlur={() => setPressed(false)}>
    <img src={source} alt="" draggable={false} style={{ display: "block", width: "100%", height: "100%", pointerEvents: "none", imageRendering: "pixelated" }} />
  </button>;
}

/** One drag owner per independent window, measured in logical stage pixels. */
export function useR16SocialWindow(initial: R16SocialRect) {
  const [position, setPosition] = useState({ left: initial.left, top: initial.top });
  const drag = useRef<{ pointerId: number; x: number; y: number; left: number; top: number; scale: number; width: number; height: number } | null>(null);
  useEffect(() => {
    const release = () => { drag.current = null; };
    window.addEventListener("blur", release);
    return () => window.removeEventListener("blur", release);
  }, []);
  const pointerDown = (event: PointerEvent<HTMLElement>) => {
    if (event.button !== 0 || !(event.target instanceof Element)
      || event.target.closest("button,input,textarea,select,[data-trade-slot]")) return;
    const bounds = event.currentTarget.getBoundingClientRect();
    const stage = event.currentTarget.closest<HTMLElement>(".client-stage-frame");
    drag.current = { pointerId: event.pointerId, x: event.clientX, y: event.clientY, ...position,
      scale: bounds.width / initial.width || 1,
      width: stage?.offsetWidth || R16_SOCIAL_LAYOUT.stage.width,
      height: stage?.offsetHeight || R16_SOCIAL_LAYOUT.stage.height };
    event.currentTarget.setPointerCapture(event.pointerId);
    event.preventDefault();
  };
  const pointerMove = (event: PointerEvent<HTMLElement>) => {
    const active = drag.current;
    if (!active || active.pointerId !== event.pointerId) return;
    setPosition(r16ClampSocialWindow({
      left: active.left + (event.clientX - active.x) / active.scale,
      top: active.top + (event.clientY - active.y) / active.scale,
    }, initial, active));
    event.preventDefault();
  };
  const pointerUp = () => { drag.current = null; };
  return { position, pointerDown, pointerMove, pointerUp };
}

/** Native message/amount boxes consume the keyboard and covered pointer frame. */
export function R16SocialModal({ label, rect, background, onCancel, onConfirm, children }:
  { label: string; rect: R16SocialRect; background: string; onCancel: () => void; onConfirm?: () => void; children: ReactNode }) {
  const root = useRef<HTMLElement | null>(null);
  useEffect(() => {
    const previous = document.activeElement;
    root.current?.querySelector<HTMLElement>("[data-r16-autofocus],input,button:not(:disabled)")?.focus();
    return () => { if (previous instanceof HTMLElement && previous.isConnected) previous.focus(); };
  }, []);
  const keyboard = (event: KeyboardEvent<HTMLElement>) => {
    event.stopPropagation();
    if (event.nativeEvent.isComposing) return;
    if (event.key === "Escape" || event.key === "Enter") {
      event.preventDefault();
      if (event.repeat) return;
      if (event.key === "Escape") onCancel(); else onConfirm?.();
    } else if (event.key === "Tab") {
      const fields = Array.from(event.currentTarget.querySelectorAll<HTMLElement>("button:not(:disabled),input:not(:disabled),[tabindex='0']"));
      if (!fields.length) { event.preventDefault(); return; }
      const index = fields.indexOf(document.activeElement as HTMLElement);
      const next = (index + (event.shiftKey ? -1 : 1) + fields.length) % fields.length;
      event.preventDefault(); fields[next].focus();
    }
  };
  return <div className="r16-social-modal-shield" data-ui-interactive="true" style={{ position: "absolute", inset: 0, zIndex: 1200, pointerEvents: "auto" }}
    onPointerDown={event => event.stopPropagation()} onClick={event => event.stopPropagation()}>
    <section ref={root} role="dialog" aria-modal="true" aria-label={label} tabIndex={-1} onKeyDownCapture={keyboard}
      style={{ position: "absolute", ...rect, color: "#fff", fontFamily: "Arial, Helvetica, sans-serif", fontSize: 32 / 3, textShadow: "1px 1px #000" }}>
      <img src={background} alt="" draggable={false} style={R16_SOCIAL_ART_STYLE} />
      {children}
    </section>
  </div>;
}

export const R16_SOCIAL_ART_STYLE: CSSProperties = { position: "absolute", inset: 0, width: "100%", height: "100%", pointerEvents: "none", imageRendering: "pixelated" };
