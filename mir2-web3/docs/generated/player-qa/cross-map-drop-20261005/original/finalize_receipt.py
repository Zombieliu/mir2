"""One-shot local P3a evidence collector; never edits repository inputs."""
from pathlib import Path
import datetime
import difflib
import hashlib
import json
import re
import shutil
import subprocess

PROJECT = Path('C:/Users/Administrator/.codex/worktrees/p3/mir2-player-journey/mir2-web3')
REPO = PROJECT.parent
OUT = Path(__file__).parent
PLAN = Path('C:/mir2-playtest-releases/20261004-native-r17/cross-map-drop-plan-01/baseline-51e8-review')
HEAD = '57729ba8c1473ca920a8ea8b3d6e11105d9d4d93'
FILES = [
    'apps/simulation/src/runtime/zone/mod.rs',
    'apps/simulation/src/runtime/zone/online_identity.rs',
    'apps/simulation/src/runtime/zone/types.rs',
    'apps/simulation/src/runtime/zone/manager.rs',
    'apps/simulation/src/runtime/zone/runtime/experience_ownership.rs',
    'apps/simulation/src/runtime/zone/runtime.rs',
    'apps/simulation/src/runtime/zone/runtime/ground_ownership.rs',
    'apps/simulation/src/runtime/zone/runtime/checkpoint.rs',
    'apps/simulation/src/runtime/drops.rs',
    'apps/gateway/src/routing.rs',
    'apps/simulation/tests/shared_cross_map_drop_lifecycle.rs',
    'apps/gateway/src/routing/tests/cross_map_drop_lifecycle_tests.rs',
    'docs/CROSS-MAP-DROP-LIFECYCLE-20261004.md',
    'apps/simulation/tests/shared_monster_ownership.rs',
]
NEW = {FILES[i] for i in [1, 6, 10, 11, 12]}

def sha(data):
    return hashlib.sha256(data).hexdigest()

def read_json(path):
    return json.loads(path.read_text(encoding='utf-8-sig'))

def write_json(name, value):
    path = OUT / name
    assert not path.exists(), f'Immutable receipt already exists: {path}'
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n', encoding='utf-8', newline='\n')

def git(*args, **kw):
    return subprocess.run(['git', '-C', str(REPO), *args], capture_output=True, **kw)

assert git('rev-parse', 'HEAD').stdout.decode().strip() == HEAD
allowed = {'mir2-web3/' + name for name in FILES}
tracked = set(git('diff', '--name-only', 'HEAD').stdout.decode().splitlines())
untracked = set(git('ls-files', '--others', '--exclude-standard').stdout.decode().splitlines())
assert tracked | untracked == allowed, (tracked | untracked) ^ allowed
assert untracked == {'mir2-web3/' + name for name in NEW}
baseline = read_json(OUT / 'baseline.json')
for item in baseline['files']:
    saved = (OUT / 'baseline' / item['file']).read_bytes()
    assert sha(saved) == item['sha256']

# Preserve exact current Windows bytes separately from normalized Git patch text.
source_rows = []
for name in FILES:
    data = (PROJECT / name).read_bytes()
    dest = OUT / 'frozen-source' / name
    assert not dest.exists()
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.write_bytes(data)
    source_rows.append({'file': name, 'sha256': sha(data), 'bytes': len(data), 'new': name in NEW})
write_json('source-manifest.json', {'head': HEAD, 'files': source_rows, 'count': len(FILES)})

original_records = []
for row in read_json(PLAN / 'source-hashes.json')['records']:
    if row['kind'] == 'original-crystal':
        original = Path(row['originalPath']).read_bytes()
        snapshot = Path(row['snapshot']).read_bytes()
        assert sha(original) == row['sha256'] == sha(snapshot)
        original_records.append({'file': row['relative'], 'sha256': row['sha256'], 'bytes': len(original),
                                 'originalPath': row['originalPath'], 'snapshot': row['snapshot']})
assert len(original_records) == 9
write_json('original-source-recheck.json', {'sources': original_records, 'allExact': True})

death_rel = 'apps/gateway/src/routing/tests/dead_experience_chain_tests.rs'
death_current = (PROJECT / death_rel).read_bytes()
death_blob = git('show', 'HEAD:mir2-web3/' + death_rel)
assert death_blob.returncode == 0
assert death_current.replace(b'\r\n', b'\n') == death_blob.stdout.replace(b'\r\n', b'\n')
write_json('death18-source-unchanged.json', {'file': death_rel, 'sha256': sha(death_current),
                                          'gitContentExact': True, 'sourceEdited': False})

