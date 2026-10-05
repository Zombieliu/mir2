import { questEndpoint, type QuestGuidanceProfile } from "./quest-action-policy";
import type { QuestRewardsPresentation, QuestRewardTooltipSource } from "./quest-reward-presentation";
import type { BevyHpOrbSlot, BevyHpOrbStatus, BevyMpOrbStatus } from "./bevy-hp-orb";
import type { BevyExperienceBarRect, BevyExperienceBarStatus } from "./bevy-experience-bar";
import type { BevyWeightBarRect, BevyWeightBarStatus } from "./bevy-weight-bar";
import type { HudBarPlanCommit, HudBarPlanRuntime } from "./bevy-hud-bar-draw-plan";
import type { Mir2Language } from "./localization";

/** Versioned browser host boundary; no transport or game-state mutation lives here. */
export type BevyQuestReward =
  | { type: "gold" | "experience"; amount: number }
  | { type: "item"; itemId: string; name: string; quantity: number; icon?: number; selectionIndex?: number; tooltipSource?: QuestRewardTooltipSource }
  | { type: "unknown"; label: string };

export type BevyQuestDefinition = {
  acceptNpcIndex?: number;
  finishNpcIndex?: number;
  group?: string;
  minLevelNeeded?: number;
  descriptionLines?: string[];
  taskDescriptionLines?: string[];
  returnDescriptionLines?: string[];
  completionDescriptionLines?: string[];
  timeLimit?: string;
  rewards?: QuestRewardsPresentation;
};

export type BevyQuestSource = BevyQuestDefinition & {
  questId: number;
  title: string;
  summary: string;
  objective: string;
  current: number;
  required: number;
  objectives?: Array<{ label: string; current?: number; required?: number }>;
};

export type BevyQuestDialogSource = {
  npcObjectId: string;
  npcName: string;
  title: string;
  body: string[];
  footer: string;
  links: Array<{ text: string; target: string; enabled?: boolean }>;
  input?: unknown;
};

export type BevyQuestDto = {
  questIndex: number;
  acceptNpcIndex: number | null;
  finishNpcIndex: number | null;
  title: string;
  group?: string;
  minLevelNeeded: number;
  detail: {
    descriptionLines: string[];
    taskDescriptionLines: string[];
    returnDescriptionLines: string[];
    completionDescriptionLines: string[];
    timeLimit?: string;
  };
  status: "notStarted" | "inProgress" | "readyToTurnIn" | "completed" | "other";
  objectives: Array<{ objectiveId: string; text: string; current: number; target: number }>;
  rewards: BevyQuestReward[];
};

export type BevyQuestUiPresentation = {
  logicalWidth: number;
  logicalHeight: number;
  stageCssScale: number;
  touch: boolean;
};

/** Read the same stage geometry used by the existing scene and input mapping. */
export function readBevyQuestPresentation(
  frame: HTMLElement | null,
  canvas: HTMLCanvasElement | null,
  touch: boolean,
): BevyQuestUiPresentation | null {
  if (!frame || !canvas || canvas.parentElement !== frame || typeof touch !== "boolean") return null;
  const logicalWidth = Number(frame.dataset.viewportSceneWidth);
  const logicalHeight = Number(frame.dataset.viewportSceneHeight);
  if (!Number.isSafeInteger(logicalWidth) || !Number.isSafeInteger(logicalHeight)
    || logicalWidth <= 0 || logicalHeight <= 0
    || frame.clientWidth !== logicalWidth || frame.clientHeight !== logicalHeight
    || canvas.clientWidth !== logicalWidth || canvas.clientHeight !== logicalHeight) return null;
  const frameRect = frame.getBoundingClientRect();
  const canvasRect = canvas.getBoundingClientRect();
  const stageCssScale = frameRect.width / logicalWidth;
  const close = (actual: number, expected: number) => Number.isFinite(actual) && Math.abs(actual - expected) <= 0.75;
  if (!Number.isFinite(stageCssScale) || stageCssScale <= 0
    || !close(frameRect.height, logicalHeight * stageCssScale)
    || !close(canvasRect.left, frameRect.left) || !close(canvasRect.top, frameRect.top)
    || !close(canvasRect.width, frameRect.width) || !close(canvasRect.height, frameRect.height)) return null;
  return { logicalWidth, logicalHeight, stageCssScale, touch };
}

