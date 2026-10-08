#Requires -Version 7.2
<# Produce an exact R2 publication plan after the pinned CMS gates.
   No private key, credential, network, installation, or promotion is used. #>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$CandidateRoot,
    [Parameter(Mandatory)][string]$DeliveryRoot,
    [Parameter(Mandatory)][string]$EngineRoot,
    [Parameter(Mandatory)][string]$FeedRoot,
    [Parameter(Mandatory)][string]$OutputRoot,
    [Parameter(Mandatory)][string]$EngineProjectRoot,
    [Parameter(Mandatory)][string]$PublicationToolProjectRoot,
    [Parameter(Mandatory)][string]$ExpectedCurrent,
    [string]$BootstrapExe,
    [string]$PythonPath,
    [switch]$IncludeRootFiles
)
$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
$project=[IO.Path]::GetFullPath($EngineProjectRoot)
$toolProject=[IO.Path]::GetFullPath($PublicationToolProjectRoot)
$SourceRevision='b4390ee4c987bbf000ba1d95761151e39fc0d54f'
$toolRevision='4c60c323aed11829abae6ee5ce6e13c675a3c1c3'
$tool=Join-Path $toolProject 'apps/game-client/windows-updater/scripts/prepare-native-publication.py'
$toolSha='DA2BFE2C8358FE28AEE10396C1B173F7D81E1A998D4A3F2EA024C0E7A9CD5535'
$pin='6C70C777B27D50949370D494B4B25798200FBDD5C171BAEEAA94D7E289900F3E'
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
function AssertSource {
    NoLinks $project;NoLinks $toolProject
    if((& git -C $toolProject rev-parse HEAD).Trim()-cne$toolRevision -or $LASTEXITCODE-ne0 -or
       @(& git -C $toolProject status --porcelain=v1 --untracked-files=all).Count-ne0 -or $LASTEXITCODE-ne0 -or
       (Sha (Bytes $tool))-cne$toolSha){throw 'Exact clean pinned closure tooling required'}
    if((& git -C $project rev-parse HEAD).Trim()-cne$SourceRevision -or $LASTEXITCODE-ne0 -or
       @(& git -C $project status --porcelain=v1 --untracked-files=all).Count-ne0 -or $LASTEXITCODE-ne0){throw 'Exact clean committed publication tooling required'}
}
AssertSource
foreach($name in @('CandidateRoot','DeliveryRoot','EngineRoot','FeedRoot','OutputRoot')){
    $value=[IO.Path]::GetFullPath((Get-Variable -Name $name -ValueOnly));NoLinks $value;Set-Variable -Name $name -Value $value
}
if(Test-Path -LiteralPath $OutputRoot){throw 'Fresh publication output directory required'}
if(!$PythonPath){$PythonPath=(Get-Command python -ErrorAction Stop).Source};NoLinks $PythonPath
Add-Type -AssemblyName System.Security.Cryptography.Pkcs
$now=[DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
$statement=Bytes (Join-Path $CandidateRoot 'RELEASE-STATEMENT.json')
CheckCms $statement (Bytes (Join-Path $CandidateRoot 'RELEASE-STATEMENT.p7s')) $now
$feedBytes=Bytes (Join-Path $FeedRoot 'latest.json');$feed=[Text.Encoding]::UTF8.GetString($feedBytes)|ConvertFrom-Json
CheckCms $feedBytes (Bytes (Join-Path $FeedRoot 'latest.p7s')) $feed.createdUnix
if($feed.createdUnix-gt($now+300)-or$feed.expiresUnix-lt$now){throw 'Current signed discovery required'}
$engineBytes=Bytes (Join-Path $EngineRoot 'ENGINE.json');$engine=[Text.Encoding]::UTF8.GetString($engineBytes)|ConvertFrom-Json
CheckCms $engineBytes (Bytes (Join-Path $EngineRoot 'ENGINE.p7s')) $engine.builtUnix
if($engine.sourceRevision-cne$SourceRevision){throw 'Engine/publication exact source required'}
$deliveryBytes=Bytes (Join-Path $DeliveryRoot 'DELIVERY.json');$delivery=[Text.Encoding]::UTF8.GetString($deliveryBytes)|ConvertFrom-Json
CheckCms $deliveryBytes (Bytes (Join-Path $DeliveryRoot 'DELIVERY.p7s')) $delivery.builtUnix
$verifiedPath=Join-Path $DeliveryRoot 'DELIVERY-VERIFICATION.json'
$verified=[Text.Encoding]::UTF8.GetString((Bytes $verifiedPath))|ConvertFrom-Json
if($verified.passed-ne$true-or$verified.deliverySha256.ToUpperInvariant()-cne(Sha $deliveryBytes)){throw 'Exact decoded delivery verification required'}
NoLinks $ExpectedCurrent
$previous=[Text.Encoding]::UTF8.GetString((Bytes $ExpectedCurrent))|ConvertFrom-Json
if($previous.schema-cne'mir2.windows.r2-channel.v1'-or$previous.channel-cne'invited'-or$previous.platform-cne'windows-x64'-or
 $previous.sequence-ne16-or$previous.sourceRevision-cne$SourceRevision-or
 $previous.feedSha256-cne'fd8109231f6077405c410047fab94ea1a5797e9963e81fd9a80a0cb86cf95dca'-or
 $previous.signatureSha256-cne'1f881c23724b892ab9814eb2eb9002a365df159a7220bb5316538a0ea181a5c7'){throw 'Exact s16 predecessor pointer required'}
if($feed.sequence-ne17-or$feed.game.identity-cne'WN-CANDIDATE-20261008-invited-22'){throw 'Exact R22/s17 discovery required'}
$gameStatement=[Text.Encoding]::UTF8.GetString($statement)|ConvertFrom-Json
if($gameStatement.gitRevision-cne'7fea5cdba8cd160d4f2c5536f14fb7ce98c6bfbc'-or
 $gameStatement.candidate-cne'WN-CANDIDATE-20261008-invited-22'-or$gameStatement.worktreeDirty-ne$false){throw 'Exact clean R22 statement required'}
[IO.Directory]::CreateDirectory($OutputRoot)|Out-Null
$arguments=@($tool,'--candidate-root',$CandidateRoot,
    '--delivery-root',$DeliveryRoot,'--delivery-verification',$verifiedPath,'--engine-root',$EngineRoot,'--feed-root',$FeedRoot)
if($ExpectedCurrent){NoLinks $ExpectedCurrent;$arguments+=@('--expected-current',[IO.Path]::GetFullPath($ExpectedCurrent))}
if($BootstrapExe){NoLinks $BootstrapExe;$arguments+=@('--bootstrap-exe',[IO.Path]::GetFullPath($BootstrapExe))}
if($IncludeRootFiles){$arguments+='--include-root-files'}
$closurePath=Join-Path $OutputRoot 'OBJECT-CLOSURE.json'
& $PythonPath @arguments '--objects-only' $closurePath *> (Join-Path $OutputRoot 'object-closure.log')
if($LASTEXITCODE-ne0){throw 'Publication closure rejected; retained output directory contains diagnostics'}
$closure=Get-Content -LiteralPath $closurePath -Raw|ConvertFrom-Json
if($closure.candidate.sourceRevision-cne$SourceRevision){throw 'Publication pointer source mismatch'}
$verification=[ordered]@{schema='mir2.windows.r2-verification.v1';passed=$true;candidateVerified=$true;cmsVerified=$true
    sourceRevision=$SourceRevision;sequence=$closure.candidate.sequence;feedSha256=$closure.candidate.feedSha256
    signatureSha256=$closure.candidate.signatureSha256;objectsSha256=$closure.objectsSha256}
$verificationPath=Join-Path $OutputRoot 'ROOT-VERIFICATION.json'
Save $verificationPath $verification
$planPath=Join-Path $OutputRoot 'PUBLICATION-PLAN.json'
& $PythonPath @arguments '--verification-receipt' $verificationPath '--output' $planPath *> (Join-Path $OutputRoot 'publication-plan.log')
if($LASTEXITCODE-ne0){throw 'Signed closure publication plan rejected'}
AssertSource
$proof=[ordered]@{schema='mir2.windows.signed-publication-plan.v1';passed=$true;sourceRevision=$SourceRevision
    gameCandidate=$closure.bindings.gameCandidate;gameSourceRevision=$closure.bindings.gameSourceRevision
    objects=$closure.objects.Count;objectsSha256=$closure.objectsSha256;sequence=$closure.candidate.sequence
    planPath=$planPath;planSha256=(Get-FileHash -LiteralPath $planPath -Algorithm SHA256).Hash
    cmsVerified=$true;published=$false;credentialsUsed=$false}
Save (Join-Path $OutputRoot 'PREPARE-PUBLICATION.json') $proof
$proof|ConvertTo-Json -Depth 8
