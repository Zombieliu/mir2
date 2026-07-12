import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import sharp from "sharp";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(SCRIPT_DIR, "..", "..", "..");
const DEFAULT_INPUT_DIR = path.resolve(
  REPO_ROOT,
  "docs",
  "generated",
  "player-qa",
  "r310-visual-watch",
);
const DEFAULT_OUTPUT_DIR = path.resolve(REPO_ROOT, "docs", "generated", "player-qa", "visual-parity");
const DEFAULT_MAX_SAMPLES = 12;
const TARGET_STAGE = { width: 1024, height: 768 };
const DEFAULT_HUD_TOP = 616;

const args = parseArgs(process.argv.slice(2));
const inputDir = path.resolve(args.input ?? args.inputDir ?? DEFAULT_INPUT_DIR);
const outputDir = path.resolve(args.output ?? args.outputDir ?? DEFAULT_OUTPUT_DIR);
const prefix = args.prefix ?? `visual-parity-${timestamp()}`;
const maxSamples = numberArg(args.maxSamples ?? args.limit, DEFAULT_MAX_SAMPLES);

await main();

async function main() {
  await fs.mkdir(outputDir, { recursive: true });
  const pairs = await discoverPairs(inputDir, maxSamples);
  if (pairs.length === 0) {
    throw new Error(`No visual watch pairs found in ${inputDir}`);
  }

  const samples = [];
  for (const pair of pairs) {
    samples.push(await analyzePair(pair));
  }

  const aggregate = aggregateSamples(samples);
  const report = {
    generatedAt: new Date().toISOString(),
    inputDir,
    sampleCount: samples.length,
    aggregate,
    samples,
  };

  const jsonPath = path.join(outputDir, `${prefix}.json`);
  const markdownPath = path.join(outputDir, `${prefix}.md`);
  await fs.writeFile(jsonPath, `${JSON.stringify(report, null, 2)}\n`);
  await fs.writeFile(markdownPath, renderMarkdown(report));

  console.log(JSON.stringify({ ok: true, jsonPath, markdownPath, aggregate }, null, 2));
}

async function discoverPairs(dir, maxCount) {
  const entries = await fs.readdir(dir, { withFileTypes: true });
  const files = await Promise.all(
    entries
      .filter((entry) => entry.isFile())
      .map(async (entry) => {
        const fullPath = path.join(dir, entry.name);
        const stat = await fs.stat(fullPath);
        return { name: entry.name, path: fullPath, mtimeMs: stat.mtimeMs };
      }),
  );
  const byName = new Map(files.map((file) => [file.name, file]));
  const pairs = [];

  for (const original of files.filter((file) => file.name.endsWith("-original.png"))) {
    const samplePrefix = original.name.replace(/-original\.png$/i, "");
    const web = byName.get(`${samplePrefix}-web.png`);
    const state = byName.get(`${samplePrefix}-web-state.json`);
    const nativeState = byName.get("native-account-state.json");
    if (!web || !state) continue;
    pairs.push({
      prefix: samplePrefix,
      originalPath: original.path,
      webPath: web.path,
      webStatePath: state.path,
      nativeStatePath: nativeState?.path ?? null,
      mtimeMs: Math.max(original.mtimeMs, web.mtimeMs, state.mtimeMs),
    });
  }

  pairs.sort((a, b) => b.mtimeMs - a.mtimeMs);
  return pairs.slice(0, Math.max(1, maxCount));
}

