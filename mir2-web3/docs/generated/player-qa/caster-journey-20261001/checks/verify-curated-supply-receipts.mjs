import fs from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
import { pathToFileURL } from 'node:url';
const [repository, evidence] = process.argv.slice(2);
const { v2SupplyStock } = await import(pathToFileURL(path.join(repository, 'apps/web/scripts/quest-agent/newcomer-v2-supply-policy.mjs')).href);
const source = JSON.parse(await fs.readFile(path.join(evidence, 'cohort-r2-supply-receipts.json'),'utf8'));
const expected = new Map([
  ['Wizard/level9-hp-real-purchase',{pool:'hp',quantity:1,cost:40}],
  ['Taoist/level9-hp-real-purchase',{pool:'hp',quantity:1,cost:40}],
  ['Wizard/level15-hp-real-restock',{pool:'hp',quantity:22,cost:880}],
  ['Wizard/level24-town-real-restock',{pool:'townTeleport',quantity:1,cost:1000}],
  ['Taoist/level19-hp-real-restock',{pool:'hp',quantity:24,cost:2640}],
  ['Taoist/level22-amulet-real-restock',{pool:'amulet',quantity:98,cost:2450}],
  ['Taoist/level22-hp-real-restock',{pool:'hp',quantity:24,cost:2640}],
]);
const results=[];
for(const item of source.cases){
  const wanted=expected.get(item.className+'/'+item.case);
  if(!wanted)continue;
  const frames=item.events.filter(event=>event.type==='worldSnapshot');
  const first=frames[0],last=frames.at(-1);
  const before=v2SupplyStock(first.payload,{now:Date.parse(first.at)});
  const after=v2SupplyStock(last.payload,{now:Date.parse(last.at)});
  const lost=item.events.filter(event=>event.packet==='LoseGold').reduce((sum,event)=>sum+Number(event.payload.gold),0);
  const purchases=item.events.filter(event=>event.direction==='sent'&&event.type==='buyItem');
  const gained=item.events.filter(event=>event.packet==='GainedItem');
  assert.equal(after[wanted.pool]-before[wanted.pool],wanted.quantity,item.className+'/'+item.case+' actual carried stock');
  assert.equal(first.payload.gold-last.payload.gold,wanted.cost,item.className+'/'+item.case+' actual wallet');
  assert.equal(lost,wanted.cost,item.className+'/'+item.case+' LoseGold');
  assert.equal(purchases.length,gained.length,item.className+'/'+item.case+' normal purchase receipts');
  results.push({className:item.className,case:item.case,firstSequence:first.sequence,lastSequence:last.sequence,
    pool:wanted.pool,before:before[wanted.pool],after:after[wanted.pool],paidGold:lost,passed:true});
}
assert.equal(results.length,7);
const report={schema:1,scope:'read-only verification of curated ordinary purchases',passed:true,cases:results,
  visualAccepted:false,rewardItemsNotCountedAsPurchases:true};
await fs.writeFile(path.join(evidence,'cohort-r2-purchases-verification.json'),JSON.stringify(report,null,2)+'\n');
console.log(JSON.stringify({passed:true,cases:results.length}));
