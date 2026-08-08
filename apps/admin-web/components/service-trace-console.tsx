"use client";

import { FormEvent, useCallback, useEffect, useMemo, useState } from "react";
import type {
  AdminServiceTraceReadModel,
  AdminServiceTracePlayer
} from "../lib/admin-api";

type ServiceNodeView = {
  nodeId: string;
  label: string;
  advertisedEndpoint: string;
  failureDomain: string;
  telemetryState: "live" | "offline";
  registrationState: "active" | "unregistered";
  sessions: number;
  sessionCapacity: number;
  zones: number;
  zoneCapacity: number;
  activeConnections: number;
  draining: boolean;
  observedAtMs?: number;
  activeZones: Array<{
    zoneId: string;
    mapScope: "all" | "explicit" | "unknown";
    mapFileNames: string[];
    sessionCount: number;
  }>;
};

type ServiceTracePayload = {
  trace: AdminServiceTraceReadModel;
  node?: ServiceNodeView;
  nodeLookup: {
    status: "matched" | "not_found" | "unavailable" | "not_applicable";
    message: string;
  };
};

type Copy = {
  title: string;
  subtitle: string;
  placeholder: string;
  search: string;
  refreshing: string;
  sensitive: string;
  sensitiveHint: string;
  noQuery: string;
  currentPath: string;
  nodeTelemetry: string;
  history: string;
  diagnostics: string;
  audit: string;
  candidates: string;
};

const copyByLocale: Record<"en" | "zh-CN", Copy> = {
  en: {
    title: "Player Service Trace",
    subtitle:
      "Resolve a player to the live Gateway, Commonware lease, Relay, service node, Zone, map and retained failover history.",
    placeholder: "Account, character name, player ID or object ID",
    search: "Trace player",
    refreshing: "Refreshing",
    sensitive: "Reveal protected endpoints",
    sensitiveHint: "Requires server_control and every query is audited.",
    noQuery: "Enter a player identity to inspect the complete serving path.",
    currentPath: "Current serving path",
    nodeTelemetry: "Matched node telemetry",
    history: "Placement and failover timeline",
    diagnostics: "Source diagnostics",
    audit: "Audit trace",
    candidates: "Choose an exact player"
  },
  "zh-CN": {
    title: "玩家服务链路追踪",
    subtitle:
      "从玩家定位到实时 Gateway、Commonware 租约、Relay、实际服务节点、Zone、地图/分线及故障迁移历史。",
    placeholder: "账号、角色名、角色 ID 或对象 ID",
    search: "查询玩家",
    refreshing: "正在刷新",
    sensitive: "显示受保护端点",
    sensitiveHint: "需要 server_control 权限，所有查询都会进入审计日志。",
    noQuery: "输入一个玩家身份，即可查看完整服务链路。",
    currentPath: "当前服务链路",
    nodeTelemetry: "匹配节点遥测",
    history: "Placement 与故障迁移时间线",
    diagnostics: "数据源诊断",
    audit: "审计追踪",
    candidates: "请选择精确角色"
  }
};

