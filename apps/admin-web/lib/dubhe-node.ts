import "server-only";

import { createHash, createPublicKey, verify } from "node:crypto";
import testnetSnapshot from "../data/dubhe-node-testnet.json";
import { mergeDubheNodeOperatorRecords } from "./dubhe-node-merge";

const NODE_ID_DOMAIN = Buffer.from("obelisk.guild-node.ed25519.v1\0", "utf8");
const ED25519_SPKI_PREFIX = Buffer.from("302a300506032b6570032100", "hex");
const DEFAULT_OPERATOR_URLS = [
  "http://127.0.0.1:19100",
  "http://127.0.0.1:19101",
  "http://127.0.0.1:29100"
];
const { acceptance: acceptanceEvidence, deployment, registration: activeRegistration } =
  testnetSnapshot;

type ZoneHostHealth = {
  hostId: string;
  processId: number;
  sessionCount: number;
  activeConnections: number;
  sessionCapacity: number;
  sessionCapacityPerZone: number;
  busiestZoneSessionCount: number;
  zoneCount: number;
  zoneCapacity: number;
  draining: boolean;
  protocolVersion: number;
};

type ZoneHostTelemetry = {
  health: ZoneHostHealth;
  zones?: ZoneHostZone[];
  startedAtMs: number;
  uptimeSeconds: number;
  acceptedConnectionsTotal: number;
  rpcRequestsTotal: number;
  rpcErrorsTotal: number;
};

type ZoneHostZone = {
  zoneId: string;
  mapScope: "all" | "explicit" | "unknown";
  mapFileNames: string[];
  sessionCount: number;
};

type ZoneHostHeartbeat = {
  payload: {
    schema: string;
    hostId: string;
    publicKey: string;
    keyGeneration: number;
    advertisedEndpoint: string;
    failureDomain: string;
    observedAtMs: number;
    sequence: number;
    processId: number;
    protocolVersion: number;
    sessionCount: number;
    sessionCapacity: number;
    sessionCapacityPerZone: number;
    busiestZoneSessionCount: number;
    zoneCount: number;
    zoneCapacity: number;
    zones?: ZoneHostZone[];
    activeConnections: number;
    draining: boolean;
  };
  signatureAlgorithm: string;
  signature: string;
};

export type DubheNodeZoneRecord = ZoneHostZone;

export type DubheNodeRecord = {
  nodeId: string;
  label: string;
  advertisedEndpoint: string;
  failureDomain: string;
  coarseRegion?: string;
  providerCode?: string;
  telemetryState: "live" | "offline";
  registrationState: "active" | "unregistered";
  heartbeatVerified: boolean;
  registrationMatched: boolean;
  keyGeneration: number;
  processId?: number;
  protocolVersion?: number;
  sessions: number;
  sessionCapacity: number;
  sessionCapacityPerZone?: number;
  busiestZoneSessionCount?: number;
  zones: number;
  zoneCapacity: number;
  activeZones: DubheNodeZoneRecord[];
  zoneDetailsVerified: boolean;
  activeConnections: number;
  draining: boolean;
  uptimeSeconds: number;
  rpcRequestsTotal: number;
  rpcErrorsTotal: number;
  observedAtMs?: number;
  workMode?: string;
  relayRttMs?: number;
  packetLossBps?: number;
  measuredUpstreamKbps?: number;
  checkpointLagMs?: number;
  placementGeneration?: number;
  verifiedWorkUnits?: number;
  sessionMilliseconds?: number;
  agentVersion?: string;
  stakeMist: number;
  operatorSuiAddress?: string;
  publicKey?: string;
  error?: string;
};

export type DubheNodeConsoleSnapshot = {
  generatedAtMs: number;
  mode: "live" | "degraded" | "offline";
  network: "testnet";
  packageId: string;
  registryId: string;
  publishTransaction: string;
  activeRegistrationTransaction: string;
  activeRegistrationCheckpoint: number;
  registeredNodeCount: number;
  retiredNodeCount: number;
  liveNodeCount: number;
  totalSessions: number;
  totalSessionCapacity: number;
  totalZones: number;
  totalZoneCapacity: number;
  totalStakeMist: number;
  nodes: DubheNodeRecord[];
  finality: {
    adapter: string;
    quorum: number;
    finalizedHeight: number;
    membershipEligible: boolean;
    evidenceGeneratedAtMs: number;
  };
  capacity: {
    completedCommands: number;
    maxSessionsPerZone: number;
    p95LatencyMs: number;
    certificateId: string;
    certificateExpiresAtMs: number;
    issuerPublicKey: string;
  };
  rewards: {
    batchId: string;
    merkleRoot: string;
    total: number;
  };
  links: {
    grafana: string;
    prometheus: string;
    prometheusAlerts: string;
    snapshotExport: string;
    registrationExplorer: string;
    packageExplorer: string;
  };
  sourceNote: string;
};