async function analyzePair(pair) {
  const webState = JSON.parse(await fs.readFile(pair.webStatePath, "utf8"));
  const nativeState = pair.nativeStatePath ? JSON.parse(await fs.readFile(pair.nativeStatePath, "utf8")) : null;
  const originalMeta = await sharp(pair.originalPath).metadata();
  const webMeta = await sharp(pair.webPath).metadata();
  const dimensions = {
    width: Math.min(originalMeta.width ?? TARGET_STAGE.width, webMeta.width ?? TARGET_STAGE.width),
    height: Math.min(originalMeta.height ?? TARGET_STAGE.height, webMeta.height ?? TARGET_STAGE.height),
  };
  const regions = buildRegions(webState, dimensions);
  const regionMetrics = {};

  for (const [name, rect] of Object.entries(regions)) {
    if (!rect) continue;
    regionMetrics[name] = await compareRegion(pair.originalPath, pair.webPath, dimensions, rect);
  }
  const hudUiMetric = aggregateRegionMetrics([
    regionMetrics.hudLeft,
    regionMetrics.hudBelt,
    regionMetrics.hudRightControls,
    regionMetrics.hudRightStatus,
    regionMetrics.hudBottomCenter,
  ]);
  if (hudUiMetric) {
    regionMetrics.hudUi = hudUiMetric;
  }

  const runtime = scoreRuntime(webState);
  const layout = scoreLayout(webState, dimensions);
  const entities = scoreEntities(webState);
  const pixels = scorePixels(regionMetrics);
  const stateDiagnostics = buildStateDiagnostics(webState, nativeState);
  const overallScore = weightedAverage([
    [runtime.score, 0.22],
    [layout.score, 0.24],
    [entities.score, 0.16],
    [pixels.score, 0.38],
  ]);
  const gapHints = buildGapHints({ runtime, layout, entities, pixels, regionMetrics, webState, stateDiagnostics });

  return {
    prefix: pair.prefix,
    paths: {
      original: pair.originalPath,
      web: pair.webPath,
      webState: pair.webStatePath,
      nativeState: pair.nativeStatePath,
    },
    dimensions: {
      original: { width: originalMeta.width ?? null, height: originalMeta.height ?? null },
      web: { width: webMeta.width ?? null, height: webMeta.height ?? null },
      compared: dimensions,
    },
    scores: {
      overall: roundScore(overallScore),
      runtime: runtime.score,
      layout: layout.score,
      entities: entities.score,
      pixels: pixels.score,
    },
    diagnostics: {
      runtime: runtime.checks,
      layout: layout.checks,
      entities: entities.checks,
      pixels: pixels.checks,
      state: stateDiagnostics,
      gapHints,
    },
    regionMetrics,
    webStateSummary: {
      screen: webState.screen ?? null,
      mapFileName: webState.mapFileName ?? null,
      mapTitle: webState.mapTitle ?? null,
      player: webState.player ?? null,
      playerHp: webState.playerHp ?? null,
      playerMaxHp: webState.playerMaxHp ?? null,
      playerMp: webState.playerMp ?? null,
      playerMaxMp: webState.playerMaxMp ?? null,
      playerExperience: webState.playerExperience ?? null,
      playerMaxExperience: webState.playerMaxExperience ?? null,
      gold: webState.gold ?? null,
      credit: webState.credit ?? null,
      currentWeight: webState.currentWeight ?? null,
      maxWeight: webState.maxWeight ?? null,
      freeBagSlots: webState.freeBagSlots ?? null,
      maxBagSlots: webState.maxBagSlots ?? null,
      inventoryItemCount: webState.inventoryItemCount ?? null,
      beltItemCount: webState.beltItemCount ?? null,
      equipmentItemCount: webState.equipmentItemCount ?? null,
      hudTexts: summarizeHudTexts(webState.hudTexts),
      beltItems: summarizeItems(webState.beltItems),
      equipmentItems: summarizeItems(webState.equipmentItems),
      entityCount: webState.entityCount ?? null,
      npcCount: webState.npcCount ?? null,
      monsterCount: webState.monsterCount ?? null,
      questMarkerCount: webState.questMarkerCount ?? null,
      transitionOverlayVisible: Boolean(webState.transitionOverlayVisible),
      network404Count: webState.network404Count ?? 0,
      consoleErrorCount: webState.consoleErrorCount ?? 0,
      criticalConsoleErrorCount: webState.criticalConsoleErrorCount ?? null,
      tutorialOverlayVisible: Boolean(webState.tutorialOverlayVisible),
      objectiveTrackerVisible: Boolean(webState.objectiveTrackerVisible),
    },
  };
}

function buildRegions(webState, dimensions) {
  const hudTop = clampRectNumber(webState.hud?.top, 0, dimensions.height) ?? DEFAULT_HUD_TOP;
  const hudHeight = Math.max(1, dimensions.height - hudTop);
  return {
    full: makeRect(0, 0, dimensions.width, dimensions.height, dimensions),
    world: makeRect(0, 0, dimensions.width, hudTop, dimensions),
    hud: rectFromState(webState.hud, dimensions),
    hudLeft: makeRect(0, hudTop, 230, hudHeight, dimensions),
    hudBelt: makeRect(230, hudTop, 240, 40, dimensions),
    hudRightControls: makeRect(900, hudTop + 34, 124, 68, dimensions),
    hudRightStatus: makeRect(900, hudTop + 96, 124, 56, dimensions),
    hudBottomCenter: makeRect(230, hudTop + 108, 670, 44, dimensions),
    minimap: rectFromState(webState.miniMap, dimensions),
    chat: rectFromState(webState.chat, dimensions),
  };
}

function rectFromState(rect, dimensions) {
  if (!rect) return null;
  return makeRect(rect.left, rect.top, rect.width, rect.height, dimensions);
}

function makeRect(left, top, width, height, dimensions) {
  const x = Math.max(0, Math.floor(Number(left) || 0));
  const y = Math.max(0, Math.floor(Number(top) || 0));
  const w = Math.min(dimensions.width - x, Math.max(1, Math.floor(Number(width) || 0)));
  const h = Math.min(dimensions.height - y, Math.max(1, Math.floor(Number(height) || 0)));
  if (w <= 0 || h <= 0) return null;
  return { left: x, top: y, width: w, height: h };
}

