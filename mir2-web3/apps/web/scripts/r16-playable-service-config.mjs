import { createHash, createHmac, randomBytes, randomUUID } from "node:crypto";
import { isAbsolute, resolve } from "node:path";
import { R16_GAMEPLAY_BASELINE } from "../../ai-player/src/generated-gameplay-baseline.mjs";

export const SERVICE_SCHEMA = "mir2-r16-playable-web/1";
export const SERVICE_PORTS = Object.freeze({ gateway: 19040, web: 19041, actor: 19042 });
const failure = () => new Error("Private playable state is incomplete or invalid; it was not reset or replaced.");
const exact = (value, keys) => value && typeof value === "object" && !Array.isArray(value)
  && Object.keys(value).length === keys.length && keys.every(key => Object.hasOwn(value, key));
const secret = value => typeof value === "string" && /^[a-f0-9]{64}$/.test(value) && !/^0+$/.test(value);
const reference = value => typeof value === "string" && /^[A-Za-z0-9._:-]{1,100}$/.test(value);
const credential = value => typeof value === "string" && /^[A-Za-z0-9]{8,15}$/.test(value);

export function parsePlayableArguments(args) {
  const result = { withAlo: false, command: "run" };
  const paths = { "--gateway-exe": "gatewayExe", "--server-entry": "serverEntry", "--data-dir": "dataDir",
    "--web-root": "webRoot", "--node-exe": "nodeExe", "--model-settings-bundle": "modelSettingsBundle" };
  const seen = new Set();
  for (let i = 0; i < args.length; i++) {
    const arg = args[i];
    if (seen.has(arg)) throw new Error("Duplicate playable service option."); seen.add(arg);
    if (arg === "--with-alo") { result.withAlo = true; continue; }
    const value = args[++i];
    if (arg === "--command" && ["status", "stop", "restartActor"].includes(value)) { result.command = value; continue; }
    if (!paths[arg] || typeof value !== "string" || !isAbsolute(value)) throw new Error("Supply explicit absolute playable service paths.");
    result[paths[arg]] = resolve(value);
  }
  if (!result.dataDir || (result.command === "run" && ["gatewayExe", "serverEntry", "webRoot", "nodeExe"].some(key => !result[key]))) throw new Error("Missing required playable service path.");
  if (result.command !== "run" && (result.withAlo || Object.keys(result).some(key => !["withAlo", "command", "dataDir"].includes(key)))) throw new Error("Control commands only accept --data-dir.");
  return result;
}

export function newPlayableState() {
  const identity = prefix => ({ accountId: "rw" + randomBytes(6).toString("hex"), password: "Aa" + randomBytes(5).toString("hex") + "7",
    characterName: prefix + randomBytes(4).toString("hex"), characterIndex: null });
  return { schema: SERVICE_SCHEMA, phase: "initializing", createdAt: new Date().toISOString(), actorStoreInitialized: false,
    world: { realmId: "ai-web-" + randomUUID(), worldEpoch: randomUUID(), archiveId: randomUUID(), residentId: "alo-web-" + randomBytes(4).toString("hex") },
    owner: identity("Mir"), actor: identity("Alo"),
    keys: Object.fromEntries(["archiveKey", "saveRecoveryMacKey", "identitySessionSecret", "identityRecoveryPepper", "controlKey"].map(key => [key, randomBytes(32).toString("hex")])) };
}

export function validatePlayableState(state, { allowInitializing = false } = {}) {
  if (!exact(state, ["schema", "phase", "createdAt", "actorStoreInitialized", "world", "owner", "actor", "keys"])
    || state.schema !== SERVICE_SCHEMA || !["initializing", "ready"].includes(state.phase)
    || (!allowInitializing && state.phase !== "ready") || typeof state.actorStoreInitialized !== "boolean"
    || typeof state.createdAt !== "string" || !Number.isFinite(Date.parse(state.createdAt))
    || !exact(state.world, ["realmId", "worldEpoch", "archiveId", "residentId"]) || !Object.values(state.world).every(reference)
    || !state.world.realmId.startsWith("ai-web-")
    || !exact(state.keys, ["archiveKey", "saveRecoveryMacKey", "identitySessionSecret", "identityRecoveryPepper", "controlKey"])
    || !Object.values(state.keys).every(secret) || new Set(Object.values(state.keys)).size !== 5) throw failure();
  for (const identity of [state.owner, state.actor]) {
    if (!exact(identity, ["accountId", "password", "characterName", "characterIndex"])
      || !credential(identity.accountId) || !credential(identity.password) || !credential(identity.characterName)
      || !(Number.isSafeInteger(identity.characterIndex) && identity.characterIndex >= 0 || state.phase === "initializing" && identity.characterIndex === null)) throw failure();
  }
  if (state.owner.accountId === state.actor.accountId || state.owner.characterName === state.actor.characterName) throw failure();
  return state;
}