type OperatorProbe = {
  operatorUrl: string;
  telemetry?: ZoneHostTelemetry;
  heartbeat?: ZoneHostHeartbeat;
  heartbeatVerified: boolean;
  error?: string;
};

type HomeOperatorTelemetry = {
  nodeId: string;
  keyGeneration: number;
  observedAtMs: number;
  coarseRegion: string;
  providerCode: string;
  workMode: string;
  activeSessions: number;
  activeZones: number;
  zoneIds: string[];
  relayRttMs: number;
  packetLossBps: number;
  measuredUpstreamKbps: number;
  checkpointLagMs: number;
  placementGeneration: number;
  verifiedWorkUnits: number;
  sessionMilliseconds: number;
  agentVersion: string;
};

type HomeOperatorNode = {
  nodeId: string;
  assignedZoneId: string;
  capacityMaxSessions: number;
  capacityMaxZones: number;
  placementGeneration: number;
  admissionExpiresAtMs: number;
  telemetry?: HomeOperatorTelemetry;
};

type HomeOperatorSnapshot = {
  generatedAtMs: number;
  nodes: HomeOperatorNode[];
};

export async function readDubheNodeConsole(): Promise<DubheNodeConsoleSnapshot> {
  const homeTelemetryUrl = process.env.DUBHE_HOME_TELEMETRY_URL?.trim();
  if (homeTelemetryUrl) {
    const operatorUrls = configuredOperatorUrls(false);
    const [homeSnapshot, probes] = await Promise.all([
      readHomeTelemetryConsole(homeTelemetryUrl),
      Promise.all(operatorUrls.map(probeOperator))
    ]);
    const operatorNodes = probes
      .filter((probe): probe is OperatorProbe & { telemetry: ZoneHostTelemetry } =>
        Boolean(probe.telemetry)
      )
      .map(recordFromProbe);
    return mergeDubheNodeOperatorRecords(
      homeSnapshot,
      operatorNodes,
      operatorUrls.length
    );
  }
  const operatorUrls = configuredOperatorUrls(true);
  const probes = await Promise.all(operatorUrls.map(probeOperator));
  const liveRecords = probes
    .filter((probe): probe is OperatorProbe & { telemetry: ZoneHostTelemetry } =>
      Boolean(probe.telemetry)
    )
    .map(recordFromProbe);
  const registeredNode = registrationRecord(liveRecords);
  const nodes = liveRecords.some((node) => node.nodeId === registeredNode.nodeId)
    ? liveRecords
    : [registeredNode, ...liveRecords];
  const liveNodes = nodes.filter((node) => node.telemetryState === "live");
  const liveProbeCount = probes.filter((probe) => probe.telemetry).length;
  const mode =
    liveProbeCount === operatorUrls.length
      ? "live"
      : liveProbeCount > 0
        ? "degraded"
        : "offline";

  return {
    generatedAtMs: Date.now(),
    mode,
    network: "testnet",
    packageId: deployment.packageId,
    registryId: deployment.registryId,
    publishTransaction: deployment.publishTransaction,
    activeRegistrationTransaction: activeRegistration.transactionDigest,
    activeRegistrationCheckpoint: activeRegistration.checkpoint,
    registeredNodeCount: deployment.registeredNodeCount,
    retiredNodeCount: deployment.retiredNodeCount,
    liveNodeCount: liveNodes.length,
    totalSessions: sum(liveNodes.map((node) => node.sessions)),
    totalSessionCapacity: sum(liveNodes.map((node) => node.sessionCapacity)),
    totalZones: sum(liveNodes.map((node) => node.zones)),
    totalZoneCapacity: sum(liveNodes.map((node) => node.zoneCapacity)),
    totalStakeMist: nodes
      .filter((node) => node.registrationState === "active")
      .reduce((total, node) => total + node.stakeMist, 0),
    nodes,
    finality: {
      adapter: "Commonware v2026.2.0",
      quorum: acceptanceEvidence.commonwareQuorum,
      finalizedHeight: acceptanceEvidence.commonwareFinalizedHeight,
      membershipEligible: acceptanceEvidence.membershipEligible,
      evidenceGeneratedAtMs: acceptanceEvidence.generatedAtMs
    },
    capacity: {
      completedCommands: acceptanceEvidence.capacityCompletedCommands,
      maxSessionsPerZone: acceptanceEvidence.capacityMaxSessionsPerZone,
      p95LatencyMs: acceptanceEvidence.capacityP95LatencyMs,
      certificateId: acceptanceEvidence.capacityCertificateId,
      certificateExpiresAtMs: acceptanceEvidence.capacityCertificateExpiresAtMs,
      issuerPublicKey: acceptanceEvidence.capacityCertificateIssuer
    },
    rewards: {
      batchId: acceptanceEvidence.rewardBatchId,
      merkleRoot: acceptanceEvidence.rewardMerkleRoot,
      total: acceptanceEvidence.rewardTotal
    },
    links: {
      grafana: process.env.DUBHE_NODE_GRAFANA_URL ?? "http://127.0.0.1:13000",
      prometheus: process.env.DUBHE_NODE_PROMETHEUS_URL ?? "http://127.0.0.1:19090",
      prometheusAlerts:
        process.env.DUBHE_NODE_PROMETHEUS_ALERTS_URL ?? "http://127.0.0.1:19090/alerts",
      snapshotExport: "/api/dubhe-nodes",
      registrationExplorer: suiTransactionUrl(activeRegistration.transactionDigest),
      packageExplorer: `https://suiscan.xyz/testnet/object/${deployment.packageId}`
    },
    sourceNote:
      mode === "offline"
        ? "No configured Zone Host operator endpoint responded; chain and acceptance evidence remain visible."
        : `${liveProbeCount}/${operatorUrls.length} configured operator endpoints responded.`
  };
}

