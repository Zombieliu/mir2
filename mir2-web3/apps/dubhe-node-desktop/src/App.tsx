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
  renewalState: string;
  renewAtMs?: number;
  lastRenewedAtMs?: number;
  renewalError?: string;
  error?: string;
};

type UpdateChannel = "stable" | "beta";

type DesktopPreferences = {
  closeToTray: boolean;
  startMinimized: boolean;
  autostartEnabled: boolean;
  autoCheckUpdates: boolean;
  updateChannel: UpdateChannel;
};

type DesktopUpdateStatus = {
  configured: boolean;
  channel: UpdateChannel;
  currentVersion: string;
  availableVersion?: string;
  notes?: string;
  publishedAt?: string;
  error?: string;
};

type DiagnosticExport = {
  path: string;
  redacted: boolean;
  generatedAtMs: number;
};

type DesktopRecoveryStatus = {
  available: boolean;
  configured: boolean;
  currentVersion: string;
  rollbackVersion?: string;
  installedVersion?: string;
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

function formatDateTime(timestamp?: number) {
  if (!timestamp) return "等待签发";
  return new Intl.DateTimeFormat("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(timestamp);
}

function renewalLabel(state?: string) {
  switch (state) {
    case "current": return "自动续期已启用";
    case "draining": return "等待存量玩家退出";
    case "renewing": return "正在轮换凭证";
    case "failed": return "续期失败，后台将重试";
    case "awaiting-certification": return "等待首次容量认证";
    case "awaiting-enrollment": return "等待首次 Enrollment";
    case "not-configured": return "未配置";
    default: return "后台检查中";
  }
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
  const [preferencesBusy, setPreferencesBusy] = useState(false);
  const [updateBusy, setUpdateBusy] = useState(false);
  const [error, setError] = useState<string>();
  const [preferences, setPreferences] = useState<DesktopPreferences>();
  const [updateStatus, setUpdateStatus] = useState<DesktopUpdateStatus>();
  const [recoveryStatus, setRecoveryStatus] = useState<DesktopRecoveryStatus>();
  const [diagnosticPath, setDiagnosticPath] = useState<string>();

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
      const [nextStatus, nextEnrollment] = await Promise.all([
        invoke<SupervisorStatus>("node_status"),
        invoke<DesktopEnrollmentStatus>("enrollment_status"),
      ]);
      setStatus(nextStatus);
      setEnrollment(nextEnrollment);
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
    void Promise.all([
      invoke<DesktopPreferences>("desktop_preferences"),
      invoke<DesktopRecoveryStatus>("desktop_recovery_status"),
    ])
      .then(([desktopPreferences, recovery]) => {
        setPreferences(desktopPreferences);
        setRecoveryStatus(recovery);
        if (desktopPreferences.autoCheckUpdates) {
          return invoke<DesktopUpdateStatus>("check_for_desktop_update");
        }
        return undefined;
      })
      .then((result) => {
        if (result) setUpdateStatus(result);
      })
      .catch((cause) => setError(String(cause)));
  }, []);

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

  async function renewCredentials() {
    if (enrollmentBusy || !enrollment?.relayReady) return;
    setEnrollmentBusy(true);
    try {
      const result = await invoke<DesktopEnrollmentStatus>("renew_node_credentials");
      setEnrollment(result);
      await refresh();
      setError(undefined);
    } catch (cause) {
      setError(String(cause));
    } finally {
      setEnrollmentBusy(false);
    }
  }

  async function savePreferences(next: DesktopPreferences) {
    if (preferencesBusy) return;
    const previous = preferences;
    setPreferences(next);
    setPreferencesBusy(true);
    try {
      const saved = await invoke<DesktopPreferences>("set_desktop_preferences", {
        preferences: next,
      });
      setPreferences(saved);
      if (saved.autoCheckUpdates || saved.updateChannel !== previous?.updateChannel) {
        const result = await invoke<DesktopUpdateStatus>("check_for_desktop_update");
        setUpdateStatus(result);
      }
      setError(undefined);
    } catch (cause) {
      setPreferences(previous);
      setError(String(cause));
    } finally {
      setPreferencesBusy(false);
    }
  }

  async function checkForUpdate(install: boolean) {
    if (updateBusy) return;
    setUpdateBusy(true);
    try {
      const result = await invoke<DesktopUpdateStatus>(
        install ? "install_desktop_update" : "check_for_desktop_update",
      );
      setUpdateStatus(result);
      setError(result.error);
    } catch (cause) {
      setError(String(cause));
    } finally {
      setUpdateBusy(false);
    }
  }

  async function exportDiagnostics() {
    try {
      const result = await invoke<DiagnosticExport>("export_diagnostics");
      setDiagnosticPath(result.path);
      setError(undefined);
    } catch (cause) {
      setError(String(cause));
    }
  }

  async function rollbackUpdate() {
    if (
      updateBusy ||
      !recoveryStatus?.available ||
      !window.confirm(`将回滚到已签名版本 v${recoveryStatus.rollbackVersion}，是否继续？`)
    ) return;
    setUpdateBusy(true);
    try {
      const result = await invoke<DesktopRecoveryStatus>("rollback_desktop_update");
      setRecoveryStatus(result);
      setError("签名回滚安装完成；请按安装器提示重新启动 Dubhe Node。");
    } catch (cause) {
      setError(String(cause));
    } finally {
      setUpdateBusy(false);
    }
  }

  async function prepareUninstall() {
    if (!window.confirm("将停止接收新玩家并关闭开机自启。节点身份会保留，是否继续？")) return;
    try {
      const result = await invoke<{ instructions: string }>("prepare_uninstall");
      setError(result.instructions);
      const refreshed = await invoke<DesktopPreferences>("desktop_preferences");
      setPreferences(refreshed);
    } catch (cause) {
      setError(String(cause));
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
                    enrollment?.renewalState === "draining" ||
                    enrollment?.renewalState === "renewing" ||
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
                <div>
                  <span><strong>凭证自动续期</strong><small>到期前 6 小时安全轮换</small></span>
                  <b>{renewalLabel(enrollment.renewalState)}</b>
                </div>
                <div>
                  <span><strong>下次续期窗口</strong><small>有活跃玩家时先 drain，不强制断线</small></span>
                  <b>{formatDateTime(enrollment.renewAtMs)}</b>
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
            {enrollment?.relayReady && (
              <button
                className="secondary-button"
                disabled={enrollmentBusy}
                onClick={() => void renewCredentials()}
                type="button"
              >
                {enrollmentBusy
                  ? "正在安全续期…"
                  : enrollment.renewalState === "draining"
                    ? "等待存量玩家退出"
                    : "立即检查并续期"}
              </button>
            )}
            {enrollment?.renewalError && <p className="page-lead">{enrollment.renewalError}</p>}
            {enrollment?.error && <p className="page-lead">{enrollment.error}</p>}
          </section>
        )}

        {activeTab === "settings" && (
          <section className="page-card">
            <p className="section-label">DESKTOP LIFECYCLE</p>
            <h2>桌面端与发行设置</h2>
            <p className="page-lead">
              关闭窗口默认隐藏到系统托盘；只有选择“停止节点并退出”才会结束后台进程。
            </p>
            <div className="settings-list">
              <div><span><strong>最大 CPU 使用率</strong><small>持续超限后自动 drain</small></span><b>75%</b></div>
              <div><span><strong>最低可用内存</strong><small>保留给你的游戏和日常应用</small></span><b>2 GB</b></div>
              <div>
                <span><strong>关闭到系统托盘</strong><small>关闭窗口时继续服务已有玩家</small></span>
                <button
                  className={preferences?.closeToTray ? "toggle on" : "toggle"}
                  disabled={!preferences || preferencesBusy}
                  onClick={() => preferences && void savePreferences({
                    ...preferences,
                    closeToTray: !preferences.closeToTray,
                  })}
                  type="button"
                  aria-pressed={preferences?.closeToTray}
                >
                  <i />
                </button>
              </div>
              <div>
                <span><strong>开机自动运行</strong><small>使用系统原生启动项，启动后隐藏到托盘</small></span>
                <button
                  className={preferences?.autostartEnabled ? "toggle on" : "toggle"}
                  disabled={!preferences || preferencesBusy}
                  onClick={() => preferences && void savePreferences({
                    ...preferences,
                    autostartEnabled: !preferences.autostartEnabled,
                  })}
                  type="button"
                  aria-pressed={preferences?.autostartEnabled}
                >
                  <i />
                </button>
              </div>
              <div>
                <span><strong>普通启动时最小化</strong><small>手动打开应用时也直接进入托盘</small></span>
                <button
                  className={preferences?.startMinimized ? "toggle on" : "toggle"}
                  disabled={!preferences || preferencesBusy}
                  onClick={() => preferences && void savePreferences({
                    ...preferences,
                    startMinimized: !preferences.startMinimized,
                  })}
                  type="button"
                  aria-pressed={preferences?.startMinimized}
                >
                  <i />
                </button>
              </div>
              <div>
                <span><strong>自动检查签名更新</strong><small>只安装离线发行密钥签名的版本</small></span>
                <button
                  className={preferences?.autoCheckUpdates ? "toggle on" : "toggle"}
                  disabled={!preferences || preferencesBusy}
                  onClick={() => preferences && void savePreferences({
                    ...preferences,
                    autoCheckUpdates: !preferences.autoCheckUpdates,
                  })}
                  type="button"
                  aria-pressed={preferences?.autoCheckUpdates}
                >
                  <i />
                </button>
              </div>
              <div>
                <span><strong>更新通道</strong><small>Beta 可提前验证新功能，随时可切回 Stable</small></span>
                <select
                  className="channel-select"
                  disabled={!preferences || preferencesBusy}
                  value={preferences?.updateChannel ?? "stable"}
                  onChange={(event) => preferences && void savePreferences({
                    ...preferences,
                    updateChannel: event.target.value as UpdateChannel,
                  })}
                >
                  <option value="stable">Stable</option>
                  <option value="beta">Beta</option>
                </select>
              </div>
              <div><span><strong>身份存储</strong><small>节点私钥不可导出到网页</small></span><b>OS Keyring</b></div>
            </div>
            <div className="settings-actions">
              <button
                className="secondary-button"
                disabled={updateBusy}
                onClick={() => void checkForUpdate(false)}
                type="button"
              >
                {updateBusy ? "正在检查…" : "检查更新"}
              </button>
              {updateStatus?.availableVersion && (
                <button
                  className="secondary-button primary"
                  disabled={updateBusy}
                  onClick={() => void checkForUpdate(true)}
                  type="button"
                >
                  安装 v{updateStatus.availableVersion}
                </button>
              )}
              <button className="secondary-button" onClick={() => void exportDiagnostics()} type="button">
                导出脱敏诊断
              </button>
              {recoveryStatus?.available && (
                <button
                  className="secondary-button"
                  disabled={updateBusy}
                  onClick={() => void rollbackUpdate()}
                  type="button"
                >
                  回滚到 v{recoveryStatus.rollbackVersion}
                </button>
              )}
              <button className="secondary-button danger" onClick={() => void prepareUninstall()} type="button">
                准备卸载
              </button>
            </div>
            <div className="release-status">
              <strong>
                当前 v{updateStatus?.currentVersion ?? "0.1.0"} · {(preferences?.updateChannel ?? "stable").toUpperCase()}
              </strong>
              <span>
                {updateStatus?.availableVersion
                  ? `发现签名版本 v${updateStatus.availableVersion}`
                  : updateStatus?.configured
                    ? "已是当前通道最新版本"
                    : "开发构建：正式安装包发布时注入签名更新源"}
              </span>
              {diagnosticPath && <code>诊断已导出：{diagnosticPath}</code>}
            </div>
          </section>
        )}
      </main>
    </div>
  );
}

export default App;
