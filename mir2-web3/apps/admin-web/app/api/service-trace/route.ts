import { NextResponse } from "next/server";
import {
  adminGet,
  type AdminServiceTraceReadModel
} from "../../../lib/admin-api";
import {
  readDubheNodeConsole,
  type DubheNodeRecord
} from "../../../lib/dubhe-node";

export const dynamic = "force-dynamic";

export type ServiceTraceApiResponse = {
  trace: AdminServiceTraceReadModel;
  node?: DubheNodeRecord;
  nodeLookup: {
    status: "matched" | "not_found" | "unavailable" | "not_applicable";
    message: string;
  };
};

export async function GET(request: Request) {
  const url = new URL(request.url);
  const query = url.searchParams.get("query")?.trim() ?? "";
  const sensitive = url.searchParams.get("sensitive") === "true";
  if (query.length < 2 || query.length > 128) {
    return NextResponse.json(
      { error: "query must contain between 2 and 128 characters" },
      { status: 400 }
    );
  }

  const response = await adminGet<AdminServiceTraceReadModel>(
    `/admin/read/service-trace?query=${encodeURIComponent(query)}&sensitive=${sensitive ? "true" : "false"}&historyLimit=64`
  );
  if (!response.ok) {
    return NextResponse.json(
      { error: response.error },
      { status: response.status ?? 502 }
    );
  }

  const nodeId = response.data.current?.serviceNodeId;
  const zoneId = response.data.current?.zoneId;
  if (!nodeId && !zoneId) {
    return NextResponse.json({
      trace: response.data,
      nodeLookup: {
        status: "not_applicable",
        message: "当前没有可用于匹配节点的在线 placement。"
      }
    } satisfies ServiceTraceApiResponse);
  }

  try {
    const fleet = await readDubheNodeConsole();
    const matchedNode = fleet.nodes.find(
      (candidate) =>
        candidate.nodeId === nodeId ||
        candidate.activeZones.some((zone) => zone.zoneId === zoneId)
    );
    const node = matchedNode
      ? {
          ...matchedNode,
          advertisedEndpoint: redactNodeEndpoint(
            matchedNode.advertisedEndpoint,
            sensitive
          )
        }
      : undefined;
    return NextResponse.json({
      trace: response.data,
      node,
      nodeLookup: node
        ? {
            status: "matched",
            message: `已从 ${fleet.mode} 遥测快照匹配服务节点。`
          }
        : {
            status: "not_found",
            message: "placement 存在，但当前遥测快照未找到对应节点。"
          }
    } satisfies ServiceTraceApiResponse);
  } catch (error) {
    return NextResponse.json({
      trace: response.data,
      nodeLookup: {
        status: "unavailable",
        message:
          error instanceof Error
            ? error.message
            : "Dubhe Node 遥测暂不可用。"
      }
    } satisfies ServiceTraceApiResponse);
  }
}

function redactNodeEndpoint(value: string, sensitive: boolean) {
  if (sensitive) return value;
  try {
    const normalized = value.includes("://") ? value : `tcp://${value}`;
    const url = new URL(normalized);
    const host = url.hostname;
    if (
      host === "localhost" ||
      host.endsWith(".local") ||
      /^(?:\d{1,3}\.){3}\d{1,3}$/.test(host) ||
      host.includes(":")
    ) {
      return "private-endpoint";
    }
    return host;
  } catch {
    return "private-endpoint";
  }
}
