# Settings behavior audit — 2026-09-21

Source audit only; no current-package native visual acceptance.

- Sound/music bars currently substitute left/right +/-10 buttons for Crystal click-position/continuous drag (MainDialogs.cs SoundBar_MouseMove / MusicSoundBar_MouseMove). Replacement is in progress.
- SkillMode is persisted but the source Ctrl/tilde mapping on Bar1Skill1..Bar2Skill8 is absent. Source only remaps bindings already requiring Ctrl or tilde; unmodified/custom unrelated bindings must survive. Correction is in progress.
- NewMove is persisted but native right-click always starts pathing and a destination effect. Original GameScene uses this only for NewMove; old movement directly follows held right-click. This remains an open functional settings gap. Both initial path creation and held-right destination updates need gating; do not cancel attack-chase paths indiscriminately when fixing it.
- Background music is deliberately muted by DEVELOPMENT_BACKGROUND_MUSIC_ENABLED=false, introduced by 9dd74c90a during parity work. Its persisted slider is not audible proof. Preserve that policy until scoped audio acceptance; sound effects remain enabled independently.

Follow-up proof must include actual setting changes, persistence after reload, dispatch/render/audio effects where applicable, and same-build screenshots. Source and automated tests alone cannot close the Settings row.

## Settings controls and same-map task guidance candidate

- Original options Sound/Music bars now accept source-position click/drag with scaled pointer coordinates, modal/focus/session cancellation and closing-frame consumption. Native thumb placement matches source. Background-music development mute remains explicit; no audible-music pass is claimed.
- SkillMode now changes only original constrained Bar1/Bar2 Ctrl/tilde bindings, skips disabled keys, preserves unrelated/custom bindings and applies after load plus in the same frame as a setting change.
- NewMove now gates right-click path creation, held retargeting and destination marker. Classic mode uses direct held-right movement and stops on release. Disabling NewMove cancels only its pointer path, preserving accepted pending movement, pursuit and non-pointer navigation.
- Same-map secondary hunt tasks use authored unfinished-objective spawn regions when no live monster is visible. Labels identify a hunting region, never a guaranteed live target. Completion or absent progress excludes that region.

Verification: native UI library1005/1005 (settings-guidance-pass-tests.log); Windows input74/74 (settings-native-input-tests.log), including old/new movement and option transitions. Initial compilation exposed the Bevy tuple-size limit; nesting preserves system order. One isolated volume test fixture omitted the real per-frame pointer latch reset; fixed without weakening the production guard. An initial quest test filter matched zero and is not counted; the explicit authored_cat_region regression and final full suite pass. Same-build visual, sound and memory acceptance remain open.
