// Verify the current audit report against actual atlas/keyed routing and PNGs.
// Run audit-map0-floor-coverage first; this does not replace a source-Lib audit.
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import sharp from 'sharp';
import {mapAtlasLibrarySupportsRawUpload} from './build-map-atlas-pack.mjs';
import {mapAtlasPathRequiresAlphaKey} from './build-native-keyed-map-pack.mjs';

export function validateFloorReport(report) {
  if (report?.map !== '0' || !Array.isArray(report.raw) || !Array.isArray(report.standalone)
      || !Array.isArray(report.missingLibraries) || report.missingLibraries.length) {
    throw Error('Invalid or incomplete map 0 audit report');
  }
  const seen = new Set();
  for (const kind of ['raw', 'standalone']) for (const lib of report[kind]) {
    if (!/^[A-Za-z0-9_]+(?:\/[A-Za-z0-9_]+)+$/.test(lib.library ?? '')
        || !Array.isArray(lib.valid) || !lib.valid.length
        || !Array.isArray(lib.missing) || lib.missing.length
        || !Array.isArray(lib.atlasMissing) || lib.atlasMissing.length) {
      throw Error('Invalid or incomplete library audit');
    }
    const raw = mapAtlasLibrarySupportsRawUpload(lib.library);
    if ((kind === 'raw') !== raw || (!raw && !mapAtlasPathRequiresAlphaKey(`/original-map/${lib.library}/1.png`))) {
      throw Error(`Unsupported or incorrect render route: ${lib.library}`);
    }
    for (const frame of lib.valid) {
      const key = `${lib.library}#${frame}`;
      if (!Number.isSafeInteger(frame) || frame < 0 || seen.has(key)) throw Error(`Invalid/duplicate report frame: ${key}`);
      seen.add(key);
    }
  }
  if (!seen.size) throw Error('Empty floor audit report');
}

export function uniqueIndex(entries, keyOf) {
  const result = new Map();
  for (const entry of entries) {
    const key = keyOf(entry);
    if (typeof key !== 'string' || result.has(key)) throw Error(`Duplicate/invalid manifest key: ${key}`);
    result.set(key, entry);
  }
  return result;
}

export async function verifyFloorCoverage(root, report) {
  validateFloorReport(report);
  const read = relative => JSON.parse(fs.readFileSync(path.join(root, relative)));
  const atlas = read('generated/map-atlas/manifest.json');
  const keyed = read('generated/native-map-keyed/manifest.json');
  const rects = uniqueIndex(atlas.pages.flatMap(p => p.r.map(r => ({p,r,key:`${p.l}#${r[0]}`}))), e => e.key);
  const keys = uniqueIndex(keyed.entries, e => e.key);
  const cache = new Map(), errors = [];
  const decode = async relative => sharp(path.join(root, relative)).ensureAlpha().raw().toBuffer({resolveWithObject:true});
  let rawChecked = 0, standaloneChecked = 0;
  for (const lib of report.raw) for (const f of lib.valid) {
    const key = `${lib.library}#${f}`, item = rects.get(key);
    if (!item) { errors.push({key,error:'missing rect'}); continue; }
    const {p,r} = item;
    if (!cache.has(p.u)) cache.set(p.u, await decode(p.u));
    const page = cache.get(p.u), source = await decode(`original-map/${lib.library}/${f}.png`);
    const validRect = r.length === 5 && r.every(Number.isSafeInteger) && r[1] >= 0 && r[2] >= 0
      && r[3] > 0 && r[4] > 0 && r[1]+r[3] <= p.w && r[2]+r[4] <= p.h;
    if (!validRect || page.info.width !== p.w || page.info.height !== p.h
        || source.info.width !== r[3] || source.info.height !== r[4]) {
      errors.push({key,error:'dimensions/rect bounds'}); continue;
    }
    let same = true;
    for (let y=0;y<r[4];y++) {
      const offset=((r[2]+y)*p.w+r[1])*4;
      if (!page.data.subarray(offset,offset+r[3]*4).equals(source.data.subarray(y*r[3]*4,(y+1)*r[3]*4))) same=false;
    }
    if (!same) errors.push({key,error:'RGBA mismatch'});
    rawChecked++;
  }
  for (const lib of report.standalone) for (const f of lib.valid) {
    const key=`${lib.library}#${f}`, e=keys.get(key);
    if (!e) { errors.push({key,error:'missing standalone'}); continue; }
    const source=await decode(`original-map/${lib.library}/${f}.png`), image=await decode(e.imageUrl);
    if (source.info.width !== e.width || source.info.height !== e.height
        || image.info.width !== e.width || image.info.height !== e.height) errors.push({key,error:'standalone dimensions'});
    if (!source.data.equals(image.data)) errors.push({key,error:'standalone RGBA mismatch'});
    standaloneChecked++;
  }
  return {rawChecked,standaloneChecked,validFloorReferences:rawChecked+standaloneChecked,noDraw:report.noDraw,errors};
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const reportPath=process.argv[2];
  if (!reportPath) throw Error('Specify JSON from audit-map0-floor-coverage --report');
  const root=fileURLToPath(new URL('../public/',import.meta.url));
  const result=await verifyFloorCoverage(root,JSON.parse(fs.readFileSync(reportPath)));
  console.log(JSON.stringify(result,null,2));
  if(result.errors.length) process.exitCode=1;
}
