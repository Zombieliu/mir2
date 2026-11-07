import type { Mir2Language } from "./localization";
import { nativeText } from "./r16-native-localization";
import { browserText } from "./r16-browser-localization";

export type R16AuthRect = Readonly<{ left: number; top: number; width: number; height: number }>;
const rect = (left: number, top: number, width: number, height: number): R16AuthRect => ({ left, top, width, height });

/** Logical stage coordinates from the canonical R16 native shell, not CSS viewport pixels. */
export const R16_AUTH_LAYOUT = {
  stage: rect(0, 0, 1024, 768),
  footer: {
    language: { right: 12, bottom: 12, width: 240, minHeight: 29 },
    languageMenuBottom: 33,
    browser: { left: 12, bottom: 12, width: 328, minHeight: 50 },
    browserMenuBottom: 56,
    // Keep the footer clear of native select actions at y=736..761.
    selectLift: 36,
  },
  login: {
    panel: rect(348, 274, 328, 220),
    title: rect(368, 286, 288, 28),
    accountLabel: rect(352, 355, 77, 21),
    passwordLabel: rect(352, 378, 77, 21),
    account: rect(433, 359, 136, 15),
    password: rect(433, 382, 136, 15),
    ok: rect(575, 355, 42, 42),
    newAccount: rect(408, 437, 100, 25),
    changePassword: rect(514, 437, 100, 25),
    safeKey: rect(408, 463, 100, 25),
    cancel: rect(514, 463, 100, 25),
    notice: rect(348, 502, 328, 38),
    wallet: rect(378, 550, 268, 34),
  },
  registration: {
    panel: rect(218, 154, 588, 460), title: rect(238, 178, 548, 30),
    fields: {
      accountId: rect(444, 257, 136, 18), password: rect(444, 283, 136, 18), confirmPassword: rect(444, 309, 136, 18),
      userName: rect(444, 343, 136, 18), birthDate: rect(444, 369, 136, 18),
      secretQuestion: rect(444, 404, 190, 18), secretAnswer: rect(444, 430, 190, 18), emailAddress: rect(444, 465, 136, 18),
    },
    description: rect(233, 494, 300, 70), notice: rect(538, 494, 252, 70),
    submit: rect(353, 579, 76, 25), cancel: rect(627, 579, 76, 25),
  },
  changePassword: {
    panel: rect(338, 250, 348, 268), title: rect(348, 265, 328, 26),
    fields: { accountId: rect(516, 325, 136, 18), oldPassword: rect(516, 363, 136, 18),
      newPassword: rect(516, 401, 136, 18), confirmPassword: rect(516, 438, 136, 18) },
    submit: rect(418, 486, 90, 25), cancel: rect(560, 486, 68, 25), notice: rect(338, 524, 348, 40),
  },
  safeKey: {
    panel: rect(689, 395, 216, 278), esc: rect(701, 407, 32, 32),
    delete: rect(829, 471, 64, 32), enter: rect(829, 631, 64, 32), random: rect(765, 631, 64, 32),
  },
  select: {
    title: rect(312, 16, 400, 34),
    notice: rect(262, 678, 500, 22),
    slotLeft: 637,
    slotTops: [194, 298, 402, 506],
    slotWidth: 288,
    occupiedSlotHeight: 56,
    emptySlotHeight: 54,
    buttons: { start: rect(132, 736, 100, 25), new: rect(296, 736, 100, 25),
      delete: rect(460, 736, 100, 25), credits: rect(624, 736, 100, 25), exit: rect(788, 736, 100, 25) },
    previewDelayMs: 250,
  },
  create: {
    panel: rect(218, 154, 588, 460),
    title: rect(238, 168, 548, 32),
    preview: { x: 338, y: 404 },
    description: rect(497, 224, 278, 170),
    nameLabel: rect(497, 399, 278, 19),
    name: rect(543, 422, 240, 20),
    classLabel: rect(497, 446, 278, 18),
    genderLabel: rect(497, 519, 278, 18),
    classes: [rect(497, 465, 90, 25), rect(591, 465, 90, 25), rect(685, 465, 90, 25),
      rect(497, 493, 137, 25), rect(638, 493, 137, 25)],
    genders: [rect(497, 539, 137, 27), rect(638, 539, 137, 27)],
    notice: rect(628, 610, 304, 28),
    submit: rect(378, 579, 100, 25), cancel: rect(643, 579, 100, 25),
    maxNameLength: 18,
  },
} as const;

