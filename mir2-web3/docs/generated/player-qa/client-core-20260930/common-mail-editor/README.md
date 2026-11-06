# Shared Mail editor/painter: Progress56

Latest build follow-up: [Progress57 combined build](../shared-mail-combined-build/README.md) accepts the finite Native/Core/three-renderer/Next03/standalone artifacts. The Source06 checkpoint below remains historical; current client/UI/mobile/Candidate and goal acceptance stay open.

2026-10-04 UTC. Source06 and this finite check round are accepted. Production combination, actual client/UI, mobile devices, whole Candidate and goal remain open. Human instruction: continue code; do not operate UI.

Windows and Web now consume the common Rust FriendTextEditor / MailLetterEditor and letter/parcel/recipient/gold/feedback painter. Thin platform adapters forward browser/native input to the existing shared editor and fullraw draft clock. The Web bridge adds a checked readonly draft_gold projection; wallet balance supplies only the prompt ceiling. Current attached gold, owner, generation and fullraw proof determine the actual snapshot/submit. The existing ABI1 transaction DTOs and legacy compatibility path remain intact.

The actual pure Bevy layout fixture exposed negative font leading and a one-pixel parent border offset. The shared painter now derives the visible caret/selection from real shaped text geometry and viewport intersection. A real long Unicode soft-wrap/scroll fixture checks this independently of the draft clock. The original failing caret assertion and the stronger caret predicate remain unchanged. The two final test-only repairs apply a 0.0001-pixel tolerance to the new selection measurement and make the overflow fixture exactly 500 UTF-16 units; no original caret predicate was weakened.

Actual checks (Source06)

- Portable editor: 29 passed; Native editor: 27 passed, serial.
- Runtime host: 21 passed; one historical M13 environment fixture remains ignored/unexecuted. These use pure App/layout state without OS WindowPlugin, real clipboard or renderer launch.
- Small Core host: 12 passed; its three normally ignored compose/parcel/SendSlot fixtures were each explicitly replayed once. Total Rust passing executions: 92 across configurations, not 92 unique tests.
- Actual generated small Core + current Page AST and controlled ports: Node 86/86; TSC noEmit/nonincremental and runtime shared-WASM check exit 0.
- SendSlot: 84 actual generated-Core transcript session arrays, 1,508 entire inputJson/outputJson string pairs replayed byte-for-byte in Rust. Compose 19 cases and parcel 47 sessions / 361 rows are separate semantic checks.
- Fresh raw Core 425,463 B; actual bindgen 0.2.118 JS 17,267 B, generated WASM 259,714 B. WASM must remain strictly below 262,144 B (margin 2,430 B); JS limit 204,800 B. Only the nonexecuting name section is stripped by the existing release pipeline.

Source and evidence

470 source inputs comprise 468 existing inputs (17 changed, 451 protected) and two new files. Root rechecked 89 references, 73 runtime artifacts, four Next metadata files and 22 real alias bindings. All source/archives and protected boundary pins match; no unrelated work was reverted. Current source accepted here is distinct from the unchanged previously built EXE/renderer/standalone artifacts.

Immutable evidence root: `C:/mir2-cross-platform-storage-20261002/repo-qa-common-mail-editor-01/`.

- `root-full-candidate06.json`: 405275 B / `3a23454da2056630894e0531355ec5bbb2bd0c120574f9cd9b5e613be77ba566`.
- `root-final-focused-validation01.json`: 187914 B / `793b2783192107e065d7fccbc856dc040bb0a2ab3a53e764e1074c683e475f2a`; 12 current positive jobs, no qualified carries.
- `root-final-actual-independent-review01.json`: 157196 B / `c0ad307cd6e8529a697dc903dd4c2fd73fcec32646188d2147fb159dbc72e10d`; 0 confirmed P0/P1/P2.
- `root-source-focused-acceptance01.json`: 5789 B / `ec95631968caad9dca0d233228b516bd35bcf603235d848d62254fab0a713168`; bounded source/focused acceptance only.
- `root-child-closure01.json`: 30581 B / `fb5e7767458503e3338f68e5122fdc578ccf8e08784bd2334c1788893fd6c72e`; 44 recorded owned child entries observed exit/close; 43 distinct owned PIDs absent at the recorded historical snapshot. No product process killed or protected native process queried.

The compiler syntax failure, zero-filter run, early Node failures, original caret failure/diagnostic and final fixture failures remain nonpositive evidence. Earlier snapshots and results were not overwritten.

Next: build the current Windows EXE, production small Core and manifest, three canonical renderer variants, Next and isolated standalone package; retain all fixed size budgets and verify their identities. Actual ordinary gameplay/login/save/relogin, browser/native IME/clipboard, touch and physical Android/iOS lifecycle need separate client/device evidence after UI operation is permitted. This pure code round proves none of those gates.
