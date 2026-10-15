# Mail reader original assets — 2026-09-21

Exported 18 missing Title frames using the existing Crystal exporter: 672, 675,
676, 370–372, 540–542, 680–685 and 686–688. Title670 and Prguse540/541/550/551/552
already existed and were verified. Title metadata grew from 326 to 344 frames;
all previous frames remain. No Prguse resources changed.

Title675 is **236×384**, origin (0,0), despite MailDialogs.cs declaring 236×300.
Its actual original bitmap visibly contains gold, five attachment cells and the
bottom button strip; the source child bottom at375 fits within384. Title672 is
236×300, Title676 is144×36 and Title670 is312×444. Delete540–542 are68×25;
collect/lock370–372 and680–688 are72×25. Prguse540 is24×16,541 is24×22;
550/551/552 are12×9,12×10,12×11.

[Per-frame geometry and SHA-256 hashes](mail-reader-assets.json) cover all24
requested frames. Source Title.Lib SHA-256:
`bd3e9485548e9b3cb5d4cb261c9a01a07752fb41fcea7f5d8569e602feaed648`.
Title675 PNG SHA-256 and exact source RGBA SHA-256 are included in that artifact.

After export, `audit-native-ui-assets.mjs --dataDir <Crystal/Data>` passed:
2,471 requirements,2,389 verified drawable frames,82 original source-empty
references,0 issues. [Full audit](mail-reader-assets-native-ui-audit.json).
None of the24 mail requirements is source-empty. Existing source-empty
classification is preserved. No substitute graphics were created.

Live native interaction, clipping, screenshots and visual acceptance are
**unverified**. No UI/source/build changes, commits or game restart occurred.
