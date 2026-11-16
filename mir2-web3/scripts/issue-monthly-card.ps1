[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][uri]$ServiceBaseUri,
    [Parameter(Mandatory = $true)][string]$AccountId,
    [Parameter(Mandatory = $true)][ValidatePattern('^[A-Za-z0-9_.:-]{12,96}$')][string]$OrderId,
    [switch]$AllowLocalHttp
)
$ErrorActionPreference = 'Stop'
if (-not $ServiceBaseUri.IsAbsoluteUri -or $ServiceBaseUri.UserInfo -or $ServiceBaseUri.Query -or $ServiceBaseUri.Fragment) {
    throw 'Use an absolute service URL without credentials, query or fragment.'
}
if ($ServiceBaseUri.Scheme -ne 'https' -and -not ($AllowLocalHttp -and $ServiceBaseUri.Scheme -eq 'http' -and $ServiceBaseUri.IsLoopback)) {
    throw 'HTTPS is required. AllowLocalHttp only permits an owned loopback QA gateway.'
}
$operatorToken = [Environment]::GetEnvironmentVariable('MIR2_GATEWAY_OPERATOR_TOKEN')
if ([string]::IsNullOrWhiteSpace($operatorToken) -or $operatorToken.Length -lt 32) {
    throw 'Set the private MIR2_GATEWAY_OPERATOR_TOKEN in this process environment.'
}
$endpoint = [uri]::new($ServiceBaseUri.AbsoluteUri.TrimEnd('/') + '/admin/monthly-cards/issue')
$requestBody = @{ accountId = $AccountId; requestId = $OrderId } | ConvertTo-Json -Compress
try {
    $receipt = Invoke-RestMethod -Uri $endpoint -Method Post -ContentType 'application/json' -TimeoutSec 30 `
        -Headers @{ Authorization = 'Bearer ' + $operatorToken } -Body $requestBody
} catch {
    throw 'Issuance was not confirmed. Retry the SAME account and OrderId; do not create another order for a timeout.'
} finally {
    $operatorToken = $null
}
if ($receipt.code -notmatch '^MC1-[A-Za-z0-9_-]{43}$' -or $receipt.durationDays -ne 30) {
    throw 'Unexpected receipt; issuance was not confirmed. Retry the same OrderId.'
}
# The voucher is intentionally returned only to the trusted operator. Do not
# run this in a shared transcript or send the operator token to a player.
[pscustomobject]@{
    AccountId = $AccountId
    OrderId = $OrderId
    ActivationCode = $receipt.code
    DurationDays = $receipt.durationDays
    RedeemBeforeMs = $receipt.redeemBeforeMs
    Replayed = $receipt.replayed
}