async function readHomeTelemetryConsole(
  configuredUrl: string
): Promise<DubheNodeConsoleSnapshot> {
  const telemetryUrl = configuredUrl.replace(/\/+$/, "");
  const operatorToken = process.env.DUBHE_HOME_TELEMETRY_OPERATOR_TOKEN?.trim();
  let operatorSnapshot: HomeOperatorSnapshot | undefined;
  let error: string | undefined;
  if (!operatorToken) {
    error = "DUBHE_HOME_TELEMETRY_OPERATOR_TOKEN is not configured.";
  } else {
    try {
      operatorSnapshot = await fetchJson<HomeOperatorSnapshot>(
        `${telemetryUrl}/v1/operator`,
        {
          headers: {
            authorization: `Bearer ${operatorToken}`
          }
        },
        4_000
      );
    } catch (cause) {
      error =
        cause instanceof Error ? cause.message : "Home telemetry operator endpoint unavailable";
    }
  }

  const generatedAtMs = operatorSnapshot?.generatedAtMs ?? Date.now();
  const nodes =
    operatorSnapshot?.nodes.map((node) => recordFromHomeTelemetry(node, generatedAtMs)) ?? [
      {
        ...registrationRecord([]),
        advertisedEndpoint: telemetryUrl,
        error: error ?? "No admitted Home Nodes were returned."
      }
    ];
  const liveNodes = nodes.filter((node) => node.telemetryState === "live");
  const mode =
    liveNodes.length === 0
      ? "offline"
      : liveNodes.length === nodes.length
        ? "live"
        : "degraded";

  return {
    generatedAtMs,
    mode,
    network: "testnet",
    packageId: deployment.packageId,
    registryId: deployment.registryId,
    publishTransaction: deployment.publishTransaction,
    activeRegistrationTransaction: activeRegistration.transactionDigest,
    activeRegistrationCheckpoint: activeRegistration.checkpoint,
    registeredNodeCount: Math.max(deployment.registeredNodeCount, nodes.length),
    retiredNodeCount: deployment.retiredNodeCount,
    liveNodeCount: liveNodes.length,
    totalSessions: sum(liveNodes.map((node) => node.sessions)),
    totalSessionCapacity: sum(liveNodes.map((node) => node.sessionCapacity)),
    totalZones: sum(liveNodes.map((node) => node.zones)),
    totalZoneCapacity: sum(liveNodes.map((node) => node.zoneCapacity)),
    totalStakeMist: nodes
      .filter((node) => node.registrationState === "active")
      .reduce((total, node) => total + node.stakeMist, 0),
    nodes,
    finality: {
      adapter: "Commonware v2026.2.0",
      quorum: acceptanceEvidence.commonwareQuorum,
      finalizedHeight: acceptanceEvidence.commonwareFinalizedHeight,
      membershipEligible: acceptanceEvidence.membershipEligible,
      evidenceGeneratedAtMs: acceptanceEvidence.generatedAtMs
    },
    capacity: {
      completedCommands: acceptanceEvidence.capacityCompletedCommands,
      maxSessionsPerZone: acceptanceEvidence.capacityMaxSessionsPerZone,
      p95LatencyMs: acceptanceEvidence.capacityP95LatencyMs,
      certificateId: acceptanceEvidence.capacityCertificateId,
      certificateExpiresAtMs: acceptanceEvidence.capacityCertificateExpiresAtMs,
      issuerPublicKey: acceptanceEvidence.capacityCertificateIssuer
    },
    rewards: {
      batchId: acceptanceEvidence.rewardBatchId,
      merkleRoot: acceptanceEvidence.rewardMerkleRoot,
      total: acceptanceEvidence.rewardTotal
    },
    links: {
      grafana:
        process.env.DUBHE_NODE_GRAFANA_URL ??
        "/ops/grafana/d/dubhe-home-nodes/dubhe-home-node-fleet?orgId=1&refresh=10s",
      prometheus:
        process.env.DUBHE_NODE_PROMETHEUS_URL ??
        "/ops/prometheus/query?g0.expr=dubhe_home_nodes_live&g0.tab=0",
      prometheusAlerts:
        process.env.DUBHE_NODE_PROMETHEUS_ALERTS_URL ?? "/ops/prometheus/alerts",
      snapshotExport: "/api/dubhe-nodes",
      registrationExplorer: suiTransactionUrl(activeRegistration.transactionDigest),
      packageExplorer: `https://suiscan.xyz/testnet/object/${deployment.packageId}`
    },
    sourceNote: error
      ? `Home telemetry is degraded: ${error}`
      : `${liveNodes.length}/${nodes.length} admitted Home Nodes are live; assigned and active Zone workloads are shown separately.`
  };
}

