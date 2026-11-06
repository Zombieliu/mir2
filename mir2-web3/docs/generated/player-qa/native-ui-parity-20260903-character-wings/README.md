# Windows Character wings and real unequip checkpoint

Date: 2026-09-03 (Asia/Shanghai). This is bounded local-development evidence,
not Crystal visual acceptance. `visualAccepted=false`, `accepted=false`,
`globalParityPercent=null`.

## Result and source

The native Character paper doll now receives the server's real armour wing
effect and renders the four original frames with their intrinsic offsets and
Crystal's additive blend. A real-pointer cycle also proves that removing the
exact armour instance clears the wing, puts the item in the first bag cell,
and re-equipping restores the wing without an optimistic local mutation.

Crystal source checkout: `92b4ce4ab488b11e65f63d3ad22de2e1f25ec08d`. The audited
files are clean at that revision:

- `Server/MirObjects/HumanObject.cs:1795-1847,1927,7046-7055`: reset
  Looks_Wings, resolve GetRealItem for wearer level/class, skip broken base
  templates and publish the real armour Effect.
- `Client/MirScenes/Dialogs/CharacterDialog.cs:47-82`: require armour, draw
  WingEffect 1/2 before armour -> weapon -> helmet-or-hair, with original
  `Prguse2.DrawBlend(..., useOffset=true)`.
- `Client/MirGraphics/DXManager.cs:353-390` and `MLibrary.cs:692+`: additive
  SourceAlpha + One and intrinsic library offsets.
- `Client/MirControls/MirItemCell.cs:788-874` and
  `Client/MirScenes/Dialogs/InventoryDialog.cs:151`: source unequip destination
  search and the raw Inventory-array belt offset.

| Effect | Gender | Original frame | x, y, width, height |
| --- | --- | --- | --- |
| 1 | Male | Prguse2/1202 | 64, 138, 148, 139 |
| 1 | Female | Prguse2/1203 | 64, 145, 148, 144 |
| 2 | Male | Prguse2/1204 | 55, 140, 156, 185 |
| 2 | Female | Prguse2/1205 | 56, 144, 156, 185 |

Unknown/zero/other effects and unknown gender do not invent a frame. No armour
means no wing. Four retained materials avoid per-frame GPU preparation churn;
the package/startup diagnostic requires all four original PNGs and metadata.

## Build and fixtures

Client Debug EXE SHA-256:
`8BA170AA654FCE1EB911033203C22F018EAD2A4B973F04A4F80A8679E2FF6F20`
(88,405,504 bytes). Gateway EXE SHA-256:
`5C268641095BD300AD7F720CAC8B3B3939FE2182E67DB8DB5E0C232F6AA497C3`.
The binaries were built from the working tree based on `8cf9e2e4e`, not from an
unchanged published commit. Production source hashes, process IDs, original
asset hashes and image hashes are in [process-provenance.json](process-provenance.json).
Only docs and a test-only serialization example changed after that build.

`apps/game-client/platform-windows/scripts/prepare-character-wing-fixture.mjs`
creates an exclusive NEW account store from an existing exact source-derived
seed. It never replaces the seed, never injects a client wing-effect field,
and does not grant GM authority. The four Warrior Lv50 cases use original
catalogue armour indexes 375, 379, 45 and 51 with full durability. The test
gateway binds only loopback TCP 7400 / WebSocket 7410 with separate temporary
identity/recovery secrets. Account stores, credentials and logs are excluded
from version control. These are labelled QA characters, not Crystal `1231`.

## Four-frame live matrix

All images are actual 1024x768 renderer captures at DPI 1.0 from the same EXE.
Run `character-wings-20260903-r3` uses the explicit auto-Character QA capture
target. Each image has its adjacent, unchanged F12-format JSON sidecar.

