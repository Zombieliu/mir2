import { cookies } from "next/headers";

export type ApiResult<T> =
  | { ok: true; data: T }
  | { ok: false; error: string; status?: number };

export type AdminCommandRecord = {
  envelope: {
    commandId: string;
    reason: string;
    operator: { email: string; role: string };
    target: { targetType: string; targetId: string };
    traceId: string;
  };
  status: string;
  resultMessage?: string;
  errorCode?: string;
  updatedAtMs: number;
};

export type AuditRecord = {
  auditId: string;
  commandId: string;
  operatorEmail: string;
  permission: string;
  target: { targetType: string; targetId: string };
  reason: string;
  status: string;
  errorCode?: string;
  traceId: string;
  completedAtMs?: number;
};

export type AdminEventRecord = {
  eventId: string;
  eventType: string;
  commandId: string;
  operatorId: string;
  status: string;
  occurredAtMs: number;
  payloadJson: string;
};

export type AdminEventsResponse = {
  degraded: boolean;
  error?: string;
  records: AdminEventRecord[];
};

export type AdminTimelineItem = {
  source: string;
  recordId: string;
  commandId?: string;
  targetId?: string;
  eventType: string;
  status: string;
  actorId?: string;
  occurredAtMs: number;
  summary: string;
};

export type AdminTimelineResponse = {
  degraded: boolean;
  error?: string;
  records: AdminTimelineItem[];
};

export type GameplayEventCommandSummary = {
  commandKind: string;
  eventCount: number;
  lastOccurredAtMs: number;
  maxSnapshotTick: number;
};

export type GameplayEventReadinessAlert = {
  level: string;
  code: string;
  message: string;
};

export type GameplayEventSummaryResponse = {
  degraded: boolean;
  ready: boolean;
  error?: string;
  generatedAtMs: number;
  windowSeconds: number;
  maxLagMs: number;
  minEventCount: number;
  totalCount: number;
  lastOccurredAtMs?: number;
  lagMs?: number;
  alerts: GameplayEventReadinessAlert[];
  commands: GameplayEventCommandSummary[];
};

export type AdminAuthMeResponse = {
  source: string;
  operator: {
    operatorId: string;
    email: string;
    role: string;
    status: string;
    permissions: string[];
    tokenConfigured: boolean;
    updatedAtMs: number;
    lastAuthenticatedAtMs?: number;
  };
};

export type ApprovalRecord = {
  approvalId: string;
  commandId: string;
  commandType: string;
  status: string;
  requestedBy: string;
  requestedReason: string;
  decidedBy?: string;
  decisionReason?: string;
  createdAtMs: number;
  updatedAtMs: number;
  decidedAtMs?: number;
};

export type SystemMailReceipt = {
  outboxId: string;
  targetKind: string;
  targetId: string;
  attachmentCount: number;
  acceptedAtMs: number;
  deliveryMode: string;
  deliveredCount: number;
  mailIds: number[];
};

export type SubmitCommandResponse = {
  commandId: string;
  result: {
    status: string;
    message: string;
  };
};

export type AdminMapPopulation = {
  mapFileName: string;
  mapTitle: string;
  characterCount: number;
  percent: number;
};

export type AdminServiceStatus = {
  name: string;
  status: string;
  detail: string;
  latencyMs?: number;
  configured: boolean;
};

export type AdminDashboardReadModel = {
  source: string;
  generatedAtMs: number;
  accountCount: number;
  characterCount: number;
  onlineNow: number;
  onlineSource: string;
  totalGold: number;
  totalCredit: number;
  activeBanCount: number;
  hotMaps: AdminMapPopulation[];
  services: AdminServiceStatus[];
  auditRecordCount: number;
  outboxReceiptCount: number;
};

export type AdminPlayerSummary = {
  playerId: string;
  accountId: string;
  characterIndex: number;
  characterName: string;
  className: string;
  gender: string;
  level: number;
  mapFileName: string;
  mapTitle: string;
  positionX: number;
  positionY: number;
  hp: number;
  maxHp: number;
  mp: number;
  gold: number;
  credit: number;
  pkPoints: number;
  chatBanned: boolean;
  chatBanUntilMs?: number;
  status: string;
  online: boolean;
  onlineSource?: string;
  playerObjectId?: number;
  runtimeTick?: number;
  storeVersion?: number;
  saveVersion?: number;
};