function recordFromHomeTelemetry(
  node: HomeOperatorNode,
  generatedAtMs: number
): DubheNodeRecord {
  const telemetry = node.telemetry;
  const live =
    telemetry !== undefined &&
    generatedAtMs >= telemetry.observedAtMs &&
    generatedAtMs - telemetry.observedAtMs <= 180_000;
  const claimsRegisteredNode = node.nodeId === activeRegistration.nodeId;
  const assignedZoneIds = [
    node.assignedZoneId,
    ...(telemetry?.zoneIds ?? [])
  ].filter((zoneId, index, values) => zoneId && values.indexOf(zoneId) === index);
  const activeZoneIds = new Set(telemetry?.zoneIds ?? []);
  const activeZones = assignedZoneIds.map((zoneId, index) => ({
    ...zoneWorkload(zoneId),
    sessionCount:
      activeZoneIds.has(zoneId) && index === 0 ? (telemetry?.activeSessions ?? 0) : 0
  }));
  return {
    nodeId: node.nodeId,
    label: shortNodeId(node.nodeId),
    advertisedEndpoint: process.env.DUBHE_HOME_RELAY_URL ?? "relay-hk.obelisk.build",
    failureDomain: telemetry
      ? `${telemetry.coarseRegion} · ${telemetry.providerCode}`
      : "admitted · awaiting telemetry",
    coarseRegion: telemetry?.coarseRegion,
    providerCode: telemetry?.providerCode,
    telemetryState: live ? "live" : "offline",
    registrationState: claimsRegisteredNode ? "active" : "unregistered",
    heartbeatVerified: Boolean(telemetry),
    registrationMatched: Boolean(telemetry) && claimsRegisteredNode,
    keyGeneration: telemetry?.keyGeneration ?? activeRegistration.keyGeneration,
    sessions: telemetry?.activeSessions ?? 0,
    sessionCapacity: node.capacityMaxSessions,
    busiestZoneSessionCount: telemetry?.activeSessions ?? 0,
    zones: telemetry?.activeZones ?? 0,
    zoneCapacity: node.capacityMaxZones,
    activeZones,
    zoneDetailsVerified: true,
    activeConnections: 0,
    draining: telemetry?.workMode === "draining",
    uptimeSeconds: 0,
    rpcRequestsTotal: telemetry?.verifiedWorkUnits ?? 0,
    rpcErrorsTotal: 0,
    observedAtMs: telemetry?.observedAtMs,
    workMode: telemetry?.workMode,
    relayRttMs: telemetry?.relayRttMs,
    packetLossBps: telemetry?.packetLossBps,
    measuredUpstreamKbps: telemetry?.measuredUpstreamKbps,
    checkpointLagMs: telemetry?.checkpointLagMs,
    placementGeneration: telemetry?.placementGeneration,
    verifiedWorkUnits: telemetry?.verifiedWorkUnits,
    sessionMilliseconds: telemetry?.sessionMilliseconds,
    agentVersion: telemetry?.agentVersion,
    stakeMist: claimsRegisteredNode ? activeRegistration.stakeMist : 0,
    operatorSuiAddress: claimsRegisteredNode
      ? activeRegistration.operatorSuiAddress
      : undefined,
    publicKey: claimsRegisteredNode ? activeRegistration.publicKey : undefined,
    error: live
      ? undefined
      : telemetry
        ? "The last signed Home Node report is stale."
        : "The node is admitted but has not submitted telemetry."
  };
}

