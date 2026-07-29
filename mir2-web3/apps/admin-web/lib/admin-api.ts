import { cookies } from "next/headers";
import { parseAdminApiResponse } from "./admin-api-response";

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

export type DailyMapMetric = {
  mapFileName: string;
  mapTitle: string;
  characterCount: number;
  percent: number;
};

export type DailyReport = {
  reportId: string;
  reportDate: string;
  timezone: string;
  scope: string;
  status: string;
  sourceWindowStartMs: number;
  sourceWindowEndMs: number;
  metrics: {
    totalAccounts: number;
    totalCharacters: number;
    onlineAtGeneration: number;
    dailyActiveAccounts: number;
    gameplayEventCount: number;
    activeZones: number;
    lastGameplayEventAtMs?: number;
    totalGoldStock: number;
    totalCreditStock: number;
    activeBans: number;
    healthyServices: number;
    configuredServices: number;
    mapPopulation: DailyMapMetric[];
    levelDistribution: Array<{ label: string; characters: number }>;
    commandDistribution: GameplayEventCommandSummary[];
  };
  evidence: {
    generatedAtMs: number;
    sources: Array<{
      source: string;
      status: string;
      detail: string;
      observedAtMs: number;
    }>;
    warnings: string[];
    privacy: string;
  };
  operationsMarkdown: string;
  playerMarkdown: string;
  generationSource: string;
  model?: string;
  promptVersion: string;
  inputSha256: string;
  contentSha256: string;
  createdBy: string;
  reviewedBy?: string;
  reviewReason?: string;
  publishedBy?: string;
  createdAtMs: number;
  updatedAtMs: number;
  reviewedAtMs?: number;
  publishedAtMs?: number;
};

export type DailyReportDelivery = {
  deliveryId: string;
  reportId: string;
  channel: string;
  destinationLabel: string;
  status: string;
  attempts: number;
  nextAttemptAtMs?: number;
  lastAttemptAtMs?: number;
  deliveredAtMs?: number;
  providerMessageId?: string;
  lastError?: string;
  createdAtMs: number;
  updatedAtMs: number;
};

export type DailyReportListResponse = {
  configured: boolean;
  schedulerEnabled: boolean;
  discordConfigured: boolean;
  timezone: string;
  schedule: string;
  reports: DailyReport[];
};

