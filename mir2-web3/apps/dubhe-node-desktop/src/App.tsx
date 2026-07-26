import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

type WorkMode = "serving" | "draining" | "paused";

type SupervisorStatus = {
  version: string;
  mode: WorkMode;
  acceptNewSessions: boolean;
  reason: string;
  cpuUsagePercent: number;
  availableMemoryBytes: number;
  activeSessions: number;
  zoneReachable: boolean;
  lastObservedAtMs: number;
  nodeId: string;
  publicKey: string;
  keyStore: string;
  managedProcesses: boolean;
  agentManaged: boolean;
  relayConnected: boolean;
  telemetryConfigured: boolean;
  telemetryAccepted: boolean;
  telemetrySequence?: number;
  lastTelemetryAtMs?: number;
  telemetryError?: string;
};

type DesktopIdentity = {
  nodeId: string;
  publicKey: string;
  created: boolean;
  keyStore: string;
};

type DesktopBootstrap = {
  identity: DesktopIdentity;
  managementTokenCreated: boolean;
  supervisorReachable: boolean;
  status?: SupervisorStatus;
};

type DesktopEnrollmentStatus = {
  configured: boolean;
  enrolled: boolean;
  capacityReady: boolean;
  relayReady: boolean;
  enrollmentId?: string;
  expiresAtMs?: number;
  relayId?: string;
  telemetryUrl?: string;
  maxSessions?: number;
  maxZones?: number;
  error?: string;
};

type Tab = "overview" | "network" | "beta" | "settings";

const nav: Array<{ id: Tab; label: string; icon: string }> = [
  { id: "overview", label: "总览", icon: "⌁" },
  { id: "network", label: "网络", icon: "◇" },
  { id: "beta", label: "生产 Beta", icon: "✓" },
  { id: "settings", label: "设置", icon: "⚙" },
];

function shortNodeId(value?: string) {
  if (!value) return "等待初始化";
  return `${value.slice(0, 18)}…${value.slice(-8)}`;
}

function formatMemory(bytes?: number) {
  if (!bytes) return "0.0 GB";
  return `${(bytes / 1024 / 1024 / 1024).toFixed(1)} GB`;
}

function relativeTime(timestamp?: number) {
  if (!timestamp) return "尚未同步";
  const seconds = Math.max(0, Math.floor((Date.now() - timestamp) / 1000));
  if (seconds < 5) return "刚刚";
  if (seconds < 60) return `${seconds} 秒前`;
  return `${Math.floor(seconds / 60)} 分钟前`;
}

function Icon({ children }: { children: string }) {
  return <span className="nav-icon" aria-hidden="true">{children}</span>;
}

