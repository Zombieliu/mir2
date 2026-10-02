import assert from 'node:assert/strict';
import {readFileSync, writeFileSync} from 'node:fs';
import {execFileSync} from 'node:child_process';
const root = 'mir2-web3/apps/game-client/';
const gameplayPath = root + 'platform-windows/src/gameplay_bridge.rs';
const gatewayPath = root + 'platform-windows/src/gateway.rs';
const gameplay = readFileSync(gameplayPath, 'utf8');
const gateway = readFileSync(gatewayPath, 'utf8');
const blocks = [];
function extract(source, name) {
  const start = source.indexOf(`fn ${name}(`);
  assert(start >= 0, name);
  const end = source.indexOf('\n}\n', start);
  assert(end > start, name);
  const block = source.slice(start, end + 3);
  assert.equal(source.split(block).length, 2, name);
  blocks.push({name, original: block});
  return block;
}
const definitionStart = gameplay.indexOf('#[derive(Debug, Clone, Default)]\nstruct QuestDefinition {');
const definitionEnd = gameplay.indexOf('\n}\n', definitionStart) + 3;
assert(definitionStart > 0 && definitionEnd > definitionStart);
const definition = gameplay.slice(definitionStart, definitionEnd);
const questNames = ['parse_quest_definition', 'transform_quest_tracker', 'transform_quest_objectives',
  'parse_quest_rewards', 'transform_npc_dialog', 'transform_nearby_npcs', 'quest_status',
  'completed_quest_ids', 'completed_quest_ids_from_snapshot'];
const helperNames = ['value_u32', 'value_i32', 'string_at', 'string_array', 'tile_distance',
  'display_npc_name', 'strip_crystal_markup'];
const questBlocks = questNames.map(name => extract(gameplay, name));
const helperBlocks = helperNames.map(name => extract(gameplay, name));
const rewardNames = ['crystal_wire_item_info', 'crystal_tooltip_source_for_preview', 'add_quest_reward_tooltip_sources'];
const rewardBlocks = rewardNames.map(name => extract(gateway, name));
const header = `//! Shared Windows/Android authoritative quest and NPC presentation projection.\n//! Pure functions extracted from frozen Windows3d735745f. No quest progress,\n//! reward, inventory, script, authentication or save authority.\n+use std::collections::HashMap;\n+use serde_json::{json, Value};\n+use crate::inventory::{CrystalItemInfoModel, CrystalItemTooltipSourceModel, CrystalUserItemModel};\n+use crate::native_player_ingress::NativeUiPlayerCursor;\n+use crate::native_inventory_ingress::{native_tooltip_viewer as crystal_tooltip_viewer,\n+    native_real_tooltip_info as crystal_real_tooltip_info, native_tooltip_info as unique_crystal_tooltip_info};\n+use crate::quest_model::{Quest, QuestDetailText, QuestObjective, QuestReward, QuestStatus,\n+    QuestTracker, CompletedQuestTracker, NpcDialogModel, NpcDialogOption, NpcDialogUpdate, NearbyNpc, NearbyNpcModel};\n+const MAX_NEARBY_NPCS: usize = 8;\n+const MAX_NEARBY_DISTANCE: u32 = 18;\n+`.replace(/^\+/gm, '');
const publicDefinition = definition.replace('struct QuestDefinition', 'pub struct QuestDefinition')
  .replace(/^    (\w+:)/gm, '    pub $1');
const shared = header + '\n' + publicDefinition + '\n' + [...questBlocks, ...rewardBlocks]
  .map(block => block.replace(/^fn /, 'pub fn ')).join('\n') + '\n' + helperBlocks.join('\n');
let nextGameplay = gameplay.replace(definition, '');
for (const block of questBlocks) nextGameplay = nextGameplay.replace(block, '');
nextGameplay = nextGameplay.replace('use crate::gateway::GatewayCommand;',
  `use mir2_client_bevy::native_quest_ingress::{QuestDefinition, ${questNames.join(', ')}};\n\nuse crate::gateway::GatewayCommand;`);
let nextGateway = gateway;
for (const block of rewardBlocks) nextGateway = nextGateway.replace(block, '');
nextGateway = nextGateway.replace('use mir2_client_bevy::native_chat_ingress::transform_chat_line;',
  `use mir2_client_bevy::native_quest_ingress::{${rewardNames.join(', ')}};\nuse mir2_client_bevy::native_chat_ingress::transform_chat_line;`);
function replacePatch(file, before, after) {
  return `*** Update File: ${file}\n@@\n` + before.split('\n').slice(0, -1).map(l => '-' + l).join('\n') + '\n'
    + after.split('\n').slice(0, -1).map(l => '+' + l).join('\n') + '\n';
}
const patch = '*** Begin Patch\n*** Add File: ' + root + 'client-bevy/src/native_quest_ingress.rs\n'
  + shared.trimEnd().split('\n').map(l => '+' + l).join('\n') + '\n'
  + replacePatch(gameplayPath, gameplay, nextGameplay)
  + replacePatch(gatewayPath, gateway, nextGateway) + '*** End Patch\n';
writeFileSync(root + 'platform-android/target/quest-ingress-20261002/extraction-blocks.json', JSON.stringify(blocks, null, 2) + '\n');
console.log(execFileSync('apply_patch', [], {input: patch, encoding: 'utf8', maxBuffer: 1024 * 1024}));
