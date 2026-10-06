import {spawnSync} from 'node:child_process';
import {openSync,closeSync} from 'node:fs';
import {resolve,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
const name=process.argv[2];
if(!['quest-focus-red','quest-focus-green','quest-focus-green-2'].includes(name))throw Error('Invalid focus label');
const fd=openSync(dirname(fileURLToPath(import.meta.url))+'/'+name+'.log','wx');
const run=spawnSync('cargo',['+1.95.0','test','--manifest-path',
 'mir2-web3/apps/game-client/platform-android/Cargo.toml','--lib','--features','ui-preview',
 '--locked','--offline','phone_focus_expands_the_actual_shared_quest_panel_without_rewriting_it',
 '--','--test-threads=1'],{env:{...process.env,CARGO_TARGET_DIR:resolve('mir2-web3/apps/game-client/platform-android/target/shared-sync-build-cache')},
 stdio:['ignore',fd,fd],timeout:600000});
closeSync(fd);console.log(JSON.stringify({name,status:run.status}));process.exit(run.status??1);
