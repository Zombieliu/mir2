# R25 Candidate publication — 2026-10-11

Status at this source commit: the actual R25 client is locally qualified and
CMS signed; the public client/feed remains R24/s19 until the original complete
immutable stage and separate pointer promotion succeed. The Gateway is already
live at `4006d988b3922d0106278720abfa9b3d384c8d5e`.

## Actual client and local delivery evidence

- Application source `d917de1ff4988e2d9fc7ca58f958cb5c80861fdf`, tree
  `0a8a1f534e5ebf127062859d0b8b3e913ff412ba`.
- Real Windows workflow [38128601107](https://github.com/Zombieliu/mir2/actions/runs/38128601107)
  passed all eight original jobs. Native tests: 941 pass, zero fail, seven
  original external/native tests ignored. The old falsely green 8d96 EXE is
  quarantined and is never a release subject.
- Actual extracted EXE: 111,788,032 bytes;
  SHA256 `901c47188259b5f2a6b73070b12456d4ecc13aab02c68b975e1843fffc507348`.
- Original full package verifier passed: 209 packaged maps/208 runtime maps,
  11 explicitly pinned pet WAVs, player/actor/item/pet asset inventories. This
  retains prior resource provenance; it does not invent fresh pixel acceptance.
- Original full delivery reconstruction and CMS gates passed: 154,579 payload
  files (987,022,098 bytes), 15 bundles (842,005,200 bytes). R24-to-R25 EXE patch
  is 25,653,268 bytes and restores the exact CI EXE. Other changed metadata and
  resource bundles still contribute to a user's download.
- Actual signed publication plan: 40 public objects, 1,052,847,868 bytes;
  plan SHA256 `9e9f65d3ec3c413a8aa2980886548910a0c358d17cefdf86c594520c5e9132e0`;
  genuine root proof SHA256 `25f6c45723c2cff0550ffb47f77afb8f9153e7d85ce0bc914ab5c7424be910d7`.

## Publication scope and safeguards

This commit adds the fixed R25/s20 transport table, predecessor fixtures and
reviewed older R20/R22/R23/R24 publication dependencies. The update engine stays
at `b4390ee4c987bbf000ba1d95761151e39fc0d54f`, EXE SHA256
`9213ec3fe1a992f757ea48c53981afa661765b42e637c1ad811e326060b170b8`.
Application, engine and publisher identities remain distinct. Existing
launchers do not require a new installer for this release.

Actual checks in the owned checkout: Python preparation 16/16, delivery 76/76,
interrupt one pass/one original Linux-only skip; all eight unchanged Node
publication suites pass (1,104/1,104). Wrangler 4.149.0 dry run succeeds with the
existing compatibility date, R2 binding and download route. Local counts are
publication checks, not additional gameplay coverage. The whole Linux tooling
gate, complete R2 stage, public Windows HTTPS/CMS check and separate promotion
remain required. The same-source stage guard explicitly covers sequence20.

The prior source publisher and derived delivery scripts retain authentication,
fixed object admission, immutable byte verification, cancellation/unknown
outcome records and compare-and-swap promotion. All earlier failures and
truncated Worker downloads are preserved and excluded; a complete current
Worker module backup was separately reconstructed and hash verified.

Fresh server comparison proves the current Caddy config only adds the two
already installed billing endpoints to the old config. Download routes and
authentication bytes are identical. R25 operators attest this actual current
config; they do not replace it, reload Caddy or restart the Gateway. Remote
free-space and current signed s19 feed checks passed before private staging.

## Evidence and remaining acceptance

Local durable evidence: `C:/mir2-build/classic-live01a8-integration-20261011/`
(`ci-03/windows-ci-01/artifact-01/CI-VERIFIED.json`,
`r25-local-publication-qualified-01/VERIFIED.json`,
`r25-own-publication-gates-01/`, `r25-caddy-compare-01/`,
`r25-worker-live-03/COMPLETE-SNAPSHOT.json`). Signed delivery and plan:
`C:/mir2-playtest-releases/20261011-live-health-movement-r25-04/`.
The short-path package and retained failed long-path attempts remain distinct.

Native dense-combat escape, continuous attacking, affected-laptop frame pacing,
new screenshots, funded commerce and final human frontend acceptance are not
claimed here. The full P1–P7/Mentor backlog, especially ordinary NPC invocation
binding and Source world population/event-outbox qualification, remains open.
The expired heartbeat stays paused and the blocked old Goal is not marked
complete. Root is the sole source/docs/Git/production writer.

## Retained first actual stage failure and dependency repair

The actual stage workflow38144814677 at publisher5772 failed its unchanged R22
offline preparation gate because the historical origin generator/template
dependency was omitted from the bounded integration closure. It never reached
Worker deployment, R2 staging or promotion. The failure log and artifact
11704665675 remain retained. The same failure reproduced locally.

Seven exact tracked predecessor Git blobs restore that historical dependency;
no test, assertion, authentication, workflow gate or current pointer changes.
All five original R20/R22/R23/R24/R25 preparation suites pass locally afterward
(26+9+16+16+16 executions). The corrected exact source still requires its own
whole Linux workflow and stage/promotion. Generated test diagnostics are kept
with scoped ignore rules, not published as application artifacts.
