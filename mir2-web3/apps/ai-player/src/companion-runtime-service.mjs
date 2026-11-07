import { createHash, randomUUID } from "node:crypto";
import { performance } from "node:perf_hooks";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { ExperienceLedger, participantEventOf } from "./experience/experience-ledger.mjs";
import { CompanionModelRuntime } from "./model/companion-model-runtime.mjs";
import { validModelName } from "./model/openai-client.mjs";

const APP_ROOT = fileURLToPath(new URL("../", import.meta.url));
const OPERATIONS = ["view", "instruct", "teach", "proposeTeaching", "confirmTeaching", "cancelTeaching", "authorizeSupply", "cancelSupply", "authorizeLife", "resumeLife", "cancelLife", "lifeActivated", "revokeFeedback", "deleteFeedback", "deleteEpisode", "clearGrowth", "recall", "resume", "exportBegin", "exportChunk", "trialEvent"];
const PRIVATE_CONTEXT = ["requestId", "residentId", "archiveId", "worldEpoch", "generation", "ownerRef", "operation", "payload", "deadlineAtMs"];
const isRef = value => typeof value === "string" && /^[A-Za-z0-9._:-]{1,100}$/.test(value);
const exact = (value, keys) => value && typeof value === "object" && !Array.isArray(value) && Object.keys(value).every(key => keys.includes(key));

export function archiveKeyFromEnvironment(configuration, environment = process.env) {
  const name = configuration.archiveKeyEnv ?? "MIR2_COMPANION_ARCHIVE_KEY";
  if (typeof name !== "string" || !/^[A-Z][A-Z0-9_]{1,99}$/.test(name)) throw new Error("Invalid archive key reference.");
  const value = environment[name];
  if (typeof value !== "string" || !/^[a-f0-9]{64,128}$/i.test(value) || value.length % 2) throw new Error("Configure a trusted owner archive key (32–64 bytes, hex) outside the portable archive.");
  return Buffer.from(value, "hex");
}

export function validateRuntimeService(profile) {
  const service = profile.runtimeService;
  if (!service?.controlEnabled) return null;
  if (!exact(service, ["controlEnabled", "runtimeId", "provider", "model", "promptVersion", "credentialRef", "archiveKeyEnv", "experiment", "maxCallsPerHour"]) || service.controlEnabled !== true) throw new Error("Invalid runtime service configuration.");
  const companion = profile.companion;
  if (!companion || !isRef(companion.archiveId) || !isRef(companion.ownerRef) || !isRef(companion.worldEpoch) || !Number.isSafeInteger(companion.characterIndex) || companion.characterIndex < 0) throw new Error("Configure the trusted archive/owner/world/character enrollment before starting this service.");
  const providerValid = service.provider === "rules" ? service.model === "rules" && service.credentialRef == null : service.provider === "openai" && validModelName(service.model) && service.credentialRef === "OPENAI_API_KEY";
  if (!isRef(service.runtimeId) || !providerValid || !isRef(service.promptVersion) || service.maxCallsPerHour !== undefined && (!Number.isInteger(service.maxCallsPerHour) || service.maxCallsPerHour < 1 || service.maxCallsPerHour > 20)) throw new Error("Invalid local model configuration.");
  if (service.experiment && !exact(service.experiment, ["experimentId", "participantRef", "ledgerPath", "currency", "environment", "offer"])) throw new Error("Invalid experiment configuration.");
  return service;
}

/** Authenticated owner commands only; this adapter never proposes game assets. */
export class CompanionRuntimeService {
  #queue = Promise.resolve();
  #registered;
  #resolveRegistered;
  #rejectRegistered;
  #listener;
  #frameListener;
  #timer;
  #exports = new Map();
  #closed = false;
  #key;
  #lastSample;
  #runtimeSession;
  #lastEngagement;
  #volatileOperations = new Map();
  #modelRuntime;

