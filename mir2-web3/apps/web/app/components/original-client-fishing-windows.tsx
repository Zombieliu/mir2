"use client";

import { useLayoutEffect, useRef, type CSSProperties } from "react";
import { localizeCrystalItemName } from "../../lib/crystal-content-localization";
import type { FishingRodView, FishingStatusView } from "../../lib/world-fishing-panel";
import { fishingItemCursorOrigin, fishingItemVisibleSize } from "../../lib/world-fishing-tackle-gesture";
import { originalItemIconPath } from "./original-client-inventory-utils";
import { OriginalItemTooltip } from "./original-client-item-tooltip";
import type { TranslateFn } from "./original-client-types";
import { R16_SOCIAL_ART_STYLE, R16SocialButton, R16SocialModal, useR16SocialWindow } from "./r16-social-controls";

export type { FishingRodView, FishingStatusView, FishingTackleView } from "../../lib/world-fishing-panel";

type FishingWindowsProps = {
  t: TranslateFn;
  rod: FishingRodView | null;
  equipmentOpen: boolean;
  status: FishingStatusView | null;
  biteAnimation?: { startedAt: number; frame: number };
  reelPending: boolean;
  attachmentPending: boolean;
  selectedSlot: number | null;
  autoCast: boolean;
  escapeCancels: boolean;
  noticeOpen?: boolean;
  onDismissNotice?: () => void;
  onCloseEquipment: () => void;
  onCloseStatus: () => void;
  onReelBite: () => void;
  onToggleAutoCast: () => void;
  onToggleEscape: () => void;
  onTackleClick: (slot: number) => void;
  onClearSelection?: () => void;
};

const EQUIPMENT = { left: 412, top: 240.5, width: 200, height: 287 };
const STATUS = { left: 390, top: 300, width: 244, height: 128 };
const NOTICE = { left: 284, top: 289, width: 456, height: 190 };
const TACKLE = [
  { name: "Hook", left: 17, top: 203 },
  { name: "Float", left: 17, top: 241 },
  { name: "Bait", left: 57, top: 241 },
  { name: "Finder", left: 97, top: 241 },
  { name: "Reel", left: 137, top: 241 },
] as const;

const WINDOW_STYLE: CSSProperties = {
  position: "absolute", zIndex: 90, pointerEvents: "auto", color: "#fff",
  fontFamily: "Arial, Helvetica, sans-serif", fontSize: 32 / 3, textShadow: "1px 1px #000",
};
const CAPTION_STYLE: CSSProperties = {
  position: "absolute", display: "grid", placeItems: "center", pointerEvents: "none",
};
const LOCALIZED_CAPTION_STYLE: CSSProperties = {
  ...CAPTION_STYLE, fontSize: 9, background: "#0e0b08", overflow: "hidden", textAlign: "center",
};
const SPRITE_STYLE: CSSProperties = { position: "absolute", pointerEvents: "none", imageRendering: "pixelated" };

export function FishingTackleCursor({ icon }: { icon: number | null }) {
  const layerRef = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    const layer = layerRef.current;
    const stage = layer?.closest(".client-stage-frame");
    if (!layer || !stage) return;
    let point: { x: number; y: number } | null = null;
    let measuredImage: HTMLImageElement | null = null;
    let size = { width: 0, height: 0 };
    const paint = () => {
      const image = layer.querySelector("img");
      if (!image?.complete || !image.naturalWidth) return;
      const bounds = layer.getBoundingClientRect();
      if (!point || !bounds.width || !bounds.height) {
        image.style.visibility = "hidden";
        return;
      }
      if (image !== measuredImage) {
        measuredImage = image;
        const canvas = document.createElement("canvas");
        canvas.width = image.naturalWidth;
        canvas.height = image.naturalHeight;
        const pixels = canvas.getContext("2d");
        size = { width: canvas.width, height: canvas.height };
        if (pixels) {
          pixels.drawImage(image, 0, 0);
          size = fishingItemVisibleSize(pixels.getImageData(0, 0, canvas.width, canvas.height).data, canvas.width, canvas.height);
        }
      }
      const screen = { width: layer.clientWidth, height: layer.clientHeight };
      const x = (point.x - bounds.left) * screen.width / bounds.width;
      const y = (point.y - bounds.top) * screen.height / bounds.height;
      const { left, top } = fishingItemCursorOrigin(x, y, size, screen);
      image.style.transform = `translate(${left}px, ${top}px)`;
      image.style.visibility = size.width && size.height ? "visible" : "hidden";
    };
    const move = (event: Event) => {
      const pointer = event as PointerEvent;
      if (!pointer.isPrimary) return;
      point = { x: pointer.clientX, y: pointer.clientY };
      paint();
    };
    const hide = () => { point = null; paint(); };
    stage.addEventListener("pointermove", move, true);
    stage.addEventListener("pointerdown", move, true);
    stage.addEventListener("pointerleave", hide);
    stage.addEventListener("pointercancel", hide, true);
    layer.addEventListener("load", paint, true);
    window.addEventListener("blur", hide);
    window.addEventListener("resize", paint);
    return () => {
      stage.removeEventListener("pointermove", move, true);
      stage.removeEventListener("pointerdown", move, true);
      stage.removeEventListener("pointerleave", hide);
      stage.removeEventListener("pointercancel", hide, true);
      layer.removeEventListener("load", paint, true);
      window.removeEventListener("blur", hide);
      window.removeEventListener("resize", paint);
    };
  }, []);
  return <div ref={layerRef} aria-hidden="true"
    style={{ position: "absolute", inset: 0, zIndex: 1000, pointerEvents: "none", overflow: "hidden" }}>
    {icon !== null && <img key={icon} src={originalItemIconPath(icon)} alt="" draggable={false}
      data-fishing-held-item={icon}
      style={{ ...SPRITE_STYLE, left: 0, top: 0, maxWidth: "none", visibility: "hidden" }} />}
  </div>;
}

