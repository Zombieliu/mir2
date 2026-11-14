# P5 native Guild rank rename: source-bound closure plan

Status: **READ-ONLY PLAN. No implementation, build, network run, native click,
service change or acceptance was performed for this plan.**

Observed root revision: `321316b791120a6fe549e4006623e63333a5543f`.
The six-file rank-rename authority Candidate is frozen separately and has not
been represented as integrated into this root. Its original RED, GREEN and
durable-failure evidence remain under `../guild-rank-implementation-01`.

All relative source paths below are resolved through `source-index.json`:
`root` is the crowded-combat-control checkout, `candidate` is the frozen
six-file final-source copy, and `crystal` is the local Crystal checkout.
Whole-file byte SHA256, sizes and excerpt ranges are recorded. Source excerpts
are local source material, not operational instructions. The parent owns
integration and all global parity documents. P3 owns routing/Zone edits until
an explicit handoff. This worker has edited only this new external directory.

## 1. Findings and intended behavior

The ordinary Windows rename button already emits the correct client command.
The frozen shared authority already commits an authenticated, permission-checked
rename and creates the correct one-rank status-7 event after durable commit.
Three independent client-facing defects remain:

1. Gateway social refresh removes the initiating actor's status-7 result.
2. SocialModel treats a one-rank status-7 event as a full roster, destroying
   untouched ranks; its completion matcher expects a nonexistent response
   `changeType` and the wrong rank index location.
3. The UI's 5-second clock does not retire that unmatched social pending key.
   An apparently enabled Save button can continue rejecting every retry.

The first proposed slice must deliver the original status-7 to the actor,
merge just its named numeric rank, retain the other ranks/members and restore
the matching editor to an operable state. A missing/denied reply must become
an **unconfirmed timeout**, not an optimistic successful rename. The original
full status-255 metadata projection may still refresh the model; it must never
stand in for the original rank-change event or manufacture request completion.

Public world rank labels are a separate upstream gap. Shared ObjectPlayer
currently contains an empty rank name. Fixing the private Guild roster alone
does not close Crystal's affected-member BroadcastInfo behavior or native
character-panel labels. Section 6 records those additional gates explicitly.

## 2. Crystal contract: command and result are different domains

| Contract | Exact source | Consequence |
| --- | --- | --- |
| Client command `EditGuildMember` type 3 means rename | Crystal `Server/MirObjects/PlayerObject.cs:9945-9961`; root `packages/protocol/src/packets.rs:341-346` | Do not confuse client 3 with server status 3, which means kicked member. |
| Permission is `CanChangeRank`, bit 1; current/equal rank can be renamed | Crystal `Shared/Enums.cs:1898-1909`, `GuildObject.cs:377-427`; frozen Candidate authority | Preserve authority permissions/hierarchy, including equal rank; no new routing-side authority. |
| Rename string is untrimmed .NET UTF-16 length 3..20, no backslash | Crystal `PlayerObject.cs:9951-9958` | Raw `" A "` and `"   "` satisfy this rule. Do not silently trim them or misread a whitespace rank as Guild exit. |
| Successful rename broadcasts `GuildMemberChange` status 7 with actual actor name and a single changed rank | Crystal `GuildObject.cs:403-426` | Preserve this event. There is no client-command echo field. |
| Envelope RankIndex defaults to 0; changed rank is in `Ranks[0].Index` | Crystal `Shared/ServerPackets.cs:4497-4531`; Candidate Gateway `shared_guilds.rs:346-355` | Correlate the nested rank index, not the envelope index. A target such as rank 2 is a necessary test. |
| Status 7 invokes a rank merge, status 255 invokes full-list replacement | Crystal `Client/MirScenes/GameScene.cs:5813-5857` | Never replace the whole roster for the ordinary status-7 rename. |
| Existing rank with matching index is replaced; own GuildRankName/options update only for that own rank | Crystal `GuildDialog.cs:1815-1839` | Other ranks/members remain, and an affected member gets their own new rank name without requiring a 255 replacement. |
| Reply calls UpdateRanks and resets the editor's clock to zero | Crystal `GuildDialog.cs:1839-1860` | A confirmed rename permits another valid edit immediately. Repeated identical names still reply and refresh. |
| Missing reply unlocks after `CMain.Time > LastRankNameChange` | Crystal `GuildDialog.cs:1898-1936` | The exact deadline is still busy; deadline+1 is available. Do not keep permanent pending solely because denial is delivered as chat. |
| GuildStatus assigns User.GuildRankName and MyRankId/options | Crystal `GameScene.cs:5860-5900` | GuildStatus is metadata, not type-3 command success; preserve own presentation fields. |
| Changed-rank online members call BroadcastInfo | Crystal `GuildObject.cs:418-420`, `PlayerObject.cs:4796-4820` | Public ObjectPlayer rank metadata must eventually update AOI viewers. NoNames masking is part of that source behavior. |

