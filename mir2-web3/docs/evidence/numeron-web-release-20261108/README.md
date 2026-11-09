# Numeron R16 public release evidence

Executed on 2026-11-08 UTC / 2026-11-09 Asia/Shanghai. User-facing entry:
https://numeron.obelisk.build/game. This publishes the canonical R16 Web source
to the existing TEST realm, not Sui mainnet or a final human parity acceptance.

Product source is `f2de762309768b51b39438c2437c14e819df9bf7`;
Linux Gateway source is `d53f9e3b22e2b92c2dc86644b6ae370fc1acab53`.
There are no Rust/Gateway/game source changes between those commits.
Documentation/evidence commits do not change the frozen deployed product.

| Evidence | What it proves |
| --- | --- |
| [Asset publication](asset-publication.json) | 42738 immutable objects read back; this f2de continuation wrote zero objects |
| [Frontend promotion](frontend-promotion.json) | Exact tested marketing/game candidates promoted, old game project unchanged |
| [Domain move](domain-move.json) | Official atomic project move, verified binding and actual public pages; no DNS change |
| [Final aliases](final-public-bindings.json) | Numeron points to the new exact candidate; old Mir2 still points to its original production deployment |
| [Server preservation](server-preservation.json) | New binary active, original129/43/43 rows exact, old config/key values and formal/wallet services retained at the recorded pre-QA checkpoint |
| [Revision alignment](revision-alignment.json) | Later TEST-only idle normal stop, fresh PG backup, single metadata key addition and actual new health revision; binary unchanged |
| [After public protocol](post-public-protocol-preservation.json) | Original account/character/save rows remain intact after the three QA accounts:132 accounts/46 roles/46 saves, same33 migrations; normal new authentication/game-fact audit rows are listed |
| [Source-hold delta](source-hold-delta.json) | Both existing financial hold rows preserved except `updated_at_ms`; amounts/status/links exact |
| [Wallet QR](wallet-qr-generation.json) | Final-domain session creation and pending poll succeed; no phone signature/payment |
| [2026-11-10 wallet recovery](wallet-qr-incident-readback.json) | Public DNS and HTTPS, real Dubhe mobile PWA approval, same-domain exchange, visible Bichon and normal logout; clean egress and virtual passkey, no physical phone |
| [Inherited images](inherited-assets-readback.json) | All three declared old-prefix map blends fully read back with exact manifest byte/hash pins and normal browser Referer; no object writes |
| [Final service check](final-idle-services.json) | Correct TEST revision, no remaining test connections/sessions/leases; formal and wallet services active |
| [43-save summary](clone43/summary.json), [execution](clone43/execution.json) | 129 real simulation StartGames; common normalization remains a recorded strict failure |
| [Clone network](clone-network/summary.json) | Four old roles,16 ordinary logins/logouts,72 cross-version projection matches; clone credentials, not a wallet ownership proof |
| [Protocol first attempt](ordinary-protocol/first-attempt.json) | Real registration/creation succeeded; external adapter incorrectly expected DOM class keys |
| [Three-class continuation](ordinary-protocol/three-classes.json), [execution](ordinary-protocol/execution.json) | Same three ordinary accounts: first V2 quest, exact reward, movement, save/relogin and normal logout passed after adapter repair |
| [Final browser summary](browser/receipt-summary.json), [detailed scope](browser/receipt.json) | Actual final-domain three-class UI login/select/city/Chinese Native Q; first role two keyboard steps; three normal logout ACKs and browser cleanup |
| [Browser resource delivery](browser/resource-delivery.json) | Three initial new-prefix404s recover through declared old-prefix200s and later SW cache; actual console4/pageerror0 retained |
| [Cold-load failures](browser-cold/first-two-receipt.json), [five-minute result](browser-cold/five-minute-failure.json), [transport diagnostic](browser-cold/transport-diagnostic.json) | Preserve actual local-network browser failures; HTTP200 and protocol success are not rendered-UI acceptance |

The strict old-save normalization differences are not waived. See the copied
[original explanation](clone43/README.md); its original external filenames are
retained as historical text while the public subset above uses the linked names.
No private original account dump, credentials, UID projection, share ticket,
wallet intent, or signed token is included.

Final actual screenshots: [Warrior](browser/role-1-Warrior-quest.png),
[Wizard](browser/role-2-Wizard-quest.png), [Taoist](browser/role-3-Taoist-quest.png).
The coordinator visually checked all three. Only synthetic QA character names
are visible. The original [post-logout selector failure](browser/ui-proof.json)
is retained: its successful logout returns character selection, which the
external helper incorrectly expected to be login. The [reconciliation](browser/warrior-reconciled.json)
and [same-browser remaining two roles](browser/ui-remaining-proof.json) record
the actual ordinary selection-exit continuation. UI progress does not silently
turn the original false result into a zero-error cold-start pass.

The shared artifact installer passed23 actual Linux tests. The final bounded
cutover tool passed41 actual Linux ROOT tests, then a real read-only plan and one
successful apply. Earlier r1/r2/r3 attempts and backup directories remain on the
server; r2/r3 restored the old service before any candidate start. The real
systemd output lists12 separate EnvironmentFiles lines, which r4 aggregates and
backs up in order. See [review receipt](server-tools/source-review.json),
[cutover source](server-tools/cutover_playtest_r16.py),
[tests](server-tools/test_cutover_playtest_r16.py), and its exact pinned
[artifact helper](server-tools/r16_candidate_install.py).

The operation tools are historical, exact-target release sources. They are not
general instructions to rerun a completed cutover. The current service no longer
meets their pre-cutover source/fresh-target guards.

No old Windows EXE or old formal game deployment was replaced. The old Numeron
Windows card was already unavailable and stays unavailable. Alo cloud execution,
fresh phone signing, paid transactions, all26 quests on the public server,
CCU/Atlas budgets and final human Accepted are outside these receipts.
