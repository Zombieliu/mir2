import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import test from 'node:test';
import fs from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import {buildClassQuestRoute,loadCrystalQuestRouteSources} from './route-manifest.mjs';
import {annotateNewcomerRoute,loadNewcomerGuidance} from './newcomer-guidance.mjs';

const sources=await loadCrystalQuestRouteSources();
const guidance=await loadNewcomerGuidance();
const config=guidance.journey;
const profile=JSON.parse(await fs.readFile(new URL('../../../../packages/game-data/data/content_profiles/platinum_176.json',import.meta.url),'utf8'));
const itemManifest=JSON.parse(await fs.readFile(new URL('../../../../packages/game-data/data/generated/crystal_item_manifest.json',import.meta.url),'utf8'));
const itemsByName=new Map(itemManifest.items.map(item=>[item.name,item]));
const threshold=level=>profile.experienceCurve.filter(row=>row.level<level).reduce((sum,row)=>sum+row.requiredExperience,0);

const expectedMilestoneRewards=[
  {level:15,classRewards:{
    Warrior:[{item:'SharpSword',count:1},{item:'Slaying',count:1}],
    Wizard:[{item:'SharpTrident',count:1},{item:'GreatFireBall',count:1}],
    Taoist:[{item:'SharpScimitar',count:1},{item:'SoulFireBall',count:1},{item:'Amulet',count:100}],
  }},
  {level:20,classRewards:{
    Warrior:[{item:'MartialSabre',count:1}],
    Wizard:[{item:'SpearWithHook',count:1}],
    Taoist:[{item:'KeenKrissSword',count:1}],
  }},
  {level:25,classRewards:{
    Warrior:[{maleItem:'ThickArmour(M)',femaleItem:'ThickArmour(F)',count:1},{item:'SolidGreatAxe',count:1}],
    Wizard:[{maleItem:'FireMagicRobe(M)',femaleItem:'FireMagicRobe(F)',count:1},{item:'SolidBronzeStaff',count:1}],
    Taoist:[{maleItem:'TaoArmour(M)',femaleItem:'TaoArmour(F)',count:1},{item:'SolidSerpentSword',count:1}],
  }},
];

test('authoring reproduces deterministic milestone rewards and updated chapter summaries',()=>{
  assert.deepEqual(config.milestoneRewards,expectedMilestoneRewards);
  assert.deepEqual(
    config.chapters.slice(2,5).map(chapter=>chapter.rewardSummary),
    [
      'A helmet plus a level-15 class weapon and core combat book.',
      'Belt and necklace choices plus a level-20 class weapon.',
      'The secret path plus level-25 class armour and a level-26 weapon.',
    ],
  );
  const expectedItems={
    SharpSword:[1196,15,7,3,1],Slaying:[974,15,1,3,1],
    SharpTrident:[1197,15,7,3,1],GreatFireBall:[993,15,2,3,1],
    SharpScimitar:[1198,15,7,3,1],SoulFireBall:[1019,18,4,3,1],Amulet:[712,18,31,3,500],
    MartialSabre:[1216,20,7,3,1],SpearWithHook:[1217,20,7,3,1],KeenKrissSword:[1218,20,7,3,1],
    'ThickArmour(M)':[1221,24,7,1,1],'ThickArmour(F)':[1222,24,7,2,1],
    'FireMagicRobe(M)':[1223,24,7,1,1],'FireMagicRobe(F)':[1224,24,7,2,1],
    'TaoArmour(M)':[1225,24,7,1,1],'TaoArmour(F)':[1226,24,7,2,1],
    SolidGreatAxe:[1240,26,7,3,1],SolidBronzeStaff:[1241,26,7,3,1],SolidSerpentSword:[1242,26,7,3,1],
  };
  for(const [name,expected] of Object.entries(expectedItems)){
    const item=itemsByName.get(name);
    assert.ok(item,`missing milestone item ${name}`);
    assert.deepEqual(
      [item.item_index,item.required_amount,item.required_class,item.required_gender,item.stack_size],
      expected,
      `unexpected authoritative metadata for ${name}`,
    );
  }
  execFileSync(process.execPath,[fileURLToPath(new URL('./build-newcomer-journey-profile.mjs',import.meta.url)),'--check']);
});

test('chapter budgets encode the reviewed 5-7 hour first-day cadence',()=>{
  assert.equal(config.targetPlayMinutes,375);
  assert.deepEqual(config.chapters.map(chapter=>chapter.targetMinutes),[20,25,60,75,90,105]);
  assert.equal(config.chapters.slice(0,2).reduce((sum,chapter)=>sum+chapter.targetMinutes,0),45);
  assert.equal(config.chapters.slice(0,3).reduce((sum,chapter)=>sum+chapter.targetMinutes,0),105);
  assert.equal(config.chapters.slice(3,5).reduce((sum,chapter)=>sum+chapter.targetMinutes,0),165);
});

