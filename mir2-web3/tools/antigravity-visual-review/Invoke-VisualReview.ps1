[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$Reference,

    [Parameter(Mandatory = $true)]
    [string]$Candidate,

    [string]$Context,
    [string]$Label = "Windows-native",
    [ValidateSet("vercel", "gemini", "antigravity")]
    [string]$Provider = "vercel",
    [string]$Model,
    [ValidateSet("low", "medium", "high")]
    [string]$Effort = "medium",
    [ValidateSet("standard", "flex", "priority")]
    [string]$ServiceTier = "standard",
    [ValidateRange(1, 3600000)]
    [int]$TimeoutMs = 180000,
    [ValidateRange(0, 10)]
    [int]$Retries = 3,
    [string]$Output,
    [string]$RunId,
    [switch]$DryRun
)

$ErrorActionPreference = "Stop"

if (-not $DryRun -and $Provider -eq "vercel" -and [string]::IsNullOrWhiteSpace($env:AI_GATEWAY_API_KEY) -and [string]::IsNullOrWhiteSpace($env:VERCEL_OIDC_TOKEN)) {
    $savedGatewayKey = [Environment]::GetEnvironmentVariable("AI_GATEWAY_API_KEY", "User")
    if ([string]::IsNullOrWhiteSpace($savedGatewayKey)) {
        throw "AI_GATEWAY_API_KEY is not configured. Run tools\antigravity-visual-review\Set-VercelGatewayKey.ps1 first."
    }
    $env:AI_GATEWAY_API_KEY = $savedGatewayKey
    $savedGatewayKey = $null
}

$scriptPath = Join-Path $PSScriptRoot "review.mjs"
$arguments = @(
    $scriptPath,
    "--reference", (Resolve-Path -LiteralPath $Reference).Path,
    "--candidate", (Resolve-Path -LiteralPath $Candidate).Path,
    "--label", $Label,
    "--provider", $Provider,
    "--effort", $Effort,
    "--service-tier", $ServiceTier,
    "--timeout-ms", $TimeoutMs,
    "--retries", $Retries
)

if ($Context) { $arguments += @("--context", (Resolve-Path -LiteralPath $Context).Path) }
if ($Model) { $arguments += @("--model", $Model) }
if ($Output) { $arguments += @("--output", $Output) }
if ($RunId) { $arguments += @("--run-id", $RunId) }
if ($DryRun) { $arguments += "--dry-run" }

& node @arguments
exit $LASTEXITCODE
