# Agent Orchestration

Current coordination policy, revised 2026-10-23 (Asia/Shanghai).
Resolve all project paths relative to the repository root and `mir2-web3`.

## Scope and reading

- The current user request takes priority over historical next-task instructions.
  For an explicit bounded task, finish that task. For general project advancement,
  select a bounded unchecked item and continue without routine confirmation.
- A small fix/review/question reads relevant files only. Substantial implementation
  or queue resumption reads this file and [current context](AGENT-CURRENT.md) once
  per working context, then the relevant topic sections. Do not re-read unchanged
  instructions at every step or load the five large progress documents in full.
- The queue, roadmap, parity docs and [old handoff](AGENT-RESUME-HANDOFF.md) retain
  detailed evidence. Their dated ports, paths, PIDs, hashes, model quotas, test
  counts and toolchain choices are historical snapshots; confirm only what the
  present task needs. They are not startup commands or current environment facts.

## Ownership and useful parallelism

- Coordinator chooses scope, assigns bounded write sets, integrates and verifies.
- Explorers are read-only unless explicitly reassigned. Use parallel work for
  independent investigation or disjoint changes that save time; no fixed team
  ceremony is required for a small change.
- Only one writer may edit a high-conflict file or tightly coupled module at a
  time, including `apps/simulation/src/runtime.rs` and split runtime modules.
- Docs workers and code workers must not edit the same docs concurrently.
- Inspect the working tree and preserve unrelated user/agent changes. Never
  discard work, overwrite a foreign task's files or claim its tests as your own.
- Use models available in the current session under root `AGENTS.md` policy.
  Historical quota percentages do not determine current availability or scheduling.

## One bounded round

1. Identify the requested behavior, affected files and acceptance evidence.
2. Inspect the relevant source and matching Crystal behavior when parity is involved.
3. Implement the smallest complete change; delegate independent work when useful.
4. Run checks appropriate to the impact, repair failures caused by this change.
5. Update only the affected status sections with a concise result and evidence link.
6. Finish the explicit task, or select the next bounded item for general advancement.

## Verification by impact

| Change | Required verification |
| --- | --- |
| Docs, instructions, search configuration | Diff, links, preservation and relevant search checks |
| Local behavior | Focused existing tests; add meaningful missing behavior coverage |
| Shared behavior, protocol, auth, save or routing | Focused and adjacent regression, affected boundary checks |
| Frontend behavior/visual change | Relevant logic/type checks and screenshot or UI smoke evidence |
| Integration, release or broad build/config change | Affected workspace/build/package gates |
| Final parity acceptance | Matching Crystal traces, player QA and human-only acceptance |

Do not run a full workspace build, package export or browser session for an
unrelated docs/small local change. Widen verification when dependencies, failures
or unresolved risk justify it; do not repeat successful checks without a reason.
Actual build/resource/credential safety preconditions still apply to commands
that need them; reducing routine scope does not bypass a required release gate.

## Evidence and status maintenance

- Checkboxes require matching commands, screenshots, packet traces or source
  comparison. Historical results apply only to their recorded snapshots.
- Backend parity changes update the relevant sections of
  [roadmap](CRYSTAL-1TO1-ROADMAP.md), [backend progress](BACKEND-1TO1-PROGRESS.md)
  and [server parity](CRYSTAL-SERVER-PARITY.md) after tests pass.
- Frontend parity changes update [frontend gaps](FRONTEND-1TO1-GAPS.md) or the
  applicable player QA doc after relevant screenshots/tests pass.
- Store detailed run output once in a topic record; link it from status documents.
  Do not paste the same round narrative into every progress file.
- Keep [current context](AGENT-CURRENT.md) short: material current decisions,
  known open work and topic links. Replace superseded summaries instead of
  prepending an ever-growing log. Keep each startup document under 120 lines.
- **100% Candidate** requires evidence for the applicable code/data/test/QA gates.
  **100% Accepted** requires final human gameplay/frontend acceptance. Blocked
  items remain visible; partial coverage does not imply global completion.

## Stop conditions

Do not stop for normal implementation choices, local refactors, test failures,
generated data refreshes or documentation updates. Seek missing input only when:

- destructive filesystem operations are necessary and not already authorized;
- required credentials, private endpoints or a live Crystal address are missing;
- required Crystal source/assets are unavailable;
- acceptance standards genuinely conflict and cannot be resolved from source;
- final human-only acceptance is needed to move from Candidate to Accepted.

Continue independent authorized work when a missing input blocks only one part.

## Historical record

The complete previous orchestration file is preserved byte-for-byte in
[orchestration history](AGENT-ORCHESTRATION-HISTORY-20261023.md). The upstream
version integrated on 2026-10-24 is also preserved byte-for-byte in
[upstream orchestration history](AGENT-ORCHESTRATION-UPSTREAM-HISTORY-20261024.md).
Read a relevant section with an explicit path only when investigating that recorded round.
The detailed [Shared Zone MVP rules](SHARED-ZONE-MVP-RULES.md) are required for
Zone/session/gateway movement and lifecycle work, not unrelated edits.
