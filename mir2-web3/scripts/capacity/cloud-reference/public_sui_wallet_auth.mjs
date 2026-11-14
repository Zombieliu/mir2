// Official public wallet login flow. Only an unfunded synthetic Ed25519 personal-message
// signer lives in the private actor file. No production signing key or chain RPC is used.
import { createRequire } from 'node:module';
import { pathToFileURL } from 'node:url';
import { performance } from 'node:perf_hooks';
const requireWeb = createRequire(new URL('../../../apps/web/package.json', import.meta.url));
const { Ed25519Keypair } = await import(pathToFileURL(requireWeb.resolve('@mysten/sui/keypairs/ed25519')).href);
const { normalizeSuiAddress } = await import(pathToFileURL(requireWeb.resolve('@mysten/sui/utils')).href);
const check = (value, message) => { if (!value) throw new Error(message); };
export const PUBLIC_AUTH_HTTP_TIMEOUT_MS = 8000;
const PUBLIC_AUTH_ROUTES = ['/api/passkey/challenge', '/api/passkey/login', '/api/channels/session/exchange'];
// Constructed only around fetch(), never from response bodies or credentials.
export class PublicAuthFetchTimeout extends Error {
  constructor(route, elapsedMs) {
    check(PUBLIC_AUTH_ROUTES.includes(route) && Number.isFinite(elapsedMs) && elapsedMs >= 0, 'Invalid public auth timeout metadata');
    super('Public wallet authentication fetch timed out at ' + route);
    this.name = 'PublicAuthFetchTimeout'; this.code = 'PUBLIC_AUTH_FETCH_TIMEOUT';
    this.route = route; this.elapsedMs = elapsedMs; this.timeoutMs = PUBLIC_AUTH_HTTP_TIMEOUT_MS;
  }
}

export function migrateGuestPool(pool) {
  if (pool.schema === 'mir2.cloud-reference-private-actors.v2') return pool;
  check(pool.schema === 'mir2.cloud-reference-private-guests.v1', 'Unknown private actor schema');
  const migrated = structuredClone(pool);
  migrated.schema = 'mir2.cloud-reference-private-actors.v2';
  migrated.migration = { fromSchema: pool.schema, atUtc: new Date().toISOString(),
    rule: 'Original guest account/cookie fields retained; new wallet identity and character index are separate' };
  for (const actor of migrated.actors) {
    actor.identityHistory = [{ authMethod: 'directGuest', accountId: actor.accountId ?? null,
      cookieRetainedInOriginalActor: Boolean(actor.cookie), characterIndex: actor.characterIndex ?? null }];
  }
  return migrated;
}

export async function exchangeSuiWallet(webBase, actor, { fetchImpl = fetch, remember = () => {}, persist = async () => {} } = {}) {
  let keypair;
  if (actor.walletIdentity) {
    check(actor.walletIdentity.kind === 'synthetic-ed25519-wallet' && typeof actor.walletIdentity.secretKey === 'string', 'Invalid private synthetic wallet');
    remember(actor.walletIdentity.secretKey);
    keypair = Ed25519Keypair.fromSecretKey(actor.walletIdentity.secretKey);
    check(normalizeSuiAddress(keypair.getPublicKey().toSuiAddress()) === actor.walletIdentity.address, 'Private wallet address/key mismatch');
  } else {
    keypair = Ed25519Keypair.generate();
    actor.walletIdentity = { kind: 'synthetic-ed25519-wallet', secretKey: keypair.getSecretKey(),
      address: normalizeSuiAddress(keypair.getPublicKey().toSuiAddress()) };
    remember(actor.walletIdentity.secretKey); await persist();
  }
  const address = actor.walletIdentity.address; remember(address); remember('sui:' + address);
  const post = async (route, body) => {
    const started = performance.now(); let response;
    try {
      response = await fetchImpl(webBase + route, { method: 'POST', redirect: 'error',
        headers: { 'content-type': 'application/json', accept: 'application/json', origin: webBase },
        body: JSON.stringify(body), signal: AbortSignal.timeout(PUBLIC_AUTH_HTTP_TIMEOUT_MS) });
    } catch (error) {
      if (error?.name === 'TimeoutError') throw new PublicAuthFetchTimeout(route, performance.now() - started);
      throw error;
    }
    check(response.ok, 'Public wallet authentication HTTP rejected: ' + response.status + ' at ' + route);
    return response.json();
  };
  const challenge = await post('/api/passkey/challenge', { address, authMethod: 'wallet' });
  check(typeof challenge.challenge === 'string' && typeof challenge.challengeId === 'string' &&
    Number.isFinite(challenge.issuedAt) && Number.isFinite(challenge.expiresAt) && challenge.expiresAt > Date.now(), 'Official wallet challenge missing or expired');
  remember(challenge.challenge);
  const message = JSON.stringify({ purpose: 'mir2-sui-login', accountId: 'sui:' + address, address,
    origin: webBase, issuedAt: challenge.issuedAt, expiresAt: challenge.expiresAt,
    nonce: challenge.challengeId, challenge: challenge.challenge, authMethod: 'wallet' });
  remember(message);
  const { signature } = await keypair.signPersonalMessage(new TextEncoder().encode(message)); remember(signature);
  const identityProof = await post('/api/passkey/login', { address, message, signature });
  check(identityProof.provider === 'suiWallet' && identityProof.accountId === 'sui:' + address && typeof identityProof.token === 'string', 'Official wallet verifier omitted valid Sui identity proof');
  remember(identityProof.token);
  const proof = await post('/api/channels/session/exchange', { provider: 'suiWallet', credential: identityProof.token });
  check(proof.provider === 'suiWallet' && typeof proof.accountId === 'string' && proof.accountId && typeof proof.token === 'string' && proof.token.includes('.'), 'Official wallet channel exchange omitted Gateway token');
  remember(proof.token); remember(proof.accountId);
  check(!actor.walletIdentity.accountId || actor.walletIdentity.accountId === proof.accountId, 'Synthetic wallet mapped to a different account on relogin');
  actor.walletIdentity.accountId = proof.accountId; await persist();
  return { accountId: proof.accountId, token: proof.token, expiresAt: proof.expiresAt };
}
