/**
 * Web projection of `config/quest-guidance/newcomer-journey-v2-ui.json`.
 *
 * The gateway sends generic QuestInfo / QuestLog packets, so this module is
 * presentation-only: it recognizes the configured V2 lines without creating,
 * completing, or advancing any quest on the client.
 */
export type NewcomerV2QuestStage =
  | "available"
  | "inProgress"
  | "readyToTurnIn"
  | "completed";

export type NewcomerV2QuestLike = {
  questId: number;
  stage: NewcomerV2QuestStage;
};

export type NewcomerV2Text = { en: string; zhCN: string };

export type NewcomerV2TranslateFn = (
  key: string,
  params?: Array<string | number>,
  fallback?: string,
) => string;

export type NewcomerV2Chapter = {
  id: string;
  title: NewcomerV2Text;
  questIds: readonly number[];
};

export const NEWCOMER_V2_CHAPTERS: readonly NewcomerV2Chapter[] = [
  {
    id: "village",
    title: { en: "Village Beginnings", zhCN: "村庄起步" },
    questIds: [2110001, 2110002, 2110003, 2110004],
  },
  {
    id: "forest",
    title: { en: "Bichon Forest", zhCN: "比奇森林" },
    questIds: [2110005, 2110006, 2110007, 2110008, 2120015],
  },
  {
    id: "cave",
    title: { en: "Cave Rescue", zhCN: "洞穴救援" },
    questIds: [2110009, 2110010, 2110011, 2110012, 2120020],
  },
  {
    id: "mine",
    title: { en: "Dead Mine", zhCN: "死亡矿区" },
    questIds: [2110013, 2110014, 2110015, 2110016, 2120025],
  },
  {
    id: "expedition",
    title: { en: "Wooma Expedition", zhCN: "沃玛远征" },
    questIds: [2110017, 2110018, 2110019],
  },
  {
    id: "graduation",
    title: { en: "Wooma Graduation", zhCN: "沃玛结业" },
    questIds: [2110020, 2110021, 2110022, 2120030],
  },
] as const;

type NewcomerV2QuestCopy = {
  title: NewcomerV2Text;
  objective: NewcomerV2Text;
  objectives?: readonly NewcomerV2ObjectiveCopy[];
};

type NewcomerV2ObjectiveCopy = {
  sourceLabels: readonly string[];
  text: NewcomerV2Text;
};

