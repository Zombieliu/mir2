# Fail-closed publisher identity gate for the Windows Candidate publication job.
# The expected UID is supplied by the protected GitHub environment. This script
# deliberately has no fallback/default publisher identity.
[CmdletBinding()]
param(
    [string]$ExpectedPublisherUid = $env:MIR2_WINDOWS_CANDIDATE_PUBLISHER_UID,
    [string]$ActualPublisherUid = $env:GITHUB_ACTOR_ID,
    [string]$RepositoryId = $env:GITHUB_REPOSITORY_ID,
    [string]$RunId = $env:GITHUB_RUN_ID,
    [string]$RunAttempt = $env:GITHUB_RUN_ATTEMPT,
    [string]$SourceRevision = $env:GITHUB_SHA,
    [string]$OutputPath = '',
    [switch]$SelfTest
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Assert-PositiveDecimalUid {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [AllowEmptyString()][string]$Value
    )

    if ([string]::IsNullOrWhiteSpace($Value)) {
        throw "$Name is mandatory and must come from protected configuration"
    }
    if ($Value -cnotmatch '^[1-9][0-9]{0,19}$') {
        throw "$Name must be a positive decimal GitHub identity ID"
    }
}

function Assert-PositiveDecimalValue {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [AllowEmptyString()][string]$Value
    )

    if ([string]::IsNullOrWhiteSpace($Value) -or $Value -cnotmatch '^[1-9][0-9]{0,19}$') {
        throw "$Name must be a positive decimal value"
    }
}

function Test-PublisherContract {
    param(
        [AllowEmptyString()][string]$Expected,
        [AllowEmptyString()][string]$Actual
    )

    Assert-PositiveDecimalUid -Name 'ExpectedPublisherUid' -Value $Expected
    Assert-PositiveDecimalUid -Name 'ActualPublisherUid' -Value $Actual
    if ($Expected -cne $Actual) {
        throw "protected publisher UID does not match the GitHub workflow actor ID"
    }
}

if ($SelfTest) {
    Test-PublisherContract -Expected '123456' -Actual '123456'

    foreach ($case in @(
        @{ Expected = ''; Actual = '123456'; Label = 'missing expected UID' },
        @{ Expected = '123456'; Actual = ''; Label = 'missing actual UID' },
        @{ Expected = 'publisher'; Actual = 'publisher'; Label = 'non-numeric UID' },
        @{ Expected = '123456'; Actual = '654321'; Label = 'mismatched UID' }
    )) {
        $rejected = $false
        try {
            Test-PublisherContract -Expected $case.Expected -Actual $case.Actual
        } catch {
            $rejected = $true
        }
        if (-not $rejected) {
            throw "publisher contract self-test accepted $($case.Label)"
        }
    }

    Write-Host 'verify-windows-publisher-contract self-test passed'
    exit 0
}

Test-PublisherContract -Expected $ExpectedPublisherUid -Actual $ActualPublisherUid
Assert-PositiveDecimalValue -Name 'RepositoryId' -Value $RepositoryId
Assert-PositiveDecimalValue -Name 'RunId' -Value $RunId
Assert-PositiveDecimalValue -Name 'RunAttempt' -Value $RunAttempt
if ([string]::IsNullOrWhiteSpace($SourceRevision) -or $SourceRevision -cnotmatch '^[0-9a-fA-F]{40}$') {
    throw 'SourceRevision must be an exact 40-character Git revision'
}
if ([string]::IsNullOrWhiteSpace($OutputPath)) {
    throw 'OutputPath is mandatory'
}

$resolvedOutput = [IO.Path]::GetFullPath($OutputPath)
$parent = Split-Path -Parent $resolvedOutput
if ([string]::IsNullOrWhiteSpace($parent)) {
    throw 'OutputPath must have a parent directory'
}
[IO.Directory]::CreateDirectory($parent) | Out-Null

$identity = [ordered]@{
    schema = 'mir2.windows.publisher-identity.v1'
    publisherUid = $ActualPublisherUid
    repositoryId = $RepositoryId
    runId = $RunId
    runAttempt = $RunAttempt
    sourceRevision = $SourceRevision.ToLowerInvariant()
    verified = $true
}
$json = ($identity | ConvertTo-Json -Depth 4) + "`n"
[IO.File]::WriteAllText($resolvedOutput, $json, [Text.UTF8Encoding]::new($false))

Write-Host "publisherUid=$ActualPublisherUid"
Write-Host "publisherIdentity=$resolvedOutput"
