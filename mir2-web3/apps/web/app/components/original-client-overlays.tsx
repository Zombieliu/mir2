"use client";

import { useEffect, useRef, useState, type ReactNode } from "react";

import {
  ORIGINAL_UI,
  type CharacterTabKey,
  type InventoryTabKey,
  type SpriteState,
} from "../../lib/original-ui";
import {
  SELECT_PORTRAIT_ANCHOR,
  SELECT_PORTRAIT_ANIMATIONS,
  type SelectPortraitFrame,
  type SelectPortraitKey,
} from "../../lib/select-portraits";
import {
  languageNativeName,
  languageFontFamily,
  SUPPORTED_LANGUAGES,
  type Mir2Language,
} from "../../lib/localization";
import type { SuiWalletSummary, LoginAuthControls, RegistrationDraft, ChangePasswordDraft } from "../../lib/client-login-runtime";
import type { CharacterCreateControls } from "../../lib/character-create";
import { crystalMainHudExperienceBarFillWidth } from "../../lib/crystal-hud-metrics";
import { formatCrystalExperiencePercent } from "../../lib/extended-server-packets";
import { playOriginalSoundId } from "../../lib/original-audio";
import { ORIGINAL_SOUND_IDS } from "../../lib/original-sound-events";
import { OriginalAudioSettingsControls } from "./original-client-audio-settings";
import { CrystalGdiTextImage, findCrystalGdiTextAsset } from "./crystal-gdi-text";
import { DubheWalletQrLogin, type DubheQrLoginState } from "./dubhe-wallet-qr-login";
import {
  R16_AUTH_LAYOUT,
  R16_AUTH_ASSETS,
  r16AuthCaption,
  r16AuthRelativeRect,
  r16AuthSafeKeyRect,
  r16BrowserCaption,
  r16NextLoginFocus,
  r16SelectAvailability,
  r16WizardPreviewOverlay,
  type R16LoginFocus,
  type R16AuthCaption,
} from "../../lib/r16-auth-layout";
import {
  handleSceneAssetImageError,
  handleSceneAssetImageLoad,
} from "./original-client-scene-map-rendering";
import type {
  EntityClassKey,
  EntityGenderKey,
} from "./original-client-types";

type TranslateFn = (
  key: string,
  params?: Array<string | number>,
  fallback?: string,
) => string;

type SelectCharacterEntryLike = {
  index: number;
  name: string;
  level: number;
  classKey: EntityClassKey;
  gender: EntityGenderKey;
  lastAccess?: string | null;
  // Synthesized placeholder for an account with no real character — rendered as
  // an empty slot, and "Start" routes to character creation instead of a doomed
  // startGame.
  synthetic?: boolean;
};

type HudPlayerLike = {
  name: string;
  level?: number;
  classKey?: EntityClassKey;
} | null;

type HudWorldLike = {
  mapTitle: string | null;
  inSafeZone: boolean;
  playerHp?: number;
  playerMaxHp?: number;
  playerMp?: number;
  playerMaxMp?: number;
  playerExperience?: number;
  playerMaxExperience?: number;
  activeBuffs: Array<{ name: string; remainingTicks: number }>;
  gold: number;
  freeBagSlots: number;
  maxBagSlots: number;
  currentWeight: number;
  maxWeight: number;
  beltItems?: unknown[];
  inventoryItems?: unknown[];
};

const CRYSTAL_MAIN_INVENTORY_SLOT_COUNT = 46;
const CRYSTAL_MAIN_HUD_WEIGHT_BAR_WIDTH = 76;
const CREATE_GENDER_OPTIONS: EntityGenderKey[] = ["male", "female"];

// Native R16 replaces baked English word art with these localized faces. Scope
// this presentation to auth screens; in-game SpriteButton users keep their art.
const R16_AUTH_STYLES = `
[data-r16-auth] { font-family: "Noto Sans SC", "Microsoft YaHei", Arial, sans-serif; color: #fff; }
[data-r16-auth] .r16-auth-label { position: absolute; margin: 0; font-weight: 400; overflow: hidden; }
[data-r16-auth="login"] .r16-auth-label { color: rgb(245,240,230); }
[data-r16-auth="login"] .login-input { box-sizing: border-box; padding: 0 2px; border: 0; border-radius: 0; background: #000; box-shadow: none; color: rgb(245,240,230); font: inherit; font-size: 10px; line-height: 15px; }
[data-r16-auth] .login-button .sprite-button, [data-r16-auth] .select-action .sprite-button,
[data-r16-auth] .select-create-actions .sprite-button { width: 100%; height: 100%; }
[data-r16-auth] [data-r16-auth-button] { position: relative; display: flex; align-items: center; justify-content: center; box-sizing: border-box; padding: 1px 2px; border: 1px solid rgb(156,125,71); border-radius: 0; background: rgb(51,38,18); color: #ffff00; font: inherit; font-size: 10px; line-height: 12px; white-space: normal; text-shadow: none; overflow: hidden; }
[data-r16-auth] [data-r16-auth-button] > img { display: none; }
[data-r16-auth] [data-r16-auth-button]:is(:hover,:focus-visible):not(:disabled) { background: rgb(92,69,28); }
[data-r16-auth] [data-r16-auth-button]:active:not(:disabled) { background: rgb(26,20,10); }
[data-r16-auth] [data-r16-auth-button]:disabled { opacity: 1; background: rgb(33,31,26); color: rgb(122,115,94); }
[data-r16-auth] :is(button,input,a,summary):focus-visible { outline: 1px solid #ffff00; outline-offset: 2px; }
[data-r16-auth="login"] .login-feedback { box-sizing: border-box; min-width: 0; padding: 0; transform: none; font-size: 15px; color: rgb(240,224,184); border: 0; background: transparent; }
[data-r16-auth] .r16-wallet-entry { position: absolute; box-sizing: border-box; border: 1px solid rgb(171,130,61); background: rgba(31,26,18,.96); color: rgb(240,224,184); font: inherit; font-size: 14px; }
[data-r16-auth] .r16-auth-browser-settings, .language-selector.native-footer { position: absolute; box-sizing: border-box; padding: 0; border: 0; background: transparent; box-shadow: none; z-index: 25000; }
[data-r16-auth] .r16-auth-settings-trigger, .language-selector.native-footer .r16-language-toggle { width: 100%; box-sizing: border-box; padding: 5px; border: 1px solid rgb(166,128,59); border-radius: 0; background: rgb(26,20,10); color: rgb(242,232,196); font: inherit; font-size: 13px; text-align: center; cursor: pointer; }
[data-r16-auth] .r16-auth-settings-trigger { min-height: 50px; display: grid; align-content: center; gap: 3px; }
[data-r16-auth] .r16-auth-settings-trigger small { font-size: 11px; }
[data-r16-auth] .r16-auth-browser-panel, .language-selector.native-footer .r16-language-menu { position: absolute; left: 0; width: 100%; box-sizing: border-box; padding: 6px; border: 1px solid rgb(166,128,59); background: rgb(14,11,8); color: rgb(242,232,196); z-index: 25001; }
[data-r16-auth] .r16-auth-browser-panel { display: grid; gap: 10px; max-height: 630px; overflow: auto; font-size: 12px; }
[data-r16-auth] .r16-auth-browser-panel > button, [data-r16-auth] .r16-auth-browser-panel .login-utility-actions button { box-sizing: border-box; width: 100%; min-height: 29px; padding: 4px 7px; border: 1px solid rgb(166,128,59); border-radius: 0; background: rgb(26,20,10); color: rgb(242,232,196); font: inherit; }
[data-r16-auth] .r16-auth-browser-panel button:disabled { opacity: .5; cursor: default; }
[data-r16-auth] .r16-auth-browser-panel .login-web3-actions { position: static; display: grid; grid-template-columns: 1fr 1fr; }
[data-r16-auth] .r16-auth-browser-panel .login-web3-actions button { width: 100%; min-height: 29px; height: auto; }
[data-r16-auth] .r16-auth-browser-panel .login-wallet-picker { position: static; width: auto; padding: 6px; margin: 0; }
[data-r16-auth] .r16-auth-browser-panel .login-utility-actions { position: static; display: grid; grid-template-columns: 1fr 1fr; width: 100%; transform: none; }
[data-r16-auth] .r16-auth-browser-panel .login-account-panel { position: static; box-sizing: border-box; width: 100%; padding: 8px; }
[data-r16-auth] .r16-auth-browser-panel .login-runtime-stamp { position: static; width: auto; margin: 8px 0 0; padding: 0; white-space: pre-wrap; overflow-wrap: anywhere; color: #bda982; }
.language-selector.native-footer .r16-language-toggle { min-height: 29px; }
.language-selector.native-footer .r16-language-menu { display: grid; gap: 2px; padding: 4px; }
.language-selector.native-footer .language-selector-button { width: 100%; min-width: 0; min-height: 25px; padding: 4px 7px; border: 1px solid rgb(166,128,59); border-radius: 0; background: rgb(26,20,10); color: rgb(242,232,196); font: inherit; font-size: 13px; text-shadow: none; }
.language-selector.native-footer .language-selector-button:is(.active,:hover,:focus-visible) { background: rgb(71,54,20); }
[data-r16-auth="select"] .select-portrait-frame { filter: none; }
[data-r16-auth="select"] .select-character-slot-card { box-sizing: border-box; border: 1px solid rgb(166,128,56); background: rgb(14,11,8); color: #fff; }
[data-r16-auth="select"] .select-character-slot-card.selected { background: rgb(51,41,18); }
[data-r16-auth="select"] .select-character-slot-copy { text-shadow: none; }
[data-r16-auth="select"] .select-character-slot-copy .name { left: 12px; top: 6px; width: 264px; height: 20px; line-height: 20px; font-size: 14px; font-weight: 400; }
[data-r16-auth="select"] .select-character-slot-copy .job { left: 12px; top: 31px; width: 174px; height: 20px; line-height: 20px; font-size: 13px; color: #fff; }
[data-r16-auth="select"] .select-character-slot-copy .level { left: 194px; top: 31px; width: 80px; height: 20px; line-height: 20px; font-size: 13px; }
[data-r16-auth] .r16-empty-slot { position: absolute; left: 12px; top: 16px; width: 264px; height: 24px; font-size: 14px; line-height: 24px; }
[data-r16-auth] .select-last-access-label { color: #fff; }
[data-r16-auth] .r16-auth-modal { position: absolute; inset: 0; z-index: 8; }
[data-r16-auth] .select-create-panel { box-sizing: border-box; min-height: 0; margin: 0; padding: 0; border: 1px solid #d9b46e; border-radius: 0; background: rgb(9,6,5); box-shadow: none; color: #e8d9ba; }
[data-r16-auth] .select-create-title { position: absolute; margin: 0; border: 0; height: auto; font-size: 22px; line-height: 32px; font-weight: 400; color: #d9b46e; }
[data-r16-auth] .r16-create-description { position: absolute; white-space: pre-line; margin: 0; font-size: 13px; line-height: 19px; }
[data-r16-auth] .select-create-name-field { display: contents; }
[data-r16-auth] .select-create-name-field span { position: absolute; font-size: 12px; line-height: 19px; color: #d9b46e; }
[data-r16-auth] .select-create-name-field input { position: absolute; box-sizing: border-box; border: 0; border-radius: 0; box-shadow: none; padding: 0 2px; background: #000; font: inherit; font-size: 13px; }
[data-r16-auth] .select-create-panel .r16-auth-label { font-size: 12px; line-height: 18px; color: #d9b46e; }
[data-r16-auth] .select-create-class-list, [data-r16-auth] .select-create-gender-row { display: contents; }
[data-r16-auth] .select-create-class-card, [data-r16-auth] .select-create-gender-button { position: absolute; display: flex; justify-content: center; align-items: center; box-sizing: border-box; margin: 0; padding: 0 2px; border: 0; border-radius: 0; box-shadow: none; background: #342619; color: #e8d9ba; font: inherit; font-size: 12px; text-align: center; }
[data-r16-auth] :is(.select-create-class-card,.select-create-gender-button):is(.active,:hover,:focus-visible) { background: #735124; }
[data-r16-auth] .select-create-class-card span { overflow: visible; white-space: normal; }
[data-r16-auth] .select-create-actions { display: contents; }
[data-r16-auth] .select-create-actions > div { position: absolute; }
[data-r16-auth] .select-create-error { inset: auto; min-height: 0; line-height: 14px; pointer-events: none; }
[data-r16-auth][data-login-auth-current="registration"] > :is(.login-dialog,.r16-wallet-entry),
[data-r16-auth][data-login-auth-current="changePassword"] > :is(.login-dialog,.r16-wallet-entry),
[data-r16-auth]:not([data-login-auth-current="login"]) > .r16-auth-browser-settings { visibility: hidden; }
[data-r16-auth] .login-auth-backdrop { display: block; padding: 0; background: transparent; }
[data-r16-auth] .login-auth-panel { position: absolute; box-sizing: border-box; margin: 0; padding: 0; max-width: none; max-height: none; border: 1px solid rgb(166,128,56); background: rgb(9,6,5); color: #fff; box-shadow: none; text-shadow: none; overflow: visible; }
[data-r16-auth] .login-auth-panel button { text-shadow: none; }
[data-r16-auth] .login-auth-panel.safe-key { border: 0; background: transparent; }
[data-r16-auth] .login-auth-header { display: contents; }
[data-r16-auth] .login-auth-header h2 { position: absolute; margin: 0; padding: 0; font-weight: 400; text-align: center; color: #d9b46e; }
[data-r16-auth] .login-auth-panel.change-password .login-auth-header h2 { text-align: left; color: #fff; }
[data-r16-auth] .login-auth-fields, [data-r16-auth] .login-auth-field, [data-r16-auth] .login-auth-actions, [data-r16-auth] .login-safe-key, [data-r16-auth] .login-safe-key-grid, [data-r16-auth] .login-safe-key-tools { display: contents; }
[data-r16-auth] .login-auth-field span { position: absolute; font-size: 12px; line-height: 22px; text-align: right; }
[data-r16-auth] .login-auth-panel.change-password .login-auth-field span { line-height: 26px; text-align: left; }
[data-r16-auth] .login-auth-field input { position: absolute; box-sizing: border-box; min-height: 0; margin: 0; border: 1px solid #808080; border-radius: 0; background: #000; box-shadow: none; padding: 0 2px; color: #fff; font: inherit; font-size: 11px; line-height: 16px; }
[data-r16-auth] .login-auth-panel.safe-key .login-auth-field input { border: 0; color: rgb(245,240,230); font-size: 10px; line-height: 15px; }
[data-r16-auth] .login-auth-actions > :is(button,div), [data-r16-auth] .r16-safe-key-button, [data-r16-auth] .r16-safe-tool { position: absolute; box-sizing: border-box; min-height: 0; margin: 0; padding: 0; border-radius: 0; }
[data-r16-auth] .r16-auth-dialog-button { border: 1px solid rgb(156,125,71); background: rgb(51,38,18); color: #ffff00; font: inherit; font-size: 10px; line-height: 12px; text-align: center; }
[data-r16-auth] .r16-auth-dialog-button:hover:not(:disabled) { background: rgb(92,69,28); }
[data-r16-auth] .r16-auth-dialog-button:disabled { background: rgb(33,31,26); color: rgb(122,115,94); }
[data-r16-auth] .r16-auth-dialog-sprite { border: 0; background: transparent; }
[data-r16-auth] .r16-auth-dialog-sprite > img { position: absolute; inset: 0; width: 100%; height: 100%; pointer-events: none; }
[data-r16-auth] .r16-auth-dialog-sprite > :is(.hover,.pressed) { display: none; }
[data-r16-auth] .r16-auth-dialog-sprite:is(:hover,:focus-visible):not(:disabled) > .hover { display: block; }
[data-r16-auth] .r16-auth-dialog-sprite:active:not(:disabled) > .pressed { display: block; }
[data-r16-auth] .login-auth-actions > div .sprite-button { width: 100%; height: 100%; }
[data-r16-auth] .login-auth-description, [data-r16-auth] .login-auth-notice { position: absolute; box-sizing: border-box; margin: 0; padding: 0; font-size: 11px; line-height: 15px; color: #e8d9ba; white-space: pre-wrap; overflow-wrap: anywhere; }
[data-r16-auth] .login-auth-description > p { display: none; margin: 0; }
[data-r16-auth] .login-auth-panel.registration:not(:has(input:focus,button:focus)) [data-r16-registration-help="accountId"] { display: block; }
${["accountId", "password", "confirmPassword", "userName", "birthDate", "secretQuestion", "secretAnswer", "emailAddress"].map(field =>
  `[data-r16-auth] .registration:has([data-login-auth-field="${field}"]:focus) [data-r16-registration-help="${field}"] { display: block; }`).join("\n")}
[data-r16-auth] .registration:has([data-login-auth-submit]:focus) [data-r16-registration-help="submit"],
[data-r16-auth] .registration:has([data-login-auth-close]:focus) [data-r16-registration-help="cancel"] { display: block; }
[data-r16-auth] .login-auth-panel.change-password .login-auth-notice { padding: 4px; background: rgba(0,0,0,.85); color: #fff; }
[data-r16-auth] .r16-safe-panel { position: absolute; inset: 0; width: 216px; height: 278px; pointer-events: none; }
[data-r16-auth] .r16-safe-key-button { display: flex; align-items: center; justify-content: center; border: 0; color: #e8d9ba; font: inherit; font-size: 14px; background: transparent; overflow: visible; }
[data-r16-auth] .r16-safe-key-button::before { content: ""; position: absolute; left: 0; top: 0; width: 32px; height: 32px; background: url("/original-ui/Prguse/1081.png") 0 0 / 32px 32px no-repeat; }
[data-r16-auth] .r16-safe-key-button > span { position: relative; }
[data-r16-auth] .r16-safe-key-button:hover:not(:disabled)::before { background-image: url("/original-ui/Prguse/1082.png"); }
[data-r16-auth] .r16-safe-key-button:active:not(:disabled)::before { background-image: url("/original-ui/Prguse/1083.png"); }
[data-r16-auth] .r16-safe-tool { border: 0; background: #342619; color: #e8d9ba; font: inherit; font-size: 12px; }
[data-r16-auth] .r16-safe-tool:hover:not(:disabled) { background: #735124; }
[data-r16-auth] .r16-auth-sr-only { position: absolute !important; width: 1px !important; height: 1px !important; margin: -1px !important; padding: 0 !important; overflow: hidden !important; clip-path: inset(50%); white-space: nowrap; border: 0; }
`;

