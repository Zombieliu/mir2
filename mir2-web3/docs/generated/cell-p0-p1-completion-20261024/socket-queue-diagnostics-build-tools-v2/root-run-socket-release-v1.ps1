param([Parameter(Mandatory=$true)][ValidatePattern('^[a-f0-9]{40}$')][string]$ExpectedHead)
. 'C:/Program Files/Microsoft Visual Studio/2022/Community/Common7/Tools/Launch-VsDevShell.ps1' -Arch amd64 -HostArch amd64 -SkipAutomaticLocation
$env:CARGO_TARGET_DIR = 'C:/mir2-build/cell-preproduction-p0-p1-20261024/target'
$env:CARGO_BUILD_JOBS = '4'
$env:CARGO_PROFILE_DEV_DEBUG = '0'
$env:CARGO_PROFILE_TEST_DEBUG = '0'
$env:GIT_NO_LAZY_FETCH = '1'
$buildEnvironment = [pscustomobject]@{ kind='ownedReleaseBuildEnvironment'; cargoBuildJobs=$env:CARGO_BUILD_JOBS; targetDirectory=$env:CARGO_TARGET_DIR; targetArchitecture=$env:VSCMD_ARG_TGT_ARCH; vcToolsVersion=$env:VCToolsVersion; linker=(Get-Command link.exe -ErrorAction Stop).Source }
$buildEnvironment | ConvertTo-Json -Compress
node 'C:/mir2-build/p01-completion-20261024/socket-diagnostics-v2/build-socket-diagnostics-v2.mjs' --repository 'C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey' --expected-head $ExpectedHead
exit $LASTEXITCODE
