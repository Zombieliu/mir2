import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync, statSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join, resolve, sep} from 'node:path';
import {createHash} from 'node:crypto';
import {HERO_CLASSES,parseStatIds,parseHeroClass,parseHeroExperience,parseHeroRules,generateHeroSettings} from './generate-crystal-hero-settings.mjs';
const bytes=s=>Buffer.from(s);
test('Hero Int64 XP preserves previous fallback, exact maxima and all 500 rows',()=>{
 const xp=parseHeroExperience(bytes('[Exp]\nLevel1=9223372036854775807\nLevel3=15\nLevel501=9'));
 assert.equal(xp.length,500);assert.equal(xp[1],'9223372036854775807');assert.equal(xp[499],'15');
 assert.equal(parseHeroExperience(bytes(''))[0],'100');
 for(const value of ['9223372036854775808','-1','3.1','NaN'])assert.throws(()=>parseHeroExperience(bytes('[Exp]\nLevel1='+value)));
});
test('Hero source formulas, zero defaults, caps and invalid values are explicit',()=>{
 const ids=parseStatIds(bytes('public enum Stat : byte { HP = 12, MP = 13, Unknown = 255 }'));
 assert.deepEqual(ids,[['HP',12],['MP',13],['Unknown',255]]);
 const value=parseHeroClass(bytes('[HP]\nFormula=hEaLtH\nBase=14\nGain=4\nGainRate=4.5\n[Caps]\nHP=100'),'Warrior',ids);
 assert.deepEqual(value.stats,[{stat:12,formula_type:0,base:14,gain:4,gain_rate:4.5,max:0}]);
 assert.deepEqual(value.caps,[{stat:12,value:100}]);
 assert.equal(parseHeroClass(bytes('[HP]\nFormula=Stat'),'Warrior',ids).stats[0].gain,0);
 for(const content of ['Formula=oops','Formula=Stat\nGain=Infinity','Formula=Stat\nGain=1e39','Formula=Stat\nBase=2147483648'])assert.throws(()=>parseHeroClass(bytes('[HP]\n'+content),'Warrior',ids));
});
test('Hero generator pins every source and check never writes or creates output',()=>{
 const temp=mkdtempSync(join(tmpdir(),'mir2-hero-settings-'));
 try {
  for(const dir of ['Shared/Data','Server','Build/Server/Debug/Configs'])mkdirSync(join(temp,dir),{recursive:true});
  const source='public enum Stat : byte { HP = 12, MP = 13 }';writeFileSync(join(temp,'Shared/Data/Stat.cs'),source);
  writeFileSync(join(temp,'Shared/BaseStats.cs'),'source formula evidence');writeFileSync(join(temp,'Server/Settings.cs'),'source loader evidence');
  for(const job of HERO_CLASSES)writeFileSync(join(temp,`Build/Server/Debug/Configs/HeroBaseStats${job}.ini`),'[HP]\nFormula=Health\nBase=14\nGain=4');
  writeFileSync(join(temp,'Build/Server/Debug/Configs/HeroExpList.ini'),'[Exp]\nLevel1=5');
  writeFileSync(join(temp,'Build/Server/Debug/Configs/HeroSettings.ini'),'[Hero]\nMaximumCount=1');
  writeFileSync(join(temp,'Build/Server/Debug/Configs/Setup.ini'),'[Items]\nMaxLuck=10');
  const out=join(temp,'out.json');const generated=generateHeroSettings(temp,out);
  assert.equal(generated.sources.length,11);assert.equal(generated.sources[0].sha256,createHash('sha256').update(source).digest('hex'));
  const original=readFileSync(out);const mtime=statSync(out).mtimeMs;generateHeroSettings(temp,out,true);assert.equal(statSync(out).mtimeMs,mtime);
  writeFileSync(join(temp,'Build/Server/Debug/Configs/HeroExpList.ini'),'[Exp]\nLevel1=6');
  assert.throws(()=>generateHeroSettings(temp,out,true));assert.deepEqual(readFileSync(out),original);
  assert.throws(()=>generateHeroSettings(temp,join(temp,'missing.json'),true));
 }finally{const target=resolve(temp);assert.ok(target.startsWith(resolve(tmpdir())+sep)&&target.includes('mir2-hero-settings-'));rmSync(target,{recursive:true,force:true});}
});

test('Hero rules preserve source defaults and exact byte/ushort domains',()=>{
 const fallback=parseHeroRules(bytes(''),bytes(''));
 assert.equal(fallback.maximum_count,9);assert.equal(fallback.max_luck,10);assert.equal(fallback.maximum_seal_count,5);
 assert.deepEqual(fallback.can_create_classes,[true,true,true,true,true]);
 const value=parseHeroRules(bytes('[Hero]\nCanCreateArcher=False\nMaximumCount=1\nMaximumSealCount=65535'),bytes('[Items]\nMaxLuck=0'));
 assert.equal(value.maximum_count,1);assert.equal(value.can_create_classes[4],false);assert.equal(value.max_luck,0);
 for(const input of ['MinimumLevel=-1','MaximumCount=256','MaximumSealCount=65536','AllowNewHero=perhaps'])assert.throws(()=>parseHeroRules(bytes('[Hero]\n'+input),bytes('')));
});
