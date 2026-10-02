import {spawnSync} from 'node:child_process';
import {openSync,closeSync} from 'node:fs';
import {resolve} from 'node:path';
const directory='mir2-web3/apps/game-client/platform-android/target/quest-exit-v21-20261002/';
const name=process.argv[2];
if(!['guard-red','guard-green'].includes(name))throw Error('Invalid bounded focus label');
const fd=openSync(directory+name+'.log','wx');
const result=spawnSync('cargo',['+1.95.0','test','--manifest-path',
 'mir2-web3/apps/game-client/platform-android/Cargo.toml','--lib','--features','ui-preview',
 '--locked','--offline','npc_received_preview_releases_actual_shared_action_guard_after_dialog_exit',
 '--','--test-threads=1'],{env:{...process.env,CARGO_TARGET_DIR:resolve('mir2-web3/apps/game-client/platform-android/target/shared-sync-build-cache')},
 stdio:['ignore',fd,fd],timeout:600000});
closeSync(fd);
console.log(JSON.stringify({name,status:result.status}));
process.exit(result.status??1);
