# Sabuk transport verification clock — 2026-10-04

R18 preflight/source51e8 compiled Windows and Linux, but overall CI37198506131
failed shared_conquest_transport. Root reproduced the failure on actual local
TCP/WebSocket listeners. The test and original calendar/projection source bytes
match published R17 exactly. It selected today18:00 after both sockets bootstrap
at the actual epoch, so evening runs correctly trip the stale-projection guard.

The fixture now selects the next policy calendar day and asserts start>epoch.
Source18:00/30-minute policy, monotonic checks, requests, capture rules and ordinary
NPC/TCP/WebSocket assertions remain. Actual failed case0/1 becomes1/1; the
fixture change does not accept new game/runtime functionality. No production
service, save or installed game is modified. The first clean Windows artifact
and original failed CI remain recorded; a clean paired rebuild/CI is required.

[Exact RED/GREEN and source identities](generated/player-qa/crowded-combat-escape-20261003/r18-calendar-preflight-20261004/README.md).
