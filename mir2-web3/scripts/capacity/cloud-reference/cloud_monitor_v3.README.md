# Read-only cloud monitor V3

V3 preserves V2's sampler, acceptance rules, protection thresholds and stopping
behavior. It adds bounded diagnostics when a **local exception** causes failure.
V2 remains frozen at SHA256
`bf108ff79128799b2b52ec97575e1f30562342eaad30b0c773c3a224d03d2312`.
The executed REMOTE bytes are identical. No service, route, VPN, query cadence,
retry policy or game deadline changes are included.

The same CLI requires the exact revision, PID, invocation ID and process start
ticks, fresh distinct output/history/control paths outside the repository, and
a local PTY. The fixed ED25519 host pin is checked before the sole password
authentication attempt. Passwords remain in memory and are never sent to sudo;
the remote command still uses sudo-n and closed stdin. The original setup,
stream-staleness, finite-duration and byte budgets remain. Missing remote exit
status is never converted to zero.

## Failure-only metadata

A locally failed terminal adds `localFailure` with fixed `phase`, `code` and an
allowlisted exception-class label. Unknown class names become `Other`. No
exception text or traceback is stored. Healthy streams and ordinary remote
guard-stop/finite/local-requested completion records retain their V2 shape.

The original four validate_sample conditions are unchanged. Only their original
ValueError raise sites attach classification metadata. The error remains a
ValueError. No sample rejected by decode/validate is retried, counted or processed further.
Existing V2 schema behavior is retained: `validate.schema` denotes a non-object
sample or non-boolean stop; V3 does not add a schemaVersion acceptance rule.

| Code | Meaning |
| --- | --- |
| decode.json / decode.encoding | JSON syntax or byte-decoding failure |
| decode.nonfinite | NaN/Infinity/-Infinity JSON constant rejected during decode |
| validate.schema | Original sample/stop shape rejection |
| validate.timestamp_schema | Missing, invalid-type or non-finite sampledAt |
| validate.stale / validate.future | Exact age from the original validation exceeded its unchanged bound |
| validate.identity | Original complete pinned-identity rejection |
| validate.activation_shape / validate.activation_fields | Original target/process/health shape or activation-field mismatch |
| validate.flat_health_revision | Original flat health.revision mismatch after activation fields match |
| channel.open_exception / channel.command_exception / channel.read_exception / channel.status_exception | Owned-channel operation failed |
| persist.exception / validate.exception / decode.exception / local.exception | Exception in that fixed phase without a more specific controlled code |
| diagnostic.unavailable | The diagnostic helper itself failed; the original failure still stops the monitor |

`rejectedSampleProjection` has a fixed key set and only finite bounded numbers,
booleans or null. It may include sampledAt, remote monotonic time, the exact
validation age, stale budget, shape/type flags and expected-field match flags.
It contains no coordinates, raw frame, command, address, account, opaque actual
invocation/revision value, auth token, cookie or environment data. Numbers
outside the projection range, non-finite numbers and missing values stay null,
never zero. Decode failures have no decoded-frame projection. Diagnostic helper
failures cannot accept a rejected sample or replace the original stop behavior.

Failure UTC/monotonic and last-persisted timestamps are distinguished. A terminal
record's copied health fields describe the last successfully persisted sample,
not a new health query at the terminal time. On local persistence failure, V3
does not re-emit the failed candidate as the terminal's health data. V2 increments
sampleCount immediately before persistence; that timing remains, and
candidateCountedBeforeFailure/candidatePersisted explicitly identify a failed
persistence attempt. This count does not establish durable coverage. A valid
JSON numeric exponent such as 1e999 can become infinity without parse_constant;
it still fails the original allow_nan=False persistence and remains a failure.

The existing originalMonitorSourceSha256 and transportSourceSha256 fields retain
their V2 provenance meaning. The coordinator must separately freeze and pin the
new V3 local source in the next campaign's source manifest; those old fields do
not certify the V3 implementation. This change does not repair, reclassify or
overwrite any historical campaign evidence.

## Offline verification

Run from the canonical workspace. Fixtures use only the system temporary
directory, synthetic samples and mocked transport; they make no real socket,
SSH, credential, private pool or ledger access.

```powershell
python -B -m unittest discover -s mir2-web3/scripts/capacity/cloud-reference -p test_cloud_monitor_v3.py -v
```

The suite runs the original 17 monitor regressions against V3, verifies V2's hash
and REMOTE bytes, compares unchanged guard-helper ASTs, and normalizes only the
four controlled diagnostic raise wrappers to prove validate_sample AST equality.
It also compares boundary acceptance and whole passing records with V2, checks
failure classifications/privacy/nulls, rejects bad frames before following good
frames, exercises channel/persistence failures, and proves diagnostic failures
cannot trigger a retry or turn a failed monitor into success.

After review and fresh activation pins, an operator can use the unchanged CLI:

```powershell
python mir2-web3/scripts/capacity/cloud-reference/cloud_monitor_v3.py --expected-pid $candidatePid --expected-revision $candidateRevision --expected-invocation-id $candidateInvocation --expected-start-ticks $candidateStartTicks --output $monitorLatest --history $monitorHistory --stop-file $monitorStop
```

No real invocation, deployment or capacity qualification is part of these
offline checks.
