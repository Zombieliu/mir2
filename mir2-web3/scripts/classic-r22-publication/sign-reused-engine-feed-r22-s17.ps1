#Requires -Version 7.2
# Feed-only R22/s17 signing; old engine payload and identity are reused exactly.
[CmdletBinding()]
param([Parameter(Mandatory)][string]$CandidateRoot,
 [Parameter(Mandatory)][string]$GameProjectRoot,
 [Parameter(Mandatory)][string]$PreviousFeedRoot,
 [Parameter(Mandatory)][string]$EngineRoot,
 [Parameter(Mandatory)][string]$OutputRoot,
 [Parameter(Mandatory)][string]$SignerThumbprint)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$pin='6C70C777B27D50949370D494B4B25798200FBDD5C171BAEEAA94D7E289900F3E'
$gameSource='7fea5cdba8cd160d4f2c5536f14fb7ce98c6bfbc'
$project=[IO.Path]::GetFullPath($GameProjectRoot)
$engineSource='b4390ee4c987bbf000ba1d95761151e39fc0d54f'
function NoLinks([string]$Path) {
    $cursor=[IO.Path]::GetFullPath($Path)
    if($cursor.StartsWith('\\')){throw 'UNC publication inputs unsupported'}
    while($cursor){
        if((Test-Path -LiteralPath $cursor)-and((Get-Item -LiteralPath $cursor -Force).Attributes-band[IO.FileAttributes]::ReparsePoint)-ne0){throw 'Linked publication path rejected'}
        $parent=Split-Path -Parent $cursor
        if(!$parent -or $parent-eq$cursor){break};$cursor=$parent
    }
}
function Bytes([string]$Path) {
    NoLinks $Path
    $item=Get-Item -LiteralPath $Path -Force
    if($item.PSIsContainer -or $item.Length-gt33554432 -or $item.LinkType-eq'HardLink'){throw 'Bounded plain metadata required'}
    foreach($stream in @(Get-Item -LiteralPath $Path -Stream *)){if($stream.Stream-cne':$DATA'){throw 'Metadata alternate stream rejected'}}
    return ,[IO.File]::ReadAllBytes($Path)
}
function Sha([byte[]]$Value){[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($Value))}
function Save([string]$Path,$Value){
    NoLinks $Path
    $bytes=[Text.UTF8Encoding]::new($false).GetBytes(($Value|ConvertTo-Json -Depth 16 -Compress))
    $stream=[IO.File]::Open($Path,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
    try{$stream.Write($bytes);$stream.Flush($true)}finally{$stream.Dispose()}
}
function CheckCms([byte[]]$Value,[byte[]]$Signature,[long]$At){
    $envelope=[Security.Cryptography.Pkcs.SignedCms]::new();$envelope.Decode($Signature)
    if($envelope.ContentInfo.Content.Length-ne0){throw 'Detached CMS required'}
    $cms=[Security.Cryptography.Pkcs.SignedCms]::new([Security.Cryptography.Pkcs.ContentInfo]::new($Value),$true)
    $cms.Decode($Signature)
    if($cms.SignerInfos.Count-ne1 -or $cms.SignerInfos[0].DigestAlgorithm.Value-cne'2.16.840.1.101.3.4.2.1'){throw 'One SHA256 CMS signer required'}
    $cms.CheckSignature($true)
    $certificate=$cms.SignerInfos[0].Certificate
    $when=[DateTimeOffset]::FromUnixTimeSeconds($At).UtcDateTime
    if(!$certificate -or $certificate.NotBefore.ToUniversalTime()-gt$when -or $certificate.NotAfter.ToUniversalTime()-lt$when){throw 'CMS certificate time mismatch'}
    $eku=$certificate.Extensions|Where-Object {$_.Oid.Value-eq'2.5.29.37'}|Select-Object -First 1
    $usage=$certificate.Extensions|Where-Object {$_.Oid.Value-eq'2.5.29.15'}|Select-Object -First 1
    if(!$eku -or @($eku.EnhancedKeyUsages|ForEach-Object {$_.Value})-notcontains'1.3.6.1.5.5.7.3.3' -or
       ($usage-and($usage.KeyUsages-band[Security.Cryptography.X509Certificates.X509KeyUsageFlags]::DigitalSignature)-eq0)){throw 'CMS code signing usage required'}
    $rsa=[Security.Cryptography.X509Certificates.RSACertificateExtensions]::GetRSAPublicKey($certificate)
    if(!$rsa-or(Sha $rsa.ExportRSAPublicKey())-cne$pin){throw 'CMS publisher key pin mismatch'}
}
function Cms([byte[]]$Bytes) {
    $cms=[Security.Cryptography.Pkcs.SignedCms]::new([Security.Cryptography.Pkcs.ContentInfo]::new($Bytes),$true)
    $signer=[Security.Cryptography.Pkcs.CmsSigner]::new($certificate)
    $signer.IncludeOption=[Security.Cryptography.X509Certificates.X509IncludeOption]::EndCertOnly
    $signer.DigestAlgorithm=[Security.Cryptography.Oid]::new('2.16.840.1.101.3.4.2.1')
    $cms.ComputeSignature($signer,$false)
    return ,$cms.Encode()
}

Add-Type -AssemblyName System.Security.Cryptography.Pkcs
foreach($dir in @($CandidateRoot,$PreviousFeedRoot,$EngineRoot,$OutputRoot)){NoLinks $dir}
if(Test-Path -LiteralPath $OutputRoot){throw 'Fresh feed output required'}
function AssertSource {
 if((& git -C $project rev-parse HEAD).Trim()-cne$gameSource -or $LASTEXITCODE-ne0 -or
  @(& git -C $project status --porcelain=v1 --untracked-files=all).Count-ne0 -or $LASTEXITCODE-ne0){throw 'Exact clean R22 source required'}
}
NoLinks $project
AssertSource
$now=[DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
$oldBytes=Bytes (Join-Path $PreviousFeedRoot 'latest.json')
if((Sha $oldBytes)-cne'FD8109231F6077405C410047FAB94EA1A5797E9963E81FD9A80A0CB86CF95DCA'){throw 'Exact public predecessor required'}
$old=[Text.Encoding]::UTF8.GetString($oldBytes)|ConvertFrom-Json
$oldSignature=Bytes (Join-Path $PreviousFeedRoot 'latest.p7s')
if((Sha $oldSignature)-cne'1F881C23724B892AB9814EB2EB9002A365DF159A7220BB5316538A0EA181A5C7'){throw 'Exact predecessor CMS required'}
CheckCms $oldBytes $oldSignature $old.createdUnix
if($old.sequence-ne16-or$old.game.identity-cne'WN-CANDIDATE-20261007-invited-20'-or$old.expiresUnix-lt$now-or$old.createdUnix-gt($now+300)){throw 'Current s16/R20 predecessor required'}
if($old.engine.directory-cne'releases/updater-9213EC3FE1A992F7-s15'-or$old.engine.identity-cne'1'-or@($old.engine.metadata).Count-ne2){throw 'Exact s15 engine component required'}
foreach($entry in $old.engine.metadata){
 if($entry.path-cnotin@('ENGINE.json','ENGINE.p7s')){throw 'Unexpected engine metadata path'}
 $actual=Bytes (Join-Path $EngineRoot $entry.path)
 if($actual.Length-ne$entry.size-or(Sha $actual)-cne$entry.sha256){throw 'Changed engine metadata rejected'}
}
$engineBytes=Bytes (Join-Path $EngineRoot 'ENGINE.json')
$engine=[Text.Encoding]::UTF8.GetString($engineBytes)|ConvertFrom-Json
CheckCms $engineBytes (Bytes (Join-Path $EngineRoot 'ENGINE.p7s')) $engine.builtUnix
$engineExe=Bytes (Join-Path $EngineRoot 'Mir2Updater.exe')
if($engine.sourceRevision-cne$engineSource-or$engine.version-ne1-or$engine.minBootstrap-ne1-or
 $engine.exeSize-ne$engineExe.Length-or$engine.exeSha256-cne(Sha $engineExe)-or
 (Sha $engineExe)-cne'9213EC3FE1A992F757EA48C53981AFA661765B42E637C1AD811E326060B170B8'){throw 'Changed s15 engine payload rejected'}
$manifest=Bytes (Join-Path $CandidateRoot 'PACKAGE-MANIFEST.json')
$versionBytes=Bytes (Join-Path $CandidateRoot 'VERSION.json')
$statementBytes=Bytes (Join-Path $CandidateRoot 'RELEASE-STATEMENT.json')
$statementSignature=Bytes (Join-Path $CandidateRoot 'RELEASE-STATEMENT.p7s')
CheckCms $statementBytes $statementSignature $now
$version=[Text.Encoding]::UTF8.GetString($versionBytes)|ConvertFrom-Json
$statement=[Text.Encoding]::UTF8.GetString($statementBytes)|ConvertFrom-Json
if($version.gitRevision-cne$gameSource-or$version.candidate-cne'WN-CANDIDATE-20261008-invited-22'-or
 $version.worktreeDirty-ne$false-or$version.clientOnly-ne$true-or
 $statement.gitRevision-cne$gameSource-or$statement.candidate-cne'WN-CANDIDATE-20261008-invited-22'-or
 $statement.worktreeDirty-ne$false-or
 $statement.versionSha256-cne(Sha $versionBytes)-or$statement.packageManifestSha256-cne(Sha $manifest)-or
 $version.packageManifestSha256-cne(Sha $manifest)){throw 'R22 signed Candidate binding mismatch'}
$certificate=Get-Item -LiteralPath ('Cert:/CurrentUser/My/'+$SignerThumbprint.ToUpperInvariant())
if(!$certificate.HasPrivateKey-or$certificate.NotBefore.ToUniversalTime()-gt[DateTime]::UtcNow-or$certificate.NotAfter.ToUniversalTime()-lt[DateTime]::UtcNow){throw 'Current private signing identity required'}
$rsa=[Security.Cryptography.X509Certificates.RSACertificateExtensions]::GetRSAPublicKey($certificate)
if(!$rsa-or(Sha $rsa.ExportRSAPublicKey())-cne$pin){throw 'Signing key pin mismatch'}
$metadata=@()
foreach($name in @('PACKAGE-MANIFEST.json','VERSION.json','RELEASE-STATEMENT.json','RELEASE-STATEMENT.p7s')){
 $raw=Bytes (Join-Path $CandidateRoot $name)
 $metadata+= [ordered]@{path=$name;size=[long]$raw.Length;sha256=Sha $raw}
}
$feed=[ordered]@{schema=$old.schema;channel=$old.channel;platform=$old.platform;sequence=17
 createdUnix=$now;expiresUnix=$now+(30*86400);minBootstrap=$old.minBootstrap
 protocol=$old.protocol;content=$old.content
 game=[ordered]@{directory='releases/game-'+$version.candidate;identity=$version.candidate;metadata=$metadata}
 engine=$old.engine}
$feedBytes=[Text.UTF8Encoding]::new($false).GetBytes(($feed|ConvertTo-Json -Depth 20 -Compress))
$feedSignature=Cms $feedBytes
CheckCms $feedBytes $feedSignature $now
AssertSource
[IO.Directory]::CreateDirectory($OutputRoot)|Out-Null
foreach($pair in @(@('latest.json',$feedBytes),@('latest.p7s',$feedSignature))){
 $stream=[IO.File]::Open((Join-Path $OutputRoot $pair[0]),[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
 try{$stream.Write([byte[]]$pair[1]);$stream.Flush($true)}finally{$stream.Dispose()}
}
Save (Join-Path $OutputRoot 'SIGN-FEED.json') ([ordered]@{schema='mir2.r22.reused-engine-feed.v1';sequence=17;gameSource=$gameSource
 engineSource=$engineSource;engineReused=$true;engineRecompiled=$false;cmsVerified=$true
 previousFeedSha256=Sha $oldBytes;feedSha256=Sha $feedBytes;feedSignatureSha256=Sha $feedSignature
 recipeSha256=(Get-FileHash -LiteralPath $PSCommandPath -Algorithm SHA256).Hash;createdUnix=$now})
[ordered]@{sequence=17;game=$version.candidate;engineReused=$true;cmsVerified=$true;feedSha256=Sha $feedBytes}|ConvertTo-Json
