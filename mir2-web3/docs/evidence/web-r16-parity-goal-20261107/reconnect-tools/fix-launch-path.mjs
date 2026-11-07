import assert from 'node:assert/strict';
import {readFile,writeFile} from 'node:fs/promises';
const file='D:/mir2-builds/r16-reconnect-browser-20261108/launch-realm.mjs';
const source=await readFile(file,'utf8');assert.equal(source.split('\n').filter(line=>line.startsWith('const executable=')).length,1);
await writeFile('D:/mir2-builds/r16-reconnect-integration-20261107/launch-realm-before-path-fix.mjs',source,{flag:'wx'});
await writeFile(file,source.replace(/^const executable=.*;$/m,'const executable="D:/mir2-builds/r16-reconnect-integration-20261107/mir2-gateway-r16-reconnect.exe";'));
console.log(JSON.stringify({testLauncherPathFixed:true,noPrivateDataOrProductSourceChanged:true}));
