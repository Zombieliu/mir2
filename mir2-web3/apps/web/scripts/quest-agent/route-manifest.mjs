import fs from "node:fs/promises";

const DATA_URLS = Object.freeze({
  quests: new URL(
    "../../../../packages/game-data/data/generated/crystal_quest_packet_manifest.json",
    import.meta.url,
  ),
  npcs: new URL(
    "../../../../packages/game-data/data/generated/crystal_npc_info_manifest.json",
    import.meta.url,
  ),
  respawns: new URL(
    "../../../../packages/game-data/data/generated/crystal_respawn_manifest.json",
    import.meta.url,
  ),
  drops: new URL(
    "../../../../packages/game-data/data/generated/crystal_drop_manifest.json",
    import.meta.url,
  ),
  npcScripts: new URL(
    "../../../../packages/game-data/data/generated/crystal_npc_manifest.json",
    import.meta.url,
  ),
  monsters: new URL(
    "../../../../packages/game-data/data/generated/crystal_monster_manifest.json",
    import.meta.url,
  ),
  items: new URL(
    "../../../../packages/game-data/data/generated/crystal_item_manifest.json",
    import.meta.url,
  ),
  contentProfile: new URL(
    "../../../../packages/game-data/data/content_profiles/platinum_176.json",
    import.meta.url,
  ),
});

export const QUEST_CLASS_MASKS = Object.freeze({
  Warrior: 0x01,
  Wizard: 0x02,
  Taoist: 0x04,
  Assassin: 0x08,
  Archer: 0x10,
});

export const QUEST_ROUTE_LEVEL_SEGMENTS = Object.freeze([
  Object.freeze({ label: "1-7", minLevel: 1, maxLevel: 7 }),
  Object.freeze({ label: "8-21", minLevel: 8, maxLevel: 21 }),
  Object.freeze({ label: "22-35", minLevel: 22, maxLevel: 35 }),
  Object.freeze({ label: "36-50", minLevel: 36, maxLevel: 50 }),
]);

/** Decode the fixed ClientQuestInfo prefix without depending on browser code. */
export function decodeCrystalQuestHeader(payloadHex) {
  const cursor = new PacketCursor(payloadHex);
  const header = {
    index: cursor.i32(),
    npcIndex: cursor.u32(),
    name: cursor.string(),
    group: cursor.string(),
    description: cursor.stringVector(),
    taskDescription: cursor.stringVector(),
    returnDescription: cursor.stringVector(),
    completionDescription: cursor.stringVector(),
    minLevelNeeded: cursor.i32(),
    maxLevelNeeded: cursor.i32(),
    questNeeded: cursor.i32(),
    classNeeded: cursor.u8(),
    questType: cursor.u8(),
    timeLimitInSeconds: cursor.i32(),
    rewardGold: cursor.u32(),
    rewardExperience: cursor.u32(),
    rewardCredit: cursor.u32(),
  };
  header.bytesRead = cursor.offset;
  return header;
}

export async function loadCrystalQuestRouteSources(urls = DATA_URLS) {
  const resolvedUrls = { ...DATA_URLS, ...urls };
  const [
    questManifest,
    npcInfoManifest,
    respawnManifest,
    dropManifest,
    npcScriptManifest,
    monsterManifest,
    itemManifest,
    contentProfile,
  ] =
    await Promise.all([
      readJson(resolvedUrls.quests),
      readJson(resolvedUrls.npcs),
      readJson(resolvedUrls.respawns),
      readJson(resolvedUrls.drops),
      readJson(resolvedUrls.npcScripts),
      readJson(resolvedUrls.monsters),
      readJson(resolvedUrls.items),
      readJson(resolvedUrls.contentProfile),
    ]);
  return {
    questManifest,
    npcInfoManifest,
    respawnManifest,
    dropManifest,
    npcScriptManifest,
    monsterManifest,
    itemManifest,
    contentProfile,
  };
}

/**
 * Build a conservative vendor-trash catalogue from authoritative Crystal
 * drops, item stats, quest requirements, and merchant Trade sections.
 *
 * Only ordinary (non-Q) drops accepted by an explicitly configured merchant
 * are considered. Class-compatible gear is vendor trash only when every base
 * stat belongs to another class's attack channel. Shared defence/utility gear
 * stays untouched because it may still be a real upgrade.
 */
export function buildSafeSupplyLootCatalog(
  sources,
  {
    className = "Warrior",
    dropTableKeys = [],
    merchants = [],
  } = {},
) {
  const classMask = QUEST_CLASS_MASKS[className];
  if (!classMask) throw new Error(`unsupported Crystal quest class: ${className}`);
  const primaryAttackStats = new Set({
    Warrior: [4, 5],
    Wizard: [6, 7],
    Taoist: [8, 9],
  }[className] ?? []);
  const allClassAttackStats = new Set([4, 5, 6, 7, 8, 9]);
  const protectedQuestItems = new Set(
    (sources?.questManifest?.quests ?? []).flatMap((quest) => [
      ...(quest?.item_tasks ?? []),
      ...(quest?.carry_items ?? []),
    ]).map((task) => normalizeName(task?.item_name)),
  );
  const itemByName = new Map(
    (sources?.itemManifest?.items ?? []).map((item) => [normalizeName(item?.name), item]),
  );
  const scriptByKey = new Map(
    (sources?.npcScriptManifest?.scripts ?? []).map((script) => [String(script?.script_key), script]),
  );
  const merchantTradeNames = merchants.map((merchant) => {
    const script = scriptByKey.get(String(merchant?.scriptKey));
    const trade = (script?.sections ?? []).find(
      (section) => normalizeName(section?.label) === "trade",
    );
    const types = (script?.sections ?? []).find(
      (section) => normalizeName(section?.label) === "types",
    );
    return {
      merchantKey: String(merchant?.merchantKey ?? ""),
      allowStatless: merchant?.allowStatless === true,
      names: new Set((trade?.lines ?? []).map(normalizeName).filter(Boolean)),
      itemTypes: new Set((types?.lines ?? [])
        .map((line) => Number.parseInt(String(line).trim(), 10))
        .filter(Number.isFinite)),
    };
  }).filter((merchant) =>
    merchant.merchantKey && (merchant.names.size > 0 || merchant.itemTypes.size > 0)
  );
  const wantedTables = new Set(dropTableKeys.map(String));
  const ordinaryDropNames = new Set(
    (sources?.dropManifest?.tables ?? [])
      .filter((table) => wantedTables.has(String(table?.table_key)))
      .flatMap((table) => table?.sections ?? [])
      .flatMap((section) => section?.entries ?? [])
      .filter((entry) => !(entry?.modifiers ?? []).some(
        (modifier) => String(modifier).toUpperCase() === "Q",
      ))
      .map((entry) => normalizeName(entry?.item_name))
      .filter((name) => name && name !== "gold" && !protectedQuestItems.has(name)),
  );

  return [...ordinaryDropNames].flatMap((normalizedName) => {
    const item = itemByName.get(normalizedName);
    if (!item) return [];
    const merchant = merchantTradeNames.find((entry) =>
      entry.names.has(normalizedName) ||
      (entry.allowStatless && entry.itemTypes.has(Number(item.item_type)))
    );
    if (!merchant) return [];
    const requiredClass = Number(item.required_class ?? 0);
    const compatible = requiredClass === 0 || (requiredClass & classMask) !== 0;
    const positiveStats = (item.stats ?? [])
      .filter((stat) => Number(stat?.value ?? 0) > 0)
      .map((stat) => Number(stat.stat));
    const clearlyOffClass = !compatible || (
      positiveStats.length > 0 &&
      positiveStats.every((stat) => allClassAttackStats.has(stat)) &&
      positiveStats.every((stat) => !primaryAttackStats.has(stat))
    );
    const provenStatlessMaterial = merchant.allowStatless && positiveStats.length === 0;
    if (!clearlyOffClass && !provenStatlessMaterial) return [];
    return [{
      name: String(item.name),
      merchantKey: merchant.merchantKey,
      itemIndex: Number(item.item_index),
      itemType: Number(item.item_type),
    }];
  }).sort((left, right) => left.name.localeCompare(right.name));
}

