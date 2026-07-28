"use client";

import { useCallback, useEffect, useState } from "react";
import type {
  DubheNodeConsoleSnapshot,
  DubheNodeRecord,
  DubheNodeZoneRecord
} from "../lib/dubhe-node";

const REFRESH_INTERVAL_MS = 10_000;

const copy = {
  en: {
    eyebrow: "Distributed map compute fabric",
    title: "Dubhe Node",
    subtitle:
      "One operational view for node identity, live Zone capacity, Sui registration, Commonware finality, and verified rewards.",
    live: "Live telemetry",
    degraded: "Partial telemetry",
    offline: "Telemetry offline",
    refresh: "Refresh now",
    refreshing: "Refreshing...",
    auto: "Auto refresh 10s",
    liveNodes: "Live nodes",
    sessions: "Hosted sessions",
    zones: "Active zones",
    stake: "Registered stake",
    testnet: "Sui testnet",
    networkPath: "Admission path",
    registration: "Sui registration",
    finality: "Commonware finality",
    certificate: "Capacity certificate",
    rewards: "Reward eligibility",
    finalized: "Finalized",
    accepted: "Eligible",
    evidence: "Last acceptance",
    expired: "Expired",
    valid: "Valid",
    nodeFleet: "Node fleet",
    nodeFleetTitle: "Identity and runtime posture",
    online: "Online",
    registered: "Registered",
    unregistered: "Local only",
    heartbeat: "Heartbeat",
    verified: "Verified",
    notVerified: "Not verified",
    sessionsLabel: "Sessions",
    busiestZoneLabel: "Busiest Zone",
    zonesLabel: "Zones",
    hostedMaps: "Hosted map workloads",
    signedRuntime: "Signed runtime",
    allMaps: "All game maps",
    mapFiles: "Map files",
    activeRuntime: "Active runtime",
    noActiveZones: "No active Zone workloads on this node.",
    zoneDetailsUnavailable: "This node reports active Zones but has not published signed map details.",
    zoneSessions: "sessions",
    rpc: "RPC requests",
    errors: "Errors",
    uptime: "Uptime",
    generation: "Key generation",
    process: "Process",
    endpoint: "Endpoint",
    domain: "Failure domain",
    noLive: "The testnet identity is registered, but no matching operator endpoint is live.",
    chain: "Chain anchor",
    chainTitle: "Testnet deployment",
    package: "Package",
    registry: "Registry",
    checkpoint: "Checkpoint",
    operator: "Operator",
    openTransaction: "Open registration",
    openPackage: "Open package",
    evidenceTitle: "Verified-work evidence",
    capacityRun: "Capacity run",
    commands: "commands",
    latency: "p95 latency",
    rewardBatch: "Reward batch",
    rewardTotal: "reward units",
    commonware: "Commonware",
    quorum: "quorum",
    operations: "Operations",
    operationsTitle: "Observe and act safely",
    grafana: "Open Grafana",
    prometheus: "Open Prometheus",
    grafanaDetail: "Fleet history, node health, and Zone workload trends",
    prometheusDetail: "PromQL queries and scrape target status",
    alerts: "Open alert status",
    alertsDetail: "Active and pending Home Node alert rules",
    exportSnapshot: "Download live snapshot",
    exportSnapshotDetail: "Current authenticated node data as JSON",
    readOnly: "Read-only console",
    source: "Source boundary",
    sourceLive:
      "Runtime metrics are live. Chain membership and reward cards are committed Gate 13 evidence.",
    sourceOffline:
      "Runtime endpoints are offline. This view is showing committed testnet and acceptance evidence.",
    updated: "Updated",
    copyId: "Copy node ID",
    copied: "Copied"
  },
  "zh-CN": {
    eyebrow: "分布式地图计算网络",
    title: "Dubhe Node",
    subtitle:
      "在一个页面查看节点身份、实时 Zone 容量、Sui 注册、Commonware 最终确认与可验证奖励。",
    live: "实时遥测",
    degraded: "部分遥测",
    offline: "遥测离线",
    refresh: "立即刷新",
    refreshing: "刷新中…",
    auto: "每 10 秒自动刷新",
    liveNodes: "在线节点",
    sessions: "承载会话",
    zones: "活跃 Zone",
    stake: "注册质押",
    testnet: "Sui 测试网",
    networkPath: "准入路径",
    registration: "Sui 注册",
    finality: "Commonware 最终确认",
    certificate: "容量证书",
    rewards: "奖励资格",
    finalized: "已最终确认",
    accepted: "具备资格",
    evidence: "最近验收",
    expired: "已过期",
    valid: "有效",
    nodeFleet: "节点网络",
    nodeFleetTitle: "身份与运行态势",
    online: "在线",
    registered: "已注册",
    unregistered: "仅本地",
    heartbeat: "心跳",
    verified: "签名已验证",
    notVerified: "未验证",
    sessionsLabel: "会话",
    busiestZoneLabel: "最拥挤 Zone",
    zonesLabel: "Zone",
    hostedMaps: "正在承载的地图",
    signedRuntime: "签名运行态",
    allMaps: "全部游戏地图",
    mapFiles: "地图文件",
    activeRuntime: "活跃运行实例",
    noActiveZones: "该节点当前没有活跃 Zone 工作负载。",
    zoneDetailsUnavailable: "该节点报告了活跃 Zone，但尚未发布签名地图明细。",
    zoneSessions: "个会话",
    rpc: "RPC 请求",
    errors: "错误",
    uptime: "运行时间",
    generation: "密钥代际",
    process: "进程",
    endpoint: "服务端点",
    domain: "故障域",
    noLive: "该身份已在测试网注册，但当前没有匹配的在线遥测端点。",
    chain: "链上锚点",
    chainTitle: "测试网部署",
    package: "Package",
    registry: "Registry",
    checkpoint: "Checkpoint",
    operator: "运营地址",
    openTransaction: "查看注册交易",
    openPackage: "查看 Package",
    evidenceTitle: "可验证工作量证据",
    capacityRun: "容量验收",
    commands: "条命令",
    latency: "p95 延迟",
    rewardBatch: "奖励批次",
    rewardTotal: "奖励单位",
    commonware: "Commonware",
    quorum: "法定票",
    operations: "运维入口",
    operationsTitle: "安全地观察与操作",
    grafana: "打开 Grafana",
    prometheus: "打开 Prometheus",
    grafanaDetail: "节点历史、健康状态与 Zone 工作负载趋势",
    prometheusDetail: "PromQL 查询与采集目标状态",
    alerts: "查看告警状态",
    alertsDetail: "家庭节点当前及待触发告警规则",
    exportSnapshot: "下载实时快照",
    exportSnapshotDetail: "导出当前已认证节点 JSON 数据",
    readOnly: "只读控制台",
    source: "数据边界",
    sourceLive: "运行指标为实时数据；链上成员与奖励卡片来自已提交的 Gate 13 验收证据。",
    sourceOffline: "运行端点当前离线；页面正在展示已提交的测试网与验收证据。",
    updated: "更新时间",
    copyId: "复制节点 ID",
    copied: "已复制"
  }
} as const;

