#Requires -Version 7.2
<# Build only from already signed releases. No signing, download, installation,
   activation, game launch or publication. Real gates precede literal inputs. #>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$CandidateRoot,
    [Parameter(Mandatory)][string]$CandidateSourceRoot,
    [Parameter(Mandatory)][string]$BundleRoot,
    [Parameter(Mandatory)][string]$PackedRoot,
    [Parameter(Mandatory)][string]$OutputRoot,
    [Parameter(Mandatory)][ValidatePattern('^[0-9a-f]{40}$')][string]$GameSourceRevision,
    [Parameter(Mandatory)][ValidatePattern('^[0-9a-f]{40}$')][string]$EngineSourceRevision,
    [ValidatePattern('^(?-i:[0-9a-f]{40})?$')][string]$InstallerSourceRevision='',
    [Parameter(Mandatory)][ValidatePattern('^WN-CANDIDATE-[A-Za-z0-9._-]+$')][string]$Candidate,
    [Parameter(Mandatory)][ValidatePattern('^[0-9A-Fa-f]{64}$')][string]$ManifestSha256,
    [Parameter(Mandatory)][string]$PackVerifierPath,
    [Parameter(Mandatory)][ValidatePattern('^[0-9A-Fa-f]{64}$')][string]$PackVerifierSha256,
    [Parameter(Mandatory)][ValidatePattern('^[0-9A-Fa-f]{40}$')][string]$TrustedSignerThumbprint,
    [Parameter(Mandatory)][ValidatePattern('^[0-9]{4}\.[0-9]{1,2}\.[0-9]{1,2}\.[0-9]{1,4}$')][string]$SetupVersion,
    [Parameter(Mandatory)][ValidatePattern('^[A-Za-z0-9._-]+$')][string]$OutputBaseName,
    [Parameter(Mandatory)][string]$RuntimeRedist,
    [Parameter(Mandatory)][string]$InnoCompiler,
    [Parameter(Mandatory)][ValidatePattern('^[0-9A-Fa-f]{64}$')][string]$InnoCompilerSha256,
    [ValidateSet('platinum_176_v27','platinum_176','platinum_fiveclass')][string]$ContentProfile='platinum_176_v27',
    [string]$PythonPath,
    [switch]$ValidationInstall,
    [string]$ValidationAppGuid='',
    [switch]$ProgressiveBootstrap,
    [switch]$PrepareOnly
)
$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
$project=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../../../..'))
$updaterScripts=Join-Path $project 'apps/game-client/windows-updater/scripts'
$runtimePin='843068991DAAA1F73AD9F6239BCE4D0F6A07A51F18C37EA2A867E9BECA71295C'
$publicKeyPin='6C70C777B27D50949370D494B4B25798200FBDD5C171BAEEAA94D7E289900F3E'
function NoLinks([string]$Path) {
    $cursor=[IO.Path]::GetFullPath($Path)
    if($cursor.StartsWith('\')){throw 'Local physical paths required'}
    while($cursor){
        if((Test-Path -LiteralPath $cursor) -and ((Get-Item -LiteralPath $cursor -Force).Attributes-band[IO.FileAttributes]::ReparsePoint)-ne0){throw 'Reparse path rejected'}
        $parent=Split-Path -Parent $cursor;if(!$parent-or$parent-eq$cursor){break};$cursor=$parent
    }
}
function Sha([string]$Path){(Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToUpperInvariant()}
function ReadJson([string]$Path){Get-Content -LiteralPath $Path -Raw|ConvertFrom-Json}
function Ordinary([string]$Path){
    NoLinks $Path;$item=Get-Item -LiteralPath $Path -Force
    if($item.PSIsContainer -or $item.LinkType-eq'HardLink'){throw 'Ordinary single-link file required'}
    foreach($stream in @(Get-Item -LiteralPath $Path -Stream *)){if($stream.Stream-cne':$DATA'){throw 'Named data stream rejected'}}
}
function CleanSource([string]$Root,[string]$Revision){
    NoLinks $Root
    $top=& git -C $Root rev-parse --show-toplevel
    if($LASTEXITCODE-ne0-or![IO.Path]::GetFullPath($Root).TrimEnd('\','/').Equals([IO.Path]::GetFullPath($top).TrimEnd('\','/'),[StringComparison]::OrdinalIgnoreCase)){throw 'Exact source root required'}
    $head=& git -C $Root rev-parse HEAD;if($LASTEXITCODE-ne0-or$head-cne$Revision){throw 'Exact source checkout required'}
    $dirty=& git -C $Root status --porcelain --untracked-files=all
    if($LASTEXITCODE-ne0-or$dirty){throw 'Clean source checkout required'}
}
function InstallerSourceBinding([string]$RecipeRoot,[string]$GameRoot,[string]$GameRevision,[string]$EngineRevision,[string]$RecipeRevision){
    CleanSource $GameRoot $GameRevision
    CleanSource $RecipeRoot $RecipeRevision
    $allowed=@(
        'mir2-web3/apps/game-client/platform-windows/scripts/invited-installer/Mir2-Invite.iss',
        'mir2-web3/apps/game-client/platform-windows/scripts/invited-installer/README.md',
        'mir2-web3/apps/game-client/platform-windows/scripts/invited-installer/build-packed-installer.ps1'
    )
    $split=$RecipeRevision-cne$EngineRevision
    $changed=@()
    $engineRoot=$RecipeRoot
    if($split){
        if($GameRevision-cne$EngineRevision){throw 'Separated installer source requires identical game and engine revisions'}
        & git -C $RecipeRoot merge-base --is-ancestor $EngineRevision $RecipeRevision
        if($LASTEXITCODE-ne0){throw 'Installer source must descend from the exact engine source'}
        $changed=@(& git -c core.quotepath=false -C $RecipeRoot diff --name-only --no-renames --no-ext-diff --no-textconv $EngineRevision $RecipeRevision --)
        if($LASTEXITCODE-ne0){throw 'Installer source delta unavailable'}
        foreach($path in $changed){if($allowed-cnotcontains$path){throw "Installer source changed a non-recipe path: $path"}}
        $engineRoot=$GameRoot
    }
    CleanSource $engineRoot $EngineRevision
    $recipeTree=& git -C $RecipeRoot rev-parse "$RecipeRevision`^{tree}"
    if($LASTEXITCODE-ne0-or$recipeTree-cnotmatch'^[0-9a-f]{40}$'){throw 'Installer source tree unavailable'}
    $engineTree=& git -C $RecipeRoot rev-parse "$EngineRevision`^{tree}"
    if($LASTEXITCODE-ne0-or$engineTree-cnotmatch'^[0-9a-f]{40}$'){throw 'Engine source tree unavailable'}
    $files=@()
    foreach($name in $allowed){
        $path=Join-Path $RecipeRoot $name;Ordinary $path
        $files+=@{path=$name;sha256=Sha $path}
    }
    return @{splitSource=$split;installerSourceRoot=$RecipeRoot;installerSourceRevision=$RecipeRevision;installerTree=$recipeTree;
        engineSourceRoot=$engineRoot;engineSourceRevision=$EngineRevision;engineTree=$engineTree;gameSourceRevision=$GameRevision;
        runtimeIsAncestor=$true;allowedChangedPaths=$allowed;changedPaths=$changed;recipeFiles=$files;clean=$true}
}
function TrustedBinary([string]$Path,[string]$Hash,[string]$Publisher){
    Ordinary $Path
    $sig=Get-AuthenticodeSignature -LiteralPath $Path
    if((Sha $Path)-cne$Hash.ToUpperInvariant()-or$sig.Status-ne'Valid'-or!$sig.SignerCertificate-or$sig.SignerCertificate.Subject-cne$Publisher){throw 'Compiler/runtime identity rejected'}
}
function Gate([string]$Exe,[string[]]$Arguments,[string]$Name){
    $log=Join-Path $OutputRoot ($Name+'.log')
    if(Test-Path -LiteralPath $log){throw 'Fresh gate log required'}
    & $Exe @Arguments *> $log
    if($LASTEXITCODE-ne0){throw "$Name failed; see retained log $log"}
}
function GameKeyPin {
    Add-Type -AssemblyName System.Security.Cryptography.Pkcs
    $bytes=[IO.File]::ReadAllBytes((Join-Path $CandidateRoot 'RELEASE-STATEMENT.json'))
    $sig=[IO.File]::ReadAllBytes((Join-Path $CandidateRoot 'RELEASE-STATEMENT.p7s'))
    $envelope=[Security.Cryptography.Pkcs.SignedCms]::new();$envelope.Decode($sig)
    if($envelope.ContentInfo.Content.Length-ne0){throw 'Detached game CMS required'}
    $cms=[Security.Cryptography.Pkcs.SignedCms]::new([Security.Cryptography.Pkcs.ContentInfo]::new($bytes),$true);$cms.Decode($sig)
    if($cms.SignerInfos.Count-ne1-or$cms.SignerInfos[0].DigestAlgorithm.Value-cne'2.16.840.1.101.3.4.2.1'){throw 'Single SHA256 game CMS signer required'}
    $cms.CheckSignature($true)
    $rsa=[Security.Cryptography.X509Certificates.RSACertificateExtensions]::GetRSAPublicKey($cms.SignerInfos[0].Certificate)
    if(!$rsa){throw 'RSA game signer required'}
    try{if([Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($rsa.ExportRSAPublicKey()))-cne$publicKeyPin){throw 'Game CMS publisher pin mismatch'}}finally{$rsa.Dispose()}
}
function VerifyCopiedScopeControl([string]$GenerationDirectory) {
    # Called only after real copied GENERATION CMS authentication. The scope is
    # authenticated by its FileEntry in that manifest, not a separate signature.
    $manifest=ReadJson (Join-Path $GenerationDirectory 'GENERATION.json')
    $present=$manifest.PSObject.Properties.Name-ccontains'requiredScope'
    if($manifest.schema-ceq'mir2.windows.generation.v1'){
        if($present){throw 'V1 must not carry required scope authority'}
        return 0
    }
    if($manifest.schema-cne'mir2.windows.generation.v2'-or!$present-or!$manifest.requiredScope){throw 'Signed v2 required scope missing'}
    $entry=$manifest.requiredScope
    if($entry.path-cne'required-scope.json'-or($entry.size-isnot[int]-and$entry.size-isnot[long])-or$entry.size-le0-or$entry.size-gt4194304-or$entry.sha256-cnotmatch'^[0-9A-F]{64}$'){throw 'Copied required scope descriptor rejected'}
    $path=Join-Path $GenerationDirectory 'required-scope.json';Ordinary $path
    if((Get-Item -LiteralPath $path).Length-ne$entry.size-or(Sha $path)-cne$entry.sha256){throw 'Copied required scope bytes differ from signed manifest'}
    return 1
}
function VerifyCopiedCms([string]$InstallerDirectory) {
    # Import in a function scope: the verifier's parameter names must not replace
    # this build's OutputRoot or other caller state when it is dot-sourced.
    . (Join-Path $updaterScripts 'verify-packed-generation.ps1') -OutputRoot $PackedRoot -ExpectedManifestSha256 $ManifestSha256 -PackVerifierPath $PackVerifierPath -PackVerifierSha256 $PackVerifierSha256
    $copied=Join-Path $InstallerDirectory 'inputs'
    $generation=Join-Path $copied ('generations/'+$ManifestSha256.ToLowerInvariant())
    $packFeed=ReadJson (Join-Path $generation 'PACK-FEED.json')
    $active=(Get-Content -LiteralPath (Join-Path $copied 'updater/active.txt') -Raw)
    $copiedEngine=Join-Path $copied ('updater/engines/'+$active)
    $engineMetadata=ReadJson (Join-Path $copiedEngine 'ENGINE.json')
    $seedMetadata=ReadJson (Join-Path $copied 'updater/seed-feed.json')
    $copiedPairs=@(
        @{root=(Join-Path $copied 'game');name='RELEASE-STATEMENT';at=[DateTimeOffset]::UtcNow.ToUnixTimeSeconds()},
        @{root=$generation;name='GENERATION';at=$packFeed.createdUnix},
        @{root=$generation;name='PACK-FEED';at=$packFeed.createdUnix},
        @{root=$copied;name='UPDATER-BUNDLE';at=$engineMetadata.builtUnix},
        @{root=$copiedEngine;name='ENGINE';at=$engineMetadata.builtUnix},
        @{root=(Join-Path $copied 'updater');name='seed-feed';at=$seedMetadata.createdUnix}
    )
    foreach($pair in $copiedPairs){
        Packed-Cms (Packed-Bytes (Join-Path $pair.root ($pair.name+'.json')) 4194304) (Packed-Bytes (Join-Path $pair.root ($pair.name+'.p7s')) 32768) $pair.at
    }
    $null=VerifyCopiedScopeControl $generation
    return $copiedPairs.Count
}
foreach($name in @('CandidateRoot','CandidateSourceRoot','BundleRoot','PackedRoot','OutputRoot','RuntimeRedist','InnoCompiler','PackVerifierPath')){
    $value=[IO.Path]::GetFullPath((Get-Variable -Name $name -ValueOnly));NoLinks $value
    if($value.IndexOfAny([char[]]'"{}')-ge0-or$value.Contains("`r")-or$value.Contains("`n")){throw 'Unsafe build path'}
    Set-Variable -Name $name -Value $value
}
if(Test-Path -LiteralPath $OutputRoot){throw 'Fresh external output required'}
NoLinks (Split-Path -Parent $OutputRoot)
if(!(Test-Path -LiteralPath (Split-Path -Parent $OutputRoot) -PathType Container)){throw 'Existing output parent required'}
foreach($inputRoot in @($CandidateRoot,$CandidateSourceRoot,$BundleRoot,$PackedRoot,$project)){
    if($OutputRoot.Equals($inputRoot,[StringComparison]::OrdinalIgnoreCase)-or$OutputRoot.StartsWith($inputRoot.TrimEnd('\','/')+'\',[StringComparison]::OrdinalIgnoreCase)-or$inputRoot.StartsWith($OutputRoot.TrimEnd('\','/')+'\',[StringComparison]::OrdinalIgnoreCase)){throw 'Input/output overlap rejected'}
}
if($ValidationInstall){if($ValidationAppGuid-cnotmatch'^[0-9A-Fa-f]{8}(-[0-9A-Fa-f]{4}){3}-[0-9A-Fa-f]{12}$'){throw 'Explicit isolated validation GUID required'}}
elseif($ValidationAppGuid){throw 'Validation GUID requires isolated mode'}
if(!$PythonPath){$PythonPath=(Get-Command python -ErrorAction Stop).Source};$PythonPath=[IO.Path]::GetFullPath($PythonPath);Ordinary $PythonPath
if(!$InstallerSourceRevision){$InstallerSourceRevision=$EngineSourceRevision}
$recipeSourceRoot=Split-Path -Parent $project
$sourceBinding=InstallerSourceBinding $recipeSourceRoot $CandidateSourceRoot $GameSourceRevision $EngineSourceRevision $InstallerSourceRevision
$engineSourceRoot=$sourceBinding.engineSourceRoot
# Runtime verifiers stay on the exact engine checkout, even when a descendant
# changes only the three reviewed installation-recipe files.
$updaterScripts=Join-Path $engineSourceRoot 'mir2-web3/apps/game-client/windows-updater/scripts'
TrustedBinary $InnoCompiler $InnoCompilerSha256 'CN=Pyrsys B.V., O=Pyrsys B.V., S=Noord-Holland, C=NL'
TrustedBinary $RuntimeRedist $runtimePin 'CN=Microsoft Corporation, O=Microsoft Corporation, L=Redmond, S=Washington, C=US'
$pwsh=(Get-Process -Id $PID).Path
[IO.Directory]::CreateDirectory($OutputRoot)|Out-Null
$recipeSha256=Sha (Join-Path $PSScriptRoot 'Mir2-Invite.iss')
$builderSha256=Sha $PSCommandPath
$proof=[ordered]@{schema='mir2.windows.packed-installer-build.v1';passed=$false;candidate=$Candidate;gameSourceRevision=$GameSourceRevision;engineSourceRevision=$EngineSourceRevision;installerSourceRevision=$InstallerSourceRevision;recipeSha256=$recipeSha256;builderSha256=$builderSha256;installerSourceProof=$sourceBinding;manifestSha256=$ManifestSha256.ToUpperInvariant();installed=$false;published=$false;gameLaunched=$false;signingRequested=$false;installerCompiled=$false;stages=@()}
try{
    $gameReport=Join-Path $OutputRoot 'strict-candidate.json'
    $gameVerifier=Join-Path $CandidateSourceRoot 'mir2-web3/apps/game-client/platform-windows/scripts/verify-windows-candidate.ps1'
    Gate $pwsh @('-NoProfile','-File',$gameVerifier,'-PackageRoot',$CandidateRoot,'-TrustedSignerThumbprint',$TrustedSignerThumbprint,'-ContentProfile',$ContentProfile,'-RequireCurrentSource','-ReportPath',$gameReport) 'strict-candidate'
    $game=ReadJson $gameReport
    if(!$game.passed-or!$game.detachedSignatureValid-or!$game.nonvisual-or$game.launchRequested-or$game.currentSourceProof.clean-ne$true-or$game.currentSourceProof.revision-cne$GameSourceRevision){throw 'Fresh complete Candidate/source/CMS proof required'}
    GameKeyPin
    $proof.stages+=@{stage='full-candidate-and-pinned-cms';reportSha256=Sha $gameReport}
    $bundleReport=Join-Path $OutputRoot 'bundle-verification.json'
    Gate $pwsh @('-NoProfile','-File',(Join-Path $updaterScripts 'verify-updater-bundle.ps1'),'-BundleRoot',$BundleRoot,'-ReportPath',$bundleReport) 'bundle-cms'
    $bundle=ReadJson $bundleReport
    if(!$bundle.passed-or$bundle.sourceRevision-cne$EngineSourceRevision-or$bundle.gameSourceRevision-cne$GameSourceRevision-or$bundle.gameCandidate-cne$Candidate-or$bundle.signingKeySha256-cne$publicKeyPin){throw 'Exact signed updater binding required'}
    $proof.stages+=@{stage='bundle-cms';reportSha256=Sha $bundleReport}
    $packReport=Join-Path $OutputRoot 'packed-full-verification.json'
    Gate $pwsh @('-NoProfile','-File',(Join-Path $updaterScripts 'verify-packed-generation.ps1'),'-OutputRoot',$PackedRoot,'-ExpectedManifestSha256',$ManifestSha256,'-PackVerifierPath',$PackVerifierPath,'-PackVerifierSha256',$PackVerifierSha256,'-PythonPath',$PythonPath,'-ReportPath',$packReport) 'full-packed-cms'
    $pack=ReadJson $packReport
    if(!$pack.passed-or!$pack.cmsVerified-or!$pack.fullResourceVerified-or$pack.metadataOnly-or!$pack.sourceClean-or$pack.gameSourceRevision-cne$GameSourceRevision-or$pack.engineSourceRevision-cne$EngineSourceRevision-or$pack.manifestSha256-cne$ManifestSha256.ToUpperInvariant()){throw 'Exact full packed/CMS proof required'}
    $proof.stages+=@{stage='full-packed-cms';reportSha256=Sha $packReport}
    $installer=Join-Path $OutputRoot 'installer'
    $inputArgs=@('-B',(Join-Path $PSScriptRoot 'prepare-packed-installer-input.py'),$CandidateRoot,$BundleRoot,$PackedRoot,$PSScriptRoot,(Join-Path $PSScriptRoot '../player-readme'),$installer,'--game-source',$GameSourceRevision,'--engine-source',$EngineSourceRevision,'--candidate',$Candidate,'--manifest-sha',$ManifestSha256)
    if($ProgressiveBootstrap){$inputArgs+=@('--progressive-bootstrap')}
    Gate $PythonPath $inputArgs 'literal-packed-inputs'
    $inputReport=Join-Path $installer 'PACKED-INSTALLER-INPUTS.json'
    $inputs=ReadJson $inputReport
    if(!$inputs.passed-or$inputs.gameLooseExeIncluded-or$inputs.gameLooseAssetsIncluded-or$inputs.selectorOrHealthIncluded-or$inputs.resourceCount-ne$pack.resourceCount){throw 'Packed literal layout rejected'}
    $modeField=$inputs.PSObject.Properties['progressiveBootstrap']
    if($modeField){
        if($inputs.progressiveBootstrap-isnot[bool]-or$inputs.progressiveBootstrap-ne$ProgressiveBootstrap.IsPresent-or$inputs.generationSchema-cne'mir2.windows.generation.v2'-or!$inputs.sourceFullResourceByteVerified){throw 'Explicit v2 bootstrap mode binding rejected'}
        if($inputs.fullResourceCount-ne$pack.resourceCount-or$inputs.requiredResourceCount-le0-or$inputs.requiredResourceCount-gt$inputs.fullResourceCount-or$inputs.fullPackCount-ne$pack.packCount-or$inputs.fullPackBytes-ne$pack.packBytes-or$inputs.packCount-ne$inputs.installedPackCount-or$inputs.requiredPackCount-le0-or$inputs.requiredPackCount-gt$inputs.fullPackCount-or$inputs.requiredPackBytes-le0-or$inputs.requiredPackBytes-gt$inputs.fullPackBytes){throw 'Full/required pack accounting rejected'}
        if($inputs.deferredPackCount-ne($pack.packCount-$inputs.installedPackCount)-or$inputs.deferredPackBytes-ne($pack.packBytes-$inputs.installedPackBytes)){throw 'Deferred pack accounting rejected'}
        if($ProgressiveBootstrap-and($inputs.installedPackCount-ne$inputs.requiredPackCount-or$inputs.installedPackBytes-ne$inputs.requiredPackBytes)){throw 'Bootstrap must contain exactly required packs'}
    }elseif($ProgressiveBootstrap){throw 'Progressive bootstrap requires authenticated v2 scope inputs'}
    if(!$ProgressiveBootstrap-and$inputs.packCount-ne$pack.packCount){throw 'Default full pack closure rejected'}
    $proof.progressiveBootstrap=$ProgressiveBootstrap.IsPresent
    $proof.fullPackCount=$pack.packCount;$proof.fullPackBytes=$pack.packBytes
    $proof.installedPackCount=$inputs.packCount
    $proof.requiredPackCount=if($modeField){$inputs.requiredPackCount}else{$pack.packCount}
    $proof.requiredPackBytes=if($modeField){$inputs.requiredPackBytes}else{$pack.packBytes}
    $proof.installedPackBytes=if($modeField){$inputs.installedPackBytes}else{$pack.packBytes}
    $proof.backgroundCompletionRequired=$ProgressiveBootstrap.IsPresent-and($inputs.packCount-lt$pack.packCount)
    $proof.allResourcePacksIncluded=$inputs.packCount-eq$pack.packCount
    $proof.gameplayVerified=$false
    # Authenticate the copied bytes as well as the original release inputs.
    # A passed JSON flag cannot authorize changed signatures between gate/copy.
    $copiedPairCount=VerifyCopiedCms $installer
    $proof.stages+=@{stage='copied-pinned-cms';pairs=$copiedPairCount;inputsSha256=Sha $inputReport}
    [IO.Directory]::CreateDirectory((Join-Path $installer 'tools'))|Out-Null
    Copy-Item -LiteralPath $RuntimeRedist -Destination (Join-Path $installer 'tools/vc_redist.x64.exe')
    TrustedBinary (Join-Path $installer 'tools/vc_redist.x64.exe') $runtimePin 'CN=Microsoft Corporation, O=Microsoft Corporation, L=Redmond, S=Washington, C=US'
    $icon=Join-Path $project 'apps/game-client/platform-windows/resources/numeron.ico';Ordinary $icon
    Copy-Item -LiteralPath $icon -Destination (Join-Path $installer 'numeron.ico')
    Gate $PythonPath @('-B',(Join-Path $installer 'validate-recipe.py'),(Split-Path -Parent $InnoCompiler)) 'ten-languages'
    $proof.inputsSha256=Sha $inputReport
    if(!$PrepareOnly){
        $compilerArgs=@('/Qp',('/O'+$OutputRoot),('/DAppVersion='+$SetupVersion),('/DMir2ReleaseBaseName='+$OutputBaseName),'/DMir2Packed=1',('/DMir2IconFile='+(Join-Path $installer 'numeron.ico')))
        if($ValidationInstall){$compilerArgs+=@('/DMir2ValidationInstall=1',('/DMir2ValidationAppGuid='+$ValidationAppGuid))}
        Gate $InnoCompiler ($compilerArgs+@((Join-Path $installer 'Mir2-Invite.iss'))) 'inno-packed-compile'
        $suffix=if($ValidationInstall){'-LocalValidation-Setup.exe'}else{'-Setup.exe'}
        $setup=Join-Path $OutputRoot ($OutputBaseName+$suffix);Ordinary $setup
        $version=(Get-Item -LiteralPath $setup).VersionInfo
        $numericVersion=@($version.FileMajorPart,$version.FileMinorPart,$version.FileBuildPart,$version.FilePrivatePart)-join'.'
        if($version.FileVersion.Trim()-cne$SetupVersion-or$numericVersion-cne$SetupVersion){throw 'Exact installer version mismatch'}
        $proof.installer=@{path=$setup;bytes=(Get-Item -LiteralPath $setup).Length;sha256=Sha $setup;authenticode=(Get-AuthenticodeSignature -LiteralPath $setup).Status.ToString()}
        $proof.installerCompiled=$true
    }
    CleanSource $CandidateSourceRoot $GameSourceRevision
    CleanSource $engineSourceRoot $EngineSourceRevision
    CleanSource $recipeSourceRoot $InstallerSourceRevision
    foreach($file in $sourceBinding.recipeFiles){if((Sha (Join-Path $recipeSourceRoot $file.path))-cne$file.sha256){throw 'Installer recipe bytes changed during build'}}
    $proof.passed=$true
}catch{$proof.failure=$_.Exception.Message;throw}
finally{[IO.File]::WriteAllText((Join-Path $OutputRoot 'BUILD-PACKED-INSTALLER.json'),($proof|ConvertTo-Json -Depth 8),[Text.UTF8Encoding]::new($false))}
$proof|ConvertTo-Json -Depth 8
