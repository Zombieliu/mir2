// Read-only terrain coverage for the exact maps named by a staged native pack.
// This distinguishes original Crystal no-draw slots from missing drawable PNGs.
import { readFile, writeFile, copyFile, mkdir, lstat } from 'node:fs/promises';
import { readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import path from 'node:path';
import os from 'node:os';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { gunzipSync } from 'node:zlib';
import { createRequire } from 'node:module';
import { loadCrystalQuestRouteSources, buildMapTravelGraph, findMapTravelRoute } from '../../../web/scripts/quest-agent/route-manifest.mjs';

const project = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../..');
const requiredJourneyMaps = ['0', '1', '2', '3', '0108', '0109', '0141', 'd001', 'd021', 'd022', 'd401', 'd601', 'd602', 'd605', 'd607'];
const previousMaps = [...requiredJourneyMaps, '0101', '0102', '0103', '0104', '0105', '0106', '0107', '0125', '0132', '0140', '0122', '0150', '0151', '0152', '0153', '0154', '0155'];
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const topologyPaths = ['packages/game-data/data/generated/crystal_respawn_manifest.json',
  'packages/game-data/data/generated/crystal_npc_manifest.json', 'packages/game-data/data/generated/crystal_map_event_manifest.json'];

export function candidateResourceOnlyNames(profile) {
  const names = profile.nativeResourceOnlyMaps === undefined ? [] : profile.nativeResourceOnlyMaps;
  if (!Array.isArray(names) || names.some(name => name !== 'D71653') || new Set(names).size !== names.length) throw new Error('Only the canonical named original resource room D71653 may be declared');
  if (names.length && profile.profileId !== 'platinum_176') throw new Error('Resource-only room requires platinum_176');
  const runtime = new Set(profile.mapWhitelist.map(map => map.fileName.toLowerCase()));
  if (names.some(name => runtime.has(name.toLowerCase()))) throw new Error('Resource-only room must not be runtime-admitted');
  return names.map(name => name.toLowerCase());
}

export function proveCandidateResourceOnlyRooms(profile, sources) {
  const names = candidateResourceOnlyNames(profile);
  const world = sources.respawnManifest.maps;
  const npcText = JSON.stringify(sources.npcScriptManifest).toLowerCase();
  const eventText = JSON.stringify(sources.mapEventManifest ?? JSON.parse(readFileSync(path.join(project, topologyPaths[2])))).toLowerCase();
  return names.map(name => {
    const matches = world.filter(map => map.map_file_name === 'D71653');
    const room = matches[0], move = room?.movements?.[0];
    if (matches.length !== 1 || room.map_index !== 235 || room.map_title !== 'TacticalMaze' || room.mini_map !== 0 || room.big_map !== 0 ||
        room.respawn_count !== 0 || room.respawns.length !== 0 || room.safe_zones.length !== 0 || room.movement_count !== 1 || room.movements.length !== 1 ||
        move.map_index !== 209 || move.source.x !== 17 || move.source.y !== 12 || move.destination.x !== 36 || move.destination.y !== 34 ||
        move.need_hole !== false || move.need_move !== false || move.conquest_index !== 0 || move.show_on_big_map !== false || move.icon !== 0 ||
        world.some(map => map.movements?.some(edge => edge.map_index === 235)) || npcText.includes(name) || eventText.includes(name)) {
      throw new Error('D71653 no longer matches its original no-entry source proof');
    }
    return { map: name, mapIndex: 235, reason: 'source-has-no-imported-entry', runtimeAdmitted: false,
      outgoingMapIndices: [209], source: { x: 17, y: 12 }, destination: { x: 36, y: 34 } };
  });
}

function candidateTopologyEvidence(profile) {
  const bytes = topologyPaths.map(file => readFileSync(path.join(project, file)));
  const sources = { respawnManifest: JSON.parse(bytes[0]), npcScriptManifest: JSON.parse(bytes[1]), mapEventManifest: JSON.parse(bytes[2]) };
  proveCandidateResourceOnlyRooms(profile, sources);
  const files = topologyPaths.map((file, i) => ({ path: file, sha256: digest(bytes[i]) }));
  return { files, sha256: digest(files.map(file => `${file.path}\0${file.sha256}\n`).join('')) };
}

export function normalizeCandidateMapNames(values) {
  if (!Array.isArray(values) || values.length === 0 || values.some(name => typeof name !== 'string' || !/^[A-Za-z0-9_-]+$/.test(name))) throw new Error('Provide nonempty safe map identities');
  return [...new Set(values.map(name => name.toLowerCase()))].sort();
}

export function resolveCandidateMapScope({ profile, profileBytes, mode = 'profile', expectedMaps } = {}) {
  if (profile?.profileId !== 'platinum_176' || !Number.isSafeInteger(profile.version) || profile.version < 1 || !Array.isArray(profile.mapWhitelist)) throw new Error('Map scope requires the identified platinum_176 content profile');
  if ((!Buffer.isBuffer(profileBytes) && typeof profileBytes !== 'string') || JSON.stringify(JSON.parse(profileBytes.toString())) !== JSON.stringify(profile)) throw new Error('Map scope profile bytes do not match its identity');
  const names = normalizeCandidateMapNames(profile.mapWhitelist.map(map => map.fileName));
  if (names.length !== profile.mapWhitelist.length) throw new Error('Content profile has duplicate map identities');
  const resourceNames = candidateResourceOnlyNames(profile);
  const evidence = candidateTopologyEvidence(profile);
  const nativeNames = normalizeCandidateMapNames([...names, ...resourceNames]);
  for (const name of previousMaps) if (!names.includes(name)) throw new Error(`Content profile omitted a previously shipped map: ${name}`);
  if (!['profile', 'explicit'].includes(mode)) throw new Error('Map scope must be profile or explicit');
  let selected = nativeNames;
  if (mode === 'explicit') selected = normalizeCandidateMapNames(expectedMaps);
  else if (expectedMaps && normalizeCandidateMapNames(expectedMaps).join('\n') !== nativeNames.join('\n')) throw new Error('Full-profile expected maps differ from the content profile');
  const selectedResourceNames = resourceNames.filter(name => selected.includes(name));
  for (const name of requiredJourneyMaps) if (!selected.includes(name)) throw new Error(`Map scope omitted a required journey map: ${name}`);
  return { schema: 'mir2.windows.candidate-map-scope.v1', mode, profileId: profile.profileId, profileVersion: profile.version,
    profileSha256: digest(profileBytes), profileMapCount: names.length, expectedMaps: selected,
    expectedMapsSha256: digest(`${selected.join('\n')}\n`), resourceOnlyMaps: selectedResourceNames,
    resourceOnlyMapsSha256: digest(`${selectedResourceNames.join('\n')}\n`), profileNativeMapCount: nativeNames.length,
    sourceTopology: evidence.files, sourceTopologySha256: evidence.sha256, completeClassicWorld: false };
}

export function compareCandidateMapScope(scope, manifestMapNames) {
  const names = normalizeCandidateMapNames(manifestMapNames);
  if (names.length !== manifestMapNames.length) throw new Error('Native manifest has duplicate map identities');
  return { mapNames: names, omittedMaps: scope.expectedMaps.filter(name => !names.includes(name)), unexpectedMaps: names.filter(name => !scope.expectedMaps.includes(name)) };
}

export function describeCandidateMapTopology(sources, expectedMaps) {
  const names = normalizeCandidateMapNames(expectedMaps);
  const world = sources.respawnManifest.maps;
  const resourceOnlyRooms = proveCandidateResourceOnlyRooms(sources.contentProfile, sources).filter(room => names.includes(room.map));
  const canonical = new Map(world.filter(map => map.map_file_name).map(map => [map.map_file_name.toLowerCase(), map.map_file_name]));
  // Hypothetical selected topology only; this never changes the active profile.
  const graph = buildMapTravelGraph({ ...sources, contentProfile: { ...sources.contentProfile, mapWhitelist: names.map(fileName => ({ fileName })) } });
  const ordinary = { ...graph, edges: graph.edges.filter(edge => edge.kind === 'map-movement' && !edge.needHole && !edge.needMove) };
  const unreachableMaps = [], conditionalEntries = [];
  for (const name of names) {
    if (name === '0') continue;
    const destination = canonical.get(name);
    const route = destination && findMapTravelRoute(graph, '0', destination);
    if (!route) { unreachableMaps.push(name); continue; }
    if (!findMapTravelRoute(ordinary, '0', destination)) conditionalEntries.push({ map: name,
      conditions: route.filter(edge => edge.kind === 'npc-script' || edge.needHole || edge.needMove).map(edge => ({
        from: edge.fromMapFileName, to: edge.toMapFileName, kind: edge.kind, needHole: edge.needHole, needMove: edge.needMove,
        ...(edge.kind === 'npc-script' ? { scriptKey: edge.scriptKey, targetSequence: edge.targetSequence, requiredItems: edge.requiredItems,
          itemCosts: edge.itemCosts, goldCost: edge.goldCost, minimumGoldExclusive: edge.minimumGoldExclusive } : { portals: edge.portals }),
      })) });
  }
  const profileNames = new Set(sources.contentProfile.mapWhitelist.map(map => map.fileName.toLowerCase()));
  const scriptText = JSON.stringify(sources.npcScriptManifest).toLowerCase();
  const selectedClassic = name => /^(?:D71[0-7]|D716(?:0[1-9]|1[0-9]|2[0-5]|5[0-3])|0157|D50[1-5]|D506[1-9]|D507[1-4]|D51[1-5]|D100(?:1[1-3]|2|3[12]|4|5[1-4]|6[12]))$/i.test(name);
  const excludedClassicMaps = world.filter(map => selectedClassic(map.map_file_name) && !profileNames.has(map.map_file_name.toLowerCase())).map(map => map.map_file_name.toLowerCase()).sort();
  const sourceOrphanRooms = world.filter(map => selectedClassic(map.map_file_name) && map.map_file_name.toLowerCase() !== '0' &&
    !world.some(from => from.movements?.some(move => move.map_index === map.map_index)) &&
    !scriptText.includes(map.map_file_name.toLowerCase())).map(map => ({
      map: map.map_file_name.toLowerCase(), includedInExpectedScope: names.includes(map.map_file_name.toLowerCase()),
      reason: 'source-has-no-imported-entry', outgoingMapIndices: (map.movements ?? []).map(move => move.map_index),
    }));
  const resourceOnlyNames = resourceOnlyRooms.map(room => room.map);
  const unreachableRuntimeMaps = unreachableMaps.filter(name => !resourceOnlyNames.includes(name));
  return { schema: 'mir2.windows.candidate-map-topology.v1', expectedMapCount: names.length, unreachableMaps, unreachableRuntimeMaps,
    conditionalEntries, excludedClassicMaps, sourceOrphanRooms, resourceOnlyRooms, runtimeGameplayAccepted: false };
}

export async function auditCandidateMapCoverage(args = process.argv.slice(2)) {
const option = name => args.includes(name) ? args[args.indexOf(name) + 1] : undefined;
const manifestPath = option('--manifest');
if (!manifestPath) throw new Error('Provide --manifest <native-map-keyed/manifest.json> and optional --output <audit.json>');
const json = async file => JSON.parse(await readFile(file, 'utf8'));
const publicRoot = path.join(project, 'apps/web/public');
const manifest = await json(manifestPath);
const profileBytes = await readFile(path.join(project, 'packages/game-data/data/content_profiles/platinum_176.json'));
const scope = resolveCandidateMapScope({ profile: JSON.parse(profileBytes), profileBytes,
  mode: option('--scope') ?? 'profile', expectedMaps: option('--expectedMaps')?.split(',') });
const selection = compareCandidateMapScope(scope, manifest.mapFileNames);
const { mapNames } = selection;
if (selection.omittedMaps.length || selection.unexpectedMaps.length) {
  const rejected = { schema: 'mir2.windows.candidate-map-coverage.v1', scope, ...selection, passed: false,
    rejection: 'Manifest maps differ from the explicit expected scope; no asset completion was attempted' };
  if (option('--output')) await writeFile(option('--output'), `${JSON.stringify(rejected, null, 2)}\n`);
  console.log(JSON.stringify(rejected, null, 2));
  process.exitCode = 1;
  return rejected;
}
const { parsePackagedMap, mapLibraryKeyForIndex, decodeCrystalMiddleAnimationCount, decodeCrystalFrontAnimationCount,
  crystalMiddleMapBlendMode, crystalFrontMapBlendMode, mapAtlasPathRequiresAlphaKey, originalMapFramePath } = await import('../../../web/scripts/build-native-keyed-map-pack.mjs');
const { packIntoPages, renderMapAtlasPage, DEFAULT_MAX_PAGE_PIXELS } = await import('../../../web/scripts/build-map-atlas-pack.mjs');
const sharp = createRequire(new URL('../../../web/package.json', import.meta.url))('sharp');
const suppliedStandalone = new Set(manifest.entries.map(entry => entry.key));
const suppliedAtlas = new Set();
const atlasRoot = path.resolve(option('--atlasRoot') ?? path.join(publicRoot, 'generated/map-atlas'));
const atlas = await json(path.join(atlasRoot, 'manifest.json'));
if (atlas.schemaVersion !== 2 || atlas.edgeExtrusion !== 1) throw new Error('Candidate map atlas must retain owned one-pixel gutters');
const relativeAtlasPage = url => {
  if (!/^\/generated\/map-atlas\/[A-Za-z0-9_-]+\/p[0-9]+\.[0-9a-f]{16}\.png$/.test(url)) throw new Error('Unsafe candidate atlas page URL');
  return url.slice('/generated/map-atlas/'.length);
};
for (const page of atlas.pages) {
  const bytes = await readFile(path.join(atlasRoot, relativeAtlasPage(page.u)));
  const dimensions = await sharp(bytes).metadata();
  if (bytes.length !== page.b || dimensions.width !== page.w || dimensions.height !== page.h || !page.u.endsWith(`.${createHash('sha256').update(bytes).digest('hex').slice(0, 16)}.png`)) throw new Error(`Candidate atlas page identity mismatch: ${page.u}`);
}
for (const page of atlas.pages) for (const rect of page.r) suppliedAtlas.add(`${page.l}#${rect[0]}`);

const sources = await loadCrystalQuestRouteSources();
const scopeRoutes = describeCandidateMapTopology(sources, scope.expectedMaps);
const graph = buildMapTravelGraph(sources);
const journey = await json(path.join(project, 'config/quest-guidance/newcomer-journey-v2.json'));
const supplies = await json(path.join(project, 'config/quest-guidance/newcomer-supplies.json'));
const graduation = await json(path.join(project, 'config/quest-guidance/newcomer-v2-graduation.json'));
const destinations = new Set(['0', ...journey.quests.flatMap(quest => quest.maps), ...Object.values(supplies.merchants).map(merchant => merchant.mapFileName), graduation.challenge.mapFileName]);
const requiredMaps = new Set([...scope.expectedMaps, ...destinations].map(name => name.toLowerCase()));
const routes = [];
for (const destination of destinations) {
  if (destination === '0') continue;
  for (const [from, to] of [['0', destination], [destination, '0']]) {
    const route = findMapTravelRoute(graph, from, to);
    if (!route?.length) throw new Error(`No ordinary journey/supply/challenge map route ${from} -> ${to}`);
    routes.push({ from, to, maps: [from, ...route.map(edge => edge.toMapFileName)] });
    for (const edge of route) {
      requiredMaps.add(edge.fromMapFileName.toLowerCase());
      requiredMaps.add(edge.toMapFileName.toLowerCase());
    }
  }
}
const omittedMaps = [...requiredMaps].filter(name => !mapNames.includes(name));
const references = new Map();
for (const mapName of mapNames) {
  if (!/^[a-z0-9_-]+$/.test(mapName)) throw new Error('Unsafe map identity');
  const bytes = gunzipSync(await readFile(path.join(project, 'apps/web/lib/generated/crystal-map-pack', `${mapName}.map.gz`)));
  const data = parsePackagedMap(bytes);
  if (!data) throw new Error(`Cannot parse ${mapName}`);
  const add = (index, frame, count, layer, additive = false) => {
    if (index < 0 || frame < 0) return;
    const library = mapLibraryKeyForIndex(index);
    for (let phase = 0; phase < Math.max(count, 1); phase++) {
      const key = `${library}#${frame + phase}`;
      if (!references.has(key)) references.set(key, { key, library, frame: frame + phase, maps: new Set(), layers: new Set(), representations: new Set() });
      references.get(key).maps.add(mapName);
      references.get(key).layers.add(layer);
      references.get(key).representations.add(additive || mapAtlasPathRequiresAlphaKey(originalMapFramePath(library, frame + phase)) ? 'standalone' : 'atlas');
    }
  };
  for (const [offset, cell] of data.cells.entries()) {
    const x = Math.floor(offset / data.height);
    const y = offset % data.height;
    // Native/Crystal draw their 2x2 floor only at even/even cells.
    if (x % 2 === 0 && y % 2 === 0) add(cell.backIndex, cell.backImage === 0 ? -1 : (cell.backImage & 0x1fffffff) - 1, 1, 'back');
    add(cell.middleIndex, cell.middleImage - 1, decodeCrystalMiddleAnimationCount(cell.middleAnimationFrame), 'middle', crystalMiddleMapBlendMode(cell.middleAnimationFrame) === 'additive');
    add(cell.frontIndex, (cell.frontImage & 0x7fff) - 1, decodeCrystalFrontAnimationCount(cell.frontAnimationFrame), 'front', crystalFrontMapBlendMode(cell.frontAnimationFrame) === 'additive');
    // Type100 also has a fourth animation layer. Its stride is not the
    // consecutive middle/front stride used by the shared legacy parser.
    if (bytes[2] === 0x43 && bytes[3] === 0x23) {
      const rawOffset = 8 + offset * 26;
      const first = bytes.readInt16LE(rawOffset + 20) - 1;
      const stride = bytes.readInt16LE(rawOffset + 22) ^ 0x2000;
      const count = bytes[rawOffset + 24];
      if (first >= 0) for (let phase = 0; phase < count; phase++) add(190, first + phase * stride, 1, 'tile-animation');
    }
  }
}
const fullRoot = path.resolve(option('--fullPackRoot') ?? path.join(publicRoot, 'generated/crystal-packs/full'));
const fullIndexBytes = await readFile(path.join(fullRoot, 'index.json'));
const full = JSON.parse(fullIndexBytes);
const libraries = new Map(full.libraries.map(library => [library.libraryKey, library]));
const loaded = new Map();
const missing = [];
for (const reference of references.values()) {
  const missingRepresentations = [...reference.representations].filter(kind => !(kind === 'standalone' ? suppliedStandalone : suppliedAtlas).has(reference.key));
  if (!missingRepresentations.length) continue;
  const descriptor = libraries.get(`Map/${reference.library}`);
  let reason = 'source-library-unavailable';
  if (descriptor) {
    if (!loaded.has(descriptor.libraryKey)) {
      const relative = descriptor.manifestUrl.replace(/^\/generated\/crystal-packs\/full\//, '');
      if (relative.startsWith('/') || relative.includes('..')) throw new Error('Unsafe full library manifest path');
      const bytes = await readFile(path.join(fullRoot, relative));
      if (createHash('sha256').update(bytes).digest('hex') !== descriptor.manifestSha256) throw new Error(`Full source manifest hash mismatch: ${descriptor.libraryKey}`);
      loaded.set(descriptor.libraryKey, JSON.parse(bytes));
    }
    const library = loaded.get(descriptor.libraryKey);
    const sourceFrame = library.frames[reference.frame];
    if (reference.frame >= library.frameSlotCount) reason = 'source-out-of-range';
    else if (sourceFrame?.index === reference.frame && (sourceFrame.noDraw || sourceFrame.width <= 0 || sourceFrame.height <= 0)) reason = 'source-no-draw';
    else reason = 'drawable-source-not-packaged';
  }
  missing.push({ ...reference, maps: [...reference.maps], layers: [...reference.layers], representations: [...reference.representations], missingRepresentations, reason });
}
const byReason = {};
for (const reference of missing) byReason[reference.reason] = (byReason[reference.reason] ?? 0) + 1;
const blocking = missing.filter(reference => !['source-no-draw', 'source-out-of-range'].includes(reference.reason));
let completedAtlas = null;
if (option('--completeAtlasRoot')) {
  // Completion creates a new tree; it never replaces a development cache.
  const destination = path.resolve(option('--completeAtlasRoot'));
  const relativeTemporary = path.relative(os.tmpdir(), destination);
  if (path.isAbsolute(relativeTemporary) || relativeTemporary.startsWith('..') || !/^native-(?:map-keyed|keyed-map)-[A-Za-z0-9-]+(?:[\\/]|$)/.test(relativeTemporary)) throw new Error('Atlas completion must use a dedicated native map temporary directory');
  for (let ancestor = path.dirname(destination); ancestor !== path.dirname(ancestor); ancestor = path.dirname(ancestor)) {
    const info = await lstat(ancestor).catch(error => { if (error.code === 'ENOENT') return null; throw error; });
    if (info?.isSymbolicLink()) throw new Error('Atlas completion may not traverse a reparse ancestor');
  }
  if (await lstat(destination).then(() => true, error => { if (error.code === 'ENOENT') return false; throw error; })) throw new Error('Atlas completion destination must not exist');
  if (blocking.some(reference => reference.reason !== 'drawable-source-not-packaged' || reference.missingRepresentations.some(kind => kind !== 'atlas'))) throw new Error('Atlas completion can repair only verified ordinary floor frames');
  await mkdir(destination, { recursive: true });
  for (const page of atlas.pages) {
    const relative = relativeAtlasPage(page.u);
    await mkdir(path.dirname(path.join(destination, relative)), { recursive: true });
    await copyFile(path.join(atlasRoot, relative), path.join(destination, relative));
  }
  const sourcePages = new Map();
  const additions = new Map();
  for (const reference of blocking) {
    const library = loaded.get(`Map/${reference.library}`);
    const frame = library.frames[reference.frame];
    const image = frame.image;
    if (frame.status !== 'packed' || frame.index !== reference.frame || !image || !Number.isInteger(image.width) || !Number.isInteger(image.height)) throw new Error('Incomplete drawable source frame');
    if (!/^\/generated\/crystal-packs\/full\/pages\/[0-9a-f]{2}\/[0-9a-f]{64}\.png$/.test(image.imageUrl)) throw new Error('Unsafe full-pack source page');
    if (!sourcePages.has(image.imageUrl)) {
      const bytes = await readFile(path.join(fullRoot, image.imageUrl.slice('/generated/crystal-packs/full/'.length)));
      const digest = createHash('sha256').update(bytes).digest('hex');
      if (image.pageKey !== `sha256:${digest}` || !image.imageUrl.endsWith(`/${digest}.png`)) throw new Error('Full-pack source image hash mismatch');
      sourcePages.set(image.imageUrl, bytes);
    }
    const encoded = await sharp(sourcePages.get(image.imageUrl)).extract({ left: image.x, top: image.y, width: image.width, height: image.height }).png().toBuffer();
    const group = additions.get(reference.library) ?? [];
    // The existing tested atlas renderer accepts Sharp's Buffer input too.
    group.push({ filePath: encoded, frame: String(reference.frame), width: image.width, height: image.height });
    additions.set(reference.library, group);
  }
  const newPages = [];
  for (const [library, frames] of additions) {
    let pageIndex = Math.max(-1, ...atlas.pages.filter(page => page.l === library).map(page => page.p)) + 1;
    const slug = library.replaceAll('/', '-');
    for (const page of packIntoPages(frames, DEFAULT_MAX_PAGE_PIXELS)) {
      const encoded = await renderMapAtlasPage(page);
      const digest = createHash('sha256').update(encoded).digest('hex');
      const relative = `${slug}/p${pageIndex}.${digest.slice(0, 16)}.png`;
      await mkdir(path.join(destination, slug), { recursive: true });
      await writeFile(path.join(destination, relative), encoded);
      newPages.push({ l: library, p: pageIndex++, w: page.width, h: page.height, b: encoded.length, u: `/generated/map-atlas/${relative}`, r: page.sources.map(source => [Number(source.frame), source.x, source.y, source.width, source.height]).sort((a, b) => a[0] - b[0]) });
    }
  }
  const pages = [...atlas.pages, ...newPages].sort((a, b) => a.l.localeCompare(b.l) || a.p - b.p);
  const completed = { ...atlas, pages, stats: { libraryCount: new Set(pages.map(page => page.l)).size, atlasPageCount: pages.length, sourceCount: pages.reduce((count, page) => count + page.r.length, 0), imageBytes: pages.reduce((count, page) => count + page.b, 0), maxPageBytes: Math.max(...pages.map(page => page.b)), maxPagePixels: DEFAULT_MAX_PAGE_PIXELS } };
  const text = `${JSON.stringify(completed)}\n`;
  const digest = createHash('sha256').update(text).digest('hex');
  await writeFile(path.join(destination, 'manifest.json'), text);
  await writeFile(path.join(destination, `manifest.${digest}.json`), text);
  completedAtlas = { addedFrames: blocking.length, addedPages: newPages.length, manifestSha256: digest, stats: completed.stats };
}
const report = {
  schema: 'mir2.windows.candidate-map-coverage.v1',
  scope, scopeRoutes, unexpectedMaps: selection.unexpectedMaps,
  mapNames, requiredMaps: [...requiredMaps].sort(), omittedMaps, routes,
  referenceCount: references.size, packagedReferenceCount: references.size - missing.length,
  missingByReason: byReason, blockingReferenceCount: blocking.length,
  manifestSha256: createHash('sha256').update(await readFile(manifestPath)).digest('hex'),
  mapAtlasManifestSha256: createHash('sha256').update(await readFile(path.join(atlasRoot, 'manifest.json'))).digest('hex'),
  fullIndexSha256: createHash('sha256').update(fullIndexBytes).digest('hex'),
  completedAtlas,
  passed: omittedMaps.length === 0 && (scope.mode !== 'profile' || scopeRoutes.unreachableRuntimeMaps.length === 0) && (blocking.length === 0 || completedAtlas !== null),
  missing,
};
if (option('--output')) await writeFile(option('--output'), `${JSON.stringify(report, null, 2)}\n`);
console.log(JSON.stringify({ ...report, routes: undefined, missing: undefined }, null, 2));
if (!report.passed) process.exitCode = 1;
return report;
}

if (process.argv[1] && pathToFileURL(path.resolve(process.argv[1])).href === import.meta.url) await auditCandidateMapCoverage();
