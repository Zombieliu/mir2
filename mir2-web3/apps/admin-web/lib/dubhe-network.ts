import "server-only";

import {
  adminGet,
  type AdminCommonwarePlacement
} from "./admin-api";
import { readDubheNodeConsole } from "./dubhe-node";
import {
  buildDubheNetworkSnapshot,
  type DubheNetworkSnapshot
} from "./dubhe-network-model";

export async function readDubheNetwork(): Promise<DubheNetworkSnapshot> {
  const [fleet, commonware] = await Promise.all([
    readDubheNodeConsole(),
    adminGet<AdminCommonwareNetworkReadModel>("/admin/read/commonware-network")
  ]);
  const snapshot = buildDubheNetworkSnapshot(fleet);
  if (commonware.ok && commonware.data.placement) {
    const placement = commonware.data.placement;
    snapshot.totals.commonwareFinalizedHeight = placement.finalizedHeight;
    snapshot.commonware = {
      status: "live",
      source: placement.source,
      finalizedHeight: placement.finalizedHeight,
      gatewayId: placement.gatewayId,
      generation: placement.generation,
      primaryHostId: placement.primaryHostId
    };
  } else if (commonware.ok) {
    snapshot.commonware = {
      status: "unavailable",
      source: "admin-api",
      finalizedHeight: snapshot.totals.commonwareFinalizedHeight,
      error: commonware.data.error
    };
  }
  return snapshot;
}

type AdminCommonwareNetworkReadModel = {
  generatedAtMs: number;
  status: "live" | "unavailable";
  error?: string;
  placement?: AdminCommonwarePlacement;
};

export type {
  DubheNetworkNode,
  DubheNetworkRegion,
  DubheNetworkSnapshot
} from "./dubhe-network-model";
