export type SpectatorLiveState = 'waiting' | 'buffering' | 'live' | 'stale' | 'replay';

export function spectatorLiveState(
  status: {
    map: string;
    delayMs: number;
    capturedAtMs: number | null;
    matches: Array<{ mapFileName: string; latestCapturedAtMs: number }>;
    replay: { active: boolean };
  } | null,
  nowMs: number,
): SpectatorLiveState {
  if (!status) return 'waiting';
  if (status.replay.active) return 'replay';
  const latest = status.matches.find((match) => match.mapFileName === status.map)?.latestCapturedAtMs;
  const reference = latest || status.capturedAtMs;
  if (!reference) return 'waiting';
  // Directory timestamps are current capture time, whereas delivered frames
  // already include the enforced public delay. Do not mistake that delay for a stall.
  const allowance = latest && status.capturedAtMs ? 10_000 : Math.max(0, status.delayMs) + 10_000;
  if (nowMs - reference > allowance) return 'stale';
  return status.capturedAtMs ? 'live' : 'buffering';
}
