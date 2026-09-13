import { delay as realDelay } from "./protocol-client.mjs";
import { distance, NavigationStalled } from "./protocol-play.mjs";
import { TravelBlockedByMonster, TravelInterrupted } from "./protocol-travel.mjs";

const READY_STAGES = new Set(["readytoturnin", "completed"]);
const DIRECTION_BY_DELTA = new Map([
  ["0,-1", "Up"], ["1,-1", "UpRight"], ["1,0", "Right"], ["1,1", "DownRight"],
  ["0,1", "Down"], ["-1,1", "DownLeft"], ["-1,0", "Left"], ["-1,-1", "UpLeft"],
]);

class ThreatenedNavigation extends Error {
  constructor(message, threatObjectId = null) {
    super(message);
    this.threatObjectId = Number.isSafeInteger(Number(threatObjectId))
      ? Number(threatObjectId)
      : null;
  }
}
class UnreachableCombatTarget extends Error {
  constructor(objectId, target, cause) {
    super(`target ${objectId} has no walk path from the current map region`, { cause });
    this.objectId = objectId;
    this.target = validPoint(target) ? { x: Number(target.x), y: Number(target.y) } : null;
  }
}
class UnresponsiveCombatTarget extends Error {
  constructor(objectId, target, attempts, cause) {
    super(`target ${objectId} made no authoritative combat progress after ${attempts} attacks`, { cause });
    this.objectId = objectId;
    this.target = validPoint(target) ? { x: Number(target.x), y: Number(target.y) } : null;
    this.attempts = attempts;
  }
}
class LostCombatTarget extends Error {
  constructor(objectId, target) {
    super(`target ${objectId} left the authoritative snapshot before attack`);
    this.objectId = objectId;
    this.target = validPoint(target) ? { x: Number(target.x), y: Number(target.y) } : null;
  }
}
class UnsafeTargetCluster extends Error {
  constructor(objectId, risk) {
    super(`target ${objectId} entered an unsafe hostile pack (${risk.adjacent} adjacent, ${risk.nearby} nearby)`);
    this.objectId = objectId;
    this.risk = risk;
  }
}
class SpawnSearchExhausted extends Error {}

/**
 * Complete the kill and monster-item objectives of one already-active quest.
 * This module only emits normal attack, harvest and pickup commands. It does
 * not choose skills, infer level gates, revive, teleport, or mutate state.
 */
export async function completeQuestObjectives(client, routeQuest, navigateNear, options = {}) {
  validateInputs(client, routeQuest, navigateNear);
  const settings = combatSettings(options);
  const questId = Number(routeQuest.questId);
  const descriptors = objectiveDescriptors(routeQuest);
  let engagements = 0;
  let harvestPasses = 0;
  let searches = 0;
  const unavailableCorpses = new Map();
  const travelThreatEvasions = new Map();
  let spawnRespawnWaits = 0;
  let spawnRespawnProgressKey = null;
  let approachBlockerClears = 0;

  for (; engagements < settings.maxEngagements; engagements += 1) {
    assertPlayerAlive(client);
    if (settings.prepare) await settings.prepare(client);
    const stateQuest = questEntry(client.snapshot, questId);
    if (!stateQuest) throw new Error(`q${questId} is absent from the authoritative quest log`);
    const stage = normalized(stateQuest.stage);
    if (READY_STAGES.has(stage)) {
      return { questId, stage: stateQuest.stage, engagements, harvestPasses, searches };
    }
    if (stage !== "inprogress") throw new Error(`q${questId} is not active (stage=${stateQuest.stage ?? "unknown"})`);

    const pending = nextPendingObjective(client.snapshot, stateQuest, descriptors);
    if (!pending) {
      await waitFor(client, () => READY_STAGES.has(normalized(questEntry(client.snapshot, questId)?.stage)), `q${questId} ready stage`, settings.questSettleTimeout);
      const settled = questEntry(client.snapshot, questId);
      return { questId, stage: settled.stage, engagements, harvestPasses, searches };
    }
    const currentProgressKey = objectiveFingerprint(client.snapshot, questId, pending);
    if (currentProgressKey !== spawnRespawnProgressKey) {
      spawnRespawnProgressKey = currentProgressKey;
      spawnRespawnWaits = 0;
      approachBlockerClears = 0;
    }

    let targetPlan = selectTargetPlan(client.snapshot, pending);
    const currentMapFileName = String(client.snapshot?.mapFileName ?? '');
    const preferConfiguredObjectiveMap = settings.preferObjectiveMapOverCurrent &&
      settings.preferredObjectiveMaps.length > 0 &&
      !settings.preferredObjectiveMaps.includes(currentMapFileName);
    if (preferConfiguredObjectiveMap) targetPlan = null;
    if (!targetPlan && settings.travel) {
      const candidates = pending.kind === 'kill' ? pending.route.spawnCandidates : (pending.route.sources ?? []).flatMap(source => source.spawnCandidates ?? []);
      const groupName = normalizeName(routeQuest?.group);
      const groupMaps = groupName ? candidates
        .filter(candidate => normalizeName(candidate?.mapTitle) === groupName)
        .map(candidate => candidate.mapFileName) : [];
      const destination = await firstReachableDestination(candidates, settings.travel, [
        ...settings.preferredObjectiveMaps,
        routeQuest?.finishNpc?.mapFileName,
        routeQuest?.startNpc?.mapFileName,
        ...groupMaps,
      ]);
      if (destination) {
        recordSearchDiagnostic(client, {
          type: 'chosenObjectiveDestination',
          questId,
          target: pending.name,
          fromMapFileName: String(client.snapshot?.mapFileName ?? ''),
          toMapFileName: String(destination.mapFileName),
        });
        try {
          await settings.travel(destination.mapFileName, {
            // Long dungeon crossings remain ordinary movement, but a rendered
            // monster that has actually struck the player may stop the route
            // so the existing bounded quest-combat path can defend the tile.
            interruptWhen: () => {
              const aggressors = provenAggressors(client, null, settings);
              if (aggressors.length === 0) return false;
              // A healthy player crossing a long dungeon does not stop on the
              // first glancing hit. Doing so lets every slower monster in the
              // entrance AOI converge while the player spends twenty seconds
              // trading blows with one pursuer. Keep running and let normal
              // navigation/potions do their work; an occupied route still
              // becomes TravelBlockedByMonster below, and low health still
              // interrupts into the bounded retreat policy.
              return !settings.continueTravelWhileHealthy ||
                playerHealthRatio(client) <= settings.multiAggressorRetreatRatio;
            },
            // Let this quest loop classify and retreat from a dense travel
            // pack. The global one-monster resolver intentionally serves
            // non-combat trips and cannot see this objective's risk limits.
            resolveBlockingMonster: false,
          });
          travelThreatEvasions.clear();
        } catch (error) {
          if (error instanceof TravelBlockedByMonster) {
            const blocker = entityById(client.snapshot, error.objectId);
            if (!blocker) continue;
            recordSearchDiagnostic(client, {
              type: 'travelBlockerCombat',
              questId,
              target: pending.name,
              mapFileName: String(client.snapshot?.mapFileName ?? ''),
              blockerObjectId: Number(blocker.objectId),
              blockerName: String(blocker.name ?? ''),
              blockerPosition: { x: Number(blocker.x), y: Number(blocker.y) },
            });
            try {
              await killExactMonster(
                client, blocker, pending, navigateNear, settings, true,
              );
              engagements += 1;
            } catch (blockerError) {
              if (blockerError instanceof UnsafeTargetCluster) {
                recordUnsafeTargetCluster(client, questId, pending, blockerError);
                deferUnsafeTarget(client, blockerError, settings);
                await retreatAndRecover(client, navigateNear, settings);
                continue;
              }
              if (blockerError instanceof LostCombatTarget ||
                  blockerError instanceof UnreachableCombatTarget ||
                  blockerError instanceof UnresponsiveCombatTarget) {
                recordDeferredCombatTarget(client, questId, pending, blockerError);
                continue;
              }
              if (!(blockerError instanceof ThreatenedNavigation)) throw blockerError;
              let cleared;
              try {
                cleared = await clearProvenAggressors(
                  client, blocker.objectId, pending, navigateNear, settings,
                  settings.maxEngagements - engagements - 1,
                );
              } catch (clearError) {
                if (isDeferredCombatTargetError(clearError)) {
                  recordDeferredCombatTarget(client, questId, pending, clearError);
                  deferUnreachableTarget(clearError, settings);
                  await retreatAndRecover(client, navigateNear, settings);
                  continue;
                }
                if (!(clearError instanceof UnsafeTargetCluster)) throw clearError;
                // A doorway blocker can attract more attackers after the
                // initial target was classified. Treat that second pack check
                // exactly like every other unsafe combat transition instead
                // of leaking the internal error out of the whole journey.
                recordUnsafeTargetCluster(client, questId, pending, clearError);
                deferUnsafeTarget(client, clearError, settings);
                await retreatAndRecover(client, navigateNear, settings);
                continue;
              }
              if (cleared === 0) throw blockerError;
              engagements += cleared;
            }
            continue;
          }
          if (!(error instanceof TravelInterrupted)) throw error;
          const travelEdge = `${error.fromMapFileName}->${error.toMapFileName}`;
          const priorEvasions = travelThreatEvasions.get(travelEdge) ?? 0;
          // An ordinary player normally keeps moving away from an incidental
          // field monster instead of committing to a long, unrewarded fight.
          // Retreat first so the next route plan can use the remembered tile;
          // only fall back to clearing when the current cave has no safe exit.
          if (!settings.preferTravelAggressorCombat &&
              priorEvasions < settings.maxTravelThreatEvasionsPerEdge) {
            try {
              await retreatAndRecover(client, navigateNear, settings);
              travelThreatEvasions.set(travelEdge, priorEvasions + 1);
              recordSearchDiagnostic(client, {
                type: 'travelThreatEvaded',
                questId,
                target: pending.name,
                mapFileName: String(client.snapshot?.mapFileName ?? ''),
                travelEdge,
              });
              continue;
            } catch (escapeError) {
              if (client.failure) throw client.failure;
              assertPlayerAlive(client);
              recordSearchDiagnostic(client, {
                type: 'travelThreatEvasionFailed',
                questId,
                target: pending.name,
                mapFileName: String(client.snapshot?.mapFileName ?? ''),
                reason: String(escapeError?.message ?? escapeError),
              });
            }
          }
          let cleared;
          try {
            cleared = await clearProvenAggressors(
              client, null, pending, navigateNear, settings,
              settings.maxEngagements - engagements - 1,
            );
          } catch (clearError) {
            if (clearError instanceof UnreachableCombatTarget ||
                clearError instanceof UnresponsiveCombatTarget ||
                clearError instanceof LostCombatTarget) {
              recordDeferredCombatTarget(client, questId, pending, clearError);
              try {
                await retreatAndRecover(client, navigateNear, settings);
                recordSearchDiagnostic(client, {
                  type: 'travelThreatEvaded',
                  questId,
                  target: pending.name,
                  mapFileName: String(client.snapshot?.mapFileName ?? ''),
                  reason: 'aggressorCouldNotBeEngaged',
                });
                continue;
              } catch (escapeError) {
                if (client.failure) throw client.failure;
                assertPlayerAlive(client);
                throw clearError;
              }
            }
            if (!(clearError instanceof UnsafeTargetCluster)) throw clearError;
            recordUnsafeTargetCluster(client, questId, pending, clearError);
            deferUnsafeTarget(client, clearError, settings);
            await retreatAndRecover(client, navigateNear, settings);
            continue;
          }
          if (cleared === 0) {
            // Retreat and live movement can carry the player out of the
            // attacker's adjacent evidence window before the fallback clear
            // begins. With no currently proven aggressor, the interruption has
            // already been resolved and the ordinary route should be planned
            // again rather than rethrowing a stale TravelInterrupted error.
            recordSearchDiagnostic(client, {
              type: 'travelThreatEvaded',
              questId,
              target: pending.name,
              mapFileName: String(client.snapshot?.mapFileName ?? ''),
              travelEdge,
              reason: 'noProvenAggressorAfterInterruption',
            });
            continue;
          }
          recordSearchDiagnostic(client, {
            type: 'travelThreatCleared',
            questId,
            target: pending.name,
            mapFileName: String(client.snapshot?.mapFileName ?? ''),
            count: cleared,
          });
          engagements += cleared;
          continue;
        }
        targetPlan = selectTargetPlan(client.snapshot, pending);
      } else if (typeof settings.travel.canReach === 'function') {
        const maps = uniqueCandidateMaps(candidates);
        throw new Error(`q${questId} ${pending.name} has no reachable monster source among maps ${maps.join(', ') || 'none'}`);
      }
    }
    if (!targetPlan) throw new Error(`q${questId} ${pending.name} has no same-map monster source`);
    let target = nearestAvailableLiveMonster(client, targetPlan.monsterNames, unavailableCorpses, settings);
    if (!target) {
      searches += 1;
      try {
        target = await searchSpawnCandidates(
          client, targetPlan, navigateNear, settings, unavailableCorpses,
        );
      } catch (error) {
        const respawnWaitLimit = spawnRespawnWaitLimit(targetPlan, settings);
        if (error instanceof SpawnSearchExhausted &&
            spawnRespawnWaits < respawnWaitLimit) {
          spawnRespawnWaits += 1;
          recordSearchDiagnostic(client, {
            type: 'spawnRespawnWait',
            questId,
            target: pending.name,
            mapFileName: String(client.snapshot?.mapFileName ?? ''),
            wait: spawnRespawnWaits,
            limit: respawnWaitLimit,
            waitMs: settings.spawnRespawnWaitMs,
          });
          await settings.sleep(settings.spawnRespawnWaitMs);
          if (settings.refreshWhileWaiting) await settings.refreshWhileWaiting(client);
          continue;
        }
        if (!(error instanceof ThreatenedNavigation)) throw error;
        let cleared;
        try {
          cleared = await clearProvenAggressors(
            client, null, pending, navigateNear, settings,
            settings.maxEngagements - engagements - 1,
          );
        } catch (clearError) {
          if (isDeferredCombatTargetError(clearError)) {
            recordDeferredCombatTarget(client, questId, pending, clearError);
            deferUnreachableTarget(clearError, settings);
            await retreatAndRecover(client, navigateNear, settings);
            continue;
          }
          if (!(clearError instanceof UnsafeTargetCluster)) throw clearError;
          recordUnsafeTargetCluster(client, questId, pending, clearError);
          deferUnsafeTarget(client, clearError, settings);
          await retreatAndRecover(client, navigateNear, settings);
          continue;
        }
        if (cleared === 0) throw error;
        engagements += cleared;
        continue;
      }
    }
    if (!target) throw new Error(`q${questId} found no live ${targetPlan.monsterNames.join(" or ")} after bounded spawn search`);

    const before = objectiveFingerprint(client.snapshot, questId, pending);
    if (!settings.focusTargetThroughAggressors) {
      try {
        engagements += await clearProvenAggressors(
          client, target.objectId, pending, navigateNear, settings,
          settings.maxEngagements - engagements - 1,
        );
      } catch (error) {
        if (isDeferredCombatTargetError(error)) {
          recordDeferredCombatTarget(client, questId, pending, error);
          deferUnreachableTarget(error, settings);
          await retreatAndRecover(client, navigateNear, settings);
          continue;
        }
        if (!(error instanceof UnsafeTargetCluster)) throw error;
        recordUnsafeTargetCluster(client, questId, pending, error);
        deferUnsafeTarget(client, error, settings);
        await retreatAndRecover(client, navigateNear, settings);
        continue;
      }
    }
    const afterThreats = questEntry(client.snapshot, questId);
    if (READY_STAGES.has(normalized(afterThreats?.stage))) {
      return { questId, stage: afterThreats.stage, engagements, harvestPasses, searches };
    }
    const engagementSequence = client.sequence ?? 0;
    let corpse;
    let unsafeTarget = null;
    let unreachableTarget = null;
    let approachBlockerCleared = false;
    let focusedTargetRetreated = false;
    while (true) {
      try {
        corpse = await killExactMonster(
          client, target, pending, navigateNear, settings,
          focusedTargetAggressorPolicy(client, target.objectId, settings),
        );
        break;
      } catch (error) {
        if (error instanceof UnreachableCombatTarget ||
            error instanceof UnresponsiveCombatTarget ||
            error instanceof LostCombatTarget) {
          const blocker = error instanceof UnreachableCombatTarget &&
            approachBlockerClears < settings.maxApproachBlockerClears
            ? nearestObjectiveApproachBlocker(
              client.snapshot,
              target,
              settings.approachBlockerSearchRadius,
            )
            : null;
          if (blocker) {
            recordSearchDiagnostic(client, {
              type: 'objectiveApproachBlockerCombat',
              questId,
              target: pending.name,
              objectId: Number(target.objectId),
              blockerObjectId: Number(blocker.objectId),
              blockerName: String(blocker.name ?? ''),
              blockerPosition: { x: Number(blocker.x), y: Number(blocker.y) },
            });
            try {
              await killExactMonster(
                client, blocker, pending, navigateNear, settings, false,
              );
              approachBlockerClears += 1;
              approachBlockerCleared = true;
              break;
            } catch (blockerError) {
              recordSearchDiagnostic(client, {
                type: 'objectiveApproachBlockerCombatFailed',
                questId,
                objectId: Number(target.objectId),
                blockerObjectId: Number(blocker.objectId),
                reason: String(blockerError?.message ?? blockerError),
              });
            }
          }
          unreachableTarget = error;
          recordDeferredCombatTarget(client, questId, pending, error);
          deferUnreachableTarget(error, settings);
          break;
        }
        if (error instanceof UnsafeTargetCluster) {
          unsafeTarget = error;
          recordUnsafeTargetCluster(client, questId, pending, error);
          deferUnsafeTarget(client, error, settings);
          break;
        }
        if (!(error instanceof ThreatenedNavigation)) throw error;
        if (settings.focusTargetThroughAggressors) {
          recordSearchDiagnostic(client, {
            type: 'focusedTargetRetreat',
            questId,
            target: pending.name,
            objectId: Number(target.objectId),
            threatObjectId: error.threatObjectId,
          });
          // The pull was proven unsafe even when the target itself was valid.
          // Quarantine that exact actor through the existing bounded event-gap
          // policy so the next search can choose a different isolated spawn
          // instead of immediately spending another potion cycle on it.
          const targetNames = new Set((targetPlan?.monsterNames ?? [pending.name]).map(normalizeName));
          const hasAlternativeTarget = (client.snapshot?.entities ?? []).some(entity =>
            Number(entity?.objectId) !== Number(target.objectId) && isLiveMonster(entity) &&
            targetNames.has(normalizeName(entity?.name)));
          if (!settings.harvestBeforeClearingAggressors || hasAlternativeTarget) {
            deferUnsafeTarget(client, { objectId: target.objectId }, settings);
          }
          await retreatAndRecover(client, navigateNear, settings);
          focusedTargetRetreated = true;
          break;
        }
        let cleared;
        try {
          cleared = await clearProvenAggressors(
            client, target.objectId, pending, navigateNear, settings,
            settings.maxEngagements - engagements - 1,
          );
        } catch (clearError) {
          if (isDeferredCombatTargetError(clearError)) {
            recordDeferredCombatTarget(client, questId, pending, clearError);
            deferUnreachableTarget(clearError, settings);
            await retreatAndRecover(client, navigateNear, settings);
            focusedTargetRetreated = true;
            break;
          }
          if (!(clearError instanceof UnsafeTargetCluster)) throw clearError;
          unsafeTarget = clearError;
          recordUnsafeTargetCluster(client, questId, pending, clearError);
          deferUnsafeTarget(client, clearError, settings);
          break;
        }
        if (cleared === 0) throw error;
        engagements += cleared;
        // A proven aggressor can itself satisfy the current objective. Trust
        // that authoritative increment and avoid killing the original target
        // after the quest has already advanced.
        if (objectiveAdvanced(client.snapshot, questId, pending, before)) break;
      }
    }
    if (unsafeTarget) {
      await retreatAndRecover(client, navigateNear, settings);
      continue;
    }
    if (approachBlockerCleared) continue;
    if (focusedTargetRetreated) continue;
    if (unreachableTarget) continue;
    if (objectiveAdvanced(client.snapshot, questId, pending, before)) {
      if (settings.afterEngagement) await settings.afterEngagement(client, navigateNear);
      continue;
    }

    const contested = contestedLethalByRemotePlayer(client, corpse?.objectId ?? target.objectId, engagementSequence);
    if (contested) {
      recordSearchDiagnostic(client, {
        type: 'contestedQuestTarget', questId, target: pending.name,
        objectId: Number(corpse?.objectId ?? target.objectId), attackerId: contested.attackerId,
      });
      continue;
    }
    if (pending.kind === "item" && targetPlan.requiresHarvest) {
      if (!settings.harvestBeforeClearingAggressors) {
        try {
          engagements += await clearProvenAggressors(
            client, corpse?.objectId, pending, navigateNear, settings,
            settings.maxEngagements - engagements - 1,
          );
        } catch (error) {
          if (isDeferredCombatTargetError(error)) {
            recordDeferredCombatTarget(client, questId, pending, error);
            deferUnreachableTarget(error, settings);
            await retreatAndRecover(client, navigateNear, settings);
            continue;
          }
          throw error;
        }
      }
      const harvest = await harvestExactCorpse(
        client, corpse, pending, before, navigateNear, settings,
      );
      harvestPasses += harvest.acceptedPasses;
      if (harvest.unavailable) {
        const unavailableObjectId = Number(corpse?.objectId ?? target.objectId);
        unavailableCorpses.set(unavailableObjectId, client.sequence ?? 0);
        recordSearchDiagnostic(client, {
          type: 'unavailableQuestCorpse', questId, target: pending.name,
          objectId: unavailableObjectId, attempts: harvest.attempts,
          distinctUnavailableCorpses: unavailableCorpses.size,
        });
        if (unavailableCorpses.size >= settings.maxUnavailableCorpses) {
          throw new Error(
            `q${questId} ${pending.name} reached unavailable corpse limit ` +
            `${unavailableCorpses.size}/${settings.maxUnavailableCorpses}`,
          );
        }
        continue;
      }
      if (settings.afterEngagement) await settings.afterEngagement(client, navigateNear);
    } else if (pending.kind === "item") {
      await collectVisibleObjectiveDrop(client, pending, before, navigateNear, settings);
      if (settings.afterEngagement) await settings.afterEngagement(client, navigateNear);
    } else {
      try {
        await waitFor(client, () => objectiveAdvanced(client.snapshot, questId, pending, before), `q${questId} ${pending.name} kill progress`, settings.questSettleTimeout);
        if (settings.afterEngagement) await settings.afterEngagement(client, navigateNear);
      } catch (error) {
        const lateContest = contestedLethalByRemotePlayer(client, corpse?.objectId ?? target.objectId, engagementSequence);
        if (!lateContest) throw error;
        recordSearchDiagnostic(client, {
          type: 'contestedQuestTarget', questId, target: pending.name,
          objectId: Number(corpse?.objectId ?? target.objectId), attackerId: lateContest.attackerId,
        });
      }
    }
  }
  throw new Error(`q${questId} combat engagement budget exceeded (${settings.maxEngagements})`);
}

