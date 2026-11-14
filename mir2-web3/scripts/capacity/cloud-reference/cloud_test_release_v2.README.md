# V2 TEST Gateway release and capacity round


This is a separate generation copied from the frozen v1 controller
`d442b1154cf5e8e69033604c629ed99bcad4498b31cc646ee7b6e96339e85f34`.
It does not reuse, rewrite or remove the v1 controller, private state, persistent
release pair, artifacts or failed-run evidence. First let the coordinator normally
finish v1 monitors and `restore-caps` with the v1 tool. V2 prepare requires the
original 51/66/51 limits, a fresh PID pin for the existing 2f release, exactly 14
original environment entries (the prior 13 plus the mandatory v1 release env last)
and the existing v1 z34 drop-in. All are immutable protected baseline inputs.

V2 owns new persistent drop-in `z` × 38 plus
`-equip-logout-20261112-v2.conf`, env
`/etc/mir2-playtest-equip-logout-20261112-v2.env`, temporary drop-in `z` × 40 plus
`-equip-logout-capacity-20261112-v2.conf`, and env
`/run/mir2-playtest-equip-logout-capacity-20261112-v2.env`. It layers above v1 while
preserving both v1 files exactly. Use a separate private `release-v2` output; do
not point this tool at v1 state. Only constants, the environment count and the
explicit v1-presence prepare check differ from the v1 transaction implementation.

`cloud_test_release_v2.py` operates only `mir2-playtest.service` on the known cloud
host. This tool does not build, dispatch CI, publish, install dependencies, change
maps, edit accounts/DB/recovery material, or control the old Gateway, Zone, VPN,
routes or firewall. Importing it and running its tests never opens SSH.

Before using it, the operator reviews the native Actions release gates and checks
the exact run `headSha`, extracted Gateway artifact and map compatibility. The
tool verifies the extracted `RELEASE.json` name, full revision, `gitDirty=false`,
binary size/SHA256 and Linux x64 ELF header. It requires an explicit matching-map
evidence file, its SHA256 and operator confirmation. The contents of that evidence
are reviewed externally; hashing it does not certify its conclusion.

Receipts and state belong in a new private directory outside the repository. File
creation uses mode 0600 (remote files are root-owned); POSIX local receipts are
0600. On Windows use a directory restricted to the current operator by its ACL.
Receipts contain selected health counters, file metadata and hashes, never process
argv, raw environment values, passwords, authentication tokens or whole metrics
histograms. Authentication is `getpass` only, SSH/sudo stdin and memory only.
Unknown SSH host keys are rejected unless an independently verified
`--host-key-sha256 SHA256:...` pin is explicitly supplied. No automatic acceptance
of an unknown key occurs.

## Scope and guards

- Initial pin: explicit old full revision, binary SHA256 and current PID; the
  helper also binds systemd InvocationID and `/proc` start ticks.
- All six current session/inflight counters and three Redis record/lease counters
  must be actual integer zero. Redis and both health listeners must be healthy.
- Auth remains 2/1/2; MemoryMax remains 1677721600 bytes. SIGINT and
  `SendSIGKILL=no` are mandatory. No `kill`, forced stop, restart, or reset-failed
  fallback exists.
- All 14 original environment files (including the v1 release env) retain order and ignore-errors flags. Their
  hashes, inode/owner/mode/mtime, original unit/drop-in files, static service
  properties, working directory, original argv tail and all non-managed process
  environment values (hashed only) are preserved. Only standard systemd activation
  environment IDs and the four explicitly managed revision/cap keys are excluded
  from the environment hash. Repeated `EnvironmentFiles=` rows are aggregated.
- The old Gateway must retain its activation and healthy port 7110; Zone must
  retain its activation. Service control has a hard allowlist for TEST start/stop
  and daemon-reload only. No VPN or routing command is present.
- Original argv must be simple literal tokens accepted by the bounded parser.
  Quoted, expanded or ambiguous args fail before any service mutation. The tool
  does not guess how to reconstruct them. Arguments are hashed in receipts.

`install` stages only the verified binary, exclusively, at:

