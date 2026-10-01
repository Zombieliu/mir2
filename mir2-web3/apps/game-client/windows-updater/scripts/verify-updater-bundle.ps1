#Requires -Version 7.2
[CmdletBinding()]
param([Parameter(Mandatory)][string]$BundleRoot,[string]$ReportPath)
$ErrorActionPreference='Stop'
Add-Type -AssemblyName System.Security.Cryptography.Pkcs
$pin='6C70C777B27D50949370D494B4B25798200FBDD5C171BAEEAA94D7E289900F3E'
$root=[IO.Path]::GetFullPath($BundleRoot)
function NoLink([string]$Path) {
    $cursor=[IO.Path]::GetFullPath($Path)
    while($cursor) {
        if(Test-Path -LiteralPath $cursor){
            if(((Get-Item -LiteralPath $cursor -Force).Attributes-band[IO.FileAttributes]::ReparsePoint)-ne0){throw 'Reparse path rejected'}
        }
        $parent=Split-Path -Parent $cursor;if(!$parent -or $parent-eq$cursor){break};$cursor=$parent
    }
}
function AssertCodeSigningCertificate([Security.Cryptography.X509Certificates.X509Certificate2]$Certificate) {
    if(!$Certificate){throw 'CMS signer certificate missing'}
    $eku=$Certificate.Extensions | Where-Object {$_.Oid.Value-eq'2.5.29.37'} | Select-Object -First 1
    if(!$eku -or @($eku.EnhancedKeyUsages | ForEach-Object {$_.Value})-notcontains'1.3.6.1.5.5.7.3.3'){
        throw 'CMS signer certificate requires the code-signing EKU'
    }
    $usage=$Certificate.Extensions | Where-Object {$_.Oid.Value-eq'2.5.29.15'} | Select-Object -First 1
    if($usage -and ($usage.KeyUsages-band[Security.Cryptography.X509Certificates.X509KeyUsageFlags]::DigitalSignature)-eq0){
        throw 'CMS signer key usage does not permit digital signatures'
    }
}
function Check([byte[]]$Bytes,[byte[]]$Signature){
    # Detached is constructor state in SignedCms, not an envelope inspection.
    # Decode without supplied content first, matching the native CMS gate.
    $envelope=[Security.Cryptography.Pkcs.SignedCms]::new()
    $envelope.Decode($Signature)
    if($envelope.ContentInfo.Content.Length-ne0){throw 'Detached CMS signature required'}
    $cms=[Security.Cryptography.Pkcs.SignedCms]::new([Security.Cryptography.Pkcs.ContentInfo]::new($Bytes),$true)
    $cms.Decode($Signature)
    if($cms.SignerInfos.Count-ne1){throw 'Exactly one detached signer required'}
    if($cms.SignerInfos[0].DigestAlgorithm.Value-cne'2.16.840.1.101.3.4.2.1'){throw 'CMS signer digest must be SHA256'}
    $cms.CheckSignature($true)
    $signingCertificate=$cms.SignerInfos[0].Certificate
    AssertCodeSigningCertificate $signingCertificate
    $rsa=[Security.Cryptography.X509Certificates.RSACertificateExtensions]::GetRSAPublicKey($signingCertificate)
    if(!$rsa){throw 'CMS signer must use an RSA public key'}
    $hash=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($rsa.ExportRSAPublicKey()))
    if($hash-cne$pin){throw 'Publisher key pin mismatch'}
}
NoLink $root
$bytes=[IO.File]::ReadAllBytes((Join-Path $root 'UPDATER-BUNDLE.json'))
Check $bytes ([IO.File]::ReadAllBytes((Join-Path $root 'UPDATER-BUNDLE.p7s')))
$statement=[Text.Encoding]::UTF8.GetString($bytes) | ConvertFrom-Json
if($statement.schema-cne'mir2.windows.updater-bundle.v1' -or $statement.files.Count-ne7){throw 'Bundle schema/closure'}
$paths=[Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
foreach($entry in $statement.files){
    if($entry.path -notmatch '^[A-Za-z0-9._/-]+$' -or $entry.path.Contains('..') -or $entry.path.StartsWith('/')){throw 'Unsafe bundle path'}
    if(!$paths.Add($entry.path)){throw 'Bundle path collision'}
    $file=Join-Path $root $entry.path;NoLink $file
    if((Get-Item -LiteralPath $file).Length-ne$entry.size -or
       (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash-cne$entry.sha256){throw 'Bundle file hash mismatch'}
}
$pointer=Get-Content -LiteralPath (Join-Path $root 'updater/active.txt') -Raw
if($pointer -cnotmatch '^[0-9A-F]{64}$'){throw 'Invalid engine pointer'}
$engineRoot=Join-Path $root "updater/engines/$pointer"
$engineBytes=[IO.File]::ReadAllBytes((Join-Path $engineRoot 'ENGINE.json'))
Check $engineBytes ([IO.File]::ReadAllBytes((Join-Path $engineRoot 'ENGINE.p7s')))
$engine=[Text.Encoding]::UTF8.GetString($engineBytes)|ConvertFrom-Json
if($engine.exeSha256-cne$pointer -or $engine.launcherSha256-cne(Get-FileHash -LiteralPath (Join-Path $root 'Mir2Launcher.exe') -Algorithm SHA256).Hash){throw 'Launcher/engine binding'}
Check ([IO.File]::ReadAllBytes((Join-Path $root 'updater/seed-feed.json'))) ([IO.File]::ReadAllBytes((Join-Path $root 'updater/seed-feed.p7s')))
$files=@(Get-ChildItem -LiteralPath $root -Recurse -Force -File)
if($files.Count-ne9){throw 'Extra or missing supplemental files'}
$allowed=@($statement.files.path)+@('UPDATER-BUNDLE.json','UPDATER-BUNDLE.p7s')
foreach($file in $files) {
    NoLink $file.FullName
    $relative=[IO.Path]::GetRelativePath($root,$file.FullName).Replace('\','/')
    if($allowed-cnotcontains$relative){throw 'Uncovered supplemental file'}
    foreach($stream in @(Get-Item -LiteralPath $file.FullName -Stream *)){
        if($stream.Stream-cne':$DATA'){throw 'Named data stream rejected'}
    }
}
$report=[ordered]@{passed=$true;schema='mir2.windows.updater-bundle-verification.v1'
    sourceRevision=$statement.sourceRevision;gameCandidate=$statement.gameCandidate;gameSourceRevision=$statement.gameSourceRevision
    files=$files.Count;engineSha256=$pointer;launcherSha256=$engine.launcherSha256;signingKeySha256=$pin
    publicAuthenticode=(Get-AuthenticodeSignature -LiteralPath (Join-Path $root 'Mir2Launcher.exe')).Status.ToString()}
if($ReportPath){[IO.File]::WriteAllText([IO.Path]::GetFullPath($ReportPath),($report|ConvertTo-Json -Depth 4),[Text.UTF8Encoding]::new($false))}
$report|ConvertTo-Json -Depth 4
