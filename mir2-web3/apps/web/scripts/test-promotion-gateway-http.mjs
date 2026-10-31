// Local-only integration harness. Gateway and PostgreSQL are real; the Dubhe relay
// session is a declared fixture signed by an in-memory SDK Ed25519 wallet.
import assert from "node:assert/strict";
import { createHash, createHmac, randomBytes, randomUUID } from "node:crypto";
import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, isAbsolute, join, relative, resolve } from "node:path";
import { tmpdir } from "node:os";
import { parseArgs } from "node:util";
import { fileURLToPath, pathToFileURL } from "node:url";
import vm from "node:vm";

const { values: args } = parseArgs({ options: {
  config: { type: "string" }, "evidence-dir": { type: "string" }, checkout: { type: "string" }, help: { type: "boolean" },
}, strict: true });
if (!args.config || args.help) {
  console.log("No integration requests run. Usage: node test-promotion-gateway-http.mjs --config <private-json-outside-repo> [--evidence-dir <outside-repo-directory>] [--checkout <repository-root>]");
  process.exit(0);
}
const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const checkout = resolve(args.checkout || join(scriptDirectory, "../../../.."));
const isInsideCheckout = (path) => { const value = relative(checkout, path); return !value || (!value.startsWith("..") && !isAbsolute(value)); };
const configPath = resolve(args.config);
assert.ok(!isInsideCheckout(configPath), "private test config must stay outside the repository");
let cfg;
try { cfg = JSON.parse(readFileSync(configPath, "utf8")); }
catch { console.error("Invalid private test configuration; no requests run."); process.exit(1); }
const web = join(checkout, "mir2-web3/apps/web");
const require = createRequire(pathToFileURL(join(web, "package.json")));
const ts = require("typescript");
const { Ed25519Keypair } = require("@mysten/sui/keypairs/ed25519");
let gateway, database;
try { gateway = new URL(cfg.gatewayUrl); database = new URL(cfg.databaseUrl); }
catch { console.error("Invalid local gateway or test database configuration; no requests run."); process.exit(1); }
assert.ok(["127.0.0.1", "localhost", "[::1]"].includes(gateway.hostname) && gateway.protocol === "http:", "loopback HTTP gateway only");
assert.ok(["127.0.0.1", "localhost", "[::1]"].includes(database.hostname) && /promotion/.test(database.pathname) && !/prod|playtest/.test(database.pathname), "dedicated promotion test database only");
assert.ok(Number(gateway.port) >= 1024 && Number(database.port) >= 1024 && database.port !== "5432", "explicit isolated gateway and non-default PostgreSQL ports required");
assert.ok(["development", "test"].includes(cfg.environment), "sandbox environment only");
assert.ok(cfg.passkeyAuthSecret?.length >= 32 && cfg.sourceSecret?.length >= 32, "test secrets required");
if (cfg.payoutWorkerSecret !== undefined) {
  assert.ok(typeof cfg.payoutWorkerSecret === "string" && cfg.payoutWorkerSecret.length >= 32, "worker test secret must contain at least 32 characters");
  assert.notEqual(cfg.payoutWorkerSecret, cfg.sourceSecret, "payout worker and source test keys must be independent");
}
const base = gateway.origin;
const fixtureOrigin = "https://promotion-fixture.invalid";
const relayOrigin = "https://relay-fixture.invalid";
const walletOrigin = "https://wallet-fixture.invalid";
Object.assign(process.env, { MIR2_PASSKEY_AUTH_SECRET: cfg.passkeyAuthSecret, MIR2_PASSKEY_ALLOWED_ORIGINS: fixtureOrigin,
  MIR2_DUBHE_CONNECT_RELAY_URL: relayOrigin, MIR2_DUBHE_WALLET_ORIGIN: walletOrigin, MIR2_DUBHE_CONNECT_NETWORK: "testnet",
  MIR2_GATEWAY_HTTP_URL: cfg.gatewayUrl, MIR2_PROMOTION_GATEWAY_ALLOWED_ORIGINS: gateway.origin,
  MIR2_PROMOTION_SESSION_SECRET: randomBytes(32).toString("base64url") });
const pgenv = { ...process.env, PGHOST: database.hostname, PGPORT: database.port, PGDATABASE: decodeURIComponent(database.pathname.slice(1)),
  PGUSER: decodeURIComponent(database.username), PGPASSWORD: decodeURIComponent(database.password), PGCONNECT_TIMEOUT: "5" };
