# Android ordinary inventory ingress — 2026-10-01

Bounded NI-05/06 **source** checkpoint. The Windows-completeness goal remains
Active. Parent/source-equivalence basis is
`32ee7faa165d3d034447c69cacafabd7b4d22377`; no APK was built from this change.
Existing v9 artifacts bind to `394307db8` and do **not** contain this inventory
adapter. Tests below ran on the implementation recorded with this checkpoint.

## Scope and equivalence

- Extract the existing Windows inventory projector and receipt parser into
  `client-bevy/src/native_inventory_ingress.rs`. Six Windows wrappers retain
  their source behavior; filesystem item geometry stays in the Windows host.
  `source-equivalence.json` compares twelve function bodies with the parent,
  allowing only namespace/host-callback substitutions. The seven-packet Windows
  route is unchanged. No server, item custody, balance or gameplay rule changes.
- Android accepts a typed InventoryModel only after validated self/map projection,
  retaining u64 identity, bag/belt/equipment/quest slots, explicit capacity and
  tooltip metadata. Same-character map loading retains personal state; changed
  identity and all seven terminal/render-failure paths reset it with DataReset.
- Java and Rust forward only DropItem, MoveItem, MergeItem, SplitItem1, SellItem,
  EquipItem and RemoveItem after accepted StartGame plus an owner snapshot.
  STARTING accepts personal receipts during map loading, not unauthenticated
  gameplay. Unsupported DeleteItem/SplitItem remain closed; explicit foreign
  owner/Hero data cannot confirm ordinary-player operations.
- A bounded sixteen-receipt FIFO retries backpressure without dropping/replaying
  confirmations; ordinary models coalesce to the latest snapshot. Existing shared
  pending matching consumes ACK/NACK, never directly changes items or gold.
  These legacy receipts are **not** all echoed request-ID protocols: MoveItem
  correlates grid/from/to. Exact skill-key/storage-V2 guarantees must not be
  indiscriminately attributed to every inventory receipt.
- Android still supplies **None for both Items and StateItem geometry**. The data
  seam is connected, but original item bitmap dimensions and equipment offsets,
  actual JNI consumption, touch operations and authoritative online play remain
  OPEN. There is no fabricated offset or guessed image ownership.

## Current source checks

| Gate | Actual result |
| --- | --- |
| Android focused inventory/scene lifetime | 12 passed,224 filtered |
| Android full normal | 236 passed |
| Android full ui-preview | 246 passed |
| Shared inventory extraction focused | 7 passed,1204 filtered |
| Shared native-player-ui full | 1203 passed,8 existing ignored |
| Java Debug / uiPreview | 33 /33 passed; zero errors/failures/skips |
| Android arm64 API31 target | Passed, Rust1.95 / NDK26.1; check only |
| Mac desktop `inventory` filter | **6 passed /2 failed**,799 filtered |
| Independent source review | No remaining bounded P0/P1/P2 reported |

Java XML suites and `java-results.json` retain the exact totals. Formatter and
source diff checks pass; raw log EOF/Gradle whitespace remains byte-preserved
and is not described as an unfiltered clean diff. The reviewer only read source,
not APKs, devices or a real environment.

### Failures and boundaries retained

- `java-before.log/xml`: one targeted test fails before the inventory receipt
  allowlist exists; missing DropItem. The final Java TLS fixture passes, but it
  is an isolated test server, not an ordinary authenticated gameplay account.
- `shared-check-before.log`: copied self-crate namespace caused E0433; changed
  to `crate::inventory`, with no rule change.
- `android-focused-first.log`:10 pass/1 fail. The malformed-quantity negative
  fixture contradicted existing Windows fallback-to-one behavior. Preserve that
  behavior, assert it separately and reject a genuinely invalid typed icon.
- `desktop-target-selection-before.log`: mistaken `--lib` for a binary-only
  Windows package; no tests ran. Correct command's actual result is retained
  separately in `desktop-inventory.log`.
- The corrected desktop run does **not** pass: geometry assertion receives0
  instead of36 while Items/StateItem metadata are unavailable, and a minimap
  input test lacks the packaged Bichon collision map. The approved Android
  diagnostic packs do not satisfy Windows' complete asset-root guard (including
  native cursor resources). Do not fabricate assets to turn that guard green,
  weaken it, or relabel the run as passed. The source-equivalence and shared
  callback regressions are distinct evidence, not a replacement full desktop gate.

## Next leaves

1. Package and validate original Items/StateItem metadata, connect Android's
   source geometry callback, then exercise the adapter in the preview specimen.
2. Bind the next clean source commit to new diagnostic APKs and actual emulator
   images/operation logs; retain v9's old identity and startup wait timeouts.
3. Audit all remaining inventory/use/shop/service routes, phone sizing/input,
   authoritative receipts and failure/unknown outcomes. No full action denominator
   or Windows-completeness percentage is established here.
4. Real login/online acceptance awaits an explicitly approved test environment;
   physical-device and human acceptance remain absent. Do not access production,
   edit real saves, infer account identity or accept offline ownership as live.

Original main/Android workspaces remain unchanged. No deployment, Windows push,
PR merge, APK/resource/cache/key Git addition or successful remote push occurred.
