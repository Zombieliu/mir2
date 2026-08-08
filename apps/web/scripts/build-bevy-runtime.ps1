param(
    [string]$Profile = "release"
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$NodeScript = Join-Path $ScriptDir "build-bevy-runtime.mjs"

& node $NodeScript $Profile
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}