# Ordinary applicable patch includes all four new Rust files and the scoped doc.
diff = git('diff', '--binary', '--no-ext-diff', 'HEAD')
assert diff.returncode == 0
patch = diff.stdout.decode('utf-8')
for name in FILES:
    if name in NEW:
        text = (PROJECT / name).read_text(encoding='utf-8').replace('\r\n', '\n')
        lines = text.splitlines(keepends=True)
        assert text.endswith('\n')
        target = 'mir2-web3/' + name
        patch += (f'diff --git a/{target} b/{target}\nnew file mode 100644\n'
                  f'--- /dev/null\n+++ b/{target}\n@@ -0,0 +1,{len(lines)} @@\n')
        patch += ''.join('+' + line for line in lines)
patch_path = OUT / 'p3a-complete.patch'
assert not patch_path.exists()
patch_path.write_text(patch, encoding='utf-8', newline='\n')
assert len(re.findall(r'^diff --git ', patch, re.M)) == len(FILES)
check = git('diff', '--check')
assert check.returncode == 0, check.stderr.decode()
reverse = git('apply', '--reverse', '--check', '--whitespace=nowarn', str(patch_path))
assert reverse.returncode == 0, reverse.stderr.decode()

# A physical source-only baseline verifies forward applicability without
# changing the real worktree, index, other repos or generated/public resources.
forward_root = OUT / 'patch-forward-check'
assert not forward_root.exists()
forward_root.mkdir()
for name in FILES:
    if name not in NEW:
        original_path = OUT / 'baseline' / name
        if not original_path.exists():
            original_path = OUT / 'fixture-adaptation-before' / name
        dest = forward_root / 'mir2-web3' / name
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_bytes(original_path.read_bytes())
forward = git('--work-tree=' + str(forward_root), 'apply', '--check', '--whitespace=nowarn', str(patch_path))
assert forward.returncode == 0, forward.stderr.decode()
write_json('patch-verification.json', {
    'patch': str(patch_path), 'sha256': sha(patch_path.read_bytes()), 'files': len(FILES),
    'gitDiffCheckExit': check.returncode, 'reverseCurrentCheckExit': reverse.returncode,
    'forwardPhysicalBaselineCheckExit': forward.returncode, 'applied': False,
    'lineEndings': 'Git patch normalized; frozen-source retains actual reviewed Windows bytes',
})

adaptation_patch = ''
for name in ['apps/simulation/tests/shared_monster_ownership.rs', 'apps/simulation/src/runtime/zone/runtime/checkpoint.rs']:
    before_path = OUT / 'fixture-adaptation-before' / name
    before = before_path.read_text(encoding='utf-8').replace('\r\n', '\n')
    after = (PROJECT / name).read_text(encoding='utf-8').replace('\r\n', '\n')
    adaptation_patch += ''.join(difflib.unified_diff(before.splitlines(keepends=True), after.splitlines(keepends=True),
                                fromfile='before/' + name, tofile='after/' + name, n=4))
(OUT / 'approved-adjacent-fixture-adaptations.patch').write_text(adaptation_patch, encoding='utf-8', newline='\n')
write_json('fixture-adaptations.json', {
    'areNewRedOrNetworkAcceptance': False,
    'additionalApprovedExistingTest': 'apps/simulation/tests/shared_monster_ownership.rs',
    'beforeSourceReceipt': 'fixture-adaptation-before/source-before.json',
    'realFailures': ['adjacent-same-map-attempt-01.log', 'simulation-checkpoint-attempt-01.log'],
    'adjacentExactPatch': 'approved-adjacent-fixture-adaptations.patch',
    'gatewayExactHunks': 'p3a-complete.patch: apps/gateway/src/routing.rs test section',
    'gatewayTests': [
        'shared_in_process_runtime_emits_gain_experience_after_kill_award_commit',
        'shared_in_process_runtime_uses_account_inventory_service_boundary',
        'gateway_kill_award_level_up_updates_zone_and_personal_vitals',
        'shared_zone_state_dispatches_current_and_pending_monster_kill_awards -> shared_zone_state_rejects_unbound_legacy_monster_kill_awards'],
    'adjacentTests': [
        'authenticated_checkpoint_retains_owner_without_extending_deadline -> authenticated_cold_checkpoint_preserves_state_but_revokes_live_owner',
        'fresh_same_map_join_revokes_old_claim_and_old_poison_identity',
        'complete_zone_checkpoint_restores_authoritative_and_derived_state'],
    'guardRationale': 'Direct fake shared native awards have no source issuance. Cold integrity must not retain a live Node. Logout clears poison caster, not the active green effect/absolute timer. Existing EXP/life/vitals/payload assertions remain; new tamper/epoch/positive-damage assertions are required.',
})

