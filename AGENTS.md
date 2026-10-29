# mir2-web3 Agent Instructions

Primary project: `mir2-web3`, resolved relative to this repository root on
Windows, macOS, and Linux. Never assume a drive letter or absolute checkout path.

## Task scope and startup

- The current user request defines the scope. Finish that request without
  silently starting unrelated parity work. When asked to advance the project
  generally, continue the bounded queue toward **100% Candidate** without routine
  confirmation. Final **Accepted** still requires human frontend acceptance.
- For a small fix, explanation, review, or docs change, read only the affected
  files and their local instructions. Do not load project-wide progress history.
- For substantial implementation or resuming an autonomous queue, read
  `mir2-web3/docs/AGENT-CURRENT.md` and
  `mir2-web3/docs/AGENT-ORCHESTRATION.md` once per working context, then only the
  relevant topic sections. Re-read when the task or those instructions change.
- `AGENT-ORCHESTRATION.md` is the source of truth for coordination. The current
  summary routes to detailed evidence; it does not certify today's code or services.

## Search and context budget

- Start with `rg` in the relevant source/test/config directory. Widen only when
  the bounded search does not answer the question; do not enumerate the repo first.
- Root `.ignore` excludes historical copies, generated evidence and binary assets
  from default search. Inspect them explicitly with `rg --no-ignore` plus a narrow
  path when the task needs history, screenshots, manifests or packaged outputs.
- Large queue/roadmap/progress/handoff docs are reference material. Find a heading
  or symbol and read that section; do not dump entire files or repeat reads.
- Keep new run logs in one topic evidence record. Update affected status rows with
  a short result and link instead of copying the same log into multiple documents.

## Coordination and verification

- One code writer per high-conflict file/module per round, including
  `apps/simulation/src/runtime.rs` and its split runtime modules. Explorers stay
  read-only unless reassigned; every worker gets a bounded write set.
- Delegate independent work when useful; a small task does not require a fixed
  explorer/worker/QA ceremony. The coordinator owns integration and verification.
- Never revert unrelated user or agent work. Inspect the working tree before edits.
- Run focused checks for the affected behavior, adjacent regression for shared
  behavior, and workspace/full packaging checks at integration or release gates
  or when the change warrants them. Docs/search-only changes need link, diff and
  search checks, not game builds, browser QA or parity test suites.
- After tests pass, backend parity changes update the relevant roadmap/progress/
  parity sections; frontend parity changes update player QA/gaps after the relevant
  screenshots/tests pass. No parity update is needed for tooling-only changes.
- Mark completion only with matching evidence. Historical tests are not fresh
  validation; a bounded coverage fraction is not the whole-project percentage.

## Model policy

Prefer `gpt-5.3-codex-spark` for bounded work only when this session exposes it.
If unavailable, use the available current model; do not repeatedly probe stale
model names or account quota snapshots. A frontier reasoning model must lead and
verify cross-branch integration, auth/security, schema migrations, production
rollout and destructive cleanup. Use medium effort for exploration/docs, high for
normal implementation, and xhigh for bounded high-risk implementation when supported.

## Runtime and production invariants

- Session is personal state; Zone is shared authoritative world state. Do not
  simulate multiplayer by adding `RemotePlayer` to each personal session.
- Keep authoritative movement, occupancy, AOI and Join/Leave in the shared Zone.
  Disconnect/save must apply the final Zone transform to ECS and runtime state.
- Never expose normal clients to `WorldCommand::MoveTo`, `Stage5Command`, debug
  `crystal:<map>:<x>:<y>` teleport, raw `PasskeyLogin { account_id }`, or QA/admin
  commands such as `qa.giveItem`, `event.spawn`, `qa.openStorage`.
- Production must not fall back to a `demo` account. Reject unauthenticated
  StartGame/NewCharacter/DeleteCharacter.
- Before changing Zone/session/gateway routing or movement, read
  `mir2-web3/docs/SHARED-ZONE-MVP-RULES.md`. It preserves the complete original
  file boundaries, synchronous state machine, movement/AOI and save contracts.
- Stop conditions are in `mir2-web3/docs/AGENT-ORCHESTRATION.md`; ordinary local
  implementation decisions and fixes do not require routine confirmation.
