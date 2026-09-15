import { hasAuthoritativePlayerDeath, observedPlayerHp } from './protocol-observation.mjs';
import { refreshCombatWorldSnapshot } from './protocol-refresh.mjs';
import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import { ProtocolClient, delay, startGameBootstrapEvidence } from './protocol-client.mjs';
import { createNavigator, selfPlayer, equipStarterGear, reviveInTown, collectNearbyGold } from './protocol-play.mjs';
import { interactQuest } from './protocol-quest-actions.mjs';
import { createMapTraveler } from './protocol-travel.mjs';
import { travelToQuestNpc } from './protocol-quest-travel.mjs';
import { clearTravelBlockingMonster, criticalProvenAggressorOffenseGuardActive } from './protocol-combat.mjs';
import { expectedItemRewards, verifyItemRewards } from './protocol-rewards.mjs';
import { prepareLoadout, combatAction, combatApproachRange, startEmergencyHpRecovery, useClassRecovery, useSupplies } from './protocol-loadout.mjs';
import {
  restockInVillage,
  createRandomTeleportEmergencyEscape,
  equipHeldAmulet,
  randomTeleportCount,
  townTeleportCount,
  useTownTeleport,
  useRandomTeleport,
  journeyWeaponFundingGold,
  hpMediumDrugCount,
} from './protocol-supplies.mjs';
import { loadObservedMonsterLocations } from './protocol-memory.mjs';
import { claimAvailableMilestones, JOURNEY_MILESTONES } from './protocol-milestones.mjs';
import {
  amuletStock,
  canFinishNearCompleteKillQuestWithoutHpStock,
  canFinishNearCompleteKillQuestWithReducedHpStock,
  canResumeStockedCombatExpedition,
  createPostEngagementSupplyGate,
  evasiveRecoveryDangerDistanceForQuest,
  evasiveRecoveryTimeoutMsForQuest,
  hpRestockTargetForActiveQuests,
  hasJourneyEmergencyEscapeQuest,
  hpDrugCount,
  isLivingEvasiveRecoveryTimeout,
  isLivingExpeditionNoWalkPath,
  isLivingLostCombatTarget,
  isLivingUnsafePackRetreatFailure,
  journeyExpeditionDepartureFloorForQuest,
  journeyEmergencyEscapeRestockTarget,
  journeyEmergencyTownTeleportRestockTarget,
  journeyEmergencyTeleportCriticalHpRatio,
  journeyEmergencyTeleportDepartureTarget,
  journeyExpeditionSupplyActive,
  journeyNavigationEmergencyEscapeBudget,
  journeyMpRestockTargetForQuest,
  journeyAmuletSupplyPolicyForQuest,
  minimumJourneyMpStockForQuest,
  mpDrugCount,
  journeyResumeDisposition,
  questRetreatBiasPosition,
  questRetreatProfile,
  q89WizardFreshCursedShamanRecoveryOptions,
  questSpawnSearchHostileClearanceFallback,
  questCombatMpUseThresholdForQuest,
  questEmergencyEscapeHpRatio,
  questPostRetreatRecoveryRatio,
  questNeedsPostRetreatRecovery,
  preferredObjectiveMapsForQuest,
  requiresTaoistAmuletRestock,
  requiresExpeditionEscapeRestock,
  shouldPreferObjectiveMapOverCurrent,
  recoverHealthWhileEvading,
  safeZoneBlocksEmergencyEscape,
  waitForPassiveHealthRecovery,
} from './protocol-survival.mjs';
import { createWizardKitingAction } from './protocol-kiting.mjs';
import { questCombatAction, questCombatApproachRange } from './protocol-quest-combat-policy.mjs';
import {
  canUseSafeDeerFunding,
  collectSafeFundingVenison,
  finishReadySupplyFundingQuest,
  safeFundingVenisonTargetCount,
} from './protocol-funding.mjs';
import {
  cofarmFollowers,
  prioritizeNearCompleteActiveQuests,
  shouldDeferCofarmQuest,
} from './protocol-quest-scheduling.mjs';