```
/opt/mir2-playtest/releases/<fullSHA>-equip-logout-20261112-v2/mir2-gateway
```

The original release and all data stay in place. A successful staged candidate is
retained even after rollback. The persistent, root-0600 last drop-in resets
`ExecStart` to that path with the original argv tail. A separate root-0600
`/etc/mir2-playtest-equip-logout-20261112-v2.env` is appended to set only
`MIR2_DEPLOY_REVISION`; `Environment=` is not used. WorkingDirectory is inherited.

`apply-caps` adds a different owned pair under `/run`, appended after the release
pair, containing only 120 active / 150 WS / 120 reconnect. `restore-caps` removes
exactly that pair and normally restarts TEST to original 51/66/51 while retaining
the new persistent binary/revision. `rollback` removes both owned pairs that are
present, preserving the candidate binary, and normally starts the old release.
All removals require full metadata/content CAS, never just a matching pathname.

## Old-binary rollback policy

**This v2 round must also prepare with `--deny-old-binary-rollback`.** Its
immediate old binary is the v1 fixed source
`2f2a411d512cbc75adf310d3fee1d1aa9d0ca9e0`, not d53. The coordinator keeps binary
downgrade denied for this round; no data-compatibility clearance is inferred from
empty counters. The older d53 ignores the later `local_trade_matches`,
`heldItemsJson`, `tradeRecoveryVersion` and `ownItems` fields and can lose new
custody/recovery data when saving. V1's separate denied-downgrade state remains
untouched. No v2 operation removes the v1 pair or repoints v1 at d53.

The prepare-only flag records `oldBinaryRollbackAllowed=false` in state and local
intent. Every later action inherits that policy from state; no CLI switch can
enable it later. Explicit `rollback` is refused before remote service mutation.
Automatic `recover_original` also refuses false, missing or unproven policy before
any host access: on a deployment/capacity failure it defers, retains all currently
owned files and the existing activation, and never repoints the service at the prior 2f binary.
Resolve the reported failure with a forward fix and a separately reviewed plan.

The approved `restore-caps` action still removes only the temporary `/run` capacity
pair and normally restarts the fixed binary at 51/66/51. That is a capacity restore,
not a binary downgrade. If another round has separately proven compatible data,
omitting the flag records explicit `oldBinaryRollbackAllowed=true` and retains the
original guarded old-binary recovery behavior. Do not omit it for this v2 round.

## Operator commands

Use one output directory for exactly one candidate. Replace all bracketed values
with reviewed facts. The original pins below describe the v1 fixed TEST release;
freshly read PID/health before use. A historical receipt is not a current PID pin.

```powershell
$tool = 'mir2-web3/scripts/capacity/cloud-reference/cloud_test_release_v2.py'
$out = 'C:/Users/Administrator/AppData/Local/Temp/mir2-equip-logout-20261112/release-v2'
python -B $tool --action prepare --output $out --expected-pid <currentOldPid> `
  --old-revision 2f2a411d512cbc75adf310d3fee1d1aa9d0ca9e0 `
  --old-binary-sha256 de462c22c44fd15f2e2a19b41a8c2c6b0131407f1f9989b1e10748b669f7a682 `
  --candidate-revision <newFullSHA> --artifact-dir <extractedGatewayDirectory> `
  --maps-evidence <reviewedEvidence.json> --maps-evidence-sha256 <evidenceSHA256> `
  --confirm-maps-match --deny-old-binary-rollback --host-key-sha256 <verifiedHostKeyPin>