Crystal's GUI name check itself is only nonempty; the server enforces 3..20.
Adding the server's exact bound to local Save validation is a deliberate
frontend admission improvement, not a claim that Crystal's text box did so.

Status 7 is also the original rank-option update shape. It must not be globally
mapped to a fabricated client `changeType: 3`. The wire lacks a nonce or command
origin field; a matching local pending rename plus source event content is a
bounded completion contract, not an exactly-once request identifier. Delayed
A -> B -> A replies cannot be distinguished by inventing fields. Preserve the
frozen authority's revision+1 per effective valid request, including same-name
requests; do not add deduplication or revise durable schema in this follow-up.

## 3. Ordinary path, exact source locations and defects

### 3.1 Windows input and pending admission

`client-bevy/src/crystal_ui/guild_panel.rs:820-850` validates current permission,
selected rank and local clock, then enqueues:

```text
GuildEditMember { change_type: 3, rank_index: selected,
                  name: "", rank_name: exact editor text }
rank_name_ready_ms = now_ms + 5000
```

`crystal_ui/overlays.rs:2690-2742` correctly rejects a full intent queue before
creating social pending. However it stores only the generic
`GuildMember { change_type, rank_index, name }`, discarding desired rank name.
For rename the stored name is empty. That key can outlive the editor clock and
silently reject later retries. Preserve capacity/duplicate protections while
adding a type-3-specific pending record rather than expanding every unrelated
Guild operation or changing the outgoing protocol.

`guild_panel.rs:148-215` obtains the actual current player name and executes
early returns when the panel is hidden/out of game. Expiration/reconciliation
must occur before a hidden-panel early return. `:379-400` uses a timer to focus
the field; `overlays.rs:14533-14582` enables Save on permission/hierarchy only,
so displayed availability and pending admission disagree. `overlays.rs:8888`
resets the clock on selection, but does not retire the generic pending key.

`platform-windows/src/gameplay_bridge.rs:3038-3063,3177-3184,3230-3283` forwards
the exact ordinary intent to the wire and handles a failed local send. Any new
rank pending key must be retired by that exact local failure branch, with its
draft/focus restored only if the selected rank and current draft still match.
Do not clear other pending social operations or another rank's draft.

### 3.2 Shared commit and actor terminal stripping

Root `gateway/src/routing.rs:14632-14633` chooses the shared Guild handler for
ordinary Guild commands. `gateway/src/shared_guilds.rs:44-80` obtains the
authenticated current presence and canonical identity. Root321316 has no
type-3 production arm yet; the frozen Candidate adds it at `:304-373`.

On successful durable commit the Candidate derives actual actor/rank/member
data from the committed record, builds status 7/top index 0/nested target index,
increments the shared coordinator generation, queues the event to actual
same-Guild online presences other than the actor, and returns it to the actor.
The six frozen files and their failure/CAS/freeze protections must remain
unchanged in the proposed native slice.

Root `routing.rs:14790-14827` then refreshes social state after a Guild command
or generation change. `:14822` removes **all** GuildStatus/GuildMemberChange in
`command_packets`, including that actor terminal, and `:14823` replaces them
with full shared projection. The peer event is in separately collected/queued
Zone packets, so the same filter does not erase it in this branch.

