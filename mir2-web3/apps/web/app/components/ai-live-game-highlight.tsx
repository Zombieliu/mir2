"use client";

import { useEffect, useMemo, useState } from "react";
import type { AiLiveStatus } from "./ai-live-overlay";

type AiLiveGameHighlightProps = {
  status: AiLiveStatus | null;
};

export function AiLiveGameHighlight({ status }: AiLiveGameHighlightProps) {
  const segment = status?.latestSegment ?? null;
  const gameChannel = status?.distribution.channels.find(
    (channel) => channel.channel === "gameOverlay",
  );
  const [hiddenSegmentId, setHiddenSegmentId] = useState<string | null>(null);

  useEffect(() => {
    if (!segment?.segmentId) return;
    const timer = window.setTimeout(
      () => setHiddenSegmentId(segment.segmentId),
      12_000,
    );
    return () => window.clearTimeout(timer);
  }, [segment?.segmentId]);

  const watchUrl = useMemo(() => {
    if (!segment) return "/spectate?aiLive=1";
    const params = new URLSearchParams({
      spectate: "1",
      aiLive: "1",
      spectateMap: segment.mapFileName,
    });
    return `/spectate?${params.toString()}`;
  }, [segment]);

  if (
    !segment
    || status?.mode !== "live"
    || !gameChannel?.enabled
    || gameChannel.state !== "ready"
    || hiddenSegmentId === segment.segmentId
  ) {
    return null;
  }

  return (
    <aside
      className="ai-live-game-highlight"
      data-testid="ai-live-game-highlight"
      aria-label="AI 世界事件"
    >
      <div className="ai-live-game-highlight__pulse" aria-hidden="true" />
      <div className="ai-live-game-highlight__copy">
        <span>AI 世界事件 · {segment.mapTitle}</span>
        <strong>{segment.subtitle}</strong>
        <small>{segment.reason} · {segment.score} HYPE</small>
      </div>
      <a href={watchUrl} target="_blank" rel="noreferrer">
        立即观战
      </a>
      <button
        type="button"
        aria-label="关闭 AI 世界事件"
        onClick={() => setHiddenSegmentId(segment.segmentId)}
      >
        ×
      </button>
    </aside>
  );
}
