# Smooth display fallback regression

User reported continued stutter on `2e6bb0d44`, process 41080. Live-readable
trace: `C:/mir2-ui-repair-20260921/render-live/20260922-203713-631-render.jsonl`.
The 12 recorded commands were confirmed, but all 12 had a reverse candidate
30–45 ms after command issuance, about 7.79–17.16 stage pixels. This is an
observed regression in the initial smooth-display integration, not acceptance.

First command at 1790081017760: world center immediately became (14936,13946),
then fell back at 1790081017804 to (14929.824,13950.783), while nominal forward
motion was from (14928,13952) toward (14976,13920). The retained frames before
takeover exactly match old stepped phase-zero displacement. Evidence extract:
`render-live/smooth-fallback-reverse-evidence.json`.

The local-command renderer was smooth, but the pending self-window fallback
used the old phase-stepped function. Native input and Bevy command ingestion
do not occur atomically, so the fallback can present first. The candidate uses
the same elapsed-time interpolation in that fallback when smooth display is
enabled, preserving source/target center compensation and endpoint clamping.
Legacy mode remains stepped. No server movement rules are changed.

Regression checks fallback progress at 0/10/20/30/40/100/600/900 ms, both applied
centers, and explicitly contrasts the old immediate 16 px run displacement.
Runtime full suite 264/264 passed (`smooth-fallback-runtime.log` in the QA root).
Release build: `smooth-fallback-release.log` in the same root.
Live repeated-command monotonicity and human smoothness are still required.
