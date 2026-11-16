# e0 exact-source release guard

The release controller targets application
`e0ceea22964c03041d45e49e506b3a8badd6984e`. At pinning time its rollback was the
then-live, two-cohort-qualified `dpl_G8iq4UBFibrWSEaSbwahtboeAom3`
(application `6ba38557ddb298df72cb9e4a23c48caf542a598e`). This controller update
does not change production traffic.

The complete live-base-to-candidate change contains four commits and 49 paths.
Independent GitHub comparisons, 35 and 18 paths, match full local Git commit
lists, statuses and blob identities. The overlap is composed by the unchanged
source-closure verifier; both comparisons are below its 300-file boundary.
The intermediate `7468b3337a` commit is a provenance checkpoint, not a deployed
application. See [complete source closure](source-closure.json).

All 2,181 app/library files in the qualified build match the application commit,
with one locked generated input. The served 68,357-byte Worker matches SHA-256
`9c301d24217f498408d77eb0b38f88f9c6825020d7d380ed2c383022db615f97` exactly.
Non-runtime test drafts were validated separately. The two local complete-game
receipts belong to the same strict-built candidate.

Only the allowed application/base/baseline/commit/file/Worker pin values changed
in the controller; release logic, other projects and credential handling are
unchanged. All 111 release/domain/entry/source-closure tests passed. See
[pin receipt](release-pin.json) and [test output](release-guards.log).

Matching backend build
[43766785046](https://github.com/Zombieliu/mir2/actions/runs/43766785046)
succeeded. The exact artifact, protected candidate, fresh 6ba backup/clone,
ordinary isolated lifecycle, paired rollout and both original public cohorts
passed. Production is e0/Fo. See [release evidence and retained visual limits](../production/README.md);
global 1:1 and human acceptance stay open. This pin-only record does not replace
the actual promotion receipt.
