# V2 attack deadline polling fix

The complete four-file regression passed **210/210** in one invocation (main185 plus other25), exit0, with all nine driver raw inputs identical before and after. The aggregate is `5ba46ae4a729c4ae8514721284da0f91d796aff247cd7c017b206790be88e2d9`.

Only V2 attacks opt into one final synchronous read of their existing acknowledgement predicate. It credits only evidence already recorded at or before the original inclusive900ms boundary; it adds no sleep, send, deadline extension or opportunity. Exact request matching, the owner/observer damage proof, ambiguity quarantine and ordinary wait semantics stay in place.

The old main175 test prefix is byte-exact. The earlier complete suite had200 cases (175+25); this change appends10. Initial main185 and adjacent25 logs are separate preliminary executions and have no invented joint before-source capture. Their original filenames and the initial count wording are retained with an explicit correction.

The original Formal11 run remains **FAILED** with its two timeout rows unchanged. This patch and these software tests do not establish a new capacity pass, P0 acceptance or permission to implement P2. The changed Rust slow-stage context also requires a new source-bound Release/full lifecycle; prior d879 evidence does not test it.

Evidence: [full execution receipt](full/frozen-full-driver-receipt-v1.json), [complete210 TAP](full/frozen-full-driver-v1.log), [captured before inputs](full/frozen-full-driver-before-v1.json), [final integration review](independent/attack-deadline-poll-fix-final-integration-review-v1.json), [count correction](independent/attack-deadline-poll-fix-independent-review-count-addendum-v1.json), [focused old-code red](focused/deadline-regression-before-fix-v1.log), [focused new-code green](focused/deadline-regression-after-fix-v1.log), [Root functional summary](integration/root-attack-deadline-functional-validation-v1.json), [functional independent check](independent/root-attack-deadline-functional-independent-review-v1.json), [explicit whitelist](public-whitelist-v1.json).

Raw source copies are text artifacts; execution scripts retain their original evidence paths and are not standalone portable launchers. The Root summary links the separate Rust8 log through its original pinned reference; this package does not duplicate that full log.