type DubheNodeConsoleProps = {
  initialSnapshot: DubheNodeConsoleSnapshot;
  locale: "en" | "zh-CN";
};

export function DubheNodeConsole({
  initialSnapshot,
  locale
}: DubheNodeConsoleProps) {
  const labels = copy[locale];
  const [snapshot, setSnapshot] = useState(initialSnapshot);
  const [refreshing, setRefreshing] = useState(false);
  const [autoRefresh, setAutoRefresh] = useState(true);
  const [copiedNodeId, setCopiedNodeId] = useState<string>();

  const refresh = useCallback(async (signal?: AbortSignal) => {
    setRefreshing(true);
    try {
      const response = await fetch("/api/dubhe-nodes", {
        cache: "no-store",
        signal
      });
      if (!response.ok) {
        throw new Error(`HTTP ${response.status}`);
      }
      setSnapshot((await response.json()) as DubheNodeConsoleSnapshot);
    } finally {
      setRefreshing(false);
    }
  }, []);

  useEffect(() => {
    if (!autoRefresh) {
      return;
    }
    const controller = new AbortController();
    const timer = window.setInterval(() => {
      void refresh(controller.signal).catch(() => undefined);
    }, REFRESH_INTERVAL_MS);
    return () => {
      controller.abort();
      window.clearInterval(timer);
    };
  }, [autoRefresh, refresh]);

  const handleCopy = useCallback(async (nodeId: string) => {
    await navigator.clipboard.writeText(nodeId);
    setCopiedNodeId(nodeId);
    window.setTimeout(() => setCopiedNodeId(undefined), 1_500);
  }, []);

  const modeLabel =
    snapshot.mode === "live"
      ? labels.live
      : snapshot.mode === "degraded"
        ? labels.degraded
        : labels.offline;
  const certificateValid = snapshot.capacity.certificateExpiresAtMs > snapshot.generatedAtMs;
  const registeredNode = snapshot.nodes.find((node) => node.registrationState === "active");

  return (
    <div className="dubhe-console">
      <header className="dubhe-hero">
        <div className="dubhe-hero-copy">
          <div className="dubhe-kicker">
            <span className="dubhe-star" aria-hidden="true">✦</span>
            <span>{labels.eyebrow}</span>
          </div>
          <h2>{labels.title}</h2>
          <p>{labels.subtitle}</p>
          <div className="dubhe-hero-meta">
            <span className={`dubhe-live-pill ${snapshot.mode}`}>
              <span className="dubhe-live-dot" aria-hidden="true" />
              {modeLabel}
            </span>
            <span className="dubhe-network-pill">{labels.testnet}</span>
            <span className="dubhe-update">
              {labels.updated} {formatTime(snapshot.generatedAtMs, locale)}
            </span>
          </div>
        </div>
        <div className="dubhe-orbit" aria-hidden="true">
          <div className="dubhe-orbit-ring ring-one" />
          <div className="dubhe-orbit-ring ring-two" />
          <div className="dubhe-orbit-core">
            <span>α</span>
            <strong>{snapshot.liveNodeCount}</strong>
            <small>NODES</small>
          </div>
          <span className="dubhe-orbit-node node-one" />
          <span className="dubhe-orbit-node node-two" />
          <span className="dubhe-orbit-node node-three" />
        </div>
      </header>

      <div className="dubhe-toolbar">
        <label className="dubhe-toggle">
          <input
            checked={autoRefresh}
            onChange={(event) => setAutoRefresh(event.target.checked)}
            type="checkbox"
          />
          <span aria-hidden="true" />
          {labels.auto}
        </label>
        <button
          className="dubhe-button primary"
          disabled={refreshing}
          onClick={() => void refresh().catch(() => undefined)}
          type="button"
        >
          {refreshing ? labels.refreshing : labels.refresh}
        </button>
      </div>

      <section className="dubhe-metrics" aria-label="Dubhe Node metrics">
        <Metric
          accent="cyan"
          detail={`${snapshot.registeredNodeCount} ${labels.registered}`}
          label={labels.liveNodes}
          value={`${snapshot.liveNodeCount}`}
        />
        <Metric
          accent="violet"
          detail={`${snapshot.totalSessionCapacity} capacity`}
          label={labels.sessions}
          value={`${snapshot.totalSessions}`}
        />
        <Metric
          accent="jade"
          detail={`${snapshot.totalZoneCapacity} capacity`}
          label={labels.zones}
          value={`${snapshot.totalZones}`}
        />
        <Metric
          accent="gold"
          detail={labels.testnet}
          label={labels.stake}
          value={`${formatMist(snapshot.totalStakeMist)} SUI`}
        />
      </section>

      <section className="dubhe-panel admission-panel">
        <div className="dubhe-section-heading">
          <div>
            <p>{labels.networkPath}</p>
            <h3>Trust → Capacity → Work</h3>
          </div>
          <span className="dubhe-readonly">{labels.readOnly}</span>
        </div>
        <div className="dubhe-admission-flow">
          <FlowStep
            caption={`#${snapshot.activeRegistrationCheckpoint}`}
            label={labels.registration}
            state={labels.finalized}
            tone="success"
          />
          <FlowConnector />
          <FlowStep
            caption={`${snapshot.finality.quorum}/4 ${labels.quorum}`}
            label={labels.finality}
            state={labels.finalized}
            tone="success"
          />
          <FlowConnector />
          <FlowStep
            caption={`${snapshot.capacity.completedCommands.toLocaleString()} ${labels.commands}`}
            label={labels.certificate}
            state={certificateValid ? labels.valid : labels.evidence}
            tone={certificateValid ? "success" : "evidence"}
          />
          <FlowConnector />
          <FlowStep
            caption={`${snapshot.rewards.total.toLocaleString()} ${labels.rewardTotal}`}
            label={labels.rewards}
            state={snapshot.finality.membershipEligible ? labels.accepted : labels.evidence}
            tone="success"
          />
        </div>
      </section>

      <div className="dubhe-layout">
        <section className="dubhe-panel fleet-panel">
          <div className="dubhe-section-heading">
            <div>
              <p>{labels.nodeFleet}</p>
              <h3>{labels.nodeFleetTitle}</h3>
            </div>
            <span className="dubhe-count">{snapshot.nodes.length}</span>
          </div>
          <div className="dubhe-node-list">
            {snapshot.nodes.map((node) => (
              <NodeCard
                copied={copiedNodeId === node.nodeId}
                key={node.nodeId}
                labels={labels}
                locale={locale}
                node={node}
                onCopy={handleCopy}
              />
            ))}
          </div>
        </section>

        <aside className="dubhe-side-stack">
          <section className="dubhe-panel chain-panel">
            <div className="dubhe-section-heading">
              <div>
                <p>{labels.chain}</p>
                <h3>{labels.chainTitle}</h3>
              </div>
              <span className="dubhe-chain-mark">SUI</span>
            </div>
            <Definition label={labels.package} value={snapshot.packageId} />
            <Definition label={labels.registry} value={snapshot.registryId} />
            <Definition
              label={labels.checkpoint}
              value={snapshot.activeRegistrationCheckpoint.toLocaleString()}
            />
            <Definition
              label={labels.operator}
              value={registeredNode?.operatorSuiAddress ?? "—"}
            />
            <div className="dubhe-link-row">
              <a
                className="dubhe-button secondary"
                href={snapshot.links.registrationExplorer}
                rel="noreferrer"
                target="_blank"
              >
                {labels.openTransaction}
              </a>
              <a
                className="dubhe-button secondary"
                href={snapshot.links.packageExplorer}
                rel="noreferrer"
                target="_blank"
              >
                {labels.openPackage}
              </a>
            </div>
          </section>

          <section className="dubhe-panel evidence-panel">
            <div className="dubhe-section-heading">
              <div>
                <p>{labels.evidence}</p>
                <h3>{labels.evidenceTitle}</h3>
              </div>
              <span className="dubhe-proof-mark">✓</span>
            </div>
            <div className="dubhe-evidence-grid">
              <Evidence
                detail={`${snapshot.capacity.maxSessionsPerZone}/Zone · ${snapshot.capacity.p95LatencyMs}ms ${labels.latency}`}
                label={labels.capacityRun}
                value={snapshot.capacity.completedCommands.toLocaleString()}
              />
              <Evidence
                detail={`${snapshot.finality.quorum}/4 ${labels.quorum}`}
                label={labels.commonware}
                value={`#${snapshot.finality.finalizedHeight}`}
              />
              <Evidence
                detail={`${snapshot.rewards.total.toLocaleString()} ${labels.rewardTotal}`}
                label={labels.rewardBatch}
                value={shortHash(snapshot.rewards.merkleRoot)}
              />
            </div>
          </section>

          <section className="dubhe-panel operations-panel">
            <div className="dubhe-section-heading">
              <div>
                <p>{labels.operations}</p>
                <h3>{labels.operationsTitle}</h3>
              </div>
              <span className="dubhe-readonly">{labels.readOnly}</span>
            </div>
            <a
              className="dubhe-operation"
              href={snapshot.links.grafana}
              rel="noreferrer"
              target="_blank"
            >
              <span className="operation-icon cyan" aria-hidden="true">↗</span>
              <span>{labels.grafana}</span>
              <small>{labels.grafanaDetail}</small>
            </a>
            <a
              className="dubhe-operation"
              href={snapshot.links.prometheus}
              rel="noreferrer"
              target="_blank"
            >
              <span className="operation-icon violet" aria-hidden="true">↗</span>
              <span>{labels.prometheus}</span>
              <small>{labels.prometheusDetail}</small>
            </a>
            <a
              className="dubhe-operation"
              href={snapshot.links.prometheusAlerts}
              rel="noreferrer"
              target="_blank"
            >
              <span className="operation-icon muted-icon" aria-hidden="true">!</span>
              <span>{labels.alerts}</span>
              <small>{labels.alertsDetail}</small>
            </a>
            <a
              className="dubhe-operation"
              download={`dubhe-node-snapshot-${snapshot.generatedAtMs}.json`}
              href={snapshot.links.snapshotExport}
            >
              <span className="operation-icon muted-icon" aria-hidden="true">↓</span>
              <span>{labels.exportSnapshot}</span>
              <small>{labels.exportSnapshotDetail}</small>
            </a>
          </section>
        </aside>
      </div>

      <footer className="dubhe-source-note">
        <span className="dubhe-source-icon" aria-hidden="true">i</span>
        <div>
          <strong>{labels.source}</strong>
          <p>{snapshot.mode === "offline" ? labels.sourceOffline : labels.sourceLive}</p>
          <small>{snapshot.sourceNote}</small>
        </div>
      </footer>
    </div>
  );
}

