# Hunting economy pure policy

This source tree is integrated as `packages/economy-policy`; the companion policy,
vectors, and Python model belong to repository `config/`, `fixtures/`, and `scripts/`.
The checked-in policy is disabled. The formula functions intentionally still run
when disabled so that the exact same policy can be checked offline. A production
caller must enforce the `enabled` flag before any mutation.

All quantities are exact integers. One permit is 1,000,000 atoms and permits the
conversion of 1,000,000 gold. Thus conversion of one gold consumes one permit atom.
For destroyed gold `D`, permit atoms minted before the hard cap are `D*K`; with
`D=7,200,000` and `K=1/10`, the result is 720,000 atoms, or **0.72 permits**, not 72.
The user selected keeping K=0.1 and recalibrating the quota. The disabled default
therefore has a 0.72-permit daily baseline and a 1.08-permit cap. With a fixed
all-in destruction of 7,200,000 gold per day, zero opening inventory and daily
consumption of all available stock, decay leaves 715,200 gross conversion gold
and 700,896 net gold. This is a scenario, not a measured production equilibrium;
destruction includes conversion fees, and split requests may round fees upward.
The K=0.2 comparison uses the same cap, so its raw 1.44 permits is capped at 1.08.

Daily settlement adds capped minting, then applies decay at `1/150`. Fractional
mint and decay numerators carry forward; whole atoms excluded by the hard cap are
discarded. State remainders are bound to the effective policy version. Changing a
denominator requires an explicit transactional state migration or reset. The rate
is the requested daily `0.2/30`, which does not compound to exactly 20% in 30 days.

Conversion quotes debit gross bound gold and gross permit atoms, credit net
unbound gold, and burn a fee rounded upward to one gold. The server must commit
all of these and the destruction ledger in one transaction; a quote itself checks
neither authentication nor balances. A one-gold quote burns its full output.

Plans have no persistence or clock side effects and do not implement durable
idempotency. Repeated planning on the same pool produces the same plan. The game
server must use a unique realm/day settlement key, a locked current pool, and a
single atomic ledger commit; it must not apply a plan twice or apply a stale plan.
Grafana is never a caller that mutates these balances.

`business_day_key` and `settlement_window` use a fixed UTC+08:00 04:00 boundary and
UTC seconds, independent of host timezone and daylight saving. At a boundary the
closed day is `business_day_key(now)-1`. Intervals are `[start,end)`.

The Python model uses the same integer algorithms, policy and fixture file. It is
only an offline comparison tool, never an authoritative inventory service.
JSON numbers must be parsed as exact integers; JavaScript adapters must use a
lossless integer parser or canonical decimal strings before handling values above
`Number.MAX_SAFE_INTEGER`.
