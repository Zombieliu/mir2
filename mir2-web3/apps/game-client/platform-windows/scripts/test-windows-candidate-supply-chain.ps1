[CmdletBinding()]
param(
    [string]$WorkflowPath = '',
    [string]$PublisherVerifierPath = ''
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$scriptDir = Split-Path -Parent $PSCommandPath
$repoRoot = [IO.Path]::GetFullPath((Join-Path $scriptDir '..\..\..\..\..')).TrimEnd('\', '/')
if ([string]::IsNullOrWhiteSpace($WorkflowPath)) {
    $WorkflowPath = Join-Path $repoRoot '.github\workflows\cross-platform-client.yml'
}
if ([string]::IsNullOrWhiteSpace($PublisherVerifierPath)) {
    $PublisherVerifierPath = Join-Path $scriptDir 'verify-windows-publisher-contract.ps1'
}

$workflow = [IO.File]::ReadAllText((Resolve-Path -LiteralPath $WorkflowPath))
$publisherVerifier = [IO.File]::ReadAllText((Resolve-Path -LiteralPath $PublisherVerifierPath))

$uses = [regex]::Matches($workflow, '(?m)^\s*-\s+uses:\s+([^\s#]+)')
if ($uses.Count -eq 0) { throw 'workflow contains no external action references' }
foreach ($match in $uses) {
    $reference = $match.Groups[1].Value
    if ($reference -cnotmatch '^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+(?:/[A-Za-z0-9_.-]+)?@[0-9a-f]{40}$') {
        throw "external action is not pinned to an immutable lowercase commit SHA: $reference"
    }
}

$requiredPins = @(
    'actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1',
    'actions/setup-node@820762786026740c76f36085b0efc47a31fe5020',
    'actions/setup-java@b6effb05e454b25005698d916606bdc6ffcbf961',
    'actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a',
    'actions/download-artifact@3e5f45b2cfb9172054b4087a40e8e0b5a5461e7c',
    'actions/attest@1e69f48acb82d1966a394da916b4c1698aa569d6'
)
foreach ($pin in $requiredPins) {
    if (-not $workflow.Contains($pin)) { throw "required immutable action pin missing: $pin" }
}

$requiredWorkflowLiterals = @(
    'windows-candidate-publish:',
    "if: `${{ github.ref == 'refs/heads/main' && github.event_name != 'pull_request' }}",
    'environment: windows-candidate-publisher',
    'id-token: write',
    'attestations: write',
    'artifact-metadata: write',
    'MIR2_WINDOWS_CANDIDATE_PUBLISHER_UID: ${{ secrets.MIR2_WINDOWS_CANDIDATE_PUBLISHER_UID }}',
    'GITHUB_ACTOR_ID: ${{ github.actor_id }}',
    'mir2-windows-x86_64-unpublished-build',
    'Generate GitHub build provenance attestation',
    'gh attestation verify',
    'GitHub provenance verification failed closed',
    'Upload verified Windows Candidate artifact'
)
foreach ($literal in $requiredWorkflowLiterals) {
    if (-not $workflow.Contains($literal)) { throw "required supply-chain gate missing: $literal" }
}
if ($workflow.Contains('MIR2_WINDOWS_CANDIDATE_PUBLISHER_UID: ${{ vars.')) {
    throw 'publisher UID must not fall back to an unprotected repository variable'
}

$attestIndex = $workflow.IndexOf('Generate GitHub build provenance attestation', [StringComparison]::Ordinal)
$verifyIndex = $workflow.IndexOf('Verify GitHub build provenance attestation', [StringComparison]::Ordinal)
$publishIndex = $workflow.IndexOf('Upload verified Windows Candidate artifact', [StringComparison]::Ordinal)
if ($attestIndex -lt 0 -or $verifyIndex -le $attestIndex -or $publishIndex -le $verifyIndex) {
    throw 'publication ordering must be package -> attest -> verify -> upload'
}

$requiredPublisherLiterals = @(
    '[string]$ExpectedPublisherUid = $env:MIR2_WINDOWS_CANDIDATE_PUBLISHER_UID',
    '[string]$ActualPublisherUid = $env:GITHUB_ACTOR_ID',
    "if ([string]::IsNullOrWhiteSpace(`$Value))",
    "if (`$Expected -cne `$Actual)",
    'protected publisher UID does not match the GitHub workflow actor ID'
)
foreach ($literal in $requiredPublisherLiterals) {
    if (-not $publisherVerifier.Contains($literal)) { throw "publisher verifier fail-closed contract missing: $literal" }
}
if ($publisherVerifier -match '(?i)(default|fallback).{0,40}(publisher|uid)\s*=') {
    throw 'publisher verifier appears to contain a default/fallback publisher identity'
}

Write-Host ("Windows Candidate supply-chain static test passed; immutableActions={0}" -f $uses.Count)