/**
 * Enumerate class-compatible Crystal skill books for opportunistic visible
 * pickup. This is inventory metadata only; the Agent still has to see the
 * real ground drop, walk into range, click it, and activate it from the bag.
 */
export function buildProgressionSkillBookCatalog(
  sources,
  { className = "Warrior", maxLevel = 50 } = {},
) {
  const classMask = QUEST_CLASS_MASKS[className];
  if (!classMask) throw new Error(`unsupported Crystal quest class: ${className}`);
  const levelLimit = Math.max(1, finiteInteger(maxLevel, 50));
  return (sources?.itemManifest?.items ?? [])
    .filter((item) => {
      const requiredClass = Number(item?.required_class ?? 0);
      const requiredLevel = Number(item?.required_amount ?? 0);
      return Number(item?.item_type) === 20 &&
        (requiredClass === 0 || (requiredClass & classMask) !== 0) &&
        requiredLevel >= 1 &&
        requiredLevel <= levelLimit;
    })
    .map((item) => ({
      itemIndex: Number(item.item_index),
      name: String(item.name),
      minLevel: Number(item.required_amount),
      requiredClass: Number(item.required_class ?? 0),
    }))
    .sort((left, right) => left.minLevel - right.minLevel || left.name.localeCompare(right.name));
}

