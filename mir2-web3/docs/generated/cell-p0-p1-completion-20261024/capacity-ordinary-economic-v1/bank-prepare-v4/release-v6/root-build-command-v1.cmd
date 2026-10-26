@call "C:\Program Files\Microsoft Visual Studio\2022\Community\Common7\Tools\VsDevCmd.bat" -arch=amd64 -host_arch=amd64
@if errorlevel 1 exit /b %errorlevel%
@set "CARGO_TARGET_DIR=C:\mir2-build\cell-preproduction-p0-p1-20261024\target"
@set "CARGO_BUILD_JOBS=4"
@set "GIT_NO_LAZY_FETCH=1"
@"C:\Program Files\nodejs\node.exe" "C:\mir2-build\p01-completion-20261024\ordinary-hp-wake-release-v6\build-release-v6.mjs" --repository "C:\Users\Administrator\.codex\worktrees\cell-preproduction-p0-p1\mir2-player-journey" --expected-head ee06c05f3813a1e627c8756d1f5fe8e2e0fbf856 --config "C:\mir2-build\p01-completion-20261024\ordinary-hp-wake-release-v6\root-qualified-config-v2.json" --config-sha256 fc7dca18b93d42180e96e89be6a46f1ea94d108077bbd4382f75aab8e52722c0
@exit /b %errorlevel%
