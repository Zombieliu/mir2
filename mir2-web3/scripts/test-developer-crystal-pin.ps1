[CmdletBinding()]
param(
    [string]$TemporaryRoot = [System.IO.Path]::GetTempPath()
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $false
Set-StrictMode -Version Latest

$Verifier = Join-Path $PSScriptRoot "verify-developer-crystal-pin.ps1"
$PowerShell = (Get-Process -Id $PID).Path
$TemporaryRoot = [System.IO.Path]::GetFullPath($TemporaryRoot)
$TestRoot = Join-Path $TemporaryRoot ("mir2-crystal-pin-test-" + [Guid]::NewGuid().ToString("N"))
$Remote = Join-Path $TestRoot "remote.git"
$Builder = Join-Path $TestRoot "builder"
$Parent = Join-Path $TestRoot "parent"
$Crystal = Join-Path $Parent "Crystal"
$Branch = "codex/handoff-parity-tools"
$BranchRef = "refs/heads/$Branch"
$Passed = 0

function Invoke-FixtureGit {
    param([string]$Directory, [string[]]$Arguments)

    $Output = @(& git -C $Directory @Arguments 2>&1)
    if ($LASTEXITCODE -ne 0) {
        throw "Fixture git $($Arguments -join ' ') failed (exit $LASTEXITCODE):`n$($Output -join "`n")"
    }
    return ($Output -join "`n").Trim()
}

function Assert-Verification {
    param([string]$Name, [bool]$ShouldPass, [string]$Message)

    $Before = Invoke-FixtureGit $Crystal @("rev-parse", "HEAD")
    $BeforeTrackingRefs = Invoke-FixtureGit $Crystal @("for-each-ref", "--format=%(refname) %(objectname)", "refs/remotes/")
    $BeforeGitlink = Invoke-FixtureGit $Parent @("ls-tree", "HEAD", "--", "Crystal")
    $Output = @(& $PowerShell -NoLogo -NoProfile -File $Verifier -RepositoryRoot $Parent 2>&1)
    $ExitCode = $LASTEXITCODE
    $Text = $Output -join "`n"
    if (($ExitCode -eq 0) -ne $ShouldPass -or $Text -notmatch [regex]::Escape($Message)) {
        throw "$Name failed (exit $ExitCode):`n$Text"
    }
    if ((Invoke-FixtureGit $Crystal @("rev-parse", "HEAD")) -ne $Before) {
        throw "$Name changed the Crystal checkout"
    }
    if ((Invoke-FixtureGit $Crystal @("for-each-ref", "--format=%(refname) %(objectname)", "refs/remotes/")) -ne $BeforeTrackingRefs -or
        (Invoke-FixtureGit $Parent @("ls-tree", "HEAD", "--", "Crystal")) -ne $BeforeGitlink) {
        throw "$Name changed a remote tracking ref or the parent gitlink"
    }
    if (Invoke-FixtureGit $Crystal @("for-each-ref", "--format=%(refname)", "refs/mir2-ci/crystal-pin/")) {
        throw "$Name left a temporary verification ref"
    }
    $script:Passed += 1
    Write-Host "[ok] $Name"
}

New-Item -ItemType Directory -Path $TestRoot | Out-Null
try {
    foreach ($Directory in @($Builder, $Parent)) {
        New-Item -ItemType Directory -Path $Directory | Out-Null
        Invoke-FixtureGit $Directory @("init", "--initial-branch=main") | Out-Null
        Invoke-FixtureGit $Directory @("config", "user.name", "Crystal pin fixture") | Out-Null
        Invoke-FixtureGit $Directory @("config", "user.email", "crystal-pin@example.invalid") | Out-Null
        Invoke-FixtureGit $Directory @("config", "commit.gpgsign", "false") | Out-Null
        Invoke-FixtureGit $Directory @("config", "core.hooksPath", (Join-Path $TestRoot "no-hooks")) | Out-Null
    }
    Invoke-FixtureGit $TestRoot @("init", "--bare", $Remote) | Out-Null
    Invoke-FixtureGit $Builder @("commit", "--allow-empty", "-m", "base") | Out-Null
    $Base = Invoke-FixtureGit $Builder @("rev-parse", "HEAD")
    Invoke-FixtureGit $Builder @("commit", "--allow-empty", "-m", "pinned") | Out-Null
    $Pin = Invoke-FixtureGit $Builder @("rev-parse", "HEAD")
    Invoke-FixtureGit $Builder @("commit", "--allow-empty", "-m", "branch advances") | Out-Null
    $Tip = Invoke-FixtureGit $Builder @("rev-parse", "HEAD")
    Invoke-FixtureGit $Builder @("checkout", "--detach", $Base) | Out-Null
    Invoke-FixtureGit $Builder @("commit", "--allow-empty", "-m", "diverged branch") | Out-Null
    $Diverged = Invoke-FixtureGit $Builder @("rev-parse", "HEAD")
    Invoke-FixtureGit $Builder @("push", $Remote, "${Tip}:${BranchRef}", "${Diverged}:refs/heads/diverged") | Out-Null
    Invoke-FixtureGit $Remote @("update-ref", $BranchRef, $Pin) | Out-Null

    # file:// forces a real shallow transport, rather than a local hardlink clone.
    $RemoteUri = ([System.Uri]::new($Remote)).AbsoluteUri
    Invoke-FixtureGit $Parent @("clone", "--depth=1", "--single-branch", "--branch", $Branch, $RemoteUri, $Crystal) | Out-Null
    Invoke-FixtureGit $Parent @("config", "--file", ".gitmodules", "submodule.Crystal.path", "Crystal") | Out-Null
    Invoke-FixtureGit $Parent @("config", "--file", ".gitmodules", "submodule.Crystal.url", $RemoteUri) | Out-Null
    Invoke-FixtureGit $Parent @("config", "--file", ".gitmodules", "submodule.Crystal.branch", $Branch) | Out-Null
    Invoke-FixtureGit $Parent @("add", ".gitmodules") | Out-Null
    Invoke-FixtureGit $Parent @("update-index", "--add", "--cacheinfo", "160000,$Pin,Crystal") | Out-Null
    Invoke-FixtureGit $Parent @("commit", "-m", "pin Crystal") | Out-Null

    if ((Invoke-FixtureGit $Crystal @("rev-parse", "--is-shallow-repository")) -ne "true") {
        throw "Fixture checkout must start shallow"
    }
    Assert-Verification "tip equality with a shallow checkout" $true "Verified Crystal $Pin"
    if ((Invoke-FixtureGit $Crystal @("rev-parse", "--is-shallow-repository")) -ne "false") {
        throw "Verification did not unshallow the checkout"
    }

    # Make the checkout shallow again before advancing the remote, so the old
    # pin/advanced-tip regression also requires fetching the complete graph.
    Invoke-FixtureGit $Crystal @("fetch", "--depth=1", "origin", $BranchRef) | Out-Null
    if ((Invoke-FixtureGit $Crystal @("rev-parse", "--is-shallow-repository")) -ne "true") {
        throw "Advanced-tip fixture must start shallow"
    }
    Invoke-FixtureGit $Remote @("update-ref", $BranchRef, $Tip) | Out-Null
    Assert-Verification "older pin remains an ancestor after remote advancement" $true "Verified Crystal $Pin"
    if ((Invoke-FixtureGit $Crystal @("rev-parse", "--is-shallow-repository")) -ne "false") {
        throw "Advanced-tip verification did not unshallow the checkout"
    }

    Invoke-FixtureGit $Remote @("update-ref", $BranchRef, $Diverged) | Out-Null
    Assert-Verification "non-ancestor fails despite a cached acceptable remote ref" $false "is not an ancestor"

    Invoke-FixtureGit $Remote @("update-ref", "-d", $BranchRef) | Out-Null
    Assert-Verification "missing remote branch fails closed" $false "Cannot fetch configured Crystal branch"

    Invoke-FixtureGit $Remote @("update-ref", $BranchRef, $Tip) | Out-Null
    Invoke-FixtureGit $Crystal @("remote", "set-url", "origin", (Join-Path $TestRoot "missing-remote.git")) | Out-Null
    Assert-Verification "unavailable remote fails closed" $false "Cannot fetch configured Crystal branch"

    Invoke-FixtureGit $Crystal @("remote", "set-url", "origin", $RemoteUri) | Out-Null
    Invoke-FixtureGit $Crystal @("checkout", "--detach", $Tip) | Out-Null
    Assert-Verification "checkout and gitlink mismatch fails" $false "Crystal mismatch"
    Invoke-FixtureGit $Crystal @("checkout", "--detach", $Pin) | Out-Null

    Invoke-FixtureGit $Parent @("config", "--file", ".gitmodules", "submodule.Crystal.branch", "bad..branch") | Out-Null
    Assert-Verification "invalid configured branch fails before fetching" $false "Invalid configured Crystal branch"
    Write-Host "[ok] Crystal pin verification: $Passed real Git fixtures passed."
}
finally {
    # Delete only the unique fixture directory inside the requested temp root.
    $Prefix = $TemporaryRoot.TrimEnd([char[]]@('/', '\')) + [System.IO.Path]::DirectorySeparatorChar
    $ResolvedTestRoot = [System.IO.Path]::GetFullPath($TestRoot)
    if (-not $ResolvedTestRoot.StartsWith($Prefix, [System.StringComparison]::OrdinalIgnoreCase) -or
        [System.IO.Path]::GetFileName($ResolvedTestRoot) -notlike "mir2-crystal-pin-test-*") {
        throw "Refusing to remove an unexpected fixture path: $ResolvedTestRoot"
    }
    Remove-Item -LiteralPath $ResolvedTestRoot -Recurse -Force
}