export async function firstReachableDestination(candidates, travel, preferredMaps = []) {
  const destinations = [...new Map((candidates ?? [])
    .filter(spawn => spawn?.mapFileName && !spawn.isBoss)
    .map(spawn => [String(spawn.mapFileName), spawn])).values()];
  if (typeof travel?.routeLength === 'function') {
    const ranked = [];
    const preferred = new Map(
      preferredMaps.filter(Boolean).map((mapFileName, index) => [String(mapFileName), index]),
    );
    for (let index = 0; index < destinations.length; index += 1) {
      const routeLength = await travel.routeLength(destinations[index].mapFileName);
      if (Number.isSafeInteger(routeLength) && routeLength >= 0) {
        ranked.push({
          destination: destinations[index],
          routeLength,
          index,
          preferredIndex: preferred.get(String(destinations[index].mapFileName)) ?? Number.POSITIVE_INFINITY,
        });
      }
    }
    return ranked.sort((left, right) =>
      left.preferredIndex - right.preferredIndex ||
      left.routeLength - right.routeLength ||
      left.index - right.index
    )[0]?.destination ?? null;
  }
  const preferred = new Map(
    preferredMaps.filter(Boolean).map((mapFileName, index) => [String(mapFileName), index]),
  );
  destinations.sort((left, right) =>
    (preferred.get(String(left.mapFileName)) ?? Number.POSITIVE_INFINITY) -
    (preferred.get(String(right.mapFileName)) ?? Number.POSITIVE_INFINITY)
  );
  if (typeof travel?.canReach !== 'function') return destinations[0] ?? null;
  const reachableByMap = new Map();
  for (const destination of destinations) {
    const mapFileName = String(destination.mapFileName);
    if (!reachableByMap.has(mapFileName)) {
      reachableByMap.set(mapFileName, Boolean(await travel.canReach(mapFileName)));
    }
    if (reachableByMap.get(mapFileName)) return destination;
  }
  return null;
}

function uniqueCandidateMaps(candidates) {
  return [...new Set((candidates ?? [])
    .filter(spawn => spawn?.mapFileName && !spawn.isBoss)
    .map(spawn => String(spawn.mapFileName)))];
}

export function liveMonster(snapshot, monsterName) {
  return nearestLiveMonster(snapshot, [monsterName]);
}

/** Clear one live monster that physically seals a normal walking transfer. */
export async function clearTravelBlockingMonster(client, initialTarget, navigateNear, options = {}) {
  if (!client || typeof client.send !== 'function' || typeof client.wait !== 'function') {
    throw new TypeError('client must provide send and wait');
  }
  if (typeof navigateNear !== 'function') throw new TypeError('navigateNear must be a function');
  const objectId = Number(initialTarget?.objectId);
  const target = entityById(client.snapshot, objectId);
  if (!target || !isLiveMonster(target)) return { objectId, cleared: false, reason: 'absent' };
  const settings = combatSettings(options);
  const pending = {
    kind: 'travel',
    name: String(target.name ?? 'monster'),
    ownerQuestId: 0,
    routeIndex: 0,
  };
  const maxApproachBlockers = positiveInteger(options.maxApproachBlockerClears, 3);
  const chain = [objectId];
  const approachBlockers = [];
  while (chain.length > 0) {
    const currentObjectId = chain.at(-1);
    const current = entityById(client.snapshot, currentObjectId);
    if (!current || !isLiveMonster(current)) {
      chain.pop();
      continue;
    }
    try {
      await killExactMonster(
        client,
        current,
        pending,
        navigateNear,
        settings,
        // Reaching town for supplies is itself the survival action. Commit to
        // the current blocker instead of switching on ordinary aggro evidence.
        false,
      );
      chain.pop();
    } catch (error) {
      if (!(error instanceof UnreachableCombatTarget) ||
          approachBlockers.length >= maxApproachBlockers) throw error;
      const blocker = nearestApproachBlockingMonster(
        client.snapshot,
        current,
        new Set(chain),
      );
      if (!blocker) throw error;
      approachBlockers.push(Number(blocker.objectId));
      chain.push(Number(blocker.objectId));
      recordSearchDiagnostic(client, {
        type: 'travelApproachBlockerCombat',
        objectId: Number(current.objectId),
        blockerObjectId: Number(blocker.objectId),
        blockerName: String(blocker.name ?? ''),
        blockerPosition: { x: Number(blocker.x), y: Number(blocker.y) },
      });
    }
  }
  const result = entityById(client.snapshot, objectId);
  return {
    objectId,
    cleared: !result || result?.dead === true || Number(result?.hp) <= 0,
    approachBlockers,
  };
}

function nearestApproachBlockingMonster(snapshot, target, excludedObjectIds) {
  const player = selectPlayer(snapshot);
  if (!player || !validPoint(player) || !validPoint(target)) return null;
  const targetDistance = distance(player, target);
  return (snapshot?.entities ?? [])
    .filter(entity => isLiveMonster(entity) &&
      !excludedObjectIds.has(Number(entity.objectId)) &&
      distance(entity, target) <= 1 &&
      distance(player, entity) < targetDistance)
    .sort((left, right) =>
      distance(player, left) - distance(player, right) ||
      distance(left, target) - distance(right, target) ||
      Number(left.objectId) - Number(right.objectId))[0] ?? null;
}