async function compareRegion(originalPath, webPath, dimensions, rect) {
  const [original, web] = await Promise.all([
    loadRegion(originalPath, dimensions, rect),
    loadRegion(webPath, dimensions, rect),
  ]);
  const length = Math.min(original.data.length, web.data.length);
  let sumSq = 0;
  let sumAbs = 0;
  let sumLumAbs = 0;
  let channelCount = 0;
  let pixelCount = 0;

  for (let index = 0; index < length; index += 4) {
    const dr = original.data[index] - web.data[index];
    const dg = original.data[index + 1] - web.data[index + 1];
    const db = original.data[index + 2] - web.data[index + 2];
    const originalLum = 0.2126 * original.data[index] + 0.7152 * original.data[index + 1] + 0.0722 * original.data[index + 2];
    const webLum = 0.2126 * web.data[index] + 0.7152 * web.data[index + 1] + 0.0722 * web.data[index + 2];
    sumSq += dr * dr + dg * dg + db * db;
    sumAbs += Math.abs(dr) + Math.abs(dg) + Math.abs(db);
    sumLumAbs += Math.abs(originalLum - webLum);
    channelCount += 3;
    pixelCount += 1;
  }

  const mse = channelCount > 0 ? sumSq / channelCount : 0;
  const rmseNormalized = Math.sqrt(mse) / 255;
  const meanAbsDelta = channelCount > 0 ? sumAbs / channelCount : 0;
  const meanLumDelta = pixelCount > 0 ? sumLumAbs / pixelCount : 0;
  const similarity = clamp01(1 - rmseNormalized);

  return {
    rect,
    pixelCount,
    similarity: roundMetric(similarity),
    rmseNormalized: roundMetric(rmseNormalized),
    meanAbsDelta: roundMetric(meanAbsDelta),
    meanLumDelta: roundMetric(meanLumDelta),
  };
}

function aggregateRegionMetrics(metrics) {
  const valid = metrics.filter((metric) => metric && Number.isFinite(metric.pixelCount) && metric.pixelCount > 0);
  if (valid.length === 0) return null;
  const pixelCount = valid.reduce((sum, metric) => sum + metric.pixelCount, 0);
  const weighted = (key) =>
    valid.reduce((sum, metric) => sum + (Number(metric[key]) || 0) * metric.pixelCount, 0) / pixelCount;
  return {
    rect: null,
    pixelCount,
    similarity: roundMetric(weighted("similarity")),
    rmseNormalized: roundMetric(weighted("rmseNormalized")),
    meanAbsDelta: roundMetric(weighted("meanAbsDelta")),
    meanLumDelta: roundMetric(weighted("meanLumDelta")),
    subregions: valid.length,
  };
}

async function loadRegion(filePath, dimensions, rect) {
  const { data, info } = await sharp(filePath)
    .resize(dimensions.width, dimensions.height, { fit: "fill", kernel: "nearest" })
    .extract(rect)
    .ensureAlpha()
    .raw()
    .toBuffer({ resolveWithObject: true });
  return { data, info };
}

function scoreRuntime(state) {
  const criticalConsoleErrorCount = state.criticalConsoleErrorCount ?? state.consoleErrorCount ?? 0;
  const checks = [
    check("screen.game", state.screen === "game", `screen=${state.screen ?? "unknown"}`),
    check("map.bichon", state.mapTitle === "BichonProvince" || state.mapFileName === "0", `${state.mapTitle ?? "?"}/${state.mapFileName ?? "?"}`),
    check("transition.clear", !state.transitionOverlayVisible, `transitionOverlayVisible=${Boolean(state.transitionOverlayVisible)}`),
    check("console.clean", criticalConsoleErrorCount === 0, `criticalConsoleErrorCount=${criticalConsoleErrorCount}; raw=${state.consoleErrorCount ?? 0}`),
    check("network.clean", (state.network404Count ?? 0) === 0, `network404Count=${state.network404Count ?? 0}`),
    check("next.dev.hidden", !state.nextDevIndicatorVisible, `nextDevIndicatorVisible=${Boolean(state.nextDevIndicatorVisible)}`),
  ];
  return { score: roundScore(scoreChecks(checks)), checks };
}

function scoreLayout(state, dimensions) {
  const checks = [
    check("stage.1024x768", near(state.stage?.width, TARGET_STAGE.width, 2) && near(state.stage?.height, TARGET_STAGE.height, 2), rectLabel(state.stage)),
    check("capture.1024x768", dimensions.width === TARGET_STAGE.width && dimensions.height === TARGET_STAGE.height, `${dimensions.width}x${dimensions.height}`),
    check("hud.anchor", near(state.hud?.left, 0, 2) && near(state.hud?.top, DEFAULT_HUD_TOP, 6) && near(state.hud?.width, TARGET_STAGE.width, 4), rectLabel(state.hud)),
    check("minimap.anchor", near(state.miniMap?.right, TARGET_STAGE.width, 3) && (state.miniMap?.width ?? 0) >= 120, rectLabel(state.miniMap)),
    check("chat.bounds", (state.chat?.width ?? 0) >= 560 && (state.chat?.bottom ?? 0) <= TARGET_STAGE.height, rectLabel(state.chat)),
  ];
  return { score: roundScore(scoreChecks(checks)), checks };
}

