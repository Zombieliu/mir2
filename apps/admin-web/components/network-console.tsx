"use client";

import Link from "next/link";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useRouter } from "next/navigation";
import type {
  DubheNetworkRegion,
  DubheNetworkSnapshot
} from "../lib/dubhe-network";
import { NetworkGlobe } from "./network-globe";

const REFRESH_INTERVAL_MS = 5_000;

type NetworkConsoleProps = {
  initialSnapshot: DubheNetworkSnapshot;
  initialRegionCode?: string;
  locale: "en" | "zh-CN";
};

export function NetworkConsole({
  initialSnapshot,
  initialRegionCode,
  locale
}: NetworkConsoleProps) {
  const router = useRouter();
  const [snapshot, setSnapshot] = useState(initialSnapshot);
  const [selectedCode, setSelectedCode] = useState(initialRegionCode);
  const [refreshing, setRefreshing] = useState(false);
  const [refreshError, setRefreshError] = useState<string>();
  const selectedRegion = useMemo(
    () => snapshot.regions.find((region) => region.code === selectedCode),
    [selectedCode, snapshot.regions]
  );

  const refresh = useCallback(async (signal?: AbortSignal) => {
    setRefreshing(true);
    try {
      const response = await fetch("/api/network", {
        cache: "no-store",
        signal
      });
      if (!response.ok) throw new Error(`HTTP ${response.status}`);
      setSnapshot((await response.json()) as DubheNetworkSnapshot);
      setRefreshError(undefined);
    } catch (cause) {
      if (cause instanceof DOMException && cause.name === "AbortError") return;
      setRefreshError(
        cause instanceof Error ? cause.message : "Network snapshot unavailable"
      );
    } finally {
      setRefreshing(false);
    }
  }, []);

  useEffect(() => {
    const controller = new AbortController();
    const timer = window.setInterval(
      () => void refresh(controller.signal),
      REFRESH_INTERVAL_MS
    );
    return () => {
      controller.abort();
      window.clearInterval(timer);
    };
  }, [refresh]);

  const selectRegion = useCallback(
    (code: string) => {
      setSelectedCode(code);
      router.push(`/network/region/${encodeURIComponent(code)}`, {
        scroll: false
      });
    },
    [router]
  );

  const copy = locale === "zh-CN" ? zhCopy : enCopy;
  const totals = snapshot.totals;
  const isLive = snapshot.mode === "live";

  return (
    <div className="network-console">
      <section className="network-stage">
        <div className="network-intro">
          <p className="network-live">
            <span aria-hidden="true" />
            {isLive ? copy.live : copy.degraded}
          </p>
          <h2>
            {copy.titleLineOne}
            <span>{copy.titleLineTwo}</span>
          </h2>
          <p>{copy.subtitle}</p>
          <div className="network-intro-actions">
            <button
              aria-busy={refreshing}
              className="network-action"
              disabled={refreshing}
              onClick={() => void refresh()}
              type="button"
            >
              {refreshing ? <span aria-hidden="true" className="button-spinner" /> : null}
              <span aria-live="polite">
                {refreshing ? copy.refreshing : copy.refresh}
              </span>
            </button>
            <Link className="network-action ghost" href="/service-trace">
              {copy.tracePlayer}
            </Link>
          </div>
          {refreshError ? (
            <p className="network-error">{copy.refreshFailed}: {refreshError}</p>
          ) : null}
        </div>

        <div className="network-globe-wrap">
          <NetworkGlobe
            onSelect={selectRegion}
            regions={snapshot.regions}
            selectedCode={selectedCode}
          />
          <div className="network-globe-hint">{copy.dragHint}</div>
        </div>

        <aside className="network-stat-stack">
          <NetworkStat
            label={copy.liveNodes}
            primary
            value={totals.liveNodes.toLocaleString()}
          />
          <div className="network-stat-pair">
            <NetworkStat
              label={copy.activePlayers}
              value={totals.activeSessions.toLocaleString()}
            />
            <NetworkStat
              label={copy.activeZones}
              value={totals.activeZones.toLocaleString()}
            />
          </div>
          <div className="network-stat-pair">
            <NetworkStat
              label={copy.capacity}
              value={totals.sessionCapacity.toLocaleString()}
            />
            <NetworkStat
              label={copy.relayRtt}
              suffix="ms"
              value={formatMetric(totals.averageRelayRttMs)}
            />
          </div>
          <NetworkStat
            detail={
              snapshot.commonware.status === "live"
                ? `${copy.liveSource} · ${snapshot.commonware.gatewayId ?? snapshot.commonware.source}`
                : snapshot.commonware.status === "evidence"
                  ? copy.acceptanceEvidence
                  : copy.sourceUnavailable
            }
            label={copy.finalizedHeight}
            value={totals.commonwareFinalizedHeight.toLocaleString()}
          />
        </aside>

        <div className="network-stage-footer">
          <span><i className="serving" />{totals.servingNodes} {copy.serving}</span>
          <span><i className="draining" />{totals.drainingNodes} {copy.draining}</span>
          <span>{totals.locatedRegions} {copy.regions}</span>
          <span>{totals.unlocatedNodes} {copy.privateRegion}</span>
          <span className="network-updated">
            {copy.updated} {formatTime(snapshot.generatedAtMs, locale)}
          </span>
        </div>
      </section>

      {selectedRegion ? (
        <RegionDetail
          copy={copy}
          locale={locale}
          onClose={() => {
            setSelectedCode(undefined);
            router.push("/network", { scroll: false });
          }}
          region={selectedRegion}
        />
      ) : (
        <section className="network-region-index">
          <div>
            <p className="network-section-label">{copy.regionalLayer}</p>
            <h3>{copy.chooseRegion}</h3>
          </div>
          <div className="network-region-grid">
            {snapshot.regions.map((region) => (
              <button
                key={region.code}
                onClick={() => selectRegion(region.code)}
                type="button"
              >
                <span>
                  <i className={region.servingNodes > 0 ? "serving" : "draining"} />
                  {region.code}
                </span>
                <strong>{region.label}</strong>
                <small>
                  {region.liveNodes} {copy.nodes} · {region.activeSessions} {copy.players}
                </small>
              </button>
            ))}
          </div>
        </section>
      )}

      <section className="network-privacy">
        <span aria-hidden="true">⌁</span>
        <div>
          <strong>{copy.privacyTitle}</strong>
          <p>{snapshot.privacy.note}</p>
        </div>
      </section>
    </div>
  );
}

