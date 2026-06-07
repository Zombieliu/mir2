// Beginner-tutorial state machine — pure, DOM-free logic.
//
// The original Crystal client has NO interactive tutorial; its onboarding is a
// passive paginated help book (Client/MirScenes/Dialogs/HelpDialog.cs, 45 image
// pages) plus newbie-village NPC-script quests. This module is therefore a
// NET-NEW, additive layer — it never touches DisplayWorld or existing consumers.
//
// The step list mirrors the HelpDialog syllabus order (Movements → Attacking →
// CollectingItems → Health → Skills → Chatting → Groups → …), trimmed to the
// operations a brand-new player needs in their first session. Completion is
// detected from the player's own outbound actions (every ClientPacket flows
// through page.tsx `send()`, which dispatches a `mir2:action` CustomEvent) plus a
// few window-open signals. The React overlay (original-client-tutorial-overlay.tsx)
// wires DOM events to this reducer; the reducer itself is plain data so it can be
// unit-tested with `node` (see scripts/test-tutorial-flow.mjs).

export type TutorialLang = "en" | "zh-CN" | "es";

export type TutorialWindow = "inventory" | "character" | "questLog";

// Localized snippet. `en` is mandatory (fallback); other locales are optional.
export interface TutorialText {
  en: string;
  "zh-CN"?: string;
  es?: string;
}

export type TutorialTrigger =
  // Completed after the player performs a tracked outbound action N times.
  | { kind: "action"; actionType: string | string[]; count?: number }
  // Completed when a tracked UI window becomes open.
  | { kind: "window"; window: TutorialWindow }
  // Advances only via the Next button (used for intro / summary cards).
  | { kind: "manual" };

export interface TutorialStep {
  id: string;
  title: TutorialText;
  body: TutorialText;
  // Short "how" hint (key / mouse gesture). Rendered emphasized on the card.
  hint?: TutorialText;
  trigger: TutorialTrigger;
  // Optional CSS selector to spotlight. If it doesn't resolve at runtime the
  // overlay degrades gracefully to a centered card (selectors are best-effort —
  // wire real anchors via `data-tutorial="…"` attributes in a follow-up).
  spotlight?: string;
}

export interface TutorialState {
  stepIndex: number;
  // Per-action cumulative counts, used by `count`-gated action triggers.
  actionCounts: Record<string, number>;
  // True once the player finishes or skips the whole flow.
  done: boolean;
}

export type TutorialEvent =
  | { kind: "action"; type: string }
  | { kind: "window"; window: TutorialWindow; open: boolean }
  | { kind: "next" }
  | { kind: "back" }
  | { kind: "skipStep" }
  | { kind: "skipAll" };

