import { createHash } from "node:crypto";
import { buildModelContext, validateMemoryReferences, MAX_MODEL_CONTEXT_BYTES } from "./memory-context.mjs";
import { OpenAIPlanClient, ModelCallError, validApiKey, validModelName } from "./openai-client.mjs";
import { PLAN_SCHEMA, PLANNER_INSTRUCTIONS, instructionText, validatePlan } from "./plan.mjs";
import { TEACHING_SCHEMA, TEACHING_INSTRUCTIONS, validateTeachingPlan } from "./teaching-plan.mjs";
import { payloadHash } from "../memory/archive-codec.mjs";
import { LIFE_PLAN_SCHEMA, LIFE_PLANNER_INSTRUCTIONS, lifePlanningRoute, validateLifePlan } from "./life-plan.mjs";
import { projectLifeResources } from "../life/resources.mjs";

const point = v => Number.isInteger(v?.x) && Number.isInteger(v?.y);
const distance = (a, b) => Math.max(Math.abs(a.x - b.x), Math.abs(a.y - b.y));
const signature = s => JSON.stringify([s.connectionEpoch, s.mapFileName, s.mapIndex, s.playerObjectId, s.selfZoneObjectId]);
const liveMonster = e => e?.kind === "monster" && point(e) && e.dead !== true && (e.hp == null || e.hp > 0) && e.disposition !== "friendly";
const objectId = id => Number.isInteger(id) && id >= 0 && id <= 0xffff_ffff;
const SAFE_BASE_WAITS = new Set(["near-party-leader", "no-authorized-party-activity", "party-combat-still-active"]);
const isCall = e => /^(?:goal|teaching|life):[a-f0-9]{64}:call$/.test(e.episodeId);
const ERRORS = { authentication: "OpenAI 密钥或账户权限未通过验证，请检查本机模型设置。", "rate-limit": "OpenAI 当前限流或额度不足；本次未重试。", timeout: "模型调用超时；未自动重试。", refused: "模型未接受这次委托，请换一种说法。", incomplete: "模型未返回完整计划，本次没有执行。", "invalid-plan": "模型计划未通过游戏规则检查，本次没有执行。", "stale-plan": "等待模型期间世界、记忆或培养版本已变化，请重新委托。", "call-budget": "已达到本机每小时的模型调用上限，请稍后再试。" };

/** Model chooses bounded goals. The existing ordinary executor remains the only actuator. */
export class CompanionModelRuntime {
  #client; #store; #controller; #configuration; #clock; #model; #abort; #pending; #task = null;
  #active = null; #listener; #closed = false; #onUsage; #calls = 0;
  #teaching = null; #teachingAbort; #teachingPending; #teachingContext;
  #lifePlan = null; #lifeAbort; #lifeGeneration = 0;