function scoreEntities(state) {
  const nameplates = Array.isArray(state.visibleNameplates) ? state.visibleNameplates : [];
  const checks = [
    check("player.present", Boolean(state.player), JSON.stringify(state.player ?? null)),
    check("entities.present", (state.entityCount ?? 0) >= 8, `entityCount=${state.entityCount ?? 0}`),
    check("npc.present", (state.npcCount ?? 0) >= 4, `npcCount=${state.npcCount ?? 0}`),
    check("nameplates.present", nameplates.length >= 8, `nameplates=${nameplates.length}`),
    check("quest.markers.scoped", (state.questMarkerCount ?? 0) <= Math.max(2, state.npcCount ?? 0), `questMarkerCount=${state.questMarkerCount ?? 0}`),
  ];
  return { score: roundScore(scoreChecks(checks)), checks };
}

function scorePixels(metrics) {
  const full = metrics.full?.similarity ?? 0;
  const world = metrics.world?.similarity ?? 0;
  const hud = metrics.hud?.similarity ?? 0;
  const hudUi = metrics.hudUi?.similarity ?? hud;
  const minimap = metrics.minimap?.similarity ?? 0;
  const chat = metrics.chat?.similarity ?? 0;
  const score = weightedAverage([
    [normalizePixelSimilarity(world), 0.42],
    [normalizePixelSimilarity(hudUi), 0.28],
    [normalizePixelSimilarity(minimap), 0.14],
    [normalizePixelSimilarity(chat), 0.10],
    [normalizePixelSimilarity(full), 0.06],
  ]);
  const checks = [
    check("world.pixel", world >= 0.52, `similarity=${formatPercent(world)}`),
    check("hud.pixel", hudUi >= 0.60 || hud >= 0.60, `full=${formatPercent(hud)}; ui=${formatPercent(hudUi)}`),
    check("minimap.pixel", minimap >= 0.58, `similarity=${formatPercent(minimap)}`),
    check("chat.pixel", chat >= 0.58, `similarity=${formatPercent(chat)}`),
  ];
  return { score: roundScore(score), checks };
}

function normalizePixelSimilarity(similarity) {
  // Same-scene captures are not frame-perfect, so treat 0.45 as weak and 0.82 as excellent.
  return clamp01((similarity - 0.45) / 0.37);
}