function nearestObjectiveApproachBlocker(snapshot, target, maximumDistance) {
  const player = selectPlayer(snapshot);
  if (!player || !validPoint(player) || !validPoint(target)) return null;
  const targetDistance = distance(player, target);
  return (snapshot?.entities ?? [])
    .filter(entity => isLiveMonster(entity) &&
      Number(entity.objectId) !== Number(target.objectId) &&
      distance(player, entity) <= maximumDistance &&
      distance(player, entity) < targetDistance)
    .sort((left, right) =>
      distance(player, left) - distance(player, right) ||
      threatHealth(left) - threatHealth(right) ||
      Number(left.objectId) - Number(right.objectId))[0] ?? null;
}

export function harvestDirection(player, corpse) {
  const dx = Math.sign(Number(corpse?.x) - Number(player?.x));
  const dy = Math.sign(Number(corpse?.y) - Number(player?.y));
  if (dx === 0 && dy === 0) {
    const facing = String(player?.direction ?? "");
    return [...DIRECTION_BY_DELTA.values()].includes(facing) ? facing : "Up";
  }
  return DIRECTION_BY_DELTA.get(`${dx},${dy}`) ?? "Up";
}

function combatSettings(options) {
  const requestedLowHealthRetreat = Number(options.lowHealthTargetRetreatRatio);
  const requestedMultiAggressorRetreat = Number(options.multiAggressorRetreatRatio);
  const requestedFinishableTargetHealth = Number(options.finishableTargetHealthRatio);
  const requestedFinishablePlayerHealth = Number(options.finishableTargetMinimumPlayerHpRatio);
  const unsafeRetreatSteps = positiveInteger(options.unsafeRetreatSteps, 8);
  const unsafeRetreatMaxSteps = Math.max(
    unsafeRetreatSteps,
    positiveInteger(options.unsafeRetreatMaxSteps, unsafeRetreatSteps * 3),
  );
  return {
    maxEngagements: positiveInteger(options.maxEngagements, 200),
    maxAttackAttempts: positiveInteger(options.maxAttackAttempts, 120),
    maxHarvestPasses: positiveInteger(options.maxHarvestPasses, 16),
    maxUnavailableCorpses: positiveInteger(options.maxUnavailableCorpses, 3),
    maxSpawnSearches: positiveInteger(options.maxSpawnSearches, 8),
    maxSpawnWaypoints: positiveInteger(options.maxSpawnWaypoints, 10_000),
    spawnSearchTimeoutMs: positiveInteger(options.spawnSearchTimeoutMs, 10 * 60_000),
    maxSpawnRespawnWaits: nonnegativeInteger(options.maxSpawnRespawnWaits, 3),
    profileAwareSpawnRespawnWaits: options.maxSpawnRespawnWaits == null,
    maxProfileSpawnRespawnWaitMs: positiveInteger(
      options.maxProfileSpawnRespawnWaitMs,
      6 * 60_000,
    ),
    spawnRespawnWaitMs: positiveInteger(options.spawnRespawnWaitMs, 30_000),
    spawnSearchMaxNonImprovingSteps: positiveInteger(options.spawnSearchMaxNonImprovingSteps, 96),
    unsafeTargetObservationTimeout: positiveInteger(options.unsafeTargetObservationTimeout, 3_000),
    spawnSearchHostileClearance: nonnegativeInteger(options.spawnSearchHostileClearance, 0),
    spawnSearchHostileClearanceFallback: nonnegativeInteger(
      options.spawnSearchHostileClearanceFallback,
      nonnegativeInteger(options.spawnSearchHostileClearance, 0),
    ),
    combatHostileClearance: nonnegativeInteger(options.combatHostileClearance, 0),
    combatHostileClearanceFallback: nonnegativeInteger(
      options.combatHostileClearanceFallback,
      nonnegativeInteger(options.combatHostileClearance, 0),
    ),
    maxApproachBlockerClears: nonnegativeInteger(options.maxApproachBlockerClears, 0),
    approachBlockerSearchRadius: positiveInteger(options.approachBlockerSearchRadius, 6),
    maxTargetAdjacent: riskLimit(options.maxTargetAdjacent),
    maxTargetNearby: riskLimit(options.maxTargetNearby),
    retreatAtActiveAggressorCount: riskLimit(options.retreatAtActiveAggressorCount),
    unsafeTargetMarks: new Map(),
    unreachableTargetMarks: new Map(),
    unreachableTargetRetryMs: positiveInteger(options.unreachableTargetRetryMs, 30_000),
    unsafeTargetRetryEventGap: positiveInteger(options.unsafeTargetRetryEventGap, 128),
    unsafeRetreatSteps,
    unsafeRetreatMaxSteps,
    unsafeRetreatSafeDistance: positiveInteger(options.unsafeRetreatSafeDistance, 4),
    unsafeRetreatStallSteps: positiveInteger(options.unsafeRetreatStallSteps, 6),
    maxRetreatBreakoutKills: positiveInteger(options.maxRetreatBreakoutKills, 1),
    unsafeRetreatBiasPosition: typeof options.unsafeRetreatBiasPosition === 'function'
      ? options.unsafeRetreatBiasPosition
      : null,
    // Optional public-protocol emergency escape (for example RandomTeleport).
    // It is deliberately disabled unless both a callback and an explicit HP
    // threshold are supplied by the journey policy.
    emergencyEscape: typeof options.emergencyEscape === 'function' ? options.emergencyEscape : null,
    emergencyEscapeHpRatio: boundedRatio(options.emergencyEscapeHpRatio, 0),
    lowHealthTargetRetreatRatio: Number.isFinite(requestedLowHealthRetreat) &&
      requestedLowHealthRetreat >= 0 && requestedLowHealthRetreat <= 1
      ? requestedLowHealthRetreat
      : 0.4,
    multiAggressorRetreatRatio: Number.isFinite(requestedMultiAggressorRetreat) &&
      requestedMultiAggressorRetreat >= 0 && requestedMultiAggressorRetreat <= 1
      ? requestedMultiAggressorRetreat
      : 0.7,
    finishableTargetHealthRatio: Number.isFinite(requestedFinishableTargetHealth) &&
      requestedFinishableTargetHealth >= 0 && requestedFinishableTargetHealth <= 1
      ? requestedFinishableTargetHealth
      : 0,
    finishableTargetMinimumPlayerHpRatio: Number.isFinite(requestedFinishablePlayerHealth) &&
      requestedFinishablePlayerHealth >= 0 && requestedFinishablePlayerHealth <= 1
      ? requestedFinishablePlayerHealth
      : 0.4,
    attackResponseTimeout: positiveInteger(options.attackResponseTimeout, 3_000),
    questSettleTimeout: positiveInteger(options.questSettleTimeout, 6_000),
    spawnObservationTimeout: positiveInteger(options.spawnObservationTimeout, 3_000),
    cannibalPlantRevealWaitMs: Math.max(2_001, positiveInteger(options.cannibalPlantRevealWaitMs, 2_100)),
    attackCadenceMs: nonnegativeInteger(options.attackCadenceMs, 650),
    harvestCadenceMs: nonnegativeInteger(options.harvestCadenceMs, 2_100),
    sustainCadenceMs: nonnegativeInteger(options.sustainCadenceMs, 2_000),
    aggressorEvidenceWindowMs: positiveInteger(options.aggressorEvidenceWindowMs, 5_000),
    aggressorEvidenceEvents: positiveInteger(options.aggressorEvidenceEvents, 128),
    targetLossSettleMs: positiveInteger(options.targetLossSettleMs, 3_000),
    approachRange: typeof options.approachRange === "function" ? options.approachRange : () => 1,
    maxCooldownWaitMs: positiveInteger(options.maxCooldownWaitMs, 30_000),
    maxMovingTargetNoProgressMs: nonnegativeInteger(options.maxMovingTargetNoProgressMs, 0),
    cooldownRefreshIntervalMs: positiveInteger(options.cooldownRefreshIntervalMs, 1_000),
    refreshWhileWaiting: typeof options.refreshWhileWaiting === "function" ? options.refreshWhileWaiting : null,
    sleep: typeof options.sleep === "function" ? options.sleep : realDelay,
    prepare: options.prepare,
    sustain: typeof options.sustain === "function" ? options.sustain : null,
    lastSustainAt: Number.NEGATIVE_INFINITY,
    action: options.action,
    travel: options.travel,
    afterEngagement: options.afterEngagement,
    recoverAfterUnsafeRetreat: typeof options.recoverAfterUnsafeRetreat === 'function'
      ? options.recoverAfterUnsafeRetreat
      : null,
    focusTargetThroughAggressors: options.focusTargetThroughAggressors === true,
    preferredObjectiveMaps: Array.isArray(options.preferredObjectiveMaps)
      ? options.preferredObjectiveMaps.map(String).filter(Boolean)
      : [],
    preferObjectiveMapOverCurrent: options.preferObjectiveMapOverCurrent === true,
    preferTravelAggressorCombat: options.preferTravelAggressorCombat === true,
    continueTravelWhileHealthy: options.continueTravelWhileHealthy === true,
    maxTravelThreatEvasionsPerEdge: positiveInteger(options.maxTravelThreatEvasionsPerEdge, 1),
    allowLowHealthFollowerRecovery: options.allowLowHealthFollowerRecovery === true,
    harvestBeforeClearingAggressors: options.harvestBeforeClearingAggressors === true,
    now: typeof options.now === 'function' ? options.now : Date.now,
  };
}

function spawnRespawnWaitLimit(targetPlan, settings) {
  if (!settings.profileAwareSpawnRespawnWaits) return settings.maxSpawnRespawnWaits;
  const delayMs = (targetPlan?.spawns ?? [])
    .map(spawn => Number(spawn?.delayMinutes) * 60_000)
    .filter(value => Number.isFinite(value) && value > 0)
    .sort((left, right) => left - right)[0];
  if (!Number.isFinite(delayMs)) return settings.maxSpawnRespawnWaits;
  const profiledWaits = Math.ceil(
    Math.min(delayMs, settings.maxProfileSpawnRespawnWaitMs) /
      settings.spawnRespawnWaitMs,
  );
  return Math.max(settings.maxSpawnRespawnWaits, profiledWaits);
}

function validateInputs(client, routeQuest, navigateNear) {
  if (!client || typeof client.send !== "function" || typeof client.wait !== "function") throw new TypeError("client must provide send and wait");
  if (!Number.isSafeInteger(Number(routeQuest?.questId)) || Number(routeQuest.questId) <= 0) throw new TypeError("routeQuest.questId must be a positive integer");
  if (typeof navigateNear !== "function") throw new TypeError("navigateNear must be a function");
}

function objectiveDescriptors(routeQuest) {
  const ownerQuestId = Number(routeQuest.questId);
  const kill = (routeQuest?.objectives?.kill ?? []).map((entry, index) => ({
    kind: "kill", name: String(entry.monsterName), route: entry, routeIndex: index, ownerQuestId,
  }));
  const item = (routeQuest?.objectives?.item ?? []).map((entry, index) => ({
    kind: "item", name: String(entry.itemName), route: entry, routeIndex: kill.length + index, ownerQuestId,
  }));
  return [...kill, ...item];
}

function nextPendingObjective(snapshot, stateQuest, descriptors) {
  const pending = [];
  for (const descriptor of descriptors) {
    const objective = snapshotObjective(stateQuest, descriptor);
    if (!objective) throw new Error(`q${stateQuest.questId} snapshot omitted objective ${descriptor.name}`);
    if (!objectiveDone(objective)) pending.push({ ...descriptor, snapshot: objective });
  }
  // Multi-objective Crystal quests often span adjacent dungeon floors. Finish
  // any objective with an authoritative source on the current map before
  // crossing another full map. Besides reducing backtracking, this lets normal
  // quest combat clear the monsters physically occupying a required corridor.
  return pending.find(descriptor => selectTargetPlan(snapshot, descriptor)) ?? pending[0] ?? null;
}

function snapshotObjective(stateQuest, descriptor) {
  const objectives = Array.isArray(stateQuest?.objectives) ? stateQuest.objectives : [];
  const wanted = normalizeName(descriptor.name);
  return objectives.find((entry) => {
    const label = normalizeName(entry?.label);
    return label && (label.includes(wanted) || wanted.includes(label));
  }) ?? objectives[descriptor.routeIndex] ?? (
    objectives.length === 0 && descriptor.routeIndex === 0 && stateQuest?.current != null
      ? { current: stateQuest.current, required: stateQuest.required, done: false }
      : null
  );
}

function objectiveDone(objective) {
  const required = Number(objective?.required);
  return objective?.done === true || (Number.isFinite(required) && required > 0 && Number(objective?.current ?? 0) >= required);
}

function selectTargetPlan(snapshot, pending) {
  const currentMap = String(snapshot?.mapFileName ?? "");
  if (pending.kind === "kill") {
    const spawns = sameMapSpawns(pending.route.spawnCandidates, currentMap);
    return spawns.length ? { monsterNames: [pending.route.monsterName], requiresHarvest: false, spawns } : null;
  }
  const sources = (pending.route.sources ?? []).map((source) => ({
    source,
    spawns: sameMapSpawns(source.spawnCandidates, currentMap),
  })).filter((entry) => entry.spawns.length > 0);
  if (!sources.length) return null;

  const visible = (snapshot?.entities ?? []).filter((entity) => isLiveMonster(entity));
  const visibleSource = sources
    .map((entry) => ({ ...entry, monster: visible.find((entity) => sameName(entity.name, entry.source.monsterName)) }))
    .filter((entry) => entry.monster)
    .sort((left, right) => distance(playerFromSnapshot(snapshot), left.monster) - distance(playerFromSnapshot(snapshot), right.monster))[0];
  const selected = visibleSource ?? sources
    .map((entry) => ({
      ...entry,
      nearest: nearestSpawnDistance(snapshot, entry.spawns),
      searchEffort: sourceSearchEffort(snapshot, entry.spawns),
    }))
    .sort((left, right) => left.searchEffort - right.searchEffort || left.nearest - right.nearest)[0];
  return {
    monsterNames: [String(selected.source.monsterName)],
    requiresHarvest: selected.source.requiresHarvest === true,
    spawns: selected.spawns,
  };
}

function sameMapSpawns(spawns, currentMap) {
  return (spawns ?? []).filter((spawn) => String(spawn?.mapFileName) === currentMap && validPoint(spawn?.position));
}

function nearestSpawnDistance(snapshot, spawns) {
  const player = playerFromSnapshot(snapshot);
  return Math.min(...spawns.map((spawn) => distance(player, spawn.position)));
}

