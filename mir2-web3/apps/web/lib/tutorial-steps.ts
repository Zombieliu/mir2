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

export type TutorialLang = "en" | "zh-CN" | "es" | "pt-BR";
export type TutorialInputProfile = "keyboardMouse" | "touch" | "gamepad";
export type TutorialGamepadFamily = "xbox" | "playstation" | "generic";

export const TUTORIAL_VERSION = 3;
export const TUTORIAL_CONTROL_EVENT = "mir2:tutorial-control";

export type TutorialWindow = "inventory" | "character" | "questLog";

// Localized snippet. `en` is mandatory (fallback); other locales are optional.
export interface TutorialText {
  en: string;
  "zh-CN"?: string;
  es?: string;
  "pt-BR"?: string;
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
  // Optional CSS selector to spotlight. Points at existing, stable class hooks
  // already present in the HUD (`.hud-button.*`, `.belt-dialog`) so no
  // presentational component needs editing. Only small, specific targets are
  // spotlit — the play area (`.client-stage-frame`) is the full 1024×768 client,
  // so ringing it just outlines the whole window and the dim falls on the black
  // letterbox; those steps rely on the card hint instead. If a selector doesn't
  // resolve at runtime the overlay degrades gracefully to a card-only step (e.g.
  // the belt bar can be toggled off).
  spotlight?: string;
}

