import {spawnSync} from 'node:child_process';
import {openSync,closeSync} from 'node:fs';
import {resolve,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
const name=process.argv[2];
if(!['quest-reflow-red','quest-reflow-green','quest-reflow-green-2','quest-reflow-final','quest-reflow-final-2'].includes(name))throw Error('Invalid focus label');
const fd=openSync(dirname(fileURLToPath(import.meta.url))+'/'+name+'.log','wx');
const run=spawnSync('cargo',['+1.95.0','test','--manifest-path',
 'mir2-web3/apps/game-client/client-bevy/Cargo.toml','--lib','--features','native-player-ui',
 '--locked','--offline','quest_ui::phone::tests',
 '--','--test-threads=1'],{env:{...process.env,CARGO_TARGET_DIR:resolve('mir2-web3/apps/game-client/platform-android/target/shared-sync-build-cache')},
 stdio:['ignore',fd,fd],timeout:600000});
closeSync(fd);console.log(JSON.stringify({name,status:run.status}));process.exit(run.status??1);