function App() {
  const [activeTab, setActiveTab] = useState<Tab>("overview");
  const [bootstrap, setBootstrap] = useState<DesktopBootstrap>();
  const [status, setStatus] = useState<SupervisorStatus>();
  const [enrollment, setEnrollment] = useState<DesktopEnrollmentStatus>();
  const [busy, setBusy] = useState(false);
  const [enrollmentBusy, setEnrollmentBusy] = useState(false);
  const [error, setError] = useState<string>();

  const initialize = useCallback(async () => {
    try {
      const result = await invoke<DesktopBootstrap>("bootstrap_node");
      setBootstrap(result);
      setStatus(result.status);
      setError(result.supervisorReachable ? undefined : "后台服务尚未启动，节点保持安全暂停状态。");
      const enrollmentStatus = await invoke<DesktopEnrollmentStatus>("enrollment_status");
      setEnrollment(enrollmentStatus);
    } catch (cause) {
      setError(String(cause));
    }
  }, []);

  const refresh = useCallback(async () => {
    try {
      const next = await invoke<SupervisorStatus>("node_status");
      setStatus(next);
      setError(undefined);
    } catch {
      setStatus(undefined);
      setError((current) => current ?? "后台服务尚未启动，节点保持安全暂停状态。");
    }
  }, []);

  useEffect(() => {
    void initialize();
  }, [initialize]);

  useEffect(() => {
    const timer = window.setInterval(() => void refresh(), 5_000);
    return () => window.clearInterval(timer);
  }, [refresh]);

  const serving =
    status?.mode === "serving" &&
    status.acceptNewSessions &&
    status.relayConnected &&
    status.telemetryAccepted &&
    enrollment?.capacityReady;
  const connected = Boolean(status?.relayConnected && status?.telemetryAccepted);
  const nodeId = status?.nodeId ?? bootstrap?.identity.nodeId;
  const readiness = useMemo(
    () => [
      { label: "节点身份已保存在系统密钥库", ready: Boolean(bootstrap?.identity.nodeId) },
      { label: "本地 Zone Host 健康", ready: Boolean(status?.zoneReachable) },
      { label: "Zone Host 与 Supervisor 由安装器托管", ready: Boolean(status?.managedProcesses) },
      { label: "官方签名 enrollment", ready: Boolean(enrollment?.enrolled) },
      { label: "生产容量证书", ready: Boolean(enrollment?.capacityReady) },
      { label: "Home Agent Relay mTLS 凭证", ready: Boolean(enrollment?.relayReady) },
      { label: "官方 Relay 隧道已连接", ready: Boolean(status?.relayConnected) },
      { label: "Collector 已接受签名遥测", ready: Boolean(status?.telemetryAccepted) },
      { label: "签名 Beta 测试计划", ready: false },
    ],
    [bootstrap, enrollment, status],
  );

  async function toggleServing() {
    if (!status || busy) return;
    setBusy(true);
    try {
      const receipt = await invoke<{ status: SupervisorStatus }>("set_node_serving", {
        serving: !serving,
      });
      setStatus(receipt.status);
      setError(undefined);
    } catch (cause) {
      setError(String(cause));
    } finally {
      setBusy(false);
    }
  }

  async function requestEnrollment() {
    if (enrollmentBusy || !enrollment?.configured) return;
    setEnrollmentBusy(true);
    try {
      const result = await invoke<DesktopEnrollmentStatus>("enroll_node");
      setEnrollment(result);
      setError(undefined);
    } catch (cause) {
      setError(String(cause));
    } finally {
      setEnrollmentBusy(false);
    }
  }

  async function requestCertification() {
    if (enrollmentBusy || !enrollment?.enrolled || enrollment.relayReady) return;
    setEnrollmentBusy(true);
    try {
      const result = await invoke<DesktopEnrollmentStatus>("certify_node");
      setEnrollment(result);
      await refresh();
      setError(undefined);
    } catch (cause) {
      setError(String(cause));
    } finally {
      setEnrollmentBusy(false);
    }
  }

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <div className="brand-mark"><span /></div>
          <div>
            <strong>Dubhe Node</strong>
            <small>Obelisk Labs</small>
          </div>
        </div>

        <nav aria-label="主要导航">
          {nav.map((item) => (
            <button
              className={activeTab === item.id ? "nav-item active" : "nav-item"}
              key={item.id}
              onClick={() => setActiveTab(item.id)}
              type="button"
            >
              <Icon>{item.icon}</Icon>
              {item.label}
            </button>
          ))}
        </nav>

        <div className="sidebar-foot">
          <div className="privacy-dot" />
          <div>
            <strong>隐私 Relay</strong>
            <span>家庭 IP 不进入产品遥测</span>
          </div>
        </div>
      </aside>

      <main className="main">
        <header>
          <div>
            <p className="eyebrow">DUBHE HOME COMPUTE</p>
            <h1>{activeTab === "overview" ? "节点总览" : nav.find((item) => item.id === activeTab)?.label}</h1>
          </div>
          <div className={connected ? "health-pill online" : "health-pill"}>
            <i />
            {connected ? "服务在线" : "安全暂停"}
          </div>
        </header>

        {error && (
          <div className="notice" role="status">
            <span>!</span>
            <div><strong>需要处理</strong><p>{error}</p></div>
            <button type="button" onClick={() => void initialize()}>重试</button>
          </div>
        )}

        {activeTab === "overview" && (
          <>
            <section className="hero-card">
              <div className="orb-wrap">
                <div className={serving ? "orb serving" : "orb"}>
                  <div className="orb-core">
                    <span>{serving ? "运行中" : status ? "已暂停" : "离线"}</span>
                    <strong>{status?.activeSessions ?? 0}</strong>
                    <small>活跃 Sessions</small>
                  </div>
                </div>
              </div>
              <div className="hero-copy">
                <p className="section-label">闲置算力模式</p>
                <h2>{serving ? "正在为游戏地图提供服务" : "节点暂不接受新玩家"}</h2>
                <p>
                  {serving
                    ? "资源策略会在电脑繁忙、休眠或网络异常时自动停止接入，并把玩家安全迁移到 standby。"
                    : "启动后只使用你设定的闲置资源。关闭应用不会泄露密钥，也不会开放家庭入站端口。"}
                </p>
                <button
                  className={serving ? "power-button stop" : "power-button"}
                  disabled={
                    !status ||
                    busy ||
                    (!serving &&
                      (!status.zoneReachable ||
                        !status.relayConnected ||
                        !status.telemetryAccepted ||
                        !enrollment?.capacityReady))
                  }
                  onClick={() => void toggleServing()}
                  type="button"
                >
                  <span className="power-icon">⌁</span>
                  {busy
                    ? "正在切换…"
                    : serving
                      ? "暂停贡献"
                      : !enrollment?.enrolled
                        ? "等待 enrollment"
                        : !enrollment.capacityReady
                          ? "等待容量认证"
                          : !status?.relayConnected
                            ? "等待 Relay 连接"
                            : !status?.telemetryAccepted
                              ? "等待遥测回执"
                              : status?.zoneReachable
                                ? "开始贡献"
                                : "等待 Zone 启动"}
                </button>
              </div>
            </section>

            <section className="metrics-grid">
              <article>
                <span className="metric-icon violet">CPU</span>
                <div><small>处理器使用</small><strong>{(status?.cpuUsagePercent ?? 0).toFixed(1)}%</strong></div>
                <em>上限 75%</em>
              </article>
              <article>
                <span className="metric-icon cyan">MEM</span>
                <div><small>可用内存</small><strong>{formatMemory(status?.availableMemoryBytes)}</strong></div>
                <em>保留 2 GB</em>
              </article>
              <article>
                <span className="metric-icon green">NET</span>
                <div><small>Zone 连接</small><strong>{status?.zoneReachable ? "健康" : "未连接"}</strong></div>
                <em>{relativeTime(status?.lastObservedAtMs)}</em>
              </article>
              <article>
                <span className="metric-icon amber">TEL</span>
                <div>
                  <small>生产遥测</small>
                  <strong>{status?.telemetryAccepted ? "已接收" : "等待"}</strong>
                </div>
                <em>
                  {status?.telemetryAccepted
                    ? `#${status.telemetrySequence ?? 0} · ${relativeTime(status.lastTelemetryAtMs)}`
                    : status?.telemetryError ?? "等待 Collector 回执"}
                </em>
              </article>
            </section>

            <section className="detail-grid">
              <article className="panel node-panel">
                <div className="panel-title"><h3>节点身份</h3><span>系统密钥库</span></div>
                <div className="node-id">
                  <div className="identity-mark">D</div>
                  <div><small>Node ID</small><code>{shortNodeId(nodeId)}</code></div>
                </div>
                <dl>
                  <div><dt>客户端版本</dt><dd>v{status?.version ?? "0.1.0"}</dd></div>
                  <div><dt>工作模式</dt><dd>{status?.mode ?? "paused"}</dd></div>
                  <div>
                    <dt>进程托管</dt>
                    <dd>
                      {status?.agentManaged
                        ? "Zone + Agent"
                        : status?.managedProcesses
                          ? "Zone only"
                          : "待安装"}
                    </dd>
                  </div>
                </dl>
              </article>
              <article className="panel activity-panel">
                <div className="panel-title"><h3>服务状态</h3><span className="live">实时</span></div>
                <div className="timeline">
                  <div className={status?.zoneReachable ? "event good" : "event"}>
                    <i /><div><strong>本地 Zone Host</strong><span>{status?.zoneReachable ? "健康检查通过" : "等待后台服务"}</span></div>
                  </div>
                  <div className={serving ? "event good" : "event"}>
                    <i />
                    <div>
                      <strong>接收新 Session</strong>
                      <span>
                        {serving
                          ? "已开放"
                          : status?.zoneReachable && !status?.agentManaged
                            ? "本地就绪，公网未开放"
                            : "已关闭"}
                      </span>
                    </div>
                  </div>
                  <div className={status?.relayConnected ? "event good" : "event"}>
                    <i />
                    <div>
                      <strong>官方 Relay</strong>
                      <span>
                        {status?.relayConnected
                          ? "QUIC + mTLS 隧道已连接"
                          : status?.agentManaged
                            ? "Home Agent 已启动，正在连接"
                            : enrollment?.capacityReady
                              ? "等待 Relay 连接"
                              : enrollment?.enrolled
                                ? "等待容量证书与 mTLS"
                                : "等待 enrollment 配置"}
                      </span>
                    </div>
                  </div>
                  <div className={status?.telemetryAccepted ? "event good" : "event"}>
                    <i />
                    <div>
                      <strong>生产遥测 Collector</strong>
                      <span>
                        {status?.telemetryAccepted
                          ? `签名报告 #${status.telemetrySequence ?? 0} 已接受`
                          : status?.telemetryError ?? "等待首份签名报告回执"}
                      </span>
                    </div>
                  </div>
                </div>
              </article>
            </section>
          </>
        )}

        {activeTab === "network" && (
          <section className="page-card">
            <p className="section-label">OUTBOUND ONLY</p>
            <h2>家庭网络无需公网 IP</h2>
            <p className="page-lead">Dubhe Node 只主动连接官方 Relay，不打开路由器端口，不接管你的普通网络流量。</p>
            <div className="network-flow">
              <div><span>01</span><strong>本机 Zone</strong><small>loopback 隔离</small></div>
              <b>→</b>
              <div><span>02</span><strong>QUIC + mTLS</strong><small>主动出站隧道</small></div>
              <b>→</b>
              <div>
                <span>03</span>
                <strong>{enrollment?.relayId ?? "官方 Relay"}</strong>
                <small>
                  {status?.relayConnected
                    ? "QUIC + mTLS 已连接"
                    : enrollment?.enrolled
                      ? "已配置，等待运行态连接"
                      : "等待 enrollment"}
                </small>
              </div>
            </div>
          </section>
        )}

        {activeTab === "beta" && (
          <section className="page-card">
            <p className="section-label">PHYSICAL HOME NETWORK</p>
            <h2>生产 Beta 准备检查</h2>
            <p className="page-lead">测试计划只能执行客户端内置的白名单动作，不接受服务器下发 Shell 或任意代码。</p>
            <div className="check-list">
              {readiness.map((item) => (
                <div key={item.label} className={item.ready ? "check ready" : "check"}>
                  <span>{item.ready ? "✓" : "·"}</span>
                  <strong>{item.label}</strong>
                  <em>{item.ready ? "完成" : "待配置"}</em>
                </div>
              ))}
            </div>
            {enrollment?.enrolled && (
              <div className="settings-list">
                <div>
                  <span><strong>Enrollment ID</strong><small>官方签名配置</small></span>
                  <b>{enrollment.enrollmentId}</b>
                </div>
                <div>
                  <span><strong>授权容量</strong><small>最终以容量证书为准</small></span>
                  <b>{enrollment.maxSessions ?? 0} Sessions / {enrollment.maxZones ?? 0} Zones</b>
                </div>
                <div>
                  <span><strong>遥测出口</strong><small>仅发送签名最小化指标</small></span>
                  <b>
                    {status?.telemetryAccepted
                      ? `Collector 已接收 #${status.telemetrySequence ?? 0}`
                      : enrollment.telemetryUrl
                        ? "已配置，等待回执"
                        : "未配置"}
                  </b>
                </div>
              </div>
            )}
            <button
              className="secondary-button"
              disabled={!enrollment?.configured || enrollmentBusy || enrollment?.enrolled}
              onClick={() => void requestEnrollment()}
              type="button"
            >
              {enrollmentBusy
                ? "正在签名并申请…"
                : enrollment?.enrolled
                  ? enrollment.capacityReady
                    ? "Enrollment 与容量证书有效"
                    : "Enrollment 有效，等待容量认证"
                  : enrollment?.configured
                    ? "签名申请入网"
                    : "未配置 Enrollment Service"}
            </button>
            {enrollment?.enrolled && !enrollment.relayReady && (
              <button
                className="secondary-button"
                disabled={enrollmentBusy || !status?.zoneReachable}
                onClick={() => void requestCertification()}
                type="button"
              >
                {enrollmentBusy
                  ? "正在执行容量挑战…"
                  : status?.zoneReachable
                    ? "执行容量认证并申请 Relay mTLS"
                    : "等待本地 Zone Host"}
              </button>
            )}
            {enrollment?.error && <p className="page-lead">{enrollment.error}</p>}
          </section>
        )}

        {activeTab === "settings" && (
          <section className="page-card">
            <p className="section-label">LOCAL-FIRST SECURITY</p>
            <h2>安全与资源策略</h2>
            <div className="settings-list">
              <div><span><strong>最大 CPU 使用率</strong><small>持续超限后自动 drain</small></span><b>75%</b></div>
              <div><span><strong>最低可用内存</strong><small>保留给你的游戏和日常应用</small></span><b>2 GB</b></div>
              <div><span><strong>自动更新</strong><small>仅接受离线发行密钥签名版本</small></span><b>Stable</b></div>
              <div><span><strong>身份存储</strong><small>节点私钥不可导出到网页</small></span><b>OS Keyring</b></div>
            </div>
          </section>
        )}
      </main>
    </div>
  );
}

export default App;
