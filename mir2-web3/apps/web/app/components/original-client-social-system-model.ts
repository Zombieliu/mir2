import { SYSTEM_MENU_SOCIAL_PANEL_DEFINITIONS } from "./original-client-social-system-definitions";
import type {
  SocialDisplayWorld,
  SocialRankingState,
  SystemMenuSocialPanel,
  SystemMenuSocialPanelDefinition,
  SystemMenuSocialPanelMetric,
  SystemMenuSocialPanelRow,
  TranslateFn,
} from "./original-client-social-system-types";

export type {
  SocialDisplayWorld,
  SystemMenuSocialPanel,
  SystemMenuSocialPanelDefinition,
  SystemMenuSocialPanelMetric,
  SystemMenuSocialPanelRow,
  SystemMenuSocialPanelTab,
  TranslateFn,
} from "./original-client-social-system-types";

export function resolveSystemMenuShellText(value: string, playerName: string | null) {
  return value.replace(/\{player\}/g, playerName ?? "-");
}

/** Maps a system-menu panel to its localized shell heading. */
export function featureTitleForSocialPanel(t: TranslateFn, panel: SystemMenuSocialPanel) {
  switch (panel) {
    case "ranking":
      return t("ui.ranking", [], "Ranking");
    case "friend":
      return t("ui.friends", [], "Friends");
    case "mentor":
      return t("ui.mentor", [], "Mentor");
    case "relationship":
      return t("ui.relationship", [], "Relationship");
    case "group":
      return t("ui.group", [], "Group");
    case "guild":
      return t("ui.guild", [], "Guild");
    case "trade":
      return t("ui.trade", [], "Trade");
    case "market":
      return t("ui.market", [], "Market");
    case "marriage":
      return t("ui.marriage", [], "Marriage");
    case "hero":
      return t("ui.hero", [], "Hero");
    case "itemRental":
      return t("ui.itemRental", [], "Item Rental");
  }
}

