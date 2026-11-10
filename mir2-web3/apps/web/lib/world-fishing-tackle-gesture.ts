import type { InventoryTabKey } from "./original-ui";

type BagTab = Extract<InventoryTabKey, "bag1" | "bag2">;
type Cell = Readonly<{ kind: "bag" | "tackle"; slot: number; node: Element }>;
type TargetEvent = { currentTarget: HTMLElement; target: EventTarget | null; stopPropagation: () => void };
type PointerEdge = TargetEvent & Pick<PointerEvent,
  "pointerId" | "isPrimary" | "button" | "timeStamp" | "clientX" | "clientY" | "defaultPrevented" | "preventDefault">;
type FishingUi = Readonly<{
  onBagCell: (tab: InventoryTabKey, slot: number) => boolean;
  onTackleClick: (slot: number) => void;
  retainTackleHold: (kind: Cell["kind"], slot: number) => (() => boolean) | null;
}>;
type Press = { root: HTMLElement; pointerId: number; at: number; start: Cell; tab: BagTab | null; held: (() => boolean) | null };

type SpriteSize = Readonly<{ width: number; height: number }>;

/** Crystal GetTrueSize ignores the transparent padding of the Items frame. */
export function fishingItemVisibleSize(pixels: Uint8ClampedArray, width: number, height: number): SpriteSize {
  let left = width, top = height, right = -1, bottom = -1;
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      if (!pixels[4 * (y * width + x) + 3]) continue;
      left = Math.min(left, x);
      top = Math.min(top, y);
      right = Math.max(right, x);
      bottom = Math.max(bottom, y);
    }
  }
  return { width: Math.max(0, right - left + 1), height: Math.max(0, bottom - top + 1) };
}

export function fishingItemCursorOrigin(x: number, y: number, sprite: SpriteSize, screen: SpriteSize) {
  return {
    left: Math.min(Math.floor(x) - Math.floor(sprite.width / 2), screen.width - sprite.width),
    top: Math.min(Math.floor(y) - Math.floor(sprite.height / 2), screen.height - sprite.height),
  };
}

function fishingCell(root: HTMLElement, target: EventTarget | null, tab: BagTab | null): Cell | null {
  const ElementType = root.ownerDocument.defaultView?.Element;
  if (!ElementType || !(target instanceof ElementType)) return null;
  const node = target.closest("[data-fishing-slot], [data-fishing-bag-item], [data-fishing-bag-cell]");
  if (!node || !root.contains(node) || node.matches(":disabled")) return null;
  const tackle = node.getAttribute("data-fishing-slot");
  const slot = tackle ?? node.getAttribute("data-fishing-bag-item") ?? node.getAttribute("data-fishing-bag-cell");
  if (slot === null || !/^(0|[1-9][0-9]?)$/.test(slot)) return null;
  if (tackle !== null) return Number(slot) < 5 ? { kind: "tackle", slot: Number(slot), node } : null;
  return tab && Number(slot) < 40 ? { kind: "bag", slot: Number(slot), node } : null;
}

function activate(ui: FishingUi, cell: Cell, tab: BagTab | null) {
  if (cell.kind === "tackle") ui.onTackleClick(cell.slot);
  else if (tab) ui.onBagCell(tab, cell.slot);
}

/** One physical left-button press across both foreground windows. Page owns the item and its receipt. */
export function createFishingTackleGesture() {
  let press: Press | null = null;

  function cancel(pointerId?: number) {
    if (!press || pointerId !== undefined && press.pointerId !== pointerId) return;
    const { root, pointerId: id } = press;
    press = null;
    if (root.hasPointerCapture(id)) root.releasePointerCapture(id);
  }

  function down(event: PointerEdge, ui: FishingUi, tab: BagTab | null) {
    if (press || event.defaultPrevented || !event.isPrimary || event.button !== 0) return;
    const root = event.currentTarget;
    const cell = fishingCell(root, event.target, tab);
    if (!cell || fishingCell(root, root.ownerDocument.elementFromPoint(event.clientX, event.clientY), tab)?.node !== cell.node) return;
    event.preventDefault();
    // Pointer admission replaces the compatibility mouse down that focuses the game stage.
    root.closest<HTMLElement>(".client-stage-frame")?.focus({ preventScroll: true });
    activate(ui, cell, tab);
    const slot = cell.slot + (cell.kind === "bag" && tab === "bag2" ? 40 : 0);
    const held = ui.retainTackleHold(cell.kind, slot);
    try { root.setPointerCapture(event.pointerId); } catch { return; }
    press = { root, pointerId: event.pointerId, at: event.timeStamp, start: cell, tab, held };
  }

  function up(event: PointerEdge, ui: FishingUi | null, tab: BagTab | null) {
    const active = press;
    if (!active || active.pointerId !== event.pointerId || active.root !== event.currentTarget || event.timeStamp < active.at) return;
    cancel(event.pointerId);
    event.preventDefault();
    event.stopPropagation();
    if (!ui || !event.isPrimary || event.button !== 0 || tab !== active.tab || !active.root.contains(active.start.node)) return;
    const target = fishingCell(active.root, active.root.ownerDocument.elementFromPoint(event.clientX, event.clientY), tab);
    if (target && (target.kind !== active.start.kind || target.slot !== active.start.slot) && active.held?.()) {
      activate(ui, target, tab);
    }
  }

  function suppressClick(event: TargetEvent & Pick<MouseEvent, "button" | "detail" | "preventDefault">, tab: BagTab | null) {
    if (event.button === 0 && event.detail > 0 && fishingCell(event.currentTarget, event.target, tab)) {
      event.preventDefault();
      event.stopPropagation();
    }
  }

  return { down, up, cancel, suppressClick };
}
