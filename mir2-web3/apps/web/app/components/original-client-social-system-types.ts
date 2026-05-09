export type TranslateFn = (
  key: string,
  params?: Array<string | number>,
  fallback?: string,
) => string;

export type SocialDisplayWorld = {
  mapTitle: string | null;
  inSafeZone: boolean;
  gold: number;
  entities: Array<{ kind: string; level?: number }>;
  activeBuffs: Array<{ name: string; remainingTicks: number }>;
  stage5Systems?: {
    group?: { members?: string[]; lootMode?: string };
    guild?: { name?: string; members?: string[]; rank?: string; permissions?: string[]; chatLog?: string[] };
    social?: { friends?: string[]; blocked?: string[] };
    relationship?: Record<string, unknown>;
    mentor?: Record<string, unknown>;
    trade?: Record<string, unknown> | null;
    auction?: Array<Record<string, unknown>>;
    hero?: Record<string, unknown> | null;
    itemRental?: Record<string, unknown>;
  };
};

export type SystemMenuSocialPanel =
  | "ranking"
  | "friend"
  | "mentor"
  | "relationship"
  | "group"
  | "guild"
  | "trade"
  | "market"
  | "marriage"
  | "hero"
  | "itemRental";

export type SystemMenuSocialPanelMetric = {
  label: string;
  value: string;
};

export type SystemMenuSocialPanelRow = {
  name: string;
  meta: string;
  note: string;
  metrics: SystemMenuSocialPanelMetric[];
};

export type SystemMenuSocialPanelTab = {
  key: string;
  label: string;
  rows: SystemMenuSocialPanelRow[];
  actions: string[];
};

export type SystemMenuSocialPanelDefinition = {
  subtitle: string;
  footer: string;
  tabs: SystemMenuSocialPanelTab[];
};