export function systemMenuSocialPanelDefinition(
  panel: SystemMenuSocialPanel,
  playerName: string | null,
  world: SocialDisplayWorld,
): SystemMenuSocialPanelDefinition {
  // This function is a read model only: it normalizes sparse runtime snapshots
  // into stable tabs/rows and never mutates authoritative social state.
  const systems = world.stage5Systems ?? {};
  const player = playerName ?? "{player}";
  const emptyRow = (name: string, meta = "Empty") =>
    systemMenuRow(name, meta, "No live data is currently available for this slot.", [
      { label: "State", value: "None" },
      { label: "Player", value: player },
      { label: "Map", value: world.mapTitle ?? "-" },
    ]);

  switch (panel) {
    case "ranking":
      return rankingPanelDefinition(player, world);
    case "friend": {
      const friends = systems.social?.friends ?? [];
      const blocked = systems.social?.blocked ?? [];
      return {
        subtitle: "Friends, block list, and memos for {player}",
        footer: "Friend list is loaded from the active runtime snapshot.",
        tabs: [
          {
            key: "friends",
            label: "Friends",
            rows: friends.length
              ? friends.map((name) =>
                  systemMenuRow(name, "Friend", "Friend entry synced from runtime social state.", socialMetrics("Online", "Friend")),
                )
              : [emptyRow("No friends")],
            actions: ["Add", "Memo", "Refresh"],
          },
          {
            key: "blocks",
            label: "Block List",
            rows: blocked.length
              ? blocked.map((name) =>
                  systemMenuRow(name, "Blocked", "Blocked entry synced from runtime social state.", socialMetrics("Muted", "Blocked")),
                )
              : [emptyRow("No blocked entries")],
            actions: ["Block", "Unblock", "Refresh"],
          },
        ],
      };
    }
    case "group": {
      const members = systems.group?.members ?? [];
      return {
        subtitle: "Group state for {player}",
        footer: `Loot mode: ${systems.group?.lootMode ?? "none"}`,
        tabs: [
          {
            key: "party",
            label: "Party",
            rows: members.length
              ? members.map((name) =>
                  systemMenuRow(name, "Member", "Active group member from the current session.", [
                    { label: "Loot", value: systems.group?.lootMode ?? "-" },
                    { label: "Count", value: String(members.length) },
                    { label: "Map", value: world.mapTitle ?? "-" },
                  ]),
                )
              : [emptyRow("No party")],
            actions: ["Create", "Loot", "Refresh"],
          },
        ],
      };
    }
    case "guild": {
      const members = systems.guild?.members ?? [];
      const chatLog = systems.guild?.chatLog ?? [];
      return {
        subtitle: `${systems.guild?.name ?? "No guild"} roster for {player}`,
        footer: `Rank: ${systems.guild?.rank ?? "-"}`,
        tabs: [
          {
            key: "members",
            label: "Members",
            rows: members.length
              ? members.map((name) =>
                  systemMenuRow(name, systems.guild?.rank ?? "Member", "Guild roster entry from live session data.", [
                    { label: "Guild", value: systems.guild?.name ?? "-" },
                    { label: "Rank", value: systems.guild?.rank ?? "-" },
                    { label: "Perms", value: String(systems.guild?.permissions?.length ?? 0) },
                  ]),
                )
              : [emptyRow("No guild members")],
            actions: ["Create", "Notice", "Chat"],
          },
          {
            key: "notice",
            label: "Notice",
            rows: chatLog.length
              ? chatLog.slice(-3).map((line) => systemMenuRow(line, "Guild chat", "Latest guild chat line from live traffic.", socialMetrics("Log", "Chat")))
              : [emptyRow("No guild notice")],
            actions: ["Chat", "Refresh"],
          },
        ],
      };
    }
    case "trade": {
      const trade = systems.trade ?? null;
      const partner = stringRecordValue(trade, ["partner", "partnerName", "name"]) ?? "No active trade";
      return {
        subtitle: "Trade exchange state for {player}",
        footer: trade ? "Trade session is open." : "No active trade session.",
        tabs: [
          {
            key: "session",
            label: "Session",
            rows: [
              systemMenuRow(partner, trade ? "Active" : "Idle", "Trade session state for this character.", [
                { label: "Gold", value: stringRecordValue(trade, ["gold", "offeredGold"]) ?? "0" },
                { label: "Accepted", value: stringRecordValue(trade, ["accepted", "confirmed"]) ?? "false" },
                { label: "Partner", value: partner },
              ]),
            ],
            actions: ["Start", "Offer 1g", "Accept", "Cancel"],
          },
        ],
      };
    }
    case "hero": {
      const hero = systems.hero ?? null;
      const heroName = stringRecordValue(hero, ["name"]) ?? "Aide";
      const heroClass = stringRecordValue(hero, ["class"]) ?? "warrior";
      const heroGender = stringRecordValue(hero, ["gender"]) ?? "female";
      const behaviour = stringRecordValue(hero, ["behaviour"]) ?? "0";
      const spawned = stringRecordValue(hero, ["spawned"]) ?? "false";
      return {
        subtitle: "Hero management for {player}",
        footer: hero ? "Hero state is loaded from the current runtime snapshot." : "No active hero is attached.",
        tabs: [
          {
            key: "status",
            label: "Status",
            rows: [
              systemMenuRow(heroName, hero ? "Active hero" : "Create hero", "Current hero slot and behaviour state.", [
                { label: "Class", value: heroClass },
                { label: "Gender", value: heroGender },
                { label: "Behaviour", value: behaviour },
              ]),
              systemMenuRow("Spawn", spawned === "true" ? "Spawned" : "Idle", "Hero spawn state reported by runtime.", [
                { label: "Level", value: stringRecordValue(hero, ["level"]) ?? "1" },
                { label: "Experience", value: stringRecordValue(hero, ["experience"]) ?? "0" },
                { label: "Player", value: player },
              ]),
            ],
            actions: ["Create", "Behaviour Guard", "Change"],
          },
        ],
      };
    }
    case "market": {
      const auctions = systems.auction ?? [];
      return {
        subtitle: "Market listings for {player}",
        footer: `${auctions.length} active listing${auctions.length === 1 ? "" : "s"}`,
        tabs: [
          {
            key: "listings",
            label: "Listings",
            rows: auctions.length
              ? auctions.map((listing, index) =>
                  systemMenuRow(
                    stringRecordValue(listing, ["item", "itemName", "name"]) ?? `Listing ${index + 1}`,
                    stringRecordValue(listing, ["seller", "owner"]) ?? "Market",
                    "Active market listing from live session data.",
                    [
                      { label: "Id", value: stringRecordValue(listing, ["id", "listingId"]) ?? String(index + 1) },
                      { label: "Price", value: stringRecordValue(listing, ["price", "gold"]) ?? "0" },
                      { label: "State", value: stringRecordValue(listing, ["state", "status"]) ?? "Open" },
                    ],
                  ),
                )
              : [emptyRow("No market listings")],
            actions: ["List", "Buy", "Cancel"],
          },
        ],
      };
    }
    case "itemRental": {
      const rental = systems.itemRental ?? {};
      const partner = stringRecordValue(rental, ["partnerName", "partner"]) ?? "Crystal Partner";
      const fee = stringRecordValue(rental, ["fee"]) ?? "100";
      const days = stringRecordValue(rental, ["days"]) ?? "7";
      const goldLocked = stringRecordValue(rental, ["goldLocked"]) ?? "false";
      const itemLocked = stringRecordValue(rental, ["itemLocked"]) ?? "false";
      return {
        subtitle: "Item rental session for {player}",
        footer: `Partner: ${partner}`,
        tabs: [
          {
            key: "session",
            label: "Session",
            rows: [
              systemMenuRow(partner, "Rental partner", "Current rental handshake and lock state.", [
                { label: "Fee", value: fee },
                { label: "Days", value: days },
                { label: "Gold lock", value: goldLocked },
              ]),
              systemMenuRow("Loan item", itemLocked === "true" ? "Locked" : "Open", "Loan item slot state.", [
                { label: "Item lock", value: itemLocked },
                { label: "Records", value: stringRecordValue(rental, ["recordCount"]) ?? "0" },
                { label: "Player", value: player },
              ]),
            ],
            actions: ["Request", "Fee 100", "Period 7", "Cancel", "List"],
          },
        ],
      };
    }
    case "marriage":
    case "relationship": {
      const relationship = systems.relationship ?? {};
      const partnerName = stringRecordValue(relationship, ["partnerName"]) ?? "";
      const allowMarriage = boolRecordValue(relationship, ["allowMarriage"], true);
      const requestFrom = stringRecordValue(relationship, ["pendingRequestFrom", "pendingDivorceFrom"]) ?? "None";
      return {
        subtitle: `${panel === "marriage" ? "Marriage" : "Relationship"} state for {player}`,
        footer: allowMarriage ? "Marriage requests are allowed." : "Marriage requests are blocked.",
        tabs: [
          {
            key: "status",
            label: "Status",
            rows: [
              systemMenuRow(partnerName || player, partnerName ? "Married" : "Single", "Current relationship state.", [
                { label: "Partner", value: partnerName || "-" },
                { label: "Ring", value: "-" },
                { label: "Request", value: requestFrom },
              ]),
            ],
            actions: ["Allow", "Request", "Divorce", "Refresh"],
          },
        ],
      };
    }
    case "mentor": {
      const mentor = systems.mentor ?? {};
      const mentorName = stringRecordValue(mentor, ["name"]) ?? "";
      const allowMentor = boolRecordValue(mentor, ["allowMentor"], true);
      const pending = stringRecordValue(mentor, ["pendingRequestFrom"]) ?? "";
      return {
        subtitle: "Mentor state for {player}",
        footer: allowMentor ? "Mentor requests are allowed." : "Mentor requests are blocked.",
        tabs: [
          {
            key: "requests",
            label: "Requests",
            rows: [
              systemMenuRow(mentorName || pending || "No mentor request", mentorName ? "Mentor" : "Open", "Current mentor state.", [
                { label: "Level", value: stringRecordValue(mentor, ["level"]) ?? "0" },
                { label: "Online", value: stringRecordValue(mentor, ["online"]) ?? "false" },
                { label: "Mentee EXP", value: stringRecordValue(mentor, ["menteeExp"]) ?? "0" },
              ]),
            ],
            actions: ["Allow", "Add", "Cancel", "Refresh"],
          },
        ],
      };
    }
  }

  return SYSTEM_MENU_SOCIAL_PANEL_DEFINITIONS[panel] ?? {
    subtitle: "System state for {player}",
    footer: "No state available.",
    tabs: [{ key: "state", label: "State", rows: [emptyRow("No rows")], actions: ["Refresh"] }],
  };
}

