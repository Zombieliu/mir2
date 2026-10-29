# Test-only fixtures. Ephemeral RSA keys stay in memory and are never exported.
# Requires PowerShell 7; no certificates or trust roots are installed.
[CmdletBinding()]
param([Parameter(Mandatory=$true)][string]$OutputRoot)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
if($PSVersionTable.PSVersion.Major -lt 7){throw 'PowerShell 7 or newer is required for these test fixtures.'}
$OutputRoot = [IO.Path]::GetFullPath($OutputRoot)
if(Test-Path -LiteralPath $OutputRoot){
    $directory = Get-Item -LiteralPath $OutputRoot -Force
    if(-not $directory.PSIsContainer -or ($directory.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0){throw 'OutputRoot must be a regular directory.'}
    if(@(Get-ChildItem -LiteralPath $OutputRoot -Force).Count -ne 0){throw 'OutputRoot must be empty; existing fixtures are never overwritten.'}
}else{[void][IO.Directory]::CreateDirectory($OutputRoot)}
Add-Type -AssemblyName System.Security.Cryptography.Pkcs
$rsa = [Security.Cryptography.RSA]::Create(2048)
$otherRsa = [Security.Cryptography.RSA]::Create(2048)
$certificates = [Collections.Generic.List[Security.Cryptography.X509Certificates.X509Certificate2]]::new()
function New-TestCertificate([string]$Name, [Security.Cryptography.RSA]$Key, [bool]$WithEku, [bool]$DigitalSignature, [DateTimeOffset]$Start, [DateTimeOffset]$End) {
    $request = [Security.Cryptography.X509Certificates.CertificateRequest]::new("CN=$Name", $Key, [Security.Cryptography.HashAlgorithmName]::SHA256, [Security.Cryptography.RSASignaturePadding]::Pkcs1)
    if($WithEku) {
        $oids = [Security.Cryptography.OidCollection]::new()
        [void]$oids.Add([Security.Cryptography.Oid]::new('1.3.6.1.5.5.7.3.3'))
        $request.CertificateExtensions.Add([Security.Cryptography.X509Certificates.X509EnhancedKeyUsageExtension]::new($oids, $true))
    }
    $usage = if($DigitalSignature){[Security.Cryptography.X509Certificates.X509KeyUsageFlags]::DigitalSignature}else{[Security.Cryptography.X509Certificates.X509KeyUsageFlags]::KeyEncipherment}
    $request.CertificateExtensions.Add([Security.Cryptography.X509Certificates.X509KeyUsageExtension]::new($usage, $true))
    $certificate = $request.CreateSelfSigned($Start, $End)
    $certificates.Add($certificate)
    return $certificate
}
$content = [Text.Encoding]::UTF8.GetBytes('{"schema":"test","release":7}')
$now = [DateTimeOffset]::UtcNow
$valid = New-TestCertificate 'Original test signing certificate' $rsa $true $true $now.AddMinutes(-5) $now.AddHours(1)
$renewed = New-TestCertificate 'Renewed same-key test certificate' $rsa $true $true $now.AddMinutes(-5) $now.AddHours(2)
$expired = New-TestCertificate 'Historic same-key test certificate' $rsa $true $true $now.AddDays(-3) $now.AddDays(-2)
$noEku = New-TestCertificate 'No EKU test certificate' $rsa $false $true $now.AddMinutes(-5) $now.AddHours(1)
$wrongUsage = New-TestCertificate 'Key encipherment only test certificate' $rsa $true $false $now.AddMinutes(-5) $now.AddHours(1)
$wrongKey = New-TestCertificate 'Other-key test certificate' $otherRsa $true $true $now.AddMinutes(-5) $now.AddHours(1)
function Write-Signature([string]$Name, [Security.Cryptography.X509Certificates.X509Certificate2[]]$Signers, [bool]$Detached = $true, [string]$Digest = '2.16.840.1.101.3.4.2.1') {
    $cms = [Security.Cryptography.Pkcs.SignedCms]::new([Security.Cryptography.Pkcs.ContentInfo]::new($content), $Detached)
    foreach($certificate in $Signers) {
        $signer = [Security.Cryptography.Pkcs.CmsSigner]::new([Security.Cryptography.Pkcs.SubjectIdentifierType]::IssuerAndSerialNumber,$certificate)
        $signer.IncludeOption = [Security.Cryptography.X509Certificates.X509IncludeOption]::EndCertOnly
        $signer.DigestAlgorithm = [Security.Cryptography.Oid]::new($Digest)
        $cms.ComputeSignature($signer,$true)
    }
    [IO.File]::WriteAllBytes((Join-Path $OutputRoot "$Name.p7s"),$cms.Encode())
}
try {
    [IO.File]::WriteAllBytes((Join-Path $OutputRoot 'content.bin'),$content)
    [IO.File]::WriteAllText((Join-Path $OutputRoot 'key-pin.txt'),[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($valid.GetPublicKey())))
    [IO.File]::WriteAllText((Join-Path $OutputRoot 'valid-time.txt'),$now.ToUnixTimeSeconds().ToString())
    Write-Signature 'valid' @($valid)
    Write-Signature 'renewed' @($renewed)
    Write-Signature 'expired' @($expired)
    Write-Signature 'multiple' @($valid,$renewed)
    Write-Signature 'attached' @($valid) $false
    Write-Signature 'sha1' @($valid) $true '1.3.14.3.2.26'
    Write-Signature 'no-eku' @($noEku)
    Write-Signature 'wrong-usage' @($wrongUsage)
    Write-Signature 'wrong-key' @($wrongKey)
    Write-Output 'Nine public-only CMS fixtures generated; private keys were not exported.'
} finally {
    foreach($certificate in $certificates) { $certificate.Dispose() }
    $rsa.Dispose()
    $otherRsa.Dispose()
}
