// Read-only terrain coverage for the exact maps named by a staged native pack.
// This distinguishes original Crystal no-draw slots from missing drawable PNGs.
import { readFile, writeFile, copyFile, mkdir, lstat } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import path from 'node:path';
import os from 'node:os';
import { fileURLToPath } from 'node:url';
import { gunzipSync } from 'node:zlib';
import { createRequire } from 'node:module';
import { parsePackagedMap, mapLibraryKeyForIndex, decodeCrystalMiddleAnimationCount, decodeCrystalFrontAnimationCount, crystalMiddleMapBlendMode, crystalFrontMapBlendMode, mapAtlasPathRequiresAlphaKey, originalMapFramePath } from '../../../web/scripts/build-native-keyed-map-pack.mjs';
import { loadCrystalQuestRouteSources, buildMapTravelGraph, findMapTravelRoute } from '../../../web/scripts/quest-agent/route-manifest.mjs';
import { packIntoPages, renderMapAtlasPage, DEFAULT_MAX_PAGE_PIXELS } from '../../../web/scripts/build-map-atlas-pack.mjs';

const sharp = createRequire(new URL('../../../web/package.json', import.meta.url))('sharp');

const project = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../..');
const args = process.argv.slice(2);
const option = name => args.includes(name) ? args[args.indexOf(name) + 1] : undefined;
const manifestPath = option('--manifest');
if (!manifestPath) throw new Error('Provide --manifest <native-map-keyed/manifest.json> and optional --output <audit.json>');
const json = async file => JSON.parse(await readFile(file, 'utf8'));
const publicRoot = path.join(project, 'apps/web/public');
const manifest = await json(manifestPath);
const mapNames = manifest.mapFileNames.map(name => String(name).toLowerCase());
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
const graph = buildMapTravelGraph(sources);
const journey = await json(path.join(project, 'config/quest-guidance/newcomer-journey-v2.json'));
const supplies = await json(path.join(project, 'config/quest-guidance/newcomer-supplies.json'));
const graduation = await json(path.join(project, 'config/quest-guidance/newcomer-v2-graduation.json'));
const destinations = new Set(['0', ...journey.quests.flatMap(quest => quest.maps), ...Object.values(supplies.merchants).map(merchant => merchant.mapFileName), graduation.challenge.mapFileName]);
const requiredMaps = new Set([...destinations].map(name => name.toLowerCase()));
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
  mapNames, requiredMaps: [...requiredMaps].sort(), omittedMaps, routes,
  referenceCount: references.size, packagedReferenceCount: references.size - missing.length,
  missingByReason: byReason, blockingReferenceCount: blocking.length,
  manifestSha256: createHash('sha256').update(await readFile(manifestPath)).digest('hex'),
  mapAtlasManifestSha256: createHash('sha256').update(await readFile(path.join(atlasRoot, 'manifest.json'))).digest('hex'),
  fullIndexSha256: createHash('sha256').update(fullIndexBytes).digest('hex'),
  completedAtlas,
  passed: omittedMaps.length === 0 && (blocking.length === 0 || completedAtlas !== null),
  missing,
};
if (option('--output')) await writeFile(option('--output'), `${JSON.stringify(report, null, 2)}\n`);
console.log(JSON.stringify({ ...report, routes: undefined, missing: undefined }, null, 2));
if (!report.passed) process.exitCode = 1;
