param([string]$LogName,[string[]]$CargoArgs)
. 'C:/Program Files/Microsoft Visual Studio/2022/Community/Common7/Tools/Launch-VsDevShell.ps1' -Arch amd64 -HostArch amd64 -SkipAutomaticLocation
$env:CARGO_TARGET_DIR='C:/mir2-build/cell-preproduction-p0-p1-20261024/target'
$env:CARGO_BUILD_JOBS='4'
$env:CARGO_PROFILE_DEV_DEBUG='0'
$env:CARGO_PROFILE_TEST_DEBUG='0'
$taskLog=Join-Path 'C:/mir2-build/p01-completion-20261024' $LogName
cargo +1.95.0 @CargoArgs *> $taskLog
$taskExit=$LASTEXITCODE
Get-Content -LiteralPath $taskLog -Tail 55
exit $taskExit
