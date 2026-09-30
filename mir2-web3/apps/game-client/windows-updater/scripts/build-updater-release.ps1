#Requires -Version 7.2
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$CandidateRoot,
    [Parameter(Mandatory)][string]$OutputRoot,
    [Parameter(Mandatory)][ValidatePattern('^[0-9a-fA-F]{40}$')][string]$SourceRevision,
    [Parameter(Mandatory)][ValidatePattern('^[0-9A-Fa-f]{40}$')][string]$SignerThumbprint,
    [Parameter(Mandatory)][ValidateRange(1,9007199254740991)][long]$Sequence,
    [string]$TargetDir
)
$ErrorActionPreference='Stop'
$project=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../../..'))
$crate=Join-Path $project 'apps/game-client/windows-updater'
$pin='6C70C777B27D50949370D494B4B25798200FBDD5C171BAEEAA94D7E289900F3E'
function Sha([byte[]]$Bytes) { [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($Bytes)) }
function FileSha([string]$Path) { (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToUpperInvariant() }
function NoLinks([string]$Path) {
    $path=[IO.Path]::GetFullPath($Path)
    while($path) {
        if(Test-Path -LiteralPath $path) {
            $item=Get-Item -LiteralPath $path -Force
            if(($item.Attributes -band [IO.FileAttributes]::ReparsePoint)-ne0) {throw 'Linked output/input rejected'}
        }
        $parent=Split-Path -Parent $path
        if(!$parent -or $parent -eq $path) {break}
        $path=$parent
    }
}
function JsonBytes($Value) { [Text.UTF8Encoding]::new($false).GetBytes(($Value | ConvertTo-Json -Depth 20 -Compress)) }
function Put([string]$Path,[byte[]]$Bytes) {
    NoLinks $Path
    [IO.Directory]::CreateDirectory((Split-Path -Parent $Path)) | Out-Null
    if(Test-Path -LiteralPath $Path) {throw "Refusing overwrite: $Path"}
    [IO.File]::WriteAllBytes($Path,$Bytes)
}
function Cms([byte[]]$Bytes) {
    $cms=[Security.Cryptography.Pkcs.SignedCms]::new([Security.Cryptography.Pkcs.ContentInfo]::new($Bytes),$true)
    $signer=[Security.Cryptography.Pkcs.CmsSigner]::new($certificate)
    $signer.IncludeOption=[Security.Cryptography.X509Certificates.X509IncludeOption]::EndCertOnly
    $signer.DigestAlgorithm=[Security.Cryptography.Oid]::new('2.16.840.1.101.3.4.2.1')
    $cms.ComputeSignature($signer,$false)
    return ,$cms.Encode()
}
function CheckCms([byte[]]$Bytes,[byte[]]$Signature) {
    $cms=[Security.Cryptography.Pkcs.SignedCms]::new([Security.Cryptography.Pkcs.ContentInfo]::new($Bytes),$true)
    $cms.Decode($Signature);$cms.CheckSignature($true)
    if($cms.SignerInfos.Count-ne1){throw 'Exactly one CMS signer required'}
    $rsa=[Security.Cryptography.X509Certificates.RSACertificateExtensions]::GetRSAPublicKey($cms.SignerInfos[0].Certificate)
    if(!$rsa -or (Sha $rsa.ExportRSAPublicKey())-cne$pin){throw 'Publisher key pin mismatch'}
}
function Entry([string]$Path,[byte[]]$Bytes) {
    [ordered]@{path=$Path;size=[long]$Bytes.Length;sha256=Sha $Bytes}
}
Add-Type -AssemblyName System.Security.Cryptography.Pkcs
if((& git -C $project rev-parse HEAD).Trim()-cne$SourceRevision.ToLowerInvariant() -or $LASTEXITCODE-ne0) {throw 'Exact source revision required'}
if(@(& git -C $project status --porcelain=v1 --untracked-files=all).Count-ne0 -or $LASTEXITCODE-ne0) {throw 'Clean source required'}
$certificate=Get-Item -LiteralPath ("Cert:/CurrentUser/My/"+$SignerThumbprint.ToUpperInvariant())
if(!$certificate.HasPrivateKey -or (Get-Date)-lt$certificate.NotBefore -or (Get-Date)-gt$certificate.NotAfter){throw 'Current valid signing identity required'}
$rsa=[Security.Cryptography.X509Certificates.RSACertificateExtensions]::GetRSAPublicKey($certificate)
if(!$rsa -or (Sha $rsa.ExportRSAPublicKey())-cne$pin){throw 'Signing key does not match launcher trust'}
$CandidateRoot=[IO.Path]::GetFullPath($CandidateRoot);NoLinks $CandidateRoot
$OutputRoot=[IO.Path]::GetFullPath($OutputRoot);NoLinks $OutputRoot
if(Test-Path -LiteralPath $OutputRoot){throw 'Fresh release output directory required'}
if(!$TargetDir){$TargetDir=Join-Path $crate 'target'}
$TargetDir=[IO.Path]::GetFullPath($TargetDir);NoLinks $TargetDir
& {
    foreach($name in @('RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS','RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER')){
        if([Environment]::GetEnvironmentVariable($name)){throw "Uncontrolled compiler environment: $name"}
    }
    try {
        $env:RUSTFLAGS="--remap-path-prefix=$project=mir2-source"
        & cargo +1.95.0 build --locked --release --manifest-path (Join-Path $crate 'Cargo.toml') --target x86_64-pc-windows-msvc --target-dir $TargetDir
        if($LASTEXITCODE-ne0){throw 'Updater release build failed'}
    } finally {Remove-Item Env:RUSTFLAGS -ErrorAction SilentlyContinue}
}
$release=Join-Path $TargetDir 'x86_64-pc-windows-msvc/release'
$launcher=[IO.File]::ReadAllBytes((Join-Path $release 'Mir2Launcher.exe'))
$engineExe=[IO.File]::ReadAllBytes((Join-Path $release 'Mir2Updater.exe'))
if((& git -C $project rev-parse HEAD).Trim()-cne$SourceRevision.ToLowerInvariant() -or @(& git -C $project status --porcelain=v1 --untracked-files=all).Count-ne0){throw 'Source changed during build'}
$now=[DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
$engineHash=Sha $engineExe
$engine=[ordered]@{
    schema='mir2.windows.updater-engine.v1';version=1;minBootstrap=1
    sourceRevision=$SourceRevision.ToLowerInvariant();builtUnix=$now
    exeSize=[long]$engineExe.Length;exeSha256=$engineHash;launcherSha256=Sha $launcher
    cargoLockSha256=FileSha (Join-Path $crate 'Cargo.lock')
}
$engineBytes=JsonBytes $engine;$engineSig=Cms $engineBytes
CheckCms $engineBytes $engineSig
$metadata=@()
foreach($name in @('PACKAGE-MANIFEST.json','VERSION.json','RELEASE-STATEMENT.json','RELEASE-STATEMENT.p7s')){
    $bytes=[IO.File]::ReadAllBytes((Join-Path $CandidateRoot $name))
    $metadata+=Entry $name $bytes
}
$statementBytes=[IO.File]::ReadAllBytes((Join-Path $CandidateRoot 'RELEASE-STATEMENT.json'))
CheckCms $statementBytes ([IO.File]::ReadAllBytes((Join-Path $CandidateRoot 'RELEASE-STATEMENT.p7s')))
$version=Get-Content -LiteralPath (Join-Path $CandidateRoot 'VERSION.json') -Raw | ConvertFrom-Json
$statement=[Text.Encoding]::UTF8.GetString($statementBytes) | ConvertFrom-Json
if($statement.versionSha256-cne(FileSha (Join-Path $CandidateRoot 'VERSION.json')) -or
   $statement.packageManifestSha256-cne(FileSha (Join-Path $CandidateRoot 'PACKAGE-MANIFEST.json')) -or
   $version.worktreeDirty-ne$false -or $version.clientOnly-ne$true){throw 'Invalid game Candidate bindings'}
$gameDirectory='releases/game-'+$version.candidate
$engineDirectory='releases/updater-'+$engineHash.Substring(0,16)+'-s'+$Sequence
$feed=[ordered]@{
    schema='mir2.windows.update-feed.v1';channel='invited';platform='windows-x64';sequence=$Sequence
    createdUnix=$now;expiresUnix=$now+(30*86400);minBootstrap=1
    protocol='crystal-mir2-v1';content='mir2.windows.package-manifest.v4'
    game=[ordered]@{directory=$gameDirectory;identity=$version.candidate;metadata=$metadata}
    engine=[ordered]@{directory=$engineDirectory;identity='1';metadata=@((Entry 'ENGINE.json' $engineBytes),(Entry 'ENGINE.p7s' $engineSig))}
}
$feedBytes=JsonBytes $feed;$feedSig=Cms $feedBytes;CheckCms $feedBytes $feedSig
$bundle=Join-Path $OutputRoot 'bundle'
$engineRelative="updater/engines/$engineHash"
Put (Join-Path $bundle 'Mir2Launcher.exe') $launcher
Put (Join-Path $bundle "$engineRelative/Mir2Updater.exe") $engineExe
Put (Join-Path $bundle "$engineRelative/ENGINE.json") $engineBytes
Put (Join-Path $bundle "$engineRelative/ENGINE.p7s") $engineSig
Put (Join-Path $bundle 'updater/active.txt') ([Text.Encoding]::ASCII.GetBytes($engineHash))
Put (Join-Path $bundle 'updater/seed-feed.json') $feedBytes
Put (Join-Path $bundle 'updater/seed-feed.p7s') $feedSig
Put (Join-Path $OutputRoot 'feed/latest.json') $feedBytes
Put (Join-Path $OutputRoot 'feed/latest.p7s') $feedSig
$engineRelease=Join-Path $OutputRoot 'engine-release'
Put (Join-Path $engineRelease 'ENGINE.json') $engineBytes
Put (Join-Path $engineRelease 'ENGINE.p7s') $engineSig
Put (Join-Path $engineRelease 'Mir2Updater.exe') $engineExe
$bundleFiles=@(Get-ChildItem -LiteralPath $bundle -Recurse -File | Sort-Object FullName | ForEach-Object {
    [ordered]@{path=[IO.Path]::GetRelativePath($bundle,$_.FullName).Replace('\','/');size=[long]$_.Length;sha256=FileSha $_.FullName}
})
$bundleStatement=JsonBytes ([ordered]@{schema='mir2.windows.updater-bundle.v1';sourceRevision=$SourceRevision;gameCandidate=$version.candidate;gameSourceRevision=$version.gitRevision;files=$bundleFiles})
Put (Join-Path $bundle 'UPDATER-BUNDLE.json') $bundleStatement
Put (Join-Path $bundle 'UPDATER-BUNDLE.p7s') (Cms $bundleStatement)
$sourceMap=[ordered]@{}
$sourceMap[$gameDirectory]=$CandidateRoot;$sourceMap[$engineDirectory]=$engineRelease
Put (Join-Path $OutputRoot 'source-map.json') (JsonBytes $sourceMap)
$receipt=[ordered]@{schema='mir2.windows.updater-build.v1';sourceRevision=$SourceRevision;sourceClean=$true
    engineSha256=$engineHash;launcherSha256=Sha $launcher;gameCandidate=$version.candidate
    gameSourceRevision=$version.gitRevision;sequence=$Sequence;builtUnix=$now
    command='cargo +1.95.0 build --locked --release --target x86_64-pc-windows-msvc'
    publicAuthenticode='not provided by detached CMS; public publisher signing remains separate'}
Put (Join-Path $OutputRoot 'BUILD-UPDATER.json') (JsonBytes $receipt)
$receipt | ConvertTo-Json -Depth 5
