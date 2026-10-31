import SignClient from '@walletconnect/sign-client';

const CHAIN = 'sui:testnet';
const METHODS = ['sui_signPersonalMessage', 'sui_signAndExecuteTransaction'];
const EVENTS = ['accountsChanged', 'chainChanged'];
const RELAY = 'wss://relay.walletconnect.com';
const FEATURES = ['sui:signPersonalMessage', 'sui:signAndExecuteTransaction'];
const invalid = (message) => new Error(message);

function canonicalOrigin(value) {
  if (typeof value !== 'string' || value.length > 512 || /[^\x21-\x7e]|\\/.test(value)) throw invalid('Invalid checkout origin');
  const url = new URL(value);
  const local = ['127.0.0.1', 'localhost', '[::1]'].includes(url.hostname);
  if (url.origin !== value || url.username || url.password || url.search || url.hash || url.pathname !== '/' ||
      (url.protocol !== 'https:' && !(url.protocol === 'http:' && local))) throw invalid('Invalid checkout origin');
  return value;
}

function canonicalBase64(value, maximum = 256 * 1024) {
  if (typeof value !== 'string' || !value || value.length > maximum || value.length % 4 ||
      !/^[A-Za-z0-9+/]+={0,2}$/.test(value)) throw invalid('Invalid wallet bytes');
  if (btoa(atob(value)) !== value) throw invalid('Invalid wallet bytes');
  return value;
}

function messageBase64(message) {
  if (!(message instanceof Uint8Array) || !message.length || message.length > 16384) throw invalid('Invalid wallet message');
  return btoa(Array.from(message, (byte) => String.fromCharCode(byte)).join(''));
}

function approvedAccounts(session, now) {
  if (!session || !/^[a-f0-9]{64}$/.test(session.topic || '') || !Number.isSafeInteger(session.expiry) ||
      !Number.isSafeInteger(session.expiry * 1000) || session.expiry * 1000 <= now || !session.namespaces || Object.keys(session.namespaces).length !== 1) {
    throw invalid('Invalid or expired wallet session');
  }
  const sui = session.namespaces.sui;
  if (!sui || !Array.isArray(sui.methods) || !METHODS.every((method) => sui.methods.includes(method)) ||
      !Array.isArray(sui.events) || !EVENTS.every((event) => sui.events.includes(event)) ||
      (sui.chains !== undefined && (!Array.isArray(sui.chains) || sui.chains.length !== 1 || sui.chains[0] !== CHAIN)) ||
      !Array.isArray(sui.accounts) || !sui.accounts.length || sui.accounts.length > 32 || new Set(sui.accounts).size !== sui.accounts.length) {
    throw invalid('Wallet did not approve the required Sui Testnet session');
  }
  return sui.accounts.map((value) => {
    if (typeof value !== 'string' || !/^sui:testnet:0x[0-9a-f]{64}$/.test(value) || /^sui:testnet:0x0{64}$/.test(value)) {
      throw invalid('Wallet approved an invalid account or network');
    }
    return { address: value.slice('sui:testnet:'.length), chains: [CHAIN], features: [...FEATURES] };
  });
}

function approvalUri(uri, now) {
  if (typeof uri !== 'string' || uri.length > 2048 || !/^wc:[a-f0-9]{64}@2\?/.test(uri)) throw invalid('Invalid wallet approval URI');
  const parsed = new URL(uri);
  const params = parsed.searchParams;
  if (parsed.hash || params.getAll('relay-protocol').length !== 1 || params.get('relay-protocol') !== 'irn' ||
      params.getAll('symKey').length !== 1 || !/^[a-f0-9]{64}$/.test(params.get('symKey') || '') ||
      [...params.keys()].some((key) => !['relay-protocol', 'symKey', 'expiryTimestamp'].includes(key)) ||
      params.getAll('expiryTimestamp').length > 1) throw invalid('Invalid wallet approval URI');
  const raw = params.get('expiryTimestamp');
  if (raw !== null && !/^[1-9][0-9]*$/.test(raw)) throw invalid('Invalid wallet approval expiry');
  const expiresAt = raw === null ? now + 300000 : Number(raw) * 1000;
  if (!Number.isSafeInteger(expiresAt) || expiresAt <= now) throw invalid('Wallet approval expired');
  return Math.min(expiresAt, now + 300000);
}

