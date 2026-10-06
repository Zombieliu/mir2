// Frozen-source equality, not an APK, Windows-device or authenticated NPC test.
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {readFileSync, writeFileSync} from 'node:fs';
import {dirname, join} from 'node:path';
import {fileURLToPath} from 'node:url';

const qa = dirname(fileURLToPath(import.meta.url));
const frozen = '3d735745f1117d42a7859e87604a106351dca935';
const hostPath = 'mir2-web3/apps/game-client/platform-windows/src/gateway.rs';
const inventoryPath = 'mir2-web3/apps/game-client/client-bevy/src/native_inventory_ingress.rs';
const npcPath = 'mir2-web3/apps/game-client/client-bevy/src/native_npc_ingress.rs';
const selected = process.argv[2] ?? null;
function run(command, args, input) {
  const result = spawnSync(command, args, {input, encoding: 'utf8', maxBuffer: 8 * 1024 * 1024});
  assert.equal(result.status, 0, `${command} failed: ${result.error ?? result.stderr}`);
  return result.stdout;
}
const git = (...args) => run('git', args).trim();
const current = path => selected ? run('git', ['show', `${selected}:${path}`]) : readFileSync(path, 'utf8');
const source = run('git', ['show', `${frozen}:${hostPath}`]);
const host = current(hostPath);
const shared = {[inventoryPath]: current(inventoryPath), [npcPath]: current(npcPath)};
const sha256 = value => createHash('sha256').update(value).digest('hex');
const helpers = [
  ['crystal_tooltip_viewer', 'native_tooltip_viewer', inventoryPath],
  ['unique_crystal_tooltip_info', 'native_tooltip_info', inventoryPath],
  ['crystal_real_tooltip_info', 'native_real_tooltip_info', inventoryPath],
  ['crystal_tooltip_source_for_user_item', 'native_tooltip_source_for_user_item', inventoryPath],
  ...['npc_shop_service_from_packet', 'transform_shop_model_from_packet',
    'transform_shop_model_from_snapshot', 'shop_good_json', 'shop_hide_added_stats',
    'transform_npc_catalog_packet', 'try_transform_shop_model_from_packet',
    'payload_has_valid_shop_array'].map(name => [name, name, npcPath]),
];
function extract(text, name) {
  const start = text.search(new RegExp(`\\bfn ${name}\\s*\\(`));
  assert(start >= 0, `missing ${name}`);
  const begin = text.indexOf('{', start);
  let end = begin + 1, depth = 1;
  for (; end < text.length && depth; end++) {
    if (text[end] === '{') depth++;
    if (text[end] === '}') depth--;
  }
  assert.equal(depth, 0, `unbalanced ${name}`);
  return {header: text.slice(start, begin), body: text.slice(begin, end)};
}
const identifiers = [
  ...helpers.map(([oldName, newName]) => [oldName, newName]),
  ['unique_crystal_tooltip_template', 'unique_crystal_item_template'],
  ['crystal_user_item_icon', 'native_user_item_icon'],
  ['native_inventory_frame_geometry', 'geometry'],
];
function normalize(body) {
  for (const [oldName, newName] of identifiers) {
    body = body.replace(new RegExp(`\\b${oldName}\\b`, 'g'), newName);
  }
  body = body.replace(/use mir2_client_bevy::shop::\{NpcShopServiceMode, NpcShopServiceSignal\};/g, '')
    .replace(/mir2_client_bevy::shop::/g, '')
    .replace(/mir2_game_data::crystal_real_item_for_player/g, 'crystal_real_item_for_player')
    .replace(/\.and_then\(item_frame_geometry\)/g,
      '.and_then(|index| geometry(NativeItemLibrary::Items, index))')
    .replace(/,\s*(?:&mut\s+)?geometry(?=\s*\))/g, '')
    .replace(/^\s*\/\/[^\n]*$/gm, '');
  const formatted = run('/Users/henryliu/.cargo/bin/rustfmt',
    ['+1.95.0', '--edition', '2021', '--emit', 'stdout', '--config', 'skip_children=true'],
    `fn projection() ${body}\n`);
  // Rustfmt treats json! tokens as opaque. Ignore only whitespace outside
  // quoted literals; every key/string value remains byte-for-byte compared.
  const literals = [];
  const tokens = formatted.replace(/"(?:\\.|[^"\\])*"/g, literal => {
    literals.push(literal);
    return `__AUDIT_LITERAL_${literals.length - 1}__`;
  }).replace(/\s+/g, '');
  return JSON.stringify({tokens, literals});
}
const entries = helpers.map(([oldName, newName, path]) => {
  const old = extract(source, oldName), next = extract(shared[path], newName);
  const oldNormalized = normalize(old.body), newNormalized = normalize(next.body);
  assert.equal(newNormalized, oldNormalized, `${oldName} changed frozen behavior`);
  const wrapper = extract(host, oldName).body;
  const module = path === inventoryPath ? 'native_inventory_ingress' : 'native_npc_ingress';
  assert(wrapper.includes(`mir2_client_bevy::${module}::${newName}(`), `missing delegation ${oldName}`);
  assert.equal(wrapper.trim().split('\n').length, 3, `non-thin wrapper ${oldName}`);
  return {frozenFunction: oldName, sharedFunction: newName, sharedPath: path,
    frozenHeader: old.header.trim(), sharedHeader: next.header.trim(),
    frozenBodySha256: sha256(old.body), sharedBodySha256: sha256(next.body),
    normalizedSha256: sha256(oldNormalized), bodyEqual: true, thinWindowsDelegation: true};
});
const result = {scope: '12 frozen Windows pure projection bodies and thin host delegates only',
  frozenWindows: frozen, selectedSource: selected ? git('rev-parse', selected) : null,
  sourceParent: selected ? git('rev-parse', `${selected}^1`) : git('rev-parse', 'HEAD'),
  permittedDifferences: ['helper names', 'shop and game-data imported namespace',
    'host geometry callback injection and forwarding', 'comments and Rustfmt formatting'],
  entries, pass: true, windowsDeviceAccepted: false, authenticatedAndroidAccepted: false,
  completeNpcReceiptsAccepted: false};
writeFileSync(join(qa, 'source-equivalence.json'), `${JSON.stringify(result, null, 2)}\n`);
console.log(JSON.stringify({pass: result.pass, functions: entries.length, frozenWindows: frozen}));
