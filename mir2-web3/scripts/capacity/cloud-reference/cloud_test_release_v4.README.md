# V4 TEST release controller

Use the canonical `mir2-web3/scripts/capacity/cloud-reference/cloud_test_release_v4.py`
only after the coordinator reviews the candidate artifacts, source compatibility,
maps and fresh live state. Preparation and offline tests do not certify a live
deployment or a capacity result. The original `__file__.parents[4]` repository
boundary remains unchanged; no repository-root override was introduced.

The frozen source baseline is v3 controller SHA256
`ff1904aed7b445350208dd38bc8af0d9d2adf005ec6e4709119ae8221192d9ef`.
All v1/v2/v3 source, release pairs, binaries, states, receipts and archives remain
untouched. V4 uses a new one-candidate private output directory and new owned paths.

## Bounded changes

- Binary path: `/opt/mir2-playtest/releases/<fullSHA>-equip-logout-20261112-v4`.
- Permanent pair: `/etc/mir2-playtest-equip-logout-20261112-v4.env` and the
  `/etc/systemd/system/mir2-playtest.service.d/` basename `z` repeated46 times
  followed by `-equip-logout-20261112-v4.conf`.
- Temporary pair: `/run/mir2-playtest-equip-logout-capacity-20261112-v4.env` and
  the `/run/systemd/system/mir2-playtest.service.d/` basename `z` repeated48 times
  followed by `-equip-logout-capacity-20261112-v4.conf`.
- Prepare requires16 unique original EnvironmentFile entries: the original13,
  then the mandatory v1, v2 and v3 release environment files, in that order.
  Their z34, z38 and z42 persistent drop-ins must also remain in order. All six
  prior release files and the other original configuration metadata/hashes remain
  protected. They never enter v4's four writable/removable `OWN_PATHS`.

Only namespace constants and the prepare count/presence/order check change from
v3. Expected environment entry counts are16 prepared,17 installed,18 with
temporary capacity, then17 after capacity restoration. The complete transaction,
artifact verification, CAS, SSH handshake and unknown-outcome handling remain
unchanged. The test suite checks AST identity of every inherited function/class
except `transaction`, whose change is limited to the prepare baseline guard.

## Preserved conditions

Every mutation checks a fresh known PID/InvocationID/start-ticks identity,
unchanged protected configuration/nonmanaged environment/argv/working directory,
and actual integer zero for all six current admission counters plus the three
Redis record/lease counters. Auth remains2/1/2, original limits51/66/51, temporary
limits120/150/120 and MemoryMax1677721600 bytes. Original routing, selectors,
data settings and all other environment files remain hashed and protected.

Only `mir2-playtest.service` can receive normal SIGINT stop/start, with
`SendSIGKILL=no` and proven normal exit. No old Gateway/Zone, Caddy, validator,
VPN, route or firewall controls are added. Old Gateway and Zone identities remain
protected. The operator retains the other protected services through production
review. Quiescence observations are not an atomic admission lock: a client can
arrive before SIGINT, and the existing immediate checks refuse observed arrivals.

For this round, **prepare must use `--deny-old-binary-rollback`**. It records
`oldBinaryRollbackAllowed=false`; later actions cannot lift that policy. Explicit
rollback and automatic old-binary recovery refuse before mutation. Faults preserve
the known activation/files for reviewed reconciliation. `restore-caps` retains
the new v4 binary and returns51/66/51. Empty counters do not establish that old
code can losslessly round-trip persisted trade custody/recovery data.

The reported restored source is
`7033a012bf39c3d8bd742874c3273ff9eacfd422`, binary
`26915bbbbb92fb567a6ac60efbba45f57e51f780a583c446a146129eb72e9cfe`.
PID654144 was a historical observation during preparation, not a future pin.
Obtain the current full activation and9-counter quiescence immediately before use.
Candidate SHA is already an explicit CLI argument; no future candidate is guessed.

Artifact preflight requires a clean full-SHA `RELEASE.json`, exact binary SHA/size,
Linux x64 ELF and an externally reviewed maps evidence file with its actual file
SHA. Native CI, every map member and source/schema/financial compatibility are
verified externally; this controller does not require Rust sources to be byte
identical, build/dispatch CI, migrate data or certify those external gates. A
changed account-scope implementation requires truthful new compatibility evidence
and its original safety/fallback checks, not weakening the release guards.

## Future operator arguments

These are placeholders, not authorization to execute. Run from the maintained
repository root only after the frontier coordinator reviews the actual artifact
and production state. Keep v3 and older transaction directories immutable.

```powershell
$tool = 'mir2-web3/scripts/capacity/cloud-reference/cloud_test_release_v4.py'
$out = '<fresh-absolute-private-outside-repository-directory>/release-v4'
python -B $tool --action prepare --output $out --expected-pid <freshCurrentPid> `
  --old-revision 7033a012bf39c3d8bd742874c3273ff9eacfd422 `
  --old-binary-sha256 26915bbbbb92fb567a6ac60efbba45f57e51f780a583c446a146129eb72e9cfe `
  --candidate-revision <verifiedNewFullSHA> --artifact-dir <verifiedExtractedGateway> `
  --maps-evidence <reviewedMapsEvidence.json> --maps-evidence-sha256 <actualEvidenceFileSHA256> `
  --confirm-maps-match --deny-old-binary-rollback --host-key-sha256 <verifiedHostKey>
python -B $tool --action install --output $out --expected-pid <samePreparedPid> --host-key-sha256 <verifiedHostKey>
python -B $tool --action apply-caps --output $out --expected-pid <installedPid> --host-key-sha256 <verifiedHostKey>
# After the actual clients drain and every fresh identity/config/6+3 guard passes:
python -B $tool --action restore-caps --output $out --expected-pid <capsPid> --host-key-sha256 <verifiedHostKey>
```

There is no blind retry after unknown SSH outcome, forced shutdown or deletion of
changed/untracked files. Root-owned managed files use0600. Local intent/state and
receipts stay private and outside the maintained checkout. Passwords remain in
getpass/memory, sent once only after the unique sudo stderr prompt. Cached or
NOPASSWD sudo receives no password bytes, including during binary upload.

## Offline checks

```powershell
python -B mir2-web3/scripts/capacity/cloud-reference/test_cloud_test_release_v4.py
```

The51 inherited checks are adapted for16 original environment entries and all
three prior release pairs. Three additional checks cover v3 replacement before
capacity restoration with no signal, equal-length v2/v3 ordering corruption and
exact inherited guard/transport AST preservation. All file/SSH/service interactions
are synthetic fakes or owned temporary fixtures. Tests need no private pool,
credentials, network, service operations, Rust build or Cargo lock.