  constructor({ client, store, controller, configuration, environment = process.env, clock = Date.now, modelClient, onUsage = () => {} }) {
    this.#client = client; this.#store = store; this.#controller = controller; this.#configuration = configuration; this.#clock = clock; this.#onUsage = onUsage;
    const key = configuration.credentialRef === "OPENAI_API_KEY" ? environment.OPENAI_API_KEY : null;
    if (configuration.provider === "openai" && validApiKey(key)) this.#model = modelClient ?? new OpenAIPlanClient({ apiKey: key, model: configuration.model, environment });
    const episodes = store.growth().episodes;
    this.#calls = episodes.filter(isCall).length;
    const last = episodes.findLast(e => e.status === "active" && e.payload?.kind === "owner-instruction");
    if (last) {
      const tail = episodes.find(e => e.status === "active" && e.episodeId === `${last.episodeId}:part2`);
      let priorObjective;
      try {
        const saved = episodes.find(e => e.status === "active" && e.episodeId === `${last.episodeId}:objective` && e.payload?.kind === "model-objective" && e.payload.outcome === "proposed");
        if (saved) priorObjective = JSON.parse(saved.payload.summary);
      } catch { /* Legacy/incomplete metadata is not evidence of requested work. */ }
      const completedKills = episodes.filter(e => e.status === "active" && e.payload?.kind === "objective-progress" && e.payload.outcome === "observed" && e.payload.action === "hunt" && e.payload.context?.sourceEventRef === last.episodeId && e.episodeId.startsWith(`${last.episodeId}:kill:`)).length;
      const requestedKills = priorObjective?.kind === "hunt" && Number.isInteger(priorObjective.count) && priorObjective.count >= 1 && priorObjective.count <= 3 ? priorObjective.count : 0;
      this.#task = { status: "paused", goalId: last.episodeId, instruction: last.payload.summary + (tail?.payload?.summary ?? ""), reply: "上次委托和实际经历已保留。重启后请重新委托，确认后再行动。", step: "awaiting-new-instruction", completedKills, requestedKills, references: [] };
    }
    this.#listener = ({ event }) => this.#observe(event);
    client.on("frame", this.#listener);
    controller.modelDriver = this;
  }

  view() {
    return { provider: this.#configuration.provider, model: this.#configuration.model, configured: Boolean(this.#model),
      modelEnabled: Boolean(this.#model), modelCalls: this.#calls, modelTask: this.#task ? structuredClone(this.#task) : { status: "idle" },
      teachingProposal: this.#teaching ? structuredClone(this.#teaching) : { status: "idle" },
      lifePlan: this.#lifePlan ? structuredClone(this.#lifePlan) : { status: "idle" } };
  }

  /** One phase request from the currently authorized life controller. This is
   * neither a new hard grant nor the party hunt actuator used below. */
  async planLife({ lifePermitId, reason, state, resources: _resources, routeId, waypoints }) {
    if (this.#closed || !this.#model) return null;
    let life, phase, currentState, now, archive;
    try { life = this.#controller.lifeController; phase = life?.view?.(); currentState = this.#client.snapshot(); now = this.#clock(); archive = this.#store.archive(); }
    catch { return null; }
    const ordinal = phase?.modelDecisionsUsed, generation = this.#lifeGeneration;
    const valid = () => {
      const current = this.#client.snapshot(), currentLife = this.#controller.lifeController?.view?.(), clock = this.#clock();
      return !this.#closed && this.#lifeGeneration === generation && this.#controller.lifeController === life && currentLife?.lifePermitId === lifePermitId && currentLife.status === "running" && currentLife.activationPending !== true &&
        currentLife.modelDecisionsUsed === ordinal && currentLife.expiresAtMs === phase?.expiresAtMs && Number.isSafeInteger(clock) && clock >= 0 &&
        Number.isSafeInteger(currentLife.startedAtMs) && currentLife.startedAtMs <= clock && [1200, 1800].includes(currentLife.durationSeconds) && currentLife.expiresAtMs - currentLife.startedAtMs === currentLife.durationSeconds * 1000 && clock < currentLife.expiresAtMs &&
        this.#controller.recallRequested !== true && current.ready === true && current.mapFileName === "0" && point(current.player) && current.player.dead !== true && current.playerHp > 0 &&
        signature(current) === signature(currentState) && typeof archive.worldEpoch === "string" && archive.worldEpoch.length > 0 && Number.isSafeInteger(archive.characterIndex) && archive.characterIndex >= 0 && this.#client.worldEpoch === archive.worldEpoch && this.#client.activeCharacterIndex === archive.characterIndex;
    };
    if (typeof lifePermitId !== "string" || !/^[A-Za-z0-9._:-]{1,100}$/.test(lifePermitId) || !["authorized", "resumed", "supply-completed", "rest-completed"].includes(reason) ||
        !Number.isInteger(ordinal) || ordinal < 1 || ordinal > 4 || !Number.isSafeInteger(now) || now < 0 || !state?.ready || signature(state) !== signature(currentState) || !valid()) return null;
    const base = `life:${createHash("sha256").update(`${lifePermitId}:${ordinal}`).digest("hex")}`, callId = `${base}:call`, episodes = this.#store.growth().episodes;
    if (episodes.some(row => row.episodeId === callId)) return null; // Includes retries, restarts and tombstones.
    const callLimit = Math.min(20, this.#configuration.maxCallsPerHour ?? 20);
    if (!Number.isInteger(callLimit) || callLimit < 1 || episodes.filter(row => isCall(row) && row.recordedAt >= now - 3_600_000).length >= callLimit) return null;
    const policyVersion = this.#store.profile().policyVersion;
    let context;
    try {
      const resourceState = this.#controller.resourceView?.() ?? projectLifeResources(currentState, { observedAtMs: now, medicineUnresolved: currentState.medicineUnresolved === true });
      // Reproject from the actual actor, using only the controller's medicine
      // quarantine hint. Caller-provided resource objects are never forwarded.
      const safeResources = projectLifeResources(currentState, { observedAtMs: now, medicineUnresolved: resourceState.medicineUnresolved === true });
      if (resourceState.inventoryCurrent === false) { safeResources.inventoryCurrent = false; safeResources.known = false; for (const key of ["hpPotionCount", "usableHpPotionCount", "freeBagSlots", "currentWeight", "maxWeight"]) safeResources[key] = null; }
      if (resourceState.usableHpPotionCount === null) safeResources.usableHpPotionCount = null;
      else if (Number.isSafeInteger(resourceState.usableHpPotionCount) && resourceState.usableHpPotionCount >= 0 && safeResources.usableHpPotionCount !== null) safeResources.usableHpPotionCount = Math.min(safeResources.usableHpPotionCount, resourceState.usableHpPotionCount);
      if (safeResources.usableHpPotionCount === null) safeResources.known = false;
      context = { ...buildModelContext({ state: currentState, store: this.#store, text: "比奇独立生活 巡猎 休息 药品 回城 真实经历", leaderName: this.#controller.leaderName }),
        phase: reason, resources: safeResources, route: lifePlanningRoute(routeId, waypoints) };
      while (Buffer.byteLength(JSON.stringify(context)) > MAX_MODEL_CONTEXT_BYTES && context.memories.length) { context.memories.pop(); context.memoryStamp.pop(); }
      while (Buffer.byteLength(JSON.stringify(context)) > MAX_MODEL_CONTEXT_BYTES && context.world.nearbyDrops.length) context.world.nearbyDrops.pop();
      while (Buffer.byteLength(JSON.stringify(context)) > MAX_MODEL_CONTEXT_BYTES && context.world.monsters.length) context.world.monsters.pop();
      if (Buffer.byteLength(JSON.stringify(context)) > MAX_MODEL_CONTEXT_BYTES || !valid()) return null;
    } catch { return null; }
    this.#lifeAbort?.abort();
    const abort = new AbortController(); this.#lifeAbort = abort;
    const matches = () => valid() && !abort.signal.aborted && this.#lifeAbort === abort && this.#store.profile().policyVersion === policyVersion;
    let knownCall = null;
    try {
      this.#store.recordEpisode({ episodeId: callId, kind: "model-call", action: "responses", outcome: "attempted", summary: "已向配置模型请求一次有限生活阶段建议；不自动重试。", context: { sourceEventRef: base } });
      this.#calls++; this.#onUsage({ operationId: callId, model: this.#configuration.model });
      if (!matches()) return null;
      this.#lifePlan = { status: "planning", ordinal, phase: reason, reply: "正在根据真实资源和经历选择下一阶段。", references: [], lastCall: { model: this.#configuration.model, status: "in-flight" } };
      const response = await this.#model.plan({ instructions: LIFE_PLANNER_INSTRUCTIONS, context, schema: LIFE_PLAN_SCHEMA, signal: abort.signal });
      if (!matches()) {
        if (!this.#closed && this.#lifeAbort === abort) this.#lifePlan = { ...this.#lifePlan, status: "paused", reply: "生活、世界或记忆状态已改变；旧模型建议未采用。", references: [], lastCall: { model: this.#configuration.model, status: "stale-plan" } };
        return null;
      }
      const call = response.call ?? {}, tokens = key => Number.isSafeInteger(call[key]) && call[key] >= 0 ? call[key] : null;
      knownCall = { model: this.#configuration.model, status: "completed", ...(validModelName(call.providerModel) ? { providerModel: call.providerModel } : {}),
        ...Object.fromEntries(["requestId", "responseId"].filter(key => typeof call[key] === "string" && /^[A-Za-z0-9_-]{1,160}$/.test(call[key])).map(key => [key, call[key]])),
        inputTokens: tokens("inputTokens"), outputTokens: tokens("outputTokens") };
      this.#store.recordEpisode({ episodeId: `${base}:usage`, kind: "model-usage", action: "responses", outcome: "received", summary: JSON.stringify({ inputTokens: knownCall.inputTokens, outputTokens: knownCall.outputTokens }), context: { sourceEventRef: callId } });
      const plan = validateLifePlan(response.plan);
      if (!validateMemoryReferences(plan.rememberedEpisodeIds, context, this.#store) || !matches()) throw new ModelCallError("stale-plan");
      this.#store.recordEpisode({ episodeId: `${base}:plan`, kind: "model-plan", action: `life-${plan.mode}`, outcome: "proposed", summary: plan.reply, context: { sourceEventRef: base } });
      this.#lifePlan = { status: "proposed", ordinal, phase: reason, reply: plan.reply, references: plan.rememberedEpisodeIds.map(episodeId => ({ episodeId, summary: context.memories.find(row => row.episodeId === episodeId).summary })), lastCall: knownCall };
      return plan;
    } catch (error) {
      if (!matches()) return null;
      const code = error instanceof ModelCallError && ["authentication", "rate-limit", "timeout", "refused", "incomplete", "invalid-plan", "stale-plan", "cancelled", "tls-trust", "transport-or-format", "http-error", "response-too-large", "invalid-response"].includes(error.code) ? error.code : "unavailable";
      this.#lifePlan = { status: "failed", ordinal, phase: reason, reply: ERRORS[code] ?? "生活阶段模型建议暂不可用；执行器保持安全规则，不自动重试。", references: [], lastCall: knownCall ?? { model: this.#configuration.model, status: code } };
      return null;
    } finally { if (this.#lifeAbort === abort) this.#lifeAbort = null; }
  }

  instruct({ requestId, text, expectedPolicyVersion }) {
    const instruction = instructionText(text);
    if (this.#closed) throw new Error("Model runtime is closed.");
    if (!this.#model) throw new ModelCallError("configuration");
    if (expectedPolicyVersion !== this.#store.profile().policyVersion) { const error = new Error("Policy version changed."); error.code = "conflict"; throw error; }
    const digest = createHash("sha256").update(instruction).digest("hex"), goalId = `goal:${createHash("sha256").update(requestId).digest("hex")}`;
    const existing = this.#store.growth().episodes.find(e => e.episodeId === goalId);
    if (existing) {
      if (existing.payload?.objectRef !== `instruction:${digest}` && existing.status !== "deleted") throw new Error("Instruction ID conflicts.");
      // A retry, including after restart/deletion, never spends another model call.
      return;
    }
    const now = this.#clock(), growth = this.#store.growth();
    const recentCalls = growth.episodes.filter(e => isCall(e) && e.recordedAt >= now - 3_600_000);
    if (recentCalls.length >= (this.#configuration.maxCallsPerHour ?? 20)) throw new ModelCallError("call-budget");
    const state = this.#client.snapshot();
    if (!state.ready || !point(state.player)) throw new Error("World is unavailable.");
    this.pause("新委托已替换上一个计划。");
    const chars = [...instruction];
    this.#store.recordEpisodes([{ episodeId: goalId, kind: "owner-instruction", action: "instruct", outcome: "received",
      summary: chars.slice(0, 500).join(""), objectRef: `instruction:${digest}`, context: { sourceEventRef: goalId } },
    ...(chars.length > 500 ? [{ episodeId: `${goalId}:part2`, kind: "owner-instruction-continuation", action: "instruct", outcome: "received", summary: chars.slice(500).join(""), context: { sourceEventRef: goalId } }] : [])]);
    this.#task = { status: "planning", goalId, instruction, reply: "正在读取当前观察和经历，等待模型计划。", step: "planning", completedKills: 0, requestedKills: 0, references: [] };
    const abort = new AbortController(); this.#abort = abort;
    let context;
    try { context = buildModelContext({ state, store: this.#store, text: instruction, leaderName: this.#controller.leaderName }); }
    catch { this.pause("委托已保存，但读取经历失败，请刷新后重新委托。"); const error = new Error("Instruction saved; model context unavailable."); error.committed = true; throw error; }
    this.#pending = this.#plan({ goalId, instruction, context, world: signature(state), policyVersion: expectedPolicyVersion, abort });
  }

  async #plan({ goalId, instruction, context, world, policyVersion, abort }) {
    const current = () => !this.#closed && !abort.signal.aborted && this.#task?.goalId === goalId && this.#task.status === "planning";
    try {
      this.#store.recordEpisode({ episodeId: `${goalId}:call`, kind: "model-call", action: "responses", outcome: "attempted", summary: "已向配置的 OpenAI 模型发起一次计划请求；不自动重试。", context: { sourceEventRef: goalId } });
      this.#calls += 1; this.#onUsage({ operationId: `${goalId}:call`, model: this.#configuration.model });
      this.#task.lastCall = { model: this.#configuration.model, status: "in-flight" };
      const response = await this.#model.plan({ instructions: PLANNER_INSTRUCTIONS, context: { instruction, ...context }, schema: PLAN_SCHEMA, signal: abort.signal });
      if (!current()) return;
      const plan = validatePlan(response.plan), state = this.#client.snapshot();
      if (!state.ready || state.player?.dead === true || state.playerHp === 0 || signature(state) !== world || this.#store.profile().policyVersion !== policyVersion || !validateMemoryReferences(plan.rememberedEpisodeIds, context, this.#store)) throw new ModelCallError("stale-plan");
      // Record generated words explicitly as a proposal, never as a successful action.
      this.#store.recordEpisodes([
        { episodeId: `${goalId}:plan`, kind: "model-plan", action: plan.objective.kind.replaceAll("_", "-"), outcome: "proposed", summary: plan.reply, context: { sourceEventRef: goalId } },
        { episodeId: `${goalId}:objective`, kind: "model-objective", action: plan.objective.kind.replaceAll("_", "-"), outcome: "proposed", summary: JSON.stringify(plan.objective), context: { sourceEventRef: goalId } },
      ]);
      this.#task = { ...this.#task, status: "running", reply: plan.reply, step: plan.objective.kind === "hunt" ? "seeking-monster" : plan.objective.kind,
        requestedKills: plan.objective.count, references: plan.rememberedEpisodeIds.map(episodeId => ({ episodeId, summary: context.memories.find(e => e.episodeId === episodeId).summary })), lastCall: response.call };
      this.#active = { goalId, plan: plan.objective, world, policyVersion, context, references: plan.rememberedEpisodeIds,
        hits: new Map(), counted: new Set(), targetId: null, startedAt: this.#clock() };
    } catch (error) {
      if (!current()) return;
      const code = error instanceof ModelCallError ? error.code : "unavailable";
      this.#task = { ...this.#task, status: "failed", step: "failed", error: ERRORS[code] ?? "模型计划暂不可用，请检查本机设置或稍后重新委托。", lastCall: { model: this.#configuration.model, status: code,
        ...(error instanceof ModelCallError && error.httpStatus ? { httpStatus: error.httpStatus } : {}),
        ...(error instanceof ModelCallError && error.requestId ? { requestId: error.requestId } : {}) } };
      this.#active = null;
    }
  }

  proposeTeaching({ requestId, text, expectedPolicyVersion }) {
    const instruction = instructionText(text);
    if (this.#closed) throw new Error("Model runtime is closed.");
    if (!this.#model) throw new ModelCallError("configuration");
    const base = `teaching:${createHash("sha256").update(requestId).digest("hex")}`, instructionId = `${base}:input`, proposalId = `${base}:proposal`;
    const digest = createHash("sha256").update(instruction).digest("hex");
    const existing = this.#store.growth().episodes.find(e => e.episodeId === instructionId);
    if (existing) {
      if (existing.status !== "deleted" && existing.payload?.objectRef !== `instruction:${digest}`) throw new Error("Teaching instruction ID conflicts.");
      return; // A retry or tombstone never produces another charge.
    }
    if (expectedPolicyVersion !== this.#store.profile().policyVersion) { const error = new Error("Policy version changed."); error.code = "conflict"; throw error; }
    const now = this.#clock();
    if (this.#store.growth().episodes.filter(e => isCall(e) && e.recordedAt >= now - 3_600_000).length >= (this.#configuration.maxCallsPerHour ?? 20)) throw new ModelCallError("call-budget");
    const state = this.#client.snapshot();
    if (!state.ready || !point(state.player)) throw new Error("World is unavailable.");
    this.#invalidateTeaching("新教学已替换上一个待确认提案。");
    const chars = [...instruction], input = [{ episodeId: instructionId, kind: "owner-teaching-instruction", action: "propose-teaching", outcome: "received",
      summary: chars.slice(0, 500).join(""), objectRef: `instruction:${digest}`, context: { sourceEventRef: instructionId } },
    ...(chars.length > 500 ? [{ episodeId: `${instructionId}:part2`, kind: "owner-teaching-continuation", action: "propose-teaching", outcome: "received", summary: chars.slice(500).join(""), context: { sourceEventRef: instructionId } }] : [])];
    this.#store.recordEpisodes(input);
    let context;
    try { context = buildModelContext({ state, store: this.#store, text: instruction, leaderName: this.#controller.leaderName }); }
    catch {
      const error = new Error("Teaching saved; model context unavailable."); error.committed = true; throw error;
    }
    this.#teachingContext = { context, inputStamps: this.#store.growth().episodes.filter(e => input.some(row => row.episodeId === e.episodeId)).map(e => ({ episodeId: e.episodeId, payloadHash: e.payloadHash })),
      instructionId, instruction, world: signature(state), startedAt: now };
    this.#teaching = { proposalId, status: "planning", sourceVersion: expectedPolicyVersion, reply: "正在解释教学；确认后才会改变习惯。", changes: {}, references: [] };
    const abort = new AbortController(); this.#teachingAbort = abort;
    this.#teachingPending = this.#proposeTeaching({ base, proposalId, instruction, expectedPolicyVersion, abort });
  }

  async #proposeTeaching({ base, proposalId, instruction, expectedPolicyVersion, abort }) {
    const current = () => !this.#closed && !abort.signal.aborted && this.#teaching?.proposalId === proposalId && this.#teaching.status === "planning";
    try {
      this.#store.recordEpisode({ episodeId: `${base}:call`, kind: "model-call", action: "responses", outcome: "attempted", summary: "已向配置模型请求一次教学提案；等待主人确认，不自动重试。", context: { sourceEventRef: this.#teachingContext.instructionId } });
      this.#calls += 1; this.#onUsage({ operationId: `${base}:call`, model: this.#configuration.model });
      this.#teaching.lastCall = { model: this.#configuration.model, status: "in-flight" };
      const captured = this.#teachingContext;
      const response = await this.#model.plan({ instructions: TEACHING_INSTRUCTIONS, context: { instruction, ...captured.context }, schema: TEACHING_SCHEMA, signal: abort.signal });
      if (!current()) return;
      const plan = validateTeachingPlan(response.plan);
      if (!this.#teachingSourcesValid(captured, expectedPolicyVersion, plan.rememberedEpisodeIds)) throw new ModelCallError("stale-plan");
      if (!plan.supported) {
        this.#teaching = { ...this.#teaching, status: "unsupported", reply: plan.reply, lastCall: response.call };
        this.#teachingContext = null; return;
      }
      const metadata = { changes: plan.changes, expectedPolicyVersion, instructionId: captured.instructionId };
      this.#store.recordEpisode({ episodeId: proposalId, kind: "model-teaching-proposal", action: "propose-teaching", outcome: "proposed",
        summary: JSON.stringify(metadata), context: { sourceEventRef: captured.instructionId } });
      captured.referenceIds = plan.rememberedEpisodeIds;
      this.#teaching = { ...this.#teaching, status: "proposed", reply: plan.reply, changes: plan.changes,
        references: plan.rememberedEpisodeIds.map(episodeId => ({ episodeId, summary: captured.context.memories.find(e => e.episodeId === episodeId).summary })), lastCall: response.call };
    } catch (error) {
      if (!current()) return;
      const code = error instanceof ModelCallError ? error.code : "unavailable";
      this.#teaching = { ...this.#teaching, status: "failed", changes: {}, reply: ERRORS[code] ?? "教学提案暂不可用；习惯没有改变。", lastCall: { model: this.#configuration.model, status: code } };
      this.#teachingContext = null;
    }
  }

  #teachingSourcesValid(captured, version, ids) {
    if (!captured) return false;
    try {
      const state = this.#client.snapshot(), episodes = new Map(this.#store.growth().episodes.map(e => [e.episodeId, e])), elapsed = this.#clock() - captured.startedAt;
      return state.ready && signature(state) === captured.world && Number.isFinite(elapsed) && elapsed >= 0 && elapsed <= 5 * 60_000 &&
        this.#store.profile().policyVersion === version && captured.inputStamps.every(stamp => episodes.get(stamp.episodeId)?.status === "active" && episodes.get(stamp.episodeId).payloadHash === stamp.payloadHash && payloadHash(episodes.get(stamp.episodeId).payload) === stamp.payloadHash) &&
        validateMemoryReferences(ids, captured.context, this.#store);
    } catch { return false; }
  }

  confirmTeaching({ requestId, proposalId, expectedPolicyVersion, auth = {} }) {
    const prior = this.#store.growth().feedback.find(e => e.feedbackId === requestId);
    if (prior) {
      if (prior.payload?.proposalId !== proposalId && prior.status !== "deleted") throw new Error("Teaching confirmation ID conflicts.");
      if (prior.status === "deleted") return;
      this.#store.teachPreferences({ ...auth, ...prior.payload, operationId: requestId, feedbackId: requestId, expectedPolicyVersion }); return;
    }
    const captured = this.#teachingContext, proposal = this.#teaching;
    if (!proposal || proposal.status !== "proposed" || proposal.proposalId !== proposalId || expectedPolicyVersion !== proposal.sourceVersion ||
        !this.#teachingSourcesValid(captured, expectedPolicyVersion, captured?.referenceIds ?? [])) throw new ModelCallError("stale-plan");
    this.#store.teachPreferences({ source: auth.source ?? "local-owner", ...auth, operationId: requestId, feedbackId: requestId, expectedPolicyVersion,
      feedbackText: [...captured.instruction].slice(0, 500).join(""), context: { mode: "party", sourceEventRef: captured.instructionId }, partyPatch: proposal.changes,
      sourceEpisodeIds: [...captured.inputStamps.map(e => e.episodeId), ...captured.referenceIds, proposalId].filter((id, index, ids) => ids.indexOf(id) === index), proposalId });
    this.pause("已确认新的培养习惯；旧行动委托已暂停，请重新委托。");
    this.#teaching = { ...proposal, status: "confirmed", reply: "教学已确认并保存。", references: [] }; this.#teachingContext = null;
  }

  cancelTeaching({ proposalId }) {
    if (this.#teaching?.proposalId !== proposalId || !["planning", "proposed", "cancelled"].includes(this.#teaching.status)) throw new Error("Teaching proposal is unavailable.");
    if (this.#teaching.status !== "cancelled") {
      this.#store.recordEpisode({ episodeId: `${proposalId}:cancel`, kind: "teaching-cancel", action: "cancel-teaching", outcome: "accepted", summary: "主人取消了待确认教学；习惯没有改变。", context: { sourceEventRef: this.#teachingContext.instructionId } });
      this.#invalidateTeaching("已取消教学，习惯没有改变。", "cancelled");
    }
  }

  #invalidateTeaching(reason, status = "invalidated") {
    this.#teachingAbort?.abort(); this.#teachingAbort = null; this.#teachingContext = null;
    if (this.#teaching && ["planning", "proposed"].includes(this.#teaching.status)) this.#teaching = { ...this.#teaching, status, reply: reason, changes: {}, references: [] };
  }

  pause(reason = "委托已暂停，请重新委托。") {
    this.#abort?.abort(); this.#abort = null; this.#active = null;
    this.#lifeGeneration++; this.#lifeAbort?.abort(); this.#lifeAbort = null;
    if (this.#lifePlan) this.#lifePlan = { ...this.#lifePlan, status: "paused", reply: reason, references: [],
      ...(this.#lifePlan.lastCall?.status === "in-flight" ? { lastCall: { ...this.#lifePlan.lastCall, status: "cancelled" } } : {}) };
    this.#invalidateTeaching(reason);
    if (this.#task && ["planning", "running"].includes(this.#task.status)) this.#task = { ...this.#task, status: "paused", step: "paused", reply: reason,
      ...(this.#task.lastCall?.status === "in-flight" ? { lastCall: { ...this.#task.lastCall, status: "cancelled" } } : {}) };
  }

  invalidateMemory(reason, { clear = false } = {}) {
    this.pause(reason);
    this.#invalidateTeaching(reason);
    if (this.#teaching?.status === "confirmed") this.#teaching = { ...this.#teaching, status: "invalidated", reply: reason, changes: {}, references: [] };
    if (clear) { this.#task = null; this.#teaching = null; this.#lifePlan = null; }
    else if (this.#task) this.#task = { ...this.#task, references: [], reply: reason, instruction: "" };
  }

  #valid(state) {
    const active = this.#active;
    if (!active) return false;
    if (!state.ready || state.player?.dead === true || state.playerHp === 0 || signature(state) !== active.world || this.#store.profile().policyVersion !== active.policyVersion ||
      !validateMemoryReferences(active.references, active.context, this.#store) || this.#clock() - active.startedAt > 10 * 60_000) { this.pause("世界、记忆或培养版本已变化，或委托已超时。请重新委托。"); return false; }
    return true;
  }

  decide({ state, baseAction }) {
    if (state.player?.dead === true || state.playerHp === 0) this.pause("角色已死亡，委托中断。复活后请重新委托。");
    if (this.#active && !this.#valid(state)) return baseAction;
    if (baseAction.kind === "attack" && baseAction.reason === "defend-confirmed-engagement") return baseAction;
    // Safety, unknown state, approved invitations and owner recall always win.
    if (!["wait", "follow", "loot", "attack", "requestApproval"].includes(baseAction.kind) || baseAction.kind === "wait" && !SAFE_BASE_WAITS.has(baseAction.reason)) return baseAction;
    if (this.#task?.status === "planning") return { kind: "wait", reason: "model-planning", policyVersion: baseAction.policyVersion };
    if (!this.#valid(state)) return baseAction;
    const active = this.#active, task = this.#task, plan = active.plan;
    const result = (kind, reason, fields = {}) => ({ kind, reason, policyVersion: baseAction.policyVersion, ...fields });
    const members = state.party?.members ?? [];
    const owner = members.includes(state.player?.name) && members.includes(this.#controller.leaderName) ? state.entities.find(e => e.kind === "player" && e.name === this.#controller.leaderName && point(e) && e.dead !== true) : null;
    const followDistance = Math.min(2, plan.maxDistanceFromOwner);
    const returning = () => {
      if (!owner) return result("wait", "model-owner-not-visible");
      task.step = "returning-to-owner";
      if (distance(state.player, owner) <= followDistance) { this.#complete(`服务器观察：已回到同组主人的 ${followDistance} 格范围内。`, state); return result("wait", "model-objective-completed"); }
      return result("follow", "model-return-to-owner", { target: { x: owner.x, y: owner.y }, desiredDistance: followDistance });
    };
    if (plan.kind === "return_home") {
      const home = this.#controller.homePosition;
      if (!point(home) || home.mapFileName && home.mapFileName !== state.mapFileName) return result("wait", "model-home-not-authorized");
      task.step = "returning-home";
      if (distance(state.player, home) === 0) { this.#complete("服务器观察：已抵达授权的比奇安全点坐标。", state); return result("wait", "model-objective-completed"); }
      return result("retreat", "model-return-home", { target: { x: home.x, y: home.y }, desiredDistance: 0 });
    }
    if (!owner) return result("wait", "model-owner-not-visible");
    if (plan.kind === "hunt" && task.completedKills >= plan.count) {
      active.targetId = null;
      if (plan.returnToOwner) return returning();
      this.#complete("服务器观察：已达到委托的参战击杀数量。", state); return result("wait", "model-objective-completed");
    }
    if (plan.kind === "wait") { task.step = "waiting"; return result("wait", "model-owner-requested-wait"); }
    // Waiting retains genuine self-defence above, but does not initiate party assistance.
    if (baseAction.kind === "attack" && baseAction.reason !== "model-hunt-observed-monster") return baseAction;
    if (plan.kind === "follow") {
      task.step = "following";
      if (baseAction.kind === "loot" && plan.loot === "after_combat") return baseAction;
      return distance(state.player, owner) > followDistance ? result("follow", "model-follow-owner", { target: { x: owner.x, y: owner.y }, desiredDistance: followDistance }) : result("wait", "model-near-owner");
    }
    if (distance(state.player, owner) > plan.maxDistanceFromOwner) return result("follow", "model-stay-inside-owner-leash", { target: { x: owner.x, y: owner.y }, desiredDistance: followDistance });
    const targets = state.entities.filter(e => liveMonster(e) && !active.counted.has(e.objectId) &&
      (plan.monsterName === null || e.name === plan.monsterName) && distance(owner, e) <= plan.maxDistanceFromOwner && distance(state.player, e) <= 6)
      .sort((a, b) => Number(b.objectId === active.targetId) - Number(a.objectId === active.targetId) || distance(state.player, a) - distance(state.player, b) || a.objectId - b.objectId);
    const target = targets[0];
    if (target) { active.targetId = target.objectId; task.step = distance(state.player, target) <= 1 ? "fighting" : "approaching-monster";
      return result("attack", "model-hunt-observed-monster", { targetId: target.objectId, target: { x: target.x, y: target.y }, desiredDistance: 1 }); }
    active.targetId = null; task.step = "seeking-monster";
    if (baseAction.kind === "loot" && plan.loot === "after_combat") return baseAction;
    return result("wait", "model-no-allowed-monster-visible");
  }

  movementConstraint(state, action) {
    if (!this.#active || !this.#valid(state) || this.#active.plan.kind === "return_home" || !["attack", "follow", "loot"].includes(action.kind)) return null;
    const names = state.party?.members ?? [];
    const owner = names.includes(state.player?.name) && names.includes(this.#controller.leaderName) ? state.entities.find(e => e.kind === "player" && e.name === this.#controller.leaderName && point(e) && e.dead !== true) : null;
    if (!owner) return null; // Only existing ordinary self-defence can move without an owner.
    const radius = this.#active.plan.maxDistanceFromOwner, currentDistance = distance(state.player, owner);
    // If the owner walked away, return without increasing the existing gap;
    // a lateral step may be needed to go around an observed occupant.
    const cap = Math.max(radius, currentDistance);
    return { bounds: { minX: Math.max(0, owner.x - cap), minY: Math.max(0, owner.y - cap), maxX: owner.x + cap, maxY: owner.y + cap },
      permits: next => distance(next, owner) <= cap };
  }

  #observe(event) {
    if (this.#closed || !this.#active || this.#task?.status !== "running") return;
    const state = this.#client.snapshot();
    if (!state.ready || !this.#valid(state)) return;
    const active = this.#active, payload = event?.payload;
    if (event?.packet === "ObjectStruck" && objectId(payload?.attackerId) && [state.playerObjectId, state.selfZoneObjectId].filter(objectId).includes(payload.attackerId) && payload.objectId === active.targetId && state.entities.some(e => e.objectId === payload.objectId && liveMonster(e))) {
      active.hits.set(payload.objectId, this.#clock());
    } else if (event?.packet === "ObjectDied" && payload?.objectId === active.targetId && active.hits.has(payload.objectId) && !active.counted.has(payload.objectId) && this.#clock() - active.hits.get(payload.objectId) <= 30_000) {
      const next = this.#task.completedKills + 1;
      // Store first; a failed write cannot advance visible progress.
      try {
        this.#store.recordEpisode({ episodeId: `${active.goalId}:kill:${next}`, kind: "objective-progress", action: "hunt", outcome: "observed",
          summary: `服务器观察：阿洛实际命中目标后收到了其死亡事件；本委托参战击杀进度 ${next}/${active.plan.count}。`, objectRef: `monster:${payload.objectId}`, locationRef: `map:${state.mapFileName}`, context: { sourceEventRef: active.goalId } });
        active.counted.add(payload.objectId); active.hits.delete(payload.objectId); this.#task.completedKills = next;
      } catch { this.pause("记录实际进度失败，已暂停委托。"); }
    }
  }

  #complete(summary, state) {
    const active = this.#active;
    try {
      this.#store.recordEpisode({ episodeId: `${active.goalId}:completed`, kind: "objective-progress", action: active.plan.kind.replaceAll("_", "-"), outcome: "observed", summary, locationRef: `map:${state.mapFileName}`, context: { sourceEventRef: active.goalId } });
      this.#task.status = "completed"; this.#task.step = "completed"; this.#active = null;
    } catch { this.pause("记录实际完成结果失败，已暂停委托。"); }
  }

  async close() {
    if (this.#closed) return;
    this.#closed = true; this.pause(); this.#invalidateTeaching("运行器已关闭。"); this.#client.off("frame", this.#listener);
    if (this.#controller.modelDriver === this) this.#controller.modelDriver = null;
    this.#model?.close?.();
    // Abort is synchronous; late provider results are fenced by #closed before any store access.
    this.#pending?.catch(() => {});
    this.#teachingPending?.catch(() => {});
  }
}