const root = args["evidence-dir"] ? resolve(args["evidence-dir"]) : mkdtempSync(join(tmpdir(), "mir2-promotion-http-"));
assert.ok(!isInsideCheckout(root), "integration evidence must stay outside the repository");
mkdirSync(root, { recursive: true });
const reportPath = join(root, "GATEWAY-HTTP-VERIFICATION.json");
if (existsSync(reportPath)) copyFileSync(reportPath, join(root, `GATEWAY-HTTP-VERIFICATION.previous-${Date.now()}.json`));
const suffix = randomBytes(5).toString("hex");
const campaignId = `http-sandbox-${suffix}`;
process.env.MIR2_PROMOTION_CAMPAIGN_ID = campaignId;
const trace = [];
const completed = [];
const workerChecks = { configuredTestKey: typeof cfg.payoutWorkerSecret === "string", authenticatedExecution: false,
  actorSuppliedByRequest: false, chainPaymentAcceptance: false };
let phase = "initialization";
function sql(text) {
  try { return execFileSync(cfg.psqlPath, ["-X", "--no-psqlrc", "-v", "ON_ERROR_STOP=1", "--tuples-only", "--no-align", "--command", text],
    { env: pgenv, encoding: "utf8", windowsHide: true, stdio: ["ignore", "pipe", "pipe"] }).trim(); }
  catch { throw new Error(`isolated PostgreSQL assertion failed during ${phase}`); }
}
function lit(value) { return `'${String(value).replaceAll("'", "''")}'`; }
async function call(path, { method = "GET", token, body, idempotency = randomUUID(), headers = {} } = {}) {
  const response = await fetch(`${base}${path}`, { method, cache: "no-store", redirect: "error", signal: AbortSignal.timeout(45000),
    headers: { accept: "application/json", ...(token ? { authorization: `Bearer ${token}` } : {}),
      ...(body === undefined ? {} : { "content-type": "application/json", "idempotency-key": idempotency }), ...headers },
    ...(body === undefined ? {} : { body: typeof body === "string" ? body : JSON.stringify(body) }) });
  const data = await response.json().catch(() => null);
  trace.push({ method, path: path.split("?")[0], status: response.status, ...(typeof data?.error === "string" ? { error: data.error } : {}) });
  return { status: response.status, data };
}
function success(result) { assert.equal(result.status, 200, `${phase}: ${result.status} ${result.data?.error || ""}`); return result.data; }
function rejected(result, expected) { assert.ok(expected ? expected.includes(result.status) : result.status >= 400, `${phase}: unexpectedly accepted ${result.status}`); }
function step(name) { phase = name; }
function done() { completed.push(phase); }
let relayValue;
const modules = new Map();
function load(name) {
  if (modules.has(name)) return modules.get(name);
  const file = join(web, "lib/server", `${name}.ts`);
  const compiled = ts.transpileModule(readFileSync(file, "utf8"), { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS } }).outputText;
  const module = { exports: {} };
  vm.runInNewContext(compiled, { module, exports: module.exports, Buffer, TextEncoder, Response, Request, Headers, URL, AbortSignal, process,
    fetch: async (url, init) => {
      if (url === `${relayOrigin}/sessions` || url.startsWith(`${relayOrigin}/sessions/`)) return Response.json(relayValue);
      const target = new URL(url); assert.equal(target.origin, base); assert.ok(target.pathname.startsWith("/v1/"));
      const result = await fetch(url, init);
      trace.push({ method: init?.method ?? "GET", path: target.pathname, status: result.status, transport: "actual-bff-function-to-gateway-http" });
      return result;
    },
    require: (specifier) => specifier === "server-only" ? {} : specifier.startsWith("./") ? load(specifier.slice(2)) : require(specifier),
  }, { filename: file });
  modules.set(name, module.exports); return module.exports;
}
const dubhe = load("dubhe-connect-auth");
function pendingSession() {
  const now = Date.now(); const id = `fixture-${randomUUID()}`;
  return { id, relayOrigin, approvalUrl: `${walletOrigin}/connect/approve?session=${id}&relay=${encodeURIComponent(relayOrigin)}`,
    pollUrl: `${relayOrigin}/sessions/${id}`, status: "pending", request: { appName: "Mir2 Web3", origin: fixtureOrigin,
      network: "testnet", walletOrigin, statement: "Sign in to Mir2 Web3 with Dubhe Wallet.", nonce: randomBytes(24).toString("base64url"),
      requestedAt: new Date(now).toISOString(), expiresAt: new Date(now + 120000).toISOString() } };
}
async function approvePending(pending, keypair) {
  const message = dubhe.buildDubheConnectLoginMessage(pending);
  const signed = await keypair.signPersonalMessage(new TextEncoder().encode(message));
  relayValue = { ...pending, status: "approved", result: { account: { address: keypair.toSuiAddress(), publicKey: keypair.getPublicKey().toBase64(), chains: ["sui:testnet"] },
    message, bytes: signed.bytes, signature: signed.signature, approvedAt: new Date().toISOString() } };
}
async function walletProof(keypair = new Ed25519Keypair()) {
  const pending = pendingSession(); relayValue = pending;
  const create = await dubhe.createDubheConnectLogin(new Request(`${fixtureOrigin}/api/wallet/dubhe/session`, { method: "POST", headers: { origin: fixtureOrigin } }));
  assert.equal(create.status, 200); const intent = await create.json();
  await approvePending(pending, keypair);
  const verified = await dubhe.readDubheConnectLogin(new Request(`${fixtureOrigin}/api/wallet/dubhe/session/status`, { method: "POST", headers: { origin: fixtureOrigin, authorization: `Bearer ${intent.intentToken}` } }));
  assert.equal(verified.status, 200); const result = await verified.json(); assert.equal(result.status, "approved");
  return { keypair, credential: result.login.token, address: keypair.toSuiAddress() };
}
async function exchange(proof, priorToken) { return call("/v1/promotions/auth/exchange", { method: "POST", token: priorToken, body: { provider: "suiWallet", credential: proof.credential } }); }
async function admin(token, command, idempotency) { return call("/v1/promotions/admin", { method: "POST", token, body: { campaignId, command }, idempotency }); }
function sourceHeaders(body, timestamp = String(Date.now())) {
  return scopedHeaders(body, cfg.sourceSecret, "/v1/promotions/source-events", timestamp);
}
function scopedHeaders(body, key, path, timestamp = String(Date.now())) {
  return { "x-promotion-source-timestamp": timestamp, "x-promotion-source-signature": createHmac("sha256", key)
    .update(`mir2-promotion-source-v1\nPOST\n${path}\n${timestamp}\n`).update(body).digest("hex") };
}
async function assertCurrentPayee(grant, address) {
  assert.ok(grant.session.credentialId, "wallet login must issue a current credential");
  const settings = success(await call("/v1/promotions/payout-address", { token: grant.token }));
  assert.equal(settings.status, "verified"); assert.equal(settings.address, address); assert.equal(settings.network, "sui:testnet");
  assert.equal(sql(`SELECT a.verified_credential_id=s.credential_id AND a.wallet_address=c.credential_subject FROM promotion_payout_addresses a JOIN identity_sessions s ON s.session_id=${lit(grant.session.sessionId)} JOIN identity_credentials c ON c.credential_id=s.credential_id WHERE a.environment=${lit(cfg.environment)} AND a.account_id=${lit(grant.session.accountId)} ORDER BY a.version DESC LIMIT 1`), "t", "payee must match the exact current login credential in PostgreSQL");
  return settings;
}
try {
  step("unauthenticated boundaries");
  assert.equal(sql("SELECT current_database()"), decodeURIComponent(database.pathname.slice(1)));
  for (const table of ["accounts", "mir2_channel_identity_bindings", "identity_sessions", "promotion_campaigns"]) {
    assert.equal(sql(`SELECT count(*) FROM ${table}`), "0", "a fresh empty test database is required; existing data will never be reset by this script");
  }
  rejected(await call(`/v1/promotions/me?campaignId=${campaignId}`), [401]);
  rejected(await call("/v1/promotions/auth/exchange", { method: "POST", body: { provider: "suiWallet", credential: "invalid" } }), [401]);
  done();

  step("real SDK wallet proof and canonical persisted identity mapping");
  const owner = await walletProof(); const initial = success(await exchange(owner));
  assert.match(initial.session.accountId, /^obl_/); assert.equal(initial.session.authMethod, "sui_wallet");
  assert.equal(sql(`SELECT credential_subject FROM identity_credentials WHERE credential_id=${lit(initial.session.credentialId)}`), owner.address);
  const own = success(await call(`/v1/promotions/me?campaignId=${campaignId}`, { token: initial.token }));
  assert.equal(own.account.id, initial.session.accountId); assert.deepEqual(own.characters, []);
  assert.equal(sql(`SELECT count(*) FROM character_saves WHERE account_id=${lit(initial.session.accountId)}`), "0");
  await assertCurrentPayee(initial, owner.address);
  done();

  step("one-use proof, strict DTO and server-derived role boundaries");
  rejected(await exchange(owner), [401]);
  const signMalformedProof = (payload) => { const encoded = Buffer.from(JSON.stringify(payload)).toString("base64url");
    return `${encoded}.${createHmac("sha256", cfg.passkeyAuthSecret).update(encoded).digest("base64url")}`; };
  for (const payload of [
    { auth: "sui-passkey-v1", accountId: `sui:${owner.address}`, provider: "suiWallet", expMs: Date.now() + 120000 },
    { auth: "sui-passkey-v2", accountId: `sui:${owner.address}`, authMethod: "wallet", jti: "", expMs: Date.now() + 120000 },
  ]) rejected(await call("/v1/promotions/auth/exchange", { method: "POST", body: { provider: "suiWallet", credential: signMalformedProof(payload) } }), [401]);
  rejected(await call("/v1/promotions/auth/exchange", { method: "POST", body: { provider: "suiWallet", credential: (await walletProof()).credential, accountId: "victim" } }), [400, 422]);
  rejected(await call(`/v1/promotions/me?campaignId=${campaignId}&accountId=victim`, { token: initial.token }), [400, 422]);
  rejected(await call(`/v1/promotions/admin?campaignId=${campaignId}`, { token: initial.token, headers: { "x-operator-role": "finance", "x-admin-account": "admin" } }), [403]);
  done();

  step("existing password fixture, real password verification and migration");
  const oldAccount = `http-old-${suffix}`; const password = `Fixture-${randomBytes(18).toString("base64url")}`;
  const raw = JSON.parse(sql(`SELECT raw_json FROM accounts WHERE account_id=${lit(initial.session.accountId)}`));
  raw.password = password; raw.characters = [{ index: 0, name: `Fixture${suffix}`, level: 30, class: "Warrior", gender: "Male" }]; raw.saves = {};
  sql(`INSERT INTO accounts(account_id,password_snapshot,raw_json,created_at) VALUES(${lit(oldAccount)},${lit(password)},${lit(JSON.stringify(raw))}::jsonb,now()-interval '1 day');
    INSERT INTO characters(account_id,character_index,character_name,class,gender,level,raw_json) VALUES(${lit(oldAccount)},0,${lit(raw.characters[0].name)},'Warrior','Male',30,${lit(JSON.stringify(raw.characters[0]))}::jsonb)`);
  rejected(await call("/v1/promotions/auth/password", { method: "POST", body: { accountId: oldAccount, password: "wrong-fixture-password" } }), [401]);
  const old = success(await call("/v1/promotions/auth/password", { method: "POST", body: { accountId: oldAccount, password } }));
  assert.equal(old.session.accountId, oldAccount); assert.equal(old.session.authMethod, "password");
  assert.equal(sql(`SELECT password_snapshot LIKE '$argon2id$%' FROM accounts WHERE account_id=${lit(oldAccount)}`), "t");
  const oldMe = success(await call(`/v1/promotions/me?campaignId=${campaignId}`, { token: old.token })); assert.equal(oldMe.characters[0].level, 30);
  const unboundApplication = await call("/v1/promotions/apply", { method: "POST", token: old.token,
    body: { campaignId, characterIndex: 0, termsVersion: `test-terms-${suffix}` } });
  rejected(unboundApplication, [409]); assert.equal(unboundApplication.data.error, "promotionWalletRequired");
  done();

  step("dual-control binding, stable old account and conflict rejection");
  const second = await walletProof(); const bound = success(await exchange(second, old.token)); assert.equal(bound.session.accountId, oldAccount);
  const originalPayee = await assertCurrentPayee(bound, second.address);
  const rebound = success(await exchange(await walletProof(second.keypair))); assert.equal(rebound.session.accountId, oldAccount);
  assert.equal((await assertCurrentPayee(rebound, second.address)).version, originalPayee.version, "same-wallet login must not create another destination version");
  assert.equal(sql(`SELECT password_snapshot LIKE '$argon2id$%' FROM accounts WHERE account_id=${lit(oldAccount)}`), "t", "wallet binding must retain the original standard account password");
  const passwordStillWorks = success(await call("/v1/promotions/auth/password", { method: "POST", body: { accountId: oldAccount, password } }));
  assert.equal(passwordStillWorks.session.accountId, oldAccount);
  rejected(await exchange(await walletProof(owner.keypair), old.token), [409]);
  assert.equal(sql(`SELECT account_id FROM identity_credentials WHERE credential_kind='sui_wallet' AND credential_subject=${lit(owner.address)} AND revoked_at_ms IS NULL`), initial.session.accountId);
  done();

  step("automatic payee follows exact current wallet and cannot select another linked credential");
  const anotherProof = await walletProof(); const another = success(await exchange(anotherProof, passwordStillWorks.token));
  assert.equal(another.session.accountId, oldAccount); assert.notEqual(another.session.credentialId, rebound.session.credentialId);
  await assertCurrentPayee(another, anotherProof.address);
  assert.equal(sql(`SELECT count(*) FROM identity_credentials WHERE account_id=${lit(oldAccount)} AND credential_kind='sui_wallet' AND revoked_at_ms IS NULL`), "2");
  for (const [token, credentialId] of [
    [rebound.token, another.session.credentialId], [another.token, rebound.session.credentialId],
    [passwordStillWorks.token, rebound.session.credentialId],
  ]) {
    const denied = await call("/v1/promotions/payout-address", { method: "POST", token, body: { credentialId, network: "sui:testnet" } });
    rejected(denied, [409]); assert.equal(denied.data.error, "promotionWalletProofRequired");
  }
  assert.equal(sql(`SELECT wallet_address FROM promotion_payout_addresses WHERE environment=${lit(cfg.environment)} AND account_id=${lit(oldAccount)} ORDER BY version DESC LIMIT 1`), anotherProof.address, "denied credential selection must not mutate payee");
  success(await call("/v1/promotions/payout-address", { method: "POST", token: rebound.token, body: { credentialId: rebound.session.credentialId, network: "sui:testnet" } }));
  await assertCurrentPayee(rebound, second.address);
  done();

  step("revocation, old password proof revocation and authoritative bans");
  success(await call("/v1/identity/sessions/revoke", { method: "POST", token: bound.token, body: { sessionId: bound.session.sessionId, reason: "sandbox_test" } }));
  rejected(await call(`/v1/promotions/me?campaignId=${campaignId}`, { token: bound.token }), [401]);
  success(await call("/v1/identity/sessions/revoke", { method: "POST", token: old.token, body: { sessionId: old.session.sessionId, reason: "sandbox_test" } }));
  const third = await walletProof(); rejected(await exchange(third, old.token), [401]);
  assert.equal(sql(`SELECT count(*) FROM identity_credentials WHERE credential_subject=${lit(third.address)}`), "0");
  sql(`UPDATE accounts SET is_banned=TRUE,ban_reason='isolated_sandbox_test',raw_json=jsonb_set(jsonb_set(raw_json,'{is_banned}','true'::jsonb),'{ban_reason}','"isolated_sandbox_test"'::jsonb),store_version=store_version+1 WHERE account_id=${lit(oldAccount)}`);
  rejected(await call(`/v1/promotions/me?campaignId=${campaignId}`, { token: rebound.token }), [409]);
  rejected(await exchange(await walletProof(second.keypair)), [401]);
  sql(`UPDATE accounts SET is_banned=FALSE,ban_reason='',raw_json=jsonb_set(jsonb_set(raw_json,'{is_banned}','false'::jsonb),'{ban_reason}','""'::jsonb),store_version=store_version+1 WHERE account_id=${lit(oldAccount)}`);
  done();

  step("real PostgreSQL operator roles and separated draft approval");
  sql(`INSERT INTO promotion_operator_roles(environment,account_id,role,granted_by,granted_at_ms) VALUES
    (${lit(cfg.environment)},${lit(oldAccount)},'operations','isolated_http_fixture',${Date.now()}),
    (${lit(cfg.environment)},${lit(initial.session.accountId)},'finance','isolated_http_fixture',${Date.now()}),
    (${lit(cfg.environment)},${lit(initial.session.accountId)},'risk','isolated_http_fixture',${Date.now()})`);
  const activeOld = success(await exchange(await walletProof(second.keypair)));
  const evidenceRef = `isolated-http-sandbox:${suffix}`;
  const draft = { campaignId, termsVersion: `test-terms-${suffix}`, startsAtMs: Date.now() - 60000, endsAtMs: Date.now() + 86400000,
    fundingMode: "revenue_share",
    rules: { minimumLevel: 30, activeUtcDays: 3, candidateTtlMs: 7 * 86400000, confirmationWindowMs: 86400000, maximumCoverageMs: 92 * 86400000, reserveBps: 0, walletCooldownMs: 72 * 3600000 },
    budgetMicroUsd: 0, memberLimit: 0, leaderLimit: 0, perLeaderMemberLimit: 0, perMemberRevenueCapMicroUsd: 0 };
  const drafted = success(await admin(activeOld.token, { action: "configureDraft", draft, evidenceRef }));
  assert.equal(drafted.recruitmentEnabled, false); assert.equal(drafted.payoutEnabled, false);
  rejected(await admin(activeOld.token, { action: "approveRules", evidenceRef }), [403]);
  success(await admin(initial.token, { action: "approveRules", evidenceRef }));
  const publicCampaign = success(await call(`/v1/promotions/public?campaignId=${campaignId}`));
  assert.equal(publicCampaign.fundingMode, "revenue_share"); assert.equal(publicCampaign.payoutEnabled, false);
  assert.equal(publicCampaign.payoutPolicy, null);
  assert.ok(Array.isArray(publicCampaign.sourceGaps));
  assert.ok(publicCampaign.sourceGaps.includes("production_acceptance_and_launch_approval_required"));
  for (const gap of publicCampaign.sourceGaps) assert.ok(["production_acceptance_and_launch_approval_required", "payment_source_reconciliation_required", "committed_game_events_pending"].includes(gap), "readiness must describe current evidence gaps, not unimplemented source adapters");
  success(await admin(activeOld.token, { action: "setRecruitment", enabled: true, evidenceRef }));
  rejected(await admin(initial.token, { action: "submitPayout", payoutId: "fake-nonexistent" }), [403]);
  done();

  step("owned character application, approved leader and immutable invitation confirmation");
  const applied = success(await call("/v1/promotions/apply", { method: "POST", token: activeOld.token, body: { campaignId, characterIndex: 0, termsVersion: draft.termsVersion } }));
  rejected(await call("/v1/promotions/apply", { method: "POST", token: initial.token, body: { campaignId, characterIndex: 0, termsVersion: draft.termsVersion } }));
  success(await admin(initial.token, { action: "reviewLeader", leaderId: applied.leaderId, decision: "approved", reason: "isolated fixture", evidenceRef }));
  const leader = success(await call(`/v1/promotions/me?campaignId=${campaignId}`, { token: activeOld.token })).leader;
  const clicked = success(await call("/v1/promotions/click", { method: "POST", body: { campaignId, code: leader.inviteCode } }));
  const memberProof = await walletProof(); const member = success(await exchange(memberProof));
  const candidate = success(await call("/v1/promotions/candidate", { method: "POST", token: member.token, body: { campaignId, code: leader.inviteCode, source: "invite_link", clickId: clicked.clickId } }));
  assert.equal(candidate.leaderDisplayName, raw.characters[0].name);
  const confirmKey = randomUUID();
  const confirmed = success(await call("/v1/promotions/confirm", { method: "POST", token: member.token, idempotency: confirmKey, body: { campaignId, candidateId: candidate.candidateId, termsVersion: draft.termsVersion } }));
  assert.equal(confirmed.status, "bound");
  success(await call("/v1/promotions/confirm", { method: "POST", token: member.token, idempotency: confirmKey, body: { campaignId, candidateId: candidate.candidateId, termsVersion: draft.termsVersion } }));
  assert.equal(sql(`SELECT count(*) FROM promotion_attributions WHERE campaign_id=${lit(campaignId)} AND account_id=${lit(member.session.accountId)}`), "1");
  rejected(await call("/v1/promotions/candidate", { method: "POST", token: initial.token, body: { campaignId, code: leader.inviteCode, source: "invite_link", clickId: clicked.clickId } }));
  rejected(await call("/v1/promotions/confirm", { method: "POST", token: initial.token, body: { campaignId, candidateId: candidate.candidateId, termsVersion: draft.termsVersion } }));
  const memberRows = success(await call(`/v1/promotions/members?campaignId=${campaignId}&offset=0&limit=20`, { token: activeOld.token }));
  assert.equal(memberRows.items.length, 1); assert.ok(!JSON.stringify(memberRows).includes(member.session.accountId));
  done();

  step("source HMAC binds raw bytes and rejects malformed Unicode without server failure");
  const invalid = JSON.stringify({ type: "gameFact" });
  rejected(await call("/v1/promotions/source-events", { method: "POST", body: invalid, headers: sourceHeaders(invalid + " ") }), [403]);
  rejected(await call("/v1/promotions/source-events", { method: "POST", body: invalid, headers: sourceHeaders(invalid) }), [400]);
  rejected(await call("/v1/promotions/source-events", { method: "POST", body: invalid,
    headers: { "x-promotion-source-timestamp": String(Date.now()), "x-promotion-source-signature": "é".repeat(32) } }), [403]);
  const fact = JSON.stringify({ type: "gameFact", fact: { source: "level_commit", kind: "levelCommitted", sourceEventId: `http-level-${suffix}`,
    accountId: oldAccount, characterIndex: 0, occurredAtMs: Date.now() - 10, level: 30, evidenceRef } });
  success(await call("/v1/promotions/source-events", { method: "POST", body: fact, headers: sourceHeaders(fact) }));
  success(await call("/v1/promotions/source-events", { method: "POST", body: fact, headers: sourceHeaders(fact) }));
  done();

  step("payout worker key and path isolation without a client-controlled server actor");
  const workerPath = "/v1/promotions/payout-worker";
  const workerBody = JSON.stringify({ action: "markSubmitted", payoutId: `nonexistent-${suffix}`, digest: "11111111111111111111111111111111" });
  rejected(await call(workerPath, { method: "POST", body: workerBody }), [403]);
  rejected(await call(workerPath, { method: "POST", body: workerBody, headers: sourceHeaders(workerBody) }), [403]);
  rejected(await call(workerPath, { method: "POST", body: workerBody, headers: scopedHeaders(workerBody, cfg.sourceSecret, workerPath) }), [403]);
  if (cfg.payoutWorkerSecret) {
    const signed = value => scopedHeaders(value, cfg.payoutWorkerSecret, workerPath);
    rejected(await call(workerPath, { method: "POST", body: workerBody, headers: scopedHeaders(workerBody, cfg.payoutWorkerSecret, "/v1/promotions/source-events") }), [403]);
    rejected(await call("/v1/promotions/source-events", { method: "POST", body: fact, headers: scopedHeaders(fact, cfg.payoutWorkerSecret, "/v1/promotions/source-events") }), [403]);
    rejected(await call(workerPath, { method: "POST", body: workerBody, headers: signed(workerBody + " ") }), [403]);
    for (const extra of [{ actor: oldAccount }, { role: "payout_submit" }, { actorAccount: oldAccount, roles: ["finance", "payout_submit"] }]) {
      const injected = JSON.stringify({ ...JSON.parse(workerBody), ...extra });
      const denied = await call(workerPath, { method: "POST", body: injected, headers: signed(injected) });
      rejected(denied, [400]); assert.equal(denied.data.error, "promotionSourceInvalid");
    }
    const unconfigured = await call(workerPath, { method: "POST", body: workerBody, headers: signed(workerBody) });
    rejected(unconfigured, [503]); assert.equal(unconfigured.data.error, "promotionConfigurationInvalid", "a signed request cannot invent a missing server actor");
    workerChecks.authenticatedConfigurationGateChecked = true;
    workerChecks.note = "Valid HMAC reached the missing server-actor gate; no journal execution or payment was accepted.";
  } else {
    workerChecks.authenticatedConfigurationGateChecked = false;
    workerChecks.note = "No independent worker test key supplied; positive HMAC/strict worker DTO checks were not run.";
  }
  assert.equal(sql(`SELECT count(*) FROM promotion_payout_attempts WHERE environment=${lit(cfg.environment)}`), "0", "worker boundary tests must not create payout attempts");
  done();

  step("zero-revenue synthetic season lock and finance-only canonical snapshot export");
  success(await admin(initial.token, { action: "setCommission", enabled: true, evidenceRef }));
  // Advance only this synthetic season's deadline in the isolated database. This
  // is a fixture clock transition, not a production edit or simulated API reply.
  const endedAt = Date.now() - 1000;
  sql(`UPDATE promotion_campaigns SET recruitment_enabled=FALSE,status='closed',ends_at_ms=${endedAt} WHERE environment=${lit(cfg.environment)} AND campaign_id=${lit(campaignId)}`);
  success(await admin(initial.token, { action: "setInputWatermark", watermarkMs: endedAt, evidenceRef }));
  const settlement = success(await admin(initial.token, { action: "createSettlement", leaderId: leader.id, evidenceRef }));
  assert.equal(settlement.totalMicroUsd, "0"); assert.equal(settlement.payoutEnabled, false);
  rejected(await call(`/v1/promotions/export?campaignId=${campaignId}&settlementId=${encodeURIComponent(settlement.settlementId)}`, { token: activeOld.token }), [403]);
  const exported = success(await call(`/v1/promotions/export?campaignId=${campaignId}&settlementId=${encodeURIComponent(settlement.settlementId)}`, { token: initial.token }));
  assert.equal(typeof exported.snapshotCanonicalJson, "string");
  assert.equal(createHash("sha256").update(exported.snapshotCanonicalJson, "utf8").digest("hex"), exported.snapshotSha256);
  assert.equal(exported.snapshotSha256, settlement.snapshotSha256);
  rejected(await admin(initial.token, { action: "approveSettlement", settlementId: settlement.settlementId, snapshotSha256: settlement.snapshotSha256, evidenceRef }), [409]);
  done();

  step("actual BFF functions to real gateway HTTP and PostgreSQL without exposing browser bearers");
  const bffAuth = load("promotion-auth"); const bffApi = load("promotion-api");
  const bffRequest = (path, { method = "POST", cookie, csrf, body, idempotency } = {}) => new Request(`${fixtureOrigin}/api/promotions/${path}`, {
    method, headers: { origin: fixtureOrigin, ...(cookie ? { cookie } : {}), ...(csrf ? { "x-promotion-csrf": csrf } : {}),
      ...(body === undefined ? {} : { "content-type": "application/json" }), ...(idempotency ? { "idempotency-key": idempotency } : {}) },
    ...(body === undefined ? {} : { body: JSON.stringify(body) }),
  });
  const bffCookies = (response) => response.headers.getSetCookie().filter((item) => !item.includes("Max-Age=0")).map((item) => item.split(";")[0]).join("; ");
  const passwordResponse = await bffAuth.promotionPassword(bffRequest("auth/password", { body: { accountId: oldAccount, password } }));
  assert.equal(passwordResponse.status, 200); const passwordCookie = bffCookies(passwordResponse);
  assert.deepEqual(await passwordResponse.json(), { status: "approved" }); assert.ok(!passwordCookie.includes(password));
  let bffBootstrap = await bffApi.promotionBootstrap(bffRequest("bootstrap", { method: "GET", cookie: passwordCookie }));
  assert.equal(bffBootstrap.status, 200); let bootstrapData = await bffBootstrap.json();
  assert.equal(bootstrapData.me.account.id, oldAccount); assert.equal(bootstrapData.characters[0].level, 30);
  assert.equal(bootstrapData.authenticated, true); assert.ok(!JSON.stringify(bootstrapData).includes('"token"'));
  const bffPending = pendingSession(); relayValue = bffPending;
  const qrResponse = await bffAuth.promotionChallenge(bffRequest("auth/challenge", { cookie: passwordCookie }));
  assert.equal(qrResponse.status, 200); const challengeCookie = bffCookies(qrResponse); const qrData = await qrResponse.json();
  assert.equal(qrData.intentToken, undefined); assert.equal(qrData.approvalUrl, bffPending.approvalUrl);
  const bffWallet = new Ed25519Keypair();
  await approvePending(bffPending, bffWallet);
  const pollResponse = await bffAuth.promotionPoll(bffRequest("auth/poll", { cookie: `${passwordCookie}; ${challengeCookie}` }));
  assert.equal(pollResponse.status, 200); assert.deepEqual(await pollResponse.json(), { status: "approved" });
  const walletCookie = bffCookies(pollResponse);
  bffBootstrap = await bffApi.promotionBootstrap(bffRequest("bootstrap", { method: "GET", cookie: walletCookie }));
  assert.equal(bffBootstrap.status, 200); bootstrapData = await bffBootstrap.json();
  assert.equal(bootstrapData.me.account.id, oldAccount); assert.equal(bootstrapData.binding.status, "linked");
  assert.ok(bootstrapData.binding.credentialId); assert.ok(!JSON.stringify(bootstrapData).includes('"token"'));
  assert.equal(bootstrapData.me.payoutSettings.address, bffWallet.toSuiAddress());
  assert.equal(sql(`SELECT a.verified_credential_id=${lit(bootstrapData.binding.credentialId)} AND a.wallet_address=${lit(bffWallet.toSuiAddress())} FROM promotion_payout_addresses a WHERE a.environment=${lit(cfg.environment)} AND a.account_id=${lit(oldAccount)} ORDER BY a.version DESC LIMIT 1`), "t");
  const bffAppeal = await bffApi.promotionProxy(bffRequest("appeals", { cookie: walletCookie, csrf: bootstrapData.csrfToken,
    idempotency: randomUUID(), body: { category: "other", message: "Synthetic local BFF integration support ticket" } }), ["appeals"]);
  assert.equal(bffAppeal.status, 200); assert.equal((await bffAppeal.json()).status, "open");
  const logout = await bffAuth.promotionLogout(bffRequest("auth/logout", { cookie: walletCookie, csrf: bootstrapData.csrfToken }));
  assert.equal(logout.status, 200); assert.ok(logout.headers.getSetCookie().every((item) => item.includes("Max-Age=0")));
  assert.equal((await bffApi.promotionBootstrap(bffRequest("bootstrap", { method: "GET", cookie: walletCookie }))).status, 401);
  assert.equal(success(await call("/v1/promotions/auth/password", { method: "POST", body: { accountId: oldAccount, password } })).session.accountId, oldAccount);
  done();

  step("no game start or real payout and closed sandbox recruitment on completion");
  success(await admin(activeOld.token, { action: "setRecruitment", enabled: false, evidenceRef }));
  assert.equal(sql(`SELECT count(*) FROM character_saves WHERE account_id IN (${lit(oldAccount)},${lit(initial.session.accountId)},${lit(member.session.accountId)})`), "0");
  assert.equal(sql(`SELECT count(*) FROM promotion_payouts p JOIN promotion_settlements s USING(environment,settlement_id) WHERE p.environment=${lit(cfg.environment)} AND s.campaign_id=${lit(campaignId)}`), "0");
  done();
  writeFileSync(reportPath, JSON.stringify({ passed: true, completed, trace, campaignId,
    transport: "real loopback gateway HTTP and native PostgreSQL; only Dubhe relay is a declared SDK-signed fixture",
    actualBffFunctionSeam: true, browserUiAcceptance: false, mobileWalletTested: false, payoutWorker: workerChecks,
    realPayment: false, gameStarted: false, dockerStarted: false, productionDataUsed: false }, null, 2));
  console.log(JSON.stringify({ passed: true, completed: completed.length, httpRequests: trace.length, report: reportPath }));
} catch (error) {
  writeFileSync(reportPath, JSON.stringify({ passed: false, failedPhase: phase, completed, trace,
    error: error instanceof assert.AssertionError ? String(error.message).slice(0, 500) : "local integration request failed; inspect server log without exposing credentials",
    realPayment: false, gameStarted: false, dockerStarted: false, productionDataUsed: false, payoutWorker: workerChecks,
    transport: "real loopback gateway HTTP and native PostgreSQL; only Dubhe relay is a declared SDK-signed fixture",
    browserUiAcceptance: false, mobileWalletTested: false }, null, 2));
  console.error(JSON.stringify({ passed: false, failedPhase: phase, completed: completed.length, report: reportPath }));
  process.exitCode = 1;
}