const output = path.resolve(process.env.MIR2_JOURNEY_OUTPUT ?? 'output/protocol-journey');
const maxAttackAttempts = 120;
const maxRetreatSteps = 3;
const bootstrapTimeoutMs = 60_000;
// Eight bottles remains the normal restock trigger. Four bottles is the
// emergency floor that lets a cash-poor character finish the nearest reward
// quest instead of becoming permanently stuck in town.
const minimumJourneyHpStock = 4;
const maxJourneyRevivals = Number(process.env.MIR2_JOURNEY_MAX_REVIVALS ?? 30);
const dangerousExpeditionQuestIds = new Set([54, 60, 62, 65, 89, 98, 99, 113, 114]);
const journeySupplyOptions = Object.freeze({
  targetHp: 24,
  targetMp: 12,
  targetAmulet: 12,
  lowStock: minimumJourneyHpStock,
  // Survival stock is the working capital for a normal low-level character.
  // Keeping an arbitrary wallet reserve while the character cannot safely
  // leave town only creates a death/restock loop.
  reserveGold: 0,
});
await fs.mkdir(output, { recursive: true });
const className = process.env.MIR2_JOURNEY_CLASS ?? 'Warrior';
if (!['Warrior', 'Wizard', 'Taoist'].includes(className)) throw new Error('Invalid class');
await fs.writeFile(path.join(output, `${className}.runner.pid`), String(process.pid));
const credentialPath = path.join(output, `${className}.private.json`);
let credentials;
try { credentials = JSON.parse(await fs.readFile(credentialPath, 'utf8')); }
catch (error) {
  if (error.code !== 'ENOENT') throw error;
  const suffix = crypto.randomBytes(4).toString('hex');
  credentials = { accountId: `jp${suffix}`, password: crypto.randomBytes(8).toString('hex'), name: `J${className[0]}${suffix}`, className };
  await fs.writeFile(credentialPath, JSON.stringify(credentials), { mode: 0o600, flag: 'wx' });
}
const runId = new Date().toISOString().replace(/[:.]/g, '-');
const traceFile = path.join(output, `${className}.${runId}.trace.jsonl`);
const client = new ProtocolClient(
  process.env.MIR2_GATEWAY_WS_URL ?? 'ws://127.0.0.1:17710/ws',
  traceFile,
);
const report = { runId, className, name: credentials.name, startedAt: new Date().toISOString(), gatewayUrl: client.url, traceFile, transport: 'normal-local-websocket', visualAccepted: false, completed: false };
try {
  // Each run writes an isolated trace so Windows readers and virus scanners
  // cannot lock a shared multi-gigabyte append target. Bootstrap memory reads
  // only the bounded recent tail of the legacy per-class trace.
  client.observedMonsterLocations = await loadObservedMonsterLocations(
    path.join(output, `${className}.trace.jsonl`),
  );
  await client.connect();
  if (!credentials.registered) {
    const result = await client.request({ type: 'newAccount', ...credentials }, 'NewAccount');
    report.registration = result.payload;
    if (![7, 8].includes(result.payload.result)) throw new Error(`Registration rejected: ${result.payload.result}`);
    credentials.registered = true;
    await fs.writeFile(credentialPath, JSON.stringify(credentials), { mode: 0o600 });
    await delay(200);
  }
  const login = await client.request({ type: 'login', accountId: credentials.accountId, password: credentials.password }, 'LoginSuccess');
  let character = login.payload.characters.find(c => c.name === credentials.name);
  if (!character) {
    const created = await client.request({ type: 'newCharacter', name: credentials.name, class: className, gender: 'Male' }, 'NewCharacterSuccess');
    character = created.payload.character;
  }
  report.characterIndex = character.index;
  const startGameAfter = client.sequence;
  client.send({ type: 'startGame', characterIndex: character.index });
  const bootstrapEvidence = await client.wait(
    () => startGameBootstrapEvidence(client, startGameAfter, credentials.name),
    'UserInformation or personal snapshot',
    bootstrapTimeoutMs,
  );
  report.bootstrapSignal = bootstrapEvidence.source;
  await client.wait(() => client.snapshot?.entities?.some(e => e.objectId === client.snapshot.playerObjectId && e.name === credentials.name), 'personal snapshot', bootstrapTimeoutMs);
  report.bootstrapPassed = true;
  if (process.env.MIR2_JOURNEY_PLAY === '1') {
    const route = JSON.parse(await fs.readFile(new URL(`../../../../docs/generated/quest-agent/${className.toLowerCase()}-1-30-newcomer-v1.json`, import.meta.url), 'utf8'));
    const activeWoomaQuest = owner => {
      const activeIds = (owner?.snapshot?.questLog ?? [])
        .filter(entry => [98, 99].includes(Number(entry?.questId)) &&
          ['inprogress', 'readytoturnin'].includes(
            String(entry?.stage ?? '').replace(/[^a-z]/gi, '').toLowerCase(),
          ))
        .map(entry => Number(entry.questId));
      return activeIds
        .map(questId => route.quests.find(quest => Number(quest?.questId) === questId))
        .find(Boolean) ?? null;
    };
    const isWizardQ89Expedition = snapshot => className === 'Wizard' &&
      (snapshot?.questLog ?? []).some(entry => Number(entry?.questId) === 89 &&
        String(entry?.stage ?? '').replace(/[^a-z]/gi, '').toLowerCase() === 'inprogress');
    const q89WizardPotionOptions = owner => isWizardQ89Expedition(owner?.snapshot)
      ? { preferredHpPotion: 'medium', restorativeReuseDelayMs: 2_500 }
      : {};
    const useJourneySupplies = (owner, options = {}) => useSupplies(owner, {
      ...options,
      ...q89WizardPotionOptions(owner),
    });
    const startJourneyEmergencyHpRecovery = (owner, options = {}) => startEmergencyHpRecovery(owner, {
      ...options,
      ...q89WizardPotionOptions(owner),
    });
    const recoverFromFreshQ89CursedShamanHit = (owner, { attacker }) => {
      const recoveryOptions = q89WizardFreshCursedShamanRecoveryOptions(
        owner?.snapshot,
        attacker,
        className,
      );
      if (!recoveryOptions) return [];
      return startJourneyEmergencyHpRecovery(owner, recoveryOptions);
    };
    const emergencyTeleport = createRandomTeleportEmergencyEscape({
      criticalHpRatio: journeyEmergencyTeleportCriticalHpRatio(className),
      teleport: async owner => {
        const maxHp = Math.max(1, Number(owner?.snapshot?.playerMaxHp ?? 0));
        const hpRatio = observedPlayerHp(owner?.snapshot) / maxHp;
        if (isWizardQ89Expedition(owner?.snapshot) &&
            hpRatio <= journeyEmergencyTeleportCriticalHpRatio(className) &&
            townTeleportCount(owner.snapshot) > 0) {
          try {
            return await useTownTeleport(owner);
          } catch {
            // A failed authoritative TownTeleport proof falls through to the
            // same factory-managed RandomTeleport attempt.
          }
        }
        return useRandomTeleport(owner);
      },
    });
    const journeyCombatAction = (owner, target) => {
      const quest = activeWoomaQuest(owner);
      return quest ? questCombatAction(owner, quest, target) : combatAction(owner, target);
    };
    const journeyCombatApproachRange = (owner, target) => {
      const quest = activeWoomaQuest(owner);
      return quest ? questCombatApproachRange(owner, quest, target) : combatApproachRange(owner, target);
    };
    const emergencyEscapeForJourney = async owner => {
      if (safeZoneBlocksEmergencyEscape(owner)) return false;
      if (isWizardQ89Expedition(owner?.snapshot) &&
          townTeleportCount(owner.snapshot) <= 0 &&
          randomTeleportCount(owner.snapshot) <= 0) return false;
      if (!isWizardQ89Expedition(owner?.snapshot) && randomTeleportCount(owner.snapshot) <= 0) return false;
      const result = await emergencyTeleport(owner);
      if (result?.deferred === true || result === false || result === true) return result;
      return result && typeof result === 'object'
        ? { ...result, success: true }
        : result;
    };
    const rawNavigate = createNavigator(client, {
      emergencyEscape: async owner => {
        // The cave remains dangerous after the last objective dies. Preserve
        // the same ordinary RandomTeleport escape until the reward NPC has
        // actually accepted the ready quest.
        const hasEmergencyExpedition = hasJourneyEmergencyEscapeQuest(owner.snapshot);
        if (!hasEmergencyExpedition) return false;
        return emergencyEscapeForJourney(owner);
      },
      emergencyEscapeHpRatio: className === 'Wizard' ? 0.65 : 0.35,
      emergencyEscapeDangerDistance: 6,
      maxEmergencyEscapesPerNavigation: journeyNavigationEmergencyEscapeBudget(className),
    });
    const navigate = (target, desiredDistance = 1, stopWhen = () => false, options = {}) => {
      const retreatProfile = isWizardQ89Expedition(client.snapshot)
        ? questRetreatProfile(89, className)
        : null;
      return rawNavigate(target, desiredDistance, stopWhen, {
        ...options,
        ...(retreatProfile?.emergencyEscapeRequiresProvenAggressors === true
          ? { emergencyEscapeRequiresProvenAggressors: true }
          : {}),
        ...(isWizardQ89Expedition(client.snapshot)
          ? { onFreshDirectHit: recoverFromFreshQ89CursedShamanHit }
          : {}),
      });
    };
    report.revivals = [];
    let revival = await reviveInTown(client);
    if (!revival && criticalStrandedPlayer(client.snapshot)) {
      try {
        await client.wait(() => playerIsDead(client.snapshot), 'critical stranded player death', 30_000);
      } catch (error) {
        if (!String(error?.message ?? '').startsWith('Timeout waiting for')) throw error;
      }
      if (playerIsDead(client.snapshot)) revival = await reviveInTown(client);
    }
    if (revival) report.revivals.push(revival);
    await equipStarterGear(client);
    report.loadout = await prepareLoadout(client);
    const travel = createMapTraveler(client, navigate, {
      // RandomTeleport can place a character in a disconnected component of a
      // reused Crystal map image. If static collision proves that the selected
      // authoritative exit is unreachable, spend one ordinary scroll and
      // retry from the new authoritative position.
      relocateDisconnectedRegion: owner => randomTeleportCount(owner.snapshot) > 0
        ? useRandomTeleport(owner)
        : false,
      resolveBlockingMonster: (owner, blocker) => clearTravelBlockingMonster(
        owner,
        blocker,
        navigate,
        {
          action: journeyCombatAction,
          approachRange: journeyCombatApproachRange,
          sustain: owner => useJourneySupplies(owner),
          attackCadenceMs: 650,
          // Shared-zone cooldown counters advance authoritatively, but ordinary
          // movement traffic does not include a fresh personal skill snapshot.
          // A ranged blocker can therefore remain at cooldown=1 forever unless
          // this travel-only combat path requests the same refresh as quests.
          refreshWhileWaiting: current => refreshCombatCooldown(current),
          combatHostileClearance: 0,
          combatHostileClearanceFallback: 0,
        },
      ),
    });
    const profile = JSON.parse(await fs.readFile(new URL('../../../../config/quest-guidance/newcomer-journey-v1.json', import.meta.url), 'utf8'));
    const routeIds = profile.chapters.flatMap(c => [...c.questIds, ...(c.classQuestIds?.[className] ?? [])]);
    const ids = prioritizeNearCompleteActiveQuests(routeIds, client.snapshot?.questLog);
    const progressionCandidates = route.quests.flatMap(quest =>
      [...(quest.rewards?.fixedItems ?? []), ...(quest.rewards?.selectableItems ?? [])]
        .map(item => ({ name: item.itemName, questId: quest.questId })));
    const journeyRestock = (owner, navigateNear, requestedSupplyOptions = {}) => {
      const protectedItemNames = route.quests
        .filter(quest => {
          const stage = String(owner.snapshot?.questLog?.find(entry =>
            Number(entry?.questId) === Number(quest.questId))?.stage ?? '').toLowerCase();
          return stage === 'inprogress' || stage === 'readytoturnin';
        })
        .flatMap(quest => quest.objectives?.item?.map(item => item.itemName) ?? []);
      const activeQuestHpTarget = hpRestockTargetForActiveQuests(owner.snapshot, {
        fallback: journeySupplyOptions.targetHp,
        targets: { 42: 32, 49: 24, 54: 80, 60: 80, 62: 80, 65: 80, 98: 80, 99: 80, 113: 80, 114: 80 },
      });
      return restockInVillage(owner, navigateNear, {
        ...journeySupplyOptions,
        ...requestedSupplyOptions,
        // q54/q62 repeatedly proved that a dense one-tile cave corridor can
        // occupy every legal first step. Carry the ordinary Ruben shop scroll
        // a human player uses for that exact emergency; other quests keep the
        // existing supply plan unchanged.
        emergencyTeleportCount: Math.max(0, ...(owner.snapshot?.questLog ?? [])
          .filter(quest => String(quest?.stage ?? '').replace(/[^a-z]/gi, '').toLowerCase() === 'inprogress')
          .map(quest => journeyEmergencyTeleportDepartureTarget(quest?.questId))),
        // q89 Wizard keeps two TownTeleport scrolls as the first critical
        // escape reserve. Other classes and quests preserve the ordinary
        // zero-default supply plan.
        emergencyTownTeleportCount: isWizardQ89Expedition(owner.snapshot) &&
          (String(owner.snapshot?.mapFileName ?? '') === '0' ||
            Number(requestedSupplyOptions.emergencyTownTeleportCount ?? 0) > 0)
          ? 2
          : 0,
        // The first D421 -> D422 round trip consumed 24 bottles before the
        // objective map was reached. Carry an evidence-based expedition
        // stock while q54 remains active instead of repeating town loops.
        targetHp: Math.max(activeQuestHpTarget, Number(requestedSupplyOptions.targetHp ?? 0)),
        ...(isWizardQ89Expedition(owner.snapshot) ? {
          targetHpMedium: Math.max(6, Number(requestedSupplyOptions.targetHpMedium ?? 0)),
          lowStockHpMedium: Math.max(3, Number(requestedSupplyOptions.lowStockHpMedium ?? 0)),
        } : {}),
        targetMp: Math.max(journeySupplyOptions.targetMp, Number(requestedSupplyOptions.targetMp ?? 0)),
        lowStock: Math.max(journeySupplyOptions.lowStock, Number(requestedSupplyOptions.lowStock ?? 0)),
        lowStockHp: Math.max(
          journeySupplyOptions.lowStock,
          Number(requestedSupplyOptions.lowStockHp ?? requestedSupplyOptions.lowStock ?? 0),
        ),
        lowStockMp: Math.max(
          journeySupplyOptions.lowStock,
          Number(requestedSupplyOptions.lowStockMp ?? requestedSupplyOptions.lowStock ?? 0),
        ),
        liquidateSuperseded: true,
        liquidateObsoleteMaterials: true,
        ensureClassWeapon: true,
        clearBlockingMonster: (current, blocker) => clearTravelBlockingMonster(
          current,
          blocker,
          navigate,
          {
            action: journeyCombatAction,
            approachRange: journeyCombatApproachRange,
            sustain: owner => useJourneySupplies(owner),
            attackCadenceMs: 650,
            refreshWhileWaiting: current => refreshCombatCooldown(current),
            combatHostileClearance: 0,
            combatHostileClearanceFallback: 0,
            maxMovingTargetNoProgressMs: 20_000,
          },
        ),
        progressionCandidates,
        protectedItemNames,
      });
    };
    const supplyGate = createPostEngagementSupplyGate({
      travel,
      navigateNear: navigate,
      restock: journeyRestock,
      minimumHpStock: minimumJourneyHpStock,
      minimumMpStock: minimumJourneyMpStockForQuest(null, className),
    });
    const refreshCombatCooldown = refreshCombatWorldSnapshot;
    const supplyGateCallOptions = (owner, questId, requiredHpStock, {
      replenishEscapeReserve = false,
      forceRestock = false,
    } = {}) => {
      const expeditionSupplyActive = journeyExpeditionSupplyActive(owner.snapshot, questId);
      const departureFloor = journeyExpeditionDepartureFloorForQuest(questId, className);
      const taoistAmuletExpedition =
        String(className).trim().toLowerCase() === 'taoist' &&
        expeditionSupplyActive &&
        (owner.snapshot?.knownSkills ?? []).some(skill => String(skill?.spell ?? '') === 'SoulFireBall');
      // R68 exhausted the 12-Amulet village target during one D2041 pack,
      // fell back to melee, and died at q60 6/8. Keep a lower field trigger
      // than the full departure target so a partly used stack can finish an
      // engagement without sending every cast back to town.
      const amuletPolicy = journeyAmuletSupplyPolicyForQuest(questId, className, owner.snapshot);
      const amuletTrigger = taoistAmuletExpedition ? amuletPolicy.minimum : 0;
      const amuletDepartureTarget = taoistAmuletExpedition ? amuletPolicy.departure : 0;
      const emergencyTeleportTarget = expeditionSupplyActive
        ? journeyEmergencyTeleportDepartureTarget(questId)
        : 0;
      const emergencyTeleportRestockTarget = journeyEmergencyEscapeRestockTarget(
        owner.snapshot,
        questId,
        { force: replenishEscapeReserve, target: emergencyTeleportTarget },
      );
      const shouldReplenishEscapeReserve = emergencyTeleportRestockTarget > 0;
      const emergencyTownTeleportTarget = journeyEmergencyTownTeleportRestockTarget(
        owner.snapshot,
        questId,
        className,
        { force: replenishEscapeReserve, target: 2 },
      );
      const shouldReplenishTownEscapeReserve = emergencyTownTeleportTarget > 0 &&
        townTeleportCount(owner.snapshot) < emergencyTownTeleportTarget;
      const q42WizardExpedition = Number(questId) === 42 &&
        String(className).trim().toLowerCase() === 'wizard';
      const q89WizardExpedition = isWizardQ89Expedition(owner.snapshot);
      const q89TownDeparture = q89WizardExpedition &&
        String(owner.snapshot?.mapFileName ?? '') === '0';
      return {
        minimumHpStock: requiredHpStock,
        // Medium HP is a q89 departure reserve only. Once the character is
        // back in the field, missing/unaffordable Medium must fall back to
        // held Small HP and the normal retreat policy rather than becoming a
        // standalone town-loop trigger.
        ...(q89TownDeparture ? { minimumHpMediumStock: 3, requiredAfterRestockHpMediumStock: 6 } : {}),
        minimumMpStock: minimumJourneyMpStockForQuest(questId, className),
        minimumAmuletStock: amuletTrigger,
        requiredAfterRestockAmuletStock: amuletDepartureTarget,
        // Four scrolls are a departure target, not a field invariant. A
        // successful emergency escape must not make a healthy expedition turn
        // around solely to replace that one scroll.
        minimumEmergencyTeleportStock: emergencyTeleportRestockTarget,
        requiredAfterRestockEmergencyTeleportStock: emergencyTeleportTarget,
        minimumEmergencyTownTeleportStock: emergencyTownTeleportTarget,
        requiredAfterRestockEmergencyTownTeleportStock: emergencyTownTeleportTarget,
        forceRestock: forceRestock || journeyWeaponFundingGold(owner.snapshot) > 0 ||
          requiresTaoistAmuletRestock(owner.snapshot, questId, className, amuletTrigger) ||
          (shouldReplenishEscapeReserve && randomTeleportCount(owner.snapshot) < emergencyTeleportTarget) ||
          shouldReplenishTownEscapeReserve ||
          (expeditionSupplyActive &&
            (hpDrugCount(owner.snapshot) < departureFloor.hp ||
              mpDrugCount(owner.snapshot) < departureFloor.mp ||
              (taoistAmuletExpedition &&
                amuletStock(owner.snapshot) < amuletDepartureTarget)) &&
            String(owner.snapshot?.mapFileName ?? '') === '0'),
        ...(expeditionSupplyActive ? {
          requiredAfterRestockHpStock: departureFloor.hp,
          ...(departureFloor.mp > 0 ? { requiredAfterRestockMpStock: departureFloor.mp } : {}),
          // Stay in town after the shop. The quest loop owns the return trip
          // because it carries the class-aware aggressor interrupt/retreat
          // policy; the generic supply traveler does not.
          returnMapFileName: '',
        } : {}),
        ...(q42WizardExpedition ? { requiredAfterRestockMpStock: 32 } : {}),
      };
    };
    const recoverEmergencyFunding = async (
      owner,
      requiredHpStock = minimumJourneyHpStock,
      questId = null,
    ) => {
      try {
        if (livingHealthRatio(owner.snapshot) < 0.9) {
          await waitForPassiveHealthRecovery(owner, {
            requiredRatio: 0.9,
            timeoutMs: 120_000,
          });
        }
        // Fund the active expedition target, not only the generic village
        // target. q54 needs enough stock for both mine transfers and combat;
        // funding only 24 bottles caused repeated under-stocked returns.
        const activeQuestHpTarget = hpRestockTargetForActiveQuests(owner.snapshot, {
          fallback: journeySupplyOptions.targetHp,
          targets: { 42: 32, 49: 24, 54: 80, 60: 80, 62: 80, 65: 80, 89: 64, 98: 80, 99: 80, 113: 80, 114: 80 },
        });
        const fundingHpDeficit = Math.max(
          1,
          Math.max(requiredHpStock, activeQuestHpTarget) - hpDrugCount(owner.snapshot),
        );
        const activeQuestMpTarget = journeyMpRestockTargetForQuest(
          questId,
          selfPlayer(owner)?.class,
          { fallback: journeySupplyOptions.targetMp },
        );
        const fundingMpDeficit = Math.max(
          0,
          activeQuestMpTarget - mpDrugCount(owner.snapshot),
        );
        const emergencyTeleportTarget = journeyEmergencyTeleportDepartureTarget(questId);
        const emergencyTeleportFunding = Math.max(
          0,
          emergencyTeleportTarget - randomTeleportCount(owner.snapshot),
        ) * 100;
        const emergencyTownTeleportTarget = journeyEmergencyTownTeleportRestockTarget(
          owner.snapshot,
          questId,
          className,
          { force: true, target: 2 },
        );
        const emergencyTownTeleportFunding = Math.max(
          0,
          emergencyTownTeleportTarget - townTeleportCount(owner.snapshot),
        ) * 1000;
        const fundingAmuletDeficit = String(selfPlayer(owner)?.class ?? '').trim().toLowerCase() === 'taoist' &&
          journeyExpeditionSupplyActive(owner.snapshot, questId)
          ? Math.max(0, journeyAmuletSupplyPolicyForQuest(questId, selfPlayer(owner)?.class).departure - amuletStock(owner.snapshot))
          : 0;
        const requiredFundingCount = safeFundingVenisonTargetCount(fundingHpDeficit, {
          className: selfPlayer(owner)?.class,
          requiredMpStock: fundingMpDeficit,
          requiredHpMediumStock: Number(questId) === 89 && className === 'Wizard'
            ? Math.max(0, 6 - hpMediumDrugCount(owner.snapshot))
            : 0,
          requiredAmuletStock: fundingAmuletDeficit,
          additionalGold: Math.max(
            0,
            journeyWeaponFundingGold(owner.snapshot) + emergencyTeleportFunding +
              emergencyTownTeleportFunding -
              Number(owner.snapshot?.gold ?? 0),
          ),
        });
        const funding = await collectSafeFundingVenison(owner, {
          route,
          travel,
          navigate,
          action: journeyCombatAction,
          approachRange: journeyCombatApproachRange,
          sustain: async current => {
            const classRecovery = await useClassRecovery(current, { hpThreshold: 0.9 });
            const consumed = startJourneyEmergencyHpRecovery(current, { hpThreshold: 0.9 });
            return { classRecovery, consumed };
          },
          refreshWhileWaiting: refreshCombatCooldown,
          requiredCount: requiredFundingCount,
          maxHunts: Math.max(8, requiredFundingCount + 4),
        });
        // Funding ends in the nearby field. Force the follow-up shop pass even
        // when a partial Amulet stack is above the lower field trigger; the
        // journey must actually sell the Venison and reach the quest's Amulet
        // departure reserve before walking back to the cave.
        const restocked = await supplyGate(owner, supplyGateCallOptions(
          owner,
          questId,
          requiredHpStock,
          { forceRestock: true },
        ));
        return { ...restocked, funding };
      } catch (error) {
        if (playerIsDead(owner.snapshot) && report.revivals.length < maxJourneyRevivals) {
          // Bootstrap/restock recovery runs before the per-quest retry block.
          // A death here still belongs to the active journey and must revive,
          // re-evaluate funding, and return through the normal public protocol.
          const revival = await reviveInTown(owner);
          if (!revival) throw error;
          report.revivals.push(revival);
          return recoverEmergencyFunding(owner, requiredHpStock, questId);
        }
        throw error;
      }
    };
    const supplyGateForQuest = async (owner, questId = null, options = {}) => {
      const q42ReducedStockFinish = Number(questId) === 42 &&
        canFinishNearCompleteKillQuestWithReducedHpStock(owner.snapshot, questId);
      const completedExpeditionObjective = dangerousExpeditionQuestIds.has(Number(questId)) &&
        !journeyExpeditionSupplyActive(owner.snapshot, questId);
      const requiredHpStock = q42ReducedStockFinish || completedExpeditionObjective
        ? 1
        : minimumJourneyHpStockForQuest(questId);
      let stock = hpDrugCount(owner.snapshot);
      let gold = Number(owner.snapshot?.gold ?? 0);
      if (stock < minimumJourneyHpStock && gold < 40 &&
          String(owner.snapshot?.mapFileName ?? '') === '0' &&
          activeEmergencyFundingQuest(owner, questId) &&
          livingHealthRatio(owner.snapshot) < 0.7) {
        // A no-cash character revived in the safe village can regenerate
        // normally, then return to the nearby funding quest. Sending it into
        // the shop first creates an unrecoverable needsFunds loop.
        await waitForPassiveHealthRecovery(owner, {
          requiredRatio: 0.75,
          timeoutMs: 120_000,
        });
        stock = hpDrugCount(owner.snapshot);
        gold = Number(owner.snapshot?.gold ?? 0);
      }
      if (stock < minimumJourneyHpStock && gold < 40 &&
          (emergencyFundingQuestState(owner, questId) ||
            canUseSafeDeerFunding(owner.snapshot))) {
        return recoverEmergencyFunding(owner, requiredHpStock, questId);
      }
      try {
        return await supplyGate(owner, supplyGateCallOptions(owner, questId, requiredHpStock, options));
      } catch (error) {
        if (playerIsDead(owner.snapshot) && report.revivals.length < maxJourneyRevivals) {
          // A resumed cave expedition may have enough restorative to move but
          // still be sealed from the town exit by a whole corridor pack. The
          // ordinary supply trip can end in an authoritative death before the
          // per-quest retry block starts. Revive through the normal protocol,
          // then re-evaluate funding and stock from the safe village state.
          const revival = await reviveInTown(owner);
          if (!revival) throw error;
          report.revivals.push(revival);
          return supplyGateForQuest(owner, questId);
        }
        if (canAttemptEmergencyFundingQuest(owner, questId, error)) {
          return recoverEmergencyFunding(owner, requiredHpStock, questId);
        }
        if (String(error?.message ?? '').startsWith('needsFunds:') &&
            String(owner.snapshot?.mapFileName ?? '') === '0' &&
            livingHealthRatio(owner.snapshot) > 0) {
          // supplyGate may have travelled here from a dangerous map before it
          // discovered the empty purse. Town is safe for passive recovery;
          // once healed, the ordinary Deer/Venison loop can fund any active
          // journey quest rather than only the special q30 recovery case.
          return recoverEmergencyFunding(owner, requiredHpStock, questId);
        }
        if (String(error?.message ?? '').startsWith('needsFunds:') &&
            canUseSafeDeerFunding(owner.snapshot)) {
          return recoverEmergencyFunding(owner, requiredHpStock, questId);
        }
        if (String(error?.message ?? '').startsWith('needsFunds:') &&
            String(owner.snapshot?.mapFileName ?? '') === '0' &&
            canFinishNearCompleteKillQuestWithoutHpStock(owner.snapshot, questId, {
              minimumHealthRatio: 0,
            })) {
          if (livingHealthRatio(owner.snapshot) < 0.9) {
            await waitForPassiveHealthRecovery(owner, {
              requiredRatio: 0.9,
              timeoutMs: 120_000,
            });
          }
          if (canFinishNearCompleteKillQuestWithoutHpStock(owner.snapshot, questId)) {
            return {
              status: 'nearCompleteNoStock',
              questId,
              stock: 0,
              gold: Number(owner.snapshot.gold),
            };
          }
        }
        throw error;
      }
    };
    const activeResumeExpeditionId = ids.find(id => {
      if (!dangerousExpeditionQuestIds.has(Number(id))) return false;
      const quest = client.snapshot?.questLog?.find(entry => Number(entry?.questId) === Number(id));
      return String(quest?.stage ?? '').replace(/[^a-z]/gi, '').toLowerCase() === 'inprogress';
    });
    if (requiresExpeditionEscapeRestock(client.snapshot, activeResumeExpeditionId)) {
      const resumeSupply = await supplyGateForQuest(client, activeResumeExpeditionId, {
        replenishEscapeReserve: true,
      });
      if (resumeSupply.status === 'restocked') {
        recordSupplyRetreat(report, activeResumeExpeditionId, resumeSupply);
      }
    }
    const resumeRecovery = await stabilizeJourneyResume(client, navigate, {
      ensureHpSupply: owner => {
        const activeQuestId = ids.find(id => {
          const quest = owner.snapshot?.questLog?.find(entry => Number(entry?.questId) === Number(id));
          return String(quest?.stage ?? '').replace(/[^a-z]/gi, '').toLowerCase() === 'inprogress';
        });
        return supplyGateForQuest(owner, activeQuestId);
      },
      startJourneyEmergencyHpRecovery,
      provokeTrappingHostile: owner => {
        const actor = selfPlayer(owner);
        const target = (owner.snapshot?.entities ?? [])
          .filter(entity => entity?.kind === 'monster' && !entity.dead &&
            Math.max(Math.abs(Number(entity.x) - Number(actor.x)), Math.abs(Number(entity.y) - Number(actor.y))) <= 1)
          .sort((left, right) => Number(left.hp ?? 0) - Number(right.hp ?? 0))[0];
        if (!target) return false;
        owner.send({ type: 'attack', objectId: Number(target.objectId) });
        return true;
      },
    });
    if (resumeRecovery.status !== 'ready') report.resumeRecovery = resumeRecovery;
    if (resumeRecovery.revival) report.revivals.push(resumeRecovery.revival);
    report.supplyFunding = await finishReadySupplyFundingQuest(client, {
      route,
      travel,
      navigate,
      questIds: ids,
    });
    const readyResumeQuestId = ids.find(id => {
      const quest = client.snapshot.questLog.find(entry => Number(entry?.questId) === Number(id));
      return String(quest?.stage ?? '').replace(/[^a-z]/gi, '').toLowerCase() === 'readytoturnin';
    });
    if (readyResumeQuestId != null) {
      report.restock = { status: 'deferredForTurnIn', questId: readyResumeQuestId };
    } else {
      const nearCompleteResumeQuestId = ids.find(id =>
        canFinishNearCompleteKillQuestWithoutHpStock(client.snapshot, id, {
          minimumHealthRatio: 0,
        }));
      const activeResumeQuestId = ids.find(id => {
        const quest = client.snapshot.questLog.find(entry => Number(entry?.questId) === Number(id));
        return String(quest?.stage ?? '').replace(/[^a-z]/gi, '').toLowerCase() === 'inprogress';
      });
      report.restock = await supplyGateForQuest(
        client,
        nearCompleteResumeQuestId ?? activeResumeQuestId,
      );
    }
    report.quests = [];
    report.milestones = [];
    const deferredQuests = new Map();
    const lostTargetRetries = new Map();
    const finishJourneyQuest = async (q, record) => {
      const reward = q.rewards.selectableItems.find(i =>
        i.count > 0 && (i.requiredClass & route.classMask) && (i.requiredGender & 1));
      const before = {
        level: selfPlayer(client).level,
        experience: client.snapshot.playerExperience,
        gold: client.snapshot.gold,
      };
      if (q.finishNpc) {
        record.finishTravel = await travelToQuestNpc(client, q, 'finish', { travel, navigate });
      }
      const beforeItems = structuredClone(client.snapshot);
      record.finish = await interactQuest(
        client,
        q,
        { type: 'finish', selectedItemIndex: reward?.selectionIndex ?? -1 },
        navigate,
      );
      record.itemRewards = verifyItemRewards(
        beforeItems,
        client.snapshot,
        expectedItemRewards(q, reward?.selectionIndex ?? -1),
      );
      record.rewardBefore = before;
      record.rewardAfter = {
        level: selfPlayer(client).level,
        experience: client.snapshot.playerExperience,
        gold: client.snapshot.gold,
      };
      record.finishedAt = new Date().toISOString();
      await fs.writeFile(path.join(output, `${className}.report.json`), JSON.stringify(report, null, 2));
    };
    const finishReadyDeferredQuests = async ({ requireReady = false } = {}) => {
      for (const [questId, deferred] of deferredQuests) {
        const state = client.snapshot.questLog.find(entry => Number(entry?.questId) === questId);
        const stage = String(state?.stage ?? '').replace(/[^a-z]/gi, '').toLowerCase();
        if (stage === 'readytoturnin') {
          await finishJourneyQuest(deferred.quest, deferred.record);
          deferredQuests.delete(questId);
          continue;
        }
        if (stage === 'completed') {
          deferred.record.persistedCompleted = true;
          deferredQuests.delete(questId);
          continue;
        }
        if (requireReady) {
          throw new Error(
            `q${questId} co-farm objectives did not complete after quests ` +
            `${deferred.followers.join(', ')}`,
          );
        }
      }
    };
    const maxQuests = Number(process.env.MIR2_JOURNEY_MAX_QUESTS ?? ids.length);
    for (let index = 0; index < Math.min(ids.length, maxQuests); index++) {
      const id = ids[index];
      try {
      await finishReadyDeferredQuests();
      report.milestones.push(...await claimAvailableMilestones(client, { profile, route, travel, navigate }));
      const q = route.quests.find(q => q.questId === id);
      let state = client.snapshot.questLog.find(q => q.questId === id);
      if (state?.stage === 'completed') { report.quests.push({ questId: id, persistedCompleted: true }); continue; }
      const record = { questId: id, startedAt: new Date().toISOString() };
      report.quests.push(record);
      console.log(JSON.stringify({ className, questId: id, stage: state?.stage, level: selfPlayer(client).level }));
      if (!state) throw new Error(`q${id} unavailable at level ${selfPlayer(client).level}`);
      if (state.stage === 'available') {
        if (q.startNpc) {
          record.startTravel = await travelToQuestNpc(client, q, 'start', { travel, navigate });
        }
        record.accept = await interactQuest(client, q, 'accept', navigate);
      }
      state = client.snapshot.questLog.find(q => q.questId === id);
      const remainingQuestIds = ids.slice(index + 1);
      if (shouldDeferCofarmQuest(id, state.stage, remainingQuestIds)) {
        const followers = cofarmFollowers(id, remainingQuestIds);
        record.deferredAt = new Date().toISOString();
        record.deferredForQuestIds = followers;
        deferredQuests.set(Number(id), { quest: q, record, followers });
        await fs.writeFile(path.join(output, `${className}.report.json`), JSON.stringify(report, null, 2));
        continue;
      }
      if (state.stage !== 'readyToTurnIn') {
        if (q.objectives.flag.length) {
          const { completeFlagObjectives } = await import('./protocol-flag-objectives.mjs');
          record.flags = await completeFlagObjectives(client, q, navigate, travel);
        }
        if (q.objectives.kill.length || q.objectives.item.length) {
          const { completeQuestObjectives } = await import('./protocol-combat.mjs');
          const retreatProfile = questRetreatProfile(id, className);
          const taoistCriticalOffenseGuard = id === 89 && className === 'Taoist'
            ? {
              hpRatio: questEmergencyEscapeHpRatio(id, className),
              minimumProvenAggressors: 2,
            }
            : null;
          const objectiveCombatAction = (owner, target) => questCombatAction(owner, q, target);
          const objectiveApproachRange = (owner, target) => questCombatApproachRange(owner, q, target);
          const playerCombatAction = createWizardKitingAction(objectiveCombatAction, navigate, {
            maxRetreatSteps,
            maxRetreatCellsPerTarget: maxAttackAttempts * maxRetreatSteps,
            fightWhenBlocked: true,
            approachRange: objectiveApproachRange,
            // The D2031 entrance can reveal a CursedShaman before the q89
            // objective is in AOI. Its source attack band ends at six while
            // the Wizard projectile band ends at nine. Do not convert that
            // theoretical advantage into an immunity claim: require a live,
            // collision-planned 7-9 tile firing position outside every
            // visible Shaman footprint before this q89 Wizard casts one.
            // Existing proven-aggressor and critical-escape gates run before
            // this action and remain the authority for a second attacker.
            ...(id === 89 && className === 'Wizard' ? {
              rangedSafetyBand: {
                protectedMonsterNames: ['CursedShaman', 'CursedShaman0'],
                minimumTargetDistance: 7,
                maximumTargetDistance: 9,
                unsafeShamanDistance: 6,
                maxRetreatSteps: 6,
              },
            } : {}),
            // A kiting action owns an asynchronous reposition before it calls
            // the captured base spell. Reuse combat's q89 Taoist policy at
            // that last possible point, then let the next combat iteration
            // enter the existing held-escape/bounded-retreat authority.
            ...(taoistCriticalOffenseGuard ? {
              beforeBaseAction: owner => criticalProvenAggressorOffenseGuardActive(owner, {
                criticalProvenAggressorOffenseGuard: taoistCriticalOffenseGuard,
                directAggressorDistance: retreatProfile.directAggressorDistance,
              }) ? { kind: 'wait', delayMs: 1 } : null,
            } : {}),
          });
          record.objectives = await completeQuestObjectives(client, q, navigate, {
            prepare: async owner => {
              const supply = await supplyGateForQuest(owner, id);
              if (supply.status === 'restocked') recordSupplyRetreat(report, id, supply);
              const loadout = await prepareLoadout(owner);
              if (className === 'Taoist') loadout.equippedAmulet = await equipHeldAmulet(owner);
              return loadout;
            },
            action: playerCombatAction,
            maxAttackAttempts,
            // A moving monster can keep invalidating the authoritative cast
            // window without ever taking damage. Quarantine that exact actor
            // after twenty seconds and select another live quest target.
            maxMovingTargetNoProgressMs: 20_000,
            approachRange: objectiveApproachRange,
            // Shared-zone projectile resolution and the authoritative quest
            // award can arrive more than seven seconds after the cast. R45
            // observed the death/progress packet at 7.18s, just after the old
            // six-second harness deadline, so keep this acceptance wait above
            // that measured delivery window.
            questSettleTimeout: 12_000,
            sustain: async (owner, context = {}) => {
              const carving = context.phase === 'harvest';
              const retreating = context.phase === 'retreat';
              const defensive = carving || retreating;
              if (retreating) {
                // Both commands are fire-and-continue here. Waiting for the
                // potion inventory acknowledgement before the first escape
                // step costs several monster attack cycles in dense fields.
                const classRecovery = await useClassRecovery(owner, { hpThreshold: 0.85 });
                const consumed = startJourneyEmergencyHpRecovery(owner, { hpThreshold: 0.85 });
                return { consumed, classRecovery };
              }
              const consumed = await useJourneySupplies(owner, {
                // Carving leaves the player stationary for multiple server
                // ticks, while a retreat must survive until it opens a real
                // safety margin. Begin recovery before the combat-only
                // threshold in both phases.
                hpThreshold: defensive ? 0.85 : 0.6,
                // R27 reached D406 with dozens of MP bottles but only 1-9
                // active MP because spell expenditure outran the late 30%
                // trigger. Start q54 caster recovery while the active pool can
                // still sustain the current target and its escape window.
                mpThreshold: questCombatMpUseThresholdForQuest(id, className),
              });
              const classRecovery = defensive
                ? await useClassRecovery(owner, { hpThreshold: 0.85 })
                : null;
              return { consumed, classRecovery };
            },
            travel,
            afterTravel: async owner => {
              const supply = await supplyGateForQuest(owner, id);
              if (supply.status !== 'restocked') return false;
              recordSupplyRetreat(report, id, supply);
              return true;
            },
            afterEngagement: async (owner, navigateNear) => {
              const currentQuest = owner.snapshot.questLog.find(entry => Number(entry?.questId) === id);
              const currentStage = String(currentQuest?.stage ?? '').replace(/[^a-z]/gi, '').toLowerCase();
              if (currentStage !== 'readytoturnin') {
                const supply = await supplyGateForQuest(owner, id);
                if (supply.status === 'restocked') recordSupplyRetreat(report, id, supply);
              }
              return collectNearbyGold(owner, navigateNear);
            },
            refreshWhileWaiting: refreshCombatCooldown,
            // Spawn declarations are search centres, not safe destinations.
            // Keep a full aggro buffer while crossing an unknown field; the
            // target-selection pass will separately admit an isolated monster.
            // q30 has one exact Currish field behind ordinary roaming snakes.
            // A six-tile exclusion around every visible monster can seal the
            // only collision route completely. Two tiles still prevent direct
            // aggro contact while allowing a running player to weave through
            // the field; the denser q33/q36 hunts keep the wider buffer.
            spawnSearchHostileClearance: id === 30 ? 2 : 4,
            // The q89 Wizard reaches CursedShaman at range nine, but its
            // ordinary spawn-search waypoint must never spend the initial
            // approach inside the measured six-tile Shaman footprint. Keep
            // the existing generic clearance (and its cave fallback) for
            // zombies; only these two Shaman names retain this wider buffer.
            spawnSearchProtectedHostileClearance: id === 89 && className === 'Wizard'
              ? { CursedShaman: 6, CursedShaman0: 6 }
              : {},
            // When that protected search cannot reach a required spawn, a
            // Wizard may remove at most two visible Shaman blockers through
            // the normal collision-checked seven-to-nine-tile firing band.
            // The combat helper leaves every other quest and class inert.
            spawnStallProtectedBlocker: id === 89 && className === 'Wizard'
              ? {
                monsterNames: ['CursedShaman', 'CursedShaman0'],
                minimumApproachDistance: 7,
                maximumApproachDistance: 9,
                clearance: 6,
                maxBlockers: 2,
              }
              : null,
            // The live D2031 -> D2032 route crosses the entrance Shaman
            // pair. Guard every physical transit cell and clear only from
            // the same collision-certified band used by stalled spawn
            // searches; this option is inert outside q89 Wizard travel.
            transitProtectedBlocker: id === 89 && className === 'Wizard'
              ? {
                monsterNames: ['CursedShaman', 'CursedShaman0'],
                minimumApproachDistance: 7,
                maximumApproachDistance: 9,
                clearance: 6,
                maxBlockers: 2,
              }
              : null,
            // Narrow Crystal cave corridors are often sealed by one monster's
            // four-cell avoidance halo. Retry the same search waypoint with a
            // one-cell buffer; proven attacks still interrupt navigation and
            // enter the existing retreat/clear loop.
            // D421/D422 use single-tile cave corridors. A one-cell halo around
            // every visible zombie can disconnect all 419 generated coverage
            // points even while the player remains near the entrance. Q54 may
            // therefore make a zero-clearance path attempt; the first proven
            // hit still interrupts navigation and enters bounded combat.
            spawnSearchHostileClearanceFallback:
              questSpawnSearchHostileClearanceFallback(id),
            combatHostileClearance: id === 30 ? 3 : ([33, 36].includes(id) ? 2 : 1),
            // R57 resumed q60 inside D2041's entrance pocket with the only
            // live SpiderFrog fourteen cells away. One passive KekTal's
            // one-cell halo disconnected that otherwise valid corridor on
            // every timed retry. Keep the first buffered attempt, then allow
            // q60 to path adjacent to a passive blocker without ever stepping
            // onto its occupied cell.
            combatHostileClearanceFallback: [30, 60].includes(id) ? 0 : ([33, 36].includes(id) ? 2 : 1),
            // D2041 has a one-cell entrance corridor. R58 proved that a
            // passive KekTal standing three cells from the player can occupy
            // the only route to a visible SpiderFrog even with zero avoidance
            // clearance. Clear at most one nearby doorway occupant per q60
            // progress step, then retry the objective target.
            maxApproachBlockerClears: id === 60 ? 1 : 0,
            approachBlockerSearchRadius: 6,
            unsafeTargetObservationTimeout: id === 30 ? 45_000 : 3_000,
            // Q42 deliberately sends a level-13+ player into the dense Serpent
            // Valley fields for twenty ordinary snakes. The R10 Taoist trace
            // selected a 0-adjacent/2-nearby RedViper; those neighbours joined
            // within four seconds and trapped the player behind three active
            // attackers. A 0/1 cap left no usable target for a later 6.5-minute
            // field pass, while 1/3 admitted all eight observed RedVipers in the
            // first pass. Use that measured middle ground and let the first
            // actual extra attacker abort the pull below. Q54's narrow mine
            // corridors still require clearing one paired zombie before the
            // player has room to move.
            // Q30 keeps its unique harvest-specific limits.
            maxTargetAdjacent: retreatProfile.maxTargetAdjacent ??
              (id === 30 ? 2 : (id === 42 ? 1 : (id === 49 ? 1 : ([54, 62].includes(id) ? 1 :
              (id === 33 && className === 'Wizard' ? 1 : 0))))),
            // The remaining q62 KekTal field presented several viable 0/2
            // and 1/1 candidates but no ordinary 0/1 isolation for more than
            // two minutes. Admit the same bounded target density already
            // proven in q54; actual extra attackers still interrupt combat.
            maxTargetNearby: retreatProfile.maxTargetNearby ??
              (id === 30 ? 4 : (id === 42 ? 3 : (id === 49 ? 4 : ([54, 62].includes(id) ? 4 :
              (id === 33 && className === 'Wizard' ? 4 : 1))))),
            // A Taoist can lose two snake hits between consecutive snapshots.
            // Begin q33 disengagement with enough HP for the mandatory
            // standstill-to-walk first movement before full-speed running.
            lowHealthTargetRetreatRatio: id === 33 && className === 'Taoist' ? 0.65 : undefined,
            // q54 carries a large, proven HP reserve specifically for mine
            // corridors. Clear a small established pack while healthy and
            // reserve full disengagement for the genuinely dangerous range.
            multiAggressorRetreatRatio: retreatProfile.multiAggressorRetreatRatio,
            // The D406 trace reached fourteen visible hostiles and four
            // simultaneous adjacent attackers while the Warrior still held
            // 69 HP drugs. Leave as soon as three attackers are proven instead
            // of waiting for HP to fall below the percentage threshold; this
            // gives the first escape command time to clear the serialized
            // gateway queue before the next shared attack wave.
            // On q42 the chosen snake remains the bounded focus target, but
            // the first independently proven attacker means that isolated pull
            // has failed and must be abandoned before the field converges.
            retreatAtActiveAggressorCount: id === 42 ? 1 : retreatProfile.retreatAtActiveAggressorCount,
            directAggressorDistance: retreatProfile.directAggressorDistance,
            // The unique Currish must be carved for JadeRing. Once ordinary
            // quest funding has supplied HP drugs, a player can burst that
            // small target and harvest immediately instead of trying to clear
            // an endlessly repopulating snake field first.
            // q49 live traces showed a Skeleton already at 40% being abandoned
            // for the first fresh aggressor, leaving both monsters alive and
            // attacking. Finish the chosen short target while healthy; the
            // focused policy still retreats for two extra aggressors or the
            // configured low-health threshold.
            // R65 left a q60 SpiderFrog at 40% after eight ranged casts because
            // one joining cave monster switched the caster into an endless
            // retreat. Keep the isolated target as the bounded focus for both
            // ranged classes; the shared low-health and multi-aggressor gates
            // still disengage unless the objective is already finishable.
            focusTargetThroughAggressors: retreatProfile.focusTargetThroughAggressors ??
              ([30, 42, 49].includes(id) || id === 33 ||
              (id === 99 && className === 'Warrior') ||
              (id === 60 && ['Wizard', 'Taoist'].includes(className))),
            // A q33 Taoist can leave a RedSnake at one ordinary strike after
            // a new TigerSnake joins. Above the critical survival floor, land
            // that bounded final blow before disengaging so the nearly dead
            // objective does not chase the player out of AOI and get lost.
            finishableTargetHealthRatio: retreatProfile.finishableTargetHealthRatio ??
              (id === 99 && className === 'Warrior' ? 0.7 :
              (id === 33 && className === 'Taoist' ? 0.25 :
              (id === 60 && ['Wizard', 'Taoist'].includes(className) ? 0.5 : undefined))),
            finishableTargetMinimumPlayerHpRatio:
              retreatProfile.finishableTargetMinimumPlayerHpRatio ??
              (id === 99 && className === 'Warrior' ? 0.7 :
              (id === 33 && className === 'Taoist' ? 0.4 :
              (id === 60 && ['Wizard', 'Taoist'].includes(className) ? 0.6 : undefined))),
            // Bill's Bichon mine quest can source both remaining Zombie5 and
            // Zombie1 in D406. Prefer that ordinary Bichon-mine route over the
            // equally short but much denser D421 crossing into D422.
            // Natural Cave has the same direct one-hop route and authoritative
            // Skeleton source as Oma Cave 1F, with 264 total respawns instead
            // of 358. Use that lower-density level-14 field rather than
            // spending one life per crowded Oma pull.
            preferredObjectiveMaps: preferredObjectiveMapsForQuest(id, className),
            preferObjectiveMapOverCurrent: shouldPreferObjectiveMapOverCurrent(id, state, className),
            // R117 exhausted one q89 Wizard pass in D2031 without an
            // objective increment, then returned to the same source on every
            // replan. The combat controller may make one D2032 preference
            // only after that zero-progress failure and only while this exact
            // rendered doorway is live. Browser snapshots expose the live
            // key/source/target map; createMapTraveler retains its normal
            // authoritative map-landing validation.
            objectiveMapFallback: id === 89 && className === 'Wizard' ? {
              fromMapFileName: 'D2031',
              toMapFileName: 'D2032',
              transferCandidates: [
                {
                  key: 'crystal-move:d2031:198:34:117:184:267',
                  source: { x: 198, y: 34 },
                },
                {
                  key: 'crystal-move:d2031:198:35:117:184:268',
                  source: { x: 198, y: 35 },
                },
              ],
            } : undefined,
            preferTravelAggressorCombat: retreatProfile.preferTravelAggressorCombat,
            continueTravelWhileHealthy: retreatProfile.continueTravelWhileHealthy,
            maxTravelThreatEvasionsPerEdge: retreatProfile.maxTravelThreatEvasionsPerEdge,
            // q60 entered D2041 at 58/207 HP with 76 bottles and two adjacent
            // monsters. It must keep moving and dosing through the same bounded
            // recovery path instead of treating a 45-second follower as an
            // unrecoverable timeout before the first SpiderFrog objective.
            allowLowHealthFollowerRecovery: retreatProfile.allowLowHealthFollowerRecovery,
            maxRetreatBreakoutKills: retreatProfile.maxRetreatBreakoutKills,
            // D421 is a long diagonal mine crossing. When exposure is equally
            // survivable, keep emergency steps moving toward the D422 transfer
            // instead of giving back the entire corridor after every pull.
            unsafeRetreatBiasPosition: snapshot => questRetreatBiasPosition(id, snapshot, className),
            // RandomTeleport is bought and consumed through the same public
            // NPCGoods/UseItem packets as a player. It is reserved for the
            // observed no-step cave trap and never replaces ordinary retreat.
            emergencyEscapeHpRatio: questEmergencyEscapeHpRatio(id, className),
            emergencyEscape: dangerousExpeditionQuestIds.has(id)
              ? emergencyEscapeForJourney
              : undefined,
            // R127 entered retreat breakout at 7/180 HP under four direct
            // zombie hits. Breakout intentionally disables ordinary target
            // interruption, so keep this narrow q89 Taoist pre-offense guard
            // ahead of its travel exemption and the wounded-target finish.
            criticalProvenAggressorOffenseGuard: taoistCriticalOffenseGuard,
            harvestBeforeClearingAggressors: id === 30,
            unsafeRetreatSteps: retreatProfile.unsafeRetreatSteps,
            unsafeRetreatSafeDistance: retreatProfile.unsafeRetreatSafeDistance,
            recoverAfterUnsafeRetreat: async (owner, navigateNear) => {
              // Once the authoritative quest state is ready, the next action
              // is the bounded route to its finish endpoint. Waiting to reach
              // 75% HP in the old hostile field can consume the remaining
              // stock while new monsters keep entering AOI, even though no
              // further objective combat is needed.
              if (!questNeedsPostRetreatRecovery(owner.snapshot, id)) return;
              if (hpDrugCount(owner.snapshot) === 0) {
                // The pack retreat has already established a real safety
                // margin. With no HP restorative left, waiting in the field
                // for 75% health cannot succeed under a pursuing monster.
                // Leave through the ordinary map route and recover/fund in
                // town before any bounded evasive-recovery wait begins.
                const supply = await supplyGateForQuest(owner, id);
                if (supply.status === 'restocked') recordSupplyRetreat(report, id, supply);
                return;
              }
              let hp = observedPlayerHp(owner.snapshot);
              let maxHp = Number(owner.snapshot?.playerMaxHp ?? 0);
              const recoveryRatio = questPostRetreatRecoveryRatio(id, className);
              if (maxHp > 0 && hp / maxHp < recoveryRatio) {
                // A lone Crystal pursuer often matches player speed, so a
                // larger geometric gap may never open. Keep moving while
                // Healing and the first potion tick, refreshing in short
                // bounded batches until departure health is restored.
                await recoverHealthWhileEvading(owner, navigateNear, {
                  requiredRatio: recoveryRatio,
                  timeoutMs: evasiveRecoveryTimeoutMsForQuest(id),
                  dangerDistance: evasiveRecoveryDangerDistanceForQuest(id),
                  retreatSteps: dangerousExpeditionQuestIds.has(id) ? 12 : 6,
                  maxEvasiveMoves: dangerousExpeditionQuestIds.has(id) ? 8 : 4,
                  biasPosition: snapshot => questRetreatBiasPosition(id, snapshot, className),
                  emergencyEscape: dangerousExpeditionQuestIds.has(id)
                    ? emergencyEscapeForJourney
                    : undefined,
                  emergencyEscapeHpRatio: questEmergencyEscapeHpRatio(id, className),
                  maxEmergencyEscapes: 1,
                  sustain: async current => {
                    const classRecovery = await useClassRecovery(current, { hpThreshold: 0.9 });
                    const consumed = startJourneyEmergencyHpRecovery(current, { hpThreshold: 0.9 });
                    return { consumed, classRecovery };
                  },
                });
              }
              // Crystal restoratives recover over time. Two immediate doses
              // cover the escape window; repeatedly consuming six before the
              // regeneration ticks wastes an entire stack during one retreat.
              for (let attempt = 0; attempt < 2; attempt += 1) {
                hp = observedPlayerHp(owner.snapshot);
                maxHp = Number(owner.snapshot?.playerMaxHp ?? 0);
                if (maxHp > 0 && hp / maxHp >= 0.9) break;
                const consumed = await useJourneySupplies(owner, { hpThreshold: 0.9, mpThreshold: 0 });
                if (!consumed.length) break;
              }
              hp = observedPlayerHp(owner.snapshot);
              maxHp = Number(owner.snapshot?.playerMaxHp ?? 0);
              if (maxHp > 0 && hp / maxHp < recoveryRatio) {
                await recoverHealthWhileEvading(owner, navigateNear, {
                  requiredRatio: recoveryRatio,
                  timeoutMs: evasiveRecoveryTimeoutMsForQuest(id),
                  dangerDistance: evasiveRecoveryDangerDistanceForQuest(id),
                  retreatSteps: id === 54 ? 12 : 6,
                });
              }
              const recoveredHp = observedPlayerHp(owner.snapshot);
              const recoveredMaxHp = Number(owner.snapshot?.playerMaxHp ?? 0);
              if (!(recoveredMaxHp > 0) || recoveredHp / recoveredMaxHp < recoveryRatio) {
                throw new Error(`unsafe-pack recovery stopped below departure health (${recoveredHp}/${recoveredMaxHp})`);
              }
              const supply = await supplyGateForQuest(owner, id);
              if (supply.status === 'restocked') recordSupplyRetreat(report, id, supply);
            },
          });
        }
      }
      await finishJourneyQuest(q, record);
      await finishReadyDeferredQuests();
      } catch (error) {
        if (hasAuthoritativePlayerDeath(client.snapshot) && report.revivals.length < maxJourneyRevivals) {
          if (report.quests.length) report.quests.at(-1).failure = error.message;
          report.revivals.push(await reviveInTown(client));
          report.restock = await supplyGateForQuest(client, id);
          if (report.restock.status === 'restocked') recordSupplyRetreat(report, id, report.restock);
          index--;
          continue;
        }
        const livingEvasiveTimeout = isLivingEvasiveRecoveryTimeout(client.snapshot, error);
        const livingExpeditionNoPath = isLivingExpeditionNoWalkPath(client.snapshot, id, error);
        const livingUnsafeRetreatFailure = isLivingUnsafePackRetreatFailure(client.snapshot, error);
        const livingLostTarget = isLivingLostCombatTarget(client.snapshot, error);
        const lostTargetRetry = livingLostTarget ? (lostTargetRetries.get(id) ?? 0) + 1 : 0;
        if (livingLostTarget) lostTargetRetries.set(id, lostTargetRetry);
        if (livingLostTarget && lostTargetRetry <= 8) {
          client.record('diagnostic', {
            type: 'livingLostCombatTargetRetry',
            questId: id,
            retry: lostTargetRetry,
            mapFileName: String(client.snapshot?.mapFileName ?? ''),
            reason: String(error?.message ?? error),
          });
          index--;
          continue;
        }
        if (livingEvasiveTimeout || livingExpeditionNoPath || livingUnsafeRetreatFailure) {
          client.record('diagnostic', {
            type: livingUnsafeRetreatFailure ? 'livingUnsafePackRetreatRetry' :
              (livingExpeditionNoPath ? 'livingExpeditionNoWalkPathRetry' :
                'livingEvasiveRecoveryTimeoutRetry'),
            questId: id,
            hp: observedPlayerHp(client.snapshot),
            maxHp: Number(client.snapshot?.playerMaxHp ?? 0),
            mapFileName: String(client.snapshot?.mapFileName ?? ''),
          });
          report.restock = await supplyGateForQuest(client, id);
          if (report.restock.status === 'restocked') recordSupplyRetreat(report, id, report.restock);
          index--;
          continue;
        }
        throw error;
      }
    }
    await finishReadyDeferredQuests({ requireReady: true });
    report.milestones.push(...await claimAvailableMilestones(client, { profile, route, travel, navigate }));
    report.completed = ids.every(id => client.snapshot.questLog.some(q => q.questId === id && q.stage === 'completed')) && selfPlayer(client).level >= 30 && JOURNEY_MILESTONES.every(level => client.snapshot.questLog.some(q => q.questId === 2100000 + level && q.stage === 'completed'));
  }
  await fs.writeFile(path.join(output, `${className}.snapshot.json`), JSON.stringify(client.snapshot, null, 2));
  console.log(JSON.stringify({ className, bootstrapPassed: true, snapshotKeys: Object.keys(client.snapshot) }));
} catch (error) {
  report.error = error.message;
  process.exitCode = 1;
  console.error(error.message);
} finally {
  report.finishedAt = new Date().toISOString();
  if (client.snapshot) await fs.writeFile(path.join(output, `${className}.snapshot.json`), JSON.stringify(client.snapshot, null, 2));
  await client.close();
  await fs.writeFile(path.join(output, `${className}.report.json`), JSON.stringify(report, null, 2));
  await fs.writeFile(path.join(output, `${className}.${runId}.report.json`), JSON.stringify(report, null, 2));
}

