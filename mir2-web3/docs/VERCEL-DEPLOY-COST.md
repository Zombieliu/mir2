# Vercel deploy cost: stop rebuilding on every merge to `main`

> Owner: architect/review session. 2026-05-31.
> Answers: "don't run a Vercel build on every PR merge to main — can't several
> PRs merge and trigger one build?"

## The situation

The web app (`apps/web`, Next.js) deploys via Vercel's **Git integration**: every
push to the production branch `main` auto-triggers a build. That build is
**heavy** — `apps/web/scripts/vercel-build.sh` installs the Rust toolchain +
`wasm-bindgen` and compiles the **Bevy runtime to WASM**, then runs `next build`.
Running it per-merge is the cost you're seeing.

But here's the key fact: **most merges don't change the web app at all.** Of the
six PRs open right now, only **#13** touches `apps/web/`; **#21, #17, #8, #19,
#22** are pure Rust/docs and currently trigger a full rebuild *for nothing*.

So the right fix isn't "manually batch the merges" — it's **skip the build when
nothing the web bundle depends on changed.** Then non-web merges cost zero build,
automatically, with no coordination.

## The fix (recommended): path-aware Ignored Build Step

`vercel-build.sh`'s own comment already names this as "the clean, complete fix."
Implemented here as `apps/web/scripts/vercel-ignore-build.sh`.

**One-time setup (Vercel dashboard):** Project → Settings → Git → *Ignored Build
Step* → set to:

```
bash scripts/vercel-ignore-build.sh
```

(path is relative to the project Root Directory, `mir2-web3/apps/web`.)

Vercel convention: the command exits **0 → skip the build** (keep current
production deploy live); **1 → build**. The script builds only when the push
touched the web bundle's real inputs:

| Build-trigger path | Why |
| --- | --- |
| `mir2-web3/apps/web/` | the Next app, its `public/` assets, `package-lock.json`, `vercel.json` |
| `mir2-web3/apps/game-client/runtime/` | the Bevy runtime Rust source compiled to WASM |

Everything else (`apps/simulation`, `apps/gateway`, `apps/admin-*`, `docs/`,
`packages/`, `Crystal/`, `.github/`) cannot change the deployed bundle, so it
skips. `apps/web` has **no** workspace `packages/` deps, so the list is exactly
these two (verified against `vercel-build.sh`, `build-bevy-runtime.mjs`,
`package.json`). It defaults to **building** if it can't compute the diff (first
commit / shallow clone) so it never ships a stale bundle. Override with
`MIR2_FORCE_VERCEL_BUILD=1`.

Effect on the current merge queue: merging #21/#17/#8/#19/#22 → **no build**;
merging #13 → **one build**. No manual batching needed.

## If you also want to batch *web* PRs into one deploy

The Ignored Build Step handles the non-web PRs. If you have **several web PRs**
and want a single deploy for the batch, two options:

1. **Staging branch (zero config).** Merge the web PRs into a `staging` branch,
   then open one PR `staging → main`. Only the final merge to `main` builds.
   Keeps auto-deploy on for `main`.
2. **Disable auto-deploy on `main`, deploy manually.** Add to
   `apps/web/vercel.json`:
   ```json
   { "git": { "deploymentEnabled": { "main": false } } }
   ```
   Then deploy when you choose with the existing scripts:
   `npm run vercel:build:prod && npm run vercel:deploy:prod`. Full control over
   *when* prod deploys, at the cost of remembering to do it. (The Ignored Build
   Step is unnecessary in this mode — nothing auto-deploys.)

**Recommendation:** wire the Ignored Build Step (option above) — it removes the
per-merge cost for the 5 non-web PRs immediately with no workflow change. Reach
for the staging branch only when you're landing multiple *web* PRs at once.

## Verify it's working

After setting the Ignored Build Step, merge a Rust/docs PR and confirm the Vercel
dashboard shows the deployment as **"Skipped"** (not a full build). Merge a web
PR and confirm it builds. Locally:

```
# from repo root, simulate the latest push:
bash mir2-web3/apps/web/scripts/vercel-ignore-build.sh; echo "exit=$? (0=skip,1=build)"
```