function crystalMainHudFreeSlots(world: HudWorldLike) {
  if (!Array.isArray(world.inventoryItems) || !Array.isArray(world.beltItems)) {
    return world.freeBagSlots;
  }

  return Math.max(
    0,
    CRYSTAL_MAIN_INVENTORY_SLOT_COUNT - world.inventoryItems.length - world.beltItems.length,
  );
}

function formatCrystalMainHudGold(gold: number) {
  const wholeGold = Math.max(0, Math.trunc(Number.isFinite(gold) ? gold : 0));
  return wholeGold.toLocaleString("en-US");
}

function crystalMainHudWeightBarFillWidth(weightRatio: number) {
  return Math.floor((CRYSTAL_MAIN_HUD_WEIGHT_BAR_WIDTH - 2) * Math.max(0, Math.min(1, weightRatio)));
}

function crystalMainHudWeightBarSprite(weightRatio: number) {
  if (weightRatio > 0.75) return ORIGINAL_UI.hud.weightBars.danger;
  if (weightRatio > 0.5) return ORIGINAL_UI.hud.weightBars.warning;
  return ORIGINAL_UI.hud.weightBars.normal;
}

export type LanguageSelectorProps = {
  language: Mir2Language;
  t: TranslateFn;
  onLanguageChange: (language: Mir2Language) => void;
  compact?: boolean;
  className?: string;
  nativeFooter?: boolean;
};

export function LanguageSelector({
  language,
  t,
  onLanguageChange,
  compact = false,
  className = "",
  nativeFooter = false,
}: LanguageSelectorProps) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLElement>(null);
  const toggleRef = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (!nativeFooter || !open) return;
    const outside = (event: PointerEvent) => { if (!rootRef.current?.contains(event.target as Node)) setOpen(false); };
    document.addEventListener("pointerdown", outside, true);
    return () => document.removeEventListener("pointerdown", outside, true);
  }, [nativeFooter, open]);
  const selectorClassName = ["language-selector", compact ? "compact" : "", nativeFooter ? "native-footer" : "", className]
    .filter(Boolean)
    .join(" ");

  return (
    <section className={selectorClassName} ref={rootRef}
      style={nativeFooter ? { ...R16_AUTH_LAYOUT.footer.language, top: "auto" } : undefined}
      onPointerDown={nativeFooter ? event => event.stopPropagation() : undefined}
      onKeyDown={event => {
        if (!nativeFooter || !open || event.nativeEvent.isComposing) return;
        if (event.key === "Escape") { event.preventDefault(); event.stopPropagation(); setOpen(false); toggleRef.current?.focus({ preventScroll: true }); }
      }}>
      {nativeFooter ? <button ref={toggleRef} type="button" className="r16-language-toggle" aria-expanded={open}
        lang={language} dir="auto" style={{ fontFamily: languageFontFamily(language) }}
        aria-label={t("ui.languageSettings", [], "Language")} onClick={() => setOpen(current => !current)}>
        {languageNativeName(language)}  ^
      </button> : null}
      {compact || nativeFooter ? null : (
        <div className="language-selector-copy">
          <strong>{t("ui.languageSettings")}</strong>
          <span>{t("ui.languageDescription")}</span>
        </div>
      )}
      {!nativeFooter || open ? <div className={nativeFooter ? "r16-language-menu" : "language-selector-buttons"}
        style={nativeFooter ? { bottom: R16_AUTH_LAYOUT.footer.languageMenuBottom } : undefined}>
        {SUPPORTED_LANGUAGES.map((option) => (
          <button
            key={option}
            type="button"
            className={`language-selector-button ${option === language ? "active" : ""}`}
            lang={option} dir="auto" style={{ fontFamily: languageFontFamily(option) }}
            aria-pressed={option === language}
            onClick={() => { onLanguageChange(option); if (nativeFooter) { setOpen(false); toggleRef.current?.focus({ preventScroll: true }); } }}
          >
            {languageNativeName(option)}
          </button>
        ))}
      </div> : null}
    </section>
  );
}

function AuthBrowserUtilities({ language, t, onOpenAccountSecurity, blocked = false, children }: {
  language: Mir2Language; t: TranslateFn; onOpenAccountSecurity?: () => void; blocked?: boolean; children?: ReactNode;
}) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLElement>(null);
  const toggleRef = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (blocked) { setOpen(false); return; }
    if (!open) return;
    const outside = (event: PointerEvent) => { if (!rootRef.current?.contains(event.target as Node)) setOpen(false); };
    document.addEventListener("pointerdown", outside, true);
    return () => document.removeEventListener("pointerdown", outside, true);
  }, [open, blocked]);
  return <section ref={rootRef} className="r16-auth-browser-settings" style={R16_AUTH_LAYOUT.footer.browser}
    onPointerDown={event => event.stopPropagation()}
    onKeyDown={event => {
      if (open && event.key === "Escape" && !event.nativeEvent.isComposing) {
        event.preventDefault(); event.stopPropagation(); setOpen(false); toggleRef.current?.focus({ preventScroll: true });
      }
    }}>
    <button ref={toggleRef} type="button" className="r16-auth-settings-trigger" aria-expanded={open && !blocked} disabled={blocked}
      onClick={() => setOpen(current => !current)}>
      <span>{r16BrowserCaption(language, "settings")}</span><small>{r16BrowserCaption(language, "settingsDetail")}</small>
    </button>
    {open && !blocked ? <div className="r16-auth-browser-panel" style={{ bottom: R16_AUTH_LAYOUT.footer.browserMenuBottom }} role="dialog" aria-label={r16BrowserCaption(language, "settings")}>
      <OriginalAudioSettingsControls t={t} language={language} stacked />
      <button type="button" disabled={!onOpenAccountSecurity} onClick={() => { setOpen(false); onOpenAccountSecurity?.(); }}>
        {r16BrowserCaption(language, "accountSecurity")}
      </button>
      {children}
    </div> : null}
  </section>;
}

