import fs from 'node:fs';
import path from 'node:path';
import sharp from 'sharp';
import {parseLibrary,decodeFrameRgba} from './crystal-library.mjs';
import {collectItemIconRequirements,sha256} from './asset-pipeline/item-icon-closure.mjs';
const root=path.resolve(import.meta.dirname,'../../..');
const source=process.argv[2];
if(!source) throw Error('Usage: node apps/web/scripts/audit-all-item-assets-20260921.mjs SOURCE_DATA_DIR');
const read=p=>JSON.parse(fs.readFileSync(path.join(root,p),'utf8'));
const manifest=read('packages/game-data/data/generated/crystal_item_manifest.json');
const items=manifest.items, req=collectItemIconRequirements(manifest);
const profile=read('packages/game-data/data/content_profiles/platinum_176.json');
const names=new Map(items.map(x=>[x.name,x]));
const unique=a=>[...new Set(a)].sort((a,b)=>typeof a==='number'?a-b:a.localeCompare(b));
const drops=read('packages/game-data/data/generated/crystal_drop_manifest.json').tables;
const npcs=read('packages/game-data/data/generated/crystal_npc_manifest.json').scripts;
const quests=read('packages/game-data/data/generated/crystal_quest_packet_manifest.json').quests;
const shopNames=scripts=>unique(scripts.flatMap(s=>{let trade=false;return s.lines.flatMap(l=>{l=l.trim();if(l.startsWith('[')){trade=l.toLowerCase()==='[trade]';return [];}return trade&&l&&!l.startsWith(';')?[l.split(/\s+/)[0]]:[];});}));
const dropNames=tables=>unique(tables.flatMap(t=>t.sections.flatMap(s=>s.entries.map(e=>e.item_name))));
const scopes={allImported:items.map(x=>x.name),platinumWhitelist:profile.itemWhitelist,allShopTrade:shopNames(npcs),platinumShopTrade:shopNames(npcs.filter(s=>profile.npcScriptWhitelist.includes(s.script_key))),allDropTables:dropNames(drops),platinumMonsterDropTables:dropNames(drops.filter(t=>profile.monsterWhitelist.includes(t.table_key))),platinumDropOverrides:unique(profile.dropOverrides.map(x=>x.item)),questCarryAndTasks:unique(quests.flatMap(q=>[...q.carry_items,...q.item_tasks].map(x=>x.item_name)))};
const libraries={};
for(const libName of ['Items','DNItems','StateItem']){
 const bytes=fs.readFileSync(path.join(source,libName+'.Lib')),lib=parseLibrary(bytes),meta=read('apps/web/public/original-ui/'+libName+'/meta.json'),frames=new Map(meta.frames.map(f=>[f.index,f]));
 const indices=libName==='StateItem'?unique(items.filter(x=>[1,2,4].includes(x.item_type)).map(x=>x.image)):libName==='DNItems'?unique([...req.uniqueImages,112,113,114,115,116]):req.uniqueImages;
 const checks=[];
 for(const index of indices){
  const original=lib.frames[index],frame=frames.get(index),pngPath=path.join(root,'apps/web/public/original-ui',libName,index+'.png');
  const c={index,sourcePresent:!!original,metadataPresent:!!frame,filePresent:fs.existsSync(pngPath),issues:[]};
  if(!original||original.width<=0||original.height<=0)c.issues.push('source-empty-or-absent');
  if(!frame)c.issues.push('missing-metadata');
  if(!c.filePresent)c.issues.push('missing-png');
  if(c.filePresent){try{const png=fs.readFileSync(pngPath),{data,info}=await sharp(png).ensureAlpha().raw().toBuffer({resolveWithObject:true});c.width=info.width;c.height=info.height;c.visiblePixels=0;for(let p=3;p<data.length;p+=4)if(data[p])c.visiblePixels++;
    if(c.visiblePixels===0)c.issues.push('fully-transparent-png');
    if(original&&original.width>0&&original.height>0){if(sha256(data)!==sha256(decodeFrameRgba(lib,original)))c.issues.push('original-pixels-differ');if(original.width!==info.width||original.height!==info.height)c.issues.push('original-geometry-differ');if(frame&&(frame.x!==original.x||frame.y!==original.y))c.issues.push('original-offset-differ');}
    if(frame?.rgbaSha256&&frame.rgbaSha256!==sha256(data))c.issues.push('rgba-metadata-hash-differ');
    if(frame?.pngSha256&&frame.pngSha256!==sha256(png))c.issues.push('png-metadata-hash-differ');
   }catch(e){c.issues.push('decode-failed');c.error=String(e);}}
  checks.push(c);
 }
 const counts={};for(const c of checks)for(const issue of c.issues)counts[issue]=(counts[issue]??0)+1;
 libraries[libName]={requiredUniqueFrames:indices.length,sourceFrameSlots:lib.count,exportMetadataFrames:meta.frames.length,sourceSha256:sha256(bytes),metadataSourceSha256:meta.sourceLibrary?.sha256,sourceIdentityMatches:meta.sourceLibrary?.sha256===sha256(bytes),counts,checks};
}
const scopeReports={};
for(const [scope,list] of Object.entries(scopes)){
 const known=scope==='allImported'?items:list.map(n=>names.get(n)).filter(Boolean);const byLibrary={};
 for(const [lib,data]of Object.entries(libraries)){const relevant=lib==='StateItem'?known.filter(x=>[1,2,4].includes(x.item_type)):known;const images=unique(relevant.flatMap(x=>lib==='StateItem'?[x.image]:[x.image,...(req.requirements.find(r=>r.itemIndex===x.item_index)?.stackImages??[])])); const problems=data.checks.filter(c=>images.includes(c.index)&&c.issues.length);byLibrary[lib]={itemCount:relevant.length,uniqueFrames:images.length,problemFrames:problems.length,problemItems:relevant.filter(x=>problems.some(c=>c.index===x.image)).map(x=>({name:x.name,image:x.image})),problems};}
 scopeReports[scope]={referencedNames:list.length,knownItems:known.length,unresolvedNames:list.filter(n=>!names.has(n)),byLibrary};
}
const report={generatedAt:new Date().toISOString(),source:path.resolve(source),itemCount:items.length,uniqueCatalogueImages:req.uniqueCatalogueImages.length,stackImages:req.uniqueStackImages,libraries,scopes:scopeReports,limitations:['Static resource/source-pixel audit; no new native runtime or human visual acceptance.','StateItem requirements are Weapon/Armour/Helmet, as original CharacterDialog draws only those three paperdoll layers; accessory cells use Items.','Platinum drop table scope matches monsterWhitelist to table_key; nested/random table reachability is not interpreted.','Quest scope covers imported carry_items/item_tasks; reward payloads and optional newcomer overlays are not decoded here.','Shop scope parses exact [Trade] stock entries; conditional GIVE commands and dynamic NPC stocks are not interpreted.','Does not assert generated native geometry tables or packaged asset copies are current.']};
const out=path.join(root,'docs/generated/player-qa/all-assets-20260921');fs.mkdirSync(out,{recursive:true});fs.writeFileSync(path.join(out,'items-audit.json'),JSON.stringify(report,null,2)+'\n');
console.log(JSON.stringify({itemCount:items.length,libraries:Object.fromEntries(Object.entries(libraries).map(([k,v])=>[k,{frames:v.requiredUniqueFrames,sourceIdentityMatches:v.sourceIdentityMatches,counts:v.counts}])),scopes:Object.fromEntries(Object.entries(scopeReports).map(([k,v])=>[k,{names:v.referencedNames,known:v.knownItems,unresolved:v.unresolvedNames,problems:Object.fromEntries(Object.entries(v.byLibrary).map(([l,s])=>[l,s.problemFrames]))}]))},null,2));