function sourceSearchEffort(snapshot, spawns) {
  const player = playerFromSnapshot(snapshot);
  return Math.min(...spawns.map((spawn) => {
    const spread = Math.max(0, Number(spawn?.spread ?? 0));
    const count = Math.max(1, Number(spawn?.count ?? 1));
    const delayMinutes = Math.max(0, Number(spawn?.delayMinutes ?? 0));
    // Distance to the spawn footprint estimates the travel leg. Expected
    // spacing and respawn delay then keep a nearby single rare variant from
    // beating a slightly farther field containing dozens of valid sources.
    const distanceToFootprint = Math.max(0, distance(player, spawn.position) - spread);
    const expectedSpacing = spread / Math.sqrt(count);
    return distanceToFootprint + expectedSpacing + delayMinutes * 2;
  }));
}

async function searchSpawnCandidates(
  client, targetPlan, navigateNear, settings, unavailableCorpses = new Map(),
) {
  const origin = playerFromSnapshot(client.snapshot);
  const candidates = [...new Map(targetPlan.spawns.map(spawn => [spawnKey(spawn), spawn])).values()]
    .sort((left, right) =>
      Number(spawnHasMatchingCorpse(client.snapshot, left, targetPlan.monsterNames)) -
        Number(spawnHasMatchingCorpse(client.snapshot, right, targetPlan.monsterNames)) ||
      distance(origin, left.position) - distance(origin, right.position)
    )
    .slice(0, settings.maxSpawnSearches);
  const bounds = knownMapBounds(client.snapshot);
  const remembered = rememberedTargetLocations(client, targetPlan.monsterNames);
  const coverage = interleavedSpawnWaypoints(candidates, bounds);
  const rememberedWaypoints = [];
  const coverageWaypoints = [];
  const waypointKeys = new Set();
  for (const waypoint of remembered) {
    const key = `${waypoint.x},${waypoint.y}`;
    if (waypointKeys.has(key)) continue;
    waypointKeys.add(key);
    rememberedWaypoints.push(waypoint);
  }
  for (const waypoint of coverage) {
    const key = `${waypoint.x},${waypoint.y}`;
    if (waypointKeys.has(key)) continue;
    waypointKeys.add(key);
    coverageWaypoints.push(waypoint);
  }
  // Revisit observed locations first, then keep each subsequent walk local.
  // Radius-first interleaving made the player cross the whole map repeatedly
  // between distant spawn declarations and could spend the entire wall-clock
  // budget before reaching an outer point in any one field.
  const orderedRemembered = nearestNeighborWaypoints(origin, rememberedWaypoints);
  const rememberedKeys = new Set(orderedRemembered.map(point => `${point.x},${point.y}`));
  const revealAwareCannibalPlant = targetPlan.monsterNames.some(name => normalizeName(name) === 'cannibalplant');
  const coverageOrigin = orderedRemembered.at(-1) ?? origin;
  const allWaypoints = [
    ...orderedRemembered,
    ...nearestNeighborWaypoints(coverageOrigin, coverageWaypoints),
  ];
  const waypoints = allWaypoints.slice(0, settings.maxSpawnWaypoints);
  const startedAt = settings.now();
  let visited = 0;
  let sawUnsafeTarget = false;
  const observeSafeTarget = () => {
    const safeTarget = nearestAvailableLiveMonster(
      client, targetPlan.monsterNames, unavailableCorpses, settings,
    );
    if (!safeTarget && nearestLiveMonster(client.snapshot, targetPlan.monsterNames)) {
      sawUnsafeTarget = true;
    }
    return safeTarget;
  };
  recordSearchDiagnostic(client, {
    type: 'spawnSearchStart',
    mapFileName: String(client.snapshot?.mapFileName ?? ''),
    targets: targetPlan.monsterNames,
    candidates: candidates.length,
    totalWaypoints: allWaypoints.length,
    waypointBudget: settings.maxSpawnWaypoints,
    timeoutMs: settings.spawnSearchTimeoutMs,
  });

  for (const waypoint of waypoints) {
    assertSpawnSearchTime(client, targetPlan, startedAt, visited, allWaypoints.length, settings);
    assertPlayerAlive(client);
    const immediate = observeSafeTarget();
    if (immediate) return immediate;

    const rememberedCannibalPlant = revealAwareCannibalPlant && rememberedKeys.has(`${waypoint.x},${waypoint.y}`);
    let threatened = false;
    const stopSearchNavigation = () => {
      assertSpawnSearchTime(client, targetPlan, startedAt, visited, allWaypoints.length, settings);
      // Prefer the objective when both it and a threat appear together. The
      // existing pre-engagement pass clears the threat before that target.
      if (observeSafeTarget()) return true;
      threatened = provenAggressors(client, null, settings).length > 0;
      return threatened;
    };
    const navigateSearchWaypoint = hostileAvoidanceRadius => navigateNear(
      waypoint,
      rememberedCannibalPlant ? 2 : 4,
      stopSearchNavigation,
      {
        hostileAvoidanceRadius,
        // A fresh snapshot cannot open a route blocked by the same visible
        // hostile buffer quickly enough to justify three one-second retries.
        // The bounded fallback below is the deliberate second attempt.
        maxNoPathRefreshes: 0,
        detectPositionCycles: true,
        maxNonImprovingSteps: settings.spawnSearchMaxNonImprovingSteps,
      },
    );
    try {
      // CannibalPlant only reveals on its two-second server check while a
      // player is within Chebyshev radius three. A remembered exact location
      // is strong enough to justify one close observation; coverage points are
      // not, so the full-spread search keeps its existing time budget.
      await navigateSearchWaypoint(settings.spawnSearchHostileClearance);
    } catch (error) {
      assertPlayerAlive(client);
      if (!isRecoverableSearchNavigationError(error)) throw error;
      await interruptThreatenedSearchAfterNavigationFailure(client, targetPlan, settings);
      const fallback = Math.min(
        settings.spawnSearchHostileClearance,
        settings.spawnSearchHostileClearanceFallback,
      );
      if (fallback >= settings.spawnSearchHostileClearance) {
        visited += 1;
        recordSpawnSearchProgress(client, targetPlan, visited, allWaypoints.length);
        continue;
      }
      recordSearchDiagnostic(client, {
        type: 'spawnSearchClearanceFallback',
        from: settings.spawnSearchHostileClearance,
        to: fallback,
        waypoint: { x: Number(waypoint.x), y: Number(waypoint.y) },
        reason: error?.code ?? error?.reason ?? String(error?.message ?? ''),
      });
      try {
        await navigateSearchWaypoint(fallback);
      } catch (fallbackError) {
        assertPlayerAlive(client);
        if (!isRecoverableSearchNavigationError(fallbackError)) throw fallbackError;
        await interruptThreatenedSearchAfterNavigationFailure(client, targetPlan, settings);
        visited += 1;
        recordSpawnSearchProgress(client, targetPlan, visited, allWaypoints.length);
        continue;
      }
    }
    visited += 1;
    recordSpawnSearchProgress(client, targetPlan, visited, allWaypoints.length);
    let seen = observeSafeTarget();
    if (seen) return seen;
    if (threatened) {
      throw new ThreatenedNavigation(
        `spawn search for ${targetPlan.monsterNames.join(" or ")} interrupted by a proven adjacent aggressor`,
      );
    }
    if (rememberedCannibalPlant) {
      await settings.sleep(settings.cannibalPlantRevealWaitMs);
      assertSpawnSearchTime(client, targetPlan, startedAt, visited, allWaypoints.length, settings);
      try {
        seen = await waitFor(
          client,
        observeSafeTarget,
          'live CannibalPlant after remembered-location reveal check',
          settings.spawnObservationTimeout,
        );
      } catch {
        // A remembered coordinate remains only a hint when no live packet arrives.
      }
      if (seen) return seen;
    }
  }
  assertSpawnSearchTime(client, targetPlan, startedAt, visited, allWaypoints.length, settings);
  try {
    const found = await waitFor(client, observeSafeTarget, `live ${targetPlan.monsterNames.join(" or ")} at spawn`, settings.spawnObservationTimeout);
    if (found) return found;
  } catch {
    // The bounded waypoint search is exhausted.
  }
  if (sawUnsafeTarget) {
    recordSearchDiagnostic(client, {
      type: 'unsafeTargetObservation',
      mapFileName: String(client.snapshot?.mapFileName ?? ''),
      targets: targetPlan.monsterNames,
      timeoutMs: settings.unsafeTargetObservationTimeout,
    });
    let threatened = false;
    try {
      await waitFor(
        client,
        () => {
          assertPlayerAlive(client);
          if (observeSafeTarget()) return true;
          threatened = provenAggressors(client, null, settings).length > 0;
          return threatened;
        },
        `safe ${targetPlan.monsterNames.join(" or ")} after pack dispersal`,
        settings.unsafeTargetObservationTimeout,
      );
      const dispersed = observeSafeTarget();
      if (dispersed) return dispersed;
      if (threatened) {
        throw new ThreatenedNavigation(
          `unsafe-target observation for ${targetPlan.monsterNames.join(" or ")} interrupted by a proven adjacent aggressor`,
        );
      }
    } catch (error) {
      // Never turn a death or a proven attack into an ordinary bounded search
      // failure. The caller can clear/retreat immediately while the player is
      // still alive; only a harmless observation timeout falls through.
      assertPlayerAlive(client);
      if (threatened || error instanceof ThreatenedNavigation) throw error;
    }
  }
  if (allWaypoints.length > waypoints.length) {
    throw new Error(`bounded full-spread spawn search waypoint budget exhausted (${waypoints.length}/${allWaypoints.length}) for ${targetPlan.monsterNames.join(" or ")}`);
  }
  throw new SpawnSearchExhausted(`bounded full-spread spawn search exhausted ${waypoints.length} waypoints across ${candidates.length} candidates without live ${targetPlan.monsterNames.join(" or ")}`);
}

async function interruptThreatenedSearchAfterNavigationFailure(client, targetPlan, settings) {
  // A pathfinder failure does not mean the world is idle. Keep survival
  // actions running while the search is blocked, then hand a proven adjacent
  // attacker back to the normal bounded clear/retreat flow immediately. Without
  // this check the candidate loop can enumerate thousands of unreachable cave
  // points while the player stands still and dies with a full potion stack.
  await sustainIfDue(client, settings, 'search', null);
  assertPlayerAlive(client);
  const threats = provenAggressors(client, null, settings);
  if (threats.length === 0) return;
  throw new ThreatenedNavigation(
    `spawn search for ${targetPlan.monsterNames.join(" or ")} failed while under a proven adjacent attack`,
    threats[0].objectId,
  );
}

function assertSpawnSearchTime(client, targetPlan, startedAt, visited, total, settings) {
  const elapsed = Math.max(0, settings.now() - startedAt);
  if (elapsed < settings.spawnSearchTimeoutMs) return;
  const map = String(client.snapshot?.mapFileName ?? 'unknown');
  throw new Error(`bounded full-spread spawn search timed out after ${elapsed}ms; visited ${visited}/${total} waypoints on map ${map} for ${targetPlan.monsterNames.join(" or ")}`);
}

function recordSpawnSearchProgress(client, targetPlan, visited, total) {
  if (visited % 25 !== 0) return;
  recordSearchDiagnostic(client, {
    type: 'spawnSearchProgress',
    mapFileName: String(client.snapshot?.mapFileName ?? ''),
    targets: targetPlan.monsterNames,
    visitedWaypoints: visited,
    totalWaypoints: total,
  });
}

function recordSearchDiagnostic(client, payload) {
  if (typeof client?.record === 'function') client.record('diagnostic', payload);
}

function interleavedSpawnWaypoints(spawns, bounds) {
  const centers = spawns.map(spawn => clipPoint(spawn.position, bounds));
  const coverage = spawns.map((spawn, candidateIndex) => {
    const center = { x: Number(spawn.position.x), y: Number(spawn.position.y) };
    const spread = Math.max(0, Math.ceil(Number(spawn.spread ?? 0)));
    const xs = coverageAxis(center.x, spread, bounds?.minX, bounds?.maxX);
    const ys = coverageAxis(center.y, spread, bounds?.minY, bounds?.maxY);
    return xs.flatMap(x => ys.map(y => ({
      x,
      y,
      candidateIndex,
      radius: Math.max(Math.abs(x - center.x), Math.abs(y - center.y)),
    })));
  });
  const waypoints = [...centers];
  const remaining = coverage.flat().sort((left, right) =>
    left.radius - right.radius ||
    left.candidateIndex - right.candidateIndex ||
    left.y - right.y || left.x - right.x
  );
  waypoints.push(...remaining.map(({ x, y }) => ({ x, y })));
  return waypoints;
}

function nearestNeighborWaypoints(origin, waypoints) {
  const remaining = waypoints.map((point, index) => ({ point, index }));
  const ordered = [];
  let cursor = origin;
  while (remaining.length > 0) {
    let nearestIndex = 0;
    let nearestDistance = distance(cursor, remaining[0].point);
    for (let index = 1; index < remaining.length; index += 1) {
      const candidateDistance = distance(cursor, remaining[index].point);
      if (candidateDistance < nearestDistance ||
          (candidateDistance === nearestDistance && remaining[index].index < remaining[nearestIndex].index)) {
        nearestIndex = index;
        nearestDistance = candidateDistance;
      }
    }
    const [{ point }] = remaining.splice(nearestIndex, 1);
    ordered.push(point);
    cursor = point;
  }
  return ordered;
}

function coverageAxis(center, spread, minimum, maximum) {
  const low = Math.ceil(minimum == null ? center - spread : Math.max(center - spread, minimum));
  const high = Math.floor(maximum == null ? center + spread : Math.min(center + spread, maximum));
  if (low > high) return [];
  const values = [];
  for (let value = low; value <= high; value += 20) values.push(value);
  if (values.at(-1) !== high) values.push(high);
  return values;
}