// These titles and objectives are the player-facing names and required work
// from newcomer-journey-v2.json. Keep them adjacent to the chapter projection;
// the dedicated Node test checks every configured V2 id remains represented.
const NEWCOMER_V2_QUEST_COPY: Readonly<Record<number, NewcomerV2QuestCopy>> = {
  2110001: copy("Your first patrol kit", "首套巡逻装备", "Report to Assistant Jane", "向简助理报告", [flag("Report to Assistant Jane", "向简助理报告")]),
  2110002: copy("Equip your weapon", "装备你的武器", "Equip a weapon you can use", "装备一件可用的武器", [flag("Equip a weapon you can use", "装备一件可用的武器")]),
  2110003: copy("Protect the village", "保卫村庄", "Defeat 2 Scarecrows and 2 Raking Cats", "消灭 2 个稻草人和 2 只钉耙猫", [
    kill("Defeat 2 Scarecrow.", "消灭 2 个稻草人"),
    kill("Defeat 2 RakingCat.", "消灭 2 只钉耙猫"),
  ]),
  2110004: copy("Your first class skill", "第一项职业技能", "Complete class practice and defeat 2 Oma", "完成职业练习并消灭 2 个 Oma", [
    kill("Defeat 2 Oma.", "消灭 2 个 Oma"),
    flag("Complete your class practice", "完成职业练习"),
  ]),
  2110005: copy("Arrive safely in Bichon", "安全抵达比奇", "Reach a Bichon safe area", "抵达比奇安全区域", [flag("Reach a Bichon safe area", "抵达比奇安全区域")]),
  2110006: copy("Prepare your supplies", "做好补给", "Buy a small HP or MP potion from a normal shop", "从普通商店购买小型 HP 或 MP 药水", [flag("Buy a small HP or MP potion from a normal shop", "从普通商店购买小型 HP 或 MP 药水")]),
  2110007: copy("Clear the forest path", "清理森林道路", "Defeat 4 Forest Yetis", "消灭 4 只森林雪人", [kill("Defeat 4 ForestYeti.", "消灭 4 只森林雪人")]),
  2110008: copy("Show your class training", "展示职业训练", "Complete class practice and defeat 2 Oma", "完成职业练习并消灭 2 个 Oma", [
    kill("Defeat 2 Oma.", "消灭 2 个 Oma"),
    flag("Complete your class practice", "完成职业练习"),
  ]),
  2110009: copy("Enter Oma Cave", "进入 Oma 洞穴", "Reach the Oma Cave entrance", "抵达 Oma 洞穴入口", [flag("Reach the Oma Cave entrance", "抵达 Oma 洞穴入口")]),
  2110010: copy("Push back the skeletons", "击退骷髅", "Defeat 4 Skeletons", "消灭 4 只骷髅", [kill("Defeat 4 Skeleton.", "消灭 4 只骷髅")]),
  2110011: copy("Practice against the undead", "对抗亡灵练习", "Complete class practice and defeat 3 Skeletons", "完成职业练习并消灭 3 只骷髅", [
    kill("Defeat 3 Skeleton.", "消灭 3 只骷髅"),
    flag("Complete your class practice", "完成职业练习"),
  ]),
  2110012: copy("Complete the cave patrol", "完成洞穴巡逻", "Complete class practice and defeat a Skeleton", "完成职业练习并消灭 1 只骷髅", [
    kill("Defeat 1 Skeleton.", "消灭 1 只骷髅"),
    flag("Complete your class practice", "完成职业练习"),
  ]),
  2110013: copy("Enter the Dead Mine", "进入死亡矿区", "Reach the Dead Mine entrance", "抵达死亡矿区入口", [flag("Reach the Dead Mine entrance", "抵达死亡矿区入口")]),
  2110014: copy("Clear the mine entrance", "清理矿区入口", "Defeat 3 Zombie2 and 2 Zombie3", "消灭 3 只 Zombie2 和 2 只 Zombie3", [
    kill("Defeat 3 Zombie2.", "消灭 3 只 Zombie2"),
    kill("Defeat 2 Zombie3.", "消灭 2 只 Zombie3"),
  ]),
  2110015: copy("Fight and reposition", "战斗并重新站位", "Complete class practice and defeat 1 Zombie2", "完成职业练习并消灭 1 只 Zombie2", [
    kill("Defeat 1 Zombie2.", "消灭 1 只 Zombie2"),
    flag("Complete your class practice", "完成职业练习"),
  ]),
  2110016: copy("Complete the mine patrol", "完成矿区巡逻", "Complete class practice and defeat 3 Zombie3", "完成职业练习并消灭 3 只 Zombie3", [
    kill("Defeat 3 Zombie3.", "消灭 3 只 Zombie3"),
    flag("Complete your class practice", "完成职业练习"),
  ]),
  2110017: copy("Prepare for the expedition", "准备远征", "Equip suitable armour and report to the Bichon Wall Board", "装备适合本职业与等级的盔甲，并向比奇城墙公告板报告", [
    flag("Equip armour for your class and level", "装备适合本职业与等级的盔甲"),
    flag("Report to the Bichon Wall Board", "向比奇城墙公告板报告"),
  ]),
  2110018: copy("Reach Wooma Temple", "抵达沃玛寺庙", "Reach the Wooma Temple entrance and complete class practice", "抵达沃玛寺庙入口并完成职业练习", [
    flag("Reach the Wooma Temple entrance", "抵达沃玛寺庙入口"),
    flag("Complete your class practice", "完成职业练习"),
  ]),
  2110019: copy("Clear a foothold", "清理前哨", "Defeat 3 Dung", "消灭 3 只 Dung", [kill("Defeat 3 Dung.", "消灭 3 只 Dung")]),
  2110020: copy("Defeat Wooma Soldiers", "击败沃玛战士", "Defeat 3 WoomaSoldier", "消灭 3 名 WoomaSoldier", [kill("Defeat 3 WoomaSoldier.", "消灭 3 名 WoomaSoldier")]),
  2110021: copy("Prove your class tactics", "证明职业战术", "Complete class practice and defeat 3 WoomaFighter", "完成职业练习并消灭 3 名 WoomaFighter", [
    kill("Defeat 3 WoomaFighter.", "消灭 3 名 WoomaFighter"),
    flag("Complete your class practice", "完成职业练习"),
  ]),
  2110022: copy("Report the expedition", "报告远征", "Return to the Bichon Wall Board", "返回比奇城墙公告板报告", [flag("Return to the Bichon Wall Board", "返回比奇城墙公告板报告")]),
  2120015: copy("Level 15 growth reward", "15 级成长奖励", "Reach level 15 and claim the growth reward", "达到 15 级并领取成长奖励"),
  2120020: copy("Level 20 growth reward", "20 级成长奖励", "Reach level 20 and claim the growth reward", "达到 20 级并领取成长奖励"),
  2120025: copy("Level 25 growth reward", "25 级成长奖励", "Reach level 25 and claim the growth reward", "达到 25 级并领取成长奖励"),
  2120030: copy("Level 30 growth reward", "30 级成长奖励", "Reach level 30 and claim the growth reward", "达到 30 级并领取成长奖励"),
};