export type DailyReportDetailResponse = {
  report: DailyReport;
  deliveries: DailyReportDelivery[];
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

export type DirectorPressureScores = {
  populationImbalanceBps: number;
  contentFatigueBps: number;
  progressionGapBps: number;
  economyInflationBps: number;
  guildDominanceBps: number;
};

export type DirectorApprovalRecord = {
  proposalId: string;
  status: string;
  riskLevel: string;
  snapshot: {
    snapshotId: string;
    gameId: string;
    regionId: string;
    observedAtMs: number;
    maps: Array<{
      zoneId: string;
      activePlayers: number;
      medianLevel: number;
      monsterKills: number;
      completedQuests: number;
    }>;
  };
  pressureScores: DirectorPressureScores;
  proposal: {
    templateId: string;
    targetZones: string[];
    durationMs: number;
    rewardBudget: number;
    rationale: string;
  };
  requestedBy: string;
  requestedAtMs: number;
  decidedBy?: string;
  decisionReason?: string;
  decidedAtMs?: number;
  commandId?: string;
  finalizedHeight?: number;
  finalizedDigest?: string;
  commonwareNetworkHeight?: number;
  commonwareNetworkStateRoot?: string;
  commonwareNetworkCommandDigest?: string;
  approvalAuditHash?: string;
  zoneReceipts: unknown[];
  lastError?: string;
  updatedAtMs: number;
};

export type DirectorAuditRecord = {
  auditId: string;
  proposalId?: string;
  action: string;
  actorId: string;
  fromStatus?: string;
  toStatus?: string;
  reason: string;
  occurredAtMs: number;
  previousHash: string;
  recordHash: string;
};

export type WorldDirectorDashboard = {
  schema: string;
  generatedAtMs: number;
  paused: boolean;
  pauseReason?: string;
  proposals: DirectorApprovalRecord[];
  audit: DirectorAuditRecord[];
  configuration: {
    executionConfigured: boolean;
    persistence: string;
    directorPublicKey?: string;
    committeeSize: number;
    zoneHostCount: number;
    automaticGenerationEnabled: boolean;
    generationIntervalSeconds: number;
    remoteCommonwareConfigured: boolean;
    remoteCommonwareRequired: boolean;
    proposalGenerator: "rule_engine" | "openai_responses" | string;
    aiConfigured: boolean;
    aiProvider?: string;
    aiModel?: string;
  };
  runtimeStatuses: Array<{
    endpoint: string;
    status: string;
    error?: string;
    runtime?: {
      finalizedHeight: number;
      installedCommandCount: number;
      appliedActionCount: number;
      spawnedMonstersTotal: number;
      broadcastMessagesTotal: number;
      lastAdvanceAtMs: number;
    };
  }>;
  pendingCount: number;
  activeCount: number;
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

export type AdminServiceTracePlayer = {
  playerId: string;
  accountId: string;
  characterIndex: number;
  characterName: string;
  online: boolean;
  mapFileName: string;
  playerObjectId?: number;
};

export type AdminServicePlacement = {
  gatewaySessionId?: string;
  gatewayId?: string;
  gatewayEndpoint?: string;
  relayId?: string;
  relayEndpoint?: string;
  serviceNodeId?: string;
  nodeKind?: string;
  zoneId?: string;
  lineId?: number;
  mapFileName?: string;
  zoneOwnerFencingToken?: number;
  handoffGeneration: number;
  routeLeaseExpiresAtMs?: number;
  updatedAtMs: number;
  tick: number;
};

export type AdminServiceTraceEvent = {
  eventId: string;
  eventType: string;
  occurredAtMs: number;
  gatewaySessionId?: string;
  gatewayId?: string;
  relayId?: string;
  serviceNodeId?: string;
  nodeKind?: string;
  zoneId?: string;
  lineId?: number;
  mapFileName?: string;
  zoneOwnerFencingToken?: number;
  handoffGeneration: number;
  reason: string;
};

export type AdminCommonwarePlacement = {
  source: string;
  gatewayId: string;
  finalizedHeight: number;
  stateRoot: string;
  zoneId: string;
  generation: number;
  primaryHostId: string;
  replicaHostIds: string[];
  primaryEndpoint: string;
  replicaEndpoints: string[];
  expiresAtMs: number;
  sessionLease?: {
    sessionId: string;
    gatewayId: string;
    zoneId: string;
    fencingToken: number;
    expiresAtMs: number;
  };
};

export type AdminServiceTraceReadModel = {
  generatedAtMs: number;
  query: string;
  status:
    | "online"
    | "degraded"
    | "stale"
    | "offline"
    | "no_runtime_record"
    | "not_found"
    | "ambiguous"
    | "unavailable";
  reason?: string;
  player?: AdminServiceTracePlayer;
  candidates: AdminServiceTracePlayer[];
  current?: AdminServicePlacement;
  history: AdminServiceTraceEvent[];
  commonware?: AdminCommonwarePlacement;
  diagnostics: Array<{
    component: string;
    status: string;
    message: string;
  }>;
  sensitiveRedacted: boolean;
  auditTraceId: string;
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

export type AdminEconomyAggregateReadModel = {
  source: string;
  generatedAtMs: number;
  configured: boolean;
  characterCount: number;
  totalGold: number;
  totalCredit: number;
  averageGold: number;
  maxGold: number;
  activeAuctionCount: number;
  activeAuctionValue: number;
  unclaimedMailCount: number;
  unclaimedMailGold: number;
  goldDistribution: Array<{ bucket: string; characters: number }>;
  topHolders: Array<{
    accountId: string;
    characterIndex: number;
    characterName: string;
    gold: number;
  }>;
  goldByMap: Array<{
    mapFileName: string;
    totalGold: number;
    characters: number;
  }>;
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

const configuredAdminApiBase = process.env.ADMIN_API_BASE_URL?.trim();
const adminApiBase = configuredAdminApiBase ?? "http://127.0.0.1:7420";
const adminApiTimeoutMs = Math.max(
  1_000,
  Math.min(30_000, Number(process.env.ADMIN_API_TIMEOUT_MS ?? 8_000) || 8_000)
);

// Permissive operator identity used ONLY for local development with the
// header/policy auth backend. In production the operator must be configured
// explicitly (or, preferably, the postgres bearer-token backend is used); we
// never grant a default all-powerful operator on a hosted deployment.
const DEV_DEFAULT_OPERATOR_PERMISSIONS =
  "account_read,account_write,account_ban,character_read,character_write,character_kick,character_message,inventory_read,inventory_grant_item,currency_grant,mail_send_system,world_broadcast,server_control,market_moderate,guild_moderate,namelist_manage,content_read,content_publish,content_rollback,audit_read,approval_manage,permission_manage";

function isProductionRuntime() {
  if (process.env.NODE_ENV === "production") return true;
  return ["MIR2_RUNTIME_ENV", "MIR2_DEPLOYMENT_ENV", "MIR2_ENV", "VERCEL_ENV"].some((name) => {
    const value = process.env[name]?.trim().toLowerCase();
    return value === "production" || value === "prod" || value === "staging";
  });
}

export async function operatorHeaders() {
  const cookieStore = await cookies();
  const cookieToken = cookieStore.get("admin_operator_token")?.value?.trim();
  // Hosted deployments keep dashboard login and Admin API service credentials
  // separate. Local development still permits switching operator tokens via
  // the login cookie.
  const token = isProductionRuntime()
    ? process.env.ADMIN_OPERATOR_TOKEN?.trim()
    : cookieToken ?? process.env.ADMIN_OPERATOR_TOKEN?.trim();
  // Fail closed in production: if the operator identity is not explicitly
  // configured, send empty headers (the admin-api rejects them) instead of
  // defaulting to a full-permission "local-gm" operator.
  const devDefaults = isProductionRuntime()
    ? { id: "", email: "", role: "", permissions: "" }
    : {
        id: "local-gm",
        email: "gm.local@mir2.dev",
        role: "ops_admin",
        permissions: DEV_DEFAULT_OPERATOR_PERMISSIONS,
      };
  const headers: Record<string, string> = {
    "x-operator-id": process.env.ADMIN_OPERATOR_ID ?? devDefaults.id,
    "x-operator-email": process.env.ADMIN_OPERATOR_EMAIL ?? devDefaults.email,
    "x-operator-role": process.env.ADMIN_OPERATOR_ROLE ?? devDefaults.role,
    "x-operator-permissions": process.env.ADMIN_OPERATOR_PERMISSIONS ?? devDefaults.permissions,
  };
  if (token) {
    headers.authorization = `Bearer ${token}`;
  }
  const proxyToken = process.env.ADMIN_API_PROXY_TOKEN?.trim();
  if (proxyToken) {
    headers["x-dubhe-admin-proxy-token"] = proxyToken;
  }
  return headers;
}

export async function adminGet<T>(path: string): Promise<ApiResult<T>> {
  if (isProductionRuntime() && !configuredAdminApiBase) {
    return { ok: false, error: "ADMIN_API_BASE_URL is not configured" };
  }
  try {
    const headers = await operatorHeaders();
    const response = await fetch(`${adminApiBase}${path}`, {
      cache: "no-store",
      headers,
      signal: AbortSignal.timeout(adminApiTimeoutMs)
    });
    const parsed = await parseAdminApiResponse(response);
    if (!parsed.ok) {
      return { ok: false, status: response.status, error: parsed.error };
    }
    const data = parsed.data;
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
  if (isProductionRuntime() && !configuredAdminApiBase) {
    return { ok: false, error: "ADMIN_API_BASE_URL is not configured" };
  }
  try {
    const authHeaders = await operatorHeaders();
    const response = await fetch(`${adminApiBase}${path}`, {
      method: "POST",
      cache: "no-store",
      headers: {
        "content-type": "application/json",
        ...authHeaders
      },
      signal: AbortSignal.timeout(adminApiTimeoutMs),
      body: JSON.stringify(body)
    });
    const parsed = await parseAdminApiResponse(response);
    if (!parsed.ok) {
      return { ok: false, status: response.status, error: parsed.error };
    }
    const data = parsed.data;
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
