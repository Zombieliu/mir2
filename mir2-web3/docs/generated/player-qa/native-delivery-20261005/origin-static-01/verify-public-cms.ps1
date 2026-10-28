#Requires -Version 7.2
$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
Add-Type -AssemblyName System.Security.Cryptography.Pkcs
$pin='6C70C777B27D50949370D494B4B25798200FBDD5C171BAEEAA94D7E289900F3E'
$root=$PSScriptRoot
$base=Split-Path -Parent $root
$plan=Join-Path $base 'origin-download-compatibility-plan-01'
$delivery=Join-Path $base 'r17-download-acceleration-02'
$candidate=Join-Path 'C:/Users/Administrator/.codex/worktrees/r17/mir2-player-journey/mir2-web3/dist' 'r17'
$expected=Get-Content -LiteralPath (Join-Path $root 'EXPECTED-02.json') -Raw | ConvertFrom-Json
function Sha([byte[]]$Bytes){ [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($Bytes)) }
function SafeBytes([string]$Path){
    $cursor=[IO.Path]::GetFullPath($Path)
    while($cursor){
        $entry=Get-Item -LiteralPath $cursor -Force
        if(($entry.Attributes-band[IO.FileAttributes]::ReparsePoint)-ne0){throw 'Linked CMS input rejected'}
        $parent=Split-Path -Parent $cursor
        if(!$parent-or$parent-eq$cursor){break}
        $cursor=$parent
    }
    $entry=Get-Item -LiteralPath $Path -Force
    if($entry.PSIsContainer-or$entry.Length-gt33554432-or$entry.LinkType-eq'HardLink'){throw 'Ordinary bounded CMS file required'}
    foreach($stream in @(Get-Item -LiteralPath $Path -Stream *)){
        if($stream.Stream-cne':$DATA'){throw 'Named stream rejected'}
    }
    return ,[IO.File]::ReadAllBytes($Path)
}
function Match([byte[]]$Value,$Expected){
    if($Value.Length-ne$Expected.size-or(Sha $Value).ToLowerInvariant()-cne$Expected.sha256.ToLowerInvariant()){
        throw 'Exact source hash binding failed'
    }
}
function Verify([byte[]]$Value,[byte[]]$Signature,[long]$At){
    $detached=[Security.Cryptography.Pkcs.SignedCms]::new()
    $detached.Decode($Signature)
    if($detached.ContentInfo.Content.Length-ne0){throw 'Detached CMS required'}
    $cms=[Security.Cryptography.Pkcs.SignedCms]::new([Security.Cryptography.Pkcs.ContentInfo]::new($Value),$true)
    $cms.Decode($Signature)
    if($cms.SignerInfos.Count-ne1-or$cms.SignerInfos[0].DigestAlgorithm.Value-cne'2.16.840.1.101.3.4.2.1'){
        throw 'Exactly one SHA256 CMS signer required'
    }
    $cms.CheckSignature($true)
    $certificate=$cms.SignerInfos[0].Certificate
    if(!$certificate){throw 'CMS signer certificate missing'}
    $when=[DateTimeOffset]::FromUnixTimeSeconds($At).UtcDateTime
    if($certificate.NotBefore.ToUniversalTime()-gt$when-or$certificate.NotAfter.ToUniversalTime()-lt$when){
        throw 'Signer not valid at metadata time'
    }
    $eku=$certificate.Extensions|Where-Object {$_.Oid.Value-eq'2.5.29.37'}|Select-Object -First 1
    $usage=$certificate.Extensions|Where-Object {$_.Oid.Value-eq'2.5.29.15'}|Select-Object -First 1
    if(!$eku-or@($eku.EnhancedKeyUsages|ForEach-Object {$_.Value})-notcontains'1.3.6.1.5.5.7.3.3'-or
       ($usage-and($usage.KeyUsages-band[Security.Cryptography.X509Certificates.X509KeyUsageFlags]::DigitalSignature)-eq0)){
        throw 'Code signing EKU/key usage required'
    }
    $rsa=[Security.Cryptography.X509Certificates.RSACertificateExtensions]::GetRSAPublicKey($certificate)
    if(!$rsa-or(Sha $rsa.ExportRSAPublicKey())-cne$pin){throw 'Pinned public key mismatch'}
    return [ordered]@{payloadSha256=(Sha $Value).ToLowerInvariant();signatureSha256=(Sha $Signature).ToLowerInvariant()
        verifiedAtUnix=$At;pinnedPublicKeySha256=$pin.ToLowerInvariant();privateKeyUsed=$false}
}
$feedBytes=SafeBytes (Join-Path $plan 'public-metadata/00-165.154.65.136.sslip.io-latest.json')
$feedSignature=SafeBytes (Join-Path $plan 'public-metadata/01-165.154.65.136.sslip.io-latest.p7s')
Match $feedBytes ($expected.sourceFiles|Where-Object path -CEQ 'feed-current/latest.json')
Match $feedSignature ($expected.sourceFiles|Where-Object path -CEQ 'feed-current/latest.p7s')
$feed=[Text.Encoding]::UTF8.GetString($feedBytes)|ConvertFrom-Json
$now=[DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
if($feed.sequence-ne12-or$feed.createdUnix-gt$now+300-or$feed.expiresUnix-le$now-or
   $feed.game.directory-cne$expected.candidateDirectory){throw 'Feed is not current exact R17'}
$feedProof=Verify $feedBytes $feedSignature $feed.createdUnix
$manifestBytes=SafeBytes (Join-Path $candidate 'PACKAGE-MANIFEST.json')
$versionBytes=SafeBytes (Join-Path $candidate 'VERSION.json')
$statementBytes=SafeBytes (Join-Path $candidate 'RELEASE-STATEMENT.json')
$statementSignature=SafeBytes (Join-Path $candidate 'RELEASE-STATEMENT.p7s')
foreach($entry in $feed.game.metadata){Match (SafeBytes (Join-Path $candidate $entry.path)) $entry}
$version=[Text.Encoding]::UTF8.GetString($versionBytes)|ConvertFrom-Json
$statement=[Text.Encoding]::UTF8.GetString($statementBytes)|ConvertFrom-Json
if($version.gitRevision-cne'6032ef8b3e27dd97bad0b20c8676ef9f185db64b'-or
   $version.worktreeDirty-ne$false-or$statement.packageManifestSha256-cne(Sha $manifestBytes)-or
   $statement.versionSha256-cne(Sha $versionBytes)-or$statement.candidate-cne$feed.game.identity){
    throw 'Authenticated Candidate metadata differs'
}
$statementProof=Verify $statementBytes $statementSignature $now
$descriptorBytes=SafeBytes (Join-Path $delivery 'DELIVERY.json')
$descriptorSignature=SafeBytes (Join-Path $delivery 'DELIVERY.p7s')
Match $descriptorBytes ($expected.objects|Where-Object path -CEQ ($expected.candidateDirectory+'/DELIVERY.json'))
Match $descriptorSignature ($expected.objects|Where-Object path -CEQ ($expected.candidateDirectory+'/DELIVERY.p7s'))
$descriptor=[Text.Encoding]::UTF8.GetString($descriptorBytes)|ConvertFrom-Json
if($descriptor.schema-cne'mir2.windows.delivery.v1'-or$descriptor.candidate-cne$version.candidate-or
   $descriptor.packageManifestSha256-cne(Sha $manifestBytes)-or$descriptor.versionSha256-cne(Sha $versionBytes)-or
   $descriptor.builtUnix-gt$now+300-or$descriptor.bundles.Count-ne11-or$descriptor.patches.Count-ne1){
    throw 'Signed DELIVERY does not bind exact R17'
}
$deliveryProof=Verify $descriptorBytes $descriptorSignature $descriptor.builtUnix
$receipt=[ordered]@{schema='mir2.origin.public-cms-verification.v1';passed=$true;cmsVerified=$true
    createdUnix=$now;candidate=$version.candidate;gameSourceRevision=$version.gitRevision
    manifestSha256=(Sha $manifestBytes).ToLowerInvariant();versionSha256=(Sha $versionBytes).ToLowerInvariant()
    feed=$feedProof;statement=$statementProof;delivery=$deliveryProof;privateKeyUsed=$false;published=$false}
$data=[Text.UTF8Encoding]::new($false).GetBytes(($receipt|ConvertTo-Json -Depth 8))
$path=Join-Path $root 'CMS-VERIFICATION-01.json'
$out=[IO.File]::Open($path,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
try{$out.Write($data);$out.Flush($true)}finally{$out.Dispose()}
$receipt|ConvertTo-Json -Depth 8