export const TUTORIAL_STEPS: TutorialStep[] = [
  {
    id: "welcome",
    title: { en: "Welcome to Mir 2", "zh-CN": "欢迎来到传奇" },
    body: {
      en: "This quick tour covers the basics: moving, fighting, looting, gear, potions and talking to NPCs. You can skip any step or close the tour at any time.",
      "zh-CN": "这个快速教程会带你过一遍基础操作:移动、战斗、拾取、装备、药水和与 NPC 对话。任意一步都可以跳过,也可以随时关闭。",
    },
    trigger: { kind: "manual" },
  },
  {
    id: "move",
    title: { en: "Move around", "zh-CN": "移动" },
    body: {
      en: "Click a nearby tile to walk toward it. Take a few steps.",
      "zh-CN": "点击附近的地面格子向那里走过去。走上几步。",
    },
    hint: { en: "Left-click the ground", "zh-CN": "鼠标左键点击地面" },
    trigger: { kind: "action", actionType: "walk", count: 3 },
  },
  {
    id: "run",
    title: { en: "Run", "zh-CN": "跑步" },
    body: {
      en: "Hold Shift and click to run instead of walk — much faster for travel.",
      "zh-CN": "按住 Shift 再点击就会跑起来,赶路快得多。",
    },
    hint: { en: "Shift + left-click", "zh-CN": "Shift + 左键点击" },
    trigger: { kind: "action", actionType: "run", count: 1 },
  },
  {
    id: "attack",
    title: { en: "Attack a monster", "zh-CN": "攻击怪物" },
    body: {
      en: "Click a monster to attack it. Defeat enemies to gain experience and loot.",
      "zh-CN": "点击怪物即可攻击。击败敌人可以获得经验和掉落。",
    },
    hint: { en: "Click a monster", "zh-CN": "点击怪物" },
    trigger: { kind: "action", actionType: ["attack", "rangeAttack", "magic", "castSkill"], count: 1 },
  },
  {
    id: "pickup",
    title: { en: "Pick up loot", "zh-CN": "拾取战利品" },
    body: {
      en: "Stand on a dropped item and pick it up. Gold and items go into your bag.",
      "zh-CN": "站到掉落物上把它捡起来。金币和物品会进入背包。",
    },
    hint: { en: "Click the item / pick-up key", "zh-CN": "点击掉落物 / 拾取键" },
    trigger: { kind: "action", actionType: "pickUpTile", count: 1 },
  },
  {
    id: "inventory",
    title: { en: "Open your bag", "zh-CN": "打开背包" },
    body: {
      en: "Open the inventory to see everything you are carrying.",
      "zh-CN": "打开背包,查看你携带的所有物品。",
    },
    hint: { en: "Press Alt+I", "zh-CN": "按 Alt+I" },
    trigger: { kind: "window", window: "inventory" },
  },
  {
    id: "equip",
    title: { en: "Equip gear", "zh-CN": "穿戴装备" },
    body: {
      en: "Double-click a weapon or armour in your bag (or drag it onto an equipment slot) to wear it.",
      "zh-CN": "双击背包里的武器或防具(或拖到装备槽)即可穿戴。",
    },
    hint: { en: "Double-click an item", "zh-CN": "双击物品" },
    trigger: { kind: "action", actionType: "equipItem", count: 1 },
  },
  {
    id: "potion",
    title: { en: "Drink a potion", "zh-CN": "使用药水" },
    body: {
      en: "When your HP gets low, use a healing potion from your bag or belt to recover.",
      "zh-CN": "当 HP 偏低时,使用背包或腰带里的治疗药水回血。",
    },
    hint: { en: "Double-click a potion", "zh-CN": "双击药水" },
    trigger: { kind: "action", actionType: "useItem", count: 1 },
  },
  {
    id: "character",
    title: { en: "Check your stats", "zh-CN": "查看角色属性" },
    body: {
      en: "Open the character window to review HP/MP, level, experience and your attack/defence.",
      "zh-CN": "打开角色面板,查看 HP/MP、等级、经验,以及你的攻击/防御。",
    },
    hint: { en: "Press Alt+C", "zh-CN": "按 Alt+C" },
    trigger: { kind: "window", window: "character" },
  },
  {
    id: "skill",
    title: { en: "Cast a skill", "zh-CN": "释放技能" },
    body: {
      en: "Use a magic skill from your skill bar. Bind skills to hotkeys so you can cast them in combat.",
      "zh-CN": "从技能栏释放一个法术/技能。把技能绑到快捷键上,战斗中就能快速施放。",
    },
    hint: { en: "Skill bar / hotkey", "zh-CN": "技能栏 / 快捷键" },
    trigger: { kind: "action", actionType: ["magic", "castSkill"], count: 1 },
  },
  {
    id: "npc",
    title: { en: "Talk to an NPC", "zh-CN": "与 NPC 对话" },
    body: {
      en: "Click an NPC to talk. NPCs run shops, repair gear, store items and hand out quests.",
      "zh-CN": "点击 NPC 开始对话。NPC 提供商店、修理、仓库存取和任务。",
    },
    hint: { en: "Click an NPC", "zh-CN": "点击 NPC" },
    trigger: { kind: "action", actionType: "interact", count: 1 },
  },
  {
    id: "quest",
    title: { en: "Track your quests", "zh-CN": "查看任务" },
    body: {
      en: "Open the quest log to see active quests and their progress, then return to the NPC to turn them in.",
      "zh-CN": "打开任务日志,查看进行中的任务和进度,完成后回到 NPC 处交付。",
    },
    hint: { en: "Press Alt+Q", "zh-CN": "按 Alt+Q" },
    trigger: { kind: "window", window: "questLog" },
  },
  {
    id: "done",
    title: { en: "You're ready!", "zh-CN": "你已经准备好了!" },
    body: {
      en: "That's the core loop: explore, fight, loot, gear up and take quests. Press the Help window (Alt+J) any time for the full reference.",
      "zh-CN": "这就是核心循环:探索、战斗、拾取、强化装备、接取任务。随时可按帮助窗口(Alt+J)查看完整说明。",
    },
    trigger: { kind: "manual" },
  },
];

