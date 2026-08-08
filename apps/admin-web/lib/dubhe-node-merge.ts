import type {
  DubheNodeConsoleSnapshot,
  DubheNodeRecord
} from "./dubhe-node";

export function mergeDubheNodeOperatorRecords(
  homeSnapshot: DubheNodeConsoleSnapshot,
  operatorNodes: DubheNodeRecord[],
  configuredOperatorCount: number,
  generatedAtMs = Date.now()
): DubheNodeConsoleSnapshot {
  if (configuredOperatorCount === 0) {
    return homeSnapshot;
  }

  const nodesById = new Map(
    homeSnapshot.nodes.map((node) => [node.nodeId, node])
  );
  for (const node of operatorNodes) {
    nodesById.set(node.nodeId, node);
  }
  const nodes = [...nodesById.values()];
  const liveNodes = nodes.filter((node) => node.telemetryState === "live");
  const liveOperatorCount = operatorNodes.filter(
    (node) => node.telemetryState === "live"
  ).length;
  const allNodesLive =
    nodes.length > 0 && liveNodes.length === nodes.length;
  const allOperatorsLive = liveOperatorCount === configuredOperatorCount;
  const mode =
    liveNodes.length === 0
      ? "offline"
      : allNodesLive && allOperatorsLive
        ? "live"
        : "degraded";

  return {
    ...homeSnapshot,
    generatedAtMs,
    mode,
    registeredNodeCount: Math.max(
      homeSnapshot.registeredNodeCount,
      nodes.length
    ),
    liveNodeCount: liveNodes.length,
    totalSessions: sum(liveNodes.map((node) => node.sessions)),
    totalSessionCapacity: sum(
      liveNodes.map((node) => node.sessionCapacity)
    ),
    totalZones: sum(liveNodes.map((node) => node.zones)),
    totalZoneCapacity: sum(liveNodes.map((node) => node.zoneCapacity)),
    totalStakeMist: nodes
      .filter((node) => node.registrationState === "active")
      .reduce((total, node) => total + node.stakeMist, 0),
    nodes,
    sourceNote: `${homeSnapshot.sourceNote} ${liveOperatorCount}/${configuredOperatorCount} configured official Zone Hosts are live.`
  };
}

function sum(values: number[]) {
  return values.reduce((total, value) => total + value, 0);
}
