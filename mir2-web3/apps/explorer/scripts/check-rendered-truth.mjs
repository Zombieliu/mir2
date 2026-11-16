import assert from 'node:assert/strict';
import {writeFile} from 'node:fs/promises';
import {archiveRecords,facts,locales} from '../lib/atlas-data.mjs';
import {getPolicyLinks,metricOrder} from '../lib/atlas-display.mjs';

const origin=process.argv[2] ?? 'http://127.0.0.1:3090';
const paths=locales.flatMap(locale=>[`/explore/${locale}`,...archiveRecords.map(({kind,slug})=>`/explore/${locale}/archive/${kind}/${slug}`)]);
const failures=[];
let next=0;
let passed=0;
async function check(path){
  const response=await fetch(new URL(path,origin),{signal:AbortSignal.timeout(15000)});
  assert.equal(response.status,200,path);
  const html=await response.text();
  const text=html.replace(/<script\b[^>]*>[\s\S]*?<\/script>/gi,' ').replace(/<style\b[^>]*>[\s\S]*?<\/style>/gi,' ').replace(/<[^>]+>/g,' ');
  assert.ok(!/12,684|18,912|8,912|48,619,204|48\.6M|2,431|7,294|128,000|\b\d+(?:\.\d+)?%|#WR-\s*\d+|RED MOON|\b(?:Mir|Bichon|Sabuk|Verified|LIVE)\b|已验证|確認済み|服务端事件一致|已验证成交/.test(text),`Unsupported display claim: ${path}`);
  assert.ok(!/<(?:meta|link)\b[^>]*(?:content|href)="http:\/\/localhost:3090/.test(html),`Localhost SEO: ${path}`);
  assert.ok(html.includes('https://numeron.obelisk.build/'),`Public canonical missing: ${path}`);
  const locale=path.split('/')[2];
  for (const link of getPolicyLinks(locale)) assert.ok(html.includes(`href="${link.href}"`),`Missing policy/support link: ${path} → ${link.href}`);
  for (const id of metricOrder) assert.ok(html.includes(`data-metric-id="${id}"`),`Missing keyed metric: ${path}/${id}`);
  assert.ok(html.includes(`lang="${locale}"`),`Locale missing: ${path}`);
  assert.ok(text.split(facts[locale][1]).length-1 <= 1,`Full development summary repeated in list rows: ${path}`);
  passed++;
}
await Promise.all(Array.from({length:8},async()=>{while(next<paths.length){const path=paths[next++];try{await check(path);}catch(error){failures.push({path,error:error.message});}}}));
for(const path of ['/explore/not-a-locale','/explore/en/archive/equipment/not-a-record']){
  const response=await fetch(new URL(path,origin));
  if(response.status!==404)failures.push({path,error:`Expected404, got${response.status}`});
}
const result={origin,checkedRoutes:paths.length,passed,failures,sourceScope:'Reconstructed deployed presentation; exact original source unavailable through truncated source API.'};
await writeFile(new URL('../TRUTH-ROUTE-CHECK.json',import.meta.url),JSON.stringify(result,null,2)+'\n');
console.log(JSON.stringify(result,null,2));
assert.equal(failures.length,0);
