"use client";

import { useState } from "react";

import {
  ORIGINAL_UI,
  type CharacterTabKey,
  type InventoryTabKey,
  type SpriteState,
} from "../../lib/original-ui";
import { SELECT_PORTRAIT_ANCHOR, type SelectPortraitFrame } from "../../lib/select-portraits";
import {
  languageNativeName,
  SUPPORTED_LANGUAGES,
  type Mir2Language,
} from "../../lib/localization";

type TranslateFn = (
  key: string,
  params?: Array<string | number>,
  fallback?: string,
) => string;

type EntityClassKey = "warrior" | "wizard" | "taoist" | "assassin" | "archer";

type SelectCharacterEntryLike = {
  index: number;
  name: string;
  level: number;
  classKey: EntityClassKey;
  gender: "male" | "female";
  lastAccess?: string | null;
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
  playerExperience?: number;
  playerMaxExperience?: number;
  activeBuffs: Array<{ name: string; remainingTicks: number }>;
  gold: number;
  freeBagSlots: number;
  maxBagSlots: number;
  currentWeight: number;
};

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
  onLanguageChange: (language: Mir2Language) => void;
  onAccountIdChange: (value: string) => void;
  onPasswordChange: (value: string) => void;
  onCreateAccount: () => void;
  onSubmitLogin: () => void;
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
  onLanguageChange,
  onAccountIdChange,
  onPasswordChange,
  onCreateAccount,
  onSubmitLogin,
  onQuickEnter,
  onResetClient,
}: LoginOverlayProps) {
  const loginNotice = loginError ?? (loginBusy ? t("ui.loggingIn") : null);
  const [showAccountPanel, setShowAccountPanel] = useState(false);

  return (
    <section className="login-overlay">
      <LanguageSelector
        language={language}
        t={t}
        compact
        className="login-language-selector"
        onLanguageChange={onLanguageChange}
      />
      <div className="login-dialog">
        <img className="login-panel" src={ORIGINAL_UI.login.dialog} alt="" draggable={false} />
        <img className="login-title" src={ORIGINAL_UI.login.title} alt="" draggable={false} />
        <img className="login-label account" src={ORIGINAL_UI.login.accountLabel} alt="" draggable={false} />
        <img className="login-label password" src={ORIGINAL_UI.login.passwordLabel} alt="" draggable={false} />
        <input
          className="login-input account"
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
      </div>
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
  selectedPortraitFrame: SelectPortraitFrame | null;
  onLanguageChange: (language: Mir2Language) => void;
  onSelectCharacter: (index: number) => void;
  onEnterWorld: () => void;
  onCreateCharacter: () => void;
  onDeleteCharacter: () => void;
  onExit: () => void;
};

export function SelectOverlay({
  language,
  t,
  characters,
  selectedCharacterIndex,
  accountId,
  selectedPortraitFrame,
  onLanguageChange,
  onSelectCharacter,
  onEnterWorld,
  onCreateCharacter,
  onDeleteCharacter,
  onExit,
}: SelectOverlayProps) {
  const selected = characters[selectedCharacterIndex] ?? null;
  const [showCreditsPanel, setShowCreditsPanel] = useState(false);
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);

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
        <img className="select-background-frame" src={ORIGINAL_UI.select.background} alt="" draggable={false} />
        <img className="select-title" src={ORIGINAL_UI.select.title} alt="" draggable={false} />
        <div className="select-server-name">{t("client.GameName", [], "Legend of Mir 2")}</div>

        <div
          className="select-portrait-anchor"
          style={{ left: SELECT_PORTRAIT_ANCHOR.x, top: SELECT_PORTRAIT_ANCHOR.y }}
        >
          {selectedPortraitFrame ? (
            <img
              className="select-portrait-frame"
              src={selectedPortraitFrame.path}
              alt=""
              draggable={false}
              style={{ left: selectedPortraitFrame.x, top: selectedPortraitFrame.y }}
            />
          ) : null}
        </div>

        <div className="select-last-access-label">{t("client.LastOnlineTitle", [], "Last Online:")}</div>
        <div className="select-last-access-value">{selected?.lastAccess ?? t("client.Never", [], "Never")}</div>

        {Array.from({ length: 4 }, (_, slotIndex) => {
          const character = characters[slotIndex] ?? null;
          return (
            <button
              key={`select-slot-${slotIndex}`}
              type="button"
              className={`select-character-slot-card row-${slotIndex + 1} ${character ? "" : "empty"} ${character && slotIndex === selectedCharacterIndex ? "selected" : ""}`}
              disabled={!character}
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

        <div className="select-action start"><SpriteButton sprite={ORIGINAL_UI.select.buttons.start} label={t("ui.startGame")} onClick={onEnterWorld} /></div>
        <div className="select-action new"><SpriteButton sprite={ORIGINAL_UI.select.buttons.newCharacter} label={t("ui.newCharacter")} onClick={onCreateCharacter} /></div>
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
  const manaRatio = ratio(world.playerMp, Math.max(world.playerMp ?? 0, 100));
  const experienceRatio = ratio(world.playerExperience, world.playerMaxExperience);
  const currentHp = world.playerHp ?? 0;
  const maxHp = world.playerMaxHp ?? 0;
  const currentMp = world.playerMp ?? 0;
  const maxMp = 100;
  const hpOnlyOrb = (player?.classKey ?? "warrior") === "warrior" && (player?.level ?? 1) < 26;
  const locationLabel = mapTitle ?? world.mapTitle ?? "";
  const buffLabel = world.activeBuffs
    .slice(0, 2)
    .map((buff) => `${buff.name}:${buff.remainingTicks}`)
    .join("  ");

  return (
    <div className="main-hud-shell">
      <div className="main-hud">
        <img className="hud-cap left" src={ORIGINAL_UI.hud.leftCap} alt="" draggable={false} />
        <img className="hud-base" src={ORIGINAL_UI.hud.base} alt="" draggable={false} />
        <img className="hud-cap right" src={ORIGINAL_UI.hud.rightCap} alt="" draggable={false} />
        <img className="hud-exp-bar" src={ORIGINAL_UI.hud.experienceBar} alt="" draggable={false} />
        <img className="hud-weight-bar" src={ORIGINAL_UI.hud.weightBar} alt="" draggable={false} />

        <div className={`hud-orb-fill hp ${hpOnlyOrb ? "hp-only" : ""}`} style={{ height: `${80 * healthRatio}px` }}>
          <img src={hpOnlyOrb ? ORIGINAL_UI.hud.healthOnlyOrb : ORIGINAL_UI.hud.healthManaOrb} alt="" draggable={false} />
        </div>
        <div className={`hud-orb-fill mp ${hpOnlyOrb ? "hidden" : ""}`} style={{ height: `${80 * manaRatio}px` }}>
          <img src={ORIGINAL_UI.hud.healthManaOrb} alt="" draggable={false} />
        </div>

        {hpOnlyOrb ? (
          <div className="hud-health-only-label">{`HP ${currentHp}/${maxHp}`}</div>
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
        <div className="hud-exp-label">{`${experienceRatio.toFixed(2).replace(/^0/, "") === ".00" ? "0.00" : (experienceRatio * 100).toFixed(2)}%`}</div>
        <div className="hud-gold-label">{connected ? `${world.gold}` : "0"}</div>
        <div className="hud-weight-label">{`${world.freeBagSlots}/${world.maxBagSlots}`}</div>
        <div className="hud-space-label">{`${world.currentWeight}`}</div>

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
  active?: boolean;
};

export function SpriteButton({ sprite, label, onClick, active = false }: SpriteButtonProps) {
  const [hovered, setHovered] = useState(false);
  const [pressed, setPressed] = useState(false);

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
      onClick={onClick}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => {
        setHovered(false);
        setPressed(false);
      }}
      onMouseDown={() => setPressed(true)}
      onMouseUp={() => setPressed(false)}
      aria-label={label}
      title={label}
    >
      <img src={source} alt="" draggable={false} />
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
  const card = ORIGINAL_UI.select.classCards[character.classKey];
  return selected ? card.active : card.base;
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