export function playablePaths(dataDir) {
  return { state: resolve(dataDir, "state.local.json"), accounts: resolve(dataDir, "accounts.json"), channelIdentities: resolve(dataDir, "channel-identities.json"), recovery: resolve(dataDir, "save-recovery"),
    supply: resolve(dataDir, "supply"), policy: resolve(dataDir, "companion-policy.local.json"), memory: resolve(dataDir, "memory"),
    actorMemory: resolve(dataDir, "memory/alo.sqlite"), experience: resolve(dataDir, "memory/experience.sqlite"), logs: resolve(dataDir, "logs") };
}

export function playablePolicy(state) {
  validatePlayableState(state);
  return { schemaVersion: 1, realmId: state.world.realmId, worldEpoch: state.world.worldEpoch,
    enrollments: [{ residentId: state.world.residentId, archiveId: state.world.archiveId, displayName: "阿洛",
      owner: { accountId: state.owner.accountId, characterIndex: state.owner.characterIndex },
      actor: { accountId: state.actor.accountId, characterIndex: state.actor.characterIndex } }] };
}

export function playableActorProfile(state, dataDir) {
  validatePlayableState(state); const paths = playablePaths(dataDir);
  return { url: "ws://127.0.0.1:19040/ws", realmId: state.world.realmId, characterName: state.actor.characterName, guardPort: SERVICE_PORTS.actor,
    timeoutMs: 25_000, allowedMaps: ["0", "0141"],
    companion: { residentId: state.world.residentId, archiveId: state.world.archiveId, worldEpoch: state.world.worldEpoch,
      ownerRef: "owner:" + createHash("sha256").update(state.owner.accountId).digest("hex"),
      characterIndex: state.actor.characterIndex, displayName: "阿洛", baseSetting: "比奇的合成人冒险者", leaderName: state.owner.characterName,
      homePosition: { x: 288, y: 616, mapFileName: "0" }, memoryPath: paths.actorMemory },
    runtimeService: { controlEnabled: true, runtimeId: "r16-web-local", provider: "rules", model: "rules", promptVersion: "rules-v1",
      credentialRef: null, archiveKeyEnv: "MIR2_COMPANION_ARCHIVE_KEY", experiment: { experimentId: "r16-web-" + state.world.archiveId,
        participantRef: "local-owner", ledgerPath: paths.experience, currency: "CNY", environment: "test", offer: null } } };
}

/** Explicit allowlist: neither ambient provider keys nor inherited production
 * database/QA/payment configuration reaches any service process. */
export function playableBaseEnvironment(environment = process.env) {
  const allowed = new Set(["PATH", "PATHEXT", "SYSTEMROOT", "WINDIR", "TEMP", "TMP", "USERPROFILE", "APPDATA", "LOCALAPPDATA", "COMSPEC", "LANG", "LC_ALL", "TZ"]);
  return Object.fromEntries(Object.entries(environment).filter(([key]) => allowed.has(key.toUpperCase())));
}

/** One server-only signing key shared by this world's Web verifier and Gateway.
 * Derive a separate domain-bound key rather than forwarding recovery/control or
 * model credentials. Existing state files and the identity-session key stay as-is.
 */
export function playableWalletAuthEnvironment(state) {
  validatePlayableState(state, { allowInitializing: true });
  const origin = "http://127.0.0.1:19041";
  const authSecret = createHmac("sha256", Buffer.from(state.keys.identitySessionSecret, "hex"))
    .update(JSON.stringify([SERVICE_SCHEMA, "wallet-auth-v1", state.world.realmId, state.world.worldEpoch, origin]))
    .digest("hex");
  return { MIR2_PASSKEY_AUTH_SECRET: authSecret, MIR2_PASSKEY_ALLOWED_ORIGINS: origin };
}

