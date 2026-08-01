# World Director branch integration audit

Status date: 2026-08-01

Fixed comparison:

- Main: `09499bd0`
- Audited branch: `origin/codex/world-director-approval-beta` at `b6e0b21e`
- Merge base: `49d37320`

Do not merge the branch tip into main. The visible 69-commit divergence is
misleading: the first 57 branch commits were already squash-integrated by main
commit `e3dfa73b9`. Reapplying them would duplicate Gate 15-21, Regional, Home
Node, and Dubhe work across hundreds of files.

The true unintegrated tail is the 12 commits after `90e9ba4d0`.

## Integration decision

| Area | Commit stack | Decision |
| --- | --- | --- |
| Production spectator and replay | `c999b114f` | Rebase as a new PR; default private and disabled |
| AI daily report and Discord | `b38c83b14` | Rebase after spectator; retain migration `0008`, discard `progress.md` |
| AI live distribution | `2af28b7b7`, `8a3cdb0e1`, `c84619bb5` | Move only as one ordered stack after spectator |
| World Director approval | `6d608d75c`, `4de7dc954`, `ac4a912ce` | Separate security-reviewed PR after daily reports |
| Admin API Linux release | `e7f26c395`, `c19fc513b` | Safe ordered cherry-pick candidate |
| Channel identity | `6b9de3879` | Redesign on current auth; use a formal `0010` migration |
| R2 thin client | `b6e0b21ea` | Reimplement after Bevy 0.19 and low-end asset policy settle |

The R2 thin-client commit must not be cherry-picked as-is. It conflicts with
the current pinned R2 release, changes the Bevy publication wrapper, and deletes
the tracked legacy WebGL2 package used by local/offline fallback. It contains no
KTX2 implementation and no Bevy engine upgrade.

## Conflict boundaries

- `apps/web/app/page.tsx`
- `apps/web/app/original-client-shell.tsx`
- `apps/web/app/globals.css`
- `apps/web/package.json`
- `apps/web/next.config.ts`
- `apps/web/scripts/build-bevy-runtime.mjs`
- `apps/gateway/src/web.rs`
- `apps/simulation/src/db_projection.rs`

Generated screenshots, `progress.md`, and old AI-live artifacts must not be
copied from the branch. Regenerate acceptance evidence from current main after
each rebased stack passes its security and runtime tests.
