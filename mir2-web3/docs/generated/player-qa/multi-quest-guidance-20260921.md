# Multi-quest guidance

Newcomer guidance now has one primary task. Chapter recommendation is the
default; the player can choose an active task in its detail footer or the HUD.
The local choice survives roster ordering and ready-to-turn-in updates, then
falls back after authoritative completion/removal. It is session-local and
separate from Crystal's persisted multi-track list.

HUD displays all objective counters, turn-in/accept actions, a detail button,
map button, up to two nearest known secondary targets, and a separate list of
other-map tasks. Additional/unknown tasks remain discoverable in the diary.
Locations come from visible monsters, cached server NPCs, imported spawn areas,
and authored V2 map names. Unknown destinations are explicitly not invented.

Big-map hunt regions prioritize the primary task before the three-area limit;
its border is cyan and labelled Tracked, with other targets amber. N5's safe
destination is only highlighted while N5 is primary. This is presentation only:
no teleport, automatic cross-map exit routing, or quest progress mutation.

Tests cover manual precedence, completion fallback, recommendation, nearby
ordering/limit, full counters and rendered text, authored other-map names, and
primary map marking without changing source coordinates or progress.
Full native UI suite passes 883/883. Live screenshots, long-text layout and
ordinary multi-task play remain pending package handoff. Simultaneous server
credit for one kill across multiple quests is not changed or newly proven here.
