# NPC medicine shop and supply-name regressions — 2026-09-30

The reported level-9 Bichon medicine purchase could overlap the inventory. The
Traditional Chinese supply card also showed missing-glyph boxes in the merchant
name. Both production paths are corrected; installed human acceptance remains
open. No server, economy, NPC destination, account or saved character changed.

## Causes and corrections

`NPCGoods` can request a shop independently of `NpcDialogModel` opening. The old
inventory placement watched only the dialogue transition, leaving the default
bag at `(0,0)`. A service transition now checks placement after actual input and
before rendering. An overlapping/offstage bag moves to Crystal's `(445,0)`;
valid existing positions survive. Buy/Sell changes and close/reopen are covered.
Ordinary frames preserve deliberate dragging. The service root is bounded to
its actual 440px dialogue/drop width rather than extending an empty input area
over the bag. Original item geometry, prices and purchase commands remain.

The supply panel passed three hard-coded Simplified Chinese merchant names to
the native catalogue, which had no matching aliases. The bundled Traditional
font lacks `剂/师/缪`, `杂/货` and `维`, explaining the exact boxes. Display lookup
now starts with the same canonical NPC identity used by the supply policy and
route. Existing nine-language NPC translations apply, including `鍊金師 Samuel`,
`商人 Bull` and `專家 Travis`; no new fonts or broad catalogue substitution is used.

## Evidence

- Four focused production service tests pass: actual `UiSurfaceSignals` open,
  usable bag/Buy controls and normal `BuyItem`, existing bag positions, tab and
  reopen transitions, and supported stage transforms.
- A production supply-render test checks all nine locales × three vendors and
  preserves each original route action and coordinates.
- Shared client suite: **1,158 passed / 0 failed / 7 explicit GPU/live ignores**.
- Actual offline GPU: **9 medicine-shop + 27 selected-vendor captures**. No
  missing glyphs, text overflow or offscreen text. The selected supply cards fit
  above the HUD and retain their bottom action. System fonts are disabled.
- Shop captures use the real service request and production overlay roots; no
  fixture manually positions the bag. Actual bounds are bag `[445,0,761,236]`
  and service `[0,224,440,558]`, with visible original medicine icons in both.
- Representative images and complete text/bounds reports are stored here.
  The complete 36-image set and build/run logs remain at
  `C:/mir2-ui-repair-20260921/shop-supply-r6-20260930/{shop-03,supply-02,logs}`.

The former multilingual shop fixture manually separated its widgets, and the
old supply fixture used system Microsoft YaHei with native locale inactive.
Neither exercised the failing branch. The new cases explicitly cover those
gaps. Early fixture captures without the item-image layout ordering are retained
locally but excluded from the final shop evidence. The final harness uses the
production ordering and asserts visible medicine icons.

## Distribution

The r6 recipe retains all nine languages and the existing installation identity.
Its full build/package receipt is recorded separately after release checks.
The running F-drive client is preserved; these tests create no native window or
network connection. Public publisher signing and the other laptop's 4551 policy
failure remain separate open issues from r5.

The installed Windows shortcut currently starts the native game directly; it
has **no automatic updater**. The old Tauri shell loads the Web client and is
not an updater for this native installer. Native code and embedded catalogues
need a new client build, but users should ultimately receive only changed files
through a versioned, verified launcher update with staging and rollback. That
distribution feature is not claimed implemented by this UI hotfix.
