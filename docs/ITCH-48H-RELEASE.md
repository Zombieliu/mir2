# Mir2 Platinum 1.76 Web Alpha — 48-hour itch.io release

## Release outcome

Ship a browser-playable Alpha on itch.io without treating the unfinished natural
22–50 progression, long soak, Windows client, or large closed test as launch
blockers.

The itch build is intentionally a small HTML5 launcher. The production game
continues to run at `https://mir2.obelisk.build`, with its existing HTTPS/WSS
Gateway, R2/CDN assets, PostgreSQL, and Redis deployment.

## Scope freeze

Required before the page becomes Public Unlisted:

- itch page launches the production game;
- the production Realm reports `platinum_176` Profile v6 and the expected content
  bundle identity;
- a fresh player can register, create/select a character, and enter the world;
- movement, normal combat, death/revive, drops, pickup, inventory, equipment,
  shops, and one map transfer work;
- logout/reconnect retains the character;
- two browser clients can enter the same Zone;
- Platinum UI does not expose Mail, credit Game Shop, On-chain Mine, or other
  out-of-profile systems;
- no P0/P1 launch blocker remains;
- the page clearly says `Web Alpha` and `In development`.

Deferred until after the itch Alpha:

- natural browser-play certification from level 22 to 50;
- final late-game XP/drop/economy balancing;
- two-hour Postgres/Redis soak;
- two-hour 20–50-player concentrated playtest;
- Windows/Tauri packaging;
- new maps, bosses, systems, or UI parity work.

## Build

From the repository root:

```bash
bash scripts/build-itch-html5.sh
node scripts/certify-itch-alpha.mjs
```

Artifact:

```text
dist/itch/mir2-platinum-176-web-alpha-html5.zip
```

The archive contains `index.html` at its root and stays within itch.io's HTML5
file-count, path-length, single-file, and unpacked-size limits.

## itch.io project settings

- Kind of project: `HTML`
- Release status: `In development`
- Pricing: free or donation-only for the Alpha
- Embed: `Click to launch in fullscreen`
- Mobile friendly: leave disabled until it is actually tested
- Visibility during validation: `Restricted`
- Visibility at the 48-hour milestone: `Public` + `Unlisted in search & browse`
- Upload: `mir2-platinum-176-web-alpha-html5.zip`
- Do not enable public search indexing until page presentation, rights, and the
  first two-hour observation are accepted.

## Store-page copy

### Short description

Browser-playable Mir2 Platinum 1.76 Alpha focused on the classic three-class
progression loop, combat, drops, equipment, maps, social systems, and persistent
characters.

### Current build

This is an in-development Web Alpha. The current release prioritizes reliable
login, world entry, combat, loot, equipment, map transfer, multiplayer presence,
and reconnect persistence. Level 22–50 content exists but its natural pacing and
late-game balance are still being calibrated.

### Controls

- Left click: select, move, and interact
- Keyboard shortcuts: shown inside the game client
- F: toggle launcher fullscreen
- If the embedded client does not load, use `Open in new window`

### Known limitations

- Desktop Chrome or Edge is recommended.
- Mobile is not currently certified.
- Late-game pacing and class balance are not final.
- This Alpha is an online game and requires the production server to be
  available.

## Minimal pre-public test budget

Maximum 90 minutes:

1. 30 minutes: fresh-account solo flow.
2. 15 minutes: two-client shared-Zone flow.
3. 15 minutes: logout/reconnect persistence.
4. 15 minutes: itch cold launch, fullscreen, audio, asset, and WSS checks.
5. 15 minutes: verify only the fixes made during the smoke pass.

The two-hour observation runs after the Public Unlisted release and replaces a
separate pre-release soak for this milestone.

## P0/P1 stop-ship conditions

- black screen or no world entry for a fresh account;
- production HTTPS/WSS or required asset origin is unavailable;
- widespread missing maps, characters, or UI assets;
- reconnect loses or corrupts character state;
- server crash, item duplication, or authentication bypass;
- the launcher cannot load the embedded client and its new-window fallback also
  fails.
- production serves a stale/non-Platinum build or does not report the expected
  `platinum_176` v6 Realm identity.

## Rollback

1. Change itch visibility back to `Restricted`.
2. Restore the last known-good production Web deployment.
3. Restore the last known-good Gateway release without changing persisted player
   data.
4. Keep the itch project URL and page intact; replace the HTML5 channel only
   after the fix passes the minimal smoke.

## Publication boundary

Before enabling public search indexing, confirm the right to distribute every
game name, trademark, image, map, sound, and other bundled asset. Restricted or
unlisted access is a rollout control, not a substitute for distribution rights.