/** Checkout capabilities and game credentials are deliberately absent here. */
export function createDubheWallet(options, dependencies = {}) {
  if (!options || !/^[a-fA-F0-9]{32}$/.test(options.projectId || '')) throw invalid('Invalid wallet project configuration');
  const origin = canonicalOrigin(options.origin);
  const projectId = options.projectId;
  const init = dependencies.initSignClient || ((configuration) => SignClient.init(configuration));
  const now = dependencies.now || Date.now;
  const schedule = dependencies.setTimer || setTimeout;
  const cancel = dependencies.clearTimer || clearTimeout;
  let client, session, accounts, connecting, stopped = false, generation = 0, expiryTimer, signing;
  const pending = new Set();
  const listeners = [];

  function notify(name, ...args) {
    try { Promise.resolve(options[name]?.(...args)).catch(() => {}); } catch { /* UI callbacks do not own wallet state. */ }
  }
  function removeListeners() {
    for (const [name, listener] of listeners.splice(0)) client?.off(name, listener);
  }
  function closeSession(value) {
    if (!value?.topic || !client) return;
    try { Promise.resolve(client.disconnect({ topic: value.topic, reason: { code: 6000, message: 'Checkout disconnected' } })).catch(() => {}); } catch { /* No retry or payment execution. */ }
  }
  function invalidate(reason) {
    generation++;
    if (expiryTimer !== undefined) cancel(expiryTimer);
    expiryTimer = undefined;
    const old = session;
    session = undefined; accounts = undefined; connecting = undefined; signing = undefined;
    removeListeners();
    for (const reject of [...pending]) reject(invalid(reason === 'expired' ? 'Wallet approval or session expired' : 'Wallet connection changed or disconnected'));
    closeSession(old);
    notify(reason === 'expired' ? 'onExpired' : 'onDisconnect');
  }
  function guarded(value, stamp, onLate) {
    return new Promise((resolve, reject) => {
      if (stopped || stamp !== generation) {
        Promise.resolve(value).then((result) => { try { onLate?.(result); } catch { /* Retire late sessions. */ } }, () => {});
        reject(invalid('Wallet response is no longer current'));
        return;
      }
      let settled = false;
      const finish = (error, result) => {
        if (settled) return;
        settled = true; pending.delete(abort);
        if (error) reject(error); else resolve(result);
      };
      const abort = (error) => finish(error);
      pending.add(abort);
      Promise.resolve(value).then((result) => {
        if (stopped || stamp !== generation) {
          try { onLate?.(result); } catch { /* Late approval is never accepted. */ }
          finish(invalid('Wallet response is no longer current'));
        } else finish(undefined, result);
      }, () => finish(invalid('Wallet request rejected or unavailable')));
    });
  }
  function listen() {
    if (listeners.length) return;
    const subscribe = (name, listener) => { client.on(name, listener); listeners.push([name, listener]); };
    for (const name of ['session_delete', 'session_expire']) subscribe(name, (event) => {
      if (session && event?.topic === session.topic) invalidate(name === 'session_expire' ? 'expired' : 'disconnected');
    });
    subscribe('session_event', (event) => {
      if (session && event?.topic === session.topic && EVENTS.includes(event.params?.event?.name)) invalidate('changed');
    });
    subscribe('session_update', (event) => {
      if (session && event?.topic === session.topic) invalidate('changed');
    });
  }
  async function connectOnce() {
    if (stopped) throw invalid('Wallet adapter is disconnected');
    if (session) return { accounts: approvedAccounts(session, now()) };
    const stamp = generation;
    try {
      expiryTimer = schedule(() => invalidate('expired'), 30000);
      if (!client) client = await guarded(init({ projectId, relayUrl: RELAY, telemetryEnabled: false,
        metadata: { name: 'Numeron USDC checkout', description: 'Confirm Sui Testnet recharge wallet and payment', url: origin, icons: [] } }), stamp);
      listen();
      const proposal = await guarded(client.connect({ requiredNamespaces: { sui: { chains: [CHAIN], methods: [...METHODS], events: [...EVENTS] } } }), stamp);
      if (!proposal || typeof proposal.approval !== 'function') throw invalid('Invalid wallet approval proposal');
      const expiresAt = approvalUri(proposal.uri, now());
      if (expiryTimer !== undefined) cancel(expiryTimer);
      expiryTimer = schedule(() => invalidate('expired'), expiresAt - now());
      notify('onUri', proposal.uri, { expiresAt });
      const approved = await guarded(proposal.approval(), stamp, closeSession);
      try { accounts = approvedAccounts(approved, now()); } catch (error) { closeSession(approved); throw error; }
      if (expiryTimer !== undefined) cancel(expiryTimer);
      session = approved;
      expiryTimer = schedule(() => invalidate('expired'), Math.min(approved.expiry * 1000 - now(), 2147483647));
      return { accounts: accounts.map((account) => ({ ...account, chains: [...account.chains], features: [...account.features] })) };
    } catch (error) {
      if (stamp === generation) invalidate('disconnected');
      throw error;
    }
  }
  function connect() {
    if (connecting) return connecting;
    const work = connectOnce(); connecting = work;
    work.then(() => { if (connecting === work) connecting = undefined; }, () => { if (connecting === work) connecting = undefined; });
    return work;
  }
  async function request(account, method, params) {
    if (stopped || !session || session.expiry * 1000 <= now()) throw invalid('Wallet is not connected or has expired');
    const approved = approvedAccounts(session, now());
    if (!account || !approved.some((item) => item.address === account.address) || !Array.isArray(account.chains) ||
        account.chains.length !== 1 || account.chains[0] !== CHAIN) throw invalid('Selected wallet account or network changed');
    if (signing) throw invalid('Another wallet approval is already pending');
    const stamp = generation, token = {}; signing = token;
    const timer = schedule(() => invalidate('expired'), Math.min(120000, session.expiry * 1000 - now()));
    try {
      const result = await guarded(client.request({ topic: session.topic, chainId: CHAIN, request: { method, params } }), stamp);
      if (stopped || stamp !== generation || !session || session.expiry * 1000 <= now()) throw invalid('Wallet response is no longer current');
      return { result, stamp };
    } finally {
      cancel(timer);
      if (signing === token) signing = undefined;
    }
  }
  return Object.freeze({
    connect,
    async signPersonalMessage({ account, message }) {
      const bytes = messageBase64(message);
      const { result, stamp } = await request(account, METHODS[0], { message: bytes });
      if (canonicalBase64(result?.bytes, 32768) !== bytes) throw invalid('Wallet returned a different message');
      const signature = canonicalBase64(result?.signature, 2048);
      if (stopped || stamp !== generation) throw invalid('Wallet response is no longer current');
      return { signature };
    },
    async signAndExecuteTransaction({ account, transactionBytesB64 }) {
      const bytes = canonicalBase64(transactionBytesB64);
      const { result, stamp } = await request(account, METHODS[1], { transactionBlock: bytes });
      if (canonicalBase64(result?.bytes) !== bytes || typeof result?.digest !== 'string' ||
          !/^[1-9A-HJ-NP-Za-km-z]{32,64}$/.test(result.digest)) throw invalid('Wallet returned a different or invalid transaction');
      const signature = canonicalBase64(result.signature, 4096);
      if (stopped || stamp !== generation) throw invalid('Wallet response is no longer current');
      return { digest: result.digest, bytes, signature };
    },
    async disconnect() {
      if (stopped) return;
      stopped = true; invalidate('disconnected');
    },
  });
}

if (typeof window !== 'undefined') window.Mir2DubheWallet = Object.freeze({ create: (options) => createDubheWallet(options) });
