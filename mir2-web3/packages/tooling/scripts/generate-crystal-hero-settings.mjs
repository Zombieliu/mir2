/** Source: Settings.LoadHeroBaseStats/LoadHeroEXP; no player-stat fallback. */
import {readFileSync, writeFileSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {resolve, join} from 'node:path';
import {pathToFileURL} from 'node:url';
export const HERO_CLASSES = ['Warrior','Wizard','Taoist','Assassin','Archer'];
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
export function parseIni(bytes) {
  const sections = new Map(); let current = new Map();
  for (const raw of bytes.toString('utf8').replace(/^\uFEFF/,'').split(/\r?\n/)) {
    const line=raw.trim(); if (!line || /^[;#]/.test(line)) continue;
    if (line.startsWith('[') && line.endsWith(']')) {
      const key=line.slice(1,-1).trim().toLowerCase();
      current=sections.get(key) ?? new Map(); sections.set(key,current); continue;
    }
    const i=line.indexOf('='); if(i>=0) current.set(line.slice(0,i).trim().toLowerCase(),line.slice(i+1).trim());
  }
  return sections;
}
function integer(value, label, min=-2147483648n, max=2147483647n) {
  if(!/^[+-]?\d+$/.test(value)) throw Error(`Invalid ${label}: ${value}`);
  const n=BigInt(value); if(n<min || n>max) throw Error(`Out of range ${label}: ${value}`); return n;
}
function float(value,label) {
  if(!/^[+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?$/.test(value))throw Error(`Invalid ${label}: ${value}`);
  const n=Math.fround(Number(value));if(!Number.isFinite(n))throw Error(`Nonfinite ${label}`);return n;
}
export function parseStatIds(bytes) {
  const body=bytes.toString('utf8').match(/public enum Stat : byte\s*\{([\s\S]*?)\}/)?.[1];
  if(!body)throw Error('Original Stat enum missing');
  const entries=[...body.matchAll(/(\w+)\s*=\s*(\d+)\s*(?:,|$)/gm)].map(m=>[m[1],Number(m[2])]);
  if(!entries.length || new Set(entries.map(e=>e[1])).size!==entries.length || entries.some(e=>e[1]>255))throw Error('Invalid original Stat enum');
  return entries.sort((a,b)=>a[1]-b[1]);
}
export function parseHeroClass(bytes, job, statIds) {
  const ini=parseIni(bytes), stats=[],caps=[];
  for(const [name,stat] of statIds) {
    const section=ini.get(name.toLowerCase()); const formula=section?.get('formula');
    if(formula) {
      const formula_type=['health','mana','weight','stat'].indexOf(formula.toLowerCase());
      if(formula_type<0)throw Error(`Unknown Hero ${job} ${name} formula: ${formula}`);
      stats.push({stat,formula_type,base:Number(integer(section.get('base')??'0','Base')),
        gain:float(section.get('gain')??'0','Gain'),gain_rate:float(section.get('gainrate')??'0','GainRate'),
        max:Number(integer(section.get('max')??'0','Max'))});
    }
    const value=Number(integer(ini.get('caps')?.get(name.toLowerCase())??'0','Cap'));
    if(value!==0)caps.push({stat,value});
  }
  return {job,stats,caps};
}
export function parseHeroExperience(bytes) {
  const section=parseIni(bytes).get('exp'); let previous=100n; const values=[];
  for(let level=1;level<=500;level++) {
    const raw=section?.get(`level${level}`);
    if(raw!==undefined)previous=integer(raw,`Hero experience Level${level}`,0n,9223372036854775807n);
    values.push(previous.toString());
  }
  return values;
}
export function parseHeroRules(heroBytes,setupBytes) {
  const hero=parseIni(heroBytes).get('hero')??new Map(), items=parseIni(setupBytes).get('items')??new Map();
  const byte=(map,key,fallback)=>Number(integer(map.get(key)??String(fallback),key,0n,255n));
  const boolean=(key,fallback=true)=>{const raw=hero.get(key);if(raw===undefined)return fallback;if(!/^(true|false)$/i.test(raw))throw Error(`Invalid ${key}: ${raw}`);return raw.toLowerCase()==='true';};
  return {allow_new:boolean('allownewhero'),minimum_level:byte(hero,'minimumlevel',22),maximum_count:byte(hero,'maximumcount',9),
    can_create_classes:HERO_CLASSES.map(job=>boolean('cancreate'+job.toLowerCase())),seal_item_name:hero.get('sealitemname')??'SealedHero',
    maximum_seal_count:Number(integer(hero.get('maximumsealcount')??'5','maximumsealcount',0n,65535n)),
    max_luck:byte(items,'maxluck',10),fire_ring_spell:items.get('firering')??'FireBall',heal_ring_spell:items.get('healring')??'Healing',blink_spell:items.get('blinkskill')??'Blink'};
}
export function generateHeroSettings(crystalRoot, output, check=false) {
  const sources=[];
  const read=relative=>{const bytes=readFileSync(join(crystalRoot,relative));sources.push({path:'Crystal/'+relative,sha256:digest(bytes)});return bytes;};
  const ids=parseStatIds(read('Shared/Data/Stat.cs'));
  read('Shared/BaseStats.cs');read('Server/Settings.cs');
  const classes=HERO_CLASSES.map(job=>parseHeroClass(read(`Build/Server/Debug/Configs/HeroBaseStats${job}.ini`),job,ids));
  const experience=parseHeroExperience(read('Build/Server/Debug/Configs/HeroExpList.ini'));
  const rules=parseHeroRules(read('Build/Server/Debug/Configs/HeroSettings.ini'),read('Build/Server/Debug/Configs/Setup.ini'));
  const encoded=JSON.stringify({sources,classes,experience,rules},null,2)+'\n';
  if(check){if(readFileSync(output,'utf8')!==encoded)throw Error('Hero settings differ from source');}
  else writeFileSync(output,encoded);
  return {sources,classes,experience,rules};
}
if(process.argv[1] && import.meta.url===pathToFileURL(resolve(process.argv[1])).href) {
  const root=resolve(import.meta.dirname,'../../..');const args=process.argv.slice(2);const positional=args.filter(a=>a!=='--check');
  if(positional.length>2 || positional.some(a=>a.startsWith('--')))throw Error('Usage: [Crystal root] [output.json] [--check]');
  const output=positional[1]??join(root,'packages/game-data/data/generated/crystal_hero_settings.json');
  generateHeroSettings(positional[0]??resolve(root,'../Crystal'),output,args.includes('--check'));
  console.log(`Hero settings ${args.includes('--check')?'verified':'generated'}: ${output}`);
}