function systemMenuRow(
  name: string,
  meta: string,
  note: string,
  metrics: SystemMenuSocialPanelMetric[],
): SystemMenuSocialPanelRow {
  return { name, meta, note, metrics };
}

function socialMetrics(state: string, note: string): SystemMenuSocialPanelMetric[] {
  return [
    { label: "State", value: state },
    { label: "Note", value: note },
    { label: "Source", value: "Live" },
  ];
}

const RANKING_TABS = [
  { key: "overall", label: "Overall", rankType: 0, onlineOnly: false },
  { key: "warrior", label: "Warrior", rankType: 1, onlineOnly: false },
  { key: "wizard", label: "Wizard", rankType: 2, onlineOnly: false },
  { key: "taoist", label: "Taoist", rankType: 3, onlineOnly: false },
  { key: "assassin", label: "Assassin", rankType: 4, onlineOnly: false },
  { key: "archer", label: "Archer", rankType: 5, onlineOnly: false },
  { key: "online", label: "Online", rankType: 0, onlineOnly: true },
] as const;

export function rankingRequestForSocialTab(tab: string): Record<string, unknown> {
  // Unknown tabs intentionally fall back to Overall so stale UI state still
  // produces a valid Crystal ranking request.
  const definition = RANKING_TABS.find((entry) => entry.key === tab) ?? RANKING_TABS[0];
  return {
    type: "getRanking",
    rankType: definition.rankType,
    rankIndex: 0,
    onlineOnly: definition.onlineOnly,
  };
}

