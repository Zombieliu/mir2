# Platinum 1.76 actor source-library closure

The current profile v26 has 174 imported monster templates and 54 weapon/armour
items. Those templates plus the ordinary V2 BoneFamiliar summon now have their
complete original source libraries exported. Equipment uses the actual imported
`shape`, with CArmour/00 included for the unequipped body.

| Library family | Libraries | Drawable source frames |
| --- | ---: | ---: |
| Monster, including BoneFamiliar 078 | 100 | 24,312 |
| Gate/00..03 | 4 | 68 |
| CArmour/00..08 | 9 | 14,544 |
| Actual CWeapon shapes, including PickAxe 42 | 26 | 21,632 |
| Total | 139 | 60,556 |

The selected source libraries contain 60,585 slots and occupy 135,371,652 bytes.
There are 29 original **zero-size** slots, all in Gate/00. Their exact indices are
in [closure.json](closure.json); they have neither drawable metadata nor PNGs.
The other 60,556 PNGs total 138,859,934 bytes (excluding mask PNGs and metadata).
There are no absent source libraries or unknown required monster templates.
No exported drawable frame is fully transparent.

The initial capacity scan reported 52,761 unexported source records, including
those 29 zero-size slots. Thus 52,732 drawable records were missing at the start.
The report's `initialMissingFrames` is the checkpoint before the successful retry,
after earlier libraries had already been completed; it is not the task-start delta.

Every drawable PNG was decoded and compared byte-for-byte with original Crystal
RGBA, including image dimensions, origin/shadow anchors and source pixel hashes.
Every declared mask was independently decoded and matched to its source. The
global manifest must match each local meta.json; source hashes and complete source
FrameSets are checked. Zero-size slots must have neither metadata nor PNG files.
`verified=true` means this asset closure passed, not visual acceptance.

CArmour libraries each retain all 1,616 frames (both 808-frame genders); CWeapon
libraries each retain all 832 frames (both 416-frame genders). This exports every
source action and direction, beyond the small prior Standing/Walking subsets.
Source references: Crystal `Client/MirGraphics/MLibrary.cs:91-100` and
`Client/MirObjects/PlayerObject.cs:584-586`.

Monster library dispatch follows Crystal `MonsterObject.cs:150-208`. The current
whitelist requires only Gate/00..03 among the special families. Dragon and Pet
dispatch are represented in the planner, but no current profile requirement is
silently mapped to Monster/950 or an unrelated monster library.

The global manifest contains 198 libraries, compared with 65 at HEAD. No previous
library was removed, and no library outside this closure changed. The exporter
updates one library at a time and retries bounded Windows sharing violations on
metadata writes. An explicit zero-size export regression and the existing Crystal
FrameSet/source snapshot tests pass.

Reproduce from the repository root with the original client Data directory:

```powershell
node apps/web/scripts/export-platinum-profile-actors.mjs --dataDir <Crystal-Data-directory> --export
node apps/web/scripts/export-platinum-profile-actors.mjs --dataDir <Crystal-Data-directory> --verify
node apps/web/scripts/test-crystal-ui-empty-source-export.mjs
node apps/web/scripts/test-crystal-library.mjs
```

The existing generic exporter initially generated 29 invalid zero-size PNGs.
After fixing its full-library filter, Gate/00 was re-exported and those newly
generated files were moved intact to
`C:/Users/Administrator/profile-actor-zero-frame-backup-20260921/Gate/00`.
They are outside the game assets. Source files were not changed.

No game was restarted, no server state was changed, and no commit or package was
created. Native action dispatch, live screenshots, original-client comparison,
and human visual acceptance remain separate gates.
