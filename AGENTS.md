# mir2-web3 Agent Instructions

Primary project: `E:\mir2\mir2-web3`

Use `docs/AGENT-ORCHESTRATION.md` as the source of truth for multi-agent coordination.

## Default Goal

Drive the project toward **100% Candidate** Crystal / Mir2 1:1 parity before requesting final human frontend acceptance.

Do not ask for routine confirmation. Proceed autonomously through the current task queue unless a stop condition in `docs/AGENT-ORCHESTRATION.md` applies.

## Required Reading

Before planning substantial work, read:

- `E:\mir2\mir2-web3\docs\AGENT-ORCHESTRATION.md`
- `E:\mir2\mir2-web3\docs\AGENT-TASK-QUEUE.md`
- `E:\mir2\mir2-web3\docs\CRYSTAL-1TO1-ROADMAP.md`
- `E:\mir2\mir2-web3\docs\BACKEND-1TO1-PROGRESS.md`
- `E:\mir2\mir2-web3\docs\CRYSTAL-SERVER-PARITY.md`

## Coordination

- Only one code worker may edit a high-conflict file such as `apps/simulation/src/runtime.rs` per round.
- Explorers should be read-only unless explicitly reassigned.
- Backend parity changes must update roadmap/progress/parity docs after tests pass.
- Frontend parity changes must update the player QA or frontend gaps docs after screenshots/tests pass.
- Never revert unrelated user or agent work.

## Model Policy

Current preferred worker model is `gpt-5.3-codex-spark`.

- `xhigh`: bounded high-risk implementation.
- `high`: normal implementation.
- `medium`: exploration, QA planning, docs.

Avoid unsupported account models unless availability is confirmed.