Proposed routing hunk: retain/reinsert only the trusted successful type-3
handler's real status-7 terminal while applying the existing metadata filter.
Keep canonical GuildStatus/full status-255 refresh, then append the preserved
terminal in a known order. Preserve original event fields byte-for-byte; do not
mint status 255 as a rename result. Do not preserve arbitrary personal/debug
Guild deltas or broaden mentor/marriage handling. A denied commit has no status-7
terminal and must not get one because metadata happens to match the request.

### 3.3 SocialModel merge and completion mismatch

`client-bevy/src/social.rs:503-542` parses every ranks array then replaces all
`guild.ranks` and `guild.members`. It never branches on `status`. A peer receiving
one changed rank consequently loses the untouched ranks/members immediately.
`:548-556` updates own permission bits if present, but not own `guild.rank_name`.

The result type has **no `changeType` field**:
`protocol/src/packets.rs:2114-2118`; binary encode at `:3545-3559` matches Crystal.
Server enum fields serialize camelCase (`:1973-1978`). This is not a casing
mismatch. `social.rs:815-816` nevertheless expects changeType and top rankIndex;
`:911-919` requires these to equal the pending client command. A typed rename
reply never completes the present type-3 pending key.

Proposed SocialModel hunk:

- Parse status explicitly. For status 7 validate a single changed rank and
  replace the existing numeric rank in place; reconstruct flattened members
  from retained ranks. Do not drop other ranks, members or Guild metadata.
- Preserve canonical raw rank text separately from member/name trimming.
  Handle GuildStatus's nonempty whitespace rank without resetting membership
  (`:435` currently treats it as exit; `:1023-1025` trims it).
- Update own rank_name/options when the changed rank is own numeric rank,
  with current membership metadata. An unrelated rank rename must not change
  own rank. Do not infer the owner from the first member or envelope index 0.
- Keep status 255 as the actual full-list projection. Unknown/duplicate rank
  deltas must be bounded and must not manufacture a terminal. A missing local
  rank requires a canonical full refresh, not whole-table replacement with the
  delta. Reuse normal GuildRequestInfo if a recovery request is needed.
- Add source event evidence for status 7/nested index/raw proposed name/actor,
  not a fabricated client changeType. Match only the dedicated rename pending
  key under the current Guild/session scope; other pending stays intact.
- Status 0..6/8 must never complete type-3 rename. Their complete source parity
  is outside this narrow slice; do not silently implement kick/promote/options.

`social.rs:1196-1248` has a one-rank status-6 serialization fixture, which cannot
prove status-7 merge. `:1510-1539` uses handwritten response changeType for a
generic member pending test. Those tests are not evidence of a real typed
rename reply; new RED must serialize the real ServerPacket shape.

### 3.4 Main-thread/native FIFO is not a demonstrated new defect

`platform-windows/src/gateway.rs:3780-3794` applies each social packet to its
network SocialModel cursor and pushes a serialized SocialModel per packet.
`runtime/src/native_ingest.rs:473-492` **does not coalesce SocialModel** and
`:529-545` marks it critical. `runtime/src/lib.rs:3037-3054` drains every queued
SocialModel in order and calls `apply_authoritative` for each.
`social.rs:298-307` preserves main-thread pending and reconciles each update.
Thus within normal queue capacity, terminal 7 followed by full projection 255
in the same frame still applies the terminal separately. It is not overwritten
by snapshot coalescing. A focused FIFO regression is useful, but no production
native queue rewrite belongs to this plan by default.

General queue/byte exhaustion can reject a critical model and the producer
currently ignores the push result. That is a separate backpressure gate. Do not
expand the write set or call it the cause of normal rename failure without a
focused failing scenario. Use the existing private native queue test guard,
not a new public fixture API.

## 4. Pending/editor behavior to implement after approval

Use a dedicated client-only rank-rename pending variant or helper with:
actual requester name, current Guild scope/local membership generation,
numeric target rank, exact desired rank text and local deadline. Keep the
ordinary outgoing type-3 packet unchanged. Do not put a new schema/nonce into
server ranks, presence, Zone or save data.

