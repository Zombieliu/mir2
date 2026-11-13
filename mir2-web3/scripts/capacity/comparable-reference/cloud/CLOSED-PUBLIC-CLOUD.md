# Canonical-ready summary V4: explicit measurement identity

Canonical filename pair: `summarize_closed.py` and `test_summarize_closed.py`.
Package `actual-v3-terminal.sanitized.json` beside the test file. This fixture is
public sanitized schema evidence; tests explicitly replace its identity when
exercising another synthetic epoch. No actual case was read or executed here.

CLI now REQUIRES all three caller pins with no old identity default:
`--expected-pid`, `--expected-start-ticks`, `--expected-invocation-id`.
PID/start must be real positive integers within their bounds, invocation must
match lowercase `[0-9a-f]{32}`. Missing/malformed pins fail before case reads.
Use the MEASUREMENT epoch, not a later restored service's PID. Terminal readback,
all history rows and all resource checks bind that explicit identity/revision.

For Root's second epoch, only AFTER normal case closure:

```powershell
$B='C:/Users/Administrator/AppData/Local/Temp/mir2-comparable-capacity-20261113'
python -B "$B/cloud-capacity-closed-summary-v4/summarize_closed.py" `
  --case-dir "$B/cloud-live-scene-comparison-v1/capacity7403-progressive-v2" `
  --expected-pid 1092834 --expected-start-ticks 1549795188 `
  --expected-invocation-id f5d9c02fa6ff464fb6e7612cf65768d0 `
  --output "$B/cloud-capacity-closed-summary-v4/capacity7403-progressive-v2.summary.json"
```

The same arguments work when Root selectively copies the script to canonical E;
substitute that script's path only. New absolute PUBLIC C output is allowed
outside the repository and source directory. Existing paths, linked parents,
private directories and any repository destination are refused; writing is still
exclusive. The output parent must already exist. Source hashing treats executed
`__file__` as trusted source, so E canonical source no longer fails the C-only
user-input guard. No other input-path restriction is loosened.

Revision7403, ELF26c, load/probe/auth pins, closed Job/monitor/P3 gates, stage SLO,
whole-trace versus completed-stage counters and resource mathematics stay fixed.
V1–V3 code/results remain immutable. No new qualification or SLO veto is added.
Unknown remote exit, separate local monitor evidence, no-op cleanup and limited
fresh-save sampling retain their existing meanings.

All19 prior tests remain, with explicit fixture identity arguments. Four minimal
interface tests were added: invalid/missing CLI pins, new epoch versus stale pin,
public output/repository/private/link rejection plus source self-hashing, and
correct new identity/output argument plumbing. Total23 tests actual0,0skip.
The existing end-to-end synthetic test also verifies a mixed-epoch history is
rejected even with no resource stages. Test artifacts are created ONLY through
`tempfile` under C (checked before writing), never beside canonical E source or
inside a repository. No credentials or real workload are used.

`source-delta.json` proves all non-interface definitions AST-identical and the
resource-counter algorithm identical after removing identity plumbing. Root may
copy/commit the exact source/test/fixture files after reviewing the small diff.
This task performed no E writes, actual case reads, client, SSH, process/service
control, private-file change or Cargo operation.
