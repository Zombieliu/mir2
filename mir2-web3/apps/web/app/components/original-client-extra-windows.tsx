"use client";

/**
 * Registry for the standalone Crystal UI windows added in this module.
 *
 * Integration contract: a host (e.g. the game UI scene / page) needs only a
 * single import plus a single `<ExtraWindows .../>` mount. Each window is
 * independently gated by its own `open` flag, so the host can wire them to
 * whatever menu / hotkey state it already owns. Every callback is optional;
 * windows degrade gracefully (action buttons disable themselves) when a
 * handler is not supplied.
 *
 * The loosely-typed page state (`Record<string, unknown>` slices for `hero`,
 * `intelligentCreatures`, `relationship`, `mentor`, `auction`, `conquest`, …)
 * should be converted into the strict prop shapes below via the adapters in
 * `lib/stage5-window-adapters.ts`, which this module also re-exports.
 */

import { memo, useEffect, useState } from "react";
import { createPortal } from "react-dom";

import { BondsWindow, type BondsWindowProps, type MentorSummary, type RelationshipSummary } from "./original-client-bonds-window";
import { BuffWindow, type BuffEntry, type BuffWindowProps } from "./original-client-buff-window";
import {
  ChatSettingsWindow,
  type ChatChannelKey,
  type ChatFontSize,
  type ChatSettings,
  type ChatSettingsWindowProps,
} from "./original-client-chat-settings-window";
import { ConquestWindow, type ConquestSummary, type ConquestWindowProps, type GuildTerritorySummary } from "./original-client-conquest-window";
import { FriendsWindow, type FriendEntry, type FriendsSummary, type FriendsWindowProps } from "./original-client-friends-window";
import { GroupWindow, type GroupMember, type GroupSummary, type GroupWindowProps } from "./original-client-group-window";
import { GuildWindow, type GuildSummary, type GuildWindowProps } from "./original-client-guild-window";
import { HelpWindow, type HelpWindowProps } from "./original-client-help-window";
import { HeroPetWindow, type CreatureSummary, type HeroSummary, type HeroPetWindowProps } from "./original-client-hero-pet-window";
import {
  DEFAULT_HOTKEY_GROUPS,
  HotkeyWindow,
  type HotkeyBinding,
  type HotkeyGroup,
  type HotkeyWindowProps,
} from "./original-client-hotkey-window";
import {
  MailWindow,
  type MailComposeDraft,
  type MailMessageSummary,
  type MailWindowProps,
} from "./original-client-mail-window";
import { MarketWindow, type MarketListing, type MarketWindowProps } from "./original-client-market-window";
import { QuestLogWindow, type QuestLogEntry, type QuestLogWindowProps } from "./original-client-quest-log-window";
import { RankingWindow, type RankingEntry, type RankingPage, type RankingTabKey, type RankingWindowProps } from "./original-client-ranking-window";
import { TradeWindow, type TradeSummary, type TradeWindowProps } from "./original-client-trade-window";
import {
  WorldMapWindow,
  type WorldMapMarker,
  type WorldMapWindowProps,
} from "./original-client-world-map-window";

export type {
  BondsWindowProps,
  BuffEntry,
  BuffWindowProps,
  ChatChannelKey,
  ChatFontSize,
  ChatSettings,
  ChatSettingsWindowProps,
  ConquestSummary,
  ConquestWindowProps,
  CreatureSummary,
  FriendEntry,
  FriendsSummary,
  FriendsWindowProps,
  GroupMember,
  GroupSummary,
  GroupWindowProps,
  GuildSummary,
  GuildTerritorySummary,
  HelpWindowProps,
  HeroSummary,
  HotkeyBinding,
  HotkeyGroup,
  HotkeyWindowProps,
  MailComposeDraft,
  MailMessageSummary,
  MailWindowProps,
  MarketListing,
  MarketWindowProps,
  MentorSummary,
  QuestLogEntry,
  RankingEntry,
  RankingPage,
  RankingTabKey,
  RankingWindowProps,
  RelationshipSummary,
  TradeSummary,
  TradeWindowProps,
  WorldMapMarker,
  WorldMapWindowProps,
};
export {
  BondsWindow,
  BuffWindow,
  ChatSettingsWindow,
  ConquestWindow,
  DEFAULT_HOTKEY_GROUPS,
  FriendsWindow,
  GroupWindow,
  GuildWindow,
  HelpWindow,
  HeroPetWindow,
  HotkeyWindow,
  MailWindow,
  MarketWindow,
  QuestLogWindow,
  RankingWindow,
  TradeWindow,
  WorldMapWindow,
};

