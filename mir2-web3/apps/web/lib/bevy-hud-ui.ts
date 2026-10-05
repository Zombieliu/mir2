/** Independent HUD ABI. Strict old Quest snapshots are never extended. */
export type CrystalPlayerStat = { stat: number; value: number };
export type PlayerWeights = { bag: number; wear: number; hand: number };
export type HudPage = "character" | "stats1" | "stats2" | "spells";
export type HudNavigation = { characterOpen: boolean; characterPage: HudPage; bagOpen: boolean; questOpen: boolean };
export type HudAction = { type: "character" | "bag" | "quest" | "closeCharacter" | "closeBag" | "openBag" | "closeQuest" | "openQuest" }
  | { type: "selectCharacterPage"; page: HudPage };
export type HudRect = { left: number; top: number; width: number; height: number };
export type HudButton = { action: "character" | "bag" | "skill" | "quest" | "option" | "menu" | "gameShop";
  rect: HudRect; normal: string; hover: string; pressed: string };
export type HudSpritePlan = { current: number; maximum: number; image: string | null; source: HudRect; destination: HudRect };
export type MainHudPlan = { hp: string; mp: string; name: string; level: string; gold: string; experience: string; weight: string;
  hpOnly: boolean; main: HudRect; orb: HudRect; experienceBar: HudRect; weightBar: HudRect; buttons: HudButton[];
  characterRect: HudRect; characterHits: Array<{ action: HudAction; rect: HudRect }>;
  experienceSprite: HudSpritePlan | null; weightSprite: HudSpritePlan | null };
export type HudPlayer = { hp: number; maxHp: number; mp: number; maxMp: number; level: number; gold: number; credit: number;
  name?: string; className?: string; gender?: string; hair?: number; wingEffect?: number; guildName?: string; guildRankName?: string; experience: number; maxExperience: number;
  currentWeight: number; currentWeightKnown: boolean; maxWeight: number; crystalStats?: CrystalPlayerStat[]; weights?: PlayerWeights };
export type HudSnapshot = { generation: number; revision: number; inGame: boolean; hostVisible: boolean;
  player: HudPlayer | null; navigation: HudNavigation; navigationRevision?: number; logicalWidth: number; logicalHeight: number; stageCssScale: number; touch: boolean };
export type HudStatus = { version: 1; frame: number; ready: boolean; characterStatsReady: boolean; generation: number; revision: number;
  navigation: HudNavigation; plan: MainHudPlan | null; error: string | null; foregroundRects: HudRect[]; modal: boolean };
export type HudRuntime = { getMir2HudUiCapabilities?: () => string; getMir2HudSourceGeometry?: () => string; setMir2HudUiSnapshot?: (json: string) => boolean;
  getMir2HudUiStatus?: () => string; dispatchMir2HudNavigation?: (json: string) => boolean };
