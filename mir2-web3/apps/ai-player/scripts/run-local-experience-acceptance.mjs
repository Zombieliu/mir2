import assert from "node:assert/strict";
import { createHash, randomUUID } from "node:crypto";
import { access, readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { setTimeout as sleep } from "node:timers/promises";
import { facingDirection } from "../src/companion-policy.mjs";
import { stringifyGatewayJson } from "../src/lossless-json.mjs";
import { verifyArchive } from "../src/memory/archive-codec.mjs";
import { loadClientCollision, nextWalkDirection } from "../src/skills/navigation.mjs";
import { createTrialFixture, launchTrialActor } from "./companion-trial-fixture.mjs";
import { gatewayExecutableArgument, repoRoot, sha256 } from "./local-gateway.mjs";

const point = value => ({ x: value.x, y: value.y });
const distance = (a, b) => Math.max(Math.abs(a.x - b.x), Math.abs(a.y - b.y));
const brief = state => ({ ready: state.ready, player: state.player && point(state.player), hp: state.playerHp,
  selfZoneObjectId: state.selfZoneObjectId, party: state.party?.members, groundDropCount: state.groundDrops?.length });

async function acceptance() {
  const startedAt = new Date().toISOString(), checks = [], commands = {}, trace = [];
  let fixture, owner, stranger, ordinaryActor, companion, probe, phase = "ordinary fixture preparation";
  const note = value => { trace.push({ at: Date.now(), ...value }); if (trace.length > 4000) trace.shift(); };
  const recordCommand = type => { commands[type] = (commands[type] ?? 0) + 1; };
  const actorEvents = message => {
    if (message?.type === "command") recordCommand(message.command);
    else if (message?.type === "fixtureStage") console.log(JSON.stringify({ phase: message.stage, ordinal: message.ordinal }));
    else if (["proof", "report", "fixtureFailure"].includes(message?.type)) note({ source: "companion", ...message });
  };
  const checkpoint = label => console.log(JSON.stringify({ phase: label }));
  async function poll(read, predicate, label, timeoutMs = 25_000) {
    const deadline = Date.now() + timeoutMs;
    while (Date.now() < deadline) { const value = await read(); if (predicate(value)) return value; await sleep(150); }
    throw new Error(`The ${label} acceptance condition did not complete.`);
  }
  try {
    fixture = await createTrialFixture({ gatewayExecutable: gatewayExecutableArgument(process.argv.slice(2)), onEvent: actorEvents });
    owner = await fixture.newOwnerClient(); stranger = await fixture.newStrangerClient();
    for (const [source, client] of [["owner", owner], ["stranger", stranger]]) {
      client.on("command", ({ type }) => recordCommand(type));
      client.on("frame", ({ event }) => {
        assert.ok(!["companionRequest", "companionResult"].includes(event.type), "Private controls must not enter ordinary frame listeners.");
        if (["ObjectStruck", "ObjectDied", "ObjectWalk", "ObjectItem"].includes(event.packet)) note({ source, event });
      });
    }
    assert.equal(owner.features.companionControl, true);
    assert.equal(owner.worldEpoch, fixture.profileA.companion.worldEpoch);
    checks.push("three ordinary Warriors were created, entered, saved, then reused under an exact trusted owner/actor policy");
    const residentId = fixture.profileA.companion.residentId;
    const control = (operation, payload = {}, requestId = randomUUID()) => owner.companionControl(operation, { residentId, payload, requestId });
    const okay = async (operation, payload = {}, requestId) => {
      const result = await control(operation, payload, requestId); assert.equal(result.status, "ok", `Owner ${operation} must commit successfully.`); return result.result;
    };

    phase = "exact authenticated owner and inviter authorization";
    const hidden = await stranger.companionControl("list");
    assert.equal(hidden.status, "ok"); assert.deepEqual(hidden.result.companions, []);
    const denied = await stranger.companionControl("view", { residentId });
    assert.equal(denied.status, "denied");
    assert.ok(!denied.result?.archive && !denied.result?.profile && !denied.result?.feedback);
    const deniedRuntime = await stranger.registerCompanionRuntime({ residentId, archiveId: fixture.profileA.companion.archiveId });
    assert.equal(deniedRuntime.status, "denied");
    phase = "foreign invitation rejected before pending state";
    ordinaryActor = await fixture.newActorClient();
    ordinaryActor.on("command", ({ type }) => recordCommand(type));
    ordinaryActor.on("frame", ({ event }) => {
      if (["GroupInvite", "AddMember", "DeleteMember", "DeleteGroup", "GroupMemberInfo"].includes(event.packet)) note({ source: "ordinary-actor", event });
    });
    await ordinaryActor.setAllowGroup(true); await stranger.setAllowGroup(true);
    const localization = JSON.parse(await readFile(resolve(repoRoot, "packages/game-data/data/generated/localization_bundle.json"), "utf8"));
    const rejectionMessages = new Set(Object.values(localization.languages).map(language => language.texts?.["server.NotAllowGroup"])
      .filter(value => typeof value === "string").map(value => value.replaceAll("{0}", fixture.actor.characterName)));
    assert.ok(rejectionMessages.size > 0, "Canonical server rejection templates must be available.");
    const afterForeignInvite = stranger.sequence;
    await stranger.inviteMember(fixture.actor.characterName);
    const refusal = await stranger.waitFor(event => event.packet === "Chat" && rejectionMessages.has(event.payload?.message), { after: afterForeignInvite });
    note({ source: "stranger", event: refusal });
    assert.ok(rejectionMessages.has(refusal.payload.message), "The stranger must receive a real server refusal, not a local missing-pending error.");
    // R16 invitations have no TTL. The server must reject before creating a
    // foreign pending invite, rather than leaving the real owner's slot stuck.
    assert.equal(ordinaryActor.snapshot().party.pendingInvite, null);
    assert.equal(ordinaryActor.snapshot().party.members.length, 0);
    assert.equal(stranger.snapshot().party.members.length, 0);
    phase = "real owner invitation immediately after foreign refusal";
    await owner.setAllowGroup(true); await owner.inviteMember(fixture.actor.characterName);
    await poll(() => Promise.resolve(ordinaryActor.snapshot()), state => state.party.pendingInvite?.name === fixture.owner.characterName, "real owner invitation immediately after foreign refusal");
    await ordinaryActor.acceptGroupInvite();
    await poll(() => Promise.resolve(owner.snapshot()), state => state.party.members.includes(fixture.actor.characterName), "real owner roster after foreign refusal");
    assert.equal(ordinaryActor.snapshot().party.members.length, 2);
    phase = "temporary real group cleanup by the member permission switch";
    assert.equal(owner.snapshot().party.leaderName, fixture.owner.characterName);
    // R16 DelMember is leader-only. A member leaves through the ordinary
    // SwitchGroup(false) path and the client restores its previous preference.
    await ordinaryActor.leaveGroup();
    await poll(() => Promise.resolve(owner.snapshot()), state => state.party.members.length === 0, "ordinary temporary group cleanup");
    await ordinaryActor.logout();
    await ordinaryActor.close();
    checks.push("stranger sees no enrollment or memory and cannot register; its real invitation receives a server refusal without pending/group state, then the true owner immediately invites and forms a real group");
    checkpoint("owner-and-real-inviter-authorization-passed");
    checkpoint("waiting-31s-for-existing-route-lease"); await sleep(31_000);

    phase = "enrolled runtime registration and private owner view";
    companion = await fixture.startActor(fixture.profileA);
    const initial = await okay("view"); assert.equal(initial.profile.policyVersion, 1);
    assert.equal(initial.archive.archiveId, fixture.profileA.companion.archiveId);
    const enrolled = await owner.companionControl("list");
    assert.equal(enrolled.result.companions.length, 1); assert.equal(enrolled.result.companions[0].online, true);
    assert.equal(initial.runtime.runtimeId, "runner-A");
    checks.push("the dedicated actor registers over real WS and only its exact authenticated owner receives the private archive view");

    phase = "real owner trial events remain self-reports";
    const visitId = randomUUID();
    const visit = await okay("trialEvent", { kind: "visit", visitId, entrySource: "unknown", reminderCount: 0 });
    assert.equal(visit.trial.environment, "test"); assert.equal(visit.trial.visitCount, 1);
    await okay("trialEvent", { kind: "choice", visitId, reason: "shared-history", reasonText: "Synthetic acceptance: preserve the same companion history." });
    await okay("trialEvent", { kind: "teachingCheck", visitId, understood: true, reasonText: "Synthetic acceptance: support before loot." });
    const notBuying = await control("trialEvent", { kind: "purchaseIntent", visitId, offerId: "disabled-test-offer" });
    assert.equal(notBuying.status, "error");
    checks.push("authenticated owner visit/choice/teaching events are explicitly synthetic self-reports; a disabled offer rejects purchase intent");

    phase = "teaching versioning and duplicate operation";
    const teachId = randomUUID();
    const taught = await okay("teach", { expectedPolicyVersion: 1 }, teachId);
    assert.equal(taught.profile.policyVersion, 2); assert.equal(taught.profile.party.assistBeforeLoot, true);
    const feedbackId = taught.feedback.find(item => item.status === "active").feedbackId;
    const duplicate = await okay("teach", { expectedPolicyVersion: 1 }, teachId);
    assert.equal(duplicate.profile.policyVersion, 2); assert.equal(duplicate.growthRevision, taught.growthRevision);
    const conflict = await control("teach", { expectedPolicyVersion: 1 });
    assert.equal(conflict.status, "conflict"); assert.equal(conflict.result.profile.policyVersion, 2);
    checks.push("WS teaching commits policy 1→2; a stable request retry is idempotent, and stale expected versions conflict");

    phase = "revocation/deletion invalidate cached export bytes";
    const beforeRevoke = await okay("exportBegin");
    const revoked = await okay("revokeFeedback", { feedbackId, expectedPolicyVersion: 2 });
    assert.equal(revoked.profile.policyVersion, 3); assert.equal(revoked.profile.party.assistBeforeLoot, false);
    assert.equal((await control("exportChunk", { exportId: beforeRevoke.exportId, index: 0 })).status, "error");
    const beforeDelete = await okay("exportBegin");
    const deleted = await okay("deleteFeedback", { feedbackId, expectedPolicyVersion: 3 });
    assert.equal(deleted.profile.policyVersion, 3);
    const tombstone = deleted.feedback.find(item => item.feedbackId === feedbackId);
    assert.equal(tombstone.status, "deleted"); assert.equal(tombstone.summary, undefined);
    assert.equal((await control("exportChunk", { exportId: beforeDelete.exportId, index: 0 })).status, "error");
    const replayDeleted = await okay("teach", { expectedPolicyVersion: 1 }, teachId);
    assert.equal(replayDeleted.profile.policyVersion, 3); assert.equal(replayDeleted.profile.party.assistBeforeLoot, false);
    const retaught = await okay("teach", { expectedPolicyVersion: 3 });
    assert.equal(retaught.profile.policyVersion, 4);
    assert.equal(retaught.profile.party.lootOnlyAfterCombat, true);
    checks.push("revoke changes policy 2→3, deletion erases the teaching text and both cached exports, replay cannot revive it, new teaching produces policy 4");
    checkpoint("private-memory-controls-passed");

    phase = "recall and resume through authenticated owner control";
    const recalled = await okay("recall"); assert.equal(recalled.runtime.recallRequested, true);
    const recalledAction = await companion.request("tick");
    assert.equal(recalledAction.kind, "retreat"); assert.equal(recalledAction.reason, "owner-recall");
    assert.equal((await okay("resume")).runtime.recallRequested, false);
    checks.push("owner recall reaches the live controller through private WS; resume clears it without a terminal-side teaching shortcut");

    phase = "full signed portable archive over bounded WS chunks";
    const exported = await okay("exportBegin"), buffers = [];
    assert.ok(exported.totalBytes > 0 && exported.totalBytes <= 2 * 1024 * 1024);
    assert.equal(exported.chunkCount, Math.ceil(exported.totalBytes / 16384));
    for (let index = 0; index < exported.chunkCount; index += 1) {
      const chunk = await okay("exportChunk", { exportId: exported.exportId, index });
      assert.equal(chunk.index, index); assert.equal(chunk.chunkCount, exported.chunkCount);
      const buffer = Buffer.from(chunk.dataBase64, "base64"); assert.ok(buffer.length <= 16384); buffers.push(buffer);
    }
    const bytes = Buffer.concat(buffers);
    assert.equal(bytes.length, exported.totalBytes); assert.equal(createHash("sha256").update(bytes).digest("hex"), exported.sha256);
    const portable = verifyArchive(bytes.toString("utf8"), Buffer.from(fixture.archiveKey, "hex"));
    assert.equal(portable.playerGrowth.policyVersion, 4);
    assert.equal(portable.characterArchive.archiveId, fixture.profileA.companion.archiveId);
    assert.ok(portable.playerGrowth.episodes.length > 0);
    assert.equal(portable.playerGrowth.feedback.find(row => row.feedbackId === feedbackId).payload, null);
    assert.equal(portable.runtimeService, undefined);
    for (const field of ["inventory", "equipment", "gold", "hp", "password", "archiveKey"]) assert.equal(portable[field], undefined);
    const exportPath = resolve(fixture.output, "owner-export.json");
    await writeFile(exportPath, bytes, { flag: "wx", mode: 0o600 });
    checks.push("the owner downloads the complete immutable HMAC archive in bounded chunks; byte/hash/signature and identity/growth validate, with no runtime settings or game assets");

    phase = "stable live archive guard before login and import";
    let probeCommands = 0;
    probe = launchTrialActor({ profile: fixture.profileB, identity: fixture.actor, archiveKey: fixture.archiveKey,
      onEvent: event => { if (event.type === "command") probeCommands += 1; } });
    await assert.rejects(probe.started); await probe.close(); probe = null;
    assert.equal(probeCommands, 0, "Changing runner/port/path must still fail before any login frame.");
    const blockedImport = await fixture.runCli("memory-cli.mjs", ["import", "--profile", fixture.profileBPath, "--input", exportPath]);
    assert.notEqual(blockedImport.code, 0);
    await assert.rejects(() => access(fixture.profileB.companion.memoryPath));
    checks.push("live A blocks a different B path/port/process before any game command and blocks actual CLI import before the destination database is created");

    phase = "A stops, actual CLI imports a fresh B database";
    const savedA = await companion.request("snapshot"), pidA = companion.pid;
    await fixture.stopActor(); companion = null;
    const imported = await fixture.runCli("memory-cli.mjs", ["import", "--profile", fixture.profileBPath, "--input", exportPath]);
    assert.equal(imported.code, 0, "Stopped A must permit the real import CLI.");
    const importedSummary = JSON.parse(imported.stdout.trim()); assert.equal(importedSummary.policyVersion, 4);
    assert.equal(importedSummary.growthRevision, portable.playerGrowth.growthRevision);
    const importRetry = await fixture.runCli("memory-cli.mjs", ["import", "--profile", fixture.profileBPath, "--input", exportPath]);
    assert.equal(importRetry.code, 0);
    checkpoint("waiting-31s-for-A-route-lease"); await sleep(31_000);
    companion = await fixture.startActor(fixture.profileB);
    assert.notEqual(companion.pid, pidA);
    const currentB = await okay("view");
    assert.equal(currentB.runtime.runtimeId, "runner-B");
    assert.deepEqual(currentB.archive, portable.characterArchive);
    assert.equal(currentB.profile.policyVersion, 4); assert.equal(currentB.growthRevision, portable.playerGrowth.growthRevision);
    assert.equal(currentB.profile.party.assistBeforeLoot, true);
    assert.deepEqual(point((await companion.request("snapshot")).player), point(savedA.player));
    checks.push("after A stops, the real CLI imports fresh B idempotently; a different Node process keeps exact identity, episodes, growth, policy 4 and separately saved server position");
    checkpoint("import-and-restart-passed");

    phase = "ordinary owner group admission after import";
    await owner.setAllowGroup(true); await owner.inviteMember(fixture.actor.characterName);
    await poll(() => companion.request("snapshot"), state => state.party.pendingInvite?.name === fixture.owner.characterName, "owner invitation");
    assert.equal((await companion.request("tick")).kind, "acceptInvite");
    await poll(() => companion.request("snapshot"), state => state.party.members.length === 2, "real shared roster");
    await poll(() => Promise.resolve(owner.snapshot()), state => state.party.members.includes(fixture.actor.characterName), "owner shared roster");
    await owner.setPeaceAttackMode();
    for (const [name, slot] of [["WoodenSword", 0], ["BaseDress(M)", 1]]) {
      const item = owner.snapshot().inventoryItems.find(value => value.name === name);
      if (item) await owner.equipOwnedItem(item.uniqueId, slot);
    }
    checkpoint("waiting-16s-for-existing-bootstrap-grace"); await sleep(16_000);
    const collision = await loadClientCollision("0");
    async function walkTo(target, desiredDistance = 0, { tick = false, maxSteps = 100 } = {}) {
      for (let index = 0; index < maxSteps; index += 1) {
        const state = owner.snapshot(); if (distance(state.player, target) <= desiredDistance) return;
        const occupied = state.entities.filter(entity => entity.objectId !== state.playerObjectId && !entity.dead).map(point);
        const direction = nextWalkDirection({ player: state.player, target, desiredDistance, collision, occupied });
        assert.ok(direction, "Ordinary path must exist to the observed target.");
        await owner.walk(direction); if (tick) await companion.request("tick"); await sleep(650);
      }
      throw new Error("Ordinary walk exceeded its bounded step budget.");
    }
    async function follow() {
      for (let index = 0; index < 35; index += 1) {
        if (distance((await companion.request("snapshot")).player, owner.snapshot().player) <= 2) return;
        await companion.request("tick"); await sleep(650);
      }
      throw new Error("The imported companion did not follow its visible owner.");
    }
    let monster = owner.snapshot().entities.filter(entity => entity.kind === "monster" && entity.name === "Scarecrow" && !entity.dead)
      .sort((a, b) => distance(owner.snapshot().player, a) - distance(owner.snapshot().player, b))[0];
    if (!monster) { await walkTo({ x: 300, y: 610 }, 3, { tick: true }); monster = owner.snapshot().entities.find(entity => entity.kind === "monster" && entity.name === "Scarecrow" && !entity.dead); }
    assert.ok(monster, "Use an existing ordinary Scarecrow; no spawned fixture entities.");
    await walkTo(monster, 4, { tick: true }); await follow();
    await companion.request("dropCandle"); await sleep(650);
    await walkTo(monster, 1);
    let engaged = false;
    for (let attempt = 0; attempt < 14; attempt += 1) {
      const current = owner.snapshot().entities.find(entity => entity.objectId === monster.objectId && !entity.dead);
      assert.ok(current, "The ordinary selected monster must remain alive.");
      if (distance(owner.snapshot().player, current) > 1) await walkTo(current, 1);
      await owner.attackDirection(facingDirection(owner.snapshot().player, current)); await sleep(650);
      const state = await companion.request("snapshot");
      if (state.engagements.some(event => event.targetId === monster.objectId || event.attackerId === monster.objectId)) { engaged = true; break; }
    }
    assert.ok(engaged, "Real ObjectStruck must establish the owner's engagement.");
    await poll(() => companion.request("snapshot"), state => state.groundDrops.some(drop => drop.name === "Candle"), "ordinary competing candle drop");
    let decision = await companion.request("tick");
    assert.equal(decision.kind, "attack"); assert.equal(decision.policyVersion, 4); assert.equal(decision.performed, true);
    for (let step = 0; decision.confirmed !== false && step < 14; step += 1) {
      await sleep(650); decision = await companion.request("tick"); assert.equal(decision.kind, "attack"); assert.equal(decision.policyVersion, 4);
    }
    assert.equal(decision.confirmed, false, "The imported companion must issue its ordinary attack after closing distance.");
    let actualStrike = false;
    for (let step = 0; step < 30; step += 1) {
      const state = await companion.request("snapshot");
      actualStrike = trace.some(entry => entry.event?.packet === "ObjectStruck" && entry.event.payload.attackerId === state.selfZoneObjectId && entry.event.payload.objectId === monster.objectId);
      if (actualStrike) break;
      await sleep(650); await companion.request("tick");
    }
    assert.ok(actualStrike, "A real observer packet must confirm the imported companion's damage, rather than only its policy decision.");
    checks.push("B accepts only its real owner, follows over ordinary movement, and on a new natural monster/drop encounter policy 4 selects support and produces observed monster damage before loot");
    checkpoint("post-import-real-adventure-passed");

    phase = "trial view and CSV retain unknown costs and no payments";
    const trial = (await okay("view")).trial;
    assert.equal(trial.environment, "test"); assert.equal(trial.offer, null); assert.deepEqual(trial.purchases, []);
    assert.equal(trial.visits[0].source, "participant-self-report"); assert.equal(trial.visits[0].entrySource, "unknown");
    assert.ok(trial.usage.confirmedAdventureCount >= 1); assert.equal(trial.usage.paidAdventureCount, 0); assert.equal(trial.usage.modelCalls, 0);
    assert.equal(trial.costs.modelMicros, "0"); assert.equal(trial.costs.hostingMicros, null);
    assert.equal(trial.costs.paymentMicros, null); assert.equal(trial.costs.revenueShareMicros, null); assert.equal(trial.costs.contributionMicros, null);
    assert.equal(trial.costs.complete, false);
    const ended = await okay("trialEvent", { kind: "visitEnd", visitId }); assert.equal(ended.trial.activeVisitId, null);
    const csvPath = resolve(fixture.output, "participants.csv");
    const csvCommand = await fixture.runCli("experience-cli.mjs", ["report", "--profile", fixture.profileBPath, "--output", csvPath]);
    assert.equal(csvCommand.code, 0);
    const csv = await readFile(csvPath, "utf8");
    assert.ok(csv.includes("synthetic-P01") && csv.includes("participant-self-report") && csv.includes('""hostingMicros"":null') && csv.includes('""contributionMicros"":null'));
    const ordinary = stringifyGatewayJson({ owner: owner.snapshot(), stranger: stranger.snapshot(), actor: await companion.request("snapshot") });
    assert.ok(!ordinary.includes(fixture.profileA.companion.archiveId) && !ordinary.includes(feedbackId) && !ordinary.includes(fixture.profileA.companion.ownerRef));
    checks.push("the shared test ledger survives A/B, closes the explicit visit, counts observed unpaid activity, exports CSV with unknown costs, and keeps private memories out of ordinary observations");

    await fixture.stopActor(); companion = null;
    const report = { schema: "mir2-companion-experience-acceptance/1", ok: true, startedAt, finishedAt: new Date().toISOString(),
      environment: "test", syntheticParticipants: true, realHumanTrial: false, paymentPerformed: false, modelEnabled: false,
      gatewayBinarySha256: await sha256(fixture.gateway.executable), normalGameClientOnly: true, authenticatedPrivateRelay: true,
      contentProfile: fixture.gateway.contentProfile, observedContentProfile: fixture.gateway.observedContentProfile, gameplayBaseline: fixture.gateway.gameplayBaseline,
      reloginSettleMs: 31_000, bootstrapSettleMs: 16_000, checks, commands,
      proof: { archiveId: portable.characterArchive.archiveId, finalPolicyVersion: 4, importedGrowthRevision: portable.playerGrowth.growthRevision,
        archiveBytes: exported.totalBytes, archiveSha256: exported.sha256, actorProcessChanged: true, observedCompanionStrike: actualStrike },
      limitations: ["synthetic local acceptance, no real human retention or payment", "rules-only service", "single local Gateway and local archive lock", "existing route-lease/bootstrap settle", "Native EXE visual proof is recorded separately"] };
    await writeFile(resolve(fixture.output, "summary.json"), JSON.stringify(report, null, 2) + "\n", { mode: 0o600 });
    await writeFile(resolve(fixture.output, "trace.json"), stringifyGatewayJson(trace) + "\n", { mode: 0o600 });
    console.log(JSON.stringify({ ok: true, checks: checks.length, report: resolve(fixture.output, "summary.json") }));
  } catch (error) {
    let diagnostic;
    try { diagnostic = { owner: owner && brief(owner.snapshot()), ordinaryActor: ordinaryActor && brief(ordinaryActor.snapshot()), actor: companion && brief(await companion.request("snapshot")) }; } catch {}
    if (fixture) {
      await writeFile(resolve(fixture.output, "summary.json"), JSON.stringify({ ok: false, startedAt, phase,
        environment: "test", syntheticParticipants: true, paymentPerformed: false, checks, commands, diagnostic,
        errorType: error?.name, errorCode: error?.code ?? null, errorMessage: String(error?.message ?? "Unknown acceptance failure").slice(0, 1000),
        contentProfile: fixture.gateway.contentProfile, observedContentProfile: fixture.gateway.observedContentProfile ?? null }, null, 2) + "\n", { mode: 0o600 });
      await writeFile(resolve(fixture.output, "trace.json"), stringifyGatewayJson(trace) + "\n", { mode: 0o600 });
      await fixture.gateway.saveDiagnostics();
    }
    console.error(JSON.stringify({ ok: false, phase, report: fixture && resolve(fixture.output, "summary.json") })); process.exitCode = 1;
  } finally { await probe?.close(); await fixture?.close(); }
}

acceptance().catch(() => { console.error("The isolated synthetic experience acceptance could not complete."); process.exitCode = 1; });
