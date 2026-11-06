import type { BevyQuestUiPresentation, BevyQuestUiSnapshot, BevyQuestUiStatus } from "./bevy-quest-ui";

export type BevyHpOrbSlot = { left: number; top: number };
export type BevyHpOrbRect = BevyHpOrbSlot & { width: number; height: number };
export type BevyHpOrbStatus = {
  supported: boolean;
  ready: boolean;
  generation: number;
  revision: number;
  hp: number | null;
  maxHp: number | null;
  hpOnly: boolean | null;
  slot: BevyHpOrbSlot | null;
  image: string | null;
  source: BevyHpOrbRect | null;
  destination: BevyHpOrbRect | null;
  layout: BevyHpOrbRect | null;
};
export type BevyMpOrbStatus = {
  supported: boolean;
  ready: boolean;
  generation: number;
  revision: number;
  mp: number | null;
  maxMp: number | null;
  hpOnly: boolean | null;
  slot: BevyHpOrbSlot | null;
  image: string | null;
  source: BevyHpOrbRect | null;
  destination: BevyHpOrbRect | null;
  layout: BevyHpOrbRect | null;
};

const near = (a: number, b: number, epsilon = 0.75) => Number.isFinite(a) && Number.isFinite(b) && Math.abs(a - b) <= epsilon;
const validRect = (rect: BevyHpOrbRect | null): rect is BevyHpOrbRect => Boolean(rect
  && Number.isFinite(rect.left) && Number.isFinite(rect.top)
  && Number.isFinite(rect.width) && Number.isFinite(rect.height)
  && rect.width >= 0 && rect.height >= 0);

/** The 104×80 image keeps the full-orb origin while its clip moves downward. */
export function readBevyHpOrbSlot(
  frame: HTMLElement | null,
  image: HTMLElement | null,
  presentation: BevyQuestUiPresentation | null,
): BevyHpOrbSlot | null {
  if (!frame || !image || !presentation || !frame.contains(image) || !image.isConnected) return null;
  const { logicalWidth, logicalHeight, stageCssScale } = presentation;
  if (!Number.isFinite(stageCssScale) || stageCssScale <= 0) return null;
  const stage = frame.getBoundingClientRect();
  const fullImage = image.getBoundingClientRect();
  if (!near(stage.width, logicalWidth * stageCssScale) || !near(stage.height, logicalHeight * stageCssScale)
    || !Number.isFinite(fullImage.left) || !Number.isFinite(fullImage.top)
    || !near(fullImage.width, 104 * stageCssScale)
    || !near(fullImage.height, 80 * stageCssScale)) return null;
  const left = (fullImage.left - stage.left) / stageCssScale;
  const top = (fullImage.top - stage.top) / stageCssScale;
  if (!Number.isFinite(left) || !Number.isFinite(top) || left < 0 || top < 0
    || left + 104 > logicalWidth + 0.75 || top + 80 > logicalHeight + 0.75) return null;
  return { left, top };
}

/** Probe support without assuming the old runtime accepts an extra snapshot field. */
export function supportsBevyHpOrb(runtime: { getMir2QuestUiStatus?: () => string } | null): boolean {
  if (!runtime?.getMir2QuestUiStatus) return false;
  try {
    const status = JSON.parse(runtime.getMir2QuestUiStatus());
    return status?.hpOrb?.supported === true;
  } catch { return false; }
}

/** Older runtimes reject nested MP player fields, including explicit nulls. */
export function supportsBevyMpOrb(runtime: { getMir2QuestUiStatus?: () => string } | null): boolean {
  if (!runtime?.getMir2QuestUiStatus) return false;
  try {
    const status = JSON.parse(runtime.getMir2QuestUiStatus());
    return status?.mpOrb?.supported === true;
  } catch { return false; }
}

/** Image ownership requires the painter's current acknowledged model and measured node. */
export function currentBevyHpOrb(
  status: BevyQuestUiStatus | null,
  snapshot: BevyQuestUiSnapshot | null,
  frameFresh: boolean,
): BevyHpOrbStatus | null {
  const orb = status?.hpOrb;
  const slot = snapshot?.hpOrbSlot;
  const player = snapshot?.player;
  if (!frameFresh || !status || !snapshot || !orb || !orb.supported || !orb.ready
    || !snapshot.inGame || !snapshot.presentation || !slot || !player
    || status.generation !== snapshot.generation || orb.generation !== snapshot.generation
    || orb.revision !== snapshot.revision || status.revision < snapshot.revision
    || !Number.isFinite(slot.left) || !Number.isFinite(slot.top)
    || !orb.slot || !near(orb.slot.left, slot.left) || !near(orb.slot.top, slot.top)
    || !Number.isSafeInteger(player.hp) || !Number.isSafeInteger(player.maxHp)
    || player.maxHp <= 0 || player.hp < 0 || !Number.isSafeInteger(player.level)
    || orb.hp !== player.hp || orb.maxHp !== player.maxHp
    || orb.hpOnly !== (player.className?.toLowerCase() === "warrior" && player.level < 26)
    || orb.image !== (orb.hpOnly ? "original-ui/Prguse/6.png" : "original-ui/Prguse/4.png")
    || !validRect(orb.source) || !validRect(orb.destination) || !validRect(orb.layout)
    || orb.source.width <= 0 || orb.destination.width <= 0 || orb.layout.width <= 0
    || !near(orb.destination.left, orb.layout.left) || !near(orb.destination.top, orb.layout.top)
    || !near(orb.destination.width, orb.layout.width) || !near(orb.destination.height, orb.layout.height)
    || orb.destination.left < 0 || orb.destination.top < 0
    || orb.destination.left + orb.destination.width > snapshot.presentation.logicalWidth + 0.75
    || orb.destination.top + orb.destination.height > snapshot.presentation.logicalHeight + 0.75) return null;
  return orb;
}

