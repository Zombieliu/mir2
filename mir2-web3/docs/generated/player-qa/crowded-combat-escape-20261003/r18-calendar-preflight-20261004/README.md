# R18 first preflight — 2026-10-04

Clean source51e8 builds the attested Windows EXE and Linux CI package. Overall
CI37198506131 fails the real TCP/WebSocket siege transport case; it must not be
called accepted or published. Root reproduces the same failure locally0/1.
The test and4 calendar/projection source files are byte-identical to published
R17. After18:00 Asia/Shanghai, its current-day18:00 clock precedes actual socket
bootstrap and the unchanged monotonic guard correctly rejects the projection.

Only this fixture moves its server-owned clock to the next calendar day and
asserts that it follows the actual epoch. No source war hours/calendar, capture,
server projection or stale guard changes. The original live transport check
then passes1/1. Raw failed CI, local RED/GREEN, native first build and attestation
remain. The first EXE remains external in native-attempt-01/release with its
original attestation. A new clean exact-source pair and CI gate remain pending.
