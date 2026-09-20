import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { parseLibrary } from '../apps/web/scripts/crystal-library.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const read = p => JSON.parse(fs.readFileSync(path.join(root, p), 'utf8'));
const exists = p => fs.existsSync(path.join(root, p));
const args = process.argv.slice(2);
const option = (name, fallback) => args.includes(name) ? args[args.indexOf(name) + 1] : fallback;
const sourceRoot = option('--sourceRoot', args[0]?.startsWith('--') ? null : args[0]);
const out = path.resolve(root, option('--outDir', 'docs/generated/player-qa/profile-assets-repair-20260921'));
const monsters = read('packages/game-data/data/generated/crystal_monster_manifest.json').monsters;
const maps = read('packages/game-data/data/generated/crystal_respawn_manifest.json').maps;
const profile = read('packages/game-data/data/content_profiles/platinum_176.json');
const atlas = read('apps/web/public/bevy-entity-atlases/manifest.json');
const frameSets = read('apps/web/public/original-ui/frame-sets.generated.json').libraries;
const runtimeLibrary = image => image >= 950 && image <= 953 ? `Gate/${String(image - 950).padStart(2, '0')}` : `Monster/${String(image).padStart(3, '0')}`;
const standalone = library => /^Monster\/\d{3}$/.test(library) || /^Gate\/0[0-3]$/.test(library) || /^Pet\/(0\d|1[0-4])$/.test(library);
function crystalLibrary(image) {
  if (image === 900 || image === 902) return 'Dragon';
  if (image === 901) return null;
  if (image >= 903 && image <= 905) return 'Monster/247';
  if (image >= 940 && image <= 944) return `Siege/${String(image - 940).padStart(2, '0')}`;
  if (image >= 950 && image <= 964) return `Gate/${String(image - 950).padStart(2, '0')}`;
  if (image >= 10000 && image <= 10014) return `Pet/${String(image - 10000).padStart(2, '0')}`;
  return runtimeLibrary(image);
}
const atlasPaths = new Set(), pages = [];
for (const a of atlas.atlases) {
  for (const p of a.pages ?? [a]) pages.push({ url: p.imageUrl, exists: exists(`apps/web/public${p.imageUrl}`) });
  for (const r of a.rects) {
    const page = (a.pages ?? [a])[r.pageIndex ?? 0];
    if (page && exists(`apps/web/public${page.imageUrl}`) && r.width > 0 && r.height > 0) atlasPaths.add(r.key.split('|')[0]);
  }
}
const whitelist = new Set(profile.monsterWhitelist), newcomerNames = new Set(['BoneFamiliar']);
function collectNewcomer(value) {
  if (!value || typeof value !== 'object') return;
  if (typeof value.monster === 'string') newcomerNames.add(value.monster);
  for (const child of Object.values(value)) collectNewcomer(child);
}
collectNewcomer(read('config/quest-guidance/newcomer-journey-v2.json'));
const allowedMaps = new Set(profile.mapWhitelist.map(x => x.fileName)), references = new Map();
for (const map of maps) for (const r of map.respawns) {
  if (!references.has(r.monster_index)) references.set(r.monster_index, []);
  references.get(r.monster_index).push({ map: map.map_file_name, profile: allowedMaps.has(map.map_file_name) && whitelist.has(r.monster_name) });
}
const libraries = [];
for (const image of [...new Set(monsters.map(m => m.image))].sort((a, b) => a - b)) {
  const library = runtimeLibrary(image), base = `apps/web/public/original-ui/${library}`;
  const meta = exists(`${base}/meta.json`) ? read(`${base}/meta.json`) : null;
  const metadata = new Map((meta?.frames ?? []).map(f => [f.index, f]));
  const sourcePath = sourceRoot ? path.join(sourceRoot, `${library}.Lib`) : null;
  let source = null, sourceError = null;
  if (sourcePath && fs.existsSync(sourcePath)) {
    try { source = parseLibrary(fs.readFileSync(sourcePath)); } catch (error) { sourceError = error.message; }
  }
  const actions = frameSets[library]?.actions ?? meta?.frameSet?.actions ?? [];
  const missingPublic = new Set(), missingNative = new Set(), sourceEmpty = new Set(), outOfRange = new Set(), unknown = new Set();
  const checked = new Map();
  function frameStatus(index) {
    if (checked.has(index)) return checked.get(index);
    const s = source?.frames[index];
    const status = { source: !source ? 'unknown' : !Number.isInteger(index) || index < 0 || index >= source.count ? 'out-of-range' : !s || s.width <= 0 || s.height <= 0 ? 'empty' : 'drawable' };
    if (status.source === 'empty') sourceEmpty.add(index);
    if (status.source === 'out-of-range') outOfRange.add(index);
    if (status.source === 'unknown') unknown.add(index);
    const f = metadata.get(index), expected = `/original-ui/${library}/${index}.png`;
    const geometryValid = f && f.path === expected && Number.isInteger(f.x) && Number.isInteger(f.y) && Number.isInteger(f.width) && Number.isInteger(f.height) && f.width > 0 && f.height > 0;
    const sourceGeometryMatches = !s || ['width', 'height', 'x', 'y'].every(key => f?.[key] === s[key]);
    let pngValid = false;
    if (geometryValid && exists(`apps/web/public${expected}`)) {
      const bytes = fs.readFileSync(path.join(root, `apps/web/public${expected}`));
      pngValid = bytes.length >= 33 && bytes.subarray(0, 8).equals(Buffer.from([137,80,78,71,13,10,26,10])) && bytes.toString('ascii', 12, 16) === 'IHDR' && bytes.readUInt32BE(16) === f.width && bytes.readUInt32BE(20) === f.height && (!f.pngSha256 || createHash('sha256').update(bytes).digest('hex') === f.pngSha256);
    }
    status.public = Boolean(geometryValid && sourceGeometryMatches && pngValid);
    status.native = atlasPaths.has(expected) ? 'atlas' : standalone(library) && status.public ? 'standalone' : 'missing';
    if (status.source === 'drawable') {
      if (!status.public) missingPublic.add(index);
      if (status.native === 'missing') missingNative.add(index);
    }
    checked.set(index, status);
    return status;
  }
  const actionAudit = actions.map(a => ({ action: a.actionName, start: a.start, count: a.count, skip: a.skip, directions: Array.from({ length: 8 }, (_, direction) => {
    const counts = { direction, drawable: 0, publicMissing: 0, nativeMissing: 0, atlas: 0, standalone: 0, sourceEmpty: 0, outOfRange: 0, unknown: 0 };
    for (let phase = 0; phase < a.count; phase++) {
      const status = frameStatus(a.start + direction * (a.count + a.skip) + phase);
      if (status.source === 'drawable') {
        counts.drawable++;
        if (!status.public) counts.publicMissing++;
        if (status.native === 'missing') counts.nativeMissing++; else counts[status.native]++;
      } else counts[{ empty: 'sourceEmpty', 'out-of-range': 'outOfRange', unknown: 'unknown' }[status.source]]++;
    }
    return counts;
  }) }));
  const sorted = set => [...set].sort((a, b) => a - b);
  libraries.push({ library, image, crystalLibrary: crystalLibrary(image), mappingPending: library !== crystalLibrary(image), sourceExists: sourcePath ? fs.existsSync(sourcePath) : null, sourceError, sourceCount: source?.count ?? null, metaExists: !!meta, metaFrames: metadata.size, actionCount: actions.length, sourceAndGeneratedActionsMatch: source ? JSON.stringify(source.frameSet.actions) === JSON.stringify(actions) : null, atlasFrames: [...atlasPaths].filter(p => p.startsWith(`/original-ui/${library}/`)).length, standaloneSupported: standalone(library), sourceDrawableSlots: source?.frames.filter(f => f && f.width > 0 && f.height > 0).length ?? null, sourceEmptySlots: source?.frames.reduce((n, f) => n + (!f || f.width <= 0 || f.height <= 0 ? 1 : 0), 0) ?? null, missingPublicFrames: sorted(missingPublic), missingNativeFrames: sorted(missingNative), sourceEmptyActionFrames: sorted(sourceEmpty), outOfRangeActionFrames: sorted(outOfRange), unknownActionFrames: sorted(unknown), actions: actionAudit });
}
const byImage = new Map(libraries.map(x => [x.image, x]));
const rows = monsters.map(m => {
  const l = byImage.get(m.image), refs = references.get(m.monster_index) ?? [];
  const understood = l.actionCount > 0 && l.sourceExists && !l.sourceError && !l.mappingPending && !l.unknownActionFrames.length && l.sourceAndGeneratedActionsMatch;
  return { index: m.monster_index, name: m.name, level: m.level, image: m.image, bodyLibrary: l.library, crystalLibrary: l.crystalLibrary, mappingPending: l.mappingPending, respawnRecords: refs.length, respawnMaps: [...new Set(refs.map(x => x.map))], profileWhitelisted: whitelist.has(m.name), profileRespawnRecords: refs.filter(x => x.profile).length, profileOverrideRecords: profile.respawnOverrides.filter(x => x.monster === m.name).length, bichon: refs.some(x => x.map === '0'), publicLibrary: l.metaExists, publicDrawableAnimationComplete: Boolean(understood && !l.missingPublicFrames.length), nativeDrawableAnimationComplete: Boolean(understood && !l.missingNativeFrames.length), descriptorOutOfRangeFrames: l.outOfRangeActionFrames.length, sourceEmptyActionFrames: l.sourceEmptyActionFrames.length, unknownActionFrames: l.unknownActionFrames.length, actionsUnknown: !understood, nativeAtlasFrames: l.atlasFrames };
});
const summarize = rs => ({ monsters: rs.length, libraries: new Set(rs.map(x => x.bodyLibrary)).size, missingPublicLibrary: rs.filter(x => !x.publicLibrary).length, incompletePublicDrawableAnimation: rs.filter(x => !x.publicDrawableAnimationComplete).length, incompleteNativeDrawableAnimation: rs.filter(x => !x.nativeDrawableAnimationComplete).length, unknownAnimationOrMapping: rs.filter(x => x.actionsUnknown).length, monstersWithSourceEmptyActions: rs.filter(x => x.sourceEmptyActionFrames).length, monstersWithDescriptorOutOfRange: rs.filter(x => x.descriptorOutOfRangeFrames).length });
const summary = { definitions: summarize(rows), respawnReferenced: summarize(rows.filter(x => x.respawnRecords)), profileWhitelist: summarize(rows.filter(x => x.profileWhitelisted)), profileSpawned: summarize(rows.filter(x => x.profileRespawnRecords || x.profileOverrideRecords)), newcomerV2TargetsAndBoneFamiliar: summarize(rows.filter(x => newcomerNames.has(x.name))), bichon: summarize(rows.filter(x => x.bichon)), profileBichon: summarize(rows.filter(x => x.bichon && x.profileWhitelisted)), atlasPages: pages.length, missingAtlasPages: pages.filter(p => !p.exists).length };
const report = { generatedAt: new Date().toISOString(), scope: 'Static workspace reachability of every imported action and all eight directions; drawable-frame closure is separate from source-empty slots and source-descriptor out-of-range references. No package, live rendering or visual acceptance claim.', sourceRoot, animationFormula: 'start + direction * (count + skip) + phase; reversal visits the same indices', nativeConstraint: 'Atlas first; otherwise canonical Monster/NNN, Gate/00..03, Pet/00..14 standalone requires valid PNG signature/IHDR, matching metadata path/geometry, source geometry and recorded PNG SHA256 when present. PNG decoding/pixel equivalence is covered separately by the profile exporter verifier.', sourceMappingCaveat: 'Runtime now maps profile images 950..953 to Gate/00..03. Non-profile pending: Crystal maps 900 and 902 to Dragon (902 has direction-specific statue frames), 903..905 to Monster/247 with special HellBomb frames (904 is not Dragon), 901 has no independent body, siege 940..944 and gates 954..964 use their own libraries, and 10000..10014 use Pet/00..14. Unknown/mismatched paths remain pending and do not establish missing original artwork.', summary, newcomerV2TargetNames: [...newcomerNames], missingConfiguredNames: [...new Set([...whitelist, ...newcomerNames])].filter(name => !monsters.some(m => m.name === name)), pages, monsters: rows, libraries };
fs.mkdirSync(out, { recursive: true });
fs.writeFileSync(path.join(out, 'monster-audit.json'), JSON.stringify(report, null, 2) + '\n');
const table = rs => ['| Monster | Library | Public drawable | Native drawable | Empty action frames | Out-of-range | Unknown |', '|---|---|---|---|---:|---:|---|', ...rs.map(x => `| ${x.name} | ${x.bodyLibrary} | ${x.publicDrawableAnimationComplete ? 'complete' : 'open'} | ${x.nativeDrawableAnimationComplete ? 'complete' : 'open'} | ${x.sourceEmptyActionFrames} | ${x.descriptorOutOfRangeFrames} | ${x.actionsUnknown} |`)].join('\n');
fs.writeFileSync(path.join(out, 'monster-audit.md'), `# Monster asset repair audit\n\n${report.scope}\n\nRun: node scripts/audit-all-monster-assets.mjs --sourceRoot <Crystal Data directory> --outDir <report directory>. The earlier all-monster-assets-20260921 baseline is preserved.\n\n${report.nativeConstraint}\n\nSource-empty slots are counted and excluded from missing exports. Out-of-range action indices are retained separately and are not counted as drawable closure failures. Unknown sources, absent actions, descriptor mismatches and pending library mappings cannot pass closure.\n\n\`\`\`json\n${JSON.stringify(summary, null, 2)}\n\`\`\`\n\n## Profile whitelist\n\n${table(rows.filter(x => x.profileWhitelisted))}\n\n## Newcomer V2 targets plus BoneFamiliar\n\n${table(rows.filter(x => newcomerNames.has(x.name)))}\n\n## Non-profile mapping follow-up\n\n${report.sourceMappingCaveat}\n\n## Descriptor exceptions\n\n${table(rows.filter(x => x.profileWhitelisted && x.descriptorOutOfRangeFrames))}\n`);
console.log(JSON.stringify(summary, null, 2));
const scoped = libraries.filter(l => rows.some(m => (m.profileWhitelisted || newcomerNames.has(m.name)) && m.bodyLibrary === l.library));
const exceptions = scoped.filter(l => l.sourceEmptyActionFrames.length || l.outOfRangeActionFrames.length).map(l => `- ${l.library}: source empty action indices [${l.sourceEmptyActionFrames.join(', ')}]; descriptor out-of-range indices [${l.outOfRangeActionFrames.join(', ')}].`);
fs.appendFileSync(path.join(out, 'monster-audit.md'), `\n## Exact source exceptions\n\n${exceptions.join('\n')}\n\nThese indices must not be repaired by inventing source artwork. Original action/state selection and permitted directions need separate runtime comparison, especially RootSpider and Sheep.\n`);
