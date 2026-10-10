# Monthly card in the ordinary GameShop — 2026-10-10

The operator requested a visible monthly-card consumable bought with Credits,
like the existing GameShop product cards. The Source already supplies the
application-owned item/product1000001 when a positive price is configured.
It was in `Scroll`, sorted under `MonthlyCard30Days`, so the All first page
shown by the operator did not expose it.

Native code `0399bbdddb0c83bd922235260a80b56c47ef3dc8` places that exact item and
product pair first after the ordinary class/category/search/section filters.
The remaining original products keep their original name order. Server prices,
Credits-only eligibility, catalog identity and purchase/parcel/use routes are
unchanged. The configured local test price is1000 Credits for30 days.

`UI-TESTS.txt` contains14 passing real client unit tests, including first-page
visibility across five classes and localized names/search across all nine
locales. `MONTHLY-TESTS.txt` contains9 passing Source tests, including ordinary
GameShop purchase, parcel claim and exact-UID UseItem, permanent purchase/use
receipts, restart and failed/uncertain publication. These Source tests use
prepared wallet fixtures; they are not a new Stripe payment or native clicks.
The initial wrong module filter ran0 tests and is retained in `UI-ZERO-FILTER.txt`;
it is excluded from acceptance. The corrected module filter runs14 tests.

The compatible Windows binary builds successfully and is copied/hash-verified
to an isolated pending file. At preparation the user's local native client is
still running; its executable/profile and the Gateway are not switched.
`RECEIPT.json` records exact commands, counts, binary hash and delivery scope.
The existing local Stripe test Gateway can supply this product without restart.
Public R22/Gatewaya41/feed17 and the unfinished original parity goal are unchanged.

Manual acceptance: normally exit the existing test client before switching the
prepared binary; log in normally, open GameShop All and see **Monthly Card
(30 Days)** first; select Credits, buy for1000, claim its parcel into the bag,
then use the item. Buying alone must not start time; use adds30 days to the
account, extending current valid access. Confirm the time after normal relogin.
Native human visual/purchase acceptance and public commercial rollout remain open.