export type LoginOverlayProps = {
  language: Mir2Language;
  t: TranslateFn;
  runtimePhase: string;
  runtimeMessage: string;
  wsState: string;
  accountId: string;
  password: string;
  loginBusy: boolean;
  loginError: string | null;
  loginAuth: LoginAuthControls;
  suiWallets: SuiWalletSummary[];
  walletPickerOpen: boolean;
  dubheWalletUrl: string;
  dubheQr: DubheQrLoginState | null;
  onLanguageChange: (language: Mir2Language) => void;
  onAccountIdChange: (value: string) => void;
  onPasswordChange: (value: string) => void;
  onCreateAccount: () => void;
  onSubmitLogin: () => void;
  onPasskeyLogin: () => void;
  onWalletPickerToggle: () => void;
  onWalletLogin: (walletId: string) => void;
  onDubheQrLogin: () => void;
  onDubheQrCancel: () => void;
  onQuickEnter: () => void;
  onResetClient: () => void;
  onOpenAccountSecurity?: () => void;
};

export function LoginOverlay({
  language,
  t,
  runtimePhase,
  runtimeMessage,
  wsState,
  accountId,
  password,
  loginBusy,
  loginError,
  loginAuth,
  suiWallets,
  walletPickerOpen,
  dubheWalletUrl,
  dubheQr,
  onLanguageChange,
  onAccountIdChange,
  onPasswordChange,
  onSubmitLogin,
  onPasskeyLogin,
  onWalletPickerToggle,
  onWalletLogin,
  onDubheQrLogin,
  onDubheQrCancel,
  onQuickEnter,
  onResetClient,
  onOpenAccountSecurity,
}: LoginOverlayProps) {
  const loginNotice = loginError ?? (loginBusy ? t("ui.loggingIn") : loginAuth.state.surface === "login" ? loginAuth.state.notice : null);
  const authBlocked = !loginAuth.ready || loginAuth.pending || loginBusy;
  const loginBlocked = loginAuth.pending || loginBusy || loginAuth.state.surface !== "login";
  const overlayRef = useRef<HTMLElement>(null);
  const authSafeFocusRef = useRef(loginAuth.state.safeFocus);
  authSafeFocusRef.current = loginAuth.state.safeFocus;
  const { surface: authSurface, epoch: authEpoch, focusField: authFocusField } = loginAuth.state;
  useEffect(() => {
    const root = overlayRef.current?.querySelector<HTMLElement>(`[data-login-auth-surface="${authSurface}"]`);
    if (!root || !authFocusField && root.contains(document.activeElement)) return;
    const requested = authFocusField?.split(".").at(-1);
    const field = requested === "account" ? "accountId" : requested
      ?? (authSurface === "safeKey" && authSafeFocusRef.current === "password" ? "password" : "accountId");
    const target = Array.from(root.querySelectorAll<HTMLInputElement>("[data-login-auth-field]")).find(input => input.dataset.loginAuthField === field && !input.disabled)
      ?? root.querySelector<HTMLButtonElement>("[data-login-auth-close]");
    if (target && document.activeElement !== target) target.focus({ preventScroll: true });
  }, [authSurface, authEpoch, authFocusField, loginAuth.ready, loginAuth.pending]);
  const [showAccountPanel, setShowAccountPanel] = useState(false);
  const dubheWalletDetected = suiWallets.some((wallet) => wallet.isDubhe);

  return (
    <section className="login-overlay" ref={overlayRef} data-r16-auth="login" data-login-auth-current={authSurface}
      onKeyDown={(event) => {
        if (event.defaultPrevented || event.nativeEvent.isComposing || event.repeat || dubheQr
          || loginAuth.state.surface !== "login") return;
        const root = overlayRef.current;
        if (event.key === "F8") {
          event.preventDefault(); event.stopPropagation();
          if (!loginBlocked) onDubheQrLogin();
        } else if (event.key === "Escape") {
          event.preventDefault(); event.stopPropagation();
          if (walletPickerOpen) onWalletPickerToggle();
          setShowAccountPanel(false);
          root?.querySelector<HTMLInputElement>('[data-r16-login-focus="accountId"]')?.focus({ preventScroll: true });
        } else if (event.key === "Tab") {
          const focus = (event.target as HTMLElement).closest<HTMLElement>('[data-r16-login-focus]')?.dataset.r16LoginFocus;
          if (!focus) return;
          const next = root?.querySelector<HTMLElement>(`[data-r16-login-focus="${r16NextLoginFocus(focus as R16LoginFocus, event.shiftKey)}"]`);
          if (!next || (next as HTMLButtonElement).disabled) return;
          event.preventDefault(); event.stopPropagation(); next.focus({ preventScroll: true });
        }
      }}>
      <style>{R16_AUTH_STYLES}</style>
      <LanguageSelector
        language={language}
        t={t}
        compact
        nativeFooter
        className="login-language-selector"
        onLanguageChange={onLanguageChange}
      />
      <form
        className="login-dialog"
        style={{ ...R16_AUTH_LAYOUT.login.panel, transform: "none" }}
        data-login-auth-surface="login"
        noValidate
        onSubmit={(event) => {
          event.preventDefault();
          if (!loginBlocked) onSubmitLogin();
        }}
      >
        <img className="login-panel" src={ORIGINAL_UI.login.dialog} alt="" draggable={false} />
        <h1 className="r16-auth-label" style={{ ...r16AuthRelativeRect(R16_AUTH_LAYOUT.login.title, R16_AUTH_LAYOUT.login.panel), fontSize: 20, lineHeight: "28px", textAlign: "center" }}>
          {r16AuthCaption(language, "login", t("ui.login"))}
        </h1>
        <label className="r16-auth-label" htmlFor="r16-login-account" style={{ ...r16AuthRelativeRect(R16_AUTH_LAYOUT.login.accountLabel, R16_AUTH_LAYOUT.login.panel), fontSize: 12, lineHeight: "21px", textAlign: "right" }}>
          {r16AuthCaption(language, "account", t("ui.accountId"))}
        </label>
        <label className="r16-auth-label" htmlFor="r16-login-password" style={{ ...r16AuthRelativeRect(R16_AUTH_LAYOUT.login.passwordLabel, R16_AUTH_LAYOUT.login.panel), fontSize: 12, lineHeight: "21px", textAlign: "right" }}>
          {r16AuthCaption(language, "password", t("ui.password"))}
        </label>
        <input
          className="login-input account"
          id="r16-login-account"
          style={r16AuthRelativeRect(R16_AUTH_LAYOUT.login.account, R16_AUTH_LAYOUT.login.panel)}
          data-gamepad-initial="true"
          data-login-auth-field="accountId"
          data-r16-login-focus="accountId"
          aria-label={r16AuthCaption(language, "account", t("ui.accountId", [], "Account ID"))}
          maxLength={24}
          disabled={loginBlocked}
          onFocus={() => { if (loginAuth.state.safeFocus !== "account") loginAuth.safeFocus("account"); }}
          value={accountId}
          onChange={(event) => onAccountIdChange(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter" && !event.nativeEvent?.isComposing) {
              event.preventDefault();
              event.stopPropagation?.();
              if (!loginBlocked) event.currentTarget.form?.querySelector<HTMLInputElement>('[data-r16-login-focus="password"]')?.focus({ preventScroll: true });
            }
          }}
          autoComplete="off"
        />
        <input
          className="login-input password"
          id="r16-login-password"
          style={r16AuthRelativeRect(R16_AUTH_LAYOUT.login.password, R16_AUTH_LAYOUT.login.panel)}
          type="password"
          data-login-auth-field="password"
          data-r16-login-focus="password"
          aria-label={r16AuthCaption(language, "password", t("ui.password", [], "Password"))}
          maxLength={32}
          disabled={loginBlocked}
          onFocus={() => { if (loginAuth.state.safeFocus !== "password") loginAuth.safeFocus("password"); }}
          value={password}
          onChange={(event) => onPasswordChange(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter" && !event.nativeEvent?.isComposing) {
              event.preventDefault();
              event.stopPropagation?.();
              if (!loginBlocked) event.currentTarget.form?.querySelector<HTMLButtonElement>('[data-r16-login-focus="login"]')?.focus({ preventScroll: true });
            }
          }}
          autoComplete="off"
        />
        <div className="login-button ok" style={r16AuthRelativeRect(R16_AUTH_LAYOUT.login.ok, R16_AUTH_LAYOUT.login.panel)}>
          <SpriteButton sprite={ORIGINAL_UI.login.buttons.ok} label={r16AuthCaption(language, "ok", t("ui.confirm", [], "OK"))} nativeCaption loginFocus="login" disabled={loginBlocked} onClick={onSubmitLogin} />
        </div>
        <div className="login-button account" style={r16AuthRelativeRect(R16_AUTH_LAYOUT.login.newAccount, R16_AUTH_LAYOUT.login.panel)}>
          <SpriteButton
            sprite={ORIGINAL_UI.login.buttons.newAccount}
            label={r16AuthCaption(language, "new_account", t("client.NewAccount", [], "New Account"))}
            nativeCaption
            loginFocus="newAccount"
            disabled={loginBlocked}
            onClick={() => loginAuth.open("registration")}
          />
        </div>
        <div className="login-button password" style={r16AuthRelativeRect(R16_AUTH_LAYOUT.login.changePassword, R16_AUTH_LAYOUT.login.panel)}>
          <SpriteButton sprite={ORIGINAL_UI.login.buttons.changePassword} label={r16AuthCaption(language, "change_password", t("ui.changePassword", [], "Change Password"))}
            nativeCaption disabled={loginBlocked} onClick={() => loginAuth.open("changePassword")} />
        </div>
        <div className="login-button view" style={r16AuthRelativeRect(R16_AUTH_LAYOUT.login.safeKey, R16_AUTH_LAYOUT.login.panel)}>
          <SpriteButton
            sprite={ORIGINAL_UI.login.buttons.viewKey}
            label={r16AuthCaption(language, "safe_key", t("ui.viewKey"))}
            nativeCaption
            disabled={loginBlocked}
            onClick={() => loginAuth.open("safeKey")}
          />
        </div>
        <div className="login-button close" style={r16AuthRelativeRect(R16_AUTH_LAYOUT.login.cancel, R16_AUTH_LAYOUT.login.panel)}>
          <SpriteButton sprite={ORIGINAL_UI.login.buttons.close} label={r16AuthCaption(language, "cancel", t("ui.close"))} nativeCaption onClick={onResetClient} />
        </div>
      </form>
      <button type="button" className="r16-wallet-entry" style={R16_AUTH_LAYOUT.login.wallet}
        disabled={loginBlocked} data-dubhe-qr-return="true" onClick={onDubheQrLogin}>
        {t("wallet.login.title", [], "Dubhe Wallet QR login")}  [F8]
      </button>
      {dubheQr ? <DubheWalletQrLogin language={language} state={dubheQr} onRetry={onDubheQrLogin} onCancel={onDubheQrCancel} /> : null}
      <AuthBrowserUtilities language={language} t={t} onOpenAccountSecurity={onOpenAccountSecurity}
        blocked={loginBusy || loginAuth.pending || loginAuth.state.surface !== "login" || dubheQr !== null}>
        <strong>{r16BrowserCaption(language, "signIn")}</strong>
        <div className="login-web3-actions" aria-label={t("ui.web3Login", [], "Web3 login")}>
          <button type="button" onClick={onPasskeyLogin} disabled={loginBusy || loginAuth.pending || loginAuth.state.surface !== "login"}>
            {t("ui.passkeyLogin", [], "Passkey")}
          </button>
          <button
            type="button"
            onClick={onWalletPickerToggle}
            disabled={loginBusy || loginAuth.pending || loginAuth.state.surface !== "login"}
            aria-expanded={walletPickerOpen}
            aria-controls="login-wallet-picker"
            data-dubhe-qr-return="true"
          >
            {t("ui.walletLogin", [], "Wallet")}
          </button>
        </div>
        {walletPickerOpen ? (
          <div id="login-wallet-picker" className="login-wallet-picker" role="dialog" aria-label="Sui wallet">
            <div className="login-wallet-picker-title">Sui Wallet</div>
            <button
              type="button"
              className="login-wallet-option dubhe login-wallet-qr-option"
              onClick={onDubheQrLogin}
              disabled={loginBusy || loginAuth.pending || loginAuth.state.surface !== "login"}
            >
              <span className="login-wallet-fallback-icon" aria-hidden="true">D</span>
              <span>{t("wallet.login.title", [], "Dubhe Wallet QR login")}</span>
            </button>
            {suiWallets.length > 0 ? (
              <div className="login-wallet-list">
                {suiWallets.map((wallet) => (
                  <button
                    key={wallet.id}
                    type="button"
                    className={`login-wallet-option ${wallet.isDubhe ? "dubhe" : ""}`}
                    onClick={() => onWalletLogin(wallet.id)}
                    disabled={loginBusy || loginAuth.pending || loginAuth.state.surface !== "login"}
                  >
                    {wallet.icon ? (
                      <img src={wallet.icon} alt="" aria-hidden="true" />
                    ) : (
                      <span className="login-wallet-fallback-icon" aria-hidden="true">
                        {wallet.name.slice(0, 1)}
                      </span>
                    )}
                    <span>{wallet.name}</span>
                  </button>
                ))}
              </div>
            ) : (
              <div className="login-wallet-empty">{t("ui.walletMissing", [], "No Sui wallet found")}</div>
            )}
            {!dubheWalletDetected ? (
              <a className="login-wallet-install" href={dubheWalletUrl} target="_blank" rel="noreferrer">
                <span className="login-wallet-fallback-icon" aria-hidden="true">
                  D
                </span>
                <span>Dubhe Wallet</span>
              </a>
            ) : null}
          </div>
        ) : null}
      <div className="login-utility-actions">
        <button type="button" disabled={loginBusy || loginAuth.pending || loginAuth.state.surface !== "login"}
          onClick={() => setShowAccountPanel(current => !current)} aria-expanded={showAccountPanel}>
          {r16BrowserCaption(language, "accountInfo")}
        </button>
        <button type="button" disabled={loginBusy || loginAuth.pending || loginAuth.state.surface !== "login"} onClick={onQuickEnter}>
          {t("ui.quickEnter", [], "Quick Enter")}
        </button>
      </div>
      {showAccountPanel ? (
        <div className="login-account-panel">
          <strong>{r16BrowserCaption(language, "accountInfo")}</strong>
          <span>{accountId || "-"}</span>
          <button type="button" onClick={() => setShowAccountPanel(false)}>
            {t("ui.close")}
          </button>
        </div>
      ) : null}
      {runtimePhase === "boot-error" || wsState === "closed" ? (
        <details className="r16-auth-diagnostics"><summary>{r16BrowserCaption(language, "diagnostics")}</summary>
          <div className="login-runtime-stamp">{`${runtimePhase} / ${wsState} / ${runtimeMessage}`}</div>
        </details>
      ) : null}
      </AuthBrowserUtilities>
      {loginAuth.state.surface !== "login" ? (
        <LoginAuthDialog language={language} t={t} loginAuth={loginAuth} blocked={authBlocked} accountId={accountId} password={password}
          onAccountIdChange={onAccountIdChange} onPasswordChange={onPasswordChange} />
      ) : null}
      {loginNotice ? <div className="login-feedback" role="status" aria-live="polite" style={R16_AUTH_LAYOUT.login.notice}>{loginNotice}</div> : null}
    </section>
  );
}

