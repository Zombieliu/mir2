// Offline adapter around the existing Web shelf packer. Never writes to input
// roots, removes files, or claims that a starter tile set covers an entire map.
import fs from 'node:fs/promises';
import path from 'node:path';
import {pathToFileURL} from 'node:url';
import {createRequire} from 'node:module';
import {createHash} from 'node:crypto';
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
  const helperSha256 = hash(await fs.readFile(helper));
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
  const report = {output,sourceRoot,helperSha256,manifestSha256:hash(json),...manifest.stats,
    scope:'Local exported raw-upload tile libraries only; keyed objects, full Bichon coverage and Android rendering unverified'};
  await fs.writeFile(path.join(output,'build-report.json'),JSON.stringify(report,null,2)+'\n',{flag:'wx'});
  console.log(JSON.stringify(report,null,2));
} catch (error) {
  console.error(`Local map atlas build failed: ${error.message}`);
  process.exitCode = 1;
}
