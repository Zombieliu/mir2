import type {
  DubheNodeConsoleSnapshot,
  DubheNodeRecord
} from "./dubhe-node";

export type DubheLocationSource = "node-reported" | "relay-fallback" | "unknown";

export type DubheNetworkNode = {
  nodeId: string;
  label: string;
  telemetryState: DubheNodeRecord["telemetryState"];
  workMode: string;
  sessions: number;
  sessionCapacity: number;
  zones: number;
  zoneCapacity: number;
  zoneIds: string[];
  mapFileNames: string[];
  relayRttMs?: number;
  packetLossBps?: number;
  measuredUpstreamKbps?: number;
  checkpointLagMs?: number;
  placementGeneration?: number;
  providerCode?: string;
  agentVersion?: string;
  observedAtMs?: number;
};

export type DubheNetworkRegion = {
  code: string;
  label: string;
  country: string;
  latitude: number;
  longitude: number;
  locationSource: Exclude<DubheLocationSource, "unknown">;
  nodeLocationKnown: boolean;
  liveNodes: number;
  servingNodes: number;
  drainingNodes: number;
  offlineNodes: number;
  activeSessions: number;
  sessionCapacity: number;
  activeZones: number;
  zoneCapacity: number;
  averageRelayRttMs?: number;
  averagePacketLossBps?: number;
  nodes: DubheNetworkNode[];
};

export type DubheNetworkSnapshot = {
  generatedAtMs: number;
  mode: DubheNodeConsoleSnapshot["mode"];
  totals: {
    admittedNodes: number;
    liveNodes: number;
    servingNodes: number;
    drainingNodes: number;
    activeSessions: number;
    sessionCapacity: number;
    activeZones: number;
    zoneCapacity: number;
    locatedRegions: number;
    unlocatedNodes: number;
    averageRelayRttMs?: number;
    commonwareFinalizedHeight: number;
    commonwareQuorum: number;
  };
  commonware: {
    status: "live" | "evidence" | "unavailable";
    source: string;
    finalizedHeight: number;
    gatewayId?: string;
    generation?: number;
    primaryHostId?: string;
    error?: string;
  };
  regions: DubheNetworkRegion[];
  unlocatedNodes: DubheNetworkNode[];
  privacy: {
    rawIpCollected: false;
    coordinatePrecision: "regional-centroid";
    note: string;
  };
};

type RegionDefinition = {
  code: string;
  label: string;
  country: string;
  latitude: number;
  longitude: number;
  aliases: string[];
};

const REGION_DEFINITIONS: RegionDefinition[] = [
  {
    code: "HK-HKG",
    label: "Hong Kong",
    country: "Hong Kong",
    latitude: 22.3193,
    longitude: 114.1694,
    aliases: ["hk", "hkg", "hk-region", "hong-kong", "hongkong"]
  },
  {
    code: "CN-EAST",
    label: "China East",
    country: "China",
    latitude: 31.2304,
    longitude: 121.4737,
    aliases: ["cn-east", "china-east", "shanghai", "cn-shanghai"]
  },
  {
    code: "CN-NORTH",
    label: "China North",
    country: "China",
    latitude: 39.9042,
    longitude: 116.4074,
    aliases: ["cn-north", "china-north", "beijing", "cn-beijing"]
  },
  {
    code: "SG-SIN",
    label: "Singapore",
    country: "Singapore",
    latitude: 1.3521,
    longitude: 103.8198,
    aliases: ["sg", "sin", "singapore", "sg-sin"]
  },
  {
    code: "JP-TYO",
    label: "Tokyo",
    country: "Japan",
    latitude: 35.6762,
    longitude: 139.6503,
    aliases: ["jp", "tyo", "tokyo", "jp-tyo"]
  },
  {
    code: "BR-SP",
    label: "São Paulo",
    country: "Brazil",
    latitude: -23.5505,
    longitude: -46.6333,
    aliases: ["br", "brazil", "sao-paulo", "br-sp"]
  },
  {
    code: "US-WEST",
    label: "US West",
    country: "United States",
    latitude: 37.7749,
    longitude: -122.4194,
    aliases: ["us-west", "usa-west", "san-francisco", "us-sfo"]
  },
  {
    code: "US-EAST",
    label: "US East",
    country: "United States",
    latitude: 39.0438,
    longitude: -77.4874,
    aliases: ["us-east", "usa-east", "virginia", "us-iad"]
  },
  {
    code: "EU-FRA",
    label: "Frankfurt",
    country: "Germany",
    latitude: 50.1109,
    longitude: 8.6821,
    aliases: ["eu", "eu-central", "frankfurt", "de-fra"]
  },
  {
    code: "AU-SYD",
    label: "Sydney",
    country: "Australia",
    latitude: -33.8688,
    longitude: 151.2093,
    aliases: ["au", "australia", "sydney", "au-syd"]
  }
];