const LOGIN_REGISTRATION_FIELDS: ReadonlyArray<{
  field: keyof RegistrationDraft; label: string; labelKey: string; caption: R16AuthCaption; hint: R16AuthCaption; type?: "text" | "password"; maxLength?: number;
}> = [
  { field: "accountId", label: "Account ID", labelKey: "ui.accountId", caption: "account", hint: "registration.account_hint", maxLength: 15 },
  { field: "password", label: "Password", labelKey: "ui.password", caption: "password", hint: "registration.password_hint", type: "password", maxLength: 15 },
  { field: "confirmPassword", label: "Confirm Password", labelKey: "ui.confirmPassword", caption: "confirm_password", hint: "registration.password_hint", type: "password", maxLength: 15 },
  { field: "userName", label: "Name", labelKey: "ui.userName", caption: "user_name", hint: "registration.name_hint", maxLength: 20 },
  { field: "birthDate", label: "Date of Birth", labelKey: "ui.birthDate", caption: "birth_date", hint: "registration.birth_hint", maxLength: 10 },
  { field: "secretQuestion", label: "Secret Question", labelKey: "ui.secretQuestion", caption: "secret_question", hint: "registration.question_hint", maxLength: 30 },
  { field: "secretAnswer", label: "Secret Answer", labelKey: "ui.secretAnswer", caption: "secret_answer", hint: "registration.answer_hint", maxLength: 30 },
  { field: "emailAddress", label: "Email", labelKey: "ui.emailAddress", caption: "email", hint: "registration.email_hint", maxLength: 50 },
];
const LOGIN_PASSWORD_FIELDS: ReadonlyArray<{
  field: keyof ChangePasswordDraft; label: string; labelKey: string; caption: R16AuthCaption; type?: "text" | "password"; maxLength: number;
}> = [
  { field: "accountId", label: "Account ID", labelKey: "ui.accountId", caption: "account", maxLength: 15 },
  { field: "oldPassword", label: "Current Password", labelKey: "ui.oldPassword", caption: "current_password", type: "password", maxLength: 15 },
  { field: "newPassword", label: "New Password", labelKey: "ui.newPassword", caption: "new_password", type: "password", maxLength: 15 },
  { field: "confirmPassword", label: "Confirm Password", labelKey: "ui.confirmPassword", caption: "confirm_password", type: "password", maxLength: 15 },
];

function LoginAuthDialog({ language = "en", t, loginAuth, blocked, accountId, password, onAccountIdChange, onPasswordChange }: {
  language?: Mir2Language; t: TranslateFn; loginAuth: LoginAuthControls; blocked: boolean; accountId: string; password: string;
  onAccountIdChange: (value: string) => void; onPasswordChange: (value: string) => void;
}) {
  const { state } = loginAuth;
  const safe = state.surface === "safeKey";
  const layout = state.surface === "registration" ? R16_AUTH_LAYOUT.registration
    : state.surface === "changePassword" ? R16_AUTH_LAYOUT.changePassword : R16_AUTH_LAYOUT.safeKey;
  const title = state.surface === "registration" ? r16AuthCaption(language, "new_account", t("client.NewAccount", [], "New Account"))
    : state.surface === "changePassword" ? r16AuthCaption(language, "change_password", t("ui.changePassword", [], "Change Password"))
    : r16AuthCaption(language, "safe_key", t("ui.viewKey", [], "Safe Key"));
  return (
    <div className="login-auth-backdrop">
      <form
        className={`login-auth-panel ${safe ? "safe-key" : state.surface === "registration" ? "registration" : "change-password"}`}
        style={layout.panel}
        data-login-auth-surface={state.surface}
        data-login-auth-epoch={state.epoch}
        role="dialog"
        aria-modal="true"
        aria-labelledby="login-auth-title"
        aria-busy={loginAuth.pending}
        noValidate
        autoComplete="off"
        onSubmit={event => {
          event.preventDefault();
          if (blocked) return;
          if (state.surface === "registration") loginAuth.submitRegistration();
          else if (state.surface === "changePassword") loginAuth.submitChangePassword();
          else if (state.surface === "safeKey") loginAuth.safeEnter();
        }}
        onKeyDown={event => {
          if (event.nativeEvent?.isComposing || event.keyCode === 229) return;
          if (event.key === "Escape") {
            event.preventDefault(); event.stopPropagation(); loginAuth.close();
          } else if (safe && (event.key === "Backspace" || event.key === "Delete")) {
            event.preventDefault(); event.stopPropagation(); if (!blocked) loginAuth.safeDelete();
          } else if (safe && event.key === "Enter") {
            event.preventDefault(); event.stopPropagation(); if (!blocked) loginAuth.safeEnter();
          } else if (event.key === "Enter" && (state.surface === "registration" || state.surface === "changePassword")) {
            const fields = Array.from(event.currentTarget.querySelectorAll<HTMLInputElement>("[data-login-auth-field]:not(:disabled)"));
            const index = fields.indexOf(event.target as HTMLInputElement);
            if (index >= 0) {
              event.preventDefault(); event.stopPropagation();
              if (state.surface === "registration") {
                const next = fields[index + 1] ?? event.currentTarget.querySelector<HTMLButtonElement>("[data-login-auth-submit]:not(:disabled)");
                next?.focus();
              }
            }
          } else if (event.key === "Tab") {
            if (safe) {
              const next = state.safeFocus === "account" ? "password" : "account";
              const field = event.currentTarget.querySelector<HTMLInputElement>(`[data-login-auth-field="${next === "account" ? "accountId" : "password"}"]:not(:disabled)`);
              if (field) { event.preventDefault(); event.stopPropagation(); field.focus({ preventScroll: true }); }
              return;
            }
            const fields = Array.from(event.currentTarget.querySelectorAll<HTMLElement>("input:not(:disabled), button:not(:disabled)"));
            const first = fields[0], last = fields.at(-1);
            if (event.shiftKey && document.activeElement === first && last) { event.preventDefault(); last.focus(); }
            else if (!event.shiftKey && document.activeElement === last && first) { event.preventDefault(); first.focus(); }
          }
        }}
      >
        {safe ? <img className="r16-safe-panel" src={R16_AUTH_ASSETS.safePanel} alt="" draggable={false} /> : null}
        <header className="login-auth-header">
          <h2 id="login-auth-title" className={safe ? "r16-auth-sr-only" : undefined}
            style={safe ? undefined : { ...r16AuthRelativeRect(state.surface === "registration" ? R16_AUTH_LAYOUT.registration.title : R16_AUTH_LAYOUT.changePassword.title, layout.panel),
              fontSize: state.surface === "registration" ? 22 : 19, lineHeight: state.surface === "registration" ? "30px" : "26px" }}>{title}</h2>
        </header>
        {state.surface === "registration" ? (
          <div className="login-auth-fields">
            {LOGIN_REGISTRATION_FIELDS.map(field => (
              <label className="login-auth-field" key={field.field}>
                <span style={{ ...r16AuthRelativeRect({ left: 239, top: R16_AUTH_LAYOUT.registration.fields[field.field].top, width: 198, height: 22 }, layout.panel) }}>
                  {r16AuthCaption(language, field.caption, t(field.labelKey, [], field.label))}
                </span>
                <input data-login-auth-field={field.field} name={field.field} aria-describedby={`login-registration-help-${field.field}`}
                  style={r16AuthRelativeRect(R16_AUTH_LAYOUT.registration.fields[field.field], layout.panel)}
                  type={field.type ?? "text"} inputMode={field.field === "emailAddress" ? "email" : undefined}
                  placeholder={field.field === "birthDate" ? "YYYY-MM-DD" : undefined}
                  maxLength={field.maxLength} autoComplete="off" spellCheck={false}
                  disabled={loginAuth.pending} value={state.registration[field.field]}
                  onChange={event => loginAuth.registrationChange(field.field, event.target.value)} />
              </label>
            ))}
          </div>
        ) : state.surface === "changePassword" ? (
          <div className="login-auth-fields">
            {LOGIN_PASSWORD_FIELDS.map(field => (
              <label className="login-auth-field" key={field.field}>
                <span style={r16AuthRelativeRect({ left: 352, top: R16_AUTH_LAYOUT.changePassword.fields[field.field].top, width: 158, height: 26 }, layout.panel)}>
                  {r16AuthCaption(language, field.caption, t(field.labelKey, [], field.label))}
                </span>
                <input data-login-auth-field={field.field} name={field.field} type={field.type ?? "text"}
                  style={r16AuthRelativeRect(R16_AUTH_LAYOUT.changePassword.fields[field.field], layout.panel)}
                  maxLength={field.maxLength} autoComplete="off" spellCheck={false}
                  disabled={loginAuth.pending} value={state.changePassword[field.field]}
                  onChange={event => loginAuth.passwordChange(field.field, event.target.value)} />
              </label>
            ))}
          </div>
        ) : (
          <div className="login-safe-key">
            <div className="login-auth-fields">
              <label className={state.safeFocus === "account" ? "login-auth-field active" : "login-auth-field"}>
                <span className="r16-auth-sr-only">{r16AuthCaption(language, "account", t("ui.accountId", [], "Account ID"))}</span>
                <input data-login-auth-field="accountId" name="accountId" aria-label={t("ui.accountId", [], "Account ID")}
                  style={r16AuthRelativeRect(R16_AUTH_LAYOUT.login.account, layout.panel)} readOnly
                  maxLength={24} autoComplete="off" spellCheck={false} disabled={loginAuth.pending} value={accountId}
                  onFocus={() => { if (state.safeFocus !== "account") loginAuth.safeFocus("account"); }}
                  onChange={event => onAccountIdChange(event.target.value)} />
              </label>
              <label className={state.safeFocus === "password" ? "login-auth-field active" : "login-auth-field"}>
                <span className="r16-auth-sr-only">{r16AuthCaption(language, "password", t("ui.password", [], "Password"))}</span>
                <input data-login-auth-field="password" name="password" aria-label={t("ui.password", [], "Password")}
                  style={r16AuthRelativeRect(R16_AUTH_LAYOUT.login.password, layout.panel)} readOnly
                  type="password" maxLength={32} autoComplete="off" disabled={loginAuth.pending} value={password}
                  onFocus={() => { if (state.safeFocus !== "password") loginAuth.safeFocus("password"); }}
                  onChange={event => onPasswordChange(event.target.value)} />
              </label>
            </div>
            <div className="login-safe-key-grid" role="group" aria-label={r16AuthCaption(language, "safe_key", t("ui.viewKey", [], "Safe Key"))}>
              {Array.from(state.safeKeys).map((key, index) => (
                <button type="button" key={index + ":" + key} data-login-safe-key={key} className="r16-safe-key-button"
                  style={r16AuthRelativeRect(r16AuthSafeKeyRect(index)!, layout.panel)} onMouseDown={event => event.preventDefault()}
                  disabled={blocked} onClick={() => loginAuth.safePress(key)}><span>{key}</span></button>
              ))}
            </div>
            <div className="login-safe-key-tools">
              <button type="button" className="r16-safe-tool" style={r16AuthRelativeRect(R16_AUTH_LAYOUT.safeKey.delete, layout.panel)} disabled={blocked}
                onMouseDown={event => event.preventDefault()} onClick={loginAuth.safeDelete}>{r16AuthCaption(language, "delete", t("ui.delete", [], "Delete"))}</button>
              <button type="button" className="r16-safe-tool" style={r16AuthRelativeRect(R16_AUTH_LAYOUT.safeKey.random, layout.panel)} disabled={blocked}
                onMouseDown={event => event.preventDefault()} onClick={loginAuth.safeRandom}>{r16AuthCaption(language, "random", t("ui.random", [], "Random"))}</button>
            </div>
          </div>
        )}
        {state.surface === "registration" ? (
          <div className="login-auth-description" style={r16AuthRelativeRect(R16_AUTH_LAYOUT.registration.description, layout.panel)}>
            {LOGIN_REGISTRATION_FIELDS.map(field => <p key={field.field} id={`login-registration-help-${field.field}`} data-r16-registration-help={field.field}>
              {r16AuthCaption(language, field.hint)}
            </p>)}
            <p data-r16-registration-help="submit">{r16AuthCaption(language, "registration.submit_hint")}</p>
            <p data-r16-registration-help="cancel">{r16AuthCaption(language, "registration.cancel_hint")}</p>
          </div>
        ) : null}
        {state.notice ? <div className="login-auth-notice" role="status" aria-live="polite"
          style={r16AuthRelativeRect(state.surface === "registration" ? R16_AUTH_LAYOUT.registration.notice
            : state.surface === "changePassword" ? R16_AUTH_LAYOUT.changePassword.notice : R16_AUTH_LAYOUT.login.notice, layout.panel)}>{state.notice}</div> : null}
        <div className="login-auth-actions">
          <button type="submit" className={safe ? "r16-safe-tool" : state.surface === "changePassword" ? "r16-auth-dialog-sprite" : "r16-auth-dialog-button"}
            aria-label={r16AuthCaption(language, safe ? "enter" : "ok")} disabled={blocked} data-login-auth-submit={state.surface}
            style={r16AuthRelativeRect(safe ? R16_AUTH_LAYOUT.safeKey.enter : state.surface === "registration" ? R16_AUTH_LAYOUT.registration.submit : R16_AUTH_LAYOUT.changePassword.submit, layout.panel)}>
            {state.surface === "changePassword" ? <>
              <img className="base" src={R16_AUTH_ASSETS.passwordSubmit.base} alt="" draggable={false} />
              <img className="hover" src={R16_AUTH_ASSETS.passwordSubmit.hover} alt="" draggable={false} />
              <img className="pressed" src={R16_AUTH_ASSETS.passwordSubmit.pressed} alt="" draggable={false} />
            </> : r16AuthCaption(language, safe ? "enter" : "ok", t("ui.enter", [], "Enter"))}
          </button>
          <button type="button" className={safe ? "r16-safe-tool" : state.surface === "changePassword" ? "r16-auth-dialog-sprite" : "r16-auth-dialog-button"}
            aria-label={safe ? "Esc" : r16AuthCaption(language, "cancel")} disabled={!safe && loginAuth.pending} data-login-auth-close="true" onClick={loginAuth.close}
            style={r16AuthRelativeRect(safe ? R16_AUTH_LAYOUT.safeKey.esc : state.surface === "registration" ? R16_AUTH_LAYOUT.registration.cancel : R16_AUTH_LAYOUT.changePassword.cancel, layout.panel)}>
            {state.surface === "changePassword" ? <>
              <img className="base" src={R16_AUTH_ASSETS.passwordCancel.base} alt="" draggable={false} />
              <img className="hover" src={R16_AUTH_ASSETS.passwordCancel.hover} alt="" draggable={false} />
              <img className="pressed" src={R16_AUTH_ASSETS.passwordCancel.pressed} alt="" draggable={false} />
            </> : safe ? "Esc" : r16AuthCaption(language, "cancel", t("ui.close", [], "Close"))}
          </button>
        </div>
      </form>
    </div>
  );
}

