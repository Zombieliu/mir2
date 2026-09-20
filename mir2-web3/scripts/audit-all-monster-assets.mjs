import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const read = p => JSON.parse(fs.readFileSync(path.join(root,p),'utf8'));
const exists = p => fs.existsSync(path.join(root,p));
const monsters = read('packages/game-data/data/generated/crystal_monster_manifest.json').monsters;
const maps = read('packages/game-data/data/generated/crystal_respawn_manifest.json').maps;
const profile = read('packages/game-data/data/content_profiles/platinum_176.json');
const atlas = read('apps/web/public/bevy-entity-atlases/manifest.json');
const frameSets = read('apps/web/public/original-ui/frame-sets.generated.json').libraries;
const sourceRoot = process.argv[2];
const atlasPaths = new Set();
const pages=[];
for (const a of atlas.atlases) {
  for (const p of a.pages ?? [a]) pages.push({url:p.imageUrl,exists:exists(`apps/web/public${p.imageUrl}`)});
  for (const r of a.rects) {
    const page=(a.pages??[a])[r.pageIndex??0];
    if(page && exists(`apps/web/public${page.imageUrl}`)) atlasPaths.add(r.key.split('|')[0]);
  }
}
const whitelist=new Set(profile.monsterWhitelist);
const newcomerNames=new Set(['BoneFamiliar']);
function collectNewcomer(value) {
  if(!value||typeof value!=='object') return;
  if(typeof value.monster==='string') newcomerNames.add(value.monster);
  for(const child of Object.values(value)) collectNewcomer(child);
}
collectNewcomer(read('config/quest-guidance/newcomer-journey-v2.json'));
const allowedMaps=new Set(profile.mapWhitelist.map(x=>x.fileName));
const references=new Map();
for(const map of maps) for(const r of map.respawns) {
  if(!references.has(r.monster_index)) references.set(r.monster_index,[]);
  references.get(r.monster_index).push({map:map.map_file_name,count:r.count,profile:allowedMaps.has(map.map_file_name)&&whitelist.has(r.monster_name)});
}
const libraries=[];
for(const image of [...new Set(monsters.map(m=>m.image))].sort((a,b)=>a-b)) {
  const library=`Monster/${String(image).padStart(3,'0')}`;
  const base=`apps/web/public/original-ui/${library}`;
  const meta=exists(`${base}/meta.json`)?read(`${base}/meta.json`):null;
  const pngs=new Set(meta?fs.readdirSync(path.join(root,base)).filter(x=>/^\d+\.png$/.test(x)).map(x=>Number(x.slice(0,-4))):[]);
  const actions=frameSets[library]?.actions??meta?.frameSet?.actions??[];
  const missingPublic=new Set(), missingNative=new Set(), blank=new Set();
  const actionAudit=actions.map(a=>{
    const directions=[];
    for(let d=0;d<8;d++) {
      let publicMissing=0,nativeMissing=0,zeroSize=0;
      for(let phase=0;phase<a.count;phase++) {
        const f=a.start+d*(a.count+a.skip)+phase;
        if(!pngs.has(f)){publicMissing++;missingPublic.add(f);}
        if(!atlasPaths.has(`/original-ui/${library}/${f}.png`)){nativeMissing++;missingNative.add(f);}
        const frame=meta?.frames?.find(x=>x.index===f);
        if(frame && (!frame.width||!frame.height)){zeroSize++;blank.add(f);}
      }
      directions.push({direction:d,publicMissing,nativeMissing,zeroSize});
    }
    return {action:a.actionName,start:a.start,count:a.count,skip:a.skip,directions};
  });
  libraries.push({library,image,sourceExists:sourceRoot?fs.existsSync(path.join(sourceRoot,`${library}.Lib`)):null,metaExists:!!meta,metaCount:meta?.count??0,metaFrames:meta?.frames?.length??0,pngCount:pngs.size,actionCount:actions.length,metaAndGeneratedActionsMatch:meta?JSON.stringify(meta.frameSet?.actions)===JSON.stringify(frameSets[library]?.actions):null,atlasFrames:[...atlasPaths].filter(p=>p.startsWith(`/original-ui/${library}/`)).length,missingPublicFrames:[...missingPublic].sort((a,b)=>a-b),missingNativeFrames:[...missingNative].sort((a,b)=>a-b),zeroSizeFrames:[...blank].sort((a,b)=>a-b),actions:actionAudit});
}
const byImage=new Map(libraries.map(x=>[x.image,x]));
const rows=monsters.map(m=>{
  const l=byImage.get(m.image), refs=references.get(m.monster_index)??[];
  const overrides=profile.respawnOverrides.filter(x=>x.monster===m.name);
  return {index:m.monster_index,name:m.name,level:m.level,image:m.image,bodyLibrary:l.library,respawnRecords:refs.length,respawnMaps:[...new Set(refs.map(x=>x.map))],profileWhitelisted:whitelist.has(m.name),profileRespawnRecords:refs.filter(x=>x.profile).length,profileOverrideRecords:overrides.length,bichon:refs.some(x=>x.map==='0'),publicLibrary:l.metaExists,publicAnimationComplete:l.actionCount>0&&l.missingPublicFrames.length===0,nativeAtlasFrames:l.atlasFrames,nativeAnimationComplete:l.actionCount>0&&l.missingNativeFrames.length===0};
});
const summarize=rs=>({monsters:rs.length,libraries:new Set(rs.map(x=>x.bodyLibrary)).size,missingPublicLibrary:rs.filter(x=>!x.publicLibrary).length,missingPublicLibraryCount:new Set(rs.filter(x=>!x.publicLibrary).map(x=>x.bodyLibrary)).size,incompletePublicAnimation:rs.filter(x=>!x.publicAnimationComplete).length,noNativeAtlas:rs.filter(x=>x.nativeAtlasFrames===0).length,noNativeAtlasLibraryCount:new Set(rs.filter(x=>x.nativeAtlasFrames===0).map(x=>x.bodyLibrary)).size,incompleteNativeAnimation:rs.filter(x=>!x.nativeAnimationComplete).length});
const summary={definitions:summarize(rows),respawnReferenced:summarize(rows.filter(x=>x.respawnRecords)),profileWhitelist:summarize(rows.filter(x=>x.profileWhitelisted)),profileSpawned:summarize(rows.filter(x=>x.profileRespawnRecords||x.profileOverrideRecords)),bichon:summarize(rows.filter(x=>x.bichon)),profileLevel1to30:summarize(rows.filter(x=>x.profileWhitelisted&&x.level>=1&&x.level<=30)),respawnRecords:maps.reduce((n,m)=>n+m.respawns.length,0),atlasPages:pages.length,missingAtlasPages:pages.filter(p=>!p.exists).length};
const report={generatedAt:new Date().toISOString(),scope:'All imported monster definitions, all respawn references, profile whitelist plus overrides. Static workspace asset reachability only; no running package or visual acceptance claim.',sourceRoot:sourceRoot??null,animationFormula:'start + direction * (count + skip) + phase, directions 0..7; phase reversal visits same indices',nativeConstraint:'Monster PNG paths are excluded by player_frame_path_parts; build_entity_layer requires atlas rect. Public PNG presence alone does not establish native reachability.',summary,pages,monsters:rows,libraries};
summary.newcomerV2TargetsAndBoneFamiliar=summarize(rows.filter(x=>newcomerNames.has(x.name)));
summary.profileBichon=summarize(rows.filter(x=>x.bichon&&x.profileWhitelisted));
report.newcomerV2TargetNames=[...newcomerNames];
report.sourceMappingCaveat='sourceExists tests the runtime Monster/{image} path, not all alternative Crystal libraries. Crystal MonsterObject.cs maps 900/904 to Dragon, 950..953 to Gate/00..03 and 10000+ to Pet/00+; these direct Monster paths must not be reported as missing original artwork.';
const out=path.join(root,'docs/generated/player-qa/all-monster-assets-20260921');
fs.mkdirSync(out,{recursive:true});
fs.writeFileSync(path.join(out,'audit.json'),JSON.stringify(report,null,2)+'\n');
const table=rs=>['| Monster | Level | Library | Respawns | PNG animation | Native atlas / animation |','|---|---:|---|---:|---|---|',...rs.map(x=>`| ${x.name} | ${x.level} | ${x.bodyLibrary} | ${x.respawnRecords} | ${x.publicAnimationComplete?'complete':'MISSING'} | ${x.nativeAtlasFrames} / ${x.nativeAnimationComplete?'complete':'MISSING'} |`)].join('\n');
fs.writeFileSync(path.join(out,'README.md'),`# Full monster asset audit\n\n${report.scope}\n\n\`node scripts/audit-all-monster-assets.mjs <Crystal Data directory>\` regenerates this report without exporting assets.\n\n\`\`\`json\n${JSON.stringify(summary,null,2)}\n\`\`\`\n\nNative Monster frames require atlas membership: platform-windows/src/atlas.rs build_entity_layer and player_frame_path_parts exclude Monster from standalone PNG fallback. All eight directions of every imported action are enumerated, including count + skip direction stride. Zero-size source frames are listed separately, not automatically treated as export defects.\n\n## Bichon actual respawn definitions\n\n${table(rows.filter(x=>x.bichon))}\n\n## Profile levels 1–30 (monster level, not quest route level)\n\n${table(rows.filter(x=>x.profileWhitelisted&&x.level>=1&&x.level<=30))}\n\n## Remediation\n\n1. Export missing public libraries from the recorded original Crystal Data source, preserving complete metadata, source frame geometry and all action directions. Review zero-size source frames separately.\n2. Add scoped, lazy native Monster PNG support analogous to verified player frames, or build bounded zone-specific atlases for every profile-used library. Existing eight atlas pages alone cannot cover the profile.\n3. Re-run this full manifest/reference audit after changes, then compare authenticated scene screenshots and movement/attack/death animations with Crystal. Static presence is not visual parity.\n4. Include summons and authored newcomer monsters in a further runtime snapshot union; this report includes all 555 definitions but spawn subsets represent imported respawns and profile overrides, not exhaustive dynamic summons.\n`);
console.log(JSON.stringify(summary,null,2));
fs.appendFileSync(path.join(out,'README.md'),`\n## Newcomer V2 exact configured targets plus BoneFamiliar\n\n${table(rows.filter(x=>newcomerNames.has(x.name)))}\n\n## Original-library mapping caveat\n\n${report.sourceMappingCaveat}\n\nThe server snapshot currently emits Monster/{image:03} in apps/simulation/src/runtime/packets.rs:8114. Crystal's client switches library family in Client/MirObjects/MonsterObject.cs:149. Profile SabukGate and PalaceWallLeft/1/2 need a dedicated Gate-family mapping check before any export plan; exporting nonexistent Monster/950.Lib is not a repair.\n`);