At confirmed source status 7, require actual actor/nested index/desired text
and current pending scope. An unchanged desired name is still a confirmation.
Retire only matching pending and update the corresponding editor from the
canonical rank. Valid peer deltas update the list but do not complete the
actor's different request. If a peer edits the selected rank during an active
local request, keep the local draft separately until matching completion or
unconfirmed timeout; do not silently overwrite the submitted draft.

At `now > deadline`, retire only that rank-rename pending, label the result
unconfirmed and permit a new valid request. Do not claim timeout means durable
failure or rename success. Source denials are ordinary chat, with no correlated
negative reply; do not match localized chat strings as transaction IDs. A
local wire-send failure is a known failure and can restore that exact draft
immediately. Session reset retires all old pending; Guild change/exit retires
only old Guild rename context; map/scene transition by itself must not erase
the current Guild or falsely complete pending.

The editor's enabled/focus state must use the same availability predicate as
Save admission. Clock comparisons should preserve source strict-after behavior;
ACK resets the matching clock without waiting five seconds. Existing selection
may reset a UI clock but must not bypass a still-unconfirmed operation for the
same numeric rank. If different-rank edits remain allowed, keep their exact
pending contexts independent and bounded. A conservative local same-rank
single-flight guard can also prevent an option type-5 request overlapping a
rename's indistinguishable status-7 reply; this is an explicit client safety
choice to review, not a wire guarantee or source global single-flight policy.

Residual protocol limitation: status 7 contains no Guild ID, request nonce or
origin command. Content matching and the existing authenticated presence/queue
fences do not prove uniqueness against every old same-name reply. Document
A -> B -> A and concurrent rank-option ambiguity rather than fabricating
changeType. No authority exactly-once/CAS weakening is proposed.

## 5. Minimum proposed write/test set, conditional on parent approval

First writer owns one bounded native-terminal/list/editor slice. P3 must finish
and hand off routing first; source bytes and new base must then be re-bound.

| Existing source file | Minimum necessary hunk |
| --- | --- |
| `apps/gateway/src/routing.rs` | Only trusted type-3 status-7 preservation/order around social refresh. No authority changes or broad refresh rewrite. |
| `apps/game-client/client-bevy/src/social.rs` | Status-7 rank merge, raw rank parsing, own-rank update, dedicated pending/event reconciliation and small test-module declaration. |
| `apps/game-client/client-bevy/src/crystal_ui/guild_panel.rs` | Rank pending context, confirmation/strict timeout cleanup before early return, draft/timer/focus availability. |
| `apps/game-client/client-bevy/src/crystal_ui/overlays.rs` | Dedicated rename queue admission and Save enabled predicate; no generic social-pending rewrite. |
| `apps/game-client/platform-windows/src/gameplay_bridge.rs` | Exact dedicated rename pending rollback after local send failure. |

Focused new tests should be separate modules/files, not added to giant inline
test blocks. Suggested reviewable locations (not created by this plan):

- `apps/gateway/tests/shared_guild_rank_terminal.rs`: actual authenticated
  ordinary command through integrated authority + ordinary Gateway wrapper.
- `apps/game-client/client-bevy/src/social_guild_rank_tests.rs`: real typed
  ServerPacket JSON merge/pending cases; cfg(test) child module only.
- `apps/game-client/client-bevy/src/crystal_ui/guild_rank_flow_tests.rs`: private
  editor helpers / intent queue / clock and preserved unrelated pending.
- A small private bridge test child module if the existing closed-channel
  helper cannot be used externally; no production test-only entry API.
- `apps/game-client/runtime/src/guild_rank_ingest_tests.rs` plus only a cfg(test)
  module declaration in `runtime/src/lib.rs` for normal-capacity FIFO and reset
  boundaries. No native_ingest production change proposed.
- Scope-only native-rank document after tests pass; parent owns global ledgers.

