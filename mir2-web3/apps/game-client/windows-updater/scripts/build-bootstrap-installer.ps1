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
    [string]$PythonPath,
    [ValidateSet('platinum_176_v27','platinum_176','platinum_fiveclass')][string]$ContentProfile = 'platinum_176_v27'
)
$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
$ContentProfile=$ContentProfile.ToLowerInvariant()
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
function CopyCanonicalVerifierSource([string]$PlatformScripts,[string]$ProofRoot) {
    # Read only constant pin dictionaries; never run the verifier entry point.
    # Its pinned-font scan needs the original complete font bytes beside it.
    $verifier=Join-Path $PlatformScripts 'verify-windows-candidate.ps1'
    NoLinks $verifier
    $tokens=$null;$parseErrors=$null
    $ast=[Management.Automation.Language.Parser]::ParseFile($verifier,[ref]$tokens,[ref]$parseErrors)
    if($parseErrors.Count-ne0){throw 'Canonical verifier has parse errors'}
    $definitions=@($ast.FindAll({param($node)
        $node-is[Management.Automation.Language.FunctionDefinitionAst] -and
        $node.Name-ceq'Get-PinnedFontScanDefinitions'
    },$false))
    if($definitions.Count-ne1){throw 'Canonical font pin definition missing or ambiguous'}
    $pins=@($definitions[0].Body.FindAll({param($node)
        $node-is[Management.Automation.Language.HashtableAst]
    },$false)|ForEach-Object {$_.SafeGetValue()})
    $expectedFonts=@('NotoSansTC.ttf','NotoSans-Regular.ttf','NotoSans-Bold.ttf',
        'NotoSansDevanagari-Regular.ttf','NotoSansDevanagari-Bold.ttf',
        'NotoSansThai-Regular.ttf','NotoSansThai-Bold.ttf',
        'NotoSansArabic-Regular.ttf','NotoSansArabic-Bold.ttf')
    if($pins.Count-ne$expectedFonts.Count){throw 'Exactly nine canonical font pins required'}
    $seen=[Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    $fontRoot=Join-Path (Split-Path -Parent $PlatformScripts) 'assets/fonts'
    $fontSources=@()
    foreach($pin in $pins){
        if($expectedFonts-cnotcontains$pin.file -or !$seen.Add($pin.file) -or
           $pin.length-isnot[int] -or $pin.length-le0 -or
           ![regex]::IsMatch([string]$pin.sha256,'\A[0-9A-F]{64}\z')){
            throw 'Canonical font pin shape/name mismatch'
        }
        $source=Join-Path $fontRoot $pin.file
        NoLinks $source
        $item=Get-Item -LiteralPath $source -Force
        if($item.PSIsContainer -or $item.LinkType-eq'HardLink' -or
           $item.Length-ne$pin.length -or (FileSha $source)-cne$pin.sha256){
            throw ('Canonical pinned font source mismatch: '+$pin.file)
        }
        $fontSources+=@{source=$source;pin=$pin}
    }
    $verificationRoot=Join-Path $ProofRoot 'verification-source'
    $verificationScripts=Join-Path $verificationRoot 'apps/game-client/platform-windows/scripts'
    $verificationFonts=Join-Path $verificationRoot 'apps/game-client/platform-windows/assets/fonts'
    [IO.Directory]::CreateDirectory($verificationScripts)|Out-Null
    [IO.Directory]::CreateDirectory($verificationFonts)|Out-Null
    [IO.Directory]::CreateDirectory((Join-Path $verificationRoot 'docs'))|Out-Null
    [IO.File]::WriteAllText((Join-Path $verificationRoot 'Cargo.toml'),'# Verification marker only; never compiled.')
    $closure=@()
    foreach($name in @('verify-windows-candidate.ps1','entity-atlas-closure.ps1',
        'player-sprite-closure.ps1','candidate-release-profile.ps1','pet-guild-asset-closure.ps1')){
        $source=Join-Path $PlatformScripts $name
        NoLinks $source
        $destination=Join-Path $verificationScripts $name
        Copy-Item -LiteralPath $source -Destination $destination
        if((FileSha $source)-cne(FileSha $destination)){throw 'Verifier copy identity mismatch'}
        $closure+=@{path=('apps/game-client/platform-windows/scripts/'+$name);sha256=FileSha $destination}
    }
    foreach($font in $fontSources){
        $destination=Join-Path $verificationFonts $font.pin.file
        Copy-Item -LiteralPath $font.source -Destination $destination
        if((Get-Item -LiteralPath $destination).Length-ne$font.pin.length -or
           (FileSha $destination)-cne$font.pin.sha256){throw 'Pinned verifier font copy identity mismatch'}
        $closure+=@{path=('apps/game-client/platform-windows/assets/fonts/'+$font.pin.file)
            sizeBytes=$font.pin.length;sha256=FileSha $destination}
    }
    SaveJson (Join-Path $ProofRoot 'canonical-verifier-source-closure.json') $closure
    return $verificationRoot
}

function AssertBootstrapFiveClassReport {
    param([Parameter(Mandatory = $true)][object]$Report,
          [Parameter(Mandatory = $true)][string]$ProjectRoot,
          [Parameter(Mandatory = $true)][string]$GameRoot,
          [Parameter(Mandatory = $true)][string]$GameCandidate,
          [Parameter(Mandatory = $true)][string]$Revision,
          [Parameter(Mandatory = $true)][string]$SignerThumbprint,
          [Parameter(Mandatory = $true)][string]$VerifierSha256,
          [Parameter(Mandatory = $true)][string]$ProfileSha256)
    foreach ($name in @('passed','detachedSignatureValid','nonvisual','attestationPresent','packageManifestPresent',
                        'contentProfileSourceAvailable','requireCurrentSource','currentSourceValidated')) {
        if ($Report.$name -isnot [bool] -or $Report.$name -ne $true) {
            throw "Five-class canonical report did not prove $name"
        }
    }
    if ($Report.launchRequested -isnot [bool] -or $Report.launchRequested -ne $false -or
        $Report.schema -cne 'mir2.windows.package-verification.v4' -or
        $Report.candidate -cne $GameCandidate -or @($Report.failures).Count -ne 0 -or
        $Report.contentProfile -cne 'platinum_fiveclass' -or $Report.contentProfileId -cne 'platinum_fiveclass' -or
        $Report.contentProfileSha256 -cne $ProfileSha256 -or $Report.verifierSha256 -cne $VerifierSha256 -or
        $Report.sourceRepoCheck -cne 'checked' -or $Report.currentSourceRevision -cne $Revision -or
        $Report.trustedSignerThumbprint -cne $SignerThumbprint.ToUpperInvariant()) {
        throw 'Five-class canonical report profile/source/CMS binding failed'
    }
    if (-not [IO.Path]::GetFullPath([string]$Report.packageRoot).Equals([IO.Path]::GetFullPath($GameRoot), [StringComparison]::OrdinalIgnoreCase) -or
        -not [IO.Path]::GetFullPath([string]$Report.currentSourceRoot).Equals([IO.Path]::GetFullPath($ProjectRoot), [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Five-class canonical report belongs to another Source or Candidate'
    }
    $profilePath = Join-Path $ProjectRoot 'packages/game-data/data/content_profiles/platinum_fiveclass.json'
    $verifierPath = Join-Path $ProjectRoot 'apps/game-client/platform-windows/scripts/verify-windows-candidate.ps1'
    if ((FileSha $profilePath) -cne $ProfileSha256 -or (FileSha $verifierPath) -cne $VerifierSha256 -or
        $Report.attestationSha256 -cne (FileSha (Join-Path $GameRoot 'BUILD-ATTESTATION.json')) -or
        $Report.packageManifestSha256 -cne (FileSha (Join-Path $GameRoot 'PACKAGE-MANIFEST.json')) -or
        $Report.exeSha256 -cne (FileSha (Join-Path $GameRoot 'mir2-platform-windows.exe'))) {
        throw 'Five-class canonical verification inputs changed or report is stale'
    }
    $attestation = ReadJson (Join-Path $GameRoot 'BUILD-ATTESTATION.json')
    $state = $Report.currentSourceProof
    if ($attestation.schema -cne 'mir2.windows.build-attestation.v2' -or
        $attestation.gitRevision.ToLowerInvariant() -cne $Revision -or
        $attestation.worktreeDirty -isnot [bool] -or $attestation.worktreeDirty -ne $false -or
        $state.clean -isnot [bool] -or $state.clean -ne $true -or $state.revision -cne $Revision -or
        $state.statusScope -cne 'git-status-z+diff+all-untracked-content-v2' -or
        [int]$state.statusLineCount -ne 0 -or
        $state.statusSha256 -cne $attestation.worktreeStatusSha256.ToUpperInvariant() -or
        $state.statusScope -cne $attestation.worktreeStatusScope -or
        [int]$attestation.worktreeStatusLineCount -ne 0) {
        throw 'Five-class canonical report lacks exact clean Source/attestation proof'
    }
    $closure = $Report.fiveClassClosure
    if ($closure.schema -cne 'mir2.windows.five-class-closure-evidence.v1' -or
        $closure.profileId -cne 'platinum_fiveclass' -or $closure.profileSha256 -cne $ProfileSha256) {
        throw 'Five-class actual closure evidence is absent or belongs to another profile'
    }
    foreach ($name in @('sourceSoundValidated','stagedSoundValidated','soundIndexExactCopy',
                        'sourceSpriteAndSkillValidated','stagedSpriteAndSkillValidated')) {
        if ($closure.$name -isnot [bool] -or $closure.$name -ne $true) {
            throw "Five-class canonical report did not complete $name"
        }
    }
    $sourceAssets = [IO.Path]::GetFullPath((Join-Path $ProjectRoot 'apps/web/public'))
    $stagedAssets = [IO.Path]::GetFullPath((Join-Path $GameRoot 'mir2-assets'))
    if (-not [IO.Path]::GetFullPath([string]$closure.sourceAssetRoot).Equals($sourceAssets, [StringComparison]::OrdinalIgnoreCase) -or
        -not [IO.Path]::GetFullPath([string]$closure.stagedAssetRoot).Equals($stagedAssets, [StringComparison]::OrdinalIgnoreCase) -or
        $closure.soundIndexRelativePath -cne 'original-ui/sound-index.generated.json' -or
        [int]$closure.availableSoundCount -le 0 -or [int]$closure.absentSoundCount -lt 0) {
        throw 'Five-class closure does not bind actual Source and staged assets'
    }
    $sourceIndex = Join-Path $sourceAssets $closure.soundIndexRelativePath
    $stagedIndex = Join-Path $stagedAssets $closure.soundIndexRelativePath
    if ($closure.soundIndexSha256 -cne (FileSha $sourceIndex) -or
        $closure.soundIndexSha256 -cne (FileSha $stagedIndex) -or
        [int64]$closure.soundIndexBytes -ne (Get-Item -LiteralPath $sourceIndex).Length -or
        [int64]$closure.soundIndexBytes -ne (Get-Item -LiteralPath $stagedIndex).Length) {
        throw 'Five-class verified sound index changed'
    }
    $requirements = @($closure.requirements)
    if ($requirements.Count -ne 2 -or @($requirements.kind | Sort-Object -Unique).Count -ne 2) {
        throw 'Five-class closure requirements proof is incomplete'
    }
    foreach ($kind in @('sprite','skill')) {
        $item = @($requirements | Where-Object { $_.kind -ceq $kind })
        $path = Join-Path $ProjectRoot "apps/web/scripts/asset-pipeline/five-class-$kind-requirements.json"
        if ($item.Count -ne 1 -or $item[0].sha256 -cne (FileSha $path)) {
            throw 'Five-class verified requirements changed or are incomplete'
        }
    }
}

function InvokeBootstrapGameVerification {
    param([Parameter(Mandatory = $true)][string]$ProjectRoot,
          [Parameter(Mandatory = $true)][string]$GameRoot,
          [Parameter(Mandatory = $true)][string]$ProofRoot,
          [Parameter(Mandatory = $true)][string]$GameCandidate,
          [Parameter(Mandatory = $true)][string]$Revision,
          [Parameter(Mandatory = $true)][string]$SignerThumbprint,
          [Parameter(Mandatory = $true)][string]$PowerShellPath,
          [ValidateSet('platinum_176_v27','platinum_176','platinum_fiveclass')][string]$Profile = 'platinum_176_v27')
    $Profile = $Profile.ToLowerInvariant()
    $sourceScripts = Join-Path $ProjectRoot 'apps/game-client/platform-windows/scripts'
    if ($Profile -ceq 'platinum_fiveclass') {
        # Keep the actual clean Source with its existing Node dependencies and
        # genuine exports; a fonts-only marker cannot attest five-class Source.
        $verifier = Join-Path $sourceScripts 'verify-windows-candidate.ps1'
        $profilePath = Join-Path $ProjectRoot 'packages/game-data/data/content_profiles/platinum_fiveclass.json'
        NoLinks $verifier; NoLinks $profilePath
        if ((ReadJson $profilePath).profileId -cne 'platinum_fiveclass') { throw 'Actual five-class Source profile missing' }
        $verifierHash = FileSha $verifier; $profileHash = FileSha $profilePath
        $reportPath = Join-Path $ProofRoot 'canonical-five-class-verification.json'
        if (Test-Path -LiteralPath $reportPath) { throw 'Canonical report must be fresh' }
        InvokeLogged $PowerShellPath @('-NoProfile','-File',$verifier,'-PackageRoot',$GameRoot,
            '-TrustedSignerThumbprint',$SignerThumbprint,'-ContentProfile','platinum_fiveclass',
            '-RequireCurrentSource','-ReportPath',$reportPath) 'strict-game-verification'
        NoLinks $reportPath
        $report = ReadJson $reportPath
        AssertBootstrapFiveClassReport -Report $report -ProjectRoot $ProjectRoot -GameRoot $GameRoot `
            -GameCandidate $GameCandidate -Revision $Revision -SignerThumbprint $SignerThumbprint `
            -VerifierSha256 $verifierHash -ProfileSha256 $profileHash
    } else {
        # Preserve the original classic helper/font copy and nonvisual gates.
        $verificationRoot = CopyCanonicalVerifierSource $sourceScripts $ProofRoot
        $verifier = Join-Path $verificationRoot 'apps/game-client/platform-windows/scripts/verify-windows-candidate.ps1'
        InvokeLogged $PowerShellPath @('-NoProfile','-File',$verifier,'-PackageRoot',$GameRoot,
            '-TrustedSignerThumbprint',$SignerThumbprint,'-ContentProfile',$Profile) 'strict-game-verification'
        $reportPath = Join-Path $verificationRoot ('docs/generated/player-qa/windows-package-preflight/'+$GameCandidate+'-verification.json')
        $report = ReadJson $reportPath
        if (!$report.passed -or !$report.detachedSignatureValid -or !$report.nonvisual -or $report.launchRequested) {
            throw 'Strict full-package/CMS verification did not pass'
        }
        $verifierHash = FileSha $verifier
        $profileHash = ''
    }
    return [pscustomobject]@{ report = $report; reportPath = $reportPath; reportSha256 = FileSha $reportPath
        verifierSha256 = $verifierHash; contentProfile = $Profile; contentProfileSha256 = $profileHash }
}

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
    gameSourceRevision=$SourceRevision;contentProfile=$ContentProfile;createdUtc=[DateTimeOffset]::UtcNow.ToString('o')
    gameLaunched=$false;installed=$false;published=$false;signingRequested=$false
    fullOfflineInstallerPreserved=$true;runtimeIncluded=$true;runtimeSha256=$runtimePin
    compilerSha256=$InnoCompilerSha256.ToUpperInvariant();stages=@()}
try {
    $gameVerification = InvokeBootstrapGameVerification -ProjectRoot $project -GameRoot $CandidateRoot `
        -ProofRoot $OutputRoot -GameCandidate $Candidate -Revision $SourceRevision `
        -SignerThumbprint $TrustedSignerThumbprint -PowerShellPath $powerShell -Profile $ContentProfile
    $report = $gameVerification.report
    $proof.gameVerification = $gameVerification
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
