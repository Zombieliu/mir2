#!/usr/bin/env node

import fs from "node:fs/promises";
import path from "node:path";

import {
  BICHON_Q1_Q9_ROUTE,
  QUEST_AGENT_CONTRACT,
  allQ1Q9Completed,
  assessGrindingSourceStall,
  assessQuestCombatResourceStrain,
  chooseImmediateMeleeTarget,
  collisionPathHasImmediateDynamicBlock,
  collisionPathNeedsPerpendicularFrontier,
  collisionPathNeedsStickyDetour,
  continuousCollisionRunAvoidsTransfers,
  dangerousHostileAvoidanceCells,
  denseAdjacentHostileCount,
  duplicateEquippedItemsForSale,
  entityAttackIsRecent,
  equipmentRepairCandidates,
  expandRespawnPatrolFields,
  findCollisionGridPath,
  incidentalTravelThreatIsTrivial,
  missingStarterEquipment,
  nearestActiveHostile,
  nearestGroundDropByName,
  nearestHealthPotionGroundDrop,
  nearestBlockingHostile,
  ordinarySupplyLootForSale,
  offensiveCombatSkillHotkey,
  restorativeSelfSkillHotkey,
  normalizedQuestStage,
  objectiveProgress,
  planHealthPotionPurchase,
  planNextQ1Q9,
  protectedTransfersForNavigation,
  questIsCompleted,
  questState,
  rankCombatTargetsByIsolation,
  rankRespawnFieldsForTravel,
  retreatPointFromHostile,
  respawnCorridorAvoidanceWaypoint,
  respawnTravelAttemptBudget,
  selectBestAvailableEquipmentUpgrade,
  selectProgressingCollisionDetour,
  shouldCaptureGoalFrame,
  shouldFundHealthPotions,
  surplusQuestMaterialsForSale,
  supersededProgressionGearForSale,
  unresolvedCombatResourceStrains,
} from "./policy.mjs";
import {
  buildProgressionEquipmentCandidates,
  chooseGrindingGoal,
  completedQuestCombatCertifications,
  planNextAuthoritativeQuest,
} from "./autonomous-policy.mjs";
import {
  buildClassQuestRoute,
  buildGrindingCatalog,
  buildMapTravelGraph,
  buildProgressionSkillBookCatalog,
  buildSafeSupplyLootCatalog,
  findMapTravelRoute,
  loadCrystalQuestRouteSources,
  minimumStartingGoldForMapTravelEdges,
} from "./route-manifest.mjs";
import {
  classifyBrowserDiagnostics,
  delay,
  isGameplayWebSocketUrl,
  launchBrowser,
  readAgentState,
  renderGameToText,
  stopBrowser,
  targetCombatEvidenceSince,
  waitUntil,
  wsEventFramesSince,
  wsPacketsSince,
} from "./browser-driver.mjs";
import { signalExitCode } from "./supervisor-policy.mjs";

const args = parseArgs(process.argv.slice(2));
const resumeEvidence = args.resumeReport
  ? JSON.parse(await fs.readFile(path.resolve(args.resumeReport), "utf8"))
  : null;
const baseUrl = args.baseUrl ?? process.env.MIR2_WEB_BASE_URL ?? "http://127.0.0.1:3001";
const gatewayWs = args.gatewayWs ?? process.env.MIR2_GATEWAY_WS_URL ?? null;
const account = args.account ?? process.env.MIR2_QUEST_AGENT_ACCOUNT ?? resumeEvidence?.account ?? defaultIdentity("QA");
const password = args.password ?? process.env.MIR2_QUEST_AGENT_PASSWORD ?? "Mir2test1";
const characterName = args.characterName ?? process.env.MIR2_QUEST_AGENT_CHARACTER ?? resumeEvidence?.characterName ?? defaultIdentity("WQ").slice(0, 12);
const createAccount = boolArg(
  args.createAccount ?? process.env.MIR2_QUEST_AGENT_CREATE_ACCOUNT,
  resumeEvidence == null,
);
const headed = boolArg(args.headed ?? process.env.MIR2_CHROME_HEADED, false);
const skipRuntime = boolArg(args.skipRuntime, false);
const maxRuntimeMs = numberArg(args.maxRuntimeMs, 120 * 60_000);
const maxGoals = numberArg(args.maxGoals, 240);
const maxQuestId = numberArg(
  args.maxQuestId ?? process.env.MIR2_QUEST_AGENT_MAX_QUEST_ID,
  9,
);
const targetLevel = numberArg(
  args.targetLevel ?? process.env.MIR2_QUEST_AGENT_TARGET_LEVEL,
  maxQuestId > 9 ? 50 : 1,
);
const className = String(
  args.className ?? process.env.MIR2_QUEST_AGENT_CLASS ?? "Warrior",
);
const extendedRouteEnabled = maxQuestId > 9 || targetLevel > 6;
const runId = args.runId ?? `${new Date().toISOString().replace(/[:.]/g, "-")}-${process.pid}`;
const outputDir = path.resolve(
  args.output ?? path.join(process.cwd(), "output", "quest-agent", runId),
);
const framesDir = path.join(outputDir, "frames");
const runUrl = buildRunUrl(baseUrl, gatewayWs, skipRuntime);
const TRANSIENT_START_GAME_ROUTE_LEASE_MESSAGE =
  "character is already online or route lease is unavailable";
const START_GAME_ROUTE_LEASE_RETRY_MS = 30_000;
let shutdownSignal = null;

const evidence = {
  runId,
  startedAt: Date.now(),
  contract: QUEST_AGENT_CONTRACT,
  route: extendedRouteEnabled
    ? `${className.toLowerCase()}-real-client-1-${targetLevel}-through-q${maxQuestId}`
    : BICHON_Q1_Q9_ROUTE.id,
  account,
  characterName,
  baseUrl,
  gatewayWs: gatewayWs ? redactGatewayUrl(gatewayWs) : null,
  presentationRuntime: skipRuntime ? "react-shell-only" : "react-shell-plus-bevy",
  inputs: [],
  goals: [],
  milestones: [],
  kills: [],
  targetQuarantines: [],
  combatResourceStrains: [],
  combatResourceRecoveries: [],
  grindingSourceStalls: [],
  deaths: 0,
  revives: 0,
  potionUses: 0,
  shopPurchases: [],
  repairs: [],
  lootPickups: [],
  goldPickups: [],
  supplyPickups: [],
  questDefinitions: [],
  inheritedCombatResourceStrains: [
    ...(Array.isArray(resumeEvidence?.inheritedCombatResourceStrains)
      ? resumeEvidence.inheritedCombatResourceStrains
      : []),
    ...(Array.isArray(resumeEvidence?.combatResourceStrains)
      ? resumeEvidence.combatResourceStrains
      : []),
  ].slice(-128).map((record) => ({ ...record })),
  inheritedCombatResourceRecoveries: [
    ...(Array.isArray(resumeEvidence?.inheritedCombatResourceRecoveries)
      ? resumeEvidence.inheritedCombatResourceRecoveries
      : []),
    ...(Array.isArray(resumeEvidence?.combatResourceRecoveries)
      ? resumeEvidence.combatResourceRecoveries
      : []),
    ...(Array.isArray(resumeEvidence?.kills)
      ? resumeEvidence.kills
          .filter((record) => record?.monsterName && Number.isFinite(Number(record?.at)))
          .map((record) => ({
            monsterName: String(record.monsterName),
            at: Number(record.at),
            reason: "confirmed-normal-client-kill",
          }))
      : []),
  ].slice(-128).map((record) => ({ ...record })),
  inheritedGrindingSourceStalls: [
    ...(Array.isArray(resumeEvidence?.inheritedGrindingSourceStalls)
      ? resumeEvidence.inheritedGrindingSourceStalls
      : []),
    ...(Array.isArray(resumeEvidence?.grindingSourceStalls)
      ? resumeEvidence.grindingSourceStalls
      : []),
  ].slice(-128).map((record) => ({ ...record })),
};

let browser = null;
let client = null;
let goalSequence = 0;
let lastPotionUseAt = 0;
let lastPotionRestockAt = 0;
let lastPotionRestockGold = -1;
let equipmentRepairRetryUntil = 0;
const equipmentRepairRouteRetryUntil = new Map();
let potionRestockInFlight = false;
let potionSupplyRecallRequested = false;
let knownHealthPotionUnitPrice = null;
let deerFundingUnavailableUntil = 0;
let supplyFundingShelterUntil = 0;
const fieldGroupCooldownUntil = new Map();
const monsterCooldownUntil = new Map();
const quarantinedMonsterUntil = new Map();
const questMonsterDeaths = new Map();
const questMonsterPreparationLevel = new Map();
const questMonsterResourceStrains = new Map();
const grindingMonsterRiskUntil = new Map();
const grindingMonsterStalls = new Map();
const recordedCombatResourceStrainGoals = new WeakSet();
const groundDropCooldownUntil = new Map();
const navigationDetourByTarget = new Map();
const navigationRejectedCollisionCellUntil = new Map();
const collisionAtlasByMap = new Map();
let collisionRegionCache = null;
let authoritativeRoute = null;
let mapTravelGraph = null;
let grindingCatalog = [];
let progressionSkillBookCatalog = [];
let safeOrdinarySupplyLootCatalog = [];
let nextDiscreteMovementInputAt = 0;
let lastCombatSkillInputAt = 0;
let lastRestorativeSkillInputAt = 0;
const FAILED_APPROACH_COOLDOWN_MS = 15_000;
const FAILED_COMBAT_COOLDOWN_MS = 30_000;
const QUARANTINED_TARGET_COOLDOWN_MS = 120_000;
const COMBAT_PROGRESS_WINDOW_MS = 45_000;
const COMBAT_HARD_DEADLINE_MS = 5 * 60_000;
const STALLED_FIELD_GROUP_COOLDOWN_MS = 300_000;
const STALLED_GRIND_SOURCE_COOLDOWN_MS = 10 * 60_000;
const OPTIONAL_DROP_UNREACHABLE_COOLDOWN_MS = 10_000;
// A shared drop remains private to its originating session for 30 seconds.
// The rendered client deliberately does not expose ownership, so a rejected
// optional pickup is cooled down for that exact public window while other
// visible drops remain eligible.
const OPTIONAL_DROP_REJECTED_COOLDOWN_MS = 30_000;
const STICKY_NAVIGATION_DETOUR_TTL_MS = 90_000;
// A Zone correction is stronger evidence than the static map atlas: dynamic
// occupancy and retained world objects can reject an otherwise-open tile.
// Preserve that observation across the short resource-sensitive navigation
// chunks, then expire it so roaming actors can free the cell naturally.
const REJECTED_COLLISION_CELL_TTL_MS = 30_000;
// Crystal item 658 has a catalog base price of 40 in the active profile.
// Merchant markup is read back from the visible shop before any purchase, but
// less than the base price can never buy even one unit here.
const HEALTH_POTION_CATALOG_BASE_PRICE = 40;
const HEALTH_POTION_DEPARTURE_STOCK = 10;
// Five bottles are working capital, not permission to enter a quest field.
// They normally enable the Deer -> Venison economy while the independent hard
// departure gate below continues to require all ten bottles. A completely
// underfunded character may also make one emergency Deer attempt at >=90% HP;
// the supply-funding loop still refuses potions, aborts below 70% HP, and
// shelters immediately if any non-target monster actually attacks.
const HEALTH_POTION_FUNDING_WORKING_STOCK = 5;
// Full stock is a departure gate while the character is physically in the
// supply area. Once a real quest-field trip has started, keep fighting until
// this reserve is reached instead of commuting hundreds of tiles after every
// single consumed bottle. Five is also the measured severe-strain threshold:
// a character that spends half a full belt in one engagement must have one
// equally expensive fight in reserve while physically withdrawing.
const HEALTH_POTION_FIELD_RESERVE = 5;
// Crystal item 658 restores 30 HP. Budget two extra bottles while recovering
// in case a nearby ordinary field monster lands hits before the character has
// walked clear of the village-edge spawn area.
const HEALTH_POTION_HEAL_AMOUNT = 30;
const HEALTH_POTION_RECOVERY_DAMAGE_BUFFER = 2;
const QUEST_DEPARTURE_HEALTH_RATIO = 0.62;
const HEALTH_POTION_RESTOCK_RETRY_MS = 5_000;
// A supply trip is worthwhile from the early Bichon hunting fields when the
// character cannot leave with the complete HP-drug stock. The trip still
// remains on source-backed maps and uses only collision-aware input.
const HEALTH_POTION_RESTOCK_RADIUS = 180;
// Underfunded characters must never chase an otherwise-valid source across
// Bichon. Keep funding respawn centres inside the village-edge beginner band;
// the normal quest/grind planner remains free to use every other source.
const HEALTH_POTION_FUNDING_FIELD_RADIUS = 64;
const SAFE_FUNDING_MIN_HEALTH_RATIO = 0.70;
const SAFE_FUNDING_READY_HEALTH_RATIO = 0.90;
// GroceryStore has zero authoritative respawns and is connected to the
// Border Village by ordinary movement portals. It is the nearest real-client
// shelter where passive Crystal regeneration cannot be interrupted by mobs.
const SAFE_RECOVERY_MAP_FILE_NAME = "0141";
const SUPPLY_FUNDING_THREAT_SHELTER_MS = 120_000;
// A monster which has not emitted a rendered attack in this interval is an
// occupancy obstacle, not a combat objective. The movement planner will route
// around it; only recent attackers may interrupt a long quest-field journey.
// One movement click may remain in its authoritative settle loop for up to
// twelve seconds. Keep the attack marker longer than that so a monster which
// just hit during the wait is still recognized on the next policy snapshot.
const ACTIVE_TRAVEL_THREAT_WINDOW_MS = 15_000;
// Killing an interrupting monster on top of the older quest corpse makes the
// latter physically unclickable: both rendered hit surfaces occupy the same
// screen pixels and the newer corpse owns the browser hit test. A real player
// avoids that by pulling the attacker away before fighting. Keep enough world
// separation that the resulting corpse cannot cover the harvest target.
const HARVEST_DEFENCE_CORPSE_CLEARANCE = 3;
const HARVEST_OVERLAY_RETRY_LIMIT = 3;
const GRIND_SCREENSHOT_SAMPLE_INTERVAL = 100;
// A central, fully hit-testable monster at this range can be selected through
// the ordinary scene UI and handed to the client's existing locked-attack
// controller. That controller refreshes the moving object by id and performs
// its own authoritative collision-aware approach. Requiring the runner to
// reach distance one before making the same physical click can deadlock in a
// crowded spawn when every local keyboard frontier is occupied.
const CLIENT_LOCKED_ATTACK_CLICK_RADIUS = 4;
// Static atlas routing is cheap once its map chunks are cached and is much
// more reliable than a local frontier around village buildings. Keep only the
// final few interaction tiles on the lightweight local planner.
const GLOBAL_COLLISION_PATH_THRESHOLD = 8;
// Shared Zone walking is authoritative on a 600 ms cadence. A position packet
// can arrive almost immediately after a step, but sending the next discrete
// input at that point is still too early and produces a correction. Leave a
// small scheduling margin so a correction is never misclassified as collision.
const DIRECT_MOVEMENT_SETTLE_MS = 620;
// Slow doubles the authoritative walking cooldown to 1,200ms. The Zone can
// buffer a movement input during the final 300ms, so wait 1,000ms from the
// latest acknowledged position change before issuing an isolated direction
// probe. Without this shared gate, the first legal probe after a pointer or
// keyboard step can be replaced while still cooling down and falsely recorded
// as collision; a later reverse probe then succeeds and creates an A<->B loop.
const DISCRETE_MOVEMENT_INPUT_GUARD_MS = 1_000;

class NavigationUnreachableError extends Error {
  constructor(message) {
    super(message);
    this.name = "NavigationUnreachableError";
  }
}

class NavigationInterruptedByDeathError extends Error {
  constructor(message) {
    super(message);
    this.name = "NavigationInterruptedByDeathError";
  }
}

class NavigationInterruptedByThreatError extends Error {
  constructor(threat) {
    super(
      `adjacent ${String(threat?.name ?? "hostile")} ${String(threat?.objectId ?? "unknown")} ` +
      "blocked live travel",
    );
    this.name = "NavigationInterruptedByThreatError";
    this.threat = threat;
  }
}

class NavigationEnteredUnexpectedMapError extends Error {
  constructor(expectedMapFileName, actualMapFileName) {
    super(`navigation entered unexpected map ${actualMapFileName} from ${expectedMapFileName}`);
    this.name = "NavigationEnteredUnexpectedMapError";
    this.expectedMapFileName = String(expectedMapFileName);
    this.actualMapFileName = String(actualMapFileName);
  }
}

class SupplyFundingSafetyError extends Error {
  constructor(message) {
    super(message);
    this.name = "SupplyFundingSafetyError";
  }
}

class CombatResourceBudgetError extends Error {
  constructor(message) {
    super(message);
    this.name = "CombatResourceBudgetError";
  }
}

class QuestAgentRuntimeLimitError extends Error {
  constructor(context) {
    super(
      `${evidence.route} reached its ${maxRuntimeMs}ms runtime limit` +
      (context ? ` during ${context}` : ""),
    );
    this.name = "QuestAgentRuntimeLimitError";
  }
}

class QuestAgentShutdownError extends Error {
  constructor(signal, context) {
    super(
      `quest-agent graceful shutdown requested by ${signal}` +
      (context ? ` during ${context}` : ""),
    );
    this.name = "QuestAgentShutdownError";
    this.signal = signal;
  }
}

for (const signal of ["SIGINT", "SIGTERM"]) {
  process.on(signal, () => {
    if (shutdownSignal != null) return;
    shutdownSignal = signal;
    console.warn(`quest-agent graceful shutdown requested (${signal})`);
    // A long CDP command can otherwise outlive every referenced timer after
    // Chrome receives a terminal signal, making Node terminate an unsettled
    // top-level await before finalizeEvidence writes the audit report. Reject
    // the in-flight browser command with the typed shutdown error so main's
    // normal finally path runs immediately while the CDP connection is alive.
    client?.cancelPending(
      new QuestAgentShutdownError(signal, "interrupting a pending browser command"),
    );
  });
}

function assertRuntimeBudget(context) {
  if (shutdownSignal != null) {
    throw new QuestAgentShutdownError(shutdownSignal, context);
  }
  if (Date.now() >= evidence.startedAt + maxRuntimeMs) {
    throw new QuestAgentRuntimeLimitError(context);
  }
}

const CLASS_ONBOARDING_SKILLS = Object.freeze({
  Warrior: Object.freeze([{ name: "Fencing", minLevel: 4 }]),
  Wizard: Object.freeze([{ name: "FireBall", minLevel: 4 }]),
  Taoist: Object.freeze([{ name: "Healing", minLevel: 4 }]),
});
const CLASS_ONBOARDING_GEAR = Object.freeze([
  Object.freeze({ name: "OldLoafer", minLevel: 4, slot: "boots" }),
]);
const SAFE_STARTER_LIQUIDATION_GEAR = Object.freeze([
  // Character creation grants this weapon before quest progression starts.
  // Treat it as q0 so a later, currently equipped quest weapon can prove that
  // it is superseded without broadening liquidation to unknown inventory.
  Object.freeze({ questId: 0, name: "WoodenSword" }),
]);
const SAFE_DUPLICATE_EQUIPPED_SUPPLY_LOOT = Object.freeze(["CopperRing"]);
const SAFE_LIQUIDATION_MERCHANTS = Object.freeze([
  Object.freeze({
    key: "blacksmith",
    scriptKey: "BichonProvince/BorderVillage/Blacksmith",
    equipSlot: "weapon",
    mapFileName: BICHON_Q1_Q9_ROUTE.mapFileName,
    npc: BICHON_Q1_Q9_ROUTE.npcs.blacksmith,
    dialogTarget: "@BuySell",
  }),
  Object.freeze({
    key: "meat",
    scriptKey: "BichonProvince/BorderVillage/Butcher",
    itemNames: Object.freeze(["Venison"]),
    mapFileName: BICHON_Q1_Q9_ROUTE.mapFileName,
    // The visible [Types] 15 / @Sell route buys ordinary harvested meat.
    npc: BICHON_Q1_Q9_ROUTE.npcs.butcher,
    dialogTarget: "@Sell",
    allowStatless: true,
  }),
  Object.freeze({
    key: "necklace",
    scriptKey: "BichonProvince/BorderVillage/Necklace",
    equipSlot: "necklace",
    mapFileName: "0141",
    // Crystal NPCInfo index 13 is materialized as loaded object 449. Client
    // interaction packets address that loaded object, while the source
    // [Types] section still proves that Clara accepts item type 5 only.
    npc: Object.freeze({ npcIndex: 449, label: "Merchant Clara", x: 7, y: 10 }),
    dialogTarget: "@BuySell",
  }),
  Object.freeze({
    key: "ring",
    scriptKey: "BichonProvince/BorderVillage/Ring",
    itemNames: SAFE_DUPLICATE_EQUIPPED_SUPPLY_LOOT,
    mapFileName: "0141",
    // Crystal NPCInfo 11 materializes as loaded object 447, and the source
    // BichonProvince/BorderVillage/Ring script explicitly trades CopperRing.
    npc: Object.freeze({ npcIndex: 447, label: "Merchant Alice", x: 20, y: 23 }),
    dialogTarget: "@BuySell",
  }),
  Object.freeze({
    itemNames: Object.freeze(["CannibalLeaf"]),
    mapFileName: BICHON_Q1_Q9_ROUTE.mapFileName,
    // BorderVillage/Materials [Types] contains only 16; CannibalLeaf is
    // Crystal item 866, type 16. NPCInfo 302 materializes as object 43.
    npc: BICHON_Q1_Q9_ROUTE.npcs.materialDealerReece,
    dialogTarget: "@Sell",
  }),
]);
const EQUIPMENT_REPAIR_THRESHOLD_RATIO = 0.25;
const EQUIPMENT_REPAIR_RETRY_MS = 5 * 60_000;
const EQUIPMENT_REPAIR_ROUTES = Object.freeze([
  Object.freeze({
    slots: Object.freeze(["weapon"]),
    mapFileName: "0",
    npc: BICHON_Q1_Q9_ROUTE.npcs.blacksmith,
  }),
  Object.freeze({
    slots: Object.freeze(["armour", "helmet", "belt", "boots"]),
    mapFileName: "0",
    npc: Object.freeze({ npcIndex: 7, label: "Merchant Whitney", x: 305, y: 608 }),
  }),
  Object.freeze({
    slots: Object.freeze(["braceletLeft", "braceletRight"]),
    mapFileName: "0141",
    npc: Object.freeze({ npcIndex: 448, label: "Merchant Betty", x: 15, y: 18 }),
  }),
  Object.freeze({
    slots: Object.freeze(["necklace"]),
    mapFileName: "0141",
    npc: Object.freeze({ npcIndex: 449, label: "Merchant Clara", x: 7, y: 10 }),
  }),
  Object.freeze({
    slots: Object.freeze(["ringLeft", "ringRight"]),
    mapFileName: "0141",
    npc: Object.freeze({ npcIndex: 447, label: "Merchant Alice", x: 20, y: 23 }),
  }),
]);

function liquidationMerchantMatches(route, candidate) {
  return (
    (route.key && route.key === candidate.liquidationMerchantKey) ||
    (route.equipSlot && route.equipSlot === candidate.equipSlot) ||
    (Array.isArray(route.itemNames) && route.itemNames.includes(String(candidate.name)))
  );
}

async function main() {
  await fs.mkdir(framesDir, { recursive: true });
  if (extendedRouteEnabled) {
    const sources = await loadCrystalQuestRouteSources();
    authoritativeRoute = buildClassQuestRoute(sources, { className, maxLevel: targetLevel });
    mapTravelGraph = buildMapTravelGraph(sources);
    grindingCatalog = buildGrindingCatalog(sources);
    progressionSkillBookCatalog = buildProgressionSkillBookCatalog(sources, {
      className,
      maxLevel: targetLevel,
    });
    safeOrdinarySupplyLootCatalog = buildSafeSupplyLootCatalog(sources, {
      className,
      dropTableKeys: ["Provinces/Scarecrow", "Provinces/Deer"],
      merchants: SAFE_LIQUIDATION_MERCHANTS
        .filter((route) => route.scriptKey)
        .map((route) => ({
          merchantKey: route.key,
          scriptKey: route.scriptKey,
          allowStatless: route.allowStatless === true,
        })),
    });
  }
  console.log(`quest-agent: ${evidence.route} target=${baseUrl} identity=[redacted]`);
  console.log(`quest-agent: evidence=${outputDir}`);

  browser = await launchBrowser({
    url: runUrl,
    headed,
    width: 1024,
    height: 768,
    onInput: (input) => evidence.inputs.push(sanitizeInput(input)),
  });
  client = browser.client;

  let fatal = null;
  let interruptionSignal = null;
  try {
    await bootstrapRealClient();
    await runQuestPolicy();
  } catch (error) {
    if (error instanceof QuestAgentShutdownError) {
      interruptionSignal = error.signal;
      console.warn(`quest-agent stopping cleanly after ${interruptionSignal}`);
    } else {
      fatal = String(error?.stack ?? error?.message ?? error);
      console.error(`quest-agent fatal: ${String(error?.message ?? error)}`);
    }
  } finally {
    await finalizeEvidence(fatal, interruptionSignal);
    await stopBrowser(browser).catch(() => {});
  }

  if (interruptionSignal != null) {
    process.exitCode = signalExitCode(interruptionSignal);
  } else if (fatal || evidence.summary?.completed !== true || evidence.summary?.shortcutAudit?.violations?.length) {
    process.exitCode = 1;
  }
}

async function bootstrapRealClient() {
  await waitUntil(client, "window.__mir2Stage5?.state?.screen != null", 45_000);
  let state = await readAgentState(client);
  recordMilestone("client-ready", state);

  if (state.screen === "login") {
    await loginThroughVisibleUi();
    state = await readAgentState(client);
  }
  if (state.screen === "select") {
    await createAndStartCharacterThroughVisibleUi();
    state = await readAgentState(client);
  }
  if (state.screen !== "game") throw new Error(`expected game screen after bootstrap, got ${state.screen}`);
  await waitUntil(
    client,
    "window.__mir2Stage5?.state?.screen === 'game' && window.__mir2Stage5?.state?.sceneInteractionReady === true",
    90_000,
  );
  await waitForAuthoritativePersonalBootstrap();
  const authoritativeBootstrap = await waitUntil(
    client,
    "window.__mir2Stage5?.state?.questLog?.some((quest) => Number(quest?.questId) === 1 && Number(quest?.required) >= 1) === true",
    30_000,
  );
  if (!authoritativeBootstrap) {
    throw new Error("game screen never replaced placeholder state with authoritative quest/item bootstrap");
  }
  captureQuestDefinitionEvidence();
  state = await readAgentState(client);
  recordMilestone("world-entered", state);
  // A persisted character can be killed while normal Bevy assets finish
  // loading. The death dialog is above the tutorial, so recover through its
  // visible town-revive button before attempting to dismiss the tutorial.
  await recoverPlayerIfNeeded(state);
  await closeTutorialIfVisible();
  await equipInitialWarriorGear();
  state = await readAgentState(client);
  await captureEvidenceFrame("world-entered", state);
  assertNoShortcutFrames();
}

async function waitForAuthoritativePersonalBootstrap(timeoutMs = 30_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const userInformationFrames = wsEventFramesSince(client, evidence.startedAt, "packet")
      .filter(({ event }) => event?.packet === "UserInformation");
    const latestUserInformation = userInformationFrames.at(-1);
    if (latestUserInformation) {
      const snapshotFrame = wsEventFramesSince(
        client,
        latestUserInformation.at,
        "worldSnapshot",
      ).find(({ event }) => {
        const snapshot = event?.payload;
        return (
          Array.isArray(snapshot?.entities) &&
          Array.isArray(snapshot?.inventoryItems) &&
          Array.isArray(snapshot?.beltItems) &&
          Array.isArray(snapshot?.equipmentItems) &&
          Array.isArray(snapshot?.questLog)
        );
      });
      if (snapshotFrame) {
        console.log(
          `quest-agent: authoritative personal bootstrap settled in ${snapshotFrame.at - latestUserInformation.at}ms`,
        );
        return snapshotFrame.event.payload;
      }
    }
    await delay(50);
  }
  throw new Error(
    "StartGame did not deliver an authoritative world snapshot after UserInformation before autonomous planning",
  );
}

function captureQuestDefinitionEvidence() {
  const definitions = wsPacketsSince(client, evidence.startedAt, "NewQuestInfo")
    .flatMap((payload) => {
      const questId = Number(payload?.info?.index ?? payload?.id);
      if (!Number.isInteger(questId) || questId < 1 || questId > maxQuestId) return [];
      return [{
        questId,
        fixedRewards: (payload?.rewards?.items ?? []).map((item) => String(item?.name ?? "")),
        selectableRewards: (payload?.rewards?.selectItems ?? []).map((item) => ({
          name: String(item?.name ?? ""),
          selectionIndex: Number(item?.selectionIndex ?? -1),
        })),
      }];
    })
    .sort((left, right) => left.questId - right.questId);
  evidence.questDefinitions = definitions;
  const target = definitions.find((entry) => entry.questId === maxQuestId);
  if (target) {
    console.log(
      `quest-agent: q${maxQuestId}-definition fixed=${target.fixedRewards.length} selectable=${target.selectableRewards.length}`,
    );
  }
}

async function closeTutorialIfVisible() {
  const visible = await client.evaluate("document.querySelector('.mir-tutorial-card') != null");
  if (!visible) return false;
  await client.clickSelector(".mir-tutorial-card button", { action: "close-tutorial" });
  const closed = await waitUntil(client, "document.querySelector('.mir-tutorial-card') == null", 5_000);
  if (!closed) throw new Error("visible tutorial close button did not dismiss the overlay");
  recordMilestone("tutorial-closed", await readAgentState(client));
  return true;
}

async function equipInitialWarriorGear() {
  let state = await readAgentState(client);
  const wanted = missingStarterEquipment(state, [
    { name: "WoodenSword", slot: "weapon" },
    { name: "BaseDress(M)", slot: "armour" },
  ]);
  if (!wanted.length) return false;
  if (state.activeNpcDialog) await closeNpcDialog();
  await openInventory();
  for (const { name } of wanted) {
    await client.clickSelector(`button.inventory-item-card[aria-label="${name}"]`, {
      action: "equip-initial-warrior-item", item: name,
    });
    const equipped = await waitUntil(
      client,
      `window.__mir2Stage5?.state?.equipmentItems?.some((item) => item?.name === ${JSON.stringify(name)}) === true`,
      10_000,
    );
    if (!equipped) throw new Error(`visible inventory activation did not equip ${name}`);
  }
  await closeInventory();
  state = await readAgentState(client);
  recordMilestone("initial-warrior-gear-equipped", state, { items: wanted.map(({ name }) => name) });
  return true;
}

async function loginThroughVisibleUi() {
  const visible = await waitForVisibleSelector(".login-input.account", 15_000);
  if (!visible) throw new Error("visible login form did not mount within 15s");
  await client.fillSelector(".login-input.account", account, { action: "enter-account" });
  await client.fillSelector(".login-input.password", password, { action: "enter-password", secret: true });
  if (createAccount) {
    // Input.insertText dispatches the same visible input events as a human, but
    // React may not have committed both controlled values before an immediate
    // pointer click on a very fast local page. Give the rendered form one short
    // commit window, then require the authoritative Gateway response before
    // attempting login. This remains ordinary mouse/text input and prevents a
    // missing NewAccount click from being misreported later as bad credentials.
    await delay(250);
    const createAccountStartedAt = Date.now();
    await client.clickSelector(".login-button.account button", { action: "create-account" });
    const createAccountDeadline = Date.now() + 15_000;
    let createAccountResponse = null;
    while (Date.now() < createAccountDeadline) {
      createAccountResponse = wsPacketsSince(client, createAccountStartedAt, "NewAccount").at(-1) ?? null;
      if (createAccountResponse) break;
      await delay(100);
    }
    if (!createAccountResponse) {
      throw new Error("visible account creation did not receive a NewAccount response");
    }
    const createAccountResult = Number(createAccountResponse.result);
    if (createAccountResult !== 8) {
      throw new Error(`visible account creation returned result ${createAccountResult}`);
    }
    await delay(250);
  }
  let state = await readAgentState(client);
  if (state.screen === "login") {
    await client.fillSelector(".login-input.account", account, { action: "enter-account" });
    await client.fillSelector(".login-input.password", password, { action: "enter-password", secret: true });
    await client.clickSelector(".login-button.ok button", { action: "login" });
  }
  const reachedSelect = await waitUntil(
    client,
    "window.__mir2Stage5?.state?.screen === 'select' || window.__mir2Stage5?.state?.screen === 'game'",
    45_000,
  );
  if (!reachedSelect) throw new Error("visible login flow did not reach character select");
  recordMilestone("login-complete", await readAgentState(client));
}

async function createAndStartCharacterThroughVisibleUi() {
  const visible = await waitForVisibleSelector(".select-overlay", 15_000);
  if (!visible) throw new Error("visible character-select screen did not mount within 15s");
  let state = await readAgentState(client);
  const existing = state.screen === "select" && (await characterExists(characterName));
  if (!existing) {
    await delay(750);
    let created = false;
    for (let attempt = 1; attempt <= 3 && !created; attempt += 1) {
      await client.clickSelector(".select-action.new button", { action: "open-character-create", attempt });
      const panel = await waitForVisibleSelector(".select-create-panel", 10_000);
      if (!panel) throw new Error("visible character-create panel did not mount");
      await client.fillSelector(".select-create-name-field input", characterName, { action: "enter-character-name" });
      await client.clickSelector(".select-create-gender-button", { action: "choose-male", text: "Male" });
      await client.clickSelector(".select-create-class-card", {
        action: `choose-${className.toLowerCase()}`,
        text: className,
      });
      const createActionVisible = await waitForVisibleSelector(
        ".select-create-actions button",
        10_000,
      );
      if (!createActionVisible) {
        throw new Error("visible character-create action did not finish rendering");
      }
      await delay(250);
      const beforeSubmit = Date.now();
      await client.clickSelector(".select-create-actions button", {
        action: `create-${className.toLowerCase()}`,
        attempt,
      });
      created = await waitUntil(
        client,
        `Array.isArray(window.__mir2Stage5?.state?.characters) && window.__mir2Stage5.state.characters.some((entry) => entry?.name === ${JSON.stringify(characterName)})`,
        12_000,
      );
      if (created) break;
      const sent = client.outgoingCommandAudit().commands.some(
        (entry) => entry.at >= beforeSubmit && entry.type === "newCharacter",
      );
      const failed = wsPacketsSince(client, beforeSubmit, "NewCharacter");
      if (sent || failed.length) {
        throw new Error(`server rejected visible ${className} creation: ${JSON.stringify(failed.at(-1) ?? {})}`);
      }
      await delay(500);
    }
    if (!created) throw new Error("visible character creation did not emit a newCharacter command after 3 real-click attempts");
    recordMilestone(`${className.toLowerCase()}-created`, await readAgentState(client));
  }

  await client.clickSelector(".select-character-slot-card", { text: characterName, action: "select-character" });
  let entered = false;
  for (let attempt = 1; attempt <= 2 && !entered; attempt += 1) {
    const startVisible = await waitForVisibleSelector(".select-action.start button", 5_000);
    if (!startVisible) throw new Error("visible START button did not settle after character selection");
    const startRequestedAt = Date.now();
    await client.clickSelector(".select-action.start button", { action: "start-game", attempt });
    const result = await waitForVisibleStartGameResult(startRequestedAt, 60_000);
    entered = result === "entered";
    if (entered) break;
    if (result !== "route-lease") {
      throw new Error("visible Start Game flow did not enter the world");
    }
    if (attempt === 2) {
      throw new Error(`visible Start Game flow failed: ${TRANSIENT_START_GAME_ROUTE_LEASE_MESSAGE}`);
    }
    evidence.milestones.push({
      kind: "start-game-route-lease-retry",
      at: Date.now(),
      attempt,
      waitMs: START_GAME_ROUTE_LEASE_RETRY_MS,
    });
    await delay(START_GAME_ROUTE_LEASE_RETRY_MS);
  }
  state = await readAgentState(client);
  recordMilestone("character-started", state);
}

async function waitForVisibleStartGameResult(startRequestedAt, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (await client.evaluate("window.__mir2Stage5?.state?.screen === 'game'")) {
      return "entered";
    }
    const routeLeaseRejected = wsEventFramesSince(client, startRequestedAt, "error")
      .some(({ event }) => String(event?.message ?? "") === TRANSIENT_START_GAME_ROUTE_LEASE_MESSAGE);
    if (routeLeaseRejected) return "route-lease";
    await delay(120);
  }
  return "timeout";
}

async function characterExists(name) {
  return client.evaluate(
    `Array.isArray(window.__mir2Stage5?.state?.characters) && window.__mir2Stage5.state.characters.some((entry) => entry?.name === ${JSON.stringify(name)})`,
  );
}

async function waitForVisibleSelector(selector, timeoutMs) {
  return waitUntil(
    client,
    `(() => { const node = document.querySelector(${JSON.stringify(selector)}); if (!(node instanceof HTMLElement)) return false; const box = node.getBoundingClientRect(); return box.width > 0 && box.height > 0; })()`,
    timeoutMs,
  );
}

async function runQuestPolicy() {
  const deadline = evidence.startedAt + maxRuntimeMs;
  let noProgressCount = 0;
  let previousFingerprint = "";

  while (goalSequence < maxGoals && Date.now() < deadline) {
    assertRuntimeBudget("starting the next autonomous goal");
    await recoverPlayerIfNeeded();
    let before = await readAgentState(client);
    if (!before.player && !before.playerDead && !before.deathOverlayVisible) {
      // StartGame and map transitions can expose the quest log one render
      // frame before the authoritative self entity. Every safety decision
      // below depends on the real player transform; never let that transient
      // snapshot bypass the supply gate and fall through into quest planning.
      const playerSettled = await waitUntil(
        client,
        `(() => { const s = window.__mir2Stage5?.state ?? {}; const entities = Array.isArray(s.entities) ? s.entities : []; const self = entities.find((entry) => String(entry?.objectId) === String(s.playerObjectId)); return s.screen === 'game' && self != null && Number.isFinite(Number(self.x)) && Number.isFinite(Number(self.y)); })()`,
        12_000,
      );
      if (!playerSettled) {
        throw new Error("rendered player snapshot did not settle before autonomous planning");
      }
      before = await readAgentState(client);
      if (!before.player) {
        throw new Error("authoritative player transform remained unavailable after render settle");
      }
    }
    if (await recoverHealthInSafeInteriorIfNeeded(before)) {
      before = await readAgentState(client);
      const maxHp = Number(before.playerMaxHp ?? 0);
      const healthRatio = maxHp > 0 ? Number(before.playerHp ?? 0) / maxHp : 1;
      if (
        before.playerDead || before.deathOverlayVisible ||
        healthRatio < SAFE_FUNDING_READY_HEALTH_RATIO ||
        Date.now() < supplyFundingShelterUntil
      ) {
        continue;
      }
    }
    if (await retreatFromUnsafeActiveThreatIfNeeded(before)) {
      // Unsafe combat recovery owns the next input. In particular, do not walk
      // toward an optional drop or NPC while an attacker is still landing
      // hits merely because the player is already inside the broad village
      // supply radius. The next policy turn will commit to the visible 0141
      // shelter entrance when the retreat raises the shelter latch.
      continue;
    }
    if (await recoverQuestDepartureHealthIfNeeded(before)) {
      // Recovery must precede every optional pickup, sale, repair, and stock
      // gate. The broad village supply radius also contains real monster
      // fields; letting a low-HP character enter an NPC trip from there can
      // consume its entire potion reserve before the later departure check.
      continue;
    }
    if (await collectVisibleHealthPotionDropIfNeeded(before).catch((error) => {
      if (
        error instanceof QuestAgentRuntimeLimitError ||
        error instanceof QuestAgentShutdownError
      ) throw error;
      console.warn(`  optional visible HP-drug pickup deferred: ${String(error?.message ?? error)}`);
      return false;
    })) {
      before = await readAgentState(client);
    }
    if (await collectVisibleProgressionSkillBookIfNeeded(before).catch((error) => {
      if (
        error instanceof QuestAgentRuntimeLimitError ||
        error instanceof QuestAgentShutdownError
      ) throw error;
      console.warn(`  optional visible skill-book pickup deferred: ${String(error?.message ?? error)}`);
      return false;
    })) {
      before = await readAgentState(client);
    }
    if (await collectNearbyGoldIfVisible(before, 8).catch((error) => {
      if (
        error instanceof QuestAgentRuntimeLimitError ||
        error instanceof QuestAgentShutdownError
      ) throw error;
      console.warn(`  optional visible gold pickup deferred: ${String(error?.message ?? error)}`);
      return false;
    })) {
      before = await readAgentState(client);
    }
    if (await collectVisibleSafeSupplyLootIfNeeded(before).catch((error) => {
      if (
        error instanceof QuestAgentRuntimeLimitError ||
        error instanceof QuestAgentShutdownError
      ) throw error;
      console.warn(`  optional visible sellable-loot pickup deferred: ${String(error?.message ?? error)}`);
      return false;
    })) {
      before = await readAgentState(client);
    }
    if (await returnToSupplyAreaForPotionsIfNeeded(before)) {
      before = await readAgentState(client);
    }
    const restocked = await restockHealthPotionsIfNeeded(before).catch((error) => {
      console.warn(`  optional visible potion restock deferred: ${String(error?.message ?? error)}`);
      return false;
    });
    if (restocked) {
      before = await readAgentState(client);
    }
    if (await fundHealthPotionsWithSafeHuntIfNeeded(before).catch((error) => {
      if (
        error instanceof QuestAgentRuntimeLimitError ||
        error instanceof QuestAgentShutdownError
      ) throw error;
      console.warn(`  optional visible potion funding deferred: ${String(error?.message ?? error)}`);
      // The character still satisfies the funding predicate. Treat the
      // failed target as an attempted supply action and restart the policy
      // loop; never fall through into a dangerous quest field with zero HP
      // drugs merely because one moving Scarecrow could not be reached.
      return true;
    })) {
      before = await readAgentState(client);
      const purchased = await restockHealthPotionsIfNeeded(before).catch((error) => {
        console.warn(`  visible potion purchase after funding deferred: ${String(error?.message ?? error)}`);
        return false;
      });
      if (purchased) before = await readAgentState(client);
      // One safe funding hunt is a complete autonomous action. Re-enter the
      // policy loop so recovery, shop state, and runtime budget are checked
      // before any long quest-field departure.
      continue;
    }
    if (localPotionSupplyIncomplete(before)) {
      // Funding can correctly return false once enough gold is already held.
      // If a prior shop attempt is still retry-throttled or transiently failed,
      // retain the hard departure gate and retry the visible purchase instead
      // of falling through into the dangerous quest route.
      console.log(
        `  hold quest departure for HP stock: ` +
        `${healthPotionQuantity(before)}/${HEALTH_POTION_DEPARTURE_STOCK}`,
      );
      await delay(500);
      continue;
    }
    if (await repairProgressionEquipmentIfNeeded(before).catch((error) => {
      if (
        error instanceof QuestAgentRuntimeLimitError ||
        error instanceof QuestAgentShutdownError
      ) throw error;
      equipmentRepairRetryUntil = Date.now() + EQUIPMENT_REPAIR_RETRY_MS;
      console.warn(`  visible equipment repair deferred: ${String(error?.message ?? error)}`);
      return false;
    })) {
      // A repair visit is a complete visible economy action. Re-read supplies,
      // threats, and the remaining damaged slots before leaving town again.
      continue;
    }
    if (!commonQ1Q6Completed(before)) {
      before = await recoverRouteMapIfAdjacent(before);
      if (String(before.mapFileName) !== BICHON_Q1_Q9_ROUTE.mapFileName) {
        throw new Error(
          `q1-q9 character left ${BICHON_Q1_Q9_ROUTE.mapFileName} through a normal map transition ` +
          `(now ${before.mapFileName}) and no adjacent visible return transfer was available`,
        );
      }
    }
    if (routeRunCompleted(before)) break;

    // Equipment and inventory interactions are modal. Re-check death before
    // opening them because a hostile that entered combat during the preceding
    // goal can kill the player between the loop's first recovery check and the
    // progression-gear step.
    before = await recoverPlayerIfNeeded(before);
    if (await equipProgressionGearIfReady(before)) before = await readAgentState(client);
    if (await equipOnboardingGearIfReady(before)) before = await readAgentState(client);
    if (await learnProgressionSkillIfReady(before)) before = await readAgentState(client);

    const planningState = preferredGrindingPlanningState(before);
    let goal = !extendedRouteEnabled
      ? planNextQ1Q9(before)
      : !commonQ1Q6Completed(before)
        ? planNextQ1Q9(before)
        : !classOnboardingCompleted(before)
          ? planNextAuthoritativeQuest(planningState, authoritativeRoute, {
              minQuestId: classOnboardingBounds().minQuestId,
              maxQuestId: classOnboardingBounds().maxQuestId,
              targetLevel: 4,
              grindingCatalog,
            })
          : planNextAuthoritativeQuest(planningState, authoritativeRoute, {
          minQuestId: 22,
          maxQuestId,
          targetLevel,
          grindingCatalog,
        });
    goal = adaptiveCombatPreparationGoal(planningState, goal);
    goal = adaptiveGrindingRiskGoal(planningState, goal);
    goalSequence += 1;
    const goalRecord = {
      sequence: goalSequence,
      goal,
      startedAt: Date.now(),
      before: compactGoalState(before, goal),
      ok: false,
    };
    evidence.goals.push(goalRecord);
    console.log(`\n[${goalSequence}] ${describeGoal(goal)}`);
    const revivesBeforeGoal = evidence.revives;

    try {
      if (goal.kind === "talk") await executeTalkGoal(goal);
      else if (goal.kind === "hunt") await executeHuntGoal(goal, before);
      else if (goal.kind === "grind") await executeHuntGoal(goal, before);
      else if (goal.kind === "quest-diary") await executeQuestDiaryGoal(goal);
      else if (goal.kind === "special-script") await executeSpecialScriptGoal(goal);
      else if (goal.kind === "wait") await delay(1_200);
      else if (goal.kind === "done") break;
      else if (goal.kind === "blocked") {
        throw new Error(`${goal.kind} q${goal.questId}: ${JSON.stringify(goal.blockers ?? goal.flags ?? goal.reason)}`);
      }
      else throw new Error(`unsupported goal kind ${goal.kind}`);
      goalRecord.ok = true;
      if (goal.kind === "hunt" && Number(goal.questId) > 0) {
        questMonsterDeaths.delete(normalizeName(goal.monsterName));
      }
    } catch (error) {
      if (
        error instanceof QuestAgentRuntimeLimitError ||
        error instanceof QuestAgentShutdownError
      ) throw error;
      goalRecord.error = String(error?.message ?? error);
      console.warn(`  retryable goal failure: ${goalRecord.error}`);
      await recoverPlayerIfNeeded();
      if (
        goal.kind === "hunt" &&
        Number(goal.questId) > 0 &&
        evidence.revives > revivesBeforeGoal
      ) {
        const key = normalizeName(goal.monsterName);
        const deaths = Number(questMonsterDeaths.get(key) ?? 0) + 1;
        questMonsterDeaths.set(key, deaths);
        console.log(`  combat risk memory: ${goal.monsterName} deaths=${deaths}`);
      }
      if (
        goal.kind === "grind" &&
        evidence.revives > revivesBeforeGoal
      ) {
        grindingMonsterRiskUntil.set(
          normalizeName(goal.monsterName),
          Date.now() + 30 * 60_000,
        );
        console.log(`  grind risk memory: ${goal.monsterName} death; cooling down source`);
      }
    }

    await delay(700);
    const after = await readAgentState(client);
    rememberQuestCombatResourceStrain(goal, before, after);
    rememberGrindingSourceStall(goal, goalRecord, before, after);
    goalRecord.after = compactGoalState(after, goal);
    goalRecord.endedAt = Date.now();
    goalRecord.durationMs = goalRecord.endedAt - goalRecord.startedAt;
    if (shouldCaptureGoalFrame(
      goal,
      before,
      after,
      goalSequence,
      GRIND_SCREENSHOT_SAMPLE_INTERVAL,
    )) {
      await captureEvidenceFrame(
        `${String(goalSequence).padStart(3, "0")}-${goal.kind}-q${goal.questId ?? "done"}`,
        after,
      );
    }
    assertNoShortcutFrames();

    const fingerprint = progressFingerprint(after);
    if (fingerprint === previousFingerprint) noProgressCount += 1;
    else noProgressCount = 0;
    previousFingerprint = fingerprint;
    if (noProgressCount >= 8) {
      throw new Error(`quest policy made no authoritative progress for ${noProgressCount} consecutive goals`);
    }
  }

  const final = await readAgentState(client);
  if (!routeRunCompleted(final)) {
    throw new Error(`${evidence.route} did not complete before limit (goals=${goalSequence}, runtimeMs=${Date.now() - evidence.startedAt})`);
  }
  recordMilestone(extendedRouteEnabled ? "extended-route-complete" : "q1-q9-complete", final);
}

function preferredGrindingPlanningState(state) {
  if (String(state?.mapFileName ?? "") !== SAFE_RECOVERY_MAP_FILE_NAME) {
    return state;
  }
  // GroceryStore is a zero-respawn shelter one ordinary doorway from the
  // Border Village. Treat that adjacent outdoor map as the locality hint for
  // grind selection; otherwise every spawn is equally "remote" and a tied
  // monster variant can send the client hundreds of tiles toward another map.
  // Only the read-only planning snapshot is adjusted. executeHuntGoal still
  // observes 0141 and must walk through the normal map transfer itself.
  return {
    ...state,
    mapFileName: String(BICHON_Q1_Q9_ROUTE.mapFileName),
  };
}

function adaptiveCombatPreparationGoal(state, plannedGoal) {
  if (plannedGoal?.kind !== "hunt" || Number(plannedGoal?.questId) <= 0) return plannedGoal;
  const monsterKey = normalizeName(plannedGoal.monsterName);
  const playerLevel = Number(state?.playerLevel ?? 1);
  let preparationLevel = Number(questMonsterPreparationLevel.get(monsterKey));
  const deaths = Number(questMonsterDeaths.get(monsterKey) ?? 0);
  const resourceStrains = Number(questMonsterResourceStrains.get(monsterKey) ?? 0);
  const sourceLevel = Number(plannedGoal.monsterLevel);
  if (
    resourceStrains >= 1 &&
    plannedGoal.harvest === true &&
    Number.isFinite(sourceLevel) &&
    sourceLevel > playerLevel
  ) {
    // A harvest source must survive both the kill and a corpse interaction,
    // but one strained corpse does not prove that every intermediate level is
    // unsafe. Live q25 evidence already proved the source kill at level 13;
    // its harvest failed because a second corpse covered the hit surface. The
    // overlay defence is now fixed, so retry after exactly one ordinary level
    // and learn from the next real lifecycle instead of blindly grinding all
    // the way to the source level. Also clamp inherited pre-fix source-level
    // targets when a resumed report carries the older conservative decision.
    const stagedPreparationLevel = Math.min(sourceLevel, playerLevel + 1);
    if (
      !Number.isFinite(preparationLevel) ||
      preparationLevel > stagedPreparationLevel
    ) {
      preparationLevel = stagedPreparationLevel;
    }
    questMonsterPreparationLevel.set(monsterKey, preparationLevel);
    console.log(
      `  combat preparation learned from prior harvest strain: ` +
      `${plannedGoal.monsterName} level ${playerLevel}->${preparationLevel}`,
    );
  }
  if (!Number.isFinite(preparationLevel) && deaths >= 2) {
    // Two deaths during the search/fight are live evidence that this character
    // cannot yet sustain the route, even if the source table's nominal level
    // looked reasonable. Gain exactly one normal level, then retry; do not
    // hide the quest behind a hand-maintained permanent threshold.
    preparationLevel = playerLevel + 1;
    questMonsterPreparationLevel.set(monsterKey, preparationLevel);
  }
  if (!Number.isFinite(preparationLevel)) return plannedGoal;
  if (playerLevel >= preparationLevel) {
    recordCombatResourceRecovery(
      plannedGoal.monsterName,
      "adaptive-preparation-complete",
    );
    questMonsterPreparationLevel.delete(monsterKey);
    questMonsterDeaths.delete(monsterKey);
    questMonsterResourceStrains.delete(monsterKey);
    console.log(
      `  combat preparation complete: ${plannedGoal.monsterName} at level ${playerLevel}`,
    );
    return plannedGoal;
  }
  const grind = chooseGrindingGoal(state, grindingCatalog, preparationLevel, {
    certifiedMonsterNames: completedQuestCombatCertifications(
      state,
      authoritativeRoute,
    ),
  });
  if (!grind) return plannedGoal;
  return {
    ...grind,
    preparationForQuestId: Number(plannedGoal.questId),
    preparationForMonsterName: String(plannedGoal.monsterName),
    preparationForMonsterLevel: Number(plannedGoal.monsterLevel ?? 0) || null,
    observedQuestMonsterDeaths: deaths,
  };
}

function adaptiveGrindingRiskGoal(state, plannedGoal) {
  if (plannedGoal?.kind !== "grind") return plannedGoal;
  const now = Date.now();
  for (const [monsterName, until] of grindingMonsterRiskUntil) {
    if (until <= now) grindingMonsterRiskUntil.delete(monsterName);
  }
  if (!grindingMonsterRiskUntil.has(normalizeName(plannedGoal.monsterName))) {
    return plannedGoal;
  }
  const alternatives = grindingCatalog.filter(
    (entry) => !grindingMonsterRiskUntil.has(normalizeName(entry.monsterName)),
  );
  const alternate = chooseGrindingGoal(
    state,
    alternatives,
    Number(plannedGoal.targetLevel ?? targetLevel),
  );
  if (!alternate) return plannedGoal;
  console.log(
    `  adaptive grind source: ${plannedGoal.monsterName}->${alternate.monsterName} ` +
    "after live resource/death risk",
  );
  return {
    ...alternate,
    preparationForQuestId: plannedGoal.preparationForQuestId,
    preparationForMonsterName: plannedGoal.preparationForMonsterName,
    preparationForMonsterLevel: plannedGoal.preparationForMonsterLevel,
  };
}

async function executeTalkGoal(goal) {
  const npc = goal.npc ?? BICHON_Q1_Q9_ROUTE.npcs[goal.npcKey];
  if (!npc) throw new Error(`unknown route NPC ${goal.npcKey}`);
  const travelState = await readAgentState(client);
  const travelResourceGoal = {
    kind: "travel",
    questId: Number(goal.questId ?? 0),
    monsterName: `q${Number(goal.questId ?? 0)} NPC travel`,
    travelLabel: `q${Number(goal.questId ?? 0)} ${String(goal.action ?? "talk")}`,
  };
  if (npc.mapFileName && String(travelState.mapFileName) !== String(npc.mapFileName)) {
    await travelToMap(npc.mapFileName, {
      minimumStartingGold: talkGoalScriptTravelGoldRequirement(travelState, goal, npc),
      resourceBaseline: travelState,
      resourceAccountingGoal: travelResourceGoal,
    });
  }
  // Quest hand-ins commonly return through the same dense beginner fields as
  // hunting. Permit the bounded navigation layer to clear only an adjacent
  // monster already certified by completed real quest combat; unproven actors
  // remain ordinary obstacles and are never attacked just to shorten travel.
  await openNpcDialog(npc, goal.target, {
    clearTrivialOccupancy: true,
    resourceBaseline: travelState,
    resourceAccountingGoal: travelResourceGoal,
  });

  const before = await readAgentState(client);
  const beforeStage = normalizedQuestStage(questState(before, goal.questId)?.stage);
  const since = Date.now();
  await clickDialogTarget(goal.target, `quest-${goal.action}-${goal.questId}`);

  const rewardChoiceTarget = goal.rewardChoiceTarget ?? (
    goal.action === "finish" && goal.selectedItemIndex !== undefined
      ? `@quest:finish:${goal.questId}:${goal.selectedItemIndex}`
      : null
  );
  if (rewardChoiceTarget) {
    const choiceVisible = await waitUntil(
      client,
      `document.querySelector(${JSON.stringify(dialogTargetSelector(rewardChoiceTarget))}) != null`,
      12_000,
    );
    if (!choiceVisible) throw new Error(`reward choice did not appear: ${rewardChoiceTarget}`);
    await clickDialogTarget(rewardChoiceTarget, `quest-reward-${goal.questId}`);
  }

  const expected = goal.action === "finish"
    ? "completed"
    : [1, 3, 7, 9].includes(goal.questId)
      ? "readytoturnin"
      : "inprogress";
  const changed = goal.npc && goal.action === "accept"
    ? await waitForQuestStages(goal.questId, ["inprogress", "readytoturnin"], 20_000)
    : await waitForQuestStage(goal.questId, expected, 20_000);
  if (!changed) {
    const state = await readAgentState(client);
    throw new Error(
      `q${goal.questId} ${goal.action} did not reach ${expected} (before=${beforeStage}, after=${normalizedQuestStage(questState(state, goal.questId)?.stage)})`,
    );
  }
  const packets = goal.action === "finish"
    ? wsPacketsSince(client, since, "CompleteQuest")
    : wsPacketsSince(client, since, "ChangeQuest");
  await closeNpcDialog();
  recordMilestone(`q${goal.questId}-${goal.action}`, await readAgentState(client), { packetCount: packets.length });
}

function talkGoalScriptTravelGoldRequirement(state, goal, npc) {
  if (!mapTravelGraph || !npc?.mapFileName) return 0;
  const outbound = findMapTravelRoute(
    mapTravelGraph,
    String(state?.mapFileName ?? ""),
    String(npc.mapFileName),
  );
  if (!outbound) return 0;
  const journey = [...outbound];
  if (goal?.action === "accept") {
    const quest = authoritativeRoute?.quests?.find(
      (entry) => Number(entry.questId) === Number(goal.questId),
    );
    const finishMap = quest?.finishNpc?.mapFileName;
    if (finishMap && String(finishMap) !== String(npc.mapFileName)) {
      const returnRoute = findMapTravelRoute(
        mapTravelGraph,
        String(npc.mapFileName),
        String(finishMap),
      );
      if (returnRoute) journey.push(...returnRoute);
    }
  }
  return minimumStartingGoldForMapTravelEdges(journey);
}

async function executeQuestDiaryGoal(goal) {
  let state = await readAgentState(client);
  if (state.activeNpcDialog) await closeNpcDialog();
  if (await client.evaluate("document.querySelector('.inventory-window') != null")) {
    await closeInventory();
  }
  await openQuestDiary();

  const stageTab = goal.action === "accept" ? "available" : "readyToTurnIn";
  await client.clickSelector(`button[data-quest-tab="${stageTab}"]`, {
    action: "quest-diary-filter",
    questId: goal.questId,
    stage: stageTab,
  });
  await delay(150);

  const rowSelector = `button[data-quest-id="${Number(goal.questId)}"]`;
  let found = false;
  for (let page = 0; page < 40; page += 1) {
    found = await waitForVisibleSelector(rowSelector, 350);
    if (found) break;
    const next = await client.evaluate(`(() => {
      const windowNode = document.querySelector('section[data-quest-stage-filter]');
      const node = windowNode?.querySelector('button[aria-label="Next"]');
      return node instanceof HTMLButtonElement && !node.disabled;
    })()`);
    if (!next) break;
    await client.clickSelector('section[data-quest-stage-filter] button[aria-label="Next"]', {
      action: "quest-diary-next-page",
      questId: goal.questId,
      page,
    });
    await delay(100);
  }
  if (!found) {
    throw new Error(`q${goal.questId} was not visible in the ${stageTab} Quest Diary pages`);
  }
  await client.clickSelector(rowSelector, {
    action: "quest-diary-select",
    questId: goal.questId,
  });

  if (goal.action === "finish" && goal.selectedItemIndex !== undefined) {
    await clickQuestDiaryReward(goal.questId, Number(goal.selectedItemIndex));
  }

  const beforeStage = normalizedQuestStage(questState(await readAgentState(client), goal.questId)?.stage);
  const since = Date.now();
  const actionSelector = goal.action === "accept"
    ? '[data-testid="quest-accept-button"]'
    : '[data-testid="quest-finish-button"]';
  await client.clickSelector(actionSelector, {
    action: `quest-diary-${goal.action}`,
    questId: goal.questId,
    selectedItemIndex: Number(goal.selectedItemIndex ?? -1),
  });
  const changed = goal.action === "finish"
    ? await waitForQuestStage(goal.questId, "completed", 20_000)
    : await waitForQuestStages(goal.questId, ["inprogress", "readytoturnin"], 20_000);
  if (!changed) {
    const afterStage = normalizedQuestStage(questState(await readAgentState(client), goal.questId)?.stage);
    throw new Error(`Quest Diary q${goal.questId} ${goal.action} did not change stage (${beforeStage}->${afterStage})`);
  }
  await closeQuestDiary();
  const packets = goal.action === "finish"
    ? wsPacketsSince(client, since, "CompleteQuest")
    : wsPacketsSince(client, since, "ChangeQuest");
  recordMilestone(`q${goal.questId}-diary-${goal.action}`, await readAgentState(client), {
    packetCount: packets.length,
  });
}

async function clickQuestDiaryReward(questId, selectedItemIndex) {
  const selector = `button[data-quest-reward-selection="${Number(selectedItemIndex)}"]`;
  const detailSelector = `[data-quest-detail="${Number(questId)}"]`;
  const exists = await waitUntil(
    client,
    `document.querySelector(${JSON.stringify(selector)}) != null`,
    8_000,
  );
  if (!exists) throw new Error(`q${questId} selectable reward ${selectedItemIndex} was not rendered`);

  for (let attempt = 0; attempt < 12; attempt += 1) {
    try {
      await client.clickSelector(selector, {
        action: "quest-diary-select-reward",
        questId,
        selectedItemIndex,
      });
      return;
    } catch (error) {
      if (!String(error?.message ?? error).startsWith("visible element not found:")) throw error;
    }
    const direction = await client.evaluate(`(() => {
      const container = document.querySelector(${JSON.stringify(detailSelector)});
      const target = document.querySelector(${JSON.stringify(selector)});
      if (!(container instanceof HTMLElement) || !(target instanceof HTMLElement)) return 0;
      const containerBox = container.getBoundingClientRect();
      const targetBox = target.getBoundingClientRect();
      return targetBox.bottom < containerBox.top ? -1 : 1;
    })()`);
    const scrolled = await client.wheelSelector(
      detailSelector,
      Number(direction) < 0 ? -140 : 140,
      { action: "scroll-quest-diary-detail", questId, selectedItemIndex, attempt: attempt + 1 },
    );
    if (!scrolled) break;
    await delay(120);
  }

  throw new Error(`q${questId} selectable reward ${selectedItemIndex} exists but could not be made physically visible`);
}

async function openQuestDiary() {
  if ((await readAgentState(client)).questWindowOpen) return;
  await client.pressKeyChord([
    { key: "Alt", code: "AltLeft", vk: 18 },
    { key: "q", code: "KeyQ", vk: 81 },
  ], { action: "open-quest-diary" });
  const opened = await waitUntil(
    client,
    "document.querySelector('section[data-quest-stage-filter]') != null",
    5_000,
  );
  if (!opened) throw new Error("Alt+Q did not open the visible Quest Diary");
}

async function closeQuestDiary() {
  const visible = await client.evaluate(
    "document.querySelector('section[data-quest-stage-filter]') != null",
  );
  if (!visible) return;
  await client.pressKeyChord([
    { key: "Alt", code: "AltLeft", vk: 18 },
    { key: "q", code: "KeyQ", vk: 81 },
  ], { action: "close-quest-diary" });
  const closed = await waitUntil(
    client,
    "document.querySelector('section[data-quest-stage-filter]') == null",
    5_000,
  );
  if (!closed) throw new Error("Alt+Q did not close the visible Quest Diary");
}

async function executeSpecialScriptGoal(goal) {
  const beforeState = await readAgentState(client);
  const travelResourceGoal = {
    kind: "travel",
    questId: Number(goal.questId ?? 0),
    monsterName: `q${Number(goal.questId ?? 0)} scripted travel`,
    travelLabel: `q${Number(goal.questId ?? 0)} special-script`,
  };
  const beforeQuest = questState(beforeState, goal.questId);
  const pendingFlag = nextIncompleteFlag(beforeQuest, goal.flags);
  if (!pendingFlag) {
    await delay(500);
    return;
  }
  const flag = goal.flags[pendingFlag.flagIndex];
  const setters = [...(flag.setters ?? [])].sort((left, right) =>
    Number(String(right.npc?.mapFileName) === String(beforeState.mapFileName)) -
      Number(String(left.npc?.mapFileName) === String(beforeState.mapFileName)) ||
    left.targetSequence.length - right.targetSequence.length
  );
  if (!setters.length) throw new Error(`q${goal.questId} flag ${flag.number} has no visible setter`);
  const beforeFingerprint = questObjectiveFingerprint(beforeQuest, pendingFlag.objectiveIndex);

  for (const setter of setters) {
    const npc = {
      npcIndex: setter.npc.objectId,
      label: setter.npc.name,
      mapFileName: setter.npc.mapFileName,
      x: setter.npc.position.x,
      y: setter.npc.position.y,
    };
    if (String((await readAgentState(client)).mapFileName) !== String(npc.mapFileName)) {
      await travelToMap(npc.mapFileName, {
        resourceBaseline: beforeState,
        resourceAccountingGoal: travelResourceGoal,
      });
    }
    if (await interactWithFlagSetter(
      npc,
      setter.targetSequence,
      goal,
      pendingFlag.objectiveIndex,
      beforeFingerprint,
      {
        resourceBaseline: beforeState,
        resourceAccountingGoal: travelResourceGoal,
      },
    )) {
      const after = await readAgentState(client);
      recordMilestone(`q${goal.questId}-flag-${flag.number}`, after, {
        npc: npc.label,
        targetSequence: setter.targetSequence,
      });
      return;
    }
  }
  throw new Error(`q${goal.questId} flag ${flag.number} did not advance through any visible script path`);
}

async function interactWithFlagSetter(
  npc,
  targetSequence,
  goal,
  flagIndex,
  beforeFingerprint,
  { resourceBaseline = null, resourceAccountingGoal = null } = {},
) {
  for (let attempt = 0; attempt < 4; attempt += 1) {
    let state = await readAgentState(client);
    if (state.activeNpcDialog) await closeNpcDialog();
    state = await readAgentState(client);
    let entity = routeNpcEntity(state, npc, 5);
    if (!entity) {
      await navigateNear(npc, 4, {
        maxAttempts: 120,
        resourceBaseline,
        resourceAccountingGoal,
      }).catch((error) => {
        if (error instanceof CombatResourceBudgetError) throw error;
        return false;
      });
      state = await readAgentState(client);
      entity = routeNpcEntity(state, npc, 5);
    }
    if (!entity) continue;
    const clicked = await clickEntity(String(entity.objectId), {
      action: "interact-visible-quest-script-object",
      questId: goal.questId,
      npc: npc.label,
      objectId: String(entity.objectId),
    });
    if (!clicked) continue;
    await waitUntil(
      client,
      `window.__mir2Stage5?.state?.activeNpcDialog != null || ${questObjectiveChangedExpression(goal.questId, flagIndex, beforeFingerprint)}`,
      12_000,
    );
    if (await client.evaluate(questObjectiveChangedExpression(goal.questId, flagIndex, beforeFingerprint))) {
      return true;
    }

    let pathFailed = false;
    for (const target of targetSequence) {
      const selector = dialogTargetSelector(target);
      const visible = await waitUntil(
        client,
        `document.querySelector(${JSON.stringify(selector)}) != null`,
        5_000,
      );
      if (!visible) {
        pathFailed = true;
        break;
      }
      await clickDialogTarget(target, `q${goal.questId}-visible-script-${target}`);
      await delay(250);
      if (await client.evaluate(questObjectiveChangedExpression(goal.questId, flagIndex, beforeFingerprint))) {
        return true;
      }
    }
    if (!pathFailed) {
      const advanced = await waitUntil(
        client,
        questObjectiveChangedExpression(goal.questId, flagIndex, beforeFingerprint),
        12_000,
      );
      if (advanced) return true;
    }
  }
  return false;
}

function nextIncompleteFlag(stateQuest, flags) {
  const objectives = stateQuest?.objectives ?? [];
  for (let flagIndex = 0; flagIndex < flags.length; flagIndex += 1) {
    const wanted = normalizeName(flags[flagIndex]?.message);
    let objectiveIndex = objectives.findIndex((objective) => {
      const label = normalizeName(objective?.label);
      return wanted && (label.includes(wanted) || wanted.includes(label));
    });
    if (objectiveIndex < 0) {
      objectiveIndex = Math.max(0, objectives.length - flags.length + flagIndex);
    }
    const objective = objectives[objectiveIndex] ?? null;
    const done = objective?.done === true || (
      Number(objective?.required ?? 0) > 0 &&
      Number(objective?.current ?? 0) >= Number(objective.required)
    );
    if (!done) return { flagIndex, objectiveIndex };
  }
  return null;
}

function questObjectiveFingerprint(stateQuest, objectiveIndex) {
  const objective = stateQuest?.objectives?.[objectiveIndex] ?? null;
  return JSON.stringify({
    stage: normalizedQuestStage(stateQuest?.stage),
    current: Number(objective?.current ?? 0),
    required: Number(objective?.required ?? 0),
    done: objective?.done === true,
  });
}

function questObjectiveChangedExpression(questId, objectiveIndex, beforeFingerprint) {
  return `(() => { const q = (window.__mir2Stage5?.state?.questLog ?? []).find((entry) => Number(entry?.questId) === ${Number(questId)}); const o = q?.objectives?.[${Number(objectiveIndex)}] ?? null; const value = JSON.stringify({stage:String(q?.stage ?? '').replace(/[^a-z]/gi, '').toLowerCase(),current:Number(o?.current ?? 0),required:Number(o?.required ?? 0),done:o?.done === true}); return value !== ${JSON.stringify(beforeFingerprint)}; })()`;
}

async function executeHuntGoal(goal, resourceBaseline = null) {
  let state = await readAgentState(client);
  resourceBaseline ??= state;
  if (state.activeNpcDialog) {
    await closeNpcDialog();
    state = await readAgentState(client);
  }
  if (goal.targetMapFileName && String(state.mapFileName) !== String(goal.targetMapFileName)) {
    await travelToMap(goal.targetMapFileName, {
      resourceBaseline,
      resourceAccountingGoal: goal,
    });
    state = await readAgentState(client);
  }
  if (rememberQuestCombatResourceStrain(goal, resourceBaseline, state)) {
    const cooledFields = coolDownQuestRespawnFieldsAtPosition(
      goal,
      state,
      state.player,
    );
    console.log(`  cooldown strained ${goal.monsterName} field groups: ${cooledFields}`);
    throw new Error(`${goal.monsterName} travel exceeded the sustainable combat resource budget`);
  }
  const wantedItem = goal.itemName ?? (goal.questId === 2 ? "GingerTea" : null);
  if (wantedItem && !goal.harvest && await collectQuestItemIfVisible(wantedItem, goal, 500)) {
    return;
  }
  let target = await findMonster(
    goal.monsterName,
    goal.fields,
    goal,
    resourceBaseline,
  );
  if (!target) throw new Error(`no live ${goal.monsterName} found through normal roaming`);

  let harvest = null;
  let combatSettled = false;
  let wantedItemProgressBeforeLastKill = null;
  // A harvest field can contain several same-name attackers. If one begins
  // attacking while the first corpse is being processed, immediately switch
  // to that live object and finish its ordinary client combat before touching
  // another corpse. Returning to the outer policy first lets recovery/supply
  // logic mistake a still-adjacent attacker for a safe stationary wait and can
  // burn the complete potion stack. Keep the chain bounded so a dense pack
  // still trips the normal resource budget rather than becoming an open-ended
  // combat loop.
  for (let engagement = 0; engagement < 4; engagement += 1) {
    const stateBefore = await readAgentState(client);
    if (rememberQuestCombatResourceStrain(goal, resourceBaseline, stateBefore)) {
      const cooledFields = coolDownQuestRespawnFieldsAtPosition(
        goal,
        stateBefore,
        stateBefore.player,
      );
      console.log(`  cooldown strained ${goal.monsterName} field groups: ${cooledFields}`);
      throw new Error(`${goal.monsterName} search exceeded the sustainable combat resource budget`);
    }
    const questBefore = questState(stateBefore, goal.questId);
    const monsterBefore = objectiveProgress(
      questBefore,
      goal.itemName ?? goal.monsterName,
    );
    if (wantedItem && !goal.harvest) {
      wantedItemProgressBeforeLastKill = monsterBefore;
    }
    const experienceBefore = Number(stateBefore.playerExperience ?? 0);
    const since = Date.now();
    const killed = await killMonster(
      target,
      goal,
      monsterBefore,
      experienceBefore,
      since,
      resourceBaseline,
    );
    if (!killed.success) {
      throw new Error(killed.reason ?? `${goal.monsterName} kill was not confirmed`);
    }

    harvest = goal.harvest
      ? await harvestCorpse(killed.corpse ?? target, goal, monsterBefore)
      : null;

    let after = await readAgentState(client);
    if (await collectNearbyGoldIfVisible(after).catch(() => false)) {
      after = await readAgentState(client);
    }
    const killRecord = {
      questId: goal.questId,
      monsterName: goal.monsterName,
      objectId: String(target.objectId),
      harvested: goal.harvest,
      harvestCompleted: harvest?.completed ?? null,
      harvestProgressed: harvest?.progressed ?? null,
      experienceBefore,
      experienceAfter: after.playerExperience,
      at: Date.now(),
    };
    evidence.kills.push(killRecord);
    if (
      goal.kind !== "grind" ||
      Number(stateBefore.playerLevel ?? 0) !== Number(after.playerLevel ?? 0) ||
      evidence.kills.length % GRIND_SCREENSHOT_SAMPLE_INTERVAL === 0
    ) {
      recordMilestone(`q${goal.questId}-${goal.harvest ? "harvest" : "kill"}-${goal.monsterName}`, after, {
        harvestCompleted: harvest?.completed ?? null,
        harvestProgressed: harvest?.progressed ?? null,
        grindCheckpoint: goal.kind === "grind",
      });
    }

    if (!goal.harvest) {
      combatSettled = true;
      break;
    }
    if (harvest.completed && !harvest.progressed) {
      console.log(
        `  harvest completed without ${goal.monsterName} quest-drop progress; ` +
        "checking field threats before continuing",
      );
    }

    const reportedThreat = harvest.interruptedByThreat ? harvest.threat : null;
    const activeThreat = reportedThreat ?? nearestActiveHostile(after, {
      excludeObjectId: killed.corpse?.objectId ?? target.objectId,
      maxDistance: 8,
      withinMs: ACTIVE_TRAVEL_THREAT_WINDOW_MS,
    });
    const liveHarvestThreat = activeThreat
      ? after.entities.find((entry) => (
          String(entry.objectId) === String(activeThreat.objectId) &&
          !entityIsCorpse(entry)
        )) ?? null
      : null;
    if (
      liveHarvestThreat &&
      normalizeName(liveHarvestThreat.name) === normalizeName(goal.monsterName) &&
      chebyshev(after.player, liveHarvestThreat) <= 8
    ) {
      console.log(
        `  clear active harvest threat before next corpse: ` +
        `${liveHarvestThreat.name} ${liveHarvestThreat.objectId}@` +
        `${liveHarvestThreat.x},${liveHarvestThreat.y}`,
      );
      target = liveHarvestThreat;
      continue;
    }
    if (!harvest.completed) {
      if (harvest.interruptedByThreat) {
        const resumed = await resumeHarvestAfterCertifiedThreats({
          goal,
          corpse: killed.corpse ?? target,
          objectiveBefore: monsterBefore,
          initialThreat: liveHarvestThreat ?? activeThreat,
          resourceBaseline,
        });
        if (resumed.harvest) {
          harvest = resumed.harvest;
          killRecord.harvestCompleted = harvest.completed;
          killRecord.harvestProgressed = harvest.progressed;
          after = await readAgentState(client);
          recordMilestone(`q${goal.questId}-harvest-resumed-${goal.monsterName}`, after, {
            harvestCompleted: harvest.completed,
            harvestProgressed: harvest.progressed,
            defendedThreats: resumed.defendedThreatIds,
            disengagedThreats: resumed.disengagedThreatIds,
          });
          if (harvest.completed) {
            if (!harvest.progressed) {
              console.log(
                `  resumed harvest completed without ${goal.monsterName} quest-drop progress`,
              );
            }
            combatSettled = true;
            break;
          }
        }
        if (resumed.unsafeThreat) {
          await disengageFromUnsafeHarvestThreat(
            goal,
            await readAgentState(client),
            resumed.unsafeThreat,
            resourceBaseline,
          );
        }
        throw new Error(
          resumed.exhausted
            ? `${goal.monsterName} harvest remained under active attack after 4 bounded defences`
            : `${goal.monsterName} harvest was preempted by active ` +
              `${resumed.unsafeThreat?.name ?? harvest.threat?.name ?? "field"} threat`,
        );
      }
      throw new Error(`${goal.monsterName} died but its harvest lifecycle was not completed`);
    }
    combatSettled = true;
    break;
  }
  if (!combatSettled) {
    throw new Error(
      `${goal.monsterName} harvest remained under active attack after 4 bounded engagements`,
    );
  }

  // One unlucky pack or collision trap is not evidence that the character
  // needs an entire extra level. A successful normal-client engagement proves
  // the source is sustainable again and breaks the consecutive-strain chain.
  recordCombatResourceRecovery(
    goal.monsterName,
    "successful-normal-client-engagement",
  );
  questMonsterResourceStrains.delete(normalizeName(goal.monsterName));

  if (wantedItem && !goal.harvest) {
    const collected = await collectQuestItemIfVisible(wantedItem, goal, 2_500);
    const finalState = await readAgentState(client);
    const finalQuest = questState(finalState, goal.questId);
    const finalStage = normalizedQuestStage(finalQuest?.stage);
    const authoritativeQuestDropProgressed =
      wantedItemProgressBeforeLastKill != null &&
      (
        objectiveProgress(finalQuest, wantedItem) > wantedItemProgressBeforeLastKill ||
        ["readytoturnin", "completed"].includes(finalStage)
      );
    if (!collected && authoritativeQuestDropProgressed) {
      // Crystal Q-drops can be credited straight into the quest container on
      // the normal kill reply instead of appearing as a pickable world item.
      // Accept only the rendered authoritative objective/stage increase; this
      // remains ordinary client play and is not a task mutation shortcut.
      console.log(
        `  authoritative Q-drop progress: ${wantedItem} credited by the normal kill lifecycle`,
      );
    } else if (!collected) {
      throw new Error(
        `${goal.monsterName} was killed, but ${wantedItem} neither appeared as a visible ` +
        "ground drop nor advanced authoritative Q-drop progress",
      );
    }
  }
}

function completedQuestCertifiesMonster(state, monsterName) {
  const wanted = normalizeName(monsterName);
  // The compact q1-q9 policy predates the generated post-tutorial route, so
  // retain its exact source-backed combat certifications here. In particular,
  // q2 renders only the GingerTea item label even though Scarecrow is its sole
  // source; the other rows are explicit kill/harvest objectives.
  const bichonCertificationQuest = new Map([
    [normalizeName("Scarecrow"), 2],
    [normalizeName("Deer"), 4],
    [normalizeName("HookingCat"), 6],
    [normalizeName("Oma"), 8],
    [normalizeName("RakingCat"), 8],
  ]).get(wanted);
  if (
    bichonCertificationQuest != null &&
    questIsCompleted(state, bichonCertificationQuest)
  ) {
    return true;
  }
  return (authoritativeRoute?.quests ?? []).some((quest) => (
    questIsCompleted(state, Number(quest.questId)) &&
    (
      (quest.objectives?.kill ?? []).some(
        (objective) => normalizeName(objective.monsterName) === wanted,
      ) ||
      (quest.objectives?.item ?? []).some((objective) => (
        (objective.sources ?? []).some(
          (source) => normalizeName(source.monsterName) === wanted,
        )
      ))
    )
  ));
}

function canDefendHarvestThreat(state, threat) {
  if (!state?.player || !threat || entityIsCorpse(threat)) return false;
  const threatProfile = grindingCatalog.find(
    (entry) => normalizeName(entry.monsterName) === normalizeName(threat.name),
  ) ?? null;
  if (
    threatProfile &&
    incidentalTravelThreatIsTrivial(
      threatProfile.level,
      Number(state.playerLevel ?? 0),
    )
  ) return true;

  const maxHp = Number(state.playerMaxHp ?? 0);
  const healthRatio = maxHp > 0 ? Number(state.playerHp ?? 0) / maxHp : 0;
  return completedQuestCertifiesMonster(state, threat.name) &&
    // The shared movement frame can place an attacker two cells away by the
    // time its just-landed melee animation is observed. This remains a bounded
    // normal-client chase, not a reason to abandon the adjacent corpse.
    chebyshev(state.player, threat) <= 2 &&
    healthRatio >= 0.75;
}

async function lureCertifiedHarvestThreatAwayFromCorpse({
  goal,
  corpse,
  threat,
  resourceBaseline = null,
}) {
  let state = await readAgentState(client);
  let liveThreat = state.entities.find((entry) => (
    String(entry.objectId) === String(threat.objectId) &&
    !entityIsCorpse(entry)
  )) ?? null;
  if (!liveThreat) return { ready: true, threat: null };
  if (chebyshev(liveThreat, corpse) >= HARVEST_DEFENCE_CORPSE_CLEARANCE) {
    return { ready: true, threat: liveThreat };
  }

  const retreat = retreatPointFromHostile(state, liveThreat, 6);
  if (!retreat) return { ready: false, threat: liveThreat };
  console.log(
    `  lure certified harvest threat away from corpse: ${liveThreat.name} ` +
    `${liveThreat.objectId} corpse=${corpse.x},${corpse.y} toward=${retreat.x},${retreat.y}`,
  );
  let moved = false;
  try {
    moved = await navigateNear(retreat, 1, {
      maxAttempts: 6,
      abortOnDeath: true,
      autoUsePotions: true,
      clearTrivialOccupancy: false,
      resourceBaseline,
      resourceAccountingGoal: goal,
    });
  } catch (error) {
    if (
      error instanceof NavigationInterruptedByDeathError ||
      error instanceof CombatResourceBudgetError
    ) throw error;
    console.log(
      `  certified harvest threat lure deferred: ${String(error?.message ?? error)}`,
    );
  }
  if (!moved) {
    state = await readAgentState(client);
    liveThreat = state.entities.find((entry) => (
      String(entry.objectId) === String(threat.objectId) &&
      !entityIsCorpse(entry)
    )) ?? null;
    return { ready: liveThreat == null, threat: liveThreat };
  }

  const separationDeadline = Date.now() + 3_500;
  while (Date.now() < separationDeadline) {
    state = await readAgentState(client);
    liveThreat = state.entities.find((entry) => (
      String(entry.objectId) === String(threat.objectId) &&
      !entityIsCorpse(entry)
    )) ?? null;
    if (!liveThreat) return { ready: true, threat: null };
    if (chebyshev(liveThreat, corpse) >= HARVEST_DEFENCE_CORPSE_CLEARANCE) {
      console.log(
        `  certified harvest threat lured clear: ${liveThreat.objectId} ` +
        `separation=${chebyshev(liveThreat, corpse)}`,
      );
      return { ready: true, threat: liveThreat };
    }
    await delay(150);
  }
  console.log(
    `  certified harvest threat stayed on corpse: ${liveThreat.objectId} ` +
    `separation=${chebyshev(liveThreat, corpse)}`,
  );
  return { ready: false, threat: liveThreat };
}

async function resumeHarvestAfterCertifiedThreats({
  goal,
  corpse,
  objectiveBefore,
  initialThreat,
  resourceBaseline = null,
}) {
  const corpseObjectId = String(corpse.objectId);
  const defendedThreatIds = [];
  const disengagedThreatIds = [];
  let threat = initialThreat;

  // A dense Crystal field can put another already-certified attacker on the
  // corpse immediately after the first defence. Keep the exact corpse and
  // clear at most four real attackers; every pass still goes through visible
  // selection, normal combat, and authoritative harvest acknowledgements.
  for (let defenceAttempt = 0; defenceAttempt < 4; defenceAttempt += 1) {
    let state = await readAgentState(client);
    const liveReportedThreat = threat
      ? state.entities.find((entry) => (
          String(entry.objectId) === String(threat.objectId) &&
          !entityIsCorpse(entry)
        )) ?? null
      : null;
    const activeThreat = liveReportedThreat ?? nearestActiveHostile(state, {
      excludeObjectId: corpseObjectId,
      maxDistance: 8,
      withinMs: ACTIVE_TRAVEL_THREAT_WINDOW_MS,
    });

    if (activeThreat) {
      if (!canDefendHarvestThreat(state, activeThreat)) {
        return {
          harvest: null,
          unsafeThreat: activeThreat,
          exhausted: false,
          defendedThreatIds,
          disengagedThreatIds,
        };
      }
      console.log(
        `  defend interrupted harvest from certified threat: ` +
        `${activeThreat.name} ${activeThreat.objectId}`,
      );
      const lured = await lureCertifiedHarvestThreatAwayFromCorpse({
        goal,
        corpse,
        threat: activeThreat,
        resourceBaseline,
      });
      if (!lured.ready) {
        return {
          harvest: null,
          unsafeThreat: lured.threat ?? activeThreat,
          exhausted: false,
          defendedThreatIds,
          disengagedThreatIds,
        };
      }
      if (!lured.threat) {
        threat = null;
      } else {
        const cleared = await clearAdjacentTravelThreat(
          lured.threat,
          goal,
          resourceBaseline,
        );
        if (cleared) {
          defendedThreatIds.push(String(lured.threat.objectId));
        } else {
          state = await readAgentState(client);
          const remainingThreat = nearestActiveHostile(state, {
            excludeObjectId: corpseObjectId,
            maxDistance: 8,
            withinMs: ACTIVE_TRAVEL_THREAT_WINDOW_MS,
          });
          if (remainingThreat) {
            threat = remainingThreat;
            continue;
          }
          disengagedThreatIds.push(String(activeThreat.objectId));
          console.log(
            `  certified harvest threat disengaged; resume corpse ${corpseObjectId}`,
          );
        }
      }
    }

    const harvest = await harvestCorpse(corpse, goal, objectiveBefore);
    if (harvest.completed || !harvest.interruptedByThreat) {
      return {
        harvest,
        unsafeThreat: null,
        exhausted: false,
        defendedThreatIds,
        disengagedThreatIds,
      };
    }
    threat = harvest.threat;
  }

  return {
    harvest: null,
    unsafeThreat: threat ?? null,
    exhausted: true,
    defendedThreatIds,
    disengagedThreatIds,
  };
}

async function disengageFromUnsafeHarvestThreat(
  goal,
  state,
  threat,
  resourceBaseline = null,
) {
  const cooldownUntil = Date.now() + STALLED_FIELD_GROUP_COOLDOWN_MS;
  const cooledFieldCount = coolDownQuestRespawnFieldsAtPosition(
    goal,
    state,
    threat,
    cooldownUntil,
  );
  for (const entity of matchingLiveMonsters(state, goal.monsterName)) {
    if (chebyshev(entity, threat) <= 18) {
      monsterCooldownUntil.set(String(entity.objectId), cooldownUntil);
    }
  }

  const retreat = retreatPointFromHostile(state, threat, 10);
  console.log(
    `  disengage unsafe harvest threat: ${threat.name} ` +
    `${threat.objectId}@${threat.x},${threat.y} ` +
    `cooldownFields=${cooledFieldCount}`,
  );
  if (!retreat) return false;
  return navigateNear(retreat, 1, {
    maxAttempts: 4,
    abortOnDeath: true,
    autoUsePotions: true,
    resourceBaseline,
    resourceAccountingGoal: goal,
  }).then(
    () => true,
    (error) => {
      if (
        error instanceof NavigationInterruptedByDeathError ||
        error instanceof CombatResourceBudgetError
      ) throw error;
      console.log(`  unsafe harvest disengage deferred: ${String(error?.message ?? error)}`);
      return false;
    },
  );
}

function coolDownQuestRespawnFieldsAtPosition(
  goal,
  state,
  position,
  cooldownUntil = Date.now() + STALLED_FIELD_GROUP_COOLDOWN_MS,
) {
  if (!position) return 0;
  const currentMapFileName = String(state?.mapFileName ?? "");
  const sourceFields = Array.isArray(goal?.fields) && goal.fields.length > 0
    ? goal.fields
    : BICHON_Q1_Q9_ROUTE.fields[goal?.monsterName] ?? [];
  let cooledFieldCount = 0;
  for (const field of sourceFields) {
    if (String(field?.mapFileName ?? currentMapFileName) !== currentMapFileName) continue;
    const center = {
      x: Number(field?.x ?? field?.position?.x),
      y: Number(field?.y ?? field?.position?.y),
    };
    const spread = Math.max(0, Number(field?.spread ?? 0));
    if (!Number.isFinite(center.x) || !Number.isFinite(center.y)) continue;
    if (chebyshev(center, position) > spread + 12) continue;
    const fieldGroupKey = [
      goal.monsterName,
      String(field?.mapFileName ?? currentMapFileName),
      center.x,
      center.y,
    ].join("|");
    fieldGroupCooldownUntil.set(fieldGroupKey, cooldownUntil);
    cooledFieldCount += 1;
  }
  return cooledFieldCount;
}

function rememberQuestCombatResourceStrain(goal, before, after) {
  if (!["hunt", "grind", "travel"].includes(goal.kind)) return false;
  const strain = assessQuestCombatResourceStrain(before, after);
  if (!strain.severe) return false;
  // The bounded field reserve is a generic last-line safety net. Live q28
  // evidence showed that a single crowded engagement can consume five drugs;
  // request a full physical supply return immediately after any measured
  // severe strain instead of spending the remaining escape reserve on the
  // next target.
  potionSupplyRecallRequested = true;
  // Evidence is recorded once, but the safety decision remains true for the
  // rest of this goal. Returning false after the first record let a caller
  // resume the same depleted hunt and consume the remaining potion reserve.
  if (recordedCombatResourceStrainGoals.has(goal)) return true;
  recordedCombatResourceStrainGoals.add(goal);
  const monsterKey = normalizeName(goal.monsterName);
  const playerLevel = Number(after?.playerLevel ?? before?.playerLevel ?? 1);
  if (goal.kind === "travel") {
    const record = {
      questId: Number(goal.questId ?? 0),
      monsterName: String(goal.monsterName ?? "normal-client travel"),
      travelLabel: String(goal.travelLabel ?? "normal-client travel"),
      playerLevel,
      preparationLevel: null,
      ...strain,
      at: Date.now(),
    };
    evidence.combatResourceStrains.push(record);
    console.log(
      `  travel resource risk: ${record.travelLabel} ` +
      `HP=${strain.hp}/${strain.maxHp} potions=${strain.potionsBefore}->${strain.potionsAfter}; ` +
      "returning to visible supply before continuing",
    );
    return true;
  }
  if (goal.kind === "grind") {
    const riskCooldownUntil = Date.now() + 30 * 60_000;
    grindingMonsterRiskUntil.set(monsterKey, riskCooldownUntil);
    const record = {
      questId: Number(goal.questId ?? 0),
      goalKind: "grind",
      monsterName: String(goal.monsterName),
      playerLevel,
      preparationLevel: null,
      riskCooldownUntil,
      ...strain,
      at: Date.now(),
    };
    evidence.combatResourceStrains.push(record);
    console.log(
      `  grind resource risk: ${goal.monsterName} ` +
      `HP=${strain.hp}/${strain.maxHp} potions=${strain.potionsBefore}->${strain.potionsAfter}; ` +
      "cooling down source",
    );
    return true;
  }
  if (Number(goal.questId) <= 0) return true;
  const resourceStrainCount = Number(questMonsterResourceStrains.get(monsterKey) ?? 0) + 1;
  questMonsterResourceStrains.set(monsterKey, resourceStrainCount);
  if (resourceStrainCount < 2) {
    const record = {
      questId: Number(goal.questId),
      monsterName: String(goal.monsterName),
      playerLevel,
      preparationLevel: null,
      consecutiveStrains: resourceStrainCount,
      ...strain,
      at: Date.now(),
    };
    evidence.combatResourceStrains.push(record);
    console.log(
      `  combat resource risk: ${goal.monsterName} ` +
      `HP=${strain.hp}/${strain.maxHp} potions=${strain.potionsBefore}->${strain.potionsAfter}; ` +
      "replenish and retry another real spawn before leveling",
    );
    return true;
  }
  const existingLevel = Number(questMonsterPreparationLevel.get(monsterKey));
  const preparationLevel = Math.max(
    playerLevel + 1,
    Number.isFinite(existingLevel) ? existingLevel : 0,
  );
  questMonsterPreparationLevel.set(monsterKey, preparationLevel);
  const record = {
    questId: Number(goal.questId),
    monsterName: String(goal.monsterName),
    playerLevel,
    preparationLevel,
    consecutiveStrains: resourceStrainCount,
    ...strain,
    at: Date.now(),
  };
  evidence.combatResourceStrains.push(record);
  console.log(
    `  combat resource risk: ${goal.monsterName} ` +
    `HP=${strain.hp}/${strain.maxHp} potions=${strain.potionsBefore}->${strain.potionsAfter}; ` +
    `prepare level ${preparationLevel}`,
  );
  return true;
}

function rememberGrindingSourceStall(goal, goalRecord, before, after) {
  if (goal?.kind !== "grind") return false;
  const monsterKey = normalizeName(goal.monsterName);
  const decision = assessGrindingSourceStall(goal, before, after, {
    failed: Boolean(goalRecord?.error),
    previousStalls: Number(grindingMonsterStalls.get(monsterKey) ?? 0),
    cooldownMs: STALLED_GRIND_SOURCE_COOLDOWN_MS,
  });
  if (decision.progressed) {
    grindingMonsterStalls.delete(monsterKey);
    return false;
  }
  if (!goalRecord?.error) return false;
  grindingMonsterStalls.set(monsterKey, decision.stallCount);
  if (!Number.isFinite(decision.cooldownUntil)) {
    console.log(
      `  grind stall memory: ${goal.monsterName} ` +
      `no EXP ${decision.stallCount}/3`,
    );
    return false;
  }
  grindingMonsterStalls.delete(monsterKey);
  grindingMonsterRiskUntil.set(monsterKey, decision.cooldownUntil);
  const record = {
    monsterName: String(goal.monsterName),
    playerLevel: Number(after?.playerLevel ?? before?.playerLevel ?? 0),
    experienceBefore: Number(before?.playerExperience ?? 0),
    experienceAfter: Number(after?.playerExperience ?? 0),
    consecutiveStalls: decision.stallCount,
    riskCooldownUntil: decision.cooldownUntil,
    at: Date.now(),
  };
  evidence.grindingSourceStalls.push(record);
  console.log(
    `  grind stall memory: ${goal.monsterName} ` +
    `${decision.stallCount} failed goals without EXP; cooling down source`,
  );
  return true;
}

function recordCombatResourceRecovery(monsterName, reason) {
  const monsterKey = normalizeName(monsterName);
  if (
    !monsterKey ||
    (!questMonsterResourceStrains.has(monsterKey) &&
      !questMonsterPreparationLevel.has(monsterKey))
  ) return false;
  evidence.combatResourceRecoveries.push({
    monsterName: String(monsterName),
    reason: String(reason),
    at: Date.now(),
  });
  return true;
}

function restoreAdaptiveCombatMemory(report) {
  const allStrains = [
    ...(Array.isArray(report?.inheritedCombatResourceStrains)
      ? report.inheritedCombatResourceStrains
      : []),
    ...(Array.isArray(report?.combatResourceStrains)
      ? report.combatResourceStrains
      : []),
  ].slice(-128);
  const recoveries = [
    ...(Array.isArray(report?.inheritedCombatResourceRecoveries)
      ? report.inheritedCombatResourceRecoveries
      : []),
    ...(Array.isArray(report?.combatResourceRecoveries)
      ? report.combatResourceRecoveries
      : []),
    ...(Array.isArray(report?.kills)
      ? report.kills.map((record) => ({
          monsterName: record?.monsterName,
          at: record?.at,
          reason: "confirmed-normal-client-kill",
        }))
      : []),
  ].slice(-256);
  const strains = unresolvedCombatResourceStrains(allStrains, recoveries);
  const now = Date.now();
  for (const record of strains) {
    const monsterKey = normalizeName(record?.monsterName);
    if (!monsterKey) continue;
    if (Number(record?.questId) <= 0) {
      // New reports identify grind strain directly. For pre-field reports,
      // infer it only when the q0 record is neither a travel nor a supply
      // action. Preserve the original 30-minute expiry instead of extending
      // a stale failure by another full window on every supervisor restart.
      const legacyGrindRecord =
        record?.goalKind == null &&
        record?.supplyFunding !== true &&
        !record?.travelLabel;
      if (record?.goalKind === "grind" || legacyGrindRecord) {
        const reportedUntil = Number(record?.riskCooldownUntil);
        const recordedAt = Number(record?.at);
        const riskCooldownUntil = Number.isFinite(reportedUntil)
          ? reportedUntil
          : Number.isFinite(recordedAt)
            ? recordedAt + 30 * 60_000
            : 0;
        if (riskCooldownUntil > now) {
          grindingMonsterRiskUntil.set(
            monsterKey,
            Math.max(
              Number(grindingMonsterRiskUntil.get(monsterKey) ?? 0),
              riskCooldownUntil,
            ),
          );
        }
      }
      continue;
    }
    const reportedStrainCount = Number(record?.consecutiveStrains ?? 1);
    const strainCount = Number.isFinite(reportedStrainCount)
      ? Math.max(1, Math.trunc(reportedStrainCount))
      : 1;
    questMonsterResourceStrains.set(
      monsterKey,
      Math.max(Number(questMonsterResourceStrains.get(monsterKey) ?? 0), strainCount),
    );
    const preparationLevel = Number(record?.preparationLevel);
    if (Number.isFinite(preparationLevel) && preparationLevel > 0) {
      questMonsterPreparationLevel.set(
        monsterKey,
        Math.max(
          Number(questMonsterPreparationLevel.get(monsterKey) ?? 0),
          preparationLevel,
        ),
      );
    }
  }
}

function restoreGrindingSourceStallMemory(report) {
  const stalls = [
    ...(Array.isArray(report?.inheritedGrindingSourceStalls)
      ? report.inheritedGrindingSourceStalls
      : []),
    ...(Array.isArray(report?.grindingSourceStalls)
      ? report.grindingSourceStalls
      : []),
  ].slice(-128);
  const now = Date.now();
  for (const record of stalls) {
    const monsterKey = normalizeName(record?.monsterName);
    const riskCooldownUntil = Number(record?.riskCooldownUntil);
    if (!monsterKey || !Number.isFinite(riskCooldownUntil) || riskCooldownUntil <= now) continue;
    grindingMonsterRiskUntil.set(
      monsterKey,
      Math.max(Number(grindingMonsterRiskUntil.get(monsterKey) ?? 0), riskCooldownUntil),
    );
  }
}

async function collectQuestItemIfVisible(itemName, goal, waitMs) {
  const deadline = Date.now() + waitMs;
  let state = await readAgentState(client);
  let drop = null;
  do {
    drop = state.groundDrops
      .filter((entry) => normalizeName(entry.name) === normalizeName(itemName))
      .sort((left, right) => chebyshev(state.player, left) - chebyshev(state.player, right))[0] ?? null;
    if (drop || Date.now() >= deadline) break;
    await delay(250);
    state = await readAgentState(client);
  } while (Date.now() < deadline);
  if (!drop) return false;

  const objectId = String(drop.objectId);
  if (chebyshev(state.player, drop) > 1) {
    const approached = await navigateNear(drop, 1, { maxAttempts: 30 })
      .then(() => true, () => false);
    if (!approached) return false;
    state = await readAgentState(client);
    drop = state.groundDrops.find((entry) => String(entry.objectId) === objectId) ?? null;
    if (!drop || !state.player || chebyshev(state.player, drop) > 1) return false;
  }
  // Picking up a quest drop is still a live-world action: nearby monsters keep
  // attacking while the client waits for the authoritative inventory/quest
  // acknowledgement. Use an ordinary visible belt key before that window when
  // health is already low, rather than dying beside a successfully killed
  // target and losing the drop lifecycle.
  await usePotionIfNeeded(state);
  state = await readAgentState(client);
  if (state.playerDead || state.deathOverlayVisible) return false;
  const inventoryBefore = state.inventoryItems
    .filter((item) => normalizeName(item.name) === normalizeName(itemName))
    .reduce((total, item) => total + Number(item.quantity ?? 1), 0);
  const objectiveBefore = (questState(state, goal.questId)?.objectives ?? [])
    .find((objective) => normalizeName(objective.label).includes(normalizeName(itemName)));
  const objectiveCurrentBefore = Number(objectiveBefore?.current ?? 0);
  let clicked = false;
  try {
    await client.clickSelector(`button.ground-drop-marker[data-object-id="${objectId}"]`, {
      action: "pick-up-quest-drop",
      item: itemName,
      objectId,
    });
    clicked = true;
  } catch {
    // The fallback below handles a marker covered by a corpse or another actor.
  }

  // A CDP mouse click can physically land on a corpse layered above the drop
  // marker even though the requested marker had a visible box. If that exact
  // drop remains after the click, use Crystal's normal nearby Space pickup.
  // The player was already walked into the server's one-tile targeted pickup range;
  // no direct pickUp command or transform mutation is injected.
  if (clicked) await delay(450);
  const afterClick = await readAgentState(client);
  if ((afterClick.groundDrops ?? []).some((entry) => String(entry.objectId) === objectId)) {
    await client.pressKey(" ", "Space", 32, {
      action: "pick-up-quest-drop-underfoot",
      item: itemName,
      objectId,
    });
  }

  const settled = await waitUntil(
    client,
    `(() => {
      const state = window.__mir2Stage5?.state ?? {};
      const quantity = (state.inventoryItems ?? [])
        .filter((item) => String(item?.name ?? '').replace(/[^a-z0-9]/gi, '').toLowerCase() === ${JSON.stringify(normalizeName(itemName))})
        .reduce((total, item) => total + Number(item?.quantity ?? 1), 0);
      const quest = (state.questLog ?? []).find((entry) => Number(entry?.questId) === ${Number(goal.questId)});
      const stage = String(quest?.stage ?? '').replace(/[^a-z]/gi, '').toLowerCase();
      const objective = (quest?.objectives ?? []).find((entry) =>
        String(entry?.label ?? '').replace(/[^a-z0-9]/gi, '').toLowerCase().includes(${JSON.stringify(normalizeName(itemName))})
      );
      const dead = Number(state.playerHp ?? 0) <= 0 || document.querySelector('[data-testid="town-revive-button"]') != null;
      return quantity > ${inventoryBefore} || Number(objective?.current ?? 0) > ${objectiveCurrentBefore} || stage === 'readytoturnin' || dead;
    })()`,
    12_000,
  );
  const after = await readAgentState(client);
  const inventoryAfter = after.inventoryItems
    .filter((item) => normalizeName(item.name) === normalizeName(itemName))
    .reduce((total, item) => total + Number(item.quantity ?? 1), 0);
  const questAfter = questState(after, goal.questId);
  const objectiveAfter = (questAfter?.objectives ?? [])
    .find((objective) => normalizeName(objective.label).includes(normalizeName(itemName)));
  const collected = inventoryAfter > inventoryBefore ||
    Number(objectiveAfter?.current ?? 0) > objectiveCurrentBefore ||
    normalizedQuestStage(questAfter?.stage) === "readytoturnin";
  if (!settled || !collected) {
    if (after.playerDead || after.deathOverlayVisible) {
      console.log(`  pickup interrupted by death: ${itemName} ${objectId}`);
      return false;
    }
    throw new Error(`${itemName} was visible but normal pickup was not confirmed`);
  }

  evidence.lootPickups.push({
    questId: goal.questId,
    itemName,
    objectId,
    x: drop.x,
    y: drop.y,
    at: Date.now(),
  });
  recordMilestone(`q${goal.questId}-pickup-${itemName}`, after, { objectId });
  await collectNearbyGoldIfVisible(after).catch((error) => {
    console.warn(`  optional nearby gold pickup deferred: ${String(error?.message ?? error)}`);
  });
  return true;
}

async function collectNearbyGoldIfVisible(providedState = null, maxDistance = 8) {
  let state = providedState ?? await readAgentState(client);
  if (state.playerDead || state.deathOverlayVisible) return false;
  const now = Date.now();
  for (const [objectId, until] of groundDropCooldownUntil) {
    if (until <= now) groundDropCooldownUntil.delete(objectId);
  }
  let drop = nearestGroundDropByName(
    state,
    "Gold",
    maxDistance,
    groundDropCooldownUntil.keys(),
  );
  if (!drop) return false;

  const objectId = String(drop.objectId);
  if (chebyshev(state.player, drop) > 1) {
    const approached = await navigateNear(drop, 1, {
      maxAttempts: Math.min(10, Math.max(4, chebyshev(state.player, drop) + 2)),
      abortOnDeath: true,
      failFastWhenCollisionPathUnavailable: true,
    }).then(() => true, () => false);
    if (!approached) {
      groundDropCooldownUntil.set(
        objectId,
        Date.now() + OPTIONAL_DROP_UNREACHABLE_COOLDOWN_MS,
      );
      return false;
    }
    state = await readAgentState(client);
    drop = state.groundDrops.find((entry) => String(entry.objectId) === objectId) ?? null;
    if (!drop || !state.player || chebyshev(state.player, drop) > 1) {
      groundDropCooldownUntil.set(
        objectId,
        Date.now() + OPTIONAL_DROP_UNREACHABLE_COOLDOWN_MS,
      );
      return false;
    }
  }
  const goldBefore = Number(state.gold ?? 0);
  let clicked = false;
  try {
    await client.clickSelector(`button.ground-drop-marker[data-object-id="${objectId}"]`, {
      action: "pick-up-nearby-gold",
      item: "Gold",
      objectId,
    });
    clicked = true;
  } catch {
    // The normal nearby Space pickup below handles a covered gold marker.
  }
  if (clicked) await delay(300);
  state = await readAgentState(client);
  if (
    !state.playerDead &&
    !state.deathOverlayVisible &&
    (state.groundDrops ?? []).some((entry) => String(entry.objectId) === objectId)
  ) {
    await client.pressKey(" ", "Space", 32, {
      action: "pick-up-nearby-gold-underfoot",
      item: "Gold",
      objectId,
    });
  }

  await waitUntil(
    client,
    `(() => { const s = window.__mir2Stage5?.state ?? {}; return Number(s.gold ?? 0) > ${goldBefore} || Number(s.playerHp ?? 0) <= 0 || document.querySelector('[data-testid="town-revive-button"]') != null; })()`,
    5_000,
  );
  const after = await readAgentState(client);
  const goldAfter = Number(after.gold ?? 0);
  if (goldAfter <= goldBefore) {
    groundDropCooldownUntil.set(
      objectId,
      Date.now() + OPTIONAL_DROP_REJECTED_COOLDOWN_MS,
    );
    return false;
  }
  groundDropCooldownUntil.delete(objectId);

  const pickup = {
    objectId,
    x: Number(drop.x),
    y: Number(drop.y),
    visibleQuantity: Number(drop.quantity ?? 0),
    goldBefore,
    goldAfter,
    at: Date.now(),
  };
  evidence.goldPickups.push(pickup);
  recordMilestone("gold-picked-up", after, pickup);
  console.log(`  visible gold pickup: ${goldBefore}->${goldAfter}`);
  return true;
}

async function collectVisibleProgressionSkillBookIfNeeded(providedState = null) {
  let state = providedState ?? await readAgentState(client);
  if (state.playerDead || state.deathOverlayVisible || !state.player) return false;
  const now = Date.now();
  for (const [objectId, until] of groundDropCooldownUntil) {
    if (until <= now) groundDropCooldownUntil.delete(objectId);
  }
  const known = new Set((state.knownSkills ?? []).map((skill) => normalizeName(skill.name)));
  const held = new Set((state.inventoryItems ?? []).map((item) => normalizeName(item.name)));
  const eligible = progressionSkillBookCatalog.filter((book) =>
    Number(state.playerLevel ?? 0) >= Number(book.minLevel ?? 0) &&
    !known.has(normalizeName(book.name)) &&
    !held.has(normalizeName(book.name))
  );
  let drop = eligible
    .map((book) => nearestGroundDropByName(
      state,
      book.name,
      8,
      groundDropCooldownUntil.keys(),
    ))
    .filter(Boolean)
    .sort((left, right) => chebyshev(state.player, left) - chebyshev(state.player, right))[0] ?? null;
  if (!drop) return false;

  const objectId = String(drop.objectId);
  const itemName = String(drop.name);
  if (chebyshev(state.player, drop) > 1) {
    const approached = await navigateNear(drop, 1, {
      maxAttempts: Math.min(10, Math.max(4, chebyshev(state.player, drop) + 2)),
      abortOnDeath: true,
      failFastWhenCollisionPathUnavailable: true,
    }).then(() => true, () => false);
    if (!approached) {
      groundDropCooldownUntil.set(objectId, Date.now() + OPTIONAL_DROP_UNREACHABLE_COOLDOWN_MS);
      return false;
    }
    state = await readAgentState(client);
    drop = state.groundDrops.find((entry) => String(entry.objectId) === objectId) ?? null;
    if (!drop || !state.player || chebyshev(state.player, drop) > 1) return false;
  }

  const quantityBefore = (state.inventoryItems ?? [])
    .filter((item) => normalizeName(item.name) === normalizeName(itemName))
    .reduce((total, item) => total + Math.max(1, Number(item.quantity ?? 1)), 0);
  let clicked = false;
  try {
    await client.clickSelector(`button.ground-drop-marker[data-object-id="${objectId}"]`, {
      action: "pick-up-progression-skill-book",
      item: itemName,
      objectId,
    });
    clicked = true;
  } catch {
    // Space below is the normal-client fallback for an overlapped marker.
  }
  if (clicked) await delay(300);
  state = await readAgentState(client);
  if (
    !state.playerDead &&
    !state.deathOverlayVisible &&
    (state.groundDrops ?? []).some((entry) => String(entry.objectId) === objectId)
  ) {
    await client.pressKey(" ", "Space", 32, {
      action: "pick-up-progression-skill-book-underfoot",
      item: itemName,
      objectId,
    });
  }
  const pickedUp = await waitUntil(
    client,
    `(() => { const s = window.__mir2Stage5?.state ?? {}; const quantity = (s.inventoryItems ?? []).filter((item) => String(item?.name ?? '').replace(/[^a-z0-9]/gi, '').toLowerCase() === ${JSON.stringify(normalizeName(itemName))}).reduce((total, item) => total + Math.max(1, Number(item?.quantity ?? 1)), 0); return quantity > ${quantityBefore} || Number(s.playerHp ?? 0) <= 0 || document.querySelector('[data-testid="town-revive-button"]') != null; })()`,
    5_000,
  );
  const after = await readAgentState(client);
  const quantityAfter = (after.inventoryItems ?? [])
    .filter((item) => normalizeName(item.name) === normalizeName(itemName))
    .reduce((total, item) => total + Math.max(1, Number(item.quantity ?? 1)), 0);
  if (!pickedUp || quantityAfter <= quantityBefore) {
    groundDropCooldownUntil.set(objectId, Date.now() + OPTIONAL_DROP_REJECTED_COOLDOWN_MS);
    return false;
  }
  groundDropCooldownUntil.delete(objectId);
  const pickup = {
    questId: null,
    itemName,
    objectId,
    x: Number(drop.x),
    y: Number(drop.y),
    category: "progression-skill-book",
    at: Date.now(),
  };
  evidence.lootPickups.push(pickup);
  recordMilestone("progression-skill-book-picked-up", after, pickup);
  console.log(`  visible progression skill book pickup: ${itemName}`);
  return true;
}

async function collectVisibleSafeSupplyLootIfNeeded(providedState = null) {
  let state = providedState ?? await readAgentState(client);
  if (
    state.playerDead || state.deathOverlayVisible ||
    !localPotionSupplyIncomplete(state)
  ) return false;
  const now = Date.now();
  for (const [objectId, until] of groundDropCooldownUntil) {
    if (until <= now) groundDropCooldownUntil.delete(objectId);
  }
  const retainedEquipmentNames = new Set(
    (state.equipmentItems ?? []).map((item) => String(item.name)),
  );
  const safeLootNames = [
    ...SAFE_STARTER_LIQUIDATION_GEAR.map(({ name }) => name),
    ...safeOrdinarySupplyLootCatalog.map(({ name }) => name),
    ...SAFE_DUPLICATE_EQUIPPED_SUPPLY_LOOT.filter(
      (name) => retainedEquipmentNames.has(name),
    ),
  ];
  let drop = safeLootNames
    .map((name) => nearestGroundDropByName(
      state,
      name,
      8,
      groundDropCooldownUntil.keys(),
    ))
    .filter(Boolean)
    .sort((left, right) => chebyshev(state.player, left) - chebyshev(state.player, right))[0] ?? null;
  if (!drop) return false;

  const objectId = String(drop.objectId);
  if (chebyshev(state.player, drop) > 1) {
    const approached = await navigateNear(drop, 1, {
      maxAttempts: Math.min(10, Math.max(4, chebyshev(state.player, drop) + 2)),
      abortOnDeath: true,
      failFastWhenCollisionPathUnavailable: true,
    }).then(() => true, () => false);
    if (!approached) {
      groundDropCooldownUntil.set(objectId, Date.now() + OPTIONAL_DROP_UNREACHABLE_COOLDOWN_MS);
      return false;
    }
    state = await readAgentState(client);
    drop = state.groundDrops.find((entry) => String(entry.objectId) === objectId) ?? null;
    if (!drop || !state.player || chebyshev(state.player, drop) > 1) return false;
  }

  const itemName = String(drop.name);
  const inventoryQuantity = (snapshot) => (snapshot.inventoryItems ?? [])
    .filter((item) => normalizeName(item.name) === normalizeName(itemName))
    .reduce((total, item) => total + Math.max(1, Number(item.quantity ?? 1)), 0);
  const quantityBefore = inventoryQuantity(state);
  let clicked = false;
  try {
    await client.clickSelector(`button.ground-drop-marker[data-object-id="${objectId}"]`, {
      action: "pick-up-visible-sellable-supply-loot",
      item: itemName,
      objectId,
    });
    clicked = true;
  } catch {
    // A corpse may cover the exact marker; normal nearby Space is the same
    // physical fallback used for quest, gold, and potion drops.
  }
  if (clicked) await delay(300);
  state = await readAgentState(client);
  if (
    !state.playerDead &&
    !state.deathOverlayVisible &&
    (state.groundDrops ?? []).some((entry) => String(entry.objectId) === objectId)
  ) {
    await client.pressKey(" ", "Space", 32, {
      action: "pick-up-visible-sellable-supply-loot-underfoot",
      item: itemName,
      objectId,
    });
  }
  await waitUntil(
    client,
    `(() => { const s = window.__mir2Stage5?.state ?? {}; const quantity = (s.inventoryItems ?? []).filter((item) => String(item?.name ?? '').replace(/[^a-z0-9]/gi, '').toLowerCase() === ${JSON.stringify(normalizeName(itemName))}).reduce((total, item) => total + Math.max(1, Number(item?.quantity ?? 1)), 0); return quantity > ${quantityBefore} || Number(s.playerHp ?? 0) <= 0 || document.querySelector('[data-testid="town-revive-button"]') != null; })()`,
    8_000,
  );
  const after = await readAgentState(client);
  const quantityAfter = inventoryQuantity(after);
  if (quantityAfter <= quantityBefore) {
    groundDropCooldownUntil.set(objectId, Date.now() + OPTIONAL_DROP_REJECTED_COOLDOWN_MS);
    return false;
  }
  groundDropCooldownUntil.delete(objectId);
  const pickup = {
    kind: "sellable-supply-loot",
    itemName,
    objectId,
    x: Number(drop.x),
    y: Number(drop.y),
    quantityBefore,
    quantityAfter,
    at: Date.now(),
  };
  evidence.supplyPickups.push(pickup);
  recordMilestone("sellable-supply-loot-picked-up", after, pickup);
  // Deer harvest acknowledgement and the physical item drop can be separated
  // by a later world frame. Once Venison is actually visible and collected,
  // that is stronger evidence than the temporary no-inventory cooldown: allow
  // the deterministic funding source to be used for the remaining stock.
  if (normalizeName(itemName) === normalizeName("Venison")) {
    deerFundingUnavailableUntil = 0;
  }
  console.log(`  visible sellable supply pickup: ${itemName} ${quantityBefore}->${quantityAfter}`);
  return true;
}

async function openNpcDialog(
  npc,
  requiredTarget = null,
  {
    clearTrivialOccupancy = false,
    resourceBaseline = null,
    resourceAccountingGoal = null,
  } = {},
) {
  let state = await readAgentState(client);
  const existing = state.activeNpcDialog;
  if (
    existing &&
    nearNpcDialog(existing, state.entities, npc) &&
    dialogHasTarget(existing, requiredTarget)
  ) return existing;
  if (existing) await closeNpcDialog();

  for (let attempt = 0; attempt < 5; attempt += 1) {
    state = await readAgentState(client);
    const entity = routeNpcEntity(state, npc, 5);
    if (!entity && await approachNpcViaVisibleWaypoint(state, npc, {
      clearTrivialOccupancy,
      resourceBaseline,
      resourceAccountingGoal,
    })) {
      continue;
    }
    if (entity) {
      const beforeClick = Date.now();
      const clicked = await clickEntity(String(entity.objectId), {
        action: "interact-npc", npc: npc.label, objectId: String(entity.objectId),
      });
      if (!clicked) {
        await navigateNear(entity, 4, {
          maxAttempts: 32,
          clearTrivialOccupancy,
          resourceBaseline,
          resourceAccountingGoal,
        });
        continue;
      }
      // The sprite hit surface activates on mouse-down, while the nameplate
      // activates on click. If React's short NPC call guard consumed the first
      // physical press (common immediately after a long arrival), retry once
      // through the other visible surface after the guard expires. Confirm the
      // retry from the observed outgoing command rather than assuming that a
      // geometrically successful CDP click reached gameplay.
      await delay(180);
      let clickCommands = outgoingGameplayCommandsSince(beforeClick);
      let sentInteract = clickCommands.some(
        (entry) => entry.type === "interact" && String(entry.objectId) === String(entity.objectId),
      );
      if (!sentInteract) {
        // A distant physical click emits normal Walk/Run first. Wait only for
        // the client to enter interaction range, then re-read the rendered
        // position and press the alternate visible surface immediately. This
        // avoids both a forced one-tile path through crowded spawn cells and
        // the former 35-second blind wait after every approach click.
        await waitUntil(
          client,
          `(() => { const s = window.__mir2Stage5?.state; const p = s?.player; const n = (s?.entities ?? []).find((entry) => String(entry?.objectId) === ${JSON.stringify(String(entity.objectId))}); return String(s?.activeNpcDialog?.npcObjectId ?? '') === ${JSON.stringify(String(entity.objectId))} || (p && n && Math.max(Math.abs(Number(p.x)-Number(n.x)), Math.abs(Number(p.y)-Number(n.y))) <= 1); })()`,
          12_000,
        );
        state = await readAgentState(client);
      }
      if (
        resourceBaseline && resourceAccountingGoal &&
        rememberQuestCombatResourceStrain(
          resourceAccountingGoal,
          resourceBaseline,
          state,
        )
      ) {
        throw new CombatResourceBudgetError(
          `${resourceAccountingGoal.monsterName} interaction approach exceeded the sustainable combat resource budget`,
        );
      }
      let arrivedEntity = routeNpcEntity(state, npc, 5);
      if (
        !sentInteract && arrivedEntity &&
        chebyshev(state.player, arrivedEntity) > 1
      ) {
        // The click-to-interact plan can stop two or three cells away when a
        // building edge or moving actor invalidates its final segment. A
        // four-tile visibility radius is not interaction range: finish the
        // approach with the same collision-aware keyboard navigation before
        // retrying the visible nameplate.
        await navigateNear(arrivedEntity, 1, {
          maxAttempts: 16,
          clearTrivialOccupancy,
          resourceBaseline,
          resourceAccountingGoal,
        }).catch((error) => {
          if (error instanceof CombatResourceBudgetError) throw error;
          return false;
        });
        state = await readAgentState(client);
        arrivedEntity = routeNpcEntity(state, npc, 5);
      }
      if (!sentInteract && arrivedEntity && chebyshev(state.player, arrivedEntity) <= 1) {
        await delay(700);
        await client.clickSelector(
          `button.entity-nameplate[data-object-id="${String(arrivedEntity.objectId)}"][data-ui-interactive="true"]`,
          {
            action: "interact-npc-nameplate-retry",
            npc: npc.label,
            objectId: String(arrivedEntity.objectId),
          },
        ).catch(() => null);
        await delay(180);
        clickCommands = outgoingGameplayCommandsSince(beforeClick);
        sentInteract = clickCommands.some(
          (entry) => entry.type === "interact" && String(entry.objectId) === String(entity.objectId),
        );
      }
      console.log(
        `  NPC input ${npc.label}: interact=${sentInteract} ` +
        `commands=[${clickCommands.map((entry) => String(entry.type ?? "unknown")).join(",")}]`,
      );
      const activated = await waitUntil(
        client,
        `String(window.__mir2Stage5?.state?.activeNpcDialog?.npcObjectId ?? '') === ${JSON.stringify(String(entity.objectId))} || window.__mir2Stage5?.state?.movementPlan != null`,
        2_000,
      );
      const sentAction = client.outgoingCommandAudit().commands.some(
        (entry) => entry.at >= beforeClick && ["walk", "run", "interact"].includes(entry.type),
      );
      if (activated || sentAction) {
        const opened = await waitUntil(
          client,
          `String(window.__mir2Stage5?.state?.activeNpcDialog?.npcObjectId ?? '') === ${JSON.stringify(String(entity.objectId))}`,
          sentInteract ? 12_000 : 2_000,
        );
        if (opened) {
          const dialog = (await readAgentState(client)).activeNpcDialog;
          if (dialogHasTarget(dialog, requiredTarget)) return dialog;
          console.log(
            `  NPC dialog ${npc.label} omitted ${requiredTarget ?? "requested target"}; ` +
            `targets=[${(dialog?.links ?? []).map((link) => String(link.target)).join(",")}]`,
          );
          await closeNpcDialog();
          await delay(450);
        }
      }
    }
    // A route NPC can legitimately begin outside the rectangular Zone AOI.
    // Coordinate-only travel must finish adjacent to the authoritative NPC
    // tile: stopping four cells away can still leave a static NPC outside the
    // rendered entity set, so every retry would repeat without ever exposing a
    // physical interaction surface. `navigateNear` will not enter the target
    // tile at distance one, which keeps this safe even before the entity loads.
    // One navigation attempt advances at most one authoritative movement
    // input. A fixed 48-attempt budget cannot reach a distant static NPC: the
    // q28 village-to-Samuel return is over 300 tiles and previously stopped
    // roughly forty tiles short after exhausting all five dialog retries.
    // Scale the coordinate-only budget from the current visible distance and
    // leave bounded room for collision detours. This changes only the number
    // of ordinary mouse/keyboard moves; it never relocates the player.
    const coordinateDistance = chebyshev(state.player, npc);
    if (!entity && clearTrivialOccupancy && coordinateDistance >= 80) {
      const travelHazards = aggressiveRespawnTravelHazards(state);
      const corridorWaypoint = respawnCorridorAvoidanceWaypoint(
        state.player,
        npc,
        travelHazards,
        {
          minimumImprovementRatio: 0.9,
          minimumLegDistance: 24,
          perpendicularOffsets: [24, 40, 64, 96, 128],
          progressRatios: [0.33, 0.5, 0.67],
        },
      );
      if (corridorWaypoint) {
        console.log(
          `  NPC hostile-corridor detour: ${npc.label} via ` +
          `${corridorWaypoint.x},${corridorWaypoint.y} ` +
          `exposure=${Number(corridorWaypoint.directExposure).toFixed(1)}->` +
          `${Number(corridorWaypoint.detourExposure).toFixed(1)}`,
        );
        const detourDistance = chebyshev(state.player, corridorWaypoint);
        const reachedDetour = await navigateNear(corridorWaypoint, 2, {
          maxAttempts: respawnTravelAttemptBudget(detourDistance),
          clearTrivialOccupancy,
          resourceBaseline,
          resourceAccountingGoal,
        }).catch((error) => {
          if (error instanceof CombatResourceBudgetError) throw error;
          console.log(
            `  NPC hostile-corridor detour deferred: ` +
            `${String(error?.message ?? error)}`,
          );
          return false;
        });
        if (reachedDetour) continue;
      }
    }
    const coordinateAttempts = entity
      ? 32
      : Math.min(640, Math.max(48, coordinateDistance + 96));
    await navigateNear(
      entity ?? npc,
      1,
      {
        maxAttempts: coordinateAttempts,
        clearTrivialOccupancy,
        resourceBaseline,
        resourceAccountingGoal,
      },
    ).catch((error) => {
      if (error instanceof CombatResourceBudgetError) throw error;
      return false;
    });
  }
  throw new Error(`could not open ${npc.label} dialog at ${npc.x},${npc.y}`);
}

function aggressiveRespawnTravelHazards(state, excludedMonsterName = null) {
  const playerLevel = Number(state?.playerLevel ?? 0);
  return grindingCatalog
    .filter((entry) => (
      normalizeName(entry.monsterName) !== normalizeName(excludedMonsterName) &&
      // Crystal AI 1/2 are the passive Hen/Deer families. Other
      // experience-bearing, non-boss source spawns can proactively engage.
      ![1, 2].includes(Number(entry.ai)) &&
      Number(entry.level) >= playerLevel - 3
    ))
    .flatMap((entry) => (entry.spawns ?? [])
      .filter((spawn) => String(spawn.mapFileName) === String(state?.mapFileName))
      .map((spawn) => ({
        x: Number(spawn.position?.x),
        y: Number(spawn.position?.y),
        count: Number(spawn.count),
        spread: Number(spawn.spread),
      })));
}

async function approachNpcViaVisibleWaypoint(
  state,
  targetNpc,
  {
    clearTrivialOccupancy = false,
    resourceBaseline = null,
    resourceAccountingGoal = null,
  } = {},
) {
  if (!state.player) return false;
  const startingDistance = chebyshev(state.player, targetNpc);
  const anchors = state.entities
    .filter((entry) => (
      entry.kind === "npc" &&
      !entityIsCorpse(entry) &&
      String(entry.objectId) !== String(targetNpc.npcIndex) &&
      chebyshev(entry, targetNpc) + 4 < startingDistance
    ))
    .sort((left, right) => chebyshev(left, targetNpc) - chebyshev(right, targetNpc));

  for (const anchor of anchors.slice(0, 3)) {
    const before = `${state.player.x},${state.player.y}`;
    const clicked = await clickEntity(String(anchor.objectId), {
      action: "approach-npc-via-visible-waypoint",
      targetNpc: targetNpc.label,
      waypointNpc: anchor.name,
      objectId: String(anchor.objectId),
    });
    if (clicked) {
      await waitUntil(
        client,
        `(() => { const s = window.__mir2Stage5?.state; const p = s?.player; const n = (s?.entities ?? []).find((entry) => String(entry?.objectId) === ${JSON.stringify(String(anchor.objectId))}); return String(s?.activeNpcDialog?.npcObjectId ?? '') === ${JSON.stringify(String(anchor.objectId))} || (p && n && Math.max(Math.abs(Number(p.x)-Number(n.x)), Math.abs(Number(p.y)-Number(n.y))) <= 2); })()`,
        35_000,
      );
    } else {
      await navigateNear(anchor, 4, {
        maxAttempts: 12,
        clearTrivialOccupancy,
        resourceBaseline,
        resourceAccountingGoal,
      }).catch((error) => {
        if (error instanceof CombatResourceBudgetError) throw error;
        return false;
      });
    }
    const after = await readAgentState(client);
    if (
      resourceBaseline && resourceAccountingGoal &&
      rememberQuestCombatResourceStrain(
        resourceAccountingGoal,
        resourceBaseline,
        after,
      )
    ) {
      throw new CombatResourceBudgetError(
        `${resourceAccountingGoal.monsterName} waypoint exceeded the sustainable combat resource budget`,
      );
    }
    if (after.activeNpcDialog) await closeNpcDialog();
    const moved = after.player && `${after.player.x},${after.player.y}` !== before;
    if (moved && chebyshev(after.player, targetNpc) < startingDistance) return true;
  }
  return false;
}

function dialogHasTarget(dialog, target) {
  if (!target) return true;
  return Array.isArray(dialog?.links) && dialog.links.some((link) => link?.target === target);
}

async function closeNpcDialog() {
  const visible = await client.evaluate("document.querySelector('.npc-dialog-panel') != null");
  if (!visible) return;
  await client.clickSelector(".npc-dialog-actions button:last-child", { action: "close-npc-dialog" });
  await waitUntil(client, "document.querySelector('.npc-dialog-panel') == null", 5_000);
}

async function clickDialogTarget(target, action) {
  const selector = dialogTargetSelector(target);
  const exists = await waitUntil(client, `document.querySelector(${JSON.stringify(selector)}) != null`, 8_000);
  if (!exists) {
    const state = await readAgentState(client);
    throw new Error(`dialog target ${target} absent; links=${JSON.stringify(state.activeNpcDialog?.links ?? [])}`);
  }

  for (let attempt = 0; attempt < 12; attempt += 1) {
    try {
      await client.clickSelector(selector, { action, dialogTarget: target });
      return;
    } catch (error) {
      if (!String(error?.message ?? error).startsWith("visible element not found:")) throw error;
    }
    const direction = await client.evaluate(`(() => {
      const container = document.querySelector('.npc-dialog-body');
      const target = document.querySelector(${JSON.stringify(selector)});
      if (!(container instanceof HTMLElement) || !(target instanceof HTMLElement)) return 0;
      const containerBox = container.getBoundingClientRect();
      const targetBox = target.getBoundingClientRect();
      return targetBox.bottom < containerBox.top ? -1 : 1;
    })()`);
    const scrolled = await client.wheelSelector(
      ".npc-dialog-body",
      Number(direction) < 0 ? -140 : 140,
      { action: "scroll-npc-dialog", dialogTarget: target, attempt: attempt + 1 },
    );
    if (!scrolled) break;
    await delay(120);
  }

  throw new Error(`dialog target ${target} exists but could not be made physically visible`);
}

function dialogTargetSelector(target) {
  const escaped = String(target).replace(/\\/g, "\\\\").replace(/"/g, '\\"');
  return `.npc-dialog-links button[data-target="${escaped}"]`;
}

async function waitForQuestStage(questId, expected, timeoutMs) {
  return waitUntil(
    client,
    `(() => { const q = (window.__mir2Stage5?.state?.questLog ?? []).find((entry) => Number(entry?.questId) === ${Number(questId)}); return String(q?.stage ?? '').replace(/[^a-z]/gi, '').toLowerCase() === ${JSON.stringify(expected)}; })()`,
    timeoutMs,
  );
}

async function waitForQuestStages(questId, expected, timeoutMs) {
  const normalized = expected.map((stage) => normalizedQuestStage(stage));
  return waitUntil(
    client,
    `(() => { const q = (window.__mir2Stage5?.state?.questLog ?? []).find((entry) => Number(entry?.questId) === ${Number(questId)}); return ${JSON.stringify(normalized)}.includes(String(q?.stage ?? '').replace(/[^a-z]/gi, '').toLowerCase()); })()`,
    timeoutMs,
  );
}

async function findMonster(
  monsterName,
  routeFields = null,
  activeGoal = null,
  resourceBaseline = null,
) {
  assertRuntimeBudget(`searching for ${monsterName}`);
  const resourceSensitiveSearch = Boolean(
    resourceBaseline && activeGoal?.supplyFunding !== true,
  );
  const revivesBeforeSearch = evidence.revives;
  const assertSearchWasNotRevived = () => {
    if (evidence.revives !== revivesBeforeSearch) {
      throw new NavigationInterruptedByDeathError(
        `player died while searching for ${monsterName}; main policy must replan from town`,
      );
    }
  };
  const assertSearchResourceBudget = (liveState) => {
    assertSafeSupplyFundingState(activeGoal, liveState, monsterName);
    if (
      resourceBaseline && activeGoal &&
      rememberQuestCombatResourceStrain(activeGoal, resourceBaseline, liveState)
    ) {
      throw new Error(
        `${monsterName} search exceeded the sustainable combat resource budget`,
      );
    }
  };
  let state = await readAgentState(client);
  assertSearchResourceBudget(state);
  await logMonsterSearch(monsterName, state, "current-aoi");
  const preferNearestSupplyTarget = activeGoal?.supplyFunding === true;
  let found = await nearestVisibleMonsterByName(
    state,
    monsterName,
    preferNearestSupplyTarget,
  );
  if (found) return found;

  const knownTargets = rankMonsterApproachTargets(
    state,
    matchingLiveMonsters(state, monsterName),
    preferNearestSupplyTarget,
  );
  for (const known of knownTargets.slice(0, 4)) {
    assertRuntimeBudget(`approaching ${monsterName}`);
    console.log(`  approach known ${monsterName}: ${known.objectId}@${known.x},${known.y}`);
    let tracked = known;
    const approachPositions = new Map();
    for (let step = 0; step < 18; step += 1) {
      assertRuntimeBudget(`approaching ${monsterName}`);
      assertSearchWasNotRevived();
      state = await readAgentState(client);
      assertSearchResourceBudget(state);
      const positionKey = state.player ? `${state.player.x},${state.player.y}` : "none";
      const visits = (approachPositions.get(positionKey) ?? 0) + 1;
      approachPositions.set(positionKey, visits);
      if (visits >= 3) {
        console.log(`  blocked ${monsterName}: repeated approach position ${positionKey}`);
        break;
      }
      const current = matchingLiveMonsters(state, monsterName)
        .find((entry) => String(entry.objectId) === String(known.objectId));
      if (!current) break;
      tracked = current;
      const desiredDistance = chebyshev(state.player, tracked) <= 4 ? 1 : 4;
      // Eight tiles still projects the sprite beneath the minimap/top HUD on a
      // 1024x768 viewport. Close to four so the real hitbox enters the central
      // interaction surface before deciding that a known target is stale. A
      // moving monster is refreshed by object id on every step; never chase
      // the coordinates from the first snapshot.
      // Keep several attempts in one navigation context so rejected click
      // segments and no-distance-progress detours survive server corrections.
      // Resource-sensitive routes still need four attempts: the collision
      // state machine requires three repeated authoritative positions before
      // it may clear one certified adjacent occupant. The resource baseline is
      // checked before every one of those attempts, so this does not relax the
      // combat budget.
      await navigateNear(tracked, desiredDistance, {
        maxAttempts: resourceSensitiveSearch ? 4 : 6,
        autoUsePotions: activeGoal?.supplyFunding !== true,
        clearTrivialOccupancy: true,
        resourceBaseline,
        resourceAccountingGoal: activeGoal,
      }).catch((error) => {
        if (
          error instanceof SupplyFundingSafetyError ||
          error instanceof CombatResourceBudgetError
        ) throw error;
      });
      state = await readAgentState(client);
      found = await nearestVisibleMonsterByName(
        state,
        monsterName,
        preferNearestSupplyTarget,
      );
      if (found) {
        await logMonsterSearch(monsterName, state, `known-transit-${known.objectId}`);
        return found;
      }
    }
    monsterCooldownUntil.set(String(known.objectId), Date.now() + FAILED_APPROACH_COOLDOWN_MS);
    console.log(`  cooldown ${monsterName}: approach did not converge for ${known.objectId}`);
  }

  const sourceFields = Array.isArray(routeFields) && routeFields.length > 0
    ? routeFields
    : BICHON_Q1_Q9_ROUTE.fields[monsterName] ?? [];
  // Runtime profile level overrides can make a lethal dense spawn look
  // numerically "safe". Rank every multi-region hunt by the same transparent
  // distance/density cost; routes without count/spread naturally reduce to
  // nearest-field ordering.
  const travelHazards = aggressiveRespawnTravelHazards(state, monsterName);
  const orderedSourceFields = rankRespawnFieldsForTravel(
    state.player,
    sourceFields,
    { hazards: travelHazards },
  );
  const fields = expandRespawnPatrolFields(orderedSourceFields, {
    player: state.player,
    hazards: resourceBaseline ? travelHazards : [],
  });
  if (!fields.length) throw new Error(`no real respawn fields configured for ${monsterName}`);
  // orderedSourceFields is freshly ranked from the live player position for
  // every search. Its cursor must therefore be local to this traversal: an
  // index carried into the newly reordered list made the next goal skip the
  // nearest spawn and walk hundreds of tiles to an arbitrary old index.
  let cursor = 0;
  for (let attempt = 0; attempt < Math.max(8, fields.length * 2); attempt += 1) {
    assertRuntimeBudget(`roaming for ${monsterName}`);
    assertSearchWasNotRevived();
    const field = fields[cursor % fields.length];
    cursor += 1;
    const fieldGroupKey = [
      monsterName,
      String(field.mapFileName ?? ""),
      Number(field.patrolCenterX ?? field.x),
      Number(field.patrolCenterY ?? field.y),
    ].join("|");
    if ((fieldGroupCooldownUntil.get(fieldGroupKey) ?? 0) > Date.now()) continue;
    state = await readAgentState(client);
    console.log(`  roam ${monsterName}: field=${field.x},${field.y} player=${state.player?.x},${state.player?.y}`);
    const fieldDistance = chebyshev(state.player, field);
    const fieldAttempts = respawnTravelAttemptBudget(fieldDistance);
    // Long fixed-coordinate walks are only a search direction. Re-scan the
    // visible AOI every few genuine movement attempts so a target encountered
    // en route wins immediately; otherwise the agent can walk past an adjacent
    // quest monster for several minutes while still chasing the field center.
    let reached = false;
    let corridorWaypoint = respawnCorridorAvoidanceWaypoint(
      state.player,
      field,
      travelHazards,
    );
    if (corridorWaypoint) {
      console.log(
        `  avoid hostile respawn corridor: via=${corridorWaypoint.x},${corridorWaypoint.y} ` +
        `exposure=${corridorWaypoint.directExposure.toFixed(1)}->` +
        `${corridorWaypoint.detourExposure.toFixed(1)}`,
      );
    }
    let travelAttempts = 0;
    let bestFieldDistance = fieldDistance;
    let stalledChunks = 0;
    while (!reached && travelAttempts < fieldAttempts) {
      assertRuntimeBudget(`travelling to ${monsterName}`);
      assertSearchWasNotRevived();
      state = await readAgentState(client);
      assertSearchResourceBudget(state);
      if (
        corridorWaypoint &&
        chebyshev(state.player, corridorWaypoint) <= 8
      ) {
        console.log(
          `  hostile corridor waypoint reached: ` +
          `${corridorWaypoint.x},${corridorWaypoint.y}`,
        );
        corridorWaypoint = null;
      }
      found = await nearestVisibleMonsterByName(
        state,
        monsterName,
        preferNearestSupplyTarget,
      );
      if (found) {
        await logMonsterSearch(monsterName, state, `transit-${field.x}-${field.y}`);
        return found;
      }
      const blockingThreat = nearestSafeBlockingHostile(state, monsterName);
      // A travelling player should keep fleeing while the authoritative route
      // is still making progress. Retaliating after every incidental hit lets
      // moving low-level monsters drag the client back into the dense town AOI.
      // Clear one safe attacker only after two genuinely stationary chunks.
      if (blockingThreat && activeGoal && stalledChunks >= 2) {
        if (activeGoal.supplyFunding) {
          supplyFundingShelterUntil = Math.max(
            supplyFundingShelterUntil,
            Date.now() + SUPPLY_FUNDING_THREAT_SHELTER_MS,
          );
          throw new SupplyFundingSafetyError(
            `${blockingThreat.name} interrupted safe potion funding`,
          );
        }
        console.log(
          `  clear stalled adjacent travel threat: ${blockingThreat.name} ` +
          `${blockingThreat.objectId}@${blockingThreat.x},${blockingThreat.y}`,
        );
        const cleared = await clearAdjacentTravelThreat(
          blockingThreat,
          activeGoal,
          resourceBaseline,
        );
        travelAttempts += 1;
        if (cleared) continue;
      }
      // Once roaming reveals the requested monster, give its live object id a
      // complete collision-aware adjacent approach before resuming the patrol.
      // Alternating one step toward the monster and one toward the old field
      // coordinate can otherwise oscillate forever around village obstacles.
      const encountered = rankMonsterApproachTargets(
        state,
        matchingLiveMonsters(state, monsterName)
          .filter((entry) => chebyshev(state.player, entry) <= 16),
        preferNearestSupplyTarget,
      )[0] ?? null;
      if (encountered) {
        const encounterAttempts = resourceSensitiveSearch ? 4 : 6;
        const encounterPlayerBefore = state.player
          ? { x: Number(state.player.x), y: Number(state.player.y) }
          : null;
        await navigateNear(encountered, 1, {
          maxAttempts: encounterAttempts,
          autoUsePotions: activeGoal?.supplyFunding !== true,
          clearTrivialOccupancy: true,
          resourceBaseline,
          resourceAccountingGoal: activeGoal,
        }).catch((error) => {
          if (
            error instanceof SupplyFundingSafetyError ||
            error instanceof CombatResourceBudgetError
          ) throw error;
          return false;
        });
        travelAttempts += encounterAttempts;
        state = await readAgentState(client);
        found = await nearestVisibleMonsterByName(
          state,
          monsterName,
          preferNearestSupplyTarget,
        );
        if (found) {
          await logMonsterSearch(monsterName, state, `encounter-${encountered.objectId}`);
          return found;
        }
        if (
          resourceSensitiveSearch &&
          encounterPlayerBefore &&
          state.player &&
          chebyshev(encounterPlayerBefore, state.player) >= 1
        ) {
          // One-attempt chunks exist so HP and potion loss are re-read after
          // every physical step. A successful step is not a failed approach:
          // keep following the live object on the next bounded chunk instead
          // of hiding it behind a 15-second cooldown.
          continue;
        }
        monsterCooldownUntil.set(String(encountered.objectId), Date.now() + FAILED_APPROACH_COOLDOWN_MS);
        continue;
      }

      // Re-scan often enough to stop and fight a same-level monster that has
      // caught the player. A longer travel chunk let an attacker consume an
      // entire potion stack while the route was still making geometric
      // progress, so "stalled" is not a safe prerequisite for self-defence.
      // The collision detour itself persists across chunks by target key.
      // A resource-sensitive hunt must re-read HP and potion state often. Two
      // physical movement attempts keep the worst-case overshoot below the
      // remaining recovery reserve even when a hostile keeps landing hits.
      const chunkAttempts = resourceSensitiveSearch ? 2 : 8;
      const chunkStartPlayer = state.player
        ? { x: Number(state.player.x), y: Number(state.player.y) }
        : null;
      let navigationError = null;
      const travelTarget = corridorWaypoint ?? field;
      const reachedTravelTarget = await navigateNear(travelTarget, 8, {
        maxAttempts: chunkAttempts,
        abortOnDeath: true,
        autoUsePotions: activeGoal?.supplyFunding !== true,
        resourceBaseline,
        resourceAccountingGoal: activeGoal,
      })
        .then(
          () => true,
          (error) => {
            navigationError = error;
            return false;
          },
        );
      if (reachedTravelTarget && corridorWaypoint) {
        console.log(
          `  hostile corridor waypoint reached: ` +
          `${corridorWaypoint.x},${corridorWaypoint.y}`,
        );
        corridorWaypoint = null;
        reached = false;
      } else {
        reached = reachedTravelTarget;
      }
      travelAttempts += chunkAttempts;
      if (
        navigationError instanceof NavigationUnreachableError &&
        corridorWaypoint
      ) {
        // The hazard-only elbow is an optional preference, not evidence that
        // the authoritative respawn field is unreachable. Crystal maps can
        // contain water, cliffs, or sealed building pockets exactly on one of
        // the two orthogonal elbow coordinates while a normal collision path
        // to the field itself remains open. Drop only the synthetic waypoint
        // and let the next chunk prove the direct physical route.
        console.log(
          `  discard unreachable hostile corridor waypoint ` +
          `${corridorWaypoint.x},${corridorWaypoint.y}; retry direct field`,
        );
        corridorWaypoint = null;
        continue;
      }
      if (navigationError instanceof NavigationUnreachableError) {
        fieldGroupCooldownUntil.set(fieldGroupKey, Date.now() + STALLED_FIELD_GROUP_COOLDOWN_MS);
        console.log(
          `  skip unreachable ${monsterName} field=${field.x},${field.y} ` +
          `group=${field.patrolCenterX ?? field.x},${field.patrolCenterY ?? field.y}: ` +
          navigationError.message,
        );
        break;
      }
      if (navigationError instanceof NavigationInterruptedByDeathError) {
        fieldGroupCooldownUntil.set(fieldGroupKey, Date.now() + STALLED_FIELD_GROUP_COOLDOWN_MS);
        console.log(
          `  abandon lethal ${monsterName} field=${field.x},${field.y} ` +
          `group=${field.patrolCenterX ?? field.x},${field.patrolCenterY ?? field.y}: ` +
          navigationError.message,
        );
        // Death recovery returns to town and clears the pre-death route
        // cursor. Abort this goal so the outer policy can collect nearby
        // supply drops, restock, and rank a fresh field from the revived
        // position. Continuing this old patrol bypasses the hard zero-potion
        // gate and can immediately repeat the same lethal journey.
        throw navigationError;
      }
      if (navigationError instanceof NavigationInterruptedByThreatError) {
        const threat = navigationError.threat;
        console.log(
          `  interrupt travel for adjacent threat: ${threat.name} ` +
          `${threat.objectId}@${threat.x},${threat.y}`,
        );
        const cleared = await clearAdjacentTravelThreat(
          threat,
          activeGoal,
          resourceBaseline,
        );
        if (!cleared) {
          monsterCooldownUntil.set(
            String(threat.objectId),
            Date.now() + FAILED_COMBAT_COOLDOWN_MS,
          );
        }
        stalledChunks = 0;
        continue;
      }
      if (navigationError instanceof SupplyFundingSafetyError) {
        throw navigationError;
      }
      if (navigationError instanceof CombatResourceBudgetError) {
        throw navigationError;
      }
      state = await readAgentState(client);
      found = await nearestVisibleMonsterByName(
        state,
        monsterName,
        preferNearestSupplyTarget,
      );
      if (found) {
        await logMonsterSearch(monsterName, state, `transit-${field.x}-${field.y}`);
        return found;
      }
      const remainingFieldDistance = chebyshev(state.player, field);
      bestFieldDistance = Math.min(bestFieldDistance, remainingFieldDistance);
      const chunkMovement = chebyshev(chunkStartPlayer, state.player);
      // Resource-sensitive travel intentionally limits each chunk to two
      // physical attempts so combat/potion budgets are re-read frequently.
      // Requiring the normal three-tile threshold there makes genuine one- or
      // two-tile collision progress mathematically unable to reset stalling.
      const meaningfulChunkMovement = resourceSensitiveSearch ? 1 : 3;
      if (chunkMovement >= meaningfulChunkMovement) {
        // A valid route around a large obstacle can increase straight-line
        // distance for many chunks. Net authoritative movement along the BFS
        // is progress; only repeatedly stationary chunks are truly stalled.
        stalledChunks = 0;
      } else {
        stalledChunks += 1;
      }
      // This threshold is expressed in eight-attempt travel chunks. Nine
      // chunks preserve the former ~72-attempt budget needed to route around
      // a large Crystal building before declaring the respawn group stalled.
      if (!reached && stalledChunks >= 9) {
        fieldGroupCooldownUntil.set(fieldGroupKey, Date.now() + STALLED_FIELD_GROUP_COOLDOWN_MS);
        console.log(
          `  skip stalled ${monsterName} field=${field.x},${field.y} ` +
          `bestDistance=${bestFieldDistance} currentDistance=${remainingFieldDistance} ` +
          `chunkMovement=${chunkMovement} ` +
          `group=${field.patrolCenterX ?? field.x},${field.patrolCenterY ?? field.y}`,
        );
        break;
      }
    }
    if (!reached) continue;
    state = await readAgentState(client);
    await logMonsterSearch(monsterName, state, `field-${field.x}-${field.y}`);
    // Reaching a source patrol point can reveal the target at the far side of
    // the AOI without making its hit surface clickable. Approach that exact
    // rendered object before rotating to the next source field; otherwise the
    // agent repeatedly walks past valid Scarecrows during supply funding.
    const fieldEncounter = rankMonsterApproachTargets(
      state,
      matchingLiveMonsters(state, monsterName)
        .filter((entry) => chebyshev(state.player, entry) <= 16),
      preferNearestSupplyTarget,
    )[0] ?? null;
    if (fieldEncounter) {
      await navigateNear(fieldEncounter, 1, {
        maxAttempts: resourceSensitiveSearch ? 1 : 12,
        autoUsePotions: activeGoal?.supplyFunding !== true,
        clearTrivialOccupancy: true,
        resourceBaseline,
        resourceAccountingGoal: activeGoal,
      }).catch((error) => {
        if (
          error instanceof SupplyFundingSafetyError ||
          error instanceof CombatResourceBudgetError
        ) throw error;
        return false;
      });
      state = await readAgentState(client);
      found = await nearestVisibleMonsterByName(
        state,
        monsterName,
        preferNearestSupplyTarget,
      );
      if (found) {
        await logMonsterSearch(monsterName, state, `field-approach-${fieldEncounter.objectId}`);
        return found;
      }
    }
    await delay(750);
  }
  return null;
}

async function clearAdjacentTravelThreat(threat, activeGoal, resourceBaseline = null) {
  const before = await readAgentState(client);
  const experienceBefore = Number(before.playerExperience ?? 0);
  const since = Date.now();
  const result = await killMonster(
    threat,
    {
      ...activeGoal,
      monsterName: String(threat.name),
      itemName: null,
      harvest: false,
      incidentalTravelThreat: true,
      incidentalTravelOrigin: before.player
        ? { x: Number(before.player.x), y: Number(before.player.y) }
        : null,
    },
    { current: 0, required: 0 },
    experienceBefore,
    since,
    resourceBaseline,
    activeGoal,
  );
  if (!result.success) {
    console.log(`  adjacent travel threat deferred: ${result.reason ?? threat.name}`);
    return false;
  }
  let after = await readAgentState(client);
  if (await collectNearbyGoldIfVisible(after).catch(() => false)) {
    after = await readAgentState(client);
  }
  evidence.kills.push({
    questId: activeGoal.questId,
    monsterName: String(threat.name),
    objectId: String(threat.objectId),
    harvested: false,
    harvestCompleted: null,
    harvestProgressed: null,
    incidental: true,
    experienceBefore,
    experienceAfter: after.playerExperience,
    at: Date.now(),
  });
  recordMilestone(`q${activeGoal.questId}-travel-threat-${threat.name}`, after, {
    objectId: String(threat.objectId),
    incidental: true,
  });
  return true;
}

async function useOffensiveCombatSkillIfReady(state, target) {
  if (
    !state?.player ||
    !target ||
    entityIsCorpse(target) ||
    String(state.selectedObjectId ?? "") !== String(target.objectId) ||
    chebyshev(state.player, target) > CLIENT_LOCKED_ATTACK_CLICK_RADIUS
  ) return false;
  const selected = offensiveCombatSkillHotkey(state.knownSkills);
  if (!selected) return false;
  const cadenceMs = Math.max(
    800,
    Number(selected.skill.delayMs ?? 0),
    Number(selected.skill.castTimeMs ?? 0),
  );
  if (Date.now() - Math.max(lastCombatSkillInputAt, lastRestorativeSkillInputAt) < cadenceMs) {
    return false;
  }

  const startedAt = Date.now();
  lastCombatSkillInputAt = startedAt;
  await client.pressKey(
    `F${selected.slot}`,
    `F${selected.slot}`,
    111 + selected.slot,
    {
      action: "cast-offensive-combat-skill",
      skill: selected.skill.name,
      slot: selected.slot,
      objectId: String(target.objectId),
    },
  );
  await delay(80);
  const command = outgoingGameplayCommandsSince(startedAt).find((entry) =>
    ["magic", "castSkill"].includes(String(entry.type))
  ) ?? null;
  if (command) {
    console.log(
      `  combat skill: F${selected.slot} ${selected.skill.name} -> ${target.objectId}`,
    );
  }
  return command != null;
}

async function useRestorativeSelfSkillIfNeeded(state, healthRatioThreshold = 0.72) {
  const hp = Number(state?.playerHp);
  const maxHp = Number(state?.playerMaxHp);
  const mp = Number(state?.playerMp);
  if (
    !state?.player ||
    state.playerDead ||
    state.deathOverlayVisible ||
    !Number.isFinite(hp) ||
    !Number.isFinite(maxHp) ||
    maxHp <= 0 ||
    hp / maxHp >= Math.max(0, Number(healthRatioThreshold ?? 0.72)) ||
    !Number.isFinite(mp) ||
    mp <= 0
  ) return false;
  const selected = restorativeSelfSkillHotkey(state.knownSkills);
  if (!selected) return false;
  const cadenceMs = Math.max(
    1_000,
    Number(selected.skill.delayMs ?? 0),
    Number(selected.skill.castTimeMs ?? 0),
  );
  if (Date.now() - Math.max(lastCombatSkillInputAt, lastRestorativeSkillInputAt) < cadenceMs) {
    return false;
  }

  const startedAt = Date.now();
  lastRestorativeSkillInputAt = startedAt;
  await client.pressKey(
    `F${selected.slot}`,
    `F${selected.slot}`,
    111 + selected.slot,
    {
      action: "cast-restorative-self-skill",
      skill: selected.skill.name,
      slot: selected.slot,
      targetId: String(state.playerObjectId),
    },
  );
  await delay(80);
  const command = outgoingGameplayCommandsSince(startedAt).find((entry) =>
    String(entry.type) === "magic" &&
    normalizeName(entry.spell) === "healing" &&
    String(entry.targetId ?? "") === String(state.playerObjectId ?? "")
  ) ?? null;
  if (command) {
    console.log(`  restorative skill: F${selected.slot} ${selected.skill.name} -> self`);
  }
  return command != null;
}

async function killMonster(
  target,
  goal,
  objectiveBefore,
  experienceBefore,
  since,
  resourceBaseline = null,
  resourceAccountingGoal = goal,
) {
  const objectId = String(target.objectId);
  if (Number(quarantinedMonsterUntil.get(objectId) ?? 0) > Date.now()) {
    return {
      success: false,
      reason: `${goal.monsterName} ${objectId} remains quarantined`,
    };
  }
  const initialHp = target.hp == null || target.hp === "" ? null : Number(target.hp);
  let lastHp = Number.isFinite(initialHp) ? initialHp : null;
  let lastProgressAt = Date.now();
  let lastTargetResponseCount = 0;
  let lastChaseInputAt = 0;
  let lastRelockPlayerSignature = null;
  let stalledRelockCount = 0;
  let corpse = null;
  const initialState = await readAgentState(client);
  assertSafeSupplyFundingState(goal, initialState, goal.monsterName);
  if (!goal.supplyFunding) {
    const healing = await useRestorativeSelfSkillIfNeeded(initialState);
    if (!healing) await usePotionIfNeeded(initialState);
  }
  const clicked = await clickEntity(objectId, {
    action: "select-and-attack", monster: goal.monsterName, objectId,
  });
  if (!clicked) {
    monsterCooldownUntil.set(objectId, Date.now() + FAILED_APPROACH_COOLDOWN_MS);
    return { success: false, reason: `${goal.monsterName} did not have a physically visible entity hitbox` };
  }

  const hardDeadline = Math.min(
    Date.now() + COMBAT_HARD_DEADLINE_MS,
    evidence.startedAt + maxRuntimeMs,
  );
  let progressDeadline = Math.min(
    Date.now() + COMBAT_PROGRESS_WINDOW_MS,
    hardDeadline,
  );
  while (Date.now() < progressDeadline) {
    assertRuntimeBudget(`fighting ${goal.monsterName}`);
    // A selected monster enables the normal client's chase loop. Observe an
    // incidental travel attacker quickly enough to cancel that lock if the
    // monster steps away; ordinary quest combat keeps the lower-frequency
    // polling used for packet correlation and potion handling.
    await delay(goal.incidentalTravelThreat ? 80 : 350);
    const state = await readAgentState(client);
    assertSafeSupplyFundingState(goal, state, goal.monsterName);
    if (
      resourceBaseline &&
      rememberQuestCombatResourceStrain(resourceAccountingGoal, resourceBaseline, state)
    ) {
      monsterCooldownUntil.set(objectId, Date.now() + FAILED_COMBAT_COOLDOWN_MS);
      return {
        success: false,
        reason: `${goal.monsterName} exceeded the sustainable combat resource budget`,
      };
    }
    if (state.playerDead) {
      await recoverPlayerIfNeeded(state, {
        autoUsePotions: goal.supplyFunding !== true,
      });
      return { success: false, reason: `player died while fighting ${goal.monsterName}; revived for retry` };
    }
    if (!goal.supplyFunding) {
      const healing = await useRestorativeSelfSkillIfNeeded(state);
      if (!healing) await usePotionIfNeeded(state);
    }

    const quest = questState(state, goal.questId);
    const progress = objectiveProgress(quest, goal.monsterName);
    const objectiveAdvanced = progress.current > objectiveBefore.current;
    const experienceAdvanced = Number(state.playerExperience ?? 0) !== experienceBefore;
    const combatEvidence = targetCombatEvidenceSince(
      client,
      since,
      objectId,
      state.playerObjectId,
    );
    const targetResponseCount = combatEvidence.struckCount
      + combatEvidence.healthCount
      + combatEvidence.damageCount
      + combatEvidence.diedCount;
    if (targetResponseCount > lastTargetResponseCount) {
      lastTargetResponseCount = targetResponseCount;
      lastProgressAt = Date.now();
      lastRelockPlayerSignature = null;
      stalledRelockCount = 0;
      // High-HP quest targets can require several minutes of ordinary combat.
      // Extend only while target-specific packets prove forward progress, and
      // retain a hard five-minute bound so an invulnerable animation cannot
      // hold the run forever.
      progressDeadline = Math.min(
        Date.now() + COMBAT_PROGRESS_WINDOW_MS,
        hardDeadline,
      );
    }
    const diedPacket = combatEvidence.targetDied;
    const live = state.entities.find((entry) => String(entry.objectId) === objectId) ?? null;
    if (entityIsCorpse(live)) corpse = live;
    if (diedPacket || entityIsCorpse(live)) {
      monsterCooldownUntil.delete(objectId);
      quarantinedMonsterUntil.delete(objectId);
      return { success: true, corpse: corpse ?? live ?? target };
    }
    if (live) await useOffensiveCombatSkillIfReady(state, live);
    if (
      goal.incidentalTravelThreat &&
      live &&
      goal.incidentalTravelOrigin &&
      chebyshev(goal.incidentalTravelOrigin, live) > 8
    ) {
      monsterCooldownUntil.set(objectId, Date.now() + FAILED_APPROACH_COOLDOWN_MS);
      return {
        success: false,
        reason: `${goal.monsterName} left the bounded travel-clearing radius`,
      };
    }
    // Incidental combat exists only to unblock the current physical step. A
    // moving attacker which is no longer adjacent has already opened an
    // escape route; chasing it turns travel into unrelated grinding and can
    // drag the player deeper into a dense spawn field. The next navigation
    // input also cancels any client-side combat approach still in flight.
    if (goal.incidentalTravelThreat && live && chebyshev(state.player, live) > 1) {
      monsterCooldownUntil.set(objectId, Date.now() + FAILED_APPROACH_COOLDOWN_MS);
      return {
        success: false,
        reason: `${goal.monsterName} disengaged from the adjacent travel block`,
      };
    }

    const outgoingAttackCount = client.outgoingCommandAudit().commands.filter(
      (entry) => entry.at >= since && entry.type === "attack",
    ).length;
    if (
      Date.now() - since > 15_000 &&
      outgoingAttackCount >= 5 &&
      !combatEvidence.targetResponded
    ) {
      const quarantine = {
        questId: goal.questId,
        monsterName: goal.monsterName,
        objectId,
        at: Date.now(),
        reason: "five real attacks produced no target-specific combat packet",
        outgoingAttackCount,
        combatEvidence,
        collateralProgress: { objectiveAdvanced, experienceAdvanced },
        target: {
          x: live?.x ?? target.x,
          y: live?.y ?? target.y,
          hp: live?.hp ?? target.hp ?? null,
          maxHp: live?.maxHp ?? target.maxHp ?? null,
        },
        player: state.player,
      };
      evidence.targetQuarantines.push(quarantine);
      const quarantineUntil = Date.now() + QUARANTINED_TARGET_COOLDOWN_MS;
      quarantinedMonsterUntil.set(objectId, quarantineUntil);
      monsterCooldownUntil.set(objectId, quarantineUntil);
      return {
        success: false,
        reason: `${goal.monsterName} ${objectId} quarantined: no target-specific combat response`,
      };
    }
    if (
      live &&
      chebyshev(state.player, live) > 1 &&
      Date.now() - lastProgressAt > 1_500 &&
      Date.now() - lastChaseInputAt > 1_500
    ) {
      // A physical scene click starts the production client's own locked run
      // chase. Refresh that lock while the moving target remains hit-testable;
      // sending a separate manual navigation gesture too early would cancel
      // the lock and let evasive Crystal monsters run out of the AOI. A
      // successful click is input evidence, not movement progress, though: if
      // three relocks observe the exact same authoritative player tile, fall
      // through to bounded collision routing so a certified adjacent occupant
      // can be cleared through the same visible combat surface.
      const relockPlayerSignature = `${state.player.x},${state.player.y}`;
      stalledRelockCount = relockPlayerSignature === lastRelockPlayerSignature
        ? stalledRelockCount + 1
        : 0;
      lastRelockPlayerSignature = relockPlayerSignature;
      const relocked = await clickEntity(objectId, {
        action: "relock-visible-moving-monster", monster: goal.monsterName, objectId,
      });
      lastChaseInputAt = Date.now();
      if (relocked && stalledRelockCount < 2) {
        console.log(
          `  combat relock: ${goal.monsterName} ${objectId}@${live.x},${live.y} ` +
          `player=${state.player?.x},${state.player?.y}`,
        );
        continue;
      }
      console.log(relocked
        ? `  combat relock stalled: ${goal.monsterName} ${objectId}@${live.x},${live.y} ` +
          `player=${state.player?.x},${state.player?.y}`
        : `  combat chase: ${goal.monsterName} ${objectId}@${live.x},${live.y} ` +
          `player=${state.player?.x},${state.player?.y}`);
      await navigateNear(live, 1, {
        maxAttempts: 4,
        autoUsePotions: goal.supplyFunding !== true,
        // Dense Crystal patrols can put a second certified monster on the
        // only adjacent chase tile. Use the same bounded, physical-combat
        // occupancy clearing as NPC travel instead of repeatedly relocking a
        // target which the normal client cannot reach through that occupant.
        clearTrivialOccupancy: true,
        resourceBaseline,
        resourceAccountingGoal,
      }).catch(() => false);
      const chased = await readAgentState(client);
      const chasedTarget = chased.entities.find((entry) => String(entry.objectId) === objectId) ?? null;
      const chasedPlayerSignature = chased.player
        ? `${chased.player.x},${chased.player.y}`
        : null;
      if (chasedPlayerSignature && chasedPlayerSignature !== relockPlayerSignature) {
        lastRelockPlayerSignature = chasedPlayerSignature;
        stalledRelockCount = 0;
      }
      if (chasedTarget && chebyshev(chased.player, chasedTarget) <= 1) {
        await clickEntity(objectId, {
          action: "reacquire-moving-monster", monster: goal.monsterName, objectId,
        });
      }
      lastChaseInputAt = Date.now();
      continue;
    }
    if (live && chebyshev(state.player, live) === 0) {
      console.log(`  combat overlap: stepping away from ${goal.monsterName} ${objectId}`);
      const signature = `${state.player.x},${state.player.y}`;
      const escaped = await tryKeyboardEscape(
        state.player,
        { x: Number(state.player.x) + 2, y: Number(state.player.y) },
        signature,
        state.mapTransfers,
      );
      if (escaped) {
        await clickEntity(objectId, {
          action: "reacquire-overlapping-monster", monster: goal.monsterName, objectId,
        });
        lastProgressAt = Date.now();
        continue;
      }
    }

    const sentAttack = client.outgoingCommandAudit().commands.some(
      (entry) => entry.at >= since && entry.type === "attack",
    );
    if (!sentAttack && Date.now() - since > 18_000) {
      monsterCooldownUntil.set(objectId, Date.now() + FAILED_COMBAT_COOLDOWN_MS);
      return { success: false, reason: `${goal.monsterName} was visible but unreachable; cooling down ${objectId}` };
    }

    const liveHp = live?.hp == null || live?.hp === "" ? null : Number(live.hp);
    if (live && lastHp !== null && Number.isFinite(liveHp) && liveHp < lastHp) {
      lastHp = liveHp;
      lastProgressAt = Date.now();
      lastRelockPlayerSignature = null;
      stalledRelockCount = 0;
    }
    if (!live && Date.now() - lastProgressAt > 4_000) {
      return { success: false, reason: `${goal.monsterName} left AOI without death/XP/objective evidence` };
    }
    if (live && Date.now() - lastProgressAt > 12_000) {
      await clickEntity(objectId, {
        action: "reacquire-monster", monster: goal.monsterName, objectId,
      });
      lastProgressAt = Date.now();
    }
  }
  assertRuntimeBudget(`fighting ${goal.monsterName}`);
  // A final attack may already be accepted when the bounded combat loop
  // reaches its deadline. The rendered death flag and ObjectDied packet can
  // arrive just after that boundary, especially when a corpse/drop button is
  // mounted over the same sprite. Give only that in-flight attack a short
  // observation grace period; do not send another attack or infer success
  // from unrelated XP/gold changes.
  const deathSettleDeadline = Math.min(
    Date.now() + 2_500,
    evidence.startedAt + maxRuntimeMs,
  );
  while (Date.now() < deathSettleDeadline) {
    const finalState = await readAgentState(client);
    const finalTarget = finalState.entities.find(
      (entry) => String(entry.objectId) === objectId,
    ) ?? null;
    const finalCombatEvidence = targetCombatEvidenceSince(
      client,
      since,
      objectId,
      finalState.playerObjectId,
    );
    if (finalCombatEvidence.targetDied || entityIsCorpse(finalTarget)) {
      monsterCooldownUntil.delete(objectId);
      quarantinedMonsterUntil.delete(objectId);
      return { success: true, corpse: finalTarget ?? target };
    }
    await delay(125);
  }
  monsterCooldownUntil.set(objectId, Date.now() + FAILED_COMBAT_COOLDOWN_MS);
  return {
    success: false,
    reason: Date.now() >= hardDeadline
      ? `${goal.monsterName} fight reached the 5m hard deadline`
      : `${goal.monsterName} fight made no target-specific progress for 45s`,
  };
}

async function harvestCorpse(corpse, goal, objectiveBefore) {
  const objectiveName = normalizeName(goal.itemName ?? goal.monsterName);
  const inventoryBeforeState = await readAgentState(client);
  const inventoryBefore = (inventoryBeforeState.inventoryItems ?? [])
    .filter((item) => normalizeName(item.name) === objectiveName)
    .reduce((total, item) => total + Math.max(1, Number(item.quantity ?? 1)), 0);
  const progressionExpression = `(() => { const s = window.__mir2Stage5?.state ?? {}; const q = (s.questLog ?? []).find((entry) => Number(entry?.questId) === ${Number(goal.questId)}); const o = (q?.objectives ?? []).find((entry) => String(entry?.label ?? '').replace(/[^a-z0-9]/gi, '').toLowerCase().includes(${JSON.stringify(objectiveName)})); const current = Number(o?.current ?? q?.current ?? 0); const inventory = (s.inventoryItems ?? []).filter((item) => String(item?.name ?? '').replace(/[^a-z0-9]/gi, '').toLowerCase() === ${JSON.stringify(objectiveName)}).reduce((total, item) => total + Math.max(1, Number(item?.quantity ?? 1)), 0); return current > ${Number(objectiveBefore.current)} || inventory > ${inventoryBefore} || String(q?.stage ?? '').replace(/[^a-z]/gi, '').toLowerCase() === 'readytoturnin'; })()`;
  const corpsePresentExpression =
    `window.__mir2Stage5?.state?.entities?.some((entry) => ` +
    `String(entry?.objectId) === ${JSON.stringify(String(corpse.objectId))} && ` +
    `(entry?.dead === true || (entry?.hp != null && Number(entry.hp) <= 0))) === true`;
  let acceptedPasses = 0;
  let unacknowledgedPasses = 0;
  for (let attempt = 0; attempt < 16; attempt += 1) {
    if (await client.evaluate(`Boolean(${progressionExpression})`)) {
      return { completed: true, progressed: true };
    }
    let state = await readAgentState(client);
    if (state.playerDead || state.deathOverlayVisible) {
      await recoverPlayerIfNeeded(state);
      return {
        completed: false,
        progressed: false,
        interruptedByDeath: true,
      };
    }
    const activeThreat = nearestActiveHostile(state, {
      excludeObjectId: corpse.objectId,
      maxDistance: 8,
      withinMs: ACTIVE_TRAVEL_THREAT_WINDOW_MS,
    });
    if (activeThreat) {
      console.log(
        `  harvest preempted by active threat: ${activeThreat.name} ` +
        `${activeThreat.objectId}@${activeThreat.x},${activeThreat.y}`,
      );
      return {
        completed: false,
        progressed: false,
        interruptedByThreat: true,
        threat: activeThreat,
      };
    }
    let liveCorpse = state.entities.find((entry) => String(entry.objectId) === String(corpse.objectId)) ?? null;
    if (!entityIsCorpse(liveCorpse)) {
      await waitUntil(
        client,
        corpsePresentExpression,
        4_000,
      );
      state = await readAgentState(client);
      liveCorpse = state.entities.find((entry) => String(entry.objectId) === String(corpse.objectId)) ?? null;
    }
    if (!entityIsCorpse(liveCorpse)) {
      // Harvest evidence belongs to the monster incarnation just killed. A
      // same-name fallback can select an older, already harvested Deer corpse
      // and turn an honest no-op into a misleading retry loop.
      // A successful final pass can remove the corpse before the quest/inventory
      // update reaches the rendered snapshot. Settle that authoritative
      // progression before declaring the exact lifecycle lost; live r30 q25
      // produced ObjectRemove followed by CannibalStem 0->1 on the next policy
      // turn.
      await waitUntil(
        client,
        `Boolean(${progressionExpression}) || Boolean(${corpsePresentExpression})`,
        4_000,
      );
      const progressedAfterRemoval = await client.evaluate(
        `Boolean(${progressionExpression})`,
      );
      if (progressedAfterRemoval) {
        console.log(
          `  harvest progress settled after corpse removal: ${corpse.objectId}`,
        );
        return { completed: true, progressed: true };
      }
      state = await readAgentState(client);
      liveCorpse = state.entities.find(
        (entry) => String(entry.objectId) === String(corpse.objectId),
      ) ?? null;
      if (entityIsCorpse(liveCorpse)) {
        console.log(
          `  harvest corpse reappeared during observation settle: ${corpse.objectId}`,
        );
        continue;
      }
      console.log(`  harvest stopped: killed ${goal.monsterName} corpse ${corpse.objectId} left the visible world`);
      return { completed: false, progressed: false };
    }
    console.log(`  harvest attempt ${attempt + 1}: corpse=${liveCorpse.objectId}@${liveCorpse.x},${liveCorpse.y} player=${state.player?.x},${state.player?.y}`);
    await navigateNear(liveCorpse, 1).catch(() => {});
    state = await waitForMovementSettled();
    const refreshed = state.entities.find((entry) => String(entry.objectId) === String(liveCorpse.objectId)) ?? null;
    if (!entityIsCorpse(refreshed)) {
      console.log(`  harvest retry: corpse ${liveCorpse.objectId} changed lifecycle while settling`);
      continue;
    }
    if (chebyshev(state.player, refreshed) > 1) {
      console.log(
        `  harvest retry: corpse ${refreshed.objectId} is no longer adjacent after movement settled ` +
        `(player=${state.player?.x},${state.player?.y} corpse=${refreshed.x},${refreshed.y})`,
      );
      continue;
    }
    const selectedCorpse = await clickEntity(String(refreshed.objectId), {
      action: "select-corpse", monster: goal.monsterName, objectId: String(refreshed.objectId),
    });
    if (!selectedCorpse) {
      console.log(`  harvest retry: corpse ${refreshed.objectId} has no physical hitbox`);
      const sameTileCorpses = state.entities.filter((entry) => (
        String(entry.objectId) !== String(refreshed.objectId) &&
        entityIsCorpse(entry) &&
        Number(entry.x) === Number(refreshed.x) &&
        Number(entry.y) === Number(refreshed.y)
      ));
      if (sameTileCorpses.length > 0) {
        console.log(
          `  harvest corpse obscured by another same-tile corpse: ` +
          `${sameTileCorpses.map((entry) => String(entry.objectId)).join(",")}`,
        );
        // There is no normal-client gesture that can select a fully covered
        // hit surface. Rapid clicks only manufacture noisy evidence. Retry
        // briefly for render ordering/lifecycle to settle, then abandon this
        // exact body and let the outer policy hunt a fresh real source.
        await delay(750);
        if (attempt + 1 >= HARVEST_OVERLAY_RETRY_LIMIT) {
          return {
            completed: false,
            progressed: false,
            obscuredByCorpse: true,
          };
        }
        continue;
      }
      // A newly materialized gold marker can sit above the corpse sprite and
      // legitimately own every physical hit-test sample. Pick only adjacent,
      // visibly rendered gold through normal UI input, then retry the exact
      // corpse; never infer a harvest or switch to another same-name body.
      await collectNearbyGoldIfVisible(state, 1).catch((error) => {
        console.warn(
          `  optional corpse-overlay gold pickup deferred: ${String(error?.message ?? error)}`,
        );
      });
      await delay(750);
      continue;
    }
    const selectionReady = await waitUntil(
      client,
      `String(window.__mir2Stage5?.state?.selectedObjectId ?? '') === ${JSON.stringify(String(refreshed.objectId))}`,
      1_000,
    );
    if (!selectionReady) {
      console.log(`  harvest retry: visible click did not select corpse ${refreshed.objectId}`);
      continue;
    }
    state = await readAgentState(client);
    const selected = state.entities.find((entry) => String(entry.objectId) === String(refreshed.objectId)) ?? null;
    if (!entityIsCorpse(selected) || chebyshev(state.player, selected) > 1) {
      console.log(`  harvest retry: corpse ${refreshed.objectId} moved out of reach after selection`);
      continue;
    }
    const harvestStartedAt = Date.now();
    await client.pressKey(" ", "Space", 32, { action: "harvest-selected-corpse" });
    const sentHarvest = client.outgoingCommandAudit().commands.some(
      (entry) => entry.at >= harvestStartedAt && entry.type === "harvest",
    );
    const harvestCommand = outgoingGameplayCommandsSince(harvestStartedAt)
      .findLast((command) => command.type === "harvest") ?? null;
    console.log(
      `  harvest input: selected=${state.selectedObjectId ?? "none"}->${refreshed.objectId} ` +
      `sent=${sentHarvest} direction=${harvestCommand?.direction ?? "none"}`,
    );
    const packetDeadline = Date.now() + 2_000;
    let accepted = false;
    while (Date.now() < packetDeadline) {
      if (await client.evaluate(`Boolean(${progressionExpression})`)) {
        return { completed: true, progressed: true };
      }
      if (wsPacketsSince(client, harvestStartedAt, "ObjectHarvested").length > 0) {
        const progressed = await waitUntil(client, progressionExpression, 5_000);
        return { completed: true, progressed };
      }
      if (wsPacketsSince(client, harvestStartedAt, "ObjectHarvest").length > 0) {
        accepted = true;
        acceptedPasses += 1;
        unacknowledgedPasses = 0;
        console.log(`  harvest pass accepted: ${acceptedPasses}`);
        break;
      }
      await delay(80);
    }
    // Crystal harvest is multi-pass and cadence-gated. Count authoritative
    // ObjectHarvest acknowledgements, not raw key presses, and leave enough
    // time for the server's action window before requesting the next pass.
    if (!accepted) {
      unacknowledgedPasses += 1;
      console.log(
        `  harvest pass unacknowledged: packets=[${wsPacketNamesSince(harvestStartedAt).join(",")}]`,
      );
      if (unacknowledgedPasses >= 3) {
        // Do not turn a late quest update into a false lifecycle failure. This
        // is observation-only: no fourth key press is sent while settling.
        const progressed = await waitUntil(
          client,
          progressionExpression,
          4_000,
        );
        if (progressed) {
          console.log(
            `  harvest progress settled after ${unacknowledgedPasses} unacknowledged passes`,
          );
        }
        return { completed: progressed, progressed };
      }
    }
    await delay(accepted ? 2_100 : 120);
  }
  const progressed = await waitUntil(client, progressionExpression, 4_000);
  return { completed: progressed, progressed };
}

function wsPacketNamesSince(since) {
  const names = [];
  for (const frame of client?.wsReceived ?? []) {
    if (frame.at < since || !isGameplayWebSocketUrl(frame.url)) continue;
    try {
      const envelope = JSON.parse(frame.payloadData);
      names.push(String(envelope?.packet ?? envelope?.type ?? "unknown"));
    } catch {
      names.push("nonJson");
    }
  }
  return names.slice(-12);
}

function outgoingGameplayCommandsSince(since) {
  const commands = [];
  for (const frame of client?.wsSent ?? []) {
    if (frame.at < since || !isGameplayWebSocketUrl(frame.url)) continue;
    try {
      const command = JSON.parse(frame.payloadData);
      if (command && typeof command === "object") commands.push(command);
    } catch {
      // Non-JSON frames are not gameplay commands.
    }
  }
  return commands;
}

async function navigateNear(
  target,
  desiredDistance,
  {
    maxAttempts = 100,
    allowTransferToMap = null,
    transferKey = null,
    abortOnDeath = false,
    interruptOnBlockingThreatName = null,
    clearTrivialOccupancy = false,
    failFastWhenCollisionPathUnavailable = false,
    autoUsePotions = true,
    resourceBaseline = null,
    resourceAccountingGoal = null,
  } = {},
) {
  const routeState = await readAgentState(client);
  const expectedMapFileName = String(routeState.mapFileName);
  const dynamicTarget = target?.objectId != null;
  const requestedTarget = { ...target };
  const detourKey = [
    expectedMapFileName,
    dynamicTarget ? `object:${requestedTarget.objectId}` : `tile:${requestedTarget.x},${requestedTarget.y}`,
  ].join("|");

  let stagnant = 0;
  let previous = null;
  let bestDistance = Number.POSITIVE_INFINITY;
  let noDistanceProgress = 0;
  let forcedDetourTarget = navigationDetourByTarget.get(detourKey) ?? null;
  if (forcedDetourTarget && !Number.isFinite(Number(forcedDetourTarget.createdAt))) {
    forcedDetourTarget.createdAt = Date.now();
  }
  const rejectedByPosition = new Map();
  const rejectedCollisionCells = new Set(
    activeRejectedCollisionCells(expectedMapFileName),
  );
  const rejectCollisionCell = (point) => {
    const key = typeof point === "string"
      ? point
      : `${Number(point?.x)},${Number(point?.y)}`;
    rejectedCollisionCells.add(key);
    rememberRejectedCollisionCell(expectedMapFileName, key);
  };
  const visitedPositions = new Set();
  const positionVisitCount = new Map();
  let denseOccupancyClears = 0;
  for (let attempt = 0; attempt < maxAttempts; attempt += 1) {
    assertRuntimeBudget(`navigating to ${requestedTarget.x},${requestedTarget.y}`);
    const beforeRecovery = await readAgentState(client);
    assertSafeSupplyFundingState(
      resourceAccountingGoal,
      beforeRecovery,
      requestedTarget.name ?? resourceAccountingGoal?.monsterName,
    );
    if (
      resourceBaseline && resourceAccountingGoal &&
      rememberQuestCombatResourceStrain(
        resourceAccountingGoal,
        resourceBaseline,
        beforeRecovery,
      )
    ) {
      throw new CombatResourceBudgetError(
        `${resourceAccountingGoal.monsterName} navigation exceeded the sustainable combat resource budget`,
      );
    }
    const interruptedByDeath = beforeRecovery.playerDead || beforeRecovery.deathOverlayVisible;
    await recoverPlayerIfNeeded(beforeRecovery, { autoUsePotions });
    if (interruptedByDeath && abortOnDeath) {
      throw new NavigationInterruptedByDeathError(
        `player died while travelling toward ${requestedTarget.x},${requestedTarget.y}`,
      );
    }
    const state = await readAgentState(client);
    if (String(state.mapFileName) !== expectedMapFileName) {
      if (allowTransferToMap && String(state.mapFileName) === String(allowTransferToMap)) {
        navigationDetourByTarget.delete(detourKey);
        return true;
      }
      throw new NavigationEnteredUnexpectedMapError(
        expectedMapFileName,
        state.mapFileName,
      );
    }
    if (interruptOnBlockingThreatName) {
      const blockingThreat = nearestSafeBlockingHostile(
        state,
        interruptOnBlockingThreatName,
      );
      if (blockingThreat) {
        throw new NavigationInterruptedByThreatError(blockingThreat);
      }
    }
    const protectedTransfers = protectedTransfersForNavigation(
      state.mapTransfers,
      allowTransferToMap,
    );
    const player = state.player;
    if (!player) throw new Error("player position unavailable during navigation");
    if (
      clearTrivialOccupancy &&
      denseOccupancyClears < 4 &&
      denseAdjacentHostileCount(state) >= 3
    ) {
      // A real melee player surrounded on three or more immediate tiles must
      // open one exit before a route to a distant, isolated target can exist.
      // Ignore the short approach cooldown in this exact trap, but continue to
      // respect the stronger no-response quarantine and the completed-quest /
      // level certification inside nearestTrivialAdjacentHostile.
      const blocker = nearestTrivialAdjacentHostile(
        state,
        null,
        quarantinedMonsterUntil,
      );
      if (blocker) {
        console.log(
          `  clear dense adjacent occupancy: ${blocker.name} ` +
          `${blocker.objectId}@${blocker.x},${blocker.y}`,
        );
        const clearingGoal = resourceAccountingGoal ?? {
          kind: "grind",
          questId: 0,
          monsterName: String(blocker.name),
          itemName: null,
          harvest: false,
          supplyFunding: true,
        };
        const cleared = await clearAdjacentTravelThreat(
          blocker,
          clearingGoal,
          resourceBaseline,
        );
        if (cleared) {
          denseOccupancyClears += 1;
          stagnant = 0;
          collisionRegionCache = null;
          attempt -= 1;
          continue;
        }
      }
    }
    const liveTarget = dynamicTarget
      ? state.entities.find((entry) => String(entry.objectId) === String(requestedTarget.objectId)) ??
        state.groundDrops.find((entry) => String(entry.objectId) === String(requestedTarget.objectId)) ??
        // Monsters and drops move or disappear, so stale coordinates are not
        // safe. Crystal NPCs are static: an AOI-edge detour may hide their
        // object for one step while their authoritative map coordinate remains
        // the correct physical destination.
        (requestedTarget.kind === "npc" ? requestedTarget : null)
      : requestedTarget;
    if (dynamicTarget && !liveTarget) {
      navigationDetourByTarget.delete(detourKey);
      throw new Error(`navigation target ${requestedTarget.objectId} left the visible world`);
    }
    const distance = chebyshev(player, liveTarget);
    const forcedDetourExpired = forcedDetourTarget &&
      Date.now() - Number(forcedDetourTarget.createdAt) >= STICKY_NAVIGATION_DETOUR_TTL_MS;
    if (
      forcedDetourTarget &&
      (chebyshev(player, forcedDetourTarget) <= 1 || forcedDetourExpired)
    ) {
      if (forcedDetourExpired) {
        console.log(
          `  expire sticky collision detour: ${forcedDetourTarget.x},${forcedDetourTarget.y}; replanning`,
        );
      }
      forcedDetourTarget = null;
      navigationDetourByTarget.delete(detourKey);
      collisionRegionCache = null;
    }
    const steeringTarget = forcedDetourTarget ?? liveTarget;
    const steeringDesiredDistance = forcedDetourTarget ? 1 : desiredDistance;
    const steeringDistance = chebyshev(player, steeringTarget);
    // A Crystal movement portal is a one-cell trigger. Four tiles is enough
    // to see and click its map surface, but not enough for the normal client
    // to enter it reliably around a building wall. Keep collision-routing
    // until adjacent, then use an ordinary direction-key step into the exact
    // movement cell. Indoor Crystal portals proved less reliable with a
    // perspective tile click at one-cell range, while keyboard movement uses
    // the same client input path and preserves the server-authoritative step.
    if (allowTransferToMap && distance <= 1) {
      const startedAt = Date.now();
      const beforeTransferSignature = `${player.x},${player.y}`;
      const portalProbe = movementProbesToward(player, liveTarget)
        .filter((probe) =>
          chebyshev(movementProbeDestination(player, probe), liveTarget) === 0
        )
        .sort((left, right) => left.keys.length - right.keys.length)[0];
      if (!portalProbe) {
        throw new Error(`no physical direction-key step reaches visible transfer ${transferKey ?? "unknown"}`);
      }
      const transferInput = {
        action: "enter-visible-map-transfer",
        transferKey,
        fromMapFileName: expectedMapFileName,
        toMapFileName: String(allowTransferToMap),
      };
      await waitForDiscreteMovementInput();
      if (portalProbe.keys.length === 1) {
        const [key] = portalProbe.keys;
        await client.pressKey(key.key, key.code, key.vk, transferInput);
      } else {
        // A simultaneous two-arrow chord does not produce a reliable diagonal
        // movement intent in the normal client: releasing the first key leaves
        // a residual cardinal turn, so a diagonally adjacent one-cell portal
        // can be retried forever without changing the authoritative position.
        // Walk one proven cardinal component now and re-enter this branch from
        // the resulting cardinal-adjacent tile. The next ordinary key press
        // reaches the exact movement cell and lets the server perform the map
        // transition. Try both components because live occupancy can block one
        // of the otherwise walkable corner cells.
        let componentMoved = false;
        for (const key of portalProbe.keys) {
          await client.pressKey(key.key, key.code, key.vk, {
            ...transferInput,
            action: "enter-visible-map-transfer-diagonal-approach",
            plannedDirection: portalProbe.direction,
            direction: key.direction,
          });
          componentMoved = await waitForPositionChange(beforeTransferSignature, 950);
          console.log(
            `  visible transfer diagonal component ${key.direction}: ` +
            `${componentMoved ? "moved" : "blocked"}`,
          );
          if (componentMoved) break;
          await waitForDiscreteMovementInput();
        }
        if (componentMoved) {
          await delay(DIRECT_MOVEMENT_SETTLE_MS);
          continue;
        }
        console.log(
          `  visible transfer ${transferKey ?? "unknown"} diagonal approach did not advance; recomputing`,
        );
        continue;
      }
      const advanced = await waitUntil(
        client,
        `(() => { const s = window.__mir2Stage5?.state ?? {}; const p = s.authoritativePlayer ?? s.player; return String(s.mapFileName ?? '') === ${JSON.stringify(String(allowTransferToMap))} || (p && (Number(p.x) + ',' + Number(p.y)) !== ${JSON.stringify(beforeTransferSignature)}); })()`,
        5_000,
      );
      let afterTransferInput = await readAgentState(client);
      if (
        advanced &&
        String(afterTransferInput.mapFileName) !== String(allowTransferToMap)
      ) {
        // The accepted movement packet can arrive several seconds before the
        // ensuing MapInformation/world snapshot pair. Give that ordinary
        // client lifecycle a dedicated grace period instead of clicking the
        // portal repeatedly while the server is already changing maps.
        await waitUntil(
          client,
          `String(window.__mir2Stage5?.state?.mapFileName ?? '') === ${JSON.stringify(String(allowTransferToMap))}`,
          10_000,
        );
        afterTransferInput = await readAgentState(client);
      }
      if (String(afterTransferInput.mapFileName) === String(allowTransferToMap)) {
        navigationDetourByTarget.delete(detourKey);
        return true;
      }
      console.log(
        `  visible transfer ${transferKey ?? "unknown"} ` +
        `${advanced ? "advanced without changing map" : "did not advance"} ` +
        `after ${Date.now() - startedAt}ms; recomputing`,
      );
      continue;
    }
    if (distance <= desiredDistance) {
      navigationDetourByTarget.delete(detourKey);
      return true;
    }
    if (distance < bestDistance) {
      bestDistance = distance;
      noDistanceProgress = 0;
    } else {
      noDistanceProgress += 1;
    }
    if (attempt === 0 || attempt % 10 === 9) {
      console.log(`  navigate: ${player.x},${player.y} -> ${liveTarget.x},${liveTarget.y} distance=${distance} attempt=${attempt + 1}`);
    }

    const signature = `${player.x},${player.y}`;
    visitedPositions.add(signature);
    const signatureVisits = Number(positionVisitCount.get(signature) ?? 0) + 1;
    positionVisitCount.set(signature, signatureVisits);
    const rejectedWaypoints = rejectedByPosition.get(signature) ?? new Set();
    rejectedByPosition.set(signature, rejectedWaypoints);
    stagnant = signature === previous ? stagnant + 1 : 0;
    previous = signature;

    if (clearTrivialOccupancy && (stagnant >= 2 || signatureVisits >= 3)) {
      // At this point the player has already repeated the same physical tile.
      // Do not exclude the requested monster name: a live target occupying an
      // adjacent exit is exactly the object which must be selected through the
      // scene and fought (or allowed to disengage) before navigation can make
      // progress. The certification/level filter below still prevents this
      // fallback from pulling an unrelated dangerous monster.
      const blocker = nearestTrivialAdjacentHostile(state);
      if (blocker) {
        console.log(
          `  clear trapped navigation occupant: ${blocker.name} ` +
          `${blocker.objectId}@${blocker.x},${blocker.y}`,
        );
        const clearingGoal = resourceAccountingGoal ?? {
          kind: "grind",
          questId: 0,
          monsterName: String(blocker.name),
          itemName: null,
          harvest: false,
          supplyFunding: true,
        };
        const cleared = await clearAdjacentTravelThreat(
          blocker,
          clearingGoal,
          resourceBaseline,
        );
        if (cleared) {
          stagnant = 0;
          collisionRegionCache = null;
          continue;
        }
      }
    }

    let collisionPath = null;
    let usedGlobalCollisionPath = false;
    if (steeringDistance > GLOBAL_COLLISION_PATH_THRESHOLD) {
      try {
        collisionPath = await collisionAtlasPathToward(
          player,
          steeringTarget,
          steeringDesiredDistance,
          state,
          protectedTransfers,
          expectedMapFileName,
          [...rejectedCollisionCells],
        );
        usedGlobalCollisionPath = Boolean(collisionPath);
        if (!collisionPath) {
          throw new NavigationUnreachableError(
            `no global walkable path on ${expectedMapFileName} from ` +
            `${player.x},${player.y} to ${steeringTarget.x},${steeringTarget.y}`,
          );
        }
      } catch (error) {
        if (error instanceof NavigationUnreachableError) throw error;
        console.warn(`  global collision route unavailable: ${String(error?.message ?? error)}`);
        collisionPath = null;
      }
    }
    collisionPath ??= await collisionPathToward(
      player,
      steeringTarget,
      steeringDesiredDistance,
      state.entities,
      protectedTransfers,
      expectedMapFileName,
      [...rejectedCollisionCells],
    ).catch((error) => {
      console.warn(`  collision route unavailable: ${String(error?.message ?? error)}`);
      rejectedCollisionCells.add(signature);
      return null;
    });
    if (
      usedGlobalCollisionPath &&
      signatureVisits >= 3 &&
      collisionPath?.[1]
    ) {
      // A server-acknowledged two-cell oscillation still reports every key
      // press as movement, so the ordinary stagnant detector never fires.
      // On the third visit to the same authoritative tile, reject that exact
      // first BFS cell and recompute once. This preserves legitimate detours
      // that temporarily increase distance, while breaking A<->B loops around
      // a stale atlas edge or moving occupancy cell.
      const cyclingCell = `${collisionPath[1].x},${collisionPath[1].y}`;
      rejectCollisionCell(cyclingCell);
      console.log(
        `  reject cycling collision edge: ${signature}->${cyclingCell}; replanning`,
      );
      collisionPath = await collisionAtlasPathToward(
        player,
        steeringTarget,
        steeringDesiredDistance,
        state,
        protectedTransfers,
        expectedMapFileName,
        [...rejectedCollisionCells],
      );
      if (!collisionPath) {
        throw new NavigationUnreachableError(
          `global collision path cycled at ${signature} toward ` +
          `${steeringTarget.x},${steeringTarget.y}`,
        );
      }
    }
    if (!collisionPath && failFastWhenCollisionPathUnavailable) {
      throw new NavigationUnreachableError(
        `no live collision path from ${player.x},${player.y} to optional target ` +
        `${steeringTarget.x},${steeringTarget.y}`,
      );
    }
    if (!forcedDetourTarget && collisionPath?.detourEndpoint) {
      forcedDetourTarget = {
        ...collisionPath.detourEndpoint,
        createdAt: Date.now(),
      };
      navigationDetourByTarget.set(detourKey, forcedDetourTarget);
      console.log(
        `  sticky collision detour: ${forcedDetourTarget.x},${forcedDetourTarget.y} ` +
        `before resuming ${liveTarget.x},${liveTarget.y}`,
      );
    }
    // At melee range the target/corpse sprite often covers the intended tile
    // center. Follow exactly the first collision-planned step with normal
    // direction keys. The generic escape probe is deliberately not used here:
    // its first open alternative can be a valid escape that moves away from a
    // fleeing Deer, turning a two-tile chase into a local oscillation.
    if (collisionPath && steeringDistance <= steeringDesiredDistance + 3) {
      const pathTarget = collisionPath[Math.min(1, collisionPath.length - 1)] ?? liveTarget;
      const movedByKeyboard = await tryCollisionPathStep(
        player,
        pathTarget,
        signature,
        protectedTransfers,
        steeringTarget,
      );
      if (movedByKeyboard) continue;
      rejectCollisionCell(pathTarget);
    }
    const waypoint = collisionPath
      ? await visibleWaypointAlongPath(player, collisionPath, state.entities, [...rejectedWaypoints])
      : await visibleWaypointToward(
        player,
        steeringTarget,
        steeringDesiredDistance,
        state.entities,
        [...rejectedWaypoints],
        protectedTransfers,
        !collisionPath || stagnant >= 3 || noDistanceProgress >= 3,
      );
    let moved = false;
    if (waypoint) {
      if (attempt === 0 || attempt % 10 === 9) {
        console.log(`  waypoint: tile=${waypoint.x},${waypoint.y} screen=${Math.round(waypoint.screenX)},${Math.round(waypoint.screenY)}`);
      }
      const movementMeta = {
        action: "navigate-visible-waypoint",
        destination: { x: steeringTarget.x, y: steeringTarget.y },
        desiredDistance: steeringDesiredDistance,
      };
      if (usedGlobalCollisionPath) {
        // A global BFS already proved this visible straight segment walkable.
        // For four or more cells, physically hold Shift plus the direction
        // keys so the normal client emits repeated run intents. This shortens
        // exposure to roaming monsters without injecting a MoveTo command.
        // Release before the end of the proven segment and replan from the
        // authoritative position; short segments retain precise pointer input.
        const pathIndex = Number(waypoint.pathIndex ?? 1);
        if (pathIndex >= 4) {
          const dx = Math.sign(Number(waypoint.x) - Number(player.x));
          const dy = Math.sign(Number(waypoint.y) - Number(player.y));
          const transferSafe = continuousCollisionRunAvoidsTransfers({
            start: player,
            direction: { x: dx, y: dy },
            plannedSteps: pathIndex,
            mapTransfers: protectedTransfers,
          });
          if (!transferSafe) {
            console.log(
              `  transfer-guarded collision run: ${player.x},${player.y}->` +
              `${waypoint.x},${waypoint.y}; using one direction step`,
            );
            const pathTarget = collisionPath[1];
            const guardedStepMoved = await tryCollisionPathStep(
              player,
              pathTarget,
              signature,
              protectedTransfers,
              steeringTarget,
            );
            if (guardedStepMoved) continue;
            rejectCollisionCell(pathTarget);
            await delay(450);
            continue;
          }
          const directionKeys = [
            ...(dx < 0 ? [{ key: "ArrowLeft", code: "ArrowLeft", vk: 37 }]
              : dx > 0 ? [{ key: "ArrowRight", code: "ArrowRight", vk: 39 }] : []),
            ...(dy < 0 ? [{ key: "ArrowUp", code: "ArrowUp", vk: 38 }]
              : dy > 0 ? [{ key: "ArrowDown", code: "ArrowDown", vk: 40 }] : []),
          ];
          const runTicks = Math.max(2, Math.floor(pathIndex / 2));
          const holdMs = Math.min(2_200, 700 + (runTicks - 2) * DIRECT_MOVEMENT_SETTLE_MS);
          await client.holdKeyChord(
            [{ key: "Shift", code: "ShiftLeft", vk: 16 }, ...directionKeys],
            holdMs,
            {
              ...movementMeta,
              action: "navigate-visible-collision-run-segment",
              pathIndex,
            },
          );
        } else {
          await client.holdTileDirection(waypoint.x, waypoint.y, "right", 500, {
            ...movementMeta,
            action: "navigate-visible-collision-segment",
            pathIndex,
          });
        }
      } else if (steeringDistance > steeringDesiredDistance + 4) {
        await client.holdTileDirection(waypoint.x, waypoint.y, "right", 700, movementMeta);
      } else {
        await client.clickTile(waypoint.x, waypoint.y, "right", movementMeta);
      }
      moved = await waitForMovementBurst(
        signature,
        steeringDistance <= steeringDesiredDistance + 4 ? 2_500 : 12_000,
        {
          autoUsePotions,
          resourceBaseline,
          resourceAccountingGoal,
        },
      );
      if (!moved) rejectedWaypoints.add(`${waypoint.x},${waypoint.y}`);
      if (moved && !usedGlobalCollisionPath && noDistanceProgress >= 3 && collisionPath?.[1]) {
        // A one-tile animation followed by an authoritative correction still
        // satisfies the movement-burst probe. If repeated attempts have made
        // no net progress, reject that first locally planned cell. Never apply
        // this heuristic to the global BFS: a valid building detour often has
        // to increase target distance for several steps before turning back.
        rejectCollisionCell(collisionPath[1]);
      }
    }
    if (
      !moved && collisionPath?.[1] &&
      steeringDistance > steeringDesiredDistance + 3
    ) {
      // Sprite/nameplate layers can cover the first visible route segment.
      // Preserve the BFS direction with one exact adjacent physical input
      // instead of falling through to an unrelated generic escape probe.
      const pathTarget = collisionPath[1];
      moved = await tryCollisionPathStep(
        player,
        pathTarget,
        signature,
        protectedTransfers,
        steeringTarget,
      );
      if (moved) continue;
      rejectCollisionCell(pathTarget);
    }
    if (!moved) {
      if (collisionPath && steeringDistance <= steeringDesiredDistance + 3) {
        // The planned adjacent cell was momentarily occupied or correction-
        // gated. Let the dynamic occupancy change and recompute from the new
        // authoritative snapshot instead of choosing an arbitrary open tile.
        await delay(450);
        continue;
      }
      moved = await tryKeyboardEscape(
        player,
        steeringTarget,
        signature,
        protectedTransfers,
        [...visitedPositions],
      );
    }
    if (!moved) stagnant += 1;
    if (stagnant >= 2) {
      const detour = {
        x: player.x + (Math.abs(steeringTarget.x - player.x) >= Math.abs(steeringTarget.y - player.y) ? 0 : 3),
        y: player.y + (Math.abs(steeringTarget.x - player.x) >= Math.abs(steeringTarget.y - player.y) ? 3 : 0),
      };
      if (!navigationTileIsTransfer(detour, protectedTransfers)) {
        await client.clickTile(detour.x, detour.y, "right", { action: "navigation-detour" }).catch(() => false);
        await waitForPositionChange(signature, 1_500);
      }
      stagnant = 0;
    }
  }
  const state = await readAgentState(client);
  assertSafeSupplyFundingState(
    resourceAccountingGoal,
    state,
    requestedTarget.name ?? resourceAccountingGoal?.monsterName,
  );
  if (
    resourceBaseline && resourceAccountingGoal &&
    rememberQuestCombatResourceStrain(
      resourceAccountingGoal,
      resourceBaseline,
      state,
    )
  ) {
    throw new CombatResourceBudgetError(
      `${resourceAccountingGoal.monsterName} navigation exceeded the sustainable combat resource budget`,
    );
  }
  throw new Error(`navigation did not reach ${requestedTarget.x},${requestedTarget.y}; stopped at ${state.player?.x},${state.player?.y}`);
}

async function collisionPathToward(
  player,
  target,
  desiredDistance,
  entities,
  mapTransfers,
  mapFileName,
  rejectedCells = [],
) {
  const region = await collisionRegionFor(player, mapFileName);
  const bounds = region.regionBounds;
  const blocked = new Set(
    region.cells
      .filter((cell) => cell?.blocked === true)
      .map((cell) => `${Number(cell.x)},${Number(cell.y)}`),
  );
  const occupied = new Set(
    entities
      .filter((entry) => !entityIsCorpse(entry))
      .map((entry) => `${Number(entry.x)},${Number(entry.y)}`),
  );
  for (const key of navigationTransferTileKeys(mapTransfers)) blocked.add(key);
  for (const key of rejectedCells) blocked.add(String(key));
  const startKey = `${Number(player.x)},${Number(player.y)}`;
  occupied.delete(startKey);

  const queue = [{ x: Number(player.x), y: Number(player.y) }];
  const previous = new Map([[startKey, null]]);
  const directions = [
    [0, -1], [1, 0], [0, 1], [-1, 0],
    [1, -1], [1, 1], [-1, 1], [-1, -1],
  ];
  const targetDistance = (point) => chebyshev(point, target);
  const targetInside = (
    Number(target.x) >= bounds.minX && Number(target.x) <= bounds.maxX &&
    Number(target.y) >= bounds.minY && Number(target.y) <= bounds.maxY
  );
  let endpoint = null;
  let detourEndpoint = null;
  let best = queue[0];
  let bestDistance = targetDistance(best);

  for (let cursor = 0; cursor < queue.length; cursor += 1) {
    const point = queue[cursor];
    const distance = targetDistance(point);
    if (distance < bestDistance) {
      best = point;
      bestDistance = distance;
    }
    if (distance <= desiredDistance) {
      endpoint = point;
      break;
    }

    for (const [dx, dy] of directions) {
      const next = { x: point.x + dx, y: point.y + dy };
      const nextKey = `${next.x},${next.y}`;
      if (
        next.x < bounds.minX || next.x > bounds.maxX ||
        next.y < bounds.minY || next.y > bounds.maxY ||
        blocked.has(nextKey) || occupied.has(nextKey) || previous.has(nextKey)
      ) continue;
      // Crystal permits diagonal input, but a diagonal may not cut through
      // either orthogonal blocker. Keeping that invariant makes every planned
      // segment executable by the normal client movement state machine.
      if (
        dx !== 0 && dy !== 0 &&
        (blocked.has(`${point.x + dx},${point.y}`) ||
          blocked.has(`${point.x},${point.y + dy}`) ||
          occupied.has(`${point.x + dx},${point.y}`) ||
          occupied.has(`${point.x},${point.y + dy}`))
      ) continue;
      previous.set(nextKey, `${point.x},${point.y}`);
      queue.push(next);
    }
  }

  endpoint ??= best;
  const horizontalGoal = Math.abs(Number(target.x) - Number(player.x)) >=
    Math.abs(Number(target.y) - Number(player.y));
  const needsPerpendicularFrontier = collisionPathNeedsPerpendicularFrontier(
    player,
    target,
    bounds,
    endpoint,
  );
  if (!endpoint || (targetInside && targetDistance(endpoint) > desiredDistance)) {
    throw new Error(`no walkable path from ${startKey} to ${target.x},${target.y}`);
  }
  if (needsPerpendicularFrontier || targetDistance(endpoint) >= targetDistance(player)) {
    // The goal lies outside this collision chunk and a wall can make every
    // immediately useful tile unreachable. Walk to the nearest reachable
    // perpendicular frontier so the next chunk reveals a route around the
    // wall. Prefer the side that also moves along the target's secondary axis.
    // This is still ordinary client movement over parsed map collision; it
    // never changes the authoritative transform directly.
    const secondaryDelta = horizontalGoal
      ? Number(target.y) - Number(player.y)
      : Number(target.x) - Number(player.x);
    const onPerpendicularFrontier = (point) => horizontalGoal
      ? point.y <= bounds.minY + 1 || point.y >= bounds.maxY - 1
      : point.x <= bounds.minX + 1 || point.x >= bounds.maxX - 1;
    const onPreferredSide = (point) => {
      // Near alignment should not send the route to the far edge merely
      // because the target differs by a couple of tiles. Consider both sides
      // and let nearest-frontier cost win until the secondary offset is real.
      if (Math.abs(secondaryDelta) <= 8) return true;
      if (horizontalGoal) {
        return secondaryDelta < 0 ? point.y <= bounds.minY + 1 : point.y >= bounds.maxY - 1;
      }
      return secondaryDelta < 0 ? point.x <= bounds.minX + 1 : point.x >= bounds.maxX - 1;
    };
    const frontierCandidates = queue.filter(
      (point) => chebyshev(point, player) >= 8 && onPerpendicularFrontier(point),
    );
    const preferredCandidates = frontierCandidates.filter(onPreferredSide);
    const frontier = (preferredCandidates.length ? preferredCandidates : frontierCandidates)
      .sort((left, right) =>
        chebyshev(left, player) - chebyshev(right, player) ||
        targetDistance(left) - targetDistance(right)
      )[0] ?? null;
    if (!frontier) {
      throw new Error(`collision route cannot make progress toward ${target.x},${target.y}`);
    }
    endpoint = frontier;
    detourEndpoint = { x: frontier.x, y: frontier.y };
  }

  const path = [];
  let key = `${endpoint.x},${endpoint.y}`;
  while (key != null) {
    const [x, y] = key.split(",").map(Number);
    path.push({ x, y });
    key = previous.get(key);
  }
  path.reverse();
  if (!detourEndpoint && collisionPathNeedsStickyDetour(player, target, path)) {
    detourEndpoint = { x: endpoint.x, y: endpoint.y };
  }
  if (detourEndpoint) path.detourEndpoint = detourEndpoint;
  return path;
}

async function collisionRegionFor(player, mapFileName = null) {
  const cachedBounds = collisionRegionCache?.regionBounds;
  const margin = 12;
  if (
    String(collisionRegionCache?.mapFileName ?? "") === String(mapFileName ?? "") &&
    cachedBounds &&
    Number(player.x) >= cachedBounds.minX + margin && Number(player.x) <= cachedBounds.maxX - margin &&
    Number(player.y) >= cachedBounds.minY + margin && Number(player.y) <= cachedBounds.maxY - margin
  ) return collisionRegionCache;

  const sceneUrl = new URL("/api/scene/crystal", baseUrl);
  const requestedMap = String(mapFileName ?? (await readAgentState(client)).mapFileName ?? "");
  sceneUrl.searchParams.set("map", requestedMap);
  sceneUrl.searchParams.set("x", String(Number(player.x)));
  sceneUrl.searchParams.set("y", String(Number(player.y)));
  // Request enough preload margin for routes that must temporarily move away
  // from the destination (for example, around Border Village's central wall).
  sceneUrl.searchParams.set("width", "56");
  sceneUrl.searchParams.set("height", "72");
  const response = await fetch(sceneUrl, { headers: { accept: "application/json" } });
  if (!response.ok) throw new Error(`scene collision request failed with HTTP ${response.status}`);
  const payload = await response.json();
  const region = payload?.originalMapRegion;
  if (!region?.regionBounds || !Array.isArray(region?.cells)) {
    throw new Error("scene response omitted originalMapRegion collision cells");
  }
  collisionRegionCache = { ...region, mapFileName: requestedMap };
  return collisionRegionCache;
}

async function collisionAtlasPathToward(
  player,
  target,
  desiredDistance,
  state,
  mapTransfers,
  mapFileName,
  rejectedCells = [],
) {
  const entities = Array.isArray(state?.entities) ? state.entities : [];
  const distance = chebyshev(player, target);
  const baseMargin = Math.min(160, Math.max(72, Math.ceil(distance * 0.25)));
  for (const margin of [baseMargin, Math.min(350, Math.max(240, baseMargin * 2))]) {
    const corridor = await collisionAtlasCorridor(mapFileName, player, target, margin);
    const occupied = entities
      // Only nearby actors can still occupy their current cell when this
      // route reaches it. Treating every moving deer/chicken in AOI as a
      // permanent far obstacle makes the full path flip on every snapshot.
      .filter((entry) => !entityIsCorpse(entry) && chebyshev(player, entry) <= 4)
      .map((entry) => ({ x: Number(entry.x), y: Number(entry.y) }));
    const hostileAvoidance = dangerousHostileAvoidanceCells(state, grindingCatalog, {
      radius: 2,
      // Completing a real kill/harvest objective is stronger evidence than a
      // static level estimate: this exact character has already defeated the
      // monster through the normal client. Keep its occupied tile blocked, but
      // stop surrounding every such spawn with an artificial danger halo.
      safeMonsterNames: entities
        .filter((entry) => completedQuestCertifiesMonster(state, entry?.name))
        .map((entry) => entry.name),
    });
    // Ordinary low-level actors block only their authoritative tile. Giving
    // every deer, scarecrow, and cat a one-cell halo turns a normal dense town
    // AOI into an artificial wall even though a real client may pass on an
    // adjacent tile. Retain halos only for meaningfully dangerous monsters.
    const dynamicAvoidance = [
      ...occupied,
      ...hostileAvoidance,
    ];
    const staticBlocked = [
      ...corridor.blocked,
      ...navigationTransferTileKeys(mapTransfers),
    ];
    const blocked = [
      ...staticBlocked,
      ...rejectedCells,
    ];
    const staticPath = findCollisionGridPath({
      start: player,
      target,
      desiredDistance,
      bounds: corridor.bounds,
      // A server-rejected cell is authoritative even when the parsed map
      // atlas marks it open. Include the bounded cross-chunk correction
      // memory in the deterministic route itself; otherwise the early
      // static-path return below selects the same rejected first step forever
      // and the escape probe merely walks A->B->A between outer chunks.
      blocked,
      occupied: [],
    });
    // Preserve the deterministic map route while its next physical input is
    // clear. Farther roaming actors are reconsidered after that authoritative
    // step, preventing the whole BFS from flipping between two detours on
    // successive snapshots.
    if (
      staticPath &&
      !collisionPathHasImmediateDynamicBlock(
        staticPath,
        dynamicAvoidance,
        12,
      )
    ) return staticPath;
    const saferPath = findCollisionGridPath({
      start: player,
      target,
      desiredDistance,
      bounds: corridor.bounds,
      blocked,
      occupied: dynamicAvoidance,
    });
    if (saferPath) {
      // Freeze a bounded waypoint from the first dynamic detour. Recomputing
      // the entire avoidance route after every moving-monster snapshot makes
      // equally good left/right paths alternate forever. The ordinary sticky
      // detour lifecycle clears this waypoint after reaching it, then replans.
      const detour = selectProgressingCollisionDetour(saferPath, player, target);
      if (detour) saferPath.detourEndpoint = { x: detour.x, y: detour.y };
      return saferPath;
    }

    // Dense spawn regions can close every two-tile safety halo. Preserve exact
    // actor occupancy but relax the halo before falling back to static-only
    // collision, so the route still prefers open space whenever one exists.
    const dynamicPath = findCollisionGridPath({
      start: player,
      target,
      desiredDistance,
      bounds: corridor.bounds,
      blocked,
      occupied,
    });
    if (dynamicPath) {
      const detour = selectProgressingCollisionDetour(dynamicPath, player, target);
      if (detour) dynamicPath.detourEndpoint = { x: detour.x, y: detour.y };
      return dynamicPath;
    }

    // A crowded shared Zone can temporarily occupy every convenient first
    // step, and rejected cells remember recent server corrections. Neither is
    // proof that a respawn region is structurally unreachable. Fall back to a
    // static-only route for movement; the next real key press is still checked
    // by the authoritative server and will be replanned if the actor remains.
    if (staticPath) return staticPath;

    // A long cross-map walk can observe enough short-lived Zone corrections
    // to cut every graph route even after the actors have moved. Only after
    // all routes that honor those memories fail, retry the parsed static map
    // without the expiring correction set. Each resulting step is still sent
    // as normal input and must be accepted by the authoritative server; a
    // genuinely occupied cell will simply be remembered and replanned again.
    if (rejectedCells.length > 0) {
      const relaxedStaticPath = findCollisionGridPath({
        start: player,
        target,
        desiredDistance,
        bounds: corridor.bounds,
        blocked: staticBlocked,
        occupied: [],
      });
      if (relaxedStaticPath) {
        console.log(
          `  collision atlas relaxed ${rejectedCells.length} expired-candidate ` +
          "corrections after every remembered route was closed",
        );
        return relaxedStaticPath;
      }
    }
  }
  return null;
}

async function collisionAtlasCorridor(mapFileName, start, target, margin) {
  const requested = {
    minX: Math.max(0, Math.min(Number(start.x), Number(target.x)) - margin),
    maxX: Math.max(Number(start.x), Number(target.x)) + margin,
    minY: Math.max(0, Math.min(Number(start.y), Number(target.y)) - margin),
    maxY: Math.max(Number(start.y), Number(target.y)) + margin,
  };
  const atlas = collisionAtlasByMap.get(String(mapFileName)) ?? {
    chunks: new Map(),
    blocked: new Set(),
    mapWidth: null,
    mapHeight: null,
  };
  collisionAtlasByMap.set(String(mapFileName), atlas);

  const spacing = 256;
  const chunks = [];
  for (let chunkX = Math.floor(requested.minX / spacing); chunkX <= Math.floor(requested.maxX / spacing); chunkX += 1) {
    for (let chunkY = Math.floor(requested.minY / spacing); chunkY <= Math.floor(requested.maxY / spacing); chunkY += 1) {
      chunks.push({ chunkX, chunkY });
    }
  }
  await Promise.all(chunks.map((chunk) => loadCollisionAtlasChunk(atlas, mapFileName, chunk)));
  const mapMaxX = Number.isFinite(Number(atlas.mapWidth)) ? Number(atlas.mapWidth) - 1 : requested.maxX;
  const mapMaxY = Number.isFinite(Number(atlas.mapHeight)) ? Number(atlas.mapHeight) - 1 : requested.maxY;
  return {
    blocked: atlas.blocked,
    bounds: {
      minX: Math.max(0, Math.floor(requested.minX)),
      maxX: Math.min(mapMaxX, Math.ceil(requested.maxX)),
      minY: Math.max(0, Math.floor(requested.minY)),
      maxY: Math.min(mapMaxY, Math.ceil(requested.maxY)),
    },
  };
}

async function loadCollisionAtlasChunk(atlas, mapFileName, chunk) {
  const key = `${chunk.chunkX},${chunk.chunkY}`;
  const cached = atlas.chunks.get(key);
  if (cached) return cached;
  const loading = (async () => {
    const spacing = 256;
    const minX = Math.max(0, chunk.chunkX * spacing);
    const minY = Math.max(0, chunk.chunkY * spacing);
    const sceneUrl = new URL("/api/scene/collision", baseUrl);
    sceneUrl.searchParams.set("map", String(mapFileName));
    sceneUrl.searchParams.set("minX", String(minX));
    sceneUrl.searchParams.set("maxX", String(minX + spacing - 1));
    sceneUrl.searchParams.set("minY", String(minY));
    sceneUrl.searchParams.set("maxY", String(minY + spacing - 1));
    const response = await fetch(sceneUrl, { headers: { accept: "application/json" } });
    if (!response.ok) throw new Error(`collision asset request failed with HTTP ${response.status}`);
    const payload = await response.json();
    if (!payload?.bounds || !Array.isArray(payload?.blockedCells)) {
      throw new Error("collision asset response omitted blocked cells");
    }
    atlas.mapWidth = Number(payload.mapWidth ?? atlas.mapWidth);
    atlas.mapHeight = Number(payload.mapHeight ?? atlas.mapHeight);
    for (const cell of payload.blockedCells) {
      atlas.blocked.add(`${Number(cell.x)},${Number(cell.y)}`);
    }
    return payload.bounds;
  })();
  atlas.chunks.set(key, loading);
  try {
    return await loading;
  } catch (error) {
    atlas.chunks.delete(key);
    throw error;
  }
}

async function visibleWaypointAlongPath(player, path, entities, rejected) {
  if (!Array.isArray(path) || path.length < 2) return null;
  const firstDx = path[1].x - path[0].x;
  const firstDy = path[1].y - path[0].y;
  const straightSegment = [];
  for (let index = 1; index < path.length && index <= 8; index += 1) {
    const previous = path[index - 1];
    const point = path[index];
    if (point.x - previous.x !== firstDx || point.y - previous.y !== firstDy) break;
    straightSegment.push({ ...point, pathIndex: index });
  }
  if (!straightSegment.length) return null;

  return client.evaluate(`
    (() => {
      const player = ${JSON.stringify({ x: Number(player.x), y: Number(player.y) })};
      const planned = ${JSON.stringify(straightSegment)};
      const occupied = new Set(${JSON.stringify(
        entities.map((entry) => `${entry.x},${entry.y}`),
      )});
      const rejected = new Set(${JSON.stringify(rejected)});
      const stageNode = document.querySelector('.client-stage-frame');
      if (!(stageNode instanceof HTMLElement)) return null;
      const stage = stageNode.getBoundingClientRect();
      const centerX = Number(stageNode.dataset.viewportTileCenterX);
      const centerY = Number(stageNode.dataset.viewportTileCenterY);
      const cellWidth = Number(stageNode.dataset.viewportCellWidth);
      const cellHeight = Number(stageNode.dataset.viewportCellHeight);
      if (![centerX, centerY, cellWidth, cellHeight].every(Number.isFinite)) return null;
      return planned
        .map((point) => ({
          ...point,
          screenX: stage.left + centerX + (point.x - player.x) * cellWidth,
          screenY: stage.top + centerY + (point.y - player.y) * cellHeight,
        }))
        .filter((point) => {
          if (
            point.screenX < stage.left + 96 || point.screenX > stage.right - 96 ||
            point.screenY < stage.top + 64 ||
            point.screenY > stage.top + Math.min(stage.height - 160, 560) ||
            rejected.has(point.x + ',' + point.y) || occupied.has(point.x + ',' + point.y)
          ) return false;
          const top = document.elementFromPoint(point.screenX, point.screenY);
          return top instanceof HTMLElement && stageNode.contains(top) &&
            !top.closest('button, a, input, textarea, select, [data-ui-interactive="true"], .npc-dialog-panel');
        })
        .sort((left, right) => right.pathIndex - left.pathIndex)[0] ?? null;
    })()
  `);
}

async function visibleWaypointToward(
  player,
  target,
  desiredDistance,
  entities,
  rejected,
  mapTransfers,
  allowDetour,
) {
  return client.evaluate(`
    (() => {
      const player = ${JSON.stringify({ x: Number(player.x), y: Number(player.y) })};
      const target = ${JSON.stringify({ x: Number(target.x), y: Number(target.y) })};
      const desiredDistance = ${Number(desiredDistance)};
      const allowDetour = ${Boolean(allowDetour)};
      const occupied = new Set(${JSON.stringify(
        entities.map((entry) => `${entry.x},${entry.y}`),
      )});
      const rejected = new Set(${JSON.stringify(rejected)});
      const forbidden = new Set(${JSON.stringify(navigationTransferTileKeys(mapTransfers))});
      const distance = (left, right) => Math.max(Math.abs(left.x - right.x), Math.abs(left.y - right.y));
      const currentDistance = distance(player, target);
      const crossesForbidden = (point) => {
        let x = player.x;
        let y = player.y;
        while (x !== point.x || y !== point.y) {
          x += Math.sign(point.x - x);
          y += Math.sign(point.y - y);
          if (forbidden.has(x + ',' + y)) return true;
        }
        return false;
      };
      const stageNode = document.querySelector('.client-stage-frame');
      if (!(stageNode instanceof HTMLElement)) return null;
      const stage = stageNode.getBoundingClientRect();
      const centerX = Number(stageNode.dataset.viewportTileCenterX);
      const centerY = Number(stageNode.dataset.viewportTileCenterY);
      const cellWidth = Number(stageNode.dataset.viewportCellWidth);
      const cellHeight = Number(stageNode.dataset.viewportCellHeight);
      if (![centerX, centerY, cellWidth, cellHeight].every(Number.isFinite)) return null;
      const points = [];
      for (let dx = -8; dx <= 8; dx += 1) {
        for (let dy = -8; dy <= 8; dy += 1) {
          points.push({
            x: player.x + dx,
            y: player.y + dy,
            center: {
              x: stage.left + centerX + dx * cellWidth,
              y: stage.top + centerY + dy * cellHeight,
            },
          });
        }
      }
      return points
        .map(({ x, y, center }) => {
          if (
            center.x < stage.left + 96 ||
            center.x > stage.right - 96 ||
            center.y < stage.top + 64 ||
            center.y > stage.top + Math.min(stage.height - 160, 560)
          ) return null;
          const top = document.elementFromPoint(center.x, center.y);
          if (!(top instanceof HTMLElement) || !stageNode.contains(top)) return null;
          if (top.closest('button, a, input, textarea, select, [data-ui-interactive="true"], .npc-dialog-panel')) {
            return null;
          }
          const point = { x, y };
          const targetDistance = distance(point, target);
          if (
            (!allowDetour && targetDistance >= currentDistance) ||
            (allowDetour && targetDistance > currentDistance + 8) ||
            targetDistance < desiredDistance ||
            distance(point, player) < 2 ||
            distance(point, player) > 8 ||
            rejected.has(point.x + ',' + point.y) ||
            occupied.has(point.x + ',' + point.y) ||
            crossesForbidden(point)
          ) return null;
          const axisRegressions =
            (Math.sign(target.x - player.x) !== 0 && Math.sign(point.x - player.x) === -Math.sign(target.x - player.x) ? 1 : 0) +
            (Math.sign(target.y - player.y) !== 0 && Math.sign(point.y - player.y) === -Math.sign(target.y - player.y) ? 1 : 0);
          const targetManhattan = Math.abs(point.x - target.x) + Math.abs(point.y - target.y);
          const detourRank = allowDetour ? (axisRegressions > 0 ? 0 : 1) : axisRegressions;
          return {
            ...point,
            axisRegressions,
            detourRank,
            targetDistance,
            targetManhattan,
            playerDistance: distance(point, player),
            screenX: center.x,
            screenY: center.y,
          };
        })
        .filter(Boolean)
        .sort((left, right) =>
          left.detourRank - right.detourRank ||
          left.targetDistance - right.targetDistance ||
          left.targetManhattan - right.targetManhattan ||
          right.playerDistance - left.playerDistance
        )[0] ?? null;
    })()
  `);
}

async function waitForPositionChange(previousSignature, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const state = await readAgentState(client);
    const current = state.player ? `${state.player.x},${state.player.y}` : "none";
    if (current !== previousSignature) {
      rememberAcknowledgedMovement();
      return true;
    }
    if (!state.movementPlan && Date.now() + 500 < deadline) await delay(120);
    else await delay(180);
  }
  return false;
}

async function waitForMovementSettled(timeoutMs = 4_000, stableMs = 750) {
  const deadline = Date.now() + timeoutMs;
  let lastSignature = null;
  let stableSince = Date.now();
  let state = await readAgentState(client);
  while (Date.now() < deadline) {
    state = await readAgentState(client);
    const signature = state.player ? `${state.player.x},${state.player.y}` : "none";
    if (signature !== lastSignature) {
      lastSignature = signature;
      stableSince = Date.now();
      rememberAcknowledgedMovement();
    }
    if (!state.movementPlan && Date.now() - stableSince >= stableMs) return state;
    await delay(100);
  }
  return state;
}

function rememberAcknowledgedMovement(at = Date.now()) {
  nextDiscreteMovementInputAt = Math.max(
    nextDiscreteMovementInputAt,
    Number(at) + DISCRETE_MOVEMENT_INPUT_GUARD_MS,
  );
}

async function waitForDiscreteMovementInput() {
  const remainingMs = nextDiscreteMovementInputAt - Date.now();
  if (remainingMs > 0) await delay(remainingMs);
}

async function tryKeyboardEscape(
  player,
  target,
  previousSignature,
  mapTransfers = [],
  avoidPositions = [],
) {
  const probes = await prioritizedMovementProbes(player, target, mapTransfers, avoidPositions);
  if (await dispatchKeyboardEscapeProbes(player, previousSignature, mapTransfers, probes)) {
    return true;
  }

  // A correction remembers rejected directions for 5.6 seconds. Pointer
  // navigation can therefore poison every cardinal around a tight corner
  // before the viable diagonal chord is attempted. Let that browser-side
  // memory expire, then try the collision-ranked directions once more.
  await delay(5_800);
  const refreshed = await readAgentState(client);
  const refreshedSignature = refreshed.player ? `${refreshed.player.x},${refreshed.player.y}` : "none";
  if (refreshedSignature !== previousSignature) return true;
  const retryProbes = await prioritizedMovementProbes(
    refreshed.player ?? player,
    target,
    mapTransfers,
    avoidPositions,
  );
  return dispatchKeyboardEscapeProbes(
    refreshed.player ?? player,
    refreshedSignature,
    mapTransfers,
    retryProbes,
  );
}

async function tryCollisionPathStep(
  player,
  next,
  previousSignature,
  mapTransfers = [],
  destination = next,
) {
  const dx = Math.sign(Number(next.x) - Number(player.x));
  const dy = Math.sign(Number(next.y) - Number(player.y));
  if (dx === 0 && dy === 0) return false;
  const horizontal = dx < 0
    ? { direction: "left", key: "ArrowLeft", code: "ArrowLeft", vk: 37 }
    : { direction: "right", key: "ArrowRight", code: "ArrowRight", vk: 39 };
  const vertical = dy < 0
    ? { direction: "up", key: "ArrowUp", code: "ArrowUp", vk: 38 }
    : { direction: "down", key: "ArrowDown", code: "ArrowDown", vk: 40 };
  const keys = [];
  if (dx !== 0) keys.push(horizontal);
  if (dy !== 0) keys.push(vertical);
  const probe = {
    direction: keys.map((key) => key.direction).join("+"),
    keys,
  };
  if (movementProbeTouchesTransfer(player, probe, mapTransfers)) return false;

  let moved = false;
  if (keys.length === 1) {
    const [key] = keys;
    await waitForDiscreteMovementInput();
    await client.pressKey(key.key, key.code, key.vk, {
      action: "navigate-collision-path-step",
      direction: probe.direction,
    });
    moved = await waitForPositionChange(previousSignature, 950);
  } else {
    // A two-key chord publishes a residual cardinal intent on the first keyup,
    // while an adjacent right-click is rejected on this authoritative Zone.
    // The BFS already proved both orthogonal corner cells open, so advance one
    // physical cardinal component and replan from the acknowledged position.
    // Prefer the axis with more remaining distance so repeated diagonals do
    // not collapse into a long horizontal-only or vertical-only staircase.
    const horizontalNeed = Math.abs(Number(destination.x) - Number(player.x));
    const verticalNeed = Math.abs(Number(destination.y) - Number(player.y));
    const componentKeys = verticalNeed > horizontalNeed ? [...keys].reverse() : keys;
    for (const key of componentKeys) {
      await waitForDiscreteMovementInput();
      await client.pressKey(key.key, key.code, key.vk, {
        action: "navigate-collision-path-diagonal-component",
        direction: key.direction,
        plannedDirection: probe.direction,
      });
      moved = await waitForPositionChange(previousSignature, 950);
      console.log(
        `  collision diagonal component ${key.direction}: ${moved ? "moved" : "blocked"}`,
      );
      if (moved) break;
    }
  }
  if (moved) await delay(DIRECT_MOVEMENT_SETTLE_MS);
  console.log(`  collision step ${probe.direction}: ${moved ? "moved" : "blocked"}`);
  return moved;
}

async function dispatchKeyboardEscapeProbes(player, previousSignature, mapTransfers, probes) {
  // The collision-ranked list contains eight bounded directions. Trying only
  // its first three can repeatedly omit the sole open cardinal when a roaming
  // monster occupies the preferred diagonal beside a building. Exhaust the
  // finite ranked set before declaring the character boxed in.
  for (const probe of probes.slice(0, 8)) {
    // CDP dispatches chord key-downs in sequence. A nominal diagonal from
    // (316,613) to (315,612) first presses Left and therefore briefly enters
    // the Barracks transfer at (315,613). Reject every intermediate step, not
    // just the diagonal endpoint.
    if (movementProbeTouchesTransfer(player, probe, mapTransfers)) continue;
    await waitForDiscreteMovementInput();
    if (probe.keys.length === 1) {
      const [key] = probe.keys;
      await client.pressKey(key.key, key.code, key.vk, {
        action: "navigate-keyboard-probe",
        direction: probe.direction,
      });
    } else {
      await client.pressKeyChord(probe.keys, {
        action: "navigate-keyboard-diagonal-probe",
        direction: probe.direction,
      });
    }
    const moved = await waitForPositionChange(previousSignature, 950);
    console.log(`  keyboard probe ${probe.direction}: ${moved ? "moved" : "blocked"}`);
    if (moved) {
      await delay(DIRECT_MOVEMENT_SETTLE_MS);
      return true;
    }
  }
  return false;
}

async function prioritizedMovementProbes(player, target, mapTransfers, avoidPositions = []) {
  const probes = movementProbesToward(player, target);
  try {
    const state = await readAgentState(client);
    const region = await collisionRegionFor(player, state.mapFileName);
    const cells = new Map(
      region.cells.map((cell) => [`${Number(cell.x)},${Number(cell.y)}`, cell]),
    );
    const occupied = new Set(
      state.entities
        .filter((entry) => !entityIsCorpse(entry))
        .map((entry) => `${Number(entry.x)},${Number(entry.y)}`),
    );
    const openNeighbourCount = (point) => {
      let count = 0;
      for (let dx = -1; dx <= 1; dx += 1) {
        for (let dy = -1; dy <= 1; dy += 1) {
          if (dx === 0 && dy === 0) continue;
          const neighbour = cells.get(`${point.x + dx},${point.y + dy}`);
          if (neighbour && neighbour.blocked !== true && neighbour.closedDoor !== true) count += 1;
        }
      }
      return count;
    };
    const ranked = probes
      .map((probe, index) => {
        const destination = movementProbeDestination(player, probe);
        const cell = cells.get(`${destination.x},${destination.y}`);
        return {
          probe,
          index,
          occupied: occupied.has(`${destination.x},${destination.y}`) ? 1 : 0,
          blocked: cell && (cell.blocked === true || cell.closedDoor === true) ? 1 : 0,
          unknown: cell ? 0 : 1,
          freedom: openNeighbourCount(destination),
          targetDistance: chebyshev(destination, target),
        };
      })
      .filter((entry) => !movementProbeTouchesTransfer(player, entry.probe, mapTransfers))
      .sort((left, right) =>
        left.occupied - right.occupied ||
        left.blocked - right.blocked ||
        left.unknown - right.unknown ||
        left.targetDistance - right.targetDistance ||
        right.freedom - left.freedom ||
        left.index - right.index
      )
      .map((entry) => entry.probe);
    const avoided = new Set(avoidPositions);
    const unvisited = ranked.filter(
      (probe) => !avoided.has(
        `${movementProbeDestination(player, probe).x},${movementProbeDestination(player, probe).y}`,
      ),
    );
    const revisits = ranked.filter(
      (probe) => avoided.has(
        `${movementProbeDestination(player, probe).x},${movementProbeDestination(player, probe).y}`,
      ),
    );
    // History is a preference, not collision. In a one-cell building pocket
    // the only physical exit may be the tile just visited; returning only the
    // novel subset made every other direction fail forever. Exhaust unseen
    // neighbours first, then the remaining legal ranked directions.
    return [...unvisited, ...revisits];
  } catch {
    return probes;
  }
}

async function travelToMap(
  targetMapFileName,
  {
    autoUsePotions = true,
    minimumStartingGold = 0,
    resourceBaseline = null,
    resourceAccountingGoal = null,
    enforceCombatResourceBudget = true,
    clearTrivialOccupancy = enforceCombatResourceBudget,
  } = {},
) {
  const targetMap = String(targetMapFileName);
  if (!mapTravelGraph) throw new Error(`map travel graph is unavailable for ${targetMap}`);
  let scriptedGoldSpent = 0;
  let journeyResourceBaseline = enforceCombatResourceBudget ? resourceBaseline : null;
  let journeyResourceGoal = enforceCombatResourceBudget ? resourceAccountingGoal : null;
  for (let hop = 0; hop < 32; hop += 1) {
    let state = await readAgentState(client);
    const currentMap = String(state.mapFileName);
    if (currentMap === targetMap) return state;
    journeyResourceBaseline ??= enforceCombatResourceBudget ? state : null;
    journeyResourceGoal ??= enforceCombatResourceBudget
      ? {
          kind: "travel",
          questId: 0,
          monsterName: `map travel ${currentMap}->${targetMap}`,
          travelLabel: `map ${currentMap}->${targetMap}`,
        }
      : null;
    if (state.activeNpcDialog) {
      await closeNpcDialog();
      state = await readAgentState(client);
    }
    if (state.questWindowOpen) {
      await closeQuestDiary();
      state = await readAgentState(client);
    }
    const path = findMapTravelRoute(mapTravelGraph, currentMap, targetMap);
    if (!path?.length) throw new Error(`no Crystal map movement path from ${currentMap} to ${targetMap}`);
    if (path[0].kind === "npc-script") {
      const requiredNow = Math.max(
        0,
        Number(minimumStartingGold ?? 0) - scriptedGoldSpent,
      );
      if (requiredNow > Number(state.gold ?? 0)) {
        state = await ensureVisibleScriptTravelFunding(requiredNow, state);
      }
      await executeVisibleNpcScriptMapTransfer(path[0], {
        autoUsePotions,
        clearTrivialOccupancy,
        resourceBaseline: journeyResourceBaseline,
        resourceAccountingGoal: journeyResourceGoal,
      });
      scriptedGoldSpent += Math.max(0, Number(path[0].goldCost ?? 0));
      continue;
    }
    const nextMap = String(path[0].toMapFileName);
    const liveTransfers = (state.mapTransfers ?? [])
      .filter((entry) => String(entry.toMapFileName) === nextMap)
      .sort((left, right) =>
        distanceToTransferBounds(state.player, left) - distanceToTransferBounds(state.player, right)
      );
    const sourcePortals = [...(path[0].portals ?? [])]
      .filter((portal) =>
        Number.isFinite(Number(portal?.source?.x)) &&
        Number.isFinite(Number(portal?.source?.y))
      )
      .sort((left, right) =>
        chebyshev(state.player, left.source) - chebyshev(state.player, right.source)
      );
    // MapInformation precedes the full world snapshot during a normal map
    // change. If the live transfer list is temporarily empty, use the same
    // Crystal source topology to choose only the physical tile to walk onto.
    // The gateway remains authoritative and performs the actual transfer.
    const transferCandidates = liveTransfers.length > 0
      ? liveTransfers
      : sourcePortals.map((sourcePortal) => ({
          key: `source-map-move:${currentMap}:${sourcePortal.source.x}:${sourcePortal.source.y}->${nextMap}`,
          mapFileName: currentMap,
          minX: Number(sourcePortal.source.x),
          maxX: Number(sourcePortal.source.x),
          minY: Number(sourcePortal.source.y),
          maxY: Number(sourcePortal.source.y),
          toMapFileName: nextMap,
        }));
    if (!transferCandidates.length) {
      throw new Error(
        `map graph selected ${currentMap}->${nextMap}, but neither the live client nor Crystal topology exposed a matching transfer`,
      );
    }
    let transfer = null;
    let lastTransferApproachError = null;
    for (const candidate of transferCandidates) {
      state = await readAgentState(client);
      if (String(state.mapFileName) === nextMap) {
        transfer = candidate;
        break;
      }
      let target = nearestPointInTransferBounds(state.player, candidate);
      let distance = distanceToTransferBounds(state.player, candidate);
      console.log(
        `  map travel ${hop + 1}: ${currentMap}->${nextMap} via ${candidate.key} ` +
        `at ${target.x},${target.y} distance=${distance}`,
      );
      try {
        let reachedDuringDetour = false;
        // The nearest valid portal can sit hundreds of tiles beyond an
        // aggressive source band. Reuse the source-backed risk surface for up
        // to three progressive physical waypoints; one elbow is insufficient
        // on long diagonal routes because the direct second leg can re-enter
        // the same band. Every waypoint is still reached through normal
        // collision-aware mouse/keyboard input.
        for (let detour = 0; detour < 3 && distance >= 80; detour += 1) {
          state = await readAgentState(client);
          const travelHazards = aggressiveRespawnTravelHazards(
            state,
            journeyResourceGoal?.kind === "hunt"
              ? journeyResourceGoal.monsterName
              : null,
          );
          let reachedSafeWaypoint = false;
          const attemptedWaypoints = new Set();
          // Risk ranking deliberately knows nothing about static collision.
          // Try the improved candidates in score order and reject only the
          // individual unreachable waypoint. Rotating the entire portal after
          // candidate zero failed made three adjacent entrances repeat the
          // same impossible lateral point without moving at all.
          for (let candidateIndex = 0; candidateIndex < 32; candidateIndex += 1) {
            const corridorWaypoint = respawnCorridorAvoidanceWaypoint(
              state.player,
              target,
              travelHazards,
              {
                minimumImprovementRatio: 0.9,
                minimumLegDistance: 24,
                perpendicularOffsets: [24, 40, 64, 96, 128],
                progressRatios: [0.33, 0.5, 0.67],
                candidateIndex,
              },
            );
            if (!corridorWaypoint) break;
            const waypointKey = `${corridorWaypoint.x},${corridorWaypoint.y}`;
            if (attemptedWaypoints.has(waypointKey)) continue;
            attemptedWaypoints.add(waypointKey);
            console.log(
              `  map hostile-corridor detour: ${currentMap}->${nextMap} via ` +
              `${corridorWaypoint.x},${corridorWaypoint.y} ` +
              `candidate=${candidateIndex + 1} ` +
              `exposure=${Number(corridorWaypoint.directExposure).toFixed(1)}->` +
              `${Number(corridorWaypoint.detourExposure).toFixed(1)}`,
            );
            const detourDistance = chebyshev(state.player, corridorWaypoint);
            try {
              await navigateNear(corridorWaypoint, 2, {
                maxAttempts: respawnTravelAttemptBudget(detourDistance),
                allowTransferToMap: nextMap,
                transferKey: candidate.key,
                autoUsePotions,
                clearTrivialOccupancy,
                resourceBaseline: journeyResourceBaseline,
                resourceAccountingGoal: journeyResourceGoal,
              });
              reachedSafeWaypoint = true;
              break;
            } catch (error) {
              if (!(error instanceof NavigationUnreachableError)) throw error;
              console.log(
                `  reject unreachable hostile-corridor waypoint: ` +
                `${corridorWaypoint.x},${corridorWaypoint.y}`,
              );
            }
          }
          if (!reachedSafeWaypoint) {
            console.log(
              `  no reachable improved hostile-corridor waypoint for ` +
              `${currentMap}->${nextMap}; retaining direct physical route with resource budget`,
            );
            break;
          }
          state = await readAgentState(client);
          if (String(state.mapFileName) === nextMap) {
            reachedDuringDetour = true;
            break;
          }
          target = nearestPointInTransferBounds(state.player, candidate);
          distance = distanceToTransferBounds(state.player, candidate);
        }
        if (!reachedDuringDetour) {
          await navigateNear(target, 0, {
            maxAttempts: Math.min(260, Math.max(50, Math.ceil(distance * 1.5))),
            allowTransferToMap: nextMap,
            transferKey: candidate.key,
            autoUsePotions,
            clearTrivialOccupancy,
            resourceBaseline: journeyResourceBaseline,
            resourceAccountingGoal: journeyResourceGoal,
          });
        }
        transfer = candidate;
        break;
      } catch (error) {
        const afterApproach = await readAgentState(client).catch(() => null);
        if (String(afterApproach?.mapFileName ?? "") === nextMap) {
          transfer = candidate;
          break;
        }
        if (!isRetryableVisibleTransferNavigationError(error)) throw error;
        lastTransferApproachError = error;
        console.log(
          `  rotate unreachable visible transfer ${candidate.key}: ` +
          `${String(error?.message ?? error)}`,
        );
      }
    }
    if (!transfer) {
      throw lastTransferApproachError ?? new Error(
        `all visible transfers from ${currentMap} to ${nextMap} were unreachable`,
      );
    }
    const ready = await waitUntil(
      client,
      `String(window.__mir2Stage5?.state?.mapFileName ?? '') === ${JSON.stringify(nextMap)} && window.__mir2Stage5?.state?.sceneInteractionReady === true`,
      45_000,
    );
    if (!ready) throw new Error(`visible transfer reached ${nextMap}, but its scene never became interactive`);
    collisionRegionCache = null;
    state = await readAgentState(client);
    recordMilestone("visible-map-transfer", state, {
      fromMapFileName: currentMap,
      toMapFileName: nextMap,
      transferKey: transfer.key,
    });
    assertNoShortcutFrames();
  }
  throw new Error(`map travel exceeded 32 visible transfers before reaching ${targetMap}`);
}

async function ensureVisibleScriptTravelFunding(targetGold, providedState = null) {
  const requiredGold = Math.max(0, Math.ceil(Number(targetGold ?? 0)));
  let state = providedState ?? await readAgentState(client);
  if (Number(state.gold ?? 0) >= requiredGold) return state;
  const supplyMap = String(BICHON_Q1_Q9_ROUTE.mapFileName);
  if (String(state.mapFileName) !== supplyMap) {
    throw new Error(
      `visible scripted travel needs ${requiredGold} gold, but safe funding is unavailable on ` +
      `map ${String(state.mapFileName)}`,
    );
  }

  for (let attempt = 0; attempt < 24; attempt += 1) {
    assertRuntimeBudget("funding a visible scripted map journey");
    if (Number(state.gold ?? 0) >= requiredGold) return state;
    const beforeGold = Number(state.gold ?? 0);
    state = await liquidateSupersededGearForPotions(state, requiredGold);
    if (Number(state.gold ?? 0) >= requiredGold) return state;
    if (Number(state.gold ?? 0) > beforeGold) continue;

    const acted = await fundHealthPotionsWithSafeHuntIfNeeded(state, {
      minimumGoldTarget: requiredGold,
      fundingReason: "visible scripted map journey",
    });
    state = await readAgentState(client);
    if (!acted) {
      throw new Error(
        `normal client economy could not fund visible scripted travel ` +
        `${Number(state.gold ?? 0)}/${requiredGold}`,
      );
    }
  }
  throw new Error(
    `visible scripted travel funding exceeded 24 ordinary economy actions ` +
    `${Number(state.gold ?? 0)}/${requiredGold}`,
  );
}

async function executeVisibleNpcScriptMapTransfer(
  edge,
  {
    autoUsePotions = true,
    clearTrivialOccupancy = true,
    resourceBaseline = null,
    resourceAccountingGoal = null,
  } = {},
) {
  const fromMap = String(edge?.fromMapFileName ?? "");
  const toMap = String(edge?.toMapFileName ?? "");
  const targetSequence = Array.isArray(edge?.targetSequence)
    ? edge.targetSequence.map(String)
    : [];
  if (!fromMap || !toMap || !targetSequence.length || !edge?.npc) {
    throw new Error(`invalid visible NPC script transfer ${fromMap}->${toMap}`);
  }

  let state = await readAgentState(client);
  if (String(state.mapFileName) !== fromMap) {
    throw new Error(
      `visible NPC script transfer expected map ${fromMap}, found ${String(state.mapFileName)}`,
    );
  }
  const minimumGoldExclusive = Number(edge.minimumGoldExclusive);
  if (
    Number.isFinite(minimumGoldExclusive) &&
    Number(state.gold ?? 0) <= minimumGoldExclusive
  ) {
    throw new Error(
      `${edge.npc.name} visible transfer ${fromMap}->${toMap} requires more than ` +
      `${minimumGoldExclusive} gold; player has ${Number(state.gold ?? 0)}`,
    );
  }
  const requiredItems = Array.isArray(edge.requiredItems) ? edge.requiredItems : [];
  for (const requirement of requiredItems) {
    const available = visibleItemQuantity(state, requirement.item);
    if (available < Number(requirement.count ?? 1)) {
      throw new Error(
        `${edge.npc.name} visible transfer ${fromMap}->${toMap} requires ` +
        `${requirement.item} x${Number(requirement.count ?? 1)}; player has ${available}`,
      );
    }
  }
  const itemCosts = Array.isArray(edge.itemCosts) ? edge.itemCosts : [];
  const itemQuantitiesBefore = Object.fromEntries(
    itemCosts.map((cost) => [String(cost.item), visibleItemQuantity(state, cost.item)]),
  );

  const npc = {
    npcIndex: Number(edge.npc.objectId),
    label: String(edge.npc.name),
    mapFileName: fromMap,
    x: Number(edge.npc.position?.x),
    y: Number(edge.npc.position?.y),
  };
  await openNpcDialog(npc, targetSequence[0], {
    clearTrivialOccupancy,
    resourceBaseline,
    resourceAccountingGoal,
  });
  state = await readAgentState(client);
  const beforeGold = Number(state.gold ?? 0);
  const beforePosition = state.player
    ? { x: Number(state.player.x), y: Number(state.player.y) }
    : null;

  for (let index = 0; index < targetSequence.length; index += 1) {
    const target = targetSequence[index];
    await clickDialogTarget(
      target,
      `visible-npc-script-transfer-${fromMap}-${toMap}-${index + 1}`,
    );
    if (index + 1 < targetSequence.length) {
      const nextTarget = targetSequence[index + 1];
      const nextVisible = await waitUntil(
        client,
        `document.querySelector(${JSON.stringify(dialogTargetSelector(nextTarget))}) != null`,
        8_000,
      );
      if (!nextVisible) {
        throw new Error(
          `${edge.npc.name} scripted transfer did not expose next visible target ${nextTarget}`,
        );
      }
    }
  }

  const ready = await waitUntil(
    client,
    `String(window.__mir2Stage5?.state?.mapFileName ?? '') === ${JSON.stringify(toMap)} && window.__mir2Stage5?.state?.sceneInteractionReady === true`,
    45_000,
  );
  if (!ready) {
    throw new Error(
      `${edge.npc.name} visible script was clicked but ${toMap} never became interactive`,
    );
  }
  collisionRegionCache = null;
  state = await readAgentState(client);
  const afterGold = Number(state.gold ?? 0);
  const goldCost = Math.max(0, Number(edge.goldCost ?? 0));
  if (goldCost > 0 && beforeGold - afterGold !== goldCost) {
    throw new Error(
      `${edge.npc.name} transfer gold audit failed: expected ${goldCost}, ` +
      `observed ${beforeGold}->${afterGold}`,
    );
  }
  for (const cost of itemCosts) {
    const item = String(cost.item);
    const expected = Math.max(0, Number(cost.count ?? 0));
    const before = Number(itemQuantitiesBefore[item] ?? 0);
    const after = visibleItemQuantity(state, item);
    if (before - after !== expected) {
      throw new Error(
        `${edge.npc.name} transfer item audit failed for ${item}: ` +
        `expected ${expected}, observed ${before}->${after}`,
      );
    }
  }
  recordMilestone("visible-npc-script-transfer", state, {
    fromMapFileName: fromMap,
    toMapFileName: toMap,
    scriptKey: String(edge.scriptKey ?? ""),
    targetSequence,
    npc: String(edge.npc.name),
    beforePosition,
    destination: edge.destination ?? null,
    goldCost,
    goldBefore: beforeGold,
    goldAfter: afterGold,
    requiredItems,
    itemCosts,
    itemQuantitiesBefore,
    itemQuantitiesAfter: Object.fromEntries(
      itemCosts.map((cost) => [String(cost.item), visibleItemQuantity(state, cost.item)]),
    ),
    autoUsePotions,
  });
  console.log(
    `  visible NPC transfer: ${fromMap}->${toMap} via ${edge.npc.name} ` +
    `targets=${targetSequence.join(",")} gold=${beforeGold}->${afterGold}`,
  );
  assertNoShortcutFrames();
  return state;
}

function visibleItemQuantity(state, itemName) {
  const requested = normalizeName(itemName);
  return [...(state?.inventoryItems ?? []), ...(state?.beltItems ?? [])]
    .filter((item) => normalizeName(item?.name ?? item?.key) === requested)
    .reduce((total, item) => total + Math.max(1, Number(item?.quantity ?? 1)), 0);
}

async function recoverRouteMapIfAdjacent(providedState) {
  let state = providedState;
  const routeMap = String(BICHON_Q1_Q9_ROUTE.mapFileName);
  if (String(state.mapFileName) === routeMap) return state;
  const transfer = (state.mapTransfers ?? []).find((entry) =>
    String(entry?.toMapFileName) === routeMap &&
    distanceToTransferBounds(state.player, entry) <= 2
  );
  if (!transfer) return state;

  const startedAt = Date.now();
  const fromMap = String(state.mapFileName);
  for (let attempt = 0; attempt < 4 && String(state.mapFileName) !== routeMap; attempt += 1) {
    const target = nearestPointInTransferBounds(state.player, transfer);
    const probe = movementProbesToward(state.player, target)
      .sort((left, right) =>
        chebyshev(movementProbeDestination(state.player, left), target) -
          chebyshev(movementProbeDestination(state.player, right), target) ||
        left.keys.length - right.keys.length
      )[0];
    if (!probe) break;
    const positionSignature = `${state.player.x},${state.player.y}`;
    await waitForDiscreteMovementInput();
    if (probe.keys.length === 1) {
      const [key] = probe.keys;
      await client.pressKey(key.key, key.code, key.vk, {
        action: "return-through-adjacent-map-transfer",
        fromMap,
        toMap: routeMap,
      });
    } else {
      await client.pressKeyChord(probe.keys, {
        action: "return-through-adjacent-map-transfer",
        fromMap,
        toMap: routeMap,
      });
    }
    await waitUntil(
      client,
      `(() => { const state = window.__mir2Stage5?.state ?? {}; const player = state.authoritativePlayer ?? state.player; return String(state.mapFileName) === ${JSON.stringify(routeMap)} || (player != null && String(player.x) + ',' + String(player.y) !== ${JSON.stringify(positionSignature)}); })()`,
      4_000,
    );
    state = await readAgentState(client);
  }
  if (String(state.mapFileName) === routeMap) {
    const sceneReady = await waitUntil(
      client,
      `String(window.__mir2Stage5?.state?.mapFileName) === ${JSON.stringify(routeMap)} && window.__mir2Stage5?.state?.sceneInteractionReady === true`,
      30_000,
    );
    if (!sceneReady) throw new Error(`returned from ${fromMap} but route scene did not become interactive`);
    state = await readAgentState(client);
    recordMilestone("route-map-returned-through-visible-transfer", state, {
      fromMap,
      toMap: routeMap,
      durationMs: Date.now() - startedAt,
    });
  }
  return state;
}

function navigationTransferTileKeys(mapTransfers = []) {
  const keys = [];
  for (const transfer of mapTransfers ?? []) {
    const minX = Number(transfer?.minX);
    const maxX = Number(transfer?.maxX);
    const minY = Number(transfer?.minY);
    const maxY = Number(transfer?.maxY);
    if (![minX, maxX, minY, maxY].every(Number.isFinite)) continue;
    for (let x = minX; x <= maxX; x += 1) {
      for (let y = minY; y <= maxY; y += 1) keys.push(`${x},${y}`);
    }
  }
  return keys;
}

function navigationTileIsTransfer(point, mapTransfers = []) {
  return navigationTransferTileKeys(mapTransfers).includes(`${Number(point?.x)},${Number(point?.y)}`);
}

function rejectedCollisionMemoryKey(mapFileName, cellKey) {
  return `${String(mapFileName)}|${String(cellKey)}`;
}

function rememberRejectedCollisionCell(mapFileName, cellKey, now = Date.now()) {
  navigationRejectedCollisionCellUntil.set(
    rejectedCollisionMemoryKey(mapFileName, cellKey),
    now + REJECTED_COLLISION_CELL_TTL_MS,
  );
}

function activeRejectedCollisionCells(mapFileName, now = Date.now()) {
  const prefix = `${String(mapFileName)}|`;
  const cells = [];
  for (const [key, until] of navigationRejectedCollisionCellUntil) {
    if (Number(until) <= now) {
      navigationRejectedCollisionCellUntil.delete(key);
      continue;
    }
    if (key.startsWith(prefix)) cells.push(key.slice(prefix.length));
  }
  return cells;
}

function distanceToTransferBounds(point, transfer) {
  return chebyshev(point, nearestPointInTransferBounds(point, transfer));
}

function nearestPointInTransferBounds(point, transfer) {
  return {
    x: Math.max(Number(transfer.minX), Math.min(Number(transfer.maxX), Number(point.x))),
    y: Math.max(Number(transfer.minY), Math.min(Number(transfer.maxY), Number(point.y))),
  };
}

function movementProbeDestination(player, probe) {
  const destination = { x: Number(player.x), y: Number(player.y) };
  for (const key of probe.keys) {
    if (key.direction === "left") destination.x -= 1;
    if (key.direction === "right") destination.x += 1;
    if (key.direction === "up") destination.y -= 1;
    if (key.direction === "down") destination.y += 1;
  }
  return destination;
}

function movementProbeTouchesTransfer(player, probe, mapTransfers = []) {
  const point = { x: Number(player.x), y: Number(player.y) };
  for (const key of probe.keys) {
    if (key.direction === "left") point.x -= 1;
    if (key.direction === "right") point.x += 1;
    if (key.direction === "up") point.y -= 1;
    if (key.direction === "down") point.y += 1;
    if (navigationTileIsTransfer(point, mapTransfers)) return true;
  }
  return false;
}

async function waitForMovementBurst(
  previousSignature,
  timeoutMs,
  {
    autoUsePotions = true,
    resourceBaseline = null,
    resourceAccountingGoal = null,
  } = {},
) {
  const startedAt = Date.now();
  let lastSignature = previousSignature;
  let lastChangeAt = startedAt;
  let moved = false;
  while (Date.now() - startedAt < timeoutMs) {
    const state = await readAgentState(client);
    if (
      resourceBaseline && resourceAccountingGoal &&
      rememberQuestCombatResourceStrain(
        resourceAccountingGoal,
        resourceBaseline,
        state,
      )
    ) {
      throw new CombatResourceBudgetError(
        `${resourceAccountingGoal.monsterName} movement exceeded the sustainable combat resource budget`,
      );
    }
    if (!state.playerDead && autoUsePotions) await usePotionIfNeeded(state);
    const current = state.player ? `${state.player.x},${state.player.y}` : "none";
    if (current !== lastSignature) {
      moved = true;
      lastSignature = current;
      lastChangeAt = Date.now();
      rememberAcknowledgedMovement(lastChangeAt);
    }
    // A real right-click can leave the client's movement plan alive across a
    // walk/run cadence gap longer than 900ms. Returning during that gap makes
    // the next physical click replace the valid route after only one tile.
    // Let the visible client finish (or abandon) its own plan first.
    if (moved && !state.movementPlan && Date.now() - lastChangeAt >= 450) {
      // A rejected route can optimistically move one tile and then receive an
      // authoritative correction to the starting tile. Count only the settled
      // position, otherwise each outer patrol chunk reports progress while the
      // player remains forever at the same collision boundary.
      return current !== previousSignature;
    }
    if (!moved && !state.movementPlan && Date.now() - startedAt >= 2_500) return false;
    await delay(140);
  }
  return moved && lastSignature !== previousSignature;
}

async function recoverPlayerIfNeeded(
  providedState = null,
  { autoUsePotions = true } = {},
) {
  let state = providedState ?? await readAgentState(client);
  let revivedInTown = false;
  if (state.wsState !== "open") {
    const reconnected = await waitUntil(
      client,
      "window.__mir2Stage5?.state?.wsState === 'open' || window.__mir2Stage5?.state?.screen === 'select' || window.__mir2Stage5?.state?.screen === 'login'",
      35_000,
    );
    if (!reconnected) throw new Error("client did not recover its WebSocket within 35s");
    state = await readAgentState(client);
    if (state.screen === "login") await loginThroughVisibleUi();
    if ((await readAgentState(client)).screen === "select") await createAndStartCharacterThroughVisibleUi();
    state = await readAgentState(client);
  }

  if (state.playerDead || state.deathOverlayVisible) {
    evidence.deaths += 1;
    // A death can arrive while another modal (most often Inventory) is open.
    // The revive overlay intentionally sits above it, so revive first and then
    // close the stale panel once input is available again.
    let revived = false;
    let reviveLocation = null;
    for (let attempt = 0; attempt < 3 && !revived; attempt += 1) {
      const actionable = await waitUntil(
        client,
        "(() => { const button = document.querySelector('[data-testid=\"town-revive-button\"]'); return button instanceof HTMLButtonElement && !button.disabled; })()",
        12_000,
      );
      if (!actionable) continue;
      const reviveRequestedAt = Date.now();
      await client.clickSelector('[data-testid="town-revive-button"]', {
        action: "revive-in-town",
        attempt: attempt + 1,
      });
      const visuallyAlive = await waitUntil(
        client,
        "window.__mir2Stage5?.state?.playerHp > 0 && document.querySelector('[data-testid=\"town-revive-button\"]') == null",
        12_000,
      );
      if (visuallyAlive) {
        // The rendered HP update can win the browser event loop by one frame
        // over the command audit. Also, a shared-Zone death can present its
        // overlay while the private mirror still has a few HP, causing the
        // first visible TownRevive click to return no packets. Confirm both
        // authoritative reply packets before treating either case as revived.
        const packetDeadline = Date.now() + 3_000;
        while (Date.now() < packetDeadline) {
          reviveLocation = wsPacketsSince(
            client,
            reviveRequestedAt,
            "UserLocation",
          ).at(-1) ?? null;
          const revivedPacket = wsPacketsSince(
            client,
            reviveRequestedAt,
            "Revived",
          ).at(-1) ?? null;
          if (reviveLocation && revivedPacket) {
            revived = true;
            break;
          }
          await delay(100);
        }
      }
      if (!revived) {
        console.log(
          `  town revive attempt ${attempt + 1} lacked authoritative ` +
          "UserLocation/Revived evidence; retrying visibly",
        );
      }
    }
    if (!revived) throw new Error("town revive did not restore the player");
    if (!reviveLocation || !Number.isFinite(Number(reviveLocation.x)) || !Number.isFinite(Number(reviveLocation.y))) {
      throw new Error("town revive restored HP without an authoritative UserLocation");
    }
    // UserLocation precedes Revived on the wire, but the rendered self entity
    // can still carry its death-field transform for another frame. HP alone is
    // therefore not a safe completion signal: starting the next route from
    // that stale position can take a second lethal hit before the town snap is
    // visible. Wait for the same self object the player sees to match the
    // authoritative revive destination.
    const renderedTownLocationSettled = await waitUntil(
      client,
      `(() => { const s = window.__mir2Stage5?.state ?? {}; const entities = Array.isArray(s.entities) ? s.entities : []; const self = entities.find((entry) => String(entry?.objectId) === String(s.playerObjectId)); return self != null && Number(self.x) === ${Number(reviveLocation.x)} && Number(self.y) === ${Number(reviveLocation.y)}; })()`,
      12_000,
    );
    if (!renderedTownLocationSettled) {
      throw new Error(
        `town revive location did not settle at ${Number(reviveLocation.x)},${Number(reviveLocation.y)}`,
      );
    }
    evidence.revives += 1;
    revivedInTown = true;
    // A death returns the character to town. Local collision detours describe
    // the old field position, so restart route choice from the authoritative
    // revive transform.
    navigationDetourByTarget.clear();
    collisionRegionCache = null;
    state = await readAgentState(client);
    if (await client.evaluate("document.querySelector('.inventory-window') != null")) {
      await closeInventory();
      state = await readAgentState(client);
    }
    recordMilestone("death-recovered", state);
  }
  if (revivedInTown) {
    await restockHealthPotionsIfNeeded(state).catch((error) => {
      console.warn(`  visible potion restock deferred: ${String(error?.message ?? error)}`);
    });
  }
  if (autoUsePotions) {
    await usePotionIfNeeded(await readAgentState(client));
  }
  return readAgentState(client);
}

function healthPotionQuantity(state) {
  return [...(state.beltItems ?? []), ...(state.inventoryItems ?? [])]
    .filter((item) => /\(hp\).*drug|health.*potion/i.test(String(item.name ?? item.key ?? "")))
    .reduce((total, item) => total + Math.max(1, Number(item.quantity ?? 1)), 0);
}

async function recoverHealthInSafeInteriorIfNeeded(providedState = null) {
  if (!extendedRouteEnabled || maxQuestId < 23) return false;
  let state = providedState ?? await readAgentState(client);
  if (!state?.player || state.playerDead || state.deathOverlayVisible) return false;
  const homeMapFileName = String(BICHON_Q1_Q9_ROUTE.mapFileName);
  const currentMapFileName = String(state.mapFileName);
  if (![homeMapFileName, SAFE_RECOVERY_MAP_FILE_NAME].includes(currentMapFileName)) {
    return false;
  }
  const maxHp = Number(state.playerMaxHp ?? 0);
  const healthRatio = maxHp > 0 ? Number(state.playerHp ?? 0) / maxHp : 1;
  let shelterActive = Date.now() < supplyFundingShelterUntil;
  if (currentMapFileName === SAFE_RECOVERY_MAP_FILE_NAME && shelterActive) {
    // The field deadline exists only to keep one retreat committed to the
    // portal. Once the ordinary map transfer succeeds, safety is established;
    // clear the deadline and wait only for the normal >=90% HP recovery gate.
    supplyFundingShelterUntil = 0;
    shelterActive = false;
  }
  // A live attacker must be escaped through ordinary movement first. Starting
  // a long shelter route while still in its attack window only burns the
  // remaining potion reserve before the player leaves the field.
  const activeShelterThreat = nearestActiveHostile(state, {
    maxDistance: 8,
    withinMs: ACTIVE_TRAVEL_THREAT_WINDOW_MS,
  });
  if (activeShelterThreat) {
    if (!shelterActive) return false;
    // With several attackers, repeatedly moving away from whichever one is
    // closest makes the retreat vector flip and can zig-zag until death. The
    // visible recovery portal is a stable safe destination. Move toward its
    // rendered transfer bounds while they are available; only fall back to a
    // geometric flee vector on maps without that ordinary exit.
    const recoveryTransfer = (state.mapTransfers ?? [])
      .filter((transfer) => (
        String(transfer.toMapFileName ?? "") === SAFE_RECOVERY_MAP_FILE_NAME &&
        [transfer.minX, transfer.maxX, transfer.minY, transfer.maxY]
          .every((value) => Number.isFinite(Number(value)))
      ))
      .sort((left, right) => (
        distanceToTransferBounds(state.player, left) -
        distanceToTransferBounds(state.player, right)
      ))[0] ?? null;
    const retreat = recoveryTransfer
      ? nearestPointInTransferBounds(state.player, recoveryTransfer)
      : retreatPointFromHostile(state, activeShelterThreat, 8);
    console.log(
      `  safe funding shelter retreat: ${activeShelterThreat.name} ` +
      `${activeShelterThreat.objectId}@${activeShelterThreat.x},${activeShelterThreat.y} ` +
      `target=${retreat?.x ?? "none"},${retreat?.y ?? "none"}`,
    );
    if (retreat) {
      // A normal flee only needs separation, but a map transfer must be
      // entered. Stopping one tile from the portal leaves the player exposed
      // and turns the shelter loop into a no-input busy wait.
      await navigateNear(retreat, recoveryTransfer ? 0 : 1, {
        maxAttempts: 2,
        abortOnDeath: true,
        // Preserve stock while HP is healthy, but let the normal potion
        // threshold save a critically injured character during the physical
        // retreat. With zero stock this remains a pure passive-recovery path.
        autoUsePotions: true,
        allowTransferToMap: recoveryTransfer
          ? SAFE_RECOVERY_MAP_FILE_NAME
          : null,
        transferKey: recoveryTransfer?.key ?? null,
      }).catch(() => false);
    }
    await delay(250);
    return true;
  }
  // This interior is the zero-potion funding shelter, not a mandatory heal
  // stop for every ordinary quest fight on map 0. A character retaining the
  // bounded field reserve is governed by the normal combat potion threshold
  // and per-goal resource budget; sending it across the whole map after each
  // non-lethal hit turns a safe field quest into an unbounded commute.
  if (
    !shelterActive &&
    healthPotionQuantity(state) >= HEALTH_POTION_FIELD_RESERVE
  ) return false;
  if (healthRatio >= SAFE_FUNDING_READY_HEALTH_RATIO && !shelterActive) return false;

  if (currentMapFileName !== SAFE_RECOVERY_MAP_FILE_NAME) {
    console.log(
      `  safe passive recovery: map ${currentMapFileName}->${SAFE_RECOVERY_MAP_FILE_NAME} ` +
      `HP=${Number(state.playerHp ?? 0)}/${maxHp} potions=${healthPotionQuantity(state)}`,
    );
    const shelterEscapeGoal = {
      kind: "travel",
      questId: 0,
      monsterName: "safe-shelter escape",
      travelLabel: `visible ${currentMapFileName}->${SAFE_RECOVERY_MAP_FILE_NAME} shelter escape`,
    };
    try {
      state = await travelToMap(SAFE_RECOVERY_MAP_FILE_NAME, {
        // The long trip can cross a newly aggroed patrol. Emergency potion
        // use is safer than deliberately dying while carrying stock; the
        // destination merchant can replenish whatever was consumed.
        autoUsePotions: true,
        // A static NPC and a wandering beginner monster can close every exit
        // from a one-cell building pocket (live r25: 305,607). After repeated
        // physical route failure, permit the existing certified-occupancy
        // path to clear exactly one adjacent low-level blocker. This remains
        // ordinary selected-target combat and the travel-wide strain guard
        // aborts at the same critical-HP boundary as every other route.
        resourceBaseline: state,
        resourceAccountingGoal: shelterEscapeGoal,
        clearTrivialOccupancy: true,
      });
    } catch (error) {
      if (
        error instanceof NavigationInterruptedByDeathError ||
        error instanceof SupplyFundingSafetyError
      ) return true;
      const recoveredDuringApproach = await readAgentState(client).catch(() => null);
      const recoveredMaxHp = Number(recoveredDuringApproach?.playerMaxHp ?? 0);
      const recoveredHealthRatio = recoveredMaxHp > 0
        ? Number(recoveredDuringApproach?.playerHp ?? 0) / recoveredMaxHp
        : 0;
      if (
        !shelterActive &&
        recoveredDuringApproach &&
        !recoveredDuringApproach.playerDead &&
        !recoveredDuringApproach.deathOverlayVisible &&
        recoveredHealthRatio >= SAFE_FUNDING_READY_HEALTH_RATIO &&
        Date.now() >= supplyFundingShelterUntil
      ) {
        recordMilestone("safe-passive-health-recovered-en-route", recoveredDuringApproach, {
          healthRatio: recoveredHealthRatio,
          potionQuantity: healthPotionQuantity(recoveredDuringApproach),
        });
        console.log(
          `  safe passive recovery completed before interior: ` +
          `${Number(recoveredDuringApproach.playerHp ?? 0)}/${recoveredMaxHp}`,
        );
        return true;
      }
      throw error;
    }
  }

  const recoveryDeadline = Math.min(
    Date.now() + 5 * 60_000,
    evidence.startedAt + maxRuntimeMs,
  );
  let nextProgressLogAt = 0;
  while (Date.now() < recoveryDeadline) {
    assertRuntimeBudget("waiting for safe passive health recovery");
    state = await readAgentState(client);
    if (
      String(state.mapFileName) === SAFE_RECOVERY_MAP_FILE_NAME &&
      Date.now() < supplyFundingShelterUntil
    ) {
      // The same function may have just completed the ordinary map transfer.
      // Its local shelterActive value was captured on the field, so repeat the
      // arrival acknowledgement here instead of idling at full HP until the
      // two-minute field latch naturally expires.
      supplyFundingShelterUntil = 0;
      shelterActive = false;
    }
    if (state.playerDead || state.deathOverlayVisible) {
      await recoverPlayerIfNeeded(state, { autoUsePotions: false });
      return true;
    }
    const liveMaxHp = Number(state.playerMaxHp ?? 0);
    const liveHealthRatio = liveMaxHp > 0
      ? Number(state.playerHp ?? 0) / liveMaxHp
      : 1;
    if (
      liveHealthRatio >= SAFE_FUNDING_READY_HEALTH_RATIO &&
      Date.now() >= supplyFundingShelterUntil
    ) {
      recordMilestone("safe-passive-health-recovered", state, {
        healthRatio: liveHealthRatio,
        potionQuantity: healthPotionQuantity(state),
      });
      console.log(
        `  safe passive recovery complete: ` +
        `${Number(state.playerHp ?? 0)}/${liveMaxHp} potions=${healthPotionQuantity(state)}`,
      );
      return true;
    }
    if (Date.now() >= nextProgressLogAt) {
      console.log(
        `  safe passive recovery waiting: ` +
        `${Number(state.playerHp ?? 0)}/${liveMaxHp} ` +
        `shelterMs=${Math.max(0, supplyFundingShelterUntil - Date.now())}`,
      );
      nextProgressLogAt = Date.now() + 30_000;
    }
    await delay(2_000);
  }
  throw new Error(
    `safe passive recovery did not reach ${SAFE_FUNDING_READY_HEALTH_RATIO} ` +
    `within ${Math.round((5 * 60_000) / 1000)}s`,
  );
}

function localPotionSupplyIncomplete(state) {
  if (!extendedRouteEnabled || maxQuestId < 23 || !state?.player) return false;
  const merchant = BICHON_Q1_Q9_ROUTE.npcs.merchantRuben;
  return String(state.mapFileName) === String(BICHON_Q1_Q9_ROUTE.mapFileName) &&
    chebyshev(state.player, merchant) <= HEALTH_POTION_RESTOCK_RADIUS &&
    healthPotionQuantity(state) < HEALTH_POTION_DEPARTURE_STOCK;
}

async function retreatFromUnsafeActiveThreatIfNeeded(providedState = null) {
  if (!extendedRouteEnabled || maxQuestId < 23) return false;
  const state = providedState ?? await readAgentState(client);
  if (!state?.player || state.playerDead || state.deathOverlayVisible) return false;
  const potionQuantity = healthPotionQuantity(state);
  const maxHp = Number(state.playerMaxHp ?? 0);
  const healthRatio = maxHp > 0 ? Number(state.playerHp ?? 0) / maxHp : 1;
  const lowStock = potionQuantity < HEALTH_POTION_FIELD_RESERVE;
  const unsafeHealth = healthRatio < QUEST_DEPARTURE_HEALTH_RATIO;
  if (!lowStock && !unsafeHealth) return false;
  const homeMapFileName = String(BICHON_Q1_Q9_ROUTE.mapFileName);
  const merchant = BICHON_Q1_Q9_ROUTE.npcs.merchantRuben;
  const inSupplyArea =
    String(state.mapFileName) === homeMapFileName &&
    chebyshev(state.player, merchant) <= HEALTH_POTION_RESTOCK_RADIUS;
  const activeThreat = nearestActiveHostile(state, {
    maxDistance: 8,
    withinMs: ACTIVE_TRAVEL_THREAT_WINDOW_MS,
  });
  if (!activeThreat) return false;
  supplyFundingShelterUntil = Math.max(
    supplyFundingShelterUntil,
    Date.now() + SUPPLY_FUNDING_THREAT_SHELTER_MS,
  );
  // A monster can momentarily occupy the same tile as the player. A pure
  // "move away" vector then has no stable direction and may flip on every
  // policy turn as the monster follows. Prefer the ordinary visible shelter
  // transfer as a fixed destination whenever this map exposes one.
  const recoveryTransfer = (state.mapTransfers ?? [])
    .filter((transfer) => (
      String(transfer.toMapFileName ?? "") === SAFE_RECOVERY_MAP_FILE_NAME &&
      [transfer.minX, transfer.maxX, transfer.minY, transfer.maxY]
        .every((value) => Number.isFinite(Number(value)))
    ))
    .sort((left, right) => (
      distanceToTransferBounds(state.player, left) -
      distanceToTransferBounds(state.player, right)
    ))[0] ?? null;
  const retreat = recoveryTransfer
    ? nearestPointInTransferBounds(state.player, recoveryTransfer)
    : retreatPointFromHostile(state, activeThreat, 8);
  console.log(
    `  unsafe ${inSupplyArea ? "supply" : "field"} disengage: ${activeThreat.name} ` +
    `${activeThreat.objectId}@${activeThreat.x},${activeThreat.y} ` +
    `HP=${Number(state.playerHp ?? 0)}/${maxHp} ` +
    `potions=${potionQuantity}/${HEALTH_POTION_FIELD_RESERVE} ` +
    `target=${retreat?.x ?? "none"},${retreat?.y ?? "none"}`,
  );
  if (retreat) {
    await navigateNear(retreat, recoveryTransfer ? 0 : 1, {
      maxAttempts: 2,
      abortOnDeath: true,
      autoUsePotions: true,
      allowTransferToMap: recoveryTransfer
        ? SAFE_RECOVERY_MAP_FILE_NAME
        : null,
      transferKey: recoveryTransfer?.key ?? null,
    }).catch(() => false);
  }
  await delay(250);
  return true;
}

async function recoverQuestDepartureHealthIfNeeded(providedState = null) {
  if (!extendedRouteEnabled || maxQuestId < 23) return false;
  let state = providedState ?? await readAgentState(client);
  if (!state?.player || state.playerDead || state.deathOverlayVisible) return false;
  const maxHp = Number(state.playerMaxHp ?? 0);
  const healthRatio = maxHp > 0 ? Number(state.playerHp ?? 0) / maxHp : 1;
  if (healthRatio >= QUEST_DEPARTURE_HEALTH_RATIO) return false;

  // With no remaining potion on a non-home map, the normal visible supply
  // return below owns the route. Holding here would starve that return forever.
  const currentMapFileName = String(state.mapFileName ?? "");
  if (
    healthPotionQuantity(state) <= 0 &&
    ![
      String(BICHON_Q1_Q9_ROUTE.mapFileName),
      SAFE_RECOVERY_MAP_FILE_NAME,
    ].includes(currentMapFileName)
  ) return false;

  const usedPotion = await usePotionIfNeeded(
    state,
    QUEST_DEPARTURE_HEALTH_RATIO,
  );
  state = await readAgentState(client);
  if (await retreatFromUnsafeActiveThreatIfNeeded(state)) return true;
  console.log(
    `  hold quest departure for HP recovery: ` +
    `${Number(state.playerHp ?? 0)}/${Number(state.playerMaxHp ?? maxHp)} ` +
    `potions=${healthPotionQuantity(state)}/${HEALTH_POTION_DEPARTURE_STOCK}`,
  );
  await delay(usedPotion ? 900 : 500);
  return true;
}

async function returnToSupplyAreaForPotionsIfNeeded(providedState = null) {
  if (!extendedRouteEnabled || maxQuestId < 23) return false;
  let state = providedState ?? await readAgentState(client);
  if (!state?.player || state.playerDead || state.deathOverlayVisible) return false;
  if (nearestActiveHostile(state, {
    maxDistance: 8,
    withinMs: ACTIVE_TRAVEL_THREAT_WINDOW_MS,
  })) return false;
  const homeMapFileName = String(BICHON_Q1_Q9_ROUTE.mapFileName);
  const merchant = BICHON_Q1_Q9_ROUTE.npcs.merchantRuben;
  const potionQuantity = healthPotionQuantity(state);
  const inSupplyArea =
    String(state.mapFileName) === homeMapFileName &&
    chebyshev(state.player, merchant) <= HEALTH_POTION_RESTOCK_RADIUS;
  if (
    !potionSupplyRecallRequested &&
    (
      potionQuantity >= HEALTH_POTION_DEPARTURE_STOCK ||
      (!inSupplyArea && potionQuantity >= HEALTH_POTION_FIELD_RESERVE)
    )
  ) return false;
  let changedMap = false;
  if (String(state.mapFileName) !== homeMapFileName) {
    console.log(
      `  ${potionSupplyRecallRequested ? "combat-strain" : "incomplete HP supply"} return: ` +
      `map ${state.mapFileName}->${homeMapFileName}, ` +
      `potions=${healthPotionQuantity(state)}/${HEALTH_POTION_DEPARTURE_STOCK}`,
    );
    state = await travelToMap(homeMapFileName, {
      enforceCombatResourceBudget: false,
    });
    changedMap = true;
  }
  const distance = chebyshev(state.player, merchant);
  // A completed map transfer invalidates the caller's pre-transfer snapshot
  // even when the destination already lies inside the broad supply radius.
  // Report that progress so the main policy re-reads map, position and stock
  // before evaluating its hard departure gate.
  if (distance <= HEALTH_POTION_RESTOCK_RADIUS) return changedMap;
  console.log(
    `  ${potionSupplyRecallRequested ? "combat-strain" : "incomplete HP supply"} return: ` +
    `player=${state.player.x},${state.player.y} ` +
    `merchant=${merchant.x},${merchant.y} distance=${distance} ` +
    `potions=${healthPotionQuantity(state)}/${HEALTH_POTION_DEPARTURE_STOCK}`,
  );
  try {
    await navigateNear(merchant, 5, {
      maxAttempts: respawnTravelAttemptBudget(distance),
      abortOnDeath: true,
    });
  } catch (error) {
    if (error instanceof NavigationEnteredUnexpectedMapError) {
      console.log(
        `  protected transfer recovery: ${error.actualMapFileName}->${homeMapFileName} ` +
        "through visible map travel",
      );
      state = await travelToMap(homeMapFileName, {
        enforceCombatResourceBudget: false,
      });
      recordMilestone("protected-transfer-recovered", state, {
        fromMapFileName: error.actualMapFileName,
        toMapFileName: homeMapFileName,
      });
      assertNoShortcutFrames();
      return true;
    }
    if (!(error instanceof NavigationInterruptedByDeathError)) throw error;
    // navigateNear already performed the normal visible TownRevive before
    // reporting this interruption. Re-enter the policy loop from that safe
    // authoritative transform, where ordinary funding/restock can continue.
    return true;
  }
  return true;
}

async function collectVisibleHealthPotionDropIfNeeded(providedState = null) {
  let state = providedState ?? await readAgentState(client);
  if (state.playerDead || state.deathOverlayVisible || healthPotionQuantity(state) > 0) return false;
  const now = Date.now();
  for (const [objectId, until] of groundDropCooldownUntil) {
    if (until <= now) groundDropCooldownUntil.delete(objectId);
  }
  let drop = nearestHealthPotionGroundDrop({
    ...state,
    groundDrops: (state.groundDrops ?? []).filter(
      (entry) => !groundDropCooldownUntil.has(String(entry.objectId)),
    ),
  }, 8);
  if (!drop) return false;

  const objectId = String(drop.objectId);
  if (chebyshev(state.player, drop) > 1) {
    const approached = await navigateNear(drop, 1, {
      maxAttempts: Math.min(10, Math.max(4, chebyshev(state.player, drop) + 2)),
      abortOnDeath: true,
      failFastWhenCollisionPathUnavailable: true,
    }).then(() => true, () => false);
    if (!approached) {
      groundDropCooldownUntil.set(
        objectId,
        Date.now() + OPTIONAL_DROP_UNREACHABLE_COOLDOWN_MS,
      );
      return false;
    }
    state = await readAgentState(client);
    drop = state.groundDrops.find((entry) => String(entry.objectId) === objectId) ?? null;
    if (!drop || !state.player || chebyshev(state.player, drop) > 1) {
      groundDropCooldownUntil.set(
        objectId,
        Date.now() + OPTIONAL_DROP_UNREACHABLE_COOLDOWN_MS,
      );
      return false;
    }
  }

  const quantityBefore = healthPotionQuantity(state);
  let clicked = false;
  try {
    await client.clickSelector(`button.ground-drop-marker[data-object-id="${objectId}"]`, {
      action: "pick-up-visible-health-potion",
      item: String(drop.name),
      objectId,
    });
    clicked = true;
  } catch {
    // A corpse can cover the marker; the ordinary Space pickup below is the
    // same nearby interaction a player would use after walking into range.
  }
  if (clicked) await delay(300);
  state = await readAgentState(client);
  if (
    !state.playerDead &&
    !state.deathOverlayVisible &&
    (state.groundDrops ?? []).some((entry) => String(entry.objectId) === objectId)
  ) {
    await client.pressKey(" ", "Space", 32, {
      action: "pick-up-visible-health-potion-underfoot",
      item: String(drop.name),
      objectId,
    });
  }
  await waitUntil(
    client,
    `(() => { const s = window.__mir2Stage5?.state ?? {}; const items = [...(s.beltItems ?? []), ...(s.inventoryItems ?? [])]; const quantity = items.filter((item) => /\\(hp\\).*drug|health.*potion/i.test(String(item?.name ?? item?.key ?? ''))).reduce((total, item) => total + Math.max(1, Number(item?.quantity ?? 1)), 0); return quantity > ${quantityBefore} || Number(s.playerHp ?? 0) <= 0 || document.querySelector('[data-testid="town-revive-button"]') != null; })()`,
    8_000,
  );
  const after = await readAgentState(client);
  const quantityAfter = healthPotionQuantity(after);
  if (quantityAfter <= quantityBefore) {
    groundDropCooldownUntil.set(
      objectId,
      Date.now() + OPTIONAL_DROP_REJECTED_COOLDOWN_MS,
    );
    return false;
  }
  groundDropCooldownUntil.delete(objectId);

  const pickup = {
    itemName: String(drop.name),
    objectId,
    x: Number(drop.x),
    y: Number(drop.y),
    quantityBefore,
    quantityAfter,
    at: Date.now(),
  };
  evidence.supplyPickups.push(pickup);
  recordMilestone("health-potion-ground-drop-picked-up", after, pickup);
  console.log(`  visible HP-drug pickup: ${quantityBefore}->${quantityAfter}`);
  return true;
}

function assertSafeSupplyFundingState(goal, state, requestedMonsterName = null) {
  if (!goal?.supplyFunding || !state?.player) return;
  const maxHp = Number(state.playerMaxHp ?? 0);
  const healthRatio = maxHp > 0 ? Number(state.playerHp ?? 0) / maxHp : 1;
  const blockingThreat = nearestSafeBlockingHostile(
    state,
    requestedMonsterName ?? goal.monsterName,
  );
  if (
    healthRatio >= SAFE_FUNDING_MIN_HEALTH_RATIO &&
    !blockingThreat
  ) return;
  supplyFundingShelterUntil = Math.max(
    supplyFundingShelterUntil,
    Date.now() + SUPPLY_FUNDING_THREAT_SHELTER_MS,
  );
  const reason = blockingThreat
    ? `${blockingThreat.name} ${blockingThreat.objectId} is actively blocking the funding route`
    : `HP ratio ${healthRatio.toFixed(3)} is below ${SAFE_FUNDING_MIN_HEALTH_RATIO}`;
  throw new SupplyFundingSafetyError(reason);
}

function assertSafeSupplyNpcActionState(state, actionLabel) {
  if (!state?.player || state.playerDead || state.deathOverlayVisible) return;
  const potionQuantity = healthPotionQuantity(state);
  const maxHp = Number(state.playerMaxHp ?? 0);
  const healthRatio = maxHp > 0 ? Number(state.playerHp ?? 0) / maxHp : 1;
  const activeThreat = nearestActiveHostile(state, {
    maxDistance: 8,
    withinMs: ACTIVE_TRAVEL_THREAT_WINDOW_MS,
  });
  // Stock is not safety while a monster is actively landing hits. Live r34
  // entered the broad village supply radius with nine drugs, then repeatedly
  // attempted a distant liquidation trip while a ForestYeti consumed every
  // one. Reject the NPC action before the stock fast-path so recovery owns the
  // next physical input.
  if (activeThreat) {
    supplyFundingShelterUntil = Math.max(
      supplyFundingShelterUntil,
      Date.now() + SUPPLY_FUNDING_THREAT_SHELTER_MS,
    );
    throw new SupplyFundingSafetyError(
      `${activeThreat.name} ${activeThreat.objectId} is attacking during ${actionLabel}`,
    );
  }
  if (potionQuantity >= HEALTH_POTION_FIELD_RESERVE) return;
  if (healthRatio >= SAFE_FUNDING_READY_HEALTH_RATIO && !activeThreat) return;
  supplyFundingShelterUntil = Math.max(
    supplyFundingShelterUntil,
    Date.now() + SUPPLY_FUNDING_THREAT_SHELTER_MS,
  );
  throw new SupplyFundingSafetyError(
    `HP ratio ${healthRatio.toFixed(3)} is below ${SAFE_FUNDING_READY_HEALTH_RATIO}`,
  );
}

async function fundHealthPotionsWithSafeHuntIfNeeded(
  providedState = null,
  { minimumGoldTarget = null, fundingReason = "health potions" } = {},
) {
  if (!extendedRouteEnabled || maxQuestId < 23) return false;
  let state = providedState ?? await readAgentState(client);
  const merchant = BICHON_Q1_Q9_ROUTE.npcs.merchantRuben;
  const missingPotionQuantity = Math.max(
    0,
    HEALTH_POTION_DEPARTURE_STOCK - healthPotionQuantity(state),
  );
  const fundingUnitPrice = Number.isFinite(knownHealthPotionUnitPrice)
    ? knownHealthPotionUnitPrice
    : HEALTH_POTION_CATALOG_BASE_PRICE;
  const explicitGoldTarget = Number(minimumGoldTarget);
  const hasExplicitGoldTarget = minimumGoldTarget != null && Number.isFinite(explicitGoldTarget);
  const fundingGoldTarget = Math.max(
    missingPotionQuantity * fundingUnitPrice,
    hasExplicitGoldTarget ? Math.max(0, explicitGoldTarget) : 0,
  );
  const currentPotionQuantity = healthPotionQuantity(state);
  if (!shouldFundHealthPotions(state, {
    homeMapFileName: BICHON_Q1_Q9_ROUTE.mapFileName,
    merchant,
    merchantRadius: HEALTH_POTION_RESTOCK_RADIUS,
    minimumGold: fundingGoldTarget,
    minimumPotions: hasExplicitGoldTarget
      ? currentPotionQuantity + 1
      : HEALTH_POTION_DEPARTURE_STOCK,
  })) return false;
  // A no-op funding check must not arm the shelter latch merely because an
  // incidental monster attacks while stock and gold already satisfy the
  // request. Enforce NPC-action safety only after funding is actually needed.
  assertSafeSupplyNpcActionState(state, fundingReason);

  assertRuntimeBudget(`funding ${fundingReason} through safe hunting`);
  if (await collectNearbyGoldIfVisible(state)) return true;
  if (await collectVisibleSafeSupplyLootIfNeeded(state)) return true;

  // A real beginner has a deterministic local economy: Deer are passive,
  // Provinces/Deer yields ordinary Venison on every completed harvest, and the
  // nearby Butcher's visible [Types] 15 shop buys it. Prefer that guaranteed
  // mouse/keyboard loop over waiting indefinitely for Scarecrow's 1/10 Gold.
  const fundingHealthRatio = Number(state.playerMaxHp ?? 0) > 0
    ? Number(state.playerHp ?? 0) / Number(state.playerMaxHp)
    : 0;
  const fundingPotionQuantity = healthPotionQuantity(state);
  const emergencyDeerHarvest =
      fundingPotionQuantity < HEALTH_POTION_FUNDING_WORKING_STOCK &&
    fundingHealthRatio >= SAFE_FUNDING_READY_HEALTH_RATIO;
  const useDeerHarvest =
    (
      fundingPotionQuantity >= HEALTH_POTION_FUNDING_WORKING_STOCK ||
      emergencyDeerHarvest
    ) &&
    fundingHealthRatio >= 0.75 &&
    Date.now() >= deerFundingUnavailableUntil;
  const fundingGoal = useDeerHarvest
    ? {
        kind: "grind",
        questId: 0,
        monsterName: "Deer",
        itemName: "Venison",
        harvest: true,
        supplyFunding: true,
        fundingReason,
        fundingGoldTarget,
      }
    : {
        kind: "grind",
        questId: 0,
        monsterName: "Scarecrow",
        harvest: false,
        supplyFunding: true,
        fundingReason,
        fundingGoldTarget,
      };
  const fundingStateBefore = state;
  const catalogFundingFields = authoritativeFundingFields(
    fundingGoal.monsterName,
    state,
  );
  const target = await findMonster(
      fundingGoal.monsterName,
      catalogFundingFields.length > 0
        ? catalogFundingFields
        : BICHON_Q1_Q9_ROUTE.fields[fundingGoal.monsterName],
      fundingGoal,
      fundingStateBefore,
    );
    if (!target) {
      throw new Error(
        `no live ${fundingGoal.monsterName} found for ordinary ${fundingReason} funding`,
      );
    }

    state = await readAgentState(client);
    const experienceBefore = Number(state.playerExperience ?? 0);
    const result = await killMonster(
      target,
      fundingGoal,
      { current: 0, required: 1 },
      experienceBefore,
      Date.now(),
      fundingStateBefore,
    );
    if (!result.success) {
      // A short-lived beginner monster can die and respawn before the
      // target-correlated ObjectDied/rendered-corpse observation settles. Do
      // not call that a confirmed kill, but also do not immediately chase the
      // next monster away from a physical Gold/HP/sellable drop that has just
      // appeared under the player. A real player would pause and pick up the
      // visible supply. Keep this recovery bounded and retain the original
      // failure if no useful drop materialises.
      const unconfirmedDropDeadline = Date.now() + 2_500;
      while (Date.now() < unconfirmedDropDeadline) {
        const afterUnconfirmedHunt = await readAgentState(client);
        if (
          await collectVisibleHealthPotionDropIfNeeded(afterUnconfirmedHunt) ||
          await collectNearbyGoldIfVisible(afterUnconfirmedHunt) ||
          await collectVisibleSafeSupplyLootIfNeeded(afterUnconfirmedHunt)
        ) {
          console.log(
            `  visible supply recovered after unconfirmed ${fundingGoal.monsterName} hunt`,
          );
          return true;
        }
        await delay(200);
      }
      throw new Error(result.reason ?? "safe potion-funding hunt was not confirmed");
    }
    let harvest = null;
    if (fundingGoal.harvest) {
      const venisonQuantityBefore = (state.inventoryItems ?? [])
        .filter((item) => normalizeName(item.name) === normalizeName("Venison"))
        .reduce((total, item) => total + Math.max(1, Number(item.quantity ?? 1)), 0);
      harvest = await harvestCorpse(
        result.corpse ?? target,
        fundingGoal,
        { current: 0, required: 1 },
      );
      if (!harvest.completed || !harvest.progressed) {
        // The corpse can disappear one world frame before its ordinary item
        // drop is mounted. Stay at the physical harvest site for a short,
        // bounded settlement window and accept only an actual visible Venison
        // pickup. ObjectHarvested alone still never counts as funding.
        const deferredHarvestDropDeadline = Date.now() + 2_500;
        let recoveredDeferredVenison = false;
        while (Date.now() < deferredHarvestDropDeadline) {
          const deferredState = await readAgentState(client);
          await collectVisibleSafeSupplyLootIfNeeded(deferredState).catch(() => false);
          const afterDeferredPickup = await readAgentState(client);
          const venisonQuantityAfter = (afterDeferredPickup.inventoryItems ?? [])
            .filter((item) => normalizeName(item.name) === normalizeName("Venison"))
            .reduce((total, item) => total + Math.max(1, Number(item.quantity ?? 1)), 0);
          if (venisonQuantityAfter > venisonQuantityBefore) {
            recoveredDeferredVenison = true;
            harvest = {
              completed: true,
              progressed: true,
              recoveredFromVisibleDrop: true,
            };
            console.log("  deferred Deer harvest drop recovered: Venison");
            break;
          }
          await delay(200);
        }
        if (!recoveredDeferredVenison) {
          // If no inventory packet/state or physical drop follows (for
          // example under a stale profile), cool this strategy down and use
          // the ordinary Scarecrow economy on the next policy turn.
          deerFundingUnavailableUntil = Date.now() + 10 * 60_000;
          throw new Error("visible Deer harvest completed without ordinary Venison inventory progress");
        }
      }
    }
    let after = await readAgentState(client);
    const supplyDropDeadline = Date.now() + 2_500;
    while (Date.now() < supplyDropDeadline) {
      if (await collectVisibleHealthPotionDropIfNeeded(after)) break;
      if (await collectNearbyGoldIfVisible(after)) break;
      if (await collectVisibleSafeSupplyLootIfNeeded(after)) break;
      await delay(200);
      after = await readAgentState(client);
    }
    after = await readAgentState(client);
    if (useDeerHarvest) {
      const strain = assessQuestCombatResourceStrain(fundingStateBefore, after);
      if (strain.severe) {
        deerFundingUnavailableUntil = Date.now() + 30 * 60_000;
        evidence.combatResourceStrains.push({
          questId: 0,
          monsterName: fundingGoal.monsterName,
          playerLevel: Number(after.playerLevel ?? fundingStateBefore.playerLevel ?? 1),
          preparationLevel: null,
          supplyFunding: true,
          ...strain,
          at: Date.now(),
        });
        console.log(
          `  supply resource risk: Deer ` +
          `HP=${strain.hp}/${strain.maxHp} potions=${strain.potionsBefore}->${strain.potionsAfter}; ` +
          "cooling down harvest source",
        );
      }
    }
    evidence.kills.push({
      questId: 0,
      monsterName: fundingGoal.monsterName,
      objectId: String(target.objectId),
      harvested: fundingGoal.harvest,
      harvestCompleted: harvest?.completed ?? null,
      harvestProgressed: harvest?.progressed ?? null,
      supplyFunding: true,
      experienceBefore,
      experienceAfter: after.playerExperience,
      at: Date.now(),
    });
    recordMilestone(
      hasExplicitGoldTarget
        ? "script-travel-funding-hunt"
        : "health-potion-funding-hunt",
      after,
      {
      objectId: String(target.objectId),
      gold: Number(after.gold ?? 0),
      fundingReason,
      fundingGoldTarget,
    });
    console.log(
      fundingGoal.harvest
        ? `  visible supply harvest: Deer -> Venison, gold=${Number(after.gold ?? 0)}`
        : `  visible supply hunt fallback: Scarecrow, gold=${Number(after.gold ?? 0)}`,
    );
  return true;
}

function authoritativeFundingFields(monsterName, state) {
  const wanted = normalizeName(monsterName);
  const currentMapFileName = String(state?.mapFileName ?? "");
  const player = state?.player ?? null;
  const supplyAnchor = BICHON_Q1_Q9_ROUTE.npcs.merchantRuben;
  return grindingCatalog
    .filter((entry) => normalizeName(entry.monsterName) === wanted)
    .flatMap((entry) => entry.spawns ?? [])
    .filter((spawn) => String(spawn.mapFileName) === currentMapFileName)
    .filter((spawn) => (
      chebyshev(supplyAnchor, spawn.position) <= HEALTH_POTION_FUNDING_FIELD_RADIUS
    ))
    .sort((left, right) => {
      const distance = (spawn) => player
        ? chebyshev(player, spawn.position)
        : Number.POSITIVE_INFINITY;
      return distance(left) - distance(right) ||
        Number(right.count ?? 0) - Number(left.count ?? 0) ||
        Number(left.delayMinutes ?? 0) - Number(right.delayMinutes ?? 0);
    })
    .slice(0, 8)
    .map((spawn) => ({
      mapFileName: String(spawn.mapFileName),
      x: Number(spawn.position?.x),
      y: Number(spawn.position?.y),
      count: Number(spawn.count),
      spread: Number(spawn.spread),
      delayMinutes: Number(spawn.delayMinutes),
    }));
}

async function restockHealthPotionsIfNeeded(providedState = null) {
  if (!extendedRouteEnabled || maxQuestId < 23 || potionRestockInFlight) return false;
  let state = providedState ?? await readAgentState(client);
  // A new ordinary gold pickup should make the shop immediately eligible.
  // Suppress only a same-balance retry after a transient UI failure.
  if (
    Date.now() - lastPotionRestockAt < HEALTH_POTION_RESTOCK_RETRY_MS &&
    Number(state.gold ?? 0) <= lastPotionRestockGold
  ) return false;
  const merchant = BICHON_Q1_Q9_ROUTE.npcs.merchantRuben;
  const supplyHomeMapFileName = String(BICHON_Q1_Q9_ROUTE.mapFileName);
  const resumingInsideLiquidationMerchant =
    String(state.mapFileName) !== supplyHomeMapFileName &&
    SAFE_LIQUIDATION_MERCHANTS.some(
      (route) => String(route.mapFileName) === String(state.mapFileName),
    );
  const initialPotionQuantity = healthPotionQuantity(state);
  const initiallyInSupplyArea =
    String(state.mapFileName) === supplyHomeMapFileName &&
    state.player &&
    chebyshev(state.player, merchant) <= HEALTH_POTION_RESTOCK_RADIUS;
  if (
    potionSupplyRecallRequested &&
    initiallyInSupplyArea &&
    initialPotionQuantity >= HEALTH_POTION_DEPARTURE_STOCK
  ) {
    potionSupplyRecallRequested = false;
  }
  if (initialPotionQuantity >= HEALTH_POTION_DEPARTURE_STOCK) return false;
  // Do not turn a normal field continuation into a liquidation trip merely
  // because the full departure stock is no longer intact. The return policy
  // above owns that decision and recalls the character once the bounded field
  // reserve is actually reached.
  if (
    !initiallyInSupplyArea &&
    !resumingInsideLiquidationMerchant &&
    initialPotionQuantity >= HEALTH_POTION_FIELD_RESERVE
  ) return false;
  // A post-funding purchase happens inside the same outer policy turn. The
  // preceding hunt or liquidation may have crossed the safe HP boundary, so
  // do not rely on the next turn's recovery gate to protect a real NPC trip.
  // This guard intentionally follows the no-op exits above: checking an
  // already-full belt must never create a shelter retreat by itself.
  assertSafeSupplyNpcActionState(state, "visible health-potion restock");
  if (
    resumingInsideLiquidationMerchant &&
    initialPotionQuantity < HEALTH_POTION_DEPARTURE_STOCK &&
    Number(state.gold ?? 0) >= HEALTH_POTION_CATALOG_BASE_PRICE
  ) {
    // A prior run may have completed a visible sale and stopped before its
    // finally block walked home. Resume that same physical supply trip before
    // evaluating Ruben's distance gate.
    state = await travelToMap(supplyHomeMapFileName, {
      enforceCombatResourceBudget: false,
    });
  }
  const estimatedPotionQuantity = healthPotionQuantity(state);
  const estimatedUnitPrice = Number.isFinite(knownHealthPotionUnitPrice)
    ? knownHealthPotionUnitPrice
    : HEALTH_POTION_CATALOG_BASE_PRICE;
  const estimatedRestockQuantityTarget = estimatedPotionQuantity <
      HEALTH_POTION_FUNDING_WORKING_STOCK
    ? Math.min(
        HEALTH_POTION_DEPARTURE_STOCK - estimatedPotionQuantity,
        HEALTH_POTION_FUNDING_WORKING_STOCK - estimatedPotionQuantity,
      )
    : Math.max(
        0,
        HEALTH_POTION_DEPARTURE_STOCK - estimatedPotionQuantity,
      );
  const estimatedMaxHp = Math.max(0, Number(state.playerMaxHp ?? 0));
  const estimatedRecoveryHp = Math.max(
    0,
    Math.ceil(estimatedMaxHp * QUEST_DEPARTURE_HEALTH_RATIO) -
      Math.max(0, Number(state.playerHp ?? 0)),
  );
  const estimatedRecoveryPotions = estimatedRecoveryHp > 0
    ? Math.ceil(estimatedRecoveryHp / HEALTH_POTION_HEAL_AMOUNT) +
      HEALTH_POTION_RECOVERY_DAMAGE_BUFFER
    : 0;
  const estimatedSupplyGoldTarget = estimatedRestockQuantityTarget * estimatedUnitPrice +
    estimatedRecoveryPotions * estimatedUnitPrice;
  if (
    Number(state.gold ?? 0) < estimatedSupplyGoldTarget &&
    estimatedPotionQuantity < HEALTH_POTION_DEPARTURE_STOCK
  ) {
    state = await liquidateSupersededGearForPotions(
      state,
      estimatedSupplyGoldTarget,
    );
  }
  const currentPotionQuantity = healthPotionQuantity(state);
  const planningUnitPrice = Number.isFinite(knownHealthPotionUnitPrice)
    ? knownHealthPotionUnitPrice
    : HEALTH_POTION_CATALOG_BASE_PRICE;
  const plannedQuantity = planHealthPotionPurchase({
    currentQuantity: currentPotionQuantity,
    gold: Number(state.gold ?? 0),
    unitPrice: planningUnitPrice,
    departureStock: HEALTH_POTION_DEPARTURE_STOCK,
    workingStock: HEALTH_POTION_FUNDING_WORKING_STOCK,
  });
  if (
    String(state.mapFileName) !== supplyHomeMapFileName ||
    !state.player ||
    chebyshev(state.player, merchant) > HEALTH_POTION_RESTOCK_RADIUS ||
    currentPotionQuantity >= HEALTH_POTION_DEPARTURE_STOCK ||
    Number(state.gold ?? 0) < HEALTH_POTION_CATALOG_BASE_PRICE ||
    (Number.isFinite(knownHealthPotionUnitPrice) &&
      Number(state.gold ?? 0) < knownHealthPotionUnitPrice) ||
    plannedQuantity <= 0
  ) return false;

  potionRestockInFlight = true;
  lastPotionRestockAt = Date.now();
  lastPotionRestockGold = Number(state.gold ?? 0);
  const beforeQuantity = healthPotionQuantity(state);
  const beforeGold = Number(state.gold ?? 0);
  const restockResourceBaseline = state;
  const restockResourceGoal = {
    kind: "travel",
    questId: 0,
    monsterName: "health-potion restock",
    travelLabel: "visible health-potion restock",
  };
  try {
    if (state.activeNpcDialog) await closeNpcDialog();
    if (await client.evaluate("document.querySelector('.quest-log-window') != null")) {
      await closeQuestDiary();
    }
    if (await client.evaluate("document.querySelector('.inventory-window') != null")) {
      await closeInventory();
    }

    await openNpcDialog(merchant, "@BuySell", {
      clearTrivialOccupancy: true,
      resourceBaseline: restockResourceBaseline,
      resourceAccountingGoal: restockResourceGoal,
    });
    await clickDialogTarget("@BuySell", "open-health-merchant-shop");
    const shopOpened = await waitForVisibleSelector(".npc-shop-window", 12_000);
    if (!shopOpened) throw new Error("Merchant Ruben NPCGoods did not open a visible shop");
    const rowSelector = '.npc-shop-row[aria-label="(HP)DrugSmall"]';
    const potionVisible = await waitForVisibleSelector(rowSelector, 8_000);
    if (!potionVisible) throw new Error("Merchant Ruben did not render (HP)DrugSmall");

    const unitPrice = Number(await client.evaluate(`(() => {
      const row = document.querySelector(${JSON.stringify(rowSelector)});
      return row instanceof HTMLElement ? Number(row.dataset.unitPrice ?? 0) : 0;
    })()`));
    if (!Number.isFinite(unitPrice) || unitPrice <= 0) {
      throw new Error(`invalid visible (HP)DrugSmall unit price ${unitPrice}`);
    }
    knownHealthPotionUnitPrice = unitPrice;
    const quantity = planHealthPotionPurchase({
      currentQuantity: beforeQuantity,
      gold: beforeGold,
      unitPrice,
      departureStock: HEALTH_POTION_DEPARTURE_STOCK,
      workingStock: HEALTH_POTION_FUNDING_WORKING_STOCK,
    });
    if (quantity <= 0) {
      console.log(`  visible shop restock deferred: ${beforeGold} gold < ${unitPrice} unit price`);
      return false;
    }

    await client.clickSelector(rowSelector, {
      action: "select-health-potion-shop-row",
      item: "(HP)DrugSmall",
      unitPrice,
    });
    for (let index = 1; index < quantity; index += 1) {
      await client.clickSelector(".npc-shop-window input[type=number] + button", {
        action: "increase-health-potion-quantity",
        item: "(HP)DrugSmall",
        quantity: index + 1,
      });
    }
    await delay(120);
    await client.clickSelector(".npc-shop-confirm", {
      action: "buy-health-potions",
      item: "(HP)DrugSmall",
      quantity,
      unitPrice,
    });
    const purchased = await waitUntil(
      client,
      `(() => { const s = window.__mir2Stage5?.state ?? {}; const items = [...(s.beltItems ?? []), ...(s.inventoryItems ?? [])]; const quantity = items.filter((item) => /\\(hp\\).*drug|health.*potion/i.test(String(item?.name ?? item?.key ?? ''))).reduce((total, item) => total + Math.max(1, Number(item?.quantity ?? 1)), 0); return quantity > ${beforeQuantity} && Number(s.gold ?? 0) < ${beforeGold}; })()`,
      30_000,
    );
    if (!purchased) throw new Error("visible potion purchase was not acknowledged by inventory and gold");
    state = await readAgentState(client);
    const purchase = {
      npc: merchant.label,
      item: "(HP)DrugSmall",
      requestedQuantity: quantity,
      unitPrice,
      quantityBefore: beforeQuantity,
      quantityAfter: healthPotionQuantity(state),
      goldBefore: beforeGold,
      goldAfter: Number(state.gold ?? 0),
      at: Date.now(),
    };
    evidence.shopPurchases.push(purchase);
    if (purchase.quantityAfter >= HEALTH_POTION_DEPARTURE_STOCK) {
      potionSupplyRecallRequested = false;
    }
    recordMilestone("health-potions-purchased", state, purchase);
    console.log(
      `  visible shop restock: HP drug ${purchase.quantityBefore}->${purchase.quantityAfter} ` +
      `gold ${purchase.goldBefore}->${purchase.goldAfter}`,
    );
    return true;
  } finally {
    if (await client.evaluate("document.querySelector('.npc-shop-window') != null").catch(() => false)) {
      await client.clickSelector(".npc-shop-close button", { action: "close-health-merchant-shop" })
        .catch(() => false);
    }
    potionRestockInFlight = false;
  }
}

async function liquidateSupersededGearForPotions(
  state,
  targetGold = HEALTH_POTION_CATALOG_BASE_PRICE,
) {
  assertSafeSupplyNpcActionState(state, "visible supply liquidation");
  const supplyHomeMapFileName = String(BICHON_Q1_Q9_ROUTE.mapFileName);
  const currentMapFileName = String(state.mapFileName);
  const atSupplyHome =
    currentMapFileName === supplyHomeMapFileName &&
    state.player &&
    chebyshev(state.player, BICHON_Q1_Q9_ROUTE.npcs.merchantRuben) <= HEALTH_POTION_RESTOCK_RADIUS;
  const atSafeLiquidationMerchant = SAFE_LIQUIDATION_MERCHANTS.some(
    (route) => String(route.mapFileName) === currentMapFileName,
  );
  if (!state.player || (!atSupplyHome && !atSafeLiquidationMerchant)) return state;
  const progressionCandidates = authoritativeRoute
    ? [
        ...SAFE_STARTER_LIQUIDATION_GEAR,
        ...buildProgressionEquipmentCandidates(authoritativeRoute),
      ]
    : [...SAFE_STARTER_LIQUIDATION_GEAR];
  const candidates = [
    ...supersededProgressionGearForSale(state, progressionCandidates),
    ...duplicateEquippedItemsForSale(state, SAFE_DUPLICATE_EQUIPPED_SUPPLY_LOOT),
    ...ordinarySupplyLootForSale(state, safeOrdinarySupplyLootCatalog),
    ...surplusQuestMaterialsForSale(state, ["CannibalLeaf"]),
  ];
  if (!candidates.length) return state;
  const selectedCandidate = candidates.find((candidate) =>
    SAFE_LIQUIDATION_MERCHANTS.some((route) => liquidationMerchantMatches(route, candidate))
  );
  if (!selectedCandidate) return state;
  const merchantRoute = SAFE_LIQUIDATION_MERCHANTS.find(
    (route) => liquidationMerchantMatches(route, selectedCandidate),
  );
  const merchant = merchantRoute.npc;
  const merchantCandidates = candidates.filter(
    (candidate) => liquidationMerchantMatches(merchantRoute, candidate),
  );
  const liquidationResourceBaseline = state;
  const liquidationResourceGoal = {
    kind: "travel",
    questId: 0,
    monsterName: "supply liquidation",
    travelLabel: `visible ${merchant.label} liquidation`,
  };

  try {
    if (currentMapFileName !== String(merchantRoute.mapFileName)) {
      state = await travelToMap(merchantRoute.mapFileName, {
        resourceBaseline: liquidationResourceBaseline,
        resourceAccountingGoal: liquidationResourceGoal,
      });
    }
    await openNpcDialog(merchant, merchantRoute.dialogTarget, {
      clearTrivialOccupancy: true,
      resourceBaseline: liquidationResourceBaseline,
      resourceAccountingGoal: liquidationResourceGoal,
    });
    await clickDialogTarget(merchantRoute.dialogTarget, "open-health-merchant-shop-for-sale");
    if (!await waitForVisibleSelector(".npc-shop-window", 12_000)) {
      throw new Error(`${merchant.label} shop did not open for gear liquidation`);
    }
    const sellTab = '.npc-shop-tab[data-shop-tab-key="sell"]';
    if (!await waitForVisibleSelector(sellTab, 8_000)) {
      throw new Error(`${merchant.label} did not expose the visible Sell tab`);
    }
    await client.clickSelector(sellTab, { action: "open-health-merchant-sell-tab" });
    if (!await waitUntil(
      client,
      `document.querySelector('.npc-shop-window')?.getAttribute('data-shop-tab') === 'sell'`,
      3_000,
    )) {
      throw new Error(`${merchant.label} Sell tab did not become active`);
    }
    await delay(120);

    for (const candidate of merchantCandidates) {
      let liveCandidate = candidate;
      let row = null;
      let rowSelected = false;
      for (let attempt = 0; attempt < 3 && !rowSelected; attempt += 1) {
        state = await readAgentState(client);
        const liveItem = (state.inventoryItems ?? []).find((item) =>
          String(item.uniqueId) === String(candidate.uniqueId)
        ) ?? (state.inventoryItems ?? []).find((item) =>
          normalizeName(item.name) === normalizeName(candidate.name)
        );
        if (!liveItem) break;
        liveCandidate = { ...candidate, uniqueId: liveItem.uniqueId };
        const visibleRowId = await client.evaluate(`(() => {
          const rows = Array.from(document.querySelectorAll('.npc-shop-row[data-item-id]'));
          const exact = rows.find((entry) => String(entry.getAttribute('data-item-id')) === ${JSON.stringify(String(liveItem.uniqueId))});
          const named = rows.find((entry) => String(entry.getAttribute('data-item-name') ?? '').replace(/[^a-z0-9]/gi, '').toLowerCase() === ${JSON.stringify(normalizeName(candidate.name))});
          return String((exact ?? named)?.getAttribute('data-item-id') ?? '');
        })()`);
        if (!visibleRowId) {
          const renderedRows = await client.evaluate(`(() =>
            Array.from(document.querySelectorAll('.npc-shop-row[data-item-id]'))
              .slice(0, 12)
              .map((entry) => ({
                id: String(entry.getAttribute('data-item-id') ?? ''),
                name: String(entry.getAttribute('data-item-name') ?? ''),
                disabled: Boolean(entry.disabled),
              }))
          )()`);
          console.log(
            `  visible sale row missing: ${candidate.name}#${String(liveItem.uniqueId)} ` +
            `rows=${JSON.stringify(renderedRows)}`,
          );
          await delay(100);
          continue;
        }
        liveCandidate = { ...liveCandidate, uniqueId: visibleRowId };
        row = `.npc-shop-row[data-item-id="${String(visibleRowId)}"]`;
        if (!await waitForVisibleSelector(row, 1_000)) {
          await delay(100);
          continue;
        }
        const scrollDirection = Number(await client.evaluate(`(() => {
          const list = document.querySelector('.npc-shop-list');
          const entry = document.querySelector(${JSON.stringify(row)});
          if (!(list instanceof HTMLElement) || !(entry instanceof HTMLElement)) return 0;
          const listBox = list.getBoundingClientRect();
          const rowBox = entry.getBoundingClientRect();
          if (rowBox.top < listBox.top) return -1;
          if (rowBox.bottom > listBox.bottom) return 1;
          return 0;
        })()`));
        if (scrollDirection !== 0) {
          await client.wheelSelector(
            ".npc-shop-list",
            scrollDirection * 240,
            {
              action: "scroll-superseded-gear-into-view",
              item: liveCandidate.name,
              uniqueId: liveCandidate.uniqueId,
            },
          );
          await delay(180);
        }
        let rowClickError = null;
        rowSelected = await client.clickSelector(row, {
          action: "select-superseded-gear-for-sale",
          item: liveCandidate.name,
          uniqueId: liveCandidate.uniqueId,
          visibleSellValue: Number(liveCandidate.sellValue),
        }).then(
          () => true,
          (error) => {
            rowClickError = String(error?.message ?? error);
            return false;
          },
        );
        if (!rowSelected) {
          const rowHitDiagnostic = await client.evaluate(`(() => {
            const entry = document.querySelector(${JSON.stringify(row)});
            if (!(entry instanceof HTMLElement)) return null;
            const box = entry.getBoundingClientRect();
            const x = box.left + box.width / 2;
            const y = box.top + box.height / 2;
            const top = document.elementFromPoint(x, y);
            return {
              box: { left: box.left, top: box.top, right: box.right, bottom: box.bottom },
              viewport: { width: window.innerWidth, height: window.innerHeight },
              topTag: top?.tagName ?? null,
              topClass: top instanceof HTMLElement ? top.className : null,
            };
          })()`);
          console.log(
            `  visible sale row not physically clickable: ${liveCandidate.name} ` +
            `error=${rowClickError} hit=${JSON.stringify(rowHitDiagnostic)}`,
          );
          await delay(100);
        }
      }
      if (!rowSelected || !row) continue;
      const beforeGold = Number(state.gold ?? 0);
      await client.clickSelector(".npc-shop-confirm", {
        action: "sell-superseded-gear-for-potions",
        item: liveCandidate.name,
        uniqueId: liveCandidate.uniqueId,
      });
      const sold = await waitUntil(
        client,
        `(() => { const s = window.__mir2Stage5?.state ?? {}; return Number(s.gold ?? 0) > ${beforeGold} && !(s.inventoryItems ?? []).some((item) => String(item?.uniqueId) === ${JSON.stringify(String(liveCandidate.uniqueId))}); })()`,
        12_000,
      );
      if (!sold) throw new Error(`visible sale was not acknowledged for ${liveCandidate.name}`);
      // The acknowledged sale predicate above already requires this exact
      // visible row to disappear from the authoritative bag snapshot. Bag
      // unique IDs are recyclable slots, not permanent item identities, so
      // retaining them across later drops would suppress unrelated new loot.
      state = await readAgentState(client);
      recordMilestone("superseded-gear-sold", state, {
        item: liveCandidate.name,
        visibleSellValue: Number(liveCandidate.sellValue),
        goldBefore: beforeGold,
        goldAfter: Number(state.gold ?? 0),
      });
      console.log(
        `  visible sale: ${liveCandidate.name} gold ${beforeGold}->${Number(state.gold ?? 0)}`,
      );
      if (Number(state.gold ?? 0) >= Math.max(
        HEALTH_POTION_CATALOG_BASE_PRICE,
        Number(targetGold ?? 0),
      )) break;
    }
  } finally {
    if (await client.evaluate("document.querySelector('.npc-shop-window') != null").catch(() => false)) {
      await client.clickSelector(".npc-shop-close button", { action: "close-merchant-after-sale" })
        .catch(() => false);
    }
    const liveState = await readAgentState(client).catch(() => null);
    if (
      liveState &&
      String(liveState.mapFileName) !== supplyHomeMapFileName
    ) {
      await travelToMap(supplyHomeMapFileName, {
        enforceCombatResourceBudget: false,
      }).catch((error) => {
        console.warn(
          `  visible return after ${merchant.label} liquidation deferred: ` +
          String(error?.message ?? error),
        );
      });
    }
  }
  return readAgentState(client);
}

async function repairProgressionEquipmentIfNeeded(providedState = null) {
  if (!extendedRouteEnabled || Date.now() < equipmentRepairRetryUntil) return false;
  let state = providedState ?? await readAgentState(client);
  if (!state?.player || state.playerDead || state.deathOverlayVisible) return false;
  const candidates = equipmentRepairCandidates(state, {
    thresholdRatio: EQUIPMENT_REPAIR_THRESHOLD_RATIO,
  });
  if (!candidates.length) return false;

  const homeMerchant = BICHON_Q1_Q9_ROUTE.npcs.merchantRuben;
  const inSupplyArea =
    String(state.mapFileName) === String(BICHON_Q1_Q9_ROUTE.mapFileName) &&
    chebyshev(state.player, homeMerchant) <= HEALTH_POTION_RESTOCK_RADIUS;
  // A broken weapon blocks progression and can justify an immediate safe
  // return. Broken defence/accessory slots wait for the character's next
  // ordinary supply visit; live r23 showed that crossing a hostile map solely
  // for armour repair can consume the entire potion stock before town.
  const urgent = candidates.some((item) => String(item.slot) === "weapon");
  if (!urgent && !inSupplyArea && String(state.mapFileName) !== SAFE_RECOVERY_MAP_FILE_NAME) {
    return false;
  }
  if (nearestActiveHostile(state, {
    maxDistance: 8,
    withinMs: ACTIVE_TRAVEL_THREAT_WINDOW_MS,
  })) return false;

  const now = Date.now();
  for (const [routeKey, until] of equipmentRepairRouteRetryUntil) {
    if (until <= now) equipmentRepairRouteRetryUntil.delete(routeKey);
  }
  const selection = candidates
    .map((candidate) => {
      const route = EQUIPMENT_REPAIR_ROUTES.find((entry) =>
        entry.slots.includes(String(candidate.slot))
      ) ?? null;
      const routeKey = route ? `${route.mapFileName}|${route.npc.npcIndex}` : null;
      return { candidate, route, routeKey };
    })
    .find(({ routeKey }) => routeKey && !equipmentRepairRouteRetryUntil.has(routeKey));
  if (!selection) {
    const unrouted = candidates.find((candidate) => !EQUIPMENT_REPAIR_ROUTES.some((entry) =>
      entry.slots.includes(String(candidate.slot))
    ));
    if (!unrouted) return false;
    equipmentRepairRetryUntil = Date.now() + EQUIPMENT_REPAIR_RETRY_MS;
    console.warn(`  no visible repair merchant route for equipped slot ${String(unrouted.slot)}`);
    return false;
  }
  const { candidate: first, route, routeKey } = selection;
  if (state.activeNpcDialog) await closeNpcDialog();
  if (String(state.mapFileName) !== String(route.mapFileName)) {
    await travelToMap(route.mapFileName, { enforceCombatResourceBudget: false });
  }
  await openNpcDialog(route.npc, "@Repair", { clearTrivialOccupancy: true });
  await clickDialogTarget("@Repair", `open-equipment-repair-${String(first.slot)}`);
  const opened = await waitUntil(
    client,
    `document.querySelector('.npc-shop-window[data-shop-tab="repair"]') != null`,
    10_000,
  );
  if (!opened) {
    equipmentRepairRetryUntil = Date.now() + EQUIPMENT_REPAIR_RETRY_MS;
    throw new Error(`${route.npc.label} visible Repair link did not open the repair list`);
  }

  let repairedCount = 0;
  try {
    for (const candidate of candidates.filter((item) => route.slots.includes(String(item.slot)))) {
      state = await readAgentState(client);
      const live = (state.equipmentItems ?? []).find((item) => String(item.slot) === String(candidate.slot));
      const currentBefore = Number(live?.durabilityCurrent);
      const maximumBefore = Number(live?.durabilityMax);
      if (
        !Number.isFinite(currentBefore) ||
        !Number.isFinite(maximumBefore) ||
        maximumBefore <= 0 ||
        currentBefore >= maximumBefore
      ) continue;
      const row = `button.npc-shop-row[data-item-id="${String(candidate.slot)}"]`;
      const available = await client.evaluate(
        `(() => { const row = document.querySelector(${JSON.stringify(row)}); return row instanceof HTMLButtonElement && !row.disabled; })()`,
      );
      if (!available) continue;
      const goldBefore = Number(state.gold ?? 0);
      await client.clickSelector(row, {
        action: "select-equipment-for-repair",
        slot: String(candidate.slot),
        item: String(candidate.name),
      });
      await client.clickSelector(".npc-shop-confirm", {
        action: "confirm-equipment-repair",
        slot: String(candidate.slot),
        item: String(candidate.name),
      });
      const repaired = await waitUntil(
        client,
        `(() => { const item = (window.__mir2Stage5?.state?.equipmentItems ?? []).find((entry) => String(entry?.slot) === ${JSON.stringify(String(candidate.slot))}); return Number(item?.durabilityCurrent ?? 0) > ${currentBefore}; })()`,
        10_000,
      );
      state = await readAgentState(client);
      const after = (state.equipmentItems ?? []).find((item) => String(item.slot) === String(candidate.slot));
      const currentAfter = Number(after?.durabilityCurrent);
      if (!repaired || !Number.isFinite(currentAfter) || currentAfter <= currentBefore) break;
      const repair = {
        item: String(candidate.name),
        slot: String(candidate.slot),
        durabilityBefore: currentBefore,
        durabilityAfter: currentAfter,
        durabilityMaxBefore: maximumBefore,
        durabilityMaxAfter: Number(after?.durabilityMax ?? maximumBefore),
        goldBefore,
        goldAfter: Number(state.gold ?? 0),
        merchant: route.npc.label,
        at: Date.now(),
      };
      evidence.repairs.push(repair);
      recordMilestone("equipment-repaired", state, repair);
      repairedCount += 1;
      console.log(
        `  visible equipment repair: ${candidate.name} ` +
        `${currentBefore}->${currentAfter}`,
      );
    }
  } finally {
    if (await client.evaluate("document.querySelector('.npc-shop-window') != null").catch(() => false)) {
      await client.clickSelector(".npc-shop-close button", {
        action: "close-equipment-repair-shop",
      }).catch(() => false);
    }
  }
  if (repairedCount === 0) {
    equipmentRepairRouteRetryUntil.set(
      routeKey,
      Date.now() + EQUIPMENT_REPAIR_RETRY_MS,
    );
  } else {
    equipmentRepairRouteRetryUntil.delete(routeKey);
    equipmentRepairRetryUntil = 0;
  }
  return repairedCount > 0;
}

async function usePotionIfNeeded(state, healthRatioThreshold = 0.62) {
  const hp = Number(state.playerHp);
  const maxHp = Number(state.playerMaxHp);
  if (
    !Number.isFinite(hp) ||
    !Number.isFinite(maxHp) ||
    maxHp <= 0 ||
    hp / maxHp >= Math.max(0, Number(healthRatioThreshold ?? 0.62)) ||
    Date.now() - lastPotionUseAt < 2_500
  ) return false;
  const potion = state.beltItems.find((item) => /\(hp\).*drug|health.*potion/i.test(String(item.name ?? item.key ?? "")));
  const slot = Number(potion?.slot);
  if (potion && Number.isInteger(slot) && slot >= 0 && slot <= 5) {
    const key = String(slot + 1);
    await client.pressKey(key, `Digit${key}`, 48 + slot + 1, { action: "use-belt-potion", item: potion.name });
  } else {
    const bagPotion = state.inventoryItems.find((item) => /\(hp\).*drug|health.*potion/i.test(String(item.name ?? item.key ?? "")));
    if (!bagPotion) return false;
    if (state.activeNpcDialog) await closeNpcDialog();
    await openInventory();
    await client.clickSelector(`button.inventory-item-card[aria-label="${bagPotion.name}"]`, {
      action: "use-inventory-potion", item: bagPotion.name,
    });
    await closeInventory();
  }
  lastPotionUseAt = Date.now();
  evidence.potionUses += 1;
  await delay(450);
  return true;
}

async function equipProgressionGearIfReady(state) {
  state = await recoverPlayerIfNeeded(state);
  const level = Number(state.playerLevel ?? 0);
  const candidates = authoritativeRoute
    ? buildProgressionEquipmentCandidates(authoritativeRoute)
    : [
        { minLevel: 5, name: BICHON_Q1_Q9_ROUTE.equipment.q6WarriorChoiceName, slot: "weapon" },
        { minLevel: 2, name: BICHON_Q1_Q9_ROUTE.equipment.q3WarriorChoiceName, slot: "weapon" },
      ];
  const desired = selectBestAvailableEquipmentUpgrade(state, candidates, level);
  if (!desired) return false;
  const { name, slot } = desired;
  if (state.activeNpcDialog) await closeNpcDialog();

  await openInventory();
  state = await readAgentState(client);
  if (state.playerDead || state.deathOverlayVisible) {
    await recoverPlayerIfNeeded(state);
    return false;
  }
  await client.clickSelector(`button.inventory-item-card[aria-label="${name}"]`, {
    action: "equip-progression-item",
    item: name,
    slot,
  });
  const equipped = await waitUntil(
    client,
    `window.__mir2Stage5?.state?.equipmentItems?.some((item) => item?.slot === ${JSON.stringify(slot)} && item?.name === ${JSON.stringify(name)}) === true`,
    10_000,
  );
  await closeInventory();
  if (equipped) recordMilestone("progression-item-equipped", await readAgentState(client), { item: name, slot });
  return equipped;
}

async function learnProgressionSkillIfReady(state) {
  state = await recoverPlayerIfNeeded(state);
  const level = Number(state.playerLevel ?? 0);
  const candidates = progressionSkillBookCatalog.length > 0
    ? progressionSkillBookCatalog
    : CLASS_ONBOARDING_SKILLS[className] ?? [];
  const desired = candidates.find(({ minLevel, name }) =>
    level >= minLevel &&
    state.inventoryItems.some((item) => item.name === name) &&
    !(state.knownSkills ?? []).some((skill) => skill.name === name)
  );
  if (!desired) return false;
  if (state.activeNpcDialog) await closeNpcDialog();

  await openInventory();
  state = await readAgentState(client);
  if (state.playerDead || state.deathOverlayVisible) {
    await recoverPlayerIfNeeded(state);
    return false;
  }
  const { name } = desired;
  await client.clickSelector(`button.inventory-item-card[aria-label="${name}"]`, {
    action: "learn-progression-skill",
    item: name,
  });
  const learned = await waitUntil(
    client,
    `window.__mir2Stage5?.state?.knownSkills?.some((skill) => skill?.name === ${JSON.stringify(name)}) === true`,
    10_000,
  );
  await closeInventory();
  if (!learned) throw new Error(`visible inventory activation did not learn ${name}`);
  recordMilestone("progression-skill-learned", await readAgentState(client), { skill: name });
  return true;
}

async function equipOnboardingGearIfReady(state) {
  state = await recoverPlayerIfNeeded(state);
  const level = Number(state.playerLevel ?? 0);
  const desired = CLASS_ONBOARDING_GEAR.find(({ minLevel, name, slot }) =>
    level >= minLevel &&
    state.inventoryItems.some((item) => item.name === name) &&
    !state.equipmentItems.some((item) => item.slot === slot && item.name === name)
  );
  if (!desired) return false;
  if (state.activeNpcDialog) await closeNpcDialog();

  await openInventory();
  state = await readAgentState(client);
  if (state.playerDead || state.deathOverlayVisible) {
    await recoverPlayerIfNeeded(state);
    return false;
  }
  const { name, slot } = desired;
  await client.clickSelector(`button.inventory-item-card[aria-label="${name}"]`, {
    action: "equip-onboarding-item",
    item: name,
    slot,
  });
  const equipped = await waitUntil(
    client,
    `window.__mir2Stage5?.state?.equipmentItems?.some((item) => item?.slot === ${JSON.stringify(slot)} && item?.name === ${JSON.stringify(name)}) === true`,
    10_000,
  );
  await closeInventory();
  if (!equipped) throw new Error(`visible inventory activation did not equip ${name}`);
  recordMilestone("onboarding-gear-equipped", await readAgentState(client), { item: name, slot });
  return true;
}

async function openInventory() {
  const open = await client.evaluate("document.querySelector('.inventory-window') != null");
  if (open) return;
  await client.clickSelector(".hud-button.inventory button", { action: "open-inventory" });
  const opened = await waitUntil(client, "document.querySelector('.inventory-window') != null", 5_000);
  if (!opened) throw new Error("visible inventory button did not open the inventory");
}

async function closeInventory() {
  const open = await client.evaluate("document.querySelector('.inventory-window') != null");
  if (!open) return;
  let state = await readAgentState(client);
  if (state.playerDead || state.deathOverlayVisible) {
    await recoverPlayerIfNeeded(state);
    return;
  }
  await client.clickSelector(".inventory-close button", { action: "close-inventory" }).catch(() => {});
  let closed = await waitUntil(client, "document.querySelector('.inventory-window') == null", 1_500);
  if (!closed) {
    state = await readAgentState(client);
    if (state.playerDead || state.deathOverlayVisible) {
      await recoverPlayerIfNeeded(state);
      return;
    }
    await client.pressKey("i", "KeyI", 73, { action: "close-inventory-keyboard-fallback" });
    closed = await waitUntil(client, "document.querySelector('.inventory-window') == null", 3_500);
  }
  if (!closed) throw new Error("visible inventory close button did not dismiss the inventory");
}

function rankMonsterApproachTargets(state, candidates, preferNearest = false) {
  const isolated = rankCombatTargetsByIsolation(state, candidates);
  if (!preferNearest || !state?.player) return isolated;
  const isolationOrder = new Map(
    isolated.map((entry, index) => [String(entry.objectId), index]),
  );
  // Supply work is intentionally short and abortable. Minimise time outside
  // the recovery/shop perimeter; use pack isolation only to break equal-range
  // ties. Ordinary quest combat keeps the safer pack-edge ordering above.
  return [...isolated].sort((left, right) => (
    chebyshev(state.player, left) - chebyshev(state.player, right) ||
    Number(isolationOrder.get(String(left.objectId)) ?? 0) -
      Number(isolationOrder.get(String(right.objectId)) ?? 0)
  ));
}

async function nearestVisibleMonsterByName(state, name, preferNearest = false) {
  const candidates = matchingLiveMonsters(state, name);
  const immediateCandidate = preferNearest
    ? rankMonsterApproachTargets(state, candidates, true)
      .find((entry) => chebyshev(state.player, entry) <= 1) ?? null
    : chooseImmediateMeleeTarget(
      state,
      candidates,
      { activeAttackWindowMs: ACTIVE_TRAVEL_THREAT_WINDOW_MS },
    );
  // A sliver of a sprite can be physically clickable beneath the HUD while
  // still being too far away for Crystal's click-to-attack route to reach.
  // The policy either chooses an adjacent active attacker or declines the
  // crowded target so findMonster can approach a safer known edge member.
  if (immediateCandidate) {
    const visibleIds = new Set(
      (await physicalEntityHitTargets([immediateCandidate.objectId]))
        .filter((target) => Number(target.clickableSamples) > 0)
        .map((target) => String(target.objectId)),
    );
    if (visibleIds.has(String(immediateCandidate.objectId))) {
      return immediateCandidate;
    }
  }
  if (preferNearest) return null;

  // When no adjacent target is usable, make the same normal scene click a
  // player would: pick a fully rendered monster close to the viewport centre
  // and let lockMonsterAttack perform the moving-target chase. Probe physical
  // hit surfaces before ranking so an entity hidden by HUD/chrome never wins.
  const clickReachCandidates = candidates.filter(
    (entry) => chebyshev(state.player, entry) <= CLIENT_LOCKED_ATTACK_CLICK_RADIUS,
  );
  const visibleIds = new Set(
    (await physicalEntityHitTargets(clickReachCandidates.map((entry) => entry.objectId)))
      .filter((target) => Number(target.clickableSamples) > 0)
      .map((target) => String(target.objectId)),
  );
  const visibleCandidates = clickReachCandidates.filter(
    (entry) => visibleIds.has(String(entry.objectId)),
  );
  return chooseImmediateMeleeTarget(state, visibleCandidates, {
    engagementRadius: CLIENT_LOCKED_ATTACK_CLICK_RADIUS,
    searchRadius: CLIENT_LOCKED_ATTACK_CLICK_RADIUS,
    activeAttackWindowMs: ACTIVE_TRAVEL_THREAT_WINDOW_MS,
  });
}

function matchingLiveMonsters(state, name) {
  const wanted = normalizeName(name);
  const now = Date.now();
  return state.entities.filter((entry) => (
    entry.kind === "monster" &&
    !entityIsCorpse(entry) &&
    Number(quarantinedMonsterUntil.get(String(entry.objectId)) ?? 0) <= now &&
    (
      Number(monsterCooldownUntil.get(String(entry.objectId)) ?? 0) <= now ||
      entityAttackIsRecent(entry, now, ACTIVE_TRAVEL_THREAT_WINDOW_MS)
    ) &&
    normalizeName(entry.name) === wanted
  ));
}

function entityIsCorpse(entry) {
  return Boolean(entry) && (
    entry.dead === true ||
    (
      entry.hp != null &&
      entry.hp !== "" &&
      Number.isFinite(Number(entry.hp)) &&
      Number(entry.hp) <= 0
    )
  );
}

async function clickEntity(objectId, meta) {
  return client.clickSelector(
    [
      `.entity-sprite-stack[data-object-id="${String(objectId)}"] .entity-sprite-hit`,
      `button.entity-nameplate[data-object-id="${String(objectId)}"][data-ui-interactive="true"]`,
    ].join(", "),
    meta,
  ).then(() => true, () => false);
}

async function logMonsterSearch(monsterName, state, phase) {
  const wanted = normalizeName(monsterName);
  const knownEntities = state.entities
    .filter((entry) => entry.kind === "monster" && !entityIsCorpse(entry) && normalizeName(entry.name) === wanted)
    .slice(0, 32);
  const known = knownEntities.map((entry) => `${entry.objectId}@${entry.x},${entry.y}`);
  const knownIds = new Set(known.map((entry) => entry.split("@", 1)[0]));
  const hitTargets = await physicalEntityHitTargets(knownEntities.map((entry) => entry.objectId));
  const knownHits = hitTargets
    .filter((target) => knownIds.has(String(target.objectId)))
    .map((target) => `${target.objectId}/${target.surface}/${target.width}x${target.height}/${target.clickableSamples}/${target.centerTop}`);
  const clickable = hitTargets
    .filter((target) => Number(target.clickableSamples) > 0)
    .map((target) => `${target.objectId}:${target.clickableSamples}`);
  console.log(`  search ${monsterName} ${phase}: player=${state.player?.x},${state.player?.y} known=[${known.join(" ")}] knownHits=[${knownHits.join(" ")}] clickable=[${clickable.join(" ")}]`);
}

async function physicalEntityHitTargets(objectIds) {
  const ids = [...new Set(objectIds.map((value) => String(value)))].slice(0, 32);
  if (!ids.length) return [];
  return client.evaluate(`
    (() => {
      const ids = new Set(${JSON.stringify(ids)});
      return Array.from(document.querySelectorAll(
        '.entity-sprite-stack[data-object-id] .entity-sprite-hit, button.entity-nameplate[data-object-id][data-ui-interactive="true"]'
      ))
        .filter((node) => ids.has(String(node.closest('[data-object-id]')?.getAttribute('data-object-id') ?? '')))
        .map((node) => {
          if (!(node instanceof HTMLElement)) return null;
          const rect = node.getBoundingClientRect();
          const samples = [[0.5, 0.5], [0.25, 0.25], [0.75, 0.25], [0.25, 0.75], [0.75, 0.75],
            [0.5, 0.2], [0.5, 0.8], [0.2, 0.5], [0.8, 0.5]];
          const hits = samples.map(([fx, fy]) => {
            const top = document.elementFromPoint(rect.left + rect.width * fx, rect.top + rect.height * fy);
            return top === node || node.contains(top);
          });
          const centerTop = document.elementFromPoint(rect.left + rect.width / 2, rect.top + rect.height / 2);
          return {
            objectId: String(node.closest('[data-object-id]')?.getAttribute('data-object-id') ?? ''),
            surface: node.classList.contains('entity-nameplate') ? 'nameplate' : 'sprite',
            left: Math.round(rect.left),
            top: Math.round(rect.top),
            width: Math.round(rect.width),
            height: Math.round(rect.height),
            clickableSamples: hits.filter(Boolean).length,
            centerTop: centerTop instanceof HTMLElement
              ? centerTop.tagName.toLowerCase() + '.' + String(centerTop.className || '')
              : null,
          };
        })
        .filter(Boolean);
    })()
  `);
}

function routeNpcEntity(state, npc, maxDistance) {
  return state.entities
    .filter((entry) => (
      entry.kind === "npc" &&
      !entityIsCorpse(entry) &&
      String(entry.objectId) === String(npc.npcIndex) &&
      chebyshev(entry, npc) <= maxDistance
    ))[0] ?? null;
}

function nearNpcDialog(dialog, entities, npc) {
  const entity = entities.find((entry) => String(entry.objectId) === String(dialog.npcObjectId));
  return Boolean(
    entity &&
    entity.kind === "npc" &&
    String(entity.objectId) === String(npc.npcIndex) &&
    chebyshev(entity, npc) <= 5
  );
}

function pointToward(from, to, maxSpan) {
  const dx = Number(to.x) - Number(from.x);
  const dy = Number(to.y) - Number(from.y);
  const distance = Math.max(Math.abs(dx), Math.abs(dy));
  if (distance <= maxSpan) return { x: Number(to.x), y: Number(to.y) };
  const scale = maxSpan / distance;
  return {
    x: Number(from.x) + Math.round(dx * scale),
    y: Number(from.y) + Math.round(dy * scale),
  };
}

function tileOccupied(state, point) {
  return state.entities.some((entry) => (
    !entityIsCorpse(entry) && entry.x === point.x && entry.y === point.y
  ));
}

function movementProbesToward(from, to) {
  const dx = Number(to.x) - Number(from.x);
  const dy = Number(to.y) - Number(from.y);
  const all = {
    right: { direction: "right", key: "ArrowRight", code: "ArrowRight", vk: 39 },
    left: { direction: "left", key: "ArrowLeft", code: "ArrowLeft", vk: 37 },
    down: { direction: "down", key: "ArrowDown", code: "ArrowDown", vk: 40 },
    up: { direction: "up", key: "ArrowUp", code: "ArrowUp", vk: 38 },
  };
  const horizontal = dx >= 0 ? [all.right, all.left] : [all.left, all.right];
  const vertical = dy >= 0 ? [all.down, all.up] : [all.up, all.down];
  const cardinals = Math.abs(dx) >= Math.abs(dy)
    ? [horizontal[0], vertical[0], vertical[1], horizontal[1]]
    : [vertical[0], horizontal[0], horizontal[1], vertical[1]];
  const diagonals = [
    [horizontal[0], vertical[0]],
    [horizontal[1], vertical[0]],
    [horizontal[0], vertical[1]],
    [horizontal[1], vertical[1]],
  ];
  return [
    ...diagonals.map((keys) => ({ direction: keys.map((key) => key.direction).join("+"), keys })),
    ...cardinals.map((key) => ({ direction: key.direction, keys: [key] })),
  ];
}

function chebyshev(a, b) {
  if (!a || !b) return Number.POSITIVE_INFINITY;
  return Math.max(Math.abs(Number(a.x) - Number(b.x)), Math.abs(Number(a.y) - Number(b.y)));
}

function normalizeName(value) {
  return String(value ?? "").replace(/[^a-z0-9]/gi, "").toLowerCase();
}

function isRetryableVisibleTransferNavigationError(error) {
  return error instanceof NavigationUnreachableError ||
    /^navigation did not reach\b/i.test(String(error?.message ?? error));
}

function nearestSafeBlockingHostile(state, requestedMonsterName) {
  const playerLevel = Number(state?.playerLevel ?? 0);
  const now = Date.now();
  return nearestBlockingHostile(
    state,
    requestedMonsterName,
    monsterCooldownUntil,
    now,
    (entity) => {
      const profile = grindingCatalog.find(
        (entry) => normalizeName(entry.monsterName) === normalizeName(entity?.name),
      ) ?? null;
      return Boolean(profile) &&
        incidentalTravelThreatIsTrivial(profile.level, playerLevel) &&
        entityAttackIsRecent(entity, now, ACTIVE_TRAVEL_THREAT_WINDOW_MS);
    },
  );
}

function nearestTrivialAdjacentHostile(
  state,
  requestedMonsterName = null,
  cooldownUntil = monsterCooldownUntil,
) {
  const playerLevel = Number(state?.playerLevel ?? 0);
  return nearestBlockingHostile(
    state,
    requestedMonsterName,
    cooldownUntil,
    Date.now(),
    (entity) => {
      const profile = grindingCatalog.find(
        (entry) => normalizeName(entry.monsterName) === normalizeName(entity?.name),
      ) ?? null;
      return (
        Boolean(profile) &&
        incidentalTravelThreatIsTrivial(profile.level, playerLevel)
      ) || completedQuestCertifiesMonster(state, entity?.name);
    },
  );
}

function routeRunCompleted(state) {
  if (!extendedRouteEnabled) return allQ1Q9Completed(state);
  if (!classOnboardingCompleted(state)) return false;
  if (Number(state?.playerLevel ?? 0) < targetLevel) return false;
  const requiredQuests = (authoritativeRoute?.quests ?? []).filter(
    (quest) => quest.questId >= 22 && quest.questId <= maxQuestId,
  );
  return requiredQuests.every(
    (quest) => normalizedQuestStage(questState(state, quest.questId)?.stage) === "completed",
  );
}

function commonQ1Q6Completed(state) {
  return [1, 2, 3, 4, 5, 6].every(
    (questId) => normalizedQuestStage(questState(state, questId)?.stage) === "completed",
  );
}

function classOnboardingBounds() {
  if (className === "Warrior") return { minQuestId: 7, maxQuestId: 9 };
  if (className === "Wizard") return { minQuestId: 10, maxQuestId: 12 };
  if (className === "Taoist") return { minQuestId: 13, maxQuestId: 15 };
  throw new Error(`no classic 1-50 onboarding branch for ${className}`);
}

function classOnboardingCompleted(state) {
  if (!commonQ1Q6Completed(state)) return false;
  const { minQuestId, maxQuestId } = classOnboardingBounds();
  return (authoritativeRoute?.quests ?? [])
    .filter((quest) => quest.questId >= minQuestId && quest.questId <= maxQuestId)
    .every((quest) => normalizedQuestStage(questState(state, quest.questId)?.stage) === "completed");
}

function progressFingerprint(state) {
  return JSON.stringify({
    level: state.playerLevel,
    exp: state.playerExperience,
    hp: state.playerHp,
    quests: state.questLog
      .filter((quest) => quest.questId >= 1 && quest.questId <= (extendedRouteEnabled ? maxQuestId : 9))
      .map((quest) => [quest.questId, quest.stage, quest.current, quest.required, quest.objectives]),
    inventory: state.inventoryItems.map((item) => [item.name, item.quantity]),
    equipment: state.equipmentItems.map((item) => item.name),
    skills: (state.knownSkills ?? []).map((skill) => skill.name),
  });
}

function compactState(state) {
  return {
    capturedAt: state.capturedAt,
    screen: state.screen,
    wsState: state.wsState,
    mapFileName: state.mapFileName,
    playerObjectId: state.playerObjectId,
    playerClass: state.playerClass,
    player: state.player,
    hp: [state.playerHp, state.playerMaxHp],
    mp: [state.playerMp, state.playerMaxMp],
    level: state.playerLevel,
    experience: [state.playerExperience, state.playerMaxExperience],
    gold: state.gold,
    quests: state.questLog.filter(
      (quest) => quest.questId >= 1 && quest.questId <= (extendedRouteEnabled ? maxQuestId : 9),
    ),
    inventory: state.inventoryItems,
    belt: state.beltItems,
    equipment: state.equipmentItems,
    skills: state.knownSkills ?? [],
    groundDrops: state.groundDrops.slice(0, 30),
    selectedObjectId: state.selectedObjectId,
    movementPlan: state.movementPlan,
    mapTransfers: state.mapTransfers,
    nearbyEntities: state.entities
      .filter((entity) => entity.kind === "monster" || entity.kind === "npc")
      .slice(0, 120),
    entityHitTargets: state.entityHitTargets,
    logs: state.logs,
    dialog: state.activeNpcDialog,
    loginFeedback: state.loginFeedback,
  };
}

function compactGoalState(state, goal) {
  if (goal?.kind !== "grind") return compactState(state);
  return {
    capturedAt: state.capturedAt,
    screen: state.screen,
    mapFileName: state.mapFileName,
    position: state.player,
    health: [state.playerHp, state.playerMaxHp],
    mana: [state.playerMp, state.playerMaxMp],
    dead: state.playerDead || state.deathOverlayVisible,
    level: state.playerLevel,
    experience: [state.playerExperience, state.playerMaxExperience],
    gold: state.gold,
    healthPotions: healthPotionQuantity(state),
    equipment: state.equipmentItems,
    skills: (state.knownSkills ?? []).map((skill) => ({
      name: skill.name,
      hotkey: skill.hotkey,
      cooldownRemainingTicks: skill.cooldownRemainingTicks,
    })),
  };
}

function recordMilestone(kind, state, extra = {}) {
  evidence.milestones.push({ kind, at: Date.now(), state: compactState(state), ...extra });
}

async function captureEvidenceFrame(label, state) {
  const fileName = `${String(evidence.milestones.length + evidence.goals.length).padStart(3, "0")}-${slug(label)}.png`;
  const filePath = path.join(framesDir, fileName);
  await client.capture(filePath);
  evidence.milestones.push({
    kind: "screenshot",
    label,
    at: Date.now(),
    file: `frames/${fileName}`,
    state: compactState(state),
  });
}

function assertNoShortcutFrames() {
  const audit = client.outgoingCommandAudit();
  if (audit.violations.length) {
    throw new Error(`shortcut contract violated: ${JSON.stringify(audit.violations)}`);
  }
}

async function finalizeEvidence(fatal, interruptionSignal = null) {
  const finalState = client ? await readAgentState(client).catch(() => null) : null;
  const shortcutAudit = client ? client.outgoingCommandAudit() : { commands: [], violations: [] };
  const commandCounts = {};
  for (const command of shortcutAudit.commands) {
    const key = String(command.type ?? "unknown");
    commandCounts[key] = (commandCounts[key] ?? 0) + 1;
  }
  const packetCounts = {};
  for (const frame of client?.wsReceived ?? []) {
    if (!isGameplayWebSocketUrl(frame.url)) continue;
    try {
      const envelope = JSON.parse(frame.payloadData);
      const key = String(envelope?.packet ?? envelope?.type ?? "unknown");
      packetCounts[key] = (packetCounts[key] ?? 0) + 1;
    } catch {
      packetCounts.nonJson = (packetCounts.nonJson ?? 0) + 1;
    }
  }
  const diagnostics = classifyBrowserDiagnostics(
    client?.consoleErrors ?? [],
    client?.networkFailures ?? [],
  );
  evidence.finishedAt = Date.now();
  evidence.renderGameToText = client ? await renderGameToText(client).catch(() => null) : null;
  evidence.finalState = finalState ? compactState(finalState) : null;
  if (fatal && client && finalState) {
    await captureEvidenceFrame("fatal", finalState).catch(() => {});
  }
  evidence.summary = {
    completed: Boolean(finalState && routeRunCompleted(finalState)),
    fatal,
    interrupted: interruptionSignal != null,
    interruptionSignal,
    runtimeMs: evidence.finishedAt - evidence.startedAt,
    goals: evidence.goals.length,
    goalsOk: evidence.goals.filter((goal) => goal.ok).length,
    kills: evidence.kills.length,
    targetQuarantines: evidence.targetQuarantines.length,
    deaths: evidence.deaths,
    revives: evidence.revives,
    potionUses: evidence.potionUses,
    shopPurchases: evidence.shopPurchases.length,
    lootPickups: evidence.lootPickups.length,
    goldPickups: evidence.goldPickups.length,
    supplyPickups: evidence.supplyPickups.length,
    inputCount: evidence.inputs.length,
    commandCounts,
    packetCounts,
    shortcutAudit,
    consoleErrorCount: client?.consoleErrors?.length ?? 0,
    networkFailureCount: client?.networkFailures?.length ?? 0,
    knownAssetFallbackConsoleErrorCount: diagnostics.knownAssetFallbackConsoleErrors.length,
    knownAssetFallbackNetworkFailureCount: diagnostics.knownAssetFallbackNetworkFailures.length,
    abortedOptionalAssetRequestCount: diagnostics.abortedOptionalAssetRequests.length,
    abortedSupersededSceneRequestCount: diagnostics.abortedSupersededSceneRequests.length,
    criticalConsoleErrorCount: diagnostics.criticalConsoleErrors.length,
    criticalNetworkFailureCount: diagnostics.criticalNetworkFailures.length,
  };

  await fs.writeFile(path.join(outputDir, "summary.json"), JSON.stringify(evidence.summary, null, 2));
  await fs.writeFile(path.join(outputDir, "report.json"), JSON.stringify(evidence, null, 2));
  await fs.writeFile(
    path.join(outputDir, "action-trail.jsonl"),
    evidence.inputs.map((entry) => JSON.stringify(entry)).join("\n") + (evidence.inputs.length ? "\n" : ""),
  );
  await fs.writeFile(
    path.join(outputDir, "browser-diagnostics.json"),
    JSON.stringify({
      consoleErrors: client?.consoleErrors ?? [],
      networkFailures: client?.networkFailures ?? [],
      classification: diagnostics,
      outgoingCommandTypes: shortcutAudit.commands,
      incomingPacketCounts: packetCounts,
    }, null, 2),
  );
  await fs.writeFile(path.join(outputDir, "report.md"), markdownReport(evidence));
  console.log(`\nquest-agent complete=${evidence.summary.completed} goals=${evidence.summary.goalsOk}/${evidence.summary.goals} kills=${evidence.summary.kills} shortcuts=${shortcutAudit.violations.length}`);
  console.log(`quest-agent report=${path.join(outputDir, "report.md")}`);
}

function markdownReport(report) {
  const lines = [
    `# Autonomous real-client quest report — ${report.runId}`,
    "",
    `- Route: \`${report.route}\``,
    `- Target: \`${report.baseUrl}\``,
    `- Account/character: \`${report.account}\` / \`${report.characterName}\``,
    `- Completed route \`${report.route}\`: **${report.summary.completed}**`,
    `- Runtime: ${Math.round(report.summary.runtimeMs / 1000)}s; goals ${report.summary.goalsOk}/${report.summary.goals}; confirmed kills ${report.summary.kills}; target quarantines ${report.summary.targetQuarantines}`,
    `- Recovery: deaths ${report.summary.deaths}, revives ${report.summary.revives}, potion uses ${report.summary.potionUses}, visible NPC purchases ${report.summary.shopPurchases}`,
    `- Loot: quest items ${report.summary.lootPickups}, nearby gold pickups ${report.summary.goldPickups}, supply pickups ${report.summary.supplyPickups}`,
    `- No-shortcut audit: ${report.summary.shortcutAudit.violations.length === 0 ? "PASS" : "FAIL"}`,
    `- Browser diagnostics: critical console ${report.summary.criticalConsoleErrorCount}, critical network ${report.summary.criticalNetworkFailureCount}; raw console ${report.summary.consoleErrorCount}, raw network ${report.summary.networkFailureCount}`,
    `- Known optional asset fallbacks: console ${report.summary.knownAssetFallbackConsoleErrorCount}, network ${report.summary.knownAssetFallbackNetworkFailureCount}, aborted assets ${report.summary.abortedOptionalAssetRequestCount}; superseded scene requests ${report.summary.abortedSupersededSceneRequestCount}`,
    "",
    "## Quest milestones",
    "",
  ];
  for (const milestone of report.milestones.filter((entry) => entry.kind !== "screenshot")) {
    const questSummary = milestone.state?.quests?.map((quest) => `q${quest.questId}:${quest.stage}`).join(", ") ?? "";
    lines.push(`- ${new Date(milestone.at).toISOString()} — ${milestone.kind}${questSummary ? ` — ${questSummary}` : ""}`);
  }
  lines.push("", "## Real input evidence", "");
  lines.push(`- Mouse/keyboard/text events: ${report.inputs.length}`);
  lines.push(`- Outgoing command types: \`${JSON.stringify(report.summary.commandCounts)}\``);
  lines.push("- Raw outgoing frames are intentionally not written because login frames can contain credentials.");
  lines.push("", "## Frames", "");
  for (const frame of report.milestones.filter((entry) => entry.kind === "screenshot")) {
    lines.push(`- [${frame.label}](${frame.file})`);
  }
  if (report.summary.fatal) lines.push("", "## Fatal", "", "```text", report.summary.fatal, "```");
  return `${lines.join("\n")}\n`;
}

function describeGoal(goal) {
  if (goal.kind === "talk") return `${goal.action} q${goal.questId} via ${goal.npc?.label ?? goal.npcKey}`;
  if (goal.kind === "quest-diary") return `${goal.action} q${goal.questId} via visible Quest Diary`;
  if (goal.kind === "hunt") return `q${goal.questId}: hunt ${goal.monsterName}${goal.harvest ? " + harvest" : ""}`;
  if (goal.kind === "grind") return `grind ${goal.monsterName} toward level ${goal.targetLevel}`;
  if (goal.kind === "wait") return `q${goal.questId}: ${goal.reason}`;
  return goal.kind;
}

function sanitizeInput(input) {
  if (input.secret) return { ...input, value: undefined, secret: true };
  return input;
}

function buildRunUrl(url, wsUrl, omitGpuRuntime) {
  const parsed = new URL(url);
  if (wsUrl) parsed.searchParams.set("gatewayWs", wsUrl);
  if (omitGpuRuntime) parsed.searchParams.set("skipRuntime", "1");
  return parsed.toString();
}

function redactGatewayUrl(value) {
  const parsed = new URL(value);
  parsed.username = "";
  parsed.password = "";
  parsed.search = "";
  parsed.hash = "";
  return parsed.toString();
}

function parseArgs(argv) {
  const parsed = {};
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (!arg.startsWith("--")) continue;
    const key = arg.slice(2);
    const next = argv[index + 1];
    if (!next || next.startsWith("--")) parsed[key] = "true";
    else {
      parsed[key] = next;
      index += 1;
    }
  }
  return parsed;
}

function numberArg(value, fallback) {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : fallback;
}

function boolArg(value, fallback) {
  if (value === undefined || value === null || value === "") return fallback;
  if (typeof value === "boolean") return value;
  return ["1", "true", "yes", "on"].includes(String(value).toLowerCase());
}

function defaultIdentity(prefix) {
  return `${prefix}${process.pid.toString(36)}${Date.now().toString(36)}`.replace(/[^a-z0-9]/gi, "").slice(0, 12).toUpperCase();
}

function slug(value) {
  return String(value).toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "").slice(0, 60);
}

restoreAdaptiveCombatMemory(resumeEvidence);
restoreGrindingSourceStallMemory(resumeEvidence);

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
