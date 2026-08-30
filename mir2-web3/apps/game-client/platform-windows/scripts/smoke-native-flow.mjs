#!/usr/bin/env node

import crypto from "node:crypto";
import fs from "node:fs/promises";
import path from "node:path";

// Headless contract smoke for the Windows-native visible flow. This speaks the
// same BrowserCommand JSON protocol as the native host, but never mutates game
// state locally: every transition is acknowledged by the real Gateway and
// Simulation. It also verifies one logout/login persistence round-trip.

let cli;
try {
  cli = parseCli(process.argv.slice(2));
} catch (error) {
  console.error(JSON.stringify({ ok: false, status: "BLOCKED", error: String(error?.message ?? error), desktopTouched: false }, null, 2));
  process.exitCode = 1;
  process.exit();
}
const outputPath = cli.outputPath;
if (cli.mode === "help") {
  printCliHelp();
  process.exit(0);
}
if (cli.mode === "self-test" || cli.mode === "dry-run") {
  await emitReport({
    ok: true,
    status: cli.mode === "self-test" ? "SELF_TEST" : "CONFIRM_REQUIRED",
    mode: cli.mode,
    desktopTouched: false,
    accountMutation: cli.mode === "dry-run" ? "not executed" : false,
    gatewayUrl: cli.gatewayUrl,
    timeoutMs: cli.timeoutMs,
    note: cli.mode === "dry-run"
      ? "Live smoke creates an account/character and requires --allow-account-mutation; no socket was opened."
      : "Self-test validates arguments only; no socket was opened.",
  });
  process.exit(0);
}
if (!cli.allowAccountMutation) {
  console.error(JSON.stringify({
    ok: false,
    status: "CONFIRM_REQUIRED",
    error: "Live smoke creates a disposable account and character; pass --allow-account-mutation explicitly.",
    desktopTouched: false,
  }, null, 2));
  process.exitCode = 2;
  process.exit();
}

const gatewayUrl = cli.gatewayUrl;
const timeoutMs = cli.timeoutMs;
const parsedCombatTimeoutMs = cli.combatTimeoutMs;
// Crystal's Scarecrow quest drop is 1/5 and a level-one warrior needs roughly
// twenty basic attacks per kill. Two minutes only covers one or two kills and
// therefore turns the canonical Q1 -> Q2 smoke into a probabilistic gate.
const combatTimeoutMs = Number.isFinite(parsedCombatTimeoutMs) ? parsedCombatTimeoutMs : 600_000;
const runToken = `${Date.now().toString(36)}${process.pid.toString(36)}`.replace(/[^a-z0-9]/gi, "");
const accountId = (process.env.MIR2_NATIVE_SMOKE_ACCOUNT ?? `nw${runToken}`).slice(0, 18);
const password = process.env.MIR2_NATIVE_SMOKE_PASSWORD ?? "native-pass";
const characterName = (process.env.MIR2_NATIVE_SMOKE_CHARACTER ?? `N${runToken}`).slice(-10);
const reuseExisting = /^(1|true|yes)$/i.test(process.env.MIR2_NATIVE_SMOKE_REUSE ?? "");
const exerciseCombat = /^(1|true|yes)$/i.test(process.env.MIR2_NATIVE_SMOKE_COMBAT ?? "");
const exerciseQuest =
  exerciseCombat || /^(1|true|yes)$/i.test(process.env.MIR2_NATIVE_SMOKE_QUEST ?? "");
const combatCaptureTargetName = (process.env.MIR2_NATIVE_SMOKE_COMBAT_TARGET ?? "Scarecrow").trim() || "Scarecrow";
const stopAfterMode = (process.env.MIR2_NATIVE_SMOKE_STOP_AFTER ?? "").trim().toLowerCase();
const stopAfterQuest2Accepted = stopAfterMode === "quest2-accepted";
const stopAfterCombatDamaged = stopAfterMode === "combat-damaged";
const parsedTotalTimeoutMs = cli.totalTimeoutMs;
const totalTimeoutMs = Number.isFinite(parsedTotalTimeoutMs) ? parsedTotalTimeoutMs : 420_000;
const overallDeadline = Date.now() + Math.max(60_000, totalTimeoutMs);
const progressEnabled = cli.progressEnabled;
const combatAnchorParts = String(process.env.MIR2_NATIVE_SMOKE_COMBAT_ANCHOR ?? "")
  .split(",")
  .map((value) => Number(value.trim()));
const combatAnchor =
  combatAnchorParts.length === 2 && combatAnchorParts.every(Number.isInteger)
    ? { x: combatAnchorParts[0], y: combatAnchorParts[1] }
    : null;
const questNpcAnchors = new Map([
  [3, { objectId: 3, kind: "npc", name: "Assistant Jane", x: 284, y: 606 }],
  [4, { objectId: 4, kind: "npc", name: "CraftsLady", x: 294, y: 619 }],
]);
const walkDirectionDeltas = new Map([
  ["up", { x: 0, y: -1 }],
  ["upright", { x: 1, y: -1 }],
  ["right", { x: 1, y: 0 }],
  ["downright", { x: 1, y: 1 }],
  ["down", { x: 0, y: 1 }],
  ["downleft", { x: -1, y: 1 }],
  ["left", { x: -1, y: 0 }],
  ["upleft", { x: -1, y: -1 }],
]);

let sequence = 0;
let latestSnapshot = null;
let authoritativePlayerLocation = null;
const messages = [];
// Keep owner lifecycle evidence independently from the bounded general packet
// history. A revive can immediately repopulate a large AOI, which would
// otherwise evict Revived/UserLocation before the wait predicate observes it.
const lifecycleMessages = [];
// Crystal `Q` monster drops are delivered directly to the eligible player's
// quest bag rather than becoming ground objects. Keep that kill-linked packet
// evidence outside the bounded general history so a busy AOI cannot evict it.
const gainedItemMessages = [];
const questDefinitionPackets = new Map();
const zoneEntities = new Map();
const zoneDrops = new Map();
const zoneTombstones = new Map();
const zoneDeathEvidence = new Map();
const zonePacketNames = new Set([
  "ObjectMonster",
  "NewMonsterInfo",
  "ObjectNpc",
  "NewNpcInfo",
  "ObjectPlayer",
  "ObjectHero",
  "ObjectWalk",
  "ObjectRun",
  "ObjectTurn",
  "ObjectHealth",
  "ObjectDied",
  "ObjectRevived",
  "ObjectRemove",
  "ObjectHide",
  "ObjectShow",
  "ObjectItem",
  "ObjectGold",
  "GainedItem",
  "GainedGold",
  "Death",
  "Revived",
  "MapChanged",
]);
let zoneOverlayEpoch = 0;
let zoneOverlayVersion = 0;
let zoneOverlayResetReason = "initial";
const socket = new WebSocket(gatewayUrl);

socket.addEventListener("message", (event) => {
  let message;
  try {
    message = JSON.parse(String(event.data));
  } catch {
    return;
  }
  const entry = { sequence: ++sequence, message };
  if (message.type === "worldSnapshot") {
    latestSnapshot = entry;
    return;
  }
  if (message.type === "packet") {
    if (message.packet === "MapChanged") {
      resetZoneOverlay("MapChanged");
      const location = packetLocation(message.payload);
      if (location) {
        authoritativePlayerLocation = {
          ...location,
          direction: message.payload?.direction ?? null,
        };
      }
    } else if (message.packet === "LogOutSuccess") {
      resetZoneOverlay("LogOut");
    } else if (message.packet === "UserLocation") {
      const location = packetLocation(message.payload);
      if (location) {
        authoritativePlayerLocation = {
          ...location,
          direction: message.payload?.direction ?? null,
        };
      }
    }
    applyZonePacket(entry);
  }
  if (message.type === "packet" && message.packet === "NewQuestInfo") {
    const questId = Number(message.payload?.id);
    if (Number.isInteger(questId)) questDefinitionPackets.set(questId, message);
  }
  if (
    message.type === "packet" &&
    ["UserLocation", "Revived", "ObjectRevived"].includes(message.packet)
  ) {
    lifecycleMessages.push(entry);
    if (lifecycleMessages.length > 128) lifecycleMessages.shift();
  }
  if (message.type === "packet" && message.packet === "GainedItem") {
    gainedItemMessages.push(entry);
    if (gainedItemMessages.length > 128) gainedItemMessages.shift();
  }
  messages.push(entry);
  if (messages.length > 500) messages.shift();
});

await waitForEvent(socket, "open", "gateway connection");
traceStage("connected", { gatewayUrl });

