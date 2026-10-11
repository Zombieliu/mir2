# Independent recharge website acceptance — 2026-10-12

Public website: <https://165.154.65.136.sslip.io/recharge/>.
This folder contains an explicit allowlist of redacted receipts and actual
browser screenshots. Provider keys, passwords, bearer/session values, full
Checkout URLs, QA character identifiers and private proxy contents are excluded.

The deployed release manifest is `release.json`, SHA256
`b78c2dd84a5e1323b04428986c3d20df9b60087208b4dd07f1c090fc4df73538`.
`PUBLIC-FINAL.json` verifies the final public bytes and security behavior;
`UI-MOBILE-FINAL.json` verifies nine language layouts and control bounds.
`MOBILE-AR.png` and `MOBILE-AR-OFFERS.png` are actual viewport captures; a clipped
full-page capture is excluded. `PUBLIC-RECHARGE.jpg` shows the public desktop page.

`PAID-ACCEPTANCE-CANDIDATE03.json` and `BROWSER-CANDIDATE03.json` qualify one actual
USD1 Sandbox Checkout, exactly100 Credits, same-offer retry reuse, browser relogin
and two ordinary game login/save cycles. These payment receipts explicitly name
the earlier candidate03; final candidate04 adds only stale-response guards.
No second payment is sent to requalify those guards. `ONLINE-WEB-FINAL.json` and
`ONLINE-GAME-FINAL.json` qualify the final site's concurrent game session and
website sign-out with normal game keepalive/logout/save afterward.
`ONLINE-GAME-EXECUTED.py.txt` retains the exact local one-off helper as evidence;
its machine-specific paths are not a portable deployment recipe.

`OWNERSHIP.json` records foreign-character/session rejection. `UNIT-TESTS.tap`
records12 focused passes. `SITE-ACTIVATIONS.json` and `SITE-APPLY-FINAL.json`
record guarded proxy/static activations. `POSTFLIGHT-FINAL.json` verifies unchanged
Gateway/source/config/PIDs, zero counters after ordinary cleanup and current
native R25/feed20 origin/CDN agreement. The website never writes that native feed.
`RETAINED-FAILURES.json` explicitly excludes failed or unqualified attempts.
`INDEX.json` records byte sizes and hashes for this allowlist and the six source
files, allowing the saved source to be compared with the actual public release.

Scope is Stripe TEST only, with no real payment, refund/dispute or native human
acceptance claim. Existing broader P1-P7/Mentor work remains open.
See [the complete scope record](../../../RECHARGE-WEBSITE-20261012.md).

Text receipts use LF for Git; INDEX retains original capture hashes separately.