export function ServiceTraceConsole({
  locale,
  initialQuery = ""
}: {
  locale: "en" | "zh-CN";
  initialQuery?: string;
}) {
  const copy = copyByLocale[locale];
  const [input, setInput] = useState(initialQuery);
  const [activeQuery, setActiveQuery] = useState(initialQuery.trim());
  const [sensitive, setSensitive] = useState(false);
  const [payload, setPayload] = useState<ServiceTracePayload>();
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const [lastUpdatedAt, setLastUpdatedAt] = useState<number>();

  const load = useCallback(
    async (query: string, signal?: AbortSignal) => {
      if (query.trim().length < 2) return;
      setLoading(true);
      setError("");
      try {
        const response = await fetch(
          `/api/service-trace?query=${encodeURIComponent(query.trim())}&sensitive=${sensitive ? "true" : "false"}`,
          { cache: "no-store", signal }
        );
        const data = (await response.json()) as ServiceTracePayload & {
          error?: string;
        };
        if (!response.ok) {
          throw new Error(data.error ?? `HTTP ${response.status}`);
        }
        setPayload(data);
        setLastUpdatedAt(Date.now());
      } catch (cause) {
        if (cause instanceof DOMException && cause.name === "AbortError") return;
        setError(cause instanceof Error ? cause.message : "Service trace unavailable");
      } finally {
        if (!signal?.aborted) setLoading(false);
      }
    },
    [sensitive]
  );

  useEffect(() => {
    if (!activeQuery) return;
    const controller = new AbortController();
    void load(activeQuery, controller.signal);
    const timer = window.setInterval(() => {
      void load(activeQuery);
    }, 10_000);
    return () => {
      controller.abort();
      window.clearInterval(timer);
    };
  }, [activeQuery, load]);

  const path = useMemo(
    () => servicePath(payload?.trace),
    [payload?.trace]
  );

  function submit(event: FormEvent) {
    event.preventDefault();
    const query = input.trim();
    if (query.length < 2) {
      setError("请输入至少 2 个字符。");
      return;
    }
    setActiveQuery(query);
    window.history.replaceState(
      null,
      "",
      `/service-trace?query=${encodeURIComponent(query)}`
    );
  }

  function choose(player: AdminServiceTracePlayer) {
    setInput(player.characterName);
    setActiveQuery(player.characterName);
  }

  return (
    <div className="trace-console">
      <section className="trace-hero">
        <div>
          <div className="trace-kicker">
            <span className="trace-pulse" />
            DUBHE SESSION PLACEMENT
          </div>
          <h2>{copy.title}</h2>
          <p>{copy.subtitle}</p>
        </div>
        <div className="trace-live">
          <span className={loading ? "trace-spinner spinning" : "trace-spinner"}>↻</span>
          <strong>{loading ? copy.refreshing : statusLabel(payload?.trace.status, locale)}</strong>
          <small>{lastUpdatedAt ? formatTime(lastUpdatedAt) : "10s live poll"}</small>
        </div>
      </section>

      <section className="trace-search-panel">
        <form className="trace-search" onSubmit={submit}>
          <input
            aria-label={copy.placeholder}
            className="control"
            onChange={(event) => setInput(event.target.value)}
            placeholder={copy.placeholder}
            value={input}
          />
          <button
            aria-busy={loading}
            className="dubhe-button"
            disabled={loading}
            type="submit"
          >
            {loading ? <span aria-hidden="true" className="button-spinner" /> : null}
            <span aria-live="polite">{loading ? copy.refreshing : copy.search}</span>
          </button>
        </form>
        <label className="trace-sensitive">
          <input
            checked={sensitive}
            onChange={(event) => setSensitive(event.target.checked)}
            type="checkbox"
          />
          <span>
            <strong>{copy.sensitive}</strong>
            <small>{copy.sensitiveHint}</small>
          </span>
        </label>
      </section>

      {error ? <div className="trace-notice danger">{error}</div> : null}
      {!activeQuery ? <div className="trace-empty">{copy.noQuery}</div> : null}
      {payload?.trace.reason ? (
        <div className={`trace-notice ${tone(payload.trace.status)}`}>
          <strong>{statusLabel(payload.trace.status, locale)}</strong>
          <span>{payload.trace.reason}</span>
        </div>
      ) : null}

      {payload?.trace.status === "ambiguous" ? (
        <section className="trace-section">
          <div className="trace-section-head">
            <div>
              <span>IDENTITY RESOLUTION</span>
              <h3>{copy.candidates}</h3>
            </div>
          </div>
          <div className="trace-candidates">
            {payload.trace.candidates.map((player) => (
              <button
                className="trace-candidate"
                key={`${player.accountId}:${player.characterIndex}`}
                onClick={() => choose(player)}
                type="button"
              >
                <strong>{player.characterName}</strong>
                <span>{player.playerId}</span>
                <small>{player.online ? "ONLINE" : "OFFLINE"} · MAP {player.mapFileName}</small>
              </button>
            ))}
          </div>
        </section>
      ) : null}

      {payload?.trace.player ? (
        <>
          <section className="trace-section">
            <div className="trace-section-head">
              <div>
                <span>LIVE REQUEST PATH</span>
                <h3>{copy.currentPath}</h3>
              </div>
              <div className={`trace-state ${tone(payload.trace.status)}`}>
                {statusLabel(payload.trace.status, locale)}
              </div>
            </div>
            <div className="trace-path">
              {path.map((step, index) => (
                <div className={`trace-hop ${step.ready ? "ready" : "missing"}`} key={step.label}>
                  <div className="trace-hop-index">{String(index + 1).padStart(2, "0")}</div>
                  <div>
                    <small>{step.label}</small>
                    <strong>{step.value}</strong>
                    <span>{step.meta}</span>
                  </div>
                </div>
              ))}
            </div>
          </section>

          <div className="trace-grid">
            <section className="trace-section">
              <div className="trace-section-head">
                <div>
                  <span>NODE OBSERVABILITY</span>
                  <h3>{copy.nodeTelemetry}</h3>
                </div>
              </div>
              {payload.node ? (
                <NodeTelemetry node={payload.node} />
              ) : (
                <div className="trace-empty compact">{payload.nodeLookup.message}</div>
              )}
            </section>

            <section className="trace-section">
              <div className="trace-section-head">
                <div>
                  <span>QUERY GOVERNANCE</span>
                  <h3>{copy.audit}</h3>
                </div>
              </div>
              <dl className="trace-facts">
                <Fact label="Trace ID" value={payload.trace.auditTraceId} mono />
                <Fact
                  label="Privacy"
                  value={payload.trace.sensitiveRedacted ? "默认脱敏" : "已授权显示"}
                />
                <Fact label="Identity" value={payload.trace.player.playerId} mono />
                <Fact
                  label="Generated"
                  value={formatTimestamp(payload.trace.generatedAtMs)}
                />
              </dl>
            </section>
          </div>

          <section className="trace-section">
            <div className="trace-section-head">
              <div>
                <span>SESSION HISTORY</span>
                <h3>{copy.history}</h3>
              </div>
              <small>{payload.trace.history.length} events / newest first</small>
            </div>
            {payload.trace.history.length ? (
              <div className="trace-timeline">
                {payload.trace.history.map((event) => (
                  <article key={event.eventId}>
                    <div className="trace-timeline-dot" />
                    <time>{formatTimestamp(event.occurredAtMs)}</time>
                    <div>
                      <strong>{eventLabel(event.eventType, locale)}</strong>
                      <p>{event.reason}</p>
                      <small>
                        {[
                          event.gatewayId,
                          event.relayId,
                          event.serviceNodeId,
                          event.zoneId,
                          event.mapFileName ? `map ${event.mapFileName}` : undefined,
                          event.lineId !== undefined ? `line ${event.lineId}` : undefined,
                          event.zoneOwnerFencingToken !== undefined
                            ? `fence ${event.zoneOwnerFencingToken}`
                            : undefined
                        ]
                          .filter(Boolean)
                          .join(" · ")}
                      </small>
                    </div>
                  </article>
                ))}
              </div>
            ) : (
              <div className="trace-empty compact">暂无保留的 Session placement 历史。</div>
            )}
          </section>

          <section className="trace-section">
            <div className="trace-section-head">
              <div>
                <span>DATA PROVENANCE</span>
                <h3>{copy.diagnostics}</h3>
              </div>
            </div>
            <div className="trace-diagnostics">
              {payload.trace.diagnostics.map((item) => (
                <div key={item.component}>
                  <span className={`trace-source-dot ${item.status}`} />
                  <strong>{item.component}</strong>
                  <small>{item.message}</small>
                </div>
              ))}
              <div>
                <span className={`trace-source-dot ${payload.nodeLookup.status}`} />
                <strong>dubhe_node_telemetry</strong>
                <small>{payload.nodeLookup.message}</small>
              </div>
            </div>
          </section>
        </>
      ) : null}
    </div>
  );
}