export function buildClassQuestRoute(sources, { className = "Warrior", maxLevel = 50 } = {}) {
  const classMask = QUEST_CLASS_MASKS[className];
  if (!classMask) throw new Error(`unsupported Crystal quest class: ${className}`);
  const acceptanceLevel = finiteInteger(maxLevel, 50);
  if (acceptanceLevel < 1) throw new Error("maxLevel must be positive");

  const quests = Array.isArray(sources?.questManifest?.quests)
    ? sources.questManifest.quests
    : [];
  const npcs = Array.isArray(sources?.npcInfoManifest?.npcs)
    ? sources.npcInfoManifest.npcs
    : [];
  const respawnMaps = Array.isArray(sources?.respawnManifest?.maps)
    ? sources.respawnManifest.maps
    : [];
  const dropTables = Array.isArray(sources?.dropManifest?.tables)
    ? sources.dropManifest.tables
    : [];
  const npcScripts = Array.isArray(sources?.npcScriptManifest?.scripts)
    ? sources.npcScriptManifest.scripts
    : [];
  const items = Array.isArray(sources?.itemManifest?.items)
    ? sources.itemManifest.items
    : [];

  const npcByLoadedObjectId = new Map(
    npcs.map((npc) => [Number(npc.loaded_object_id), npc]),
  );
  const scriptByKey = new Map(npcScripts.map((script) => [String(script.script_key), script]));
  const flagSettersByNumber = buildFlagSetterBindings(npcs, npcScripts, scriptByKey);
  const monsters = Array.isArray(sources?.monsterManifest?.monsters)
    ? sources.monsterManifest.monsters
    : [];
  const runtimeProfile = buildRuntimeProfileIndex(sources?.contentProfile);
  const itemByName = new Map(items.map((item) => [normalizeName(item?.name), item]));
  const spawnRows = flattenRespawns(respawnMaps, monsters, sources?.contentProfile);
  const itemDropRows = [
    ...flattenQuestItemDrops(dropTables),
    ...flattenProfileDropOverrides(sources?.contentProfile),
  ];

  const routeQuests = quests
    .map((template) => ({ template, header: decodeCrystalQuestHeader(template.payload_hex) }))
    .filter(({ header }) => {
      if (header.minLevelNeeded < 1 || header.minLevelNeeded > acceptanceLevel) return false;
      return header.classNeeded === 0 || (header.classNeeded & classMask) !== 0;
    })
    .map(({ template, header: importedHeader }) => {
      const prerequisiteOverride = runtimeProfile.questPrerequisiteOverrides.get(
        Number(importedHeader.index),
      ) ?? null;
      const header = {
        ...importedHeader,
        questNeeded: Number(
          prerequisiteOverride?.requiredQuestId ?? importedHeader.questNeeded,
        ),
      };
      const startNpc = npcRouteBinding(
        npcByLoadedObjectId.get(Number(header.npcIndex)),
        scriptByKey,
      );
      const finishNpc = npcRouteBinding(
        npcByLoadedObjectId.get(Number(template.finish_npc_index)),
        scriptByKey,
      );
      const killObjectives = (template.kill_tasks ?? []).map((task) => ({
        monsterIndex: Number(task.monster_index),
        monsterName: String(task.monster_name),
        count: Number(task.count),
        message: String(task.message ?? ""),
        spawnCandidates: spawnCandidatesForTask(spawnRows, task),
      }));
      const itemObjectives = (template.item_tasks ?? []).map((task) => ({
        itemIndex: Number(task.item_index),
        itemName: String(task.item_name),
        count: Number(task.count),
        message: String(task.message ?? ""),
        sources: itemSourcesForTask(itemDropRows, spawnRows, task),
      }));
      const carryObjectives = (template.carry_items ?? []).map((task) => ({
        itemIndex: Number(task.item_index),
        itemName: String(task.item_name),
        count: Number(task.count),
        message: String(task.message ?? ""),
      }));
      const flagObjectives = (template.flag_tasks ?? []).map((task) => ({
        number: Number(task.number),
        message: String(task.message ?? ""),
        setters: flagSettersByNumber.get(Number(task.number)) ?? [],
      }));
      const rewards = decodeCrystalQuestRewards(template.payload_hex, header.bytesRead);
      const rewardOverrides = runtimeProfile.questRewardOverrides.get(Number(header.index)) ?? [];
      const profileFixedItems = rewardOverrides.map((rule) => {
        const item = itemByName.get(normalizeName(rule.item));
        if (!item) throw new Error(`q${header.index} reward override item ${rule.item} is missing`);
        return {
          selectionIndex: rewards.fixedItems.length,
          itemIndex: Number(item.item_index),
          itemName: String(item.name),
          itemImage: Number(item.image),
          requiredClass: Number(item.required_class),
          requiredGender: Number(item.required_gender),
          count: Number(rule.count),
          profileOverride: true,
          sourceNote: String(rule.sourceNote),
        };
      });
      const specialHandlers = specialHandlersForQuest({
        header,
        startNpcIndex: Number(header.npcIndex),
        finishNpcIndex: Number(template.finish_npc_index),
        startNpc,
        finishNpc,
        killObjectives,
        itemObjectives,
        carryObjectives,
        flagObjectives,
      });
      const contentBlockers = routeBlockers({
        startNpcIndex: Number(header.npcIndex),
        finishNpcIndex: Number(template.finish_npc_index),
        startNpc,
        finishNpc,
        killObjectives,
        itemObjectives,
        flagObjectives,
      });
      const runtimeBlockers = runtimeProfileBlockers({
        profile: runtimeProfile,
        startNpc,
        finishNpc,
        killObjectives,
        itemObjectives,
        flagObjectives,
      });
      return {
        questId: Number(header.index),
        name: header.name,
        group: header.group,
        sourceFile: String(template.file_name),
        eligibility: {
          minLevel: Number(header.minLevelNeeded),
          maxLevel: Number(header.maxLevelNeeded),
          requiredQuestId: Number(header.questNeeded),
          importedRequiredQuestId: Number(importedHeader.questNeeded),
          prerequisiteOverride: prerequisiteOverride == null ? null : {
            requiredQuestId: Number(prerequisiteOverride.requiredQuestId),
            sourceNote: String(prerequisiteOverride.sourceNote),
          },
          classMask: Number(header.classNeeded),
          questType: Number(header.questType),
          timeLimitSeconds: Number(header.timeLimitInSeconds),
        },
        startNpc,
        finishNpc,
        descriptions: {
          description: header.description,
          task: header.taskDescription,
          return: header.returnDescription,
          completion: header.completionDescription,
        },
        rewards: {
          gold: Number(header.rewardGold),
          experience: Number(header.rewardExperience),
          credit: Number(header.rewardCredit),
          fixedItems: [...rewards.fixedItems, ...profileFixedItems],
          selectableItems: rewards.selectableItems,
          profileOverrides: rewardOverrides,
        },
        objectives: {
          kill: killObjectives,
          item: itemObjectives,
          carry: carryObjectives,
          flag: flagObjectives,
        },
        specialHandlers,
        contentBlockers,
        runtimeBlockers,
        blockers: [...new Set([...contentBlockers, ...runtimeBlockers])].sort(),
      };
    })
    .sort((left, right) =>
      left.eligibility.minLevel - right.eligibility.minLevel || left.questId - right.questId
    );

  const eligibleQuestIds = new Set(routeQuests.map((quest) => quest.questId));
  for (const quest of routeQuests) {
    const required = quest.eligibility.requiredQuestId;
    if (required > 0 && !eligibleQuestIds.has(required)) {
      const blocker = `required quest q${required} is outside the ${className} route`;
      quest.contentBlockers.push(blocker);
      quest.contentBlockers.sort();
      quest.blockers.push(blocker);
      quest.blockers.sort();
    }
  }

  const handlerCounts = countValues(routeQuests.flatMap((quest) => quest.specialHandlers));
  const blockerCounts = countValues(routeQuests.flatMap((quest) => quest.blockers));
  const segments = QUEST_ROUTE_LEVEL_SEGMENTS.map((segment) => {
    const segmentQuests = routeQuests.filter((quest) =>
      quest.eligibility.minLevel >= segment.minLevel &&
      quest.eligibility.minLevel <= Math.min(segment.maxLevel, acceptanceLevel)
    );
    return {
      ...segment,
      questCount: segmentQuests.length,
      questIds: segmentQuests.map((quest) => quest.questId),
      specialHandlers: countValues(segmentQuests.flatMap((quest) => quest.specialHandlers)),
      blockedQuestIds: segmentQuests.filter((quest) => quest.blockers.length > 0).map((quest) => quest.questId),
    };
  });

  return {
    schema: "mir2-real-client-quest-route/3",
    source: {
      questDbVersion: Number(sources.questManifest?.crystal_db_version ?? 0),
      questDbCustomVersion: Number(sources.questManifest?.crystal_db_custom_version ?? 0),
      questCount: quests.length,
      runtimeProfileId: runtimeProfile.profileId,
      runtimeProfileVersion: runtimeProfile.version,
    },
    className,
    classMask,
    maxLevel: acceptanceLevel,
    routeQuestCount: routeQuests.length,
    segments,
    capabilityMatrix: handlerCounts,
    blockerMatrix: blockerCounts,
    quests: routeQuests,
  };
}

export async function buildAuthoritativeClassQuestRoute(options = {}) {
  return buildClassQuestRoute(await loadCrystalQuestRouteSources(), options);
}

/**
 * Build a directed map graph from the same Crystal MapInfo movements sent by
 * the simulation. It contains topology only: the runner still resolves and
 * enters the live transfer bounds exposed by the current client snapshot.
 */
export function buildMapTravelGraph(sources, { respectRuntimeProfile = true } = {}) {
  const runtimeProfile = buildRuntimeProfileIndex(sources?.contentProfile);
  const allMaps = Array.isArray(sources?.respawnManifest?.maps)
    ? sources.respawnManifest.maps
    : [];
  const maps = respectRuntimeProfile && runtimeProfile.enabled
    ? allMaps.filter((map) => runtimeProfile.maps.has(normalizeMapFileName(map?.map_file_name)))
    : allMaps;
  const mapByIndex = new Map(
    maps.map((map) => [Number(map.map_index), map]),
  );
  const nodes = maps.map((map) => ({
    mapIndex: Number(map.map_index),
    mapFileName: String(map.map_file_name),
    mapTitle: String(map.map_title),
  }));
  const edges = [];

  for (const map of maps) {
    const grouped = new Map();
    for (const movement of map.movements ?? []) {
      const destination = mapByIndex.get(Number(movement.map_index));
      if (!destination) continue;
      const key = [
        String(destination.map_file_name),
        Boolean(movement.need_hole),
        Boolean(movement.need_move),
      ].join("|");
      const edge = grouped.get(key) ?? {
        kind: "map-movement",
        fromMapFileName: String(map.map_file_name),
        fromMapTitle: String(map.map_title),
        toMapFileName: String(destination.map_file_name),
        toMapTitle: String(destination.map_title),
        needHole: Boolean(movement.need_hole),
        needMove: Boolean(movement.need_move),
        portals: [],
      };
      edge.portals.push({
        source: {
          x: Number(movement.source?.x),
          y: Number(movement.source?.y),
        },
        destination: {
          x: Number(movement.destination?.x),
          y: Number(movement.destination?.y),
        },
      });
      grouped.set(key, edge);
    }
    edges.push(...grouped.values());
  }

  edges.push(...buildVisibleNpcScriptTransferEdges(sources, {
    maps,
    runtimeProfile,
    respectRuntimeProfile,
  }));

  return {
    schema: "mir2-real-client-map-graph/2",
    nodes,
    edges: edges.sort((left, right) =>
      left.fromMapFileName.localeCompare(right.fromMapFileName, undefined, { numeric: true }) ||
      left.toMapFileName.localeCompare(right.toMapFileName, undefined, { numeric: true }) ||
      left.kind.localeCompare(right.kind) ||
      Number(left.needHole) - Number(right.needHole) ||
      Number(left.needMove) - Number(right.needMove)
    ),
  };
}