FINAL_LOGS = [
    'green-simulation-attempt-06.log', 'gateway-tests-attempt-08.log',
    'adjacent-same-map-attempt-02.log', 'simulation-checkpoint-attempt-02.log',
    'simulation-experience-owner-attempt-01.log', 'simulation-online-identity-attempt-01.log',
    *[f'gateway-adjacent-{i:02}-attempt-01.log' for i in range(1, 6)],
]
tests = []
for name in FINAL_LOGS:
    data = (OUT / name).read_bytes()
    text = data.decode('utf-8-sig')
    result = re.findall(r'test result: ok\. (\d+) passed; (\d+) failed', text)
    assert len(result) == 1 and result[0][1] == '0', name
    names = re.findall(r'^test ([\w:]+) \.\.\. ok\s*$', text, re.M)
    assert len(names) == int(result[0][0]), (name, len(names), result)
    tests.append({'log': name, 'sha256': sha(data), 'passed': len(names), 'failed': 0, 'tests': names})
assert sum(row['passed'] for row in tests) == 89
assert len({name for row in tests for name in row['tests']}) == 89
write_json('focused-tests.json', {'distinctPassed': 89, 'failed': 0, 'selections': tests,
                                'localOnly': True, 'target': 'C:/mir2-build/gateway-tests-r50', 'jobs': 2})

failures = [
    ('red-simulation-01.log', 'real-pre-production-RED', '0 passed / 6 failed on original production; ownership/return/protection/sync/cancel/leave reproduce defects'),
    ('green-simulation-attempt-01.log', 'compile', 'LowerHex formatting attempt; no passing test claim'),
    ('green-simulation-attempt-03.log', 'compile', 'Fixture misspelled experience_selection'),
    ('green-simulation-attempt-04.log', 'prepared-fixture', 'Parallel poison HP15 expectation was wrong for original damage4; HP12 fixture repaired without formula change'),
    ('gateway-tests-attempt-01.log', 'compile', 'Test-only imports/packet shapes/types'),
    ('gateway-tests-attempt-02.log', 'process-abort', 'Fixture compared personal vs shared object identity while holding mutex; panic poisoned it and Drop panicked; actual shared id assertion moved outside lock'),
    ('gateway-tests-attempt-03.log', 'prepared-fixture', 'Ordinary PickUp used monster coordinate instead of actual scattered source drop; walk exact source-born tile'),
    ('gateway-tests-attempt-05.log', 'compile', 'GainedItem packet shape/item-index type in fixture'),
    ('gateway-tests-attempt-06.log', 'prepared-fixture', 'Seed1 produces another legitimate drop on tile; second PickUp was another item, not duplication; retained failure and source-rate seed6 single-item fixture'),
    ('adjacent-same-map-attempt-01.log', 'approved-adjacent-fixture', '19 passed / 2 failed: cold restored root/online claim and erase-green-poison expectations contradict source Node lifetime'),
    ('simulation-checkpoint-attempt-01.log', 'approved-adjacent-fixture', '16 passed / 1 failed: cold root equals original-live-root assertion'),
]
write_json('failure-history.json', {'rawFailuresNeverOverwritten': True, 'failures': [
    {'log': name, 'sha256': sha((OUT / name).read_bytes()), 'kind': kind, 'reason': why}
    for name, kind, why in failures], 'originalRed': 'red-simulation-01.json',
    'sourceBeforeAdaptation': 'fixture-adaptation-before/source-before.json'})

