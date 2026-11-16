# Native map-layout relocation repair — 2026-09-03

## Scope and cause

The user reported a black Bichon world at `(302, 634)` after the native client
was rebuilt into a system-temp target. Actors, effects, HUD and minimap were
visible. The full visual pack is on the existing F-drive resource volume;
the checkout's `public/generated/crystal-packs/full` is a junction to it.
No missing full-pack download was established.

The executable-adjacent `mir2-assets` junction targets `apps/web/public`.
The map layout is separately stored in `apps/web/lib/generated/crystal-map-pack`.
On Windows the old `mir2-assets/../lib/...` lookup was normalized relative to
the installation alias, not the real checkout. All three old map-0 candidates
at that relocated entry were missing, while the checkout map was present.
Startup checked texture manifests but not map layout, so it opened anyway.

## Correction

- One shared locator now serves startup diagnostics and the map loader.
- Packaged `crystal-map-pack` and `generated/crystal-map-pack` retain priority.
- Development sibling lookup canonicalizes the asset root before selecting
  the physical parent's `lib/generated/crystal-map-pack` directory. No host
  drive letter or checkout path is embedded in the correction.
- Startup rejects a missing Bichon layout, decodes it before opening a window,
  and reports the actual map path. An unreadable map fails explicitly.
- The chat diagnostic says `Full index + local map pack found`, not `Full ready`;
  an index is not a full asset/render acceptance claim.

## Verification

The repaired executable is built from base `6296e27415be7e27c3b6ed201745506c6114f58d`
plus the source hashes in [verification.json](verification.json). At that
record's `recordedAt` timestamp the fix was local and unpublished. Its
`codeCommitted` / `codePushed` values preserve capture-time facts; they are not
a current publication-status tracker. See branch history for later publication.

| Check | Result |
| --- | --- |
| Asset/path tests, including a real Windows junction and wrong-sibling negative control | 6 passed |
| Reported Bichon `(302, 634)` viewport | 607 atlas tiles, 242 standalone tiles, all 221 referenced local images present |
| Entire native host suite, relocated asset entry and non-repository working directory, `--test-threads=1` | 537 passed, 0 failed, 0 ignored |
| Offline locked native build, Rust 1.95.0 | passed, 26.47 seconds |
| Rust formatting and `git diff --check` | passed |

Focused checks are subsets of the 537-test total, not additional passes.
The first default-parallel run had 534 passes and three GameShop receipt/session
boundary failures. They are retained in the verification record. The runtime
documents one replaceable process-global native queue; these failures are
consistent with host tests replacing each other's queue. Serial execution
passes, but this does not fix or accept parallel test isolation. Gateway,
GameShop and runtime queue code were not modified for this repair.

Reproduction from the project root (choose an external build target when disk
space is limited and set `MIR2_NATIVE_ASSET_ROOT` to the installed alias for
the relocation check):

```text
cargo +1.95.0 test --locked --offline --manifest-path apps/game-client/platform-windows/Cargo.toml --bin mir2-platform-windows -- --test-threads=1
cargo +1.95.0 build --locked --offline --manifest-path apps/game-client/platform-windows/Cargo.toml --bin mir2-platform-windows
```

## Live handoff and preservation

The user had closed the broken client before the rebuild. The repaired native
executable was opened with the computer-use skill and observed at the login
screen, connected to the existing local Gateway. Its startup path necessarily
passed the new map-layout decode check. User authentication was not automated;
the skill requires manual login. Actual in-game visual verification remains
pending and must not be replaced by the login screenshot or headless counts.

No resource files, resource junctions, character state or account-store contents
were edited. The map and account-store hashes were unchanged at the pre-login
handoff. The previous executable was copied to a verified system-temp backup;
no resource pack was copied, no Gateway was restarted and no process was killed.
Exact executable hashes, local logs and backup paths are in the JSON record.

This fixes a native launch/resource lookup defect only. All 33 existing backlog
IDs, original-pair/DPI/light/soak/signing/legal/human gates remain open:
`visualAccepted=false`, `accepted=false`, `globalParityPercent=null`.
