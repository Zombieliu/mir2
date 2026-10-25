import fs from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
const base='C:/mir2-build/p01-completion-20261024';
let text=await fs.readFile(path.join(base,'check-compact-wire-formal10-preseal-v2.mjs'),'utf8');
for(const [before,after]of[['compact-wire-formal10-preseal-input-check-v2.json','compact-wire-formal10-preseal-input-check-v3.json'],['compact-wire-formal10-schema-normalization-v2.json','compact-wire-formal10-schema-normalization-v3.json']]){
  assert.equal(text.split(before).length-1,1);text=text.replace(before,after);
}
const anchor="const file=path.join(base,'compact-wire-formal10-preseal-input-check-v3.json');";
assert.equal(text.split(anchor).length-1,1);
text=text.replace(anchor,"result.hpSchemaSynthetic=await pin(path.join(base,'compact-wire-formal10-hp-schema-synthetic-v3.json'));result.hpSchemaSemantics={finiteStatusEnum:true,nonnegativeSafeIntegerCounts:true,emptyDictionaryAllowed:true,verifiedEventsCheckedEqualsVerifiedCount:true,successProofCheckedMeansPositiveCount:true,noVerifiedDoseObservedMeansZeroCount:true,noFakeGameplayPass:true};"+anchor);
await fs.writeFile(path.join(base,'check-compact-wire-formal10-preseal-v3.mjs'),text,{flag:'wx'});
