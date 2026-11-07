import assert from 'node:assert/strict';
import {copyFile,readFile,writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
const base=import.meta.dirname,source='E:/mir2-player-journey/mir2-web3/apps/web',stage=base+'/apps/web';
const files=['app/page.tsx','app/original-client-shell.tsx','app/components/original-client-group-window.tsx','lib/group-operations.ts','lib/stage5-window-adapters.ts','lib/use-window-keydown.ts'];
const sha=bytes=>createHash('sha256').update(bytes).digest('hex'),pins={};
for(const file of files){const bytes=await readFile(source+'/'+file);await copyFile(source+'/'+file,stage+'/'+file);assert.equal(sha(await readFile(stage+'/'+file)),sha(bytes));pins[file]=sha(bytes);}
const label=process.argv[2]??'current';assert(/^[a-z0-9-]+$/.test(label));
await writeFile(base+'/source-'+label+'-pins.json',JSON.stringify({schema:'mir2.r16.group.source.v1',pins,createdAt:new Date().toISOString()},null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({synced:true,files:files.length}));
