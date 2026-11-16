/**
 * Real, isolated R16 Player Web acceptance.
 *
 * Start Web and a private ordinary-account Gateway separately. This runner never
 * starts a service, edits a save, grants an asset, spawns a monster, or teleports.
 * The first player acts exclusively through Playwright's real DOM/pointer/key
 * input. Passive WebSocket observation supplies authoritative evidence; no page
 * action/debug/runtime API is called and no network route is mocked.
 *
 * Example (supply the actual private realm and loopback ports):
 * node apps/web/scripts/run-r16-web-acceptance.mjs --web-url http://127.0.0.1:PORT/
 *   --gateway-url ws://127.0.0.1:PORT/ws --realm-id ai-... --output-dir ABSOLUTE
 *
 * Playwright is deliberately not installed by this runner. Set
 * MIR2_PLAYWRIGHT_MODULE to an existing Playwright package/entry if necessary;
 * MIR2_BROWSER_EXE may select an installed Chromium browser. Never pass a
 * password, model key, private account file, or token on the command line.
 */
import { createHash, randomBytes } from "node:crypto";
import { lstat, mkdir, readFile, realpath, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { dirname, isAbsolute, parse, relative, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { setTimeout as sleep } from "node:timers/promises";
import { GatewayClient, DIRECTIONS } from "../../ai-player/src/gateway-client.mjs";
import { ObservationReducer } from "../../ai-player/src/observation.mjs";
import { parseGatewayJson } from "../../ai-player/src/lossless-json.mjs";
import { loadClientCollision, nextWalkDirection } from "../../ai-player/src/skills/navigation.mjs";
import { fieldApproachStep } from "../../ai-player/scripts/field-encounter.mjs";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");
const require = createRequire(import.meta.url);
const digest = value => createHash("sha256").update(String(value)).digest("hex");
const serializable = value => JSON.stringify(value, (_, entry) => typeof entry === "bigint" ? entry.toString() : entry, 2);
const distance = (a, b) => a && b ? Math.max(Math.abs(a.x - b.x), Math.abs(a.y - b.y)) : Infinity;
const point = value => ({ x: value.x, y: value.y });
const samePoint = (a, b) => a && b && a.x === b.x && a.y === b.y;
const inBounds = (p, b) => p && Number.isInteger(p.x) && Number.isInteger(p.y)
  && p.x >= b.minX && p.x <= b.maxX && p.y >= b.minY && p.y <= b.maxY;
const live = value => value?.dead !== true && value?.hp !== 0;
const uid = value => String(value);
const quantity = item => Number(item?.quantity ?? item?.count ?? 0);
const allItems = state => [...state.inventoryItems, ...state.beltItems, ...state.equipmentItems];
const ownedQuantity = (state, id) => allItems(state).filter(item => uid(item.uniqueId) === uid(id)).reduce((sum, item) => sum + quantity(item), 0);
const templateItems = (state, reference) => allItems(state).filter(item => item.key === reference.key && item.name === reference.name);
const templateQuantity = (state, reference) => templateItems(state, reference).reduce((sum, item) => sum + quantity(item), 0);
const itemBrief = item => ({ uniqueId: item.uniqueId == null ? null : uid(item.uniqueId), key: item.key, name: item.name,
  slot: item.slot, container: item.container, quantity: quantity(item) });
const orderedItems = items => items.map(itemBrief).sort((a, b) => `${a.container}:${a.slot}:${a.uniqueId}:${a.key}`.localeCompare(`${b.container}:${b.slot}:${b.uniqueId}:${b.key}`));
const assetFingerprint = state => serializable({ gold: state.gold, inventory: orderedItems(state.inventoryItems),
  belt: orderedItems(state.beltItems), equipment: orderedItems(state.equipmentItems) });
const dropBrief = value => ({ objectId: value.objectId, ...point(value), name: value.name, icon: value.icon,
  quantity: value.quantity, sourceMonster: value.sourceMonster, ownerObjectId: value.ownerObjectId,
  ownershipRemainingTicks: value.ownershipRemainingTicks });

/** Legacy spawn packets have no monster provenance; bind their ordered context explicitly. */
export function naturalDropCandidates({ frames, state, encounter }) {
  if (!encounter || state.connectionEpoch !== encounter.connectionEpoch || state.characterEpoch !== encounter.characterEpoch
    || state.mapFileName !== encounter.mapFileName) return [];
  const sameContext = frame => frame.source === "browser" && frame.sequence > encounter.after
    && frame.connectionEpoch === encounter.connectionEpoch && frame.characterEpoch === encounter.characterEpoch
    && frame.mapFileName === encounter.mapFileName;
  const relevant = frames.filter(sameContext), targetId = uid(encounter.monster.objectId);
  const deaths = relevant.filter(frame => frame.packet === "ObjectDied" && uid(frame.payload.objectId) === targetId);
  const death = deaths.at(-1), location = death?.payload.location;
  if (!death || !Number.isInteger(location?.x) || !Number.isInteger(location?.y)) return [];
  const hits = relevant.filter(frame => frame.sequence < death.sequence && frame.packet === "ObjectStruck"
    && uid(frame.payload.objectId) === targetId);
  const ownHit = hits.at(-1), ownIds = new Set(encounter.ownObjectIds.map(uid));
  // The latest observed hit must be ours; another player's hit cannot inherit
  // our earlier engagement. A death alone does not prove our kill.
  if (!ownHit || !ownIds.has(uid(ownHit.payload.attackerId))) return [];
  const deathIndex = relevant.indexOf(death), candidates = [], seen = new Set();
  for (const [index, spawn] of relevant.slice(deathIndex + 1, deathIndex + 33).entries()) {
    if (["ObjectDied", "Death"].includes(spawn.packet)) break;
    if (spawn.at < death.at || spawn.at - death.at > 1500) break;
    if (!["ObjectGold", "ObjectItem"].includes(spawn.packet)) continue;
    const id = uid(spawn.payload.objectId), position = spawn.payload.location;
    if (encounter.dropIdsBefore.has(id) || seen.has(id) || !Number.isInteger(position?.x) || !Number.isInteger(position?.y)
      || distance(position, location) > 3) continue;
    const drop = state.groundDrops.find(value => uid(value.objectId) === id && samePoint(value, position));
    if (!drop || drop.ownerObjectId != null && uid(drop.ownerObjectId) !== "0" && !ownIds.has(uid(drop.ownerObjectId))) continue;
    seen.add(id);
    candidates.push({ drop, provenance: { association: "own-hit/death/ordered-nearby-legacy-spawn", inferredPacketAssociation: true,
      monster: { objectId: encounter.monster.objectId, name: encounter.monster.name, originallyObservedPosition: point(encounter.monster) },
      ownHit: { sequence: ownHit.sequence, receivedAtMs: ownHit.at, attackerId: ownHit.payload.attackerId },
      death: { sequence: death.sequence, receivedAtMs: death.at, location: point(location) },
      spawn: { sequence: spawn.sequence, receivedAtMs: spawn.at, packet: spawn.packet, objectId: spawn.payload.objectId,
        location: point(position), gold: spawn.payload.gold },
      packetsAfterDeath: index + 1, elapsedSinceDeathMs: spawn.at - death.at, previouslyObservedDropId: false } });
  }
  return candidates;
}

/** A real cardinal key avoids the canvas's corpse/entity click semantics. */
export function nextCardinalWalkDirection({ player, target, desiredDistance = 0, collision, occupied = [], movementBounds = null }) {
  if (![player, target].every(value => Number.isInteger(value?.x) && Number.isInteger(value?.y))
    || !(collision?.blocked instanceof Set)) return null;
  const bounds = movementBounds ?? { minX: 0, minY: 0, maxX: collision.width - 1, maxY: collision.height - 1 };
  if (!inBounds(player, bounds) || !inBounds(target, { minX: 0, minY: 0, maxX: collision.width - 1, maxY: collision.height - 1 })) return null;
  const key = value => `${value.x},${value.y}`, occupiedKeys = new Set(occupied.map(key));
  const queue = [{ ...point(player), first: null }], visited = new Set([key(player)]);
  for (let index = 0; index < queue.length && index < 1_500_000; index++) {
    const current = queue[index];
    if (distance(current, target) <= desiredDistance) return current.first;
    for (const [direction, dx, dy] of [["Up", 0, -1], ["Right", 1, 0], ["Down", 0, 1], ["Left", -1, 0]]) {
      const next = { x: current.x + dx, y: current.y + dy, first: current.first ?? direction }, nextKey = key(next);
      if (!inBounds(next, bounds) || visited.has(nextKey) || collision.blocked.has(nextKey) || occupiedKeys.has(nextKey)) continue;
      visited.add(nextKey); queue.push(next);
    }
  }
  return null;
}

/** A facing-only server ACK is an accepted input, never progress along a path. */
export function cardinalAcknowledgement({ before, state, ack, afterSequence, direction }) {
  if (!state.ready || !state.player || !before.player || state.revision <= before.revision || !ack || ack.packet !== "UserLocation"
    || ack.sequence <= afterSequence || state.connectionEpoch !== before.connectionEpoch || state.characterEpoch !== before.characterEpoch
    || state.mapFileName !== before.mapFileName || ack.connectionEpoch !== before.connectionEpoch || ack.characterEpoch !== before.characterEpoch
    || ack.mapFileName !== before.mapFileName || !samePoint(ack.position, state.player) || ack.direction !== state.player.direction) return null;
  if (distance(before.player, state.player) > 0) return "moved";
  return state.player.direction === direction && state.player.direction !== before.player.direction ? "turned-only" : null;
}

export function clearMouseStepCandidates({ state, collision, bounds }) {
  if (!state.player) return [];
  return [["Right", 1, 0], ["Down", 0, 1], ["Left", -1, 0], ["Up", 0, -1]].map(([direction, dx, dy]) => ({
    direction, dx, dy, tile: { x: state.player.x + dx, y: state.player.y + dy } })).filter(value =>
    inBounds(value.tile, bounds) && !collision.blocked.has(`${value.tile.x},${value.tile.y}`)
    && !state.groundDrops.some(drop => samePoint(drop, value.tile))
    && !state.entities.some(entity => entity.objectId !== state.playerObjectId
      && (samePoint(entity, value.tile) || live(entity) && ["monster", "player"].includes(entity.kind) && distance(entity, value.tile) <= 2)));
}

/** Only known public asset paths may appear in diagnostics; never queries or API paths. */
export function safeResourceLocation(rawUrl, publicOrigin) {
  if (!rawUrl) return null;
  try {
    const url = new URL(rawUrl), publicPath = /^\/(?:original-ui|original-map|original-effects|generated|bevy-entity-atlases|bevy-runtime(?:-releases)?|client-core|bootstrap|pwa|_next\/static)\//u.test(url.pathname)
      || ["/favicon.ico", "/icon.svg", "/manifest.webmanifest", "/original-ui-manifest.json"].includes(url.pathname);
    return url.origin === publicOrigin && publicPath ? { pathname: url.pathname }
      : { locationHash: digest(`${url.origin}${url.pathname}`) };
  } catch { return { locationHash: digest(rawUrl) }; }
}

/** Classify only fixed server messages; arbitrary error text is never retained. */
export function gatewayErrorEvidence(event) {
  const message = typeof event?.message === "string" ? event.message : "";
  const limited = /^too many authentication attempts; retry after ([0-9]{1,4}) seconds$/u.exec(message);
  if (limited && Number(limited[1]) >= 2 && Number(limited[1]) <= 3600) return {
    category: "authentication-rate-limited", retryAfterSeconds: Number(limited[1]), messageHash: digest(message) };
  return { category: message === "invalid authentication request" ? "invalid-authentication-request" : "unclassified",
    messageHash: digest(message) };
}

export function privateAccountStatePath(value, outputDir) {
  demand(typeof value === "string" && isAbsolute(value), "PRIVATE_QA_ACCOUNT_STATE_REQUIRES_ABSOLUTE_PATH");
  const path = resolve(value), parent = dirname(path);
  const outside = (root, target) => { const rel = relative(root, target); return rel !== "" && (rel.startsWith("..") || isAbsolute(rel)); };
  demand(/[/\\]private-qa[/\\][^/\\]+\.local\.json$/iu.test(path), "PRIVATE_QA_ACCOUNT_STATE_REQUIRES_PRIVATE_QA_LOCAL_JSON");
  demand(outside(repoRoot, path) && outside(resolve(outputDir), path) && outside(parent, resolve(outputDir)),
    "PRIVATE_QA_ACCOUNT_STATE_MUST_BE_OUTSIDE_REPOSITORY_AND_REPORTS");
  return path;
}

export function reusedPrimaryIdentity(state, { realmId, gatewayUrl }, characterName) {
  demand(state?.schema === "mir2.r16.private-ordinary-qa-identities.v1" && state.realmId === realmId && state.gatewayUrl === gatewayUrl,
    "PRIVATE_QA_ACCOUNT_STATE_REALM_OR_SCHEMA_MISMATCH");
  const primary = state.primary;
  demand(primary && typeof primary.accountId === "string" && typeof primary.password === "string"
    && /^wr16[a-f0-9]{10}$/u.test(primary.accountId) && /^[A-Za-z0-9]{10,15}$/u.test(primary.password)
    && /^Web[a-f0-9]{8}$/u.test(primary.characterName), "PRIVATE_QA_ACCOUNT_STATE_IS_NOT_OWNED_WR16_IDENTITY");
  demand(/^Web[a-f0-9]{8}$/u.test(characterName) && characterName !== primary.characterName, "REUSED_ACCOUNT_REQUIRES_FRESH_CHARACTER_NAME");
  return { accountId: primary.accountId, password: primary.password, characterName };
}

export function newCharacterSelectionEvidence(previous, created, name, reused) {
  demand(Array.isArray(previous) && previous.length < 4, "ORDINARY_TEST_CHARACTER_SLOTS_FULL");
  demand(previous.every(value => Number.isSafeInteger(value.index) && value.index >= 0 && typeof value.name === "string")
    && new Set(previous.map(value => value.index)).size === previous.length, "LOGIN_CHARACTER_INDICES_NOT_UNAMBIGUOUS");
  demand(created && created.name === name && Number.isSafeInteger(created.index) && created.index >= 0
    && !previous.some(value => value.index === created.index || value.name === name), "NEW_CHARACTER_DID_NOT_RECEIVE_NEW_SERVER_INDEX");
  demand(!reused || previous.length > 0 && created.index !== 0, "REUSED_TEST_ACCOUNT_DID_NOT_CREATE_NEW_NONZERO_SLOT");
  return { previousCharacterIndices: previous.map(value => value.index), createdCharacterIndex: created.index, reusedTestAccount: reused };
}

function failure(code) { const error = new Error(code); error.acceptanceCode = code; return error; }
function demand(condition, code) { if (!condition) throw failure(code); }
function loopback(value, protocols, code) {
  let url; try { url = new URL(value); } catch { throw failure(code); }
  demand(protocols.includes(url.protocol) && ["127.0.0.1", "localhost", "[::1]"].includes(url.hostname)
    && !url.username && !url.password && !url.hash, code);
  return url;
}

function validateOptions(options) {
  const web = loopback(options.webUrl, ["http:", "https:"], "USE_PRIVATE_LOOPBACK_WEB");
  const gateway = loopback(options.gatewayUrl, ["ws:", "wss:"], "USE_PRIVATE_LOOPBACK_GATEWAY");
  demand(["/ws", "/playtest/ws"].includes(gateway.pathname) && !gateway.search, "USE_ORDINARY_GATEWAY_ROUTE");
  demand(typeof options.realmId === "string" && /^ai-[A-Za-z0-9-]+$/.test(options.realmId), "USE_ISOLATED_AI_PREFIX_REALM");
  demand(typeof options.outputDir === "string" && isAbsolute(options.outputDir), "EXPLICIT_EXTERNAL_OUTPUT_REQUIRED");
  const outputDir = resolve(options.outputDir), withinRepo = relative(repoRoot, outputDir);
  demand(outputDir !== parse(outputDir).root && withinRepo !== "" && (withinRepo.startsWith("..") || isAbsolute(withinRepo)), "OUTPUT_MUST_BE_EXTERNAL_DEDICATED_DIRECTORY");
  for (const name of web.searchParams.keys()) demand(["gatewayWs"].includes(name), "CLEAN_WEB_URL_REQUIRED");
  const maxDropKills = Number(options.maxDropKills ?? 24);
  demand(Number.isSafeInteger(maxDropKills) && maxDropKills > 0 && maxDropKills <= 64, "DROP_HUNT_BUDGET_MUST_BE_1_TO_64");
  demand(!(options.stopAfterEquip && options.stopAfterLoot), "CHOOSE_ONE_FOCUSED_DIAGNOSTIC");
  web.searchParams.set("gatewayWs", gateway.href);
  const privateAccountState = options.privateAccountState ? privateAccountStatePath(options.privateAccountState, outputDir) : null;
  const reusePrivateAccountState = options.reusePrivateAccountState ? privateAccountStatePath(options.reusePrivateAccountState, outputDir) : null;
  demand(!privateAccountState || privateAccountState !== reusePrivateAccountState, "REUSED_PRIVATE_QA_FILE_MUST_NOT_BE_OVERWRITTEN");
  return { ...options, webUrl: web.href, gatewayUrl: gateway.href, outputDir, maxDropKills,
    ...(privateAccountState ? { privateAccountState } : {}), ...(reusePrivateAccountState ? { reusePrivateAccountState } : {}) };
}

async function configOptions(options) {
  if (!options.config) return options;
  demand(isAbsolute(options.config), "CONFIG_REQUIRES_ABSOLUTE_PRIVATE_PATH");
  const privateConfig = JSON.parse(await readFile(options.config, "utf8"));
  // The driver's owner/actor credentials and private archive paths must never
  // enter a report. This journey registers two separate ordinary characters.
  return { ...options, webUrl: options.webUrl ?? privateConfig.origin,
    gatewayUrl: options.gatewayUrl ?? privateConfig.gatewayUrl, realmId: options.realmId ?? privateConfig.realmId };
}

export async function loadPlaywright(modulePath = process.env.MIR2_PLAYWRIGHT_MODULE) {
  const candidates = modulePath ? [modulePath] : ["playwright", "playwright-core", "@playwright/test"];
  for (const candidate of candidates) {
    try {
      const imported = await import(pathToFileURL(require.resolve(candidate)).href);
      const module = imported.chromium ? imported : imported.default;
      if (module?.chromium?.launch) return module;
    } catch { /* Try only existing packages. No install/download fallback. */ }
  }
  throw failure("PLAYWRIGHT_NOT_INSTALLED_SET_MIR2_PLAYWRIGHT_MODULE");
}

/** Source geometry only; never reads a private runtime/frame or action export. */
async function sourceUiLayout() {
  const keyboard = JSON.parse(await readFile(resolve(repoRoot, "apps/game-client/client-bevy/src/crystal_ui/keyboard_defaults.json"), "utf8"));
  const bag = await readFile(resolve(repoRoot, "apps/game-client/client-bevy/src/portable_bag_ui.rs"), "utf8");
  const layouts = await readFile(resolve(repoRoot, "apps/game-client/client-bevy/src/crystal_ui/panel_layouts.rs"), "utf8");
  const shop = await readFile(resolve(repoRoot, "apps/game-client/client-bevy/src/portable_npc_shop_ui.rs"), "utf8");
  const paint = await readFile(resolve(repoRoot, "apps/game-client/client-bevy/src/crystal_ui/shop_paint.rs"), "utf8");
  const respawnBytes = await readFile(resolve(repoRoot, "apps/web/lib/generated/crystal_respawn_manifest.json"));
  const map = JSON.parse(respawnBytes).maps.find(value => value.map_file_name === "0");
  const safeZones = (map?.safe_zones ?? []).map(zone => ({ center: point(zone.location), size: zone.size, startPoint: zone.start_point,
    bounds: { minX: zone.location.x - zone.size, maxX: zone.location.x + zone.size,
      minY: zone.location.y - zone.size, maxY: zone.location.y + zone.size } }));
  const homeSafeZone = safeZones.find(zone => zone.startPoint && samePoint(zone.center, { x: 288, y: 616 }));
  demand(homeSafeZone && safeZones.every(zone => Number.isInteger(zone.size) && zone.size >= 0), "CANONICAL_TOWN_SAFE_GEOMETRY_UNAVAILABLE");
  const origin = /else\s*\{\s*\(([\d.]+),\s*([\d.]+)\)\s*\};\s*let text_font/u.exec(bag);
  const pair = name => new RegExp(`${name}: Point = Point \\{ x: (\\d+), y: (\\d+) \\}`).exec(layouts);
  const grid = pair("INVENTORY_GRID_ORIGIN"), step = pair("INVENTORY_GRID_STEP");
  const shopWidth = /pub const SURFACE_WIDTH: f32 = ([\d.]+);/u.exec(shop), shopHeight = /pub const SURFACE_HEIGHT: f32 = ([\d.]+);/u.exec(shop);
  const sell = /"Sell",\s*scaled\(CrystalRect::new\(([\d.]+),\s*([\d.]+),\s*([\d.]+),\s*([\d.]+)\), scale\),\s*&font,\s*NpcShopUiAction::HandoffSell/u.exec(paint);
  demand(origin && grid && step && /INVENTORY_PAGE_COLUMNS: usize = 8/u.test(layouts), "BAG_SOURCE_GEOMETRY_CHANGED");
  demand(shopWidth && shopHeight && sell && shop.includes("left: Val::Px((p.logical_width - width) * 0.5)")
    && shop.includes("top: Val::Px((p.logical_height - height) * 0.5)"), "NPC_SHOP_SOURCE_GEOMETRY_CHANGED");
  return { keyboard, bag: { x: Number(origin[1]), y: Number(origin[2]), gridX: Number(grid[1]), gridY: Number(grid[2]),
    stepX: Number(step[1]), stepY: Number(step[2]), columns: 8, cellCenterX: 18, cellCenterY: 16 },
    shop: { width: Number(shopWidth[1]), height: Number(shopHeight[1]),
      sell: { x: Number(sell[1]), y: Number(sell[2]), width: Number(sell[3]), height: Number(sell[4]) } },
    safeZones, homeSafeZone, safeZoneSourceSha256: digest(respawnBytes) };
}

function keyChord(keyboard, functionName) {
  const binding = keyboard.find(row => row.function === functionName);
  demand(binding && binding.key !== "None" && binding.tilde !== 1, "CANONICAL_KEY_BINDING_MISSING");
  const key = /^D[0-9]$/.test(binding.key) ? `Digit${binding.key.slice(1)}`
    : /^[A-Z]$/.test(binding.key) ? `Key${binding.key}` : binding.key;
  return [...["Control", "Alt", "Shift"].filter(name => binding[{ Control: "ctrl", Alt: "alt", Shift: "shift" }[name]] === 1), key].join("+");
}

/** Matches the ordinary desktop NPC text wrapping in quest_ui.rs, not a game action. */
function npcWrappedLines(text) {
  const lines = [], width = text => Array.from(text).reduce((sum, char) => sum + (char.codePointAt(0) < 128 ? 1 : 2), 0);
  for (const source of String(text).split(/\r?\n/u)) {
    let row = "", used = 0;
    for (const word of source.trim().split(/\s+/u).filter(Boolean)) {
      const chunks = []; let chunk = "", chunkWidth = 0;
      for (const char of word) {
        const advance = char.codePointAt(0) < 128 ? 1 : 2;
        if (chunk && chunkWidth + advance > 54) { chunks.push(chunk); chunk = ""; chunkWidth = 0; }
        chunk += char; chunkWidth += advance;
      }
      if (chunk) chunks.push(chunk);
      if (chunks.length > 1) { if (row) lines.push(row); lines.push(...chunks); row = ""; used = 0; continue; }
      const next = chunks[0] ?? "", nextWidth = width(next);
      if (!row) { row = next; used = nextWidth; }
      else if (used + nextWidth + 1 <= 54) { row += ` ${next}`; used += nextWidth + 1; }
      else { lines.push(row); row = next; used = nextWidth; }
    }
    if (row) lines.push(row);
  }
  return lines;
}

function npcOptionRow(dialog, target) {
  const canonical = text => String(text).replace(/[^\p{L}\p{N}]/gu, "").toLowerCase();
  const source = [dialog.title, ...dialog.body, dialog.footer].filter(Boolean);
  if (source[0] && canonical(source[0]) === canonical(dialog.npcName)) source.shift();
  let index = source.reduce((sum, line) => sum + npcWrappedLines(line).length, 0);
  for (const link of dialog.links) {
    // Quest service actions have separate canonical controls, not inline rows.
    if (/^@(accept|finish|complete|abort|cancel)quest/iu.test(link.target)) continue;
    if (link.target === target) return index;
    index += npcWrappedLines(link.text).length;
  }
  throw failure("OBSERVED_NPC_OPTION_NOT_IN_CANONICAL_ROWS");
}

function brief(state) {
  return { ready: state.ready, connectionEpoch: state.connectionEpoch, characterEpoch: state.characterEpoch,
    revision: state.revision, tick: state.tick, mapFileName: state.mapFileName, inSafeZone: state.inSafeZone,
    player: state.player && { ...point(state.player), name: state.player.name, objectId: state.playerObjectId,
      direction: state.player.direction, level: state.player.level }, hp: state.playerHp, maxHp: state.playerMaxHp,
    gold: state.gold, inventoryStale: state.inventoryStale, inventory: state.inventoryItems.map(itemBrief),
    belt: state.beltItems.map(itemBrief), equipment: state.equipmentItems.map(itemBrief), groundDrops: state.groundDrops.map(dropBrief),
    activeNpcDialog: state.activeNpcDialog ? { npcObjectId: state.activeNpcDialog.npcObjectId,
      npcName: state.activeNpcDialog.npcName, title: state.activeNpcDialog.title } : null,
    party: state.party, trade: { active: state.trade.active, uncertain: state.trade.uncertain,
      idHash: state.trade.id ? digest(state.trade.id) : null, ownGold: state.trade.ownGold,
      ownItems: state.trade.ownItems, partnerGold: state.trade.partnerGold, partnerItems: state.trade.partnerItems,
      locked: state.trade.locked, escrowPrepared: state.trade.escrowPrepared,
      deliveryItems: state.trade.deliveryItems, deliveryGold: state.trade.deliveryGold,
      finalConfirmTradeIdHash: state.trade.lastFinalConfirm?.tradeId ? digest(state.trade.lastFinalConfirm.tradeId) : null } };
}

const PACKET_FIELDS = ["result", "objectId", "attackerId", "targetId", "success", "amount", "gold", "uniqueId", "count",
  "from", "to", "grid", "direction", "hp", "maxHp", "percent", "location", "x", "y", "locked", "heroInventory",
  "name", "image", "quantity", "sourceMonster", "ownerObjectId", "ownershipRemainingTicks"];
const EVIDENCE_PACKETS = new Set(["NewAccount", "LoginSuccess", "NewCharacterSuccess", "StartGame", "LogOutSuccess", "UserLocation",
  "ObjectAttack", "ObjectStruck", "ObjectHealth", "ObjectDied", "HealthChanged", "UseItem", "GainedItem", "DeleteItem", "RefreshItem",
  "GainGold", "LoseGold", "ObjectItem", "ObjectGold", "ObjectRemove", "GainExperience", "SwitchGroup", "GroupInvite", "GroupMemberInfo",
  "NPCSell", "SellItem", "TradeRequest", "TradeAccept", "TradeGold", "TradeItem", "TradeCancel", "TradeConfirm", "DepositTradeItem", "RetrieveTradeItem"]);
function packetEvidence(event) {
  const payload = {};
  for (const key of PACKET_FIELDS) {
    const value = event.payload?.[key];
    if (value == null || ["string", "number", "bigint", "boolean"].includes(typeof value)) payload[key] = value;
    else if (key === "location" && Number.isInteger(value.x) && Number.isInteger(value.y)) payload.location = point(value);
  }
  if (["GainedItem", "RefreshItem"].includes(event.packet)) {
    const item = event.payload?.item;
    if (item && typeof item === "object") payload.item = { uniqueId: item.uniqueId ?? item.unique_id,
      count: item.count ?? item.quantity, itemIndex: item.itemIndex ?? item.item_index };
  }
  if (event.packet === "LoginSuccess" && Array.isArray(event.payload?.characters)) payload.characters = event.payload.characters.map(value => ({
    index: value.index, name: value.name }));
  if (event.packet === "NewCharacterSuccess" && event.payload?.character) payload.character = {
    index: event.payload.character.index, name: event.payload.character.name };
  return { packet: event.packet, payload };
}

/**
 * Returns only safe evidence. Optional private test identities stay outside the
 * screenshots, traces, stdout, environment dumps and ordinary outcome reports.
 */
export async function runR16WebAcceptance(input) {
  const options = validateOptions(await configOptions(input)), uiLayout = await sourceUiLayout();
  const playwright = await loadPlaywright(options.playwrightModule);
  const collision = await loadClientCollision("0");
  await mkdir(options.outputDir, { recursive: false }); // Never overwrite another run.
  // Match the real shared Crystal form's 3-15/5-15 alphanumeric validation.
  let primaryIdentity = { accountId: `wr16${randomBytes(5).toString("hex")}`, password: `Ab${randomBytes(5).toString("hex")}7`,
    characterName: `Web${randomBytes(4).toString("hex")}` };
  let retainedPrimaryCharacterName = null;
  if (options.reusePrivateAccountState) {
    const path = options.reusePrivateAccountState, stat = await lstat(path);
    demand(stat.isFile() && !stat.isSymbolicLink() && stat.size > 0 && stat.size <= 16_384
      && relative(path, await realpath(path)) === "", "REUSED_PRIVATE_QA_FILE_MUST_BE_OWNED_REGULAR_FILE");
    let saved;
    try { saved = JSON.parse(await readFile(path, "utf8")); } catch { throw failure("PRIVATE_QA_ACCOUNT_STATE_INVALID"); }
    retainedPrimaryCharacterName = saved.primary?.characterName ?? null;
    while (primaryIdentity.characterName === retainedPrimaryCharacterName) primaryIdentity.characterName = `Web${randomBytes(4).toString("hex")}`;
    primaryIdentity = reusedPrimaryIdentity(saved, options, primaryIdentity.characterName);
  }
  const peerIdentity = { accountId: `wr16${randomBytes(5).toString("hex")}`, password: `Ab${randomBytes(5).toString("hex")}7`,
    characterName: `Pair${randomBytes(3).toString("hex")}` };
  if (options.privateAccountState) {
    await mkdir(dirname(options.privateAccountState), { recursive: true });
    // Private ordinary test identities only. Write once before their normal
    // registration; actual registration success remains a separate UI receipt.
    // This object/path is never copied to a report, trace, screenshot or stdout.
    await writeFile(options.privateAccountState, serializable({ schema: "mir2.r16.private-ordinary-qa-identities.v1",
      createdAt: new Date().toISOString(), realmId: options.realmId, gatewayUrl: options.gatewayUrl,
      primary: primaryIdentity, peer: peerIdentity, registrationState: "verify-the-ordinary-registration-receipts" }), { flag: "wx", mode: 0o600 });
  }
  const summary = { schema: "mir2.r16.real-web-acceptance.v1", startedAt: new Date().toISOString(), passed: false,
    scope: { firstPlayer: "real browser DOM/Canvas pointer and keyboard", secondPlayer: "ordinary authenticated GatewayClient",
      humanOperators: false, networkMock: false, debugActionCalls: false, privilegedCommands: false,
      savesEdited: false, assetsGranted: false, monstersSpawned: false, teleport: false, modelCalls: 0,
      backendParityAcceptance: false, candidate100: false, humanAccepted: false },
    realmIdHash: digest(options.realmId), expectedProfile: { profileId: "platinum_fiveclass", profileVersion: 1 },
    naturalDropHuntBudget: options.maxDropKills,
    checks: [], screenshots: [], commandCounts: { browser: {}, peer: {} }, safetyEvents: [], phase: "browser-start" };
  if (options.stopAfterEquip) summary.scope.diagnosticOnly = "stop-after-owned-sword-equip";
  if (options.reusePrivateAccountState) summary.scope.reusedTestAccount = true;
  if (options.stopAfterLoot) summary.scope.diagnosticOnly = "stop-after-natural-monster-loot-pickup";
  if (options.requireNativeBag || options.stopAfterEquip) summary.scope.nativeBagRequired = true;
  const events = [], primaryObservation = new ObservationReducer();
  let browser, context, page, peer, activeSocket, generation = 0, sequence = 0, fatalCode = null;
  let lastPrimaryFingerprint = "", lastPeerFingerprint = "", preferKeyboardNavigation = true;
  let lastPhysicalPointer = null, lastBrowserInteract = null, lastMovementTarget = null;
  let loginCharacters = [], lastPrimaryStartGame = null;
  const received = [], primary = () => primaryObservation.snapshot();
  const lastFullSnapshot = { browser: null, peer: null }, lastUserLocation = { browser: null, peer: null };
  const note = (source, type, fields = {}) => {
    events.push({ at: Date.now(), source, type, ...fields });
    if (events.length > 20_000) { events.shift(); summary.eventsTruncated = true; }
  };
  const safety = (code, fields = {}) => { summary.safetyEvents.push({ at: Date.now(), code, ...fields }); note("runner", "safety", { code, ...fields }); };
  const check = (name, evidence) => { summary.checks.push({ name, passed: true, ...evidence }); note("runner", "check", { name }); };
  const phase = name => { summary.phase = name; options.onProgress?.({ phase: name }); };
  const healthy = () => { if (fatalCode) throw failure(fatalCode); };
  const currentSnapshotSafetyMismatch = (source, state) => {
    const snapshot = lastFullSnapshot[source], ack = lastUserLocation[source];
    return snapshot && ack && snapshot.connectionEpoch === state.connectionEpoch && snapshot.characterEpoch === state.characterEpoch
      && ack.connectionEpoch === state.connectionEpoch && ack.characterEpoch === state.characterEpoch
      && snapshot.mapFileName === "0" && state.mapFileName === "0" && samePoint(snapshot.position, state.player)
      && samePoint(ack.position, state.player) && snapshot.inSafeZone === false
      && uiLayout.safeZones.some(zone => inBounds(state.player, zone.bounds));
  };
  const acceptedTownSafety = (code, source = "browser", state = source === "browser" ? primary() : peer.snapshot()) => {
    healthy();
    const ack = lastUserLocation[source], snapshot = lastFullSnapshot[source];
    demand(state.mapFileName === "0" && inBounds(state.player, uiLayout.homeSafeZone.bounds)
      && ack && ack.connectionEpoch === state.connectionEpoch && ack.characterEpoch === state.characterEpoch
      && ack.mapFileName === state.mapFileName && samePoint(ack.position, state.player), code);
    demand(!currentSnapshotSafetyMismatch(source, state), "CURRENT_POSITION_FULL_SNAPSHOT_REPORTS_UNSAFE");
    note(source, "safe-zone-geometry-and-accepted-position-proof", { position: point(state.player), bounds: uiLayout.homeSafeZone.bounds,
      sourceSha256: uiLayout.safeZoneSourceSha256, acceptedUserLocation: ack, observedSnapshotFlag: state.inSafeZone,
      fullSnapshot: snapshot, snapshotAgeMs: snapshot ? Date.now() - snapshot.receivedAtMs : null,
      snapshotAtCurrentPosition: Boolean(snapshot && samePoint(snapshot.position, state.player)), flagOverridden: false });
  };
  const poll = async (read, predicate, code, timeout = 25_000) => {
    const deadline = Date.now() + timeout;
    while (Date.now() < deadline) { healthy(); const value = await read(); if (await predicate(value)) return value; await sleep(100); }
    throw failure(code);
  };
  const countCommand = (source, command) => {
    const type = typeof command?.type === "string" ? command.type : "invalid";
    summary.commandCounts[source][type] = (summary.commandCounts[source][type] ?? 0) + 1;
    note(source, "command", { command: type }); // No raw auth/chat/action payload is retained.
    if (source === "browser" && type === "newAccount" && options.reusePrivateAccountState) {
      fatalCode = "REUSED_TEST_ACCOUNT_SENT_NEW_ACCOUNT"; safety(fatalCode);
    }
    if (source === "browser" && type === "startGame") {
      lastPrimaryStartGame = { characterIndex: command.characterIndex, at: Date.now(), sequence };
      note(source, "start-game-character-selection", lastPrimaryStartGame);
    }
    if (source === "browser" && type === "interact") {
      const state = primary(), pointer = lastPhysicalPointer;
      lastBrowserInteract = pointer && Date.now() - pointer.at <= 1500 && pointer.connectionEpoch === state.connectionEpoch
        && pointer.characterEpoch === state.characterEpoch ? { ...pointer, objectId: uid(command.objectId), commandAt: Date.now() } : null;
    }
    if (/^(moveTo|stage5Command|passkeyLogin)$/iu.test(type) || /^(qa\.|event\.)/iu.test(type)
      || type === "chat" && /(?:qa\.|event\.spawn|crystal:)/iu.test(String(command.message ?? ""))
      || ["login", "newAccount"].includes(type) && command.accountId === "demo") {
      fatalCode = "PRIVILEGED_OR_DEMO_COMMAND_DETECTED"; safety(fatalCode, { source, command: type });
    }
  };
  const observe = (source, event, ordinal) => {
    if (event.type === "error") { fatalCode = "GATEWAY_ERROR_EVENT"; safety(fatalCode, { source, ...gatewayErrorEvidence(event) }); return; }
    if (event.type === "realmInfo") {
      const realm = event.payload;
      if (realm?.realmId !== options.realmId || realm.profileId !== "platinum_fiveclass" || realm.profileVersion !== 1) {
        fatalCode = "REALM_OR_R16_PROFILE_MISMATCH"; safety(fatalCode, { source });
      }
      if (source === "browser") summary.observedProfile = { profileId: realm?.profileId, profileVersion: realm?.profileVersion,
        sharedParty: realm?.features?.sharedParty === true };
    }
    if (event.type === "packet") {
      const state = source === "browser" ? primary() : peer.snapshot();
      const frame = { source, sequence: ordinal, at: Date.now(), connectionEpoch: state.connectionEpoch,
        characterEpoch: state.characterEpoch, mapFileName: state.mapFileName, ...packetEvidence(event) };
      received.push(frame);
      if (received.length > 15_000) received.shift();
      if (EVIDENCE_PACKETS.has(event.packet)) note(source, "packet", frame);
    }
    if (["packet", "worldSnapshot"].includes(event.type)) {
      const state = source === "browser" ? primary() : peer.snapshot();
      if (event.type === "worldSnapshot") {
        lastFullSnapshot[source] = { sequence: ordinal, receivedAtMs: Date.now(), connectionEpoch: state.connectionEpoch,
          characterEpoch: state.characterEpoch, mapFileName: event.payload.mapFileName,
          position: state.player ? point(state.player) : null, inSafeZone: event.payload.inSafeZone, tick: event.payload.tick,
          groundDrops: state.groundDrops.map(dropBrief) };
        note(source, "full-world-snapshot-metadata", lastFullSnapshot[source]);
      } else if (["UserLocation", "UserAttackMove"].includes(event.packet)) {
        const location = event.payload.location ?? event.payload;
        if (Number.isInteger(location.x) && Number.isInteger(location.y)) {
          lastUserLocation[source] = { sequence: ordinal, receivedAtMs: Date.now(), packet: event.packet,
            connectionEpoch: state.connectionEpoch, characterEpoch: state.characterEpoch, mapFileName: state.mapFileName,
            position: point(location), direction: event.payload.direction };
          note(source, "accepted-user-location-metadata", lastUserLocation[source]);
        }
      }
      if (currentSnapshotSafetyMismatch(source, state)) {
        fatalCode = "CURRENT_POSITION_FULL_SNAPSHOT_REPORTS_UNSAFE";
        safety(fatalCode, { source, fullSnapshot: lastFullSnapshot[source], lastUserLocation: lastUserLocation[source] });
      }
      const fingerprint = serializable({ position: state.player && point(state.player), hp: state.playerHp, gold: state.gold,
        inventory: state.inventoryItems.map(itemBrief), belt: state.beltItems.map(itemBrief), groundDrops: state.groundDrops.map(dropBrief),
        trade: state.trade, party: state.party });
      if (fingerprint !== (source === "browser" ? lastPrimaryFingerprint : lastPeerFingerprint)) {
        if (source === "browser") lastPrimaryFingerprint = fingerprint; else lastPeerFingerprint = fingerprint;
        note(source, "authoritative-observation", { state: brief(state) });
      }
    }
  };
  const screenshot = async name => {
    if (!page || page.isClosed()) return;
    const leaf = `${String(summary.screenshots.length + 1).padStart(2, "0")}-${name}.png`;
    try {
      await page.screenshot({ path: resolve(options.outputDir, leaf), fullPage: true,
        mask: [page.locator("[data-login-auth-field]")] });
      summary.screenshots.push(leaf);
    } catch { safety("SCREENSHOT_CAPTURE_FAILED"); }
  };
  const nativeKey = async functionName => {
    healthy(); note("runner", "native-key-input", { functionName, before: await nativeInputSnapshot() });
    await page.keyboard.press(keyChord(uiLayout.keyboard, functionName));
  };
  const sceneGeometry = (publicObservation = null) => page.locator(".client-stage-frame").evaluate((element, observation) => {
    const rect = element.getBoundingClientRect(), data = element.dataset;
    const main = element.closest("main"), diagnostic = name => {
      const value = main?.getAttribute(name);
      if (!value) return null;
      if (value.length > 131072) return { invalid: "oversize-public-diagnostic" };
      try { return JSON.parse(value); } catch { return { invalid: "malformed-public-diagnostic" }; }
    };
    const active = document.activeElement, editable = active instanceof HTMLElement
      && (active.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(active.tagName));
    const nearIds = new Set(observation?.nearbyObjectIds ?? []);
    const publicDom = { activeElement: active ? { tag: active.tagName, id: active.id || null, role: active.getAttribute("role"),
      editable, withinGameFrame: element.contains(active) } : null,
      visibleModals: [...document.querySelectorAll('[role="dialog"], .npc-dialog-panel, .npc-shop-window, [data-trade-state="open"]')]
        .filter(value => { const rect = value.getBoundingClientRect(), style = getComputedStyle(value);
          return rect.width > 0 && rect.height > 0 && style.display !== "none" && style.visibility !== "hidden"; })
        .map(value => ({ tag: value.tagName, role: value.getAttribute("role"), ariaModal: value.getAttribute("aria-modal"),
          npcDialog: value.classList.contains("npc-dialog-panel"), npcShop: value.classList.contains("npc-shop-window"),
          trade: value.getAttribute("data-trade-state") === "open" })),
      nearbyObjectElements: nearIds.size ? [...element.querySelectorAll("[data-object-id]")]
        .filter(value => nearIds.has(value.getAttribute("data-object-id"))).slice(0, 80).map(value => {
          const rect = value.getBoundingClientRect(), style = getComputedStyle(value);
          const className = typeof value.className === "string" ? value.className.slice(0, 240) : "";
          return { objectId: value.getAttribute("data-object-id"), tag: value.tagName, className,
            deadClass: /dead/iu.test(className), visible: rect.width > 0 && rect.height > 0 && style.display !== "none" && style.visibility !== "hidden",
            pointerEvents: style.pointerEvents, bounds: { x: rect.x, y: rect.y, width: rect.width, height: rect.height } };
        }) : [] };
    return { x: rect.x, y: rect.y, width: rect.width, height: rect.height,
      logicalWidth: Number(data.viewportSceneWidth), logicalHeight: Number(data.viewportSceneHeight),
      playerX: Number(data.viewportPlayerX), playerY: Number(data.viewportPlayerY),
      centerX: Number(data.viewportTileCenterX), centerY: Number(data.viewportTileCenterY),
      cellWidth: Number(data.viewportCellWidth), cellHeight: Number(data.viewportCellHeight), bagOwner: data.bagUiOwner,
      bagOwnerRevision: data.bagOwnerRevision ?? null, screen: main?.getAttribute("data-client-screen"),
      sceneAssetsPending: element.classList.contains("scene-assets-pending"),
      visible: rect.width > 0 && rect.height > 0 && getComputedStyle(element).visibility !== "hidden"
        && getComputedStyle(element).display !== "none", focused: document.hasFocus(), documentVisibility: document.visibilityState,
      sharedCanvasId: main?.getAttribute("data-shared-ui-surface") ?? null, inputProfile: main?.getAttribute("data-input-profile"),
      publicBag: diagnostic("data-bag-input-status"), publicShop: diagnostic("data-npc-shop-input-status"),
      publicMovement: diagnostic("data-movement-input-status"), publicDom };
  }, publicObservation);
  const nativeInputSnapshot = async ({ target = null } = {}) => {
    const state = primary(), currentTarget = target ?? (lastMovementTarget?.connectionEpoch === state.connectionEpoch
      && lastMovementTarget.characterEpoch === state.characterEpoch && lastMovementTarget.mapFileName === state.mapFileName ? lastMovementTarget.tile : null);
    const observedEntities = state.entities.filter(value => distance(value, state.player) <= 3 || currentTarget && distance(value, currentTarget) <= 3)
      .map(value => ({ objectId: value.objectId, kind: value.kind, name: value.name, ...point(value), direction: value.direction, hp: value.hp, dead: value.dead }));
    const { publicBag, publicShop, publicMovement, publicDom, ...geometry } = await sceneGeometry({ nearbyObjectIds: observedEntities.map(value => uid(value.objectId)) });
    return { geometry, publicBag, publicShop, publicMovement, publicDom,
      publicObservedEntities: observedEntities, publicMovementTarget: currentTarget ? { tile: point(currentTarget),
        importedCollisionBlocked: collision.blocked.has(`${currentTarget.x},${currentTarget.y}`) } : null,
      publicNpcDialog: state.activeNpcDialog ? { npcObjectId: state.activeNpcDialog.npcObjectId,
        npcName: state.activeNpcDialog.npcName, title: state.activeNpcDialog.title } : null, run: { observedSocketGeneration: generation,
      connectionEpoch: state.connectionEpoch, characterEpoch: state.characterEpoch } };
  };
  const bagInputKey = value => serializable({ run: value.run, geometry: value.geometry });
  const stableNativeBag = async () => {
    let stableSince = 0, lastKey = null, samples = 0;
    const value = await poll(nativeInputSnapshot, current => {
      const g = current.geometry, key = bagInputKey(current);
      if (g.bagOwner !== "bevy" || current.publicBag?.active !== true || current.publicBag?.status?.ready !== true
        || current.publicBag?.status?.inputEnabled !== true || !g.visible || !g.focused || g.screen !== "game" || g.documentVisibility !== "visible") {
        stableSince = 0; lastKey = null; samples = 0; return false;
      }
      if (lastKey !== key) { lastKey = key; stableSince = Date.now(); samples = 1; return false; }
      return ++samples >= 4 && Date.now() - stableSince >= 650;
    }, "NATIVE_BAG_OWNER_GEOMETRY_DID_NOT_STABILIZE");
    note("runner", "native-bag-input-stable", { stableMs: Date.now() - stableSince, samples, state: value });
    return value;
  };
  const shopWindowInteractive = async locator => await locator.count() === 1
    && await locator.getAttribute("aria-disabled") !== "true" && await locator.getAttribute("inert") === null;
  const stableShopRoute = async () => {
    let stableSince = 0, lastKey = null, samples = 0, result = null;
    await poll(nativeInputSnapshot, async current => {
      const host = current.publicShop?.host, g = current.geometry;
      let route = null;
      if (host?.active === true && host.transitioning === false && host.blocksInput === true && host.errorPresent === false
        && current.publicShop.renderServiceCurrent === true && g.visible && g.focused && g.screen === "game") route = "native";
      else if (!host || host.active === false && host.transitioning === false && host.blocksInput === false) {
        const dom = page.locator(".npc-shop-window:visible");
        if (await shopWindowInteractive(dom)) {
          const tab = await dom.getAttribute("data-shop-tab");
          if (tab === "sell") route = "dom-sell";
          else if (tab === "buy" && await dom.locator('[data-shop-tab-key="sell"]').isEnabled().catch(() => false)) route = "dom-buy";
        }
      }
      if (!route) { stableSince = 0; lastKey = null; samples = 0; return false; }
      const key = serializable({ route, host: host ?? null, run: current.run, geometry: g });
      if (lastKey !== key) { lastKey = key; stableSince = Date.now(); samples = 1; return false; }
      if (++samples < 3 || Date.now() - stableSince < 500) return false;
      result = { route, state: current }; return true;
    }, "NPC_SHOP_NO_STABLE_NATIVE_OR_INTERACTIVE_DOM_OWNER");
    note("runner", "npc-shop-stable-input-owner", { ...result, stableMs: Date.now() - stableSince, samples });
    return result;
  };
  const canvasClick = async (x, y, { clickCount = 1, requireNpcShopOwner = false, observedBefore = null, observeAfter = true } = {}) => {
    let lastDeferred = null;
    const before = requireNpcShopOwner ? await poll(nativeInputSnapshot, current => {
      demand(primary().playerHp > 0, "PLAYER_DIED_BEFORE_NATIVE_SHOP_INPUT");
      const host = current.publicShop?.host, g = current.geometry;
      const ready = host?.active === true && host.transitioning === false && host.blocksInput === true
        && host.errorPresent === false && current.publicShop.renderServiceCurrent === true && g.visible && g.focused;
      if (!ready) {
        const key = serializable({ host: host ?? null, run: current.run });
        if (key !== lastDeferred) { lastDeferred = key; note("runner", "native-shop-pointer-deferred-without-input", { publicShop: current.publicShop, run: current.run }); }
      }
      return ready;
    }, "NPC_SHOP_OWNER_NOT_CURRENT_AT_PHYSICAL_INPUT", 10_000) : observedBefore ?? await nativeInputSnapshot();
    demand(before.run.observedSocketGeneration === generation && before.run.connectionEpoch === primary().connectionEpoch
      && before.run.characterEpoch === primary().characterEpoch,
      "BROWSER_CONNECTION_CHANGED_BEFORE_PHYSICAL_INPUT");
    const geo = before.geometry;
    demand(geo.logicalWidth > 0 && geo.logicalHeight > 0 && x >= 0 && y >= 0 && x < geo.logicalWidth && y < geo.logicalHeight, "INVALID_PUBLIC_SCENE_GEOMETRY");
    const screenX = geo.x + x / geo.logicalWidth * geo.width, screenY = geo.y + y / geo.logicalHeight * geo.height, startedAt = Date.now();
    note("runner", "native-pointer-input", { startedAt, logicalX: x, logicalY: y, screenX, screenY, clickCount, before });
    lastPhysicalPointer = { at: startedAt, sequence, connectionEpoch: primary().connectionEpoch, characterEpoch: primary().characterEpoch };
    await page.mouse.click(screenX, screenY, { clickCount, delay: 65 });
    const durationMs = Date.now() - startedAt;
    note("runner", "native-pointer-input-complete", { startedAt, durationMs, after: observeAfter ? await nativeInputSnapshot() : null });
  };
  const clickWorldObject = async selector => {
    const object = page.locator(selector); await object.waitFor({ state: "visible" });
    const bounds = await object.boundingBox(); demand(bounds && bounds.width > 0 && bounds.height > 0, "WORLD_OBJECT_HAS_NO_VISIBLE_HIT_TARGET");
    // The shared canvas may legitimately be above DOM entity labels. Deliver
    // the physical pointer to the visible label; the product's hit router owns
    // passthrough. Do not force-click an occluded DOM handler.
    lastPhysicalPointer = { at: Date.now(), sequence, connectionEpoch: primary().connectionEpoch, characterEpoch: primary().characterEpoch };
    await page.mouse.click(bounds.x + bounds.width / 2, bounds.y + bounds.height / 2, { delay: 65 });
  };
  const focusWorld = async () => {
    const before = await nativeInputSnapshot();
    if (before.geometry.focused && before.publicDom.activeElement?.editable !== true) {
      note("runner", "world-focus-already-available-without-input", { state: before }); return;
    }
    note("runner", "world-dom-canvas-focus", { before });
    // Standard DOM focus only. A pointer at the player's tile can select an
    // overlapping NPC or corpse and is never a harmless focus operation.
    const canvas = page.locator("#mir2-web3-canvas:visible");
    demand(await canvas.count() === 1, "VISIBLE_GAME_CANVAS_FOCUS_TARGET_AMBIGUOUS");
    await canvas.focus();
    const after = await nativeInputSnapshot();
    demand(after.geometry.focused && after.publicDom.activeElement?.editable !== true, "GAME_CANVAS_DID_NOT_OBTAIN_DOM_FOCUS");
    note("runner", "world-dom-canvas-focus-complete", { after });
  };
  const uiStep = async (direction, observedInput = null) => {
    const offsets = [[0, -1], [1, -1], [1, 0], [1, 1], [0, 1], [-1, 1], [-1, 0], [-1, -1]];
    const index = DIRECTIONS.indexOf(direction); demand(index >= 0, "INVALID_NAVIGATION_DIRECTION");
    const before = primary(), afterSequence = sequence, [dx, dy] = offsets[index],
      observedBefore = observedInput ?? await nativeInputSnapshot(), geo = observedBefore.geometry;
    // Hit the adjacent world tile on the real canvas; React/Bevy owns the intent.
    await canvasClick(geo.centerX + dx * geo.cellWidth, geo.centerY + dy * geo.cellHeight, { observedBefore, observeAfter: false });
    let accepted = null;
    const after = await poll(primary, state => {
      demand(state.connectionEpoch === before.connectionEpoch && state.characterEpoch === before.characterEpoch,
        "BROWSER_CONNECTION_CHANGED_DURING_UI_ACTION");
      demand(state.mapFileName === before.mapFileName, "BROWSER_MAP_CHANGED_DURING_UI_ACTION");
      const ack = lastUserLocation.browser, outcome = cardinalAcknowledgement({ before, state, ack, afterSequence, direction });
      if (outcome) accepted = { outcome, ack };
      return Boolean(outcome);
    }, "UI_WALK_NOT_ACKNOWLEDGED", 4_000);
    demand(after.mapFileName === "0" && after.playerHp > 0, "UI_WALK_LEFT_MAP_OR_PLAYER_DIED");
    note("runner", "native-mouse-navigation-accepted", { direction, outcome: accepted.outcome,
      moved: accepted.outcome === "moved", turnedOnly: accepted.outcome === "turned-only", from: point(before.player),
      position: point(after.player), lastUserLocation: accepted.ack });
    await sleep(450);
    return accepted;
  };
  const tryOneFreeMouseStep = async () => {
    const state = primary(), before = await nativeInputSnapshot(), geo = before.geometry;
    summary.navigation = { default: "real-cardinal-keyboard", optionalMouse: { attempted: false, completedMovement: false } };
    if (before.publicBag?.active || before.publicShop?.host?.blocksInput || before.publicNpcDialog
      || before.publicDom.visibleModals.length || !geo.focused || !geo.visible || !samePoint(state.player, { x: geo.playerX, y: geo.playerY })) {
      summary.navigation.optionalMouse.reason = "no-current-clear-world-input-surface";
      note("runner", "optional-mouse-navigation-skipped", { ...summary.navigation, before }); return;
    }
    const candidates = clearMouseStepCandidates({ state, collision, bounds: uiLayout.homeSafeZone.bounds }).map(value => ({ ...value,
      screenX: geo.x + (geo.centerX + value.dx * geo.cellWidth) / geo.logicalWidth * geo.width,
      screenY: geo.y + (geo.centerY + value.dy * geo.cellHeight) / geo.logicalHeight * geo.height }));
    const hitChecks = await page.evaluate(points => points.map(value => {
      const stack = document.elementsFromPoint(value.screenX, value.screenY);
      const canvas = stack.some(element => element instanceof HTMLCanvasElement && ["mir2-web3-canvas", "mir2-quest-ui-canvas"].includes(element.id));
      const blocked = stack.some(element => element.closest('.entity-nameplate, .entity-sprite-hit, .ground-drop-marker, button, input, textarea, select, [role="dialog"], .npc-dialog-panel, .npc-shop-window'));
      return { direction: value.direction, free: canvas && !blocked, stack: stack.slice(0, 12).map(element => ({
        tag: element.tagName, id: element.id || null, class: typeof element.className === "string" ? element.className.slice(0, 100) : null })) };
    }), candidates);
    const selected = candidates.find(value => hitChecks.some(hit => hit.direction === value.direction && hit.free));
    note("runner", "optional-mouse-public-free-hit-check", { position: point(state.player), candidates, hitChecks });
    if (!selected || !samePoint(primary().player, state.player)
      || !clearMouseStepCandidates({ state: primary(), collision, bounds: uiLayout.homeSafeZone.bounds }).some(value => value.direction === selected.direction)) {
      summary.navigation.optionalMouse.reason = "no-stable-unoccupied-public-free-hit-tile";
      note("runner", "optional-mouse-navigation-skipped", summary.navigation); return;
    }
    summary.navigation.optionalMouse = { attempted: true, completedMovement: false, direction: selected.direction, target: selected.tile };
    const accepted = await uiStep(selected.direction, before);
    summary.navigation.optionalMouse.completedMovement = accepted.outcome === "moved";
    summary.navigation.optionalMouse.outcome = accepted.outcome;
  };
  const uiCardinalStep = async direction => {
    const key = { Up: "ArrowUp", Right: "ArrowRight", Down: "ArrowDown", Left: "ArrowLeft" }[direction];
    demand(key, "KEYBOARD_NAVIGATION_REQUIRES_ONE_CARDINAL_KEY");
    const before = primary(), afterSequence = sequence, delta = { Up: [0, -1], Right: [1, 0], Down: [0, 1], Left: [-1, 0] }[direction];
    const target = { x: before.player.x + delta[0], y: before.player.y + delta[1] };
    lastMovementTarget = { tile: target, direction, at: Date.now(), connectionEpoch: before.connectionEpoch,
      characterEpoch: before.characterEpoch, mapFileName: before.mapFileName };
    const observedBefore = await nativeInputSnapshot({ target }), geo = observedBefore.geometry;
    demand(geo.screen === "game" && geo.visible && geo.focused && geo.documentVisibility === "visible",
      "WORLD_NOT_FOCUSED_BEFORE_CARDINAL_KEY");
    demand(observedBefore.run.observedSocketGeneration === generation && observedBefore.run.connectionEpoch === before.connectionEpoch
      && observedBefore.run.characterEpoch === before.characterEpoch, "BROWSER_CONNECTION_CHANGED_BEFORE_CARDINAL_KEY");
    summary.lastUiAction = "ordinary-cardinal-key-movement";
    const heldAt = Date.now();
    note("runner", "native-cardinal-key-input", { direction, key, afterSequence, before: observedBefore, position: point(before.player),
      target, heldAt, maximumAckWaitMs: 4000 });
    let accepted = null, after, inputError = null, heldForMs;
    try {
      await page.keyboard.down(key);
      after = await poll(primary, state => {
        demand(state.connectionEpoch === before.connectionEpoch && state.characterEpoch === before.characterEpoch,
          "BROWSER_CONNECTION_CHANGED_DURING_UI_ACTION");
        demand(state.mapFileName === before.mapFileName, "BROWSER_MAP_CHANGED_DURING_UI_ACTION");
        const ack = lastUserLocation.browser;
        const outcome = cardinalAcknowledgement({ before, state, ack, afterSequence, direction });
        if (outcome) accepted = { outcome, ack };
        return Boolean(outcome);
      }, "UI_CARDINAL_KEY_WALK_NOT_ACKNOWLEDGED", Math.max(1, 4000 - (Date.now() - heldAt)));
    } catch (error) { inputError = error; throw error; }
    finally {
      try { await page.keyboard.up(key); }
      catch (error) {
        safety("CARDINAL_KEY_RELEASE_FAILED", { key, detailHash: digest(error?.stack ?? error?.message) });
        if (!inputError) throw failure("CARDINAL_KEY_RELEASE_FAILED");
      }
      heldForMs = Date.now() - heldAt;
      note("runner", "native-cardinal-key-released", { direction, key, heldAt, heldForMs, acknowledged: Boolean(accepted),
        outcome: accepted?.outcome ?? null });
    }
    demand(after.mapFileName === "0" && after.playerHp > 0, "UI_WALK_LEFT_MAP_OR_PLAYER_DIED");
    note("runner", "native-cardinal-key-accepted", { direction, outcome: accepted.outcome,
      moved: accepted.outcome === "moved", turnedOnly: accepted.outcome === "turned-only", from: point(before.player),
      position: point(after.player), lastUserLocation: accepted.ack, heldForMs });
    await sleep(450);
  };
  const walk = async (source, target, desiredDistance = 0, maxSteps = 96, movementBounds = null) => {
    for (let step = 0; step < maxSteps; step++) {
      healthy(); const state = source === "browser" ? primary() : peer.snapshot();
      const destination = typeof target === "function" ? target(state) : target;
      demand(destination && state.ready && state.playerHp > 0, "ORDINARY_WALK_TARGET_OR_PLAYER_UNAVAILABLE");
      if (movementBounds) demand(inBounds(state.player, movementBounds), "ACCEPTED_WALK_LEFT_DECLARED_SAFE_GEOMETRY");
      if (distance(state.player, destination) <= desiredDistance) return state;
      const occupied = state.entities.filter(entity => entity.objectId !== state.playerObjectId && live(entity)).map(point);
      const keyboard = source === "browser" && preferKeyboardNavigation;
      const direction = (keyboard ? nextCardinalWalkDirection : nextWalkDirection)({ player: state.player, target: destination,
        desiredDistance, collision, occupied, movementBounds });
      demand(direction, "NO_ORDINARY_PATH_TO_OBSERVED_TARGET");
      if (source === "browser") {
        if (keyboard) await uiCardinalStep(direction); else await uiStep(direction);
      } else { await peer.walk(direction); await sleep(500); }
    }
    throw failure("ORDINARY_WALK_BUDGET_EXHAUSTED");
  };
  const openBag = async () => {
    const state = primary(), dialog = state.activeNpcDialog;
    if (dialog) {
      demand(lastBrowserInteract && uid(dialog.npcObjectId) === lastBrowserInteract.objectId
        && lastBrowserInteract.connectionEpoch === state.connectionEpoch && lastBrowserInteract.characterEpoch === state.characterEpoch,
      "NPC_DIALOG_WITHOUT_OWNED_UI_INTERACTION_BEFORE_BAG");
      note("runner", "close-own-physically-opened-npc-dialog-before-bag", { lastBrowserInteract, before: await nativeInputSnapshot() });
      const close = page.locator(".npc-dialog-panel:visible .npc-dialog-actions button:enabled");
      if (await close.count() === 1) await close.click();
      else {
        await page.locator("#mir2-quest-ui-canvas:visible").focus();
        await page.keyboard.press("Escape", { delay: 80 });
      }
      await poll(primary, value => !value.activeNpcDialog, "OWN_NPC_DIALOG_DID_NOT_CLOSE_BEFORE_BAG");
      note("runner", "own-npc-dialog-close-observed", { after: await nativeInputSnapshot() });
    }
    await focusWorld();
    await nativeKey("Inventory");
    if (options.requireNativeBag || options.stopAfterEquip) { await stableNativeBag(); return; }
    await poll(sceneGeometry, async geo => geo.bagOwner === "bevy" || await page.locator(".inventory-item-card:visible").count() > 0, "BAG_UI_DID_NOT_OPEN");
    if ((await sceneGeometry()).bagOwner === "bevy") await stableNativeBag();
  };
  const bagDoubleClick = async item => {
    const domItem = page.getByRole("button", { name: item.name, exact: true }).and(page.locator(".inventory-item-card:visible"));
    if (!options.requireNativeBag && !options.stopAfterEquip && await domItem.count() === 1) { await domItem.dblclick({ delay: 80 }); return; }
    demand((await sceneGeometry()).bagOwner === "bevy" && item.container !== "bag2" && Number.isInteger(item.slot) && item.slot >= 0 && item.slot < 40,
      "OWNED_BAG_ITEM_HAS_NO_UNAMBIGUOUS_UI_TARGET");
    const before = await stableNativeBag(), bag = uiLayout.bag, column = item.slot % bag.columns, row = Math.floor(item.slot / bag.columns);
    const economicCommands = () => Object.entries(summary.commandCounts.browser)
      .filter(([type]) => ["equipItem", "useItem", "moveItem", "dropItem", "depositTradeItem", "sellItem", "buyItem"].includes(type))
      .reduce((total, [, count]) => total + count, 0);
    const beforeCommands = economicCommands();
    summary.lastUiAction = "native-owned-bag-double-click";
    await canvasClick(bag.x + bag.gridX + column * bag.stepX + bag.cellCenterX,
      bag.y + bag.gridY + row * bag.stepY + bag.cellCenterY, { clickCount: 2 });
    const after = await nativeInputSnapshot(), ownerChanged = bagInputKey(before) !== bagInputKey(after);
    note("runner", "native-bag-double-click-result", { item: itemBrief(item), ownerChanged, economicCommandSent: economicCommands() > beforeCommands, before, after });
    if (ownerChanged && economicCommands() === beforeCommands) {
      await stableNativeBag();
      // One new physical input is permitted only after an observed ownership or
      // geometry change and proof that no economic command entered the wire.
      demand(economicCommands() === beforeCommands && ownedQuantity(primary(), item.uniqueId) === quantity(item), "BAG_RETRY_HAS_POSSIBLE_ECONOMIC_ACTION");
      await screenshot("native-bag-owner-changed-before-unentered-retry");
      note("runner", "native-bag-unentered-input-retry", { item: itemBrief(item), before: await nativeInputSnapshot() });
      await canvasClick(bag.x + bag.gridX + column * bag.stepX + bag.cellCenterX,
        bag.y + bag.gridY + row * bag.stepY + bag.cellCenterY, { clickCount: 2 });
    }
  };
  const fresh = state => state.ready && !state.inventoryStale && state.fullSnapshot?.inventoryComplete;
  const packetsAfter = (after, name, predicate = () => true) => received.filter(frame => frame.source === "browser"
    && frame.sequence > after && frame.packet === name && predicate(frame.payload));
  const sellOwnDressUi = async () => {
    const state = primary(), dress = state.inventoryItems.find(item => item.name === "BaseDress(M)" && quantity(item) === 1);
    demand(fresh(state) && dress && Number.isSafeInteger(dress.sellValue) && dress.sellValue > 0, "NO_OBSERVED_OWNED_STARTER_DRESS_SALE_VALUE");
    const potionBefore = state.inventoryItems.filter(item => /Drug|Potion/iu.test(item.name)).map(itemBrief);
    const candleBefore = state.inventoryItems.filter(item => item.name === "Candle").map(itemBrief);
    await walk("browser", { x: 305, y: 608 }, 1);
    const merchant = primary().entities.find(entity => entity.kind === "npc" && entity.name === "Merchant_Whitney" && entity.x === 305 && entity.y === 608);
    demand(merchant, "REAL_WHITNEY_NOT_IN_ORDINARY_AOI");
    await clickWorldObject(`.entity-nameplate[data-object-id="${merchant.objectId}"]`);
    await poll(primary, value => value.activeNpcDialog?.npcObjectId === merchant.objectId, "WHITNEY_UI_DIALOG_NOT_OBSERVED");
    const dialog = primary().activeNpcDialog, link = dialog.links.find(value => ["@sell", "@buysell"].includes(value.target.toLowerCase()));
    demand(link, "WHITNEY_HAS_NO_OBSERVED_SELL_LINK");
    const exactLink = page.locator(`.npc-dialog-links button[data-target=${JSON.stringify(link.target)}]:visible`), beforeService = sequence;
    if (await exactLink.count() === 1) await exactLink.click();
    else {
      const source = await readFile(resolve(repoRoot, "apps/game-client/client-bevy/src/quest_ui.rs"), "utf8");
      demand(source.includes("NPC_DIALOG_VISIBLE_ROWS: usize = 9")
        && source.includes("QuestLogRect::new(18.0, 31.0 + index as f32 * 17.0, 390.0, 17.0)"), "NPC_DIALOG_SOURCE_GEOMETRY_CHANGED");
      const row = npcOptionRow(dialog, link.target), pageIndex = Math.floor(row / 9);
      demand(pageIndex <= 4, "NPC_DIALOG_OPTION_OUTSIDE_BOUNDED_UI_PAGES");
      for (let index = 0; index < pageIndex; index++) { await canvasClick(421, 175); await sleep(200); }
      await canvasClick(120, 31 + (row % 9) * 17 + 8);
    }
    await poll(() => packetsAfter(beforeService, "NPCSell"), frames => frames.length > 0, "WHITNEY_SELL_SERVICE_UI_NOT_ACCEPTED");
    summary.lastUiAction = "wait-stable-npc-shop-owner";
    const route = await stableShopRoute();
    const handoffSource = () => serializable({ run: { socket: generation, connection: primary().connectionEpoch, character: primary().characterEpoch },
      merchant: primary().entities.find(entity => entity.objectId === merchant.objectId && entity.kind === "npc")?.objectId ?? null,
      position: point(primary().player), assets: assetFingerprint(primary()),
      interactions: ["interact", "selectNpcDialog", "submitNpcInput"].map(type => summary.commandCounts.browser[type] ?? 0),
      servicePackets: received.filter(frame => frame.source === "browser" && ["NPCGoods", "NPCSell", "NPCResponse", "NPCPearlGoods", "MapInfo"].includes(frame.packet))
        .map(frame => [frame.sequence, frame.packet]),
      economic: ["buyItem", "sellItem", "equipItem", "useItem", "moveItem", "dropItem"].map(type => summary.commandCounts.browser[type] ?? 0) });
    let nativeSourceBefore = null, nativeSellPoint = null;
    if (route.route === "native") {
      summary.lastUiAction = "click-native-npc-shop-footer-sell";
      await screenshot("ordinary-whitney-native-buy-owner");
      const current = await nativeInputSnapshot(), g = current.geometry;
      const scale = g.inputProfile === "touch" ? 1.28 / (g.width / g.logicalWidth) : 1, layout = uiLayout.shop;
      nativeSellPoint = { x: (g.logicalWidth - layout.width * scale) / 2 + (layout.sell.x + layout.sell.width / 2) * scale,
        y: (g.logicalHeight - layout.height * scale) / 2 + (layout.sell.y + layout.sell.height / 2) * scale };
      nativeSourceBefore = handoffSource();
      note("runner", "npc-shop-native-sell-handoff-input", { publicState: current, sourceGeometry: layout });
      await canvasClick(nativeSellPoint.x, nativeSellPoint.y, { requireNpcShopOwner: true });
    } else if (route.route === "dom-buy") {
      summary.lastUiAction = "click-interactive-dom-npc-shop-sell-tab";
      const tab = page.locator('.npc-shop-window[data-shop-tab="buy"]:visible [data-shop-tab-key="sell"]');
      await tab.click();
    }
    summary.lastUiAction = "wait-native-retirement-and-dom-sell-owner";
    const shop = page.locator('.npc-shop-window[data-shop-tab="sell"]:visible');
    const sellOwnerReady = async current => (!current.publicShop?.host || current.publicShop.host.active === false
      && current.publicShop.host.transitioning === false && current.publicShop.host.blocksInput === false)
      && await shopWindowInteractive(shop);
    try {
      await poll(nativeInputSnapshot, sellOwnerReady, "NPC_SELL_HANDOFF_DID_NOT_RELEASE_NATIVE_INPUT", nativeSellPoint ? 4_000 : 25_000);
    } catch (error) {
      if (!nativeSellPoint || error.acceptanceCode !== "NPC_SELL_HANDOFF_DID_NOT_RELEASE_NATIVE_INPUT") throw error;
      const tabAlreadyHandedOff = await page.locator('.npc-shop-window[data-shop-tab="sell"]').count() > 0;
      demand(!tabAlreadyHandedOff && primary().playerHp > 0 && fresh(primary()) && handoffSource() === nativeSourceBefore,
        "NPC_SELL_RETRY_HAS_HANDOFF_OR_CHANGED_SOURCE");
      const retryOwner = await stableShopRoute();
      demand(retryOwner.route === "native" && handoffSource() === nativeSourceBefore, "NPC_SELL_RETRY_SOURCE_NOT_CURRENT");
      summary.lastUiAction = "one-unentered-native-sell-user-reclick";
      note("runner", "native-sell-user-reclick-without-economic-request", { sourceUnchanged: true, localTabHandoff: false, state: retryOwner.state });
      await screenshot("native-sell-no-handoff-before-one-user-reclick");
      await canvasClick(nativeSellPoint.x, nativeSellPoint.y, { requireNpcShopOwner: true });
      summary.lastUiAction = "wait-native-retirement-after-one-user-reclick";
      await poll(nativeInputSnapshot, sellOwnerReady, "NPC_SELL_HANDOFF_DID_NOT_RELEASE_NATIVE_INPUT");
    }
    note("runner", "npc-shop-dom-sell-input-owner", { state: await nativeInputSnapshot() });
    const pickerButton = shop.locator("[data-r16-npc-sell-open]");
    if (await pickerButton.count() === 1) {
      summary.lastUiAction = "open-r16-npc-sell-owned-item-picker";
      await pickerButton.click();
      await shop.locator("[data-r16-npc-sell-picker]").waitFor({ state: "visible" });
    }
    summary.lastUiAction = "wait-owned-dress-sell-row";
    const item = shop.locator(`.npc-shop-row[data-item-id="${uid(dress.uniqueId)}"]`);
    await item.waitFor({ state: "visible" });
    demand(Number(await item.getAttribute("data-unit-price")) === dress.sellValue, "UI_DRESS_SALE_PRICE_DIFFERS_FROM_OWNED_VALUE");
    summary.lastUiAction = "choose-exact-owned-dress-sell-row";
    await item.click();
    if (await pickerButton.count() === 1) {
      await shop.locator("[data-r16-npc-sell-picker]").waitFor({ state: "hidden" });
      demand(await pickerButton.getAttribute("data-item-id") === uid(dress.uniqueId)
        && Number(await pickerButton.getAttribute("data-unit-price")) === dress.sellValue, "NPC_SELL_PICKER_DID_NOT_BIND_EXACT_OWNED_UID");
    }
    await screenshot("ordinary-whitney-owned-dress-sale-review");
    const before = primary(), beforeSale = sequence;
    demand(ownedQuantity(before, dress.uniqueId) === 1, "DRESS_SALE_OWNER_CHANGED");
    summary.lastUiAction = "click-owned-dress-sale-confirm";
    await shop.locator(".npc-shop-confirm").click();
    const sold = await poll(primary, value => fresh(value) && value.gold === before.gold + dress.sellValue && ownedQuantity(value, dress.uniqueId) === 0
      && packetsAfter(beforeSale, "SellItem", payload => payload.success === true && uid(payload.uniqueId) === uid(dress.uniqueId)).length > 0,
    "ORDINARY_UI_DRESS_SALE_NOT_CONFIRMED");
    demand(serializable(sold.inventoryItems.filter(item => /Drug|Potion/iu.test(item.name)).map(itemBrief)) === serializable(potionBefore)
      && serializable(sold.inventoryItems.filter(item => item.name === "Candle").map(itemBrief)) === serializable(candleBefore), "DRESS_SALE_CHANGED_POTION_OR_CANDLE");
    check("ordinary-ui-owned-dress-resale-funds-trade", { merchant: merchant.name, item: itemBrief(dress), quantity: 1,
      observedSaleValue: dress.sellValue, goldBefore: before.gold, goldAfter: sold.gold, potionAndCandlePreserved: true,
      combatRevenueClaimed: false, helperApiCalled: false });
    await screenshot("ordinary-whitney-owned-dress-sold"); summary.lastUiAction = "close-npc-shop";
    const close = shop.locator("button.npc-shop-close, .npc-shop-close button");
    demand(await close.count() === 1, "NPC_SHOP_CLOSE_UI_TARGET_AMBIGUOUS"); await close.click();
    await shop.waitFor({ state: "hidden" }); await focusWorld();
    summary.lastUiAction = null;
  };
  const loginUi = async ({ create = false } = {}) => {
    await page.locator(".login-input.account").waitFor({ state: "visible", timeout: 60_000 });
    await poll(() => page.locator(".login-input.account").isEnabled(), Boolean, "LOGIN_PROTOCOL_NOT_READY", 60_000);
    if (create) {
      await page.locator(".login-button.account button").click();
      const registration = page.locator('[data-login-auth-surface="registration"]');
      await registration.waitFor({ state: "visible" });
      const fields = { accountId: primaryIdentity.accountId, password: primaryIdentity.password, confirmPassword: primaryIdentity.password,
        userName: "Web Acceptance", birthDate: "2000-01-01", secretQuestion: "Private acceptance fixture", secretAnswer: randomBytes(12).toString("hex"),
        emailAddress: `${primaryIdentity.accountId}@example.invalid` };
      for (const [field, value] of Object.entries(fields)) await registration.locator(`[data-login-auth-field="${field}"]`).fill(value);
      const after = sequence;
      await registration.locator('[data-login-auth-submit="registration"]').click();
      await poll(() => packetsAfter(after, "NewAccount"), frames => frames.some(frame => frame.payload.result === 8), "UI_ACCOUNT_REGISTRATION_NOT_ACCEPTED");
      if (await registration.isVisible()) await registration.locator("[data-login-auth-close]").first().click();
      check("ordinary-account-registration-through-ui", { result: 8 });
    }
    await page.locator(".login-input.account").fill(primaryIdentity.accountId);
    await page.locator(".login-input.password").fill(primaryIdentity.password);
    const after = sequence;
    await page.locator(".login-button.ok button").click();
    const login = await poll(() => packetsAfter(after, "LoginSuccess"), frames => frames.length > 0, "UI_LOGIN_NOT_ACCEPTED");
    demand(Array.isArray(login.at(-1).payload.characters), "LOGIN_CHARACTER_LIST_NOT_OBSERVED");
    loginCharacters = login.at(-1).payload.characters;
    await page.locator(".select-overlay").waitFor({ state: "visible", timeout: 30_000 });
  };
  const selectUi = async ({ create = false } = {}) => {
    if (create) {
      const previous = loginCharacters.map(value => ({ ...value }));
      const oldNames = await page.locator(".select-character-slot-card:not(.empty) .name").allTextContents();
      demand(previous.length < 4 && oldNames.length < 4, "ORDINARY_TEST_CHARACTER_SLOTS_FULL");
      if (options.reusePrivateAccountState) demand(previous.some(value => value.name === retainedPrimaryCharacterName)
        && oldNames.includes(retainedPrimaryCharacterName), "REUSED_TEST_ACCOUNT_OLD_CHARACTER_NOT_RETAINED");
      await page.locator(".select-action.new button").click();
      await page.locator(".select-create-name-field input").fill(primaryIdentity.characterName);
      const after = sequence;
      await page.locator(".select-create-actions button").first().click();
      const created = await poll(() => packetsAfter(after, "NewCharacterSuccess"), frames => frames.length > 0, "UI_NEW_CHARACTER_NOT_ACCEPTED");
      const evidence = newCharacterSelectionEvidence(previous, created.at(-1).payload.character, primaryIdentity.characterName,
        Boolean(options.reusePrivateAccountState));
      primaryIdentity.characterIndex = evidence.createdCharacterIndex;
      await poll(() => page.locator(".select-character-slot-card:not(.empty) .name").allTextContents(),
        names => names.length === oldNames.length + 1 && names.includes(primaryIdentity.characterName) && oldNames.every(value => names.includes(value)),
        "NEW_CHARACTER_UI_DID_NOT_RETAIN_PREVIOUS_SLOTS");
      check("ordinary-new-character-through-ui", { ...evidence, previousCharactersRetainedInSelection: true });
    }
    const selected = page.locator(".select-character-slot-card").filter({ hasText: primaryIdentity.characterName });
    demand(await selected.count() === 1, "NEW_CHARACTER_SELECTION_NOT_UNIQUE");
    await selected.click();
    await poll(() => selected.getAttribute("aria-pressed"), value => value === "true", "NEW_CHARACTER_UI_SELECTION_NOT_ACKNOWLEDGED");
    await page.locator(".select-action.start button").click();
    await poll(primary, state => fresh(state) && state.player?.name === primaryIdentity.characterName && state.mapFileName === "0", "UI_START_GAME_NOT_AUTHORITATIVE", 60_000);
    demand(Number.isSafeInteger(primaryIdentity.characterIndex) && lastPrimaryStartGame?.characterIndex === primaryIdentity.characterIndex,
      "START_GAME_DID_NOT_SELECT_NEW_SERVER_CHARACTER_INDEX");
    await poll(() => page.locator(".client-stage-frame").getAttribute("class"), value => !value?.includes("scene-assets-pending"), "SCENE_ASSETS_NOT_READY", 90_000);
    const tutorialClose = page.locator('.mir-tutorial-card button[aria-label="Close tutorial"], .mir-tutorial-card button[aria-label="关闭教程"]');
    if (await tutorialClose.isVisible()) await tutorialClose.click();
    await focusWorld();
  };
  const tradeSection = () => page.locator('[data-trade-state="open"]');
  // These are the real two direct footer buttons. Their labels are localized.
  const tradeAction = index => tradeSection().locator(":scope > div").filter({ has: page.locator(":scope > button") }).last().locator(":scope > button").nth(index);
  const clickTradeAction = async name => {
    const native = page.locator(`[data-r16-trade-${name}]:visible`);
    if (await native.count() === 1) await native.click();
    else await tradeAction(name === "confirm" ? 0 : 1).click();
  };
  const beginTrade = async () => {
    await focusWorld(); await nativeKey("Trade");
    await poll(() => peer.snapshot(), state => state.tradePendingInvite?.name === primaryIdentity.characterName, "UI_TRADE_INVITE_NOT_OBSERVED");
    const invite = peer.snapshot().tradePendingInvite;
    await peer.acceptOwnerGiftInvite(primaryIdentity.characterName, invite.id);
    await poll(primary, state => fresh(state) && state.trade.active && !state.trade.uncertain
      && state.trade.partnerName === peerIdentity.characterName && state.trade.ownOfferKnown, "TRADE_UI_NOT_CURRENT");
    await tradeSection().waitFor({ state: "visible" });
  };
  const makeOffer = async item => {
    const before = primary(); demand(before.gold >= 1 && ownedQuantity(before, item.uniqueId) === quantity(item), "TRADE_ASSETS_NOT_OWNED");
    const section = tradeSection(), nativeGold = page.locator("[data-r16-trade-gold]:visible");
    if (await nativeGold.count() === 1) {
      await nativeGold.click();
      await page.locator("[data-r16-trade-gold-input]").fill("1");
      await page.locator("[data-r16-trade-gold-submit]").click();
    } else {
      await section.locator('input[inputmode="numeric"]').fill("1"); await section.locator("form button[type=submit]").click();
    }
    await poll(primary, state => fresh(state) && state.trade.ownGold === 1 && state.gold === before.gold - 1, "TRADE_GOLD_CUSTODY_UNCONFIRMED");
    const picker = section.locator("button[title]").filter({ hasText: `${item.name} ×${quantity(item)}` });
    demand(await picker.count() === 1, "TRADE_ITEM_UI_TARGET_AMBIGUOUS");
    await picker.click();
    // A real double click tests stale selection/duplicate deposit protection.
    await section.locator('button[data-trade-slot="0"]').dblclick({ delay: 90 });
    await poll(() => peer.snapshot(), state => fresh(state) && state.trade.partnerGold === 1
      && state.trade.partnerItems.filter(entry => uid(entry.uniqueId) === uid(item.uniqueId)).reduce((sum, entry) => sum + quantity(entry), 0) === quantity(item),
    "EXACT_UID_TRADE_OFFER_UNCONFIRMED");
    await sleep(350);
    demand(primary().trade.ownGold === 1 && peer.snapshot().trade.partnerItems.filter(entry => uid(entry.uniqueId) === uid(item.uniqueId)).length === 1,
      "DUPLICATE_OFFER_CREATED_EXTRA_CUSTODY");
  };

  try {
    browser = await playwright.chromium.launch({ headless: options.headless === true,
      ...(options.browserExe || process.env.MIR2_BROWSER_EXE ? { executablePath: options.browserExe || process.env.MIR2_BROWSER_EXE } : {}) });
    context = await browser.newContext({ viewport: null, acceptDownloads: false });
    page = await context.newPage(); page.setDefaultTimeout(20_000);
    // Observe trusted DOM pointer edges on the public canvas only. This listener
    // reads no field values and never prevents, dispatches or changes an action.
    const observedGameKeys = ["Inventory", "Group", "Pickup"].map(functionName => ({ functionName,
      code: keyChord(uiLayout.keyboard, functionName).split("+").at(-1) }));
    summary.observedGameKeyBindings = observedGameKeys;
    await page.addInitScript(configuredKeys => {
      for (const type of ["pointerdown", "pointerup"]) document.addEventListener(type, event => {
        const target = event.target;
        if (!(target instanceof HTMLCanvasElement) || !["mir2-web3-canvas", "mir2-quest-ui-canvas"].includes(target.id)) return;
        console.debug("__MIR2_R16_POINTER_OBSERVATION__" + JSON.stringify({ edgeType: type, timeStamp: event.timeStamp,
          trusted: event.isTrusted, canvasId: target.id, x: event.clientX, y: event.clientY, button: event.button }));
      }, { capture: true, passive: true });
      for (const type of ["keydown", "keyup"]) window.addEventListener(type, event => {
        if (!document.querySelector('main[data-client-screen="game"]')
          || !["ArrowUp", "ArrowRight", "ArrowDown", "ArrowLeft"].includes(event.key)
          || event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return;
        console.debug("__MIR2_R16_CARDINAL_KEY_OBSERVATION__" + JSON.stringify({ edgeType: type,
          timeStamp: event.timeStamp, trusted: event.isTrusted, key: event.key, repeat: event.repeat }));
      }, { capture: true, passive: true });
      for (const type of ["keydown", "keyup"]) window.addEventListener(type, event => {
        if (!document.querySelector('main[data-client-screen="game"]')) return;
        const target = event.target;
        if (target instanceof HTMLElement && (target.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName))) return;
        const matches = configuredKeys.filter(value => value.code === event.code);
        if (!matches.length) return;
        const observed = { edgeType: type, timeStamp: event.timeStamp, trusted: event.isTrusted, code: event.code,
          functions: matches.map(value => value.functionName), repeat: event.repeat,
          modifiers: { control: event.ctrlKey, alt: event.altKey, shift: event.shiftKey, meta: event.metaKey },
          target: target instanceof Element ? { tag: target.tagName, id: target.id || null } : null };
        console.debug("__MIR2_R16_GAME_KEY_OBSERVATION__" + JSON.stringify({ ...observed, phase: "capture", defaultPrevented: event.defaultPrevented }));
        window.setTimeout(() => console.debug("__MIR2_R16_GAME_KEY_OBSERVATION__" + JSON.stringify({ ...observed,
          phase: "after-dispatch", observedAt: performance.now(), defaultPrevented: event.defaultPrevented })), 0);
      }, { capture: true, passive: true });
      // Observe only the public read-only status attributes. Capturing their
      // transitions preserves a brief fallback reason between pointer edges.
      new MutationObserver(records => {
        for (const record of records) {
          const target = record.target;
          if (!(target instanceof HTMLElement) || target.tagName !== "MAIN") continue;
          const text = target.getAttribute(record.attributeName);
          if (!text || text.length > 131072) continue;
          try { console.debug("__MIR2_R16_PUBLIC_UI_STATUS__" + JSON.stringify({ attribute: record.attributeName,
            timeStamp: performance.now(), status: JSON.parse(text) })); } catch { /* Diagnostic only. */ }
        }
      }).observe(document, { attributes: true, subtree: true,
        attributeFilter: ["data-bag-input-status", "data-npc-shop-input-status", "data-movement-input-status"] });
    }, observedGameKeys);
    page.on("pageerror", error => { fatalCode = "UNCAUGHT_BROWSER_EXCEPTION"; safety(fatalCode, { errorHash: digest(error?.stack ?? error?.message) }); });
    page.on("framenavigated", frame => {
      if (frame !== page.mainFrame()) return;
      try { const url = new URL(frame.url()); note("browser", "main-frame-navigation", { origin: url.origin, path: url.pathname }); }
      catch { /* Initial about:blank has no credential-bearing location. */ }
    });
    page.on("console", message => {
      if (message.type() === "debug" && message.text().startsWith("__MIR2_R16_POINTER_OBSERVATION__")) {
        try { note("browser", "physical-pointer-edge", JSON.parse(message.text().slice("__MIR2_R16_POINTER_OBSERVATION__".length))); } catch { /* Diagnostic only. */ }
      }
      if (message.type() === "debug" && message.text().startsWith("__MIR2_R16_PUBLIC_UI_STATUS__")) {
        try { note("browser", "public-ui-status-change", JSON.parse(message.text().slice("__MIR2_R16_PUBLIC_UI_STATUS__".length))); } catch { /* Diagnostic only. */ }
      }
      if (message.type() === "debug" && message.text().startsWith("__MIR2_R16_CARDINAL_KEY_OBSERVATION__")) {
        try { note("browser", "physical-cardinal-key-edge", JSON.parse(message.text().slice("__MIR2_R16_CARDINAL_KEY_OBSERVATION__".length))); } catch { /* Diagnostic only. */ }
      }
      if (message.type() === "debug" && message.text().startsWith("__MIR2_R16_GAME_KEY_OBSERVATION__")) {
        try { note("browser", "physical-game-key-edge", JSON.parse(message.text().slice("__MIR2_R16_GAME_KEY_OBSERVATION__".length))); } catch { /* Diagnostic only. */ }
      }
      if (message.type() === "error") {
        const location = message.location();
        safety("BROWSER_CONSOLE_ERROR", { messageHash: digest(message.text()), location: {
          ...safeResourceLocation(location.url, new URL(options.webUrl).origin), lineNumber: location.lineNumber, columnNumber: location.columnNumber } });
      }
    });
    page.on("response", response => {
      if (response.status() < 400) return;
      note("browser", "http-error-response", { status: response.status(), resourceType: response.request().resourceType(),
        location: safeResourceLocation(response.url(), new URL(options.webUrl).origin) });
    });
    page.on("websocket", socket => {
      try { const url = new URL(socket.url()); note("browser", "websocket-open", { origin: url.origin, path: url.pathname, expected: socket.url() === options.gatewayUrl }); }
      catch { safety("INVALID_WEBSOCKET_OBSERVED_URL"); }
      if (socket.url() !== options.gatewayUrl) return;
      activeSocket = socket; const owner = ++generation; primaryObservation.resetConnection();
      socket.on("framesent", frame => {
        if (activeSocket !== socket || owner !== generation) return;
        try { countCommand("browser", parseGatewayJson(String(frame.payload))); }
        catch { fatalCode = "UNPARSEABLE_BROWSER_COMMAND"; safety(fatalCode); }
      });
      socket.on("framereceived", frame => {
        if (activeSocket !== socket || owner !== generation) return;
        try {
          const event = parseGatewayJson(String(frame.payload));
          primaryObservation.apply(event); observe("browser", event, ++sequence);
        } catch { fatalCode = "UNPARSEABLE_GATEWAY_OBSERVATION"; safety(fatalCode); }
      });
      socket.on("close", () => { if (activeSocket === socket) { activeSocket = null; primaryObservation.resetConnection(); } });
    });
    phase(options.reusePrivateAccountState ? "ordinary-ui-reused-account-login-new-character-start" : "ordinary-ui-register-login-create-start");
    await page.goto(options.webUrl, { waitUntil: "domcontentloaded", timeout: 90_000 });
    await loginUi({ create: !options.reusePrivateAccountState });
    if (options.reusePrivateAccountState) check("ordinary-reused-test-account-login-through-ui", { newAccountRequests: summary.commandCounts.browser.newAccount ?? 0 });
    await selectUi({ create: true });
    demand(summary.observedProfile?.sharedParty === true, "SHARED_PARTY_NOT_ADVERTISED");
    demand(primary().inSafeZone, "ORDINARY_CHARACTER_DID_NOT_ENTER_TOWN_SAFETY");
    check("login-selection-startgame-town", { state: brief(primary()) }); await screenshot("ordinary-town-entry");

    phase("ordinary-owned-sword-ui");
    const sword = primary().inventoryItems.find(item => item.name === "WoodenSword");
    if (sword) {
      await openBag(); await bagDoubleClick(sword);
      await poll(primary, state => fresh(state) && state.equipmentItems.some(item => item.slot === "weapon" && item.name === sword.name), "OWNED_SWORD_UI_EQUIP_FAILED");
      await screenshot("owned-sword-equipped");
      if (options.stopAfterEquip) {
        demand(summary.commandCounts.browser.equipItem === 1, "FOCUSED_BAG_EQUIP_REQUEST_COUNT_NOT_ONE");
        summary.safeNativeInputState = await nativeInputSnapshot();
        summary.safeBagInputStatus = summary.safeNativeInputState.publicBag;
        check("focused-owned-sword-equip-diagnostic", { state: brief(primary()), publicBag: summary.safeBagInputStatus });
        if ((summary.safeBagInputStatus?.diagnostic?.transitions ?? []).length === 0) {
          summary.lastUiAction = "focused-equipped-bag-close-and-reopen";
          const firstOwner = summary.safeBagInputStatus.owner;
          await nativeKey("Inventory");
          await poll(sceneGeometry, geo => geo.bagOwner === "react", "FOCUSED_EQUIPPED_BAG_DID_NOT_CLOSE");
          await openBag();
          demand(summary.commandCounts.browser.equipItem === 1 && !primary().inventoryItems.some(item => uid(item.uniqueId) === uid(sword.uniqueId))
            && primary().equipmentItems.filter(item => item.slot === "weapon" && item.name === sword.name).length === 1,
          "FOCUSED_BAG_REOPEN_CHANGED_EQUIPMENT_OR_REPEATED_REQUEST");
          summary.safeNativeInputState = await nativeInputSnapshot();
          summary.safeBagInputStatus = summary.safeNativeInputState.publicBag;
          check("focused-native-bag-close-reopen-retains-exact-equipped-sword", { firstOwner, reopenedPublicBag: summary.safeBagInputStatus,
            equipRequests: summary.commandCounts.browser.equipItem });
          await screenshot("equipped-sword-retained-after-native-bag-reopen");
        } else note("runner", "focused-bag-reopen-skipped-after-recorded-fallback", { publicBag: summary.safeBagInputStatus });
        summary.diagnosticCompleted = true;
        phase("focused-bag-equip-diagnostic-complete");
        return summary;
      }
      await nativeKey("Inventory"); await sleep(250);
    }
    demand(primary().equipmentItems.some(item => item.slot === "weapon"), "NO_ORDINARY_WEAPON_FOR_COMBAT");
    demand(!primary().equipmentItems.some(item => item.slot === "armour"), "MEDICAL_ENCOUNTER_REQUIRES_UNARMOURED_STARTER");
    const ownedPotion = [...primary().beltItems, ...primary().inventoryItems].find(item => /(?:\(HP\)DrugSmall|HPPotion|HealthPotion|SunPotion)/iu.test(item.name));
    demand(ownedPotion && quantity(ownedPotion) > 0, "NO_OWNED_STARTER_HP_POTION");
    const originalPotionQuantity = ownedQuantity(primary(), ownedPotion.uniqueId);

    phase("ordinary-field-monster-combat-potion");
    await tryOneFreeMouseStep();
    await walk("browser", { x: 300, y: 614 }, 2);
    let monster, encounterStart = sequence;
    const explorationWaypoints = [{ x: 298, y: 626 }, { x: 298, y: 629 }, { x: 302, y: 629 }];
    let explorationWaypoint = 0, explorationSteps = 0;
    summary.fieldExplorationBudget = { waypoints: explorationWaypoints, maxWaypoints: 3, maxAcceptedKeySteps: 40 };
    for (let step = 0; step < 60; step++) {
      const state = primary(); demand(state.playerHp > 0, "PLAYER_DIED_DURING_FIELD_APPROACH");
      const observedMonsters = state.entities.filter(entity => entity.kind === "monster").map(entity => ({ objectId: entity.objectId,
        name: entity.name, ...point(entity), hp: entity.hp, dead: entity.dead }));
      monster = state.entities.filter(entity => entity.kind === "monster" && entity.name === "Scarecrow" && live(entity)
        && distance(entity, { x: 288, y: 616 }) > 10).sort((a, b) => distance(state.player, a) - distance(state.player, b))[0];
      note("runner", "ordinary-field-aoi-observation", { position: point(state.player), hp: state.playerHp, observedMonsters,
        selectedMonster: monster?.objectId ?? null, explorationWaypoint, explorationSteps });
      if (!monster) {
        while (explorationWaypoint < explorationWaypoints.length && distance(state.player, explorationWaypoints[explorationWaypoint]) <= 1) explorationWaypoint++;
        demand(explorationWaypoint < explorationWaypoints.length && explorationSteps < 40, "NO_REAL_FIELD_SCARECROW_AFTER_BOUNDED_UI_EXPLORATION");
        const occupied = state.entities.filter(entity => entity.objectId !== state.playerObjectId && live(entity)).map(point);
        const direction = nextCardinalWalkDirection({ player: state.player, target: explorationWaypoints[explorationWaypoint],
          desiredDistance: 1, collision, occupied });
        demand(direction, "NO_LEGAL_BOUNDED_UI_EXPLORATION_ROUTE");
        preferKeyboardNavigation = true;
        note("runner", "ordinary-field-exploration-key-step", { waypoint: explorationWaypoints[explorationWaypoint], direction,
          ordinal: explorationSteps + 1, position: point(state.player) });
        await uiCardinalStep(direction); explorationSteps++;
        continue;
      }
      if (!inBounds(state.player, uiLayout.homeSafeZone.bounds) && distance(state.player, monster) === 1) break;
      const occupied = state.entities.filter(entity => entity.objectId !== state.playerObjectId && live(entity)).map(point);
      const approach = fieldApproachStep({ player: state.player, monster, collision, occupied });
      demand(approach, "NO_LEGAL_FIELD_ENCOUNTER_APPROACH");
      if (preferKeyboardNavigation) {
        const direction = nextCardinalWalkDirection({ player: state.player, target: approach.tile, collision, occupied });
        demand(direction, "NO_LEGAL_CARDINAL_FIELD_ENCOUNTER_APPROACH"); await uiCardinalStep(direction);
      } else await uiStep(approach.direction);
    }
    demand(monster && !inBounds(primary().player, uiLayout.homeSafeZone.bounds) && distance(primary().player, monster) === 1, "FIELD_ENCOUNTER_NOT_REACHED");
    const monsterHpBefore = monster.hp;
    encounterStart = sequence;
    const captureDropEncounter = (target, after) => {
      const state = primary(), encounter = { after, monster: { ...target }, dropIdsBefore: new Set(state.groundDrops.map(value => uid(value.objectId))),
        ownObjectIds: [state.playerObjectId, state.selfZoneObjectId].filter(value => value != null),
        connectionEpoch: state.connectionEpoch, characterEpoch: state.characterEpoch, mapFileName: state.mapFileName };
      note("runner", "natural-drop-encounter-baseline", { ...encounter, dropIdsBefore: [...encounter.dropIdsBefore] });
      return encounter;
    };
    let dropEncounter = captureDropEncounter(monster, encounterStart);
    await clickWorldObject(`.entity-nameplate[data-object-id="${monster.objectId}"]`);
    let potionUsed = false, potionEvidence, monsterDamaged = false, dead = false;
    const battleDeadline = Date.now() + 120_000;
    while (Date.now() < battleDeadline) {
      healthy(); const state = primary(), current = state.entities.find(entity => entity.objectId === monster.objectId);
      demand(state.playerHp > 0, "PLAYER_DIED_DURING_REAL_COMBAT");
      monsterDamaged ||= current?.hp != null && monsterHpBefore != null && current.hp < monsterHpBefore
        || packetsAfter(encounterStart, "ObjectHealth", payload => payload.objectId === monster.objectId && payload.percent < 100).length > 0;
      dead ||= packetsAfter(encounterStart, "ObjectDied", payload => payload.objectId === monster.objectId).length > 0;
      if (!potionUsed && state.playerHp < state.playerMaxHp && state.playerHp > 0 && fresh(state)) {
        const before = brief(state), after = sequence;
        const beltItem = state.beltItems.find(item => uid(item.uniqueId) === uid(ownedPotion.uniqueId));
        if (beltItem) { await focusWorld(); await nativeKey(`Belt${beltItem.slot + 1}`); }
        else { await openBag(); const currentPotion = primary().inventoryItems.find(item => uid(item.uniqueId) === uid(ownedPotion.uniqueId));
          demand(currentPotion, "EXACT_OWNED_POTION_NO_LONGER_AVAILABLE"); await bagDoubleClick(currentPotion); }
        const consumed = await poll(primary, value => fresh(value) && ownedQuantity(value, ownedPotion.uniqueId) === originalPotionQuantity - 1
          && packetsAfter(after, "UseItem", payload => payload.success === true && uid(payload.uniqueId) === uid(ownedPotion.uniqueId)).length > 0,
        "OWNED_POTION_UI_USE_NOT_CONFIRMED");
        if (!beltItem) {
          summary.lastUiAction = "close-owned-potion-bag-and-wait-for-release";
          await nativeKey("Inventory");
          const closed = await poll(nativeInputSnapshot, async value => value.geometry.bagOwner === "react"
            && value.publicBag?.active === false && value.publicBag?.owner?.owner === "react"
            && await page.locator(".inventory-window:visible, .inventory-item-card:visible").count() === 0,
          "BAG_CLOSE_NOT_ACKNOWLEDGED", 4_000);
          note("runner", "owned-potion-bag-close-confirmed", { state: closed });
        }
        potionUsed = true; potionEvidence = { uniqueId: uid(ownedPotion.uniqueId), before, afterConsumption: brief(consumed), beforeHp: before.hp };
        await screenshot("real-owned-potion-consumed");
        if (live(primary().entities.find(entity => entity.objectId === monster.objectId))
          && !packetsAfter(encounterStart, "ObjectDied", payload => payload.objectId === monster.objectId).length)
          await clickWorldObject(`.entity-nameplate[data-object-id="${monster.objectId}"]`);
      }
      if (dead && potionUsed) break;
      await sleep(100);
    }
    demand(dead && monsterDamaged, "REAL_MONSTER_DAMAGE_AND_DEATH_NOT_CONFIRMED");
    demand(potionUsed, "NO_NATURAL_DAMAGE_AND_OWNED_POTION_USE");
    check("real-monster-combat-and-owned-potion", { monster: { objectId: monster.objectId, name: monster.name, hpBefore: monsterHpBefore }, potion: potionEvidence });
    await poll(primary, state => state.playerHp > potionEvidence.beforeHp, "NO_AUTHORITATIVE_POST_POTION_HP_RECOVERY", 30_000);
    check("server-reported-hp-recovery-after-potion", { hpBefore: potionEvidence.beforeHp, hpAfter: primary().playerHp });
    await screenshot("real-monster-death");

    phase("real-monster-drop-ui-pickup");
    preferKeyboardNavigation = true;
    const newlyDropped = () => naturalDropCandidates({ frames: received, state: primary(), encounter: dropEncounter })[0];
    await sleep(1500);
    note("runner", "natural-drop-first-death-observation", { monsterId: monster.objectId, matched: Boolean(newlyDropped()),
      deathReceipts: packetsAfter(encounterStart, "ObjectDied", payload => uid(payload.objectId) === uid(monster.objectId)),
      spawnReceipts: received.filter(frame => frame.source === "browser" && frame.sequence > encounterStart
        && ["ObjectGold", "ObjectItem"].includes(frame.packet)), groundDrops: primary().groundDrops.map(dropBrief) });
    const additionalKills = [];
    // Ordinary drop tables are probabilistic. A missing roll is neither a
    // gateway failure nor permission to fabricate a drop. Continue a bounded
    // real hunt after the proven medical encounter, preferring town targets.
    if (!newlyDropped()) await walk("browser", { x: 288, y: 616 }, 1);
    for (let attempt = 0; !newlyDropped() && attempt < options.maxDropKills; attempt++) {
      monster = primary().entities.filter(value => value.kind === "monster" && value.name === "Scarecrow" && live(value))
        .sort((a, b) => Number(!inBounds(a, uiLayout.homeSafeZone.bounds)) - Number(!inBounds(b, uiLayout.homeSafeZone.bounds))
          || distance(primary().player, a) - distance(primary().player, b))[0];
      demand(monster, "NO_OBSERVED_LIVE_MONSTER_FOR_BOUNDED_DROP_HUNT");
      const huntedId = monster.objectId;
      await walk("browser", value => value.entities.find(entity => entity.objectId === huntedId && live(entity)), 1, 36);
      if (inBounds(primary().player, uiLayout.homeSafeZone.bounds)) acceptedTownSafety("BOUNDED_DROP_HUNT_TOWN_SAFETY_NOT_CONFIRMED");
      else note("runner", "ordinary-drop-hunt-field-position", { position: point(primary().player), hp: primary().playerHp, huntedId });
      monster = primary().entities.find(value => value.objectId === huntedId);
      const after = sequence;
      dropEncounter = captureDropEncounter(monster, after);
      await clickWorldObject(`.entity-nameplate[data-object-id="${huntedId}"]`);
      await poll(primary, value => {
        demand(value.playerHp > 0, "PLAYER_DIED_DURING_BOUNDED_DROP_HUNT");
        return packetsAfter(after, "ObjectDied", payload => payload.objectId === huntedId).length > 0;
      }, "BOUNDED_REAL_MONSTER_KILL_NOT_CONFIRMED", 60_000);
      additionalKills.push({ objectId: huntedId, name: monster.name });
      await sleep(1000);
      note("runner", "ordinary-drop-roll", { monster: huntedId, actualDropObserved: Boolean(newlyDropped()), ordinal: attempt + 1 });
    }
    const dropMatch = newlyDropped();
    demand(dropMatch, "NO_NATURAL_DROP_WITHIN_BOUNDED_REAL_KILLS");
    const { drop } = dropMatch;
    note("runner", "natural-monster-drop-linked", { drop: dropBrief(drop), provenance: dropMatch.provenance,
      sameDeathCandidates: naturalDropCandidates({ frames: received, state: primary(), encounter: dropEncounter })
        .map(value => ({ drop: dropBrief(value.drop), provenance: value.provenance })) });
    const beforePickup = assetFingerprint(primary());
    await clickWorldObject(`.ground-drop-marker[data-object-id="${drop.objectId}"]`);
    await poll(primary, state => fresh(state) && !state.groundDrops.some(value => value.objectId === drop.objectId)
      && assetFingerprint(state) !== beforePickup, "REAL_MONSTER_DROP_PICKUP_NOT_CONFIRMED", 35_000);
    const additionalDrops = [];
    // A kill can legitimately leave several items on the same tile. Remove
    // each observed real drop once before testing an empty-tile pickup.
    for (let count = 0; count < 16; count++) {
      const remaining = primary().groundDrops.find(value => distance(value, primary().player) === 0);
      if (!remaining) break;
      demand(!additionalDrops.includes(remaining.objectId), "REMAINING_DROP_PICKUP_RESULT_UNKNOWN");
      additionalDrops.push(remaining.objectId);
      await clickWorldObject(`.ground-drop-marker[data-object-id="${remaining.objectId}"]`);
      await poll(primary, state => fresh(state) && !state.groundDrops.some(value => value.objectId === remaining.objectId), "ADDITIONAL_REAL_DROP_PICKUP_NOT_CONFIRMED");
    }
    demand(!primary().groundDrops.some(value => distance(value, primary().player) === 0), "PICKUP_TILE_NOT_EMPTY_FOR_REPEAT_CHECK");
    const afterPickup = assetFingerprint(primary());
    await focusWorld(); await nativeKey("Pickup"); await nativeKey("Pickup"); await sleep(1000);
    demand(fresh(primary()) && assetFingerprint(primary()) === afterPickup, "REPEATED_PICKUP_CHANGED_RETIRED_DROP_ASSETS");
    check("real-monster-drop-ui-pickup-and-repeat-stability", { drop: dropBrief(drop), provenance: dropMatch.provenance,
      additionalOrdinaryKillsForDrop: additionalKills, additionalRealDrops: additionalDrops, after: brief(primary()),
      duplicateScope: "two ordinary pickup keys after observed drops on the current tile were removed" });
    await screenshot("real-monster-drop-picked-up");
    if (options.stopAfterLoot) {
      demand(!summary.safetyEvents.some(event => ["BROWSER_CONSOLE_ERROR", "SCREENSHOT_CAPTURE_FAILED"].includes(event.code)), "BROWSER_OR_SCREENSHOT_ERROR_RECORDED");
      summary.diagnosticCompleted = true; phase("focused-natural-loot-diagnostic-complete"); return summary;
    }
    await walk("browser", { x: 288, y: 616 }, 1);
    acceptedTownSafety("PLAYER_DID_NOT_RETURN_TO_TOWN");
    if (primary().gold < 1) {
      phase("ordinary-ui-whitney-starter-dress-resale-after-combat-medicine-loot");
      await sellOwnDressUi();
      await walk("browser", { x: 288, y: 616 }, 1);
      acceptedTownSafety("PLAYER_DID_NOT_RETURN_TO_TOWN_AFTER_ORDINARY_RESALE");
    }

    phase("second-ordinary-player-and-party-ui");
    peer = new GatewayClient({ url: options.gatewayUrl, realmId: options.realmId, timeoutMs: 25_000 });
    peer.on("command", command => countCommand("peer", command));
    peer.on("frame", frame => observe("peer", frame.event, frame.sequence));
    const peerRealm = await peer.connect();
    demand(peerRealm.profileId === "platinum_fiveclass" && peerRealm.profileVersion === 1, "PEER_R16_PROFILE_MISMATCH");
    await peer.createAccount(peerIdentity.accountId, peerIdentity.password); await peer.login(peerIdentity.accountId, peerIdentity.password);
    const peerCharacter = await peer.createCharacter(peerIdentity.characterName); peerIdentity.characterIndex = peerCharacter.index;
    await peer.startGame(peerCharacter.index); await peer.setAllowGroup(true);
    await poll(primary, state => state.entities.some(entity => entity.kind === "player" && entity.name === peerIdentity.characterName), "TWO_REAL_CLIENTS_NOT_IN_SAME_ZONE");
    await focusWorld(); await nativeKey("Group");
    const group = page.locator("section[data-group-size]"); await group.waitFor({ state: "visible" });
    const nativeAdd = group.locator("[data-r16-group-add]");
    if (await nativeAdd.count() === 1) {
      await nativeAdd.click(); await page.locator("[data-r16-group-name]").fill(peerIdentity.characterName);
      await page.locator("[data-r16-group-name-submit]").click();
    } else {
      await group.locator("form input").fill(peerIdentity.characterName); await group.locator("form button[type=submit]").click();
    }
    await poll(() => peer.snapshot(), state => state.party.pendingInvite?.name === primaryIdentity.characterName, "UI_PARTY_INVITE_NOT_RECEIVED");
    demand(peer.snapshot().party.members.length === 0, "INVITATION_PREMATURELY_COUNTED_AS_PARTY");
    await peer.acceptGroupInvite();
    await poll(primary, state => state.party.members.length === 2 && state.party.members.includes(peerIdentity.characterName), "UI_PARTY_ROSTER_NOT_AUTHORITATIVE");
    await poll(() => peer.snapshot(), state => state.party.members.length === 2 && state.party.members.includes(primaryIdentity.characterName), "PEER_PARTY_ROSTER_NOT_AUTHORITATIVE");
    demand(await group.locator(`[data-member-name="${peerIdentity.characterName}"]`).count() === 1, "PEER_NOT_RENDERED_IN_UI_PARTY");
    check("two-ordinary-players-shared-party-through-ui", { browser: primary().party, peer: peer.snapshot().party, actualHumanOperators: 0 });
    await screenshot("real-shared-party"); await nativeKey("Group"); await sleep(250);

    phase("ordinary-facing-player-trade");
    // The primary's last real walk supplies its direction. Move the peer into
    // that observed front cell and turn the peer normally; no transform setter.
    await walk("peer", { x: primary().player.x + 3, y: primary().player.y }, 0);
    await walk("browser", { x: peer.snapshot().player.x - 1, y: peer.snapshot().player.y }, 0);
    const offsets = [[0, -1], [1, -1], [1, 0], [1, 1], [0, 1], [-1, 1], [-1, 0], [-1, -1]], facing = DIRECTIONS.indexOf(primary().player.direction);
    demand(facing >= 0, "PRIMARY_FACING_NOT_OBSERVED");
    await walk("peer", { x: primary().player.x + offsets[facing][0], y: primary().player.y + offsets[facing][1] }, 0);
    await peer.turn(DIRECTIONS[(facing + 4) % 8]);
    await poll(primary, state => state.entities.some(entity => entity.name === peerIdentity.characterName
      && distance(entity, state.player) === 1 && entity.direction === DIRECTIONS[(facing + 4) % 8]), "TRADE_MUTUAL_FACING_NOT_OBSERVED");
    // Starter Candle is intentional: exercise the normal shared-template and
    // potentially colliding legacy UID path rather than choosing around it.
    const transferable = primary().inventoryItems.find(item => item.name === "Candle" && quantity(item) === 1 && item.container === "bag1");
    demand(transferable && primary().gold > 0, "NO_ORDINARILY_OWNED_TRADE_ITEM_AND_GOLD");
    const beforePrimary = assetFingerprint(primary()), beforePeer = assetFingerprint(peer.snapshot());
    await beginTrade(); await makeOffer(transferable); await screenshot("real-trade-offer-before-cancel");
    await clickTradeAction("cancel");
    await poll(primary, state => fresh(state) && !state.trade.active && assetFingerprint(state) === beforePrimary, "CANCEL_DID_NOT_RESTORE_PRIMARY_ASSETS");
    await poll(() => peer.snapshot(), state => fresh(state) && !state.trade.active && assetFingerprint(state) === beforePeer, "CANCEL_CHANGED_PEER_ASSETS");
    check("ui-trade-cancel-returns-exact-gold-and-item", { item: itemBrief(transferable), gold: 1 }); await screenshot("real-trade-cancelled");

    phase("real-ui-trade-commit-gold-item-and-duplicate-protection");
    const commitBeforePrimary = primary(), commitBeforePeer = peer.snapshot();
    const peerUidQuantityBefore = ownedQuantity(commitBeforePeer, transferable.uniqueId);
    const peerTemplateBefore = templateQuantity(commitBeforePeer, transferable), primaryTemplateBefore = templateQuantity(commitBeforePrimary, transferable);
    await beginTrade(); await makeOffer(transferable);
    await clickTradeAction("confirm");
    await poll(primary, state => fresh(state) && state.trade.active && state.trade.locked && state.trade.escrowPrepared, "UI_TRADE_CONFIRM_NOT_PREPARED");
    const peerTrade = peer.snapshot().trade;
    await peer.confirmReceiveOnlyGift({ tradeId: peerTrade.id, ownerName: primaryIdentity.characterName });
    const settledPrimary = await poll(primary, state => fresh(state) && !state.trade.active && !state.trade.uncertain
      && state.gold === commitBeforePrimary.gold - 1 && ownedQuantity(state, transferable.uniqueId) === 0
      && templateQuantity(state, transferable) === primaryTemplateBefore - quantity(transferable), "PRIMARY_TRADE_SETTLEMENT_UNCONFIRMED");
    const settledPeer = await poll(() => peer.snapshot(), state => fresh(state) && !state.trade.active && !state.trade.uncertain
      && state.gold === commitBeforePeer.gold + 1 && templateQuantity(state, transferable) === peerTemplateBefore + quantity(transferable), "PEER_TRADE_SETTLEMENT_UNCONFIRMED");
    const peerIds = [...settledPeer.inventoryItems, ...settledPeer.beltItems].map(item => uid(item.uniqueId));
    demand(new Set(peerIds).size === peerIds.length,
      "DUPLICATE_UID_IN_PEER_AFTER_SETTLEMENT");
    demand(settledPrimary.trade.lastFinalConfirm?.tradeId && settledPeer.trade.lastFinalConfirm?.tradeId, "FINAL_TRADE_RECEIPTS_NOT_OBSERVED");
    demand(settledPrimary.gold + settledPeer.gold === commitBeforePrimary.gold + commitBeforePeer.gold, "TRADE_GOLD_NOT_CONSERVED");
    const stablePrimary = assetFingerprint(settledPrimary), stablePeer = assetFingerprint(settledPeer);
    await sleep(2000);
    demand(assetFingerprint(primary()) === stablePrimary && assetFingerprint(peer.snapshot()) === stablePeer, "SETTLED_TRADE_DUPLICATED_LATER");
    check("ui-trade-owned-candle-and-gold-delivery-once", { item: itemBrief(transferable), transferredGold: 1,
      browserBefore: brief(commitBeforePrimary), browserAfter: brief(settledPrimary), peerBefore: brief(commitBeforePeer), peerAfter: brief(settledPeer),
      recipientUidQuantityBefore: peerUidQuantityBefore, recipientUidQuantityAfter: ownedQuantity(settledPeer, transferable.uniqueId),
      recipientTemplateBefore: templateItems(commitBeforePeer, transferable).map(itemBrief), recipientTemplateAfter: templateItems(settledPeer, transferable).map(itemBrief),
      actualDeliveryItemReceipts: settledPeer.trade.deliveryItems,
      duplicateProtection: { realDoubleClickDeposit: true, settledBalancesStable: true, arbitraryPacketReplayTested: false } });
    await screenshot("real-trade-settled");

    phase("ordinary-save-relogin-persistence");
    await focusWorld(); const logoutAfter = sequence; await nativeKey("Logout");
    await poll(() => packetsAfter(logoutAfter, "LogOutSuccess"), frames => frames.length > 0, "UI_LOGOUT_SAVE_NOT_ACKNOWLEDGED");
    demand(await peer.logout(), "PEER_LOGOUT_SAVE_NOT_ACKNOWLEDGED"); await peer.close();
    await page.reload({ waitUntil: "domcontentloaded", timeout: 60_000 });
    await loginUi(); await selectUi();
    peer = new GatewayClient({ url: options.gatewayUrl, realmId: options.realmId, timeoutMs: 25_000 });
    peer.on("command", command => countCommand("peer", command)); peer.on("frame", frame => observe("peer", frame.event, frame.sequence));
    await peer.connect(); await peer.login(peerIdentity.accountId, peerIdentity.password); await peer.startGame(peerIdentity.characterIndex);
    await poll(primary, state => fresh(state) && assetFingerprint(state) === stablePrimary, "PRIMARY_RELOGIN_ASSETS_DIFFER");
    await poll(() => peer.snapshot(), state => fresh(state) && assetFingerprint(state) === stablePeer, "PEER_RELOGIN_ASSETS_DIFFER");
    check("ordinary-save-relogin-retains-exact-settled-assets", { browser: brief(primary()), peer: brief(peer.snapshot()) });
    await screenshot("saved-trade-assets-after-relogin");
    healthy();
    demand(summary.screenshots.length >= 8, "SCREENSHOT_EVIDENCE_INCOMPLETE");
    demand(!summary.safetyEvents.some(event => ["BROWSER_CONSOLE_ERROR", "SCREENSHOT_CAPTURE_FAILED"].includes(event.code)), "BROWSER_OR_SCREENSHOT_ERROR_RECORDED");
    summary.passed = true; phase("complete");
  } catch (error) {
    summary.failureCode = error.acceptanceCode ?? (error?.name === "TimeoutError" ? "PLAYWRIGHT_UI_TIMEOUT" : "ACCEPTANCE_OPERATION_FAILED");
    summary.failurePhase = summary.phase;
    // Playwright error text may contain a field's entered value. Retain a hash
    // only; never copy an exception message/stack into ordinary evidence.
    safety(summary.failureCode, { errorName: error?.name ?? "Error", detailHash: digest(error?.stack ?? error?.message) });
    if (page && !page.isClosed()) {
      const notices = await page.locator(".login-auth-notice, .login-feedback, .select-create-error").allTextContents().catch(() => []);
      const secrets = [primaryIdentity.password, primaryIdentity.accountId, peerIdentity.password, peerIdentity.accountId];
      summary.safeUiNotices = notices.map(value => secrets.reduce((text, secret) => text.replaceAll(secret, "[redacted]"), value).slice(0, 500));
      // Public DOM only. Read no input values, private runtime objects or auth data.
      summary.safeShopDomDiagnostics = await page.locator(".npc-shop-window").evaluateAll(elements => elements.map(element => {
        const bounds = element.getBoundingClientRect(), style = getComputedStyle(element);
        return { tab: element.getAttribute("data-shop-tab"), disabled: element.getAttribute("aria-disabled"), inert: element.hasAttribute("inert"),
          bounds: { x: bounds.x, y: bounds.y, width: bounds.width, height: bounds.height }, pointerEvents: style.pointerEvents,
          tabs: [...element.querySelectorAll("[data-shop-tab-key]")].map(button => ({ tab: button.getAttribute("data-shop-tab-key"),
            disabled: button.disabled === true, pressed: button.getAttribute("aria-pressed") })) };
      })).catch(() => []);
      summary.safeShopInputStatus = await page.locator("main[data-npc-shop-input-status]").getAttribute("data-npc-shop-input-status").catch(() => null);
      summary.safeNativeInputState = await nativeInputSnapshot().catch(() => null);
      summary.safeBagInputStatus = summary.safeNativeInputState?.publicBag ?? null;
      note("runner", "failure-public-ui-state", { bag: summary.safeBagInputStatus, shop: summary.safeNativeInputState?.publicShop ?? null });
    }
    await screenshot("failure");
  } finally {
    summary.finishedAt = new Date().toISOString();
    summary.finalBrowser = brief(primary()); summary.finalPeer = peer ? brief(peer.snapshot()) : null;
    summary.finalWorldSnapshotMetadata = lastFullSnapshot; summary.finalAcceptedUserLocation = lastUserLocation;
    // Disconnect only this runner's owned clients. Economic actions are never
    // retried, including unknown offers or confirmations, during cleanup.
    try { await peer?.close(); } catch { safety("OWNED_PEER_CLOSE_FAILED"); }
    try { await context?.close(); } catch { safety("OWNED_CONTEXT_CLOSE_FAILED"); }
    try { await browser?.close(); } catch { safety("OWNED_BROWSER_CLOSE_FAILED"); }
    await writeFile(resolve(options.outputDir, "events.json"), serializable({ schema: "mir2.r16.safe-ui-events.v1", events }), { flag: "wx" });
    await writeFile(resolve(options.outputDir, "outcome.json"), serializable(summary), { flag: "wx" });
  }
  return summary;
}

function parseArgs(args) {
  const options = { webUrl: process.env.MIR2_R16_WEB_URL, gatewayUrl: process.env.MIR2_R16_GATEWAY_WS,
    realmId: process.env.MIR2_R16_REALM_ID, outputDir: process.env.MIR2_R16_ACCEPTANCE_OUTPUT };
  const names = { "--web-url": "webUrl", "--gateway-url": "gatewayUrl", "--realm-id": "realmId", "--output-dir": "outputDir",
    "--playwright-module": "playwrightModule", "--browser-exe": "browserExe", "--config": "config", "--max-drop-kills": "maxDropKills",
    "--private-account-state": "privateAccountState", "--reuse-private-account-state": "reusePrivateAccountState" };
  for (let index = 0; index < args.length; index++) {
    const arg = args[index];
    if (arg === "--stop-after-equip") { options.stopAfterEquip = true; continue; }
    if (arg === "--stop-after-loot") { options.stopAfterLoot = true; continue; }
    if (arg === "--require-native-bag") { options.requireNativeBag = true; continue; }
    if (arg === "--help" || arg === "--preflight" || arg === "--headless") { options[arg.slice(2)] = true; continue; }
    demand(names[arg] && args[index + 1] && !args[index + 1].startsWith("--"), "INVALID_ARGUMENT_USE_HELP");
    options[names[arg]] = args[++index];
  }
  return options;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const options = parseArgs(process.argv.slice(2));
    if (options.help) {
      process.stdout.write("Real R16 Web UI acceptance; services must already be running in a private loopback world.\n"
        + "--config ABSOLUTE_PRIVATE_DRIVER_JSON --output-dir NEW_ABSOLUTE_EXTERNAL_DIR\n"
        + "or --web-url URL --gateway-url WS_URL --realm-id ai-... --output-dir NEW_ABSOLUTE_EXTERNAL_DIR\n"
        + "[--playwright-module EXISTING_PACKAGE_OR_ENTRY] [--browser-exe CHROMIUM_EXE] [--headless] [--max-drop-kills 1..64]\n"
        + "--stop-after-equip ends only the focused Bag diagnostic; it does not pass the complete journey.\n"
        + "--stop-after-loot ends after real combat, owned medicine and natural ground-drop pickup; it does not pass the complete journey.\n"
        + "--require-native-bag waits for real Native ownership and forbids the DOM Bag fallback throughout the journey.\n"
        + "--private-account-state ABS_PRIVATE_QA_LOCAL_JSON privately preserves only this run's ordinary test identities, using write-once storage outside reports.\n"
        + "--reuse-private-account-state ABS_PRIVATE_QA_LOCAL_JSON logs the saved ordinary wr16 primary in through the UI and creates a fresh nonzero-slot character; the old file/character is retained.\n"
        + "--preflight checks source layout, client collision and existing Playwright only; no service/browser/model/build starts.\n");
    } else if (options.preflight) {
      await sourceUiLayout(); await loadClientCollision("0"); await loadPlaywright(options.playwrightModule);
      process.stdout.write(serializable({ preflight: true, browserStarted: false, serviceStarted: false, modelCalls: 0 }) + "\n");
    } else {
      const result = await runR16WebAcceptance({ ...options, onProgress: value => process.stdout.write(JSON.stringify(value) + "\n") });
      process.stdout.write(JSON.stringify({ passed: result.passed, checks: result.checks.length, failureCode: result.failureCode,
        failurePhase: result.failurePhase, diagnosticCompleted: result.diagnosticCompleted, outputDir: resolve(options.outputDir) }) + "\n");
      if (!result.passed && !result.diagnosticCompleted) process.exitCode = 1;
    }
  } catch (error) {
    process.stderr.write(JSON.stringify({ passed: false, failureCode: error.acceptanceCode ?? "PREFLIGHT_OR_LAUNCH_FAILED" }) + "\n");
    process.exitCode = 1;
  }
}