/**
 * Quest translate callbacks do not expose their selected language. The shared
 * Track affordance is present in every supported bundle and distinguishes the
 * Chinese presentation without making a second language channel mandatory.
 */
export function localizeNewcomerV2Text(
  copy: NewcomerV2Text,
  t: NewcomerV2TranslateFn,
): string {
  return t("ui.questTrack", [], "__newcomer_v2_language_probe__") === "追踪"
    ? copy.zhCN
    : copy.en;
}

function copy(
  titleEn: string,
  titleZhCN: string,
  objectiveEn: string,
  objectiveZhCN: string,
  objectives?: readonly NewcomerV2ObjectiveCopy[],
): NewcomerV2QuestCopy {
  return {
    title: { en: titleEn, zhCN: titleZhCN },
    objective: { en: objectiveEn, zhCN: objectiveZhCN },
    ...(objectives ? { objectives } : {}),
  };
}

/** Match both static `NewQuestInfo` messages and dynamic `ChangeQuest` kill lines. */
function kill(message: string, zhCN: string): NewcomerV2ObjectiveCopy {
  const monsterName = message.match(/^Defeat\s+\d+\s+(.+?)\.?$/i)?.[1];
  return {
    sourceLabels: [message, ...(monsterName ? [`Kill ${monsterName}`] : [])],
    text: { en: message, zhCN },
  };
}

function flag(message: string, zhCN: string): NewcomerV2ObjectiveCopy {
  return { sourceLabels: [message], text: { en: message, zhCN } };
}

export function newcomerV2ObjectiveTextFor(
  questId: number,
  serverLabel: string,
): NewcomerV2Text | null {
  const normalized = normalizeObjectiveLabel(serverLabel);
  if (!normalized) return null;
  const objective = newcomerV2QuestCopyFor(questId)?.objectives?.find((entry) =>
    entry.sourceLabels.some((sourceLabel) => normalizeObjectiveLabel(sourceLabel) === normalized),
  );
  return objective?.text ?? null;
}

function normalizeObjectiveLabel(label: string): string {
  return label
    .trim()
    .replace(/\s+\d+\s*\/\s*\d+\s*$/, "")
    .replace(/[.。]+$/, "")
    .replace(/\s+/g, " ")
    .toLocaleLowerCase();
}

export function newcomerV2ChapterForQuest(questId: number): NewcomerV2Chapter | null {
  return NEWCOMER_V2_CHAPTERS.find((chapter) => chapter.questIds.includes(questId)) ?? null;
}

export function newcomerV2QuestCopyFor(questId: number): NewcomerV2QuestCopy | null {
  return NEWCOMER_V2_QUEST_COPY[questId] ?? null;
}

export function isNewcomerV2Quest(questId: number): boolean {
  return newcomerV2QuestCopyFor(questId) !== null;
}

export type NewcomerV2ChapterProgress = NewcomerV2Chapter & {
  number: number;
  recordedQuestCount: number;
  completedRecordedQuestCount: number;
};

export type NewcomerV2JourneyProgress = {
  chapters: readonly NewcomerV2ChapterProgress[];
  currentChapter: NewcomerV2ChapterProgress | null;
  recordedQuestCount: number;
  completedRecordedQuestCount: number;
};

type NewcomerV2GraduationClass = "warrior" | "wizard" | "taoist";

export type NewcomerV2GraduationGoalKey = "equipment" | "skill" | "challenge";

export type NewcomerV2GraduationGoal = {
  key: NewcomerV2GraduationGoalKey;
  label: string;
  title: string;
  requirements: string;
  normalAcquisition: string;
};

export type NewcomerV2Graduation = {
  title: string;
  goals: readonly NewcomerV2GraduationGoal[];
};

// Keep the eligibility set coupled to the six UI chapters. The test verifies
// this derived set against the Native handoff configuration, so a future
// chapter edit cannot silently change graduation eligibility in the browser.
export const NEWCOMER_V2_GRADUATION_REQUIRED_COMPLETION_IDS: readonly number[] =
  NEWCOMER_V2_CHAPTERS.flatMap((chapter) => chapter.questIds);

const NEWCOMER_V2_GRADUATION_CLASS_GOALS: Readonly<
  Record<NewcomerV2GraduationClass, readonly NewcomerV2GraduationGoal[]>