function recordSupplyRetreat(report, questId, supply) {
  (report.supplyRetreats ??= []).push({
    questId,
    at: new Date().toISOString(),
    status: supply.status,
    restockCount: supply.restockCount,
    returnMap: supply.returnMap ?? null,
  });
}

function playerIsDead(snapshot) {
  return hasAuthoritativePlayerDeath(snapshot);
}

function criticalStrandedPlayer(snapshot) {
  const hp = observedPlayerHp(snapshot);
  const maxHp = Number(snapshot?.playerMaxHp ?? 0);
  const actor = (snapshot?.entities ?? []).find(entity =>
    Number(entity?.objectId) === Number(snapshot?.playerObjectId));
  const nearbyHostiles = actor ? (snapshot?.entities ?? []).filter(entity =>
    String(entity?.kind ?? '').toLowerCase() === 'monster' &&
    String(entity?.disposition ?? '').toLowerCase() === 'hostile' &&
    entity?.dead !== true && Number(entity?.hp ?? 1) > 0 &&
    Math.max(Math.abs(Number(entity?.x) - Number(actor.x)), Math.abs(Number(entity?.y) - Number(actor.y))) <= 8
  ).length : 0;
  const recoverableHp = hp + hpDrugCount(snapshot) * 30;
  return String(snapshot?.mapFileName ?? '') !== '0' && hp > 0 && maxHp > 0 &&
    Number(snapshot?.gold ?? 0) < 40 && recoverableHp < maxHp * 0.75 &&
    (hp / maxHp < 0.25 || nearbyHostiles >= 2);
}