function buildStateDiagnostics(webState, nativeState = null) {
  const levelText = textValue(webState.hudTexts?.level?.text);
  const nameText = textValue(webState.hudTexts?.name?.text);
  const healthText = textValue(webState.hudTexts?.healthOnly?.text ?? webState.hudHealthOnlyLabel);
  const expText = textValue(webState.hudTexts?.exp?.text);
  const goldText = textValue(webState.hudTexts?.gold?.text);
  const weightText = textValue(webState.hudTexts?.weight?.text);
  const spaceText = textValue(webState.hudTexts?.space?.text);
  const beltItems = summarizeItems(webState.beltItems);
  const inventoryItems = summarizeItems(webState.inventoryItems);
  const equipmentItems = summarizeItems(webState.equipmentItems);
  const equipmentNames = equipmentItems.map((item) => item.name).filter(Boolean);
  const starterEquipmentNames = ["Wooden Sword", "Cloth Armour", "Copper Necklace", "Wood Bracelet", "Straw Sandals", "Rope Belt"];
  const starterEquipmentMatches = starterEquipmentNames.filter((name) => equipmentNames.includes(name));
  const currentWeight = numberOrNull(webState.currentWeight);
  const maxWeight = numberOrNull(webState.maxWeight);
  const expectedHudWeight =
    currentWeight == null || maxWeight == null ? null : String(Math.max(0, Math.floor(maxWeight - currentWeight)));
  const expectedHudSpace = String(Math.max(0, 46 - beltItems.length - inventoryItems.length));
  const signals = [];

  if (levelText === "1") signals.push("level=1");
  if ((webState.playerMaxHp ?? null) === 18 || healthText === "HP 18/18") signals.push(`hp=${healthText || `${webState.playerHp ?? "?"}/${webState.playerMaxHp ?? "?"}`}`);
  if ((webState.gold ?? null) === 0 || goldText === "0") signals.push("gold=0");
  if ((webState.beltItemCount ?? beltItems.length) === 0) signals.push("belt=empty");
  if ((webState.inventoryItemCount ?? inventoryItems.length) === 0) signals.push("inventory=empty");
  if (starterEquipmentMatches.length >= 4) signals.push(`starterGear=${starterEquipmentMatches.join("/")}`);
  if (webState.playerMaxMp == null) signals.push("maxMp=missing");
  if (expectedHudWeight != null && weightText !== expectedHudWeight) {
    signals.push(`hudWeight=${weightText ?? "?"}/expected:${expectedHudWeight}`);
  }
  if (spaceText !== expectedHudSpace) signals.push(`hudSpace=${spaceText ?? "?"}/expected:${expectedHudSpace}`);

  const nativeCharacter = extractNativeCharacter(nativeState);
  const comparisons = buildNativeStateComparisons({
    web: {
      name: nameText,
      level: Number.parseInt(levelText ?? "", 10),
      hp: webState.playerHp,
      mp: webState.playerMp,
      gold: webState.gold,
      beltItems,
      equipmentItems,
    },
    nativeCharacter,
    nativeGold: nativeState?.account?.gold ?? null,
  });
  const mismatches = comparisons.filter((comparison) => comparison.status === "mismatch");
  const statePollutionRisk = signals.length >= 3 || mismatches.length >= 2;
  const summaryParts = [];
  if (nameText) summaryParts.push(`name=${nameText}`);
  if (levelText) summaryParts.push(`level=${levelText}`);
  if (healthText) summaryParts.push(`health=${healthText}`);
  if (webState.playerMp != null || webState.playerMaxMp != null) {
    summaryParts.push(`mp=${webState.playerMp ?? "?"}/${webState.playerMaxMp ?? "?"}`);
  }
  if (webState.gold != null) summaryParts.push(`gold=${webState.gold}`);
  if (currentWeight != null || maxWeight != null) {
    summaryParts.push(`weight=${currentWeight ?? "?"}/${maxWeight ?? "?"}`);
  }
  if (weightText || spaceText) summaryParts.push(`hudWeightSpace=${weightText ?? "?"}/${spaceText ?? "?"}`);
  summaryParts.push(`beltItems=${webState.beltItemCount ?? beltItems.length}`);
  summaryParts.push(`inventoryItems=${webState.inventoryItemCount ?? inventoryItems.length}`);
  summaryParts.push(`equipment=${equipmentNames.length > 0 ? equipmentNames.join(", ") : "none"}`);

  return {
    statePollutionRisk,
    summary: summaryParts.join("; "),
    signals,
    nativeSummary: nativeCharacter
      ? [
          `nativeName=${nativeCharacter.name}`,
          `nativeLevel=${nativeCharacter.level}`,
          `nativeHp=${nativeCharacter.hp}`,
          `nativeMp=${nativeCharacter.mp}`,
          `nativeGold=${nativeState?.account?.gold ?? "?"}`,
          `nativeBelt=${nativeCharacter.beltItems?.map((item) => item.name).filter(Boolean).join(", ") || "empty"}`,
          `nativeEquipment=${nativeCharacter.equipmentItems?.map((item) => item.name).filter(Boolean).join(", ") || "none"}`,
        ].join("; ")
      : null,
    comparisons,
    mismatches,
    hud: {
      nameText,
      levelText,
      healthText,
      expText,
      goldText,
      weightText,
      spaceText,
      expectedHudWeight,
      expectedHudSpace,
    },
    items: {
      beltItems,
      inventoryItems,
      equipmentItems,
    },
  };
}

function extractNativeCharacter(nativeState) {
  const characters = nativeState?.account?.characters;
  if (!Array.isArray(characters) || characters.length === 0) return null;
  return characters[0];
}

function buildNativeStateComparisons({ web, nativeCharacter, nativeGold }) {
  if (!nativeCharacter) return [];
  const comparisons = [];
  comparisons.push(compareScalar("level", nativeCharacter.level, web.level));
  comparisons.push(compareScalar("hp", nativeCharacter.hp, web.hp));
  comparisons.push(compareScalar("mp", nativeCharacter.mp, web.mp));
  comparisons.push(compareScalar("gold", nativeGold, web.gold));
  comparisons.push(compareItemNames("belt", nativeCharacter.beltItems, web.beltItems));
  comparisons.push(compareItemNames("equipment", nativeCharacter.equipmentItems, web.equipmentItems));
  return comparisons;
}

function compareScalar(name, nativeValue, webValue) {
  const nativeMissing = nativeValue == null || Number.isNaN(nativeValue);
  const webMissing = webValue == null || Number.isNaN(webValue);
  const status = nativeMissing || webMissing ? "unknown" : Number(nativeValue) === Number(webValue) ? "match" : "mismatch";
  return { name, native: nativeMissing ? null : nativeValue, web: webMissing ? null : webValue, status };
}

function compareItemNames(name, nativeItems, webItems) {
  const nativeNames = itemNames(nativeItems);
  const webNames = itemNames(webItems);
  const status = arraysEqual(nativeNames, webNames) ? "match" : "mismatch";
  return { name, native: nativeNames, web: webNames, status };
}

