import type { BevyQuestUiPresentation, BevyQuestUiSnapshot, BevyQuestUiStatus } from "./bevy-quest-ui";

export type BevyWeightBarRect = { left: number; top: number; width: number; height: number };
export type BevyWeightBarStatus = {
  supported: boolean; ready: boolean; generation: number; revision: number;
  currentWeight: number | null; maxWeight: number | null;
  slot: BevyWeightBarRect | null; image: string | null;
  source: BevyWeightBarRect | null; destination: BevyWeightBarRect | null; layout: BevyWeightBarRect | null;
};
export type WeightAuthority = { generation: number; playerObjectId: string; currentWeight: number; maxWeight: number };

const near = (a: number, b: number, tolerance = 0.75) => Number.isFinite(a) && Number.isFinite(b)
  && Math.abs(a - b) <= tolerance;
const validRect = (rect: BevyWeightBarRect | null): rect is BevyWeightBarRect => Boolean(rect
  && [rect.left, rect.top, rect.width, rect.height].every(Number.isFinite)
  && rect.width >= 0 && rect.height >= 0);
const sameRect = (a: BevyWeightBarRect, b: BevyWeightBarRect) => near(a.left, b.left)
  && near(a.top, b.top) && near(a.width, b.width) && near(a.height, b.height);

export function safeWeightPair(currentWeight: unknown, maxWeight: unknown) {
  return Number.isInteger(currentWeight) && Number.isInteger(maxWeight)
    && (currentWeight as number) >= 0 && (currentWeight as number) <= 0xffff
    && (maxWeight as number) > 0 && (maxWeight as number) <= 0xffff
    ? { currentWeight: currentWeight as number, maxWeight: maxWeight as number } : null;
}

export function weightAuthorityFromPacket(generation: number, playerObjectId: string | null,
  currentWeight: unknown, maxWeight: unknown): WeightAuthority | null {
  const pair = safeWeightPair(currentWeight, maxWeight);
  return Number.isSafeInteger(generation) && generation > 0 && playerObjectId && pair
    ? { generation, playerObjectId, ...pair } : null;
}

export function matchingWeightAuthority(authority: WeightAuthority | null, generation: number,
  playerObjectId: string | null, currentWeight: unknown, maxWeight: unknown): WeightAuthority | null {
  return authority && authority.generation === generation && authority.playerObjectId === playerObjectId
    && authority.currentWeight === currentWeight && authority.maxWeight === maxWeight ? authority : null;
}

/** Project raw packet fields without borrowing a missing half from display state. */
export function projectSnapshotWeightPair(
  packet: { currentWeight?: unknown; maxWeight?: unknown },
  authority: WeightAuthority | null,
  generation: number,
  playerObjectId: string | null,
): { authority: WeightAuthority | null; currentWeight: number; maxWeight: number } {
  const hasCurrent = Object.prototype.hasOwnProperty.call(packet, "currentWeight");
  const hasMax = Object.prototype.hasOwnProperty.call(packet, "maxWeight");
  if (!hasCurrent && !hasMax) {
    const retained = authority && authority.generation === generation
      && authority.playerObjectId === playerObjectId
      && safeWeightPair(authority.currentWeight, authority.maxWeight) ? authority : null;
    return { authority: retained, currentWeight: retained?.currentWeight ?? 0,
      maxWeight: retained?.maxWeight ?? 0 };
  }
  const complete = hasCurrent && hasMax
    ? weightAuthorityFromPacket(generation, playerObjectId, packet.currentWeight, packet.maxWeight) : null;
  // Supplied incomplete/invalid fields may still display, but never confer
  // authority or borrow a missing half from the old pair.
  const currentWeight = Number.isInteger(packet.currentWeight)
    && (packet.currentWeight as number) >= 0 && (packet.currentWeight as number) <= 0xffff
    ? packet.currentWeight as number : 0;
  const maxWeight = Number.isInteger(packet.maxWeight)
    && (packet.maxWeight as number) >= 0 && (packet.maxWeight as number) <= 0xffff
    ? packet.maxWeight as number : 0;
  return { authority: complete, currentWeight, maxWeight };
}

/** The actual movement shortcut calls this guard before it can skip React's HUD commit. */
export function movementSnapshotWeightMatches(snapshot: { currentWeight: unknown; maxWeight: unknown },
  world: { currentWeight: unknown; maxWeight: unknown }): boolean {
  return snapshot.currentWeight === world.currentWeight && snapshot.maxWeight === world.maxWeight;
}

export function readBevyWeightBarSlot(frame: HTMLElement | null, anchor: HTMLElement | null,
  presentation: BevyQuestUiPresentation | null): BevyWeightBarRect | null {
  if (!frame || !anchor || !presentation || !frame.contains(anchor) || !anchor.isConnected) return null;
  const { logicalWidth, logicalHeight, stageCssScale } = presentation;
  if (!Number.isFinite(stageCssScale) || stageCssScale <= 0) return null;
  const stage = frame.getBoundingClientRect();
  const full = anchor.getBoundingClientRect();
  if (![stage.width, stage.height, full.left, full.top, full.width, full.height].every(Number.isFinite)
    || !near(stage.width, logicalWidth * stageCssScale)
    || !near(stage.height, logicalHeight * stageCssScale)
    || !near(full.width / full.height, 76 / 12, 1)) return null;
  const slot = { left: (full.left - stage.left) / stageCssScale,
    top: (full.top - stage.top) / stageCssScale,
    width: full.width / stageCssScale, height: full.height / stageCssScale };
  return validRect(slot) && slot.left >= 0 && slot.top >= 0 && slot.width > 0 && slot.height > 0
    && slot.left + slot.width <= logicalWidth + 0.75 && slot.top + slot.height <= logicalHeight + 0.75
    ? slot : null;
}