function rememberedTargetLocations(client, monsterNames) {
  const wanted = new Set(monsterNames.map(normalizeName));
  const points = [];
  for (const entity of client.snapshot?.entities ?? []) {
    if (normalized(entity?.kind) === 'monster' && wanted.has(normalizeName(entity?.name)) && validPoint(entity)) {
      points.push({ x: Number(entity.x), y: Number(entity.y) });
    }
  }
  const currentMap = String(client.snapshot?.mapFileName ?? '');
  let boundary = -1;
  for (let index = (client.events ?? []).length - 1; index >= 0; index -= 1) {
    const event = client.events[index];
    // A fresh same-map snapshot updates authority but does not invalidate
    // monster locations observed earlier in this visit. Reset hints only when
    // the client actually enters the map; otherwise every quest interaction or
    // combat refresh makes the next target search blind again.
    if (['MapChanged', 'MapInformation'].includes(event?.packet) &&
        String(event?.payload?.fileName ?? '') === currentMap) {
      boundary = index;
      break;
    }
  }
  for (const event of (client.events ?? []).slice(boundary + 1)) {
    if (!['ObjectMonster', 'NewMonsterInfo'].includes(event?.packet) || !wanted.has(normalizeName(event?.payload?.name))) continue;
    const point = packetPoint(event.payload);
    if (point) points.push(point);
  }
  for (const observation of client.observedMonsterLocations ?? []) {
    if (String(observation?.mapFileName ?? '') !== currentMap || !wanted.has(normalizeName(observation?.monsterName))) continue;
    if (validPoint(observation)) points.push({ x: Number(observation.x), y: Number(observation.y) });
  }
  return points.sort((left, right) => distance(playerFromSnapshot(client.snapshot), left) - distance(playerFromSnapshot(client.snapshot), right));
}

function knownMapBounds(snapshot) {
  const source = snapshot?.mapBounds ?? snapshot?.map_bounds;
  const width = Number(snapshot?.mapWidth ?? snapshot?.map_width ?? snapshot?.mapSize?.width ?? snapshot?.map_size?.width);
  const height = Number(snapshot?.mapHeight ?? snapshot?.map_height ?? snapshot?.mapSize?.height ?? snapshot?.map_size?.height);
  const values = {
    minX: finiteNumber(source?.minX ?? source?.min_x) ?? (Number.isFinite(width) ? 0 : null),
    maxX: finiteNumber(source?.maxX ?? source?.max_x) ?? (Number.isFinite(width) ? width - 1 : null),
    minY: finiteNumber(source?.minY ?? source?.min_y) ?? (Number.isFinite(height) ? 0 : null),
    maxY: finiteNumber(source?.maxY ?? source?.max_y) ?? (Number.isFinite(height) ? height - 1 : null),
  };
  return Object.values(values).some(value => value != null) ? values : null;
}

function clipPoint(point, bounds) {
  return {
    x: clipCoordinate(Number(point.x), bounds?.minX, bounds?.maxX),
    y: clipCoordinate(Number(point.y), bounds?.minY, bounds?.maxY),
  };
}

function clipCoordinate(value, minimum, maximum) {
  return Math.max(minimum ?? value, Math.min(maximum ?? value, value));
}

function packetPoint(payload) {
  const x = finiteNumber(payload?.x ?? payload?.location?.x);
  const y = finiteNumber(payload?.y ?? payload?.location?.y);
  return x == null || y == null ? null : { x, y };
}

function spawnHasMatchingCorpse(snapshot, spawn, monsterNames) {
  const wanted = new Set(monsterNames.map(normalizeName));
  const spread = Math.max(0, Number(spawn.spread ?? 0));
  return (snapshot?.entities ?? []).some(entity =>
    normalized(entity?.kind) === 'monster' &&
    wanted.has(normalizeName(entity?.name)) &&
    (entity?.dead === true || (entity?.hp != null && Number(entity.hp) <= 0)) &&
    validPoint(entity) &&
    Math.max(Math.abs(Number(entity.x) - Number(spawn.position.x)), Math.abs(Number(entity.y) - Number(spawn.position.y))) <= spread
  );
}

async function killExactMonster(client, initialTarget, pending, navigateNear, settings, aggressorInterruptPolicy = true) {
  const objectId = Number(initialTarget.objectId);
  if (!Number.isSafeInteger(objectId) || objectId <= 0) throw new Error(`q${questIdFor(pending)} target has no valid objectId`);
  const startingProgress = objectiveFingerprint(
    client.snapshot,
    questIdFor(pending),
    pending,
  );
  const completedMissingTarget = fallback => objectiveAdvanced(
    client.snapshot,
    questIdFor(pending),
    pending,
    startingProgress,
  ) ? { ...fallback, objectId, dead: true, hp: 0 } : null;
  let noResponse = 0;
  let cooldownWaitMs = 0;
  let cooldownWaitSinceRefreshMs = 0;
  let lastAuthoritativeProgressAt = settings.now();
  for (let attempt = 0; attempt < settings.maxAttackAttempts; attempt += 1) {
    assertPlayerAlive(client);
    let target = entityById(client.snapshot, objectId);
    if (target?.dead === true || Number(target?.hp) <= 0) return target;
    if (!target) target = await settleMissingTarget(client, objectId, settings);
    if (!target) {
      const completed = completedMissingTarget(initialTarget);
      if (completed) return completed;
      throw new LostCombatTarget(objectId, initialTarget);
    }
    throwIfLowHealthTargetPressure(client, target, pending, settings);
    let approachRange = await combatApproachRange(settings, client, target);
    await approachCombatTarget(
      client, target, approachRange, objectId, navigateNear, settings, aggressorInterruptPolicy,
    );
    let refreshed = entityById(client.snapshot, objectId);
    if (!refreshed) refreshed = await settleMissingTarget(client, objectId, settings);
    if (!refreshed) {
      const completed = completedMissingTarget(target);
      if (completed) return completed;
      throw new LostCombatTarget(objectId, target);
    }
    if (refreshed.dead === true || Number(refreshed.hp) <= 0) return refreshed;
    if (distance(playerFromSnapshot(client.snapshot), refreshed) > approachRange) continue;
    throwIfUnsafeTargetPack(client, refreshed, settings);
    await sustainIfDue(client, settings, 'attack', objectId);
    assertPlayerAlive(client);
    let actionable = entityById(client.snapshot, objectId);
    if (!actionable) {
      const completed = completedMissingTarget(refreshed);
      if (completed) return completed;
      throw new LostCombatTarget(objectId, refreshed);
    }
    if (actionable.dead === true || Number(actionable.hp) <= 0) return actionable;
    throwIfLowHealthTargetPressure(client, actionable, pending, settings);
    approachRange = await combatApproachRange(settings, client, actionable);
    if (distance(playerFromSnapshot(client.snapshot), actionable) > approachRange) {
      await approachCombatTarget(
        client, actionable, approachRange, objectId, navigateNear, settings, aggressorInterruptPolicy,
      );
      actionable = entityById(client.snapshot, objectId);
      if (!actionable) {
        const completed = completedMissingTarget(refreshed);
        if (completed) return completed;
        throw new LostCombatTarget(objectId, refreshed);
      }
      if (actionable.dead === true || Number(actionable.hp) <= 0) return actionable;
    }
    if (distance(playerFromSnapshot(client.snapshot), actionable) > approachRange) continue;
    throwIfUnsafeTargetPack(client, actionable, settings);
    const aggressorBeforeAttack = interruptingAggressor(
      client, objectId, settings, aggressorInterruptPolicy,
    );
    if (aggressorBeforeAttack) {
      throw new ThreatenedNavigation(
        `attack on target ${objectId} interrupted by a proven adjacent aggressor`,
        aggressorBeforeAttack.objectId,
      );
    }
    const beforeHp = finiteHp(actionable.hp);
    const beforePercent = finiteHp(actionable.healthPercent);
    const beforePosition = validPoint(actionable)
      ? { x: Number(actionable.x), y: Number(actionable.y) }
      : null;
    const beforeProgress = objectiveFingerprint(client.snapshot, questIdFor(pending), pending);
    const afterSequence = client.sequence ?? 0;
    if (settings.action) {
      const action = await settings.action(client, actionable);
      if (action?.kind === "wait") {
        if (cooldownWaitMs >= settings.maxCooldownWaitMs) {
          throw new Error(`target ${objectId} cooldown stayed unavailable for ${settings.maxCooldownWaitMs}ms without an attack`);
        }
        const delayMs = Math.min(
          settings.attackResponseTimeout,
          positiveInteger(action.delayMs, Math.max(1, settings.attackCadenceMs)),
          settings.maxCooldownWaitMs - cooldownWaitMs,
        );
        await settings.sleep(delayMs);
        cooldownWaitMs += delayMs;
        cooldownWaitSinceRefreshMs += delayMs;
        if (settings.refreshWhileWaiting && cooldownWaitSinceRefreshMs >= settings.cooldownRefreshIntervalMs) {
          const beforeRefreshSequence = client.sequence ?? 0;
          await settings.refreshWhileWaiting(client);
          if (!(client.events ?? []).some(event =>
            event.sequence > beforeRefreshSequence && event.direction === "received" && event.type === "worldSnapshot")) {
            throw new Error(`target ${objectId} cooldown refresh produced no authoritative worldSnapshot`);
          }
          cooldownWaitSinceRefreshMs %= settings.cooldownRefreshIntervalMs;
        }
        // A cooldown-only wait sent no combat command and therefore must not
        // consume one of the bounded authoritative attack attempts.
        attempt -= 1;
        continue;
      }
      if (action.targetId === client.snapshot.playerObjectId) { await settings.sleep(1000); continue; }
    } else client.send({ type: "attack", objectId });
    try {
      await waitFor(client, () => {
        assertPlayerAlive(client);
        const live = entityById(client.snapshot, objectId);
        // A target hit may take the full response timeout to arrive. Do not
        // spend those three seconds standing still after a different adjacent
        // monster has already proved that it is attacking the player. Wake the
        // response wait now; the common post-attack check below will enter the
        // existing bounded retreat path.
        const responseAggressor = interruptingAggressor(
          client, objectId, settings, aggressorInterruptPolicy,
        );
        return live?.dead === true || Number(live?.hp) <= 0 ||
          (beforeHp != null && finiteHp(live?.hp) != null && finiteHp(live.hp) < beforeHp) ||
          (beforePercent != null && finiteHp(live?.healthPercent) != null && live.healthPercent < beforePercent) ||
          objectiveAdvanced(client.snapshot, questIdFor(pending), pending, beforeProgress) ||
          receivedPacketAfter(client, afterSequence, "ObjectDied", objectId) ||
          responseAggressor != null;
      }, `target ${objectId} combat response`, settings.attackResponseTimeout);
      noResponse = 0;
    } catch (error) {
      const movedTarget = entityById(client.snapshot, objectId);
      if (beforePosition && movedTarget && validPoint(movedTarget) &&
          distance(beforePosition, movedTarget) > 0) {
        // Passive field animals can leave melee range while the attack lock is
        // resolving. That is movement authority, not proof that the target is
        // unresponsive. Re-approach its current tile and reserve the three-hit
        // quarantine for a stationary actor that ignores real attacks.
        noResponse = 0;
        if (settings.maxMovingTargetNoProgressMs > 0 &&
            settings.now() - lastAuthoritativeProgressAt >= settings.maxMovingTargetNoProgressMs) {
          throw new UnresponsiveCombatTarget(objectId, target, attempt + 1, error);
        }
        recordSearchDiagnostic(client, {
          type: 'combatTargetMovedBeforeHit',
          objectId,
          from: beforePosition,
          to: { x: Number(movedTarget.x), y: Number(movedTarget.y) },
        });
        continue;
      }
      if (++noResponse >= 3) {
        throw new UnresponsiveCombatTarget(objectId, target, noResponse, error);
      }
    }
    lastAuthoritativeProgressAt = settings.now();
    const result = entityById(client.snapshot, objectId);
    if (result?.dead === true || Number(result?.hp) <= 0 || receivedPacketAfter(client, afterSequence, "ObjectDied", objectId)) return result ?? { ...actionable, dead: true, hp: 0 };
    throwIfLowHealthTargetPressure(client, result ?? actionable, pending, settings);
    // Monsters can converge only after navigation has reached attack range. A
    // fast target hit must not hide the fact that another adjacent monster hit
    // the player during the same response window; interrupt before the next
    // target swing so the outer loop can retreat or clear that aggressor.
    const aggressorAfterAttack = interruptingAggressor(
      client, objectId, settings, aggressorInterruptPolicy,
    );
    if (aggressorAfterAttack) {
      throw new ThreatenedNavigation(
        `attack on target ${objectId} interrupted by a proven adjacent aggressor`,
        aggressorAfterAttack.objectId,
      );
    }
    await settings.sleep(settings.attackCadenceMs);
  }
  throw new Error(`target ${objectId} attack budget exceeded (${settings.maxAttackAttempts})`);
}

async function approachCombatTarget(
  client, target, approachRange, objectId, navigateNear, settings, aggressorInterruptPolicy,
) {
  let threatened = null;
  let unsafeRisk = null;
  const stopWhen = () => {
    const liveTarget = entityById(client.snapshot, objectId);
    if (liveTarget) {
      const liveHostiles = (client.snapshot?.entities ?? []).filter(isPotentialHostileMonster);
      const risk = targetPackRisk(liveTarget, liveHostiles);
      if (!targetRiskAllowed(risk, settings)) {
        unsafeRisk = risk;
        return true;
      }
    }
    threatened = interruptingAggressor(client, objectId, settings, aggressorInterruptPolicy);
    return Boolean(threatened);
  };
  const navigateWithClearance = hostileAvoidanceRadius => navigateNear(
    target,
    approachRange,
    stopWhen,
    { hostileAvoidanceRadius, allowedHostileObjectIds: [objectId] },
  );
  try {
    await navigateWithClearance(settings.combatHostileClearance);
  } catch (error) {
    if (!String(error?.message ?? '').startsWith('No walk path')) throw error;
    const fallback = Math.min(
      settings.combatHostileClearance,
      settings.combatHostileClearanceFallback,
    );
    if (fallback >= settings.combatHostileClearance) {
      throw new UnreachableCombatTarget(objectId, target, error);
    }
    recordSearchDiagnostic(client, {
      type: 'combatApproachClearanceFallback',
      objectId,
      from: settings.combatHostileClearance,
      to: fallback,
    });
    try {
      await navigateWithClearance(fallback);
    } catch (fallbackError) {
      if (!String(fallbackError?.message ?? '').startsWith('No walk path')) throw fallbackError;
      throw new UnreachableCombatTarget(objectId, target, fallbackError);
    }
  }
  if (unsafeRisk) throw new UnsafeTargetCluster(objectId, unsafeRisk);
  if (threatened) {
    throw new ThreatenedNavigation(
      `navigation to target ${objectId} interrupted by a proven adjacent aggressor`,
      threatened.objectId,
    );
  }
}