try {
  send({ type: "clientVersion" });

  let checkpoint = sequence;
  if (!reuseExisting) {
    send({
      type: "newAccount",
      accountId,
      password,
      birthDateBinary: 0,
      userName: accountId,
      secretQuestion: "",
      secretAnswer: "",
      emailAddress: "",
    });
    const accountReply = await waitForPacket("NewAccount", checkpoint);
    assert(
      Number(accountReply.payload?.result) === 8,
      `NewAccount rejected with result ${accountReply.payload?.result ?? "missing"}`,
    );
  }

  checkpoint = sequence;
  send({ type: "login", accountId, password });
  const firstLogin = await waitForPacket("LoginSuccess", checkpoint);
  assert(Array.isArray(firstLogin.payload?.characters), "LoginSuccess omitted character roster");

  let characterIndex;
  if (reuseExisting) {
    const existing = firstLogin.payload.characters.find((character) => character?.name === characterName);
    characterIndex = Number(existing?.index);
    assert(Number.isInteger(characterIndex), `existing character ${characterName} was not in LoginSuccess`);
  } else {
    checkpoint = sequence;
    send({ type: "newCharacter", name: characterName, gender: "Male", class: "Warrior" });
    const created = await waitForPacket("NewCharacterSuccess", checkpoint);
    characterIndex = Number(created.payload?.character?.index);
    assert(Number.isInteger(characterIndex), "NewCharacterSuccess omitted integer character index");
  }

  let firstWorld = await startGameAndReadWorld(characterIndex);
  traceStage("start-game", summarizeWorld(firstWorld));
  let questResult = null;
  if (exerciseQuest) {
    const assistant = await completeAssistantQuest(firstWorld);
    questResult = assistant;
    if (exerciseCombat) {
      const craftLady = await completeCraftLadyQuest(assistant.world, {
        stopAfterQuest2Accepted,
        stopAfterCombatDamaged,
      });
      questResult = {
        world: craftLady.world,
        report: { assistant: assistant.report, craftLady: craftLady.report },
      };
      if (craftLady.stopAfter) {
        questResult.stopAfter = craftLady.stopAfter;
        questResult.evidence = craftLady.evidence;
      }
    }
  }
  if (questResult) firstWorld = questResult.world;
  if (exerciseCombat && !questResult?.stopAfter) {
    const groundItemRoundTrip = await exerciseOrdinaryGroundItemRoundTrip(firstWorld);
    firstWorld = groundItemRoundTrip.world;
    if (questResult?.report) {
      questResult.report.ordinaryGroundItemRoundTrip = groundItemRoundTrip.report;
    }
  }
  const firstSummary = summarizeWorld(firstWorld);
  assert(firstSummary.playerName === characterName, "first StartGame bootstrapped the wrong character");
  let didStopEarly = false;
  let stopAfterReport = null;

  if (questResult?.stopAfter === "quest2-accepted" || questResult?.stopAfter === "combat-damaged") {
    checkpoint = sequence;
    send({ type: "logOut" });
    await waitForPacket("LogOutSuccess", checkpoint);
    stopAfterReport = {
      ok: true,
      status: "HANDOFF",
      stopAfter: questResult.stopAfter,
      gatewayUrl,
      accountId,
      characterName,
      characterIndex,
      precondition: {
        reason: questResult.stopAfter,
        evidence: questResult.evidence ?? null,
      },
      firstWorld: firstSummary,
      questPackets: [
        ...messages
          .filter(({ message }) => ["ChangeQuest", "CompleteQuest", "ObjectHealth"].includes(message.packet))
          .map(({ message }) => message),
      ]
        .slice(0, 12)
        .map(summarizeQuestPacket),
    };
    didStopEarly = true;
  }

  if (didStopEarly) {
    await emitReport(stopAfterReport);
  } else {
    checkpoint = sequence;
    send({ type: "logOut" });
    await waitForPacket("LogOutSuccess", checkpoint);

    checkpoint = sequence;
    send({ type: "clientVersion" });
    send({ type: "login", accountId, password });
    const secondLogin = await waitForPacket("LoginSuccess", checkpoint);
    const rosterCharacter = secondLogin.payload?.characters?.find(
      (character) => Number(character?.index) === characterIndex,
    );
    assert(rosterCharacter?.name === characterName, "re-login roster lost the created character");

    const secondWorld = await startGameAndReadWorld(characterIndex);
    const secondSummary = summarizeWorld(secondWorld);
    assert(secondSummary.playerName === firstSummary.playerName, "character identity changed after re-login");
    assert(secondSummary.mapFileName === firstSummary.mapFileName, "map did not persist after re-login");
    assert(secondSummary.position?.x === firstSummary.position?.x, "player x did not persist after re-login");
    assert(secondSummary.position?.y === firstSummary.position?.y, "player y did not persist after re-login");
    verifyPersistence(firstWorld, secondWorld, { exerciseQuest, exerciseCombat });

    const persistedGroundRoundTrip = questResult?.report?.ordinaryGroundItemRoundTrip;
    if (persistedGroundRoundTrip) {
      const persistedQuantity = itemQuantityByUniqueId(
        secondWorld,
        persistedGroundRoundTrip.uniqueId,
      );
      assert(
        persistedQuantity === Number(persistedGroundRoundTrip.quantityBefore),
        `ordinary item ${persistedGroundRoundTrip.uniqueId} did not persist after re-login`,
      );
      persistedGroundRoundTrip.persistedQuantityAfterRelogin = persistedQuantity;
    }

    const questReport = questResult?.report ?? null;
    const assistantReport = exerciseCombat ? questReport?.assistant : questReport;
    const craftLadyReport = exerciseCombat ? questReport?.craftLady : null;
    const groundRoundTripReport = exerciseCombat
      ? questReport?.ordinaryGroundItemRoundTrip
      : null;
    const freshCombatProven =
      !craftLadyReport?.resumed &&
      Number(craftLadyReport?.combat?.attacks ?? 0) > 0 &&
      Number(craftLadyReport?.combat?.monsterKills ?? 0) > 0;
    const freshQuest2CompletionProven =
      !craftLadyReport?.resumed &&
      Array.isArray(craftLadyReport?.completePacket?.completedQuests) &&
      craftLadyReport.completePacket.completedQuests.includes(2);
    const ordinaryGroundRoundTripProven =
      Number(groundRoundTripReport?.quantityBefore ?? 0) > 0 &&
      groundRoundTripReport?.dropAckSuccess === true &&
      Number(groundRoundTripReport?.gainedUniqueId ?? -1) ===
        Number(groundRoundTripReport?.uniqueId ?? -2) &&
      Number(groundRoundTripReport?.persistedQuantityAfterRelogin ?? -1) ===
        Number(groundRoundTripReport.quantityBefore);
    const verifiedCapabilities = reuseExisting
      ? ["account.login", "character.start-game", "session.logout-relogin", "persistence.position"]
      : [
          "account.register-login",
          "character.create-start-game",
          "session.logout-relogin",
          "persistence.position",
        ];
    if (assistantReport?.resumed) verifiedCapabilities.push("quest.q1.persisted-completed");
    else if (assistantReport) verifiedCapabilities.push("quest.q1.complete");
    if (craftLadyReport?.resumed) verifiedCapabilities.push("quest.q2.persisted-completed");
    if (freshCombatProven) verifiedCapabilities.push("combat.basic-attack-kill", "quest.q2.rng-task-item");
    if (freshQuest2CompletionProven) verifiedCapabilities.push("quest.q2.complete-rewards");
    if (ordinaryGroundRoundTripProven) verifiedCapabilities.push("item.drop-pickup-same-unique-id");
    if (exerciseQuest) verifiedCapabilities.push("persistence.quest-state");
    if (exerciseCombat) verifiedCapabilities.push("persistence.gold-inventory");

    const report = {
      ok: true,
      status: "PASS",
      verifiedScope: freshCombatProven && freshQuest2CompletionProven && ordinaryGroundRoundTripProven
        ? "bichon-q1-q2-combat-ground-item-persistence"
        : exerciseCombat
          ? "completed-character-resume-ground-item-persistence"
          : exerciseQuest
          ? "bichon-q1-persistence"
          : "account-character-start-game-persistence",
      verifiedCapabilities,
      runConfiguration: {
        exerciseQuest,
        exerciseCombat,
        reuseExisting,
        stopAfterMode: stopAfterMode || null,
        timeoutMs,
        combatTimeoutMs,
        totalTimeoutMs,
      },
      gatewayUrl,
      accountId,
      characterName,
      characterIndex,
      questResult: questReport,
      firstWorld: firstSummary,
      secondWorld: secondSummary,
      questPackets: [
        ...(Array.isArray(firstWorld.questLog) ? firstWorld.questLog : [])
          .map((quest) => questDefinitionPackets.get(Number(quest?.questId)))
          .filter(Boolean),
        ...messages
          .filter(({ message }) => ["ChangeQuest", "CompleteQuest"].includes(message.packet))
          .map(({ message }) => message),
      ]
        .slice(0, 8)
        .map(summarizeQuestPacket),
    };
    await emitReport(report);
  }
  send({ type: "disconnect" });
} catch (error) {
  console.error(
    JSON.stringify(
      {
        ok: false,
        status: "BLOCKED",
        gatewayUrl,
        accountId,
        characterName,
        error: String(error?.message ?? error),
        recentPackets: messages.slice(-20).map(({ message }) => ({
          type: message.type,
          packet: message.packet,
          payload: compactRecord(message.payload),
        })),
        recentZonePackets: recentZonePacketDiagnostics(30),
        zoneOverlay: summarizeZoneOverlay(),
      },
      null,
      2,
    ),
  );
  process.exitCode = 1;
} finally {
  socket.close();
}

async function startGameAndReadWorld(characterIndex) {
  resetZoneOverlay("StartGame");
  const checkpoint = sequence;
  send({ type: "startGame", characterIndex });
  const reply = await waitForPacket("StartGame", checkpoint);
  assert(Number(reply.payload?.result) === 4, `StartGame rejected with result ${reply.payload?.result}`);
  await waitForPacket("UserInformation", checkpoint);
  const snapshot = await waitFor(
    () => {
      if (!latestSnapshot || latestSnapshot.sequence <= checkpoint) return null;
      const payload = currentWorld();
      return typeof payload?.mapFileName === "string" && typeof payload?.playerHp === "number"
        ? payload
        : null;
    },
    "authoritative in-game worldSnapshot",
  );
  return snapshot;
}

function resetZoneOverlay(reason) {
  zoneEntities.clear();
  zoneDrops.clear();
  zoneTombstones.clear();
  zoneDeathEvidence.clear();
  zoneOverlayEpoch += 1;
  zoneOverlayVersion += 1;
  zoneOverlayResetReason = reason;
  authoritativePlayerLocation = null;
  latestSnapshot = null;
}

function applyZonePacket(entry) {
  const packet = entry.message?.packet;
  const payload = entry.message?.payload;
  if (!zonePacketNames.has(packet) || !payload || typeof payload !== "object") return;

  const objectId = packetObjectId(payload);
  if (packet === "ObjectMonster" || packet === "NewMonsterInfo") {
    if (objectId === null) return;
    const location = packetLocation(payload);
    const patch = {
      kind: "monster",
      disposition: "hostile",
    };
    const name = packetName(payload);
    if (name !== null) patch.name = name;
    if (location) Object.assign(patch, location);
    if (typeof payload.direction === "string") patch.direction = payload.direction;
    if (typeof payload.dead === "boolean") patch.dead = payload.dead;
    for (const field of ["ai", "image", "light", "nameColourArgb"]) {
      const value = Number(payload[field]);
      if (Number.isFinite(value)) patch[field] = value;
    }
    patchZoneEntity(objectId, patch, entry.sequence, { spawn: true });
    if (payload.dead === true) {
      zoneDeathEvidence.set(String(objectId), { sequence: entry.sequence, packet });
    } else if (payload.dead === false) {
      zoneDeathEvidence.delete(String(objectId));
    }
    return;
  }

  if (packet === "ObjectNpc" || packet === "NewNpcInfo" || packet === "ObjectPlayer" || packet === "ObjectHero") {
    if (objectId === null) return;
    const location = packetLocation(payload);
    const npc = packet === "ObjectNpc" || packet === "NewNpcInfo";
    const patch = {
      kind: npc ? "npc" : "player",
      disposition: npc ? "neutral" : "friendly",
    };
    const name = packetName(payload);
    if (name !== null) patch.name = name;
    if (location) Object.assign(patch, location);
    if (typeof payload.direction === "string") patch.direction = payload.direction;
    patchZoneEntity(objectId, patch, entry.sequence, { spawn: true });
    return;
  }

  if (packet === "ObjectWalk" || packet === "ObjectRun" || packet === "ObjectTurn") {
    if (objectId === null) return;
    const patch = {};
    const location = packetLocation(payload);
    if (location) Object.assign(patch, location);
    if (typeof payload.direction === "string") patch.direction = payload.direction;
    patchZoneEntity(objectId, patch, entry.sequence);
    return;
  }

  if (packet === "ObjectHealth") {
    if (objectId === null) return;
    const percent = packetHealthPercent(payload);
    if (percent === null) return;
    patchZoneEntity(
      objectId,
      {
        hpPercent: percent,
        dead: percent <= 0,
      },
      entry.sequence,
    );
    if (percent <= 0) {
      zoneDeathEvidence.set(String(objectId), { sequence: entry.sequence, packet });
    } else {
      zoneDeathEvidence.delete(String(objectId));
    }
    return;
  }

  if (packet === "ObjectRevived") {
    if (objectId === null) return;
    patchZoneEntity(objectId, { dead: false }, entry.sequence, { spawn: true });
    zoneDeathEvidence.delete(String(objectId));
    return;
  }

  if (packet === "Revived") {
    const playerObjectId = Number(latestSnapshot?.message?.payload?.playerObjectId);
    if (!Number.isSafeInteger(playerObjectId) || playerObjectId <= 0) return;
    patchZoneEntity(playerObjectId, { dead: false }, entry.sequence, { spawn: true });
    zoneDeathEvidence.delete(String(playerObjectId));
    return;
  }

  if (packet === "ObjectDied") {
    if (objectId === null) return;
    const location = packetLocation(payload);
    patchZoneEntity(
      objectId,
      {
        ...(location ?? {}),
        dead: true,
      },
      entry.sequence,
    );
    zoneDeathEvidence.set(String(objectId), { sequence: entry.sequence, packet });
    return;
  }

  if (packet === "ObjectRemove" || packet === "ObjectHide") {
    if (objectId !== null) tombstoneZoneObject(objectId, entry.sequence, packet);
    return;
  }

  if (packet === "ObjectShow") {
    const key = zoneObjectKey(objectId);
    if (key && zoneTombstones.delete(key)) zoneOverlayVersion += 1;
    return;
  }

  if (packet === "ObjectItem" || packet === "ObjectGold") {
    if (objectId === null) return;
    const location = packetLocation(payload);
    if (!location) return;
    const key = zoneObjectKey(objectId);
    if (!key) return;
    const previous = zoneDrops.get(key) ?? {};
    const name = packetName(payload);
    const quantity =
      packet === "ObjectGold" && Number.isFinite(Number(payload.gold))
        ? Number(payload.gold)
        : Number.isFinite(Number(payload.quantity))
          ? Number(payload.quantity)
          : 1;
    zoneTombstones.delete(key);
    zoneEntities.delete(key);
    zoneDrops.set(key, {
      ...previous,
      objectId,
      ...(name !== null ? { name } : packet === "ObjectGold" ? { name: "Gold" } : {}),
      ...location,
      quantity,
      dropKind: packet === "ObjectGold" ? "gold" : "item",
      _zoneSequence: entry.sequence,
    });
    zoneOverlayVersion += 1;
  }
}

