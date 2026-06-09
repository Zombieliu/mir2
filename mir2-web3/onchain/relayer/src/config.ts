import 'dotenv/config';

export type SuiNetwork = 'testnet' | 'mainnet' | 'devnet' | 'localnet';

export interface RelayerConfig {
  network: SuiNetwork;
  packageId: string;
  /** Move modules whose events we bridge (DESIGN §4: mine + redeem). */
  modules: string[];
  pollIntervalMs: number;
  stateDir: string;
  /** M3: gateway/admin-api injection endpoint. Absent => log-only sink (M2 default). */
  injectUrl?: string;
  /** M3: operator token authorising injection (env only, never committed). */
  operatorToken?: string;
}

export function loadConfig(): RelayerConfig {
  const packageId = process.env.MINE_PACKAGE_ID;
  if (!packageId) {
    throw new Error('Missing MINE_PACKAGE_ID (set it in onchain/.env or the environment).');
  }
  return {
    network: (process.env.SUI_NETWORK ?? 'testnet') as SuiNetwork,
    packageId,
    modules: (process.env.RELAYER_MODULES ?? 'mine_system,redeem_system')
      .split(',')
      .map((m) => m.trim())
      .filter(Boolean),
    pollIntervalMs: Number(process.env.RELAYER_POLL_MS ?? 4000),
    stateDir: process.env.RELAYER_STATE_DIR ?? 'relayer/.state',
    injectUrl: process.env.GATEWAY_INJECT_URL || undefined,
    operatorToken: process.env.OPERATOR_TOKEN || undefined,
  };
}