# Every original planned case is mapped to actual bounded evidence. Open or
# partial variants explicitly do not claim every original planned step ran.
SIM = 'green-simulation-attempt-06.log'
GW = 'gateway-tests-attempt-08.log'
SAME = 'adjacent-same-map-attempt-02.log'
CP = 'simulation-checkpoint-attempt-02.log'
LIFE = 'gateway-adjacent-05-attempt-01.log'
case_evidence = {
    'xp-01': ('bounded-pass', [SIM, GW], ['owner_cross_map_retains_experience_and_source_drop', 'ordinary_walk_source_route_and_carried_poison_award_are_source_bound'], 'Manager transfer and one-caster native Walk/poison variant, not network multi-client journey.'),
    'xp-02': ('bounded-pass', [SIM, GW], ['owner_return_to_source_does_not_revoke_claim', 'ordinary_walk_source_route_and_carried_poison_award_are_source_bound'], 'Actual ordinary imported return retains proof.'),
    'xp-03': ('bounded-pass', [SIM, GW], ['periodic_poison_renews_owner_after_caster_changed_map'], 'Source tick after caster transfer refreshes current owner.'),
    'xp-04': ('bounded-pass', [SIM, SAME], ['strict_expiration_can_be_renewed_only_by_source_legal_current_owner', 'exact_five_second_deadline_keeps_original_owner', 'strictly_after_five_seconds_next_effective_attacker_claims_owner'], 'Strict five-second equality and expiry, source-legal renewal.'),
    'xp-05': ('partial', [SAME, LIFE], ['solo_dead_poison_owner_keeps_source_kill_credit', 'dead_poison_level_refills_pools_without_online_revive'], 'Actual dead-owner same-map chain retained. Combined dead-owner cross-map XP journey not executed.'),
    'xp-06': ('bounded-same-map-regression-pass', [SAME], ['new_effective_attacker_replaces_dead_exp_owner'], 'Dead-owner replacement positive damage regression; cross-map dead variant not executed.'),
    'xp-07': ('bounded-pass', [SIM], ['true_leave_from_other_map_releases_original_ground_owner', 'same_map_fresh_admission_releases_old_ground_owner_and_experience'], 'True all-map reference revocation.'),
    'xp-08': ('bounded-pass', [SIM, GW, SAME], ['fresh_join_cannot_inherit_old_epoch_even_with_reused_ids', 'fresh_login_never_accepts_a_replayed_previous_online_award', 'fresh_same_map_join_revokes_old_claim_and_old_poison_identity'], 'Old/new epoch ABA and old poison lethal tick rejection.'),
    'xp-09': ('bounded-pass', [SIM, 'simulation-online-identity-attempt-01.log'], ['encoded_owner_binds_manager_epoch_account_character_object_and_session', 'online_epoch_exhaustion_fails_closed_without_recycling'], 'All proof fields/other manager rejected; overflow actual admit, not mirrored arithmetic.'),
    'drop-01': ('bounded-boundary-pass', [SIM, GW], ['owned_native_drop_has_strict_original_one_minute_boundary', 'source_item_exact_payload_receives_one_uid_and_survives_normal_save_login'], 'Gold tests actual relative59999/60000/60001; source-rate item verifies payload/UID. Not every planned item/time combination run.'),
    'drop-02': ('bounded-pass', [SIM, GW], ['owner_cross_map_retains_experience_and_source_drop', 'ordinary_walk_source_route_and_carried_poison_award_are_source_bound'], 'Source map retains drop; stranger denied, normal owner returns/picks up.'),
    'drop-03': ('bounded-pass', [SIM], ['dead_online_owner_keeps_source_ground_protection_until_true_despawn'], 'Typed full-HP corpse keeps original owner protection until true despawn.'),
    'drop-04': ('bounded-pass', [SIM], ['true_leave_from_other_map_releases_original_ground_owner'], 'Offline source owner releases early; new epoch cannot inherit.'),
    'drop-05': ('bounded-pass', [SIM], ['repeated_sync_does_not_restart_original_protection'], 'Projection sync cannot rebase original protection.'),
    'drop-06': ('bounded-pass', [SIM, GW], ['cancelled_claim_does_not_restart_original_protection', 'detached_custody_retains_deadline_ttl_and_stable_ledger_key', 'ordinary_pickup_with_a_prepared_full_bag_restores_exact_original_custody_clocks'], 'Actual PickUp bag reservation/cancel and delayed manager custody variants; slot-free later network retry not executed.'),
    'drop-07': ('bounded-pass', [SIM], ['detached_custody_retains_deadline_ttl_and_stable_ledger_key', 'expired_held_ground_ttl_never_resurrects_on_cancel'], 'Post-owner deadline rollback opens; held expired TTL never resurrects.'),
    'drop-08': ('partial', [SIM, CP, GW], ['source_issuance_binds_original_reward_before_and_after_map_membership_change', 'lost_proof_is_not_recreated_from_new_membership_or_fresh_login', 'exact_ground_drop_identity_is_checkpointed_and_state_root_protected', 'source_item_exact_payload_receives_one_uid_and_survives_normal_save_login'], 'Source/identity/payload/generation/tamper/repeat negatives covered; not every original claim substitution combination executed.'),
    'drop-09': ('partial', [SIM], ['manual_unowned_drop_projection_never_invents_native_item_owner'], 'Unowned manual projection does not gain native owner; full ordinary manual/death constructor journey not executed.'),
    'drop-10': ('bounded-pass', [SIM], ['irregular_tick_cadence_does_not_replace_birth_with_latest_sync_or_tick'], 'Irregular absolute now_ms and strict boundary; not every original cadence permutation.'),
    'fault-01': ('bounded-capture-pass', [GW], ['cold_world_checkpoint_keeps_source_item_payload_and_ttl_but_revokes_all_online_owners'], 'Capture retains actual live instance proof. All deserialization deliberately cold; no hot decode authority restoration.'),
    'fault-02': ('bounded-pass', [SIM, GW, SAME], ['cold_manager_checkpoint_never_recreates_original_online_nodes', 'cold_world_checkpoint_keeps_source_item_payload_and_ttl_but_revokes_all_online_owners', 'authenticated_cold_checkpoint_preserves_state_but_revokes_live_owner'], 'Valid cold commitments keep payload/TTL and revoke live authority; fresh Join/Login cannot reclaim.'),
    'fault-03': ('bounded-pass', [CP, 'simulation-experience-owner-attempt-01.log'], ['legacy_v5_authenticates_original_item_before_ignoring_unbound_forward_owner_fields', 'legacy_v4_forward_claim_and_forced_impact_are_neutralized'], 'Legacy1-5 selected migration fixtures/root validation and forward-owner neutralization.'),
    'fault-04': ('bounded-negative-pass', [SIM, CP, SAME], ['encoded_owner_binds_manager_epoch_account_character_object_and_session', 'new_online_metadata_is_integrity_bound_but_cold_decode_never_grants_authority', 'checkpoint_exp_owner_tampering_cannot_bypass_root_authentication'], 'Current metadata/payload tampering rejected; original complete checkpoint HP tamper rejected.'),
    'fault-05': ('partial', [GW, LIFE], ['unknown_publication_retains_exact_source_receipt_without_faked_exp_success', 'known_reward_save_failure_keeps_source_epoch_for_one_exact_retry'], 'Actual file rename/directory-sync unknown outcome freezes exact receipt and ordinary Logout; SQL lost response/recovery restart/reconnect settlement not executed.'),
    'fault-06': ('bounded-pass', [GW], ['known_reward_save_failure_keeps_source_epoch_for_one_exact_retry', 'ordinary_pickup_with_a_prepared_full_bag_restores_exact_original_custody_clocks'], 'Definitive reward failure retries once; rejected ordinary pickup preserves source custody clocks.'),
    'fault-07': ('open', [], [], 'Both DB and recovery journal failure not executed; existing durability extension not claimed as immediate original Disconnect parity.'),
    'fault-08': ('partial', [LIFE, GW], ['dead_poison_retained_bootstrap_preserves_online_life', 'fresh_login_never_accepts_a_replayed_previous_online_award'], 'Same-runtime bootstrap vs fresh local credential login checked; WS retention timeout/token rotation not executed.'),
    'fault-09': ('bounded-pass', [SIM], ['parallel_source_zone_tick_matches_sequential_cross_map_owner_and_payload'], 'Four zones exercise actual parallel path; equivalent ordered reward payload/clocks/current issuance, independently random manager namespaces preclude raw-root equality.'),
    'protocol-01': ('bounded-ordinary-variant-pass', [GW], ['ordinary_walk_source_route_and_carried_poison_award_are_source_bound', 'source_item_exact_payload_receives_one_uid_and_survives_normal_save_login'], 'One prepared caster departs before actual poison kill. Imported Walk/return, MapInformation, spend, source drop, EXP, UID and normal saves proven; planned rival multi-client variant/live transport not run.'),
    'protocol-02': ('partial', [GW], ['unknown_publication_retains_exact_source_receipt_without_faked_exp_success', 'ordinary_walk_source_route_and_carried_poison_award_are_source_bound'], 'Ordinary LogOut rejects unconfirmed save and preserves proof; successful LogOut/Login revokes. Crystal combat-delay LogOutFailed packet variant not executed.'),
    'scope-01': ('open-source-gap', [], [], 'Candidate name/equal party split unchanged; original level-weighted object-reference party parity unaccepted.'),
    'scope-02': ('open-source-gap', [], [], 'Boss/PK contribution and original LastHitter not rewritten or accepted; no natural Boss/book drop result.'),
    'scope-03': ('open-architecture-boundary', [], [], 'One manager only. Cross-Gateway ZoneId/process handoff needs separate authority coordinator and remains unaccepted.'),
    'life-51e8-preserve': ('bounded-regression-pass', [LIFE, SAME], ['dead_poison_level_refills_pools_without_online_revive', 'native_zero_hp_death_rejects_valid_magic_without_spend_hit_or_turn', 'full_hp_poison_level_corpse_blocks_normal_potion_but_scroll_explicitly_revives'], 'All12 frozen death18 actual ordinary Magic/poison-death cases pass unchanged; no new combined dead-crossmap network journey claim.'),
}
planned = read_json(PLAN / 'planned-cases.json')['cases']
assert set(case_evidence) == {case['id'] for case in planned}
passed_names = {name.split('::')[-1] for selection in tests for name in selection['tests']}
actual_cases = []
for case in planned:
    status, logs, names, note = case_evidence[case['id']]
    assert set(logs) <= set(FINAL_LOGS)
    assert set(names) <= passed_names, (case['id'], set(names) - passed_names)
    actual_cases.append({'id': case['id'], 'plannedTitle': case['title'], 'status': status,
                         'logs': logs, 'tests': names, 'boundary': note,
                         'allOriginalPlannedStepsAccepted': False, 'networkAccepted': False})
