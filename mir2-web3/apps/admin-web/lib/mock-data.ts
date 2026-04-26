export const metrics = [
  { title: "Online Now", value: "18,742", delta: "+12.4% vs yesterday" },
  { title: "DAU / MAU", value: "241k / 1.92m", delta: "42.8% stickiness" },
  { title: "Revenue Today", value: "$184.6k", delta: "+8.1% event lift" },
  { title: "Risk Queue", value: "426", delta: "+31 high confidence", negative: true }
];

export const hotMaps = [
  { label: "Bichon Province", value: 92 },
  { label: "Ancient Mine 3F", value: 78 },
  { label: "Wooma Temple", value: 66 },
  { label: "Red Moon Valley", value: 48 }
];

export const servers = [
  ["S1 Dragon Gate", "8,420", "34ms", "Healthy"],
  ["S2 Jade Ruins", "6,112", "41ms", "Healthy"],
  ["S3 Ash River", "3,486", "64ms", "Warn"],
  ["EU Mirror", "724", "91ms", "Healthy"]
];

export const players = [
  ["AZ-1048", "MoonBlade", "Warrior", "52", "1,284,000", "VIP 8", "Normal"],
  ["AZ-1182", "JadeFox", "Taoist", "49", "916,420", "VIP 5", "Risk"],
  ["BK-2031", "IronOx", "Wizard", "47", "802,300", "VIP 2", "Muted"],
  ["CY-7720", "RedLotus", "Warrior", "54", "1,441,900", "VIP 9", "Normal"],
  ["DK-4419", "SilentCart", "Taoist", "38", "301,120", "VIP 0", "Banned"]
];

export const economyRows = [
  ["Gold", "+4.8b", "-4.1b", "+7.2%", "Watch"],
  ["Yuanbao", "+812k", "-796k", "+1.1%", "Stable"],
  ["Bound Currency", "+2.2m", "-2.3m", "-0.4%", "Stable"],
  ["Blessed Oil", "18,420", "16,010", "+14.8%", "Inflation"]
];

export const activities = [
  ["Celestial Sign-in", "2026-04-27 08:00", "Sign-in", "Running", "41.2%"],
  ["Red Moon Boss Rush", "2026-04-27 20:00", "Boss", "Draft", "Preview"],
  ["Jade Rebate", "2026-04-28 00:00", "Top-up", "Scheduled", "$92k forecast"],
  ["Sabuk Ranking", "2026-04-30 19:00", "Rank", "Draft", "Needs review"]
];

export const riskRows = [
  ["JadeFox", "Multi-login cluster", "High", "9 devices / 3 regions"],
  ["SilentCart", "Gold farming loop", "Critical", "14h route repetition"],
  ["MineMule07", "Trade graph hub", "High", "126 suspicious edges"],
  ["CloudFish", "Script idle pattern", "Medium", "88% timing similarity"]
];