function NodeTelemetry({ node }: { node: ServiceNodeView }) {
  return (
    <div>
      <div className="trace-node-title">
        <div className={`trace-node-orb ${node.telemetryState}`} />
        <div>
          <strong>{node.label}</strong>
          <small>{node.nodeId}</small>
        </div>
        <span>{node.telemetryState.toUpperCase()}</span>
      </div>
      <dl className="trace-facts two-column">
        <Fact label="Sessions" value={`${node.sessions} / ${node.sessionCapacity}`} />
        <Fact label="Zones" value={`${node.zones} / ${node.zoneCapacity}`} />
        <Fact label="Connections" value={String(node.activeConnections)} />
        <Fact label="Failure domain" value={node.failureDomain || "-"} />
        <Fact label="Admission" value={node.registrationState} />
        <Fact label="Draining" value={node.draining ? "yes" : "no"} />
      </dl>
      <div className="trace-zone-list">
        {node.activeZones.map((zone) => (
          <div key={zone.zoneId}>
            <strong>{zone.zoneId}</strong>
            <span>{zone.sessionCount} sessions</span>
            <small>{zone.mapFileNames.join(", ") || zone.mapScope}</small>
          </div>
        ))}
      </div>
    </div>
  );
}

function Fact({
  label,
  value,
  mono = false
}: {
  label: string;
  value: string;
  mono?: boolean;
}) {
  return (
    <div>
      <dt>{label}</dt>
      <dd className={mono ? "mono" : ""}>{value || "-"}</dd>
    </div>
  );
}