const PRIVATE_REGION_MARKERS = new Set([
  "",
  "unknown",
  "unreported",
  "desktop-local",
  "local-lab",
  "privacy-protected"
]);

export function buildDubheNetworkSnapshot(
  fleet: DubheNodeConsoleSnapshot
): DubheNetworkSnapshot {
  const regions = new Map<
    string,
    {
      definition: RegionDefinition;
      locationSource: Exclude<DubheLocationSource, "unknown">;
      nodeLocationKnown: boolean;
      nodes: DubheNetworkNode[];
    }
  >();
  const unlocatedNodes: DubheNetworkNode[] = [];

  for (const node of fleet.nodes) {
    const networkNode = toNetworkNode(node);
    const location = resolveNodeLocation(node);
    if (!location) {
      unlocatedNodes.push(networkNode);
      continue;
    }
    const existing = regions.get(location.definition.code);
    if (existing) {
      existing.nodes.push(networkNode);
      existing.nodeLocationKnown &&= location.nodeLocationKnown;
      if (location.locationSource === "relay-fallback") {
        existing.locationSource = "relay-fallback";
      }
    } else {
      regions.set(location.definition.code, {
        ...location,
        nodes: [networkNode]
      });
    }
  }

  const regionalSnapshots = [...regions.values()]
    .map(({ definition, locationSource, nodeLocationKnown, nodes }) => {
      const liveNodes = nodes.filter((node) => node.telemetryState === "live");
      const servingNodes = liveNodes.filter((node) => node.workMode === "serving");
      const drainingNodes = liveNodes.filter((node) => node.workMode === "draining");
      return {
        code: definition.code,
        label: definition.label,
        country: definition.country,
        latitude: definition.latitude,
        longitude: definition.longitude,
        locationSource,
        nodeLocationKnown,
        liveNodes: liveNodes.length,
        servingNodes: servingNodes.length,
        drainingNodes: drainingNodes.length,
        offlineNodes: nodes.length - liveNodes.length,
        activeSessions: sum(liveNodes.map((node) => node.sessions)),
        sessionCapacity: sum(liveNodes.map((node) => node.sessionCapacity)),
        activeZones: sum(liveNodes.map((node) => node.zones)),
        zoneCapacity: sum(liveNodes.map((node) => node.zoneCapacity)),
        averageRelayRttMs: average(
          liveNodes.map((node) => node.relayRttMs).filter(isNumber)
        ),
        averagePacketLossBps: average(
          liveNodes.map((node) => node.packetLossBps).filter(isNumber)
        ),
        nodes: nodes.sort((left, right) => left.nodeId.localeCompare(right.nodeId))
      } satisfies DubheNetworkRegion;
    })
    .sort(
      (left, right) =>
        right.liveNodes - left.liveNodes ||
        right.activeSessions - left.activeSessions ||
        left.code.localeCompare(right.code)
    );

  const allNodes = regionalSnapshots.flatMap((region) => region.nodes);
  const liveNodes = allNodes
    .concat(unlocatedNodes)
    .filter((node) => node.telemetryState === "live");
  const relayRtts = liveNodes.map((node) => node.relayRttMs).filter(isNumber);

  return {
    generatedAtMs: fleet.generatedAtMs,
    mode: fleet.mode,
    totals: {
      admittedNodes: fleet.nodes.length,
      liveNodes: liveNodes.length,
      servingNodes: liveNodes.filter((node) => node.workMode === "serving").length,
      drainingNodes: liveNodes.filter((node) => node.workMode === "draining").length,
      activeSessions: sum(liveNodes.map((node) => node.sessions)),
      sessionCapacity: sum(liveNodes.map((node) => node.sessionCapacity)),
      activeZones: sum(liveNodes.map((node) => node.zones)),
      zoneCapacity: sum(liveNodes.map((node) => node.zoneCapacity)),
      locatedRegions: regionalSnapshots.length,
      unlocatedNodes:
        unlocatedNodes.length +
        regionalSnapshots
          .filter((region) => !region.nodeLocationKnown)
          .reduce((total, region) => total + region.nodes.length, 0),
      averageRelayRttMs: average(relayRtts),
      commonwareFinalizedHeight: fleet.finality.finalizedHeight,
      commonwareQuorum: fleet.finality.quorum
    },
    commonware: {
      status: "evidence",
      source: fleet.finality.adapter,
      finalizedHeight: fleet.finality.finalizedHeight
    },
    regions: regionalSnapshots,
    unlocatedNodes,
    privacy: {
      rawIpCollected: false,
      coordinatePrecision: "regional-centroid",
      note:
        "节点仅按签名 coarseRegion 或官方 Relay 区域聚合；坐标是区域中心点，不包含家庭 IP 或住宅位置。"
    }
  };
}