python -B $tool --action install --output $out --expected-pid <sameOldPid> --host-key-sha256 <verifiedHostKeyPin>
python -B $tool --action apply-caps --output $out --expected-pid <newInstalledPid> --host-key-sha256 <verifiedHostKeyPin>
# After clients have normally drained and all 6+3 counters are zero:
python -B $tool --action restore-caps --output $out --expected-pid <capsActivationPid> --host-key-sha256 <verifiedHostKeyPin>
# Only for a DIFFERENT, proven-compatible round with recorded allowed policy;
# the denied v2 round above refuses this command before any service mutation:
python -B $tool --action rollback --output $out --expected-pid <currentRecordedPid> --host-key-sha256 <verifiedHostKeyPin>
```

`prepare` is read-only remotely. Every later action requires the recorded complete
activation plus a fresh PID argument, fresh configuration/health checks and CAS.
The transport polls stdout and stderr concurrently through the SSH channel. A
unique, non-secret `sudo -S -p` marker authorizes at most one password line, only
before the root helper's flushed `rpcStarted` acknowledgment. With NOPASSWD or a
cached sudo credential, no password bytes are sent. Once the root helper starts,
stdin is reserved for the exact advertised binary body. A wrong, repeated, late
or incomplete prompt, unexpected stderr, disconnect or timeout fails closed; no
raw stderr or password is copied to a receipt. This also prevents a queued root
acknowledgment from racing a late prompt into an artifact password prefix.

The binary is streamed through that channel after the separate artifact readiness
handshake, in chunks no larger than 1 MiB, not placed in a writable remote upload
directory or passed in argv. RPC lines remain bounded to 512 KiB; the entire call
is bounded to 480 seconds, with no authentication retry.
Each action archives a receipt and a state snapshot. Local state replacement uses
CAS and a per-output exclusive lock; stale locks require operator investigation.

## Failure boundaries

Quiescence checks are observations, not an atomic admission lock. A real client
can arrive between the final empty check and SIGINT; normal shutdown/save remains
the original Gateway behavior. The tool does not modify auth or bypass admission
to close that race. Run in a coordinated empty maintenance window.

When recorded old-binary rollback policy is explicitly true, fault recovery
attempts only the known activation, approved
old/candidate binaries and original/new managed cap values. Fresh sessions,
unhealthy/unavailable observations, configuration drift, an unexpected activation,
foreign/replaced files or an unproven normal stop defer recovery without force.
The receipt retains the failed guard and the recovery result. A deferred state only permits the explicit
guarded rollback after the cause is resolved and quiescence is genuinely proved.
Allowed recovery restores the old release and original caps, not merely the
capacity pair. The denied mode above performs no such downgrade or deletion.

An SSH loss can make the remote outcome unknown. The tool records this explicitly
and does not invent a successful transaction or blindly stop/retry an unknown
process. Reconcile identities and owned-file metadata read-only before further
action; do not rerun an `install` against an unknown outcome. Signals delivered to
the helper request the same guarded recovery, not SIGKILL. The helper cannot
guarantee recovery after SIGKILL, host failure or an unobserved transport loss.

The final stop/start wait is bounded to 90 seconds each. A failed/unhealthy start
can therefore require operator reconciliation rather than a forced availability
claim. New staged binaries and all old releases/evidence are retained.

## Offline verification

```powershell
python -B mir2-web3/scripts/capacity/cloud-reference/test_cloud_test_release_v2.py
python -B mir2-web3/scripts/capacity/cloud-reference/cloud_test_release_v2.py --help
```

Fake hosts cover complete install/caps/restore/rollback, all nine quiescence
guards, repeated systemd rows, identity/config/argv/environment/peer drift, file
replacement, partial pair creation, ignored cap values, fresh arrivals, refused
force paths, artifact/map pins and local receipt boundaries. These are tooling
checks; they do not certify a remote deployment or the candidate game behavior.

Rollback-policy tests also prove denied-mode install/caps/restore retains the fixed
binary, explicit rollback has zero mutations, fault recovery retains activation
and owned files even with empty counters, missing policy cannot authorize recovery,
and later requests cannot lift the recorded deny policy.

Fake SSH tests also cover password-required, NOPASSWD and cached sudo, fragmented
markers, a repeated or late prompt, partial prompt/EOF/closed channels, timeout,
exact multi-chunk artifact bytes and stderr redaction. The earlier unconditional
password-writing source `5c59df4e...` was archived outside the repository for
comparison and must not be used to deploy. No remote deployment ran with it.

V2 tests additionally prove the v1 env/drop-in pair survives install, caps,
capacity restore and allowed rollback, and that a changed v1 hash/inode or a
missing/optional v1 environment entry is rejected before TEST service control.
The new 47-method suite reuses the original 44 tests without counting them twice.
