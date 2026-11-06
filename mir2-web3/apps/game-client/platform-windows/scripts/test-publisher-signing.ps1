Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'publisher-signing.psm1') -Force
$good = @{SelfSigned=$false; CodeSigningEku=$true; RsaKey=$true; TimeValid=$true; HasPrivateKey=$true; ChainValid=$true; MicrosoftProgramRoot=$true}
if (@(Get-PublisherCertificateIssues -Facts ([pscustomobject]$good) -RequirePrivateKey).Count) { throw 'valid publisher fixture rejected' }
foreach ($field in $good.Keys) {
    $bad = $good.Clone()
    $bad[$field] = -not $bad[$field]
    if (@(Get-PublisherCertificateIssues -Facts ([pscustomobject]$bad) -RequirePrivateKey).Count -ne 1) { throw "missing independent rejection for $field" }
}
# An actual unsigned PE must fail before it could be advertised as compatible
# with protected Windows devices. No certificate or policy is installed here.
$project = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../../..'))
$previousRelease = Join-Path $project 'dist/r4/mir2-platform-windows.exe'
if (Test-Path -LiteralPath $previousRelease -PathType Leaf) {
    $result = Test-WindowsPublisherSignature -Path $previousRelease -ExpectedThumbprint ('0' * 40)
    if ($result.Ready -or $result.Status -ne 'NotSigned' -or $result.Issues.Count -lt 2) { throw 'unsigned r4 release was not rejected' }
    Write-Host 'actual unsigned r4 PE rejection passed'
} else {
    Write-Host 'actual unsigned PE check skipped: the optional local r4 artifact is unavailable'
}
Write-Host 'publisher certificate requirement fixtures passed'