Do not change the frozen Candidate six authority files, durable schema, kick,
war, PK, economy, Guild notice/bank/Sabuk, Zone schema or personal player saves.
The test-only runtime module declaration is an explicit additional file hunk
requiring approval; it is not silently included in the five production files.

## 6. Own character label and AOI gates that this slice cannot conceal

### Own character panel

`crystal_ui/overlays.rs:12290-12300` renders the character Guild label from
UiReadModel, not SocialModel. Native cursor takes Guild fields from world
snapshots/UserInformation (`platform-windows/gateway.rs:899-910,977-987`). The
ordinary GuildStatus branch is not projected into that cursor's own player
label (`:3924-3981` packet dispatch). Therefore a correct SocialModel own rank
does not alone prove the character panel's displayed rank refreshed.

One optional extra, independently approved hunk is needed: either project
authoritative own GuildStatus into UiReadModel in `platform-windows/gateway.rs`,
or make the relevant own-character label consult current authoritative
SocialModel without introducing inconsistent fallbacks. The latter may reuse
the already-scoped overlays file but requires checking its caller flow. Test
actual GuildStatus -> displayed own label, including leave, whitespace text,
scene changes and a rename of a different rank. Do not claim this gate closed
by the roster merge test alone.

### Public world/AOI rank

`simulation/runtime/zone/packets.rs:16-22` emits ObjectPlayer with
`guild_rank_name: String::new()`. `zone/types.rs:127-143` chat profile has Guild
name but no rank, and `session.rs:919-931` Join chat profile supplies only Guild
name. `zone/runtime.rs:3161-3169` profile update emits no new world packet.
`simulation/runtime/shared_guilds.rs:823-842` tracks Guild name announcement,
not rank changes. The frozen Candidate queues private status-7 events to
same-Guild members, but does not reproduce affected members' BroadcastInfo.

The Windows ObjectPlayer overlay already copies the wire rank name
(`gameplay_bridge.rs:1311-1344`, tested at `:7561-7600`). No new invented client
AOI-rank event is needed. The missing value and broadcast are upstream.

Current public Zone getters provide ID/transform/chat profile/vitals, not a
full canonical ObjectPlayer snapshot. `player_has_visible_object` at
`zone/runtime.rs:975-983` requires the object table and is not a general player
AOI membership API. Do not reconstruct ObjectPlayer from stale personal Session
or misuse this getter to claim online-player visibility. Existing trusted
observer delivery (`routing.rs:11277-11345`; Zone packet canonicalization and
visibility at `:12949-13029,13167-13226`) must remain the audience authority.

A separate no-Zone-schema approach to review is a narrow server-only canonical
player packet getter/projection plus Gateway enrichment from committed shared
Guild membership at the exact current presence. Emit existing ObjectPlayer for
affected rank members through trusted AOI after successful rename; decorate
initial join/re-entry packets as well, or a later movement/join can erase the
label again. This necessarily adds tightly scoped Zone runtime/getter and
Gateway queue/projection work and must await P3's write handoff and approval.
No code for that approach exists in this plan and its suitability is not yet
acceptance. Do not add rank to Zone/save schema under this scope.

Required safeguards: full canonical transform/class/dead/life/object identity
unchanged, actual current actor/affected members only, stale/reused presence
IDs rejected, existing ObjectPlayer/remove ordering retained, NoNames masking
not leaked, nearby outsiders receive only public world metadata, and private
rosters reach only same-Guild authenticated members. Far/other-map members get
the private rank update but not a fake local ObjectPlayer. Check leave/transfer
and re-entry separately. This public-world gate remains **OPEN** until that
source/ordinary/native path is verified, even if the five-file slice passes.

## 7. Source-bound RED sequence, proposed only

The correct baseline for RED is current root plus the frozen six-file authority
integrated byte-identically. An unavailable-command failure on unintegrated
root321316 would not test these native defects. Parent first pins a reviewed
new base; tests must not inject raw authority or bypass normal authentication
to masquerade as ordinary Gateway closure. Retain raw first failures.

