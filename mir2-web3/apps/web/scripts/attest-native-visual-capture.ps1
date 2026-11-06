[CmdletBinding()]
param(
    [string]$PackageRoot = '',
    [string]$CandidateImagePath = '',
    [string]$CandidateStatePath = '',
    [int]$ProcessId = 0,
    [string]$TrustedReleaseSignerThumbprint = '',
    [string]$EvidenceSignerThumbprint = '',
    [string]$CaptureAttestationPath = '',
    [string]$CaptureSignaturePath = '',
    [string]$CaptureSpkiPath = '',
    [string]$PackageVerificationPath = '',
    [string]$TrustedPolicyPath = '',
    [string]$ExpectedChallenge = '',
    [switch]$SelfTest,
    [switch]$Help
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Show-Usage {
    @'
Usage (formal package):
  powershell -NoProfile -File attest-native-visual-capture.ps1 `
    -PackageRoot <candidate-directory> `
    -CandidateImagePath <native.png> -CandidateStatePath <native.json> `
    -ProcessId <running mir2-platform-windows PID> `
    -TrustedReleaseSignerThumbprint <out-of-band SHA-1 thumbprint> `
    -EvidenceSignerThumbprint <CurrentUser\My RSA evidence certificate thumbprint> `
    [-TrustedPolicyPath <fixed-policy.json>] -ExpectedChallenge <128-bit-hex>
    [-CaptureAttestationPath <out.json>] [-CaptureSignaturePath <out.sig>] [-CaptureSpkiPath <out.der>]

Self-test mode deliberately skips formal package verification and prints an integration-only warning:
  ... -SelfTest -PackageVerificationPath <fixture-verification.json> ...
'@ | Write-Host
}

function Normalize-Thumbprint {
    param([Parameter(Mandatory)][string]$Value, [Parameter(Mandatory)][string]$Label)
    $normalized = ($Value -replace '\s', '').ToUpperInvariant()
    if ($normalized -notmatch '^[0-9A-F]{40}$') { throw "$Label must be a 40-hex SHA-1 thumbprint." }
    return $normalized
}

function Normalize-Sha256 {
    param([Parameter(Mandatory)][string]$Value, [Parameter(Mandatory)][string]$Label)
    $normalized = $Value.Trim().ToLowerInvariant()
    if ($normalized -notmatch '^[0-9a-f]{64}$') { throw "$Label must be a lowercase SHA-256." }
    return $normalized
}

function Get-FullPath {
    param([Parameter(Mandatory)][string]$Path, [Parameter(Mandatory)][string]$Label)
    if ([string]::IsNullOrWhiteSpace($Path)) { throw "$Label is required." }
    return [IO.Path]::GetFullPath((Resolve-Path -LiteralPath $Path -ErrorAction Stop).Path).TrimEnd('\', '/')
}

function Test-Within {
    param([Parameter(Mandatory)][string]$Path, [Parameter(Mandatory)][string]$Root)
    $p = [IO.Path]::GetFullPath($Path).TrimEnd('\', '/')
    $r = [IO.Path]::GetFullPath($Root).TrimEnd('\', '/')
    return $p.Equals($r, [StringComparison]::OrdinalIgnoreCase) -or $p.StartsWith($r + '\', [StringComparison]::OrdinalIgnoreCase)
}

function Test-Reparse {
    param([Parameter(Mandatory)][IO.FileSystemInfo]$Item)
    return (($Item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0)
}

function Assert-NoReparseAncestors {
    param([Parameter(Mandatory)][string]$Path)
    $cursor = [IO.Path]::GetFullPath($Path).TrimEnd('\', '/')
    while (-not (Test-Path -LiteralPath $cursor)) {
        $parent = Split-Path -Parent $cursor
        if ([string]::IsNullOrWhiteSpace($parent) -or $parent -eq $cursor) { break }
        $cursor = $parent
    }
    while (-not [string]::IsNullOrWhiteSpace($cursor)) {
        if (Test-Path -LiteralPath $cursor) {
            $item = Get-Item -LiteralPath $cursor -Force
            if (Test-Reparse $item) { throw "reparse/symlink ancestor rejected: $($item.FullName)" }
        }
        $parent = Split-Path -Parent $cursor
        if ([string]::IsNullOrWhiteSpace($parent) -or $parent -eq $cursor) { break }
        $cursor = $parent
    }
}

function Assert-NoReparseTree {
    param([Parameter(Mandatory)][string]$Path)
    Assert-NoReparseAncestors $Path
    $root = Get-Item -LiteralPath $Path -Force -ErrorAction Stop
    if (Test-Reparse $root) { throw "reparse/symlink root rejected: $Path" }
    $stack = [Collections.Generic.Stack[string]]::new()
    if ($root.PSIsContainer) { $stack.Push($root.FullName) }
    while ($stack.Count -gt 0) {
        foreach ($child in Get-ChildItem -LiteralPath $stack.Pop() -Force) {
            if (Test-Reparse $child) { throw "reparse/symlink entry rejected: $($child.FullName)" }
            if ($child.PSIsContainer) { $stack.Push($child.FullName) }
        }
    }
}

function Get-Sha256Bytes {
    param([Parameter(Mandatory)][byte[]]$Bytes)
    $sha=[Security.Cryptography.SHA256]::Create(); try { return ([BitConverter]::ToString($sha.ComputeHash($Bytes))).Replace('-','').ToLowerInvariant() } finally { $sha.Dispose() }
}
function Get-FileSha256 {
    param([Parameter(Mandatory)][string]$Path)
    return Get-Sha256Bytes (Read-StrictBytes $Path 'file hash input')
}
function Read-StrictBytes {
    param([Parameter(Mandatory)][string]$Path, [Parameter(Mandatory)][string]$Label)
    Assert-NoReparseAncestors $Path
    $item = Get-Item -LiteralPath $Path -Force -ErrorAction Stop
    if ($item.PSIsContainer -or (Test-Reparse $item)) { throw "$Label must be a regular non-reparse file." }
    $bytes=[IO.File]::ReadAllBytes($item.FullName); return ,$bytes
}

function Read-StrictJson {
    param([Parameter(Mandatory)][string]$Path, [Parameter(Mandatory)][string]$Label)
    $bytes = Read-StrictBytes $Path $Label
    if ($bytes.Length -ge 3 -and $bytes[0] -eq 0xEF -and $bytes[1] -eq 0xBB -and $bytes[2] -eq 0xBF) { throw "$Label must not contain a UTF-8 BOM." }
    $utf8 = [Text.UTF8Encoding]::new($false, $true)
    try { $text = $utf8.GetString($bytes) } catch { throw "$Label is not strict UTF-8: $($_.Exception.Message)" }
    if ([string]::IsNullOrWhiteSpace($text)) { throw "$Label is empty." }
    Assert-NoDuplicateJsonKeys -Text $text -Label $Label
    try { $value = $text | ConvertFrom-Json } catch { throw "$Label is invalid JSON: $($_.Exception.Message)" }
    return [pscustomobject]@{ Path = (Resolve-Path -LiteralPath $Path).Path; Bytes = $bytes; Text = $text; Value = $value; Sha256 = (Get-Sha256Bytes $bytes) }
}

function Assert-ClosedProperties {
    param([Parameter(Mandatory)][object]$Value, [Parameter(Mandatory)][string]$Label, [Parameter(Mandatory)][string[]]$Expected)
    if ($null -eq $Value -or $Value -is [Collections.IDictionary]) { throw "$Label must be a JSON object." }
    $actual = @($Value.PSObject.Properties.Name | Sort-Object)
    $wanted = @($Expected | Sort-Object)
    if (($actual -join "`n") -cne ($wanted -join "`n")) { throw "$Label has an unexpected property set." }
}

function Get-UtcTimestamp {
    param([Parameter(Mandatory)][string]$Value, [Parameter(Mandatory)][string]$Label)
    if ($Value -notmatch '(?i)(Z|\+00:00)$') { throw "$Label must carry an explicit UTC offset." }
    try { return [DateTimeOffset]::Parse($Value, [Globalization.CultureInfo]::InvariantCulture, [Globalization.DateTimeStyles]::RoundtripKind).ToUniversalTime() } catch { throw "$Label is not a valid UTC timestamp." }
}

function Get-PngU32 { param([byte[]]$Bytes,[int]$Offset); return (([uint32]$Bytes[$Offset] -shl 24) -bor ([uint32]$Bytes[$Offset+1] -shl 16) -bor ([uint32]$Bytes[$Offset+2] -shl 8) -bor [uint32]$Bytes[$Offset+3]) }
function Get-PngCrc32 { param([byte[]]$Bytes); [uint32]$crc=4294967295; foreach($b in $Bytes){$crc=$crc -bxor $b; for($i=0;$i -lt 8;$i++){if(($crc -band 1)-ne 0){$crc=($crc -shr 1)-bxor 0xEDB88320}else{$crc=$crc -shr 1}}}; return [uint32]($crc -bxor 4294967295) }
function Test-PngHeader {
    param([Parameter(Mandatory)][byte[]]$Bytes,[Parameter(Mandatory)][string]$Label)
    $maxBytes=32MB
    if($Bytes.Length -lt 45 -or $Bytes.Length -gt $maxBytes){throw "$Label must be a complete PNG no larger than 32 MiB."}
    $signature=[byte[]](137,80,78,71,13,10,26,10); for($i=0;$i -lt 8;$i++){if($Bytes[$i] -ne $signature[$i]){throw "$Label has an invalid PNG signature."}}
    $offset=8; $sawIhdr=$false; $sawIdat=$false; $sawIend=$false; $width=0; $height=0
    while($offset -lt $Bytes.Length){
        if($offset+12 -gt $Bytes.Length){throw "$Label has a truncated PNG chunk."}
        $length=[int](Get-PngU32 $Bytes $offset); $type=[Text.Encoding]::ASCII.GetString($Bytes,$offset+4,4); $dataStart=$offset+8; $dataEnd=$dataStart+$length; $crcEnd=$dataEnd+4
        if($length -lt 0 -or $dataEnd -lt $dataStart -or $crcEnd -gt $Bytes.Length){throw "$Label has an invalid PNG chunk length."}
        $crcInput=New-Object byte[] (4+$length); [Array]::Copy($Bytes,$offset+4,$crcInput,0,$crcInput.Length); $expectedCrc=Get-PngU32 $Bytes $dataEnd; $actualCrc=Get-PngCrc32 $crcInput; if($expectedCrc -ne $actualCrc){throw "$Label has an invalid $type CRC."}
        if(-not $sawIhdr -and $type -cne 'IHDR'){throw "$Label must begin with IHDR."}
        if($type -ceq 'IHDR'){
            if($sawIhdr -or $length -ne 13){throw "$Label has an invalid IHDR."}; $sawIhdr=$true; $width=Get-PngU32 $Bytes $dataStart; $height=Get-PngU32 $Bytes ($dataStart+4); if($width -ne 1024 -or $height -ne 768 -or $Bytes[$dataStart+8] -ne 8 -or @([byte]2,[byte]6) -notcontains $Bytes[$dataStart+9] -or $Bytes[$dataStart+10] -ne 0 -or $Bytes[$dataStart+11] -ne 0 -or $Bytes[$dataStart+12] -ne 0){throw "$Label must be non-interlaced 8-bit RGB/RGBA 1024x768."}
        } elseif($type -ceq 'IDAT') { if(-not $sawIhdr -or $sawIend){throw "$Label has IDAT outside the image stream."}; $sawIdat=$true
        } elseif($type -ceq 'IEND') { if($length -ne 0 -or $sawIend){throw "$Label has an invalid IEND."}; $sawIend=$true; if($crcEnd -ne $Bytes.Length){throw "$Label has trailing bytes after IEND."} }
        $offset=$crcEnd; if($sawIend){break}
    }
    if(-not $sawIhdr -or -not $sawIdat -or -not $sawIend){throw "$Label is missing IHDR, IDAT, or IEND."}
}
function Test-PngDecode {
    param([Parameter(Mandatory)][byte[]]$Bytes,[Parameter(Mandatory)][string]$Label)
    try { Add-Type -AssemblyName System.Drawing -ErrorAction Stop } catch { throw "$Label decoder unavailable: $($_.Exception.Message)" }
    $stream=[IO.MemoryStream]::new([byte[]]$Bytes); $decoded=$null; $clone=$null
    try {
        $decoded=[System.Drawing.Image]::FromStream($stream,$true,$true)
        if($decoded.Width -ne 1024 -or $decoded.Height -ne 768){throw "$Label decoded dimensions are not 1024x768."}
        $clone=[System.Drawing.Bitmap]::new($decoded)
        if($clone.Width -ne 1024 -or $clone.Height -ne 768){throw "$Label clone dimensions are not 1024x768."}
        [void]$clone.GetPixel(0,0); [void]$clone.GetPixel(1023,767)
    } catch { throw "$Label failed complete PNG decode: $($_.Exception.Message)" }
    finally { if($null -ne $clone){$clone.Dispose()}; if($null -ne $decoded){$decoded.Dispose()}; $stream.Dispose() }
}
function Validate-CaptureState {
    param([Parameter(Mandatory)][object]$State,[Parameter(Mandatory)][string]$StatePath,[Parameter(Mandatory)][string]$ImagePath,[Parameter(Mandatory)][string]$ImageSha256)
    Assert-ClosedProperties $State $StatePath @('schemaVersion','producer','runId','scene','capturedAt','imagePath','imageSha256','logicalSize','dpiScale','uiState','world','build','challenge','producerPid','processStartUtc')
    if([string]$State.schemaVersion -cne 'mir2-native-visual-capture-v1' -or [string]$State.producer -cne 'windows-native'){throw "$StatePath schema/producer mismatch."}
    if([string]$State.runId -notmatch '^[a-zA-Z0-9][a-zA-Z0-9._-]{0,95}$'){throw "$StatePath runId is invalid."}
    if([string]$State.scene -notin @('login','character-select','in-game','quest-accepted','combat','quest-complete')){throw "$StatePath scene is unsupported."}
    $captured=Get-UtcTimestamp ([string]$State.capturedAt) "$StatePath capturedAt"
    if([string]::IsNullOrWhiteSpace([string]$State.imagePath)){throw "$StatePath imagePath is empty."}
    $declaredImagePath=[IO.Path]::GetFullPath((Join-Path (Split-Path -Parent $StatePath) ([string]$State.imagePath))).TrimEnd('\','/'); if($declaredImagePath -ine [IO.Path]::GetFullPath($ImagePath).TrimEnd('\','/')){throw ($StatePath+' imagePath does not identify the supplied PNG.')}
    if((Normalize-Sha256 ([string]$State.imageSha256) "$StatePath imageSha256") -cne $ImageSha256){throw "$StatePath imageSha256 does not match the PNG."}
    Assert-ClosedProperties $State.logicalSize "$StatePath logicalSize" @('width','height'); if([int]$State.logicalSize.width -ne 1024 -or [int]$State.logicalSize.height -ne 768){throw "$StatePath logicalSize must be 1024x768."}
    if($State.dpiScale -isnot [ValueType] -or $State.dpiScale -is [bool] -or [double]$State.dpiScale -lt 0.5 -or [double]$State.dpiScale -gt 4){throw "$StatePath dpiScale is invalid."}
    if([string]::IsNullOrWhiteSpace([string]$State.uiState)){throw "$StatePath uiState is empty."}
    if(@('in-game','quest-accepted','combat','quest-complete') -contains [string]$State.scene){Assert-ClosedProperties $State.world "$StatePath world" @('map','x','y','light'); if([string]::IsNullOrWhiteSpace([string]$State.world.map) -or $State.world.x -isnot [int] -or $State.world.y -isnot [int] -or [string]::IsNullOrWhiteSpace([string]$State.world.light)){throw "$StatePath world is invalid."}} elseif($null -ne $State.world){throw "$StatePath world must be null for a non-world scene."}
    $challenge=([string]$State.challenge).Trim().ToLowerInvariant(); if($challenge -notmatch '^[0-9a-f]{32,}$'){throw "$StatePath challenge must be at least 128-bit hex."}
    try{$producerPid=[int64]$State.producerPid}catch{throw "$StatePath producerPid is invalid."}; if($producerPid -le 0 -or $producerPid -gt [int]::MaxValue){throw "$StatePath producerPid is invalid."}
    $producerStart=Get-UtcTimestamp ([string]$State.processStartUtc) "$StatePath processStartUtc"
    Assert-ClosedProperties $State.build "$StatePath build" @('sourceRevision','executableSha256','assetManifestSha256'); if([string]$State.build.sourceRevision -notmatch '^[0-9a-f]{40}$'){throw "$StatePath build.sourceRevision must be lowercase 40-hex."}
    $exeHash=Normalize-Sha256 ([string]$State.build.executableSha256) "$StatePath build.executableSha256"; $manifestHash=Normalize-Sha256 ([string]$State.build.assetManifestSha256) "$StatePath build.assetManifestSha256"
    return [pscustomobject]@{CapturedAt=$captured;RunId=[string]$State.runId;Scene=[string]$State.scene;SourceRevision=[string]$State.build.sourceRevision;ExeSha256=$exeHash;ManifestSha256=$manifestHash;Challenge=$challenge;ProducerPid=[int]$producerPid;ProcessStartUtc=$producerStart}
}
function Find-RepoRoot {
    param([Parameter(Mandatory)][string]$Start)
    $cursor = (Get-Item -LiteralPath $Start).FullName
    while ($true) {
        if ((Test-Path (Join-Path $cursor '.git')) -or ((Test-Path (Join-Path $cursor 'apps')) -and (Test-Path (Join-Path $cursor 'Cargo.toml')))) { return $cursor }
        $parent = Split-Path -Parent $cursor
        if ([string]::IsNullOrWhiteSpace($parent) -or $parent -eq $cursor) { throw "repository root not found from $Start" }
        $cursor = $parent
    }
}

function Invoke-FormalVerification {
    param([Parameter(Mandatory)][string]$PackageRoot,[Parameter(Mandatory)][string]$TrustedThumbprint)
    $repoRoot=Find-RepoRoot $PSScriptRoot; $verifier=Join-Path $repoRoot 'apps\game-client\platform-windows\scripts\verify-windows-candidate.ps1'; if(-not(Test-Path -LiteralPath $verifier -PathType Leaf)){throw "formal verifier missing: $verifier"}
    $version=Read-StrictJson (Join-Path $PackageRoot 'VERSION.json') 'VERSION.json'; $candidate=[string]$version.Value.candidate; if($candidate -notmatch '^WN-CANDIDATE-[A-Za-z0-9._-]+$'){throw 'formal package candidate identity is invalid.'}
    $expected=Join-Path $repoRoot ('docs\generated\player-qa\windows-package-preflight\'+$candidate+'-verification.json'); $parent=Split-Path -Parent $expected; Assert-NoReparseAncestors $parent; if(Test-Path -LiteralPath $expected){throw 'BLOCKED: verifier output already exists; formal mode requires a unique no-overwrite output from this run.'}
    $started=[DateTime]::UtcNow; $output=''; $exitCode=0; try{$output=(& powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File $verifier -PackageRoot $PackageRoot -TrustedSignerThumbprint $TrustedThumbprint 2>&1 | Out-String); $exitCode=$LASTEXITCODE}catch{throw "formal verifier invocation failed: $($_.Exception.Message)"}
    $finished=[DateTime]::UtcNow; if($exitCode -ne 0){throw "formal package verification failed: $output"}; if(-not(Test-Path -LiteralPath $expected -PathType Leaf)){throw 'BLOCKED: formal verifier did not create its expected verification output.'}; $info=Get-Item -LiteralPath $expected -Force; if($info.CreationTimeUtc -lt $started.AddSeconds(-2) -or $info.LastWriteTimeUtc -lt $started.AddSeconds(-2) -or $info.LastWriteTimeUtc -gt $finished.AddSeconds(2)){throw 'BLOCKED: verifier output timestamp is not from this invocation.'}; if($info.Length -le 0){throw 'formal verifier output is empty.'}
    return Read-StrictJson $expected 'package verification JSON'
}
function Validate-PackageEvidence {
    param([Parameter(Mandatory)][object]$Verification, [Parameter(Mandatory)][string]$PackageRoot, [Parameter(Mandatory)][string]$TrustedThumbprint, [Parameter(Mandatory)][string]$ExeSha256, [Parameter(Mandatory)][string]$ManifestSha256, [switch]$SelfTest)
    $verificationFields = @('schema','packageRoot','nonvisual','launchRequested','sourceRepoCheck','currentSourceRevision','attestationPresent','attestationSha256','packageManifestPresent','packageManifestSha256','packageManifestAggregateSha256','trustedSignerThumbprint','detachedSignatureValid','exeSha256','peValid','peImports','peDelayImports','packageFileCount','failures','passed','visualAccepted'); if ($SelfTest) { $verificationFields += @('selfTestOnly','integrationOnly') }; Assert-ClosedProperties $Verification.Value 'package verification JSON' $verificationFields
    if ([string]$Verification.Value.schema -cne 'mir2.windows.package-verification.v4') { throw 'package verification schema mismatch.' }

if([IO.Path]::GetFullPath([string]$Verification.Value.packageRoot).TrimEnd('\','/') -ine [IO.Path]::GetFullPath($PackageRoot).TrimEnd('\','/')){throw 'package verification packageRoot differs from the verified package.'}
    if ($Verification.Value.nonvisual -ne $true -or $Verification.Value.launchRequested -ne $false -or $Verification.Value.visualAccepted -ne $false) { throw 'package verification is not the required nonvisual result.' }
    if ($Verification.Value.attestationPresent -ne $true -or $Verification.Value.packageManifestPresent -ne $true -or $Verification.Value.peValid -ne $true -or $Verification.Value.detachedSignatureValid -ne $true -or $Verification.Value.passed -ne $true) { throw 'package verification did not pass all required formal gates.' }
    if (@($Verification.Value.failures).Count -ne 0) { throw 'package verification contains failures.' }
    if ((Normalize-Thumbprint ([string]$Verification.Value.trustedSignerThumbprint) 'package verification signer') -cne $TrustedThumbprint) { throw 'package verification trusted signer mismatch.' }
    if ((Normalize-Sha256 ([string]$Verification.Value.exeSha256) 'package verification EXE') -cne $ExeSha256) { throw 'package verification EXE mismatch.' }
    if ((Normalize-Sha256 ([string]$Verification.Value.packageManifestSha256) 'package verification manifest') -cne $ManifestSha256) { throw 'package verification manifest mismatch.' }
    $attestationHash = Normalize-Sha256 ([string]$Verification.Value.attestationSha256) 'package verification attestationSha256'
    $aggregateHash = Normalize-Sha256 ([string]$Verification.Value.packageManifestAggregateSha256) 'package verification aggregateSha256'
    if ($SelfTest -and ($Verification.Value.selfTestOnly -ne $true -or $Verification.Value.integrationOnly -ne $true)) { throw 'self-test verification fixture must explicitly declare selfTestOnly=true and integrationOnly=true.' }
    return [pscustomobject]@{ AttestationSha256 = $attestationHash; AggregateSha256 = $aggregateHash }
}

function Import-PkcsAssembly {
    $roots=@(); if(-not [string]::IsNullOrWhiteSpace($env:ProgramFiles)){$roots+=Join-Path $env:ProgramFiles 'dotnet\shared\Microsoft.WindowsDesktop.App'}; $dll=$null; foreach($root in $roots){if(Test-Path -LiteralPath $root){$dll=Get-ChildItem -Path $root -Filter 'System.Security.Cryptography.Pkcs.dll' -Recurse -File -ErrorAction SilentlyContinue | Sort-Object FullName | Select-Object -First 1; if($null -ne $dll){break}}}; if($null -eq $dll){throw 'System.Security.Cryptography.Pkcs is unavailable.'}; return [Reflection.Assembly]::LoadFrom($dll.FullName)
}
function Assert-DetachedCmsSignature {
    param([Parameter(Mandatory)][byte[]]$ContentBytes,[Parameter(Mandatory)][byte[]]$SignatureBytes,[Parameter(Mandatory)][string]$TrustedThumbprint)
    $pwsh=Get-Command pwsh -ErrorAction SilentlyContinue; if($null -eq $pwsh){throw 'BLOCKED: pwsh with System.Security.Cryptography.Pkcs is required for CMS verification.'}; $dll=Get-ChildItem -Path (Join-Path $env:ProgramFiles 'dotnet\shared\Microsoft.WindowsDesktop.App') -Filter 'System.Security.Cryptography.Pkcs.dll' -Recurse -File -ErrorAction SilentlyContinue | Sort-Object FullName | Select-Object -First 1; if($null -eq $dll){throw 'BLOCKED: System.Security.Cryptography.Pkcs assembly is unavailable.'}
    $dir=Join-Path ([IO.Path]::GetTempPath()) ('mir2-cms-verify-'+[guid]::NewGuid().ToString('N')); New-Item -ItemType Directory -Path $dir -Force | Out-Null; Assert-NoReparseAncestors $dir; $contentPath=Join-Path $dir 'content.bin'; $signaturePath=Join-Path $dir 'signature.bin'; [IO.File]::WriteAllBytes($contentPath,$ContentBytes); [IO.File]::WriteAllBytes($signaturePath,$SignatureBytes); $oldC=$env:MIR2_CMS_CONTENT_PATH; $oldS=$env:MIR2_CMS_SIGNATURE_PATH; $oldD=$env:MIR2_CMS_PKCS_DLL
    try {
        $env:MIR2_CMS_CONTENT_PATH=$contentPath; $env:MIR2_CMS_SIGNATURE_PATH=$signaturePath; $env:MIR2_CMS_PKCS_DLL=$dll.FullName
        $child=[string]::Join([Environment]::NewLine,@(
            '[Reflection.Assembly]::LoadFrom($env:MIR2_CMS_PKCS_DLL)|Out-Null',
            '$content=[System.Security.Cryptography.Pkcs.ContentInfo]::new([IO.File]::ReadAllBytes($env:MIR2_CMS_CONTENT_PATH)); $cms=[System.Security.Cryptography.Pkcs.SignedCms]::new($content,$true); $cms.Decode([IO.File]::ReadAllBytes($env:MIR2_CMS_SIGNATURE_PATH)); $cms.CheckSignature($true); $signers=@($cms.SignerInfos); if($signers.Count -ne 1 -or $null -eq $signers[0].Certificate){throw ''CMS signer certificate missing''}; [string]$signers[0].Certificate.Thumbprint'
        ))
        $output=& $pwsh.Source -NoProfile -NonInteractive -Command $child 2>&1 | Out-String; if($LASTEXITCODE -ne 0){throw "CMS child verification failed: $output"}; $lines=@($output.Trim() -split "`r?`n"); $lastLine=[string]$lines[-1]; if($lastLine -notmatch '^[0-9A-Fa-f]{40}$'){throw "CMS child returned unexpected signer output: $output"}; $actual=Normalize-Thumbprint $lastLine 'release CMS signer'; if($actual -cne $TrustedThumbprint){throw 'release CMS signer thumbprint does not match TrustedReleaseSignerThumbprint.'}
    } catch {throw "release CMS detached signature verification failed: $($_.Exception.ToString())"} finally {if($null -eq $oldC){Remove-Item Env:MIR2_CMS_CONTENT_PATH -ErrorAction SilentlyContinue}else{$env:MIR2_CMS_CONTENT_PATH=$oldC}; if($null -eq $oldS){Remove-Item Env:MIR2_CMS_SIGNATURE_PATH -ErrorAction SilentlyContinue}else{$env:MIR2_CMS_SIGNATURE_PATH=$oldS}; if($null -eq $oldD){Remove-Item Env:MIR2_CMS_PKCS_DLL -ErrorAction SilentlyContinue}else{$env:MIR2_CMS_PKCS_DLL=$oldD}; if(Test-Path -LiteralPath $dir){Remove-Item -LiteralPath $dir -Recurse -Force -ErrorAction SilentlyContinue}}
}
function Read-ReleaseBinding {
    param([Parameter(Mandatory)][string]$PackageRoot, [Parameter(Mandatory)][string]$Candidate, [Parameter(Mandatory)][string]$ExeSha256, [Parameter(Mandatory)][string]$ManifestSha256, [Parameter(Mandatory)][string]$SourceRevision, [Parameter(Mandatory)][string]$TrustedThumbprint, [switch]$SelfTest)
    $statementPath = Join-Path $PackageRoot 'RELEASE-STATEMENT.json'
    $signaturePath = Join-Path $PackageRoot 'RELEASE-STATEMENT.p7s'
    $statement = Read-StrictJson $statementPath 'RELEASE-STATEMENT.json'
    $signatureBytes = Read-StrictBytes $signaturePath 'RELEASE-STATEMENT.p7s'
    Assert-DetachedCmsSignature $statement.Bytes $signatureBytes $TrustedThumbprint
    Assert-ClosedProperties $statement.Value 'RELEASE-STATEMENT.json' @('schema','candidate','exeSha256','packageManifestSha256','packageManifestAggregateSha256','versionSha256','buildAttestationSha256','gitRevision','worktreeDirty','worktreeStatusSha256')
    if ([string]$statement.Value.schema -cne 'mir2.windows.release-statement.v1' -or [string]$statement.Value.candidate -cne $Candidate -or $statement.Value.worktreeDirty -ne $false) { throw 'release statement identity or clean-worktree binding mismatch.' }
    if ((Normalize-Sha256 ([string]$statement.Value.exeSha256) 'release EXE') -cne $ExeSha256 -or (Normalize-Sha256 ([string]$statement.Value.packageManifestSha256) 'release manifest') -cne $ManifestSha256) { throw 'release statement EXE/manifest mismatch.' }
    if ([string]$statement.Value.gitRevision -cne $SourceRevision) { throw 'release statement source revision mismatch.' }
    foreach ($field in @('packageManifestAggregateSha256','versionSha256','buildAttestationSha256','worktreeStatusSha256')) { [void](Normalize-Sha256 ([string]$statement.Value.$field) "release $field") }
    if ($SelfTest -and $signatureBytes.Length -eq 0) { throw 'self-test release signature fixture is empty.' }
    return [pscustomobject]@{ StatementPath=$statement.Path; StatementBytes=$statement.Bytes; StatementSha256 = $statement.Sha256; SignaturePath=$signaturePath; SignatureBytes=$signatureBytes; SignatureSha256 = (Get-Sha256Bytes $signatureBytes); PackageManifestAggregateSha256=(Normalize-Sha256 ([string]$statement.Value.packageManifestAggregateSha256) 'release aggregate'); BuildAttestationSha256=(Normalize-Sha256 ([string]$statement.Value.buildAttestationSha256) 'release build attestation'); Value = $statement.Value }
}
function Get-RunningCandidateProcess {
    param([Parameter(Mandatory)][int]$TargetProcessId, [Parameter(Mandatory)][string]$PackageRoot, [Parameter(Mandatory)][string]$ExpectedExeSha256)
    if ($TargetProcessId -le 0) { throw 'ProcessId must be positive.' }
    $proc = Get-CimInstance Win32_Process -Filter "ProcessId=$TargetProcessId" -ErrorAction Stop
    if ($null -eq $proc -or [string]$proc.Name -cne 'mir2-platform-windows.exe') { throw "PID $TargetProcessId is not the exact mir2-platform-windows.exe process." }
    if ([string]::IsNullOrWhiteSpace([string]$proc.ExecutablePath)) { throw 'running process has no inspectable executable path.' }
    $path = [IO.Path]::GetFullPath([string]$proc.ExecutablePath)
    $expectedPath = Join-Path $PackageRoot 'mir2-platform-windows.exe'
    if (-not (Test-Within $path $PackageRoot) -or [IO.Path]::GetFullPath($path).TrimEnd('\') -ine [IO.Path]::GetFullPath($expectedPath).TrimEnd('\')) { throw 'running process executable is not the package executable.' }
    $hash = Get-Sha256Bytes (Read-StrictBytes $path 'running candidate executable')
    if ($hash -cne $ExpectedExeSha256) { throw 'running process executable hash does not match the candidate sidecar.' }
    $start = Get-Process -Id $TargetProcessId -ErrorAction Stop
    try { $startUtc = $start.StartTime.ToUniversalTime() } catch { throw 'process start time is unavailable.' }
    return [pscustomobject]@{ Path = $path; Sha256 = $hash; StartUtc = [DateTimeOffset]$startUtc; Pid = $TargetProcessId }
}

function Get-RsaPrivateKey { param([Security.Cryptography.X509Certificates.X509Certificate2]$Certificate); try { $rsa=[Security.Cryptography.X509Certificates.RSACertificateExtensions]::GetRSAPrivateKey($Certificate); if($null -ne $rsa){ return $rsa } } catch {} ; if($Certificate.HasPrivateKey -and $null -ne $Certificate.PrivateKey){ return $Certificate.PrivateKey }; throw 'evidence certificate has no accessible RSA private key.' }

function Get-EvidenceCertificate {
    param([Parameter(Mandatory)][string]$Thumbprint)
    $normalized = Normalize-Thumbprint $Thumbprint 'EvidenceSignerThumbprint'
    $cert = @(Get-ChildItem -Path ('Cert:\CurrentUser\My\' + $normalized) -ErrorAction Stop)[0]
    if ($null -eq $cert) { throw "evidence certificate $normalized was not found in CurrentUser\My." }
    if ($cert.NotBefore.ToUniversalTime() -gt [DateTime]::UtcNow -or $cert.NotAfter.ToUniversalTime() -lt [DateTime]::UtcNow) { throw 'evidence certificate is not currently valid.' }
    if ($cert.PublicKey.Oid.Value -ne '1.2.840.113549.1.1.1' -or $cert.PublicKey.Key.KeySize -lt 3072) { throw 'evidence certificate must contain an RSA public key of at least 3072 bits.' }
    $usage = $cert.Extensions | Where-Object { $_.Oid.Value -eq '2.5.29.15' } | Select-Object -First 1
    if ($null -eq $usage -or (($usage.KeyUsages -band [Security.Cryptography.X509Certificates.X509KeyUsageFlags]::DigitalSignature) -eq 0)) { throw 'evidence certificate must explicitly permit DigitalSignature key usage.' }
    $private = Get-RsaPrivateKey $cert
    if ($null -eq $private) { throw 'evidence certificate has no accessible RSA private key.' }
    $private.Dispose()
    return $cert
}

function Encode-DerLength {
    param([int]$Length)
    if ($Length -lt 128) { return [byte[]]$Length }
    $parts = New-Object System.Collections.Generic.List[byte]
    $n = $Length
    while ($n -gt 0) { $parts.Insert(0, [byte]($n -band 0xFF)); $n = [Math]::Floor($n / 256) }
    return [byte[]]@([byte](0x80 -bor $parts.Count)) + $parts.ToArray()
}

function Wrap-Der {
    param([byte]$Tag, [byte[]]$Content)
    return [byte[]]@($Tag) + (Encode-DerLength $Content.Length) + $Content
}

function Sign-RsaPkcs1Sha256 { param([Security.Cryptography.RSA]$PrivateKey, [byte[]]$Bytes); if($PrivateKey -is [Security.Cryptography.RSACryptoServiceProvider]){ return $PrivateKey.SignData($Bytes, 'SHA256') }; $previous=$ErrorActionPreference; $ErrorActionPreference='Stop'; try { return $PrivateKey.SignData($Bytes, [Security.Cryptography.HashAlgorithmName]::SHA256, [Security.Cryptography.RSASignaturePadding]::Pkcs1) } finally { $ErrorActionPreference=$previous } }

function Get-RsaSpki {
    param([Parameter(Mandatory)][Security.Cryptography.X509Certificates.X509Certificate2]$Certificate)
    if ($Certificate.PublicKey.Oid.Value -ne '1.2.840.113549.1.1.1') { throw 'evidence certificate is not RSA.' }
    $oid = [byte[]](0x06,0x09,0x2A,0x86,0x48,0x86,0xF7,0x0D,0x01,0x01,0x01)
    $algorithm = Wrap-Der 0x30 ($oid + [byte[]](0x05,0x00))
    $bitString = $Certificate.PublicKey.EncodedKeyValue.RawData
    if ($bitString.Length -eq 0) { throw 'certificate public key is empty.' }
    if ($bitString[0] -ne 0x03) { $bitString = Wrap-Der 0x03 ([byte[]](0) + $bitString) }
    if ($bitString.Length -lt 3 -or $bitString[0] -ne 0x03) { throw 'certificate public key is not a DER BIT STRING.' }
    $spkiContent = $algorithm + $bitString
    return Wrap-Der 0x30 $spkiContent
}

function Assert-BytesEqual {
    param([Parameter(Mandatory)][byte[]]$Expected,[Parameter(Mandatory)][byte[]]$Actual,[Parameter(Mandatory)][string]$Label)
    if($Expected.Length -ne $Actual.Length){throw "$Label changed length during attestation."}
    for($i=0;$i -lt $Expected.Length;$i++){if($Expected[$i] -ne $Actual[$i]){throw "$Label changed during attestation."}}
}
function Assert-CaptureInputsUnchanged {
    param([Parameter(Mandatory)][string]$ImagePath,[Parameter(Mandatory)][byte[]]$ImageBytes,[Parameter(Mandatory)][string]$StatePath,[Parameter(Mandatory)][byte[]]$StateBytes)
    $imageNow=Read-StrictBytes $ImagePath 'candidate PNG recheck'; Assert-BytesEqual $ImageBytes $imageNow 'candidate PNG'; if((Get-Sha256Bytes $imageNow) -cne (Get-Sha256Bytes $ImageBytes)){throw 'candidate PNG hash changed during attestation.'}
    $stateNow=Read-StrictBytes $StatePath 'candidate state recheck'; Assert-BytesEqual $StateBytes $stateNow 'candidate state'; if((Get-Sha256Bytes $stateNow) -cne (Get-Sha256Bytes $StateBytes)){throw 'candidate state hash changed during attestation.'}
}
function Assert-ControlledInputsUnchanged {
    param([Parameter(Mandatory)][string]$PackageRoot,[Parameter(Mandatory)][object]$PackageBefore,[Parameter(Mandatory)][object]$Version,[Parameter(Mandatory)][object]$Policy,[Parameter(Mandatory)][object]$Release,[Parameter(Mandatory)][object]$Verification)
    $packageNow=Get-PackageSnapshot $PackageRoot
    Assert-PackageSnapshot $PackageRoot $PackageBefore $packageNow
    $versionNow=Read-StrictJson $Version.Path 'VERSION.json recheck'; Assert-BytesEqual $Version.Bytes $versionNow.Bytes 'VERSION.json'
    $policyNow=Read-StrictJson $Policy.Path 'trusted native visual policy recheck'; Assert-BytesEqual $Policy.Bytes $policyNow.Bytes 'trusted native visual policy'
    $statementNow=Read-StrictJson $Release.StatementPath 'RELEASE-STATEMENT.json recheck'; Assert-BytesEqual $Release.StatementBytes $statementNow.Bytes 'RELEASE-STATEMENT.json'
    $signatureNow=Read-StrictBytes $Release.SignaturePath 'RELEASE-STATEMENT.p7s recheck'; Assert-BytesEqual $Release.SignatureBytes $signatureNow 'RELEASE-STATEMENT.p7s'
    $verificationNow=Read-StrictJson $Verification.Path 'package verification JSON recheck'; Assert-BytesEqual $Verification.Bytes $verificationNow.Bytes 'package verification JSON'
}
function Write-NoOverwriteRollbackSafeGroup {
    param([Parameter(Mandatory)][string[]]$Paths,[Parameter(Mandatory)][byte[][]]$Payloads,[switch]$SelfTest)
    if(@($Paths).Count -ne 3 -or @($Payloads).Count -ne 3){throw 'exactly three attestation outputs are required.'}
    $fullPaths=@(); $parents=@(); $seen=[Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    for($i=0;$i -lt 3;$i++){
        $full=[IO.Path]::GetFullPath($Paths[$i])
        if(-not $seen.Add($full)){throw 'capture output paths must be distinct.'}
        $parent=Split-Path -Parent $full
        if(-not(Test-Path -LiteralPath $parent -PathType Container)){throw "capture output parent is missing: $parent"}
        Assert-NoReparseAncestors $parent
        if(Test-Path -LiteralPath $full){throw "refusing to start: output already exists: $full"}
        $fullPaths+=$full; $parents+=$parent
    }
    if(@($parents | Select-Object -Unique).Count -ne 1){throw 'capture outputs must share one parent for grouped commit.'}
    $temps=@(); $moved=New-Object System.Collections.Generic.List[int]
    $inject=($SelfTest -and [string]$env:MIR2_NATIVE_ATTEST_SELFTEST_FAIL_AFTER_FIRST -eq '1')
    $injectRollback=($SelfTest -and [string]$env:MIR2_NATIVE_ATTEST_SELFTEST_FAIL_ROLLBACK -eq '1')
    $primaryFailure=$null; $rollbackFailures=New-Object System.Collections.Generic.List[string]; $cleanupFailures=New-Object System.Collections.Generic.List[string]
    try {
        for($i=0;$i -lt 3;$i++){
            $tmp=Join-Path $parents[0] ('.'+[IO.Path]::GetFileName($fullPaths[$i])+'.'+[guid]::NewGuid().ToString('N')+'.tmp')
            $temps+=$tmp
            $stream=$null
            try {
                $stream=[IO.File]::Open($tmp,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
                $stream.Write($Payloads[$i],0,$Payloads[$i].Length); $stream.Flush($true)
            } finally { if($null -ne $stream){$stream.Dispose()} }
            $written=Read-StrictBytes $tmp 'staged attestation output'
            Assert-BytesEqual $Payloads[$i] $written 'staged attestation output'
        }
        for($i=0;$i -lt 3;$i++){
            if($inject -and $i -eq 1){throw 'SELFTEST injected grouped commit failure.'}
            if(Test-Path -LiteralPath $fullPaths[$i]){throw "refusing to overwrite output created during commit: $($fullPaths[$i])"}
            [IO.File]::Move($temps[$i],$fullPaths[$i]); [void]$moved.Add($i)
        }
    } catch {
        $primaryFailure=$_.Exception.Message
    }
    if($null -ne $primaryFailure){
        for($j=$moved.Count-1;$j -ge 0;$j--){
            $i=$moved[$j]; $target=$fullPaths[$i]
            if(Test-Path -LiteralPath $target){
                if($injectRollback){[void]$rollbackFailures.Add($target); continue}
                try {
                    $current=Read-StrictBytes $target 'rollback output'
                    Assert-BytesEqual $Payloads[$i] $current 'rollback output'
                    Remove-Item -LiteralPath $target -Force -ErrorAction Stop
                    if(Test-Path -LiteralPath $target){[void]$rollbackFailures.Add($target)}
                } catch { [void]$rollbackFailures.Add("$target ($($_.Exception.Message))") }
            }
        }
    }
    foreach($tmp in $temps){
        if(Test-Path -LiteralPath $tmp){
            try {Remove-Item -LiteralPath $tmp -Force -ErrorAction Stop; if(Test-Path -LiteralPath $tmp){[void]$cleanupFailures.Add($tmp)}} catch {[void]$cleanupFailures.Add("$tmp ($($_.Exception.Message))")}
        }
    }
    if($null -ne $primaryFailure){
        $message=$primaryFailure
        if($rollbackFailures.Count -gt 0){$message += '; rollback failed; residual paths: ' + ([string]::Join(', ',[string[]]$rollbackFailures))}
        if($cleanupFailures.Count -gt 0){$message += '; temporary cleanup failed: ' + ([string]::Join(', ',[string[]]$cleanupFailures))}
        throw $message
    }
    if($cleanupFailures.Count -gt 0){throw ('temporary cleanup failed: ' + ([string]::Join(', ',[string[]]$cleanupFailures)))}
}
function Get-PackageSnapshot {
    param([Parameter(Mandatory)][string]$PackageRoot)
    Assert-NoReparseTree $PackageRoot; $exe=Join-Path $PackageRoot 'mir2-platform-windows.exe'; $manifest=Join-Path $PackageRoot 'PACKAGE-MANIFEST.json'; if(-not(Test-Path -LiteralPath $exe -PathType Leaf) -or -not(Test-Path -LiteralPath $manifest -PathType Leaf)){throw 'package EXE or PACKAGE-MANIFEST.json is missing.'}; Assert-NoReparseAncestors $exe; Assert-NoReparseAncestors $manifest; $exeBytes=Read-StrictBytes $exe 'package executable'; $manifestBytes=Read-StrictBytes $manifest 'package manifest'; return [pscustomobject]@{ExePath=$exe;ManifestPath=$manifest;ExeBytes=$exeBytes;ManifestBytes=$manifestBytes;ExeSha256=(Get-Sha256Bytes $exeBytes);ManifestSha256=(Get-Sha256Bytes $manifestBytes)}
}
function Assert-PackageSnapshot { param([Parameter(Mandatory)][string]$PackageRoot,[Parameter(Mandatory)][object]$Before,[Parameter(Mandatory)][object]$After); if($Before.ExeSha256 -cne $After.ExeSha256 -or $Before.ManifestSha256 -cne $After.ManifestSha256){throw 'BLOCKED: verified package EXE or manifest changed during capture attestation.'}; Assert-BytesEqual $Before.ExeBytes $After.ExeBytes 'package executable'; Assert-BytesEqual $Before.ManifestBytes $After.ManifestBytes 'package manifest' }
function Read-TrustPolicy {
    param([Parameter(Mandatory)][string]$PackageRoot,[Parameter(Mandatory)][string]$Candidate,[Parameter(Mandatory)][string]$TrustedRelease,[Parameter(Mandatory)][string]$ExpectedChallenge,[Parameter(Mandatory)][string]$PolicyPath)
    if([string]::IsNullOrWhiteSpace($PolicyPath)){throw 'BLOCKED: TrustedPolicyPath is required; caller pins are not a trust root.'}
    $repoRoot=Find-RepoRoot $PSScriptRoot; $full=if([IO.Path]::IsPathRooted($PolicyPath)){Get-FullPath $PolicyPath 'TrustedPolicyPath'}else{if($PolicyPath -cne 'docs\generated\player-qa\native-visual-policy.json'){throw 'BLOCKED: relative policy path is not the fixed repository policy path.'}; Get-FullPath (Join-Path $repoRoot $PolicyPath) 'TrustedPolicyPath'}
    if(Test-Within $full $PackageRoot){throw 'BLOCKED: policy must be outside the candidate package.'}
    if([IO.Path]::IsPathRooted($PolicyPath)){ $policyRoot=$env:MIR2_NATIVE_TRUSTED_POLICY_ROOT; if([string]::IsNullOrWhiteSpace($policyRoot) -or -not(Test-Within $full (Get-FullPath $policyRoot 'MIR2_NATIVE_TRUSTED_POLICY_ROOT'))){throw 'BLOCKED: absolute policy path must be under the externally fixed CI policy root.'} }
    $fixedHash=Normalize-Sha256 ([string]$env:MIR2_NATIVE_TRUSTED_POLICY_SHA256) 'MIR2_NATIVE_TRUSTED_POLICY_SHA256'; $policy=Read-StrictJson $full 'trusted native visual policy'; if($policy.Sha256 -cne $fixedHash){throw 'BLOCKED: trusted policy SHA-256 does not match the externally fixed policy hash.'}
    Assert-ClosedProperties $policy.Value 'trusted native visual policy' @('schemaVersion','policyId','candidate','trustedReleaseSignerThumbprint','evidenceSignerSpkiSha256','challenge','challengeIssuedAt','challengeExpiresAt','challengeAuthority')
    if([string]$policy.Value.schemaVersion -cne 'mir2.native-visual-policy.v1'){throw 'trusted policy schema mismatch.'}; if([string]$policy.Value.candidate -cne $Candidate){throw 'trusted policy candidate mismatch.'}; if((Normalize-Thumbprint ([string]$policy.Value.trustedReleaseSignerThumbprint) 'trusted policy release signer') -cne $TrustedRelease){throw 'trusted policy release signer mismatch.'}; $spkiPin=Normalize-Sha256 ([string]$policy.Value.evidenceSignerSpkiSha256) 'trusted policy evidence SPKI'; if($spkiPin -eq ('0'*64)){throw 'trusted policy evidence SPKI pin is empty.'}; if(([string]$policy.Value.challenge).ToLowerInvariant() -cne $ExpectedChallenge){throw 'trusted policy challenge mismatch.'}; if([string]$policy.Value.challengeAuthority -cne 'external-one-time-required'){throw 'BLOCKED: trusted policy challengeAuthority is not external-one-time-required.'}
    $issued=Get-UtcTimestamp ([string]$policy.Value.challengeIssuedAt) 'trusted policy challengeIssuedAt'; $expires=Get-UtcTimestamp ([string]$policy.Value.challengeExpiresAt) 'trusted policy challengeExpiresAt'; $window=($expires-$issued).TotalSeconds; $now=[DateTimeOffset]::UtcNow; if($expires -le $issued -or $window -le 0 -or $window -gt 900){throw 'BLOCKED: trusted policy challenge window is invalid or exceeds 15 minutes.'}; if($now -lt $issued.AddSeconds(-30) -or $now -gt $expires){throw 'BLOCKED: trusted policy challenge is outside its active time window.'}
    return [pscustomobject]@{Sha256=$policy.Sha256;Bytes=$policy.Bytes;EvidenceSignerSpkiSha256=$spkiPin;IssuedAt=$issued;ExpiresAt=$expires;ChallengeAuthority=[string]$policy.Value.challengeAuthority;Path=$full}
}
function Assert-CapturePaths {
    param([Parameter(Mandatory)][string]$PackageRoot,[Parameter(Mandatory)][string[]]$Paths,[switch]$SelfTest)
    $seen=[Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase); $parents=@()
    foreach($path in $Paths){$full=[IO.Path]::GetFullPath($path); if(-not $seen.Add($full)){throw 'capture output paths must be distinct.'}; $parent=Split-Path -Parent $full; if(-not(Test-Path -LiteralPath $parent -PathType Container)){New-Item -ItemType Directory -Path $parent -Force | Out-Null}; Assert-NoReparseAncestors $parent; if(-not $SelfTest -and (Test-Within $full $PackageRoot)){throw 'formal capture outputs must be outside the verified package.'}; if(Test-Path -LiteralPath $full){throw "refusing to start: output already exists: $full"}; $parents+=$parent }
    if(@($parents | Select-Object -Unique).Count -ne 1){throw 'capture outputs must share one parent for grouped commit.'}
}
function Invoke-Attestation {
    param([switch]$SelfTest)
    if([string]::IsNullOrWhiteSpace($PackageRoot) -or [string]::IsNullOrWhiteSpace($CandidateImagePath) -or [string]::IsNullOrWhiteSpace($CandidateStatePath) -or $ProcessId -le 0){throw 'PackageRoot, CandidateImagePath, CandidateStatePath, and ProcessId are required.'}
    $package=Get-FullPath $PackageRoot 'PackageRoot'; $image=Get-FullPath $CandidateImagePath 'CandidateImagePath'; $statePath=Get-FullPath $CandidateStatePath 'CandidateStatePath'; Assert-NoReparseTree $package
    $imageParent=Split-Path -Parent $image; $stateParent=Split-Path -Parent $statePath; Assert-NoReparseTree $imageParent; if($stateParent -ine $imageParent){Assert-NoReparseTree $stateParent}
    if((Test-Within $image $package) -or (Test-Within $statePath $package)){throw 'BLOCKED: formal capture PNG/sidecar must be outside the verified package.'}
    if(-not $SelfTest -and -not [string]::IsNullOrWhiteSpace($PackageVerificationPath)){throw 'BLOCKED: formal mode rejects caller-selected PackageVerificationPath.'}
    $imageBytes=Read-StrictBytes $image 'candidate PNG'; Test-PngHeader $imageBytes 'candidate PNG'; Test-PngDecode $imageBytes 'candidate PNG'; $imageHash=Get-Sha256Bytes $imageBytes
    $state=Read-StrictJson $statePath 'candidate capture state'; $capture=Validate-CaptureState $state.Value $statePath $image $imageHash
    $imageInfo=Get-Item -LiteralPath $image -Force; $stateInfo=Get-Item -LiteralPath $statePath -Force; $captureDeadline=$capture.CapturedAt.UtcDateTime.AddSeconds(2); if($imageInfo.LastWriteTimeUtc -gt $captureDeadline -or $stateInfo.LastWriteTimeUtc -gt $captureDeadline){throw 'capture timestamps are inconsistent with PNG/sidecar file timestamps.'}
    $expectedChallenge=([string]$ExpectedChallenge).Trim().ToLowerInvariant(); if($expectedChallenge -notmatch '^[0-9a-f]{32,}$'){throw 'BLOCKED: ExpectedChallenge must be at least 128-bit hex.'}; if($capture.Challenge -cne $expectedChallenge){throw 'BLOCKED: sidecar challenge does not match ExpectedChallenge.'}
    $trustedRelease=Normalize-Thumbprint $TrustedReleaseSignerThumbprint 'TrustedReleaseSignerThumbprint'
    if($SelfTest){if([string]::IsNullOrWhiteSpace($PackageVerificationPath)){throw 'SelfTest requires an explicit fixture verification JSON.'}; $verification=Read-StrictJson $PackageVerificationPath 'self-test package verification JSON'}else{$verification=Invoke-FormalVerification $package $trustedRelease}
    $packageEvidence=Validate-PackageEvidence $verification $package $trustedRelease $capture.ExeSha256 $capture.ManifestSha256 -SelfTest:$SelfTest
    $version=Read-StrictJson (Join-Path $package 'VERSION.json') 'VERSION.json'; $candidate=[string]$version.Value.candidate; if($candidate -notmatch '^WN-CANDIDATE-[A-Za-z0-9._-]+$'){throw 'VERSION candidate identity is invalid.'}; if([string]$version.Value.gitRevision -cne $capture.SourceRevision){throw 'VERSION source revision and sidecar source revision disagree.'}
    $policy=Read-TrustPolicy $package $candidate $trustedRelease $expectedChallenge $TrustedPolicyPath
    $release=Read-ReleaseBinding $package $candidate $capture.ExeSha256 $capture.ManifestSha256 $capture.SourceRevision $trustedRelease -SelfTest:$SelfTest
    $packageBefore=Get-PackageSnapshot $package; if($packageBefore.ExeSha256 -cne $capture.ExeSha256 -or $packageBefore.ManifestSha256 -cne $capture.ManifestSha256){throw 'candidate sidecar does not match the current package EXE/manifest.'}
    if((Normalize-Sha256 ([string]$verification.Value.exeSha256) 'package verification EXE') -cne $packageBefore.ExeSha256){throw 'package verification EXE differs from current package.'}
    if((Normalize-Sha256 ([string]$verification.Value.attestationSha256) 'package verification attestationSha256') -cne $release.BuildAttestationSha256){throw 'package verification attestationSha256 is not bound to the signed release statement.'}
    if((Normalize-Sha256 ([string]$verification.Value.packageManifestAggregateSha256) 'package verification aggregateSha256') -cne $release.PackageManifestAggregateSha256){throw 'package verification aggregateSha256 is not bound to the signed release statement.'}
    $running=Get-RunningCandidateProcess $ProcessId $package $capture.ExeSha256; if($running.Pid -ne $capture.ProducerPid){throw 'BLOCKED: sidecar producerPid does not match the exact running process.'}; if($running.StartUtc.UtcDateTime.Ticks -ne $capture.ProcessStartUtc.UtcDateTime.Ticks){throw 'BLOCKED: sidecar processStartUtc does not match the exact running process.'}; if($running.StartUtc -gt $capture.CapturedAt){throw 'process start time is after capture time.'}; if($capture.CapturedAt -gt [DateTimeOffset]::UtcNow.AddMinutes(2)){throw 'capture timestamp is in the future.'}; if(([DateTimeOffset]::UtcNow-$capture.CapturedAt).TotalMinutes -gt 15){throw 'capture is older than the 15-minute evidence window.'}
    $cert=Get-EvidenceCertificate $EvidenceSignerThumbprint; try { $spki=Get-RsaSpki $cert; $spkiHash=Get-Sha256Bytes $spki; if($spkiHash -cne $policy.EvidenceSignerSpkiSha256){throw 'BLOCKED: evidence signer SPKI does not match the fixed policy pin.'}
        $packageMid=Get-PackageSnapshot $package; Assert-PackageSnapshot $package $packageBefore $packageMid; Assert-ControlledInputsUnchanged $package $packageBefore $version $policy $release $verification
        $attestedAt=[DateTimeOffset]::UtcNow; if($attestedAt -lt $capture.CapturedAt -or $attestedAt -lt $running.StartUtc){throw 'attestation timestamp is earlier than an input timestamp.'}
        $formalBlockers='BLOCKED_UNSIGNED_PACKAGE_VERIFICATION_RACE;BLOCKED_PATH_BASED_TOCTOU_NO_NOFOLLOW;BLOCKED_EXTERNAL_CHALLENGE_CONSUMPTION'; $statement=[ordered]@{schemaVersion='mir2-native-capture-attestation-v1';formalAcceptance=if($SelfTest){'SELFTEST_ONLY;'+$formalBlockers}else{$formalBlockers};attestedAt=$attestedAt.ToString('o',[Globalization.CultureInfo]::InvariantCulture);runId=$capture.RunId;scene=$capture.Scene;capturedAt=$state.Value.capturedAt;stateSha256=$state.Sha256;imageSha256=$imageHash;processId=$running.Pid;processStartUtc=$running.StartUtc.ToString('o',[Globalization.CultureInfo]::InvariantCulture);producerPid=$capture.ProducerPid;challenge=$capture.Challenge;challengeIssuedAt=$policy.IssuedAt.ToString('o',[Globalization.CultureInfo]::InvariantCulture);challengeExpiresAt=$policy.ExpiresAt.ToString('o',[Globalization.CultureInfo]::InvariantCulture);challengeAuthority=$policy.ChallengeAuthority;challengeConsumption='external-required-not-proven';exeSha256=$capture.ExeSha256;candidate=$candidate;sourceRevision=$capture.SourceRevision;packageManifestSha256=$capture.ManifestSha256;releaseStatementSha256=$release.StatementSha256;releaseSignatureSha256=$release.SignatureSha256;packageVerificationSha256=$verification.Sha256;policySha256=$policy.Sha256;trustedReleaseSignerThumbprint=$trustedRelease;signatureAlgorithm='RSA-PKCS1-SHA256';evidenceSignerSpkiSha256=$spkiHash}
        $json=[Text.UTF8Encoding]::new($false).GetBytes(($statement|ConvertTo-Json -Compress -Depth 8)); $attestationPath=if([string]::IsNullOrWhiteSpace($CaptureAttestationPath)){Join-Path $stateParent (([IO.Path]::GetFileNameWithoutExtension($statePath))+'.attestation.json')}else{[IO.Path]::GetFullPath($CaptureAttestationPath)}; $signaturePath=if([string]::IsNullOrWhiteSpace($CaptureSignaturePath)){[IO.Path]::ChangeExtension($attestationPath,'.sig')}else{[IO.Path]::GetFullPath($CaptureSignaturePath)}; $spkiPath=if([string]::IsNullOrWhiteSpace($CaptureSpkiPath)){[IO.Path]::ChangeExtension($attestationPath,'.spki.der')}else{[IO.Path]::GetFullPath($CaptureSpkiPath)}; Assert-CapturePaths $package @($attestationPath,$signaturePath,$spkiPath) -SelfTest:$SelfTest
        Assert-CaptureInputsUnchanged $image $imageBytes $statePath $state.Bytes
        $private=Get-RsaPrivateKey $cert; try{$signature=Sign-RsaPkcs1Sha256 $private $json}finally{$private.Dispose()}
        $packageAfter=Get-PackageSnapshot $package; Assert-PackageSnapshot $package $packageBefore $packageAfter
        Assert-CaptureInputsUnchanged $image $imageBytes $statePath $state.Bytes
        Assert-ControlledInputsUnchanged $package $packageBefore $version $policy $release $verification
        Write-NoOverwriteRollbackSafeGroup @($attestationPath,$signaturePath,$spkiPath) ([byte[][]]@($json,$signature,$spki)) -SelfTest:$SelfTest
        $status=if($SelfTest){'passed'}else{'blocked'}
        return [pscustomobject]@{status=$status;formalAcceptance=if($SelfTest){'SELFTEST_ONLY;'+$formalBlockers}else{$formalBlockers};formalBlockers=@('BLOCKED_UNSIGNED_PACKAGE_VERIFICATION_RACE','BLOCKED_PATH_BASED_TOCTOU_NO_NOFOLLOW','BLOCKED_EXTERNAL_CHALLENGE_CONSUMPTION');integrationOnly=[bool]$SelfTest;challengeConsumption='external-required-not-proven';attestationPath=$attestationPath;signaturePath=$signaturePath;spkiPath=$spkiPath;candidate=$candidate;runId=$capture.RunId;scene=$capture.Scene;processId=$running.Pid;producerPid=$capture.ProducerPid;challenge=$capture.Challenge;challengeIssuedAt=$policy.IssuedAt.ToString('o',[Globalization.CultureInfo]::InvariantCulture);challengeExpiresAt=$policy.ExpiresAt.ToString('o',[Globalization.CultureInfo]::InvariantCulture);policySha256=$policy.Sha256;exeSha256=$capture.ExeSha256;packageManifestSha256=$capture.ManifestSha256;packageManifestAggregateSha256=$packageEvidence.AggregateSha256;buildAttestationSha256=$packageEvidence.AttestationSha256;packageVerificationPath=$verification.Path;packageVerificationSha256=$verification.Sha256;trustedReleaseSignerThumbprint=$trustedRelease;evidenceSignerSpkiSha256=$spkiHash}
    } finally { $cert.Dispose() }
}
$script:JsonScanText = ""
$script:JsonScanIndex = 0
function Skip-JsonWhitespace { while($script:JsonScanIndex -lt $script:JsonScanText.Length -and [char]::IsWhiteSpace($script:JsonScanText[$script:JsonScanIndex])){$script:JsonScanIndex++} }
function Read-JsonStringToken {
    if($script:JsonScanIndex -ge $script:JsonScanText.Length -or $script:JsonScanText[$script:JsonScanIndex] -ne [char]34){throw 'JSON string expected'}
    $script:JsonScanIndex++; $sb=[Text.StringBuilder]::new()
    while($script:JsonScanIndex -lt $script:JsonScanText.Length){
        $ch=$script:JsonScanText[$script:JsonScanIndex]; $script:JsonScanIndex++
        if($ch -eq [char]34){return $sb.ToString()}
        if([int][char]$ch -lt 0x20){throw 'JSON control character in string'}
        if($ch -eq [char]92){
            if($script:JsonScanIndex -ge $script:JsonScanText.Length){throw 'JSON escape is truncated'}
            $esc=$script:JsonScanText[$script:JsonScanIndex]; $script:JsonScanIndex++
            switch([int][char]$esc){
                34 {[void]$sb.Append([char]34);continue}
                92 {[void]$sb.Append([char]92);continue}
                47 {[void]$sb.Append([char]47);continue}
                98 {[void]$sb.Append([char]8);continue}
                102 {[void]$sb.Append([char]12);continue}
                110 {[void]$sb.Append([char]10);continue}
                114 {[void]$sb.Append([char]13);continue}
                116 {[void]$sb.Append([char]9);continue}
                117 { if($script:JsonScanIndex+4 -gt $script:JsonScanText.Length){throw "JSON unicode escape is truncated"}; $hex=$script:JsonScanText.Substring($script:JsonScanIndex,4); if($hex -notmatch "^[0-9A-Fa-f]{4}$"){throw "JSON unicode escape is invalid"}; [void]$sb.Append([char][Convert]::ToInt32($hex,16)); $script:JsonScanIndex+=4; continue }
                default {throw "JSON escape is invalid"}
            }        } else {[void]$sb.Append($ch)}
    }
    throw 'JSON string is unterminated'
}
function Read-JsonValueForDuplicateScan {
    Skip-JsonWhitespace
    if($script:JsonScanIndex -ge $script:JsonScanText.Length){throw 'JSON value is missing'}
    $ch=$script:JsonScanText[$script:JsonScanIndex]
    if($ch -eq [char]34){[void](Read-JsonStringToken);return}
    if($ch -eq [char]123){Read-JsonObjectForDuplicateScan;return}
    if($ch -eq [char]91){Read-JsonArrayForDuplicateScan;return}
    $start=$script:JsonScanIndex
    while($script:JsonScanIndex -lt $script:JsonScanText.Length -and $script:JsonScanText[$script:JsonScanIndex] -notin @([char]44,[char]93,[char]125) -and -not [char]::IsWhiteSpace($script:JsonScanText[$script:JsonScanIndex])){$script:JsonScanIndex++}
    if($script:JsonScanIndex -eq $start){throw 'JSON value is invalid'}
}
function Read-JsonObjectForDuplicateScan {
    if($script:JsonScanText[$script:JsonScanIndex] -ne [char]123){throw 'JSON object expected'}; $script:JsonScanIndex++; $seen=[Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal); Skip-JsonWhitespace
    if($script:JsonScanIndex -lt $script:JsonScanText.Length -and $script:JsonScanText[$script:JsonScanIndex] -eq [char]125){$script:JsonScanIndex++;return}
    while($true){Skip-JsonWhitespace; $key=Read-JsonStringToken; if(-not $seen.Add($key)){throw 'duplicate JSON object key rejected'}; Skip-JsonWhitespace; if($script:JsonScanIndex -ge $script:JsonScanText.Length -or $script:JsonScanText[$script:JsonScanIndex] -ne [char]58){throw 'JSON object colon expected'}; $script:JsonScanIndex++; Read-JsonValueForDuplicateScan; Skip-JsonWhitespace; if($script:JsonScanIndex -ge $script:JsonScanText.Length){throw 'JSON object is unterminated'}; $delimiter=$script:JsonScanText[$script:JsonScanIndex]; $script:JsonScanIndex++; if($delimiter -eq [char]125){return}; if($delimiter -ne [char]44){throw 'JSON object delimiter is invalid'}}
}
function Read-JsonArrayForDuplicateScan {
    if($script:JsonScanText[$script:JsonScanIndex] -ne [char]91){throw 'JSON array expected'}; $script:JsonScanIndex++; Skip-JsonWhitespace; if($script:JsonScanIndex -lt $script:JsonScanText.Length -and $script:JsonScanText[$script:JsonScanIndex] -eq [char]93){$script:JsonScanIndex++;return}
    while($true){Read-JsonValueForDuplicateScan; Skip-JsonWhitespace; if($script:JsonScanIndex -ge $script:JsonScanText.Length){throw 'JSON array is unterminated'}; $delimiter=$script:JsonScanText[$script:JsonScanIndex]; $script:JsonScanIndex++; if($delimiter -eq [char]93){return}; if($delimiter -ne [char]44){throw 'JSON array delimiter is invalid'}}
}
function Assert-NoDuplicateJsonKeys { param([Parameter(Mandatory)][string]$Text,[Parameter(Mandatory)][string]$Label); $script:JsonScanText=$Text; $script:JsonScanIndex=0; try { Skip-JsonWhitespace; Read-JsonValueForDuplicateScan; Skip-JsonWhitespace; if($script:JsonScanIndex -ne $script:JsonScanText.Length){throw 'JSON trailing data rejected'} } catch { throw "$Label failed strict duplicate-key JSON validation: $($_.Exception.Message)" } finally {$script:JsonScanText="";$script:JsonScanIndex=0} }
if ($Help) { Show-Usage; exit 0 }
try {
    if ($SelfTest) { Write-Warning 'SELFTEST ONLY: formal verify-windows-candidate.ps1 was intentionally skipped; this output is integration-only.' }
    $result = Invoke-Attestation -SelfTest:$SelfTest
    $result | ConvertTo-Json -Depth 8
    exit 0
} catch {
    Write-Error $_.Exception.Message
    exit 1
}