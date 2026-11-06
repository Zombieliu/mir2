[CmdletBinding()]
param(
    [switch]$SessionOnly,
    [switch]$SkipTest
)

$ErrorActionPreference = "Stop"
$secureKey = Read-Host "Paste your Gemini API key (input is hidden)" -AsSecureString
$keyPointer = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($secureKey)
$plainKey = $null

try {
    $plainKey = [Runtime.InteropServices.Marshal]::PtrToStringBSTR($keyPointer)
    if ([string]::IsNullOrWhiteSpace($plainKey) -or $plainKey.Trim().Length -lt 20) {
        throw "The value does not look like a complete Gemini API key. Nothing was saved."
    }
    $plainKey = $plainKey.Trim()
    $env:GEMINI_API_KEY = $plainKey

    if (-not $SessionOnly) {
        [Environment]::SetEnvironmentVariable("GEMINI_API_KEY", $plainKey, "User")
        Write-Host "Gemini API key saved to the current Windows user's environment."
    }
    else {
        Write-Host "Gemini API key set for this PowerShell process only."
    }

    if (-not $SkipTest) {
        $geminiCommand = Get-Command gemini.cmd -ErrorAction SilentlyContinue
        if (-not $geminiCommand) { $geminiCommand = Get-Command gemini -ErrorAction Stop }
        Write-Host "Running a minimal authenticated Gemini CLI test..."
        & $geminiCommand.Source --prompt "Return exactly READY and nothing else." --output-format json --approval-mode plan --skip-trust
        if ($LASTEXITCODE -ne 0) { throw "Gemini CLI authentication test failed with exit code $LASTEXITCODE." }
    }
}
finally {
    if ($keyPointer -ne [IntPtr]::Zero) {
        [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($keyPointer)
    }
    $plainKey = $null
    Remove-Variable plainKey -ErrorAction SilentlyContinue
    Remove-Variable secureKey -ErrorAction SilentlyContinue
}
