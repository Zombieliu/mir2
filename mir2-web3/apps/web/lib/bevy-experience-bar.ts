import type { BevyQuestUiPresentation, BevyQuestUiSnapshot, BevyQuestUiStatus } from "./bevy-quest-ui";

export type BevyExperienceBarRect = { left: number; top: number; width: number; height: number };
export type BevyExperienceBarStatus = {
  supported: boolean;
  ready: boolean;
  generation: number;
  revision: number;
  experience: number | null;
  maxExperience: number | null;
  slot: BevyExperienceBarRect | null;
  image: string | null;
  source: BevyExperienceBarRect | null;
  destination: BevyExperienceBarRect | null;
  layout: BevyExperienceBarRect | null;
};

const near = (a: number, b: number, epsilon = 0.75) => Number.isFinite(a) && Number.isFinite(b) && Math.abs(a - b) <= epsilon;
const validRect = (rect: BevyExperienceBarRect | null): rect is BevyExperienceBarRect => Boolean(rect
  && [rect.left, rect.top, rect.width, rect.height].every(Number.isFinite)
  && rect.width >= 0 && rect.height >= 0);
const sameRect = (a: BevyExperienceBarRect, b: BevyExperienceBarRect) => near(a.left, b.left)
  && near(a.top, b.top) && near(a.width, b.width) && near(a.height, b.height);

/** Keep the full 1004×8 anchor measured even when its image crop is empty. */
export function readBevyExperienceBarSlot(
  frame: HTMLElement | null,
  anchor: HTMLElement | null,
  presentation: BevyQuestUiPresentation | null,
): BevyExperienceBarRect | null {
  if (!frame || !anchor || !presentation || !frame.contains(anchor) || !anchor.isConnected) return null;
  const { logicalWidth, logicalHeight, stageCssScale } = presentation;
  if (!Number.isFinite(stageCssScale) || stageCssScale <= 0) return null;
  const stage = frame.getBoundingClientRect();
  const full = anchor.getBoundingClientRect();
  if (!near(stage.width, logicalWidth * stageCssScale)
    || !near(stage.height, logicalHeight * stageCssScale)
    || !near(full.width / full.height, 1004 / 8, 1)
    || ![full.left, full.top, full.width, full.height].every(Number.isFinite)) return null;
  const slot = { left: (full.left - stage.left) / stageCssScale,
    top: (full.top - stage.top) / stageCssScale,
    width: full.width / stageCssScale, height: full.height / stageCssScale };
  if (!validRect(slot) || slot.width <= 0 || slot.height <= 0
    || slot.left < 0 || slot.top < 0
    || slot.left + slot.width > logicalWidth + 0.75
    || slot.top + slot.height > logicalHeight + 0.75) return null;
  return slot;
}

export function safeExperiencePair(experience: unknown, maxExperience: unknown):
  { experience: number; maxExperience: number } | null {
  return Number.isSafeInteger(experience) && Number.isSafeInteger(maxExperience)
    ? { experience: experience as number, maxExperience: maxExperience as number } : null;
}

export type ExperienceAuthority = { generation: number; playerObjectId: string;
  experience: number; maxExperience: number };

/** Packet provenance is separate from the compatibility world's display defaults. */
export function experienceAuthorityFromPacket(
  generation: number, playerObjectId: string | null, experience: unknown, maxExperience: unknown,
): ExperienceAuthority | null {
  const pair = safeExperiencePair(experience, maxExperience);
  return Number.isSafeInteger(generation) && generation > 0 && playerObjectId && pair && pair.maxExperience > 0
    ? { generation, playerObjectId, ...pair } : null;
}

export function matchingExperienceAuthority(
  authority: ExperienceAuthority | null, generation: number, playerObjectId: string | null,
  experience: unknown, maxExperience: unknown,
): ExperienceAuthority | null {
  return authority && authority.generation === generation && authority.playerObjectId === playerObjectId
    && authority.experience === experience && authority.maxExperience === maxExperience ? authority : null;
}

export function advanceExperienceAuthority(
  authority: ExperienceAuthority | null, generation: number, playerObjectId: string | null,
  currentExperience: unknown, currentMaxExperience: unknown, rawAmount: unknown,
): ExperienceAuthority | null {
  const matched = matchingExperienceAuthority(authority, generation, playerObjectId,
    currentExperience, currentMaxExperience);
  if (!matched || !Number.isSafeInteger(rawAmount) || (rawAmount as number) <= 0) return null;
  const next = matched.experience + (rawAmount as number);
  return Number.isSafeInteger(next) ? { ...matched, experience: next } : null;
}

