import fs from 'node:fs/promises';
import {buildAuthoritativeClassQuestRoute} from './route-manifest.mjs';

const root = new URL('../../../../', import.meta.url);
const checkOnly = process.argv.includes('--check');
const routes = await Promise.all(['Warrior','Wizard','Taoist'].map(className => buildAuthoritativeClassQuestRoute({className,maxLevel:40})));
const route = routes[0];
const quests = new Map(route.quests.map(q => [q.questId, q]));
const profile = JSON.parse(await fs.readFile(new URL('packages/game-data/data/content_profiles/platinum_176.json',root),'utf8'));
const curve = profile.experienceCurve.filter(row => row.level < 30).map(row => row.requiredExperience);
const threshold = level => curve.slice(0, level - 1).reduce((a,b) => a+b, 0);
const definitions = [
  ['village','Your first adventure',1,5,6,[1,2,3,5,6],5,'Meet the village, equip your rewards, and earn your first skill book.','Starter weapon, jewellery and your class skill book.'],
  ['bichon','Beyond the village',6,10,11,[22,23,24,25,26,27,29,30,33],8,'Follow the guide to Bichon and learn how to prepare for a new map.','Weapon upgrades and armour; short, guaranteed quest-material hunts.'],
  ['frontier','Ready for the caves',11,15,16,[35,36,37,39,40,41,42,49],10,'Finish the snake-wine story and prove yourself against ordinary cave enemies.','A helmet plus a level-15 class weapon and core combat book.'],
  ['mines','Mines and rescue',16,20,21,[51,52,53,54,61,65,60,62],8,'Prepare supplies, clear the mines, and help the woodland expedition.','Belt and necklace choices plus a level-20 class weapon.'],
  ['expedition','Lead an expedition',21,25,26,[83,86,87,88,97,102,103,110,111,112,98,89,99],10,'Investigate the missing carriage, scout Wooma, and uncover the secret path.','The secret path plus level-25 class armour and a level-26 weapon.'],
  ['island','A new horizon',26,30,30,[113,114,117,118,119,121,124,122,123],12,'Secure the supply route and complete your first Prajna Island expedition.','A bangle upgrade and the level-30 growth reward.'],
];
const hints = {
  Warrior:'Equip a suitable weapon. Learn Fencing from the book when eligible; practise approaching one enemy and watch your health.',
  Wizard:'Learn FireBall from the book when eligible and bind it to a spell key. Keep distance and carry enough mana potions.',
  Taoist:'Learn Healing from the book when eligible and bind it to a spell key. Practise healing yourself and balancing health with mana.',
};
const milestoneRewards = [
  {
    level:15,
    classRewards:{
      Warrior:[{item:'SharpSword',count:1},{item:'Slaying',count:1}],
      Wizard:[{item:'SharpTrident',count:1},{item:'GreatFireBall',count:1}],
      Taoist:[{item:'SharpScimitar',count:1},{item:'SoulFireBall',count:1},{item:'Amulet',count:100}],
    },
  },
  {
    level:20,
    classRewards:{
      Warrior:[{item:'MartialSabre',count:1}],
      Wizard:[{item:'SpearWithHook',count:1}],
      Taoist:[{item:'KeenKrissSword',count:1}],
    },
  },
  {
    level:25,
    classRewards:{
      Warrior:[{maleItem:'ThickArmour(M)',femaleItem:'ThickArmour(F)',count:1},{item:'SolidGreatAxe',count:1}],
      Wizard:[{maleItem:'FireMagicRobe(M)',femaleItem:'FireMagicRobe(F)',count:1},{item:'SolidBronzeStaff',count:1}],
      Taoist:[{maleItem:'TaoArmour(M)',femaleItem:'TaoArmour(F)',count:1},{item:'SolidSerpentSword',count:1}],
    },
  },
];
let cumulative = 0;
const overrides = [];
const chapters = definitions.map(([id,title,minLevel,maxLevel,exitLevel,ids,cap,goal,rewardSummary], index) => {
  const classQuestIds = index === 0 ? {Warrior:[7,8,9],Wizard:[10,11,12],Taoist:[13,14,15]} : {Warrior:[],Wizard:[],Taoist:[]};
  const order = [...ids,...classQuestIds.Warrior];
  const start = cumulative;
  const end = threshold(exitLevel);
  // Authoring allocation: spread the chapter budget, but never leave the next
  // required quest behind a level gate. The independent audit checks every step.
  order.forEach((questId, position) => {
    const q = quests.get(questId);
    if (!q) throw new Error(`Missing source quest ${questId}`);
    const next = quests.get(order[position+1]);
    const target = Math.max(Math.floor(start+(end-start)*(position+1)/order.length), next ? threshold(next.eligibility.minLevel) : end);
    const rewardExperience = Math.max(0,target-cumulative);
    cumulative += rewardExperience;
    const tasks = [];
    for (const task of q.objectives.kill) tasks.push(`Defeat ${Math.min(task.count,cap)} ${task.monsterName}.`);
    for (const task of q.objectives.item) {
      const sourceNames = [...new Set(task.sources.map(source => source.monsterName))];
      if (!sourceNames.length) throw new Error(`Missing material source ${questId}/${task.itemName}`);
      // Prefer canonical display names over duplicate numbered drop tables.
      const names = sourceNames.filter(name => !sourceNames.includes(name.replace(/\d+$/, '')) || !/\d+$/.test(name));
      const harvest = task.sources.some(source => source.requiresHarvest);
      tasks.push(`Collect ${Math.min(task.count,2)} ${task.itemName} from ${names.join(' or ')}${harvest ? ' corpses; finish harvesting with Alt + left mouse' : ''}. Needed quest materials are guaranteed.`);
    }
    const entry = {questId,rewardExperience,killCountCap:cap,itemCountCap:2};
    if (q.objectives.item.length) entry.guaranteedQuestDrops=q.objectives.item.map(t=>t.itemName);
    if (tasks.length) entry.taskDescription=tasks;
    if (questId===61) entry.requiredQuestId=54;
    overrides.push(entry);
    if (index===0 && questId>=7) {
      for (const offset of [3,6]) overrides.push({...entry,questId:questId+offset});
    }
  });
  return {id,title,minLevel,maxLevel,exitLevel,questIds:ids,classQuestIds,goal,rewardSummary,classHints:hints,questExperienceBudget:end-start};
});
// Present the low-count snake-wine preparation before the frontier chapter,
// then finish the shortened snake hunt before entering the denser Skeleton
// cave. Three of each snake still teaches pulling without becoming a potion
// wall, and q33's armour and gold fund the first cave trip. Apply this after
// reward authoring so every quest keeps its exact XP and the level-30 route
// total remains deterministic.
const bichonChapter=chapters.find(chapter=>chapter.id==='bichon');
const frontierChapter=chapters.find(chapter=>chapter.id==='frontier');
const serpentPreparationQuestIds=[35,36,37];
const snakeHuntQuestId=33;
const frontierQuestOrder=[33,49,39,40,41,42];
const snakeIndex=bichonChapter?.questIds.indexOf(snakeHuntQuestId)??-1;
const preparationReward=serpentPreparationQuestIds.reduce((total,questId)=>
  total+(overrides.find(entry=>entry.questId===questId)?.rewardExperience??Number.NaN),0);
