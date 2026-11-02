[CmdletBinding()]
param(
    [string]$RepositoryRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot "../.."))
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $false
Set-StrictMode -Version Latest

function Invoke-CheckedGit {
    param([string]$Directory, [string[]]$Arguments, [string]$Failure)

    $Output = @(& git -C $Directory @Arguments 2>&1)
    if ($LASTEXITCODE -ne 0) {
        throw "$Failure (git exit $LASTEXITCODE):`n$($Output -join "`n")"
    }
    return ($Output -join "`n").Trim()
}

$RepositoryRoot = [System.IO.Path]::GetFullPath($RepositoryRoot)
$CrystalRoot = Join-Path $RepositoryRoot "Crystal"
$Gitlink = Invoke-CheckedGit $RepositoryRoot @("ls-tree", "HEAD", "--", "Crystal") "Cannot read the Crystal gitlink"
if ($Gitlink -notmatch '^160000 commit ([0-9a-f]{40,64})\tCrystal$') {
    throw "HEAD does not contain a valid Crystal gitlink"
}
$Expected = $Matches[1]
$Actual = Invoke-CheckedGit $CrystalRoot @("rev-parse", "--verify", "HEAD^{commit}") "Cannot read the Crystal checkout"
if ($Expected -ne $Actual) {
    throw "Crystal mismatch: expected $Expected, found $Actual"
}

$Branch = Invoke-CheckedGit $RepositoryRoot @(
    "config", "--file", ".gitmodules", "--get", "submodule.Crystal.branch"
) "Cannot read the configured Crystal branch"
Invoke-CheckedGit $RepositoryRoot @("check-ref-format", "refs/heads/$Branch") "Invalid configured Crystal branch" | Out-Null
$Shallow = Invoke-CheckedGit $CrystalRoot @("rev-parse", "--is-shallow-repository") "Cannot inspect Crystal history"
if ($Shallow -notin @("true", "false")) {
    throw "Cannot determine whether Crystal history is shallow"
}

# Fetch in the checkout to retain actions/checkout's local authentication. Only
# a unique verification ref is updated; no checkout, reset or gitlink change.
$VerificationRef = "refs/mir2-ci/crystal-pin/" + [Guid]::NewGuid().ToString("N")
try {
    $FetchArguments = @(
        "fetch", "--no-tags", "--no-recurse-submodules", "--no-write-fetch-head",
        "--filter=blob:none", "--refmap="
    )
    if ($Shallow -eq "true") {
        $FetchArguments += "--unshallow"
    }
    $FetchArguments += @("origin", "+refs/heads/${Branch}:${VerificationRef}")
    Invoke-CheckedGit $CrystalRoot $FetchArguments "Cannot fetch configured Crystal branch '$Branch'" | Out-Null
    $Shallow = Invoke-CheckedGit $CrystalRoot @("rev-parse", "--is-shallow-repository") "Cannot inspect fetched Crystal history"
    if ($Shallow -ne "false") {
        throw "Fetched Crystal history is still shallow; cannot prove ancestry"
    }

    $Tip = Invoke-CheckedGit $CrystalRoot @("rev-parse", "--verify", "${VerificationRef}^{commit}") "Cannot resolve the fetched Crystal branch"
    Invoke-CheckedGit $CrystalRoot @(
        "merge-base", "--is-ancestor", $Expected, $Tip
    ) "Crystal commit $Expected is not an ancestor of configured handoff branch '$Branch'" | Out-Null
    Write-Host "Verified Crystal $Expected is checked out and is an ancestor of $Branch ($Tip)."
}
finally {
    Invoke-CheckedGit $CrystalRoot @("update-ref", "-d", $VerificationRef) "Cannot remove the temporary Crystal verification ref" | Out-Null
}