function patchZoneEntity(objectId, patch, packetSequence, options = {}) {
  const key = zoneObjectKey(objectId);
  if (!key) return;
  const tombstone = zoneTombstones.get(key);
  if (tombstone && !options.spawn) return;
  const previous = zoneEntities.get(key) ?? {};
  const resurrecting = Boolean(tombstone) || previous.dead === true;
  if (options.spawn) {
    zoneTombstones.delete(key);
    zoneDrops.delete(key);
  }
  const next = {
    ...previous,
    ...patch,
    objectId,
    _zoneSequence: packetSequence,
    ...(options.spawn ? { _zoneSpawnSequence: packetSequence } : {}),
  };
  if (options.spawn && resurrecting && patch.dead === false) delete next.hpPercent;
  zoneEntities.set(key, next);
  zoneOverlayVersion += 1;
}

function tombstoneZoneObject(objectId, packetSequence, packet) {
  const key = zoneObjectKey(objectId);
  if (!key) return;
  zoneEntities.delete(key);
  zoneDrops.delete(key);
  zoneTombstones.set(key, { sequence: packetSequence, packet });
  zoneOverlayVersion += 1;
}

function currentWorld() {
  return mergeWorldWithZoneOverlay(latestSnapshot?.message?.payload ?? null);
}

function mergeWorldWithZoneOverlay(snapshot) {
  if (!snapshot || typeof snapshot !== "object") return null;
  if (
    snapshot._zoneOverlayEpoch === zoneOverlayEpoch &&
    snapshot._zoneOverlayVersion === zoneOverlayVersion
  ) {
    return snapshot;
  }

  const entities = [];
  const entityIds = new Set();
  for (const entity of Array.isArray(snapshot.entities) ? snapshot.entities : []) {
    const key = zoneObjectKey(entity?.objectId);
    if (key && zoneTombstones.has(key)) continue;
    if (!key) {
      entities.push(entity);
      continue;
    }
    entityIds.add(key);
    entities.push({ ...entity, ...(zoneEntities.get(key) ?? {}) });
  }
  for (const [key, entity] of zoneEntities) {
    if (entityIds.has(key) || zoneTombstones.has(key) || !entity?.kind) continue;
    if (!Number.isFinite(Number(entity.x)) || !Number.isFinite(Number(entity.y))) continue;
    entities.push({ ...entity });
  }

  const groundDrops = [];
  const dropIds = new Set();
  for (const drop of Array.isArray(snapshot.groundDrops) ? snapshot.groundDrops : []) {
    const key = zoneObjectKey(drop?.objectId);
    if (key && zoneTombstones.has(key)) continue;
    if (!key) {
      groundDrops.push(drop);
      continue;
    }
    dropIds.add(key);
    groundDrops.push({ ...drop, ...(zoneDrops.get(key) ?? {}) });
  }
  for (const [key, drop] of zoneDrops) {
    if (dropIds.has(key) || zoneTombstones.has(key)) continue;
    groundDrops.push({ ...drop });
  }

  return {
    ...snapshot,
    entities,
    groundDrops,
    _zoneOverlayEpoch: zoneOverlayEpoch,
    _zoneOverlayVersion: zoneOverlayVersion,
  };
}

function zoneObjectKey(value) {
  const objectId = Number(value);
  return Number.isSafeInteger(objectId) && objectId > 0 ? String(objectId) : null;
}

function packetObjectId(payload) {
  const objectId = Number(payload?.objectId);
  return Number.isSafeInteger(objectId) && objectId > 0 ? objectId : null;
}

function packetLocation(payload) {
  const location = payload?.location && typeof payload.location === "object" ? payload.location : payload;
  const x = Number(location?.x);
  const y = Number(location?.y);
  return Number.isInteger(x) && Number.isInteger(y) ? { x, y } : null;
}

function packetHealthPercent(payload) {
  const percent = Number(payload?.percent);
  return Number.isFinite(percent) && percent >= 0 && percent <= 100 ? percent : null;
}

function packetName(payload) {
  return typeof payload?.name === "string" && payload.name.trim() ? payload.name : null;
}

function recentZonePacketDiagnostics(limit) {
  return messages
    .filter(({ message }) => message.type === "packet" && zonePacketNames.has(message.packet))
    .slice(-limit)
    .map(({ sequence: packetSequence, message }) => {
      const payload = message.payload ?? {};
      return {
        sequence: packetSequence,
        packet: message.packet,
        objectId: packetObjectId(payload),
        location: packetLocation(payload),
        percent: packetHealthPercent(payload),
        dead: typeof payload.dead === "boolean" ? payload.dead : null,
        name: packetName(payload),
        gold: Number.isFinite(Number(payload.gold)) ? Number(payload.gold) : null,
        item: payload.item ? compactRecord(payload.item, ["uniqueId", "itemIndex", "name", "count"]) : null,
      };
    });
}

function summarizeZoneOverlay() {
  return {
    epoch: zoneOverlayEpoch,
    version: zoneOverlayVersion,
    resetReason: zoneOverlayResetReason,
    entities: [...zoneEntities.values()].slice(-12).map((entity) =>
      compactRecord(entity, ["objectId", "kind", "name", "x", "y", "direction", "hpPercent", "dead", "_zoneSequence"]),
    ),
    drops: [...zoneDrops.values()].slice(-12).map((drop) =>
      compactRecord(drop, ["objectId", "dropKind", "name", "quantity", "x", "y", "_zoneSequence"]),
    ),
    tombstones: [...zoneTombstones.entries()].slice(-12).map(([objectId, tombstone]) => ({
      objectId,
      ...tombstone,
    })),
    deaths: [...zoneDeathEvidence.entries()].slice(-12).map(([objectId, evidence]) => ({
      objectId,
      ...evidence,
    })),
  };
}

async function completeAssistantQuest(initialWorld) {
  const questIndex = 1;
  const initialQuest = findQuest(initialWorld, questIndex);
  if (initialQuest?.stage === "completed") {
    traceStage("quest1-resumed", { stage: initialQuest.stage });
    return {
      world: initialWorld,
      report: {
        questIndex,
        resumed: true,
        completedQuestIds: [questIndex],
        finalExperience: initialWorld.playerExperience ?? null,
        finalInventory: summarizeInventory(initialWorld),
      },
    };
  }
  assert(initialQuest?.stage === "available", "Assistant's Request was not available on a fresh character");
  const movement = [];
  const jane = await walkAdjacentToNpc(3, "Assistant Jane", movement);
  let checkpoint = sequence;
  send({ type: "interact", objectId: jane.objectId });
  const janeDialog = await waitForWorld(
    (world) => Number(world.activeNpcDialog?.npcObjectId) === jane.objectId,
    "Jane NPC dialog",
    checkpoint,
  );

  checkpoint = sequence;
  send({ type: "acceptQuest", npcIndex: 3, questIndex });
  const acceptedWorld = await waitForWorld(
    (world) => {
      const quest = findQuest(world, questIndex);
      return quest && quest.stage !== "available";
    },
    "Assistant's Request acceptance",
    checkpoint,
  );

  const craftsLady = await walkAdjacentToNpc(4, "CraftsLady", movement);
  checkpoint = sequence;
  send({ type: "interact", objectId: craftsLady.objectId });
  const craftsDialog = await waitForWorld(
    (world) => Number(world.activeNpcDialog?.npcObjectId) === craftsLady.objectId,
    "CraftsLady NPC dialog",
    checkpoint,
  );

  checkpoint = sequence;
  send({ type: "finishQuest", questIndex, selectedItemIndex: -1 });
  const completedPacket = await waitForPacket("CompleteQuest", checkpoint);
  assert(
    Array.isArray(completedPacket.payload?.completedQuests) &&
      completedPacket.payload.completedQuests.includes(questIndex),
    "CompleteQuest did not include Assistant's Request",
  );
  const completedWorld = await waitForWorld(
    (world) => {
      const quest = findQuest(world, questIndex);
      return !quest || quest.stage === "completed";
    },
    "completed quest snapshot",
    checkpoint,
  );
  traceStage("quest1-completed", { movementSteps: movement.length });

  return {
    world: completedWorld,
    report: {
      questIndex,
      jane: { objectId: jane.objectId, x: jane.x, y: jane.y },
      craftsLady: { objectId: craftsLady.objectId, x: craftsLady.x, y: craftsLady.y },
      movementSteps: movement.length,
      movement,
      janeDialog: summarizeDialog(janeDialog.activeNpcDialog),
      acceptedStage: findQuest(acceptedWorld, questIndex)?.stage ?? null,
      craftsDialog: summarizeDialog(craftsDialog.activeNpcDialog),
      completedQuestIds: completedPacket.payload.completedQuests,
      finalExperience: completedWorld.playerExperience ?? null,
      finalInventory: (Array.isArray(completedWorld.inventoryItems) ? completedWorld.inventoryItems : [])
        .map((item) => ({ key: item?.key ?? null, name: item?.name ?? null, quantity: item?.quantity ?? null })),
    },
  };
}

