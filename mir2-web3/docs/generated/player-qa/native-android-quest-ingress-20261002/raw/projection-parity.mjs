import assert from 'node:assert/strict';
import {readFileSync,writeFileSync} from 'node:fs';
import {execFileSync,spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
const root='mir2-web3/apps/game-client/';
const frozen='3d735745f1117d42a7859e87604a106351dca935';
const sha=value=>createHash('sha256').update(value).digest('hex');
const base=(path)=>execFileSync('git',['show',`${frozen}:${root+path}`],{encoding:'utf8',maxBuffer:4*1024*1024});
const gameplay=base('platform-windows/src/gameplay_bridge.rs');
const gateway=base('platform-windows/src/gateway.rs');
const shared=readFileSync(root+'client-bevy/src/native_quest_ingress.rs','utf8');
const inventory=readFileSync(root+'client-bevy/src/native_inventory_ingress.rs','utf8');
function block(source,name,predicate=()=>true){
 const regex=new RegExp(`^([ \\t]*)(?:pub )?fn ${name}\\(`,'gm');
 for(const match of source.matchAll(regex)){
  const end=source.indexOf('\n'+match[1]+'}\n',match.index);
  assert(end>match.index,name);
  const found=source.slice(match.index,end+match[1].length+3);
  if(predicate(found))return found.replace(/^[ \t]*pub fn /m,'fn ');
 }
 throw new Error('Missing shared symbol '+name);
}
function normalize(body){
 const result=spawnSync('rustup',['run','1.95.0','rustfmt','--edition','2021','--emit','stdout'],
  {input:body,encoding:'utf8',timeout:30000});
 assert.equal(result.status,0,result.stderr);
 return result.stdout.trim();
}
const questNames=['parse_quest_definition','transform_quest_tracker','transform_quest_objectives',
 'parse_quest_rewards','transform_npc_dialog','transform_nearby_npcs','quest_status',
 'completed_quest_ids','completed_quest_ids_from_snapshot'];
const helpers=['value_u32','value_i32','string_at','string_array','tile_distance','display_npc_name','strip_crystal_markup'];
const rewards=['crystal_wire_item_info','crystal_tooltip_source_for_preview','add_quest_reward_tooltip_sources'];
const records=[];
function check(name,original,actual){
 const left=normalize(original),right=normalize(actual);
 assert.equal(right,left,'Frozen Windows projection changed: '+name);
 records.push({name,bodyIdentical:true,normalizedSha256:sha(left)});
}
for(const name of [...questNames,...helpers])check(name,block(gameplay,name),
 block(shared,name,body=>name!=='value_i32'||body.includes('value: &Value)')));
for(const name of rewards)check(name,block(gateway,name),block(shared,name));
check('gateway_value_i32',block(gateway,'value_i32'),
 block(shared,'value_i32',body=>body.includes('Option<&Value>')));
const struct=(source)=>{
 const begin=source.indexOf('struct QuestDefinition {');
 const end=source.indexOf('\n}\n',begin);
 assert(begin>=0&&end>begin);
 return source.slice(begin,end+3).replace(/^    pub /gm,'    ');
};
check('QuestDefinition_fields',struct(gameplay),struct(shared));
const aliases=[['crystal_tooltip_viewer','native_tooltip_viewer'],
 ['unique_crystal_tooltip_info','native_tooltip_info'],
 ['crystal_real_tooltip_info','native_real_tooltip_info'],
 ['unique_crystal_tooltip_template','unique_crystal_item_template']];
for(const [before,after] of aliases){
 let original=block(gateway,before);
 for(const [oldName,newName] of aliases)original=original.replaceAll(oldName,newName);
 check(`transitive_${after}`,original,block(inventory,after));
}
const currentWindowsGameplay=readFileSync(root+'platform-windows/src/gameplay_bridge.rs','utf8');
const currentWindowsGateway=readFileSync(root+'platform-windows/src/gateway.rs','utf8');
for(const name of questNames)assert(!new RegExp(`^fn ${name}\\(`,'m').test(currentWindowsGameplay));
for(const name of rewards)assert(!new RegExp(`^fn ${name}\\(`,'m').test(currentWindowsGateway));
const output={frozenWindowsCommit:frozen,scope:'Pure presentation projections only; not live quest, rendering, reward or save acceptance',
 sharedProjectionSha256:sha(shared),rows:records,allBodiesIdentical:records.every(r=>r.bodyIdentical),
 WindowsOriginalCallSitesUseSharedExports:true};
writeFileSync(root+'platform-android/target/quest-ingress-20261002/projection-parity.json',JSON.stringify(output,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({rows:records.length,allBodiesIdentical:output.allBodiesIdentical,frozenWindowsCommit:frozen}));