export function resolveRegion(
  value: string | undefined
): RegionDefinition | undefined {
  const normalized = normalizeRegion(value);
  if (PRIVATE_REGION_MARKERS.has(normalized)) return undefined;
  return REGION_DEFINITIONS.find(
    (definition) =>
      normalizeRegion(definition.code) === normalized ||
      definition.aliases.some((alias) => normalizeRegion(alias) === normalized)
  );
}

function resolveNodeLocation(node: DubheNodeRecord) {
  const reported = resolveRegion(node.coarseRegion ?? failureDomainRegion(node.failureDomain));
  if (reported) {
    return {
      definition: reported,
      locationSource: "node-reported" as const,
      nodeLocationKnown: true
    };
  }
  const relay = relayRegion(node.advertisedEndpoint);
  if (relay) {
    return {
      definition: relay,
      locationSource: "relay-fallback" as const,
      nodeLocationKnown: false
    };
  }
  return undefined;
}

function toNetworkNode(node: DubheNodeRecord): DubheNetworkNode {
  const mapFileNames = node.activeZones.flatMap((zone) => zone.mapFileNames);
  return {
    nodeId: node.nodeId,
    label: node.label,
    telemetryState: node.telemetryState,
    workMode: node.workMode ?? (node.draining ? "draining" : "serving"),
    sessions: node.sessions,
    sessionCapacity: node.sessionCapacity,
    zones: node.zones,
    zoneCapacity: node.zoneCapacity,
    zoneIds: node.activeZones.map((zone) => zone.zoneId),
    mapFileNames: [...new Set(mapFileNames)].sort(),
    relayRttMs: node.relayRttMs,
    packetLossBps: node.packetLossBps,
    measuredUpstreamKbps: node.measuredUpstreamKbps,
    checkpointLagMs: node.checkpointLagMs,
    placementGeneration: node.placementGeneration,
    providerCode: node.providerCode,
    agentVersion: node.agentVersion,
    observedAtMs: node.observedAtMs
  };
}

function failureDomainRegion(value: string) {
  return value.split("·", 1)[0]?.trim();
}

function relayRegion(value: string) {
  const normalized = value.toLowerCase();
  if (normalized.includes("relay-hk") || normalized.includes("hong-kong")) {
    return resolveRegion("HK-HKG");
  }
  if (normalized.includes("relay-sg") || normalized.includes("singapore")) {
    return resolveRegion("SG-SIN");
  }
  if (normalized.includes("relay-br") || normalized.includes("sao-paulo")) {
    return resolveRegion("BR-SP");
  }
  return undefined;
}

function normalizeRegion(value: string | undefined) {
  return (value ?? "")
    .trim()
    .toLowerCase()
    .replace(/[_\s]+/gu, "-");
}

function sum(values: number[]) {
  return values.reduce((total, value) => total + value, 0);
}

function average(values: number[]) {
  if (values.length === 0) return undefined;
  return values.reduce((total, value) => total + value, 0) / values.length;
}

function isNumber(value: number | undefined): value is number {
  return typeof value === "number" && Number.isFinite(value);
}