> = {
  warrior: [
    graduationGoal("equipment", "Equipment", "JudgementMace", "Level 30. Compatible with Warrior, Wizard, and Taoist.", "Equipment target for your next hunt."),
    graduationGoal("skill", "Skill", "ShoulderDash", "Warrior, level 30. Skill levels: 30 / 32 / 34.", "Hunt EvilCentipede in Life Death Coffin for this book."),
    graduationGoal("challenge", "Challenge", "Insect Cave N 2F sweep", "Medium challenge: Centipede and Tongs.", "Reach Insect Cave N 2F from Insect Cave W 1F or W 2F."),
  ],
  wizard: [
    graduationGoal("equipment", "Equipment", "WarMageStaff", "Level 30. Compatible with Warrior, Wizard, and Taoist.", "Equipment target for your next hunt."),
    graduationGoal("skill", "Skill", "ThunderStorm", "Wizard, level 30. Skill levels: 30 / 32 / 34.", "Hunt EvilCentipede in Life Death Coffin for this book."),
    graduationGoal("challenge", "Challenge", "Insect Cave N 2F sweep", "Medium challenge: Centipede and Tongs.", "Reach Insect Cave N 2F from Insect Cave W 1F or W 2F."),
  ],
  taoist: [
    graduationGoal("equipment", "Equipment", "SoulSpringWand", "Level 30. Compatible with Warrior, Wizard, and Taoist.", "Equipment target for your next hunt."),
    graduationGoal("skill", "Skill", "Purification", "Taoist, level 30. Skill levels: 30 / 32 / 35.", "Hunt WhiteBoar in Angled Stone Tomb B4 for this book."),
    graduationGoal("challenge", "Challenge", "Insect Cave N 2F sweep", "Medium challenge: Centipede and Tongs.", "Reach Insect Cave N 2F from Insect Cave W 1F or W 2F."),
  ],
};

function graduationGoal(
  key: NewcomerV2GraduationGoalKey,
  label: string,
  title: string,
  requirements: string,
  normalAcquisition: string,
): NewcomerV2GraduationGoal {
  return { key, label, title, requirements, normalAcquisition };
}

function graduationClassFor(playerClass: string | null | undefined): NewcomerV2GraduationClass | null {
  const normalized = playerClass?.trim().toLocaleLowerCase();
  return normalized === "warrior" || normalized === "wizard" || normalized === "taoist"
    ? normalized
    : null;
}

/**
 * Graduation is a display-only handoff. It appears only for the actual
 * server-reported level and a fully recorded completed route; missing history
 * is never treated as completion.
 */
export function newcomerV2GraduationFor(
  quests: readonly NewcomerV2QuestLike[],
  playerLevel: number | null | undefined,
  playerClass: string | null | undefined,
): NewcomerV2Graduation | null {
  const classKey = graduationClassFor(playerClass);
  if (
    !classKey
    || typeof playerLevel !== "number"
    || !Number.isFinite(playerLevel)
    || playerLevel < 30
  ) return null;

  const stageByQuestId = new Map(quests.map((quest) => [quest.questId, quest.stage]));
  if (!NEWCOMER_V2_GRADUATION_REQUIRED_COMPLETION_IDS.every(
    (questId) => stageByQuestId.get(questId) === "completed",
  )) {
    return null;
  }

  return {
    title: "Wooma Graduation",
    goals: NEWCOMER_V2_GRADUATION_CLASS_GOALS[classKey],
  };
}

/**
 * A QuestInfo definition can be present before the server exposes an active
 * line. Only in-progress / ready-to-turn-in records establish a current
 * chapter. Missing records are deliberately not treated as completed.
 */
export function newcomerV2JourneyProgress(
  quests: readonly NewcomerV2QuestLike[],
): NewcomerV2JourneyProgress | null {
  const byQuestId = new Map<number, NewcomerV2QuestStage>();
  for (const quest of quests) {
    if (isNewcomerV2Quest(quest.questId)) byQuestId.set(quest.questId, quest.stage);
  }
  if (byQuestId.size === 0) return null;

  const chapters = NEWCOMER_V2_CHAPTERS.map((chapter, index) => {
    const recordedQuestIds = chapter.questIds.filter((questId) => byQuestId.has(questId));
    return {
      ...chapter,
      number: index + 1,
      recordedQuestCount: recordedQuestIds.length,
      completedRecordedQuestCount: recordedQuestIds.filter(
        (questId) => byQuestId.get(questId) === "completed",
      ).length,
    };
  });
  const currentChapter = chapters.find((chapter) =>
    chapter.questIds.some((questId) => {
      const stage = byQuestId.get(questId);
      return stage === "inProgress" || stage === "readyToTurnIn";
    }),
  ) ?? null;

  return {
    chapters,
    currentChapter,
    recordedQuestCount: byQuestId.size,
    completedRecordedQuestCount: [...byQuestId.values()].filter((stage) => stage === "completed").length,
  };
}