async function retreatFromUnsafePack(client, navigateNear, settings) {
  const originActor = playerFromSnapshot(client.snapshot);
  // Snapshot entities are mutated in place by packet reducers. Keep the
  // retreat origin immutable so distance/progress and diagnostics do not move
  // together with the live player entity.
  const origin = { x: Number(originActor.x), y: Number(originActor.y) };
  const originHostiles = (client.snapshot?.entities ?? []).filter(isPotentialHostileMonster);
  const originExposure = hostileExposure(origin, originHostiles, settings.unsafeRetreatSafeDistance);
  const minimumRetreatProgress = Math.min(6, settings.unsafeRetreatSteps);
  const safe = () => {
    const player = selectPlayer(client.snapshot);
    if (!player) return false;
    const hostiles = (client.snapshot?.entities ?? []).filter(isPotentialHostileMonster);
    return unsafeRetreatIsSafe({
      origin,
      player,
      hostiles,
      safeDistance: settings.unsafeRetreatSafeDistance,
      minimumRetreatProgress,
      originExposure,
      playerHp: client.snapshot?.playerHp,
      playerMaxHp: client.snapshot?.playerMaxHp,
      allowLowHealthFollowerRecovery: settings.allowLowHealthFollowerRecovery,
    });
  };
  if (safe()) return;
  const directions = [
    [0, -1], [1, -1], [1, 0], [1, 1],
    [0, 1], [-1, 1], [-1, 0], [-1, -1],
  ];
  let lastError = null;
  let emergencyEscapeAttempted = false;
  let emergencyEscapeRetryAt = Number.NEGATIVE_INFINITY;
  const tryEmergencyEscape = async (reason, current) => {
    if (!settings.emergencyEscape || emergencyEscapeAttempted || settings.now() < emergencyEscapeRetryAt) return false;
    const hpRatio = playerHealthRatio(client);
    if (hpRatio > settings.emergencyEscapeHpRatio) return false;
    emergencyEscapeAttempted = true;
    recordSearchDiagnostic(client, {
      type: 'emergencyEscapeAttempt',
      reason,
      hpRatio,
      threshold: settings.emergencyEscapeHpRatio,
      from: current,
    });
    try {
      const escaped = await settings.emergencyEscape(client, { current, reason, settings });
      if (escaped?.deferred === true) {
        emergencyEscapeAttempted = false;
        emergencyEscapeRetryAt = settings.now() + Math.max(1, Number(escaped.retryAfterMs) || 1);
        recordSearchDiagnostic(client, {
          type: 'emergencyEscapeDeferred',
          reason,
          hpRatio,
          retryAfterMs: Number(escaped.retryAfterMs ?? 0),
        });
        return false;
      }
      if (escaped === true || escaped?.success === true) {
        const relocated = selectPlayer(client.snapshot);
        recordSearchDiagnostic(client, {
          type: 'emergencyEscapeSuccess',
          reason,
          hpRatio,
          from: current,
          to: relocated ? { x: Number(relocated.x), y: Number(relocated.y) } : null,
        });
        return true;
      }
    } catch (error) {
      lastError = error;
      recordSearchDiagnostic(client, {
        type: 'emergencyEscapeFailure',
        reason,
        hpRatio,
        message: String(error?.message ?? error),
      });
    }
    return false;
  };
  // R31 proved that a low-health player can still find a momentarily open
  // local step while one confirmed attacker and another adjacent hostile are
  // already converging. Requiring both monsters to land a recent hit misses
  // that narrow interval even though their authoritative positions prove the
  // immediate surround.
  // Walking that short path merely postpones the same surround until recovery.
  // Use the opt-in public scroll before the pursuit closes again.
  if ((originExposure.adjacent >= 2 || originExposure.withinThree >= 3) &&
      provenAggressors(client, null, settings).length >= 1 &&
      await tryEmergencyEscape('criticalPackPressure', origin)) return;
  const attemptEmergencyEscape = async () => {
    // Recompute after every authoritative movement. A long A* route that was
    // safest at its old endpoint can become dangerous as monsters converge,
    // and retrying opposite endpoints from that stale origin causes the
    // player to oscillate through the same choke point.
    const visited = new Set([`${origin.x},${origin.y}`]);
    let successfulSteps = 0;
    let breakoutKills = 0;
    let stalledSteps = 0;
    let bestExposure = originExposure;
    const escapeStart = { ...origin };
    const breakOut = async (current, reason) => {
      const breakers = provenAggressors(client, null, settings);
      if (breakers.length === 0 || breakoutKills >= settings.maxRetreatBreakoutKills) return false;
      for (const breaker of breakers) {
        recordSearchDiagnostic(client, {
          type: 'unsafePackBreakoutCombat',
          objectId: Number(breaker.objectId),
          name: String(breaker.name ?? ''),
          from: current,
          reason,
          stalledSteps,
          breakoutKill: breakoutKills + 1,
        });
        try {
          await killExactMonster(
            client,
            breaker,
            { kind: 'travel', name: String(breaker.name ?? 'monster'), ownerQuestId: 0, routeIndex: 0 },
            navigateNear,
            { ...settings, maxTargetAdjacent: Number.POSITIVE_INFINITY, maxTargetNearby: Number.POSITIVE_INFINITY },
            false,
          );
        } catch (error) {
          if (!isDeferredCombatTargetError(error)) throw error;
          // R16 selected an adjacent SpiderFrog as a breakout target even
          // though dynamic occupancy made its tile unreachable. Do not let one
          // bad breaker abort the entire quest or get selected repeatedly;
          // record it and try another proven attacker in this bounded pass.
          recordSearchDiagnostic(client, {
            type: 'unsafePackBreakoutTargetUnreachable',
            objectId: Number(breaker.objectId),
            name: String(breaker.name ?? ''),
            reason: String(error?.message ?? error),
          });
          deferUnreachableTarget(error, settings);
          continue;
        }
        breakoutKills += 1;
        stalledSteps = 0;
        const actor = playerFromSnapshot(client.snapshot);
        bestExposure = hostileExposure(
          actor,
          (client.snapshot?.entities ?? []).filter(isPotentialHostileMonster),
          settings.unsafeRetreatSafeDistance,
        );
        return true;
      }
      return false;
    };
    while (successfulSteps < settings.unsafeRetreatMaxSteps) {
      assertPlayerAlive(client);
      if (safe()) break;

      const currentActor = playerFromSnapshot(client.snapshot);
      const current = { x: Number(currentActor.x), y: Number(currentActor.y) };
      const currentHostiles = (client.snapshot?.entities ?? []).filter(isPotentialHostileMonster);
      const currentExposure = hostileExposure(current, currentHostiles, settings.unsafeRetreatSafeDistance);
      if (stalledSteps >= settings.unsafeRetreatStallSteps && currentExposure.withinSafeDistance > 1 &&
          await breakOut(current, 'retreatExposureStalled')) {
        continue;
      }
      const requestedBias = settings.unsafeRetreatBiasPosition?.(client.snapshot) ?? null;
      const retreatBias = validPoint(requestedBias)
        ? { x: Number(requestedBias.x), y: Number(requestedBias.y) }
        : null;
      const currentBiasDistance = retreatBias ? distance(current, retreatBias) : 0;
      const occupied = new Set(currentHostiles.map(entity => `${Number(entity.x)},${Number(entity.y)}`));
      const allowedHostileObjectIds = currentHostiles.map(entity => Number(entity.objectId));
      const remainingBudget = settings.unsafeRetreatMaxSteps - successfulSteps;
      const candidates = directions.map(([dx, dy]) => {
        const first = { x: current.x + dx, y: current.y + dy };
        const second = { x: current.x + dx * 2, y: current.y + dy * 2 };
        const canRun = remainingBudget >= 2 && !occupied.has(`${second.x},${second.y}`);
        const target = canRun ? second : first;
        return {
          target,
          first,
          steps: canRun ? 2 : 1,
          firstExposure: hostileExposure(first, currentHostiles, settings.unsafeRetreatSafeDistance),
          targetExposure: hostileExposure(target, currentHostiles, settings.unsafeRetreatSafeDistance),
          visited: visited.has(`${target.x},${target.y}`),
          biasProgress: retreatBias ? currentBiasDistance - distance(target, retreatBias) : 0,
        };
      }).filter(candidate => !occupied.has(`${candidate.first.x},${candidate.first.y}`))
        .sort(compareRetreatCandidates);

      let moved = false;
      for (const candidate of candidates) {
        const beforeActor = playerFromSnapshot(client.snapshot);
        const before = { x: Number(beforeActor.x), y: Number(beforeActor.y) };
        try {
          await navigateNear(candidate.target, 0, safe, {
            maxSuccessfulSteps: candidate.steps,
            maxAttempts: candidate.steps * 4,
            maxNoPathRefreshes: 0,
            hostileAvoidanceRadius: 0,
            allowedHostileObjectIds,
            // The first escape step must be authoritative before Healing or a
            // potion can impose an action lock. Sustain runs immediately after
            // a proven coordinate change below.
            autoUseSupplies: false,
          });
        } catch (error) {
          if (client.failure) throw client.failure;
          assertPlayerAlive(client);
          lastError = error;
        }
        const afterActor = playerFromSnapshot(client.snapshot);
        const movedDistance = distance(before, afterActor);
        if (movedDistance <= 0) continue;
        visited.add(`${before.x},${before.y}`);
        successfulSteps += movedDistance;
        const afterHostiles = (client.snapshot?.entities ?? []).filter(isPotentialHostileMonster);
        const afterExposure = hostileExposure(afterActor, afterHostiles, settings.unsafeRetreatSafeDistance);
        if (compareHostileExposure(afterExposure, bestExposure) < 0) {
          bestExposure = afterExposure;
          stalledSteps = 0;
        } else {
          stalledSteps += movedDistance;
        }
        moved = true;
        await sustainIfDue(client, settings, 'retreat', null);
        break;
      }
      if (!moved) {
        // If every walkable first step is occupied or the only apparent exit
        // is rejected by authoritative collision, a real player must cut down
        // one of the monsters already striking them before an escape tile can
        // exist. Keep this bounded and target only a proven adjacent attacker.
        if (await tryEmergencyEscape('noAuthoritativeEscapeStep', current)) return true;
        if (!await breakOut(current, 'noAuthoritativeEscapeStep')) return false;
        continue;
      }
    }
    if (!safe()) return false;

    const player = playerFromSnapshot(client.snapshot);
    const currentHostiles = (client.snapshot?.entities ?? []).filter(isPotentialHostileMonster);
    recordSearchDiagnostic(client, {
      type: 'unsafePackEscapeFallback',
      from: escapeStart,
      to: { x: Number(player.x), y: Number(player.y) },
      ignoredHostileTrails: currentHostiles.length,
      iterative: true,
      successfulSteps,
      stepBudget: settings.unsafeRetreatMaxSteps,
    });
    recordSearchDiagnostic(client, {
      type: 'unsafePackRetreat',
      from: origin,
      to: { x: Number(player.x), y: Number(player.y) },
      originExposure,
      currentExposure: hostileExposure(player, currentHostiles, settings.unsafeRetreatSafeDistance),
      emergencyFallback: true,
    });
    return true;
  };
  if (await attemptEmergencyEscape()) return;
  const finalPlayer = selectPlayer(client.snapshot);
  const finalHostiles = (client.snapshot?.entities ?? []).filter(isPotentialHostileMonster);
  const finalExposure = finalPlayer
    ? hostileExposure(finalPlayer, finalHostiles, settings.unsafeRetreatSafeDistance)
    : originExposure;
  const materialProgress = finalPlayer ? distance(origin, finalPlayer) : 0;
  // A cave corner can exhaust every locally walkable candidate just after the
  // pursuer falls outside the authoritative attack-evidence window. The strict
  // clearance radius still sees nearby ambient monsters, but there is no actor
  // currently proven to be attacking. Preserve the real movement and let the
  // ordinary route planner leave the corner instead of aborting a living run.
  if (finalPlayer && provenAggressors(client, null, settings).length === 0) {
    const improved = exposureImproved(finalExposure, originExposure);
    recordSearchDiagnostic(client, {
      type: 'unsafePackPartialRetreat',
      from: origin,
      to: { x: Number(finalPlayer.x), y: Number(finalPlayer.y) },
      materialProgress,
      exposureImproved: improved,
      originExposure,
      currentExposure: finalExposure,
      reason: materialProgress > 0
        ? 'noProvenAggressorAfterPartialEscape'
        : 'noProvenAggressorAtLocalDeadEnd',
    });
    return;
  }
  throw new Error(`unsafe hostile pack retreat failed from ${origin.x},${origin.y}`, { cause: lastError });
}

export function compareRetreatCandidates(left, right) {
  // Never trade into extra close-range exposure. Within equally survivable
  // candidates, preserve route progress before avoiding a revisited tile; this
  // prevents a narrow cave from pushing the player back toward its entrance.
  const leftBias = Number(left.biasProgress ?? 0);
  const rightBias = Number(right.biasProgress ?? 0);
  if (leftBias === 0 && rightBias === 0) {
    return Number(left.visited) - Number(right.visited) ||
      compareHostileExposure(left.firstExposure, right.firstExposure) ||
      compareHostileExposure(left.targetExposure, right.targetExposure) ||
      right.steps - left.steps;
  }
  return left.firstExposure.adjacent - right.firstExposure.adjacent ||
    left.targetExposure.adjacent - right.targetExposure.adjacent ||
    left.firstExposure.withinThree - right.firstExposure.withinThree ||
    left.targetExposure.withinThree - right.targetExposure.withinThree ||
    rightBias - leftBias ||
    Number(left.visited) - Number(right.visited) ||
    compareHostileExposure(left.firstExposure, right.firstExposure) ||
    compareHostileExposure(left.targetExposure, right.targetExposure) ||
    right.steps - left.steps;
}

function compareHostileExposure(left, right) {
  return left.adjacent - right.adjacent ||
    left.withinThree - right.withinThree ||
    left.weighted - right.weighted ||
    right.nearest - left.nearest;
}

function hostileExposure(point, hostiles, safeDistance) {
  let adjacent = 0;
  let withinThree = 0;
  let withinSafeDistance = 0;
  let weighted = 0;
  let nearest = Number.POSITIVE_INFINITY;
  for (const hostile of hostiles) {
    const separation = distance(point, hostile);
    nearest = Math.min(nearest, separation);
    if (separation <= 1) adjacent += 1;
    if (separation <= 3) withinThree += 1;
    if (separation < safeDistance) withinSafeDistance += 1;
    weighted += Math.max(0, safeDistance + 1 - separation);
  }
  return { adjacent, withinThree, withinSafeDistance, weighted, nearest };
}

