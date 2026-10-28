[CmdletBinding()]
param([switch]$CheckOnly)

$ErrorActionPreference = 'Stop'
& (Join-Path $PSScriptRoot 'start-native-wallet-client.ps1') `
    -GatewayWsUrl 'wss://165.154.65.136.sslip.io/playtest/ws' `
    -AuthHttpUrl 'https://auth-playtest.165.154.65.136.sslip.io' `
    -CheckOnly:$CheckOnly
