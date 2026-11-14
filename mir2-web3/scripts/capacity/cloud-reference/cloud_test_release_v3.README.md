# V3 TEST release controller

The reviewed controller is maintained at the canonical repository path below.
Its original C-directory preparation remains a frozen offline input. Only use
this canonical copy after artifact/source and live-state verification:

```
mir2-web3/scripts/capacity/cloud-reference/cloud_test_release_v3.py
```

The original `__file__.parents[4]` outside-repository receipt guard is unchanged.
It is correct at that canonical path. Offline boundary tests patch `__file__` to a
future canonical location; this does not authorize live execution from C prep.
No repository-root override or other deployment control surface was added.

The source baseline is frozen v2 controller
`4904f19b79d7d5bd9863ba7b2f7a76ce822318fb0d88fcc9bfb31578378c8f75`.
All old v2 source, state, receipts, artifacts and failure archives stay immutable.
The new candidate revision and binary are not known yet; no candidate/deployment
or 100-player conclusion is implied by this tool preparation.

## Bounded release changes

- New binary directory suffix: `<fullSHA>-equip-logout-20261112-v3` below the
  existing `/opt/mir2-playtest/releases` root.
- Permanent owned pair: `/etc/mir2-playtest-equip-logout-20261112-v3.env` and the
  TEST service drop-in basename `z` repeated 42 times followed by
  `-equip-logout-20261112-v3.conf`.
- Temporary owned pair: `/run/mir2-playtest-equip-logout-capacity-20261112-v3.env`
  and the TEST service drop-in basename `z` repeated 44 times followed by
  `-equip-logout-capacity-20261112-v3.conf`.
- Prepare requires exactly 15 unique original environment entries. The final two
  must be the original v1 environment file, then the v2 environment file, both
  mandatory. The v1 z34 and v2 z38 drop-ins must both exist in that order in the
  original drop-in list. Their complete file metadata/hashes and the original
  13 environment entries remain in `protectedConfig`; neither pair enters v3's
  writable/removable `OWN_PATHS`.

The original transaction code, SSH authentication handshake, artifact validation,
maps evidence confirmation, root-owned 0600 files, local intents/state CAS,
known-activation checks, failure/interruption behavior and unknown-SSH-outcome
refusal remain unchanged. Only namespace constants and the stronger prepare
count/presence guard differ from v2.

## Preserved conditions

Every mutation requires fresh known PID + InvocationID + `/proc` start ticks,
exact protected configuration/environment/argv/working-directory hashes, and
actual integer zero for all six current admission counters and three Redis
record/lease counters. Auth remains 2/1/2, restored capacity 51/66/51, temporary
capacity 120/150/120 and MemoryMax 1677721600 bytes. All non-managed environment
values, including routing/selectors/data settings, remain hashed and protected.

Only `mir2-playtest.service` gets normal SIGINT stop/start, with
`SendSIGKILL=no` and proven normal exit. The command allowlist has no restart,
kill, old-service, Caddy, validator, VPN, route or firewall control. The old Gateway
and Zone activation/health checks are preserved. The operator must independently
retain the other protected services as part of production review.

Current live baseline is source
`824f181b48e9f615035280a9e6b4c93af50b5208`, binary
`23c897db8b097fdaa4a0b27c87cb98e0c439c23257ed13a490b0e986faaf2979`.
The parent reported PID 534435 while requesting this preparation. That historical
PID is not a current authorization or pin: the coordinator must obtain a fresh
PID, full service identity and actual empty/quiescent state immediately before use.

This round must set `--deny-old-binary-rollback`. It is recorded false for
`oldBinaryRollbackAllowed`, inherited by subsequent actions, and cannot be enabled
later through CLI. A denied or unproven downgrade is refused before old-binary
recovery; faults leave the known activation/files for reviewed reconciliation.
`restore-caps` preserves the new v3 binary while returning to original limits.
No fresh-empty observation proves persisted data is losslessly downgrade-safe.

Artifact preflight still requires full candidate SHA, clean `RELEASE.json`, exact
binary SHA/size and Linux x64 ELF. The externally reviewed native CI/maps evidence
must match that candidate and carry a pinned evidence path/hash. This controller
does not build, dispatch CI, migrate a DB or certify those external gates itself.

## Future operator arguments (not an execution instruction)

Only after canonical promotion and the frontier coordinator's artifact/source /
production review, run from the verified repository root. Replace all placeholder
values with current reviewed facts; do not execute this C prep file.

```powershell
$tool = 'mir2-web3/scripts/capacity/cloud-reference/cloud_test_release_v3.py'
$out = 'C:/Users/Administrator/AppData/Local/Temp/mir2-equip-logout-20261112/release-v3'
python -B $tool --action prepare --output $out --expected-pid <freshCurrentPid> `
  --old-revision 824f181b48e9f615035280a9e6b4c93af50b5208 `
  --old-binary-sha256 23c897db8b097fdaa4a0b27c87cb98e0c439c23257ed13a490b0e986faaf2979 `
  --candidate-revision <verifiedNewFullSHA> --artifact-dir <verifiedExtractedGateway> `
  --maps-evidence <reviewedMapsEvidence.json> --maps-evidence-sha256 <evidenceSHA256> `
  --confirm-maps-match --deny-old-binary-rollback --host-key-sha256 <independentlyVerifiedHostKey>
python -B $tool --action install --output $out --expected-pid <samePreparedPid> --host-key-sha256 <verifiedHostKey>
python -B $tool --action apply-caps --output $out --expected-pid <installedPid> --host-key-sha256 <verifiedHostKey>
# Only after the real clients drain and every fresh source/config/identity/6+3 guard passes:
python -B $tool --action restore-caps --output $out --expected-pid <capsPid> --host-key-sha256 <verifiedHostKey>
```

There is no automatic retry of an unknown SSH outcome, forced shutdown or deletion
of a changed/untracked file. A client can arrive between an empty observation and
SIGINT; this remains an observation, not an atomic admission lock. Passwords stay
in `getpass`/memory and are sent only after the unique sudo stderr prompt. Cached
or NOPASSWD sudo receives no password bytes. Public/archive credentials are never
supplied as argv, environment or files by this tool.

## Offline checks

```powershell
python -B mir2-web3/scripts/capacity/cloud-reference/test_cloud_test_release_v3.py
```

The copied v2 checks cover complete transactions, six+three drain checks, identity
and configuration drift, typed repeated systemd environment rows, exact file CAS,
normal exit, denied downgrade, source/artifact/maps pins, interruption/timeout and
password/NOPASSWD/cached SSH channel behavior. Added cases cover both prior pairs,
all four v3 namespace collisions, prior inode changes after install with zero new
signals, protected pair order, canonical output boundaries and forbidden controls.
All SSH/service/credential interactions in these checks are synthetic fakes; no
private cohort, actual login, network, remote operation or Rust build is needed.

Canonical test fixtures use the system temporary directory so receipt fixtures stay outside the maintained checkout. This changes test placement only; controller outside-repository validation remains byte-identical to the reviewed preparation.
