"use client";

import { useEffect, useRef, useState } from "react";

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
  SUPPORTED_LANGUAGES,
  type Mir2Language,
} from "../../lib/localization";
import type { SuiWalletSummary } from "../../lib/client-login-runtime";
import { crystalMainHudExperienceBarFillWidth } from "../../lib/crystal-hud-metrics";
import { formatCrystalExperiencePercent } from "../../lib/extended-server-packets";
import { playOriginalSoundId } from "../../lib/original-audio";
import { ORIGINAL_SOUND_IDS } from "../../lib/original-sound-events";
import { OriginalAudioSettingsControls } from "./original-client-audio-settings";
import { CrystalGdiTextImage, findCrystalGdiTextAsset } from "./crystal-gdi-text";
import {
  handleSceneAssetImageError,
  handleSceneAssetImageLoad,
} from "./original-client-scene-map-rendering";
import type {
  CreateCharacterDraft,
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
const CREATE_CLASS_OPTIONS: EntityClassKey[] = ["warrior", "wizard", "taoist", "assassin", "archer"];
const CREATE_GENDER_OPTIONS: EntityGenderKey[] = ["male", "female"];

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
};

export function LanguageSelector({
  language,
  t,
  onLanguageChange,
  compact = false,
  className = "",
}: LanguageSelectorProps) {
  const selectorClassName = ["language-selector", compact ? "compact" : "", className]
    .filter(Boolean)
    .join(" ");

  return (
    <section className={selectorClassName}>
      {compact ? null : (
        <div className="language-selector-copy">
          <strong>{t("ui.languageSettings")}</strong>
          <span>{t("ui.languageDescription")}</span>
        </div>
      )}
      <div className="language-selector-buttons">
        {SUPPORTED_LANGUAGES.map((option) => (
          <button
            key={option}
            type="button"
            className={`language-selector-button ${option === language ? "active" : ""}`}
            aria-pressed={option === language}
            onClick={() => onLanguageChange(option)}
          >
            {languageNativeName(option)}
          </button>
        ))}
      </div>
    </section>
  );
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
  suiWallets: SuiWalletSummary[];
  walletPickerOpen: boolean;
  dubheWalletUrl: string;
  onLanguageChange: (language: Mir2Language) => void;
  onAccountIdChange: (value: string) => void;
  onPasswordChange: (value: string) => void;
  onCreateAccount: () => void;
  onSubmitLogin: () => void;
  onPasskeyLogin: () => void;
  onWalletPickerToggle: () => void;
  onWalletLogin: (walletId: string) => void;
  onQuickEnter: () => void;
  onResetClient: () => void;
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
  suiWallets,
  walletPickerOpen,
  dubheWalletUrl,
  onLanguageChange,
  onAccountIdChange,
  onPasswordChange,
  onCreateAccount,
  onSubmitLogin,
  onPasskeyLogin,
  onWalletPickerToggle,
  onWalletLogin,
  onQuickEnter,
  onResetClient,
}: LoginOverlayProps) {
  const loginNotice = loginError ?? (loginBusy ? t("ui.loggingIn") : null);
  const [showAccountPanel, setShowAccountPanel] = useState(false);
  const dubheWalletDetected = suiWallets.some((wallet) => wallet.isDubhe);

  return (
    <section className="login-overlay">
      <LanguageSelector
        language={language}
        t={t}
        compact
        className="login-language-selector"
        onLanguageChange={onLanguageChange}
      />
      <OriginalAudioSettingsControls t={t} compact className="login-audio-settings" />
      <form
        className="login-dialog"
        onSubmit={(event) => {
          event.preventDefault();
          onSubmitLogin();
        }}
      >
        <img className="login-panel" src={ORIGINAL_UI.login.dialog} alt="" draggable={false} />
        <img className="login-title" src={ORIGINAL_UI.login.title} alt="" draggable={false} />
        <img className="login-label account" src={ORIGINAL_UI.login.accountLabel} alt="" draggable={false} />
        <img className="login-label password" src={ORIGINAL_UI.login.passwordLabel} alt="" draggable={false} />
        <input
          className="login-input account"
          data-gamepad-initial="true"
          value={accountId}
          onChange={(event) => onAccountIdChange(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              event.preventDefault();
              onSubmitLogin();
            }
          }}
          autoComplete="off"
        />
        <input
          className="login-input password"
          type="password"
          value={password}
          onChange={(event) => onPasswordChange(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              event.preventDefault();
              onSubmitLogin();
            }
          }}
          autoComplete="off"
        />
        <div className="login-button ok">
          <SpriteButton sprite={ORIGINAL_UI.login.buttons.ok} label={t("ui.login")} onClick={onSubmitLogin} />
        </div>
        <div className="login-button account">
          <SpriteButton
            sprite={ORIGINAL_UI.login.buttons.newAccount}
            label={t("client.NewAccount", [], "New Account")}
            onClick={onCreateAccount}
          />
        </div>
        <div className="login-button password">
          <SpriteButton sprite={ORIGINAL_UI.login.buttons.changePassword} label={t("ui.quickEnter")} onClick={onQuickEnter} />
        </div>
        <div className="login-button view">
          <SpriteButton
            sprite={ORIGINAL_UI.login.buttons.viewKey}
            label={t("ui.viewKey")}
            onClick={() => setShowAccountPanel((current) => !current)}
          />
        </div>
        <div className="login-button close">
          <SpriteButton sprite={ORIGINAL_UI.login.buttons.close} label={t("ui.close")} onClick={onResetClient} />
        </div>
        <div className="login-web3-actions" aria-label={t("ui.web3Login", [], "Web3 login")}>
          <button type="button" onClick={onPasskeyLogin} disabled={loginBusy}>
            {t("ui.passkeyLogin", [], "Passkey")}
          </button>
          <button
            type="button"
            onClick={onWalletPickerToggle}
            disabled={loginBusy}
            aria-expanded={walletPickerOpen}
            aria-controls="login-wallet-picker"
          >
            {t("ui.walletLogin", [], "Wallet")}
          </button>
        </div>
        {walletPickerOpen ? (
          <div id="login-wallet-picker" className="login-wallet-picker" role="dialog" aria-label="Sui wallet">
            <div className="login-wallet-picker-title">Sui Wallet</div>
            {suiWallets.length > 0 ? (
              <div className="login-wallet-list">
                {suiWallets.map((wallet) => (
                  <button
                    key={wallet.id}
                    type="button"
                    className={`login-wallet-option ${wallet.isDubhe ? "dubhe" : ""}`}
                    onClick={() => onWalletLogin(wallet.id)}
                    disabled={loginBusy}
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
              <div className="login-wallet-empty">No Sui wallet found</div>
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
      </form>
      {showAccountPanel ? (
        <div className="login-account-panel">
          <strong>{t("ui.viewKey")}</strong>
          <span>{accountId || "-"}</span>
          <button type="button" onClick={() => setShowAccountPanel(false)}>
            {t("ui.close")}
          </button>
        </div>
      ) : null}
      {loginNotice ? <div className="login-feedback">{loginNotice}</div> : null}
      {runtimePhase === "boot-error" || wsState === "closed" ? (
        <div className="login-runtime-stamp" aria-hidden="true">{`${runtimePhase} / ${wsState} / ${runtimeMessage}`}</div>
      ) : null}
    </section>
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
  onCreateCharacter: (draft: CreateCharacterDraft) => void;
  onDeleteCharacter: () => void;
  onIdentityLinkPasskey: () => void;
  onIdentityLinkWallet: (walletId?: string) => void;
  onExit: () => void;
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
  onCreateCharacter,
  onDeleteCharacter,
  onIdentityLinkPasskey,
  onIdentityLinkWallet,
  onExit,
}: SelectOverlayProps) {
  const selectedRaw = characters[selectedCharacterIndex] ?? null;
  // A synthesized placeholder is not a playable character.
  const selected = selectedRaw && !selectedRaw.synthetic ? selectedRaw : null;
  const [showCreditsPanel, setShowCreditsPanel] = useState(false);
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);
  const [showCreatePanel, setShowCreatePanel] = useState(false);
  const [showIdentityPanel, setShowIdentityPanel] = useState(false);
  const [createName, setCreateName] = useState("");
  const [createClassKey, setCreateClassKey] = useState<EntityClassKey>("warrior");
  const [createGender, setCreateGender] = useState<EntityGenderKey>("male");
  const [createError, setCreateError] = useState<string | null>(null);
  const createPortraitFrame = selectPortraitFrameFor(createClassKey, createGender);
  // The selected-character portrait idle animation lives HERE (not in the shell)
  // so its 120ms tick re-renders only this overlay, not the ~3000-line shell.
  const [portraitFrameIndex, setPortraitFrameIndex] = useState(0);
  useEffect(() => {
    setPortraitFrameIndex(0);
  }, [selectedCharacterIndex, selectedPortraitFrames.length]);
  useEffect(() => {
    if (showCreatePanel || selectedPortraitFrames.length <= 1) {
      return;
    }
    const timer = window.setInterval(() => {
      setPortraitFrameIndex((current) => (current + 1) % selectedPortraitFrames.length);
    }, 120);
    return () => window.clearInterval(timer);
  }, [showCreatePanel, selectedPortraitFrames.length]);
  const animatedSelectedFrame =
    selectedPortraitFrames[portraitFrameIndex % Math.max(selectedPortraitFrames.length, 1)] ?? null;
  const activePortraitFrame = showCreatePanel ? createPortraitFrame : animatedSelectedFrame;

  useEffect(() => {
    if (showCreatePanel) {
      setShowCreditsPanel(false);
      setShowDeleteConfirm(false);
    }
  }, [showCreatePanel]);

  function openCreatePanel() {
    setCreateError(null);
    setCreateName((current) => current.trim() || randomCharacterName());
    setShowCreatePanel(true);
  }

  function closeCreatePanel() {
    setShowCreatePanel(false);
    setCreateError(null);
  }

  function submitCreateCharacter() {
    const trimmedName = createName.trim();
    if (!trimmedName) {
      setCreateError(t("client.PleaseEnterCharacterName", [], createPanelText(language, "nameRequired")));
      return;
    }
    if (characters.length >= 4) {
      setCreateError(createPanelText(language, "slotsFull"));
      return;
    }
    if (characters.some((character) => character.name.toLowerCase() === trimmedName.toLowerCase())) {
      setCreateError(t("client.CharacterNameExists", [], createPanelText(language, "nameExists")));
      return;
    }
    onCreateCharacter({
      name: trimmedName,
      classKey: createClassKey,
      gender: createGender,
    });
    setCreateName("");
    closeCreatePanel();
  }

  return (
    <section className="select-overlay">
      <div className="select-scene">
        <LanguageSelector
          language={language}
          t={t}
          compact
          className="select-language-selector"
          onLanguageChange={onLanguageChange}
        />
        <OriginalAudioSettingsControls t={t} compact className="select-audio-settings" />
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
        <img className="select-title" src={ORIGINAL_UI.select.title} alt="" draggable={false} />
        <div className="select-server-name">{t("client.GameName", [], "Legend of Mir 2")}</div>

        <div
          className="select-portrait-anchor"
          style={{ left: SELECT_PORTRAIT_ANCHOR.x, top: SELECT_PORTRAIT_ANCHOR.y }}
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
        </div>

        <div className="select-last-access-label">{t("client.LastOnlineTitle", [], "Last Online:")}</div>
        <div className="select-last-access-value">{selected?.lastAccess ?? t("client.Never", [], "Never")}</div>

        {Array.from({ length: 4 }, (_, slotIndex) => {
          const slotEntry = characters[slotIndex] ?? null;
          // The synthesized placeholder renders as an empty slot (no real character).
          const character = slotEntry && !slotEntry.synthetic ? slotEntry : null;
          return (
            <button
              key={`select-slot-${slotIndex}`}
              type="button"
              className={`select-character-slot-card row-${slotIndex + 1} ${character ? "" : "empty"} ${character && slotIndex === selectedCharacterIndex ? "selected" : ""}`}
              disabled={!character}
              data-gamepad-initial={character && slotIndex === selectedCharacterIndex ? "true" : undefined}
              onClick={() => {
                if (character) {
                  onSelectCharacter(slotIndex);
                }
              }}
            >
              {character ? (
                <>
                  <img
                    className="select-character-slot-frame"
                    src={classCardForCharacter(character, slotIndex === selectedCharacterIndex)}
                    alt=""
                    draggable={false}
                  />
                  <div className="select-character-slot-copy">
                    <strong className="name">{character.name}</strong>
                    <span className="level">{character.level}</span>
                    <span className="job">{selectClassLabel(t, character.classKey)}</span>
                  </div>
                </>
              ) : (
                <img className="select-character-slot-frame" src={ORIGINAL_UI.select.emptySlot} alt="" draggable={false} />
              )}
            </button>
          );
        })}

        <div className="select-action start"><SpriteButton sprite={ORIGINAL_UI.select.buttons.start} label={t("ui.startGame")} onClick={selected ? onEnterWorld : openCreatePanel} /></div>
        <div className="select-action new"><SpriteButton sprite={ORIGINAL_UI.select.buttons.newCharacter} label={t("ui.newCharacter")} onClick={openCreatePanel} /></div>
        <div className="select-action delete">
          <SpriteButton
            sprite={ORIGINAL_UI.select.buttons.deleteCharacter}
            label={t("ui.deleteCharacter")}
            onClick={() => setShowDeleteConfirm((current) => !current)}
          />
        </div>
        <div className="select-action credits">
          <SpriteButton
            sprite={ORIGINAL_UI.select.buttons.credits}
            label={t("ui.credits")}
            onClick={() => setShowCreditsPanel((current) => !current)}
          />
        </div>
        <div className="select-action exit"><SpriteButton sprite={ORIGINAL_UI.select.buttons.exit} label={t("ui.exit")} onClick={onExit} /></div>
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
        {showCreatePanel ? (
          <div className="select-create-panel">
            <div className="select-create-title">{t("ui.newCharacter")}</div>
            <label className="select-create-name-field">
              <span>{createPanelText(language, "name")}</span>
              <input
                value={createName}
                maxLength={12}
                autoComplete="off"
                onChange={(event) => {
                  setCreateName(event.target.value.replace(/\s+/g, ""));
                  setCreateError(null);
                }}
                onKeyDown={(event) => {
                  if (event.key === "Enter") {
                    event.preventDefault();
                    submitCreateCharacter();
                  } else if (event.key === "Escape") {
                    event.preventDefault();
                    closeCreatePanel();
                  }
                }}
                autoFocus
              />
            </label>
            <div className="select-create-gender-row" aria-label={createPanelText(language, "gender")}>
              {CREATE_GENDER_OPTIONS.map((gender) => (
                <button
                  key={gender}
                  type="button"
                  className={`select-create-gender-button ${createGender === gender ? "active" : ""}`}
                  aria-pressed={createGender === gender}
                  onClick={() => {
                    setCreateGender(gender);
                    setCreateError(null);
                  }}
                >
                  {createPanelText(language, gender)}
                </button>
              ))}
            </div>
            <div className="select-create-class-list" aria-label={createPanelText(language, "class")}>
              {CREATE_CLASS_OPTIONS.map((classKey) => (
                <button
                  key={classKey}
                  type="button"
                  className={`select-create-class-card ${createClassKey === classKey ? "active" : ""}`}
                  aria-pressed={createClassKey === classKey}
                  onClick={() => {
                    setCreateClassKey(classKey);
                    setCreateError(null);
                  }}
                >
                  <img
                    src={classIconForKey(classKey, createClassKey === classKey)}
                    alt=""
                    draggable={false}
                  />
                  <span>{selectClassLabel(t, classKey)}</span>
                </button>
              ))}
            </div>
            {createError ? <div className="select-create-error">{createError}</div> : null}
            <div className="select-create-actions">
              <SpriteButton
                sprite={ORIGINAL_UI.select.buttons.createCharacter}
                label={createPanelText(language, "create")}
                onClick={submitCreateCharacter}
              />
              <SpriteButton
                sprite={ORIGINAL_UI.select.buttons.cancel}
                label={t("ui.close")}
                onClick={closeCreatePanel}
              />
            </div>
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
  showCharacter: boolean;
  showInventory: boolean;
  activeCharacterTab: CharacterTabKey;
  activeInventoryTab: InventoryTabKey;
  onToggleCharacter: () => void;
  onToggleInventory: () => void;
  onOpenCharacterTab: (tab: CharacterTabKey) => void;
  onOpenInventoryTab: (tab: InventoryTabKey) => void;
  onDropGold: () => void;
  onLogout: () => void;
  showGameShop: boolean;
  onToggleGameShop: () => void;
  showMenu: boolean;
  onToggleMenu: () => void;
};

export function MainHud({
  t,
  connected,
  mapTitle,
  player,
  world,
  showCharacter,
  showInventory,
  activeCharacterTab,
  activeInventoryTab,
  onToggleCharacter,
  onToggleInventory,
  onOpenCharacterTab,
  onOpenInventoryTab,
  showGameShop,
  onToggleGameShop,
  showMenu,
  onToggleMenu,
}: MainHudProps) {
  const healthRatio = ratio(world.playerHp, world.playerMaxHp);
  const manaRatio = ratio(world.playerMp, world.playerMaxMp);
  const experienceRatio = ratio(world.playerExperience, world.playerMaxExperience);
  const currentHp = world.playerHp ?? 0;
  const maxHp = world.playerMaxHp ?? 0;
  const currentMp = world.playerMp ?? 0;
  const maxMp = world.playerMaxMp ?? 0;
  const hpOnlyOrb = (player?.classKey ?? "warrior") === "warrior" && (player?.level ?? 1) < 26;
  const hpOnlyText = `HP ${currentHp}/${maxHp}`;
  const hpOnlyGdiText = findCrystalGdiTextAsset({
    text: hpOnlyText,
    foreground: "#ffffff",
    outline: true,
  });
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
          style={{ width: `${experienceBarFillWidth}px` }}
        >
          {experienceBarFillWidth > 0 ? (
            <img
              className="hud-exp-bar-fill"
              src={ORIGINAL_UI.hud.experienceBar}
              alt=""
              draggable={false}
            />
          ) : null}
        </div>
        <div
          className="hud-weight-bar"
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

        {hpOnlyOrb ? (
          <div className="hud-health-only-label">
            {hpOnlyGdiText ? (
              <CrystalGdiTextImage asset={hpOnlyGdiText} accessibleText={hpOnlyText} />
            ) : hpOnlyText}
          </div>
        ) : (
          <>
            <div className="hud-top-label">{`${currentHp}    ${currentMp}`}</div>
            <div className="hud-bottom-label">{`${maxHp}    ${maxMp}`}</div>
          </>
        )}
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

        <div className="hud-button shop">
          <SpriteButton sprite={ORIGINAL_UI.hud.buttons.gameShop} label={t("ui.gameShop")} onClick={onToggleGameShop} active={showGameShop} />
        </div>
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
          <SpriteButton sprite={ORIGINAL_UI.hud.buttons.quest} label={t("ui.quest")} onClick={() => onOpenInventoryTab("quest")} active={showInventory && activeInventoryTab === "quest"} />
        </div>
        <div className="hud-button option">
          <SpriteButton sprite={ORIGINAL_UI.hud.buttons.option} label={t("ui.options")} onClick={() => onOpenCharacterTab("stats2")} active={showCharacter && activeCharacterTab === "stats2"} />
        </div>
      </div>
    </div>
  );
}

export type SpriteButtonProps = {
  sprite: SpriteState;
  label: string;
  onClick: () => void;
  onPointerActivate?: () => void;
  active?: boolean;
};

export function SpriteButton({ sprite, label, onClick, onPointerActivate, active = false }: SpriteButtonProps) {
  const [hovered, setHovered] = useState(false);
  const [pressed, setPressed] = useState(false);
  const pointerActivatedRef = useRef(false);
  const suppressClickRef = useRef(false);

  let source = sprite.base;
  if (pressed && sprite.pressed) {
    source = sprite.pressed;
  } else if (active && sprite.active) {
    source = sprite.active;
  } else if ((hovered || active) && sprite.hover) {
    source = sprite.hover;
  }

  return (
    <button
      type="button"
      className="sprite-button"
      onClick={(event) => {
        if (onPointerActivate && event.detail !== 0 && suppressClickRef.current) {
          suppressClickRef.current = false;
          return;
        }
        playOriginalSoundId(ORIGINAL_SOUND_IDS.uiButtonClick);
        onClick();
      }}
      onPointerDown={(event) => {
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
        setPressed(true);
        if (onPointerActivate && !pointerActivatedRef.current) {
          pointerActivatedRef.current = true;
          suppressClickRef.current = true;
          playOriginalSoundId(ORIGINAL_SOUND_IDS.uiButtonClick);
          onPointerActivate();
        }
      }}
      onMouseUp={() => {
        pointerActivatedRef.current = false;
        setPressed(false);
      }}
      aria-label={label}
      title={label}
    >
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
  const dictionary: Record<Mir2Language, Record<typeof key, string>> = {
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
