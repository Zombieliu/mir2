# Activities and player portal, 2026-10-12

This extension reuses the existing invited-realm game account authority and
the independent `/recharge/` website. The three views are `#activities`,
`#player` and `#recharge`; navigating between them does not create a new account
or take an active character's game lease. Stripe remains TEST.

## First-phase scope

The activity service reserves account-owned rights for public-launch delivery:

| Phase | Requirement | Reserved right |
| --- | --- | --- |
| Closed test | Level30 | Bound30-day monthly item; time starts when used after delivery |
| Closed test | Level40 | Permanent commemorative title, no combat statistics |
| Closed test | Level50 | Class-specific appearance outfit, no combat statistics |
| Beta | Level40 | Beta commemorative title |
| Beta | Level50 | Beta class-specific appearance outfit |

Each account can reserve each stable phase/reward ID once. The first cosmetic
recipient/class is immutable. The server derives level from owned committed
character saves, never from a browser value. Source account/save transactions
observe the phase's maximum attained level in the same publication, preserving
qualification after normal character deletion. The webpage states that these
are registrations for public-launch delivery: it does not claim immediate mail,
equipped titles/outfits or newly credited game time.

Beta recharge records use verified LIVE USD provider payment evidence within
the sealed beta window, without a level gate. TEST orders are excluded. Refunds,
disputes, missing original history and already consumed payment provenance
require reconciliation. This records a return basis, not a second wallet award.
`monthlyDays` remains zero because existing monthly purchases have no complete
real-payment attribution. Final settlement needs a separately verified allocator
to prevent returning both the same Credits and the monthly time bought with them.

Referral rewards, public-launch bonus time, title ownership/display, separate
cosmetic equipment and idempotent free monthly-item delivery are later work.
Deleting account rows or restoring an old full account database can erase the
ledger; a future wipe must preserve/export `accounts.activities` and the original
billing history. Character-only deletion is covered by automated checks.

## Authority and configuration

- `GET /v1/activities/status` and `POST /v1/activities/claim` use the same
  revocable bearer authority as billing. Only the authenticated account is read.
- Claim accepts `characterIndex`, `rewardId` and `requestId` only, rejects unknown
  and duplicate JSON fields, and publishes through existing bounded Source CAS.
- The ledger is additive `AccountRecord.activities` JSON with account schema9.
  No SQL table migration or new credential store is introduced.
- `MIR2_ACTIVITY_PHASE=disabled|closed|beta|public`,
  `MIR2_ACTIVITY_PHASE_START_MS` and optional `MIR2_ACTIVITY_PHASE_END_MS` are
  operator-owned. Disabled is the default. A campaign's start/end cannot be
  silently reused with new dates; advancing the phase seals earlier records.
- Gateway and every external Source host must use the same policy and schema9
  binary. Drain the pair before changing phases. Do not roll an old schema8
  writer over accounts with activity rights; restore configuration or repair
  forward without restoring a database over newer player progress.
- Public proxy routes are two exact authenticated paths, not a `/v1/*` wildcard.
  Responses are private/no-store. The browser keeps no password or bearer in
  persistent storage, clears personal snapshots on expiry/ban/sign-out, and
  disables claims after a failed/unknown refresh.

## Verification checkpoint

Local website29/29, Source activity29/29, adjacent billing/monthly/gift59 passed
and one opt-in gift PostgreSQL ignore; Gateway activity HTTP4/4. The five Source
integration suites total88 passed plus that existing opt-in ignore. Desktop
1280px and phone390px browser checks cover all nine languages; Arabic is RTL.
Independent actual PostgreSQL writers, ordinary Source save/logout and deployed
browser acceptance are still pending at this candidate checkpoint. No production
rollout is attested by these local results.

Run from `mir2-web3`:

```sh
node --test apps/recharge-web/tests/*.test.mjs
cargo test -p mir2-simulation --features test-support --test activities
cargo test -p mir2-gateway --lib web::activities
```

Actual database proof is opt-in: `MIR2_ACTIVITY_TEST_ISOLATED=1` and a private
`MIR2_ACTIVITY_TEST_DATABASE_URL` pointing only to a generated
`mir2_activities_qa_` database. The test creates a generated schema, uses four
independent account caches and then executes normal Login/StartGame, external
web registration, Turn, direct save, LogOut and re-login. It does not pre-refresh
the owner cache to conceal conflicts and does not drop an existing database.
