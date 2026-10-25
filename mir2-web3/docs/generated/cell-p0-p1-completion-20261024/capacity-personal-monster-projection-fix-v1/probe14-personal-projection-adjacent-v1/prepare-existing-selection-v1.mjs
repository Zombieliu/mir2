import fs from 'node:fs/promises'; import path from 'node:path'; import crypto from 'node:crypto'; import assert from 'node:assert/strict';
const base='C:/mir2-build/p01-completion-20261024',project='C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey/mir2-web3',directory=path.join(base,'probe14-personal-projection-adjacent-v1');
const baselineFile=path.join(base,'socket-diagnostics-adjacent-selection-v1.json'),baselineBytes=await fs.readFile(baselineFile),baseline=JSON.parse(baselineBytes),sha=b=>crypto.createHash('sha256').update(b).digest('hex');
assert.equal(baseline.tests.length,161);assert.equal(new Set(baseline.tests).size,161);assert.equal(baseline.expectedPassed,159);assert.equal(baseline.expectedIgnored,2);
const groups=[
 {file:'apps/gateway/src/routing.rs',module:'routing::tests',tests:[
  ['personal_tick_drops_shared_monster_motion_but_preserves_other_packets','Preserve the existing packet-family split: suppress shared-monster motion while retaining unrelated owner/chat packets.'],
  ['observer_aoi_remove_does_not_tombstone_retained_native_monster_globally','Observer viewport withdrawal must not globally retire a retained authoritative native ID.'],
  ['shared_map_sync_preserves_zone_native_monster_authority','A stale personal/shared-map projection must not reset the Zone monster transform or vitals.'],
  ['shared_gateway_tick_does_not_run_private_monster_authority','Personal Tick still advances compatibility state without producing a second monster authority.'],
  ['shared_zone_state_records_object_monster_spawn_packet','First admission of a genuinely unregistered ObjectMonster still creates the shared entity.'],
  ['shared_zone_seeds_distant_current_map_monsters_for_later_owner_aoi_entry','Newly activated distant private manifest monsters can still be promoted on real AOI entry.'],
  ['shared_zone_state_preserves_owned_generated_entity_owner_during_snapshot_merge','Do not erase owned-pet provenance during the shared snapshot merge.'],
  ['shared_zone_state_save_transform_updates_presence_before_pending_drain','Final authoritative transform must reach saved presence before queued packet drainage.'] ]},
 {file:'apps/gateway/src/routing/tests/private_monster_snapshot_tests.rs',module:'routing::tests::private_monster_snapshot_tests',tests:[
  ['shared_snapshot_omits_buried_zombie_until_real_three_cell_reveal','Keep genuine private burial until the authoritative reveal; admit ObjectMonster before ObjectShow.'],
  ['shared_snapshot_visibility_survives_aoi_leave_rejoin_and_never_reburies_zombie','Preserve the difference between viewport removal, revealed lifetime state and rejoin.'],
  ['shared_snapshot_private_plants_do_not_hide_stone_zuma_or_owned_summon_metadata','Preserve true Hide/Show, public stone-Zuma metadata and owned summon data.'],
  ['shared_snapshot_map_transition_and_global_remove_do_not_resurrect_private_actors','A source-map private actor must not reappear after transition or genuine global removal.'] ]},
 {file:'apps/gateway/src/routing/tests/pending_zone_aoi_tests.rs',module:'routing::tests::pending_zone_aoi_tests',tests:[
  ['town_revive_discards_stale_queued_spawn_and_reentering_aoi_gets_one_fresh_spawn','Drop stale queued creation on town-revive and deliver exactly one current AOI spawn.'],
  ['authoritative_zone_death_discards_queued_live_spawn_and_positive_health','A pending old live spawn/positive health must not resurrect the authoritative corpse.'] ]},
 {file:'apps/gateway/src/routing/tests/live_aoi_order_tests.rs',module:'routing::tests::live_aoi_order_tests',tests:[
  ['live_aoi_order_old_direct_remove_cannot_delete_new_live_reentry','An old direct removal must not overtake a newer live create.'],
  ['live_aoi_order_old_live_create_cannot_resurrect_after_new_direct_remove','An old live create must not overtake a newer direct removal.'],
  ['live_aoi_order_personal_flush_cannot_bypass_backlog_but_owner_location_has_priority','Personal flush retains FIFO while the dedicated owner-location priority remains intact.'],
  ['live_aoi_order_monster_lifecycle_and_personal_rewards_keep_the_existing_direct_path','Zone monster lifecycle and unrelated personal rewards retain the direct outbound path.'] ]},
 {file:'apps/gateway/src/routing/tests/harvest_corpse_expiry_tests.rs',module:'routing::tests::harvest_corpse_expiry_tests',tests:[
  ['expired_shared_harvest_corpse_leaves_gateway_projection_until_scheduled_revive','Corpse expiry cannot let a stale personal live projection bypass the scheduled Zone revive.'],
  ['unobserved_harvest_corpse_expiry_is_reconciled_without_a_remove_recipient','The same corpse lifecycle remains correct with no observing removal recipient.'] ]},
 {file:'apps/gateway/src/routing/tests/incremental_monster_hydration_tests.rs',module:'routing::tests::incremental_monster_hydration_tests',tests:[
  ['async_walk_into_aoi_hydrates_actual_distant_manifest_spawn_on_serialized_drain','Deferred movement into AOI still hydrates and admits the genuine distant manifest spawn.'] ]},
 {file:'apps/gateway/src/routing/tests/zone_journey_event_bridge_tests.rs',module:'routing::tests::zone_journey_event_bridge_tests',tests:[
  ['public_v2_safe_arrival_pending_walk_survives_autosave_before_tick','The personal synchronization path must not lose pending movement or quest progress across autosave.'],
  ['public_v2_pending_safe_zone_snapshot_follows_zone_before_private_tick','Pending Zone movement remains visible to the public quest/safe-zone snapshot before personal Tick.'] ]},
 {file:'apps/gateway/src/routing/tests/personal_monster_projection_tests.rs',module:'routing::tests::personal_monster_projection_tests',tests:[
  ['harvested_zone_corpse_blocks_reactivated_personal_live_projection_until_zone_revive','Retain the pre-existing corpse/Zone ownership rule while adding the new projection red cases.'],
  ['projection_filter_preserves_owner_pet_and_unadmitted_initial_monster','Retain owner-pet and initial unadmitted monster exceptions; this is an existing test, not the new fix coverage.'] ]}
];
const sourceCache=new Map(),additional=[];
for(const group of groups){let source=sourceCache.get(group.file);if(!source){const bytes=await fs.readFile(path.join(project,group.file));source={file:group.file,bytes:bytes.length,sha256:sha(bytes),text:bytes.toString('utf8')};sourceCache.set(group.file,source);}for(const [name,reason]of group.tests){const exact=group.module+'::'+name,needle='fn '+name+'(',index=source.text.indexOf(needle);assert.ok(index>=0,'Selected existing test not present: '+exact);assert.equal(source.text.indexOf(needle,index+1),-1);assert.ok(!baseline.tests.includes(exact),'Already covered by original161: '+exact);additional.push({name:exact,file:group.file,line:source.text.slice(0,index).split('\n').length,reason});}}
assert.equal(additional.length,25);assert.equal(new Set(additional.map(row=>row.name)).size,additional.length);
const reuse=[
 ['routing::tests::movement_deadline_tests::movement_deadline_owner_wakes_without_maintenance_even_with_pending_inputs','Owner movement deadline independent of maintenance.'],
 ['routing::tests::movement_deadline_tests::movement_deadline_maintenance_cost_does_not_drift_and_late_ticks_skip_burst','Cadence deadline does not drift or replay bursts.'],
 ['routing::tests::movement_deadline_tests::movement_deadline_gateway_dispatches_due_move_without_maintenance_and_freeze_suppresses_wake','Actual gateway due movement plus authority fence.'],
 ['routing::tests::owner_health_sync_tests::owner_health_routing_live_queued_level_is_saved_in_actual_fenced_teardown','Live queued authority and actual final-save teardown.'],
 ['routing::tests::zone_journey_event_bridge_tests::zone_journey_event_is_owner_bound_deduplicated_and_checkpointed','Owner-bound quest/journey receipt and checkpoint dedup remain.'],
 ['routing::tests::shared_in_process_registry_routes_monster_attack_through_zone_native_combat','Native Zone combat dispatch remains authoritative.']
].map(([name,reason])=>{assert.ok(baseline.tests.includes(name));return{name,reason,alreadyInBaseline:true};});
const merged=[...baseline.tests,...additional.map(row=>row.name)].sort();assert.equal(new Set(merged).size,186);
const result={schemaVersion:1,preparedAt:new Date().toISOString(),scope:'Read-only bounded existing-test selection for the personal/shared monster projection correction. No Cargo, build, runtime import, realm or test execution.',
 baseline:{file:baselineFile,bytes:baselineBytes.length,sha256:sha(baselineBytes),selectedExactNames:161,historicalExpectedPassed:159,historicalExpectedIgnored:2,historicalEvidenceOnly:true},
 additionalExistingCount:additional.length,additionalExisting:additional,reuseFromBaseline:reuse,mergedExistingCount:merged.length,mergedExistingTests:merged,
 duplicateExactNames:[],intersectionWithBaseline:[],writerNewRedTestsIncluded:false,
 newWriterTests:'Coordinator must append the writer final focused names by set union after source freeze. The two existing personal_monster_projection tests above must not run twice when that module is also selected.',
 sourceReadPins:[...sourceCache.values()].map(({text,...pin})=>pin),sourcePinsProvisional:true,coordinatorMustRefreshPinsAfterWriterFreeze:true,
 expectedNewIgnoredTests:0,noFreshPassingResultClaimed:true,capacityAccepted:false,p2ImplementationPermitted:false};