function exposureImproved(current, origin) {
  return current.adjacent < origin.adjacent ||
    (current.adjacent === origin.adjacent && current.withinThree < origin.withinThree) ||
    (current.adjacent === origin.adjacent && current.withinThree === origin.withinThree &&
      current.weighted < origin.weighted);
}

export function unsafeRetreatIsSafe({
  origin, player, hostiles, safeDistance, minimumRetreatProgress, originExposure,
  playerHp, playerMaxHp, allowLowHealthFollowerRecovery = false,
}) {
  if (hostiles.length === 0) return true;
  const current = hostileExposure(player, hostiles, safeDistance);
  if (current.nearest >= safeDistance) return true;

  const hp = Number(playerHp ?? player.hp ?? 0);
  const maxHp = Number(playerMaxHp ?? player.maxHp ?? 0);
  // A single follower at three cells is still lethal when the player cannot
  // survive its next hit. Keep moving to the full clearance radius before any
  // recovery action can stop movement or impose an action lock.
  if (maxHp > 0 && hp / maxHp < 0.25) return false;
  if (maxHp > 0 && hp / maxHp < 0.75 && !allowLowHealthFollowerRecovery) return false;
  if (maxHp > 0 && hp / maxHp < 0.75 && current.withinSafeDistance > 1) return false;

  // Dense Crystal spawn fields can make a global clearance radius
  // impossible even after a useful pull. A healthy player may accept a
  // material retreat that isolates one follower; a low-health player keeps
  // moving until the full radius is clear before any blocking recovery work.
  const moved = distance(origin, player);
  return moved >= minimumRetreatProgress && current.adjacent === 0 &&
    current.withinThree <= 1 && exposureImproved(current, originExposure);
}

async function retreatAndRecover(client, navigateNear, settings) {
  await retreatFromUnsafePack(client, navigateNear, settings);
  if (settings.recoverAfterUnsafeRetreat) {
    await settings.recoverAfterUnsafeRetreat(client, navigateNear);
  }
}

function minimumEntityDistance(point, entities) {
  return entities.length === 0 ? Number.POSITIVE_INFINITY :
    Math.min(...entities.map(entity => distance(point, entity)));
}

function recordUnsafeTargetCluster(client, questId, pending, error) {
  recordSearchDiagnostic(client, {
    type: 'unsafeTargetCluster', questId, target: pending.name,
    objectId: error.objectId, ...error.risk,
  });
}

function deferUnsafeTarget(client, error, settings) {
  if (error?.transientLowHealth === true) return;
  const objectId = Number(error?.objectId);
  if (!Number.isFinite(objectId)) return;
  settings.unsafeTargetMarks.set(objectId, Number(client?.sequence ?? 0));
}

function throwIfLowHealthTargetPressure(client, target, pending, settings) {
  if (pending?.kind === 'travel' || settings.lowHealthTargetRetreatRatio <= 0) return;
  if (shouldFinishLowHealthTarget(client, target, settings)) return;
  const player = selectPlayer(client.snapshot);
  if (!player || distance(player, target) > 1) return;
  const hp = Number(client.snapshot?.playerHp ?? player.hp ?? 0);
  const maxHp = Number(client.snapshot?.playerMaxHp ?? player.maxHp ?? 0);
  if (!(maxHp > 0) || hp / maxHp > settings.lowHealthTargetRetreatRatio) return;
  if (!provenAggressors(client, null, settings).some(entity =>
    Number(entity.objectId) === Number(target.objectId))) return;
  recordSearchDiagnostic(client, {
    type: 'lowHealthTargetRetreat',
    questId: questIdFor(pending),
    target: pending.name,
    objectId: Number(target.objectId),
    hp,
    maxHp,
  });
  const liveHostiles = (client.snapshot?.entities ?? []).filter(isPotentialHostileMonster);
  const error = new UnsafeTargetCluster(Number(target.objectId), targetPackRisk(target, liveHostiles));
  error.transientLowHealth = true;
  throw error;
}

function recordDeferredCombatTarget(client, questId, pending, error) {
  recordSearchDiagnostic(client, {
    type: error instanceof LostCombatTarget
      ? 'lostCombatTarget'
      : (error instanceof UnresponsiveCombatTarget
        ? 'unresponsiveCombatTarget'
        : 'unreachableCombatTarget'),
    questId, target: pending.name, objectId: error.objectId,
    location: error.target, attempts: error.attempts,
  });
}

function isRecoverableSearchNavigationError(error) {
  return error instanceof NavigationStalled ||
    String(error?.message ?? '').startsWith('No walk path') ||
    String(error?.message ?? '').startsWith('Navigation successful step budget exceeded');
}

function isDeferredCombatTargetError(error) {
  return error instanceof UnreachableCombatTarget ||
    error instanceof UnresponsiveCombatTarget ||
    error instanceof LostCombatTarget;
}

function deferUnreachableTarget(error, settings) {
  const objectId = Number(error?.objectId);
  if (!Number.isFinite(objectId)) return;
  settings.unreachableTargetMarks.set(objectId, {
    location: error.target,
    markedAtMs: settings.now(),
  });
}

async function combatApproachRange(settings, client, target) {
  return Math.min(18, positiveInteger(await settings.approachRange(client, target), 1));
}

async function clearProvenAggressors(client, excludedObjectId, pending, navigateNear, settings, budget) {
  let cleared = 0;
  let preferredObjectId = null;
  while (true) {
    assertPlayerAlive(client);
    const currentThreats = provenAggressors(client, excludedObjectId, settings);
    const threat = currentThreats.find(candidate =>
      Number(candidate.objectId) === Number(preferredObjectId)) ?? currentThreats[0];
    if (!threat) return cleared;
    if (lowHealthUnderMultipleAggressors(client, excludedObjectId, settings)) {
      throw unsafeAggressorPack(client, threat);
    }
    if (cleared >= Math.max(0, budget)) {
      throw new Error(`q${questIdFor(pending)} aggressor clear exceeded remaining combat engagement budget`);
    }
    // Existing simultaneous aggressors are cleared one at a time to avoid
    // target ping-pong. A newly attacking object is different: AI25 zombies
    // can revive beside the player while another aggressor is being cleared.
    // Let that new life pre-empt once, then re-rank it at the top of the next
    // bounded pass instead of ignoring it until the current target dies.
    const establishedObjectIds = new Set(currentThreats.map(candidate => Number(candidate.objectId)));
    try {
      await killExactMonster(
        client,
        threat,
        pending,
        navigateNear,
        settings,
        candidate => Number(candidate.objectId) !== Number(excludedObjectId) && (
          lowHealthUnderMultipleAggressors(client, excludedObjectId, settings) ||
          !establishedObjectIds.has(Number(candidate.objectId))
        ),
      );
    } catch (error) {
      if (!(error instanceof ThreatenedNavigation) || error.threatObjectId == null) throw error;
      if (lowHealthUnderMultipleAggressors(client, excludedObjectId, settings)) {
        throw unsafeAggressorPack(client, threat);
      }
      preferredObjectId = error.threatObjectId;
      continue;
    }
    cleared += 1;
    preferredObjectId = null;
  }
}

function lowHealthUnderMultipleAggressors(client, excludedObjectId, settings) {
  const player = selectPlayer(client.snapshot);
  const hp = Number(client.snapshot?.playerHp ?? player?.hp ?? 0);
  const maxHp = Number(client.snapshot?.playerMaxHp ?? player?.maxHp ?? 0);
  const aggressorCount = provenAggressors(client, excludedObjectId, settings).length;
  return aggressorCount >= settings.retreatAtActiveAggressorCount ||
    (maxHp > 0 && hp / maxHp <= settings.multiAggressorRetreatRatio && aggressorCount >= 2);
}

function playerHealthRatio(client) {
  const player = selectPlayer(client.snapshot);
  const hp = Number(client.snapshot?.playerHp ?? player?.hp ?? 0);
  const maxHp = Number(client.snapshot?.playerMaxHp ?? player?.maxHp ?? 0);
  return maxHp > 0 ? hp / maxHp : 0;
}

function unsafeAggressorPack(client, threat) {
  const liveHostiles = (client.snapshot?.entities ?? []).filter(isPotentialHostileMonster);
  return new UnsafeTargetCluster(Number(threat.objectId), targetPackRisk(threat, liveHostiles));
}

function interruptingAggressor(client, excludedObjectId, settings, policy) {
  if (policy === false) return null;
  const threats = provenAggressors(client, excludedObjectId, settings);
  if (typeof policy === 'function') return threats.find(policy) ?? null;
  return threats[0] ?? null;
}

function focusedTargetAggressorPolicy(client, excludedObjectId, settings) {
  if (!settings.focusTargetThroughAggressors) return true;
  return () => {
    const target = entityById(client.snapshot, excludedObjectId);
    if (target && shouldFinishLowHealthTarget(client, target, settings)) return false;
    const player = selectPlayer(client.snapshot);
    const hp = Number(client.snapshot?.playerHp ?? player?.hp ?? 0);
    const maxHp = Number(client.snapshot?.playerMaxHp ?? player?.maxHp ?? 0);
    const lowHealth = maxHp > 0 && hp / maxHp <= settings.multiAggressorRetreatRatio;
    const aggressorCount = provenAggressors(client, excludedObjectId, settings).length;
    return lowHealth || aggressorCount >= 2 ||
      aggressorCount >= settings.retreatAtActiveAggressorCount;
  };
}

function provenAggressors(client, excludedObjectId, settings) {
  const player = selectPlayer(client.snapshot);
  if (!player) return [];
  const events = (client.events ?? []).slice(-settings.aggressorEvidenceEvents);
  const direct = new Set();
  const facing = new Set();
  const cutoff = settings.now() - settings.aggressorEvidenceWindowMs;
  for (const event of events) {
    if (event?.direction !== 'received' || !recentEvent(event, cutoff)) continue;
    if (event.packet === 'ObjectStruck' && Number(event.payload?.objectId) === Number(client.snapshot.playerObjectId)) {
      direct.add(Number(event.payload?.attackerId));
    }
    if (event.packet === 'ObjectAttack') {
      const attacker = entityById(client.snapshot, event.payload?.objectId);
      if (attacker && attacksTowardPlayer(attacker, event.payload, player)) facing.add(Number(attacker.objectId));
    }
  }
  return (client.snapshot?.entities ?? [])
    .filter(entity => Number(entity?.objectId) !== Number(excludedObjectId) &&
      isPotentialHostileMonster(entity) &&
      distance(player, entity) <= 1 && (direct.has(Number(entity.objectId)) || facing.has(Number(entity.objectId))))
    .sort((left, right) => threatHealth(left) - threatHealth(right) ||
      distance(player, left) - distance(player, right) || Number(left.objectId) - Number(right.objectId));
}

function attacksTowardPlayer(attacker, payload, player) {
  const origin = validPoint(payload?.location) ? payload.location : attacker;
  if (distance(origin, player) > 1) return false;
  const dx = Math.sign(Number(player.x) - Number(origin.x));
  const dy = Math.sign(Number(player.y) - Number(origin.y));
  return DIRECTION_BY_DELTA.get(`${dx},${dy}`) === String(payload?.direction ?? '');
}

function threatHealth(entity) {
  const hp = finiteHp(entity?.hp);
  if (hp != null) return hp;
  const percent = finiteHp(entity?.healthPercent);
  return percent == null ? Number.POSITIVE_INFINITY : percent;
}

function shouldFinishLowHealthTarget(client, target, settings) {
  if (!(settings.finishableTargetHealthRatio > 0)) return false;
  const targetHp = finiteHp(target?.hp);
  const targetMaxHp = finiteHp(target?.maxHp);
  const targetPercent = finiteHp(target?.healthPercent);
  const targetRatio = targetHp != null && targetMaxHp > 0
    ? targetHp / targetMaxHp
    : (targetPercent != null ? targetPercent / 100 : Number.POSITIVE_INFINITY);
  if (targetRatio > settings.finishableTargetHealthRatio) return false;
  const player = selectPlayer(client.snapshot);
  const playerHp = Number(client.snapshot?.playerHp ?? player?.hp ?? 0);
  const playerMaxHp = Number(client.snapshot?.playerMaxHp ?? player?.maxHp ?? 0);
  return playerMaxHp > 0 && playerHp / playerMaxHp >= settings.finishableTargetMinimumPlayerHpRatio;
}

function recentEvent(event, cutoff) {
  const timestamp = typeof event?.at === 'number' ? event.at : Date.parse(event?.at);
  return !Number.isFinite(timestamp) || timestamp >= cutoff;
}

async function settleMissingTarget(client, objectId, settings) {
  try {
    await waitFor(client, () => playerIsDead(client) || entityById(client.snapshot, objectId),
      `target ${objectId} removal settle`, settings.targetLossSettleMs);
  } catch {
    // The player remained alive and the same target stayed absent.
  }
  assertPlayerAlive(client);
  return entityById(client.snapshot, objectId);
}

function playerIsDead(client) {
  const player = selectPlayer(client.snapshot);
  return !player || player.dead === true || Number(player.hp) <= 0 || Number(client.snapshot?.playerHp) <= 0;
}

function contestedLethalByRemotePlayer(client, objectId, afterSequence) {
  const events = (client.events ?? []).filter(event => event.sequence > afterSequence && event.direction === 'received');
  const lethalIndex = events.findIndex(event =>
    (event.packet === 'ObjectDied' && Number(event.payload?.objectId) === Number(objectId)) ||
    (event.packet === 'ObjectHealth' && Number(event.payload?.objectId) === Number(objectId) && Number(event.payload?.percent) <= 0));
  if (lethalIndex < 0) return null;
  const strikes = events.slice(0, lethalIndex + 1).filter(event =>
    event.packet === 'ObjectStruck' && Number(event.payload?.objectId) === Number(objectId));
  const last = strikes.at(-1);
  const attackerId = Number(last?.payload?.attackerId);
  return isObservedRemotePlayer(client, attackerId) ? { attackerId } : null;
}

