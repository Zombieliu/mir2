# Newcomer V2 Web guidance candidate

The Quest Diary now groups the authoritative V2 rows into the six reviewed
chapters. Its journey count uses the fixed 26-quest route; missing records never
count as completed. The active chapter is derived only from in-progress or
ready-to-turn-in server rows. Existing accept, finish and tracking commands are
unchanged.

English and Chinese titles cover all 22 main quests and four growth claims.
Objective labels match the server's text rather than its array order, preserving
current, required and done fields. Unknown labels remain intact. Tests generate
all main kill/flag labels from the runtime configuration and reverse their order,
including the Node 21 independent kill and class-practice counters.

The display-only graduation choices use actual class, actual server level and
all 26 completed records. Missing level/history hides the handoff. The chooser
is outside the selected-quest branch, so it remains available when the Active
filter is empty after graduation. Equipment/Skill/Challenge buttons update only
local selection, with no accept/finish command or rewards. Closing clears the
choice. The equipment acquisition gap is recorded in graduation-acquisition.md.

Root verification: `test-newcomer-v2-journey.mjs`,
`test-quest-localization.mjs` and `npx tsc --noEmit --incremental false`
pass after the empty-filter correction. The React review checks top-level
hooks, memoized derivation, static module data, button type and aria-pressed;
no new fetch, effect-driven server command or server action is introduced.
This is a tested UI candidate, not a screenshot or Crystal comparison.

`accepted=false`, `visualAccepted=false`, `globalParityPercent=null`.