async function completeCraftLadyQuest(initialWorld, options = {}) {
  const {
    stopAfterQuest2Accepted = false,
    stopAfterCombatDamaged = false,
  } = options;

  const questIndex = 2;
  const initialQuest = findQuest(initialWorld, questIndex);
  if (initialQuest?.stage === "completed") {
    return {
      world: initialWorld,
      report: {
        questIndex,
        resumed: true,
        finalExperience: initialWorld.playerExperience ?? null,
        finalGold: initialWorld.gold ?? null,
        finalInventory: summarizeInventory(initialWorld),
      },
    };
  }
  assert(
    !initialQuest || ["available", "inProgress", "readyToTurnIn"].includes(initialQuest.stage),
    `CraftsLady quest cannot resume from stage ${initialQuest?.stage ?? "missing"}`,
  );
  let nowWorld = await ensurePlayerAlive(initialWorld, "starting or resuming Quest2");
  traceStage("quest2-begin", {
    stage: initialQuest?.stage ?? "missing",
    position: summarizeWorld(nowWorld).position,
  });

  const movement = [];
  let checkpoint;
  let acceptedWorld = nowWorld;
  if (!initialQuest || initialQuest.stage === "available") {
    const craftsLady = await walkAdjacentToNpc(4, "CraftsLady", movement);
    checkpoint = sequence;
    send({ type: "interact", objectId: craftsLady.objectId });
    await waitForWorld(
      (world) => Number(world.activeNpcDialog?.npcObjectId) === craftsLady.objectId,
      "CraftsLady NPC dialog",
      checkpoint,
    );

    checkpoint = sequence;
    send({ type: "acceptQuest", npcIndex: 4, questIndex });
    acceptedWorld = await waitForWorld(
      (world) => {
        const quest = findQuest(world, questIndex);
        return quest && ["inProgress", "readyToTurnIn"].includes(quest.stage);
      },
      "Quest2 acceptance",
      checkpoint,
    );
    nowWorld = await ensurePlayerAlive(acceptedWorld, "confirming Quest2 status after acceptance");
    traceStage("quest2-accepted", { stage: findQuest(nowWorld, questIndex)?.stage ?? null });
  }
  const startQuest = findQuest(await ensurePlayerAlive(acceptedWorld, "confirming Quest2 status"), questIndex);
  assert(startQuest, "Quest2 was not visible after acceptance/resume");
  const resumedReadyToTurnIn = startQuest.stage === "readyToTurnIn";
  if (stopAfterQuest2Accepted) {
    const startStage = startQuest.stage;
    assert(
      stopAfterQuest2Accepted && ["inProgress", "readyToTurnIn"].includes(startStage),
      `Quest2 stop-after condition was not satisfied at stage ${startStage ?? "missing"}`,
    );
    return {
      world: acceptedWorld,
      stopAfter: "quest2-accepted",
      evidence: {
        stage: startStage,
        quest2StageSnapshot: compactRecord(findQuest(acceptedWorld, questIndex), [
          "questId",
          "title",
          "stage",
          "current",
          "required",
          "objective",
          "objectives",
          "rewards",
        ]),
      },
      report: {
        questIndex,
        stage: startStage,
      },
    };
  }
  nowWorld = await ensurePlayerAlive(nowWorld, "preparing combat");
  if (combatAnchor && !resumedReadyToTurnIn) {
    await walkTo(
      { ...combatAnchor, name: "combat field anchor" },
      movement,
      4,
      "combat field anchor",
      { maxSteps: 160 },
    );
    nowWorld = await ensurePlayerAlive(currentWorld() ?? nowWorld, "arriving at combat field");
    traceStage("quest2-combat-anchor", { position: summarizeWorld(nowWorld).position });
  }
  const preCombatExp = Number(nowWorld.playerExperience ?? 0);
  const preCombatGold = Number(nowWorld.gold ?? 0);
  const preCombatInventory = inventorySignature(nowWorld);

  const combatDeadline = Date.now() + Math.max(30_000, combatTimeoutMs);
  traceStage("quest2-combat-start", {
    timeoutMs: Math.max(30_000, combatTimeoutMs),
    position: summarizeWorld(nowWorld).position,
  });
  const combatCheckpoint = sequence;
  let attackCount = 0;
  let monsterKills = 0;
  let pickupAttempts = 0;
  const pickedDropIds = [];
  const seenDropIds = new Set();
  const attackedMonsterIds = new Set();
  const confirmedKillIds = new Set();
  const activeCombatTargetName = stopAfterCombatDamaged ? combatCaptureTargetName : "Scarecrow";
  let pinnedCombatTargetId = null;
  let readyToTurnIn = resumedReadyToTurnIn;
  let directQuestItemGain = null;

  while (
    !resumedReadyToTurnIn &&
    Date.now() < combatDeadline &&
    (!readyToTurnIn || (pickedDropIds.length === 0 && directQuestItemGain === null))
  ) {
    assertWithinOverallDeadline("running Quest2 combat");
    collectConfirmedCombatKills(attackedMonsterIds, combatCheckpoint, confirmedKillIds);
    monsterKills = confirmedKillIds.size;
    const world = currentWorld() ?? nowWorld;
    assert(world, "world snapshot dropped while running quest2 combat flow");
    nowWorld = await ensurePlayerAlive(world, "running Quest2 combat");
    const quest = findQuest(nowWorld, questIndex);
    assert(quest, "Quest2 disappeared from authoritative quest log");
    readyToTurnIn = readyToTurnIn || quest.stage === "readyToTurnIn";
    if (Number.isInteger(quest.current) && Number.isInteger(quest.required) && quest.current >= quest.required) {
      readyToTurnIn = true;
    }
    directQuestItemGain =
      directQuestItemGain ?? findGainedItemAfter("GingerTea", 1112, combatCheckpoint);

    const existingPickup = await collectNearbyGroundDrops(nowWorld, movement);
    pickupAttempts += existingPickup.attempts;
    pickedDropIds.push(...existingPickup.pickedIds);
    for (const objectId of existingPickup.seenIds) seenDropIds.add(String(objectId));
    nowWorld = existingPickup.world;
    if (readyToTurnIn && (pickedDropIds.length > 0 || directQuestItemGain !== null)) {
      break;
    }

    let scarecrow =
      pinnedCombatTargetId === null ? null : findMonsterById(nowWorld, pinnedCombatTargetId);
    if (!scarecrow || !isMonsterAlive(scarecrow) || !scarecrow.name?.startsWith(activeCombatTargetName)) {
      pinnedCombatTargetId = null;
      scarecrow = findNearestMonster(nowWorld, activeCombatTargetName);
    }
    if (!scarecrow) {
      await delay(800);
      continue;
    }
    pinnedCombatTargetId = Number(scarecrow.objectId);

    const beforeMonster = findMonsterById(nowWorld, scarecrow.objectId);
    const beforeQuest = findQuest(nowWorld, questIndex);
    const beforeDropIds = dropObjectIds(nowWorld);
    const beforeExp = Number(nowWorld.playerExperience ?? 0);
    const beforeQuestCurrent = Number(beforeQuest.current ?? 0);
    const beforePercent = monsterHpPercent(beforeMonster);
    const reachedScarecrow = await walkTo(scarecrow, movement, 1, activeCombatTargetName, {
      allowTargetGone: true,
      allowIncomplete: true,
      maxSteps: 24,
    });
    if (!reachedScarecrow) {
      await delay(250);
      continue;
    }
    const attackCheckpoint = sequence;
    send({ type: "attack", objectId: Number(scarecrow.objectId) });
    attackCount += 1;
    traceStage("quest2-attack", {
      attackCount,
      objectId: Number(scarecrow.objectId),
      beforePercent,
      position: summarizeWorld(nowWorld).position,
    });
    attackedMonsterIds.add(String(scarecrow.objectId));

    const progress = await waitForOrNull(
      () => {
        throwForGatewayErrorAfter(attackCheckpoint, `attack ${activeCombatTargetName} ${scarecrow.objectId}`);
        const latest = currentWorld() ?? nowWorld;
        const packetProgress = targetCombatPacketProgress(
          scarecrow.objectId,
          attackCheckpoint,
          beforePercent,
        );
        if (packetProgress) {
          if (
            stopAfterCombatDamaged &&
            packetProgress.reason === "monster_health" &&
            Number.isFinite(packetProgress.hpPercent) &&
            packetProgress.hpPercent > 0 &&
            packetProgress.hpPercent < 100
          ) {
            return { world: latest, ...packetProgress, stopAfterCombatDamaged: true };
          }
          return { world: latest, ...packetProgress };
        }

        const afterDropIds = dropObjectIds(latest);
        if ([...afterDropIds].some((objectId) => !beforeDropIds.has(objectId))) {
          return { world: latest, reason: "drop_spawned" };
        }

        if (!latestSnapshot || latestSnapshot.sequence <= attackCheckpoint) return null;
        const activeQuest = findQuest(latest, questIndex);
        if (activeQuest && activeQuest.stage === "readyToTurnIn") {
          return { world: latest, reason: "quest_ready" };
        }

        const activeMonster = findMonsterById(latest, scarecrow.objectId);
        if (!activeMonster || !isMonsterAlive(activeMonster)) {
          return { world: latest, reason: "monster_died" };
        }

        const activeExp = Number(latest.playerExperience ?? 0);
        const latestQuest = findQuest(latest, questIndex);
        if (latestQuest && Number.isInteger(latestQuest.current) && Number(latestQuest.current) > beforeQuestCurrent) {
          return { world: latest, reason: "quest_progress" };
        }
        if (activeExp > beforeExp) {
          return { world: latest, reason: "quest_reward" };
        }
        const beforeHp = monsterHp(beforeMonster);
        const afterHp = monsterHp(activeMonster);
        if (beforeHp !== null && afterHp !== null && afterHp < beforeHp) {
          return { world: latest, reason: "monster_damaged" };
        }
        const afterPercent = monsterHpPercent(activeMonster);
        if (beforePercent !== null && afterPercent !== null && afterPercent < beforePercent) {
          return { world: latest, reason: "monster_health", hpPercent: afterPercent };
        }

        return null;
      },
      `${activeCombatTargetName} attack progress (${attackCount})`,
      5000,
    );
    if (!progress) {
      traceStage("quest2-attack-retry", {
        attackCount,
        objectId: Number(scarecrow.objectId),
      });
      await delay(650);
      continue;
    }
    if (progress?.stopAfterCombatDamaged) {
      traceStage("quest2-combat-damaged", {
        attackCount,
        objectId: Number(scarecrow.objectId),
        targetName: activeCombatTargetName,
        hpPercent: progress.hpPercent,
      });
      return {
        world: progress.world ?? currentWorld() ?? nowWorld,
        stopAfter: "combat-damaged",
        evidence: {
          hpPercent: progress.hpPercent,
          monsterObjectId: Number(scarecrow.objectId),
          monsterName: activeCombatTargetName,
          stopAfterCombatDamaged: true,
          stopAfterPacketSequence: progress.packetSequence,
          packetsAfterDamage: [
            ...messages
              .filter(
                ({ sequence: packetSequence, message }) =>
                  packetSequence >= progress.packetSequence &&
                  message.type === "packet" &&
                  message.packet === "ObjectHealth" &&
                  Number(packetObjectId(message.payload)) === Number(scarecrow.objectId),
              )
              .slice(-8)
              .map(({ sequence: packetSequence, message }) => ({
                sequence: packetSequence,
                objectId: Number(scarecrow.objectId),
                hpPercent: packetHealthPercent(message.payload),
                payload: compactRecord(message.payload, ["percent", "objectId"]),
              })),
      ],
        },
        report: {
          questIndex,
          attacks: attackCount,
          stopAfterCombatDamaged: true,
        },
      };
    }
    nowWorld = progress?.world ?? currentWorld() ?? nowWorld;
    const latestMonster = findMonsterById(nowWorld, scarecrow.objectId);
    if (progress?.killConfirmed || (isMonsterAlive(beforeMonster) && !isMonsterAlive(latestMonster))) {
      confirmedKillIds.add(String(scarecrow.objectId));
      pinnedCombatTargetId = null;
    }
    collectConfirmedCombatKills(attackedMonsterIds, combatCheckpoint, confirmedKillIds);
    monsterKills = confirmedKillIds.size;

    const pickup = await collectNearbyGroundDrops(nowWorld, movement);
    pickupAttempts += pickup.attempts;
    pickedDropIds.push(...pickup.pickedIds);
    for (const objectId of pickup.seenIds) seenDropIds.add(String(objectId));
    nowWorld = pickup.world;
    directQuestItemGain =
      directQuestItemGain ?? findGainedItemAfter("GingerTea", 1112, combatCheckpoint);
    await delay(650);
  }

  collectConfirmedCombatKills(attackedMonsterIds, combatCheckpoint, confirmedKillIds);
  monsterKills = confirmedKillIds.size;

  assert(readyToTurnIn, "Quest2 did not reach ReadyToTurnIn during native combat timeout");
  if (!resumedReadyToTurnIn) {
    assert(
      pickedDropIds.length > 0 || directQuestItemGain !== null,
      `Quest2 combat produced neither a real ground pickup nor the canonical direct Q-item gain (attacks=${attackCount}, kills=${monsterKills}, seen=${[
        ...seenDropIds,
      ].join(",") || "none"})`,
    );
  }

  const questReadyWorld = currentWorld() ?? nowWorld;
  const readyQuest = findQuest(questReadyWorld, questIndex);
  assert(readyQuest && readyQuest.stage === "readyToTurnIn", "Quest2 was not ready to turn in after combat");
  assert(
    questProgressSatisfied(readyQuest) || itemQuantity(questReadyWorld, "GingerTea", "quest") >= 1,
    "Quest2 reached ReadyToTurnIn without authoritative GingerTea progress",
  );
  if (resumedReadyToTurnIn) {
    assert(
      itemQuantity(questReadyWorld, "GingerTea") >= 1,
      "resumed Quest2 was ReadyToTurnIn but its authoritative GingerTea was missing",
    );
    traceStage("quest2-resumed-ready", {
      position: summarizeWorld(questReadyWorld).position,
      gingerTea: itemQuantity(questReadyWorld, "GingerTea"),
    });
  }

  nowWorld = questReadyWorld;
  const postDropsExp = Number(nowWorld.playerExperience ?? 0);
  if (!resumedReadyToTurnIn) {
    assert(
      monsterKills > 0 && postDropsExp > preCombatExp,
      `Quest2 combat did not prove a rewarded kill (kills=${monsterKills}, EXP ${preCombatExp} -> ${postDropsExp})`,
    );
  }

  const jane = await walkAdjacentToNpc(3, "Assistant Jane", movement);
  checkpoint = sequence;
  send({ type: "interact", objectId: jane.objectId });
  const beforeTurnInWorld = await waitForWorld(
    (world) => Number(world.activeNpcDialog?.npcObjectId) === jane.objectId,
    "Jane NPC dialog for Quest2 finish",
    checkpoint,
  );
  const beforeTurnInExp = Number(beforeTurnInWorld.playerExperience ?? 0);
  const beforeTurnInGold = Number(beforeTurnInWorld.gold ?? 0);
  const beforePendant = itemQuantity(beforeTurnInWorld, "GoldenPendant");
  const beforeRing = itemQuantity(beforeTurnInWorld, "CopperRing");
  checkpoint = sequence;
  send({ type: "finishQuest", questIndex, selectedItemIndex: -1 });
  const completePacket = await waitForPacket("CompleteQuest", checkpoint);
  assert(
    Array.isArray(completePacket.payload?.completedQuests) &&
      completePacket.payload.completedQuests.includes(questIndex),
    "CompleteQuest did not include The Crafts' Request",
  );
  const completedWorld = await waitForWorld(
    (world) => {
      const quest = findQuest(world, questIndex);
      const questCompleted = !quest || quest.stage === "completed";
      const rewardsSettled =
        Number(world.playerExperience ?? 0) === beforeTurnInExp + 30 &&
        Number(world.gold ?? 0) === beforeTurnInGold + 200 &&
        itemQuantity(world, "GoldenPendant") >= beforePendant + 1 &&
        itemQuantity(world, "CopperRing") >= beforeRing + 1;
      return questCompleted && rewardsSettled;
    },
    "completed Quest2 reward-settled snapshot",
    checkpoint,
  );
  traceStage("quest2-completed", {
    attacks: attackCount,
    monsterKills,
    pickedDrops: pickedDropIds.length,
  });

  const postExp = Number(completedWorld.playerExperience ?? 0);
  const postGold = Number(completedWorld.gold ?? 0);
  const postInventory = inventorySignature(completedWorld);
  assert(postExp - beforeTurnInExp === 30, `Quest2 turn-in EXP delta was ${postExp - beforeTurnInExp}, expected 30`);
  assert(postGold - beforeTurnInGold === 200, `Quest2 turn-in gold delta was ${postGold - beforeTurnInGold}, expected 200`);
  assert(
    itemQuantity(completedWorld, "GoldenPendant") >= beforePendant + 1,
    "Quest2 did not grant GoldenPendant",
  );
  assert(itemQuantity(completedWorld, "CopperRing") >= beforeRing + 1, "Quest2 did not grant CopperRing");

  return {
    world: completedWorld,
    report: {
      questIndex,
      preCombat: {
        exp: preCombatExp,
        gold: preCombatGold,
        inventory: preCombatInventory,
      },
      preCombatExp: preCombatExp,
      preCombatGold: preCombatGold,
      combat: {
        resumedReadyToTurnIn,
        attacks: attackCount,
        monsterKills,
        directQuestItemGain: directQuestItemGain
          ? {
              sequence: directQuestItemGain.sequence,
              item: compactRecord(directQuestItemGain.message.payload?.item, [
                "itemIndex",
                "item_index",
                "name",
                "count",
                "unique_id",
              ]),
            }
          : null,
        groundPickupsAttempted: pickupAttempts,
        groundPickupsSucceeded: pickedDropIds.length,
        pickedDropIds,
        seenDropIds: [...seenDropIds],
      },
      completePacket: {
        completedQuests: completePacket.payload?.completedQuests,
      },
      completion: {
        readyStage: readyQuest?.stage ?? null,
        expGain: postExp - preCombatExp,
        goldGain: postGold - preCombatGold,
        inventoryDelta: `${preCombatInventory} -> ${postInventory}`,
      },
      finalExperience: postExp,
      finalGold: postGold,
    },
  };
}