const snakeHuntReward=overrides.find(entry=>entry.questId===snakeHuntQuestId)?.rewardExperience??Number.NaN;
if(!bichonChapter||!frontierChapter||snakeIndex<0||
  serpentPreparationQuestIds.some(questId=>!frontierChapter.questIds.includes(questId))||
  frontierQuestOrder.some(questId=>questId!==snakeHuntQuestId&&!frontierChapter.questIds.includes(questId))||
  !Number.isSafeInteger(preparationReward)||!Number.isSafeInteger(snakeHuntReward)){
  throw new Error('Newcomer Serpent Valley route structure changed');
}
frontierChapter.questIds=frontierChapter.questIds.filter(questId=>!serpentPreparationQuestIds.includes(questId));
bichonChapter.questIds.splice(snakeIndex,0,...serpentPreparationQuestIds);
bichonChapter.questExperienceBudget+=preparationReward;
frontierChapter.questExperienceBudget-=preparationReward;
bichonChapter.questIds=bichonChapter.questIds.filter(questId=>questId!==snakeHuntQuestId);
frontierChapter.questIds=frontierQuestOrder;
bichonChapter.questExperienceBudget-=snakeHuntReward;
frontierChapter.questExperienceBudget+=snakeHuntReward;
const snakeHuntOverride=overrides.find(entry=>entry.questId===snakeHuntQuestId);
snakeHuntOverride.killCountCap=3;
snakeHuntOverride.taskDescription=['Defeat 3 TigerSnake.','Defeat 3 RedSnake.'];
overrides.push({questId:4,rewardExperience:80,itemCountCap:1,guaranteedQuestDrops:['DeerMeat'],taskDescription:['Optional harvest lesson: collect 1 DeerMeat from a Deer corpse.','Complete the skinning actions with Alt + left mouse. The needed task meat is guaranteed.']});
const allQuests = new Map(routes.flatMap(r=>r.quests).map(q=>[q.questId,q]));
const respawns=JSON.parse(await fs.readFile(new URL('packages/game-data/data/generated/crystal_respawn_manifest.json',root),'utf8'));
const mapNames=new Map(respawns.maps.map(map=>[map.map_file_name,map.map_title]));
const npcLocation=npc=>{
  if(!npc?.name||!mapNames.has(npc.mapFileName)||!Number.isInteger(npc.position?.x)||!Number.isInteger(npc.position?.y))throw new Error('Missing authoritative NPC location');
  return {name:npc.name.replaceAll('_',' '),mapName:mapNames.get(npc.mapFileName),mapFileName:npc.mapFileName,x:npc.position.x,y:npc.position.y};
};
for(const entry of overrides){
  const q=allQuests.get(entry.questId);
  entry.startInDiary=q.specialHandlers.includes('quest-diary-accept');
  entry.finishInDiary=q.specialHandlers.includes('quest-diary-finish');
  entry.startNpc=entry.startInDiary?null:npcLocation(q.startNpc);
  entry.finishNpc=entry.finishInDiary?null:npcLocation(q.finishNpc);
}
const config={schema:1,profile:'newcomer-v1',maxLevel:30,description:'Opt-in newcomer journey. Earn progression through ordinary quest completion; no daily wait or mandatory rare boss. Crystal mode retains its original values.',chapters,milestoneRewards,questOverrides:overrides};
const configPath = new URL('config/quest-guidance/newcomer-journey-v1.json',root);
const renderedConfig = JSON.stringify(config,null,2)+'\n';
if(checkOnly){
  const committedConfig=await fs.readFile(configPath,'utf8');
  if(JSON.stringify(JSON.parse(committedConfig))!==JSON.stringify(config))throw new Error('Generated newcomer journey profile differs from the committed config.');
  console.log(`Verified ${chapters.length} chapters, ${milestoneRewards.length} milestone rewards and ${overrides.length} overrides; ${cumulative} fixed route XP per class.`);
} else {
  await fs.writeFile(configPath,renderedConfig);
}
if(!checkOnly){
const guidancePath = new URL('config/quest-guidance/newcomer-v1.json',root);
const guidance = JSON.parse(await fs.readFile(guidancePath,'utf8'));
guidance.description = 'Optional guidance through level 40. The separate newcomer journey profile supplies opt-in 1-30 task pacing; Crystal mode is unchanged.';
const mainOrder = chapters.flatMap(c=>[...c.questIds,...c.classQuestIds.Warrior,...c.classQuestIds.Wizard,...c.classQuestIds.Taoist]);
const main = new Set(mainOrder);
const overridesById = new Map(overrides.map(row=>[row.questId,row]));
for(const entry of guidance.quests){
  if(main.has(entry.id)) entry.category='recommended';
  else if(allQuests.get(entry.id)?.eligibility.minLevel<=30 && entry.category==='recommended') entry.category='optional';
  if(entry.id===58) entry.category='challenge';
  if(entry.id===84) entry.hint='Optional thread collection: the original quantities and drop rates apply. Return with stronger equipment; this task is not required for the level-30 journey.';
  const override=overridesById.get(entry.id);
  if(override?.taskDescription) entry.hint=override.taskDescription.join(' ');
}
// Sort every entry topologically, including optional reverse-ID chains.
const pending = new Map(guidance.quests.map(entry=>[entry.id,entry]));
const done = new Set();
let order=10;
while(pending.size){
  const eligible=[...pending.values()].filter(entry=>{
    const q=allQuests.get(entry.id);
    const predecessor=overridesById.get(entry.id)?.requiredQuestId ?? q?.eligibility.requiredQuestId;
    return !predecessor || done.has(predecessor);
  }).sort((a,b)=>{
    const rank=id=>main.has(id)?mainOrder.indexOf(id):10000+(allQuests.get(id)?.eligibility.minLevel??99)*100+id;
    return rank(a.id)-rank(b.id);
  });
  if(!eligible.length) throw new Error('Quest prerequisite cycle while authoring guidance');
  const entry=eligible[0]; entry.order=order; order+=10; done.add(entry.id); pending.delete(entry.id);
}
await fs.writeFile(guidancePath,JSON.stringify(guidance,null,2)+'\n');
console.log(`Authored ${chapters.length} chapters, ${overrides.length} overrides; ${cumulative} fixed route XP per class.`);
}
