# Q89 Wizard range and spawn-blocker follow-up

Date: 2026-09-16. `accepted=false`, `visualAccepted=false`.
Normal local WebSocket gateway R54; no quest, monster, currency or saved-state
edits were used for this run. Warrior is Lv30 (55/55 mandatory + 4/4 milestones);
Wizard and Taoist remain Lv25 (45/55 + 3/4 each): **155/177 completed units**.

R122 uses existing held Medium HP potions after a fresh direct Shaman hit at
85% HP; potion cadence and emergency escape safeguards remain in charge.
R123 protects the Wizard's seven-to-nine-tile Shaman firing band and preserves
six-tile named clearance during spawn search, including fallback paths.
Its normal live run exhausted 391 waypoints without CursedPriest progress,
then saved/logged out. The objective remained 2/3; no route success is claimed.

R124 handles that protected search stall by trying at most two visible Shaman
blockers per search. Actual runner options enable it only for q89 Wizard.
Movement uses the real collision navigator and ordinary receipts; the Shaman
target is not exempt from avoidance. Optional blocker clearance requires its
current same-map dead/HP-zero state or a fresh blocker-specific `ObjectDied`
after the attempt boundary. Independent quest progress plus AOI removal cannot
prove a blocker died. Unsafe or lost targets defer through ordinary recovery.

Root rejected the initial delivery for missing runner wiring and a test that
assigned a fixed endpoint, and additionally required strict blocker death proof.
Corrected tests use `createNavigator` with `loadProtocolCollisionMap('D2031')`,
record each physical movement cell and check both Shaman clearances. The
read-only review records those fixes separately from any live acceptance.

- Focused: **190/190**, SHA256 `FC4FE71DF02A51B1008D90C68E39EAD573D1BE850DBB00088FB6373F7FB99999`.
- Full controller: **645/645**, SHA256 `83A3D334E02E10BAED267A05143A763E095C41578FDAD488145186BF49C258FF`.
- Raw logs: `q89-spawn-stall-r124-focused.log`, `q89-spawn-stall-r124-full.log`.
- Review: `quest-agent-r124-review.md`.

Live run `Wizard.2026-09-15T19-37-28-605Z` logged in and resumed normally at q89.
By 19:38:57 UTC, it recorded two `spawnStallProtectedBlockerAttempt` and two
death-confirmed `spawnStallProtectedBlockerCleared` events, and the owner had
moved to D2031 (254,254), HP100/100, alive. CursedPriest was still 2/3;
ShiZombie and CursedZombie were 3/3 each. This proves the corrected live branch
was invoked and opened the initial search corridor. It does not prove quest
completion, Shaman immunity, the remaining routes, UI or animation acceptance.
Taoist R104 is stopped with its saved Lv25 checkpoint retained.