await fs.writeFile(path.join(directory,'existing-adjacent-selection-v1.json'),JSON.stringify(result,null,2)+'\n',{flag:'wx'});
const lines=['# Existing adjacent selection for the personal monster projection fix','',
 'Reuse the historical161 exact-name selection: its historical expectation was159pass plus2ignored. This is not a fresh passing result. Add the25 exact existing tests below, then union in the writer final new focused tests after source freeze. The existing union is186 unique names; no original baseline name is duplicated.','',
 'The original selection already covers all three movement-deadline cases, actual fenced final-save, the owner-bound journey checkpoint and native Zone combat dispatch. Do not duplicate those runs. The two existing personal-monster projection cases are included here; de-duplicate them if the full focused module is selected.','',
 '| Exact test | Source | Reason |','|---|---|---|',...additional.map(row=>'| `'+row.name+'` | `'+row.file+':'+row.line+'` | '+row.reason+' |'),'','Source hashes and line numbers are read-time references only. The sole source writer is still changing routing/projection tests; the coordinator refreshes final pins and executes all Rust validation. This package adds no tests or game-source changes.',''];
await fs.writeFile(path.join(directory,'README.md'),lines.join('\n'),{flag:'wx'});
console.log(JSON.stringify({additional:additional.length,baseline:baseline.tests.length,merged:merged.length,noDuplicate:true,file:path.join(directory,'existing-adjacent-selection-v1.json')}));