export type SelectOverlayProps = {
  language: Mir2Language;
  t: TranslateFn;
  characters: SelectCharacterEntryLike[];
  selectedCharacterIndex: number;
  accountId: string;
  identityProvider: string | null;
  identityLinkBusy: boolean;
  identityLinkStatus: string | null;
  suiWallets: SuiWalletSummary[];
  selectedPortraitFrames: SelectPortraitFrame[];
  onLanguageChange: (language: Mir2Language) => void;
  onSelectCharacter: (index: number) => void;
  onEnterWorld: () => void;
  characterCreate: CharacterCreateControls;
  onDeleteCharacter: () => void;
  onIdentityLinkPasskey: () => void;
  onIdentityLinkWallet: (walletId?: string) => void;
  onExit: () => void;
  onOpenAccountSecurity?: () => void;
};

export function SelectOverlay({
  language,
  t,
  characters,
  selectedCharacterIndex,
  accountId,
  identityProvider,
  identityLinkBusy,
  identityLinkStatus,
  suiWallets,
  selectedPortraitFrames,
  onLanguageChange,
  onSelectCharacter,
  onEnterWorld,
  characterCreate,
  onDeleteCharacter,
  onIdentityLinkPasskey,
  onIdentityLinkWallet,
  onExit,
  onOpenAccountSecurity,
}: SelectOverlayProps) {
  const selectedRaw = characters[selectedCharacterIndex] ?? null;
  // A synthesized placeholder is not a playable character.
  const selected = selectedRaw && !selectedRaw.synthetic ? selectedRaw : null;
  const availability = r16SelectAvailability(characters, selectedCharacterIndex);
  const createPending = characterCreate.status === "pending";
  const canCreate = availability.canCreate && characterCreate.classes.length > 0 && !createPending;
  let createNotice: string | null = null;
  switch (characterCreate.status) {
    case "pending":
      createNotice = t("shell.notice.create_pending", [], "Character creation is pending.");
      break;
    case "slotsFull":
      createNotice = t("client.YouCannotMakeMoreCharacters", [4], createPanelText(language, "slotsFull"));
      break;
    case "nameTaken":
      createNotice = t("client.CharacterNameExists", [], createPanelText(language, "nameExists"));
      break;
    case "failed":
      createNotice = t("shell.notice.no_reason", [], "Character creation failed.");
      break;
  }
  const [showCreditsPanel, setShowCreditsPanel] = useState(false);
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);
  const [showCreatePanel, setShowCreatePanel] = useState(false);
  const [showIdentityPanel, setShowIdentityPanel] = useState(false);
  const [createName, setCreateName] = useState("");
  const [createClass, setCreateClassKey] = useState<EntityClassKey>("warrior");
  const createClassKey = characterCreate.classes.includes(createClass) ? createClass : characterCreate.classes[0] ?? "warrior";
  const [createGender, setCreateGender] = useState<EntityGenderKey>("male");
  const [createError, setCreateError] = useState<string | null>(null);
  const selectRootRef = useRef<HTMLElement>(null);
  const createNameRef = useRef<HTMLInputElement>(null);
  const modalOpen = showCreditsPanel || showDeleteConfirm || showCreatePanel || showIdentityPanel;
  // The selected-character portrait idle animation lives HERE (not in the shell)
  // so its R16 250ms tick re-renders only this overlay, not the game shell.
  const [portraitFrameIndex, setPortraitFrameIndex] = useState(0);
  useEffect(() => {
    setPortraitFrameIndex(0);
  }, [selectedCharacterIndex, selectedPortraitFrames.length, showCreatePanel, createClassKey, createGender]);
  useEffect(() => {
    if (!showCreatePanel && selectedPortraitFrames.length <= 1) {
      return;
    }
    const timer = window.setInterval(() => {
      setPortraitFrameIndex((current) => (current + 1) % (showCreatePanel ? 16 : selectedPortraitFrames.length));
    }, R16_AUTH_LAYOUT.select.previewDelayMs);
    return () => window.clearInterval(timer);
  }, [showCreatePanel, selectedPortraitFrames.length]);
  const animatedSelectedFrame =
    selectedPortraitFrames[portraitFrameIndex % Math.max(selectedPortraitFrames.length, 1)] ?? null;
  const createPortraitKey = `${createClassKey}${createGender === "female" ? "Female" : "Male"}` as SelectPortraitKey;
  const createPortraitFrame = SELECT_PORTRAIT_ANIMATIONS[createPortraitKey][portraitFrameIndex % 16] ?? null;
  const activePortraitFrame = showCreatePanel ? createPortraitFrame : animatedSelectedFrame;
  const selectedPortraitOverlay = r16WizardPreviewOverlay(selected?.classKey, selected?.gender, portraitFrameIndex);
  const createPortraitOverlay = r16WizardPreviewOverlay(createClassKey, createGender, portraitFrameIndex);

  useEffect(() => {
    if (showCreatePanel) {
      setShowCreditsPanel(false);
      setShowDeleteConfirm(false);
    }
  }, [showCreatePanel]);

  useEffect(() => {
    if (characterCreate.status === "created" || characterCreate.status === "slotsFull") {
      setShowCreatePanel(false);
      setCreateName("");
      setCreateError(null);
    } else if ((characterCreate.status === "failed" || characterCreate.status === "nameTaken") && showCreatePanel) {
      createNameRef.current?.focus({ preventScroll: true });
    }
  }, [characterCreate.status, showCreatePanel]);

  function openCreatePanel() {
    if (!canCreate) return;
    characterCreate.clearNotice();
    setCreateError(null);
    setCreateName("");
    setCreateClassKey(characterCreate.classes[0]);
    setCreateGender("male");
    setShowCreatePanel(true);
  }

  function closeCreatePanel() {
    setShowCreatePanel(false);
    setCreateError(null);
  }

  function submitCreateCharacter() {
    if (createPending) return;
    const trimmedName = createName.trim();
    if (!trimmedName) {
      setCreateError(t("client.PleaseEnterCharacterName", [], createPanelText(language, "nameRequired")));
      return;
    }
    if (!canCreate) {
      setCreateError(availability.canCreate
        ? t("shell.notice.no_reason", [], "Character creation is unavailable.")
        : t("client.YouCannotMakeMoreCharacters", [4], createPanelText(language, "slotsFull")));
      return;
    }
    if (characters.some((character) => character.name.toLowerCase() === trimmedName.toLowerCase())) {
      setCreateError(t("client.CharacterNameExists", [], createPanelText(language, "nameExists")));
      return;
    }
    setCreateError(null);
    characterCreate.submit({ name: trimmedName, classKey: createClassKey, gender: createGender });
  }

  return (
    <section className="select-overlay" ref={selectRootRef} data-r16-auth="select"
      onKeyDown={(event) => {
        if (event.defaultPrevented || event.nativeEvent.isComposing || event.repeat) return;
        if (event.key === "Escape") {
          event.preventDefault(); event.stopPropagation();
          if (showCreatePanel) closeCreatePanel();
          else if (modalOpen) { setShowCreditsPanel(false); setShowDeleteConfirm(false); setShowIdentityPanel(false); }
          else onExit();
        } else if (event.key === "Enter" && !modalOpen
          && !(event.target as HTMLElement).closest("button,input,a,select,textarea,summary")) {
          event.preventDefault(); event.stopPropagation();
          if (availability.hasSelection && !createPending) onEnterWorld();
        }
      }}>
      <style>{R16_AUTH_STYLES}</style>
      <div className="select-scene" style={{ width: R16_AUTH_LAYOUT.stage.width, height: R16_AUTH_LAYOUT.stage.height }}>
        {/* Native's global footer overlaps the 1024x768 select actions. Keep its
            menu geometry while moving this footer group above that action row. */}
        <div className="r16-select-footer" style={{ position: "absolute", left: 0, right: 0, bottom: R16_AUTH_LAYOUT.footer.selectLift, height: 0 }}>
        <LanguageSelector
          language={language}
          t={t}
          compact
          nativeFooter
          className="select-language-selector"
          onLanguageChange={onLanguageChange}
        />
        <AuthBrowserUtilities language={language} t={t} onOpenAccountSecurity={onOpenAccountSecurity} blocked={modalOpen} />
        </div>
        {accountId.startsWith("obl_") && !identityProvider?.startsWith("crazyGames") ? (
          <button
            type="button"
            className="select-identity-trigger"
            onClick={() => setShowIdentityPanel((current) => !current)}
          >
            Obelisk ID · {identityProvider ?? "channel"}
          </button>
        ) : null}
        <img className="select-background-frame" src={ORIGINAL_UI.select.background} alt="" draggable={false} />
        <h1 className="r16-auth-label" style={{ ...R16_AUTH_LAYOUT.select.title, textAlign: "center", lineHeight: "34px", fontSize: 22 }}>
          {r16AuthCaption(language, "character_select", t("ui.selectCharacter", [], "Select Character"))}
        </h1>
        <div className="select-server-name">{t("client.GameName", [], "Legend of Mir 2")}</div>

        <div
          className="select-portrait-anchor"
          style={{ left: SELECT_PORTRAIT_ANCHOR.x, top: SELECT_PORTRAIT_ANCHOR.y, visibility: showCreatePanel ? "hidden" : undefined }}
        >
          {activePortraitFrame ? (
            <img
              className="select-portrait-frame"
              src={activePortraitFrame.path}
              alt=""
              draggable={false}
              style={{ left: activePortraitFrame.x, top: activePortraitFrame.y }}
            />
          ) : null}
          {selectedPortraitOverlay ? <img className="select-portrait-frame" src={selectedPortraitOverlay.path} alt="" draggable={false}
            style={{ left: selectedPortraitOverlay.x, top: selectedPortraitOverlay.y, mixBlendMode: "plus-lighter" }} /> : null}
        </div>

        {selected ? <>
          <div className="select-last-access-label">{r16AuthCaption(language, "last_online", t("client.LastOnlineTitle", [], "Last Online:"))}</div>
          <div className="select-last-access-value">{selected.lastAccess ?? r16AuthCaption(language, "never", t("client.Never", [], "Never"))}</div>
        </> : null}

        {Array.from({ length: 4 }, (_, slotIndex) => {
          const slotEntry = characters[slotIndex] ?? null;
          // The synthesized placeholder renders as an empty slot (no real character).
          const character = slotEntry && !slotEntry.synthetic ? slotEntry : null;
          return (
            <button
              key={`select-slot-${slotIndex}`}
              type="button"
              className={`select-character-slot-card row-${slotIndex + 1} ${character ? "" : "empty"} ${character && slotIndex === selectedCharacterIndex ? "selected" : ""}`}
              style={{ left: R16_AUTH_LAYOUT.select.slotLeft, top: R16_AUTH_LAYOUT.select.slotTops[slotIndex],
                width: R16_AUTH_LAYOUT.select.slotWidth, height: character ? R16_AUTH_LAYOUT.select.occupiedSlotHeight : R16_AUTH_LAYOUT.select.emptySlotHeight }}
              disabled={!character || modalOpen}
              aria-pressed={!!character && slotIndex === selectedCharacterIndex}
              data-gamepad-initial={character && slotIndex === selectedCharacterIndex ? "true" : undefined}
              onClick={() => {
                if (character) {
                  onSelectCharacter(slotIndex);
                }
              }}
            >
              {character ? (
                <>
                  <div className="select-character-slot-copy">
                    <strong className="name">{character.name}</strong>
                    <span className="level">{r16AuthCaption(language, "level", `Nv. ${character.level}`).replace("{level}", String(character.level))}</span>
                    <span className="job">{r16AuthCaption(language, character.classKey, selectClassLabel(t, character.classKey))}</span>
                  </div>
                </>
              ) : (
                <span className="r16-empty-slot">{r16AuthCaption(language, "empty_slot", t("client.Empty", [], "Empty Slot"))}</span>
              )}
            </button>
          );
        })}

        <div className="select-action start" style={R16_AUTH_LAYOUT.select.buttons.start}><SpriteButton sprite={ORIGINAL_UI.select.buttons.start} label={r16AuthCaption(language, "start", t("ui.startGame"))} nativeCaption disabled={!availability.hasSelection || modalOpen || createPending} onClick={onEnterWorld} /></div>
        <div className="select-action new" style={R16_AUTH_LAYOUT.select.buttons.new}><SpriteButton sprite={ORIGINAL_UI.select.buttons.newCharacter} label={r16AuthCaption(language, "create", t("ui.newCharacter"))} nativeCaption disabled={!canCreate || modalOpen} onClick={openCreatePanel} /></div>
        <div className="select-action delete" style={R16_AUTH_LAYOUT.select.buttons.delete}>
          <SpriteButton
            sprite={ORIGINAL_UI.select.buttons.deleteCharacter}
            label={r16AuthCaption(language, "delete_character", t("ui.deleteCharacter"))}
            nativeCaption disabled={!availability.hasSelection || modalOpen || createPending}
            onClick={() => setShowDeleteConfirm((current) => !current)}
          />
        </div>
        <div className="select-action credits" style={R16_AUTH_LAYOUT.select.buttons.credits}>
          <SpriteButton
            sprite={ORIGINAL_UI.select.buttons.credits}
            label={r16AuthCaption(language, "credits", t("ui.credits"))}
            nativeCaption disabled={modalOpen}
            onClick={() => setShowCreditsPanel((current) => !current)}
          />
        </div>
        <div className="select-action exit" style={R16_AUTH_LAYOUT.select.buttons.exit}><SpriteButton sprite={ORIGINAL_UI.select.buttons.exit} label={r16AuthCaption(language, "exit", t("ui.exit"))} nativeCaption disabled={modalOpen} onClick={onExit} /></div>
        {showIdentityPanel ? (
          <div className="select-identity-panel">
            <strong>{identityPanelText(language, "title")}</strong>
            <span className="select-identity-id">{accountId}</span>
            <span>{identityPanelText(language, "description")}</span>
            <div className="select-identity-actions">
              <button type="button" disabled={identityLinkBusy} onClick={onIdentityLinkPasskey}>
                {identityLinkBusy
                  ? identityPanelText(language, "linking")
                  : identityPanelText(language, "passkey")}
              </button>
              <button
                type="button"
                disabled={identityLinkBusy || suiWallets.length === 0}
                onClick={() => onIdentityLinkWallet(suiWallets[0]?.id)}
              >
                {identityPanelText(language, "wallet")}
              </button>
            </div>
            {suiWallets.length === 0 ? (
              <span>{identityPanelText(language, "noWallet")}</span>
            ) : null}
            {identityLinkStatus ? <span className="select-identity-status">{identityLinkStatus}</span> : null}
          </div>
        ) : null}
        {!showCreatePanel && createNotice ? <div className="select-create-error" style={R16_AUTH_LAYOUT.select.notice} role="status">{createNotice}</div> : null}
        {showCreatePanel ? (
          <div className="r16-auth-modal">
          <form className="select-create-panel" style={R16_AUTH_LAYOUT.create.panel} role="dialog" aria-modal="true" aria-labelledby="r16-create-title" aria-busy={createPending}
            onSubmit={event => { event.preventDefault(); submitCreateCharacter(); }}
            onKeyDown={event => {
              if (event.nativeEvent.isComposing) return;
              const field = (event.target as HTMLElement).closest<HTMLElement>("[data-r16-create-focus]")?.dataset.r16CreateFocus;
              if (createPending && event.key === "Tab") {
                event.preventDefault(); event.stopPropagation();
                event.currentTarget.querySelector<HTMLElement>('[data-r16-create-focus="cancel"]')?.focus({ preventScroll: true });
              } else if (createPending && event.key === "Enter" && field !== "cancel") {
                event.preventDefault(); event.stopPropagation();
              } else if (event.key === "Tab" && field) {
                const order = ["name", "class", "gender", "create", "cancel"];
                const next = order[(order.indexOf(field) + (event.shiftKey ? 4 : 1)) % order.length];
                const target = event.currentTarget.querySelector<HTMLElement>(`[data-r16-create-focus="${next}"][aria-pressed="true"]`)
                  ?? event.currentTarget.querySelector<HTMLElement>(`[data-r16-create-focus="${next}"]`);
                if (target) { event.preventDefault(); event.stopPropagation(); target.focus({ preventScroll: true }); }
              } else if (event.key === "Enter" && (field === "name" || field === "class" || field === "gender")) {
                event.preventDefault(); event.stopPropagation();
                if (field === "name") event.currentTarget.querySelector<HTMLElement>('[data-r16-create-focus="class"][aria-pressed="true"]')?.focus({ preventScroll: true });
                else if (field === "class" && characterCreate.classes.length) {
                  const classes = characterCreate.classes;
                  setCreateClassKey(classes[(classes.indexOf(createClassKey) + 1) % classes.length]);
                }
                else setCreateGender(createGender === "male" ? "female" : "male");
              }
            }}>
            <div className="select-create-title" id="r16-create-title" style={r16AuthRelativeRect(R16_AUTH_LAYOUT.create.title, R16_AUTH_LAYOUT.create.panel)}>
              {r16AuthCaption(language, "character_create", t("ui.newCharacter"))}
            </div>
            <div className="select-portrait-anchor" style={{ left: R16_AUTH_LAYOUT.create.preview.x - R16_AUTH_LAYOUT.create.panel.left, top: R16_AUTH_LAYOUT.create.preview.y - R16_AUTH_LAYOUT.create.panel.top }}>
              {createPortraitFrame ? <img className="select-portrait-frame" src={createPortraitFrame.path} alt="" draggable={false} style={{ left: createPortraitFrame.x, top: createPortraitFrame.y }} /> : null}
              {createPortraitOverlay ? <img className="select-portrait-frame" src={createPortraitOverlay.path} alt="" draggable={false}
                style={{ left: createPortraitOverlay.x, top: createPortraitOverlay.y, mixBlendMode: "plus-lighter" }} /> : null}
            </div>
            <p className="r16-create-description" style={r16AuthRelativeRect(R16_AUTH_LAYOUT.create.description, R16_AUTH_LAYOUT.create.panel)}>
              {r16AuthCaption(language, `${createClassKey}_description`)}
            </p>
            <label className="select-create-name-field">
              <span style={r16AuthRelativeRect(R16_AUTH_LAYOUT.create.nameLabel, R16_AUTH_LAYOUT.create.panel)}>{r16AuthCaption(language, "character_name", createPanelText(language, "name"))}</span>
              <input
                style={r16AuthRelativeRect(R16_AUTH_LAYOUT.create.name, R16_AUTH_LAYOUT.create.panel)}
                ref={createNameRef}
                data-r16-create-focus="name"
                readOnly={createPending}
                value={createName}
                maxLength={R16_AUTH_LAYOUT.create.maxNameLength}
                autoComplete="off"
                onChange={(event) => {
                  if (createPending) return;
                  setCreateName(event.target.value.replace(/\s+/g, ""));
                  setCreateError(null);
                  characterCreate.clearNotice();
                }}
                onKeyDown={(event) => {
                  if (event.key === "Escape" && !event.nativeEvent.isComposing) {
                    event.preventDefault();
                    event.stopPropagation();
                    closeCreatePanel();
                  }
                }}
                autoFocus
              />
            </label>
            <div className="select-create-gender-row" role="group" aria-label={r16AuthCaption(language, "gender", createPanelText(language, "gender"))}>
              <span className="r16-auth-label" style={r16AuthRelativeRect(R16_AUTH_LAYOUT.create.genderLabel, R16_AUTH_LAYOUT.create.panel)}>{r16AuthCaption(language, "gender", createPanelText(language, "gender"))}</span>
              {CREATE_GENDER_OPTIONS.map((gender, index) => (
                <button
                  key={gender}
                  type="button"
                  data-r16-create-focus="gender"
                  disabled={createPending}
                  style={r16AuthRelativeRect(R16_AUTH_LAYOUT.create.genders[index], R16_AUTH_LAYOUT.create.panel)}
                  className={`select-create-gender-button ${createGender === gender ? "active" : ""}`}
                  aria-pressed={createGender === gender}
                  onClick={() => {
                    setCreateGender(gender);
                    setCreateError(null);
                    characterCreate.clearNotice();
                  }}
                >
                  {r16AuthCaption(language, gender, createPanelText(language, gender))}
                </button>
              ))}
            </div>
            <div className="select-create-class-list" role="group" aria-label={r16AuthCaption(language, "class", createPanelText(language, "class"))}>
              <span className="r16-auth-label" style={r16AuthRelativeRect(R16_AUTH_LAYOUT.create.classLabel, R16_AUTH_LAYOUT.create.panel)}>{r16AuthCaption(language, "class", createPanelText(language, "class"))}</span>
              {characterCreate.classes.map((classKey, index) => (
                <button
                  key={classKey}
                  type="button"
                  data-r16-create-focus="class"
                  disabled={createPending}
                  style={r16AuthRelativeRect(R16_AUTH_LAYOUT.create.classes[index], R16_AUTH_LAYOUT.create.panel)}
                  className={`select-create-class-card ${createClassKey === classKey ? "active" : ""}`}
                  aria-pressed={createClassKey === classKey}
                  onClick={() => {
                    setCreateClassKey(classKey);
                    setCreateError(null);
                    characterCreate.clearNotice();
                  }}
                >
                  <span>{r16AuthCaption(language, classKey, selectClassLabel(t, classKey))}</span>
                </button>
              ))}
            </div>
            {createError || createNotice ? (
              <div className="select-create-error" style={r16AuthRelativeRect(R16_AUTH_LAYOUT.create.notice, R16_AUTH_LAYOUT.create.panel)} role="status">
                {createError ?? createNotice}
              </div>
            ) : null}
            <div className="select-create-actions">
              <div style={r16AuthRelativeRect(R16_AUTH_LAYOUT.create.submit, R16_AUTH_LAYOUT.create.panel)}>
              <SpriteButton
                sprite={ORIGINAL_UI.select.buttons.createCharacter}
                label={r16AuthCaption(language, "create", createPanelText(language, "create"))}
                nativeCaption createFocus="create"
                disabled={!canCreate}
                onClick={submitCreateCharacter}
              />
              </div>
              <div style={r16AuthRelativeRect(R16_AUTH_LAYOUT.create.cancel, R16_AUTH_LAYOUT.create.panel)}>
              <SpriteButton
                sprite={ORIGINAL_UI.select.buttons.cancel}
                label={r16AuthCaption(language, "cancel", t("ui.close"))}
                nativeCaption createFocus="cancel"
                onClick={closeCreatePanel}
              />
              </div>
            </div>
          </form>
          </div>
        ) : null}
        {showCreditsPanel ? (
          <div className="select-credits-panel">
            <strong>{t("ui.credits")}</strong>
            <span>{t("client.GameName", [], "Legend of Mir 2")}</span>
            <span>{accountId}</span>
            <button type="button" onClick={() => setShowCreditsPanel(false)}>
              {t("ui.close")}
            </button>
          </div>
        ) : null}
        {showDeleteConfirm ? (
          <div className="select-delete-panel">
            <strong>{t("ui.deleteCharacter")}</strong>
            <span>{selected?.name ?? "-"}</span>
            <div className="select-delete-actions">
              <button
                type="button"
                onClick={() => {
                  onDeleteCharacter();
                  setShowDeleteConfirm(false);
                }}
              >
                {t("ui.confirm", [], "Confirm")}
              </button>
              <button type="button" onClick={() => setShowDeleteConfirm(false)}>
                {t("ui.close")}
              </button>
            </div>
          </div>
        ) : null}
      </div>
    </section>
  );
}