/** Return the lowest-cost directed Crystal movement path, including its edges. */
export function findMapTravelRoute(graph, fromMapFileName, toMapFileName) {
  const from = String(fromMapFileName);
  const to = String(toMapFileName);
  if (from === to) return [];
  const outgoing = new Map();
  for (const edge of graph?.edges ?? []) {
    const list = outgoing.get(String(edge.fromMapFileName)) ?? [];
    list.push(edge);
    outgoing.set(String(edge.fromMapFileName), list);
  }

  // Dijkstra rather than plain BFS: ordinary visible walk transfers cost 1,
  // while hole/forced-move portals remain usable fallbacks but are less likely
  // to be selected as surprising shortcuts through a dungeon.
  const distance = new Map([[from, 0]]);
  const previous = new Map();
  const pending = [{ mapFileName: from, cost: 0 }];
  while (pending.length) {
    pending.sort((left, right) => left.cost - right.cost || left.mapFileName.localeCompare(right.mapFileName));
    const current = pending.shift();
    if (!current || current.cost !== distance.get(current.mapFileName)) continue;
    if (current.mapFileName === to) break;
    for (const edge of outgoing.get(current.mapFileName) ?? []) {
      const edgeCost = edge.kind === "npc-script"
        ? 8 + Math.max(1, Number(edge.targetSequence?.length ?? 0))
        : 1 + (edge.needMove ? 2 : 0) + (edge.needHole ? 4 : 0);
      const nextCost = current.cost + edgeCost;
      if (nextCost >= (distance.get(edge.toMapFileName) ?? Number.POSITIVE_INFINITY)) continue;
      distance.set(edge.toMapFileName, nextCost);
      previous.set(edge.toMapFileName, edge);
      pending.push({ mapFileName: edge.toMapFileName, cost: nextCost });
    }
  }
  if (!previous.has(to)) return null;
  const route = [];
  let cursor = to;
  while (cursor !== from) {
    const edge = previous.get(cursor);
    if (!edge) return null;
    route.push(edge);
    cursor = edge.fromMapFileName;
  }
  route.reverse();
  return route;
}

/** Minimum starting balance that satisfies every strict scripted-travel check. */
export function minimumStartingGoldForMapTravelEdges(edges) {
  let alreadySpent = 0;
  let required = 0;
  for (const edge of edges ?? []) {
    if (edge?.kind !== "npc-script") continue;
    const minimumExclusive = Number(edge.minimumGoldExclusive);
    if (Number.isFinite(minimumExclusive)) {
      required = Math.max(required, alreadySpent + minimumExclusive + 1);
    }
    alreadySpent += Math.max(0, Number(edge.goldCost ?? 0));
  }
  return required;
}

/**
 * Derive paid/scripted travel only from an enabled NPC placement, an enabled
 * Crystal script, and a click path that begins at that script's visible main
 * dialog. The runner must still walk to the NPC and physically click every
 * target in `targetSequence`; this graph edge is planning metadata, never a
 * direct MOVE command.
 */