function servicePath(trace?: AdminServiceTraceReadModel) {
  const player = trace?.player;
  const current = trace?.current;
  const commonware = trace?.commonware;
  return [
    {
      label: "PLAYER",
      value: player?.characterName ?? "Unresolved",
      meta: player ? `${player.playerId} · object ${player.playerObjectId ?? "-"}` : "identity",
      ready: Boolean(player)
    },
    {
      label: "GATEWAY",
      value: current?.gatewayId ?? "No live Gateway",
      meta: current?.gatewaySessionId ?? "session unavailable",
      ready: Boolean(current?.gatewayId)
    },
    {
      label: "COMMONWARE",
      value: commonware ? `Height ${commonware.finalizedHeight}` : "Lease unavailable",
      meta: commonware
        ? `generation ${commonware.generation} · fence ${commonware.sessionLease?.fencingToken ?? current?.zoneOwnerFencingToken ?? "-"}`
        : "finalized placement",
      ready: Boolean(commonware)
    },
    {
      label: "RELAY",
      value: current?.relayId ?? "Direct / unknown",
      meta: current?.relayEndpoint ?? "no relay metadata",
      ready: Boolean(current?.relayId || current?.relayEndpoint)
    },
    {
      label: "SERVICE NODE",
      value: current?.serviceNodeId ?? commonware?.primaryHostId ?? "Unknown node",
      meta: current?.nodeKind ?? "node kind unavailable",
      ready: Boolean(current?.serviceNodeId || commonware?.primaryHostId)
    },
    {
      label: "ZONE / MAP",
      value: current?.zoneId ?? "No active Zone",
      meta: [
        current?.mapFileName ? `map ${current.mapFileName}` : undefined,
        current?.lineId !== undefined ? `line ${current.lineId}` : undefined,
        current ? `tick ${current.tick}` : undefined
      ]
        .filter(Boolean)
        .join(" · "),
      ready: Boolean(current?.zoneId)
    }
  ];
}

function statusLabel(status: AdminServiceTraceReadModel["status"] | undefined, locale: string) {
  const zh: Record<string, string> = {
    online: "在线且链路完整",
    degraded: "在线但链路降级",
    stale: "Session 已过期",
    offline: "玩家离线",
    no_runtime_record: "尚无运行态记录",
    not_found: "未找到玩家",
    ambiguous: "命中多个角色",
    unavailable: "数据源不可用"
  };
  const en: Record<string, string> = {
    online: "Online · complete",
    degraded: "Online · degraded",
    stale: "Stale session",
    offline: "Player offline",
    no_runtime_record: "No runtime record",
    not_found: "Player not found",
    ambiguous: "Multiple matches",
    unavailable: "Source unavailable"
  };
  return (locale === "zh-CN" ? zh : en)[status ?? ""] ?? "Ready";
}

function tone(status: string) {
  if (status === "online" || status === "ready" || status === "matched") return "success";
  if (status === "unavailable" || status === "not_found") return "danger";
  return "warn";
}

function eventLabel(eventType: string, locale: string) {
  const zh: Record<string, string> = {
    session_started: "玩家进入世界",
    placement_assigned: "首次服务分配",
    session_reconnected: "Gateway Session 重连",
    map_transfer: "地图或分线切换",
    placement_changed: "服务节点迁移 / 故障切换",
    relay_changed: "Relay 路由变化",
    disconnected: "Session 断开"
  };
  const en: Record<string, string> = {
    session_started: "Session started",
    placement_assigned: "Initial placement",
    session_reconnected: "Gateway session reconnected",
    map_transfer: "Map or line transfer",
    placement_changed: "Placement changed / failover",
    relay_changed: "Relay route changed",
    disconnected: "Session disconnected"
  };
  return (locale === "zh-CN" ? zh : en)[eventType] ?? eventType;
}

function formatTimestamp(value: number) {
  return value ? new Date(value).toLocaleString() : "-";
}

function formatTime(value: number) {
  return new Date(value).toLocaleTimeString();
}
