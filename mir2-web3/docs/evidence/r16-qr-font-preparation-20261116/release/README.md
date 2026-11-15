# Exact-source release preparation

The release controller now targets application source
`6ba38557ddb298df72cb9e4a23c48caf542a598e`, rather than the superseded `ef6d20c095`.
The production rollback target remains `dpl_AWssgszkpKJznMksLsP7hiRFK6U8`
(`20de5ea4fd93b3c61e4aa590dca2ea2d8f5d2339`). This preparation does not publish
or promote a deployment.

The complete base-to-application change contains 63 commits and 386 paths.
Two independently fetched GitHub comparisons (296 and 105 paths) match full local
Git commit lists, path statuses and blob identities. The overlap is composed by
the existing source-closure verifier; neither segment reaches GitHub's 300-file
truncation boundary. See [source closure](source-closure.json).

The previously qualified local build matches all 2,181 committed application and
library files, with the single already locked generated-input exception. Both
recorded full local game receipts match the same build. The 65,861-byte Worker
retains SHA-256 `4ea2465084ba90eafe9f969a5ba3c42c8b0d2ff61fb05febb14a72f21916de40`.
See [release pin](release-pin.json).

All 111 release, domain, entry and source-closure tests passed. The hosted release
workflow now also runs the source-closure suite. Controller comparison confirms
only the allowed source/pin fields changed; the baseline and other release logic
remain unchanged. All 29 recorded unrelated WIP hashes remain intact. See
[validation](release-validation.json) and [test output](all-release-guards.log).

The matching backend build is run
[43755627882](https://github.com/Zombieliu/mir2/actions/runs/43755627882).
Its Gateway packaging has completed; the promotion backend is still building at
this record. Artifact qualification, protected Web staging, guarded paired
rollout and the original public cold cohorts remain pending. The full project
1:1 and human acceptance gates remain open.
