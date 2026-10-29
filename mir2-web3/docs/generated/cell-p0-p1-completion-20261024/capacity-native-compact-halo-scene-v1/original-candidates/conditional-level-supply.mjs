import fs from 'node:fs';
import { loadGeometry, PARAMETERS, ROOT, BASE, PROJECT, pin, writeNew } from './static-lib.mjs';
const g=await loadGeometry(),sceneFile=ROOT+'/candidate-100-compact-v2.json',validationFile=ROOT+'/validation-compact-v2.json';
const validation=JSON.parse(fs.readFileSync(validationFile));
const paths={stats:PROJECT+'/apps/simulation/src/runtime/stats.rs',leveling:PROJECT+'/apps/simulation/src/runtime/leveling.rs',
  authoritativeDamage:PROJECT+'/apps/simulation/src/runtime/zone/runtime.rs',experience:PROJECT+'/apps/simulation/src/runtime/zone/experience.rs',
  experienceProfile:PROJECT+'/apps/simulation/src/runtime/shared_experience_profile.rs',config:PROJECT+'/apps/simulation/src/config.rs',
  items:PROJECT+'/packages/game-data/data/generated/crystal_item_manifest.json',profile:PROJECT+'/packages/game-data/data/content_profiles/platinum_176.json',
  starterEquipmentDriver:PROJECT+'/apps/web/scripts/quest-agent/protocol-play.mjs',inventory:PROJECT+'/apps/simulation/src/runtime/inventory.rs'};
const stats=fs.readFileSync(paths.stats,'utf8'),leveling=fs.readFileSync(paths.leveling,'utf8'),items=JSON.parse(fs.readFileSync(paths.items)).items,
  profile=JSON.parse(fs.readFileSync(paths.profile));
for(const exact of['s!(CRYSTAL_STAT_MIN_DC, Stat, 0, 5.0','s!(CRYSTAL_STAT_MAX_DC, Stat, 0, 5.0','s!(CRYSTAL_STAT_ACCURACY, Stat, 5, 0.0','s!(CRYSTAL_STAT_HP, Health, 14, 4.0, 4.5)']){
  if(!stats.includes(exact))throw new Error('Pinned Warrior model differs: '+exact);
}
const sword=items.find(i=>i.name==='WoodenSword'),minDc=sword?.stats.find(s=>s.stat===4)?.value,maxDc=sword?.stats.find(s=>s.stat===5)?.value;
if(minDc!==2||maxDc!==4)throw new Error('Unexpected ordinary starter WoodenSword DC');
const curve=leveling.match(/CRYSTAL_EXPERIENCE_LIST: \[i64; 500\] = \[([\s\S]*?)\];/)?.[1].match(/[0-9]+/g)?.map(Number);
if(curve?.length!==500)throw new Error('Unexpected native XP curve');
const cumulative=level=>curve.slice(0,level-1).reduce((a,b)=>a+b,0);
if(cumulative(20)!==456900)throw new Error('Unexpected level20 XP threshold');
const targetTemplates=PARAMETERS.monsterNames.map(name=>g.templates.find(t=>t.name===name));
if(targetTemplates.some(t=>t.min_ac!==0||t.max_ac!==0))throw new Error('Conditional AC0 model no longer applies');
const expectedHits=(hp,lo,hi)=>{const e=[0];for(let h=1;h<=hp;h++){let sum=0;for(let d=lo;d<=hi;d++)sum+=e[Math.max(0,h-d)];e[h]=1+sum/(hi-lo+1);}return e[hp];};
const targetsFor=s=>{const ids=new Set(s.ruleSupply.flatMap(r=>r.objectIds));return g.targets.filter(t=>ids.has(t.objectId)).map(t=>({...t,experience:g.templates.find(m=>m.name===t.name).experience}));};
const levels=[1,5,10,15,20,25,30];
const stages=validation.stages.map(s=>{
  const targets=targetsFor(s),required=12*s.fighterIndices.length;
  return{actors:s.count,fighterCount:s.fighterIndices.length,targets:targets.length,targetCounts:s.uniqueCounts,
    requiredAttacksPerMinute:18*s.fighterIndices.length,requiredPositiveHitsPerMinute:required,
    levels:levels.map(level=>{const lo=2+Math.floor(level/5),hi=4+Math.floor(level/5),rate=targets.reduce((sum,t)=>sum+expectedHits(t.hp,lo,hi)/Math.max(3,t.delayMinutes),0);
      return{level,ordinaryStarterMinDc:lo,ordinaryStarterMaxDc:hi,baseAccuracy:5,baseHealthBeforeEquipmentOrBuffs:Math.trunc(14+(level/4+4.5+level/20)*level),
        cumulativeNativeXpToLevel:cumulative(level),idealUniformExpectedPositiveHitsPerMinute:rate,aggregateRateMarginOverRequired:rate-required,
        minimumDcConditionalUpperPositiveHitsPerMinute:targets.reduce((sum,t)=>sum+Math.ceil(t.hp/lo)/Math.max(3,t.delayMinutes),0),
        maximumDcConditionalLowerPositiveHitsPerMinute:targets.reduce((sum,t)=>sum+Math.ceil(t.hp/hi)/Math.max(3,t.delayMinutes),0),
        idealInitialPositiveHitStock:targets.reduce((sum,t)=>sum+expectedHits(t.hp,lo,hi),0),
        idealUniformTwoHourHitsIncludingInitial:targets.reduce((sum,t)=>sum+expectedHits(t.hp,lo,hi)*(1+Math.floor(120/Math.max(3,t.delayMinutes))),0),
        continuousWindowSupplyProven:false};}),
    ruleSupply:s.ruleSupply,activeTargetIdsDeduplicated:true};
});
const full=targetsFor(validation.stages.at(-1)),closedBound=minutes=>({minutes,
  idealKillsIncludingInitial:full.reduce((sum,t)=>sum+1+Math.floor(minutes/Math.max(3,t.delayMinutes)),0),
  deliberatelyOvergenerousTier2XpAssignedAllToOneFreshActor:full.reduce((sum,t)=>sum+2*t.experience*(1+Math.floor(minutes/Math.max(3,t.delayMinutes))),0)});
