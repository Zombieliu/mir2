#Requires -Version 7.2
<#
Build an online installer from immutable, already signed game/updater releases.
The complete Candidate is strictly verified before generating seed-only inputs.
No key selection, signing, publication, game launch, installation, or OS changes.
The full offline installer recipe remains available without Mir2Bootstrap.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$CandidateRoot,
    [Parameter(Mandatory)][string]$BundleRoot,
    [Parameter(Mandatory)][string]$OutputRoot,
    [Parameter(Mandatory)][ValidatePattern('^[0-9a-f]{40}$')][string]$SourceRevision,
    [Parameter(Mandatory)][ValidatePattern('^WN-CANDIDATE-[A-Za-z0-9._-]+$')][string]$Candidate,
    [Parameter(Mandatory)][ValidatePattern('^[0-9A-Fa-f]{40}$')][string]$TrustedSignerThumbprint,
    [Parameter(Mandatory)][ValidatePattern('^[0-9]{4}\.[0-9]{1,2}\.[0-9]{1,2}\.[0-9]{1,4}$')][string]$SetupVersion,
    [Parameter(Mandatory)][ValidatePattern('^[A-Za-z0-9._-]+-Bootstrap$')][string]$OutputBaseName,
    [Parameter(Mandatory)][string]$RuntimeRedist,
    [Parameter(Mandatory)][string]$InnoCompiler,
    [Parameter(Mandatory)][ValidatePattern('^[0-9A-Fa-f]{64}$')][string]$InnoCompilerSha256,
    [string]$PythonPath
)
$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
$project=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../../..'))
$platformScripts=Join-Path $project 'apps/game-client/platform-windows/scripts'
$recipeRoot=Join-Path $platformScripts 'invited-installer'
$guideRoot=Join-Path $platformScripts 'player-readme'
$publicKeyPin='6C70C777B27D50949370D494B4B25798200FBDD5C171BAEEAA94D7E289900F3E'
$runtimePin='843068991DAAA1F73AD9F6239BCE4D0F6A07A51F18C37EA2A867E9BECA71295C'