// Re-export the typed adapters so a host can import everything from one module.
export {
  adaptBuffs,
  adaptConquest,
  adaptCreatures,
  adaptFriends,
  adaptGroup,
  adaptGuildTerritory,
  adaptHero,
  adaptMarketListings,
  adaptMentor,
  adaptRankingPage,
  adaptActiveRankingPage,
  adaptRelationship,
  adaptTrade,
  adaptWorldMapMarkers,
  classKeyFromUnknown,
  rankingPageKeyForTab,
  rankingTabKey,
  type Stage5SystemsLike,
} from "../../lib/stage5-window-adapters";

type TranslateFn = (
  key: string,
  params?: Array<string | number>,
  fallback?: string,
) => string;

type WindowToggle = {
  open: boolean;
  onClose: () => void;
};

export type ExtraWindowsProps = {
  /** Shared localization/format helper, reused from the host. */
  t: TranslateFn;

  questLog?: WindowToggle &
    Pick<
      QuestLogWindowProps,
      | "quests"
      | "onTrackQuest"
      | "onAbandonQuest"
      | "onShareQuest"
      | "onAcceptQuest"
      | "onFinishQuest"
      | "playerClass"
    >;

  heroPet?: WindowToggle &
    Pick<
      HeroPetWindowProps,
      | "hero"
      | "creatures"
      | "onSummonHero"
      | "onDismissHero"
      | "onSummonCreature"
      | "onReleaseCreature"
      | "onCyclePickupMode"
      | "onSetHeroBehaviour"
      | "onRecallHero"
    >;

  guild?: WindowToggle &
    Pick<
      GuildWindowProps,
      | "guild"
      | "playerName"
      | "onEditNotice"
      | "onInviteMember"
      | "onKickMember"
      | "onSendGuildChat"
      | "onChangeMemberRank"
      | "onSaveRank"
      | "onDepositGold"
      | "onWithdrawGold"
    >;

  group?: WindowToggle &
    Pick<
      GroupWindowProps,
      | "group"
      | "playerName"
      | "onInviteMember"
      | "onKickMember"
      | "onLeaveGroup"
      | "onToggleLootMode"
      | "onToggleAllowInvites"
    >;

  friends?: WindowToggle &
    Pick<
      FriendsWindowProps,
      | "social"
      | "onAddFriend"
      | "onRemoveFriend"
      | "onBlockPlayer"
      | "onUnblockPlayer"
      | "onWhisper"
      | "onMail"
      | "onEditMemo"
    >;

  bonds?: WindowToggle &
    Pick<
      BondsWindowProps,
      | "relationship"
      | "mentor"
      | "onAllowMarriage"
      | "onProposeMarriage"
      | "onDivorce"
      | "onAllowMentor"
      | "onAddMentor"
      | "onCancelMentor"
    >;

  ranking?: WindowToggle &
    Pick<
      RankingWindowProps,
      "activeTab" | "page" | "playerName" | "onSelectTab" | "onRefresh" | "onToggleOnlineOnly"
    >;

  market?: WindowToggle &
    Pick<
      MarketWindowProps,
      | "listings"
      | "gold"
      | "cityCurrencies"
      | "onBuy"
      | "onCancel"
      | "onList"
      | "onSearch"
      | "onRefresh"
      | "onCollect"
    >;

  mail?: WindowToggle &
    Pick<
      MailWindowProps,
      | "mail"
      | "gold"
      | "attachableItems"
      | "onOpen"
      | "onClaimAttachment"
      | "onDeleteMail"
      | "onSendMail"
    >;

  conquest?: WindowToggle &
    Pick<
      ConquestWindowProps,
      "conquest" | "territory" | "guildName" | "onStartWar" | "onToggleGate" | "onSetTaxRate"
    >;

  trade?: WindowToggle &
    Pick<
      TradeWindowProps,
      | "trade"
      | "myGold"
      | "myItemCount"
      | "myConfirmed"
      | "onAccept"
      | "onConfirm"
      | "onCancel"
      | "onSetGold"
    >;

  buffs?: WindowToggle & Pick<BuffWindowProps, "buffs" | "ticksPerSecond" | "onRemoveBuff">;

  worldMap?: WindowToggle &
    Pick<WorldMapWindowProps, "currentMap" | "playerPosition" | "markers" | "onTeleport">;

  /** Static help reference overlay (no server data required). */
  help?: WindowToggle;

  /** Static keyboard-layout overlay; pass `groups` to show rebound keys. */
  hotkeys?: WindowToggle & Pick<HotkeyWindowProps, "groups">;

  chatSettings?: WindowToggle & Pick<ChatSettingsWindowProps, "settings" | "onApply">;
};