export function pickText(text: TutorialText, lang: TutorialLang): string {
  return text[lang] ?? text.en;
}

export function createTutorialState(): TutorialState {
  return { stepIndex: 0, actionCounts: {}, done: false };
}

function actionTypesOf(trigger: TutorialTrigger): string[] {
  if (trigger.kind !== "action") return [];
  return Array.isArray(trigger.actionType) ? trigger.actionType : [trigger.actionType];
}

// Is the current step satisfied given accumulated action counts / a window event?
// `justOpened` is the window that just opened (if the event was a window event).
function isStepSatisfied(
  step: TutorialStep,
  counts: Record<string, number>,
  justOpened: TutorialWindow | null,
): boolean {
  const trigger = step.trigger;
  if (trigger.kind === "manual") return false; // only the Next button advances
  if (trigger.kind === "window") return justOpened === trigger.window;
  const needed = trigger.count ?? 1;
  const total = actionTypesOf(trigger).reduce((sum, type) => sum + (counts[type] ?? 0), 0);
  return total >= needed;
}

function clampIndex(index: number): number {
  if (index < 0) return 0;
  if (index > TUTORIAL_STEPS.length - 1) return TUTORIAL_STEPS.length - 1;
  return index;
}

// Pure reducer: (state, event) -> state. The overlay component owns wiring DOM
// events to this; keeping it pure makes the whole flow unit-testable.
export function reduceTutorial(state: TutorialState, event: TutorialEvent): TutorialState {
  if (state.done) return state;

  switch (event.kind) {
    case "skipAll":
      return { ...state, done: true };

    case "next":
    case "skipStep": {
      const nextIndex = state.stepIndex + 1;
      if (nextIndex > TUTORIAL_STEPS.length - 1) {
        return { ...state, done: true };
      }
      return { ...state, stepIndex: nextIndex };
    }

    case "back":
      return { ...state, stepIndex: clampIndex(state.stepIndex - 1) };

    case "action": {
      const counts = { ...state.actionCounts, [event.type]: (state.actionCounts[event.type] ?? 0) + 1 };
      const step = TUTORIAL_STEPS[state.stepIndex];
      const next = { ...state, actionCounts: counts };
      if (step && isStepSatisfied(step, counts, null)) {
        return reduceTutorial(next, { kind: "next" });
      }
      return next;
    }

    case "window": {
      if (!event.open) return state;
      const step = TUTORIAL_STEPS[state.stepIndex];
      if (step && isStepSatisfied(step, state.actionCounts, event.window)) {
        return reduceTutorial(state, { kind: "next" });
      }
      return state;
    }

    default:
      return state;
  }
}

export function currentStep(state: TutorialState): TutorialStep | null {
  return TUTORIAL_STEPS[state.stepIndex] ?? null;
}