export type MainHudProps = {
  t: TranslateFn;
  connected: boolean;
  mapTitle: string | null;
  player: HudPlayerLike;
  world: HudWorldLike;
  hpView?: boolean;
  showCharacter: boolean;
  showInventory: boolean;
  showQuestLog: boolean;
  activeCharacterTab: CharacterTabKey;
  activeInventoryTab: InventoryTabKey;
  onToggleCharacter: () => void;
  onToggleInventory: () => void;
  onToggleQuestLog: () => void;
  onToggleOptions?: () => void;
  onOpenCharacterTab: (tab: CharacterTabKey) => void;
  onOpenInventoryTab: (tab: InventoryTabKey) => void;
  onDropGold: () => void;
  onLogout: () => void;
  showGameShop: boolean;
  onToggleGameShop: () => void;
  showGameShopAction?: boolean;
  showMenu: boolean;
  onToggleMenu: () => void;
};

/** Unmigrated passive information, retained after the shared main HUD handoff. */
export function HudSpriteFallback({ plan, experienceOwned, weightOwned }: {
  plan: import("../../lib/bevy-hud-ui").MainHudPlan | null; experienceOwned: boolean; weightOwned: boolean;
}) {
  if (!plan) return null;
  return <>{([[plan.experienceSprite, experienceOwned], [plan.weightSprite, weightOwned]] as const).map(([sprite, owned], index) =>
    !owned && sprite?.image && sprite.source.width > 0 ? <div key={index} data-shared-hud-sprite-fallback={index === 0 ? "experience" : "weight"}
      style={{ position: "absolute", left: sprite.destination.left, top: sprite.destination.top, width: sprite.destination.width,
        height: sprite.destination.height, overflow: "hidden", pointerEvents: "none" }}>
      <img src={`/${sprite.image}`} alt="" draggable={false} style={{ position: "absolute", left: -sprite.source.left, top: -sprite.source.top, maxWidth: "none" }} />
    </div> : null)}</>;
}

