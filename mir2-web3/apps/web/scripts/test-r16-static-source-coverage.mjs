import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import test from 'node:test';
import {normalizeStaticResourceSources,staticResourceDescriptor} from './lib/r16-static-waterfall.mjs';
const config=JSON.parse(readFileSync(new URL('../../../config/numeron-r16-web-assets.json',import.meta.url)));
const bases=['https://numeron.obelisk.build',config.assetBaseUrl,...config.browserFallbackBaseUrls];
const sources=normalizeStaticResourceSources(bases);
test('the explicit current R16 configuration has seven distinct bounded observation sources',()=>{
 assert.equal(bases.length,7);assert.equal(new Set(bases).size,7);assert.equal(sources.length,7);
});
test('all seven prefixes retain image, font and map observations, including hotlink and inherited R2',()=>{
 for(const base of bases)for(const[path,category]of [['/original-ui/Prguse/1656.png','ui'],['/original-ui/fonts/NotoSansTC.ttf','font'],['/original-map/Tiles/1.png','map']]){
  const result=staticResourceDescriptor(base+path,sources);assert(result,base);assert.equal(result.url,base+path);assert.equal(result.category,category);
 }
});
test('the five prefixes absent from the old upstream selector are now explicit rather than inferred',()=>{
 const old=normalizeStaticResourceSources(['https://numeron.obelisk.build',config.assetBaseUrl]);
 const path='/original-ui/Prguse2/1202.png';
 assert.equal(bases.filter(base=>!staticResourceDescriptor(base+path,old)).length,5);
 assert(bases.every(base=>staticResourceDescriptor(base+path,sources)));
});
test('auth, signing, unconfigured releases and unrelated origins stay outside the static capture',()=>{
 for(const url of ['https://numeron.obelisk.build/api/dubhe-connect/login?intent=synthetic',
  'https://wallet.dubhe.obelisk.build/approve','https://relay.dubhe.obelisk.build/api/intent',
  'https://assets.mir2.obelisk.build/mir2/v/unapproved/original-ui/Prguse/1.png',
  'https://other.example.test/original-ui/Prguse/1.png'])assert.equal(staticResourceDescriptor(url,sources),null);
});
test('allowed static observation still strips queries and refuses URL credentials',()=>{
 const url=bases[2]+'/original-ui/Prguse/1656.png';assert.equal(staticResourceDescriptor(url+'?temporary=synthetic',sources).url,url);
 assert.equal(staticResourceDescriptor(url.replace('https://','https://user:password@'),sources),null);
});