function NoLinks([string]$Path) {
    $cursor=[IO.Path]::GetFullPath($Path)
    if($cursor.StartsWith('\\')){throw 'UNC build inputs/outputs are unsupported'}
    while($cursor) {
        if(Test-Path -LiteralPath $cursor){
            if(((Get-Item -LiteralPath $cursor -Force).Attributes-band[IO.FileAttributes]::ReparsePoint)-ne0){
                throw 'Linked/reparse build path rejected'
            }
        }
        $parent=Split-Path -Parent $cursor
        if(!$parent -or $parent-eq$cursor){break}
        $cursor=$parent
    }
}
function FileSha([string]$Path) { (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToUpperInvariant() }
function SaveJson([string]$Path,$Value) {
    if(Test-Path -LiteralPath $Path){throw 'Refusing to overwrite a proof file'}
    [IO.File]::WriteAllText($Path,($Value|ConvertTo-Json -Depth 20),[Text.UTF8Encoding]::new($false))
}
function ReadJson([string]$Path) { Get-Content -LiteralPath $Path -Raw|ConvertFrom-Json }
function AssertBinary([string]$Path,[string]$Digest,[string]$Publisher) {
    NoLinks $Path
    $file=Get-Item -LiteralPath $Path -Force
    if($file.PSIsContainer -or $file.LinkType-eq'HardLink'){throw 'Ordinary non-hardlinked binary input required'}
    foreach($stream in @(Get-Item -LiteralPath $Path -Stream *)){
        if($stream.Stream-cne':$DATA'){throw 'Named binary data stream rejected'}
    }
    if((FileSha $Path)-cne$Digest.ToUpperInvariant()){throw 'Trusted binary SHA256 mismatch'}
    $signature=Get-AuthenticodeSignature -LiteralPath $Path
    if($signature.Status-ne'Valid' -or !$signature.SignerCertificate -or
       $signature.SignerCertificate.Subject-cne$Publisher){throw 'Trusted binary publisher signature mismatch'}
}
function InvokeLogged([string]$Executable,[string[]]$Arguments,[string]$Name) {
    $log=Join-Path $OutputRoot ($Name+'.log')
    if(Test-Path -LiteralPath $log){throw 'Refusing to overwrite a build log'}
    & $Executable @Arguments *> $log
    $exitCode=$LASTEXITCODE
    if($exitCode-ne0){throw "$Name failed ($exitCode); see retained log $log"}
}
function AssertPinnedGameCms([string]$Root) {
    Add-Type -AssemblyName System.Security.Cryptography.Pkcs
    $bytes=[IO.File]::ReadAllBytes((Join-Path $Root 'RELEASE-STATEMENT.json'))
    $signature=[IO.File]::ReadAllBytes((Join-Path $Root 'RELEASE-STATEMENT.p7s'))
    $envelope=[Security.Cryptography.Pkcs.SignedCms]::new()
    $envelope.Decode($signature)
    if($envelope.ContentInfo.Content.Length-ne0){throw 'Detached game CMS signature required'}
    $cms=[Security.Cryptography.Pkcs.SignedCms]::new([Security.Cryptography.Pkcs.ContentInfo]::new($bytes),$true)
    $cms.Decode($signature)
    if($cms.SignerInfos.Count-ne1 -or
       $cms.SignerInfos[0].DigestAlgorithm.Value-cne'2.16.840.1.101.3.4.2.1'){
        throw 'Exactly one SHA256 game CMS signer required'
    }
    $cms.CheckSignature($true)
    $certificate=$cms.SignerInfos[0].Certificate
    $eku=$certificate.Extensions | Where-Object {$_.Oid.Value-eq'2.5.29.37'} | Select-Object -First 1
    $usage=$certificate.Extensions | Where-Object {$_.Oid.Value-eq'2.5.29.15'} | Select-Object -First 1
    $now=[DateTime]::UtcNow
    if(!$certificate -or $certificate.Thumbprint-cne$TrustedSignerThumbprint.ToUpperInvariant() -or
       $certificate.NotBefore.ToUniversalTime()-gt$now -or $certificate.NotAfter.ToUniversalTime()-lt$now -or
       !$eku -or @($eku.EnhancedKeyUsages|ForEach-Object {$_.Value})-notcontains'1.3.6.1.5.5.7.3.3' -or
       ($usage -and ($usage.KeyUsages-band[Security.Cryptography.X509Certificates.X509KeyUsageFlags]::DigitalSignature)-eq0)){
        throw 'Game CMS signer identity/time/usage mismatch'
    }
    $rsa=[Security.Cryptography.X509Certificates.RSACertificateExtensions]::GetRSAPublicKey($certificate)
    if(!$rsa -or [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($rsa.ExportRSAPublicKey()))-cne$publicKeyPin){
        throw 'Game CMS key does not match the launcher trust pin'
    }
}

foreach($name in @('CandidateRoot','BundleRoot','OutputRoot','RuntimeRedist','InnoCompiler')){
    $value=[IO.Path]::GetFullPath((Get-Variable -Name $name -ValueOnly))
    NoLinks $value
    if($value.IndexOfAny([char[]]'"{}')-ge0 -or $value.Contains("`r") -or $value.Contains("`n")){
        throw 'Unsafe Inno build path'
    }
    Set-Variable -Name $name -Value $value
}
if(Test-Path -LiteralPath $OutputRoot){throw 'Fresh external output directory required'}
if(!(Test-Path -LiteralPath (Split-Path -Parent $OutputRoot) -PathType Container)){
    throw 'Output parent directory must exist'
}
foreach($inputRoot in @($CandidateRoot,$BundleRoot,$recipeRoot,$guideRoot)){
    if($OutputRoot.StartsWith($inputRoot.TrimEnd('\','/')+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)){
        throw 'Output must not be inside an input directory'
    }
}
if(!$PythonPath){$PythonPath=(Get-Command python -ErrorAction Stop).Source}
$PythonPath=[IO.Path]::GetFullPath($PythonPath);NoLinks $PythonPath
$powerShell=(Get-Process -Id $PID).Path
AssertBinary $InnoCompiler $InnoCompilerSha256 'CN=Pyrsys B.V., O=Pyrsys B.V., S=Noord-Holland, C=NL'
AssertBinary $RuntimeRedist $runtimePin 'CN=Microsoft Corporation, O=Microsoft Corporation, L=Redmond, S=Washington, C=US'
$version=ReadJson (Join-Path $CandidateRoot 'VERSION.json')
if($version.candidate-cne$Candidate -or $version.gitRevision-cne$SourceRevision -or
   $version.worktreeDirty-ne$false -or $version.clientOnly-ne$true){throw 'Exact clean client-only Candidate required'}

[IO.Directory]::CreateDirectory($OutputRoot)|Out-Null
$proof=[ordered]@{schema='mir2.windows.bootstrap-build.v1';passed=$false;candidate=$Candidate
    gameSourceRevision=$SourceRevision;createdUtc=[DateTimeOffset]::UtcNow.ToString('o')
    gameLaunched=$false;installed=$false;published=$false;signingRequested=$false
    fullOfflineInstallerPreserved=$true;runtimeIncluded=$true;runtimeSha256=$runtimePin
    compilerSha256=$InnoCompilerSha256.ToUpperInvariant();stages=@()}
try {
    # Copy the unchanged canonical verifier and its exact helper closure to an
    # external marker project. Its default evidence writer stays in this proof
    # tree; no repository/global status document or original report is touched.
    $verificationRoot=Join-Path $OutputRoot 'verification-source'
    $verificationScripts=Join-Path $verificationRoot 'apps/game-client/platform-windows/scripts'
    [IO.Directory]::CreateDirectory($verificationScripts)|Out-Null
    [IO.Directory]::CreateDirectory((Join-Path $verificationRoot 'docs'))|Out-Null
    [IO.File]::WriteAllText((Join-Path $verificationRoot 'Cargo.toml'),'# Verification marker only; never compiled.')
    $verifierNames=@('verify-windows-candidate.ps1','entity-atlas-closure.ps1',
        'player-sprite-closure.ps1','candidate-release-profile.ps1','pet-guild-asset-closure.ps1')
    $verifierClosure=@()
    foreach($name in $verifierNames){
        $source=Join-Path $platformScripts $name
        NoLinks $source
        $destination=Join-Path $verificationScripts $name
        Copy-Item -LiteralPath $source -Destination $destination
        if((FileSha $source)-cne(FileSha $destination)){throw 'Verifier copy identity mismatch'}
        $verifierClosure+=@{path=$name;sha256=FileSha $source}
    }
    SaveJson (Join-Path $OutputRoot 'canonical-verifier-source-closure.json') $verifierClosure
    InvokeLogged $powerShell @('-NoProfile','-File',(Join-Path $verificationScripts 'verify-windows-candidate.ps1'),
        '-PackageRoot',$CandidateRoot,'-TrustedSignerThumbprint',$TrustedSignerThumbprint) 'strict-game-verification'
    $report=ReadJson (Join-Path $verificationRoot ('docs/generated/player-qa/windows-package-preflight/'+$Candidate+'-verification.json'))
    if(!$report.passed -or !$report.detachedSignatureValid -or !$report.nonvisual -or $report.launchRequested){
        throw 'Strict full-package/CMS verification did not pass'
    }
    AssertPinnedGameCms $CandidateRoot
    $proof.stages+=@{stage='strict-game-and-pinned-cms';passed=$true;fileCount=$report.packageFileCount
        manifestSha256=$report.packageManifestSha256;sourceRepoCheck=$report.sourceRepoCheck}
    InvokeLogged $powerShell @('-NoProfile','-File',(Join-Path $PSScriptRoot 'verify-updater-bundle.ps1'),
        '-BundleRoot',$BundleRoot,'-ReportPath',(Join-Path $OutputRoot 'updater-bundle-verification.json')) 'updater-bundle-verification'
    $bundleReport=ReadJson (Join-Path $OutputRoot 'updater-bundle-verification.json')
    if(!$bundleReport.passed -or $bundleReport.gameCandidate-cne$Candidate -or
       $bundleReport.gameSourceRevision-cne$SourceRevision -or $bundleReport.signingKeySha256-cne$publicKeyPin){
        throw 'Updater bundle/CMS exact game binding did not pass'
    }
    $proof.stages+=@{stage='updater-bundle-cms';passed=$true;engineSha256=$bundleReport.engineSha256}
    $installerRoot=Join-Path $OutputRoot 'installer'
    InvokeLogged $PythonPath @((Join-Path $PSScriptRoot 'prepare-bootstrap-installer.py'),$CandidateRoot,
        $BundleRoot,$recipeRoot,$guideRoot,$installerRoot,'--expected-source',$SourceRevision,
        '--expected-candidate',$Candidate) 'literal-bootstrap-inputs'
    $inputs=ReadJson (Join-Path $installerRoot 'BOOTSTRAP-INPUTS.json')
    if(!$inputs.passed -or $inputs.gamePayloadIncluded -or $inputs.gameAssetsIncluded -or
       $inputs.gameBuildAttestationIncluded){throw 'Bootstrap literal input closure failed'}
    $runtimeOutput=Join-Path $installerRoot 'tools/vc_redist.x64.exe'
    [IO.Directory]::CreateDirectory((Split-Path -Parent $runtimeOutput))|Out-Null
    Copy-Item -LiteralPath $RuntimeRedist -Destination $runtimeOutput
    AssertBinary $runtimeOutput $runtimePin 'CN=Microsoft Corporation, O=Microsoft Corporation, L=Redmond, S=Washington, C=US'
    # Version strings are output labels only. The supplied game/seed bytes stay
    # immutable and independently authenticated; no gameplay is rebuilt here.
    $recipePath=Join-Path $installerRoot 'Mir2-Invite.iss'
    $recipe=[IO.File]::ReadAllText($recipePath)
    if(@([regex]::Matches($recipe,'(?m)^#define AppVersion "[^"]+"\r?$')).Count-ne1 -or
       @([regex]::Matches($recipe,'(?m)^VersionInfoVersion=[^\r\n]+\r?$')).Count-ne1){throw 'Recipe version anchors changed'}
    $recipe=[regex]::Replace($recipe,'(?m)^#define AppVersion "[^"]+"\r?$',('#define AppVersion "'+$SetupVersion+'"'))
    $recipe=[regex]::Replace($recipe,'(?m)^VersionInfoVersion=[^\r\n]+\r?$',('VersionInfoVersion='+$SetupVersion))
    [IO.File]::WriteAllText($recipePath,$recipe,[Text.UTF8Encoding]::new($true))
    InvokeLogged $PythonPath @((Join-Path $installerRoot 'validate-recipe.py'),(Split-Path -Parent $InnoCompiler)) 'nine-language-validation'
    $proof.stages+=@{stage='literal-inputs-and-nine-languages';passed=$true;installedFiles=$inputs.installedFileCount
        installedBytes=$inputs.installedBytes;fullPackageFiles=$inputs.fullPackageFileCount}
    $artifacts=Join-Path $OutputRoot 'artifacts'
    [IO.Directory]::CreateDirectory($artifacts)|Out-Null
    InvokeLogged $InnoCompiler @('/Qp',('/O'+$artifacts),('/F'+$OutputBaseName),'/DMir2Bootstrap=1',$recipePath) 'inno-bootstrap-compile'
    $setup=Join-Path $artifacts ($OutputBaseName+'.exe')
    $setupItem=Get-Item -LiteralPath $setup
    $proof.stages+=@{stage='inno-compile';passed=$true}
    $proof.installer=@{path=$setup;sizeBytes=$setupItem.Length;sha256=FileSha $setup
        publicAuthenticode=(Get-AuthenticodeSignature -LiteralPath $setup).Status.ToString()}
    $proof.bootstrapInputsSha256=FileSha (Join-Path $installerRoot 'BOOTSTRAP-INPUTS.json')
    $proof.recipeSha256=FileSha $recipePath
    $proof.passed=$true
} catch {
    $proof.failure=$_.Exception.Message
    throw
} finally {
    SaveJson (Join-Path $OutputRoot 'BUILD-BOOTSTRAP.json') $proof
}
$proof|ConvertTo-Json -Depth 12
