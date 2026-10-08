# Fresh item batch grants — 2026-10-09

Six final actual serial CargoGuard calls on source03:

| Call | Passing executions | Failed / ignored |
| --- | ---: | --- |
| grant-tests-03 | 8 | 0 / 0 |
| quest-regression-01 | 115 | 0 / 0 |
| mining-regression-01 | 9 | 0 / 0 |
| fishing-regression-01 | 23 | 0 / 0 |
| npc-regression-01 | 35 | 0 / 0 |
| simulation-tests-01 | 18 | 0 / 0 |

**208 passing executions, 204 distinct fully qualified test names, 16 new tests.** Four repeated names across broad filters are documented in `result-audit-01.json`; executions are not unique scenarios. New tests are batch planning8, quest4, mining2 and fishing2, each verified by name in original stdout. All actual calls used toolchain1.95.0, locked/offline, jobs1, nonincremental compilation and serial test threads.

The nine authored sources match `authored-source-03.json`; all460 declared inputs were hashed before and after each final call and in the final review. All451 protected inputs match production parent `4644753b78c201cfa7dccc6cf6d694ce10021235` using canonical Git clean filters. This is a bounded simulation input declaration, not the entire workspace or toolchain. The independent source01/02/03 and final [actual result review](independent-result-review.json) are retained.

Original Guard executable/source, license, Cargo/rustup, PowerShell, probe and policy pins remain unchanged. Each final nonce records FORWARD, actualGetVolumeC and PolicyB completed/exited/disposed, exit0. Minimum sampled C free bytes195130953728 exceed the original53687091200 threshold. Independently recomputed sampleBegin-to-child conservative freshness is at most945.9200ms, within2000ms; sampleEnd-to-child is84.8420ms. Recorded durations are observations, not deadline promises.

`raw-evidence.json` records106 exact byte-checked copies, including eight actual Guard calls (six final and two historical), all40 original nonce files, raw logs, policies, source/authority bindings, helper snapshots and audits. The later independent final review is separately byte-checked by `independent-result-review-copy.json` and is not counted in that earlier manifest. `.gitattributes` keeps original bytes, including empty probe stderr files and trailing Cargo blank lines. No compiled cache is in this tree.

| Preserved historical record | Actual result | Repair |
| --- | --- | --- |
| grant-tests-01 / source01 | E0433 compile failure, zero tests | Four new enum references corrected to actual `crate::config::ItemGrade`; no assertions changed |
| grant-tests-02 / source02 | Four pass, four fail | Five nonexistent potion fixture names changed to the real `(HP)DrugSmall`; business assertions preserved |
| serial-wait-01 | No Cargo, no tests, no nonce | External Cargo remained busy during bounded45s wait |
| serial-preflight-01 | No Cargo, no tests, no nonce | Original runner refused overlapping foreign Cargo |

These attempts are excluded from final passing totals. The source-review helper's intermediate JavaScript parse rejection did not execute a shell command, mutate product sources or start Cargo; the final independent audit completed normally.

The private fresh-grant planner validates complete quantity/carrier/identity and all capacity before minting any new root. It retains existing compatible stack identities, rejects custody or nested-object imports, checks Quest slots independently, and publishes only the final whole inventory. Discarded IDs are burned, never recycled. Ordinary quests stage all carries and rewards before progress/currency/XP changes. Mining keeps purity and original successful swing cost; failed minting preserves nodes/pickaxe. Fishing keeps frozen random item stats and all final changed stacks; failed minting cancels autocast without success events or additional reel cost. Prior casting costs remain spent.

**Accepted scope: synchronous staged inventory publication plus controlled File UID protection.** Quest/mining/fishing account atomic persistence is not implemented by this patch. Full all-issuer migration, complete external/world/history census, old duplicate reconciliation, PostgreSQL authority and explicit unknown-publication recovery remain open. Existing common add/normalize and custody paths are not broadly rewritten. Default config/normal gateway do not enable the new binding; the public catalog remains disabled. No game, GUI, gateway, deployment, database or installer was run. Gathering/crops, ordinary workshops, side-management UI, construction/food and guild production remain subsequent work. [Implementation progress](../../SLG-PRODUCTION-IMPLEMENTATION.zh-CN.md).