/** Recheck the rendered HUD's live values before yielding its React image. */
export function matchesBevyHpOrbView(
  orb: BevyHpOrbStatus | null,
  slot: BevyHpOrbSlot | null,
  hp: number,
  maxHp: number,
  hpOnly: boolean,
): boolean {
  return Boolean(orb && slot && orb.ready && orb.slot
    && orb.hp === hp && orb.maxHp === maxHp && orb.hpOnly === hpOnly
    && near(orb.slot.left, slot.left) && near(orb.slot.top, slot.top));
}

/** MP ownership is independent of HP and of the Quest panel's ready flag. */
export function currentBevyMpOrb(
  status: BevyQuestUiStatus | null,
  snapshot: BevyQuestUiSnapshot | null,
  frameFresh: boolean,
): BevyMpOrbStatus | null {
  const orb = status?.mpOrb;
  const slot = snapshot?.hpOrbSlot;
  const player = snapshot?.player;
  if (!frameFresh || !status || !snapshot || !orb || orb.supported !== true || orb.ready !== true
    || !snapshot.inGame || !snapshot.presentation || !slot || !player
    || status.generation !== snapshot.generation || orb.generation !== snapshot.generation
    || orb.revision !== snapshot.revision || status.revision < snapshot.revision
    || !Number.isFinite(slot.left) || !Number.isFinite(slot.top)
    || slot.left < 0 || slot.top < 0
    || slot.left + 104 > snapshot.presentation.logicalWidth + 0.75
    || slot.top + 80 > snapshot.presentation.logicalHeight + 0.75
    || !orb.slot || !near(orb.slot.left, slot.left) || !near(orb.slot.top, slot.top)
    || !Number.isSafeInteger(player.mp) || !Number.isSafeInteger(player.maxMp)
    || (player.mp ?? -1) < 0 || (player.maxMp ?? 0) <= 0
    || !Number.isSafeInteger(player.level) || player.level < 1
    || typeof player.className !== "string" || player.className.length === 0
    || (player.className.toLowerCase() === "warrior" && player.level < 26)
    || orb.mp !== player.mp || orb.maxMp !== player.maxMp || orb.hpOnly !== false
    || orb.image !== "original-ui/Prguse/4.png"
    || !validRect(orb.source) || !validRect(orb.destination) || !validRect(orb.layout)
    || !near(orb.source.left, 51) || !near(orb.source.width, 50)
    || !near(orb.destination.left, slot.left + 51) || !near(orb.destination.width, 50)
    || !near(orb.destination.left, orb.layout.left) || !near(orb.destination.top, orb.layout.top)
    || !near(orb.destination.width, orb.layout.width) || !near(orb.destination.height, orb.layout.height)
    || orb.destination.left < 0 || orb.destination.top < 0
    || orb.destination.left + orb.destination.width > snapshot.presentation.logicalWidth + 0.75
    || orb.destination.top + orb.destination.height > snapshot.presentation.logicalHeight + 0.75) return null;
  // UiReadModel::normalized_mp and Crystal's crop both use f32 arithmetic.
  const expectedHeight = Math.floor(Math.fround(80 * Math.min(1,
    Math.fround(Math.fround(player.mp as number) / Math.fround(player.maxMp as number)))));
  if (!near(orb.source.top, 80 - expectedHeight)
    || !near(orb.source.height, expectedHeight)
    || !near(orb.destination.top, slot.top + 80 - expectedHeight)
    || !near(orb.destination.height, expectedHeight)) return null;
  return orb;
}

/** The still-rendered DOM MP label/model must match before hiding only its image. */
export function matchesBevyMpOrbView(
  orb: BevyMpOrbStatus | null,
  slot: BevyHpOrbSlot | null,
  mp: number | null | undefined,
  maxMp: number | null | undefined,
  hpOnly: boolean,
): boolean {
  return Boolean(orb && slot && orb.ready && orb.slot && !hpOnly && orb.hpOnly === false
    && Number.isSafeInteger(mp) && Number.isSafeInteger(maxMp) && (mp ?? -1) >= 0 && (maxMp ?? 0) > 0
    && orb.mp === mp && orb.maxMp === maxMp
    && near(orb.slot.left, slot.left) && near(orb.slot.top, slot.top));
}