  constructor({ client, store, controller, configuration, realmId, report = () => {}, environment = process.env, clock = Date.now, modelClient, supplyController = null, lifeController = null, giftController = null }) {
    this.client = client; this.store = store; this.controller = controller; this.configuration = configuration; this.realmId = realmId; this.report = report; this.clock = clock;
    this.supplyController = supplyController;
    this.lifeController = lifeController; this.giftController = giftController;
    this.#key = archiveKeyFromEnvironment(configuration, environment);
    this.archive = store.archive();
    this.generation = null;
    this.#registered = new Promise((resolve, reject) => { this.#resolveRegistered = resolve; this.#rejectRegistered = reject; });
    // Attach rejection handling even if registration fails before a request.
    this.#registered.catch(() => {});
    try { if (configuration.experiment) {
      const experiment = configuration.experiment;
      this.ledger = new ExperienceLedger({ path: resolve(APP_ROOT, experiment.ledgerPath ?? ".data/experience.sqlite"),
        experimentId: experiment.experimentId, realmId, worldEpoch: this.archive.worldEpoch,
        currency: experiment.currency ?? "CNY", environment: experiment.environment ?? "trial", offer: experiment.offer ?? null, clock });
      this.ledger.registerParticipant(this.archive.ownerRef, experiment.participantRef);
    }
      this.#modelRuntime = new CompanionModelRuntime({ client, store, controller, configuration, environment, clock, modelClient,
        onUsage: ({ operationId, model }) => { if (this.ledger && this.#runtimeSession) this.ledger.recordModelCall({ sessionId: this.#runtimeSession, operationId, model }); } });
    } catch (error) { this.ledger?.close(); this.#key.fill(0); throw error; }
  }

  async start() {
    this.#listener = request => {
      if (this.#closed) return;
      this.#queue = this.#queue.then(async () => { await this.#registered; await this.#handle(request); }).catch(() => {
        this.report({ type: "companionControlUnavailable" });
      });
    };
    this.client.on("companionRequest", this.#listener);
    try {
      if (this.client.worldEpoch !== this.archive.worldEpoch) throw new Error("The registered world epoch does not match this archive.");
      const registration = await this.client.registerCompanionRuntime({ residentId: this.archive.residentId, archiveId: this.archive.archiveId });
      if (this.#closed) throw new Error("Companion service closed during registration.");
      const result = registration.result;
      if (registration.status !== "ok" || !result || result.archiveId !== this.archive.archiveId || result.ownerRef !== this.archive.ownerRef || result.worldEpoch !== this.archive.worldEpoch || !Number.isSafeInteger(result.generation) || result.generation < 1) throw new Error("Runtime enrollment was not accepted.");
      this.generation = result.generation;
      this.#resolveRegistered();
      if (this.ledger) {
        this.#runtimeSession = this.ledger.startRuntime({ ownerRef: this.archive.ownerRef, archiveId: this.archive.archiveId, runtimeId: this.configuration.runtimeId });
        this.#lastSample = performance.now();
        this.#timer = setInterval(() => this.#sample(), 1000); this.#timer.unref?.();
        this.#frameListener = ({ event }) => {
          if (event.packet !== "ObjectStruck") return;
          const state = this.client.snapshot(), info = event.payload;
          const ownIds = [state.playerObjectId, state.selfZoneObjectId].filter(id => Number.isInteger(id) && id >= 0 && id <= 0xffff_ffff);
          if (!ownIds.includes(info?.attackerId) || !this.#ownerInParty(state) || !state.entities.some(entity => entity.kind === "monster" && entity.objectId === info.objectId)) return;
          const evidenceRef = `strike:${state.connectionEpoch}:${state.revision}:${info.objectId}`;
          if (evidenceRef === this.#lastEngagement) return;
          this.#lastEngagement = evidenceRef;
          try { this.ledger.recordObservedAdventure({ ownerRef: this.archive.ownerRef, archiveId: this.archive.archiveId, evidenceRef }); }
          catch { this.report({ type: "companionExperienceWriteFailed" }); }
        };
        this.client.on("frame", this.#frameListener);
      }
      this.report({ type: "companionControlReady", archiveId: this.archive.archiveId, runtimeId: this.configuration.runtimeId, modelEnabled: this.#modelRuntime.view().modelEnabled });
    } catch (error) { this.#rejectRegistered(error); await this.close(); throw error; }
  }

  get modelEnabled() { return this.#modelRuntime?.view().modelEnabled === true; }
  async planLife(input) {
    const blocked = () => { const state = this.client.snapshot(), gift = this.giftController?.view(); return this.#closed || state.trade?.active || state.trade?.uncertain || gift && !["idle", "completed", "cancelled"].includes(gift.status); };
    if (blocked()) return null;
    const result = await this.#modelRuntime.planLife(input);
    return blocked() ? null : result;
  }
  #ownerInParty(state) {
    const members = state.party?.members ?? [];
    return members.includes(state.player?.name) && members.includes(this.controller.leaderName);
  }
  #sample() {
    if (this.#closed || !this.ledger) return;
    const now = performance.now(), elapsed = now - this.#lastSample; this.#lastSample = now;
    const state = this.client.snapshot();
    try {
      this.ledger.measureRuntime(this.#runtimeSession, { elapsedMs: state.ready ? Math.min(Math.floor(elapsed), 5000) : 0,
        sharedParty: state.ready && this.#ownerInParty(state), coverageGap: elapsed > 5000 || !state.ready });
    } catch { this.report({ type: "companionExperienceWriteFailed" }); }
    for (const [id, value] of this.#exports) if (value.expiresAt < this.clock()) this.#exports.delete(id);
  }
  #view(query = {}) {
    return { ...this.store.ownerView(query), runtime: { runtimeId: this.configuration.runtimeId, ...this.#modelRuntime.view(), promptVersion: this.configuration.promptVersion, recallRequested: this.controller.recallRequested,
      ...(this.supplyController ? { supplyTask: this.supplyController.view() } : {}),
      ...(this.lifeController ? { lifeTask: this.lifeController.view(), resources: this.controller.resourceView?.() ?? this.lifeController.view().resources ?? null } : {}),
      ...(this.giftController ? { giftTask: this.giftController.view() } : {}) },
      trial: this.ledger?.ownerView(this.archive.ownerRef, this.archive.archiveId) ?? null };
  }
  #auth(request) {
    return { source: "gateway-owner", ownerRef: request.ownerRef, archiveId: request.archiveId, worldEpoch: request.worldEpoch, operationId: request.requestId };
  }
  #clearExports() {
    for (const value of this.#exports.values()) value.buffer.fill(0);
    this.#exports.clear();
  }
  async #handle(request) {
    if (this.#closed || !exact(request, PRIVATE_CONTEXT) || request.generation !== this.generation || request.residentId !== this.archive.residentId || request.archiveId !== this.archive.archiveId || request.worldEpoch !== this.archive.worldEpoch || request.ownerRef !== this.archive.ownerRef || !isRef(request.requestId) || !OPERATIONS.includes(request.operation) || !Number.isSafeInteger(request.deadlineAtMs) || request.deadlineAtMs < this.clock()) return;
    let status = "ok", result, committed = false;
    try {
      const payload = request.payload ?? {}, auth = this.#auth(request);
      if (["instruct", "teach", "proposeTeaching", "confirmTeaching", "authorizeSupply", "authorizeLife", "resumeLife", "revokeFeedback", "deleteFeedback", "deleteEpisode", "clearGrowth"].includes(request.operation) && (!Number.isSafeInteger(payload.expectedPolicyVersion) || payload.expectedPolicyVersion < 1)) throw new Error("A current policy version is required");
      if (["instruct", "authorizeSupply", "authorizeLife", "resumeLife"].includes(request.operation)) {
        const state = this.client.snapshot(), gift = this.giftController?.view();
        if (state.trade?.active || state.trade?.uncertain || gift && !["idle", "completed", "cancelled"].includes(gift.status)) throw new Error("Resolve the current owner gift trade first");
      }
      if (["instruct", "authorizeSupply"].includes(request.operation) && this.lifeController && !["idle", "completed", "cancelled", "failed"].includes(this.lifeController.view().status)) throw new Error("Stop the current life before starting a different task");
      switch (request.operation) {
        case "view":
          if (!exact(payload, ["cursor", "limit"])) throw new Error("Invalid view request");
          result = this.#view(payload); break;
        case "instruct":
          if (!exact(payload, ["text", "expectedPolicyVersion"])) throw new Error("Invalid instruction request");
          this.#modelRuntime.instruct({ requestId: request.requestId, ...payload });
          committed = true; this.#clearExports(); result = this.#view(); break;
        case "proposeTeaching":
          if (!exact(payload, ["text", "expectedPolicyVersion"])) throw new Error("Invalid teaching proposal request");
          this.#modelRuntime.proposeTeaching({ requestId: request.requestId, ...payload });
          committed = true; this.#clearExports(); result = this.#view(); break;
        case "confirmTeaching":
          if (!exact(payload, ["proposalId", "expectedPolicyVersion"]) || !isRef(payload.proposalId)) throw new Error("Invalid teaching confirmation");
          this.#modelRuntime.confirmTeaching({ requestId: request.requestId, ...payload, auth });
          committed = true; this.#clearExports(); result = this.#view(); break;
        case "cancelTeaching":
          if (!exact(payload, ["proposalId"]) || !isRef(payload.proposalId)) throw new Error("Invalid teaching cancellation");
          this.#modelRuntime.cancelTeaching(payload); committed = true; this.#clearExports(); result = this.#view(); break;
        case "authorizeSupply":
          if (!exact(payload, ["expectedPolicyVersion", "budgetGold", "quantity", "expiresInSeconds", "grant"]) || !this.supplyController || !payload.grant) throw new Error("Supply authorization is unavailable");
          if (payload.expectedPolicyVersion !== this.store.profile().policyVersion) { const error = new Error("Policy version changed"); error.code = "conflict"; throw error; }
          this.supplyController.authorize(payload.grant); committed = true;
          this.#modelRuntime.invalidateMemory("已接受有限补给，旧委托已暂停。"); result = this.#view(); break;
        case "cancelSupply":
          if (!exact(payload, []) || !this.supplyController) throw new Error("Supply cancellation is unavailable");
          this.supplyController.cancel("owner-revoked"); committed = true; result = this.#view(); break;
        case "authorizeLife": case "resumeLife": {
          const resume = request.operation === "resumeLife";
          const keys = resume ? ["expectedPolicyVersion", "lifePermitId", "grant"] : ["expectedPolicyVersion", "budgetGold", "durationSeconds", "grant"];
          if (!exact(payload, keys) || !this.lifeController || !payload.grant || resume && payload.lifePermitId !== payload.grant.lifePermitId || !resume && (payload.budgetGold !== payload.grant.budgetGold || payload.durationSeconds !== payload.grant.durationSeconds)) throw new Error("Life authorization is unavailable");
          if (payload.expectedPolicyVersion !== this.store.profile().policyVersion) { const error = new Error("Policy version changed"); error.code = "conflict"; throw error; }
          this.#modelRuntime.pause("已接受自主生活，旧模型委托已暂停。");
          this.controller.recallRequested = false;
          this.lifeController[resume ? "resume" : "authorize"](payload.grant, { awaitActivation: true });
          committed = true; result = this.#view(); break;
        }
        case "lifeActivated":
          if (!exact(payload, ["lifePermitId"]) || !isRef(payload.lifePermitId) || !this.lifeController) throw new Error("Invalid internal life activation");
          this.lifeController.confirmActivation(payload.lifePermitId); committed = true; result = this.#view(); break;
        case "cancelLife":
          if (!exact(payload, []) || !this.lifeController) throw new Error("Life cancellation is unavailable");
          this.lifeController.cancel("owner-revoked"); this.#modelRuntime.pause("主人已撤销自主生活。"); committed = true; result = this.#view(); break;
        case "teach":
          if (!exact(payload, ["expectedPolicyVersion"])) throw new Error("Invalid teaching request");
          this.store.teachSupportBeforeLoot({ ...auth, ...payload, feedbackId: request.requestId,
            feedbackText: "组队时先支援，再拾取；离开队伍前先问我。", context: { mode: "party" } });
          committed = true; this.#clearExports(); this.#modelRuntime.invalidateMemory("已更新培养习惯，请重新委托以使用新习惯。");
          result = this.#view(); break;
        case "revokeFeedback": case "deleteFeedback": case "deleteEpisode": {
          const key = request.operation === "deleteEpisode" ? "episodeId" : "feedbackId";
          if (!exact(payload, [key, "expectedPolicyVersion"])) throw new Error("Invalid memory request");
          this.store[request.operation]({ ...auth, ...payload }); committed = true; this.#clearExports(); this.#modelRuntime.invalidateMemory("记忆已变化，请重新委托。"); result = this.#view(); break;
        }
        case "clearGrowth":
          if (!exact(payload, ["confirmArchiveId", "expectedPolicyVersion"])) throw new Error("Invalid reset request");
          this.store.clearGrowth({ ...auth, ...payload }); committed = true; this.#clearExports(); this.#modelRuntime.invalidateMemory("成长记忆已清除，旧计划已暂停。", { clear: true }); result = this.#view(); break;
        case "recall": case "resume":
          if (!exact(payload, [])) throw new Error("Invalid recall request");
          {
            const fingerprint = `${request.operation}:empty`;
            const previous = this.#volatileOperations.get(request.requestId);
            if (previous && previous !== fingerprint) throw new Error("Operation ID conflicts");
            if (!previous) {
              if (request.operation === "recall") { this.lifeController?.cancel("owner-recalled"); this.supplyController?.cancel("owner-recalled"); }
              this.controller.recallRequested = request.operation === "recall";
              if (request.operation === "recall") this.#modelRuntime.pause("主人已召回，模型委托已暂停。");
              this.#volatileOperations.set(request.requestId, fingerprint);
              if (this.#volatileOperations.size > 512) this.#volatileOperations.delete(this.#volatileOperations.keys().next().value);
            }
            committed = true; result = this.#view(); break;
          }
        case "trialEvent":
          if (!this.ledger) throw new Error("Trial recording is not configured");
          this.ledger.recordParticipantEvent({ operationId: request.requestId, ownerRef: this.archive.ownerRef, archiveId: this.archive.archiveId, event: participantEventOf(payload) });
          committed = true;
          result = this.#view(); break;
        case "exportBegin": {
          if (!exact(payload, [])) throw new Error("Invalid export request");
          for (const [id, value] of this.#exports) if (value.expiresAt < this.clock()) this.#exports.delete(id);
          if (this.#exports.size >= 2) throw new Error("An export is already pending");
          const buffer = Buffer.from(this.store.exportArchive({ key: this.#key }), "utf8");
          if (buffer.length > 2 * 1024 * 1024) throw new Error("The complete archive exceeds the export limit");
          const exportId = randomUUID(), chunkCount = Math.ceil(buffer.length / 16384);
          this.#exports.set(exportId, { buffer, expiresAt: this.clock() + 120_000, ownerRef: request.ownerRef });
          result = { exportId, totalBytes: buffer.length, sha256: createHash("sha256").update(buffer).digest("hex"), chunkCount }; break;
        }
        case "exportChunk": {
          if (!exact(payload, ["exportId", "index"]) || !isRef(payload.exportId) || !Number.isSafeInteger(payload.index) || payload.index < 0) throw new Error("Invalid export chunk request");
          const value = this.#exports.get(payload.exportId), chunkCount = value && Math.ceil(value.buffer.length / 16384);
          if (!value || value.expiresAt < this.clock() || value.ownerRef !== request.ownerRef || payload.index >= chunkCount) throw new Error("Export is not available");
          result = { exportId: payload.exportId, index: payload.index, chunkCount, dataBase64: value.buffer.subarray(payload.index * 16384, (payload.index + 1) * 16384).toString("base64") }; break;
        }
      }
    } catch (error) {
      if (error.committed === true) { committed = true; this.#clearExports(); this.#modelRuntime.invalidateMemory("记忆已更新，请刷新后重新委托。", { clear: true }); }
      if (error.code === "conflict") { status = "conflict"; result = this.#view(); }
      else { status = committed ? "unknown" : "error"; result = { message: "The companion result is unavailable; refresh the current state before deciding whether to retry." }; }
    }
    if (Buffer.byteLength(JSON.stringify(result)) > 56 * 1024) { status = committed ? "unknown" : "error"; result = { message: "This page exceeds the response limit; use a smaller page." }; }
    if (!this.#closed) this.client.replyCompanionRuntime({ requestId: request.requestId, generation: this.generation, status, result });
  }

  async close() {
    if (this.#closed) return;
    if (this.#runtimeSession) this.#sample();
    this.#closed = true;
    await this.#modelRuntime?.close();
    this.#rejectRegistered(new Error("Companion service closed."));
    clearInterval(this.#timer);
    if (this.#listener) this.client.off("companionRequest", this.#listener);
    if (this.#frameListener) this.client.off("frame", this.#frameListener);
    await this.#queue;
    try {
      if (this.#runtimeSession) this.ledger?.stopRuntime(this.#runtimeSession);
    } catch { this.report({ type: "companionExperienceWriteFailed" }); }
    finally {
      try { this.ledger?.close(); }
      finally { this.#clearExports(); this.#key.fill(0); this.#volatileOperations.clear(); }
    }
  }
}
