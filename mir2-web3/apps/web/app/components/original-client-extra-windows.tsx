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
import { OptionsWindow, type OptionsWindowProps } from "./original-client-options-window";
import { HeroPetWindow, type CreatureSummary, type HeroSummary, type HeroPetWindowProps } from "./original-client-hero-pet-window";
import { HeroManagementWindow, type HeroManagementWindowProps } from "./original-client-hero-management-window";
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
import { OriginalInspectWindow, type OriginalInspectWindowProps } from "./original-client-inspect-window";
import type { RankingPlayerInspect } from "../../lib/shared-ranking-inspect";
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
  OriginalInspectWindowProps,
  RankingPlayerInspect,
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
  OriginalInspectWindow,
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
      | "canAcceptQuest"
      | "canFinishQuest"
      | "isQuestActionPending"
      | "questClientStatus"
      | "onRetryQuestClient"
      | "playerClass"
      | "playerLevel"
    >;

  heroManagement?: WindowToggle & Omit<HeroManagementWindowProps, "t" | "onClose">;
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
      | "creatureSource"
      | "petActionPending"
      | "onPetAction"
      | "onOpenHeroManagement"
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
      | "inventoryItems"
      | "emptyInventorySlots"
      | "itemsReady"
      | "itemActionPending"
      | "itemSourceKey"
      | "onDepositItem"
      | "onRetrieveItem"
      | "onMoveItem"
      | "onRefreshStorage"
      | "canStoreItem"
      | "canRetrieveItem"
      | "incomingInvite"
      | "onReplyInvite"
      | "buffSource"
      | "buffPending"
      | "onBuffAction"
      | "onReadStorageItemTooltip"
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
      | "incomingInvite"
      | "onReplyInvite"
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
      | "onRefresh"
    >;

  bonds?: WindowToggle &
    Pick<
      BondsWindowProps,
      | "relationship"
      | "mentor"
      | "onAllowMarriage"
      | "onProposeMarriage"
      | "onDivorce"
      | "onMailPartner"
      | "onWhisperPartner"
      | "onAllowMentor"
      | "onAddMentor"
      | "onCancelMentor"
      | "incomingRequests"
      | "onReplyRequest"
    >;

  ranking?: WindowToggle &
    Pick<
      RankingWindowProps,
      "activeTab" | "page" | "playerName" | "onSelectTab" | "onRefresh" | "onToggleOnlineOnly"
      | "requestPending" | "onlineOnly" | "onPrevious" | "onNext"
      | "onInspect"
    >;

  inspect?: WindowToggle & { info: RankingPlayerInspect | null }
    & Pick<OriginalInspectWindowProps, "onReadItemTooltip">;

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
      | "composeState"
      | "onDraftChange"
      | "normalizeMessage"
      | "parcelState"
      | "parcelItems"
      | "onParcelAction"
      | "presentation"
      | "onPresentationChange"
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
      | "myItems"
      | "onReadItemTooltip"
      | "inventoryItems"
      | "emptyInventorySlots"
      | "itemsReady"
      | "itemActionPending"
      | "itemSourceKey"
      | "onDepositItem"
      | "onRetrieveItem"
      | "availableGold"
    >;

  buffs?: WindowToggle & Pick<BuffWindowProps, "buffs" | "ticksPerSecond" | "onRemoveBuff">;

  worldMap?: WindowToggle &
    Pick<WorldMapWindowProps, "currentMap" | "playerPosition" | "markers" | "onTeleport">;

  help?: WindowToggle & Omit<HelpWindowProps, "t" | "onClose">;
  options?: WindowToggle & Omit<OptionsWindowProps, "t" | "onClose">;

  hotkeys?: WindowToggle & Omit<HotkeyWindowProps, "t" | "onClose">;

  chatSettings?: WindowToggle & Pick<ChatSettingsWindowProps, "settings" | "onApply">;
};