function zoneWorkload(zoneId: string): Omit<ZoneHostZone, "sessionCount"> {
  if (zoneId === "primary") {
    return {
      zoneId,
      mapScope: "all",
      mapFileNames: []
    };
  }
  const mapLine = /^map:([^:]+):line:\d+$/u.exec(zoneId);
  if (mapLine) {
    return {
      zoneId,
      mapScope: "explicit",
      mapFileNames: [mapLine[1]]
    };
  }
  return {
    zoneId,
    mapScope: "unknown",
    mapFileNames: []
  };
}

function configuredOperatorUrls(includeDefaults: boolean) {
  const configured = process.env.DUBHE_NODE_OPERATOR_URLS?.split(",")
    .map((value) => value.trim().replace(/\/+$/, ""))
    .filter(Boolean);
  return configured?.length
    ? configured
    : includeDefaults
      ? DEFAULT_OPERATOR_URLS
      : [];
}

async function probeOperator(operatorUrl: string): Promise<OperatorProbe> {
  const operatorToken = process.env.DUBHE_NODE_OPERATOR_TOKEN?.trim();
  const init = operatorToken
    ? {
        headers: {
          "x-mir2-zone-operator-token": operatorToken
        }
      }
    : {};
  try {
    const telemetry = await fetchJson<ZoneHostTelemetry>(
      `${operatorUrl}/healthz`,
      init
    );
    let heartbeat: ZoneHostHeartbeat | undefined;
    let heartbeatError: string | undefined;
    try {
      heartbeat = await fetchJson<ZoneHostHeartbeat>(
        `${operatorUrl}/v1/heartbeat`,
        init
      );
    } catch (error) {
      heartbeatError =
        error instanceof Error ? error.message : "Heartbeat endpoint unavailable";
    }
    return {
      operatorUrl,
      telemetry,
      heartbeat,
      heartbeatVerified: heartbeat ? verifyHeartbeat(heartbeat) : false,
      error: heartbeatError
    };
  } catch (error) {
    return {
      operatorUrl,
      heartbeatVerified: false,
      error: error instanceof Error ? error.message : "Operator endpoint unavailable"
    };
  }
}

async function fetchJson<T>(
  url: string,
  init: RequestInit = {},
  timeoutMs = 1_500
): Promise<T> {
  const response = await fetch(url, {
    ...init,
    cache: "no-store",
    signal: AbortSignal.timeout(timeoutMs)
  });
  if (!response.ok) {
    throw new Error(`${new URL(url).host} returned HTTP ${response.status}`);
  }
  return (await response.json()) as T;
}