const unsigned = (value: unknown, max = 0xffff_ffff): value is number => Number.isSafeInteger(value) && Number(value) >= 0 && Number(value) <= max;
/** Explicit compatibility navigation edits share the page lifetime across HMR. */
export function nextHudNavigationRevision() {
  const scope = globalThis as typeof globalThis & { __mir2HudNavigationRevision?: number };
  const next = (scope.__mir2HudNavigationRevision ?? 0) + 1;
  if (!Number.isSafeInteger(next)) throw Error("HUD navigation revision exhausted");
  return scope.__mir2HudNavigationRevision = next;
}
export function readHudCapabilities(runtime: HudRuntime | null) {
  try {
    const value = JSON.parse(runtime?.getMir2HudUiCapabilities?.() ?? "null");
    if (!value || Object.keys(value).sort().join() !== "characterStatsAbiVersion,compiled,hudUiAbiVersion,schemaVersion,startup"
      || value.schemaVersion !== 1 || ![0, 1].includes(value.hudUiAbiVersion) || value.characterStatsAbiVersion !== value.hudUiAbiVersion
      || typeof value.compiled !== "boolean" || typeof value.startup !== "boolean" || value.compiled !== (value.hudUiAbiVersion === 1)
      || value.startup && !value.compiled) return null;
    return value as { schemaVersion: 1; hudUiAbiVersion: 0 | 1; characterStatsAbiVersion: 0 | 1; compiled: boolean; startup: boolean };
  } catch { return null; }
}
export function supportsHud(runtime: HudRuntime | null) {
  const capability = readHudCapabilities(runtime);
  return Boolean(capability?.compiled && capability.startup && runtime?.setMir2HudUiSnapshot && runtime.getMir2HudUiStatus && runtime.dispatchMir2HudNavigation && runtime.getMir2HudSourceGeometry);
}
export function readHudGeometry(runtime: HudRuntime | null): MainHudPlan | null {
  try {
    if (!supportsHud(runtime)) return null;
    const geometry = JSON.parse(runtime?.getMir2HudSourceGeometry?.() ?? "null");
    return geometry && [geometry.main, geometry.orb, geometry.experienceBar, geometry.weightBar, geometry.characterRect].every(validRect)
      && Array.isArray(geometry.buttons) && Array.isArray(geometry.characterHits) ? geometry : null;
  } catch { return null; }
}
export function projectAuthoritativeHudPlayer(world: { playerHp?: number; playerMaxHp?: number; playerMp?: number; playerMaxMp?: number;
  gold: number; credit: number; playerExperience: number; playerMaxExperience: number; currentWeight: number; maxWeight: number;
  playerCrystalStats?: CrystalPlayerStat[] | null; playerWeights?: PlayerWeights | null; stage5Systems?: { guild?: { name?: string; rank?: string } } },
  player: { name: string; level?: number; classKey?: string; genderKey?: string; hair?: number; wingEffect?: number } | null): HudPlayer | null {
  if (!player || ![world.playerHp, world.playerMaxHp, world.playerMp, world.playerMaxMp].every(value => Number.isInteger(value)
    && Number(value) >= -0x8000_0000 && Number(value) <= 0x7fff_ffff)
    || !unsigned(world.gold) || !unsigned(world.credit) || !unsigned(player.level)
    || ![world.playerExperience, world.playerMaxExperience].every(value => Number.isSafeInteger(value) && value >= 0)) return null;
  const stats = world.playerCrystalStats;
  if (stats !== null && stats !== undefined && (!Array.isArray(stats) || stats.some(row => !unsigned(row.stat, 255)
    || !Number.isInteger(row.value) || row.value < -0x8000_0000 || row.value > 0x7fff_ffff))) return null;
  const weights = world.playerWeights;
  if (weights !== null && weights !== undefined && ![weights.bag, weights.wear, weights.hand].every(value => unsigned(value))) return null;
  return { hp: world.playerHp!, maxHp: world.playerMaxHp!, mp: world.playerMp!, maxMp: world.playerMaxMp!,
    name: player.name, level: player.level!, className: player.classKey, gender: player.genderKey, hair: player.hair, wingEffect: player.wingEffect,
    guildName: world.stage5Systems?.guild?.name, guildRankName: world.stage5Systems?.guild?.rank,
    gold: world.gold, credit: world.credit, experience: world.playerExperience, maxExperience: world.playerMaxExperience,
    currentWeight: unsigned(world.currentWeight, 65535) ? world.currentWeight : 0,
    currentWeightKnown: false, maxWeight: unsigned(world.maxWeight, 65535) ? world.maxWeight : 0,
    ...(stats == null ? {} : { crystalStats: stats.map(row => ({ ...row })) }),
    ...(weights == null ? {} : { weights: { ...weights } }) };
}
export function projectHudSnapshotFields(snapshot: { playerCrystalStats?: CrystalPlayerStat[] | null; playerWeights?: PlayerWeights | null }) {
  return { playerCrystalStats: snapshot.playerCrystalStats ?? undefined, playerWeights: snapshot.playerWeights ?? undefined };
}
export function sameHudSnapshotFields(snapshot: { playerCrystalStats?: CrystalPlayerStat[] | null; playerWeights?: PlayerWeights | null },
  current: { playerCrystalStats?: CrystalPlayerStat[] | null; playerWeights?: PlayerWeights | null }) {
  return JSON.stringify(projectHudSnapshotFields(snapshot)) === JSON.stringify(projectHudSnapshotFields(current));
}
export function readHudStatus(runtime: HudRuntime | null, generation: number): HudStatus | null {
  try {
    const s = JSON.parse(runtime?.getMir2HudUiStatus?.() ?? "null");
    if (!s || s.version !== 1 || s.generation !== generation || !unsigned(s.frame, Number.MAX_SAFE_INTEGER)
      || !unsigned(s.revision, Number.MAX_SAFE_INTEGER) || typeof s.ready !== "boolean" || typeof s.characterStatsReady !== "boolean"
      || !s.navigation || ![s.navigation.characterOpen, s.navigation.bagOpen, s.navigation.questOpen].every(v => typeof v === "boolean")
      || typeof s.modal !== "boolean" || !Array.isArray(s.foregroundRects) || !s.foregroundRects.every(validRect)
      || !["character", "stats1", "stats2", "spells"].includes(s.navigation.characterPage)) return null;
    if (s.ready && (!s.plan || !Array.isArray(s.plan.buttons) || s.plan.buttons.length !== 7
      || ![s.plan.main, s.plan.orb, s.plan.experienceBar, s.plan.weightBar, ...s.plan.buttons.map((b: HudButton) => b.rect)].every(validRect))) return null;
    return s;
  } catch { return null; }
}
function validRect(r: HudRect) { return r && [r.left, r.top, r.width, r.height].every(Number.isFinite) && r.width > 0 && r.height > 0; }
export function containsHudRect(r: HudRect, x: number, y: number) {
  return validRect(r) && x >= r.left && y >= r.top && x < r.left + r.width && y < r.top + r.height;
}
export type HudPointerLease = { pointerId: number; generation: number; revision: number; origin: "ui" | "world"; action: HudAction | HudButton["action"] | null };
/** Origin is chosen before a world hold is created and survives crossing drags. */
export class HudPointerRouter {
  held: HudPointerLease | null = null;
  down(status: HudStatus, pointerId: number, x: number, y: number, foreground: boolean): HudPointerLease | null {
    if (this.held || !status.ready || !status.plan || foreground || status.modal
      || status.foregroundRects.some(rect => containsHudRect(rect, x, y))) return null;
    let action: HudPointerLease["action"] = null;
    const stats = status.characterStatsReady && containsHudRect(status.plan.characterRect, x, y);
    if (stats) {
      action = status.plan.characterHits.find(hit => containsHudRect(hit.rect, x, y))?.action ?? null;
    } else action = status.plan.buttons.find(button => containsHudRect(button.rect, x, y))?.action ?? null;
    return this.held = { pointerId, generation: status.generation, revision: status.revision,
      origin: stats || containsHudRect(status.plan.main, x, y) ? "ui" : "world", action };
  }
  matches(status: HudStatus | null) { return Boolean(this.held && status?.ready && this.held.generation === status.generation); }
  cancel() { const held = this.held; this.held = null; return held; }
}