async function walkTo(target, movement, stopDistance, label = "target", options = {}) {
  const maxSteps = options.maxSteps ?? 220;
  const distanceLabel = Number.isFinite(stopDistance) ? stopDistance : 1;
  const targetLabel = label && typeof label === "string" ? label : String(label);
  const visitCounts = new Map();
  traceStage("walk-begin", { target: targetLabel, x: target?.x ?? null, y: target?.y ?? null });
  for (let step = 0; step < maxSteps; step += 1) {
    assertWithinOverallDeadline(`walking to ${targetLabel}`);
    const world = await ensurePlayerAlive(currentWorld(), `walking to ${targetLabel}`);
    const player = playerEntity(world);
    assert(player, `player disappeared while walking to ${targetLabel}`);
    const liveTarget = resolveWalkTarget(target, world);
    if (!liveTarget) {
      if (options.allowTargetGone) return false;
      throw new Error(`${targetLabel} disappeared while walking`);
    }
    if (tileDistance(player, liveTarget) <= distanceLabel) {
      traceStage("walk-complete", { target: targetLabel, step, x: player.x, y: player.y });
      return true;
    }
    const currentKey = `${Number(player.x)},${Number(player.y)}`;
    visitCounts.set(currentKey, (visitCounts.get(currentKey) ?? 0) + 1);
    const preferred = [...walkDirectionDeltas.entries()]
      .map(([direction, delta], order) => {
        const next = { x: Number(player.x) + delta.x, y: Number(player.y) + delta.y };
        const nextKey = `${next.x},${next.y}`;
        return {
          direction,
          order,
          score: tileDistance(next, liveTarget) + (visitCounts.get(nextKey) ?? 0) * 4,
        };
      })
      .sort((left, right) => left.score - right.score || left.order - right.order)
      .map((candidate) => candidate.direction);

    let moved = false;
    for (const direction of preferred) {
      const before = { x: player.x, y: player.y };
      send({ type: "walk", direction });
      const next = await waitForPositionChange(before, 2_000);
      if (!next) continue;
      movement.push({ direction, from: before, to: { x: next.x, y: next.y } });
      await delay(250);
      moved = true;
      break;
    }
    if (!moved) {
      const recovered = await ensurePlayerAlive(
        currentWorld(),
        `recovering after blocked movement toward ${targetLabel}`,
      );
      const recoveredPlayer = playerEntity(recovered);
      if (
        recoveredPlayer &&
        (Number(recoveredPlayer.x) !== Number(player.x) || Number(recoveredPlayer.y) !== Number(player.y))
      ) {
        traceStage("walk-resumed-after-revive", {
          target: targetLabel,
          x: recoveredPlayer.x,
          y: recoveredPlayer.y,
        });
        continue;
      }
      if (options.allowIncomplete) {
        traceStage("walk-reselect-blocked", {
          target: targetLabel,
          x: player.x,
          y: player.y,
        });
        return false;
      }
      throw new Error(
        `all legal walk intents were rejected while heading for ${targetLabel} from (${player.x},${player.y})`,
      );
    }
  }
  if (options.allowIncomplete) {
    const player = playerEntity(currentWorld());
    traceStage("walk-reselect-budget", {
      target: targetLabel,
      maxSteps,
      x: player?.x ?? null,
      y: player?.y ?? null,
    });
    return false;
  }
  throw new Error(`walk to ${targetLabel} exceeded ${maxSteps} authoritative steps`);
}

function resolveWalkTarget(target, world) {
  const key = zoneObjectKey(target?.objectId);
  if (!key) return target;
  if (zoneTombstones.has(key) && target?.kind !== "npc") return null;
  return (
    (Array.isArray(world?.entities)
      ? world.entities.find((entity) => zoneObjectKey(entity?.objectId) === key)
      : null) ??
    findGroundDropById(world, target.objectId) ??
    target
  );
}

async function walkAdjacentTo(target, movement) {
  return walkTo(target, movement, 1, target?.name ?? "target");
}

async function walkAdjacentToNpc(objectId, label, movement) {
  const anchor = questNpcAnchors.get(Number(objectId));
  assert(anchor, `no production quest anchor is configured for NPC ${objectId}`);
  const target = findNpc(currentWorld(), objectId) ?? anchor;
  await walkAdjacentTo(target, movement);
  const npc = await waitFor(
    () => findNpc(currentWorld(), objectId),
    `${label} AOI arrival`,
    5_000,
  );
  assert(npc, `${label} (object ${objectId}) was absent after walking to its map position`);
  return npc;
}

