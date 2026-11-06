#Requires -Version 7.2
<# Sign only an explicitly reviewed create-only draft, then run full independent
   verification. Never exports a private key, executes the game or publishes. #>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$OutputRoot,
    [Parameter(Mandatory)][ValidatePattern('^[0-9A-Fa-f]{40}$')][string]$CertificateThumbprint,
    [Parameter(Mandatory)][ValidatePattern('^[0-9A-Fa-f]{64}$')][string]$ExpectedManifestSha256,
    [Parameter(Mandatory)][string]$PackVerifierPath,
    [Parameter(Mandatory)][ValidatePattern('^[0-9A-Fa-f]{64}$')][string]$PackVerifierSha256,
    [string]$PythonPath
)
$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
$OutputRoot=[IO.Path]::GetFullPath($OutputRoot)
if(!$PythonPath){$PythonPath=(Get-Command python -ErrorAction Stop).Source}
$PythonPath=[IO.Path]::GetFullPath($PythonPath)
. (Join-Path $PSScriptRoot 'verify-packed-generation.ps1') -OutputRoot $OutputRoot -ExpectedManifestSha256 $ExpectedManifestSha256 -PackVerifierPath $PackVerifierPath -PackVerifierSha256 $PackVerifierSha256 -PythonPath $PythonPath
$ExpectedManifestSha256=$ExpectedManifestSha256.ToUpperInvariant()
Packed-NoLinks $OutputRoot
$prepared=Packed-Python $PythonPath @('verify-draft','--output-root',$OutputRoot)
if($prepared.manifestSha256-cne$ExpectedManifestSha256-or$prepared.sourceClean-ne$true-or$prepared.cmsVerified-ne$false-or$prepared.unsigned-ne$true){throw 'Reviewed unsigned manifest/source snapshot required'}
$generation=Join-Path $OutputRoot $prepared.generationDirectory
foreach($path in @((Join-Path $generation 'GENERATION.p7s'),(Join-Path $OutputRoot 'latest.json'),(Join-Path $OutputRoot 'latest.p7s'),(Join-Path $OutputRoot 'SIGN-PACKED-GENERATION.json'))){
    if(Test-Path -LiteralPath $path){throw 'Refusing to overwrite a signature, feed or signing receipt'}
}
$pack=Packed-FullCheck $OutputRoot $prepared.generationDirectory $prepared.inputBindings[2].sha256 $prepared.stats $PackVerifierPath $PackVerifierSha256 (Join-Path $OutputRoot 'PRE-SIGN-PACK-CHECK.json')
$certificate=Get-Item -LiteralPath ('Cert:/CurrentUser/My/'+$CertificateThumbprint.ToUpperInvariant())
if(!$certificate.HasPrivateKey){throw 'Explicit current-user certificate must have its private signing identity'}
$now=[DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
Packed-Certificate $certificate $now
Packed-Certificate $certificate $prepared.createdUnix
function Sign-PackedBytes([byte[]]$Bytes){
    $cms=[Security.Cryptography.Pkcs.SignedCms]::new([Security.Cryptography.Pkcs.ContentInfo]::new($Bytes),$true)
    $signer=[Security.Cryptography.Pkcs.CmsSigner]::new($certificate)
    $signer.IncludeOption=[Security.Cryptography.X509Certificates.X509IncludeOption]::EndCertOnly
    $signer.DigestAlgorithm=[Security.Cryptography.Oid]::new('2.16.840.1.101.3.4.2.1')
    $cms.ComputeSignature($signer,$false)
    $signature=$cms.Encode();Packed-Cms $Bytes $signature $prepared.createdUnix
    return ,$signature
}
$manifest=Packed-Bytes (Join-Path $generation 'GENERATION.json') 4194304
if((Packed-Sha $manifest)-cne$ExpectedManifestSha256){throw 'Manifest changed before signing'}
$recheck=Packed-Python $PythonPath @('verify-draft','--output-root',$OutputRoot)
if(($recheck|ConvertTo-Json -Depth 50 -Compress)-cne($prepared|ConvertTo-Json -Depth 50 -Compress)){throw 'Source/artifact snapshot changed before signing'}
Packed-Save (Join-Path $generation 'GENERATION.p7s') (Sign-PackedBytes $manifest)
$feedObject=Packed-Python $PythonPath @('finalize-feed','--output-root',$OutputRoot,'--expected-manifest-sha',$ExpectedManifestSha256)
$feed=Packed-Bytes (Join-Path $OutputRoot 'latest.json') 32768
Packed-Save (Join-Path $OutputRoot 'latest.p7s') (Sign-PackedBytes $feed)
& (Join-Path $PSScriptRoot 'verify-packed-generation.ps1') -OutputRoot $OutputRoot -ExpectedManifestSha256 $ExpectedManifestSha256 -PackVerifierPath $PackVerifierPath -PackVerifierSha256 $PackVerifierSha256 -PythonPath $PythonPath | Out-Null
if($LASTEXITCODE-ne0){throw 'Final independent generation verification failed'}
$verification=Packed-Bytes (Join-Path $OutputRoot 'VERIFY-PACKED-GENERATION.json') 1048576
$proof=[ordered]@{schema='mir2.windows.packed-generation-signing.v1';passed=$true;cmsVerified=$true;fullResourceVerified=$true;sourceClean=$true;published=$false
    manifestSha256=$ExpectedManifestSha256;catalogSha256=$prepared.inputBindings[2].sha256;certificateThumbprint=$CertificateThumbprint.ToUpperInvariant();signingKeySha256=$script:PackedSigningKey
    gameSourceRevision=$prepared.gameSource.revision;engineSourceRevision=$prepared.engineSource.revision;resourceSourceRevision=$prepared.resourceProvenance.resourceSourceRevision
    resourceManifestSha256=$prepared.resourceProvenance.manifestSha256;resourceStrictReportSha256=$prepared.resourceProvenance.reportSha256
    verificationSha256=Packed-Sha $verification;packVerifierSha256=$PackVerifierSha256.ToUpperInvariant();signedUnix=$now
    resourceCount=$prepared.stats.resourceCount;uniquePayloads=$prepared.stats.uniquePayloads;logicalBytes=$prepared.stats.logicalBytes;uniquePayloadBytes=$prepared.stats.uniquePayloadBytes
    packCount=$prepared.stats.packCount;packBytes=$prepared.stats.packBytes
    preparerSha256=Packed-Sha (Packed-Bytes (Join-Path $PSScriptRoot 'prepare-packed-generation.py') 1048576)
    signerSha256=Packed-Sha (Packed-Bytes $PSCommandPath 1048576)
    verifierSha256=Packed-Sha (Packed-Bytes (Join-Path $PSScriptRoot 'verify-packed-generation.ps1') 1048576)}
Packed-Save (Join-Path $OutputRoot 'SIGN-PACKED-GENERATION.json') (Packed-Json $proof)
$proof|ConvertTo-Json -Depth 12
