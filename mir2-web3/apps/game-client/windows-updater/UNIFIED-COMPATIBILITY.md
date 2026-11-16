# Unified Windows release compatibility

This source combines the signed packed-generation updater (engine 2) with
r10's exact ROOT17 admission, ten locales and strict canonical preference parser.
No build command publishes a discovery alias. Build into fresh local output
roots and keep the existing production feeds unchanged during acceptance.

## Installed r10 and the older packed line

r10's immutable signed installer seed is sequence `2026102910`. The historical
packed release is sequence `31`, not `2026102931`. It cannot be activated over
r10. Do not delete a seed/receipt, lower a sequence floor, or reuse a signed
sequence with different bytes.

Only a **fresh CMS-authenticated, schema-valid pack feed strictly below the
signed legacy floor**, rejected before object download or state writes, carries
the typed `BelowLegacyFloor` classification. A retained legacy launch also
requires no packed selector or signed pack receipt, no pending legacy update,
an unchanged authenticated floor, and a valid signed legacy EXE/attestation.
The updater then launches precisely that installed game and skips v4 network
discovery. Equal sequence, signature/schema/expiry failures, bad objects and
unknown errors cannot use this route. A migrated installation continues only
its existing authenticated healthy selection under the existing policy.

## New immutable artifacts

The unified installer is `2026.10.29.11`, candidate
`WN-CANDIDATE-20261029-unified-11`; normal and validation AppIds remain separate.
The normal AppId is unchanged. Validation has its own Start-menu group and
profile. Existing game preferences must survive ordinary replacement.

For compatibility with the earlier ROOT12 five-class updater, new v4 packages
put these exact five unchanged documentation/license payloads under
`mir2-assets/legal/`: `README.zh-CN.txt`, `BILLING-HAN-SOURCE.json`,
`OFL-NotoSansSC-BillingHan.txt`, `SIMPLIFIED-SOURCE.json`, and
`OFL-NotoSansCJKsc.txt`. The installer's outer Simplified Chinese README remains
available. Full SC font bytes remain embedded. The package verifier accepts
either the historical r10 root layout or the complete new legal layout, rejects
mixed/duplicate layouts, and retains exact file, byte, signature and closure
checks. ROOT17 remains supported for installed r10 and explicit rollback.

Reserve a new canonical unified v4 sequence above every signed installed seed
(for example `2026102911`), then a higher packed sequence (for example
`2026102912`). These are planned values, not published artifacts. Before signing,
check the actual retained receipt maximum and allocate unique canonical feed
bytes per sequence. The root legacy bridge must remain within its older
200,000-entry bound; five-class/Wallet28 has the independently verified
300,000-entry/64 MiB bound. Do not publish a high-sequence old Game bridge that
would replace r10 with an older game. Old bridges and the final unified package
need separate compatibility fixtures before promotion.

Existing 33 packs prove only their own signed resource inventory. Reuse a
content-addressed object only after the merged complete logical path/size/SHA
inventory matches it; new legal payloads or changed resources require a new
catalog/pack closure and actual read verification. Record native-game source,
engine source, resource source and each verification scope separately.

## Release checks

Run updater unit/bin tests, archive/publication tests, exact legal-layout tests,
and real pinned-CMS legacy/packed fixtures. Verify invalid CMS, equal/replayed
sequences, corrupt metadata/objects and partially migrated states fail closed.
Then build clean source and verify the actual Candidate and updater bundle.
Compile normal and isolated installers; use isolated profile copies for real
Launcher -> Updater -> Game, ten-locale/SC login readiness, three billing pages,
default asset discovery, uninstall/reinstall and retained preferences. No
payments are required. Keep public-feed staging/promote as separate operator
actions with current-pointer/CAS validation; no production credential is needed
for source, local build or offline acceptance.