export function r16AuthRelativeRect(value: R16AuthRect, parent: R16AuthRect): R16AuthRect {
  return { ...value, left: value.left - parent.left, top: value.top - parent.top };
}

export function r16AuthSafeKeyRect(index: number): R16AuthRect | null {
  if (!Number.isInteger(index) || index < 0 || index >= 36) return null;
  const slot = index < 10 ? index : index - 10;
  const row = index < 10 ? Math.floor(index / 6) : 2 + Math.floor(slot / 6);
  return rect(701 + slot % 6 * 32, 439 + row * 32, 32, 30);
}

export const R16_AUTH_ASSETS = {
  safePanel: "/original-ui/Prguse/1080.png",
  safeKey: { base: "/original-ui/Prguse/1081.png", hover: "/original-ui/Prguse/1082.png", pressed: "/original-ui/Prguse/1083.png" },
  passwordSubmit: { base: "/original-ui/Title/107.png", hover: "/original-ui/Title/108.png", pressed: "/original-ui/Title/109.png" },
  passwordCancel: { base: "/original-ui/Title/110.png", hover: "/original-ui/Title/111.png", pressed: "/original-ui/Title/112.png" },
} as const;

/** Bounded source projection of native-i18n/shell.json + extra/zh-CN.json.
 * Tests compare every caption to those canonical catalogs, so the browser does
 * not ship a second copy of the entire native vocabulary for two screens. */
export const R16_AUTH_CAPTIONS = {
  login: ["Login", "登录", "Entrar"], account: ["Account", "账号", "Conta"],
  password: ["Password", "密码", "Senha"], ok: ["OK", "确定", "OK"],
  current_password: ["Current Password", "当前密码", "Senha atual"], new_password: ["New Password", "新密码", "Nova senha"],
  confirm_password: ["Confirm Password", "确认密码", "Confirmar senha"], user_name: ["Name (optional)", "姓名（选填）", "Nome (opcional)"],
  birth_date: ["Birth Date (optional)", "出生日期（选填）", "Nascimento (opcional)"],
  secret_question: ["Security Question (optional)", "安全问题（选填）", "Pergunta secreta (opcional)"],
  secret_answer: ["Security Answer (optional)", "安全答案（选填）", "Resposta secreta (opcional)"], email: ["Email (optional)", "电子邮件（选填）", "E-mail (opcional)"],
  delete: ["Delete", "删除", "Excluir"], enter: ["Enter", "确认", "Confirmar"], random: ["Random", "重排", "Embaralhar"],
  "registration.account_hint": ["Account: 3–15 letters or digits.", "账号：3–15 位英文字母或数字。", "Conta: 3–15 letras ou números."],
  "registration.password_hint": ["Password: 10–15 letters or digits. Must differ from the account and avoid common passwords. Both entries must match.", "密码：10–15 位英文字母或数字。不可与账号相同，也不可使用常见弱密码；两次输入须一致。", "Senha: 10–15 letras ou números. Não use a conta nem senhas comuns. As duas entradas devem ser iguais."],
  "registration.name_hint": ["Name (optional): up to 20 characters.", "姓名（选填）：最多 20 个字符。", "Nome (opcional): até 20 caracteres."],
  "registration.birth_hint": ["Birth date (optional): YYYY-MM-DD.", "出生日期（选填）：格式为 YYYY-MM-DD。", "Nascimento (opcional): formato AAAA-MM-DD."],
  "registration.question_hint": ["Security question (optional): up to 30 characters.", "安全问题（选填）：最多 30 个字符。", "Pergunta secreta (opcional): até 30 caracteres."],
  "registration.answer_hint": ["Security answer (optional): up to 30 characters.", "安全答案（选填）：最多 30 个字符。", "Resposta secreta (opcional): até 30 caracteres."],
  "registration.email_hint": ["Email (optional): a valid full address, up to 50 characters.", "电子邮件（选填）：填写完整有效的邮箱地址，最多 50 个字符。", "E-mail (opcional): endereço completo e válido, até 50 caracteres."],
  "registration.submit_hint": ["Create an account using the information above.", "使用以上信息创建账号。", "Criar uma conta com os dados acima."],
  "registration.cancel_hint": ["Return to Login.", "返回登录画面。", "Voltar à tela de login."],
  new_account: ["New Account", "创建账号", "Criar conta"],
  change_password: ["Change Password", "修改密码", "Alterar senha"],
  safe_key: ["Safe Key", "安全键盘", "Teclado seguro"], cancel: ["Cancel", "取消", "Cancelar"],
  start: ["Start", "开始", "Iniciar"], create: ["Create", "创建", "Criar"],
  delete_character: ["Delete Character", "删除角色", "Excluir personagem"],
  credits: ["Credits", "制作名单", "Créditos"], exit: ["Exit", "离开", "Sair"],
  character_select: ["Select Character", "选择角色", "Selecionar personagem"],
  character_create: ["Create Character", "创建角色", "Criar personagem"],
  character_name: ["Character Name", "角色名称", "Nome do personagem"],
  class: ["Class", "职业", "Classe"], gender: ["Gender", "性别", "Gênero"],
  male: ["Male", "男", "Masculino"], female: ["Female", "女", "Feminino"],
  empty_slot: ["Empty Slot", "空角色槽位", "Espaço vazio"],
  last_online: ["Last Online:", "上次上线：", "Último acesso:"], never: ["Never", "尚未登录", "Nunca"],
  level: ["Lv. {level}", "等级 {level}", "Nv. {level}"],
  warrior: ["Warrior", "战士", "Guerreiro"], wizard: ["Wizard", "法师", "Mago"],
  taoist: ["Taoist", "道士", "Taoísta"], assassin: ["Assassin", "刺客", "Assassino"],
  archer: ["Archer", "弓箭手", "Arqueiro"],
  warrior_description: ["Warriors are resilient frontline fighters.\nThey favor close combat and heavy weapons.", "战士是坚韧的前线战斗者。\n擅长近身战斗与重型武器。", "Guerreiros são resistentes na linha de frente.\nPreferem combate corpo a corpo e armas pesadas."],
  wizard_description: ["Wizards command powerful elemental magic.\nThey favor ranged spells and careful positioning.", "法师掌握强大的元素魔法。\n擅长远程法术，需留意站位。", "Magos dominam a magia elemental.\nUsam feitiços à distância e precisam se posicionar bem."],
  taoist_description: ["Taoists support allies and master spiritual arts.\nThey balance healing, buffs, and combat.", "道士精通灵术，能支持队友。\n兼顾治疗、辅助与战斗。", "Taoístas apoiam aliados com artes espirituais.\nEquilibram cura, melhorias e combate."],
  assassin_description: ["Assassins strike with paired blades and stealth.\nThey favor quick attacks and careful movement.", "刺客以双刃与潜行出击。\n擅长快速攻击，并需灵活走位。", "Assassinos atacam com duas lâminas e furtividade.\nPreferem golpes rápidos e movimento cuidadoso."],
  archer_description: ["Archers attack from range with bows and special shots.\nKeep your distance and watch your positioning.", "弓手以弓箭和特殊射击远攻。\n保持距离，留意站位。", "Arqueiros atacam à distância com arcos e tiros especiais.\nMantenha distância e cuide da posição."],
} as const;