function recordFromProbe(probe: OperatorProbe & { telemetry: ZoneHostTelemetry }): DubheNodeRecord {
  const health = probe.telemetry.health;
  const heartbeat = probe.heartbeat?.payload;
  const claimsRegisteredNode = health.hostId === activeRegistration.nodeId;
  const registrationMatched =
    probe.heartbeatVerified &&
    claimsRegisteredNode &&
    heartbeat?.publicKey === activeRegistration.publicKey &&
    heartbeat.keyGeneration === activeRegistration.keyGeneration;
  const carriesSignedZoneDetails =
    probe.heartbeatVerified &&
    heartbeat?.schema === "obelisk.zone-host-heartbeat.v3";
  const activeZones =
    carriesSignedZoneDetails && heartbeat
      ? normalizeSignedZones(heartbeat.zones ?? [])
      : [];
  const zoneDetailsVerified =
    carriesSignedZoneDetails &&
    (health.zoneCount === 0 || activeZones.length === health.zoneCount);
  return {
    nodeId: health.hostId,
    label: shortNodeId(health.hostId),
    advertisedEndpoint: heartbeat?.advertisedEndpoint ?? probe.operatorUrl,
    failureDomain: heartbeat?.failureDomain ?? "unreported",
    telemetryState: "live",
    registrationState: claimsRegisteredNode ? "active" : "unregistered",
    heartbeatVerified: probe.heartbeatVerified,
    registrationMatched,
    keyGeneration: heartbeat?.keyGeneration ?? 0,
    processId: health.processId,
    protocolVersion: health.protocolVersion,
    sessions: health.sessionCount,
    sessionCapacity: health.sessionCapacity,
    sessionCapacityPerZone:
      probe.heartbeatVerified && heartbeat
        ? heartbeat.sessionCapacityPerZone
        : health.sessionCapacityPerZone,
    busiestZoneSessionCount:
      probe.heartbeatVerified && heartbeat
        ? heartbeat.busiestZoneSessionCount
        : health.busiestZoneSessionCount,
    zones: health.zoneCount,
    zoneCapacity: health.zoneCapacity,
    activeZones,
    zoneDetailsVerified,
    activeConnections: health.activeConnections,
    draining: health.draining,
    uptimeSeconds: probe.telemetry.uptimeSeconds,
    rpcRequestsTotal: probe.telemetry.rpcRequestsTotal,
    rpcErrorsTotal: probe.telemetry.rpcErrorsTotal,
    observedAtMs: heartbeat?.observedAtMs,
    workMode: health.draining ? "draining" : "serving",
    stakeMist: claimsRegisteredNode ? activeRegistration.stakeMist : 0,
    operatorSuiAddress: claimsRegisteredNode
      ? activeRegistration.operatorSuiAddress
      : undefined,
    publicKey: heartbeat?.publicKey,
    error: probe.heartbeatVerified
      ? undefined
      : probe.error ?? "Heartbeat signature verification failed"
  };
}

function registrationRecord(liveRecords: DubheNodeRecord[]): DubheNodeRecord {
  const live = liveRecords.find((node) => node.nodeId === activeRegistration.nodeId);
  if (live) {
    return live;
  }
  return {
    nodeId: activeRegistration.nodeId,
    label: shortNodeId(activeRegistration.nodeId),
    advertisedEndpoint: activeRegistration.endpoint,
    failureDomain: activeRegistration.failureDomain,
    telemetryState: "offline",
    registrationState: "active",
    heartbeatVerified: false,
    registrationMatched: false,
    keyGeneration: activeRegistration.keyGeneration,
    sessions: 0,
    sessionCapacity: activeRegistration.maxSessions,
    zones: 0,
    zoneCapacity: activeRegistration.maxZones,
    activeZones: [],
    zoneDetailsVerified: false,
    activeConnections: 0,
    draining: false,
    uptimeSeconds: 0,
    rpcRequestsTotal: 0,
    rpcErrorsTotal: 0,
    stakeMist: activeRegistration.stakeMist,
    operatorSuiAddress: activeRegistration.operatorSuiAddress,
    publicKey: activeRegistration.publicKey,
    error: "Registered on testnet; no live operator endpoint matched this identity."
  };
}

