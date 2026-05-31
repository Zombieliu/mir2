# Sound Import Runbook (local Crystal client → repo)

The web client resolves sounds **presence-aware**: only `.wav` files actually committed under
`apps/web/public/original-ui/Sound/` are playable; everything else degrades gracefully (no 404s,
recorded in audio diagnostics). Only 4 sounds are committed today
(`100`, `Login2`, `NewChar`, `Select2`). The remaining ~446 are raw-asset-limited — their bytes
live in the original Crystal client (the `Debug/` folder: `Debug/Sound/` with `SoundList.lst`
and the `.wav` files), not in this repo.

This runbook closes that gap from a machine where the Crystal `Debug/` folder is on local disk.
The pipeline is already built, tested end-to-end (`npm run test:sound-export`), and needs no code
changes — running it is purely an asset step.

## Prerequisites

- The Crystal client `Debug/` folder available locally, e.g. downloaded from Drive
  (`mir2/Debug/`). It must contain `Debug/Sound/SoundList.lst` and the `.wav` files.
- Node + deps installed in `apps/web` (`npm ci`).

## Steps

From `apps/web`:

```bash
# 1. Export every SoundList.lst entry whose .wav exists into public/original-ui/Sound/,
#    and regenerate public/original-ui/sound-index.generated.json from the real client list.
CRYSTAL_CLIENT_ROOT="/absolute/path/to/Debug" npm run export:crystal-sounds

# 2. Rebuild the present-sound manifest from the files that just landed on disk.
npm run generate:present-sounds

# 3. Verify the whole resource/audio system offline (manifest, coverage, preflight, tests).
npm run assets:verify:offline
```

`export:crystal-sounds` accepts the client root three ways (first wins):
`node ./scripts/export-crystal-sounds.mjs <path>` · `CRYSTAL_CLIENT_ROOT=<path>` ·
default `../../../downloads/crystal-client-full`. You may point at either `.../Debug` or
`.../Debug/Sound` directly.

## What to expect

- Console: `Exported <N>/<450> SoundList entries from <M> source wavs`.
- Three known fallbacks are normal and intended (the exact source clip is absent in current
  Crystal builds, so an adjacent clip keeps the id playable):
  `22.wav→23.wav` (id 10022), `109.wav→110.wav` (id 10109), `ZombieRevive.wav→64.wav` (id 705).
- Anything still missing is listed in the warning and recorded in the index with
  `sourceExists:false` — the client simply skips those ids.
- `report:asset-coverage` should jump from `4/450` toward `~450/450`.

## Commit

```bash
git add apps/web/public/original-ui/Sound/ \
        apps/web/public/original-ui/sound-index.generated.json \
        apps/web/lib/generated/crystal-present-sounds.generated.json \
        apps/web/docs/generated/assets docs/generated/assets
git commit -m "assets(sound): import Crystal sound library"
git push -u origin <branch>
```

Repo-size note: the full set is ~250–350 MB, dominated by a handful of multi-MB BGM tracks
(`Main`, `30xxx`, `Login3`, `Select3`). If you want to keep git lean, commit the small SFX and
host the large BGM on the CDN/R2 instead — the presence-aware resolver and the asset-version hash
(which already includes the present-sound manifest) handle either split without code changes.

## Why no code is needed

`lib/original-sound-index.ts` resolves a sound id only when its file is in the present-sound
manifest, so newly-exported sounds light up automatically; `lib/original-audio.ts` plays them and
records any still-missing ids. The chain — export → present manifest → resolver → playback — is
covered by `npm run test:sound-export` (synthetic Crystal `Sound/` fixture, no real client
needed), so a regression here fails CI rather than shipping silently.
