[CmdletBinding()]
param(
    [switch]$SessionOnly,
    [switch]$SkipTest
)

$ErrorActionPreference = "Stop"
$secureKey = Read-Host "Paste your Vercel AI Gateway API key (input is hidden)" -AsSecureString
$keyPointer = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($secureKey)
$plainKey = $null

try {
    $plainKey = [Runtime.InteropServices.Marshal]::PtrToStringBSTR($keyPointer)
    if ([string]::IsNullOrWhiteSpace($plainKey) -or $plainKey.Trim().Length -lt 20) {
        throw "The value does not look like a complete Vercel AI Gateway key. Nothing was saved."
    }
    $plainKey = $plainKey.Trim()

    if (-not $SkipTest) {
        Write-Host "Validating the key with the Vercel AI Gateway credits endpoint (no model tokens used)..."
        $headers = @{
            Authorization = "Bearer $plainKey"
            Accept = "application/json"
        }
        $creditResponse = Invoke-RestMethod `
            -Method Get `
            -Uri "https://ai-gateway.vercel.sh/v1/credits" `
            -Headers $headers
        if ($null -eq $creditResponse.balance -or $null -eq $creditResponse.total_used) {
            throw "The Gateway accepted the request, but its credits response was incomplete. Nothing was saved."
        }
        Write-Host "Authenticated Vercel AI Gateway key check passed."
    }

    $env:AI_GATEWAY_API_KEY = $plainKey
    if (-not $SessionOnly) {
        [Environment]::SetEnvironmentVariable("AI_GATEWAY_API_KEY", $plainKey, "User")
        Write-Host "Vercel AI Gateway key saved to the current Windows user's environment."
    }
    else {
        Write-Host "Vercel AI Gateway key set for this PowerShell process only."
    }
}
finally {
    if ($keyPointer -ne [IntPtr]::Zero) {
        [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($keyPointer)
    }
    $plainKey = $null
    Remove-Variable creditResponse -ErrorAction SilentlyContinue
    Remove-Variable headers -ErrorAction SilentlyContinue
    Remove-Variable plainKey -ErrorAction SilentlyContinue
    Remove-Variable secureKey -ErrorAction SilentlyContinue
}
