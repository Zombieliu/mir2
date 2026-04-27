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

export type AdminPlayerDetail = {
  summary: AdminPlayerSummary;
  inventoryCount: number;
  beltCount: number;
  storageCount: number;
  equipmentCount: number;
  questStateCount: number;
  skillStateCount: number;
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
    updatedAtMs: number;
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

export function operatorHeaders() {
  const headers: Record<string, string> = {
    "x-operator-id": process.env.ADMIN_OPERATOR_ID ?? "local-gm",
    "x-operator-email": process.env.ADMIN_OPERATOR_EMAIL ?? "gm.local@mir2.dev",
    "x-operator-role": process.env.ADMIN_OPERATOR_ROLE ?? "ops_admin",
    "x-operator-permissions":
      process.env.ADMIN_OPERATOR_PERMISSIONS ??
      "account_read,account_ban,character_read,character_kick,inventory_read,inventory_grant_item,currency_grant,mail_send_system,content_publish,audit_read,approval_manage,permission_manage"
  };
  if (process.env.ADMIN_OPERATOR_TOKEN) {
    headers.authorization = `Bearer ${process.env.ADMIN_OPERATOR_TOKEN}`;
  }
  return headers;
}

export async function adminGet<T>(path: string): Promise<ApiResult<T>> {
  try {
    const response = await fetch(`${adminApiBase}${path}`, {
      cache: "no-store",
      headers: operatorHeaders()
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
    const response = await fetch(`${adminApiBase}${path}`, {
      method: "POST",
      cache: "no-store",
      headers: {
        "content-type": "application/json",
        ...operatorHeaders()
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