function ExtraWindowsInner({
  t,
  questLog,
  heroPet,
  heroManagement,
  guild,
  group,
  friends,
  bonds,
  ranking,
  inspect,
  market,
  mail,
  conquest,
  trade,
  buffs,
  worldMap,
  help,
  options,
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
          playerLevel={questLog.playerLevel}
          onTrackQuest={questLog.onTrackQuest}
          onAbandonQuest={questLog.onAbandonQuest}
          onShareQuest={questLog.onShareQuest}
          onAcceptQuest={questLog.onAcceptQuest}
          onFinishQuest={questLog.onFinishQuest}
          canAcceptQuest={questLog.canAcceptQuest}
          canFinishQuest={questLog.canFinishQuest}
          isQuestActionPending={questLog.isQuestActionPending}
          questClientStatus={questLog.questClientStatus}
          onRetryQuestClient={questLog.onRetryQuestClient}
          onClose={questLog.onClose}
        />
      ) : null}

      {heroManagement?.open ? <HeroManagementWindow {...heroManagement} t={t} /> : null}
      {heroPet?.open ? (
        <HeroPetWindow
          t={t}
          hero={heroPet.hero}
          creatures={heroPet.creatures}
          creatureSource={heroPet.creatureSource}
          petActionPending={heroPet.petActionPending}
          onPetAction={heroPet.onPetAction}
          onOpenHeroManagement={heroPet.onOpenHeroManagement}
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
          buffSource={guild.buffSource}
          buffPending={guild.buffPending}
          onBuffAction={guild.onBuffAction}
          onReadStorageItemTooltip={guild.onReadStorageItemTooltip}
          incomingInvite={guild.incomingInvite}
          onReplyInvite={guild.onReplyInvite}
          playerName={guild.playerName}
          onEditNotice={guild.onEditNotice}
          onInviteMember={guild.onInviteMember}
          onKickMember={guild.onKickMember}
          onSendGuildChat={guild.onSendGuildChat}
          onChangeMemberRank={guild.onChangeMemberRank}
          onSaveRank={guild.onSaveRank}
          onDepositGold={guild.onDepositGold}
          onWithdrawGold={guild.onWithdrawGold}
          inventoryItems={guild.inventoryItems}
          emptyInventorySlots={guild.emptyInventorySlots}
          itemsReady={guild.itemsReady}
          itemActionPending={guild.itemActionPending}
          itemSourceKey={guild.itemSourceKey}
          onDepositItem={guild.onDepositItem}
          onRetrieveItem={guild.onRetrieveItem}
          onMoveItem={guild.onMoveItem}
          onRefreshStorage={guild.onRefreshStorage}
          canStoreItem={guild.canStoreItem}
          canRetrieveItem={guild.canRetrieveItem}
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
          incomingInvite={group.incomingInvite}
          onReplyInvite={group.onReplyInvite}
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
          onRefresh={friends.onRefresh}
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
          onMailPartner={bonds.onMailPartner}
          onWhisperPartner={bonds.onWhisperPartner}
          onAllowMentor={bonds.onAllowMentor}
          onAddMentor={bonds.onAddMentor}
          onCancelMentor={bonds.onCancelMentor}
          incomingRequests={bonds.incomingRequests}
          onReplyRequest={bonds.onReplyRequest}
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
          requestPending={ranking.requestPending}
          onlineOnly={ranking.onlineOnly}
          onPrevious={ranking.onPrevious}
          onNext={ranking.onNext}
          onInspect={ranking.onInspect}
          onClose={ranking.onClose}
        />
      ) : null}

      {inspect?.open && inspect.info ? <OriginalInspectWindow t={t} info={inspect.info}
        onClose={inspect.onClose} onReadItemTooltip={inspect.onReadItemTooltip} /> : null}

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
          key={mail.presentation?.key}
          t={t}
          mail={mail.mail}
          gold={mail.gold}
          attachableItems={mail.attachableItems}
          onOpen={mail.onOpen}
          onClaimAttachment={mail.onClaimAttachment}
          onDeleteMail={mail.onDeleteMail}
          onSendMail={mail.onSendMail}
          composeState={mail.composeState}
          onDraftChange={mail.onDraftChange}
          normalizeMessage={mail.normalizeMessage}
          parcelState={mail.parcelState}
          parcelItems={mail.parcelItems}
          onParcelAction={mail.onParcelAction}
          onClose={mail.onClose}
          presentation={mail.presentation}
          onPresentationChange={mail.onPresentationChange}
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
          myItems={trade.myItems}
          onReadItemTooltip={trade.onReadItemTooltip}
          inventoryItems={trade.inventoryItems}
          emptyInventorySlots={trade.emptyInventorySlots}
          itemsReady={trade.itemsReady}
          itemActionPending={trade.itemActionPending}
          itemSourceKey={trade.itemSourceKey}
          onDepositItem={trade.onDepositItem}
          onRetrieveItem={trade.onRetrieveItem}
          availableGold={trade.availableGold}
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

      {help?.open ? <HelpWindow {...help} t={t} /> : null}
      {options?.open ? <OptionsWindow {...options} t={t} /> : null}

      {hotkeys?.open ? (
        <HotkeyWindow {...hotkeys} t={t} />
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
