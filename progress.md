Original prompt: 目前缺少：摇杆拖动移动教学、Walk/Run 模式说明、Attack/Approach/Pick 按钮说明、S1-S3/物品快捷键说明、Char/Bag 移动面板入口说明、从手机菜单重新打开教程入口、根据 touch/gamepad/keyboardMouse 显示不同文案。给我写一个。

## Progress

- Created isolated worktree `feat/mobile-controls-tutorial` from current `origin/main`.
- Confirmed the existing beginner tutorial is desktop-only and runs once from localStorage.
- Added separate keyboard/mouse, touch, and gamepad tutorial flows with versioned per-input completion state.
- Added touch lessons for joystick drag, Walk/Run, Attack, Approach, Pick, S1-S3/items, Char/Bag, and menu replay.
- Wired semantic touch/gamepad control events so using the highlighted control advances the guide.
- Added profile-aware Help copy and `Menu -> Help -> Replay controls tutorial`.
- Localized the new Help/replay copy for English, Simplified Chinese, Spanish, and Brazilian Portuguese.
- Browser-verified the touch flow at 932x430: the tutorial card does not overlap the joystick, Run, or Attack; real joystick drag and Run clicks advance their steps; Help replay restarts at step 1.
- Verified `render_game_to_text` exposes tutorial state for browser QA.
- Passed tutorial flow (18 checks), device-profile, responsive-stage, gamepad-input, and TypeScript checks.

## TODO

- Review the final diff and commit the implementation branch.
