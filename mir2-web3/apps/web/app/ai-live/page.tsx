"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import type {
  AiDistributionChannel,
  AiLiveStatus,
} from "../components/ai-live-overlay";

function number(value: number | undefined) {
  return new Intl.NumberFormat("zh-CN").format(value ?? 0);
}

function channelStateLabel(state: string) {
  switch (state) {
    case "ready": return "已验证";
    case "waiting": return "等待真实信号";
    case "degraded": return "链路异常";
    case "disabled": return "已暂停";
    default: return "待配置";
  }
}

export default function AiLiveControlPage() {
  const base = useMemo(() => "/api/ai-live", []);
  const [status, setStatus] = useState<AiLiveStatus | null>(null);
  const [operatorToken, setOperatorToken] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const response = await fetch(`${base}/status`, { cache: "no-store" });
      if (!response.ok) throw new Error(`Gateway 返回 ${response.status}`);
      setStatus(await response.json() as AiLiveStatus);
      setError(null);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "无法连接 Gateway");
    }
  }, [base]);

  useEffect(() => {
    const saved = window.sessionStorage.getItem("mir2.aiLiveOperatorToken");
    if (saved) setOperatorToken(saved);
    void refresh();
    const timer = window.setInterval(() => void refresh(), 2_000);
    return () => window.clearInterval(timer);
  }, [refresh]);

  async function control(action: "live" | "shadow" | "pause") {
    if (!operatorToken.trim()) {
      setError("请先输入导播令牌。令牌只保存在当前浏览器标签页。");
      return;
    }
    setBusy(true);
    try {
      window.sessionStorage.setItem("mir2.aiLiveOperatorToken", operatorToken);
      const response = await fetch(`${base}/control`, {
        method: "POST",
        headers: {
          "content-type": "application/json",
          authorization: `Bearer ${operatorToken}`,
        },
        body: JSON.stringify({ action }),
      });
      const body = await response.json() as AiLiveStatus | { error?: string };
      if (!response.ok) {
        throw new Error("error" in body ? body.error ?? "控制失败" : "控制失败");
      }
      setStatus(body as AiLiveStatus);
      setError(null);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "控制失败");
    } finally {
      setBusy(false);
    }
  }

  async function channelControl(
    channel: AiDistributionChannel,
    action: "enable" | "disable" | "retry",
  ) {
    if (!operatorToken.trim()) {
      setError("请先输入导播令牌。令牌只保存在当前浏览器标签页。");
      return;
    }
    setBusy(true);
    try {
      window.sessionStorage.setItem("mir2.aiLiveOperatorToken", operatorToken);
      const response = await fetch(`${base}/distribution`, {
        method: "POST",
        headers: {
          "content-type": "application/json",
          authorization: `Bearer ${operatorToken}`,
        },
        body: JSON.stringify({ channel, action }),
      });
      const body = await response.json() as { error?: string };
      if (!response.ok) throw new Error(body.error ?? "渠道控制失败");
      await refresh();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "渠道控制失败");
    } finally {
      setBusy(false);
    }
  }

  const segment = status?.latestSegment;
  const broadcastUrl = "/spectate?aiLive=1&spectateMode=director";

  return (
    <main style={{
      minHeight: "100vh",
      padding: "clamp(24px, 5vw, 72px)",
      background: "radial-gradient(circle at 70% 0%, #10283a 0, #07121e 34%, #030811 76%)",
      color: "#eafaff",
      fontFamily: '"Inter", "PingFang SC", "Microsoft YaHei", sans-serif',
    }}>
      <div style={{ maxWidth: 1120, margin: "0 auto" }}>
        <header style={{ display: "flex", justifyContent: "space-between", alignItems: "end", gap: 24, flexWrap: "wrap" }}>
          <div>
            <p style={{ margin: 0, color: "#62dbef", letterSpacing: "0.24em", fontSize: 11 }}>
              DUBHE BROADCAST OPERATIONS
            </p>
            <h1 style={{ margin: "12px 0 6px", fontSize: "clamp(34px, 6vw, 70px)", lineHeight: 1 }}>
              AI 直播控制台
            </h1>
            <p style={{ margin: 0, color: "#7e96a7" }}>
              影子彩排、正式直播与安全暂停。玩家服务不经过本控制面。
            </p>
          </div>
          <div style={{
            padding: "10px 14px",
            border: "1px solid rgba(103, 225, 244, .22)",
            borderRadius: 999,
            background: "rgba(6, 18, 31, .78)",
            color: status?.running ? "#72efc3" : "#ffbf73",
            fontSize: 12,
          }}>
            ● {status?.running ? "Gateway 工作器在线" : "等待 Gateway"}
          </div>
        </header>

        {error ? (
          <div role="alert" style={{ marginTop: 24, padding: 14, borderLeft: "3px solid #ff6b78", background: "rgba(96, 20, 32, .3)", color: "#ffadb5" }}>
            {error}
          </div>
        ) : null}

        <section style={{
          display: "grid",
          gridTemplateColumns: "repeat(auto-fit, minmax(150px, 1fr))",
          gap: 12,
          marginTop: 30,
        }}>
          {[
            ["模式", status?.mode ?? "offline"],
            ["片段", number(status?.metrics.generatedSegmentsTotal)],
            ["模型降级", number(status?.metrics.modelFailureTotal)],
            ["语音降级", number(status?.metrics.ttsFailureTotal)],
            ["待分发", number(status?.metrics.queuedDistributionDeliveries)],
          ].map(([label, value]) => (
            <article key={label} style={{ minHeight: 108, padding: 18, border: "1px solid rgba(130, 205, 226, .13)", borderRadius: 14, background: "rgba(8, 21, 35, .72)" }}>
              <span style={{ color: "#627d8f", fontSize: 10, letterSpacing: ".12em" }}>{label}</span>
              <strong style={{ display: "block", marginTop: 18, fontSize: 24 }}>{value}</strong>
            </article>
          ))}
        </section>

        <section style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(min(100%, 360px), 1fr))", gap: 16, marginTop: 16 }}>
          <article style={{ minHeight: 300, padding: 26, border: "1px solid rgba(130, 205, 226, .13)", borderRadius: 18, background: "rgba(8, 21, 35, .76)" }}>
            <span style={{ color: "#62dbef", fontSize: 10, letterSpacing: ".18em" }}>CURRENT PROGRAM</span>
            <h2 style={{ margin: "18px 0 8px", fontSize: 30 }}>{segment?.subtitle ?? "等待高光事件"}</h2>
            <p style={{ minHeight: 58, color: "#9bb1bf", lineHeight: 1.75 }}>{segment?.commentary ?? "系统会继续观察脱敏赛事流，普通移动不会触发解说。"}</p>
            <div style={{ display: "flex", gap: 10, flexWrap: "wrap", marginTop: 28, color: "#6f8797", fontSize: 11 }}>
              <span>{segment?.mapTitle ?? "无地图"}</span>
              <span>·</span>
              <span>{segment?.target ?? "自动镜头"}</span>
              <span>·</span>
              <span>{segment ? `${segment.score} HYPE` : "-- HYPE"}</span>
            </div>
            <a href={broadcastUrl} target="_blank" rel="noreferrer" style={{ display: "inline-block", marginTop: 26, color: "#73e5f7" }}>
              打开干净播出画面 ↗
            </a>
          </article>

          <aside style={{ padding: 24, border: "1px solid rgba(130, 205, 226, .13)", borderRadius: 18, background: "rgba(8, 21, 35, .76)" }}>
            <form onSubmit={(event) => event.preventDefault()}>
              <label style={{ display: "grid", gap: 8, color: "#7f98a8", fontSize: 11 }}>
                导播令牌
                <input
                  type="password"
                  value={operatorToken}
                  onChange={(event) => setOperatorToken(event.target.value)}
                  placeholder="仅当前标签页保存"
                  autoComplete="off"
                  style={{ padding: "12px 13px", border: "1px solid rgba(130, 205, 226, .18)", borderRadius: 8, background: "#06111d", color: "#eafaff", font: "inherit" }}
                />
              </label>
              <div style={{ display: "grid", gap: 9, marginTop: 18 }}>
                {([
                  ["live", "开始正式直播", "#54e1bd"],
                  ["shadow", "进入影子彩排", "#71dff2"],
                  ["pause", "安全暂停", "#ffbd70"],
                ] as const).map(([action, label, color]) => (
                  <button
                    key={action}
                    type="button"
                    disabled={busy}
                    onClick={() => void control(action)}
                    style={{ minHeight: 44, border: `1px solid ${color}55`, borderRadius: 9, background: `${color}12`, color, cursor: "pointer", font: "inherit" }}
                  >
                    {label}
                  </button>
                ))}
              </div>
              <p style={{ margin: "20px 0 0", color: "#566f80", fontSize: 10, lineHeight: 1.6 }}>
                正式直播才会生成语音并进入多渠道分发；影子模式只验证选题与文案。
              </p>
            </form>
          </aside>
        </section>

        <section style={{ marginTop: 16, padding: 24, border: "1px solid rgba(130, 205, 226, .13)", borderRadius: 18, background: "rgba(8, 21, 35, .76)" }}>
          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "end", gap: 16, flexWrap: "wrap" }}>
            <div>
              <span style={{ color: "#62dbef", fontSize: 10, letterSpacing: ".18em" }}>DISTRIBUTION FABRIC</span>
              <h2 style={{ margin: "10px 0 0", fontSize: 26 }}>节目分发渠道</h2>
            </div>
            <p style={{ margin: 0, color: "#6f8797", fontSize: 11 }}>
              一次生成，多渠道投递；任一渠道失败不会影响玩家
            </p>
          </div>
          {status?.distribution.launch ? (
            <div
              data-testid="ai-launch-readiness"
              style={{
                display: "flex",
                justifyContent: "space-between",
                gap: 18,
                marginTop: 18,
                padding: 16,
                border: `1px solid ${status.distribution.launch.readyForLaunch ? "#54e1bd44" : "#f2c86b33"}`,
                borderRadius: 12,
                background: status.distribution.launch.readyForLaunch ? "#54e1bd0d" : "#f2c86b0c",
                flexWrap: "wrap",
              }}
            >
              <div>
                <strong>
                  首发渠道 {status.distribution.launch.readyChannels}/{status.distribution.launch.requiredChannels}
                </strong>
                <p style={{ margin: "6px 0 0", color: "#718a9a", fontSize: 11 }}>
                  Web/HLS、游戏内事件、Discord、YouTube RTMPS
                </p>
              </div>
              <div style={{ color: status.distribution.launch.readyForLaunch ? "#54e1bd" : "#f2c86b", fontSize: 12 }}>
                {status.distribution.launch.readyForLaunch
                  ? "全部收到真实运行证据"
                  : `仍需验证 ${status.distribution.launch.blockers.length} 项`}
              </div>
            </div>
          ) : null}
          <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(240px, 1fr))", gap: 12, marginTop: 20 }}>
            {(status?.distribution.channels ?? []).map((channel) => {
              const color = channel.state === "ready"
                ? "#54e1bd"
                : channel.state === "degraded"
                  ? "#ff9d85"
                  : channel.state === "waiting"
                    ? "#f2c86b"
                  : "#708899";
              return (
                <article key={channel.channel} style={{ padding: 18, border: `1px solid ${color}33`, borderRadius: 14, background: "rgba(3, 12, 22, .66)" }}>
                  <div style={{ display: "flex", justifyContent: "space-between", gap: 12 }}>
                    <strong>
                      {channel.label}
                      {channel.launchRequired ? <small style={{ marginLeft: 7, color: "#5f7a8c" }}>首发</small> : null}
                    </strong>
                    <span style={{ color, fontSize: 10 }}>{channelStateLabel(channel.state)}</span>
                  </div>
                  <p style={{ margin: "12px 0", color: "#657f91", fontSize: 11 }}>
                    {channel.deliveryMode} · 成功 {number(channel.deliveredTotal)} · 等待 {number(channel.queued)}
                  </p>
                  {channel.runtime ? (
                    <p style={{ margin: "0 0 12px", color: "#7193a6", fontSize: 10, lineHeight: 1.55 }}>
                      {channel.runtime.platform} · {channel.runtime.workerId}<br />
                      心跳 {new Date(channel.runtime.lastHeartbeatAtMs).toLocaleTimeString("zh-CN")}
                    </p>
                  ) : channel.channel === "rtmpBroadcast" && channel.configured ? (
                    <p style={{ margin: "0 0 12px", color: "#f2c86b", fontSize: 10 }}>
                      等待 YouTube 编码器心跳，不会把开关当成在线
                    </p>
                  ) : null}
                  {channel.lastError ? (
                    <p style={{ margin: "0 0 12px", color: "#ff9d85", fontSize: 10 }}>
                      {channel.lastError}
                    </p>
                  ) : null}
                  <div style={{ display: "flex", gap: 8 }}>
                    <button
                      type="button"
                      disabled={busy || !channel.configured}
                      onClick={() => void channelControl(channel.channel, channel.enabled ? "disable" : "enable")}
                      style={{ minHeight: 34, padding: "0 12px", border: `1px solid ${color}44`, borderRadius: 8, background: `${color}12`, color, cursor: channel.configured ? "pointer" : "not-allowed" }}
                    >
                      {!channel.configured ? "待配置" : channel.enabled ? "暂停渠道" : "启用渠道"}
                    </button>
                    {channel.queued > 0 ? (
                      <button
                        type="button"
                        disabled={busy}
                        onClick={() => void channelControl(channel.channel, "retry")}
                        style={{ minHeight: 34, padding: "0 12px", border: "1px solid #71dff244", borderRadius: 8, background: "#71dff212", color: "#71dff2", cursor: "pointer" }}
                      >
                        立即重试
                      </button>
                    ) : null}
                  </div>
                </article>
              );
            })}
          </div>
          <p style={{ margin: "18px 0 0", color: "#526a7a", fontSize: 10, lineHeight: 1.7 }}>
            后续阶段保持关闭：Discord Go Live、短视频 Clip、Twitch、Bilibili。它们不影响首发验收。
          </p>
        </section>
      </div>
    </main>
  );
}
