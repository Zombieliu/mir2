#Requires -Version 7.2
<# Sign optional delivery metadata only after Candidate CMS and byte reconstruction.
   This never edits the game, exports a key, installs, deploys, or changes a feed. #>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$CandidateRoot,
    [Parameter(Mandatory)][string]$DeliveryRoot,
    [Parameter(Mandatory)][string]$GameProjectRoot,
    [Parameter(Mandatory)][string]$VerificationToolProjectRoot,
    [Parameter(Mandatory)][ValidatePattern('^[0-9a-f]{40}$')][string]$SourceRevision,
    [Parameter(Mandatory)][ValidatePattern('^[0-9A-Fa-f]{40}$')][string]$SignerThumbprint,
    [string[]]$BaseExe=@(),
    [string]$PythonPath
)
$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
$project=[IO.Path]::GetFullPath($GameProjectRoot)
$toolProject=[IO.Path]::GetFullPath($VerificationToolProjectRoot)
$gameRevision='87fd85a0d5a819e5b38c90d192421f9239f65678'
$toolRevision='4c60c323aed11829abae6ee5ce6e13c675a3c1c3'
$verificationTool=Join-Path $toolProject 'apps/game-client/windows-updater/scripts/verify-native-delivery.py'
$verificationToolSha='A2457E3897B07EA3929879D7C1460DAD3A0032D43F14B609A3DDD649BDB3514F'
$gameCandidate='WN-CANDIDATE-20261010-invited-24'
$pin='6C70C777B27D50949370D494B4B25798200FBDD5C171BAEEAA94D7E289900F3E'
function Sha([byte[]]$Bytes) { [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($Bytes)) }
function NoLinks([string]$Path) {
    $cursor=[IO.Path]::GetFullPath($Path)
    if($cursor.StartsWith('\\')){throw 'UNC inputs are unsupported'}
    while($cursor){
        if(Test-Path -LiteralPath $cursor){
            $item=Get-Item -LiteralPath $cursor -Force
            if(($item.Attributes-band[IO.FileAttributes]::ReparsePoint)-ne0){throw 'Linked signing input rejected'}
        }
        $parent=Split-Path -Parent $cursor
        if(!$parent -or $parent-eq$cursor){break};$cursor=$parent
    }
}
function Bytes([string]$Path) {
    NoLinks $Path
    $item=Get-Item -LiteralPath $Path -Force
    if($item.PSIsContainer -or $item.Length-gt33554432 -or $item.LinkType-eq'HardLink'){throw 'Bounded ordinary signing input required'}
    foreach($stream in @(Get-Item -LiteralPath $Path -Stream *)){
        if($stream.Stream-cne':$DATA'){throw 'Named data stream rejected'}
    }
    return ,[IO.File]::ReadAllBytes($Path)
}
function Save([string]$Path,[byte[]]$Value) {
    NoLinks $Path
    $stream=[IO.File]::Open($Path,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
    try{$stream.Write($Value);$stream.Flush($true)}finally{$stream.Dispose()}
}
function CheckCertificate($Certificate,[long]$At) {
    if(!$Certificate){throw 'Missing code signing certificate'}
    $when=[DateTimeOffset]::FromUnixTimeSeconds($At).UtcDateTime
    if($Certificate.NotBefore.ToUniversalTime()-gt$when -or $Certificate.NotAfter.ToUniversalTime()-lt$when){throw 'Certificate time mismatch'}
    $eku=$Certificate.Extensions|Where-Object {$_.Oid.Value-eq'2.5.29.37'}|Select-Object -First 1
    $usage=$Certificate.Extensions|Where-Object {$_.Oid.Value-eq'2.5.29.15'}|Select-Object -First 1
    if(!$eku -or @($eku.EnhancedKeyUsages|ForEach-Object {$_.Value})-notcontains'1.3.6.1.5.5.7.3.3' -or
       ($usage -and ($usage.KeyUsages-band[Security.Cryptography.X509Certificates.X509KeyUsageFlags]::DigitalSignature)-eq0)){
        throw 'Code signing EKU/key usage required'
    }
    $rsa=[Security.Cryptography.X509Certificates.RSACertificateExtensions]::GetRSAPublicKey($Certificate)
    if(!$rsa -or (Sha $rsa.ExportRSAPublicKey())-cne$pin){throw 'Signing key pin mismatch'}
}
function CheckCms([byte[]]$Value,[byte[]]$Signature,[long]$At) {
    $envelope=[Security.Cryptography.Pkcs.SignedCms]::new();$envelope.Decode($Signature)
    if($envelope.ContentInfo.Content.Length-ne0){throw 'Detached CMS required'}
    $cms=[Security.Cryptography.Pkcs.SignedCms]::new([Security.Cryptography.Pkcs.ContentInfo]::new($Value),$true)
    $cms.Decode($Signature)
    if($cms.SignerInfos.Count-ne1 -or $cms.SignerInfos[0].DigestAlgorithm.Value-cne'2.16.840.1.101.3.4.2.1'){throw 'Exactly one SHA256 CMS signer required'}
    $cms.CheckSignature($true);CheckCertificate $cms.SignerInfos[0].Certificate $At
}
function AssertSource {
    NoLinks $project;NoLinks $toolProject
    if($SourceRevision-cne$gameRevision){throw 'Exact frozen R24 game source required'}
    if((& git -C $project rev-parse HEAD).Trim()-cne$gameRevision -or $LASTEXITCODE-ne0 -or
       @(& git -C $project status --porcelain=v1 --untracked-files=all).Count-ne0 -or $LASTEXITCODE-ne0){throw 'Exact clean frozen R24 game source required'}
    if((& git -C $toolProject rev-parse HEAD).Trim()-cne$toolRevision -or $LASTEXITCODE-ne0 -or
       @(& git -C $toolProject status --porcelain=v1 --untracked-files=all).Count-ne0 -or $LASTEXITCODE-ne0 -or
       (Sha (Bytes $verificationTool))-cne$verificationToolSha){throw 'Exact clean pinned delivery verification tooling required'}
}
function AssertCandidateIdentity($Version,$Statement) {
    if($Version.gitRevision-cne$gameRevision -or $Statement.gitRevision-cne$gameRevision -or
       $Version.candidate-cne$gameCandidate -or $Statement.candidate-cne$gameCandidate -or
       $Statement.worktreeDirty-ne$false){throw 'Exact clean signed R23 Candidate identity required'}
}
AssertSource
$CandidateRoot=[IO.Path]::GetFullPath($CandidateRoot);NoLinks $CandidateRoot
$DeliveryRoot=[IO.Path]::GetFullPath($DeliveryRoot);NoLinks $DeliveryRoot
$signaturePath=Join-Path $DeliveryRoot 'DELIVERY.p7s'
$receiptPath=Join-Path $DeliveryRoot 'SIGN-DELIVERY.json'
if((Test-Path -LiteralPath $signaturePath)-or(Test-Path -LiteralPath $receiptPath)){throw 'Refusing to overwrite delivery signature/proof'}
if(!$PythonPath){$PythonPath=(Get-Command python -ErrorAction Stop).Source};NoLinks $PythonPath
Add-Type -AssemblyName System.Security.Cryptography.Pkcs
$now=[DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
$versionBytes=Bytes (Join-Path $CandidateRoot 'VERSION.json')
$manifestBytes=Bytes (Join-Path $CandidateRoot 'PACKAGE-MANIFEST.json')
$statementBytes=Bytes (Join-Path $CandidateRoot 'RELEASE-STATEMENT.json')
CheckCms $statementBytes (Bytes (Join-Path $CandidateRoot 'RELEASE-STATEMENT.p7s')) $now
$version=[Text.Encoding]::UTF8.GetString($versionBytes)|ConvertFrom-Json
$statement=[Text.Encoding]::UTF8.GetString($statementBytes)|ConvertFrom-Json
if($version.worktreeDirty-ne$false -or $version.clientOnly-ne$true -or
   $statement.packageManifestSha256-cne(Sha $manifestBytes) -or
   $statement.versionSha256-cne(Sha $versionBytes) -or $statement.candidate-cne$version.candidate){throw 'Candidate CMS does not bind metadata'}
AssertCandidateIdentity $version $statement
$descriptorBytes=Bytes (Join-Path $DeliveryRoot 'DELIVERY.json')
$descriptor=[Text.Encoding]::UTF8.GetString($descriptorBytes)|ConvertFrom-Json
if($descriptor.candidate-cne$version.candidate -or $descriptor.packageManifestSha256-cne(Sha $manifestBytes) -or
   $descriptor.versionSha256-cne(Sha $versionBytes)){throw 'Delivery Candidate binding mismatch'}
$certificate=Get-Item -LiteralPath ('Cert:/CurrentUser/My/'+$SignerThumbprint.ToUpperInvariant())
if(!$certificate.HasPrivateKey){throw 'Existing private signing identity required'}
CheckCertificate $certificate $now;CheckCertificate $certificate $descriptor.builtUnix
$arguments=@($verificationTool,'--candidate-root',$CandidateRoot,'--delivery-root',$DeliveryRoot,
    '--receipt',(Join-Path $DeliveryRoot 'DELIVERY-VERIFICATION.json'))
foreach($base in $BaseExe){NoLinks $base;$arguments+=@('--base-exe',[IO.Path]::GetFullPath($base))}
$log=Join-Path $DeliveryRoot 'byte-verification.log'
if(Test-Path -LiteralPath $log){throw 'Fresh verification log required'}
& $PythonPath @arguments *> $log
if($LASTEXITCODE-ne0){throw "Delivery byte reconstruction rejected; retained log: $log"}
if((Sha (Bytes (Join-Path $DeliveryRoot 'DELIVERY.json')))-cne(Sha $descriptorBytes)){throw 'Descriptor changed during verification'}
AssertSource
$cms=[Security.Cryptography.Pkcs.SignedCms]::new([Security.Cryptography.Pkcs.ContentInfo]::new($descriptorBytes),$true)
$signer=[Security.Cryptography.Pkcs.CmsSigner]::new($certificate)
$signer.IncludeOption=[Security.Cryptography.X509Certificates.X509IncludeOption]::EndCertOnly
$signer.DigestAlgorithm=[Security.Cryptography.Oid]::new('2.16.840.1.101.3.4.2.1')
$cms.ComputeSignature($signer,$false)
$signature=$cms.Encode();CheckCms $descriptorBytes $signature $descriptor.builtUnix
Save $signaturePath $signature
$proof=[ordered]@{schema='mir2.windows.delivery-signing.v1';passed=$true;cmsVerified=$true;decodedPayloadVerified=$true
    sourceRevision=$SourceRevision;candidate=$version.candidate;gameSourceRevision=$version.gitRevision
    deliverySha256=Sha $descriptorBytes;signatureSha256=Sha $signature;manifestSha256=Sha $manifestBytes
    bundles=$descriptor.bundles.Count;patches=$descriptor.patches.Count;createdUnix=$now;published=$false}
Save $receiptPath ([Text.UTF8Encoding]::new($false).GetBytes(($proof|ConvertTo-Json -Depth 8 -Compress)))
$proof|ConvertTo-Json -Depth 8
