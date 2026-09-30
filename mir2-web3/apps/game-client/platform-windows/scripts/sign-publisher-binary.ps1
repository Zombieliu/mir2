[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Path,
    [Parameter(Mandatory = $true)][string]$PublisherThumbprint,
    [Parameter(Mandatory = $true)][string]$SignToolPath,
    [Parameter(Mandatory = $true)][uri]$TimestampUrl
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'publisher-signing.psm1') -Force
Invoke-WindowsPublisherSigning -Path $Path -Thumbprint $PublisherThumbprint -SignToolPath $SignToolPath -TimestampUrl $TimestampUrl | ConvertTo-Json -Depth 5