export function supportsBevyExperienceBar(runtime: { getMir2QuestUiStatus?: () => string } | null): boolean {
  if (!runtime?.getMir2QuestUiStatus) return false;
  try { return JSON.parse(runtime.getMir2QuestUiStatus())?.experienceBar?.supported === true; }
  catch { return false; }
}

/** Unknown fields must be absent in the JSON sent to an older deny-unknown runtime. */
export function stripUnsupportedExperienceBar<T extends {
  player: { experience?: number | null; maxExperience?: number | null };
  experienceBarSlot?: BevyExperienceBarRect | null;
}>(snapshot: T, supported: boolean): T {
  if (supported) return snapshot;
  const player = { ...snapshot.player };
  delete player.experience;
  delete player.maxExperience;
  const legacy = { ...snapshot, player };
  delete legacy.experienceBarSlot;
  return legacy;
}

/** Require the actual painter's current model, crop, destination and post-layout node. */
export function currentBevyExperienceBar(
  status: BevyQuestUiStatus | null,
  snapshot: BevyQuestUiSnapshot | null,
  frameFresh: boolean,
): BevyExperienceBarStatus | null {
  const bar = status?.experienceBar;
  const slot = snapshot?.experienceBarSlot;
  const player = snapshot?.player;
  if (!frameFresh || !status || !snapshot || !bar || bar.supported !== true || bar.ready !== true
    || !snapshot.inGame || !snapshot.hostVisible || !snapshot.presentation || !slot || !player
    || status.generation !== snapshot.generation || bar.generation !== snapshot.generation
    || bar.revision !== snapshot.revision || status.revision < snapshot.revision
    || !safeExperiencePair(player.experience, player.maxExperience)
    || (player.maxExperience ?? 0) <= 0 || bar.experience !== player.experience
    || bar.maxExperience !== player.maxExperience || bar.image !== "original-ui/Prguse/8.png"
    || !validRect(slot) || !validRect(bar.slot) || !sameRect(bar.slot, slot)
    || !Number.isFinite(snapshot.presentation.logicalWidth)
    || !Number.isFinite(snapshot.presentation.logicalHeight)
    || !Number.isFinite(snapshot.presentation.stageCssScale)
    || snapshot.presentation.logicalWidth <= 0 || snapshot.presentation.logicalHeight <= 0
    || snapshot.presentation.logicalWidth > 16_384 || snapshot.presentation.logicalHeight > 16_384
    || snapshot.presentation.stageCssScale <= 0 || snapshot.presentation.stageCssScale > 16
    || !validRect(bar.source) || !validRect(bar.destination) || !validRect(bar.layout)
    || !near(bar.source.left, 0) || !near(bar.source.top, 0) || !near(bar.source.height, 8)
    || !near(bar.destination.left, slot.left) || !near(bar.destination.top, slot.top)
    || !near(bar.destination.height, slot.height) || !sameRect(bar.destination, bar.layout)
    || slot.left < 0 || slot.top < 0 || slot.width <= 0 || slot.height <= 0
    || Math.abs(slot.width / slot.height - 1004 / 8) > 1
    || slot.left + slot.width > snapshot.presentation.logicalWidth + 0.75
    || slot.top + slot.height > snapshot.presentation.logicalHeight + 0.75) return null;
  // PlayerStats::normalized_experience calculates an f64 ratio, then returns f32.
  const ratio = Math.fround(Math.max(0, Math.min(1, (player.experience as number) / (player.maxExperience as number))));
  const width = Math.floor(Math.fround(1001 * ratio));
  if (!near(bar.source.width, width)
    || !near(bar.destination.width, width * (slot.width / 1004))) return null;
  return bar;
}

/** The visible React HUD must still describe the same exact authoritative pair. */
export function matchesBevyExperienceBarView(
  bar: BevyExperienceBarStatus | null,
  slot: BevyExperienceBarRect | null,
  experience: unknown,
  maxExperience: unknown,
  liveAnchor?: Pick<HTMLElement, "getAttribute"> | null,
): boolean {
  const rawExperience = liveAnchor?.getAttribute("data-experience");
  const rawMaxExperience = liveAnchor?.getAttribute("data-max-experience");
  const livePair = rawExperience !== undefined && rawMaxExperience !== undefined
    && rawExperience !== null && rawMaxExperience !== null
    && /^-?\d+$/.test(rawExperience) && /^-?\d+$/.test(rawMaxExperience)
    ? safeExperiencePair(Number(rawExperience), Number(rawMaxExperience)) : null;
  return Boolean(bar && slot && bar.ready && bar.slot && safeExperiencePair(experience, maxExperience)
    && (!liveAnchor || (livePair && livePair.experience === experience && livePair.maxExperience === maxExperience))
    && (maxExperience as number) > 0 && bar.experience === experience && bar.maxExperience === maxExperience
    && sameRect(bar.slot, slot));
}
