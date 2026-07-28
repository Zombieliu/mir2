import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import ts from "typescript";

const sourceUrl = new URL("../lib/dubhe-network-model.ts", import.meta.url);
const source = readFileSync(sourceUrl, "utf8");
const compiled = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.CommonJS,
    target: ts.ScriptTarget.ES2022,
    strict: true,
    skipLibCheck: true
  },
  fileName: sourceUrl.pathname,
  reportDiagnostics: true
});
const diagnostics = (compiled.diagnostics ?? []).filter(
  (diagnostic) => diagnostic.category === ts.DiagnosticCategory.Error
);
assert.deepEqual(diagnostics, []);
const loaded = { exports: {} };
new Function("exports", "module", "require", compiled.outputText)(
  loaded.exports,
  loaded,
  () => {
    throw new Error("dubhe-network-model must not have runtime dependencies");
  }
);

const { buildDubheNetworkSnapshot } = loaded.exports;

test("aggregates node-reported regions without exposing endpoints or IPs", () => {
  const snapshot = buildDubheNetworkSnapshot(
    fleet([
      node({
        nodeId: "node-br",
        advertisedEndpoint: "203.0.113.24:9444",
        coarseRegion: "BR-SP",
        providerCode: "home",
        sessions: 7,
        sessionCapacity: 24,
        zones: 2,
        zoneCapacity: 4,
        relayRttMs: 28,
        activeZones: [
          {
            zoneId: "map:0:line:1",
            mapScope: "explicit",
            mapFileNames: ["0"],
            sessionCount: 7
          }
        ]
      })
    ])
  );

  assert.equal(snapshot.regions.length, 1);
  assert.equal(snapshot.regions[0].code, "BR-SP");
  assert.equal(snapshot.regions[0].nodeLocationKnown, true);
  assert.equal(snapshot.regions[0].activeSessions, 7);
  assert.equal(snapshot.regions[0].averageRelayRttMs, 28);
  assert.equal(snapshot.regions[0].nodes[0].mapFileNames[0], "0");
  assert.equal(snapshot.privacy.rawIpCollected, false);
  assert.equal(JSON.stringify(snapshot).includes("203.0.113.24"), false);
});

test("marks Relay fallback separately instead of pretending it is a home location", () => {
  const snapshot = buildDubheNetworkSnapshot(
    fleet([
      node({
        nodeId: "node-private",
        coarseRegion: "desktop-local",
        advertisedEndpoint: "relay-hk.obelisk.build",
        workMode: "draining"
      })
    ])
  );

  assert.equal(snapshot.regions[0].code, "HK-HKG");
  assert.equal(snapshot.regions[0].locationSource, "relay-fallback");
  assert.equal(snapshot.regions[0].nodeLocationKnown, false);
  assert.equal(snapshot.regions[0].drainingNodes, 1);
});

test("keeps nodes unlocated when neither a coarse region nor Relay region exists", () => {
  const snapshot = buildDubheNetworkSnapshot(
    fleet([
      node({
        nodeId: "node-unlocated",
        coarseRegion: "198.51.100.9",
        advertisedEndpoint: "private-endpoint"
      })
    ])
  );

  assert.equal(snapshot.regions.length, 0);
  assert.equal(snapshot.unlocatedNodes.length, 1);
  assert.equal(snapshot.totals.unlocatedNodes, 1);
  assert.equal(JSON.stringify(snapshot).includes("198.51.100.9"), false);
});

function fleet(nodes) {
  return {
    generatedAtMs: 1_000_000,
    mode: "live",
    network: "testnet",
    packageId: "0x1",
    registryId: "0x2",
    publishTransaction: "publish",
    activeRegistrationTransaction: "register",
    activeRegistrationCheckpoint: 10,
    registeredNodeCount: nodes.length,
    retiredNodeCount: 0,
    liveNodeCount: nodes.length,
    totalSessions: nodes.reduce((total, value) => total + value.sessions, 0),
    totalSessionCapacity: nodes.reduce(
      (total, value) => total + value.sessionCapacity,
      0
    ),
    totalZones: nodes.reduce((total, value) => total + value.zones, 0),
    totalZoneCapacity: nodes.reduce(
      (total, value) => total + value.zoneCapacity,
      0
    ),
    totalStakeMist: 0,
    nodes,
    finality: {
      adapter: "Commonware",
      quorum: 3,
      finalizedHeight: 42,
      membershipEligible: true,
      evidenceGeneratedAtMs: 999_000
    },
    capacity: {
      completedCommands: 1,
      maxSessionsPerZone: 16,
      p95LatencyMs: 10,
      certificateId: "certificate",
      certificateExpiresAtMs: 2_000_000,
      issuerPublicKey: "issuer"
    },
    rewards: {
      batchId: "batch",
      merkleRoot: "root",
      total: 0
    },
    links: {
      grafana: "/grafana",
      prometheus: "/prometheus",
      prometheusAlerts: "/alerts",
      snapshotExport: "/api/dubhe-nodes",
      registrationExplorer: "/registration",
      packageExplorer: "/package"
    },
    sourceNote: "test"
  };
}

function node(overrides) {
  return {
    nodeId: "node",
    label: "node",
    advertisedEndpoint: "relay-hk.obelisk.build",
    failureDomain: "desktop-local · home",
    coarseRegion: "desktop-local",
    providerCode: "home",
    telemetryState: "live",
    registrationState: "active",
    heartbeatVerified: true,
    registrationMatched: true,
    keyGeneration: 1,
    sessions: 0,
    sessionCapacity: 16,
    zones: 1,
    zoneCapacity: 4,
    activeZones: [],
    zoneDetailsVerified: true,
    activeConnections: 0,
    draining: false,
    uptimeSeconds: 1,
    rpcRequestsTotal: 0,
    rpcErrorsTotal: 0,
    workMode: "serving",
    stakeMist: 0,
    ...overrides
  };
}