function rankingPanelDefinition(player: string, world: SocialDisplayWorld): SystemMenuSocialPanelDefinition {
  const pages = world.rankings ?? {};
  const current = world.rankingCurrentKey ? pages[world.rankingCurrentKey] : undefined;
  return {
    subtitle: "Crystal ranking for {player}",
    footer: current
      ? `My rank: ${current.myRank > 0 ? current.myRank : "Not listed"} / ${current.count}`
      : "Ranking data pending",
    tabs: RANKING_TABS.map((tab) => {
      const page = pages[rankingKey(tab.rankType, tab.onlineOnly)];
      return {
        key: tab.key,
        label: tab.label,
        rows: rankingRows(page, player),
        actions: ["Refresh"],
      };
    }),
  };
}

function rankingRows(
  page: SocialRankingState | undefined,
  player: string,
): SystemMenuSocialPanelRow[] {
  if (!page) {
    return [
      systemMenuRow("No ranking data", "Pending", "Open this ranking tab to load current rows.", [
        { label: "Player", value: player },
        { label: "Rank", value: "-" },
        { label: "Count", value: "0" },
      ]),
    ];
  }
  if (!page.entries.length) {
    return [
      systemMenuRow("No ranked characters", page.onlineOnly ? "Online" : "Empty", "No characters are listed for this ranking.", [
        { label: "My Rank", value: page.myRank > 0 ? String(page.myRank) : "-" },
        { label: "Count", value: String(page.count) },
        { label: "Rank Type", value: String(page.rankType) },
      ]),
    ];
  }
  return page.entries.map((entry) =>
    systemMenuRow(entry.name, `#${entry.rank}`, `Level ${entry.level} ${rankingClassLabel(entry.classKey)}`, [
      { label: "Rank", value: String(entry.rank) },
      { label: "Level", value: String(entry.level) },
      { label: "Class", value: rankingClassLabel(entry.classKey) },
      { label: "PlayerId", value: String(entry.playerId) },
    ]),
  );
}