test('all three routes reach every next level gate without repeatables, daily waits or optional harvest',()=>{
  const totals=[];
  for(const className of ['Warrior','Wizard','Taoist']){
    const source=buildClassQuestRoute(sources,{className,maxLevel:40});
    const route=annotateNewcomerRoute(source,guidance,config);
    const byId=new Map(route.quests.map(q=>[q.questId,q]));
    const done=new Set(); let experience=0;
    for(const chapter of config.chapters){
      const before=experience;
      for(const id of [...chapter.questIds,...chapter.classQuestIds[className]]){
        const q=byId.get(id); assert.ok(q,`${className} missing ${id}`);
        assert.ok(!done.has(id),`duplicate main quest ${id}`);
        assert.equal(q.eligibility.questType,0,`main quest ${id} must be once-only`);
        assert.ok(experience>=threshold(q.eligibility.minLevel),`${className} level gate before ${id}: ${experience}`);
        assert.ok(!q.eligibility.requiredQuestId||done.has(q.eligibility.requiredQuestId),`missing predecessor of ${id}`);
        assert.ok(Number.isSafeInteger(q.rewards.experience)&&q.rewards.experience>=0);
        experience+=q.rewards.experience;done.add(id);
      }
      assert.equal(experience-before,chapter.questExperienceBudget);
      assert.ok(experience>=threshold(chapter.exitLevel));
    }
    assert.ok(!done.has(4));
    assert.ok(!done.has(84), 'higher-risk thread hunt must remain optional');
    for(const boss of [58,100,101,115,116])assert.ok(!done.has(boss));
    assert.equal(done.size,55,`${className} mandatory quest count`);
    assert.equal(experience,threshold(30));totals.push(experience);
  }
  assert.equal(new Set(totals).size,1);
});

test('the shortened snake hunt follows the funding quests within a deterministic XP budget',()=>{
  const bichon=config.chapters.find(chapter=>chapter.id==='bichon');
  const frontier=config.chapters.find(chapter=>chapter.id==='frontier');
  assert.ok(bichon.questIds.indexOf(27)<bichon.questIds.indexOf(35));
  assert.ok(bichon.questIds.indexOf(35)<bichon.questIds.indexOf(36));
  assert.ok(bichon.questIds.indexOf(36)<bichon.questIds.indexOf(37));
  assert.ok(!bichon.questIds.includes(33));
  assert.ok(bichon.questIds.indexOf(29) < bichon.questIds.indexOf(30));
  assert.ok(bichon.questIds.indexOf(30) < bichon.questIds.indexOf(35));
  assert.ok([35,36,37].every(questId=>!frontier.questIds.includes(questId)));
  assert.deepEqual(frontier.questIds,[33,49,39,40,41,42]);
  const mines=config.chapters.find(chapter=>chapter.id==='mines');
  assert.ok(mines.questIds.indexOf(54)<mines.questIds.indexOf(61));
  assert.ok(mines.questIds.indexOf(61)<mines.questIds.indexOf(65));
  assert.ok(mines.questIds.indexOf(65)<mines.questIds.indexOf(60));
  const expedition=config.chapters.find(chapter=>chapter.id==='expedition');
  assert.deepEqual(expedition.questIds,[83,86,87,88,97,98,99,102,103,110,111,112,89]);
  assert.ok(expedition.questIds.indexOf(98)<expedition.questIds.indexOf(102));
  assert.ok(expedition.questIds.indexOf(99)<expedition.questIds.indexOf(102));
  assert.ok(expedition.questIds.indexOf(112)<expedition.questIds.indexOf(89));
  assert.equal(bichon.questExperienceBudget,49558);
  assert.equal(frontier.questExperienceBudget,65742);

  const rewards=new Map(config.questOverrides.map(row=>[row.questId,row.rewardExperience]));
  assert.equal(rewards.get(35),12875);
  assert.equal(rewards.get(36),12875);
  assert.equal(rewards.get(37),12875);
  assert.equal(rewards.get(33),1367);
  const snakeOverride=config.questOverrides.find(row=>row.questId===33);
  assert.equal(snakeOverride.killCountCap,3);
  assert.deepEqual(snakeOverride.taskDescription,['Defeat 3 TigerSnake.','Defeat 3 RedSnake.']);

  const route=buildClassQuestRoute(sources,{className:'Warrior',maxLevel:40});
  const serpent=route.quests.find(quest=>quest.questId===35);
  const snakeHunt=route.quests.find(quest=>quest.questId===33);
  assert.equal(serpent.eligibility.minLevel,10);
  assert.equal(serpent.eligibility.requiredQuestId,27);
  assert.equal(snakeHunt.eligibility.requiredQuestId,0);
});