function RegionDetail({
  copy,
  locale,
  onClose,
  region
}: {
  copy: typeof zhCopy;
  locale: "en" | "zh-CN";
  onClose: () => void;
  region: DubheNetworkRegion;
}) {
  return (
    <section className="network-region-detail">
      <header>
        <div>
          <p className="network-section-label">{copy.regionalLayer} · {region.code}</p>
          <h3>{region.label}</h3>
          <span>
            {region.nodeLocationKnown ? copy.nodeReported : copy.relayFallback}
          </span>
        </div>
        <button className="network-close" onClick={onClose} type="button">
          {copy.returnGlobal}
        </button>
      </header>
      <div className="network-region-metrics">
        <RegionMetric label={copy.liveNodes} value={region.liveNodes} />
        <RegionMetric label={copy.activePlayers} value={region.activeSessions} />
        <RegionMetric label={copy.capacity} value={region.sessionCapacity} />
        <RegionMetric label={copy.activeZones} value={region.activeZones} />
        <RegionMetric
          label={copy.relayRtt}
          suffix=" ms"
          value={formatMetric(region.averageRelayRttMs)}
        />
        <RegionMetric
          label={copy.packetLoss}
          suffix=" bps"
          value={formatMetric(region.averagePacketLossBps)}
        />
      </div>
      <div className="network-node-table">
        <div className="network-node-row heading">
          <span>{copy.serviceNode}</span>
          <span>{copy.state}</span>
          <span>{copy.sessions}</span>
          <span>{copy.zoneMap}</span>
          <span>{copy.telemetry}</span>
        </div>
        {region.nodes.map((node) => {
          const status =
            node.telemetryState === "offline"
              ? "offline"
              : node.workMode === "draining"
                ? "draining"
                : "serving";
          return (
            <div className="network-node-row" key={node.nodeId}>
              <span>
                <strong>{node.label}</strong>
                <small>{node.providerCode ?? "home"} · {node.agentVersion ?? "agent"}</small>
              </span>
              <span className={`network-node-state ${status}`}>
                <i />
                {status}
              </span>
              <span>{node.sessions} / {node.sessionCapacity}</span>
              <span>
                <strong>{node.zoneIds.join(", ") || copy.noActiveZone}</strong>
                <small>{node.mapFileNames.join(", ") || copy.allMaps}</small>
              </span>
              <span>
                <strong>{formatMetric(node.relayRttMs)} ms</strong>
                <small>{formatTime(node.observedAtMs, locale)}</small>
              </span>
            </div>
          );
        })}
      </div>
      <footer>
        <p>{copy.tracePrompt}</p>
        <Link className="network-action" href="/service-trace">
          {copy.openServiceTrace}
        </Link>
      </footer>
    </section>
  );
}

