The bounded Guild rank rename implementation is source-frozen against GN `0e22935261beea4240b2dc32e7faf5a75cda7a08`. Root must forward-check only the two routing hunks onto its P3-integrated source; the full old GN routing file must not replace P3. This delivery is API/model/UI-helper Candidate evidence, not native click, screenshot, paired-network or human acceptance.

The complete eleven-file patch is `guild-rank-native-scoped.patch`, SHA256 `c7b93b789138853f0beee5d94330cf603c23b4cf484fc6a3178eed62efe60c0e`. The separately frozen routing patch is `routing-guild-rank-terminal.patch`, SHA256 `60a6954a86da7dc78a32626650a9bfbf4b7fc89b72e5a0e12e9adef37a88a74d`. `FROZEN-DELIVERY.json` contains every before/after file SHA, exact run command/input/receipt SHA and all raw failures; its SHA256 is `b43c6f12369fb9912541b8aeea83f0055defa7587c86feb829e4cf1a323e5a2f`.

All twelve Cargo runs used Rust 1.95.0, offline, locked, at most two jobs and `C:/mir2-build/gateway-tests-r51`. Every run's tracked source was byte-stable throughout execution. No Cargo/rustc process was present at the final process inventory. Git remains uncommitted at the assigned base; no release, installer, network, service or player-save work was performed.

| Exact project-relative file | Change |
| --- | --- |
| `apps/game-client/client-bevy/src/social.rs` | Merge source status7 into only its actual nested numeric rank; preserve other ranks/members; update own rank model/options; exact raw spaces/UTF16; local request proof/scope/strict timeout. |
| `apps/game-client/client-bevy/src/crystal_ui/guild_panel.rs` | Capture real editor actor/rank/text/scope; source-reply/hidden-timeout cleanup; preserve actor and clock after membership reset; two narrow legacy fixture corrections. |
| `apps/game-client/client-bevy/src/crystal_ui/overlays.rs` | Dedicated bounded rename admission/selection context; exact source deadline in focus/Save availability; generic type3 route cannot bypass context. |
| `apps/game-client/platform-windows/src/gameplay_bridge.rs` | Send the unchanged ordinary type3 packet only for its retained valid context; reject known stale/expired/unscoped input; known local send/discard failure retires only that exact context. |
| `apps/game-client/runtime/src/lib.rs` | Only a three-line cfg(test) child-module declaration in existing private native-data tests; byte-removal equality against baseline verified. |
| `apps/gateway/src/routing.rs` | Only two scoped hunks: actual type3 command flag before trusted durable handling, then preserve its original actor status7 before canonical Guild metadata. No terminal-test wiring is required here because the new test is an independent Cargo integration target. |
| `apps/game-client/client-bevy/src/social_guild_rank_tests.rs` | Seven real typed `ServerPacket` checks, including three ranks/four members, nonzero nested index and malformed required source fields. |
| `apps/game-client/client-bevy/src/crystal_ui/guild_rank_flow_tests.rs` | Twelve typed-cursor/editor-helper checks, including exact actor/index/raw text, metadata non-ACK, same-name reply, old proof, local scope, strict >5000 ms and hidden cleanup. |
| `apps/game-client/platform-windows/src/guild_rank_wire_tests.rs` | Four production mapper/local transport boundary checks; closed sender, other editor, known stale/expired queued input, and accepted send without a fake success. |
| `apps/game-client/runtime/src/native_data_path_tests/guild_rank_ingest_tests.rs` | Four actual bounded native FIFO/reset/main-thread consumer checks, reusing existing `ingest_app`. Exact packet JSON has typed protocol counterparts in the client tests; no fake queue or native production change. |
| `apps/gateway/tests/shared_guild_rank_terminal.rs` | Three ordinary authenticated Gateway wrapper checks with isolated prepared Guild fixtures: exact actor source7 before255, repeated same-name requests, denied no-success; cross-zone member-only delivery. |

The prior six-file durable authority slice, `native_ingest.rs`, all Cargo manifests/locks, Zone files, global docs, configuration and wire/durable schemas are untouched. Their applicable baseline hashes and exact eleven-file Git status are in the frozen receipt. Exact final source bytes are under `final-source/mir2-web3`; RED inputs remain separately copied under their run-specific `*-source` directories.