function ExtraWindowsInner({
  t,
  questLog,
  heroPet,
  guild,
  group,
  friends,
  bonds,
  ranking,
  market,
  mail,
  conquest,
  trade,
  buffs,
  worldMap,
  help,
  hotkeys,
  chatSettings,
}: ExtraWindowsProps) {
  const [stageRoot, setStageRoot] = useState<HTMLElement | null>(null);

  useEffect(() => {
    // Fast Refresh and shell/screen remounts can replace the stage element
    // without remounting this registry. Re-resolve after every registry render
    // so portals never remain attached to a detached, invisible stage node.
    const nextStageRoot = document.querySelector<HTMLElement>(".client-stage-frame");
    setStageRoot((current) => current === nextStageRoot ? current : nextStageRoot);
  });

  if (!stageRoot?.isConnected) return null;

  return createPortal(
    <>
      {questLog?.open ? (
        <QuestLogWindow
          t={t}
          quests={questLog.quests}
          playerClass={questLog.playerClass}
          onTrackQuest={questLog.onTrackQuest}
          onAbandonQuest={questLog.onAbandonQuest}
          onShareQuest={questLog.onShareQuest}
          onAcceptQuest={questLog.onAcceptQuest}
          onFinishQuest={questLog.onFinishQuest}
          onClose={questLog.onClose}
        />
      ) : null}

      {heroPet?.open ? (
        <HeroPetWindow
          t={t}
          hero={heroPet.hero}
          creatures={heroPet.creatures}
          onSummonHero={heroPet.onSummonHero}
          onDismissHero={heroPet.onDismissHero}
          onSummonCreature={heroPet.onSummonCreature}
          onReleaseCreature={heroPet.onReleaseCreature}
          onCyclePickupMode={heroPet.onCyclePickupMode}
          onSetHeroBehaviour={heroPet.onSetHeroBehaviour}
          onRecallHero={heroPet.onRecallHero}
          onClose={heroPet.onClose}
        />
      ) : null}

      {guild?.open ? (
        <GuildWindow
          t={t}
          guild={guild.guild}
          playerName={guild.playerName}
          onEditNotice={guild.onEditNotice}
          onInviteMember={guild.onInviteMember}
          onKickMember={guild.onKickMember}
          onSendGuildChat={guild.onSendGuildChat}
          onChangeMemberRank={guild.onChangeMemberRank}
          onSaveRank={guild.onSaveRank}
          onDepositGold={guild.onDepositGold}
          onWithdrawGold={guild.onWithdrawGold}
          onClose={guild.onClose}
        />
      ) : null}

      {group?.open ? (
        <GroupWindow
          t={t}
          group={group.group}
          playerName={group.playerName}
          onInviteMember={group.onInviteMember}
          onKickMember={group.onKickMember}
          onLeaveGroup={group.onLeaveGroup}
          onToggleLootMode={group.onToggleLootMode}
          onToggleAllowInvites={group.onToggleAllowInvites}
          onClose={group.onClose}
        />
      ) : null}

      {friends?.open ? (
        <FriendsWindow
          t={t}
          social={friends.social}
          onAddFriend={friends.onAddFriend}
          onRemoveFriend={friends.onRemoveFriend}
          onBlockPlayer={friends.onBlockPlayer}
          onUnblockPlayer={friends.onUnblockPlayer}
          onWhisper={friends.onWhisper}
          onMail={friends.onMail}
          onEditMemo={friends.onEditMemo}
          onClose={friends.onClose}
        />
      ) : null}

      {bonds?.open ? (
        <BondsWindow
          t={t}
          relationship={bonds.relationship}
          mentor={bonds.mentor}
          onAllowMarriage={bonds.onAllowMarriage}
          onProposeMarriage={bonds.onProposeMarriage}
          onDivorce={bonds.onDivorce}
          onAllowMentor={bonds.onAllowMentor}
          onAddMentor={bonds.onAddMentor}
          onCancelMentor={bonds.onCancelMentor}
          onClose={bonds.onClose}
        />
      ) : null}

      {ranking?.open ? (
        <RankingWindow
          t={t}
          activeTab={ranking.activeTab}
          page={ranking.page}
          playerName={ranking.playerName}
          onSelectTab={ranking.onSelectTab}
          onRefresh={ranking.onRefresh}
          onToggleOnlineOnly={ranking.onToggleOnlineOnly}
          onClose={ranking.onClose}
        />
      ) : null}

      {market?.open ? (
        <MarketWindow
          t={t}
          listings={market.listings}
          gold={market.gold}
          cityCurrencies={market.cityCurrencies}
          onBuy={market.onBuy}
          onCancel={market.onCancel}
          onList={market.onList}
          onSearch={market.onSearch}
          onRefresh={market.onRefresh}
          onCollect={market.onCollect}
          onClose={market.onClose}
        />
      ) : null}

      {mail?.open ? (
        <MailWindow
          t={t}
          mail={mail.mail}
          gold={mail.gold}
          attachableItems={mail.attachableItems}
          onOpen={mail.onOpen}
          onClaimAttachment={mail.onClaimAttachment}
          onDeleteMail={mail.onDeleteMail}
          onSendMail={mail.onSendMail}
          onClose={mail.onClose}
        />
      ) : null}

      {conquest?.open ? (
        <ConquestWindow
          t={t}
          conquest={conquest.conquest}
          territory={conquest.territory}
          guildName={conquest.guildName}
          onStartWar={conquest.onStartWar}
          onToggleGate={conquest.onToggleGate}
          onSetTaxRate={conquest.onSetTaxRate}
          onClose={conquest.onClose}
        />
      ) : null}

      {trade?.open ? (
        <TradeWindow
          t={t}
          trade={trade.trade}
          myGold={trade.myGold}
          myItemCount={trade.myItemCount}
          myConfirmed={trade.myConfirmed}
          onAccept={trade.onAccept}
          onConfirm={trade.onConfirm}
          onCancel={trade.onCancel}
          onSetGold={trade.onSetGold}
          onClose={trade.onClose}
        />
      ) : null}

      {buffs?.open ? (
        <BuffWindow
          t={t}
          buffs={buffs.buffs}
          ticksPerSecond={buffs.ticksPerSecond}
          onRemoveBuff={buffs.onRemoveBuff}
          onClose={buffs.onClose}
        />
      ) : null}

      {worldMap?.open ? (
        <WorldMapWindow
          t={t}
          currentMap={worldMap.currentMap}
          playerPosition={worldMap.playerPosition}
          markers={worldMap.markers}
          onTeleport={worldMap.onTeleport}
          onClose={worldMap.onClose}
        />
      ) : null}

      {help?.open ? <HelpWindow t={t} onClose={help.onClose} /> : null}

      {hotkeys?.open ? (
        <HotkeyWindow t={t} groups={hotkeys.groups} onClose={hotkeys.onClose} />
      ) : null}

      {chatSettings?.open ? (
        <ChatSettingsWindow
          t={t}
          settings={chatSettings.settings}
          onApply={chatSettings.onApply}
          onClose={chatSettings.onClose}
        />
      ) : null}
    </>,
    stageRoot,
  );
}

// Memoised so a host flush that doesn't change any window prop bag skips re-rendering the whole
// window registry. With shallow-equal props this is inert-but-safe today (the host still rebuilds
// the per-window prop objects each render); it begins to hold once those prop bags stop changing
// identity (Stage 5 selector migration). Skipping is never a correctness risk: `open` gates every
// window, so a closed registry renders only empty fragments regardless.
export const ExtraWindows = memo(ExtraWindowsInner);