write_json('case-results.json', {'plannedCount': 34, 'cases': actual_cases,
                               'completeP3Accepted': False, 'everyPlannedScenarioPassed': False})

write_json('final-receipt.json', {
    'schema': 'mir2.p3a.bounded-implementation-final.v1',
    'createdAt': datetime.datetime.now(datetime.timezone.utc).isoformat(), 'baselineHead': HEAD,
    'worktree': str(PROJECT), 'status': 'frozen-for-parent-review-no-commit',
    'files': 14, 'distinctFocusedPassingTests': 89, 'currentFocusedFailures': 0,
    'originalRedBeforeProductionEdits': True, 'firstFailuresRetained': True,
    'sourceManifest': 'source-manifest.json', 'fullPatch': 'p3a-complete.patch',
    'fullPatchSha256': sha(patch_path.read_bytes()), 'patchChecks': 'patch-verification.json',
    'focusedTests': 'focused-tests.json', 'planned34ActualMapping': 'case-results.json',
    'failures': 'failure-history.json', 'fixtureAdaptations': 'fixture-adaptations.json',
    'sourceOriginalRecheck': 'original-source-recheck.json', 'death18Unchanged': 'death18-source-unchanged.json',
    'target': 'C:/mir2-build/gateway-tests-r50', 'jobs': 2,
    'networkRun': False, 'servicesStarted': False, 'playerSavesChanged': False,
    'generatedResourcesChanged': False, 'globalDocsChanged': False,
    'releaseBuildRun': False, 'packageRun': False, 'commitRun': False, 'pushRun': False, 'publishRun': False,
    'acceptance': {'oneManagerPreparedLocal': True, 'completeP3': False, 'partyParity': False,
                   'bossParity': False, 'pkLastHitterParity': False, 'crossZoneId': False,
                   'crossProcess': False, 'liveTransport': False, 'leveledJourney': False,
                   'naturalBossEquipmentBooks': False, 'humanFrontend': False},
})
print(json.dumps({'files': len(FILES), 'passing': 89, 'patchSha256': sha(patch_path.read_bytes()),
                  'forwardPatchCheck': forward.returncode, 'reversePatchCheck': reverse.returncode}, indent=2))