export function MainHudStatus({ t, mapTitle, world }: Pick<MainHudProps, "t" | "mapTitle" | "world">) {
  const buffLabel = world.activeBuffs.slice(0, 2).map(buff => `${buff.name}:${buff.remainingTicks}`).join("  ");
  return <div className="hud-passive-compat" style={{ position: "absolute", left: 0, top: 618, width: 1024, height: 152, pointerEvents: "none" }}>
    <div className="hud-map-label">{mapTitle ?? world.mapTitle ?? ""}{world.inSafeZone ? ` ${t("ui.safeZone", [], "Safe Zone")}` : ""}</div>
    {buffLabel ? <div className="hud-buff-label">{buffLabel}</div> : null}
    <div className="hud-space-label">{crystalMainHudFreeSlots(world)}</div>
  </div>;
}

/** Native shared_hud.rs HPView labels; coordinates are in the 1024x768 stage. */
export function crystalMainHudHealthLabels({ player, world, hpView = true }: Pick<MainHudProps, "player" | "world" | "hpView">) {
  const hpOnly = (player?.classKey ?? "warrior") === "warrior" && (player?.level ?? 1) < 26;
  const hp = world.playerHp ?? 0, maxHp = world.playerMaxHp ?? 0;
  const mp = world.playerMp ?? 0, maxMp = world.playerMaxMp ?? 0;
  if (hpView) return [
    { kind: "compactHp", text: `HP ${hp}/${maxHp}`, left: 0, top: 673, width: 100, height: 14, compact: true },
    ...(!hpOnly ? [{ kind: "compactMp", text: `MP ${mp}/${maxMp} `, left: 0, top: 688, width: 100, height: 14, compact: true }] : []),
  ];
  return [
    { kind: "alternateTop", text: hpOnly ? `${hp}\n--` : ` ${hp}    ${mp} \n---------------`, left: 9, top: 666, width: 85, height: 30, compact: false },
    { kind: "alternateBottom", text: hpOnly ? String(maxHp) : ` ${maxHp}    ${maxMp} `, left: 9, top: 696, width: 85, height: 30, compact: false },
  ];
}

/** Passive labels only. Shared renderer hosts must retire their own labels before opting in. */
export function MainHudHealthLabels({ player, world, hpView = true, originTop = 618 }: Pick<MainHudProps, "player" | "world" | "hpView"> & { originTop?: number }) {
  return <>{crystalMainHudHealthLabels({ player, world, hpView }).map(label =>
    <div key={label.kind} data-hud-health-label={label.kind}
      style={{ position: "absolute", left: label.left, top: label.top - originTop, width: label.width, height: label.height,
        display: label.compact ? "flex" : "block", alignItems: "center", justifyContent: "center",
        overflow: "hidden", pointerEvents: "none", whiteSpace: "pre", textAlign: "center",
        fontFamily: "Arial, Helvetica, sans-serif", fontSize: 32 / 3, fontWeight: 400, color: "#fff",
        textShadow: "1px 1px 0 #000" }}>{label.text}</div>)}</>;
}