export function buildVisibleNpcScriptTransferEdges(
  sources,
  { maps = null, runtimeProfile = null, respectRuntimeProfile = true } = {},
) {
  const profile = runtimeProfile ?? buildRuntimeProfileIndex(sources?.contentProfile);
  const allMaps = Array.isArray(maps)
    ? maps
    : Array.isArray(sources?.respawnManifest?.maps)
      ? sources.respawnManifest.maps
      : [];
  const mapByFileName = new Map(
    allMaps.map((map) => [normalizeMapFileName(map?.map_file_name), map]),
  );
  const scripts = Array.isArray(sources?.npcScriptManifest?.scripts)
    ? sources.npcScriptManifest.scripts
    : [];
  const scriptByKey = new Map(
    scripts.map((script) => [normalizeScriptKey(script?.script_key), script]),
  );
  const npcs = Array.isArray(sources?.npcInfoManifest?.npcs)
    ? sources.npcInfoManifest.npcs
    : [];
  const edges = [];

  for (const npc of npcs) {
    const sourceMapKey = normalizeMapFileName(npc?.map_file_name);
    const scriptKey = String(npc?.script_key ?? npc?.file_name ?? "");
    if (!sourceMapKey || !mapByFileName.has(sourceMapKey)) continue;
    if (
      respectRuntimeProfile && profile.enabled &&
      !profile.maps.has(sourceMapKey)
    ) continue;
    if (
      respectRuntimeProfile && profile.enabled &&
      !profile.npcScripts.has(normalizeScriptKey(scriptKey))
    ) continue;
    const script = scriptByKey.get(normalizeScriptKey(scriptKey));
    if (!script) continue;
    const sections = Array.isArray(script.sections) ? script.sections : [];
    const sectionByLabel = new Map(
      sections.map((section) => [normalizeScriptLabel(section.label), section]),
    );
    const starts = ["@main", "main"]
      .map(normalizeScriptLabel)
      .filter((label) => sectionByLabel.has(label));
    if (!starts.length) continue;

    for (const section of sections) {
      const move = scriptedMoveAction(section);
      if (!move) continue;
      const destinationMapKey = normalizeMapFileName(move.mapFileName);
      const destinationMap = mapByFileName.get(destinationMapKey);
      if (!destinationMap) continue;
      if (
        respectRuntimeProfile && profile.enabled &&
        !profile.maps.has(destinationMapKey)
      ) continue;
      const targetSequence = clickablePathToScriptLabel(
        sectionByLabel,
        starts,
        normalizeScriptLabel(section.label),
      );
      // A scripted MOVE that runs merely by opening a dialog is not an
      // auditable player choice. Keep only transfers with at least one
      // rendered link the runner can physically click.
      if (targetSequence == null || targetSequence.length === 0) continue;
      const sourceMap = mapByFileName.get(sourceMapKey);
      const gold = scriptedGoldPolicy(section);
      const items = scriptedItemPolicy(section);
      edges.push({
        kind: "npc-script",
        fromMapFileName: String(sourceMap.map_file_name),
        fromMapTitle: String(sourceMap.map_title),
        toMapFileName: String(destinationMap.map_file_name),
        toMapTitle: String(destinationMap.map_title),
        needHole: false,
        needMove: false,
        portals: [],
        scriptKey,
        targetSequence,
        destination: { x: move.x, y: move.y },
        goldCost: gold.cost,
        minimumGoldExclusive: gold.minimumExclusive,
        requiredItems: items.required,
        itemCosts: items.costs,
        npc: npcRouteBinding(npc, new Map([[scriptKey, script]])),
      });
    }
  }

  const seen = new Set();
  return edges.filter((edge) => {
    const key = [
      edge.fromMapFileName,
      edge.toMapFileName,
      edge.npc?.objectId,
      edge.scriptKey,
      edge.targetSequence.join("|"),
    ].join(":");
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

function scriptedMoveAction(section) {
  for (const line of section?.lines ?? []) {
    const match = String(line).trim().match(/^MOVE\s+(\S+)\s+(-?\d+)\s+(-?\d+)\s*$/i);
    if (!match) continue;
    return {
      mapFileName: match[1],
      x: Number(match[2]),
      y: Number(match[3]),
    };
  }
  return null;
}

function scriptedGoldPolicy(section) {
  let cost = 0;
  let minimumExclusive = null;
  for (const line of section?.lines ?? []) {
    const take = String(line).trim().match(/^TAKEGOLD\s+(\d+)\s*$/i);
    if (take) cost += Number(take[1]);
    const condition = String(line).trim().match(/^CHECKGOLD\s+>\s+(\d+)\s*$/i);
    if (condition) {
      minimumExclusive = Math.max(minimumExclusive ?? 0, Number(condition[1]));
    }
  }
  return { cost, minimumExclusive };
}

function scriptedItemPolicy(section) {
  const required = new Map();
  const costs = new Map();
  for (const line of section?.lines ?? []) {
    const condition = String(line).trim().match(/^CHECKITEM\s+(\S+)(?:\s+(\d+))?\s*$/i);
    if (condition) {
      const item = condition[1];
      required.set(item, Math.max(required.get(item) ?? 0, Number(condition[2] ?? 1)));
    }
    const take = String(line).trim().match(/^TAKEITEM\s+(\S+)(?:\s+(\d+))?\s*$/i);
    if (take) {
      const item = take[1];
      costs.set(item, (costs.get(item) ?? 0) + Number(take[2] ?? 1));
    }
  }
  return {
    required: [...required].map(([item, count]) => ({ item, count })),
    costs: [...costs].map(([item, count]) => ({ item, count })),
  };
}

/**
 * Produce non-boss, experience-bearing grind choices grouped with real spawn
 * locations. The policy can prefer the current map and a level band without
 * inventing private teleport or spawn commands.
 */
export function buildGrindingCatalog(sources) {
  const runtimeProfile = buildRuntimeProfileIndex(sources?.contentProfile);
  const maps = Array.isArray(sources?.respawnManifest?.maps)
    ? sources.respawnManifest.maps
    : [];
  const monsters = Array.isArray(sources?.monsterManifest?.monsters)
    ? sources.monsterManifest.monsters
    : [];
  const rows = flattenRespawns(maps, monsters, sources?.contentProfile)
    .filter((spawn) =>
      spawn.monsterLevel > 0 && spawn.monsterExperience > 0 &&
      spawn.monsterHp > 0 && spawn.isBoss !== true && spawn.count > 0 &&
      (!runtimeProfile.enabled || (
        runtimeProfile.maps.has(normalizeMapFileName(spawn.mapFileName)) &&
        runtimeProfile.monsters.has(normalizeName(spawn.monsterName))
      ))
    );
  const byMonster = new Map();
  for (const spawn of rows) {
    const key = `${spawn.monsterIndex}|${normalizeName(spawn.monsterName)}`;
    const entry = byMonster.get(key) ?? {
      monsterIndex: spawn.monsterIndex,
      monsterName: spawn.monsterName,
      ai: spawn.monsterAi,
      level: spawn.monsterLevel,
      hp: spawn.monsterHp,
      experience: spawn.monsterExperience,
      spawns: [],
    };
    entry.spawns.push(spawn);
    byMonster.set(key, entry);
  }
  return [...byMonster.values()]
    .map((entry) => ({
      ...entry,
      spawns: entry.spawns
        .sort((left, right) => right.count - left.count || left.delayMinutes - right.delayMinutes)
        .slice(0, 24),
    }))
    .sort((left, right) =>
      left.level - right.level || right.experience - left.experience || left.monsterName.localeCompare(right.monsterName)
    );
}

/** Decode the reward vectors immediately following the fixed header. */
export function decodeCrystalQuestRewards(payloadHex, offset = undefined) {
  const cursor = new PacketCursor(payloadHex);
  cursor.offset = offset ?? decodeCrystalQuestHeader(payloadHex).bytesRead;
  return {
    fixedItems: cursor.questItemRewardVector(),
    selectableItems: cursor.questItemRewardVector(),
  };
}

function npcRouteBinding(npc, scriptByKey) {
  if (!npc) return null;
  const script = scriptByKey.get(String(npc.file_name)) ?? null;
  return {
    objectId: Number(npc.loaded_object_id),
    databaseIndex: Number(npc.npc_index),
    name: String(npc.name),
    mapFileName: String(npc.map_file_name),
    position: {
      x: Number(npc.location?.x),
      y: Number(npc.location?.y),
    },
    scriptKey: String(npc.file_name),
    script: script ? {
      commandDirectives: [...new Set(script.command_directives ?? [])].sort(),
      labels: (script.labels ?? []).map((entry) => String(entry.label)),
      inserts: (script.inserts ?? []).map((entry) => ({
        targetPath: String(entry.target_path),
        targetLabel: String(entry.target_label),
      })),
    } : null,
  };
}

function flattenRespawns(maps, monsters = [], profile = null) {
  const monsterByIndex = new Map(
    monsters.map((monster) => [Number(monster.monster_index), monster]),
  );
  const imported = maps.flatMap((map) => (map.respawns ?? []).map((respawn) => ({
    monsterIndex: Number(respawn.monster_index),
    monsterName: String(respawn.monster_name),
    monsterAi: Number(respawn.monster_ai),
    monsterHp: Number(respawn.monster_hp),
    monsterLevel: Number(monsterByIndex.get(Number(respawn.monster_index))?.level ?? 0),
    monsterExperience: Number(monsterByIndex.get(Number(respawn.monster_index))?.experience ?? 0),
    isBoss: monsterByIndex.get(Number(respawn.monster_index))?.is_boss === true,
    mapFileName: String(map.map_file_name),
    mapTitle: String(map.map_title),
    position: {
      x: Number(respawn.location?.x),
      y: Number(respawn.location?.y),
    },
    count: Number(respawn.count),
    spread: Number(respawn.spread),
    delayMinutes: Number(respawn.delay_minutes),
    respawnIndex: Number(respawn.respawn_index),
  })));
  const mapByName = new Map(
    maps.map((map) => [normalizeMapFileName(map.map_file_name), map]),
  );
  const monsterByName = new Map(
    monsters.map((monster) => [normalizeName(monster.name), monster]),
  );
  const overrides = (profile?.respawnOverrides ?? []).flatMap((rule, ruleIndex) => {
    const map = mapByName.get(normalizeMapFileName(rule.mapFileName));
    const monster = monsterByName.get(normalizeName(rule.monster));
    if (!map || !monster) return [];
    return [{
      monsterIndex: Number(monster.monster_index),
      monsterName: String(monster.name),
      monsterAi: Number(monster.ai),
      monsterHp: Number(monster.hp),
      monsterLevel: Number(monster.level),
      monsterExperience: Number(monster.experience),
      isBoss: monster.is_boss === true,
      mapFileName: String(map.map_file_name),
      mapTitle: String(map.map_title),
      position: {
        x: Number(rule.position?.x),
        y: Number(rule.position?.y),
      },
      count: Number(rule.count),
      spread: Number(rule.spread),
      delayMinutes: Number(rule.delayMinutes),
      respawnIndex: 10_000 + ruleIndex,
      profileRespawn: true,
      sourceQuestId: Number(rule.sourceQuestId),
      sourceNote: String(rule.sourceNote ?? ""),
    }];
  });
  return [...imported, ...overrides];
}

function spawnCandidatesForTask(spawnRows, task) {
  const monsterIndex = Number(task.monster_index);
  const monsterName = normalizeName(task.monster_name);
  return spawnRows
    .filter((spawn) =>
      spawn.monsterIndex === monsterIndex || normalizeName(spawn.monsterName) === monsterName
    )
    .sort((left, right) =>
      right.count - left.count || left.delayMinutes - right.delayMinutes ||
      left.mapFileName.localeCompare(right.mapFileName) || left.respawnIndex - right.respawnIndex
    )
    .slice(0, 16);
}

function flattenQuestItemDrops(tables) {
  return tables.flatMap((table) => (table.sections ?? []).flatMap((section) =>
    (section.entries ?? [])
      .filter((entry) => normalizeName(entry.item_name) !== "gold")
      .map((entry) => ({
        tableKey: String(table.table_key),
        monsterName: String(table.table_key).split("/").at(-1) ?? "",
        itemName: String(entry.item_name),
        chanceNumerator: Number(entry.chance_numerator),
        chanceDenominator: Number(entry.chance_denominator),
        amount: entry.amount == null ? null : Number(entry.amount),
        modifiers: [...(entry.modifiers ?? [])],
        questRequired: (entry.modifiers ?? []).includes("Q"),
      }))
  ));
}

function flattenProfileDropOverrides(profile) {
  return (profile?.dropOverrides ?? []).map((entry) => ({
    tableKey: `profile:${String(profile?.profileId ?? "unconfigured")}`,
    profileOverride: true,
    monsterName: String(entry.monster),
    mapFileName: entry.mapFileName == null ? null : String(entry.mapFileName),
    itemName: String(entry.item),
    chanceNumerator: Number(entry.chanceNumerator),
    chanceDenominator: Number(entry.chanceDenominator),
    amount: null,
    modifiers: entry.questRequired === true ? ["Q"] : [],
    questRequired: entry.questRequired === true,
    sourceNote: entry.sourceNote == null ? null : String(entry.sourceNote),
  }));
}

function itemSourcesForTask(itemDropRows, spawnRows, task) {
  const wanted = normalizeName(task.item_name);
  return itemDropRows
    .filter((drop) => normalizeName(drop.itemName) === wanted)
    .map((drop) => {
      const exactSpawns = spawnRows.filter((spawn) =>
        normalizeName(spawn.monsterName) === normalizeName(drop.monsterName) &&
        (drop.mapFileName == null ||
          normalizeMapFileName(spawn.mapFileName) === normalizeMapFileName(drop.mapFileName))
      );
      return {
        ...drop,
        requiresHarvest: exactSpawns.some((spawn) => harvestableMonsterAi(spawn.monsterAi)),
        spawnCandidates: exactSpawns
          .sort((left, right) => right.count - left.count || left.delayMinutes - right.delayMinutes)
          .slice(0, 16),
      };
    })
    .sort((left, right) =>
      left.chanceDenominator - right.chanceDenominator || left.tableKey.localeCompare(right.tableKey)
    );
}

function specialHandlersForQuest({
  header,
  startNpcIndex,
  finishNpcIndex,
  startNpc,
  finishNpc,
  killObjectives,
  itemObjectives,
  carryObjectives,
  flagObjectives,
}) {
  const handlers = [];
  if (startNpcIndex === 0) handlers.push("quest-diary-accept");
  if (finishNpcIndex === 0) handlers.push("quest-diary-finish");
  if (killObjectives.length) handlers.push("kill-objective");
  if (itemObjectives.length) handlers.push("item-drop-objective");
  if (itemObjectives.some((task) => task.sources.some((source) => source.requiresHarvest))) {
    handlers.push("harvest-objective");
  }
  if (carryObjectives.length) handlers.push("carry-delivery");
  if (flagObjectives.length) handlers.push("flag-script-objective");
  if (header.timeLimitInSeconds > 0) handlers.push("timed-quest");
  if (header.questNeeded > 0) handlers.push("prerequisite-chain");
  if (startNpc && finishNpc && startNpc.mapFileName !== finishNpc.mapFileName) {
    handlers.push("cross-map-dialogue");
  }
  if (!killObjectives.length && !itemObjectives.length && !flagObjectives.length) {
    handlers.push("dialogue-objective");
  }
  return [...new Set(handlers)].sort();
}

function routeBlockers({
  startNpcIndex,
  finishNpcIndex,
  startNpc,
  finishNpc,
  killObjectives,
  itemObjectives,
  flagObjectives,
}) {
  const blockers = [];
  if (startNpcIndex !== 0 && !startNpc) blockers.push("start NPC has no loaded-object binding");
  if (finishNpcIndex !== 0 && !finishNpc) blockers.push("finish NPC has no loaded-object binding");
  for (const objective of killObjectives) {
    if (!objective.spawnCandidates.length) {
      blockers.push(`no real respawn for ${objective.monsterName}`);
    }
  }
  for (const objective of itemObjectives) {
    if (!objective.sources.length) blockers.push(`no drop source for ${objective.itemName}`);
    else if (objective.sources.every((source) => !source.spawnCandidates.length)) {
      blockers.push(`Q-drop source for ${objective.itemName} has no real respawn`);
    }
  }
  for (const objective of flagObjectives) {
    if (!objective.setters?.length) {
      blockers.push(`flag ${objective.number} has no visible scripted-world setter`);
    }
  }
  return [...new Set(blockers)].sort();
}

function buildRuntimeProfileIndex(profile) {
  const mapRows = Array.isArray(profile?.mapWhitelist) ? profile.mapWhitelist : [];
  const monsterRows = Array.isArray(profile?.monsterWhitelist) ? profile.monsterWhitelist : [];
  const npcScriptRows = Array.isArray(profile?.npcScriptWhitelist)
    ? profile.npcScriptWhitelist
    : [];
  const questPrerequisiteRows = Array.isArray(profile?.questPrerequisiteOverrides)
    ? profile.questPrerequisiteOverrides
    : [];
  const questRewardRows = Array.isArray(profile?.questRewardOverrides)
    ? profile.questRewardOverrides
    : [];
  const questRewardOverrides = new Map();
  for (const row of questRewardRows) {
    const questId = Number(row?.questId);
    const list = questRewardOverrides.get(questId) ?? [];
    list.push({
      item: String(row?.item ?? ""),
      count: Number(row?.count ?? 0),
      sourceNote: String(row?.sourceNote ?? ""),
    });
    questRewardOverrides.set(questId, list);
  }
  return {
    enabled: Boolean(profile && mapRows.length && monsterRows.length),
    profileId: String(profile?.profileId ?? "unconfigured"),
    version: Number(profile?.version ?? 0),
    maps: new Set(mapRows.map((row) => normalizeMapFileName(row?.fileName))),
    monsters: new Set(monsterRows.map((name) => normalizeName(name))),
    npcScripts: new Set(npcScriptRows.map(normalizeScriptKey)),
    questPrerequisiteOverrides: new Map(questPrerequisiteRows.map((row) => [
      Number(row?.questId),
      {
        requiredQuestId: Number(row?.requiredQuestId),
        sourceNote: String(row?.sourceNote ?? ""),
      },
    ])),
    questRewardOverrides,
  };
}

function runtimeProfileBlockers({
  profile,
  startNpc,
  finishNpc,
  killObjectives,
  itemObjectives,
  flagObjectives,
}) {
  if (!profile?.enabled) return [];
  const blockers = [];
  const mapAllowed = (fileName) => profile.maps.has(normalizeMapFileName(fileName));
  const monsterAllowed = (name) => profile.monsters.has(normalizeName(name));
  const npcScriptAllowed = (scriptKey) => profile.npcScripts.has(normalizeScriptKey(scriptKey));

  if (startNpc && !mapAllowed(startNpc.mapFileName)) {
    blockers.push(`runtime profile disallows start NPC map ${startNpc.mapFileName}`);
  }
  if (startNpc && !npcScriptAllowed(startNpc.scriptKey)) {
    blockers.push(`runtime profile disallows start NPC script ${startNpc.scriptKey}`);
  }
  if (finishNpc && !mapAllowed(finishNpc.mapFileName)) {
    blockers.push(`runtime profile disallows finish NPC map ${finishNpc.mapFileName}`);
  }
  if (finishNpc && !npcScriptAllowed(finishNpc.scriptKey)) {
    blockers.push(`runtime profile disallows finish NPC script ${finishNpc.scriptKey}`);
  }
  for (const objective of killObjectives) {
    if (!monsterAllowed(objective.monsterName)) {
      blockers.push(`runtime profile disallows monster ${objective.monsterName}`);
    } else if (
      objective.spawnCandidates.length > 0 &&
      !objective.spawnCandidates.some((spawn) => mapAllowed(spawn.mapFileName))
    ) {
      blockers.push(`runtime profile has no allowed respawn map for ${objective.monsterName}`);
    }
  }
  for (const objective of itemObjectives) {
    const sourcesWithSpawns = objective.sources.filter((source) => source.spawnCandidates.length > 0);
    if (
      sourcesWithSpawns.length > 0 &&
      !sourcesWithSpawns.some((source) =>
        monsterAllowed(source.monsterName) &&
        source.spawnCandidates.some((spawn) => mapAllowed(spawn.mapFileName))
      )
    ) {
      blockers.push(`runtime profile has no allowed source for ${objective.itemName}`);
    }
  }
  for (const objective of flagObjectives) {
    if (
      objective.setters?.length > 0 &&
      !objective.setters.some((setter) =>
        mapAllowed(setter.npc?.mapFileName) &&
        npcScriptAllowed(setter.scriptKey ?? setter.npc?.scriptKey)
      )
    ) {
      blockers.push(`runtime profile has no allowed flag setter ${objective.number}`);
    }
  }
  return [...new Set(blockers)].sort();
}

function buildFlagSetterBindings(npcs, scripts, scriptByKey) {
  const npcsByScript = new Map();
  for (const npc of npcs) {
    const key = String(npc.file_name);
    const list = npcsByScript.get(key) ?? [];
    list.push(npc);
    npcsByScript.set(key, list);
  }
  const result = new Map();
  for (const script of scripts) {
    const flags = [...String(script.raw_text ?? "").matchAll(/\bSET\s+\[(\d+)]\s+1\b/gi)]
      .map((match) => Number(match[1]));
    if (!flags.length) continue;
    const bindings = npcsByScript.get(String(script.script_key)) ?? [];
    if (!bindings.length) continue;
    for (const flagNumber of new Set(flags)) {
      const targetSequences = clickablePathsToFlagSetter(script, flagNumber);
      if (!targetSequences.length) continue;
      const list = result.get(flagNumber) ?? [];
      for (const npc of bindings) {
        for (const targetSequence of targetSequences) {
          list.push({
            npc: npcRouteBinding(npc, scriptByKey),
            targetSequence,
            scriptKey: String(script.script_key),
          });
        }
      }
      result.set(flagNumber, deduplicateFlagSetters(list));
    }
  }
  return result;
}

function clickablePathsToFlagSetter(script, flagNumber) {
  const sections = Array.isArray(script?.sections) ? script.sections : [];
  const sectionByLabel = new Map(
    sections.map((section) => [normalizeScriptLabel(section.label), section]),
  );
  const starts = ["@main", "main"]
    .map(normalizeScriptLabel)
    .filter((label) => sectionByLabel.has(label));
  const targets = new Set(
    sections
      .filter((section) => (section.lines ?? []).some((line) =>
        new RegExp(`\\bSET\\s+\\[${Number(flagNumber)}]\\s+1\\b`, "i").test(String(line))
      ))
      .map((section) => normalizeScriptLabel(section.label)),
  );
  if (!starts.length || !targets.size) return [];

  return [...targets]
    .map((target) => clickablePathToScriptLabel(sectionByLabel, starts, target))
    .filter((path) => path != null);
}

function clickablePathToScriptLabel(sectionByLabel, starts, target) {
  const pending = starts.map((label) => ({ label, cost: 0, clicks: [] }));
  const best = new Map(starts.map((label) => [label, 0]));
  while (pending.length) {
    pending.sort((left, right) => left.cost - right.cost || left.clicks.length - right.clicks.length);
    const current = pending.shift();
    if (!current || current.cost !== best.get(current.label)) continue;
    if (current.label === target) return current.clicks;
    const section = sectionByLabel.get(current.label);
    if (!section) continue;
    for (const edge of scriptSectionEdges(section)) {
      if (!sectionByLabel.has(edge.label)) continue;
      const nextCost = current.cost + edge.cost;
      if (nextCost >= (best.get(edge.label) ?? Number.POSITIVE_INFINITY)) continue;
      best.set(edge.label, nextCost);
      pending.push({
        label: edge.label,
        cost: nextCost,
        clicks: edge.cost === 0 ? current.clicks : [...current.clicks, edge.target],
      });
    }
  }
  return null;
}

function scriptSectionEdges(section) {
  const edges = [];
  for (const line of section.lines ?? []) {
    for (const match of String(line).matchAll(/\bGOTO\s+(@?[^\s<]+)/gi)) {
      edges.push({ label: normalizeScriptLabel(match[1]), target: match[1], cost: 0 });
    }
    for (const match of String(line).matchAll(/<[^<>]*?\/(@[^<>\s]+)>/g)) {
      edges.push({ label: normalizeScriptLabel(match[1]), target: match[1], cost: 1 });
    }
  }
  return edges;
}

function normalizeScriptLabel(value) {
  return String(value ?? "").trim().replace(/[>]+$/g, "").toLowerCase();
}

function normalizeScriptKey(value) {
  return String(value ?? "").trim().replace(/\\/g, "/").toLowerCase();
}

function deduplicateFlagSetters(setters) {
  const seen = new Set();
  return setters.filter((setter) => {
    const key = [setter.npc.objectId, setter.scriptKey, setter.targetSequence.join("|")].join(":");
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

function harvestableMonsterAi(ai) {
  // Crystal's MonsterObject.GetMonster factory maps these AIs to
  // HarvestMonster itself or a transitive subclass (Deer, SpittingSpider,
  // CannibalPlant, CaveMaggot, ToxicGhoul, SandWorm, and CreeperPlant).
  // Their Drop() implementation is intentionally empty: items are resolved
  // only after the client performs the corpse-harvest passes.
  return [1, 2, 4, 5, 7, 9, 28, 35, 153].includes(Number(ai));
}

function countValues(values) {
  return Object.fromEntries(
    [...values.reduce((counts, value) => {
      counts.set(value, (counts.get(value) ?? 0) + 1);
      return counts;
    }, new Map())].sort(([left], [right]) => left.localeCompare(right)),
  );
}

function normalizeName(value) {
  return String(value ?? "").replace(/[^a-z0-9]/gi, "").toLowerCase();
}

function normalizeMapFileName(value) {
  return String(value ?? "").trim().replace(/\.map$/i, "").toLowerCase();
}

function finiteInteger(value, fallback) {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? Math.trunc(parsed) : fallback;
}

async function readJson(url) {
  return JSON.parse(await fs.readFile(url, "utf8"));
}

class PacketCursor {
  constructor(payloadHex) {
    if (typeof payloadHex !== "string" || payloadHex.length % 2 !== 0 || /[^0-9a-f]/i.test(payloadHex)) {
      throw new Error("Crystal quest payload must be even-length hexadecimal");
    }
    this.bytes = Buffer.from(payloadHex, "hex");
    this.offset = 0;
  }

  ensure(length) {
    if (this.offset + length > this.bytes.length) {
      throw new Error(`Crystal quest payload ended at ${this.offset}; need ${length} more bytes`);
    }
  }

  u8() {
    this.ensure(1);
    return this.bytes[this.offset++];
  }

  i32() {
    this.ensure(4);
    const value = this.bytes.readInt32LE(this.offset);
    this.offset += 4;
    return value;
  }

  u32() {
    this.ensure(4);
    const value = this.bytes.readUInt32LE(this.offset);
    this.offset += 4;
    return value;
  }


  u16() {
    this.ensure(2);
    const value = this.bytes.readUInt16LE(this.offset);
    this.offset += 2;
    return value;
  }

  bool() {
    return this.u8() !== 0;
  }

  sevenBitLength() {
    let result = 0;
    let shift = 0;
    for (let index = 0; index < 5; index += 1) {
      const byte = this.u8();
      result |= (byte & 0x7f) << shift;
      if ((byte & 0x80) === 0) return result >>> 0;
      shift += 7;
    }
    throw new Error("invalid 7-bit encoded Crystal string length");
  }

  string() {
    const length = this.sevenBitLength();
    this.ensure(length);
    const value = this.bytes.toString("utf8", this.offset, this.offset + length);
    this.offset += length;
    return value;
  }

  stringVector() {
    const count = this.i32();
    if (count < 0 || count > 10_000) throw new Error(`invalid Crystal string vector count ${count}`);
    return Array.from({ length: count }, () => this.string());
  }


  questItemRewardVector() {
    const count = this.i32();
    if (count < 0 || count > 10_000) throw new Error(`invalid Crystal reward vector count ${count}`);
    return Array.from({ length: count }, (_, selectionIndex) => {
      const item = this.itemInfo();
      return {
        selectionIndex,
        itemIndex: item.itemIndex,
        itemName: item.itemName,
        itemImage: item.itemImage,
        requiredClass: item.requiredClass,
        requiredGender: item.requiredGender,
        count: this.u16(),
      };
    });
  }

  itemInfo() {
    const itemIndex = this.i32();
    const itemName = this.string();
    const itemType = this.u8();
    const grade = this.u8();
    const requiredType = this.u8();
    const requiredClass = this.u8();
    const requiredGender = this.u8();
    this.u8(); // item set
    this.u16(); // signed shape; width is all the route builder needs
    this.u8(); // weight
    this.u8(); // light
    this.u8(); // required amount
    const itemImage = this.u16();
    this.u16(); // durability
    this.u16(); // stack size
    this.u32(); // price
    this.bool(); // start item
    this.u8(); // effect
    this.u8(); // packed bools
    this.u16(); // signed bind
    this.u16(); // signed unique
    this.u8(); // random stats id
    this.bool(); // can fast run
    this.bool(); // can awakening
    this.u8(); // slots
    const statCount = this.i32();
    if (statCount < 0 || statCount > 10_000) throw new Error(`invalid Crystal item stat count ${statCount}`);
    for (let index = 0; index < statCount; index += 1) {
      this.u8();
      this.i32();
    }
    if (this.bool()) this.string();
    return {
      itemIndex,
      itemName,
      itemImage,
      itemType,
      grade,
      requiredType,
      requiredClass,
      requiredGender,
    };
  }
}