export type AdminPlayersReadModel = {
  source: string;
  generatedAtMs: number;
  players: AdminPlayerSummary[];
};

export type AdminAccountSummary = {
  accountId: string;
  passwordConfigured: boolean;
  characterCount: number;
  storageSize: number;
  hasStoragePassword: boolean;
  isBanned: boolean;
  banReason?: string;
  banUntilMs?: number;
  bannedAtMs?: number;
  storeVersion?: number;
};

export type AdminAccountsReadModel = {
  source: string;
  generatedAtMs: number;
  accounts: AdminAccountSummary[];
};

export type AdminAccountDetail = {
  summary: AdminAccountSummary;
  characters: AdminPlayerSummary[];
  hasExpandedStorage: boolean;
  expandedStorageExpiryTimeBinaryDatetime: number;
  storagePasswordLastSetBinaryDatetime: number;
};

export type AdminPlayerDetail = {
  summary: AdminPlayerSummary;
  inventoryCount: number;
  beltCount: number;
  storageCount: number;
  equipmentCount: number;
  questStateCount: number;
  skillStateCount: number;
  npcFlagCount: number;
  activeNpcFlags: Array<{
    index: number;
    active: boolean;
  }>;
  mailCount: number;
  unclaimedMailCount: number;
  auctionListingCount: number;
  groupMemberCount: number;
  guildName?: string;
  activeBanReason?: string;
  banUntilMs?: number;
  bannedAtMs?: number;
};

export type AdminEconomyAsset = {
  asset: string;
  total: number;
  holders: number;
  average: number;
  state: string;
};

export type AdminDistributionBucket = {
  key: string;
  label: string;
  value: number;
  amount: number;
};

export type AdminEconomyReadModel = {
  source: string;
  generatedAtMs: number;
  assets: AdminEconomyAsset[];
  goldDistribution: AdminDistributionBucket[];
  priceFeeds: Array<{
    item: string;
    latestPrice: number;
    sampleCount: number;
    source: string;
    updatedAtMs: number;
  }>;
  priceFeedConfigured: boolean;
};

export type AdminMarketReadModel = {
  source: string;
  generatedAtMs: number;
  listings: Array<{
    ownerPlayerId: string;
    accountId: string;
    characterIndex: number;
    characterName: string;
    listingId: number;
    seller: string;
    itemKey: string;
    price: number;
    sold: boolean;
    cancelled: boolean;
    expired: boolean;
  }>;
};

export type AdminGuildsReadModel = {
  source: string;
  generatedAtMs: number;
  guilds: Array<{
    guildName: string;
    memberCount: number;
    members: string[];
    ranks: string[];
    chatLog: string[];
  }>;
};

export type AdminNameListsReadModel = {
  source: string;
  generatedAtMs: number;
  root: string;
  lists: Array<{
    listName: string;
    path: string;
    playerCount: number;
    players: string[];
  }>;
};

export type AdminContentReadModel = {
  generatedAtMs: number;
  categories: Array<{
    category: string;
    source: string;
    totalRecords: number;
    status: string;
    editable: boolean;
  }>;
  bundles: Array<{
    bundleId: string;
    category: string;
    recordKey: string;
    path: string;
    updatedAtMs: number;
  }>;
};

export type AdminControlReadModel = {
  generatedAtMs: number;
  gatewayControlConfigured: boolean;
  controlActions: string[];
  logs: Array<{
    name: string;
    path: string;
    configured: boolean;
    lines: string[];
    error?: string;
  }>;
};

export type AdminActivitiesReadModel = {
  source: string;
  generatedAtMs: number;
  configured: boolean;
  activities: Array<{
    activityId: string;
    name: string;
    startAtMs?: number;
    activityType: string;
    status: string;
    signal: string;
    reward: string;
    condition: string;
    realms: string[];
    updatedAtMs: number;
  }>;
};