function NetworkStat({
  label,
  detail,
  primary,
  suffix,
  value
}: {
  label: string;
  detail?: string;
  primary?: boolean;
  suffix?: string;
  value: string;
}) {
  return (
    <article className={primary ? "network-stat primary" : "network-stat"}>
      <span>{label}</span>
      <strong>{value}<small>{suffix}</small></strong>
      {detail ? <em>{detail}</em> : null}
    </article>
  );
}

function RegionMetric({
  label,
  suffix,
  value
}: {
  label: string;
  suffix?: string;
  value: number | string;
}) {
  return (
    <article>
      <span>{label}</span>
      <strong>{value}{suffix}</strong>
    </article>
  );
}

function formatMetric(value?: number) {
  return value === undefined ? "—" : value.toFixed(value >= 100 ? 0 : 1);
}

function formatTime(value: number | undefined, locale: "en" | "zh-CN") {
  if (!value) return "—";
  return new Intl.DateTimeFormat(locale, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit"
  }).format(value);
}

const zhCopy = {
  live: "实时 · DUBHE NETWORK",
  degraded: "降级 · DUBHE NETWORK",
  titleLineOne: "看见游戏世界",
  titleLineTwo: "如何被共同运行。",
  subtitle:
    "每一个光点代表一个真实服务区域。节点、Relay、地图与玩家 Session 通过签名遥测实时汇聚。",
  refresh: "刷新实时数据",
  refreshing: "正在刷新…",
  tracePlayer: "追踪玩家链路",
  refreshFailed: "刷新失败",
  dragHint: "拖动旋转 · 点击光点查看区域",
  liveNodes: "在线节点",
  activePlayers: "活跃玩家",
  activeZones: "活跃 Zone",
  capacity: "Session 容量",
  relayRtt: "平均 Relay RTT",
  finalizedHeight: "Commonware Finalized Height",
  liveSource: "实时",
  acceptanceEvidence: "验收证据快照",
  sourceUnavailable: "实时数据源不可用",
  serving: "贡献中",
  draining: "Draining",
  regions: "个服务区域",
  privateRegion: "个位置受保护节点",
  updated: "更新于",
  regionalLayer: "区域视图",
  chooseRegion: "选择一个服务区域",
  nodes: "节点",
  players: "玩家",
  privacyTitle: "家庭 IP 不进入产品遥测",
  nodeReported: "节点主动选择的粗粒度区域",
  relayFallback: "家庭区域未知，仅显示官方 Relay 所在区域",
  returnGlobal: "返回全球",
  packetLoss: "平均丢包",
  serviceNode: "服务节点",
  state: "状态",
  sessions: "Sessions",
  zoneMap: "Zone / 地图",
  telemetry: "实时遥测",
  noActiveZone: "暂无活跃 Zone",
  allMaps: "通用地图范围",
  tracePrompt: "需要确认某个玩家此刻由哪个节点、Zone 和 Relay 提供服务？",
  openServiceTrace: "打开玩家服务追踪"
};

const enCopy: typeof zhCopy = {
  live: "LIVE · DUBHE NETWORK",
  degraded: "DEGRADED · DUBHE NETWORK",
  titleLineOne: "See how the game world",
  titleLineTwo: "is run together.",
  subtitle:
    "Every light represents a real service region. Signed telemetry connects nodes, Relays, maps, and player Sessions in real time.",
  refresh: "Refresh live data",
  refreshing: "Refreshing…",
  tracePlayer: "Trace a player",
  refreshFailed: "Refresh failed",
  dragHint: "Drag to rotate · select a light for regional detail",
  liveNodes: "Live nodes",
  activePlayers: "Active players",
  activeZones: "Active Zones",
  capacity: "Session capacity",
  relayRtt: "Average Relay RTT",
  finalizedHeight: "Commonware Finalized Height",
  liveSource: "live",
  acceptanceEvidence: "acceptance evidence snapshot",
  sourceUnavailable: "live source unavailable",
  serving: "serving",
  draining: "draining",
  regions: "service regions",
  privateRegion: "location-protected nodes",
  updated: "Updated",
  regionalLayer: "Regional view",
  chooseRegion: "Choose a service region",
  nodes: "nodes",
  players: "players",
  privacyTitle: "Home IPs never enter product telemetry",
  nodeReported: "Coarse region selected by the node operator",
  relayFallback: "Home region unknown; showing the official Relay region only",
  returnGlobal: "Return global",
  packetLoss: "Average packet loss",
  serviceNode: "Service node",
  state: "State",
  sessions: "Sessions",
  zoneMap: "Zone / map",
  telemetry: "Live telemetry",
  noActiveZone: "No active Zone",
  allMaps: "General map scope",
  tracePrompt: "Need to confirm which node, Zone, and Relay serves a player right now?",
  openServiceTrace: "Open service trace"
};