async function exerciseOrdinaryGroundItemRoundTrip(initialWorld) {
  const beforeWorld = currentWorld() ?? initialWorld;
  const item = selectOrdinaryDropProofItem(beforeWorld);
  assert(item, "Quest2 rewards did not contain a normal droppable inventory item");

  const uniqueId = Number(item.uniqueId);
  const beforeQuantity = itemQuantityByUniqueId(beforeWorld, uniqueId);
  const beforeDropIds = dropObjectIds(beforeWorld);
  const dropCheckpoint = sequence;
  send({
    type: "dropItem",
    key: String(item.key ?? ""),
    uniqueId,
    count: 1,
    heroInventory: false,
  });

  const dropped = await waitFor(
    () => {
      throwForGatewayErrorAfter(dropCheckpoint, `drop ordinary item ${uniqueId}`);
      const ackEntry = messages.find(
        (entry) =>
          entry.sequence > dropCheckpoint &&
          entry.message.type === "packet" &&
          entry.message.packet === "DropItem" &&
          Number(entry.message.payload?.uniqueId) === uniqueId,
      );
      if (ackEntry?.message.payload?.success === false) {
        throw new Error(`DropItem rejected for authoritative uniqueId ${uniqueId}`);
      }
      if (ackEntry?.message.payload?.success !== true) return null;
      const world = currentWorld();
      if (!world) return null;
      const drop = collectGroundDropsFromSnapshot(world).find(
        (candidate) =>
          !beforeDropIds.has(String(candidate?.objectId)) &&
          String(candidate?.name ?? "").toLowerCase() === String(item.name ?? "").toLowerCase(),
      );
      return drop ? { world, drop, ackEntry } : null;
    },
    `ordinary item ${uniqueId} appears as ObjectItem`,
  );

  const movement = [];
  const pickupSequence = sequence;
  const pickup = await collectNearbyGroundDrops(dropped.world, movement, {
    objectIds: new Set([String(dropped.drop.objectId)]),
    expectedUniqueId: uniqueId,
  });
  assert(
    pickup.pickedIds.map(Number).includes(Number(dropped.drop.objectId)),
    `ordinary ground object ${dropped.drop.objectId} was not picked up`,
  );
  const afterWorld = pickup.world;
  const gainedEntry = gainedItemMessages.find(
    (entry) =>
      entry.sequence > pickupSequence &&
      (Number(entry.message.payload?.item?.uniqueId) === uniqueId ||
        Number(entry.message.payload?.item?.unique_id) === uniqueId),
  );
  assert(gainedEntry, `ordinary pickup omitted GainedItem for uniqueId ${uniqueId}`);
  const removalEntry = findZonePacketAfter(
    new Set(["ObjectRemove", "ObjectHide"]),
    dropped.drop.objectId,
    pickupSequence,
  );
  assert(removalEntry, `ordinary pickup omitted ObjectRemove for ${dropped.drop.objectId}`);

  traceStage("ordinary-ground-item-round-trip", {
    uniqueId,
    name: item.name,
    objectId: Number(dropped.drop.objectId),
    movementSteps: movement.length,
  });
  return {
    world: afterWorld,
    report: {
      item: compactRecord(item, ["uniqueId", "key", "name", "container", "quantity"]),
      droppedObject: compactRecord(dropped.drop, ["objectId", "name", "quantity", "x", "y"]),
      dropAckSequence: dropped.ackEntry.sequence,
      dropAckSuccess: true,
      gainedItemSequence: gainedEntry.sequence,
      gainedUniqueId: uniqueId,
      removalSequence: removalEntry.sequence,
      uniqueId,
      quantityBefore: beforeQuantity,
      expectedQuantityAfterDrop: beforeQuantity - 1,
      persistedQuantityAfterRelogin: null,
      movementSteps: movement.length,
    },
  };
}

async function collectNearbyGroundDrops(world, movement, options = {}) {
  world = mergeWorldWithZoneOverlay(world) ?? currentWorld() ?? world;
  const player = playerEntity(world);
  const allowedObjectIds = options.objectIds
    ? new Set([...options.objectIds].map((value) => String(value)))
    : null;
  const expectedUniqueId = Number.isSafeInteger(Number(options.expectedUniqueId))
    ? Number(options.expectedUniqueId)
    : null;
  const candidates = collectGroundDropsFromSnapshot(world)
    .filter(
      (drop) =>
        Number.isFinite(Number(drop?.x)) &&
        Number.isFinite(Number(drop?.y)) &&
        (!allowedObjectIds || allowedObjectIds.has(String(drop?.objectId))),
    )
    .sort((left, right) => (player ? tileDistance(player, left) - tileDistance(player, right) : 0))
    .slice(0, 4);
  if (!candidates.length) {
    return { attempts: 0, pickedIds: [], seenIds: [], world };
  }
  const seenIds = [...new Set(candidates.map((entry) => String(entry.objectId)).filter(Boolean))];
  const pickedIds = [];

  for (const drop of candidates) {
    const dropId = Number(drop.objectId);
    if (!Number.isInteger(dropId)) continue;
    let dropWorld = currentWorld() ?? world;
    const currentDrop = findGroundDropById(dropWorld, dropId);
    if (!currentDrop) continue;
    const reachedDrop = await walkTo(currentDrop, movement, 0, `ground drop ${dropId}`, {
      allowTargetGone: true,
    });
    if (!reachedDrop) continue;
    dropWorld = currentWorld() ?? dropWorld;
    if (!findGroundDropById(dropWorld, dropId)) continue;
    const beforeInventory = inventorySignature(dropWorld);
    const beforeGold = Number(dropWorld.gold ?? 0);
    const pickupCheckpoint = sequence;
    send({ type: "pickUp", objectId: dropId });
    const after = await waitFor(
      () => {
        throwForGatewayErrorAfter(pickupCheckpoint, `pick up ground object ${dropId}`);
        const next = currentWorld();
        if (!next) return null;
        const currentDrops = dropObjectIds(next);
        const currentInventory = inventorySignature(next);
        const currentGold = Number(next.gold ?? 0);
        const removalPacket = findZonePacketAfter(
          new Set(["ObjectRemove", "ObjectHide"]),
          dropId,
          pickupCheckpoint,
        );
        const gainPacket = findZonePacketAfter(
          new Set(["GainedItem", "GainedGold"]),
          null,
          pickupCheckpoint,
        );
        const removed = Boolean(removalPacket) && !currentDrops.has(String(dropId));
        if (!removed) return null;
        if (expectedUniqueId !== null) {
          const exactGain = gainedItemMessages.find(
            (entry) =>
              entry.sequence > pickupCheckpoint &&
              (Number(entry.message.payload?.item?.uniqueId) === expectedUniqueId ||
                Number(entry.message.payload?.item?.unique_id) === expectedUniqueId),
          );
          return exactGain
            ? { world: next, removalPacket, gainPacket: exactGain }
            : null;
        }
        if (!latestSnapshot || latestSnapshot.sequence <= pickupCheckpoint) return null;
        if (currentInventory !== beforeInventory || currentGold !== beforeGold) {
          return { world: next, removalPacket, gainPacket };
        }
        return null;
      },
      `pickup drop ${dropId}`,
      4000,
    );
    if (after) {
      pickedIds.push(dropId);
    }
  }

  return {
    attempts: candidates.length,
    pickedIds,
    seenIds,
    world: currentWorld() ?? world,
  };
}

async function waitForPositionChange(before, durationMs) {
  const startedAt = Date.now();
  while (Date.now() - startedAt < durationMs) {
    const player = playerEntity(currentWorld());
    if (player && (player.x !== before.x || player.y !== before.y)) return player;
    await delay(20);
  }
  return null;
}

async function waitForWorld(predicate, label, afterSequence) {
  return waitFor(() => {
    if (!latestSnapshot || latestSnapshot.sequence <= afterSequence) return null;
    const world = currentWorld();
    return predicate(world) ? world : null;
  }, label);
}

function playerEntity(world) {
  world = mergeWorldWithZoneOverlay(world) ?? world;
  if (!world || !Array.isArray(world.entities)) return null;
  const player =
    world.entities.find((entity) => String(entity?.objectId) === String(world.playerObjectId)) ??
    world.entities.find((entity) => entity?.kind === "selfPlayer") ??
    null;
  if (!player || !authoritativePlayerLocation) return player;
  return { ...player, ...authoritativePlayerLocation };
}

function assertPlayerAlive(world, context) {
  const player = playerEntity(world);
  const exactHp = Number(world?.playerHp);
  const hpPercent = monsterHpPercent(player);
  const dead =
    (Number.isFinite(exactHp) && exactHp <= 0) ||
    hpPercent === 0 ||
    player?.dead === true ||
    player?.isDead === true;
  assert(
    !dead,
    `player died while ${context} (objectId=${world?.playerObjectId ?? "unknown"}, HP=${
      Number.isFinite(exactHp) ? exactHp : "unknown"
    }, hpPercent=${hpPercent ?? "unknown"}, position=${player ? `(${player.x},${player.y})` : "unknown"})`,
  );
}

async function ensurePlayerAlive(world, context = "waiting for revival window") {
  world = mergeWorldWithZoneOverlay(world) ?? world;
  if (!world) return world;
  const snapshot = world;
  const player = playerEntity(snapshot);
  const playerMaxHp = Number(snapshot?.playerMaxHp);
  const playerHp = Number(snapshot?.playerHp);
  const playerPercent = Number.isFinite(playerHp) ? (playerHp / (Number.isFinite(playerMaxHp) ? playerMaxHp : 1)) * 100 : null;
  const overlayPercent = monsterHpPercent(player);
  const isDead =
    playerMaxHp > 0 &&
    (player?.dead === true ||
      player?.isDead === true ||
      Number.isFinite(playerHp) && playerHp <= 0 ||
      Number.isFinite(playerPercent) && playerPercent <= 0 ||
      Number.isFinite(overlayPercent) && overlayPercent <= 0);

  if (!isDead) {
    return snapshot;
  }
  assert(
    playerMaxHp > 0,
    `cannot recover player while ${context}: max_hp ${Number.isFinite(playerMaxHp) ? playerMaxHp : "missing"} is not positive`,
  );

  const playerObjectId = Number(snapshot.playerObjectId);
  const reviveCheckpoint = sequence;
  send({ type: "townRevive" });

  const revivedSnapshot = await waitFor(
    () => {
      throwForGatewayErrorAfter(
        reviveCheckpoint,
        `townRevive while ${context}`,
      );
      let reviveConfirmed = false;
      let userLocationConfirmed = false;
      for (const entry of lifecycleMessages) {
        if (entry.sequence <= reviveCheckpoint || entry.message.type !== "packet") {
          continue;
        }
        if (entry.message.packet === "UserLocation") {
          // S.UserLocation is owner-only and intentionally carries x/y/direction
          // without an objectId. If a future envelope includes one, still
          // validate that it belongs to the active player.
          const locationObjectId = packetObjectId(entry.message.payload);
          if (
            !Number.isFinite(playerObjectId) ||
            !Number.isFinite(locationObjectId) ||
            locationObjectId === playerObjectId
          ) {
            userLocationConfirmed = true;
          }
        }
        if (entry.message.packet === "Revived") {
          const revivedObjectId = packetObjectId(entry.message.payload);
          if (!Number.isFinite(playerObjectId) || !Number.isFinite(revivedObjectId) || revivedObjectId === playerObjectId) {
            reviveConfirmed = true;
          }
          continue;
        }
        if (entry.message.packet === "ObjectRevived") {
          const revivedObjectId = packetObjectId(entry.message.payload);
          if (!Number.isFinite(playerObjectId) || !Number.isFinite(revivedObjectId) || revivedObjectId === playerObjectId) {
            reviveConfirmed = true;
          }
        }
      }
      const current = currentWorld();
      if (!current || !latestSnapshot || latestSnapshot.sequence <= reviveCheckpoint) return null;
      if (!reviveConfirmed || !userLocationConfirmed) return null;
      const currentHp = Number(current.playerHp);
      const currentMaxHp = Number(current.playerMaxHp);
      const currentPlayer = playerEntity(current);
      const currentPercent = monsterHpPercent(currentPlayer);
      const hasPositiveHp =
        Number.isFinite(currentHp)
          ? currentHp > 0
          : Number.isFinite(currentPercent)
            ? currentPercent > 0
            : false;
      if (!hasPositiveHp || (Number.isFinite(currentMaxHp) && currentMaxHp <= 0)) {
        return null;
      }
      return { world: current, evidence: { reviveConfirmed, userLocationConfirmed } };
    },
    `townRevive confirmation (${context})`,
    8_000,
  );
  return revivedSnapshot?.world ?? snapshot;
}