const xp2h=closedBound(120),xp5h=closedBound(300);
const result=writeNew(ROOT+'/conditional-level-supply-v1.json',{schemaVersion:1,createdAt:new Date().toISOString(),scope:'Conditional ordinary starter-Warrior level/XP/positive-hit supply analysis for the proposed compact-v2 scene; no gameplay execution or acceptance',
  scenePin:pin(sceneFile),validationPin:pin(validationFile),parameters:PARAMETERS,
  nativeModel:{weapon:'WoodenSword',weaponDc:{min:minDc,max:maxDc},baseMinMaxDc:'floor(level/5)',baseAccuracy:5,
    attackModel:'Independent uniform integer draw inside effective DC band, AC0, no added DC/Luck/critical/buffs/skills; effective original starter weapon remains equipped. Expected positive hits per kill computed by finite HP dynamic programming, including final overkill clipping.',
    targetTemplates:targetTemplates.map(t=>({name:t.name,hp:t.hp,minAc:t.min_ac,maxAc:t.max_ac,agility:t.agility,level:t.level,baseExperience:t.experience})),
    normalEquipment:'The unchanged capacity driver ordinarily equips available starter inventory once. No automatic purchase, upgrade, grant, inventory refill or forced XP/level lock is performed.',
    healthScope:'BaseHealth is class/level only. Real gear, buffs, damage, regeneration, held finite HP potion, durability and actual snapshot health must be observed; no survival estimate is supplied.',
    ratePolicy:profile.ratePolicy.monsterExperienceTiers},
  stages,closedPoolProgression:{canonicalSlots:full.length,scope:'Conditional closed129canonical-slot pool only, no external wandering-in targets/quest/other XP/positive bonuses, every slot killed immediately after due refresh; all rewards assigned to ONE actor at tier2 for the whole bound. Level penalties, shared allocation, travel and kill time omitted, so deliberately overgenerous.',
    twoHours:xp2h,entire18000SecondBudget:xp5h,requiredXpLevel20:cumulative(20),requiredXpLevel25:cumulative(25),
    level20NotReachedEvenByThisOvergenerousClosedFiveHourModel:xp5h.deliberatelyOvergenerousTier2XpAssignedAllToOneFreshActor<cumulative(20)},
  relatedHistoricalRiskSummary:pin(BASE+'/halo-natural-progression-risk-summary-v2.json'),
  conclusions:['FixedDamage10 is a sensitivity case, not actual fresh-player damage. Starter-only Warrior levels1/5/10/15 have DC2-4/3-5/4-6/5-7 against targetAC0.',
    'Expected conditional supply is not a lower bound, runtime simulation, actual target census or per-fighter allocation proof. Shared IDs cannot be credited independently to ten fighters.',
    'The new pool must not inherit the historical126slot XP number. This artifact recalculates129deduplicated stable slots and both2h/5h conditional XP bounds.',
    'Failure of a fixed-D10 hypothetical rate does not establish that a fresh two-hour run fails. Conversely an expected aggregate rate over120/min does not prove every fighter and every60s window meets18attempts/12positivehits.',
    'Real ordinary loot is not credited in this model. Existing held potion is finite; driver does not pick up, buy or equip dropped upgrades. Any real equipment/buff/XP source change invalidates simple starter-only bands.',
    'Fresh early-density/actuallevel/damage/targetID/respawn/witness/scout data and unchanged full acceptance gates remain necessary. P0pending; no P2 authorization.'],
  sourcePins:[...g.sourcePins,...Object.values(paths).map(pin),pin(import.meta.filename)],capacityAccepted:false,accepted:false});
console.log(JSON.stringify({result,ten:stages.find(s=>s.actors===10).levels.filter(l=>[1,5,10,15].includes(l.level)),hundred:stages.at(-1).levels.filter(l=>[1,5,10,15].includes(l.level)),closedPool:closedBound(300)}));