| Character / catalogue armour | Observed Bichon coordinate | Capture |
| --- | --- | --- |
| WingOneM / HeavenArmour(M), 375 | 290,620 | [male effect 1](wing-one-m-r3-character-1788379280820-1.png) |
| WingOneF / HeavenArmour(F), 379 | 289,619 | [female effect 1](wing-one-f-r3-character-1788379320171-1.png) |
| WingTwoM / MirArmour(M), 45 | 290,620 | [male effect 2](wing-two-m-r3-character-1788379365612-1.png) |
| WingTwoF / MirArmour(F), 51 | 289,619 | [female effect 2](wing-two-f-r3-character-1788379435322-1.png) |

The observed coordinates differ across the separate cases and are not
relabeled as one same-state pair. The table verifies the bounded wing layer,
not the entire actor or equipment-icon surface.

## Real-pointer unequip/re-equip cycle

Run `character-wings-20260903-r4`, process 231488, uses WingOneM at the unchanged
BichonProvince `(290,620)` position throughout. Automatic UI preparation is
disabled. The route closes the entry notice, opens Character, selects the
armour, sends Unequip, waits for server state, opens Inventory, selects the
first-cell item, sends Equip, waits for server state and reopens Character.

| Step | Observed result | Renderer screenshot |
| --- | --- | --- |
| 1 | HeavenArmour and effect-1 wing present | [equipped](wing-cycle-r4-in-game-1788379572616-1.png) |
| 2 | Armour removed, wing absent | [unequipped](wing-cycle-r4-in-game-1788379606156-2.png) |
| 3 | Exact armour icon in first bag cell, free count 45 | [first bag cell](wing-cycle-r4-in-game-1788379619484-3.png) |
| 4 | Armour and original wing restored | [re-equipped](wing-cycle-r4-in-game-1788379659928-4.png) |

The old operation sent `grid=equipment,to=-1`, which the server rejects.
The current WebSocket/Simulation destination is a normalized 0-based BAG
index, unlike Crystal's raw array (belt 0..5, first bag 6). Sending +6 was
also caught by live QA because it landed in the seventh cell. The final
client sends the real first free normalized bag index, checks capacity and
current item identity, and never treats a free belt cell as that bag index.
Belt-first amulet removal/merge still needs a separate faithful protocol
path. No general raw-Crystal packet compatibility is claimed here.

## Validation

- Native UI library: 519 passed; native runtime: 212 passed.
- Windows binary tests: 521 passed; fresh Debug EXE build passed.
- Simulation library: 1491 passed; Gateway library: 666 passed, 1 ignored.
- `character_wings`: 5 passed, covering both genders/effects, no/plain/broken
  armour, base versus instance durability, authoritative catalogue identity,
  exact-instance first-cell unequip/re-equip, save/reload and @SETLIGHT.
- `great_fox_spirit_recall`: 2 passed after the snapshot-field addition.
- `test-crystal-account-state.mjs`: 9 passed; wing fixture syntax, four Rust
  workspace formatting checks, diff and all eight image/sidecar hashes pass.
- The former `/ARArmour/00/24.png` CI failure is fixed by an explicit test
  atlas/availability fixture with walk/range/stand and unavailable-library
  coverage. Production manifest/filesystem fallback is unchanged.

## Still open

There is no original Crystal same-state screenshot pair in this report.
All sidecars correctly remain `mir2-native-visual-capture-draft-v1` with
`eligible=false`: authoritative light is null and trusted package provenance
is unavailable. The separate process ledger does not rewrite or promote them.

The generic Item operation popup and name-derived equip destinations are not
Crystal's double-click/drag/locking UI. Belt/amulet behavior, full Character
tabs/class/hair/helmet/gear coverage, other item surfaces, original paired
comparison, real 125/150% DPI, signing/package identity and human acceptance
remain open. This matrix also exposes absent `Items/595.png` and `605.png`
MirArmour icons and head-only high-armour world actors; those are retained as
WN-ITEM-002 and WN-ACTOR-001, not hidden by the working paper doll. The QA auto
target's post-completion UI preparation remains WN-QA-002; the r4 manual
cycle avoids it. No existing backlog leaf or global denominator was removed.