export interface TutorialState {
  input: TutorialInputProfile;
  gamepadFamily: TutorialGamepadFamily;
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

const KEYBOARD_MOUSE_TUTORIAL_STEPS: TutorialStep[] = [
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
    spotlight: ".hud-button.inventory",
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
    spotlight: ".hud-button.character",
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
    spotlight: ".belt-dialog",
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
    spotlight: ".hud-button.quest",
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

const TOUCH_TUTORIAL_STEPS: TutorialStep[] = [
  {
    id: "touch-welcome",
    title: { en: "Touch controls", "zh-CN": "移动端操作" },
    body: {
      en: "This quick tour explains every control around the game screen. You can keep playing while the guide is open, skip any step, or replay it later from Menu → Help.",
      "zh-CN": "这个快速教程会介绍游戏画面两侧的全部操作。教程打开时仍可继续游玩；任意步骤都能跳过，也可之后从“菜单 → 帮助”重新播放。",
    },
    trigger: { kind: "manual" },
  },
  {
    id: "touch-move",
    title: { en: "Drag the movement stick", "zh-CN": "拖动摇杆移动" },
    body: {
      en: "Press and drag the left stick toward the direction you want to travel. Release it to stop.",
      "zh-CN": "按住左侧摇杆并朝目标方向拖动，角色就会移动；松开后停止。",
    },
    hint: { en: "Drag the left joystick", "zh-CN": "拖动左侧摇杆试一试" },
    trigger: { kind: "action", actionType: "touch:move" },
    spotlight: ".mir-mobile-stick-shell",
  },
  {
    id: "touch-run",
    title: { en: "Switch Walk / Run", "zh-CN": "切换步行 / 跑步" },
    body: {
      en: "Run is the movement-mode switch. When it is lit, the stick runs; tap it again to walk for precise movement.",
      "zh-CN": "Run 是移动模式开关。亮起时拖动摇杆会跑步；再次点击会切换为步行，方便精确走位。",
    },
    hint: { en: "Tap Run to switch modes", "zh-CN": "点击 Run 切换移动模式" },
    trigger: { kind: "action", actionType: "touch:run" },
    spotlight: ".mir-mobile-action.run",
  },
  {
    id: "touch-attack",
    title: { en: "Attack", "zh-CN": "攻击目标" },
    body: {
      en: "Select a monster, then tap Attack for the primary combat action.",
      "zh-CN": "先选择怪物，再点击 Attack 执行主要攻击。",
    },
    hint: { en: "Select a target, then tap Attack", "zh-CN": "选择目标后点击 Attack" },
    trigger: { kind: "action", actionType: "touch:attack" },
    spotlight: ".mir-mobile-action.primary",
  },
  {
    id: "touch-approach",
    title: { en: "Approach a target", "zh-CN": "接近目标" },
    body: {
      en: "Approach moves you toward the selected monster or NPC without immediately attacking.",
      "zh-CN": "Approach 会让角色接近当前选中的怪物或 NPC，但不会立刻攻击。",
    },
    hint: { en: "Select a target, then tap Approach", "zh-CN": "选择目标后点击 Approach" },
    trigger: { kind: "action", actionType: "touch:approach" },
    spotlight: ".mir-mobile-action.approach",
  },
  {
    id: "touch-pick",
    title: { en: "Pick up loot", "zh-CN": "拾取掉落物" },
    body: {
      en: "Pick selects the nearest visible drop and walks over to collect it. The button activates when loot is nearby.",
      "zh-CN": "Pick 会选择最近的可见掉落物，并走过去拾取；附近有掉落物时按钮才会启用。",
    },
    hint: { en: "Tap Pick when a drop is nearby", "zh-CN": "附近有掉落物时点击 Pick" },
    trigger: { kind: "action", actionType: "touch:pick" },
    spotlight: ".mir-mobile-action.pick",
  },
  {
    id: "touch-quick",
    title: { en: "Skills and quick items", "zh-CN": "技能与快捷物品" },
    body: {
      en: "S1–S3 cast your first skills. Numbered buttons use belt items such as potions. Empty slots show a plus sign.",
      "zh-CN": "S1–S3 会释放前几个技能；数字按钮使用腰带里的药水等物品；空槽会显示加号。",
    },
    hint: { en: "Tap an S or numbered quick slot", "zh-CN": "点击一个 S 或数字快捷按钮" },
    trigger: { kind: "action", actionType: "touch:quick" },
    spotlight: ".mir-mobile-action.quick",
  },
  {
    id: "touch-panels",
    title: { en: "Character and Bag", "zh-CN": "角色与背包" },
    body: {
      en: "Char opens equipment and stats. Bag opens inventory, quests and storage tabs. Tap either button again to close it.",
      "zh-CN": "Char 打开装备和属性；Bag 打开背包、任务和仓库页签。再次点击对应按钮即可关闭。",
    },
    hint: { en: "Tap Char or Bag", "zh-CN": "点击 Char 或 Bag" },
    trigger: { kind: "action", actionType: "touch:panel" },
    spotlight: ".mir-mobile-panel-row",
  },
  {
    id: "touch-replay",
    title: { en: "Replay this guide", "zh-CN": "重新播放教程" },
    body: {
      en: "Open the round Menu button, choose Help, then select Replay controls tutorial whenever you need a refresher.",
      "zh-CN": "需要复习时，打开圆形“菜单”按钮，选择“帮助”，再点击“重新播放操作教学”。",
    },
    hint: { en: "Menu → Help → Replay controls tutorial", "zh-CN": "菜单 → 帮助 → 重新播放操作教学" },
    trigger: { kind: "manual" },
    spotlight: ".hud-button.menu",
  },
  {
    id: "touch-done",
    title: { en: "Ready to play", "zh-CN": "可以开始冒险了" },
    body: {
      en: "Use the stick to move, Run to choose speed, the right-side buttons for combat, and Char or Bag for your panels.",
      "zh-CN": "用摇杆移动、Run 选择速度、右侧按钮进行战斗，并通过 Char 或 Bag 管理角色与物品。",
    },
    trigger: { kind: "manual" },
  },
];

function tutorialGamepadLabels(family: TutorialGamepadFamily) {
  if (family === "playstation") {
    return {
      primary: "×",
      cancel: "○",
      pick: "□",
      approach: "△",
      leftBumper: "L1",
      rightBumper: "R1",
      leftTrigger: "L2",
      rightTrigger: "R2",
      view: "Create",
      menu: "Options",
    };
  }
  if (family === "xbox") {
    return {
      primary: "A",
      cancel: "B",
      pick: "X",
      approach: "Y",
      leftBumper: "LB",
      rightBumper: "RB",
      leftTrigger: "LT",
      rightTrigger: "RT",
      view: "View",
      menu: "Menu",
    };
  }
  return {
    primary: "1",
    cancel: "2",
    pick: "3",
    approach: "4",
    leftBumper: "L1",
    rightBumper: "R1",
    leftTrigger: "L2",
    rightTrigger: "R2",
    view: "Select",
    menu: "Start",
  };
}

function createGamepadTutorialSteps(family: TutorialGamepadFamily): TutorialStep[] {
  const labels = tutorialGamepadLabels(family);
  const controllerName =
    family === "playstation"
      ? { en: "PlayStation controller", "zh-CN": "PlayStation 手柄" }
      : family === "xbox"
        ? { en: "Xbox controller", "zh-CN": "Xbox 手柄" }
        : { en: "game controller", "zh-CN": "游戏手柄" };

  return [
    {
      id: "gamepad-welcome",
      title: {
        en: family === "playstation" ? "PlayStation controls" : "Controller controls",
        "zh-CN": family === "playstation" ? "PlayStation 手柄操作" : "手柄操作",
      },
      body: {
        en: `This tour covers movement, combat, shortcuts and menu navigation for your ${controllerName.en}.`,
        "zh-CN": `这个教程会介绍${controllerName["zh-CN"]}的移动、战斗、快捷操作和菜单导航。`,
      },
      trigger: { kind: "manual" },
    },
    {
      id: "gamepad-move",
      title: { en: "Move", "zh-CN": "移动" },
      body: {
        en: "Use the left stick or D-pad to move. Push farther to run and ease the stick for precise movement.",
        "zh-CN": "使用左摇杆或方向键移动。大幅推动摇杆会跑步，轻推可精确走位。",
      },
      hint: { en: "Left stick / D-pad", "zh-CN": "左摇杆 / 方向键" },
      trigger: { kind: "action", actionType: "gamepad:move" },
    },
    {
      id: "gamepad-actions",
      title: { en: "Combat actions", "zh-CN": "战斗操作" },
      body: {
        en: `${labels.primary} performs the primary action, ${labels.approach} approaches the selected target, and ${labels.pick} picks up the nearest drop.`,
        "zh-CN": `${labels.primary} 执行主要操作，${labels.approach} 接近选中目标，${labels.pick} 拾取最近的掉落物。`,
      },
      hint: {
        en: `${labels.primary}: Attack · ${labels.approach}: Approach · ${labels.pick}: Pick`,
        "zh-CN": `${labels.primary}：攻击 · ${labels.approach}：接近 · ${labels.pick}：拾取`,
      },
      trigger: { kind: "action", actionType: ["gamepad:primary", "gamepad:approach", "gamepad:pick"] },
    },
    {
      id: "gamepad-quick",
      title: { en: "Skills and items", "zh-CN": "技能与物品" },
      body: {
        en: `${labels.leftTrigger} and ${labels.rightTrigger} cast skill slots 1 and 2. ${labels.leftBumper} and ${labels.rightBumper} use belt items 1 and 2.`,
        "zh-CN": `${labels.leftTrigger}、${labels.rightTrigger} 释放技能槽 1、2；${labels.leftBumper}、${labels.rightBumper} 使用腰带物品 1、2。`,
      },
      hint: {
        en: `${labels.leftTrigger} / ${labels.rightTrigger}: Skills · ${labels.leftBumper} / ${labels.rightBumper}: Items`,
        "zh-CN": `${labels.leftTrigger} / ${labels.rightTrigger}：技能 · ${labels.leftBumper} / ${labels.rightBumper}：物品`,
      },
      trigger: { kind: "action", actionType: "gamepad:quick" },
    },
    {
      id: "gamepad-panels",
      title: { en: "Character and Bag", "zh-CN": "角色与背包" },
      body: {
        en: `${labels.view} opens Character. ${labels.menu} opens Bag. While a panel is open, the D-pad moves focus, ${labels.primary} activates and ${labels.cancel} goes back.`,
        "zh-CN": `${labels.view} 打开角色，${labels.menu} 打开背包。面板打开时，用方向键移动焦点、${labels.primary} 确认、${labels.cancel} 返回。`,
      },
      hint: {
        en: `${labels.view}: Character · ${labels.menu}: Bag`,
        "zh-CN": `${labels.view}：角色 · ${labels.menu}：背包`,
      },
      trigger: { kind: "action", actionType: "gamepad:panel" },
    },
    {
      id: "gamepad-replay",
      title: { en: "Replay this guide", "zh-CN": "重新播放教程" },
      body: {
        en: "Open the game Menu, choose Help, then activate Replay controls tutorial.",
        "zh-CN": "打开游戏菜单，选择“帮助”，再确认“重新播放操作教学”。",
      },
      trigger: { kind: "manual" },
    },
    {
      id: "gamepad-done",
      title: { en: "Controller ready", "zh-CN": "手柄已准备就绪" },
      body: {
        en: "You can complete movement, combat and the core UI without a mouse or keyboard.",
        "zh-CN": "现在无需鼠标和键盘，也能完成移动、战斗和核心界面操作。",
      },
      trigger: { kind: "manual" },
    },
  ];
}

const GAMEPAD_TUTORIAL_STEPS: Record<TutorialGamepadFamily, TutorialStep[]> = {
  xbox: createGamepadTutorialSteps("xbox"),
  playstation: createGamepadTutorialSteps("playstation"),
  generic: createGamepadTutorialSteps("generic"),
};

export const TUTORIAL_STEPS = KEYBOARD_MOUSE_TUTORIAL_STEPS;

export function tutorialStepsForInput(
  input: TutorialInputProfile,
  gamepadFamily: TutorialGamepadFamily = "generic",
): TutorialStep[] {
  if (input === "touch") return TOUCH_TUTORIAL_STEPS;
  if (input === "gamepad") return GAMEPAD_TUTORIAL_STEPS[gamepadFamily];
  return KEYBOARD_MOUSE_TUTORIAL_STEPS;
}

export function tutorialCompletionStorageKey(
  input: TutorialInputProfile,
  gamepadFamily: TutorialGamepadFamily = "generic",
): string {
  const inputKey = input === "gamepad" ? `${input}:${gamepadFamily}` : input;
  return `mir2:tutorialCompleted:v${TUTORIAL_VERSION}:${inputKey}`;
}

export function pickText(text: TutorialText, lang: TutorialLang): string {
  return text[lang] ?? text.en;
}

export function createTutorialState(
  input: TutorialInputProfile = "keyboardMouse",
  gamepadFamily: TutorialGamepadFamily = "generic",
): TutorialState {
  return { input, gamepadFamily, stepIndex: 0, actionCounts: {}, done: false };
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

function clampIndex(index: number, maxIndex: number): number {
  if (index < 0) return 0;
  if (index > maxIndex) return maxIndex;
  return index;
}

// Pure reducer: (state, event) -> state. The overlay component owns wiring DOM
// events to this; keeping it pure makes the whole flow unit-testable.
export function reduceTutorial(state: TutorialState, event: TutorialEvent): TutorialState {
  if (state.done) return state;
  const steps = tutorialStepsForInput(state.input, state.gamepadFamily);

  switch (event.kind) {
    case "skipAll":
      return { ...state, done: true };

    case "next":
    case "skipStep": {
      const nextIndex = state.stepIndex + 1;
      if (nextIndex > steps.length - 1) {
        return { ...state, done: true };
      }
      return { ...state, stepIndex: nextIndex };
    }

    case "back":
      return { ...state, stepIndex: clampIndex(state.stepIndex - 1, steps.length - 1) };

    case "action": {
      const counts = { ...state.actionCounts, [event.type]: (state.actionCounts[event.type] ?? 0) + 1 };
      const step = steps[state.stepIndex];
      const next = { ...state, actionCounts: counts };
      if (step && isStepSatisfied(step, counts, null)) {
        return reduceTutorial(next, { kind: "next" });
      }
      return next;
    }

    case "window": {
      if (!event.open) return state;
      const step = steps[state.stepIndex];
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
  return tutorialStepsForInput(state.input, state.gamepadFamily)[state.stepIndex] ?? null;
}