function verifyHeartbeat(heartbeat: ZoneHostHeartbeat) {
  try {
    const schemaHasZoneDetails =
      heartbeat.payload.schema === "obelisk.zone-host-heartbeat.v3";
    if (
      heartbeat.signatureAlgorithm !== "ed25519-zip215" ||
      (!schemaHasZoneDetails &&
        heartbeat.payload.schema !== "obelisk.zone-host-heartbeat.v2") ||
      heartbeat.payload.keyGeneration <= 0 ||
      heartbeat.payload.sessionCapacity <= 0 ||
      heartbeat.payload.sessionCapacityPerZone <= 0 ||
      heartbeat.payload.sessionCapacityPerZone > heartbeat.payload.sessionCapacity ||
      heartbeat.payload.sessionCount > heartbeat.payload.sessionCapacity ||
      heartbeat.payload.busiestZoneSessionCount >
        heartbeat.payload.sessionCapacityPerZone ||
      heartbeat.payload.busiestZoneSessionCount > heartbeat.payload.sessionCount ||
      heartbeat.payload.zoneCapacity <= 0 ||
      heartbeat.payload.zoneCount > heartbeat.payload.zoneCapacity
    ) {
      return false;
    }
    const zones = heartbeat.payload.zones ?? [];
    if (schemaHasZoneDetails) {
      if (
        zones.length !== heartbeat.payload.zoneCount ||
        sum(zones.map((zone) => zone.sessionCount)) !== heartbeat.payload.sessionCount
      ) {
        return false;
      }
      const zoneIds = new Set<string>();
      for (const zone of zones) {
        const mapsAreValid =
          zone.mapScope === "explicit"
            ? zone.mapFileNames.length > 0 &&
              zone.mapFileNames.every((map) => validTelemetryIdentifier(map))
            : (zone.mapScope === "all" || zone.mapScope === "unknown") &&
              zone.mapFileNames.length === 0;
        if (
          !validTelemetryIdentifier(zone.zoneId) ||
          zoneIds.has(zone.zoneId) ||
          !Number.isSafeInteger(zone.sessionCount) ||
          zone.sessionCount <= 0 ||
          zone.sessionCount > heartbeat.payload.sessionCapacityPerZone ||
          !mapsAreValid
        ) {
          return false;
        }
        zoneIds.add(zone.zoneId);
      }
    }
    const publicKeyBytes = Buffer.from(heartbeat.payload.publicKey, "base64url");
    if (publicKeyBytes.length !== 32) {
      return false;
    }
    const derivedNodeId = `ed25519:${createHash("sha256")
      .update(NODE_ID_DOMAIN)
      .update(publicKeyBytes)
      .digest("hex")}`;
    if (derivedNodeId !== heartbeat.payload.hostId) {
      return false;
    }
    const publicKey = createPublicKey({
      key: Buffer.concat([ED25519_SPKI_PREFIX, publicKeyBytes]),
      format: "der",
      type: "spki"
    });
    return verify(
      null,
      Buffer.from(JSON.stringify(heartbeat.payload)),
      publicKey,
      Buffer.from(heartbeat.signature, "base64url")
    );
  } catch {
    return false;
  }
}

function normalizeSignedZones(zones: ZoneHostZone[]): DubheNodeZoneRecord[] {
  return zones
    .map((zone) => ({
      zoneId: zone.zoneId,
      mapScope: zone.mapScope,
      mapFileNames: [...zone.mapFileNames].sort((left, right) =>
        left.localeCompare(right, undefined, { numeric: true })
      ),
      sessionCount: zone.sessionCount
    }))
    .sort((left, right) => left.zoneId.localeCompare(right.zoneId));
}

function validTelemetryIdentifier(value: string) {
  return (
    typeof value === "string" &&
    value.trim().length > 0 &&
    value.length <= 160 &&
    !/[\u0000-\u001f\u007f]/u.test(value)
  );
}

function sum(values: number[]) {
  return values.reduce((total, value) => total + value, 0);
}

function shortNodeId(nodeId: string) {
  const value = nodeId.replace(/^ed25519:/, "");
  return `NODE ${value.slice(0, 6).toUpperCase()}`;
}

function suiTransactionUrl(digest: string) {
  return `https://suiscan.xyz/testnet/tx/${digest}`;
}
