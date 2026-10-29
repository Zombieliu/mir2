# Native Mail exact quote ticket: 2026-10-04 source checkpoint

Bounded quote-ticket Source04 and focused checks are accepted. The cross-platform
goal remains active. Latest human direction is code only; UI/client work is held.

## Changed behavior

A checked unique quote token and full Native command ticket bind before queue
publication. Admission failure and dropped batch tails retire only their original
definitely-unsent token. Production commit_owned_frame validates the final fence
and calls actual start_send under the same short lock. Entry is irreversible,
including write failure; receipt publication precedes awaiting flush. Entry and
UI timeout share the monotonic clock. Queue acceptance does not prove transmission.
Typed receipt/Cost order, tombstones and poison survive all three same-stream
reset layers; true newer stream clears old state. Parcel observes poison before
reserving. Legacy Web request-time behavior and wire/DTO contracts are unchanged.

## Actual finite checks

| Check | Result |
| --- | --- |
| Windows production check | exit0 |
| Common Core Mail | 11 passed |
| Native UI Mail | 110 passed |
| Runtime Mail / Native ingest | 23 / 36 passed |
| Portable Mail | 30 passed |
| Shared-WASM type check | exit0 |
| Whole Windows binary suite | 812 passed, 0 failed, 3 default ignored |
| Web Rust host Mail | 5 passed, 2 default ignored |
| Explicit compiled parcel / compose replay | 1 / 1 passed |

1029 passing Rust test executions are not1029 unique scenarios. Seven positive
checks in earlier snapshots of this phase carry by exact consumed inputs. Windows
and host/replays ran on Source04. Retained actual JS data covers42 parcel sessions,
294 full outputs and11 Page/wrapper compose captures. No new JS execution is claimed.

Initial Native fixture:109 passed/1 failed due to missing required queue resource;
Source02 initializes it explicitly. Windows Source02:811 passed/1 failed/3 ignored
because the test assumed capacity zero while production enforces minimum8. Root
fills the real eight-entry normal lane. Its Source03 repair then failed compile101,
zero tests, with wrong MailLockItem enum; Source04 adds two bytes to MailLockedItem.
The static-review oversight and all real failures/logs are retained. Production
queue behavior and original assertions remain unchanged.

## Frozen input and closure

467 current files,467 final archives and467 before archives match:12 changed,
455 protected, zero new product files. All84 references/73 immutable runtime pins/
4 Next metadata/22 lexical-realpath aliases match. Actual spawn/exit/close was
observed for all28 known Cargo/probe children, absent at final Root snapshot.
No product process was killed. Node checks do not claim .NET Dispose or Policy B
production-build acceptance. Independent review:zero remaining confirmed P0/P1/P2
for this bounded scope, including the final result/carry evidence review.

QA root: C:/mir2-cross-platform-storage-20261002/repo-qa-native-mail-quote-ticket-01.
Source: root-full-candidate04.json,428904 B,
3370e6644a075f0eaccb926b7729d03e5bd035af175ba8c433c1ec88a3b4df53.
Acceptance: root-source-acceptance01.json,21941 B,
a8d9e8d49af4dbf69f0c79aabce8a8312b550511ab114a0d9fd48f6820b7674a.
The same QA folder preserves exact carry, coverage qualification, per-job guards,
logs, all three failure records and owned-child closure.

## Limits and next work

Entry tests invoke the production commit through controlled Sinks, not the entire
connected quote socket loop or live client. Runtime critical refusal poisons UI
immediately; termination waits for the next poll after current writer flush. A
permanently blocked flush is not interrupted. Dedicated quote-receipt byte-cap
refusal-to-poison coverage remains for the combined terminal follow-up. Existing
invalid-Cost and protected marker/ACK byte tests are separately labelled.

Next is common send-flight state and typed Native MailSent ACK with complete-draft
ABA generation and exact pending binding. M14c shared composer/editor/painter and
thin platform text adapters follow. Fresh Native/small-WASM/three canonical/Next/
package production builds must share a new accepted fingerprint under unchanged
caps. Actual gameplay/live ACK/save/relogin/public delivery, Android/iOS device,
human acceptance, Candidate and whole goal remain open.