function canAttemptEmergencyFundingQuest(owner, questId, error) {
  if (!String(error?.message ?? '').startsWith('needsFunds:')) return false;
  return emergencyFundingQuestState(owner, questId);
}

function emergencyFundingQuestState(owner, questId) {
  if (!activeEmergencyFundingQuest(owner, questId)) return false;
  const funding = owner.snapshot?.questLog?.find(candidate => Number(candidate?.questId) === 30);
  const stage = String(funding?.stage ?? '').toLowerCase();
  const ratio = livingHealthRatio(owner.snapshot);
  if (!(ratio > 0)) return false;
  // Once the ring is secured, travel to Bradley and claim its 800 gold even
  // when the fight ended below the normal departure threshold. Before combat,
  // require enough health to survive one isolated Currish without potions.
  return stage === 'readytoturnin' || ratio >= 0.7;
}

function activeEmergencyFundingQuest(owner, questId) {
  if (questId != null && Number(questId) !== 30) return false;
  const funding = owner.snapshot?.questLog?.find(candidate => Number(candidate?.questId) === 30);
  const stage = String(funding?.stage ?? '').toLowerCase();
  return ['available', 'inprogress', 'readytoturnin'].includes(stage);
}

function livingHealthRatio(snapshot) {
  const hp = observedPlayerHp(snapshot);
  const maxHp = Number(snapshot?.playerMaxHp ?? 0);
  return hp > 0 && maxHp > 0 ? hp / maxHp : 0;
}