export type R16AuthCaption = keyof typeof R16_AUTH_CAPTIONS;
export const R16_AUTH_TRADITIONAL: Record<R16AuthCaption, string> = {
  login: "登入", account: "帳號", password: "密碼", ok: "確定", new_account: "建立帳號",
  current_password: "目前密碼", new_password: "新密碼", confirm_password: "確認密碼", user_name: "姓名（選填）",
  birth_date: "出生日期（選填）", secret_question: "安全問題（選填）", secret_answer: "安全答案（選填）", email: "電子郵件（選填）",
  delete: "刪除", enter: "確認", random: "重排",
  "registration.account_hint": "帳號：3–15 位英文字母或數字。",
  "registration.password_hint": "密碼：10–15 位英文字母或數字。不可與帳號相同，也不可使用常見弱密碼；兩次輸入須一致。",
  "registration.name_hint": "姓名（選填）：最多 20 個字元。", "registration.birth_hint": "出生日期（選填）：格式為 YYYY-MM-DD。",
  "registration.question_hint": "安全問題（選填）：最多 30 個字元。", "registration.answer_hint": "安全答案（選填）：最多 30 個字元。",
  "registration.email_hint": "電子郵件（選填）：填寫完整有效的信箱地址，最多 50 個字元。",
  "registration.submit_hint": "使用以上資料建立帳號。", "registration.cancel_hint": "返回登入畫面。",
  change_password: "變更密碼", safe_key: "安全鍵盤", cancel: "取消", start: "開始", create: "建立",
  delete_character: "刪除角色", credits: "製作名單", exit: "離開", character_select: "選擇角色",
  character_create: "建立角色", character_name: "角色名稱", class: "職業", gender: "性別", male: "男", female: "女",
  empty_slot: "空角色欄位", last_online: "上次上線：", never: "尚未登入", level: "等級 {level}",
  warrior: "戰士", wizard: "法師", taoist: "道士", assassin: "刺客", archer: "弓箭手",
  warrior_description: "戰士是堅韌的前線戰鬥者。\n擅長近身戰鬥與重型武器。",
  wizard_description: "法師掌握強大的元素魔法。\n擅長遠距法術，需留意站位。",
  taoist_description: "道士精通靈術，能支援隊友。\n兼顧治療、輔助與戰鬥。",
  assassin_description: "刺客以雙刃與潛行出擊。\n擅長快速攻擊，並需靈活走位。",
  archer_description: "弓手以弓箭和特殊射擊遠攻。\n保持距離，留意站位。",
};
export function r16AuthCaption(language: Mir2Language | "zh-TW", key: R16AuthCaption, spanishFallback?: string): string {
  const values = R16_AUTH_CAPTIONS[key];
  return language === "es" && spanishFallback ? spanishFallback : nativeText(language, `shell.${key}`, values[0]);
}