| RED case | Meaningful assertion on old source / required GREEN behavior |
| --- | --- |
| T1 ordinary actor terminal | Authenticated type 3 targets rank 2. Commit succeeds/revision+1. Actor gets original status 7, actor name, top index 0, nested index 2 and canonical changed rank. Full 255 cannot substitute. Old routing strips 7. |
| T2 online peer/cross-Guild deny | Same-Guild member in another Zone receives one-rank source 7; unrelated Guild receives neither private event nor full roster. Preserve existing durable/membership/permission fences. |
| T3 typed one-rank merge | Seed 3 ranks/4 members; serialize the real ServerPacket status 7. Only target rank changes, other members/options/metadata remain. Old SocialModel replaces all ranks. |
| T4 own-rank update | Changed rank equals own numeric rank: own rank text/options update. Different rank: own values unchanged. Do not correlate on top index 0 or first roster member. |
| T5 pending terminal | Exact actual actor/nested target/raw desired name completes matching type-3 pending. Old response requires missing changeType. Group/notice/other-rank pending stays. Same-name request also completes. |
| T6 nonterminal fences | Metadata 255, foreign actor, wrong name/index, statuses 6/8, malformed/duplicate/unknown rank never manufacture rename completion. Already canonical metadata alone is not ACK. |
| T7 editor retry | Ordinary UI enqueue -> busy -> source 7 before 5 s -> canonical editor/draft available -> another valid request succeeds. Verify displayed Save enabled predicate matches admission. |
| T8 unconfirmed timeout | No terminal: deadline-1 and exact deadline remain blocked; deadline+1 retires only exact pending and permits retry. Timeout does not mark success or mutate rank. Exercise hidden panel, scene and Guild/session reset. |
| T9 local send/capacity failure | Full outbound queue creates no new pending. Closed transport restores only matching pending/draft/focus; unrelated operations remain. |
| T10 raw rank strings | Typed source names `" A "`, `"   "` survive status-7 and GuildStatus without trimming/leave. UTF-16 boundary/supplementary characters follow authority. Input validation, if added, rejects out-of-bound/backslash before enqueue. |
| T11 native FIFO | At ordinary capacity enqueue real SocialModel after 7 then after 255; drain main thread and reconcile both in order. Session reset retires prior scope; scene reset is not a Guild exit. No coalescing rewrite. |
| T12 source-options ambiguity | No global 7 -> client3 mapping. A type-5/peer delta does not complete a differing pending rename. Document indistinguishable identical-content/no-nonce cases and any local same-rank serialization adaptation. |

Separate additional gates after explicit approval: GuildStatus -> own character
label; confirmed rename -> affected members' public ObjectPlayer through AOI;
near outsider versus far Guild recipient; NoNames; cross-map re-entry; retained
dead actor/reused identity; packet order. These are not covered by same-map
authority tests or private roster merge.

Run only these focused tests and necessary adjacent social/notice/roster/
pending/bridge cases after GREEN. Client crates have independent manifests;
use their exact feature modes, source-bound package and root-agreed cache with
jobs <= 2. Do not run Cargo concurrently against P3/root's active cache. No
build or proposed test command was executed during this read-only task.

## 8. Acceptance and handoff boundaries

After the first slice passes, label it native-terminal/list/editor **Candidate**,
not full Guild management or human acceptance. Keep public AOI rank, own-label
projection if deferred, type-5 parity, kick/leave/war/Guild kill EXP and existing
SQL/transport/native-human gates open. No automatic global parity increase.

Parent reviews exact new write set and source before assignment. Authority
source remains frozen; routing/Zone cannot be edited concurrently with P3.
Implementation RED/GREEN, exact paired server/client builds and ordinary
network/native interaction are subsequent work, not results of this plan.
No installation/feed publication, service restart, F-drive operation or player
save was authorized/performed here.

The first external capture attempted a 111-line excerpt of the frozen 73-line
authority helper and failed before any index/excerpt output. The raw range
assertion is retained in `capture-preflight-failure-01.txt`; only the new
external capture range was corrected. The successful capture verified all six
frozen Candidate file hashes and whole-byte stability of 36 observed files.
This is a source-capture preflight correction, not a test result.
