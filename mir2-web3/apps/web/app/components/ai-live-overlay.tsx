"use client";

import { useEffect, useMemo, useRef, useState } from "react";

export type AiLiveSegment = {
  schema: string;
  segmentId: string;
  createdAtMs: number;
  mapFileName: string;
  mapTitle: string;
  target: string | null;
  score: number;
  reason: string;
  commentary: string;
  subtitle: string;
  source: "model" | "deterministicFallback";
  model: string | null;
  audioUrl: string | null;
  frameDigest: string;
  frameSequence: number;
  eventKinds: string[];
};

export type AiDistributionChannel =
  | "gameOverlay"
  | "webBroadcast"
  | "rtmpBroadcast"
  | "discordWebhook"
  | "discordGoLive"
  | "clipExport";

export type AiChannelStatus = {
  channel: AiDistributionChannel;
  label: string;
  deliveryMode: "inProcess" | "pull" | "relay" | "push";
  configured: boolean;
  enabled: boolean;
  state: "ready" | "disabled" | "degraded" | "unconfigured";
  queued: number;
  deliveredTotal: number;
  failureTotal: number;
  deadLettersTotal: number;
  lastSuccessAtMs: number | null;
  lastFailureAtMs: number | null;
  lastError: string | null;
};

export type AiDistributionStatus = {
  schema: string;
  channels: AiChannelStatus[];
  recentReceipts: Array<{
    jobId: string;
    contentId: string;
    channel: AiDistributionChannel;
    deliveredAtMs: number;
    attempts: number;
  }>;
  metrics: {
    deliveredTotal: number;
    failureTotal: number;
    deadLettersTotal: number;
    queuedDeliveries: number;
  };
};

export type AiLiveStatus = {
  schema: string;
  enabled: boolean;
  mode: "shadow" | "live" | "paused";
  running: boolean;
  latestSegment: AiLiveSegment | null;
  recentSegments: AiLiveSegment[];
  providers: {
    textConfigured: boolean;
    ttsConfigured: boolean;
    discordConfigured: boolean;
    broadcastUrlConfigured: boolean;
  };
  metrics: {
    generatedSegmentsTotal: number;
    modelFailureTotal: number;
    ttsFailureTotal: number;
    distributionSuccessTotal: number;
    distributionFailureTotal: number;
    distributionDeadLettersTotal: number;
    queuedDistributionDeliveries: number;
    queuedDiscordDeliveries: number;
  };
  distribution: AiDistributionStatus;
};

type AiLiveOverlayProps = {
  status: AiLiveStatus | null;
  gatewayWebSocketUrl: string;
  audioEnabled: boolean;
};

function resolveAudioUrl(path: string | null, gatewayWebSocketUrl: string) {
  if (!path || typeof window === "undefined") return null;
  if (/^https?:\/\//i.test(path)) return path;
  try {
    const gateway = new URL(gatewayWebSocketUrl, window.location.href);
    gateway.protocol = gateway.protocol === "wss:" ? "https:" : "http:";
    gateway.pathname = path.startsWith("/") ? path : `/${path}`;
    gateway.search = "";
    gateway.hash = "";
    return gateway.toString();
  } catch {
    return null;
  }
}

export function AiLiveOverlay({
  status,
  gatewayWebSocketUrl,
  audioEnabled,
}: AiLiveOverlayProps) {
  const segment = status?.latestSegment ?? null;
  const [audioMuted, setAudioMuted] = useState(!audioEnabled);
  const audioRef = useRef<HTMLAudioElement | null>(null);
  const audioUrl = useMemo(
    () => resolveAudioUrl(segment?.audioUrl ?? null, gatewayWebSocketUrl),
    [gatewayWebSocketUrl, segment?.audioUrl],
  );

  useEffect(() => {
    setAudioMuted(!audioEnabled);
  }, [audioEnabled]);

  useEffect(() => {
    const audio = audioRef.current;
    if (!audio || !audioUrl || audioMuted) return;
    audio.currentTime = 0;
    void audio.play().catch(() => {
      setAudioMuted(true);
    });
  }, [audioMuted, audioUrl, segment?.segmentId]);

  return (
    <section
      className="ai-live-broadcast"
      data-testid="ai-live-overlay"
      aria-label="Dubhe AI 直播"
    >
      <header className="ai-live-broadcast__header">
        <div className="ai-live-broadcast__brand">
          <span className={status?.mode === "live" ? "is-live" : ""} />
          DUBHE AI LIVE
        </div>
        <div className="ai-live-broadcast__map">
          {segment?.mapTitle ?? "等待世界事件"}
        </div>
      </header>

      <div className="ai-live-broadcast__status">
        <span>{status?.mode === "live" ? "直播" : status?.mode === "shadow" ? "彩排" : "暂停"}</span>
        <b>{segment ? `${segment.score} HYPE` : "-- HYPE"}</b>
      </div>

      {segment ? (
        <div
          key={segment.segmentId}
          className="ai-live-broadcast__lower-third"
          data-testid="ai-live-lower-third"
        >
          <div className="ai-live-broadcast__eyebrow">
            <span>{segment.reason}</span>
            {segment.target ? <strong>镜头 · {segment.target}</strong> : null}
          </div>
          <h2>{segment.subtitle}</h2>
          <p>{segment.commentary}</p>
          <small>
            {segment.source === "model" ? "AI 解说" : "规则解说"} · #{segment.frameSequence}
          </small>
        </div>
      ) : (
        <div className="ai-live-broadcast__waiting">
          <span />
          正在观察世界，等待值得直播的事件
        </div>
      )}

      {audioUrl ? (
        <>
          <audio ref={audioRef} src={audioUrl} preload="auto" muted={audioMuted} />
          <button
            type="button"
            className="ai-live-broadcast__audio"
            data-testid="ai-live-audio"
            onClick={() => setAudioMuted((muted) => !muted)}
          >
            {audioMuted ? "开启 AI 语音" : "AI 语音已开启"}
          </button>
        </>
      ) : null}

      <footer className="ai-live-broadcast__footer">
        <span>只读观战</span>
        <span>不接触玩家控制链路</span>
      </footer>
    </section>
  );
}