export type BevyQuestUiSnapshot = {
  generation: number;
  revision: number;
  language?: Mir2Language;
  inGame: boolean;
  hostVisible: boolean;
  questLogOpen: boolean;
  openRevision: number;
  blocksGameplayKeys: boolean;
  turnInBlocked: boolean;
  presentation: BevyQuestUiPresentation;
  profile: QuestGuidanceProfile;
  quests: BevyQuestDto[];
  completedKnown: boolean;
  completedQuestIds: number[];
  dialog: {
    isOpen: boolean;
    npcObjectId: number | null;
    npcName: string | null;
    lines: string[];
    options: Array<{ optionId: string; label: string; enabled: boolean }>;
    hasInput: boolean;
  };
  player: { hp: number; maxHp: number; mp?: number | null; maxMp?: number | null;
    experience?: number | null; maxExperience?: number | null;
    currentWeight?: number | null; maxWeight?: number | null;
    level: number; className?: string; name?: string };
  hpOrbSlot?: BevyHpOrbSlot | null;
  experienceBarSlot?: BevyExperienceBarRect | null;
  weightBarSlot?: BevyWeightBarRect | null;
  hudBarPlan?: HudBarPlanCommit;
};

export type BevyQuestUiStatus = {
  questLocaleVersion?: number;
  language?: Mir2Language;
  frame: number;
  ready: boolean;
  error: string | null;
  capturesPointer: boolean;
  questLogOpen: boolean;
  generation: number;
  revision: number;
  openRevision: number;
  hpOrb?: BevyHpOrbStatus;
  mpOrb?: BevyMpOrbStatus;
  experienceBar?: BevyExperienceBarStatus;
  weightBar?: BevyWeightBarStatus;
};

export type BevyQuestUiIntent = {
  generation: number;
  type: string;
  questIndex?: number;
  npcIndex?: number;
  selectedItemIndex?: number;
  target?: string;
};

export type BevyQuestUiIntentResult = { accepted: boolean; requestId?: string; error?: string };

export type BevyQuestUiRuntime = HudBarPlanRuntime & {
  setMir2QuestUiSnapshot?: (json: string) => boolean;
  setMir2QuestUiIntentSink?: (sink: (json: string) => string) => void;
  clearMir2QuestUiIntentSink?: () => void;
  setMir2QuestUiOperationAck?: (json: string) => boolean;
  getMir2QuestUiStatus?: () => string;
};

type HostCounters = { generation: number; request: number };
// A Bevy App can outlive a React remount/HMR. Neither session identities nor
// request IDs may restart while that App can still retain queued messages.
function counters(): HostCounters {
  const scope = globalThis as typeof globalThis & { __mir2QuestHostCounters?: HostCounters };
  return scope.__mir2QuestHostCounters ??= { generation: 0, request: 0 };
}

function increment(key: keyof HostCounters): number {
  const state = counters();
  if (!Number.isSafeInteger(state[key]) || state[key] >= Number.MAX_SAFE_INTEGER) {
    throw new Error("Quest host sequence exhausted; reload the client.");
  }
  return ++state[key];
}

export function nextQuestUiGeneration(): number { return increment("generation"); }
export function nextQuestRequestId(): string { return `qs-${increment("request").toString().padStart(16, "0")}`; }

function unsigned(value: number | undefined, fallback = 0): number {
  if (value === undefined) return fallback;
  if (!Number.isSafeInteger(value) || value < 0 || value > 0xffff_ffff) {
    throw new Error("Invalid authoritative quest number");
  }
  return value;
}

