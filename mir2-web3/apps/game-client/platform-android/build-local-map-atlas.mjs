// Offline adapter around the existing Web shelf packer. Never writes to input
// roots, removes files, or claims that a starter tile set covers an entire map.
import fs from 'node:fs/promises';
import path from 'node:path';
import {pathToFileURL} from 'node:url';
import {createRequire} from 'node:module';
import {createHash} from 'node:crypto';
import {gunzipSync} from 'node:zlib';
const [webInput, outputInput] = process.argv.slice(2);
if (!webInput || !outputInput) {
  console.error('Usage: node build-local-map-atlas.mjs EXISTING_WEB_ROOT NEW_OUTPUT_PUBLIC_ROOT');
  process.exit(2);
}
const hash = data => createHash('sha256').update(data).digest('hex');
try {
  const webRoot = await fs.realpath(webInput);
  const sourceRoot = await fs.realpath(path.join(webRoot, 'public/original-map'));
  const parent = await fs.realpath(path.dirname(path.resolve(outputInput)));
  const output = path.join(parent, path.basename(outputInput));
  if (output === webRoot || output.startsWith(webRoot + path.sep)) throw new Error('Output must be outside the source checkout');
  // Fail before writing anything if the destination exists (including symlinks).
  try { await fs.lstat(output); throw new Error('Output already exists'); }
  catch (error) { if (error.code !== 'ENOENT') throw error; }
  const helper = path.join(webRoot, 'scripts/build-map-atlas-pack.mjs');
  const bichonMapSource = path.join(webRoot, 'lib/generated/crystal-map-pack/0.map.gz');
  const helperSha256 = hash(await fs.readFile(helper));
  const bichonMapCompressed = await fs.readFile(bichonMapSource);
  if (!bichonMapCompressed.length || bichonMapCompressed.length > 4 * 1024 * 1024) throw new Error('Invalid Bichon map layout');
  const bichonMapBytes = gunzipSync(bichonMapCompressed, {maxOutputLength:32 * 1024 * 1024});
  if (bichonMapBytes.length < 8 || bichonMapBytes[2] !== 0x43 || bichonMapBytes[3] !== 0x23) throw new Error('Bichon map is not Crystal type 100');
  const {packIntoPages, mapAtlasLibrarySupportsRawUpload, DEFAULT_MAX_PAGE_PIXELS} = await import(pathToFileURL(helper));
  const sharp = createRequire(helper)('sharp');
  const libraries = [];
  async function discover(dir, parts = []) {
    for (const entry of await fs.readdir(dir, {withFileTypes:true})) {
      if (!entry.isDirectory()) continue; // Do not follow directory symlinks.
      const next = [...parts, entry.name];
      const child = path.join(dir, entry.name);
      if (next.length === 2) {
        if (mapAtlasLibrarySupportsRawUpload(next.join('/'))) libraries.push({key:next.join('/'), dir:child});
      } else if (next.length < 2) await discover(child, next);
    }
  }
  await discover(sourceRoot);
  if (!libraries.length || libraries.length > 32) throw new Error('Invalid starter library count');
  await fs.mkdir(output); // Exclusive fresh output; never clean/reuse existing packs.
  const atlasRoot = path.join(output, 'generated/map-atlas');
  await fs.mkdir(atlasRoot, {recursive:true});
  const pages = [];
  let totalSources = 0;
  for (const lib of libraries.sort((a,b) => a.key.localeCompare(b.key))) {
    const files = (await fs.readdir(lib.dir, {withFileTypes:true})).filter(e => e.isFile() && /^\d+\.png$/.test(e.name));
    totalSources += files.length;
    if (totalSources > 10000) throw new Error('Starter source limit exceeded');
    const sources = [];
    for (const entry of files) {
      const filePath = path.join(lib.dir, entry.name);
      const stat = await fs.stat(filePath);
      if (stat.size > 16 * 1024 * 1024) throw new Error('Source image too large');
      const metadata = await sharp(filePath, {limitInputPixels:4096*4096}).metadata();
      sources.push({filePath,frame:entry.name.slice(0,-4),width:metadata.width,height:metadata.height});
    }
    const packed = packIntoPages(sources, DEFAULT_MAX_PAGE_PIXELS);
    if (pages.length + packed.length > 128) throw new Error('Starter page limit exceeded');
    for (const [index, page] of packed.entries()) {
      const bytes = await sharp({create:{width:page.width,height:page.height,channels:4,background:{r:0,g:0,b:0,alpha:0}}})
        .composite(page.sources.map(s => ({input:s.filePath,left:s.x,top:s.y})))
        .png({compressionLevel:9,adaptiveFiltering:true}).toBuffer();
      const relative = `${lib.key.replaceAll('/', '-')}/p${index}.${hash(bytes).slice(0,16)}.png`;
      await fs.mkdir(path.dirname(path.join(atlasRoot,relative)), {recursive:true});
      await fs.writeFile(path.join(atlasRoot,relative),bytes,{flag:'wx'});
      pages.push({l:lib.key,p:index,w:page.width,h:page.height,b:bytes.length,u:`/generated/map-atlas/${relative}`,
        r:page.sources.map(s => [Number(s.frame),s.x,s.y,s.width,s.height]).sort((a,b)=>a[0]-b[0])});
    }
  }
  const manifest = {schemaVersion:2,kind:'mir2-map-atlas-manifest',pages,
    stats:{libraryCount:libraries.length,atlasPageCount:pages.length,sourceCount:totalSources,
      imageBytes:pages.reduce((sum,p)=>sum+p.b,0),maxPageBytes:Math.max(...pages.map(p=>p.b)),maxPagePixels:DEFAULT_MAX_PAGE_PIXELS}};
  const json = JSON.stringify(manifest)+'\n';
  await fs.writeFile(path.join(atlasRoot,'manifest.json'),json,{flag:'wx'});
  const mapPackRoot = path.join(output, 'generated/crystal-map-pack');
  await fs.mkdir(mapPackRoot, {recursive:true});
  await fs.writeFile(path.join(mapPackRoot, '0.map'), bichonMapBytes, {flag:'wx'});
  const parsedMap = (() => {
    const width = bichonMapBytes.readUInt16LE(4);
    const height = bichonMapBytes.readUInt16LE(6);
    if (8 + width * height * 26 > bichonMapBytes.length) throw new Error('Incomplete Bichon cell data');
    const cells = [];
    for (let offset = 8; offset < 8 + width * height * 26; offset += 26) {
      cells.push({
        middleIndex:bichonMapBytes.readInt16LE(offset + 6), middleImage:bichonMapBytes.readInt16LE(offset + 8),
        frontIndex:bichonMapBytes.readInt16LE(offset + 10), frontImage:bichonMapBytes.readInt16LE(offset + 12),
        frontAnimationFrame:bichonMapBytes[offset + 16], middleAnimationFrame:bichonMapBytes[offset + 18],
      });
    }
    return {width,height,cells};
  })();
  const mapMir3LibraryKey = (index, base, root) => {
    const offset = index - base;
    if (offset < 0 || offset >= 75) return null;
    const state = Math.floor(offset / 15), slot = offset % 15;
    const names = ['Tilesc','Tiles30c','Tiles5c','SmTilesc','Housesc','Cliffsc','Dungeonsc','Innersc','Furnituresc','Wallsc','SmObjectsc','Animationsc','Object1c','Object2c'];
    const name = names[slot];
    if (!name) return null;
    if (root === 'WemadeMir3' && (name === 'Object1c' || name === 'Object2c')) return `${root}/${name}`;
    if (root === 'WemadeMir3') {
      const folder = ['', 'Wood', 'Sand', 'Snow', 'Forest'][state];
      return folder ? `${root}/${folder}/${name}` : `${root}/${name}`;
    }
    return `${root}/${name}${['','wood','sand','snow','forest'][state] ?? ''}`;
  };
  const libraryKey = index => mapMir3LibraryKey(index, 200, 'WemadeMir3') ?? mapMir3LibraryKey(index, 300, 'ShandaMir3') ?? ({
    0:'WemadeMir2/Tiles',1:'WemadeMir2/SmTiles',2:'WemadeMir2/Objects',90:'WemadeMir2/Objects_32bit',
    100:'ShandaMir2/Tiles',110:'ShandaMir2/SmTiles',120:'ShandaMir2/Objects',190:'ShandaMir2/AniTiles1',
  }[index] ?? (index >= 3 && index <= 29 ? `WemadeMir2/Objects${index - 1}`
    : index >= 101 && index <= 109 ? `ShandaMir2/Tiles${index - 99}`
    : index >= 111 && index <= 119 ? `ShandaMir2/SmTiles${index - 109}`
    : index >= 121 && index <= 150 ? `ShandaMir2/Objects${index - 119}` : 'WemadeMir2/Tiles'));
  const standaloneLibrary = library => /\/(?:objects(?:_32bit|\d*)?|smobjects\d*|furnitures?c?|walls?c?|animations?c?|houses?c?|cliffs?c?|dungeons?c?|inners?c?|object[12]c)$/i.test(`/${library}`);
  const references = new Map();
  const addFamily = (library, frame, count, additive) => {
    for (let phase = 0; phase < Math.max(1, count); phase += 1) {
      if (frame + phase < 0 || (!additive && !standaloneLibrary(library))) continue;
      const key = `${library}#${frame + phase}`;
      const old = references.get(key);
      if (!old || (additive && !old.additive)) references.set(key, {key,library,frame:frame + phase,additive});
    }
  };
  for (const cell of parsedMap.cells) {
    const middleCount = cell.middleAnimationFrame > 0 && cell.middleAnimationFrame < 255 ? cell.middleAnimationFrame & 15 : 0;
    const middleAdditive = middleCount === 8 || middleCount === 10 || (cell.middleAnimationFrame & 128) !== 0;
    if (cell.middleIndex >= 0 && cell.middleImage > 0) addFamily(libraryKey(cell.middleIndex), cell.middleImage - 1, middleCount, middleAdditive);
    const frontCount = cell.frontAnimationFrame > 0 ? cell.frontAnimationFrame & 127 : 0;
    const frontFrame = (cell.frontImage & 0x7fff) - 1;
    if (cell.frontIndex >= 0 && frontFrame >= 0) addFamily(libraryKey(cell.frontIndex), frontFrame, frontCount, (cell.frontAnimationFrame & 128) !== 0);
  }
  const keyedRoot = path.join(output, 'generated/native-map-keyed');
  const keyedPages = path.join(keyedRoot, 'pages');
  await fs.mkdir(keyedPages, {recursive:true});
  const keyedEntries = [];
  const emittedHashes = new Set();
  let keyedMissing = 0, keyedBytes = 0, additiveEntries = 0;
  for (const reference of [...references.values()].sort((a,b) => a.key.localeCompare(b.key))) {
    const source = path.join(sourceRoot, ...reference.library.split('/'), `${reference.frame}.png`);
    let bytes;
    try { bytes = await fs.readFile(source); } catch (error) {
      if (error.code === 'ENOENT') { keyedMissing += 1; continue; }
      throw error;
    }
    if (!bytes.length || bytes.length > 16 * 1024 * 1024) throw new Error(`Invalid standalone source ${reference.key}`);
    const metadata = await sharp(bytes, {limitInputPixels:4096*4096}).metadata();
    if (!metadata.width || !metadata.height) throw new Error(`Missing standalone dimensions ${reference.key}`);
    const digest = hash(bytes), fileName = `${digest}.png`;
    if (!emittedHashes.has(digest)) {
      await fs.writeFile(path.join(keyedPages, fileName), bytes, {flag:'wx'});
      emittedHashes.add(digest);
      keyedBytes += bytes.length;
    }
    if (reference.additive) additiveEntries += 1;
    keyedEntries.push({key:reference.key,imageUrl:`/generated/native-map-keyed/pages/${fileName}`,width:metadata.width,height:metadata.height});
  }
  const keyedManifest = {schemaVersion:1,kind:'mir2-native-map-keyed-manifest',mapFileName:'0',mapFileNames:['0'],entries:keyedEntries,
    stats:{mapCount:1,referenceCount:references.size,emittedEntryCount:keyedEntries.length,keyedEntryCount:keyedEntries.length-additiveEntries,
      additiveEntryCount:additiveEntries,fullPackEntryCount:0,noDrawReferenceCount:0,missingSourceCount:keyedMissing,removedArtifacts:0,imageBytes:keyedBytes}};
  const keyedJson = JSON.stringify(keyedManifest)+'\n';
  await fs.writeFile(path.join(keyedRoot,'manifest.json'),keyedJson,{flag:'wx'});
  const report = {output,sourceRoot,helperSha256,manifestSha256:hash(json),...manifest.stats,
    bichonMapCompressedBytes:bichonMapCompressed.length,bichonMapCompressedSha256:hash(bichonMapCompressed),
    bichonMapBytes:bichonMapBytes.length,bichonMapSha256:hash(bichonMapBytes),
    nativeKeyedManifestSha256:hash(keyedJson),nativeKeyedReferenceCount:references.size,
    nativeKeyedEntryCount:keyedEntries.length,nativeKeyedMissingSourceCount:keyedMissing,nativeKeyedImageBytes:keyedBytes,
    scope:'Local exported raw-upload tile libraries, Bichon 0.map, and locally available immutable standalone object frames; Android rendering unverified'};
  await fs.writeFile(path.join(output,'build-report.json'),JSON.stringify(report,null,2)+'\n',{flag:'wx'});
  console.log(JSON.stringify(report,null,2));
} catch (error) {
  console.error(`Local map atlas build failed: ${error.message}`);
  process.exitCode = 1;
}
