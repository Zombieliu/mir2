# Android NPC service/catalogue ingress — 2026-10-02

Status: **NI-11 PARTIAL, source-only**. The complete Windows-alignment goal is
Active. NI-10 full quest/dialog incoming state, complete service receipts, actual
JNI/online/physical-device acceptance remain OPEN. No new APK was built in this
leaf; the raw directory's `v17` label is not a delivered version.

## Source and frozen denominator

- Execution: independent `codex/android-shared-sync`; source parent
  `189e1714a92e3bba1102743659b03782f9e3bdb3`.
- Tested immutable NPC source:
  `4e35d3a34041f47370ba7f1338d4b5e0f8d7c0f8`. Both auditors were rerun against
  this commit and all 15 current source files matched its Git bytes. A later
  evidence-only binding commit does not change that tested code or the logs.
- Reviewed gameplay baseline: exact Windows
  `3d735745f1117d42a7859e87604a106351dca935`, already normally merged at
  `04ae04fc82badb8dd1a15d5dade108ade0fe586b`. All earlier proof retains its
  original source/package association.
- At the last read-only upstream check, `56ee063fb4f9d6b581ec548659066ef9c5e05e9b`
  adds r9 installer/update **documentation/evidence only**, not game code.
  Its installer delivery is not installed native GUI acceptance or Android
  acceptance. It does not silently rebind this frozen gameplay baseline.
- Exact tested file hashes and, after commit, immutable source are recorded in
  [source-results.json](source-results.json). Re-run the auditors from the
  repository root with that source SHA; do not rebind older APKs.

## Bounded implementation

Four pure tooltip helpers and eight NPC catalog/service helpers are extracted
from the same Windows implementation into shared native modules. The Windows
host retains thin delegates and its own original Items geometry provider. The
[equivalence audit](source-equivalence.json) compares all 12 bodies, preserving
quoted JSON keys/values and allowing only the recorded naming, imports,
geometry injection and formatting/comment differences. It does not compare or
duplicate any server purchase, repair, quest, wallet or custody rule.

Java's explicit public allowlist now forwards NPCResponse, NPCGoods,
NPCPearlGoods, NPCSell, NPCRepair and NPCSRepair only after authenticated
StartGame, owner validation and a current scene snapshot. Coordinates alone
after MapChanged cannot authorize old-scene service packets. Existing login,
credentials, account ownership and TLS rules are not bypassed.

The Rust host validates owner/character/map lifetime, foreign/Hero packets,
page/rate/catalog shape and original item geometry, then queues typed shared
catalog/service messages. Local limits are 32 messages/128 KiB total,
64 KiB/projected catalog, 256 goods, 16 KiB/raw packet and 1 MiB/raw snapshot.
Over-count/over-byte/catalog projection failures reject the whole new enqueue
without changing currency or old ownership. Passive snapshots refresh catalogs
but never fabricate an opening/closing packet.

Only a successfully accepted new Interact/non-Exit dialog request begins the
reply gate. Exit remains latched even if its outbound send fails. A real
NPCResponse can close the old child without cancelling an accepted new request
that is still waiting across frames. Any valid opening retires that pending
request, even when a later Closed wins in the same frame. Explicit Exit,
CloseShop, general CloseWindows/Escape, map and session boundaries cancel it;
old parent pages and delayed opening replies cannot unlock it.

## Actual tests, including original failures

| Gate | Actual result | Boundary |
| --- | --- | --- |
| Rust accepted-request failure-first | 1 pass / 2 fail | Actual outbound host systems before repair |
| Java ingress failure-first | 0 pass / 2 fail | Synthetic local TLS/MockWebServer, not login |
| Shared cross-frame failure-first | 1 pass / 1 fail | Both actual NPC sync consumers |
| Shared pending-close failure-first | 0 pass / 3 fail | Real Escape/button paths, with/without BAG |
| Final shared UI | 1230 pass / 10 ignored | Complete serial source regression; ignored unchanged |
| Final Android Rust | 290 normal / 304 preview, no failures/ignores | Both host feature variants |
| Final shared runtime | 292 pass / 1 ignored | Ordered observation and scene/session reset included |
| Fresh Java | 39 normal + 39 preview, no fail/skip | 42 tasks actually executed; XML retained |
| Actual arm64 API31 target | Debug + uiPreview pass | Target checks, not APK packaging |
| Windows affected pure tests on Mac | 6 pass in five focused runs | Packet/currency/tooltip/accepted-request source only |
| Wider Windows NPC filter on Mac | 19 pass / **6 fail** | Geometry/marker/map-fixture failures retained; not a broad gate pass |
| Frozen-source extraction | 12 bodies equal | Not Windows device or online acceptance |

The initial Rust borrow compile error, a new fixture's wrong close-button enum,
the old next-merchant fixture lacking a successful Interact, and the auditor's
first namespace/geometry normalization failure are retained separately. None is
counted as a successful player test. The merchant fixture now includes the
accepted begin hook that both production hosts already execute; its capability
assertions are unchanged, and independent stale-parent tests still require Exit
to stay locked. Read-only review found and closed both bounded lifecycle P2s;
the reviewer did not run builds/devices and did not sign off the whole goal.

Original logs/XML are archived without rewriting failures. [source-results.mjs](source-results.mjs)
checks exact counts, hashes and original-byte preservation.

## Not accepted, and next safe leaf

- No full tracker/dialog/detail/turn-in producer (NI-10). NPCResponse here is a
  service-close boundary, not proof of a rendered real NPC dialogue.
- No complete Buy/Sell/Repair/SRepair authoritative result/wallet/item loop,
  real NPC interaction, live JNI flow or physical-device pass.
- Host enqueue of a catalog/service pair is atomic locally. Failed push retries
  only the undelivered suffix; already delivered catalog is not replayed. Shared
  runtime coalescing/critical/ACK eviction can still lose an accepted message
  downstream (NI-19). **No end-to-end lossless or full-chain atomic claim.**
- Latest installed v16 still binds `8cd2e7eaeead8c71f74fbbaa9250bb2d875fed04`,
  not this refreshed/NPC source. Its 10 observed GL0x506 startup errors and all
  earlier package/render/input failures remain unchanged, not retrospectively
  passed.
- Next: exact refreshed native packages and actual emulator NPC/UI diagnostics,
  then NI-10 incoming producer and remaining Windows denominator. Approved
  HTTPS/WSS environment/account and physical device are still external gates.
  Do not use production, existing human saves, admin commands or fake login.

Original main dirty checkout and original Android checkout were read-only
verified unchanged. Nothing was pushed to the Windows branch, merged, deployed
or uploaded as APK/assets/keys.
