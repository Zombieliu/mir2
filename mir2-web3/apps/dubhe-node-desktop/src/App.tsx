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
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string>();

  const initialize = useCallback(async () => {
    try {
      const result = await invoke<DesktopBootstrap>("bootstrap_node");
      setBootstrap(result);
      setStatus(result.status);
      setError(result.supervisorReachable ? undefined : "后台服务尚未启动，节点保持安全暂停状态。");
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
      setError("后台服务尚未启动，节点保持安全暂停状态。");
    }
  }, []);

  useEffect(() => {
    void initialize();
  }, [initialize]);

  useEffect(() => {
    const timer = window.setInterval(() => void refresh(), 5_000);
    return () => window.clearInterval(timer);
  }, [refresh]);

  const serving = status?.mode === "serving" && status.acceptNewSessions;
  const connected = Boolean(status?.zoneReachable);
  const nodeId = status?.nodeId ?? bootstrap?.identity.nodeId;
  const readiness = useMemo(
    () => [
      { label: "节点身份已保存在系统密钥库", ready: Boolean(bootstrap?.identity.nodeId) },
      { label: "本地 Zone Host 健康", ready: Boolean(status?.zoneReachable) },
      { label: "Agent 与 Supervisor 由安装器托管", ready: Boolean(status?.managedProcesses) },
      { label: "生产 enrollment 与容量证书", ready: false },
      { label: "签名 Beta 测试计划", ready: false },
    ],
    [bootstrap, status],
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
                  disabled={!status || busy}
                  onClick={() => void toggleServing()}
                  type="button"
                >
                  <span className="power-icon">⌁</span>
                  {busy ? "正在切换…" : serving ? "暂停贡献" : "开始贡献"}
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
                <span className="metric-icon amber">RWD</span>
                <div><small>今日预计奖励</small><strong>—</strong></div>
                <em>等待权威回执</em>
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
                  <div><dt>进程托管</dt><dd>{status?.managedProcesses ? "已启用" : "待安装"}</dd></div>
                </dl>
              </article>
              <article className="panel activity-panel">
                <div className="panel-title"><h3>服务状态</h3><span className="live">实时</span></div>
                <div className="timeline">
                  <div className={connected ? "event good" : "event"}>
                    <i /><div><strong>本地 Zone Host</strong><span>{connected ? "健康检查通过" : "等待后台服务"}</span></div>
                  </div>
                  <div className={serving ? "event good" : "event"}>
                    <i /><div><strong>接收新 Session</strong><span>{serving ? "已开放" : "已关闭"}</span></div>
                  </div>
                  <div className="event">
                    <i /><div><strong>官方 Relay</strong><span>等待 enrollment 配置</span></div>
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
              <div><span>03</span><strong>官方 Relay</strong><small>隐藏家庭 IP</small></div>
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
            <button className="secondary-button" disabled type="button">等待生产 enrollment</button>
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