export function MainHud({
  t,
  connected,
  mapTitle,
  player,
  world,
  hpView = true,
  showCharacter,
  showInventory,
  showQuestLog,
  activeCharacterTab,
  activeInventoryTab,
  onToggleCharacter,
  onToggleInventory,
  onToggleQuestLog,
  onToggleOptions,
  onOpenCharacterTab,
  onOpenInventoryTab,
  showGameShop,
  onToggleGameShop,
  showGameShopAction = true,
  showMenu,
  onToggleMenu,
}: MainHudProps) {
  const healthRatio = ratio(world.playerHp, world.playerMaxHp);
  const manaRatio = ratio(world.playerMp, world.playerMaxMp);
  const experienceRatio = ratio(world.playerExperience, world.playerMaxExperience);
  const hpOnlyOrb = (player?.classKey ?? "warrior") === "warrior" && (player?.level ?? 1) < 26;
  const locationLabel = mapTitle ?? world.mapTitle ?? "";
  const buffLabel = world.activeBuffs
    .slice(0, 2)
    .map((buff) => `${buff.name}:${buff.remainingTicks}`)
    .join("  ");
  const remainingBagWeight = Math.max(0, Math.floor(world.maxWeight - world.currentWeight));
  const crystalFreeSlots = crystalMainHudFreeSlots(world);
  const experienceBarFillWidth = crystalMainHudExperienceBarFillWidth(experienceRatio);
  const bagWeightRatio = ratio(world.currentWeight, world.maxWeight);
  const weightBarFillWidth = crystalMainHudWeightBarFillWidth(bagWeightRatio);
  const weightBarSprite = crystalMainHudWeightBarSprite(bagWeightRatio);

  return (
    <div className="main-hud-shell">
      <div className="main-hud">
        <img className="hud-cap left" src={ORIGINAL_UI.hud.leftCap} alt="" draggable={false} />
        <img className="hud-base" src={ORIGINAL_UI.hud.base} alt="" draggable={false} />
        <img className="hud-cap right" src={ORIGINAL_UI.hud.rightCap} alt="" draggable={false} />
        <div
          className="hud-exp-bar"
          data-experience-ratio={experienceRatio.toFixed(4)}
          data-fill-width={experienceBarFillWidth}
          data-experience={world.playerExperience}
          data-max-experience={world.playerMaxExperience}
        >
          <div className="hud-exp-bar-clip" style={{ width: `${experienceBarFillWidth}px` }}>
            {experienceBarFillWidth > 0 ? (
              <img className="hud-exp-bar-fill" src={ORIGINAL_UI.hud.experienceBar} alt="" draggable={false} />
            ) : null}
          </div>
        </div>
        <div
          className="hud-weight-bar"
          data-current-weight={world.currentWeight}
          data-max-weight={world.maxWeight}
          data-weight-ratio={bagWeightRatio.toFixed(4)}
          data-fill-width={weightBarFillWidth}
          data-mir2-original-src={weightBarSprite}
        >
          <div className="hud-weight-bar-clip" style={{ width: `${weightBarFillWidth}px` }}>
            {weightBarFillWidth > 0 ? (
              <img
                className="hud-weight-bar-fill"
                src={weightBarSprite}
                alt=""
                draggable={false}
                data-mir2-original-src={weightBarSprite}
                onError={handleSceneAssetImageError}
                onLoad={handleSceneAssetImageLoad}
              />
            ) : null}
          </div>
        </div>

        <div className={`hud-orb-fill hp ${hpOnlyOrb ? "hp-only" : ""}`} style={{ height: `${80 * healthRatio}px` }}>
          <img src={hpOnlyOrb ? ORIGINAL_UI.hud.healthOnlyOrb : ORIGINAL_UI.hud.healthManaOrb} alt="" draggable={false} />
        </div>
        <div className={`hud-orb-fill mp ${hpOnlyOrb ? "hidden" : ""}`} style={{ height: `${80 * manaRatio}px` }}>
          <img src={ORIGINAL_UI.hud.healthManaOrb} alt="" draggable={false} />
        </div>

        <MainHudHealthLabels player={player} world={world} hpView={hpView} />
        <div className="hud-level-label">{player?.level ?? 1}</div>
        <div className="hud-name-label">{player?.name ?? ""}</div>
        <div className="hud-map-label">
          {locationLabel}
          {world.inSafeZone ? ` ${t("ui.safeZone", [], "Safe Zone")}` : ""}
        </div>
        {buffLabel ? <div className="hud-buff-label">{buffLabel}</div> : null}
        <div className="hud-exp-label">{formatCrystalExperiencePercent(experienceRatio)}</div>
        <div className="hud-gold-label">{connected ? formatCrystalMainHudGold(world.gold) : "0"}</div>
        <div className="hud-weight-label">{remainingBagWeight}</div>
        <div className="hud-space-label">{crystalFreeSlots}</div>

        {showGameShopAction ? (
          <div className="hud-button shop">
            <SpriteButton sprite={ORIGINAL_UI.hud.buttons.gameShop} label={t("ui.gameShop")} onClick={onToggleGameShop} active={showGameShop} />
          </div>
        ) : null}
        <div className="hud-button menu">
          <SpriteButton sprite={ORIGINAL_UI.hud.buttons.menu} label={t("ui.menu")} onClick={onToggleMenu} active={showMenu} />
        </div>
        <div className="hud-button character">
          <SpriteButton sprite={ORIGINAL_UI.hud.buttons.character} label={t("ui.character")} onClick={onToggleCharacter} active={showCharacter && activeCharacterTab === "char"} />
        </div>
        <div className="hud-button inventory">
          <SpriteButton sprite={ORIGINAL_UI.hud.buttons.inventory} label={t("ui.inventory")} onClick={onToggleInventory} active={showInventory && activeInventoryTab === "bag1"} />
        </div>
        <div className="hud-button skill">
          <SpriteButton sprite={ORIGINAL_UI.hud.buttons.skill} label={t("ui.skills")} onClick={() => onOpenCharacterTab("spells")} active={showCharacter && activeCharacterTab === "spells"} />
        </div>
        <div className="hud-button quest">
          <SpriteButton sprite={ORIGINAL_UI.hud.buttons.quest} label={t("ui.quest")} onClick={onToggleQuestLog} active={showQuestLog} />
        </div>
        <div className="hud-button option">
          <SpriteButton sprite={ORIGINAL_UI.hud.buttons.option} label={t("ui.options")} onClick={onToggleOptions} disabled={!onToggleOptions} />
        </div>
      </div>
    </div>
  );
}

export type SpriteButtonProps = {
  sprite: SpriteState;
  label: string;
  onClick?: () => void;
  onPointerActivate?: () => void;
  active?: boolean;
  disabled?: boolean;
  /** R16 auth word-art replacement; other game buttons keep their sprite face. */
  nativeCaption?: boolean;
  loginFocus?: R16LoginFocus;
  createFocus?: "create" | "cancel";
};

export function SpriteButton({ sprite, label, onClick, onPointerActivate, active = false, disabled = false,
  nativeCaption = false, loginFocus, createFocus }: SpriteButtonProps) {
  const [hovered, setHovered] = useState(false);
  const [pressed, setPressed] = useState(false);
  const pointerActivatedRef = useRef(false);
  const suppressClickRef = useRef(false);

  let source = sprite.base;
  if (!disabled && pressed && sprite.pressed) {
    source = sprite.pressed;
  } else if (!disabled && active && sprite.active) {
    source = sprite.active;
  } else if (!disabled && (hovered || active) && sprite.hover) {
    source = sprite.hover;
  }

  return (
    <button
      type="button"
      className={`sprite-button${disabled ? " disabled" : ""}`}
      disabled={disabled}
      data-r16-auth-button={nativeCaption || undefined}
      data-r16-login-focus={loginFocus}
      data-r16-create-focus={createFocus}
      onClick={(event) => {
        if (disabled) return;
        if (onPointerActivate && event.detail !== 0 && suppressClickRef.current) {
          suppressClickRef.current = false;
          return;
        }
        playOriginalSoundId(ORIGINAL_SOUND_IDS.uiButtonClick);
        onClick?.();
      }}
      onPointerDown={(event) => {
        if (disabled) return;
        if (!onPointerActivate) return;
        event.preventDefault();
        pointerActivatedRef.current = true;
        suppressClickRef.current = true;
        setPressed(true);
        playOriginalSoundId(ORIGINAL_SOUND_IDS.uiButtonClick);
        onPointerActivate();
      }}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => {
        pointerActivatedRef.current = false;
        suppressClickRef.current = false;
        setHovered(false);
        setPressed(false);
      }}
      onMouseDown={() => {
        if (disabled) return;
        setPressed(true);
        if (onPointerActivate && !pointerActivatedRef.current) {
          pointerActivatedRef.current = true;
          suppressClickRef.current = true;
          playOriginalSoundId(ORIGINAL_SOUND_IDS.uiButtonClick);
          onPointerActivate();
        }
      }}
      onMouseUp={() => {
        if (disabled) return;
        pointerActivatedRef.current = false;
        setPressed(false);
      }}
      aria-label={label}
      title={label}
      aria-disabled={disabled || undefined}
    >
      {nativeCaption ? <span>{label}</span> : null}
      <img
        src={source}
        alt=""
        draggable={false}
        data-mir2-original-src={source}
        onError={handleSceneAssetImageError}
        onLoad={handleSceneAssetImageLoad}
      />
    </button>
  );
}

function ratio(value?: number, max?: number) {
  if (value === undefined || max === undefined || max <= 0) {
    return 0;
  }

  return Math.max(0, Math.min(1, value / max));
}

function classCardForCharacter(character: SelectCharacterEntryLike, selected: boolean) {
  return classCardForKey(character.classKey, selected);
}

function classCardForKey(classKey: EntityClassKey, selected: boolean) {
  const card = ORIGINAL_UI.select.classCards[classKey];
  return selected ? card.active : card.base;
}

function classIconForKey(classKey: EntityClassKey, selected: boolean) {
  const sprite = ORIGINAL_UI.gameShop.classTabs[classKey];
  return selected ? sprite.active ?? sprite.hover ?? sprite.base : sprite.base;
}

function selectPortraitFrameFor(classKey: EntityClassKey, gender: EntityGenderKey) {
  const key = `${classKey}${gender === "male" ? "Male" : "Female"}` as SelectPortraitKey;
  const frames = SELECT_PORTRAIT_ANIMATIONS[key];
  return frames[0] ?? null;
}

function randomCharacterName() {
  return `MIR${Math.random().toString(36).slice(2, 6).toUpperCase()}`;
}

function createPanelText(language: Mir2Language, key: "name" | "gender" | "class" | "male" | "female" | "create" | "nameRequired" | "nameExists" | "slotsFull") {
  const dictionary: Partial<Record<Mir2Language, Record<typeof key, string>>> & Record<"en", Record<typeof key, string>> = {
    en: {
      name: "Name",
      gender: "Gender",
      class: "Class",
      male: "Male",
      female: "Female",
      create: "Create",
      nameRequired: "Please enter the character name.",
      nameExists: "A character with this name already exists.",
      slotsFull: "All character slots are full.",
    },
    "zh-CN": {
      name: "\u540d\u79f0",
      gender: "\u6027\u522b",
      class: "\u804c\u4e1a",
      male: "\u7537",
      female: "\u5973",
      create: "\u521b\u5efa",
      nameRequired: "\u8bf7\u8f93\u5165\u89d2\u8272\u540d\u3002",
      nameExists: "\u5df2\u5b58\u5728\u540c\u540d\u89d2\u8272\u3002",
      slotsFull: "\u89d2\u8272\u680f\u4f4d\u5df2\u6ee1\u3002",
    },
    es: {
      name: "Nombre",
      gender: "Genero",
      class: "Clase",
      male: "Hombre",
      female: "Mujer",
      create: "Crear",
      nameRequired: "Enter a character name.",
      nameExists: "A character with this name already exists.",
      slotsFull: "All character slots are full.",
    },
    "pt-BR": {
      name: "Nome",
      gender: "Sexo",
      class: "Classe",
      male: "Masculino",
      female: "Feminino",
      create: "Criar",
      nameRequired: "Digite o nome do personagem.",
      nameExists: "Já existe um personagem com esse nome.",
      slotsFull: "Todos os espaços de personagem estão cheios.",
    },
  };
  return dictionary[language]?.[key] ?? dictionary.en[key];
}

function identityPanelText(
  language: Mir2Language,
  key: "title" | "description" | "linking" | "passkey" | "wallet" | "noWallet",
) {
  const english = {
    title: "Character ownership",
    description: "Bind a Passkey to keep using these characters across supported channels.",
    linking: "Linking...",
    passkey: "Link Sui Passkey",
    wallet: "Link Dubhe / Sui Wallet",
    noWallet: "No Sui wallet detected. Passkey is recommended.",
  };
  const chinese: typeof english = {
    title: "角色归属",
    description: "绑定 Passkey 后，可在支持的渠道继续使用这些角色。",
    linking: "绑定中…",
    passkey: "绑定 Sui Passkey",
    wallet: "绑定 Dubhe / Sui Wallet",
    noWallet: "未检测到 Sui 钱包；建议优先使用 Passkey。",
  };
  return (language === "zh-CN" ? chinese : english)[key];
}

function selectClassLabel(t: TranslateFn, classKey: EntityClassKey) {
  switch (classKey) {
    case "warrior":
      return t("client.Warrior", [], "Warrior");
    case "wizard":
      return t("client.Wizard", [], "Wizard");
    case "taoist":
      return t("client.Taoist", [], "Taoist");
    case "assassin":
      return t("client.Assassin", [], "Assassin");
    case "archer":
      return t("client.Archer", [], "Archer");
  }
}