test('material guidance names real sources and the island collection precedes bone hunts',()=>{
  const route=buildClassQuestRoute(sources,{className:'Warrior',maxLevel:40});
  for(const override of config.questOverrides){
    const quest=route.quests.find(q=>q.questId===override.questId);
    if(!quest)continue;
    const hint=guidance.quests.find(q=>q.id===override.questId).hint;
    for(const name of override.guaranteedQuestDrops??[]){
      const objective=quest.objectives.item.find(t=>t.itemName===name);
      assert.ok(objective.sources.some(s=>hint.includes(s.monsterName)),`missing source hint ${override.questId}/${name}`);
      if(objective.sources.some(s=>s.requiresHarvest))assert.match(hint,/harvest|skinning/i);
    }
  }
  const ids=config.chapters.at(-1).questIds;
  assert.ok(ids.indexOf(121)<ids.indexOf(124)&&ids.indexOf(124)<ids.indexOf(122));
});

test('caps and guarantees reference real objectives while preserving the baseline route',()=>{
  assert.equal(new Set(config.questOverrides.map(row=>row.questId)).size,config.questOverrides.length);
  for(const className of ['Warrior','Wizard','Taoist']){
    const source=buildClassQuestRoute(sources,{className,maxLevel:40});
    const original=JSON.stringify(source);
    const route=annotateNewcomerRoute(source,guidance,config);
    assert.equal(JSON.stringify(source),original);
    for(const q of route.quests){
      const override=q.newcomerOverride;if(!override)continue;
      for(const name of override.guaranteedQuestDrops??[])assert.ok(q.objectives.item.some(t=>t.itemName===name),`unrelated guarantee ${q.questId}/${name}`);
      assert.ok(q.objectives.item.every(t=>t.count>0&&t.count<=1));
      assert.ok(q.objectives.kill.every(t=>t.count>0&&t.count<=3));
    }
    const meat=route.quests.find(q=>q.questId===4);
    assert.equal(meat.objectives.item[0].count,1);
    assert.deepEqual(meat.newcomerOverride.guaranteedQuestDrops,['DeerMeat']);
    assert.equal(source.quests.find(q=>q.questId===4).objectives.item[0].count,5);
    assert.equal(source.quests.find(q=>q.questId===61).eligibility.requiredQuestId,58);
    assert.equal(route.quests.find(q=>q.questId===61).eligibility.requiredQuestId,54);
  }
});

test('the full route has a bounded and class-neutral action budget',()=>{
  const expectedKills=[15,3,18,18,24,36];
  const expectedItems=[1,6,0,2,1,1];
  for(const className of ['Warrior','Wizard','Taoist']){
    const route=annotateNewcomerRoute(buildClassQuestRoute(sources,{className,maxLevel:40}),guidance,config);
    const byId=new Map(route.quests.map(quest=>[quest.questId,quest]));
    const totals=config.chapters.map(chapter=>{
      const quests=[...chapter.questIds,...chapter.classQuestIds[className]].map(id=>byId.get(id));
      return {
        kills:quests.reduce((sum,quest)=>sum+quest.objectives.kill.reduce((subtotal,task)=>subtotal+task.count,0),0),
        items:quests.reduce((sum,quest)=>sum+quest.objectives.item.reduce((subtotal,task)=>subtotal+task.count,0),0),
      };
    });
    assert.deepEqual(totals.map(total=>total.kills),expectedKills,`${className} kill budget`);
    assert.deepEqual(totals.map(total=>total.items),expectedItems,`${className} material budget`);
    assert.equal(totals.reduce((sum,total)=>sum+total.kills,0),114);
    assert.equal(totals.reduce((sum,total)=>sum+total.items,0),11);
    assert.deepEqual(byId.get(62).objectives.kill.map(task=>task.count),[2,2]);
    assert.deepEqual(byId.get(89).objectives.kill.map(task=>task.count),[3,3,3]);
  }
});

test('journey locations distinguish source NPCs from diary accept and finish actions',()=>{
  for(const className of ['Warrior','Wizard','Taoist']){
    const route=buildClassQuestRoute(sources,{className,maxLevel:40});
    for(const override of config.questOverrides){
      const quest=route.quests.find(q=>q.questId===override.questId);
      if(!quest)continue;
      for(const [npcField,diaryField,handler] of [['startNpc','startInDiary','quest-diary-accept'],['finishNpc','finishInDiary','quest-diary-finish']]){
        assert.equal(override[diaryField],quest.specialHandlers.includes(handler));
        if(override[diaryField])assert.equal(override[npcField],null);
        else {
          const location=override[npcField],npc=quest[npcField];
          assert.equal(location.name,npc.name.replaceAll('_',' '));
          assert.equal(location.mapFileName,npc.mapFileName);
          assert.equal(location.x,npc.position.x);assert.equal(location.y,npc.position.y);
          const map=sources.respawnManifest.maps.find(m=>m.map_file_name===npc.mapFileName);
          assert.equal(location.mapName,map.map_title);
        }
      }
    }
  }
});
