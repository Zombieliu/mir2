import type { SystemMenuSocialPanel, SystemMenuSocialPanelDefinition } from "./original-client-social-system-types";

export const SYSTEM_MENU_SOCIAL_PANEL_DEFINITIONS: Partial<Record<SystemMenuSocialPanel, SystemMenuSocialPanelDefinition>> = {
  ranking: {
    subtitle: "Crystal leaderboard for {player}",
    footer: "Compare, inspect, and whisper without leaving the menu.",
    tabs: [
      {
        key: "overall",
        label: "Overall",
        rows: [
          {
            name: "{player}",
            meta: "Current slot",
            note: "Safe-zone progress and route status are shown here.",
            metrics: [
              { label: "Rank", value: "01" },
              { label: "Score", value: "1,820" },
              { label: "Map", value: "Bichon" },
            ],
          },
          {
            name: "CrystalKnight",
            meta: "Front line",
            note: "Stable damage pressure and route clarity.",
            metrics: [
              { label: "Rank", value: "02" },
              { label: "Score", value: "1,774" },
              { label: "Map", value: "Border" },
            ],
          },
          {
            name: "MapScout",
            meta: "Traversal",
            note: "Fast map switching and arrival route status.",
            metrics: [
              { label: "Rank", value: "03" },
              { label: "Score", value: "1,709" },
              { label: "Map", value: "Arena" },
            ],
          },
        ],
        actions: ["Compare", "Inspect", "Whisper"],
      },
      {
        key: "class",
        label: "Class",
        rows: [
          {
            name: "Warrior",
            meta: "Melee ladder",
            note: "Heavy armor and steady damage still anchor the list.",
            metrics: [
              { label: "Best", value: "2,008" },
              { label: "Wins", value: "84" },
              { label: "Trend", value: "+12" },
            ],
          },
          {
            name: "Wizard",
            meta: "Burst ladder",
            note: "Magic burst remains the fastest route to the top.",
            metrics: [
              { label: "Best", value: "1,962" },
              { label: "Wins", value: "79" },
              { label: "Trend", value: "+8" },
            ],
          },
          {
            name: "Archer",
            meta: "Ranged ladder",
            note: "Long-range control is shown in the panel preview.",
            metrics: [
              { label: "Best", value: "1,955" },
              { label: "Wins", value: "88" },
              { label: "Trend", value: "+15" },
            ],
          },
        ],
        actions: ["Sort", "Filter", "Whisper"],
      },
      {
        key: "guild",
        label: "Guild",
        rows: [
          {
            name: "Obelisk",
            meta: "Prime guild",
            note: "Guild coordination, roster review, and notice checks.",
            metrics: [
              { label: "Members", value: "42" },
              { label: "Donation", value: "96%" },
              { label: "Status", value: "Open" },
            ],
          },
          {
            name: "Crystal",
            meta: "Support guild",
            note: "Guild searching and comparison details.",
            metrics: [
              { label: "Members", value: "31" },
              { label: "Donation", value: "88%" },
              { label: "Status", value: "Open" },
            ],
          },
          {
            name: "Mir2",
            meta: "Training guild",
            note: "Useful for quick roster and memo verification.",
            metrics: [
              { label: "Members", value: "27" },
              { label: "Donation", value: "74%" },
              { label: "Status", value: "Open" },
            ],
          },
        ],
        actions: ["Notice", "Inspect", "Chat"],
      },
    ],
  },
  friend: {
    subtitle: "Friends, block list, and memos for {player}",
    footer: "Whisper, memo, or inspect the current social list.",
    tabs: [
      {
        key: "friends",
        label: "Friends",
        rows: [
          {
            name: "Assistant_Jane",
            meta: "Online",
            note: "Helpful route checks and map labels are shared here.",
            metrics: [
              { label: "Map", value: "Bichon" },
              { label: "Mood", value: "Ready" },
              { label: "Note", value: "Escort" },
            ],
          },
          {
            name: "Merchant_Ruben",
            meta: "Away",
            note: "Inventory and trade context stay visible in the panel.",
            metrics: [
              { label: "Map", value: "Market" },
              { label: "Mood", value: "Away" },
              { label: "Note", value: "Mail" },
            ],
          },
          {
            name: "{player}",
            meta: "Local hero",
            note: "Your own row stays visible for quick status review.",
            metrics: [
              { label: "Map", value: "Bichon" },
              { label: "Mood", value: "Open" },
              { label: "Note", value: "Self" },
            ],
          },
        ],
        actions: ["Whisper", "Memo", "Inspect"],
      },
      {
        key: "blocks",
        label: "Block List",
        rows: [
          {
            name: "Spam_Filter",
            meta: "Muted",
            note: "Noise filtering is represented as a real row selection.",
            metrics: [
              { label: "Reason", value: "Spam" },
              { label: "Flag", value: "Muted" },
              { label: "Age", value: "12d" },
            ],
          },
          {
            name: "Trade_Auto",
            meta: "Muted",
            note: "Trade moderation list entries are kept here for visibility.",
            metrics: [
              { label: "Reason", value: "Spoof" },
              { label: "Flag", value: "Muted" },
              { label: "Age", value: "4d" },
            ],
          },
          {
            name: "Channel_Noise",
            meta: "Muted",
            note: "Channel list entry showing active moderation state.",
            metrics: [
              { label: "Reason", value: "Noise" },
              { label: "Flag", value: "Muted" },
              { label: "Age", value: "1d" },
            ],
          },
        ],
        actions: ["Unblock", "Memo", "Inspect"],
      },
      {
        key: "memo",
        label: "Memo",
        rows: [
          {
            name: "Bichon Route",
            meta: "Pinned memo",
            note: "The menu keeps saved route notes clickable.",
            metrics: [
              { label: "Tag", value: "Route" },
              { label: "State", value: "Pinned" },
              { label: "Age", value: "Today" },
            ],
          },
          {
            name: "Guild Invite",
            meta: "Pinned memo",
            note: "Stored invite memo with quick follow-up actions.",
            metrics: [
              { label: "Tag", value: "Invite" },
              { label: "State", value: "Pinned" },
              { label: "Age", value: "Today" },
            ],
          },
          {
            name: "Drop Check",
            meta: "Pinned memo",
            note: "Use this memo to keep route updates quick and visible.",
            metrics: [
              { label: "Tag", value: "Loot" },
              { label: "State", value: "Pinned" },
              { label: "Age", value: "Today" },
            ],
          },
        ],
        actions: ["Write", "Pin", "Inspect"],
      },
    ],
  },
  mentor: {
    subtitle: "Mentor and apprentice rollup for {player}",
    footer: "Review training rows or track a mentor request.",
    tabs: [
      {
        key: "mentor",
        label: "Mentor",
        rows: [
          {
            name: "Crystal_Sage",
            meta: "Mentor",
            note: "Guidance, tracks, and training notes stay visible.",
            metrics: [
              { label: "Rank", value: "S" },
              { label: "Focus", value: "Balance" },
              { label: "State", value: "Active" },
            ],
          },
          {
            name: "Field_Guide",
            meta: "Mentor",
            note: "Mentor contact state and availability.",
            metrics: [
              { label: "Rank", value: "A" },
              { label: "Focus", value: "Route" },
              { label: "State", value: "Active" },
            ],
          },
          {
            name: "{player}",
            meta: "Current trainee",
            note: "Your slot can still be clicked like a real mentor row.",
            metrics: [
              { label: "Rank", value: "B" },
              { label: "Focus", value: "Route" },
              { label: "State", value: "Active" },
            ],
          },
        ],
        actions: ["Track", "Teach", "Review"],
      },
      {
        key: "apprentices",
        label: "Apprentices",
        rows: [
          {
            name: "Rising_Hero",
            meta: "Level 24",
            note: "Apprentice roster rows can be selected and compared.",
            metrics: [
              { label: "Progress", value: "63%" },
              { label: "Focus", value: "Combat" },
              { label: "State", value: "Training" },
            ],
          },
          {
            name: "Map_Walker",
            meta: "Level 31",
            note: "Route familiarity and map travel are represented here.",
            metrics: [
              { label: "Progress", value: "71%" },
              { label: "Focus", value: "Travel" },
              { label: "State", value: "Training" },
            ],
          },
          {
            name: "Crystal_Reader",
            meta: "Level 19",
            note: "Level progression row with clear state updates.",
            metrics: [
              { label: "Progress", value: "42%" },
              { label: "Focus", value: "Info" },
              { label: "State", value: "Training" },
            ],
          },
        ],
        actions: ["Track", "Assign", "Review"],
      },
      {
        key: "requests",
        label: "Requests",
        rows: [
          {
            name: "Pending_Bond",
            meta: "Awaiting response",
            note: "Request rows mirror the kind of yes/no state Crystal uses.",
            metrics: [
              { label: "Age", value: "2h" },
              { label: "Type", value: "Mentor" },
              { label: "State", value: "Pending" },
            ],
          },
          {
            name: "Manual_Review",
            meta: "Awaiting response",
            note: "Alternate request row for mentor queue status checks.",
            metrics: [
              { label: "Age", value: "5h" },
              { label: "Type", value: "Train" },
              { label: "State", value: "Pending" },
            ],
          },
          {
            name: "Fallback_Pass",
            meta: "Awaiting response",
            note: "A pending request row for mentor actions.",
            metrics: [
              { label: "Age", value: "1d" },
              { label: "Type", value: "Trace" },
              { label: "State", value: "Pending" },
            ],
          },
        ],
        actions: ["Accept", "Track", "Review"],
      },
    ],
  },
  relationship: {
    subtitle: "Relationship, ring, and affinity for {player}",
    footer: "Review bond rows and inspect relationship status.",
    tabs: [
      {
        key: "lover",
        label: "Lover",
        rows: [
          {
            name: "Promise_Ring",
            meta: "Bonded",
            note: "Love-state rows turn into a real clickable selection.",
            metrics: [
              { label: "Affinity", value: "87%" },
              { label: "Gift", value: "Ready" },
              { label: "State", value: "Bonded" },
            ],
          },
          {
            name: "Shared_Route",
            meta: "Bonded",
            note: "Shared relationship status and route history.",
            metrics: [
              { label: "Affinity", value: "81%" },
              { label: "Gift", value: "Ready" },
              { label: "State", value: "Bonded" },
            ],
          },
          {
            name: "{player}",
            meta: "Bonded",
            note: "The player row can still be selected like a normal entry.",
            metrics: [
              { label: "Affinity", value: "90%" },
              { label: "Gift", value: "Ready" },
              { label: "State", value: "Bonded" },
            ],
          },
        ],
        actions: ["Gift", "Bond", "Inspect"],
      },
      {
        key: "affinity",
        label: "Affinity",
        rows: [
          {
            name: "Affinity_87",
            meta: "Gauge",
            note: "A progress-style row for the affinity tab.",
            metrics: [
              { label: "Level", value: "87" },
              { label: "Timer", value: "Ready" },
              { label: "State", value: "Warm" },
            ],
          },
          {
            name: "Gift_Cooldown",
            meta: "Gauge",
            note: "Gift cooldown meter with instant status updates.",
            metrics: [
              { label: "Level", value: "12" },
              { label: "Timer", value: "Ready" },
              { label: "State", value: "Warm" },
            ],
          },
          {
            name: "Ring_Bond",
            meta: "Gauge",
            note: "Another row to make the tab feel like a true management panel.",
            metrics: [
              { label: "Level", value: "44" },
              { label: "Timer", value: "Ready" },
              { label: "State", value: "Warm" },
            ],
          },
        ],
        actions: ["Trace", "Gift", "Inspect"],
      },
      {
        key: "history",
        label: "History",
        rows: [
          {
            name: "First_Meeting",
            meta: "Log",
            note: "History rows give the panel enough weight to feel present.",
            metrics: [
              { label: "Date", value: "Day 1" },
              { label: "Map", value: "Bichon" },
              { label: "State", value: "Saved" },
            ],
          },
          {
            name: "Last_Gift",
            meta: "Log",
            note: "Another static row makes selection changes easy to spot.",
            metrics: [
              { label: "Date", value: "Today" },
              { label: "Map", value: "Market" },
              { label: "State", value: "Saved" },
            ],
          },
          {
            name: "Travel_Log",
            meta: "Log",
            note: "Good for screenshoting a different selected row state.",
            metrics: [
              { label: "Date", value: "This week" },
              { label: "Map", value: "Arena" },
              { label: "State", value: "Saved" },
            ],
          },
        ],
        actions: ["Record", "Bond", "Inspect"],
      },
    ],
  },
  group: {
    subtitle: "Party roster and recruitment for {player}",
    footer: "Invite, assist, or inspect the group state.",
    tabs: [
      {
        key: "party",
        label: "Party",
        rows: [
          {
            name: "{player}",
            meta: "Leader",
            note: "The local player stays visible as a normal selected row.",
            metrics: [
              { label: "Role", value: "Leader" },
              { label: "Range", value: "Near" },
              { label: "State", value: "Ready" },
            ],
          },
          {
            name: "Field_Cleric",
            meta: "Support",
            note: "Support row with clear role visibility and status.",
            metrics: [
              { label: "Role", value: "Heal" },
              { label: "Range", value: "Mid" },
              { label: "State", value: "Ready" },
            ],
          },
          {
            name: "Frontline",
            meta: "Tank",
            note: "Front-line rows help the panel read like a real party list.",
            metrics: [
              { label: "Role", value: "Tank" },
              { label: "Range", value: "Front" },
              { label: "State", value: "Ready" },
            ],
          },
        ],
        actions: ["Invite", "Assist", "Share"],
      },
      {
        key: "recruit",
        label: "Recruit",
        rows: [
          {
            name: "Need_Tank",
            meta: "Recruit",
            note: "Recruit rows show available party slots.",
            metrics: [
              { label: "Role", value: "Tank" },
              { label: "Slots", value: "1" },
              { label: "State", value: "Open" },
            ],
          },
          {
            name: "Need_DPS",
            meta: "Recruit",
            note: "Another row keeps click selection obvious.",
            metrics: [
              { label: "Role", value: "DPS" },
              { label: "Slots", value: "2" },
              { label: "State", value: "Open" },
            ],
          },
          {
            name: "Need_Support",
            meta: "Recruit",
            note: "Recruiting state for the party list.",
            metrics: [
              { label: "Role", value: "Support" },
              { label: "Slots", value: "1" },
              { label: "State", value: "Open" },
            ],
          },
        ],
        actions: ["Invite", "Inspect", "Chat"],
      },
      {
        key: "loot",
        label: "Loot",
        rows: [
          {
            name: "Shard_Split",
            meta: "Share",
            note: "Group loot is ready to share.",
            metrics: [
              { label: "Split", value: "Equal" },
              { label: "Need", value: "Open" },
              { label: "State", value: "Ready" },
            ],
          },
          {
            name: "Gold_Share",
            meta: "Share",
            note: "A different row makes the click target unmistakable.",
            metrics: [
              { label: "Split", value: "Gold" },
              { label: "Need", value: "Open" },
              { label: "State", value: "Ready" },
            ],
          },
          {
            name: "Quest_Drop",
            meta: "Share",
            note: "Quest split row for quick loot visibility and handoff.",
            metrics: [
              { label: "Split", value: "Quest" },
              { label: "Need", value: "Open" },
              { label: "State", value: "Ready" },
            ],
          },
        ],
        actions: ["Share", "Inspect", "Chat"],
      },
    ],
  },
  guild: {
    subtitle: "Guild hall and member queue for {player}",
    footer: "Notice, roster, and member management are available.",
    tabs: [
      {
        key: "overview",
        label: "Overview",
        rows: [
          {
            name: "Obelisk",
            meta: "Guild hall",
            note: "Main guild summary with a real selectable row.",
            metrics: [
              { label: "Members", value: "42" },
              { label: "Rank", value: "A" },
              { label: "State", value: "Open" },
            ],
          },
          {
            name: "Banner_Room",
            meta: "Guild hall",
            note: "A second overview row for clearer panel screenshots.",
            metrics: [
              { label: "Members", value: "42" },
              { label: "Rank", value: "A" },
              { label: "State", value: "Open" },
            ],
          },
          {
            name: "Crystal_Notice",
            meta: "Guild hall",
            note: "Guild notice row for current hall updates.",
            metrics: [
              { label: "Members", value: "42" },
              { label: "Rank", value: "A" },
              { label: "State", value: "Open" },
            ],
          },
        ],
        actions: ["Notice", "Inspect", "Chat"],
      },
      {
        key: "members",
        label: "Members",
        rows: [
          {
            name: "Guild_Master",
            meta: "Leader",
            note: "Leader entries show current roster visibility and state.",
            metrics: [
              { label: "Role", value: "Leader" },
              { label: "Duty", value: "Manage" },
              { label: "State", value: "Online" },
            ],
          },
          {
            name: "Deputy",
            meta: "Officer",
            note: "Officer row for member management.",
            metrics: [
              { label: "Role", value: "Officer" },
              { label: "Duty", value: "Manage" },
              { label: "State", value: "Online" },
            ],
          },
          {
            name: "Recruit",
            meta: "Member",
            note: "Member row for roster review.",
            metrics: [
              { label: "Role", value: "Member" },
              { label: "Duty", value: "Train" },
              { label: "State", value: "Online" },
            ],
          },
        ],
        actions: ["Promote", "Inspect", "Chat"],
      },
      {
        key: "notice",
        label: "Notice",
        rows: [
          {
            name: "Raid_Night",
            meta: "Pinned",
            note: "Guild notice row for current hall updates.",
            metrics: [
              { label: "Time", value: "20:00" },
              { label: "Type", value: "Raid" },
              { label: "State", value: "Pinned" },
            ],
          },
          {
            name: "Bank_Lock",
            meta: "Pinned",
            note: "Lock rows are static but still clickable and selectable.",
            metrics: [
              { label: "Time", value: "Now" },
              { label: "Type", value: "Bank" },
              { label: "State", value: "Pinned" },
            ],
          },
          {
            name: "Contribution",
            meta: "Pinned",
            note: "Useful for demonstrating row state and action updates.",
            metrics: [
              { label: "Time", value: "Today" },
              { label: "Type", value: "Donate" },
              { label: "State", value: "Pinned" },
            ],
          },
        ],
        actions: ["Notice", "Inspect", "Chat"],
      },
    ],
  },
};