function isObservedRemotePlayer(client, objectId) {
  if (!Number.isSafeInteger(objectId) || objectId <= 0 || objectId === Number(client.snapshot?.playerObjectId)) return false;
  const selfName = normalizeName(selectPlayer(client.snapshot)?.name);
  const current = entityById(client.snapshot, objectId);
  if (current && ['player', 'remoteplayer'].includes(normalized(current.kind)) && normalizeName(current.name) !== selfName) return true;
  return (client.events ?? []).some(event => event.direction === 'received' && event.packet === 'ObjectPlayer' &&
    Number(event.payload?.objectId) === objectId && normalizeName(event.payload?.name) !== selfName);
}

async function harvestExactCorpse(client, initialCorpse, pending, beforeProgress, navigateNear, settings) {
  const objectId = Number(initialCorpse?.objectId);
  let acceptedPasses = 0;
  let noResponse = 0;
  for (let attempt = 0; attempt < settings.maxHarvestPasses; attempt += 1) {
    assertPlayerAlive(client);
    if (objectiveAdvanced(client.snapshot, questIdFor(pending), pending, beforeProgress)) {
      return { acceptedPasses, unavailable: false, attempts: attempt };
    }
    const corpse = entityById(client.snapshot, objectId);
    if (!corpse || (corpse.dead !== true && Number(corpse.hp) > 0)) {
      try {
        await waitFor(client, () => objectiveAdvanced(client.snapshot, questIdFor(pending), pending, beforeProgress), `q${questIdFor(pending)} harvest progress after corpse removal`, settings.questSettleTimeout);
        return { acceptedPasses, unavailable: false, attempts: attempt };
      } catch (error) {
        throw new Error(`corpse ${objectId} disappeared without authoritative ${pending.name} progress`, { cause: error });
      }
    }
    await navigateNear(corpse, 1);
    const player = playerFromSnapshot(client.snapshot);
    const refreshed = entityById(client.snapshot, objectId);
    if (!refreshed || distance(player, refreshed) > 1) throw new Error(`corpse ${objectId} is no longer harvestable at adjacent range`);
    await sustainIfDue(client, settings, 'harvest', objectId);
    assertPlayerAlive(client);
    const harvestPlayer = playerFromSnapshot(client.snapshot);
    const harvestable = entityById(client.snapshot, objectId);
    if (!harvestable || (harvestable.dead !== true && Number(harvestable.hp) > 0) || distance(harvestPlayer, harvestable) > 1) {
      throw new Error(`corpse ${objectId} is no longer harvestable at adjacent range`);
    }
    const afterSequence = client.sequence ?? 0;
    client.send({ type: "harvest", direction: harvestDirection(harvestPlayer, harvestable) });
    try {
      const accepted = await waitFor(client, () =>
        objectiveAdvanced(client.snapshot, questIdFor(pending), pending, beforeProgress) ||
        receivedPacketAfter(client, afterSequence, "ObjectHarvest") ||
        receivedPacketAfter(client, afterSequence, "ObjectHarvested") ||
        !entityById(client.snapshot, objectId),
      `corpse ${objectId} harvest response`, settings.attackResponseTimeout);
      if (accepted) acceptedPasses += 1;
      noResponse = 0;
    } catch (error) {
      if (++noResponse >= 3) {
        return { acceptedPasses, unavailable: true, attempts: noResponse };
      }
    }
    if (objectiveAdvanced(client.snapshot, questIdFor(pending), pending, beforeProgress)) {
      return { acceptedPasses, unavailable: false, attempts: attempt + 1 };
    }
    await settings.sleep(settings.harvestCadenceMs);
  }
  throw new Error(`corpse ${objectId} harvest pass budget exceeded (${settings.maxHarvestPasses})`);
}

async function sustainIfDue(client, settings, phase, objectId) {
  if (!settings.sustain) return;
  const now = settings.now();
  if (now - settings.lastSustainAt < settings.sustainCadenceMs) return;
  settings.lastSustainAt = now;
  await settings.sustain(client, { phase, objectId });
}

async function collectVisibleObjectiveDrop(client, pending, beforeProgress, navigateNear, settings) {
  try {
    await waitFor(client, () =>
      objectiveAdvanced(client.snapshot, questIdFor(pending), pending, beforeProgress) ||
      visibleDrop(client.snapshot, pending.name),
    `q${questIdFor(pending)} ${pending.name} drop or progress`, settings.questSettleTimeout);
  } catch (error) {
    throw new Error(`${pending.name} neither appeared nor advanced authoritative quest progress`, { cause: error });
  }
  if (objectiveAdvanced(client.snapshot, questIdFor(pending), pending, beforeProgress)) return;
  const drop = visibleDrop(client.snapshot, pending.name);
  if (!drop) return;
  await navigateNear(drop, 1);
  const objectId = Number(drop.objectId);
  client.send({ type: "pickUp", objectId });
  await waitFor(client, () =>
    objectiveAdvanced(client.snapshot, questIdFor(pending), pending, beforeProgress) ||
    !(client.snapshot?.groundDrops ?? []).some((entry) => Number(entry.objectId) === objectId),
  `pickup ${pending.name}`, settings.questSettleTimeout);
  if (!objectiveAdvanced(client.snapshot, questIdFor(pending), pending, beforeProgress)) {
    throw new Error(`${pending.name} pickup was not confirmed by authoritative quest progress`);
  }
}

function selectPlayer(snapshot) {
  return (snapshot?.entities ?? []).find((entity) => String(entity.objectId) === String(snapshot?.playerObjectId));
}

function playerFromSnapshot(snapshot) {
  const player = selectPlayer(snapshot);
  if (!validPoint(player)) throw new Error("authoritative player position is unavailable");
  return player;
}

function assertPlayerAlive(client) {
  const player = selectPlayer(client.snapshot);
  if (!player || player.dead === true || Number(player.hp) <= 0 || Number(client.snapshot?.playerHp) <= 0) {
    throw new Error("Player died during quest combat");
  }
}

function nearestLiveMonster(snapshot, monsterNames) {
  const wanted = new Set(monsterNames.map(normalizeName));
  let player;
  try { player = playerFromSnapshot(snapshot); } catch { return null; }
  return (snapshot?.entities ?? [])
    .filter((entity) => isLiveMonster(entity) && wanted.has(normalizeName(entity.name)))
    .sort((left, right) => distance(player, left) - distance(player, right) || Number(left.objectId) - Number(right.objectId))[0] ?? null;
}

function nearestAvailableLiveMonster(client, monsterNames, unavailableCorpses, settings) {
  const excluded = new Set();
  for (const [objectId, markedAtSequence] of unavailableCorpses) {
    const current = entityById(client.snapshot, objectId);
    const revived = current && isLiveMonster(current) &&
      receivedPacketAfter(client, markedAtSequence, "ObjectRevived", objectId);
    if (revived) unavailableCorpses.delete(objectId);
    else excluded.add(objectId);
  }
  const wanted = new Set(monsterNames.map(normalizeName));
  let player;
  try { player = playerFromSnapshot(client.snapshot); } catch { return null; }
  const liveHostiles = (client.snapshot?.entities ?? []).filter(isPotentialHostileMonster);
  return (client.snapshot?.entities ?? [])
    .filter(entity => isLiveMonster(entity) && wanted.has(normalizeName(entity.name)) &&
      !excluded.has(Number(entity.objectId)) &&
      !unreachableTargetDeferred(entity, settings) &&
      !unsafeTargetDeferred(client, Number(entity.objectId), settings))
    .map(entity => ({
      entity,
      risk: targetPackRisk(entity, liveHostiles),
    }))
    .filter(candidate => targetRiskAllowed(candidate.risk, settings))
    .sort((left, right) =>
      left.risk.adjacent - right.risk.adjacent ||
      left.risk.nearby - right.risk.nearby ||
      distance(player, left.entity) - distance(player, right.entity) ||
      Number(left.entity.objectId) - Number(right.entity.objectId))[0]?.entity ?? null;
}

function unreachableTargetDeferred(entity, settings) {
  const objectId = Number(entity?.objectId);
  if (!settings.unreachableTargetMarks.has(objectId)) return false;
  const mark = settings.unreachableTargetMarks.get(objectId);
  const markedLocation = mark?.location ?? mark;
  // A moving monster can make a previously disconnected approach reachable.
  // A stationary target can also become reachable when a temporary corridor
  // blocker moves. Retry it after one bounded world interval so a single bad
  // actor cannot monopolise the objective, while a newly opened route is not
  // misreported as a missing spawn for the rest of the session.
  const retryDue = Number.isFinite(Number(mark?.markedAtMs)) &&
    settings.now() - Number(mark.markedAtMs) >= settings.unreachableTargetRetryMs;
  if (!validPoint(markedLocation) || distance(entity, markedLocation) >= 1 || retryDue) {
    settings.unreachableTargetMarks.delete(objectId);
    return false;
  }
  return true;
}

function unsafeTargetDeferred(client, objectId, settings) {
  const markedAtSequence = settings.unsafeTargetMarks.get(objectId);
  if (markedAtSequence == null) return false;
  if (receivedPacketAfter(client, markedAtSequence, "ObjectRevived", objectId)) {
    settings.unsafeTargetMarks.delete(objectId);
    return false;
  }
  const current = entityById(client.snapshot, objectId);
  const eventGap = Number(client.sequence ?? 0) - Number(markedAtSequence);
  if (current && isLiveMonster(current) && eventGap >= settings.unsafeTargetRetryEventGap) {
    const liveHostiles = (client.snapshot?.entities ?? []).filter(isPotentialHostileMonster);
    if (targetRiskAllowed(targetPackRisk(current, liveHostiles), settings)) {
      settings.unsafeTargetMarks.delete(objectId);
      return false;
    }
  }
  return true;
}

function targetPackRisk(target, liveHostiles) {
  let adjacent = 0;
  let nearby = 0;
  for (const hostile of liveHostiles) {
    if (Number(hostile.objectId) === Number(target.objectId)) continue;
    const separation = distance(target, hostile);
    if (separation <= 1) adjacent += 1;
    if (separation <= 3) nearby += 1;
  }
  return { adjacent, nearby };
}

function targetRiskAllowed(risk, settings) {
  return risk.adjacent <= settings.maxTargetAdjacent && risk.nearby <= settings.maxTargetNearby;
}

function throwIfUnsafeTargetPack(client, target, settings) {
  if (shouldFinishLowHealthTarget(client, target, settings)) return;
  const liveHostiles = (client.snapshot?.entities ?? []).filter(isPotentialHostileMonster);
  const risk = targetPackRisk(target, liveHostiles);
  if (!targetRiskAllowed(risk, settings)) {
    throw new UnsafeTargetCluster(Number(target.objectId), risk);
  }
}

function isLiveMonster(entity) {
  return normalized(entity?.kind) === "monster" && entity?.dead !== true && !(entity?.hp != null && Number(entity.hp) <= 0) && validPoint(entity);
}

function isPotentialHostileMonster(entity) {
  // ObjectMonster/NewMonsterInfo do not carry disposition. Until a full
  // snapshot explicitly marks the actor friendly, navigation and pack-risk
  // checks must treat a live monster packet as potentially hostile.
  const disposition = normalized(entity?.disposition);
  return isLiveMonster(entity) && (!disposition || disposition === 'hostile');
}

function visibleDrop(snapshot, itemName) {
  const wanted = normalizeName(itemName);
  const player = playerFromSnapshot(snapshot);
  return (snapshot?.groundDrops ?? [])
    .filter((drop) => normalizeName(drop?.name ?? drop?.itemName ?? drop?.key) === wanted && validPoint(drop))
    .sort((left, right) => distance(player, left) - distance(player, right))[0] ?? null;
}

function objectiveFingerprint(snapshot, questId, pending) {
  const quest = questEntry(snapshot, questId);
  const objective = snapshotObjective(quest, pending);
  return `${normalized(quest?.stage)}:${Number(objective?.current ?? 0)}:${objective?.done === true}`;
}

function objectiveAdvanced(snapshot, questId, pending, before) {
  const quest = questEntry(snapshot, questId);
  if (READY_STAGES.has(normalized(quest?.stage))) return true;
  const objective = snapshotObjective(quest, pending);
  if (objectiveDone(objective)) return true;
  return objectiveFingerprint(snapshot, questId, pending) !== before && Number(objective?.current ?? 0) > Number(String(before).split(":")[1] ?? 0);
}

function questEntry(snapshot, questId) {
  return (snapshot?.questLog ?? []).find((entry) => Number(entry?.questId) === Number(questId)) ?? null;
}

function questIdFor(pending) {
  return Number(pending?.questId ?? pending?.stateQuestId ?? pending?.ownerQuestId);
}

function entityById(snapshot, objectId) {
  return (snapshot?.entities ?? []).find((entry) => Number(entry?.objectId) === Number(objectId)) ?? null;
}

function receivedPacketAfter(client, sequence, packet, objectId = null) {
  return (client.events ?? []).some((event) => event.sequence > sequence && event.direction === "received" && event.packet === packet && (objectId == null || Number(event.payload?.objectId) === objectId));
}

async function waitFor(client, predicate, label, timeout) {
  return client.wait(predicate, label, timeout);
}

function spawnKey(spawn) {
  return `${spawn.mapFileName}:${spawn.respawnIndex ?? "?"}:${spawn.position.x},${spawn.position.y}`;
}

function validPoint(value) {
  return Number.isFinite(Number(value?.x)) && Number.isFinite(Number(value?.y));
}

function normalizeName(value) {
  return String(value ?? "").replace(/[^a-z0-9]/gi, "").toLowerCase();
}

function sameName(left, right) { return normalizeName(left) === normalizeName(right); }
function normalized(value) { return String(value ?? "").replace(/[^a-z]/gi, "").toLowerCase(); }
function finiteNumber(value) { const number = Number(value); return Number.isFinite(number) ? number : null; }
function finiteHp(value) { if (value == null) return null; const hp = Number(value); return Number.isFinite(hp) ? hp : null; }
function positiveInteger(value, fallback) { const parsed = Math.trunc(Number(value)); return Number.isFinite(parsed) && parsed > 0 ? parsed : fallback; }
function nonnegativeInteger(value, fallback) { const parsed = Math.trunc(Number(value)); return Number.isFinite(parsed) && parsed >= 0 ? parsed : fallback; }
function boundedRatio(value, fallback) {
  if (value == null) return fallback;
  const parsed = Number(value);
  return Number.isFinite(parsed) && parsed >= 0 && parsed <= 1 ? parsed : fallback;
}
function riskLimit(value) { return value == null ? Number.POSITIVE_INFINITY : nonnegativeInteger(value, Number.POSITIVE_INFINITY); }
