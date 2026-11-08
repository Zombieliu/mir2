import assert from "node:assert/strict";
import { fork } from "node:child_process";
import { randomBytes } from "node:crypto";
import { writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { setTimeout as sleep } from "node:timers/promises";
import { GatewayClient } from "../src/gateway-client.mjs";
import { facingDirection } from "../src/companion-policy.mjs";
import { stringifyGatewayJson } from "../src/lossless-json.mjs";
import { loadClientCollision, nextWalkDirection } from "../src/skills/navigation.mjs";
import { appRoot, repoRoot, freePort, isolatedEnvironment, sha256, startLocalGateway } from "./local-gateway.mjs";

const args = process.argv.slice(2);
let executable;
if (args.length) {
  if (args.length !== 2 || args[0] !== "--gateway-exe") throw new Error("Use --gateway-exe <path>.");
  executable = resolve(args[1]);
}
const distance = (a, b) => Math.max(Math.abs(a.x - b.x), Math.abs(a.y - b.y));
const point = player => ({ x: player.x, y: player.y });
const brief = state => ({ player: state.player, hp: state.playerHp, maxHp: state.playerMaxHp, inSafeZone: state.inSafeZone, party: state.party, inventoryStale: state.inventoryStale,
  nearbyActors: state.entities.filter(entity => entity.kind !== "monster"),
  engagements: state.engagements, groundDrops: state.groundDrops, inventory: state.inventoryItems,
  monsters: state.entities.filter(entity => entity.kind === "monster") });

async function acceptance() {
  const gateway = await startLocalGateway({ executable });
  const startedAt = new Date().toISOString();
  const checks = [], commands = {}, trace = [], processes = [];
  let phase = "ordinary account entry";
  let leader, companion, profile;
  const identities = ["Lead", "Alo"].map(prefix => ({ account: `aic${randomBytes(5).toString("hex")}`,
    password: `Ab${randomBytes(8).toString("hex")}7`, name: `${prefix}${randomBytes(3).toString("hex")}` }));
  const note = (event) => { trace.push({ at: Date.now(), ...event }); if (trace.length > 4_000) trace.shift(); };
  const checkpoint = label => console.log(JSON.stringify({ phase: label }));
  const observeCommand = type => { commands[type] = (commands[type] ?? 0) + 1; };
  const launchCompanion = () => {
    const child = fork(resolve(appRoot, "scripts/companion-fixture.mjs"), [], {
      cwd: repoRoot, windowsHide: true, serialization: "advanced", stdio: ["ignore", "pipe", "pipe", "ipc"],
      env: isolatedEnvironment({ MIR2_AI_PLAYER_ACCOUNT: identities[1].account, MIR2_AI_PLAYER_PASSWORD: identities[1].password }),
    });
    processes.push(child);
    let nextId = 0;
    const pending = new Map();
    child.stdout.on("data", () => {}); child.stderr.on("data", () => {});
    child.on("message", message => {
      if (message.type === "result") {
        const waiter = pending.get(message.id);
        if (!waiter) return;
        clearTimeout(waiter.timer); pending.delete(message.id);
        if (waiter.command === "tick") note({ source: "companion", type: "tickResult", result: message.result, error: message.error });
        if (message.error) waiter.reject(new Error(message.error)); else waiter.resolve(message.result);
      } else if (message.type === "command") observeCommand(message.command);
      else if (["report", "proof"].includes(message.type)) note({ source: "companion", ...message });
    });
    child.on("exit", () => { for (const waiter of pending.values()) { clearTimeout(waiter.timer); waiter.reject(new Error("Companion process exited.")); } pending.clear(); });
    return { pid: child.pid, request(command, fields = {}) {
      return new Promise((resolve, reject) => {
        const id = ++nextId;
        const timer = setTimeout(() => { pending.delete(id); reject(new Error(`Companion ${command} timed out.`)); }, 35_000);
        pending.set(id, { resolve, reject, timer, command });
        child.send({ id, command, ...fields });
      });
    }, child };
  };
  const poll = async (read, predicate, label, timeoutMs = 25_000) => {
    const deadline = Date.now() + timeoutMs;
    while (Date.now() < deadline) {
      const state = await read();
      if (predicate(state)) return state;
      await sleep(150);
    }
    throw new Error(`Timed out: ${label}.`);
  };
  const collision = await loadClientCollision("0");
  const walkTo = async (client, target, desiredDistance = 0, { tickCompanion = false, maxSteps = 75 } = {}) => {
    for (let step = 0; step < maxSteps; step += 1) {
      const state = client.snapshot();
      if (distance(state.player, target) <= desiredDistance) return;
      const occupied = state.entities.filter(entity => entity.objectId !== state.playerObjectId && entity.dead !== true).map(point);
      const direction = nextWalkDirection({ player: state.player, target, desiredDistance, collision, occupied });
      if (!direction) throw new Error("No ordinary client path to the observed target.");
      await client.walk(direction);
      if (tickCompanion) await companion.request("tick");
      await sleep(650);
    }
    throw new Error("Ordinary walk step budget exhausted.");
  };
  const waitCompanionNearLeader = async () => {
    for (let step = 0; step < 35; step += 1) {
      if (distance((await companion.request("snapshot")).player, leader.snapshot().player) <= 2) return;
      await companion.request("tick"); await sleep(650);
    }
    throw new Error("Companion did not follow the approved visible leader.");
  };
  const engage = async (monster) => {
    for (let attempt = 0; attempt < 14; attempt += 1) {
      const state = leader.snapshot();
      const current = state.entities.find(entity => entity.objectId === monster.objectId && !entity.dead);
      if (!current) throw new Error("The selected monster left view or died before engagement.");
      if (distance(state.player, current) > 1) await walkTo(leader, current, 1);
      await leader.attackDirection(facingDirection(leader.snapshot().player, current));
      await sleep(650);
      const observed = await companion.request("snapshot");
      if (observed.engagements.some(event => event.targetId === monster.objectId || event.attackerId === monster.objectId)) return;
    }
    throw new Error("No real ObjectStruck confirmed the party's combat.");
  };
  try {
    leader = new GatewayClient({ url: gateway.url, realmId: gateway.realmId, timeoutMs: 25_000 });
    leader.on("command", ({ type }) => observeCommand(type));
    leader.on("frame", ({ event }) => { if (["ObjectStruck", "ObjectDied", "ObjectItem", "ObjectWalk"].includes(event.packet)) note({ source: "leader", event }); });
    await leader.connect(); await leader.createAccount(identities[0].account, identities[0].password);
    await leader.login(identities[0].account, identities[0].password);
    const character = await leader.createCharacter(identities[0].name);
    await leader.startGame(character.index); await leader.setPeaceAttackMode();
    for (const [name, slot] of [["WoodenSword", 0], ["BaseDress(M)", 1]]) {
      const item = leader.snapshot().inventoryItems.find(item => item.name === name);
      if (item) await leader.equipOwnedItem(item.uniqueId, slot);
    }
    profile = { url: gateway.url, realmId: gateway.realmId, characterName: identities[1].name, timeoutMs: 25_000,
      guardPort: await freePort(), allowedMaps: ["0", "0141"], companion: { residentId: "alo-local-acceptance",
        leaderName: identities[0].name, memoryPath: resolve(gateway.output, "alo.sqlite") } };
    companion = launchCompanion();
    await companion.request("start", { profile, create: true });
    assert.equal(leader.features.sharedParty, true);
    checks.push("two ordinary equipped Warriors entered the same Bichon Zone");

    phase = "real shared invitation and acceptance";
    await leader.setAllowGroup(true);
    await leader.inviteMember(identities[1].name);
    await poll(() => companion.request("snapshot"), state => state.party.pendingInvite?.name === identities[0].name, "real invitation");
    assert.equal((await companion.request("snapshot")).party.members.length, 0);
    const admitted = await companion.request("tick");
    assert.equal(admitted.kind, "acceptInvite");
    await poll(() => companion.request("snapshot"), state => state.party.members.length === 2, "companion group roster");
    await poll(() => Promise.resolve(leader.snapshot()), state => state.party.members.includes(identities[1].name), "leader group roster");
    checks.push("a pending invitation alone is not membership; acceptance updates both authoritative rosters"); checkpoint("shared-party-passed");

    phase = "ordinary follow";
    const start = leader.snapshot().player;
    await walkTo(leader, { x: start.x + 5, y: start.y }, 0, { tickCompanion: true });
    await waitCompanionNearLeader();
    const remote = leader.snapshot().entities.find(entity => entity.kind === "player" && entity.name === identities[1].name);
    assert.ok(remote);
    assert.ok(trace.some(entry => entry.source === "leader" && entry.event?.packet === "ObjectWalk" && entry.event.payload.objectId === remote.objectId));
    checks.push("ordinary companion Walk intents are visible to the human observer and close the follow distance"); checkpoint("follow-passed");

    phase = "prepare ordinary combat and a permitted candle drop";
    let monster = leader.snapshot().entities.filter(entity => entity.kind === "monster" && entity.name === "Scarecrow" && !entity.dead)
      .sort((a, b) => distance(start, a) - distance(start, b))[0];
    if (!monster) {
      await walkTo(leader, { x: 300, y: 610 }, 3, { tickCompanion: true });
      monster = leader.snapshot().entities.find(entity => entity.kind === "monster" && entity.name === "Scarecrow" && !entity.dead);
    }
    assert.ok(monster, "A normal visible Scarecrow must exist; never spawn a test monster.");
    // Leave the candle behind the fighting position. The untrained policy also
    // defends itself when adjacent to a hostile; teaching must not disable that.
    await walkTo(leader, monster, 4, { tickCompanion: true });
    await waitCompanionNearLeader();
    await companion.request("dropCandle");
    await sleep(650);
    await walkTo(leader, monster, 1);
    await engage(monster);
    await poll(() => companion.request("snapshot"), state => state.groundDrops.some(drop => drop.name === "Candle"), "visible ordinary candle drop");
    const beforeState = await companion.request("snapshot");
    note({ phase: "before-teaching", state: brief(beforeState) });
    const before = await companion.request("tick");
    assert.equal(before.kind, "loot", "The safe collection preference should really choose the permitted drop before support.");
    assert.equal(before.performed, true);
    // Let the untrained preference finish the real pickup, without creating a game result.
    let pickedUp = before.changed === true;
    for (let step = 0; !pickedUp && step < 10; step += 1) {
      await sleep(650);
      const result = await companion.request("tick");
      pickedUp = result.kind === "loot" && result.changed === true;
    }
    assert.ok(pickedUp, "Ordinary pickup must change the companion's own inventory.");
    checks.push("before feedback, the real safe collection policy walks to and picks up the ordinary candle during a confirmed party engagement"); checkpoint("before-teaching-passed");

    phase = "teach support first and execute the changed preference";
    await companion.request("dropCandle");
    await sleep(650);
    await engage(monster);
    const taught = await companion.request("teach", { feedbackId: "first-party-feedback" });
    assert.equal(taught.policyVersion, 2);
    const trainedState = await companion.request("snapshot");
    assert.ok(trainedState.groundDrops.some(drop => drop.name === "Candle"));
    note({ phase: "after-teaching", state: brief(trainedState) });
    const after = await companion.request("tick");
    assert.equal(after.kind, "attack"); assert.equal(after.policyVersion, 2);
    assert.equal(after.performed, true);
    checks.push("the same ordinary drop/combat conflict selects support after local-owner feedback (policy 1 -> 2)");

    phase = "companion confirmed damage and post-combat pickup";
    let hit = false, postCombatPickup = false;
    const combatDeadline = Date.now() + 65_000;
    while (Date.now() < combatDeadline) {
      const state = await companion.request("snapshot");
      const selfZoneId = state.selfZoneObjectId;
      hit ||= trace.some(entry => entry.event?.packet === "ObjectStruck" && entry.event.payload.attackerId === selfZoneId && entry.event.payload.objectId === monster.objectId);
      const result = await companion.request("tick");
      if (result.kind === "loot" && result.changed) { postCombatPickup = true; break; }
      if (leader.snapshot().playerHp <= leader.snapshot().playerMaxHp * 0.65) {
        const potion = [...leader.snapshot().beltItems, ...leader.snapshot().inventoryItems].find(item => item.name === "(HP)DrugSmall");
        if (potion) await leader.useOwnedItem(potion.uniqueId, potion.container === "belt" ? "belt" : "inventory");
      }
      await sleep(650);
    }
    assert.ok(hit, "An actual ObjectStruck must bind this companion's Zone ID to damage on the selected monster.");
    assert.ok(postCombatPickup, "The taught companion must resume ordinary pickup after observed combat ends.");
    checks.push("the observer confirms the companion's real monster strike; the taught companion resumes pickup after the fight"); checkpoint("combat-and-training-passed");

    phase = "restart the actual companion process";
    const saved = await companion.request("snapshot");
    const oldPid = companion.pid;
    await companion.request("stop");
    await poll(() => Promise.resolve(leader.snapshot()), state => state.party.members.length === 0, "party disconnect cleanup");
    checkpoint("waiting-31s-for-existing-route-lease"); await sleep(31_000);
    companion = launchCompanion();
    const restored = await companion.request("start", { profile, create: false });
    assert.notEqual(companion.pid, oldPid);
    assert.deepEqual(point(restored.player), point(saved.player));
    const persistedReport = trace.filter(entry => entry.type === "report" && entry.event.type === "companionReady").at(-1);
    assert.equal(persistedReport.event.policyVersion, 2);
    // Existing Gateway bootstrap grace is 15s. Fast-path movement does not
    // currently wake its personal packet drain; do not kill a second monster
    // while waiting for that real observer stream to become active.
    checkpoint("waiting-16s-for-existing-bootstrap-grace"); await sleep(16_000);
    await leader.inviteMember(identities[1].name);
    await poll(() => companion.request("snapshot"), state => state.party.pendingInvite?.name === identities[0].name, "fresh invitation after restart");
    assert.equal((await companion.request("tick")).kind, "acceptInvite");
    checks.push("a different Node process reloads saved character position and policy version 2 without teaching again; stale party membership is cleared");

    phase = "next adventure retains the learned choice after process restart";
    const nextMonster = leader.snapshot().entities.filter(entity => entity.kind === "monster" && entity.name === "Scarecrow" && !entity.dead && entity.objectId !== monster.objectId)
      .sort((a, b) => distance(leader.snapshot().player, a) - distance(leader.snapshot().player, b))[0];
    assert.ok(nextMonster);
    await walkTo(leader, nextMonster, 4, { tickCompanion: true });
    await waitCompanionNearLeader();
    await companion.request("dropCandle"); await sleep(650);
    await walkTo(leader, nextMonster, 1);
    await engage(nextMonster);
    let remembered = await companion.request("tick");
    assert.equal(remembered.kind, "attack"); assert.equal(remembered.policyVersion, 2); assert.equal(remembered.performed, true);
    for (let step = 0; remembered.confirmed !== false && step < 12; step += 1) {
      await sleep(650); remembered = await companion.request("tick");
      assert.equal(remembered.kind, "attack"); assert.equal(remembered.policyVersion, 2);
    }
    assert.equal(remembered.confirmed, false, "The restarted companion must send a real attack after closing distance.");
    checks.push("a new real monster/drop encounter after process restart still chooses support, without new teaching");
    checkpoint("restart-and-next-adventure-passed");

    phase = "owner recall withdrawal";
    await walkTo(leader, { x: 295, y: 616 }, 0, { tickCompanion: true });
    await waitCompanionNearLeader();
    const beforeRecall = await companion.request("snapshot");
    await companion.request("recall", { value: true });
    const recalled = await companion.request("tick");
    assert.equal(recalled.kind, "retreat"); assert.equal(recalled.reason, "owner-recall");
    assert.equal(recalled.moved, true);
    const afterRecall = await companion.request("snapshot");
    const home = { x: 288, y: 616 };
    assert.ok(distance(afterRecall.player, home) < distance(beforeRecall.player, home));
    await companion.request("recall", { value: false });
    checks.push("owner recall causes a real ordinary walk back toward the configured Bichon safe starting position");

    phase = "natural field damage, owned potion, and automatic danger withdrawal";
    checkpoint("field-damage-observation");
    // The original starter dress can fully block Scarecrow's tiny damage.
    // Remove that owned item with the same normal packet as a human player;
    // the monster, HP and damage rules remain untouched.
    await companion.request("removeArmour");
    // Leave town safety through ordinary paths. No HP setter, test damage,
    // manual tick, account-store editing or monster spawn is used.
    // The earlier follow/combat stages use the autonomous controller. Here we
    // arrange one bounded ordinary encounter before enabling its medical
    // decision, so the single starter potion is not spent while travelling.
    await walkTo(leader, { x: 300, y: 614 }, 2);
    let field = leader.snapshot().entities.filter(entity => entity.kind === "monster" && entity.name === "Scarecrow" && !entity.dead &&
      (Math.abs(entity.x - 288) > 10 || Math.abs(entity.y - 616) > 10))
      .sort((a, b) => distance(leader.snapshot().player, a) - distance(leader.snapshot().player, b))[0];
    assert.ok(field, "A real field Scarecrow must exist outside the original Bichon safe zone.");
    await walkTo(leader, field, 3);
    for (let step = 0; step < 16; step += 1) {
      const state = await companion.request("snapshot");
      const current = state.entities.find(entity => entity.objectId === field.objectId && !entity.dead);
      assert.ok(current);
      if (distance(state.player, current) === 1) break;
      const direction = nextWalkDirection({ player: state.player, target: current, desiredDistance: 1, collision,
        occupied: state.entities.filter(entity => entity.objectId !== state.playerObjectId && !entity.dead).map(point) });
      assert.ok(direction); await companion.request("walk", { direction }); await sleep(650);
    }
    await companion.request("attackObservedMonster", { objectId: field.objectId });
    // Target selection uses the nearest exposed player. Move the armoured
    // human into town so the actual monster attacks the unarmoured companion.
    await walkTo(leader, { x: 295, y: 616 });
    await poll(() => companion.request("snapshot"), state => !state.inSafeZone, "authoritative safety flag follows ordinary field movement");
    const damaged = await poll(() => companion.request("snapshot"), state => state.playerHp > 0 && state.playerHp <= state.playerMaxHp * 0.65,
      "actual field monster damage below potion threshold", 100_000);
    checkpoint("natural-field-damage-passed");
    const ownedPotion = [...damaged.beltItems, ...damaged.inventoryItems].find(item => item.name === "(HP)DrugSmall");
    assert.ok(ownedPotion, "The single original potion must still be owned.");
    const potion = await companion.request("tick");
    assert.equal(potion.kind, "usePotion"); assert.equal(potion.confirmed, true);
    const consumed = await companion.request("snapshot");
    assert.ok(![...consumed.inventoryItems, ...consumed.beltItems].some(item => String(item.uniqueId) === String(ownedPotion.uniqueId)));
    checks.push("natural field damage triggers a successful exact-UID UseItem and consumes the original owned potion");
    checkpoint("owned-potion-consumed");
    await poll(() => companion.request("snapshot"), state => state.playerHp > damaged.playerHp,
      "server-owned timed potion recovery", 25_000);
    checks.push("the server subsequently reports actual HP recovery after the consumed potion");
    checkpoint("potion-recovery-passed");
    await poll(() => companion.request("snapshot"), state => state.playerHp > 0 && state.playerHp <= state.playerMaxHp * 0.65,
      "a later real low-health state without another owned potion", 100_000);
    let withdrew = false;
    for (let step = 0; step < 12; step += 1) {
      const result = await companion.request("tick");
      if (result.kind === "retreat" && result.reason === "unsafe-withdraw" && result.moved) { withdrew = true; break; }
      await sleep(650);
    }
    assert.ok(withdrew, "A low-health companion without available potion must actually withdraw through ordinary movement.");
    let escaped = false;
    for (let step = 0; step < 35; step += 1) {
      const state = await companion.request("snapshot");
      assert.ok(state.playerHp > 0, "Retreat must keep the companion alive.");
      if (state.inSafeZone) { escaped = true; break; }
      await companion.request("tick"); await sleep(650);
    }
    assert.ok(escaped, "The ordinary retreat must reach the actual Bichon safe zone.");
    checks.push("the low-health companion without another available potion automatically walks back into town safety alive"); checkpoint("potion-and-danger-withdrawal-passed");

    await companion.request("stop"); await leader.logout();
    const report = { schema: "mir2-alo-companion-acceptance/1", ok: true, startedAt, finishedAt: new Date().toISOString(),
      realmId: gateway.realmId, gatewayBinarySha256: await sha256(gateway.executable), normalClientOnly: true, modelEnabled: false,
      memoryScope: "local-owner-feedback-and-action-episodes", checks, commands,
      reloginSettleMs: 31_000, bootstrapSettleMs: 16_000,
      limitations: ["single-Gateway shared party", "local owner feedback control", "existing route lease and bootstrap grace settle", "no model planner or frontend visual acceptance"] };
    await writeFile(resolve(gateway.output, "summary.json"), stringifyGatewayJson(report) + "\n");
    await writeFile(resolve(gateway.output, "trace.json"), stringifyGatewayJson(trace) + "\n");
    console.log(JSON.stringify({ ok: true, checks: checks.length, report: resolve(gateway.output, "summary.json") }));
  } catch (error) {
    let diagnostic;
    try { diagnostic = { leader: leader && brief(leader.snapshot()), companion: companion && brief(await companion.request("snapshot")) }; } catch { /* closed client */ }
    await writeFile(resolve(gateway.output, "summary.json"), stringifyGatewayJson({ ok: false, startedAt, phase, error: error.message, checks, commands, diagnostic }) + "\n");
    await writeFile(resolve(gateway.output, "trace.json"), stringifyGatewayJson(trace) + "\n");
    await gateway.saveDiagnostics();
    console.error(JSON.stringify({ ok: false, phase, error: error.message, report: resolve(gateway.output, "summary.json") }));
    process.exitCode = 1;
  } finally {
    await leader?.close();
    for (const child of processes) if (child.exitCode === null) child.kill();
    await gateway.close();
  }
}

acceptance().catch(error => { console.error(`Companion acceptance could not start: ${error.message}`); process.exitCode = 1; });