type Copy = (typeof copy)[keyof typeof copy];

function Metric({
  accent,
  detail,
  label,
  value
}: {
  accent: "cyan" | "violet" | "jade" | "gold";
  detail: string;
  label: string;
  value: string;
}) {
  return (
    <article className={`dubhe-metric ${accent}`}>
      <span className="dubhe-metric-spark" aria-hidden="true" />
      <p>{label}</p>
      <strong>{value}</strong>
      <small>{detail}</small>
    </article>
  );
}

function FlowStep({
  caption,
  label,
  state,
  tone
}: {
  caption: string;
  label: string;
  state: string;
  tone: "success" | "evidence";
}) {
  return (
    <div className="dubhe-flow-step">
      <span className={`dubhe-flow-icon ${tone}`} aria-hidden="true">✓</span>
      <div>
        <p>{label}</p>
        <strong>{state}</strong>
        <small>{caption}</small>
      </div>
    </div>
  );
}

function FlowConnector() {
  return <span className="dubhe-flow-connector" aria-hidden="true" />;
}

function NodeCard({
  copied,
  labels,
  locale,
  node,
  onCopy
}: {
  copied: boolean;
  labels: Copy;
  locale: "en" | "zh-CN";
  node: DubheNodeRecord;
  onCopy: (nodeId: string) => Promise<void>;
}) {
  const online = node.telemetryState === "live";
  return (
    <article className={`dubhe-node-card ${online ? "online" : "offline"}`}>
      <div className="dubhe-node-head">
        <div className="dubhe-node-identity">
          <span className="dubhe-node-glyph" aria-hidden="true">✦</span>
          <div>
            <div className="dubhe-node-title">
              <h4>{node.label}</h4>
              <span className={`dubhe-status-dot ${online ? "online" : "offline"}`}>
                {online ? labels.online : labels.offline}
              </span>
              <span className={`dubhe-status-dot ${node.registrationState}`}>
                {node.registrationState === "active" ? labels.registered : labels.unregistered}
              </span>
            </div>
            <button
              className="dubhe-node-id"
              onClick={() => void onCopy(node.nodeId)}
              title={labels.copyId}
              type="button"
            >
              {copied ? labels.copied : compactId(node.nodeId)}
            </button>
          </div>
        </div>
        <span
          className={`dubhe-verified ${node.heartbeatVerified ? "yes" : "no"}`}
          title={`${labels.heartbeat}: ${
            node.heartbeatVerified ? labels.verified : labels.notVerified
          }`}
        >
          {node.heartbeatVerified ? "✓" : "!"}
        </span>
      </div>

      <div className="dubhe-node-capacity">
        <CapacityBar
          label={labels.sessionsLabel}
          max={node.sessionCapacity}
          value={node.sessions}
        />
        {node.sessionCapacityPerZone !== undefined &&
        node.busiestZoneSessionCount !== undefined ? (
          <CapacityBar
            label={labels.busiestZoneLabel}
            max={node.sessionCapacityPerZone}
            value={node.busiestZoneSessionCount}
          />
        ) : null}
        <CapacityBar label={labels.zonesLabel} max={node.zoneCapacity} value={node.zones} />
      </div>

      <div className="dubhe-zone-workloads">
        <div className="dubhe-zone-workloads-head">
          <div>
            <small>{labels.hostedMaps}</small>
            <strong>
              {node.zoneDetailsVerified ? node.activeZones.length : node.zones}
            </strong>
          </div>
          <span className={node.zoneDetailsVerified ? "verified" : "unverified"}>
            {node.zoneDetailsVerified ? `✓ ${labels.signedRuntime}` : labels.notVerified}
          </span>
        </div>
        {node.activeZones.length > 0 ? (
          <div className="dubhe-zone-list">
            {node.activeZones.map((zone) => (
              <ZoneWorkload key={zone.zoneId} labels={labels} zone={zone} />
            ))}
          </div>
        ) : (
          <p className="dubhe-zone-empty">
            {node.zones > 0 ? labels.zoneDetailsUnavailable : labels.noActiveZones}
          </p>
        )}
      </div>

      <div className="dubhe-node-stats">
        <NodeStat label={labels.rpc} value={node.rpcRequestsTotal.toLocaleString()} />
        <NodeStat label={labels.errors} value={node.rpcErrorsTotal.toLocaleString()} />
        <NodeStat label={labels.uptime} value={formatDuration(node.uptimeSeconds, locale)} />
        <NodeStat label={labels.generation} value={`#${node.keyGeneration}`} />
      </div>

      <div className="dubhe-node-foot">
        <span>
          <small>{labels.endpoint}</small>
          {node.advertisedEndpoint}
        </span>
        <span>
          <small>{labels.domain}</small>
          {node.failureDomain}
        </span>
        {node.processId ? (
          <span>
            <small>{labels.process}</small>
            {node.processId}
          </span>
        ) : null}
      </div>
      {node.error ? <p className="dubhe-node-error">{node.error}</p> : null}
    </article>
  );
}