export function supportsBevyWeightBar(runtime: { getMir2QuestUiStatus?: () => string } | null): boolean {
  if (!runtime?.getMir2QuestUiStatus) return false;
  try { return JSON.parse(runtime.getMir2QuestUiStatus())?.weightBar?.supported === true; }
  catch { return false; }
}

/** Older deny-unknown runtimes must receive neither nested values nor the slot. */
export function stripUnsupportedWeightBar<T extends {
  player: { currentWeight?: number | null; maxWeight?: number | null };
  weightBarSlot?: BevyWeightBarRect | null;
}>(snapshot: T, supported: boolean): T {
  if (supported) return snapshot;
  const player = { ...snapshot.player };
  delete player.currentWeight;
  delete player.maxWeight;
  const legacy = { ...snapshot, player };
  delete legacy.weightBarSlot;
  return legacy;
}

const selectedImage = (ratio: number) => ratio <= 0.5 ? "original-ui/Prguse/76.png"
  : ratio <= 0.75 ? "original-ui/UI_32bit/473.png" : "original-ui/UI_32bit/472.png";

export function currentBevyWeightBar(status: BevyQuestUiStatus | null,
  snapshot: BevyQuestUiSnapshot | null, frameFresh: boolean): BevyWeightBarStatus | null {
  const bar = status?.weightBar;
  const slot = snapshot?.weightBarSlot;
  const pair = safeWeightPair(snapshot?.player.currentWeight, snapshot?.player.maxWeight);
  if (!frameFresh || !status || !snapshot || !bar || !bar.supported || !bar.ready
    || !snapshot.inGame || !snapshot.hostVisible || !snapshot.presentation || !slot || !pair
    || status.generation !== snapshot.generation || bar.generation !== snapshot.generation
    || bar.revision !== snapshot.revision || status.revision < snapshot.revision
    || bar.currentWeight !== pair.currentWeight || bar.maxWeight !== pair.maxWeight
    || !validRect(slot) || !validRect(bar.slot) || !sameRect(bar.slot, slot)
    || !validRect(bar.source) || !validRect(bar.destination) || !validRect(bar.layout)
    || !Number.isFinite(snapshot.presentation.logicalWidth)
    || !Number.isFinite(snapshot.presentation.logicalHeight)
    || !Number.isFinite(snapshot.presentation.stageCssScale)
    || snapshot.presentation.logicalWidth <= 0 || snapshot.presentation.logicalHeight <= 0
    || snapshot.presentation.logicalWidth > 16_384 || snapshot.presentation.logicalHeight > 16_384
    || snapshot.presentation.stageCssScale <= 0 || snapshot.presentation.stageCssScale > 16
    || slot.left < 0 || slot.top < 0 || slot.width <= 0 || slot.height <= 0
    || Math.abs(slot.width / slot.height - 76 / 12) > 1
    || slot.left + slot.width > snapshot.presentation.logicalWidth + 0.75
    || slot.top + slot.height > snapshot.presentation.logicalHeight + 0.75
    || !near(bar.source.left, 0) || !near(bar.source.top, 0) || !near(bar.source.height, 12)
    || !near(bar.destination.left, slot.left) || !near(bar.destination.top, slot.top)
    || !near(bar.destination.height, slot.height) || !sameRect(bar.destination, bar.layout)) return null;
  const ratio = Math.fround(Math.max(0, Math.min(1, pair.currentWeight / pair.maxWeight)));
  const width = Math.floor(Math.fround(74 * ratio));
  return bar.image === selectedImage(ratio) && near(bar.source.width, width)
    && near(bar.destination.width, width * slot.width / 76) ? bar : null;
}

export function matchesBevyWeightBarView(bar: BevyWeightBarStatus | null, slot: BevyWeightBarRect | null,
  currentWeight: unknown, maxWeight: unknown, liveAnchor?: Pick<HTMLElement, "getAttribute"> | null): boolean {
  const rawCurrent = liveAnchor?.getAttribute("data-current-weight");
  const rawMax = liveAnchor?.getAttribute("data-max-weight");
  const livePair = rawCurrent !== undefined && rawCurrent !== null && rawMax !== undefined && rawMax !== null
    && /^\d+$/.test(rawCurrent) && /^\d+$/.test(rawMax)
    ? safeWeightPair(Number(rawCurrent), Number(rawMax)) : null;
  return Boolean(bar && bar.ready && slot && bar.slot && safeWeightPair(currentWeight, maxWeight)
    && (!liveAnchor || (livePair && livePair.currentWeight === currentWeight && livePair.maxWeight === maxWeight))
    && bar.currentWeight === currentWeight && bar.maxWeight === maxWeight && sameRect(bar.slot, slot));
}