export function playableGatewayEnvironment(state, dataDir, { enrolled = false, supply = {}, environment = process.env } = {}) {
  validatePlayableState(state, { allowInitializing: !enrolled }); const paths = playablePaths(dataDir);
  const supplyKeys = ["MIR2_COMPANION_SUPPLY_JOURNAL_PATH", "MIR2_COMPANION_SUPPLY_JOURNAL_MAC_KEY"];
  if (enrolled && (!exact(supply, supplyKeys) || !secret(supply.MIR2_COMPANION_SUPPLY_JOURNAL_MAC_KEY)
    || supply.MIR2_COMPANION_SUPPLY_JOURNAL_PATH !== resolve(paths.supply, "supply-server.journal.json"))) throw failure();
  return { ...playableBaseEnvironment(environment), MIR2_GATEWAY_TCP_ADDR: "127.0.0.1:0", MIR2_GATEWAY_WEB_ADDR: "127.0.0.1:19040",
    MIR2_ACCOUNT_STORE_BACKEND: "file", MIR2_ACCOUNT_STORE_PATH: paths.accounts, MIR2_CHANNEL_IDENTITY_STORE_PATH: paths.channelIdentities,
    MIR2_SAVE_RECOVERY_DIR: paths.recovery,
    MIR2_SAVE_RECOVERY_MAC_KEY: state.keys.saveRecoveryMacKey, MIR2_IDENTITY_SESSION_SECRET: state.keys.identitySessionSecret,
    ...playableWalletAuthEnvironment(state),
    MIR2_IDENTITY_RECOVERY_PEPPER: state.keys.identityRecoveryPepper, MIR2_REALM_ID: state.world.realmId,
    MIR2_CONTENT_PROFILE: R16_GAMEPLAY_BASELINE.contentProfile, MIR2_QUEST_CADENCE: R16_GAMEPLAY_BASELINE.questCadence,
    MIR2_RUNTIME_ENV: "test", MIR2_ZONE_ROUTING_MODE: "single",
    MIR2_GATEWAY_ALLOW_UNSAFE_LOCAL_PLAYER_COMMANDS: "0", MIR2_GATEWAY_STDIN_SHUTDOWN: "1", MIR2_AI_LIVE_ENABLED: "0",
    MIR2_GATEWAY_TOKIO_WORKER_THREADS: "2", ...(enrolled ? { MIR2_COMPANION_POLICY_PATH: paths.policy, ...supply } : {}) };
}

export function playableNextEnvironment(state, environment = process.env) {
  validatePlayableState(state);
  return { ...playableBaseEnvironment(environment), ...playableProxyEnvironment(environment), ...playableWalletAuthEnvironment(state), NODE_ENV: "production", HOSTNAME: "127.0.0.1", PORT: "19041",
    NEXT_TELEMETRY_DISABLED: "1", NEXT_PUBLIC_MIR2_GATEWAY_WS_URL: "ws://127.0.0.1:19040/ws",
    MIR2_GATEWAY_HTTP_URL: "http://127.0.0.1:19040", NEXT_PUBLIC_MIR2_QUEST_GUIDANCE: R16_GAMEPLAY_BASELINE.questGuidance };
}

export function playableActorWorkerEnvironment(environment = process.env) {
  return { ...playableBaseEnvironment(environment), ...playableProxyEnvironment(environment) };
}

function playableProxyEnvironment(environment) {
  const proxyKeys = new Set(["http_proxy", "HTTP_PROXY", "https_proxy", "HTTPS_PROXY", "all_proxy", "ALL_PROXY", "no_proxy", "NO_PROXY"]);
  return Object.fromEntries(Object.entries(environment).filter(([key]) => proxyKeys.has(key)));
}

export function playableControlAddress(dataDir, platform = process.platform, temporaryDirectory = process.env.TEMP ?? "/tmp") {
  const id = createHash("sha256").update(resolve(dataDir).toLowerCase()).digest("hex").slice(0, 32);
  return platform === "win32" ? "\\\\.\\pipe\\mir2-r16-web-" + id : resolve(temporaryDirectory, "mir2-r16-web-" + id + ".sock");
}