function minimumJourneyHpStockForQuest(questId) {
  if (dangerousExpeditionQuestIds.has(Number(questId))) return 24;
  if ([42, 49].includes(Number(questId))) return 12;
  return minimumJourneyHpStock;
}

async function stabilizeJourneyResume(owner, navigateNear, {
  ensureHpSupply,
  startJourneyEmergencyHpRecovery: startHpRecovery = startEmergencyHpRecovery,
  provokeTrappingHostile,
} = {}) {
  let disposition = journeyResumeDisposition(owner.snapshot);
  if (['ready', 'readyToTurnIn'].includes(disposition.status)) return disposition;
  if (canResumeStockedCombatExpedition(owner.snapshot)) {
    return { ...disposition, status: 'stockedCombatExpedition' };
  }
  if (disposition.status === 'awaitDeath' && !playerIsDead(owner.snapshot)) {
    try {
      await owner.wait(() => playerIsDead(owner.snapshot), 'lethal resumed field pressure', 15_000);
    } catch (error) {
      if (!String(error?.message ?? '').startsWith('Timeout waiting for')) throw error;
    }
    if (!playerIsDead(owner.snapshot)) disposition = journeyResumeDisposition(owner.snapshot);
  }
  if (playerIsDead(owner.snapshot)) {
    return { ...disposition, status: 'revived', revival: await reviveInTown(owner) };
  }
  if (hpDrugCount(owner.snapshot) === 0 && typeof ensureHpSupply === 'function') {
    try {
      const supply = await ensureHpSupply(owner);
      return { ...disposition, status: 'suppliedBeforeRecovery', supply };
    } catch (error) {
      if (playerIsDead(owner.snapshot)) {
        return { ...disposition, status: 'revived', revival: await reviveInTown(owner) };
      }
      const trappedByCollision = String(error?.message ?? '').startsWith('No walk path');
      if (!trappedByCollision || !['evade', 'awaitDeath'].includes(disposition.status)) throw error;
      owner.record('diagnostic', {
        type: 'trappedResumeAwaitingTownRevive',
        reason: String(error.message),
        mapFileName: String(owner.snapshot?.mapFileName ?? ''),
        adjacent: disposition.adjacent,
        nearby: disposition.nearby,
      });
      // A persisted character can reconnect inside a completely occupied cave
      // cell. With no restorative and no legal first step, ordinary travel is
      // impossible; let the already-attacking pack finish the authoritative
      // death, then use the normal town-revive protocol.
      if (typeof provokeTrappingHostile === 'function') await provokeTrappingHostile(owner);
      await owner.wait(() => playerIsDead(owner.snapshot), 'trapped resumed field death', 45_000);
      return { ...disposition, status: 'revived', revival: await reviveInTown(owner) };
    }
  }
  try {
    const recovery = await recoverHealthWhileEvading(owner, navigateNear, {
      requiredRatio: 0.75,
      timeoutMs: 90_000,
      dangerDistance: 7,
      retreatSteps: 16,
      maxEvasiveMoves: 16,
      sustainCadenceMs: 4_000,
      sustain: async current => {
        const classRecovery = await useClassRecovery(current, { hpThreshold: 0.9 });
        const consumed = startHpRecovery(current, { hpThreshold: 0.9 });
        return { classRecovery, consumed };
      },
    });
    return { ...disposition, status: 'recovered', recovery };
  } catch (error) {
    if (!playerIsDead(owner.snapshot)) throw error;
    return { ...disposition, status: 'revived', revival: await reviveInTown(owner) };
  }
}