function FishingBar({ label, frame, top, height, value }: { label: string; frame: number; top: number; height: number; value: number }) {
  const percent = Math.min(100, Math.max(0, value));
  const width = Math.floor(216 * percent / 100);
  return <div role="progressbar" aria-label={label} aria-valuenow={percent}
    style={{ position: "absolute", left: 14, top, width: 216, height, pointerEvents: "none" }}>
    <div style={{ width, height, overflow: "hidden" }}>
      <img src={`/original-ui/Prguse/${frame}.png`} alt="" draggable={false}
        style={{ display: "block", width: 216, maxWidth: "none", height, imageRendering: "pixelated" }} />
    </div>
  </div>;
}

export function OriginalClientFishingWindows({ t, rod, equipmentOpen, status, biteAnimation, reelPending, attachmentPending,
  selectedSlot, autoCast, escapeCancels, noticeOpen, onDismissNotice, onCloseEquipment, onCloseStatus, onReelBite,
  onToggleAutoCast, onToggleEscape, onTackleClick, onClearSelection }: FishingWindowsProps) {
  const equipmentWindow = useR16SocialWindow(EQUIPMENT);
  const statusWindow = useR16SocialWindow(STATUS);
  const close = t("ui.close", [], "Close");
  const fishing = t("game.032", [], "Fishing");
  const escape = t("game.complete.007", [], "Esc to exit");
  const rodFrame = rod ? (rod.shape === 49 ? 1333 : 1335) + Number(rod.hasHook) : null;
  const notice = t("client.NoFishingRod", [], "You are not holding a fishing rod.");
  return <>
    {equipmentOpen && rod && rodFrame !== null && <section role="dialog"
      aria-label={t("npc.prose.e2eed1b4d37624b7", [], "Fishing Equipment")} aria-busy={attachmentPending}
      data-mir2-fishing-window="equipment" data-ui-interactive="true"
      onPointerDown={equipmentWindow.pointerDown} onPointerMove={equipmentWindow.pointerMove}
      onPointerUp={equipmentWindow.pointerUp} onLostPointerCapture={equipmentWindow.pointerUp}
      style={{ ...WINDOW_STYLE, ...EQUIPMENT, ...equipmentWindow.position }}>
      <img src="/original-ui/Prguse/1340.png" alt="" draggable={false} style={R16_SOCIAL_ART_STYLE} />
      <div style={{ ...CAPTION_STYLE, left: 10, top: 4, width: 180, height: 20 }}>{localizeCrystalItemName(rod.name, t)}</div>
      <R16SocialButton library="Prguse2" frame={360} rect={{ left: 175, top: 3, width: 24, height: 21 }}
        label={close} data-fishing-equipment-close="true" onClick={onCloseEquipment} />
      <img src={`/original-ui/StateItem/${rodFrame}.png`} alt="" draggable={false}
        style={{ ...SPRITE_STYLE, left: 10, top: 40, width: 176, height: rodFrame === 1333 ? 197 : 196 }} />
      {TACKLE.slice(0, rod.slots.length).map(({ name, left, top }, slot) => {
        const item = rod.slots[slot];
        const selected = selectedSlot === slot;
        const slotName = t(`client.ItemType${name}`, [], name);
        const content = item?.name ? localizeCrystalItemName(item.name, t) : item ? null : t("client.Empty", [], "Empty");
        const label = slotName + (content ? ` · ${content}` : "") + (item && item.quantity > 1 ? ` ×${item.quantity}` : "");
        return <button key={name} type="button" aria-label={label} aria-pressed={selected}
          data-fishing-slot={slot} data-fishing-item-id={item?.uniqueId} disabled={attachmentPending}
          onClick={() => onTackleClick(slot)} onContextMenu={event => {
            event.preventDefault();
            event.stopPropagation();
            onClearSelection?.();
          }}
          style={{ position: "absolute", left, top, width: 34, height: 30, padding: 0, border: 0,
            color: "inherit", textShadow: "inherit", background: "transparent", cursor: "pointer" }}>
          {item?.imageIndex != null && <img src={originalItemIconPath(item.imageIndex)} alt="" draggable={false}
            style={{ ...R16_SOCIAL_ART_STYLE, objectFit: "scale-down" }} />}
          {item && item.quantity > 1 && <span aria-hidden="true"
            style={{ position: "absolute", bottom: 0, right: 1, pointerEvents: "none", fontSize: 10 }}>{item.quantity}</span>}
          {item && content && !selected && <OriginalItemTooltip t={t} name={content}
            itemType={slotName} quantity={item.quantity}
            durabilityCurrent={item.durabilityCurrent === null ? undefined : Math.floor(item.durabilityCurrent / 1000)}
            durabilityMax={item.durabilityMax === null ? undefined : Math.floor(item.durabilityMax / 1000)} />}
        </button>;
      })}
    </section>}
    {status && <section role="dialog" aria-label={fishing} aria-busy={reelPending}
      data-mir2-fishing-window="status" data-ui-interactive="true" data-fishing-found-fish={status.foundFish}
      onPointerDown={statusWindow.pointerDown} onPointerMove={statusWindow.pointerMove}
      onPointerUp={statusWindow.pointerUp} onLostPointerCapture={statusWindow.pointerUp}
      style={{ ...WINDOW_STYLE, ...STATUS, ...statusWindow.position, zIndex: 91 }}>
      <img src="/original-ui/Prguse/1341.png" alt="" draggable={false} style={R16_SOCIAL_ART_STYLE} />
      <FishingBar label={t("client.SuccessChance", [status.chance], `${status.chance}%`)} frame={1342} top={64} height={12} value={status.chance} />
      <FishingBar label={t("client.Progress", [], "Progress")} frame={1349} top={79} height={8} value={status.progress} />
      <div aria-hidden="true" style={{ ...CAPTION_STYLE, left: 14, top: 62, width: 216, height: 12 }}>{status.chance}%</div>
      <R16SocialButton library="Prguse2" frame={360} rect={{ left: 216, top: 4, width: 24, height: 21 }}
        label={close} data-fishing-reel="true" disabled={reelPending} onClick={onCloseStatus} />
      {fishing === "Fishing" ? <img src="/original-ui/Title/149.png" alt="" draggable={false}
        style={{ ...SPRITE_STYLE, left: 47, top: 95, width: 60, height: 25 }} />
        : <div aria-hidden="true" style={{ ...LOCALIZED_CAPTION_STYLE, left: 47, top: 95, width: 60, height: 25 }}>{fishing}</div>}
      {status.foundFish && biteAnimation && <R16SocialButton key={biteAnimation.startedAt}
        frame={biteAnimation.frame} hoverFrame={biteAnimation.frame} pressedFrame={142}
        rect={{ left: 47, top: 95, width: 60, height: 25 }}
        label={t("ui.fishingReel", [], "Reel in")} data-fishing-bite="true"
        disabled={reelPending} onClick={onReelBite} />}
      {rod?.canAutoCast && <>
        <R16SocialButton frame={180} rect={{ left: 110, top: 95, width: 48, height: 25 }}
          label={t("game.complete.005", [], "Auto")} aria-pressed={autoCast} data-fishing-autocast="true"
          disabled={reelPending} onClick={onToggleAutoCast} />
        <img src={`/original-ui/Prguse/${autoCast ? 1344 : 1343}.png`} alt="" draggable={false}
          style={{ ...SPRITE_STYLE, left: 172, top: 95, width: 28, height: 25 }} />
      </>}
      <button type="button" aria-label={escape} title={escape} aria-pressed={escapeCancels} data-fishing-escape="true"
        onClick={onToggleEscape} style={{ position: "absolute", left: 135, top: 41, width: 12, height: 12,
          border: 0, padding: 0, background: "transparent", cursor: "pointer" }}>
        <img src="/original-ui/Prguse/1346.png" alt="" draggable={false} style={R16_SOCIAL_ART_STYLE} />
        {escapeCancels && <img src="/original-ui/Prguse/1347.png" alt="" draggable={false}
          style={{ ...SPRITE_STYLE, left: 0, top: 0, width: 16, height: 12 }} />}
      </button>
      {escape === "Esc to exit" ? <img src="/original-ui/Title/45.png" alt="" draggable={false}
        style={{ ...SPRITE_STYLE, left: 150, top: 40, width: 68, height: 16 }} />
        : <div aria-hidden="true" style={{ ...LOCALIZED_CAPTION_STYLE, left: 150, top: 40, width: 68, height: 16 }}>{escape}</div>}
    </section>}
    {noticeOpen && onDismissNotice && <div data-mir2-fishing-window="notice" data-ui-interactive="true" style={{ display: "contents" }}>
      <R16SocialModal label={notice} rect={NOTICE} background="/original-ui/Prguse/360.png"
        onCancel={onDismissNotice} onConfirm={onDismissNotice}>
        <div style={{ position: "absolute", left: 35, top: 35, width: 390, height: 110 }}>{notice}</div>
        <R16SocialButton frame={200} rect={{ left: 360, top: 157, width: 76, height: 25 }}
          label={t("game.051", [], "OK")} data-fishing-notice-dismiss="true" onClick={onDismissNotice} />
      </R16SocialModal>
    </div>}
  </>;
}