const BROWSER_CAPTIONS = {
  settings: ["Browser settings", "浏览器设置", "瀏覽器設定", "Configurações do navegador", "Ajustes del navegador"],
  settingsDetail: ["Audio · account · sign-in options", "声音 · 账号 · 登录方式", "聲音 · 帳號 · 登入方式", "Áudio · conta · acesso", "Audio · cuenta · acceso"],
  accountSecurity: ["Account recovery / security", "账号恢复与安全", "帳號復原與安全", "Recuperação e segurança da conta", "Recuperación y seguridad de la cuenta"],
  accountInfo: ["Account Info", "账号信息", "帳號資訊", "Informações da conta", "Información de la cuenta"],
  signIn: ["Other sign-in options", "其他登录方式", "其他登入方式", "Outras formas de entrar", "Otras formas de entrar"],
  diagnostics: ["Connection diagnostics", "连接诊断", "連線診斷", "Diagnóstico da conexão", "Diagnóstico de conexión"],
} as const;

export function r16BrowserCaption(language: string, key: keyof typeof BROWSER_CAPTIONS): string {
  const caption = browserText(language, `browser.${key}`);
  if (caption !== undefined) return caption;
  const index = language === "zh-CN" ? 1 : language === "zh-TW" ? 2 : language === "pt-BR" ? 3 : language === "es" ? 4 : 0;
  return BROWSER_CAPTIONS[key][index];
}

export const R16_LOGIN_FOCUS_ORDER = ["accountId", "password", "login", "newAccount"] as const;
export type R16LoginFocus = typeof R16_LOGIN_FOCUS_ORDER[number];
export function r16NextLoginFocus(current: R16LoginFocus, backwards = false): R16LoginFocus {
  const index = R16_LOGIN_FOCUS_ORDER.indexOf(current);
  return R16_LOGIN_FOCUS_ORDER[(index + (backwards ? 3 : 1)) % R16_LOGIN_FOCUS_ORDER.length];
}

export function r16SelectAvailability(characters: ReadonlyArray<{ synthetic?: boolean }>, selectedSlot: number) {
  const selected = Number.isInteger(selectedSlot) && selectedSlot >= 0 ? characters[selectedSlot] : undefined;
  return { hasSelection: !!selected && !selected.synthetic,
    canCreate: characters.filter(character => !character.synthetic).length < 4 };
}

/** Crystal's Wizard preview adds a second, additive layer on the same clock. */
export const R16_WIZARD_PREVIEW_OFFSETS = {
  male: [[-90, -244], [-91, -245], [-94, -246], [-96, -246], [-97, -245], [-99, -245], [-102, -247], [-104, -247],
    [-106, -247], [-104, -247], [-101, -247], [-99, -245], [-98, -245], [-96, -246], [-94, -245], [-91, -245]],
  female: [[-59, -219], [-61, -219], [-62, -220], [-65, -220], [-67, -220], [-67, -221], [-70, -221], [-70, -221],
    [-71, -221], [-70, -221], [-69, -221], [-69, -221], [-67, -220], [-65, -220], [-63, -219], [-61, -219]],
} as const;

export function r16WizardPreviewOverlay(classKey: string | undefined, gender: "male" | "female" | undefined, frame: number) {
  if (classKey !== "wizard" || !gender || !Number.isInteger(frame) || frame < 0) return null;
  const index = frame % 16;
  const [x, y] = R16_WIZARD_PREVIEW_OFFSETS[gender][index];
  return { path: `/original-ui/ChrSel/${(gender === "female" ? 880 : 600) + index}.png`, x, y };
}
