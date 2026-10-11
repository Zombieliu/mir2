# Local NPC input admission — bounded candidate

This round fixes ordinary script-input admission in the personal simulation.
It does not complete P7 shared-world invocation, CHECKHUM, delayed GOTO,
Source CAS/world outbox, full P1–P7/Mentor, or native/human acceptance.
R25/feed20 is already public; this new backend change is not yet published.

## Result

An undisplayed `@@` target could previously issue a prompt before the menu
link check. A confirmation could also execute a displayed input link without
having requested its prompt. Base-label matching accepted additional target
arguments, and cloned `NpcStateResource` could restore a consumed display.

The new private `DisplayedNpcMenu` is established only by normal server
`set_dialog`. An input request must match that actual full display and actor
binding. `PendingNpcInput` then authorizes one confirmation, consumed before
script mutation or a fallible save. Neither resource is Clone, serialized,
part of a character save, or restored from a UI/checkpoint copy. Copying either
a link page or an input page alone cannot create this permission.

Bindings capture the actual local player/NPC Entity generations, strictly
read account and selected character, local map index/file, NPC coordinates,
script key/path, and imported UTF-8 raw text and execution representation
hashes. These are not original disk-byte, Zone Join, online-epoch, instance,
life, or population authority receipts.

Display replacement, dismissal, logout/disconnect (including a failed save),
load, relocation, entity rebuilding, actual death, accepted revival, and the
existing rollback paths invalidate permissions. Wrong NPC/argument packets
preserve a legitimate current prompt. Healthy vitals synchronization also
preserves it. Full-target authorization retains empty arguments, whitespace,
and trailing bytes; the execution argument parser is not an authority check.
Both `[@page](args)` and `[@page(args)]` parse without losing the outer syntax.

The original Crystal `NPCScript.cs` button check precedes input requests and
matches the complete button. Original `MirConnection.NPCConfirmInput` does
not itself require a previously requested, single-use prompt. The latter is
deliberate security hardening, not a claim that original Crystal had it.

## Verification and retained failures

Evidence root: `C:/mir2-build/p7-npc-dialog-admission-20261011`.
Tests use Rust 1.89 and the locked dependencies. There are no new ignores or
relaxed assertions. All checks use isolated stores, not real player saves.

| Check | Actual result | Evidence |
| --- | --- | --- |
| Initial 8 canonical input checks, before repair | 2 pass / 6 fail | `baseline-01.log` |
| Added review cases against the partial repair | 8 pass / 3 fail | `review-baseline-01.log` |
| First integration compile | failed namespace lookup | `fixed-01.log` |
| Corrected first slice | 11 pass / 0 fail | `fixed-02.log` |
| Initial lifecycle slice | 19 pass / 0 fail | `lifecycle-01.log` |
| First broader NPC regression | 278 pass / 3 fail | `adjacent-01.log` |
| Exact parent `02f800e1d8ebe13053d487729d7203559e6baa84`, same 3 tests | 0 pass / 3 fail | `parent-regressions-01.log` |
| Repaired focused input/parser/old fixtures | 28 pass / 0 fail | `repair-focused-01.log` |
| First complete broader NPC/lifecycle/health/movement regression | 317 pass / 0 fail | `adjacent-02.log` |
| Legacy bracket confirmation on c6b460b0ad | 0 pass / 1 fail | `legacy-confirm-baseline-01.log` |
| Final focused input/parser/old fixtures | 29 pass / 0 fail | `repair-focused-02.log` |
| Final broader NPC/lifecycle/health/movement regression | 318 pass / 0 fail | `adjacent-03.log` |

The 29 focused checks are a subset of the final 318 distinct test names, which
include 24 new checks. All earlier 317 passed names remain a subset; counts are
not added together. `LOCAL-REGRESSION-02.json` binds the actual original logs
and changed source hashes, retaining `LOCAL-REGRESSION-01.json`. The parent comparison temporarily
replaced only 14 Root-owned tracked Rust files with their exact Git parent
bytes, then restored every captured byte; `parent-baseline-01/subject.json`
and `RESTORED.json` retain the checksums and restoration proof.

The three parent failures were stale preparation: an already-omitted condition
was checked against the parsed script instead of sealed pre-parser source;
the old buff fixture used invalid token count/enum case; and an independent
DefaultNPC death event had not been settled before the silent merchant check.
Preparation now uses the actual sealed source, valid original buff syntax,
and a normal KeepAlive to settle death. All original result assertions remain;
the omission test also checks that the verified parsed line is empty.

Canonical NPC2134, `GuildTerritory/GA0/GTAdmin-GA10`, GA10 at (13,27), provides
the real Main → summon → agitonerecall → input packet path. The tests assert
the genuine MOV input value, not successful GTRECALL or shared-world effects.
Other tests cover cross-runtime/link/prompt copies, wrong NPC/arguments,
logout/reentry, disconnect, death/revive, NPC replacement, local map binding,
trusted map round trip, and actual injected save failures with rollback.
The parameterized input-page check is explicitly a synthetic server display
using that canonical NPC body through real CallNpc/NpcConfirmInput packets:
the imported catalog currently has no parameterized input link.

Read-only review found and prompted both the clone-link negative and complete
bracket/parameter packet check. The new Linux workflow gate includes all 29
focused cases alongside every original release gate.

A subsequent read-only review found that confirmation normalized the full
legacy `[@@page](args)` before full-target authorization, which incorrectly
added another label marker. Its real packet test failed on c6b460b0ad. Keeping
the raw trimmed target now lets the existing matcher/parser handle both
legacy and whole-target brackets without discarding any argument bytes. The
review reports no new blocker. Superseded Linux run38158000833 was cancelled
after this known failure; it is not qualified or promoted.

## Publication and remaining scope

Implementation and local regression are complete for this bounded candidate.
Exact-source Linux CI/package qualification and a normal drained Gateway
switch still remain. There is no client wire/schema change or new installer
requirement. Current public delivery remains app d917/engine b439/feed20 and
Gateway4006; the existing running player session is not forcibly disconnected.

The wire format has no prompt nonce. A delayed confirmation after the server
legitimately issues a new identical prompt cannot be distinguished by this
local mechanism. General copied-menu navigation, trusted shared Join/life/
population capture, ordered/deferred invocation, and world-effect publication
remain separate work. Native dense-combat/laptop/player and full-game human
acceptance remain open. The expired 12-hour heartbeat remains paused.