| Raw run | Outcome | Evidence |
| --- | --- | --- |
| `client-red-01` | 7 pass / 9 fail | Genuine list replacement, own rank, status7 rejection/correlation, same-name wait, hidden timeout, old Guild scope and source name admission failures. |
| `client-green-01` | 16 pass | The original semantic RED test input bytes were unchanged. |
| `gateway-red-01` | 1 pass / 2 fail | Durable commit occurred, but actor source7 was stripped for changed and repeated same-name replies; denial passed. |
| `wire-red-01` | 1 pass / 3 fail | Dedicated unsent pending leaked, other editor rollback lacked exact context, and known stale/expired queued input was transmitted. First Windows dependency compilation took 1406.547 seconds. |
| `client-red-02` | 79 pass / 4 fail / 1 ignored | Two real additional failures: missing nested source fields defaulted onto rank0; membership reset erased current owner/clock. Two legacy fixtures lacked newly required Guild/owner/rank context or expected the old zero local epoch. |
| `client-green-02` | 83 pass / 1 ignored | Same broad Guild scope after the two fixes; senior-rank denial and full leave reset assertions retained. Existing ignored test is `native_shell_ui::i18n_visual_tests::guild_name_and_sabuk_npc_pages_render_offscreen`, which needs real assets/GPU and was not run. |
| `wire-green-01` | 4 pass | Original four wire RED tests pass, no optimistic state or source ACK fabrication. |
| `fifo-green-01` | 4 pass | Real source7 then255 FIFO in one main frame; metadata-only non-ACK; SceneReset retention; DataReset retires old queued reply and pending. Production native queue was already correct and remains byte-identical. |
| `gateway-green-01` | 3 pass | Original failures eliminated; additional explicit ordering fence requires source7 before full255 metadata. |
| `client-social-adjacent-01` | 35 pass | Existing group/trade/Guild packet and pending semantics remain passing. |
| `wire-adjacent-01` | 99 pass | Existing production gameplay bridge checks remain passing. |
| `gateway-guild-adjacent-01` | 4 lifecycle/bank + 4 notice + 8 rank pass | Ordinary cross-zone bank, recruiting/creation lifecycle, refusal/permission/failure fences, notice, authority rank, durable reopen/new factory and relogin checks all pass. |

Counts above overlap and must not be added into a parity percentage. `.log`, `-input.json`, `-receipt.json` are preserved for every run. The earliest two runner receipts had an empty convenience `failures` array because the initial parser expected captured-output headers; original raw panic lines and test results are unchanged. The final frozen receipt separately derives every failure from raw nocapture output. No Cargo fixture/compiler failure was hidden.

The freeze helper initially rejected the correct write set because Git porcelain v1 is repository-relative even when status.relativePaths is set. The unchanged failure was reproduced and raw-captured as `freeze-harness-prefix-red-01.log`/`.json`, then only the external helper was corrected to derive/validate the repository-to-project prefix. This changed no source or test evidence. `git diff --check` and a read-only `git apply --reverse --check` on the complete frozen patch both passed.

The immutable source plan at `../guild-rank-native-plan-01/PLAN.md` remains SHA256 `a9e9536f61de36939ba8cb8ba8f36e96e09d4969044551a3649a9a1bcae44bac`. Its Crystal excerpt/source index binds `PlayerObject.cs` 9945–9961 (bit1, equal-or-lower target, raw UTF16 3..20 and backslash), `GuildObject.cs` 377–427 (same-name set/save/broadcast, status7 with actor and one actual nested rank), server packet shape 4497–4531, `GameScene.cs` 5813–5857 and `GuildDialog.cs` 1815–1860/1898–1936 (merge rather than full replacement; strict deadline). Client command3 and server status7 are distinct. No source255 projection is relabeled as status7.

Remaining gates are explicit:

- Actual native mouse/keyboard click and rendered editor/list acceptance, production fonts/assets and screenshots are unexecuted. Tests call the actual Save helper/hidden process, mapper and consumer, not a physical click.
- UiReadModel own-character Guild/rank label and AOI ObjectPlayer rank label remain open; no cursor/Zone schema changes were made.
- The ordinary protocol has no nonce. New source arrivals and cached projections can be distinguished locally; delayed old same-content replies across A/B/A cannot be uniquely identified. Each valid repeated request still commits revision+1 according to Crystal.
- Other rank commands/status6/status8, option5 pending, kick/leave management, Guild war and kill-EXP closure remain outside this slice.
- P3-plus-native final integration, paired TCP/WSS, native binary/installer build and rollout are root-owned and not accepted by this delivery.

Source is frozen. Do not extend these files until root review and explicit reassignment.
