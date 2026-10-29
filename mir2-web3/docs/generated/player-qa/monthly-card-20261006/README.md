# Monthly card Candidate evidence — 2026-10-06

Account-wide 30-day calendar access is implemented in source, including trusted
operator-issued codes, authenticated redemption, renewal and saved expiry logout.
The public realm, updater feed, installed clients and production account stores
were not changed. Required access defaults OFF; no price or online payment is
configured. This does not close the existing classic P1–P8 Goal or mining P6.

## Server and native checks

| Check | Actual result | Evidence |
| --- | --- | --- |
| Calendar, account isolation, exactly-once redemption, File reload and rollback | 9/9 | [CI results](server-ci-results.txt) |
| Isolated PostgreSQL 16, four independent writers, reopened store and renewal | 1/1, executed explicitly | [CI results](server-ci-results.txt) |
| Real Axum HTTP/shared WebSocket entry, expiry/save/re-entry and expired resume | 3/3 | [CI results](server-ci-results.txt) |
| Existing native resume | 22/22 | [Local results](resume-regression-results.txt) |
| Existing account store | 22/22 | [Local results](account-store-regression-results.txt) |
| Native Windows monthly wire/correlation and code redaction | 1/1 | [Checkpoint binary results](windows-wire-results.txt) |
| Native monthly presentation, correlation and nine-language state/input | 5/5 | [Final binary results](client-final-results.txt) |
| Actual offline GPU open/closed modal frames | 1 explicit test, all 9 locales | [Executed GPU result](gpu-final-results.txt), [layouts](monthly-card-visual-report.json) |

The [complete server run37464517808](https://github.com/Zombieliu/mir2/actions/runs/37464517808)
is successful at `cb330c515a3f16a3d5fb0acbeac0f8fe35b1aedc`.
[Fetched run/step metadata](server-ci-run.json) and filtered actual job output
record the executed PostgreSQL test, rather than counting its local ignored state
as acceptance. The native Windows monthly wire/redaction test also passed 1/1
at that implementation checkpoint. Final changes to the UI layer follow that
server source; their file hashes and screenshot hashes are in the
[execution receipt](execution-receipt.json).

## Actual rendering and retained first failure

The first GPU fixture reported zero missing glyphs and valid label bounds but
manual inspection found that the modal was invisible behind the shell root.
Its [occluded frame](first-occluded-zh-TW.png) and
[original layout-only report](first-occluded-layout-report.json) are retained as
failed evidence. The original report's `passed: true` is not visual acceptance.

The monthly layer now renders at GlobalZIndex 2000 above the root at 1000. The
fixture additionally compares actual open/closed images and refuses an unchanged
frame. All nine final cases record `visibleFrameChanged: true`, zero missing
glyphs, zero overflowing text nodes and zero nodes outside the viewport.
The [closed Traditional Chinese baseline](monthly-card-zh-TW-closed.png) and
final open frame demonstrate the difference. Traditional Chinese, Arabic and
Brazilian Portuguese final frames were also visually inspected.

| Locale | Final modal screenshot |
| --- | --- |
| Traditional Chinese | [zh-TW](monthly-card-zh-TW.png) |
| English | [en](monthly-card-en.png) |
| Brazilian Portuguese | [pt-BR](monthly-card-pt-BR.png) |
| Russian | [ru](monthly-card-ru.png) |
| Hindi | [hi](monthly-card-hi.png) |
| Indonesian | [id](monthly-card-id.png) |
| Vietnamese | [vi](monthly-card-vi.png) |
| Thai | [th](monthly-card-th.png) |
| Arabic | [ar](monthly-card-ar.png) |

These are a stable 1024×768 offline fixture using read-only original R10 UI/font
assets, fake character slots, a dummy activation code and a fixed date. The
expired state and future date are deliberately supplied together to exercise
both labels; they are not an actual subscription receipt. No account secret or
real redeemable code appears in these screenshots.

## Remaining acceptance

No new install package or paired Gateway deployment was performed. Ordinary
player clicks, live code redemption, human language/visual acceptance and paid
activation remain unexecuted. Stage matching clients and all durable account
writers with REQUIRED=0 before any paid switch. Do not mix an older PostgreSQL
account serializer with schema7 records. See the
[complete rules and operator runbook](../../../MONTHLY-CARD-ACCESS-20261006.md).