function ZoneWorkload({
  labels,
  zone
}: {
  labels: Copy;
  zone: DubheNodeZoneRecord;
}) {
  const visibleMaps = zone.mapFileNames.slice(0, 4);
  const remainingMaps = zone.mapFileNames.length - visibleMaps.length;
  const title =
    zone.mapScope === "all"
      ? labels.allMaps
      : visibleMaps.length > 0
        ? visibleMaps.join(" · ")
        : labels.activeRuntime;
  return (
    <div className="dubhe-zone-row">
      <span className="dubhe-zone-pulse" aria-hidden="true" />
      <div className="dubhe-zone-copy">
        <strong>
          {title}
          {remainingMaps > 0 ? ` +${remainingMaps}` : ""}
        </strong>
        <span>
          <code>{zone.zoneId}</code>
          {zone.mapScope === "explicit" ? ` · ${labels.mapFiles}` : ""}
        </span>
      </div>
      <span className="dubhe-zone-sessions">
        <strong>{zone.sessionCount}</strong>
        <small>{labels.zoneSessions}</small>
      </span>
    </div>
  );
}

function CapacityBar({
  label,
  max,
  value
}: {
  label: string;
  max: number;
  value: number;
}) {
  return (
    <div className="dubhe-capacity">
      <div>
        <span>{label}</span>
        <strong>{value} / {max}</strong>
      </div>
      <progress
        aria-label={`${label}: ${value} / ${max}`}
        className="dubhe-capacity-track"
        max={Math.max(1, max)}
        value={value}
      />
    </div>
  );
}

