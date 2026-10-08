/** Spectators may use a different realm without redirecting player credentials. */
export function spectatorWebSocketUrl(
  playerWebSocketUrl: string,
  configuredSpectatorUrl: string | undefined,
  pageUrl: string,
): string {
  const explicit = configuredSpectatorUrl?.trim();
  const url = new URL(explicit || playerWebSocketUrl);
  if (!['ws:', 'wss:'].includes(url.protocol) || url.username || url.password) {
    throw new Error('Spectator endpoint must be a WebSocket URL without embedded credentials');
  }
  if (explicit) {
    if (!url.pathname.endsWith('/spectator/ws') || url.search || url.hash) {
      throw new Error('Configured spectator endpoint must end in /spectator/ws without query parameters');
    }
  } else {
    if (!/\/ws\/?$/.test(url.pathname)) {
      throw new Error('Player endpoint must end in /ws to derive the spectator endpoint');
    }
    url.pathname = url.pathname.replace(/\/ws\/?$/, '/spectator/ws');
    // Player connection parameters are not spectator authentication.
    url.search = '';
    url.hash = '';
  }
  const query = new URL(pageUrl).searchParams;
  for (const [source, destination] of [
    ['spectateMap', 'map'],
    ['spectateTarget', 'target'],
    ['spectateDelayMs', 'delayMs'],
    ['spectateMode', 'mode'],
    ['spectateToken', 'token'],
    ['replayId', 'replayId'],
  ]) {
    const value = query.get(source);
    if (value) url.searchParams.set(destination, value);
  }
  return url.toString();
}

/** Keep realm prefixes such as /playtest when deriving an HTTP API base. */
export function gatewayHttpBase(websocketUrl: string): string {
  const url = new URL(websocketUrl);
  if (!['ws:', 'wss:'].includes(url.protocol) || url.username || url.password) {
    throw new Error('Invalid Gateway WebSocket endpoint');
  }
  const suffix = url.pathname.endsWith('/spectator/ws') ? /\/spectator\/ws$/ : /\/ws\/?$/;
  if (!suffix.test(url.pathname)) throw new Error('Gateway endpoint does not end in /ws');
  url.protocol = url.protocol === 'wss:' ? 'https:' : 'http:';
  url.pathname = url.pathname.replace(suffix, '').replace(/\/+$/, '');
  url.search = '';
  url.hash = '';
  return url.toString().replace(/\/+$/, '');
}
