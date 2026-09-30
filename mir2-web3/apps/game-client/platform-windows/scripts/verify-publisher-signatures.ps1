[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string[]]$Path,
    [Parameter(Mandatory = $true)][string]$PublisherThumbprint
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'publisher-signing.psm1') -Force
$results = @($Path | ForEach-Object { Test-WindowsPublisherSignature -Path $_ -ExpectedThumbprint $PublisherThumbprint })
$results | ConvertTo-Json -Depth 6
if (@($results | Where-Object { -not $_.Ready }).Count) { exit 1 }