function findNpc(world, objectId) {
  world = mergeWorldWithZoneOverlay(world) ?? world;
  return (Array.isArray(world?.entities) ? world.entities : []).find(
    (entity) => entity?.kind === "npc" && Number(entity?.objectId) === Number(objectId),
  );
}

function findQuest(world, questIndex) {
  return (Array.isArray(world?.questLog) ? world.questLog : []).find(
    (quest) => Number(quest?.questId) === Number(questIndex),
  );
}

function collectGroundDropsFromSnapshot(world) {
  world = mergeWorldWithZoneOverlay(world) ?? world;
  return Array.isArray(world?.groundDrops) ? world.groundDrops : [];
}

function findGroundDropById(world, objectId) {
  return collectGroundDropsFromSnapshot(world).find(
    (drop) => Number(drop?.objectId) === Number(objectId),
  );
}

function dropObjectIds(worldOrDrops) {
  const groundDrops = Array.isArray(worldOrDrops)
    ? worldOrDrops
    : collectGroundDropsFromSnapshot(worldOrDrops);
  return new Set(
    groundDrops.map((drop) => String(drop?.objectId)).filter((id) => id && id !== "undefined"),
  );
}

function inventorySignature(world) {
  const items = Array.isArray(world?.inventoryItems) ? world.inventoryItems : [];
  return JSON.stringify(
    items
      .map((item) => ({
        container: item?.container ?? null,
        key: item?.key ?? null,
        name: item?.name ?? null,
        quantity: Number(item?.quantity ?? 0),
      }))
      .sort((left, right) =>
        `${left.container ?? ""}|${left.key ?? ""}|${left.name ?? ""}`.localeCompare(
          `${right.container ?? ""}|${right.key ?? ""}|${right.name ?? ""}`,
        ),
      ),
  );
}

function selectOrdinaryDropProofItem(world) {
  const items = (Array.isArray(world?.inventoryItems) ? world.inventoryItems : []).filter(
    (item) =>
      Number.isSafeInteger(Number(item?.uniqueId)) &&
      Number(item.uniqueId) > 0 &&
      Number(item?.quantity ?? 0) > 0 &&
      String(item?.container ?? "").toLowerCase() !== "quest",
  );
  return (
    items.find((item) => String(item?.name ?? "").toLowerCase() === "copperring") ??
    items.find((item) => String(item?.name ?? "").toLowerCase() === "goldenpendant") ??
    null
  );
}

function itemQuantityByUniqueId(world, uniqueId) {
  return (Array.isArray(world?.inventoryItems) ? world.inventoryItems : [])
    .filter((item) => Number(item?.uniqueId) === Number(uniqueId))
    .reduce((sum, item) => sum + Number(item?.quantity ?? 0), 0);
}

function summarizeInventory(world) {
  return (Array.isArray(world?.inventoryItems) ? world.inventoryItems : []).map((item) => ({
    container: item?.container ?? null,
    key: item?.key ?? null,
    name: item?.name ?? null,
    quantity: Number(item?.quantity ?? 0),
  }));
}

function itemQuantity(world, itemName, container = null) {
  const expectedName = String(itemName).toLowerCase();
  const expectedContainer = container === null ? null : String(container).toLowerCase();
  return (Array.isArray(world?.inventoryItems) ? world.inventoryItems : [])
    .filter((item) => {
      const name = String(item?.name ?? item?.key ?? "").toLowerCase();
      const itemContainer = String(item?.container ?? "").toLowerCase();
      return name === expectedName && (expectedContainer === null || itemContainer === expectedContainer);
    })
    .reduce((sum, item) => sum + Number(item?.quantity ?? 0), 0);
}

function questProgressSatisfied(quest) {
  const current = Number(quest?.current);
  const required = Number(quest?.required);
  return (
    quest?.stage === "readyToTurnIn" ||
    (Number.isFinite(current) && Number.isFinite(required) && required > 0 && current >= required)
  );
}

function verifyPersistence(firstWorld, secondWorld, { exerciseQuest, exerciseCombat }) {
  assert(
    Number(secondWorld.playerExperience ?? 0) === Number(firstWorld.playerExperience ?? 0),
    "player experience did not persist after re-login",
  );
  assert(Number(secondWorld.gold ?? 0) === Number(firstWorld.gold ?? 0), "player gold did not persist after re-login");
  assert(
    inventorySignature(secondWorld) === inventorySignature(firstWorld),
    "inventory did not persist after re-login",
  );
  if (exerciseQuest) {
    assert(findQuest(secondWorld, 1)?.stage === "completed", "Quest1 completion did not persist after re-login");
  }
  if (exerciseCombat) {
    assert(findQuest(secondWorld, 2)?.stage === "completed", "Quest2 completion did not persist after re-login");
    assert(itemQuantity(secondWorld, "GoldenPendant") >= 1, "GoldenPendant did not persist after re-login");
    assert(itemQuantity(secondWorld, "CopperRing") >= 1, "CopperRing did not persist after re-login");
  }
}

function findGainedItemAfter(itemName, itemIndex, afterSequence) {
  const expectedName = String(itemName).toLowerCase();
  const expectedIndex = Number(itemIndex);
  return (
    gainedItemMessages.find(({ sequence: packetSequence, message }) => {
      if (packetSequence <= afterSequence) return false;
      const item = message.payload?.item;
      const name = String(item?.name ?? "").toLowerCase();
      const index = Number(item?.itemIndex ?? item?.item_index);
      return name === expectedName || (Number.isFinite(expectedIndex) && index === expectedIndex);
    }) ?? null
  );
}

function findZonePacketAfter(packetNames, objectId, afterSequence) {
  const objectKey = objectId === null ? null : zoneObjectKey(objectId);
  return (
    messages.find(({ sequence: packetSequence, message }) => {
      if (packetSequence <= afterSequence || message.type !== "packet" || !packetNames.has(message.packet)) {
        return false;
      }
      return objectKey === null || zoneObjectKey(message.payload?.objectId) === objectKey;
    }) ?? null
  );
}

function targetCombatPacketProgress(objectId, afterSequence, beforePercent) {
  const objectKey = zoneObjectKey(objectId);
  if (!objectKey) return null;
  for (const entry of messages) {
    if (entry.sequence <= afterSequence || entry.message.type !== "packet") continue;
    if (zoneObjectKey(entry.message.payload?.objectId) !== objectKey) continue;
    const packet = entry.message.packet;
    if (packet === "ObjectHealth") {
      const hpPercent = packetHealthPercent(entry.message.payload);
      if (hpPercent === null) continue;
      if (
        hpPercent > 0 &&
        (beforePercent === null ? hpPercent >= 100 : hpPercent >= beforePercent)
      ) {
        continue;
      }
      return {
        reason: hpPercent <= 0 ? "monster_died" : "monster_health",
        hpPercent,
        damaged: beforePercent === null ? hpPercent < 100 : hpPercent < beforePercent,
        killConfirmed: hpPercent <= 0,
        packetSequence: entry.sequence,
      };
    }
    if (packet === "ObjectDied") {
      return { reason: "monster_died", killConfirmed: true, packetSequence: entry.sequence };
    }
    if (packet === "ObjectRemove" || packet === "ObjectHide") {
      return { reason: "monster_removed", killConfirmed: false, packetSequence: entry.sequence };
    }
    if (
      (packet === "ObjectMonster" || packet === "NewMonsterInfo") &&
      entry.message.payload?.dead === true
    ) {
      return { reason: "monster_died", killConfirmed: true, packetSequence: entry.sequence };
    }
  }
  return null;
}

function collectConfirmedCombatKills(attackedMonsterIds, afterSequence, confirmedKillIds) {
  for (const [objectKey, evidence] of zoneDeathEvidence) {
    if (attackedMonsterIds.has(objectKey) && evidence.sequence > afterSequence) {
      confirmedKillIds.add(objectKey);
    }
  }
  for (const entry of messages) {
    if (entry.sequence <= afterSequence || entry.message.type !== "packet") continue;
    const objectKey = zoneObjectKey(entry.message.payload?.objectId);
    if (!objectKey || !attackedMonsterIds.has(objectKey)) continue;
    if (entry.message.packet === "ObjectDied") {
      confirmedKillIds.add(objectKey);
      continue;
    }
    if (entry.message.packet === "ObjectHealth" && packetHealthPercent(entry.message.payload) === 0) {
      confirmedKillIds.add(objectKey);
      continue;
    }
    if (
      (entry.message.packet === "ObjectMonster" || entry.message.packet === "NewMonsterInfo") &&
      entry.message.payload?.dead === true
    ) {
      confirmedKillIds.add(objectKey);
    }
  }
}

function isMonster(entity) {
  return entity && (entity.kind === "monster" || entity.kind === "monsterInfo" || entity.kind === "monsterObject");
}

function isMonsterAlive(entity) {
  if (!entity) return false;
  if (entity.dead === true) return false;
  if (entity.isDead === true) return false;
  if (entity.status === "dead") return false;
  const hpPercent = monsterHpPercent(entity);
  if (hpPercent !== null) return hpPercent > 0;
  if (entity.dead === false && Number.isInteger(entity._zoneSpawnSequence)) return true;
  if (typeof entity.hp === "number" && Number.isInteger(entity.hp)) return entity.hp > 0;
  if (typeof entity.health === "number" && Number.isInteger(entity.health)) return entity.health > 0;
  return true;
}

function monsterHp(entity) {
  const candidates = [entity?.hp, entity?.health, entity?.currHp, entity?.currentHp, entity?.vitality];
  for (const candidate of candidates) {
    const hp = Number(candidate);
    if (Number.isFinite(hp)) return hp;
  }
  return null;
}

function monsterHpPercent(entity) {
  const percent = Number(entity?.hpPercent);
  return Number.isFinite(percent) && percent >= 0 && percent <= 100 ? percent : null;
}

function findMonsterById(world, objectId) {
  world = mergeWorldWithZoneOverlay(world) ?? world;
  return (Array.isArray(world?.entities) ? world.entities : []).find((entity) => {
    if (!isMonster(entity)) return false;
    return Number(entity?.objectId) === Number(objectId);
  });
}

function findNearestMonster(world, name) {
  world = mergeWorldWithZoneOverlay(world) ?? world;
  const player = playerEntity(world);
  if (!player) return null;
  const targetName = String(name ?? "").toLowerCase();
  let candidate = null;
  let candidateDistance = Infinity;
  for (const entity of Array.isArray(world?.entities) ? world.entities : []) {
    if (!isMonster(entity) || !isMonsterAlive(entity)) continue;
    if (targetName && String(entity?.name ?? "").toLowerCase() !== targetName) continue;
    const distance = tileDistance(player, entity);
    if (distance < candidateDistance) {
      candidateDistance = distance;
      candidate = entity;
    }
  }
  return candidate;
}

function tileDistance(a, b) {
  return Math.max(Math.abs(Number(a.x) - Number(b.x)), Math.abs(Number(a.y) - Number(b.y)));
}

function summarizeDialog(dialog) {
  if (!dialog) return null;
  return {
    npcObjectId: dialog.npcObjectId ?? null,
    npcName: dialog.npcName ?? null,
    title: dialog.title ?? null,
    body: Array.isArray(dialog.body) ? dialog.body.slice(0, 6) : [],
    links: Array.isArray(dialog.links)
      ? dialog.links.slice(0, 8).map((link) => ({ text: link?.text ?? null, target: link?.target ?? null }))
      : [],
  };
}