function itemNames(items) {
  if (!Array.isArray(items)) return [];
  return items.map((item) => item?.name).filter(Boolean);
}

function arraysEqual(left, right) {
  if (left.length !== right.length) return false;
  return left.every((value, index) => value === right[index]);
}

function summarizeHudTexts(hudTexts) {
  if (!hudTexts || typeof hudTexts !== "object") return null;
  return Object.fromEntries(
    Object.entries(hudTexts).map(([key, value]) => [key, value?.text ?? null]),
  );
}

function summarizeItems(items) {
  if (!Array.isArray(items)) return [];
  return items.map((item) => ({
    name: item?.name ?? null,
    slot: item?.slot ?? item?.equipmentSlot ?? null,
    quantity: item?.quantity ?? null,
    icon: item?.icon ?? null,
    durabilityCurrent: item?.durabilityCurrent ?? null,
    durabilityMax: item?.durabilityMax ?? null,
  }));
}

function textValue(value) {
  if (value == null) return null;
  const text = String(value).trim();
  return text.length > 0 ? text : null;
}

function numberOrNull(value) {
  const number = Number(value);
  return Number.isFinite(number) ? number : null;
}

function buildGapHints({ runtime, layout, entities, pixels, regionMetrics, webState, stateDiagnostics }) {
  const hints = [];
  const worldSimilarity = regionMetrics.world?.similarity ?? 0;
  const hudSimilarity = regionMetrics.hud?.similarity ?? 0;
  const hudUiSimilarity = regionMetrics.hudUi?.similarity ?? hudSimilarity;
  const minimapSimilarity = regionMetrics.minimap?.similarity ?? 0;
  const chatSimilarity = regionMetrics.chat?.similarity ?? 0;
  if (runtime.score < 1) {
    hints.push(gap("P0", "Capture/runtime health is not clean", failed(runtime.checks).join("; ")));
  }
  if ((webState.transitionOverlayVisible ?? false) || webState.screen !== "game") {
    hints.push(gap("P0", "Comparison is not on a stable in-game frame", `screen=${webState.screen ?? "unknown"}`));
  }
  if (webState.tutorialOverlayVisible) {
    hints.push(gap("P1", "Web-only beginner tutorial overlay is visible", "Crystal has no matching in-world tutorial panel; parity captures suppress it, and default UX should be gated behind an explicit non-Crystal mode."));
  }
  if (webState.objectiveTrackerVisible) {
    hints.push(gap("P1", "Web-only objective tracker is visible", "The top-center quest tracker is useful onboarding, but it is not present in Crystal and materially changes the first-screen silhouette."));
  }
  if (layout.score < 0.95) {
    hints.push(gap("P1", "Stage/HUD geometry still has measurable drift", failed(layout.checks).join("; ")));
  }
  if (worldSimilarity < 0.52) {
    hints.push(gap("P1", "World scene pixels diverge strongly", "Prioritize camera anchor, map raster, entity z-order, blend/glow and lighting before small UI tweaks."));
  } else if (worldSimilarity < 0.90) {
    hints.push(gap("P2", "World scene still needs human visual review", regionMetricDetail("world", regionMetrics.world)));
  }
  if (stateDiagnostics?.statePollutionRisk && hudUiSimilarity < 0.90) {
    hints.push(
      gap(
        "P1",
        "HUD score is polluted by dynamic character state",
        stateDiagnostics.nativeSummary
          ? `${stateDiagnostics.summary}; ${stateDiagnostics.nativeSummary}; mismatches=${stateDiagnostics.mismatches.map((entry) => entry.name).join(", ")}`
          : `${stateDiagnostics.summary}; signals=${stateDiagnostics.signals.join(", ")}`,
      ),
    );
  }
  if (hudUiSimilarity < 0.60) {
    hints.push(gap("P1", "HUD pixels diverge from Crystal", "Audit HUD image assets, HP/MP orb clipping, chat frame placement, belt alignment and bottom border offsets."));
  } else if (hudUiSimilarity < 0.85) {
    hints.push(gap("P1", "HUD state/assets are visibly off Crystal", hudMetricDetail(regionMetrics)));
  } else if (hudSimilarity < 0.85) {
    hints.push(gap("P2", "HUD full crop includes dynamic edge/background noise", hudMetricDetail(regionMetrics)));
  }
  if (minimapSimilarity < 0.58) {
    hints.push(gap("P2", "MiniMap crop/colors differ", "Re-check minimap crop transform, radar dot colors, frame offset and safe-zone/title rendering."));
  } else if (minimapSimilarity < 0.85) {
    hints.push(gap("P2", "MiniMap still needs crop/color review", regionMetricDetail("minimap", regionMetrics.minimap)));
  }
  if (chatSimilarity > 0 && chatSimilarity < 0.85) {
    hints.push(gap("P2", "Chat panel content/state differs", regionMetricDetail("chat", regionMetrics.chat)));
  }
  if (entities.score < 0.9) {
    hints.push(gap("P2", "Entity/nameplate evidence is incomplete", failed(entities.checks).join("; ")));
  }
  if (pixels.score < 0.45) {
    hints.push(gap("P1", "Pixel score says the scene is still visually low-parity", "This is expected until captures are same-coordinate and render/HUD layers are tuned; keep using this as trend evidence."));
  }
  return hints;
}

