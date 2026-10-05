import type { QuestActionInput, QuestActionDecision, ClientCoreRuntime } from "./client-core-runtime";

export type QuestGuidanceProfile = QuestActionInput["profile"];

export type QuestActionHostQuest = {
  stage?: string | null;
  acceptNpcIndex?: unknown;
  finishNpcIndex?: unknown;
  rewards?: {
    selectItems?: ReadonlyArray<{ selectionIndex?: unknown }>;
  };
};

export type QuestActionHostDialog = {
  npcObjectId: string;
  links: ReadonlyArray<{ target: string; enabled?: boolean }>;
};

export type PendingQuestAction = {
  requestId: string;
  operation: "acceptQuest" | "finishQuest" | "abandonQuest";
  questIndex: number;
  npcIndex?: number;
  selectedItemIndex?: number;
};

/** One quest operation remains in flight even if the player changes reward choice. */
export function hasPendingQuestAction(
  pendingActions: Iterable<PendingQuestAction>,
  questIndex: number,
  operation: PendingQuestAction["operation"],
): boolean {
  for (const pending of pendingActions) {
    if (pending.operation === operation && pending.questIndex === questIndex) return true;
  }
  return false;
}

/** Correlate the snapshot receipt with the exact submitted command. */
export function matchingQuestOperationAck(
  raw: unknown,
  pending: PendingQuestAction | undefined,
): { success: boolean } | null {
  if (!pending || !raw || typeof raw !== "object" || Array.isArray(raw)) return null;
  const ack = raw as Record<string, unknown>;
  if (ack.requestId !== pending.requestId
    || ack.operation !== pending.operation
    || ack.questIndex !== pending.questIndex
    || typeof ack.success !== "boolean"
    || (pending.operation === "acceptQuest" && ack.npcIndex !== pending.npcIndex)
    || (pending.operation === "finishQuest" && ack.selectedItemIndex !== pending.selectedItemIndex)
  ) return null;
  return { success: ack.success };
}

/** The deployment setting is presentation only; the server still decides quest state. */
export function questGuidanceProfile(value: string | undefined): QuestGuidanceProfile {
  return value === "newcomer-v1" || value === "newcomer-v2" ? value : "crystal";
}

/** Keep an absent or malformed endpoint absent. An explicit zero is meaningful. */
export function questEndpoint(value: unknown): number | undefined {
  return typeof value === "number"
    && Number.isSafeInteger(value)
    && value >= 0
    && value <= 0xffff_ffff
    ? value
    : undefined;
}

function dialogNpcIndex(dialog: QuestActionHostDialog | null): number | null {
  if (!dialog || !/^[1-9][0-9]*$/.test(dialog.npcObjectId)) return null;
  const value = Number(dialog.npcObjectId);
  return Number.isSafeInteger(value) && value <= 0xffff_ffff ? value : null;
}

function dialogOffersAction(
  dialog: QuestActionHostDialog | null,
  questId: number,
  action: QuestActionInput["action"],
  selectedRewardIndex: number | null,
): boolean {
  if (!dialog || dialogNpcIndex(dialog) === null) return false;
  const targets = action === "accept"
    ? [`@AcceptQuest:${questId}`, `@quest:accept:${questId}`]
    : [
        ...(selectedRewardIndex === null ? [] : [`@quest:finish:${questId}:${selectedRewardIndex}`]),
        `@FinishQuest:${questId}`,
        `@quest:finish:${questId}`,
      ];
  const enabledTargets = new Set(targets.map((target) => target.toLowerCase()));
  return dialog.links.some((link) =>
    link.enabled !== false
    && typeof link.target === "string"
    && enabledTargets.has(link.target.trim().toLowerCase()),
  );
}

export function questActionInput({
  action,
  profile,
  questId,
  quest,
  dialog,
  selectedRewardIndex,
  pending,
}: {
  action: QuestActionInput["action"];
  profile: QuestGuidanceProfile;
  questId: number;
  quest: QuestActionHostQuest | null;
  dialog: QuestActionHostDialog | null;
  selectedRewardIndex?: number;
  pending: boolean;
}): QuestActionInput | null {
  if (selectedRewardIndex !== undefined
    && (!Number.isSafeInteger(selectedRewardIndex) || selectedRewardIndex < 0 || selectedRewardIndex > 0x7fff_ffff)) {
    return null;
  }
  const status = quest?.stage === "available" ? "notStarted"
    : quest?.stage === "inProgress" ? "inProgress"
      : quest?.stage === "readyToTurnIn" ? "readyToTurnIn"
        : quest?.stage ? "other" : null;
  const selectableRewardIndices = quest?.rewards?.selectItems?.map((item, index) => {
    const selected = item.selectionIndex === undefined ? index : item.selectionIndex;
    return selected;
  }) ?? [];
  if (selectableRewardIndices.some((index) =>
    typeof index !== "number" || !Number.isSafeInteger(index) || index < 0 || index > 0x7fff_ffff,
  )) return null;
  return {
    action,
    profile,
    status,
    acceptNpcIndex: questEndpoint(quest?.acceptNpcIndex),
    finishNpcIndex: questEndpoint(quest?.finishNpcIndex),
    selectableRewardIndices: selectableRewardIndices as number[],
    selectedRewardIndex: selectedRewardIndex === undefined ? null : selectedRewardIndex,
    dialogNpcIndex: dialogNpcIndex(dialog),
    dialogActionOffered: dialogOffersAction(
      dialog,
      questId,
      action,
      selectedRewardIndex === undefined ? null : selectedRewardIndex,
    ),
    pending,
  };
}

export function resolveQuestHostAction(
  runtime: ClientCoreRuntime,
  input: QuestActionInput,
): QuestActionDecision | null {
  try {
    const decision = runtime.resolveQuestAction(input);
    return decision && typeof decision.eligible === "boolean" ? decision : null;
  } catch {
    return null;
  }
}