function summarizeWorld(payload) {
  const playerObjectId = payload.playerObjectId;
  const entities = Array.isArray(payload.entities) ? payload.entities : [];
  const player = playerEntity(payload);
  return {
    mapFileName: payload.mapFileName ?? null,
    mapTitle: payload.mapTitle ?? null,
    playerObjectId: playerObjectId ?? null,
    playerName: player?.name ?? null,
    position: player ? { x: player.x ?? null, y: player.y ?? null } : null,
    hp: payload.playerHp ?? null,
    maxHp: payload.playerMaxHp ?? null,
    gold: payload.gold ?? null,
    inventoryCount: Array.isArray(payload.inventoryItems) ? payload.inventoryItems.length : 0,
    equipmentSlots: Array.isArray(payload.equipmentItems)
      ? payload.equipmentItems.map((item) => item?.slot ?? null)
      : [],
    nearbyNpcs: entities
      .filter((entity) => entity?.kind === "npc")
      .slice(0, 20)
      .map((entity) => ({
        objectId: entity.objectId ?? null,
        name: entity.name ?? null,
        x: entity.x ?? null,
        y: entity.y ?? null,
        questIds: Array.isArray(entity.questIds) ? entity.questIds : [],
      })),
    questLog: summarizeList(payload.questLog, [
      "questId",
      "title",
      "stage",
      "current",
      "required",
      "objective",
      "objectives",
      "rewards",
    ]),
    activeNpcDialog: compactRecord(payload.activeNpcDialog),
    groundDrops: summarizeList(payload.groundDrops, ["objectId", "name", "quantity", "x", "y"]),
  };
}

function summarizeQuestPacket(message) {
  const info = message.payload?.info;
  return {
    packet: message.packet,
    payload: compactRecord(message.payload, [
      "id",
      "questId",
      "name",
      "state",
      "taken",
      "completed",
      "current",
      "required",
      "descriptionLines",
      "objectives",
      "rewards",
      "completedQuests",
    ]),
    definition:
      info && typeof info === "object"
        ? {
            index: info.index ?? null,
            npcIndex: info.npc_index ?? info.npcIndex ?? null,
            finishNpcIndex: info.finish_npc_index ?? info.finishNpcIndex ?? null,
            questNeeded: info.quest_needed ?? info.questNeeded ?? null,
            returnDescription: Array.isArray(info.return_description)
              ? info.return_description.slice(0, 5)
              : [],
            completionDescription: Array.isArray(info.completion_description)
              ? info.completion_description.slice(0, 5)
              : [],
          }
        : null,
  };
}

function summarizeList(value, fields, limit = 5) {
  if (!Array.isArray(value)) return { type: value === null ? "null" : typeof value, length: null, samples: [] };
  return {
    type: "array",
    length: value.length,
    samples: value.slice(0, limit).map((entry) => compactRecord(entry, fields)),
  };
}

function compactRecord(value, fields = []) {
  if (!value || typeof value !== "object" || Array.isArray(value)) return value ?? null;
  const keys = Object.keys(value).sort();
  const selected = fields.length > 0 ? fields : keys.slice(0, 20);
  return {
    keys,
    values: Object.fromEntries(
      selected
        .filter((field) => Object.hasOwn(value, field))
        .map((field) => [field, compactValue(value[field])]),
    ),
  };
}

function compactValue(value) {
  if (Array.isArray(value)) return value.slice(0, 5).map((entry) => {
    if (entry && typeof entry === "object") return compactRecord(entry);
    return entry;
  });
  if (value && typeof value === "object") return compactRecord(value);
  return value;
}

function send(command) {
  assert(socket.readyState === WebSocket.OPEN, `socket is not open for ${command.type}`);
  socket.send(JSON.stringify(command));
}

async function waitForPacket(packet, afterSequence) {
  return waitFor(
    () => {
      const reply = messages.find(
        (entry) => entry.sequence > afterSequence && entry.message.type === "packet" && entry.message.packet === packet,
      )?.message;
      if (reply) return reply;
      throwForGatewayErrorAfter(afterSequence, `wait for packet ${packet}`);
      return null;
    },
    `packet ${packet}`,
  );
}

function throwForGatewayErrorAfter(afterSequence, context) {
  const fault = messages.find(
    (entry) => entry.sequence > afterSequence && entry.message.type === "error",
  )?.message;
  if (!fault) return;
  const detail = fault.payload?.message ?? fault.message ?? fault.payload?.reason ?? JSON.stringify(fault.payload ?? fault);
  throw new Error(`${context} rejected by Gateway: ${detail}`);
}

async function waitFor(predicate, label, timeoutOverrideMs = null) {
  const effectiveTimeoutMs = Number.isFinite(timeoutOverrideMs) ? timeoutOverrideMs : timeoutMs;
  const startedAt = Date.now();
  while (Date.now() - startedAt < effectiveTimeoutMs) {
    assertWithinOverallDeadline(label);
    const value = predicate();
    if (value) return value;
    await new Promise((resolve) => setTimeout(resolve, 20));
  }
  throw new Error(`${label} timed out after ${effectiveTimeoutMs} ms`);
}

async function waitForOrNull(predicate, label, timeoutOverrideMs = null) {
  const effectiveTimeoutMs = Number.isFinite(timeoutOverrideMs) ? timeoutOverrideMs : timeoutMs;
  const startedAt = Date.now();
  while (Date.now() - startedAt < effectiveTimeoutMs) {
    assertWithinOverallDeadline(label);
    const value = predicate();
    if (value) return value;
    await delay(20);
  }
  return null;
}

function assertWithinOverallDeadline(context) {
  assert(Date.now() <= overallDeadline, `overall smoke timeout while ${context} (${totalTimeoutMs} ms)`);
}

function traceStage(stage, details = null) {
  if (!progressEnabled) return;
  const suffix = details === null ? "" : ` ${JSON.stringify(details)}`;
  console.error(`[native-smoke] ${stage}${suffix}`);
}

function waitForEvent(target, type, label) {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error(`${label} timed out after ${timeoutMs} ms`)), timeoutMs);
    target.addEventListener(
      type,
      () => {
        clearTimeout(timer);
        resolve();
      },
      { once: true },
    );
    target.addEventListener(
      "error",
      () => {
        clearTimeout(timer);
        reject(new Error(`${label} failed`));
      },
      { once: true },
    );
  });
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function emitReport(report) {
  const evidence = {
    schemaVersion: "mir2-windows-native-flow-evidence-v2",
    generatedAt: new Date().toISOString(),
    producer: "windows-native-protocol-smoke",
    desktopTouched: false,
    ...report,
  };
  const serialized = `${JSON.stringify(evidence, null, 2)}\n`;
  console.log(serialized.trimEnd());
  if (!outputPath) return;

  const resolved = path.resolve(outputPath);
  await fs.mkdir(path.dirname(resolved), { recursive: true });
  const handle = await fs.open(resolved, "wx", 0o600);
  try {
    await handle.writeFile(serialized, "utf8");
    await handle.sync();
  } finally {
    await handle.close();
  }
  const sha256 = crypto.createHash("sha256").update(serialized, "utf8").digest("hex");
  console.error(JSON.stringify({ evidencePath: resolved, sha256, overwrite: false }));
}

function parseCli(argv) {
  const args = {
    gatewayUrl: process.env.MIR2_GATEWAY_WS_URL ?? "ws://127.0.0.1:7110/ws",
    timeoutMs: positiveInteger(process.env.MIR2_NATIVE_SMOKE_TIMEOUT_MS ?? 20_000, "MIR2_NATIVE_SMOKE_TIMEOUT_MS"),
    combatTimeoutMs: positiveInteger(process.env.MIR2_NATIVE_SMOKE_COMBAT_TIMEOUT_MS ?? 600_000, "MIR2_NATIVE_SMOKE_COMBAT_TIMEOUT_MS"),
    totalTimeoutMs: positiveInteger(process.env.MIR2_NATIVE_SMOKE_TOTAL_TIMEOUT_MS ?? 420_000, "MIR2_NATIVE_SMOKE_TOTAL_TIMEOUT_MS"),
    progressEnabled: !/^(0|false|no)$/i.test(process.env.MIR2_NATIVE_SMOKE_PROGRESS ?? "1"),
    outputPath: process.env.MIR2_NATIVE_SMOKE_OUTPUT ?? null,
    allowAccountMutation: false,
    mode: "run",
  };
  const valueFlags = new Map([
    ["--gateway-url", "gatewayUrl"],
    ["--timeout-ms", "timeoutMs"],
    ["--combat-timeout-ms", "combatTimeoutMs"],
    ["--total-timeout-ms", "totalTimeoutMs"],
    ["--output", "outputPath"],
  ]);
  let positionalGateway = null;
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    if (token === "--help" || token === "-h") { args.mode = "help"; continue; }
    if (token === "--self-test") { args.mode = "self-test"; continue; }
    if (token === "--dry-run") { args.mode = "dry-run"; continue; }
    if (token === "--allow-account-mutation") { args.allowAccountMutation = true; continue; }
    const equals = token.indexOf("=");
    const name = equals > 2 ? token.slice(0, equals) : token;
    const key = valueFlags.get(name);
    if (key) {
      const value = equals > 2 ? token.slice(equals + 1) : argv[++index];
      if (value === undefined || value === "" || value.startsWith("--")) throw new Error(`${name} requires a value`);
      args[key] = value;
      continue;
    }
    if (token.startsWith("--")) throw new Error(`unknown argument: ${token}`);
    if (positionalGateway !== null) throw new Error(`unexpected positional argument: ${token}`);
    positionalGateway = token;
  }
  if (positionalGateway !== null) args.gatewayUrl = positionalGateway;
  args.gatewayUrl = validateGatewayUrl(args.gatewayUrl);
  args.timeoutMs = positiveInteger(args.timeoutMs, "--timeout-ms");
  args.combatTimeoutMs = positiveInteger(args.combatTimeoutMs, "--combat-timeout-ms");
  args.totalTimeoutMs = positiveInteger(args.totalTimeoutMs, "--total-timeout-ms");
  if (args.totalTimeoutMs < 60_000) throw new Error("--total-timeout-ms must be at least 60000");
  return args;
}

function validateGatewayUrl(value) {
  let parsed;
  try { parsed = new URL(String(value)); } catch { throw new Error(`gateway URL is invalid: ${value}`); }
  if (!["ws:", "wss:"].includes(parsed.protocol)) throw new Error(`gateway URL must use ws or wss: ${value}`);
  return parsed.toString();
}

function positiveInteger(value, flag) {
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed <= 0) throw new Error(`${flag} must be a positive integer; received ${value}`);
  return parsed;
}

function printCliHelp() {
  console.log(`Usage:
  node apps/game-client/platform-windows/scripts/smoke-native-flow.mjs [gateway-url] [options]

Options:
  --gateway-url URL             Gateway ws(s) URL
  --timeout-ms N                Per-packet timeout (default: 20000)
  --combat-timeout-ms N         Optional combat timeout (default: 120000)
  --total-timeout-ms N          Overall timeout (default: 420000)
  --output PATH                 Create (never overwrite) a durable JSON evidence file
  --dry-run                     Validate only; do not open a socket
  --self-test                   Validate only; do not open a socket
  --allow-account-mutation      Explicitly allow disposable account/character creation

Safety:
  Live mode creates an account and character through the real Gateway and is
  CONFIRM_REQUIRED unless --allow-account-mutation is present. It never controls
  the desktop, deletes a character, changes a password, or sends local fake state.`);
}