export type AdminServersReadModel = {
  generatedAtMs: number;
  accountStoreSource: string;
  accountCount: number;
  characterCount: number;
  zonesOnline: number;
  zonesSource: string;
  zoneRuntimeConfigured: boolean;
  zones: Array<{
    zoneId: string;
    name: string;
    status: string;
    host: string;
    processId: string;
    mapCount: number;
    playerCount: number;
    tickRate: number;
    uptimeSeconds: number;
    source: string;
    updatedAtMs: number;
  }>;
  services: AdminServiceStatus[];
};

export type AdminOperatorsReadModel = {
  source: string;
  generatedAtMs: number;
  configured: boolean;
  operators: Array<{
    operatorId: string;
    email: string;
    role: string;
    status: string;
    permissions: string[];
    tokenConfigured: boolean;
    updatedAtMs: number;
    lastAuthenticatedAtMs?: number;
  }>;
};

export type AdminRiskReadModel = {
  source: string;
  generatedAtMs: number;
  cases: Array<{
    playerId: string;
    accountId: string;
    characterName: string;
    signal: string;
    risk: string;
    evidence: string;
    banUntilMs?: number;
  }>;
  graph: Array<{
    edgeId: string;
    from: string;
    to: string;
    signal: string;
    risk: string;
    evidence: string;
    updatedAtMs: number;
  }>;
};

const adminApiBase = process.env.ADMIN_API_BASE_URL ?? "http://127.0.0.1:7420";

export async function operatorHeaders() {
  const cookieStore = await cookies();
  const token =
    cookieStore.get("admin_operator_token")?.value?.trim() ??
    process.env.ADMIN_OPERATOR_TOKEN?.trim();
  const headers: Record<string, string> = {
    "x-operator-id": process.env.ADMIN_OPERATOR_ID ?? "local-gm",
    "x-operator-email": process.env.ADMIN_OPERATOR_EMAIL ?? "gm.local@mir2.dev",
    "x-operator-role": process.env.ADMIN_OPERATOR_ROLE ?? "ops_admin",
    "x-operator-permissions":
      process.env.ADMIN_OPERATOR_PERMISSIONS ??
      "account_read,account_write,account_ban,character_read,character_write,character_kick,character_message,inventory_read,inventory_grant_item,currency_grant,mail_send_system,world_broadcast,server_control,market_moderate,guild_moderate,namelist_manage,content_read,content_publish,audit_read,approval_manage,permission_manage"
  };
  if (token) {
    headers.authorization = `Bearer ${token}`;
  }
  return headers;
}

export async function adminGet<T>(path: string): Promise<ApiResult<T>> {
  try {
    const headers = await operatorHeaders();
    const response = await fetch(`${adminApiBase}${path}`, {
      cache: "no-store",
      headers
    });
    const data = (await response.json()) as unknown;
    if (!response.ok) {
      return {
        ok: false,
        status: response.status,
        error: errorMessage(data) ?? `HTTP ${response.status}`
      };
    }
    return { ok: true, data: data as T };
  } catch (error) {
    return {
      ok: false,
      error: error instanceof Error ? error.message : "Admin API unavailable"
    };
  }
}

export async function adminPost<T>(
  path: string,
  body: unknown
): Promise<ApiResult<T>> {
  try {
    const authHeaders = await operatorHeaders();
    const response = await fetch(`${adminApiBase}${path}`, {
      method: "POST",
      cache: "no-store",
      headers: {
        "content-type": "application/json",
        ...authHeaders
      },
      body: JSON.stringify(body)
    });
    const data = (await response.json()) as unknown;
    if (!response.ok) {
      return {
        ok: false,
        status: response.status,
        error: errorMessage(data) ?? `HTTP ${response.status}`
      };
    }
    return { ok: true, data: data as T };
  } catch (error) {
    return {
      ok: false,
      error: error instanceof Error ? error.message : "Admin API unavailable"
    };
  }
}

function errorMessage(data: unknown): string | undefined {
  if (
    data &&
    typeof data === "object" &&
    "error" in data &&
    typeof data.error === "string"
  ) {
    return data.error;
  }
  return undefined;
}