function aggregateSamples(samples) {
  const latest = samples[0];
  const averages = {
    overall: average(samples.map((sample) => sample.scores.overall)),
    runtime: average(samples.map((sample) => sample.scores.runtime)),
    layout: average(samples.map((sample) => sample.scores.layout)),
    entities: average(samples.map((sample) => sample.scores.entities)),
    pixels: average(samples.map((sample) => sample.scores.pixels)),
  };
  const topGaps = rankGaps(samples);
  return {
    latestPrefix: latest.prefix,
    latestScore: latest.scores.overall,
    averageScores: Object.fromEntries(Object.entries(averages).map(([key, value]) => [key, roundScore(value)])),
    estimatedHumanParityBand: estimateHumanParityBand(averages.overall),
    topGaps,
  };
}

function rankGaps(samples) {
  const counts = new Map();
  for (const sample of samples) {
    for (const hint of sample.diagnostics.gapHints) {
      const key = `${hint.priority}|${hint.title}`;
      const current = counts.get(key) ?? { ...hint, count: 0 };
      current.count += 1;
      counts.set(key, current);
    }
  }
  const priorityRank = { P0: 0, P1: 1, P2: 2, P3: 3 };
  return [...counts.values()]
    .sort((a, b) => (priorityRank[a.priority] ?? 9) - (priorityRank[b.priority] ?? 9) || b.count - a.count)
    .slice(0, 8);
}

function estimateHumanParityBand(score) {
  const center = Math.round(clamp01(score) * 100);
  const low = Math.max(0, center - 7);
  const high = Math.min(100, center + 7);
  return `${low}-${high}%`;
}

function renderMarkdown(report) {
  const lines = [];
  lines.push("# Crystal/Web Visual Parity Report");
  lines.push("");
  lines.push(`Generated: ${report.generatedAt}`);
  lines.push(`Input: \`${report.inputDir}\``);
  lines.push(`Samples: ${report.sampleCount}`);
  lines.push("");
  lines.push("## Summary");
  lines.push("");
  lines.push(`- Latest sample: \`${report.aggregate.latestPrefix}\``);
  lines.push(`- Latest weighted score: ${formatPercent(report.aggregate.latestScore)}`);
  lines.push(`- Estimated human visual/feel parity band: **${report.aggregate.estimatedHumanParityBand}**`);
  lines.push(`- Runtime health average: ${formatPercent(report.aggregate.averageScores.runtime)}`);
  lines.push(`- Layout average: ${formatPercent(report.aggregate.averageScores.layout)}`);
  lines.push(`- Entity/nameplate average: ${formatPercent(report.aggregate.averageScores.entities)}`);
  lines.push(`- Pixel trend average: ${formatPercent(report.aggregate.averageScores.pixels)}`);
  lines.push("");
  lines.push("> Pixel trend is not a final acceptance score: Crystal/Web captures can be animation-frame and coordinate-offset mismatched. Use the score for trend/regression detection, then resolve the listed gaps with human visual review.");
  lines.push("");
  lines.push("## Top Gaps");
  lines.push("");
  if (report.aggregate.topGaps.length === 0) {
    lines.push("- No recurring automated gaps found in this sample set.");
  } else {
    for (const gapHint of report.aggregate.topGaps) {
      lines.push(`- \`${gapHint.priority}\` ${gapHint.title} (${gapHint.count}/${report.sampleCount} samples): ${gapHint.detail}`);
    }
  }
  lines.push("");
  const stateDiagnostics = report.samples.filter((sample) => sample.diagnostics.state?.statePollutionRisk);
  if (stateDiagnostics.length > 0) {
    lines.push("## State Diagnostics");
    lines.push("");
    lines.push("> These checks do not replace native-state extraction. They mark Web captures that look like a fresh/starter character, so HUD/chat pixel deltas are not mistaken for pure asset/layout defects.");
    lines.push("");
    for (const sample of stateDiagnostics) {
      const state = sample.diagnostics.state;
      const details = state.nativeSummary
        ? `${state.summary}; ${state.nativeSummary}; mismatches=${state.mismatches.map((entry) => entry.name).join(", ")}`
        : `${state.summary}; signals=${state.signals.join(", ")}`;
      lines.push(`- \`${sample.prefix}\`: ${details}`);
    }
    lines.push("");
  }
  lines.push("## Samples");
  lines.push("");
  lines.push("| Sample | Overall | Runtime | Layout | Entities | Pixels | World | HUD Full | HUD UI | Chat | MiniMap |");
  lines.push("| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |");
  for (const sample of report.samples) {
    lines.push(
      `| \`${sample.prefix}\` | ${formatPercent(sample.scores.overall)} | ${formatPercent(sample.scores.runtime)} | ${formatPercent(sample.scores.layout)} | ${formatPercent(sample.scores.entities)} | ${formatPercent(sample.scores.pixels)} | ${formatPercent(sample.regionMetrics.world?.similarity ?? 0)} | ${formatPercent(sample.regionMetrics.hud?.similarity ?? 0)} | ${formatPercent(sample.regionMetrics.hudUi?.similarity ?? sample.regionMetrics.hud?.similarity ?? 0)} | ${formatPercent(sample.regionMetrics.chat?.similarity ?? 0)} | ${formatPercent(sample.regionMetrics.minimap?.similarity ?? 0)} |`,
    );
  }
  lines.push("");
  lines.push("## Next Pass");
  lines.push("");
  lines.push("- Capture a fresh Crystal/Web pair at the same map/coordinate after each fix.");
  lines.push("- Treat P0/P1 gaps as implementation candidates before expanding feature coverage.");
  lines.push("- Once scores stabilize, run a movement recording pass and attach it to this report family.");
  lines.push("");
  return `${lines.join("\n")}\n`;
}

