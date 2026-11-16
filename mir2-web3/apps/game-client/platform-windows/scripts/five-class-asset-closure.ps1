# Explicit source-bound closure for five-class Candidate packages. The ordinary
# classic profile remains unchanged; neither guard mutates exported assets.
function Assert-FiveClassAssetClosure {
    param([Parameter(Mandatory=$true)][string]$ProjectRoot,
          [Parameter(Mandatory=$true)][string]$AssetRoot,
          [Parameter(Mandatory=$true)][string]$ProfilePath)
    $profile = Get-Content -LiteralPath $ProfilePath -Raw | ConvertFrom-Json
    if ($profile.profileId -cne 'platinum_fiveclass') { throw 'Five-class closure requires its exact source profile' }
    $pipeline = Join-Path $ProjectRoot 'apps/web/scripts/asset-pipeline'
    $node = (Get-Command node -ErrorAction Stop).Source
    foreach ($kind in @('sprite','skill')) {
        $validator = Join-Path $pipeline "five-class-$kind-closure.mjs"
        $requirements = Join-Path $pipeline "five-class-$kind-requirements.json"
        if (-not (Test-Path -LiteralPath $validator -PathType Leaf) -or
            -not (Test-Path -LiteralPath $requirements -PathType Leaf)) { throw "Missing five-class $kind proof" }
        & $node $validator --mode verify --profile $ProfilePath --assets $AssetRoot --requirements $requirements
        if ($LASTEXITCODE -ne 0) { throw "Five-class $kind asset closure failed" }
    }
}
