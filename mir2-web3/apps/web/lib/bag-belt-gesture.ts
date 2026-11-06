import type { ItemActionRef } from "../app/components/original-client-types";

/** Measured endpoints of the six actual Belt buttons, in stage logical pixels. */
export type BagBeltDropTarget = Readonly<{ slot: number; uniqueId: number | null }>;
export type BagBeltDropGeometry = Readonly<{ revision: number; targets: ReadonlyArray<BagBeltDropTarget & Readonly<{ left: number; top: number; width: number; height: number }>> }>;
export type BagBeltRect = readonly [number, number, number, number];
export type BagBeltGeometry = BagBeltDropGeometry & Readonly<{ stage: HTMLElement; stageRect: BagBeltRect;
  buttons: readonly HTMLButtonElement[]; scale: number; virtualWidth: number; virtualHeight: number;
  devicePixelRatio: number; page: "bag1" | "bag2" }>;
/** The Page owns token creation, custody, arming and one-use consumption. */
export type BagBeltGestureProof = Readonly<{ token: object }>;
export type BagBeltReleasePoint = Readonly<{ clientX: number; clientY: number }>;
export type BagBeltButtonBinding = Readonly<{ slot: number; node: HTMLButtonElement;
  item: Readonly<{ authoritativeUniqueId?: number }> | null }>;
export type BagBeltCallbacks = {
  onBagBeltDropGeometry?: (geometry: BagBeltGeometry | null) => void;
  onBeginBagToBeltGesture?: (source: Readonly<ItemActionRef> | null, geometry: BagBeltGeometry, owner: "react" | "bevy") => BagBeltGestureProof | null;
  onArmBagToBeltGesture?: (proof: BagBeltGestureProof, geometry: BagBeltGeometry, target: BagBeltDropTarget, point: BagBeltReleasePoint) => boolean;
  onBagToBeltMove?: (item: Readonly<ItemActionRef>, target: BagBeltDropTarget, proof: BagBeltGestureProof, geometry: BagBeltGeometry) => boolean;
  onCancelBagToBeltGesture?: (proof: BagBeltGestureProof) => void;
};
const safe = (value: number) => Number.isSafeInteger(value) && value >= 0;
export function validBagBeltDropGeometry(value: BagBeltDropGeometry | null | undefined): value is BagBeltDropGeometry {
  if (!value || !safe(value.revision) || value.revision === 0 || !Array.isArray(value.targets) || value.targets.length !== 6) return false;
  const slots = new Set<number>(), ids = new Set<number>();
  for (const target of value.targets) {
    if (!target || typeof target !== "object" || !safe(target.slot) || target.slot >= 6 || slots.has(target.slot)
      || target.uniqueId !== null && (!safe(target.uniqueId) || ids.has(target.uniqueId))
      || ![target.left, target.top, target.width, target.height].every(Number.isFinite)
      || target.left < 0 || target.top < 0 || target.width <= 0 || target.height <= 0) return false;
    slots.add(target.slot); if (target.uniqueId !== null) ids.add(target.uniqueId);
  }
  for (let i = 0; i < 6; i++) for (let j = i + 1; j < 6; j++) {
    const a = value.targets[i], b = value.targets[j];
    if (a.left < b.left + b.width && b.left < a.left + a.width && a.top < b.top + b.height && b.top < a.top + a.height) return false;
  }
  return true;
}
export function sameBagBeltGeometry(a: BagBeltGeometry | null, b: BagBeltGeometry | null): boolean {
  return !!a && !!b && a.stage === b.stage && a.page === b.page && a.scale === b.scale
    && a.virtualWidth === b.virtualWidth && a.virtualHeight === b.virtualHeight && a.devicePixelRatio === b.devicePixelRatio
    && a.stageRect.every((n, i) => n === b.stageRect[i]) && a.buttons.length === 6 && b.buttons.length === 6
    && a.buttons.every((node, i) => node === b.buttons[i]) && JSON.stringify(a.targets) === JSON.stringify(b.targets);
}
export function bagBeltTargetAtClientPoint(geometry: BagBeltGeometry, x: number, y: number): BagBeltDropTarget | null {
  if (!Number.isFinite(x) || !Number.isFinite(y) || !validBagBeltDropGeometry(geometry) || geometry.scale <= 0) return null;
  const logicalX = (x - geometry.stageRect[0]) / geometry.scale, logicalY = (y - geometry.stageRect[1]) / geometry.scale;
  const targets = geometry.targets.filter(t => logicalX >= t.left && logicalX < t.left + t.width && logicalY >= t.top && logicalY < t.top + t.height);
  if (targets.length !== 1) return null;
  const target = targets[0], button = geometry.buttons[geometry.targets.indexOf(target)];
  try {
    const hit = document.elementFromPoint(x, y), style = window.getComputedStyle(button);
    if (!button || !button.isConnected || button.disabled || button.hidden || !hit
      || hit !== button && !button.contains(hit) || style.display === "none" || style.visibility === "hidden"
      || style.visibility === "collapse" || style.pointerEvents === "none" || style.opacity === "0") return null;
  } catch { return null; }
  return Object.freeze({ slot: target.slot, uniqueId: target.uniqueId });
}

export function bagBeltReleasePointIsCurrent(geometry: BagBeltGeometry, target: BagBeltDropTarget,
  point: BagBeltReleasePoint): boolean {
  if (!bagBeltGeometryIsCurrent(geometry)) return false;
  const actual = bagBeltTargetAtClientPoint(geometry, point.clientX, point.clientY);
  return !!actual && actual.slot === target.slot && actual.uniqueId === target.uniqueId;
}

/** Re-read actual nodes at the final dispatch boundary; a committed snapshot alone is insufficient. */
export function bagBeltGeometryIsCurrent(geometry: BagBeltGeometry | null): geometry is BagBeltGeometry {
  if (!geometry || !validBagBeltDropGeometry(geometry) || !geometry.stage.isConnected || geometry.buttons.length !== 6
    || geometry.devicePixelRatio !== window.devicePixelRatio || document.visibilityState !== "visible" || !document.hasFocus()
    || !Number.isFinite(geometry.scale) || geometry.scale <= 0) return false;
  const stage = geometry.stage.getBoundingClientRect();
  if (![stage.left, stage.top, stage.width, stage.height].every((value, index) => value === geometry.stageRect[index])) return false;
  return geometry.buttons.every((button, index) => {
    if (!button.isConnected || !geometry.stage.contains(button)) return false;
    const target = geometry.targets[index], r = button.getBoundingClientRect();
    return !!target && (r.left - stage.left) / geometry.scale === target.left && (r.top - stage.top) / geometry.scale === target.top
      && r.width / geometry.scale === target.width && r.height / geometry.scale === target.height;
  });
}