function check(name, ok, detail) {
  return { name, ok: Boolean(ok), detail };
}

function gap(priority, title, detail) {
  return { priority, title, detail };
}

function regionMetricDetail(name, metric) {
  if (!metric) return `${name}=missing`;
  return [
    `${name} similarity=${formatPercent(metric.similarity ?? 0)}`,
    `meanAbsDelta=${roundMetric(metric.meanAbsDelta ?? 0)}`,
    `meanLumDelta=${roundMetric(metric.meanLumDelta ?? 0)}`,
  ].join("; ");
}

function hudMetricDetail(metrics) {
  const subregionDetails = [
    ["left", metrics.hudLeft],
    ["belt", metrics.hudBelt],
    ["rightControls", metrics.hudRightControls],
    ["rightStatus", metrics.hudRightStatus],
    ["bottomCenter", metrics.hudBottomCenter],
  ]
    .filter(([, metric]) => metric)
    .map(([name, metric]) => `${name}=${formatPercent(metric.similarity ?? 0)}`)
    .join(", ");
  return [
    regionMetricDetail("hudFull", metrics.hud),
    regionMetricDetail("hudUi", metrics.hudUi),
    subregionDetails ? `subregions: ${subregionDetails}` : null,
  ].filter(Boolean).join("; ");
}

function failed(checks) {
  return checks.filter((item) => !item.ok).map((item) => `${item.name}: ${item.detail}`);
}

function scoreChecks(checks) {
  if (checks.length === 0) return 1;
  return checks.filter((item) => item.ok).length / checks.length;
}

function weightedAverage(entries) {
  let weighted = 0;
  let weightSum = 0;
  for (const [value, weight] of entries) {
    weighted += clamp01(value) * weight;
    weightSum += weight;
  }
  return weightSum > 0 ? weighted / weightSum : 0;
}

function average(values) {
  const finite = values.filter((value) => Number.isFinite(value));
  if (finite.length === 0) return 0;
  return finite.reduce((sum, value) => sum + value, 0) / finite.length;
}

function near(value, expected, tolerance) {
  return Number.isFinite(Number(value)) && Math.abs(Number(value) - expected) <= tolerance;
}

function rectLabel(rect) {
  if (!rect) return "missing";
  return `${roundMetric(rect.left ?? 0)},${roundMetric(rect.top ?? 0)} ${roundMetric(rect.width ?? 0)}x${roundMetric(rect.height ?? 0)}`;
}

function clampRectNumber(value, min, max) {
  const number = Number(value);
  if (!Number.isFinite(number)) return null;
  return Math.max(min, Math.min(max, number));
}

function clamp01(value) {
  return Math.max(0, Math.min(1, Number.isFinite(value) ? value : 0));
}

function roundScore(value) {
  return Math.round(clamp01(value) * 1000) / 1000;
}

function roundMetric(value) {
  return Math.round((Number(value) || 0) * 1000) / 1000;
}

function formatPercent(value) {
  return `${Math.round(clamp01(value) * 100)}%`;
}

function parseArgs(argv) {
  const parsed = {};
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (!arg.startsWith("--")) continue;
    const key = arg.slice(2);
    const value = argv[index + 1]?.startsWith("--") || argv[index + 1] === undefined ? "true" : argv[++index];
    parsed[key] = value;
  }
  return parsed;
}

function numberArg(value, fallback) {
  const number = Number(value);
  return Number.isFinite(number) ? number : fallback;
}

function timestamp() {
  return new Date().toISOString().replace(/[-:]/g, "").replace(/\..+$/, "").replace("T", "-");
}