function rankingKey(rankType: number, onlineOnly: boolean) {
  return `${rankType}:${onlineOnly ? "online" : "all"}`;
}

function rankingClassLabel(classKey: SocialRankingState["entries"][number]["classKey"]) {
  switch (classKey) {
    case "wizard":
      return "Wizard";
    case "taoist":
      return "Taoist";
    case "assassin":
      return "Assassin";
    case "archer":
      return "Archer";
    case "warrior":
    default:
      return "Warrior";
  }
}

function stringRecordValue(record: Record<string, unknown> | null | undefined, keys: string[]) {
  // Runtime snapshots have evolved across packet and Stage 5 representations;
  // ordered aliases keep the panel compatible without leaking `unknown` values.
  if (!record) return null;
  for (const key of keys) {
    const value = record[key];
    if (typeof value === "string" || typeof value === "number" || typeof value === "boolean") {
      return String(value);
    }
  }
  return null;
}

function numberRecordValue(record: Record<string, unknown> | null | undefined, keys: string[], fallback = 0) {
  const value = stringRecordValue(record, keys);
  if (value === null) return fallback;
  const number = Number(value);
  return Number.isFinite(number) ? number : fallback;
}

function boolRecordValue(record: Record<string, unknown> | null | undefined, keys: string[], fallback = false) {
  if (!record) return fallback;
  for (const key of keys) {
    const value = record[key];
    if (typeof value === "boolean") return value;
  }
  return fallback;
}

function recordObjectValue(record: Record<string, unknown> | null | undefined, key: string) {
  const value = record?.[key];
  return value && typeof value === "object" && !Array.isArray(value) ? (value as Record<string, unknown>) : null;
}

export function clientCommandForSocialAction(
  panel: SystemMenuSocialPanel,
  tab: string,
  action: string,
  rowName: string,
): Record<string, unknown> | null {
  // This system-menu surface intentionally exposes only packets that are fully
  // described by the selected live state. Returning null disables the action:
  // the normal player UI must never fill missing values through a generic
  // Stage5 command, placeholder name, or placeholder auction id.
  const normalized = action.toLowerCase();
  if (panel === "friend" && normalized === "refresh") {
    return { type: "refreshFriends" };
  }
  if (panel === "trade" && rowName !== "No active trade" && normalized === "accept") {
    return { type: "tradeConfirm", locked: true };
  }
  if (panel === "trade" && rowName !== "No active trade" && normalized === "cancel") {
    return { type: "tradeCancel" };
  }
  if (panel === "itemRental" && normalized === "list") {
    return { type: "getRentedItems" };
  }
  if (panel === "mentor" && normalized === "allow") {
    return { type: "allowMentor" };
  }
  if (panel === "mentor" && normalized === "cancel") {
    return { type: "cancelMentor" };
  }
  if ((panel === "relationship" || panel === "marriage") && normalized === "allow") {
    return { type: "changeMarriage" };
  }
  if ((panel === "relationship" || panel === "marriage") && normalized === "divorce") {
    return { type: "divorceRequest" };
  }
  if (panel === "market" && normalized === "refresh") {
    return { type: "marketRefresh" };
  }
  if (panel === "ranking" && normalized === "refresh") {
    return rankingRequestForSocialTab(tab);
  }
  void tab;
  return null;
}