export function projectBevyQuests(
  sources: readonly BevyQuestSource[],
  authoritativeStages: ReadonlyMap<number, string>,
): BevyQuestDto[] {
  // A static NewQuestInfo definition alone is never an available quest.
  return sources.filter((quest) => authoritativeStages.has(quest.questId)).map((quest) => {
    if (!Number.isSafeInteger(quest.questId) || quest.questId <= 0 || quest.questId > 0x7fff_ffff) {
      throw new Error("Invalid authoritative quest ID");
    }
    const stage = authoritativeStages.get(quest.questId);
    const status = stage === "available" ? "notStarted"
      : stage === "inProgress" || stage === "readyToTurnIn" || stage === "completed" ? stage : "other";
    const rewards: BevyQuestReward[] = [];
    if (quest.rewards?.gold !== undefined) rewards.push({ type: "gold", amount: unsigned(quest.rewards.gold) });
    if (quest.rewards?.experience !== undefined) rewards.push({ type: "experience", amount: unsigned(quest.rewards.experience) });
    // Credit has no typed shared reward yet. Keep its server amount visible.
    if (quest.rewards?.credit) rewards.push({ type: "unknown", label: `${unsigned(quest.rewards.credit)} Credit` });
    for (const item of quest.rewards?.items ?? []) {
      rewards.push({ type: "item", itemId: String(item.itemIndex ?? ""), name: item.name,
        quantity: unsigned(item.count, 1), icon: item.icon === undefined ? undefined : unsigned(item.icon),
        ...(item.tooltipSource ? { tooltipSource: item.tooltipSource } : {}) });
    }
    for (const [index, item] of (quest.rewards?.selectItems ?? []).entries()) {
      const selectionIndex = unsigned(item.selectionIndex, index);
      if (selectionIndex > 0x7fff_ffff) throw new Error("Invalid quest reward selection");
      rewards.push({ type: "item", itemId: String(item.itemIndex ?? ""), name: item.name,
        quantity: unsigned(item.count, 1), icon: item.icon === undefined ? undefined : unsigned(item.icon), selectionIndex,
        ...(item.tooltipSource ? { tooltipSource: item.tooltipSource } : {}) });
    }
    const objectives = quest.objectives ?? (quest.objective
      ? [{ label: quest.objective, current: quest.current, required: quest.required }] : []);
    return {
      questIndex: quest.questId,
      acceptNpcIndex: questEndpoint(quest.acceptNpcIndex) ?? null,
      finishNpcIndex: questEndpoint(quest.finishNpcIndex) ?? null,
      title: quest.title,
      group: quest.group,
      minLevelNeeded: Math.min(unsigned(quest.minLevelNeeded), 0x7fff_ffff),
      detail: {
        descriptionLines: quest.descriptionLines ?? (quest.summary ? [quest.summary] : []),
        taskDescriptionLines: quest.taskDescriptionLines ?? [],
        returnDescriptionLines: quest.returnDescriptionLines ?? [],
        completionDescriptionLines: quest.completionDescriptionLines ?? [],
        timeLimit: quest.timeLimit,
      },
      status,
      objectives: objectives.map((objective, index) => ({
        objectiveId: `${quest.questId}:${index}`, text: objective.label,
        current: unsigned(objective.current), target: unsigned(objective.required),
      })),
      rewards,
    };
  });
}

export function projectBevyQuestDialog(dialog: BevyQuestDialogSource | null): BevyQuestUiSnapshot["dialog"] {
  const npc = dialog && /^[1-9][0-9]*$/.test(dialog.npcObjectId) ? questEndpoint(Number(dialog.npcObjectId)) : undefined;
  return {
    isOpen: dialog !== null && npc !== undefined,
    npcObjectId: npc ?? null,
    npcName: dialog?.npcName ?? null,
    lines: dialog ? [dialog.title, ...dialog.body, dialog.footer].filter(Boolean) : [],
    options: (dialog?.links ?? []).map((link) => ({ optionId: link.target, label: link.text, enabled: link.enabled !== false })),
    hasInput: Boolean(dialog?.input),
  };
}

export function isBevyQuestLanguage(value: unknown): value is Mir2Language {
  return value === "en" || value === "zh-CN" || value === "es" || value === "pt-BR";
}

/** Probe the version independently of readiness; malformed v1 status cannot become ready. */
export function supportsBevyQuestLocale(runtime: BevyQuestUiRuntime | null): boolean {
  try { return JSON.parse(runtime?.getMir2QuestUiStatus?.() ?? "null")?.questLocaleVersion === 1; }
  catch { return false; }
}

export function stripUnsupportedQuestLocale<T extends { language?: Mir2Language }>(snapshot: T, supported: boolean): T {
  if (supported) return snapshot;
  const result = { ...snapshot };
  delete result.language;
  return result;
}

export function currentBevyQuestLocale(status: BevyQuestUiStatus | null, requested: Mir2Language | undefined,
  minimumRevision: number): boolean {
  return Boolean(status && status.revision >= minimumRevision
    && (status.questLocaleVersion !== 1 || (isBevyQuestLanguage(status.language)
      && status.language === (requested ?? "zh-CN"))));
}

export function readBevyQuestUiStatus(runtime: BevyQuestUiRuntime | null, generation: number): BevyQuestUiStatus | null {
  if (!runtime?.getMir2QuestUiStatus) return null;
  try {
    const status = JSON.parse(runtime.getMir2QuestUiStatus()) as BevyQuestUiStatus;
    if (status.generation !== generation || typeof status.ready !== "boolean"
      || typeof status.capturesPointer !== "boolean" || typeof status.questLogOpen !== "boolean"
      || !Number.isSafeInteger(status.frame) || status.frame < 0
      || !Number.isSafeInteger(status.revision) || !Number.isSafeInteger(status.openRevision)
      || (status.questLocaleVersion === 1 && !isBevyQuestLanguage(status.language))) return null;
    return status;
  } catch { return null; }
}
