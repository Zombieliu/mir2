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
 */

import { GuildWindow, type GuildSummary, type GuildWindowProps } from "./original-client-guild-window";
import { HeroPetWindow, type CreatureSummary, type HeroSummary, type HeroPetWindowProps } from "./original-client-hero-pet-window";
import { QuestLogWindow, type QuestLogEntry, type QuestLogWindowProps } from "./original-client-quest-log-window";

export type { CreatureSummary, GuildSummary, HeroSummary, QuestLogEntry };
export { GuildWindow, HeroPetWindow, QuestLogWindow };

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
    Pick<QuestLogWindowProps, "quests" | "onTrackQuest" | "onAbandonQuest">;

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
    >;
};

export function ExtraWindows({ t, questLog, heroPet, guild }: ExtraWindowsProps) {
  return (
    <>
      {questLog?.open ? (
        <QuestLogWindow
          t={t}
          quests={questLog.quests}
          onTrackQuest={questLog.onTrackQuest}
          onAbandonQuest={questLog.onAbandonQuest}
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
          onClose={guild.onClose}
        />
      ) : null}
    </>
  );
}