function NodeStat({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <small>{label}</small>
      <strong>{value}</strong>
    </div>
  );
}

function Definition({ label, value }: { label: string; value: string }) {
  return (
    <div className="dubhe-definition">
      <span>{label}</span>
      <code title={value}>{compactId(value)}</code>
    </div>
  );
}

function Evidence({
  detail,
  label,
  value
}: {
  detail: string;
  label: string;
  value: string;
}) {
  return (
    <div className="dubhe-evidence">
      <span>{label}</span>
      <strong>{value}</strong>
      <small>{detail}</small>
    </div>
  );
}

function compactId(value: string) {
  if (value.length <= 24) {
    return value;
  }
  return `${value.slice(0, 12)}…${value.slice(-8)}`;
}

function shortHash(value: string) {
  return `${value.slice(0, 7)}…${value.slice(-5)}`;
}

function formatMist(value: number) {
  return new Intl.NumberFormat("en-US", {
    maximumFractionDigits: 3
  }).format(value / 1_000_000_000);
}

function formatTime(value: number, locale: "en" | "zh-CN") {
  return new Intl.DateTimeFormat(locale, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit"
  }).format(value);
}

function formatDuration(value: number, locale: "en" | "zh-CN") {
  if (!value) {
    return "—";
  }
  const hours = Math.floor(value / 3_600);
  const minutes = Math.floor((value % 3_600) / 60);
  return locale === "zh-CN" ? `${hours}时 ${minutes}分` : `${hours}h ${minutes}m`;
}
