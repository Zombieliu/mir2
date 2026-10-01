import { spawnSync } from 'node:child_process';
import { openSync, closeSync, writeFileSync, mkdirSync } from 'node:fs';
import { resolve, join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
const qa = join(dirname(fileURLToPath(import.meta.url)), 'final-source');
mkdirSync(qa, { recursive: true });
const android = resolve('mir2-web3/apps/game-client/platform-android');
const env = { ...process.env,
  CARGO_TARGET_DIR: join(android, 'target/shared-sync-build-cache'),
  JAVA_HOME: '/opt/homebrew/opt/openjdk@17/libexec/openjdk.jdk/Contents/Home',
  ANDROID_SDK_ROOT: '/Users/henryliu/Library/Android/sdk',
  ANDROID_NDK_HOME: '/Users/henryliu/Library/Android/sdk/ndk/26.1.10909125',
  MIR2_ANDROID_UI_ASSET_ROOT: join(android, 'target/weight-assets-KtITw2/shared-ui-assets'),
  MIR2_ANDROID_WORLD_ASSET_ROOT: join(android, 'target/shared-sync-build-cache/local-world-20260911-objects'),
  MIR2_ANDROID_ENTITY_ASSET_ROOT: join(android, 'target/shared-sync-build-cache/entity-proof-assets-with-archer-bow-20260912'),
  MIR2_ANDROID_ENTITY_ASSET_PACK_ID: 'android-archer-mount-bow-proof-20260912',
};
const records = [];
function gate(name, command, args, options = {}) {
  const started = new Date().toISOString();
  const fd = openSync(join(qa, name + '.log'), 'wx');
  const result = spawnSync(command, args, { env, stdio: ['ignore', fd, fd],
    timeout: 3_600_000, ...options });
  closeSync(fd);
  const record = { name, command, args, started, finished: new Date().toISOString(),
    exitCode: result.status, error: result.error?.message };
  records.push(record);
  writeFileSync(join(qa, 'gate-commands.json'), JSON.stringify(records, null, 2) + '\n');
  console.log(JSON.stringify(record));
  if (result.status !== 0) throw new Error(`${name} failed; preserve the original log`);
}
const rust = ['+1.95.0', 'test', '--manifest-path', join(android, 'Cargo.toml'),
  '--lib', '--locked', '--offline'];
gate('android-normal-final', 'cargo', [...rust, '--', '--test-threads=1']);
gate('android-preview-final', 'cargo', [...rust, '--features', 'ui-preview', '--', '--test-threads=1']);
gate('java-fresh', './gradlew', ['--no-daemon', '--offline', '--rerun-tasks',
  'testDebugUnitTest', 'testUiPreviewUnitTest'], { cwd: join(android, 'android') });
for (const variant of ['debug', 'uiPreview']) {
  gate(`api31-${variant}`, 'bash', [join(android, 'build-android.sh')], {
    env: { ...env, MIR2_ANDROID_MODE: 'check', MIR2_ANDROID_VARIANT: variant },
  });
}
