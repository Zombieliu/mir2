[CmdletBinding()]
param([switch]$Help)

Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$Attester=Join-Path $PSScriptRoot 'attest-native-visual-capture.ps1'

function Fail-Test { param([string]$Message); throw "SELFTEST FAILED: $Message" }
function Assert-True { param([bool]$Condition,[string]$Message); if(-not $Condition){Fail-Test $Message} }
function Sha256Bytes { param([byte[]]$Bytes); $sha=[Security.Cryptography.SHA256]::Create(); try{return ([BitConverter]::ToString($sha.ComputeHash($Bytes))).Replace('-','').ToLowerInvariant()}finally{$sha.Dispose()} }
function Sha256File { param([string]$Path); return Sha256Bytes ([IO.File]::ReadAllBytes($Path)) }
function Write-Utf8Json { param([string]$Path,[object]$Value); [IO.File]::WriteAllText($Path,($Value|ConvertTo-Json -Compress -Depth 12),[Text.UTF8Encoding]::new($false)) }
function BigEndian32 { param([uint32]$Value); return [byte[]]@([byte](($Value-shr 24)-band 255),[byte](($Value-shr 16)-band 255),[byte](($Value-shr 8)-band 255),[byte]($Value-band 255)) }
function Join-Bytes { param([object[]]$Parts); $list=New-Object 'System.Collections.Generic.List[byte]'; foreach($part in $Parts){foreach($b in [byte[]]$part){[void]$list.Add([byte]$b)}};return ,([byte[]]$list.ToArray()) }
function Encode-DerLength { param([int]$Length);if($Length-lt 128){return [byte[]]$Length};$parts=New-Object 'System.Collections.Generic.List[byte]';$n=$Length;while($n-gt 0){$parts.Insert(0,[byte]($n-band 255));$n=[Math]::Floor($n/256)};return Join-Bytes @([byte](0x80-bor $parts.Count),$parts.ToArray()) }
function Wrap-Der { param([byte]$Tag,[byte[]]$Content);return Join-Bytes @([byte[]]@($Tag),(Encode-DerLength $Content.Length),$Content) }
function Crc32 { param([byte[]]$Bytes); [uint32]$crc=4294967295; foreach($b in $Bytes){$crc=$crc-bxor $b; for($i=0;$i-lt 8;$i++){if(($crc-band 1)-ne 0){$crc=($crc-shr 1)-bxor 0xEDB88320}else{$crc=$crc-shr 1}}}; return [uint32]($crc-bxor 4294967295) }
function PngChunk { param([string]$Type,[byte[]]$Data); $t=[Text.Encoding]::ASCII.GetBytes($Type); return Join-Bytes @((BigEndian32 $Data.Length),$t,$Data,(BigEndian32 (Crc32 (Join-Bytes @($t,$Data))))) }
function Write-TestPng {
    param([string]$Path)
    $raw=New-Object byte[] ((1024*4+1)*768)
    for($y=0;$y-lt 768;$y++){ $raw[$y*4097]=0; for($x=0;$x-lt 1024;$x++){ $o=$y*4097+1+$x*4; $raw[$o]=40;$raw[$o+1]=80;$raw[$o+2]=120;$raw[$o+3]=255 } }
    $ms=[IO.MemoryStream]::new();try{$deflate=[IO.Compression.DeflateStream]::new($ms,[IO.Compression.CompressionLevel]::Fastest,$true);try{$deflate.Write($raw,0,$raw.Length)}finally{$deflate.Dispose()};$idat=$ms.ToArray()}finally{$ms.Dispose()}
$ihdr=Join-Bytes @((BigEndian32 1024),(BigEndian32 768),[byte[]](8,6,0,0,0))
[IO.File]::WriteAllBytes($Path,(Join-Bytes @([byte[]](137,80,78,71,13,10,26,10),(PngChunk 'IHDR' $ihdr),(PngChunk 'IDAT' $idat),(PngChunk 'IEND' ([byte[]]::new(0))))))
}
function Corrupt-IdatWithValidCrc {
    param([byte[]]$Bytes)
    $copy=New-Object byte[] $Bytes.Length;[Array]::Copy($Bytes,$copy,$Bytes.Length);$type=[Text.Encoding]::ASCII.GetBytes('IDAT')
    for($offset=8;$offset-lt $copy.Length-12;$offset++){ $match=$true;for($j=0;$j-lt 4;$j++){if($copy[$offset+$j]-ne $type[$j]){$match=$false;break}};if($match){$length=[int](([uint32]$copy[$offset-4]-shl 24)-bor([uint32]$copy[$offset-3]-shl 16)-bor([uint32]$copy[$offset-2]-shl 8)-bor[uint32]$copy[$offset-1]);$copy[$offset+4]=$copy[$offset+4]-bxor 0xFF;$crcInput=New-Object byte[] (4+$length);[Array]::Copy($copy,$offset,$crcInput,0,$crcInput.Length);[Array]::Copy((BigEndian32 (Crc32 $crcInput)),0,$copy,$offset+4+$length,4);return ,$copy} }
    throw 'IDAT fixture not found'
}
function SpkiSha256 { param([Security.Cryptography.X509Certificates.X509Certificate2]$Certificate);if($Certificate.PublicKey.Oid.Value -ne '1.2.840.113549.1.1.1'){throw 'SelfTest certificate is not RSA.'};$oid=[byte[]](0x06,0x09,0x2A,0x86,0x48,0x86,0xF7,0x0D,0x01,0x01,0x01);$algorithm=Wrap-Der 0x30 (Join-Bytes @($oid,[byte[]](0x05,0)));$bitString=$Certificate.PublicKey.EncodedKeyValue.RawData;if($bitString.Length-eq 0){throw 'SelfTest public key is empty.'};if($bitString[0]-ne 0x03){$bitString=Wrap-Der 0x03 (Join-Bytes @([byte[]](0),$bitString))};return Sha256Bytes (Wrap-Der 0x30 (Join-Bytes @($algorithm,$bitString))) }

function Write-DetachedCms {
    param([string]$Path,[Security.Cryptography.X509Certificates.X509Certificate2]$Certificate,[byte[]]$ContentBytes)
    $pwsh=Get-Command pwsh -ErrorAction Stop
    $dir=Join-Path (Split-Path -Parent $Path) ('.cms-selftest-'+[guid]::NewGuid().ToString('N'))
    $contentPath=Join-Path $dir 'content.bin';$signaturePath=Join-Path $dir 'signature.p7s';$primaryError=$null;$cleanupErrors=[System.Collections.Generic.List[string]]::new()
    try {
        New-Item -ItemType Directory -Path $dir -Force|Out-Null;[IO.File]::WriteAllBytes($contentPath,$ContentBytes)
        $oldContent=$env:MIR2_SELFTEST_CMS_CONTENT_PATH;$oldSignature=$env:MIR2_SELFTEST_CMS_SIGNATURE_PATH;$oldThumbprint=$env:MIR2_SELFTEST_CMS_CERT_THUMBPRINT
        try {
            $env:MIR2_SELFTEST_CMS_CONTENT_PATH=$contentPath;$env:MIR2_SELFTEST_CMS_SIGNATURE_PATH=$signaturePath;$env:MIR2_SELFTEST_CMS_CERT_THUMBPRINT=$Certificate.Thumbprint
            $child=[string]::Join([Environment]::NewLine,@(
                '$ErrorActionPreference=''Stop'''
                '$thumbprint=($env:MIR2_SELFTEST_CMS_CERT_THUMBPRINT -replace ''\s'','''').ToUpperInvariant()'
                '$certificate=Get-ChildItem -Path ''Cert:\CurrentUser\My'' -ErrorAction Stop | Where-Object { $_.Thumbprint -eq $thumbprint } | Select-Object -First 1'
                'if($null -eq $certificate -or -not $certificate.HasPrivateKey){throw ''SelfTest CMS certificate with private key was not found by thumbprint.''}'
                '$content=[System.Security.Cryptography.Pkcs.ContentInfo]::new([IO.File]::ReadAllBytes($env:MIR2_SELFTEST_CMS_CONTENT_PATH))'
                '$cms=[System.Security.Cryptography.Pkcs.SignedCms]::new($content,$true)'
                '$signer=[System.Security.Cryptography.Pkcs.CmsSigner]::new($certificate)'
                '$signer.IncludeOption=[System.Security.Cryptography.X509Certificates.X509IncludeOption]::EndCertOnly'
                '$cms.ComputeSignature($signer)'
                '[IO.File]::WriteAllBytes($env:MIR2_SELFTEST_CMS_SIGNATURE_PATH,$cms.Encode())'
            ))
            $output=((& $pwsh.Source -NoProfile -NonInteractive -Command $child 2>&1|ForEach-Object{$_.ToString()})-join [Environment]::NewLine)
            if($LASTEXITCODE -ne 0){throw "pwsh CMS signing failed: $output"}
            if(-not(Test-Path -LiteralPath $signaturePath -PathType Leaf)){throw 'pwsh CMS signing produced no signature.'}
            Move-Item -LiteralPath $signaturePath -Destination $Path -Force
        } finally {
            $envCleanup=[System.Collections.Generic.List[string]]::new()
            try{if($null-eq $oldContent){Remove-Item Env:MIR2_SELFTEST_CMS_CONTENT_PATH -ErrorAction Stop}else{$env:MIR2_SELFTEST_CMS_CONTENT_PATH=$oldContent}}catch{$envCleanup.Add("MIR2_SELFTEST_CMS_CONTENT_PATH: $($_.Exception.Message)")}
            try{if($null-eq $oldSignature){Remove-Item Env:MIR2_SELFTEST_CMS_SIGNATURE_PATH -ErrorAction Stop}else{$env:MIR2_SELFTEST_CMS_SIGNATURE_PATH=$oldSignature}}catch{$envCleanup.Add("MIR2_SELFTEST_CMS_SIGNATURE_PATH: $($_.Exception.Message)")}
            try{if($null-eq $oldThumbprint){Remove-Item Env:MIR2_SELFTEST_CMS_CERT_THUMBPRINT -ErrorAction Stop}else{$env:MIR2_SELFTEST_CMS_CERT_THUMBPRINT=$oldThumbprint}}catch{$envCleanup.Add("MIR2_SELFTEST_CMS_CERT_THUMBPRINT: $($_.Exception.Message)")}
            if($envCleanup.Count){throw ('CMS environment cleanup failed: '+([string]::Join('; ',[string[]]$envCleanup)))}
        }
    } catch { $primaryError=$_.Exception.Message } finally {
        if(Test-Path -LiteralPath $dir){try{Remove-Item -LiteralPath $dir -Recurse -Force -ErrorAction Stop}catch{$cleanupErrors.Add("CMS fixture cleanup $($dir): $($_.Exception.Message)")}}
    }
    if($primaryError -or $cleanupErrors.Count){$messages=[System.Collections.Generic.List[string]]::new();if($primaryError){$messages.Add($primaryError)};foreach($message in $cleanupErrors){$messages.Add($message)};throw ($messages -join '; ')}
}
function New-Case { param([hashtable]$Base,[string]$Name);$copy=@{};foreach($k in $Base.Keys){$copy[$k]=$Base[$k]};$copy.CaptureAttestationPath=Join-Path $outputs ($Name+'.json');$copy.CaptureSignaturePath=Join-Path $outputs ($Name+'.sig');$copy.CaptureSpkiPath=Join-Path $outputs ($Name+'.der');return $copy }
function Quote-ProcessArgument {
    param([AllowEmptyString()][string]$Value)
    if($null -eq $Value){$Value=''}
    if($Value -notmatch '[\s"]'){return $Value}
    return '"' + $Value.Replace('"','\"') + '"'
}
function Invoke-Attester {
    param([hashtable]$Arguments)
    $argv=@('-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-File',$Attester)
    foreach($k in $Arguments.Keys){
        if($Arguments[$k]-is [bool]){
            if($Arguments[$k]){$argv+=('-'+$k)}
        }else{
            $argv+=('-'+$k);$argv+=([string]$Arguments[$k])
        }
    }
    if($PSVersionTable.PSEdition -eq 'Core'){$exe=(Get-Command pwsh -ErrorAction Stop).Source}else{$exe=(Get-Command powershell.exe -ErrorAction Stop).Source}
    $psi=New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName=$exe
    $psi.Arguments=(($argv|ForEach-Object{Quote-ProcessArgument ([string]$_)})-join ' ')
    $psi.UseShellExecute=$false
    $psi.CreateNoWindow=$true
    $psi.WindowStyle=[System.Diagnostics.ProcessWindowStyle]::Hidden
    $psi.RedirectStandardOutput=$true
    $psi.RedirectStandardError=$true
    $child=$null;$primaryError=$null;$cleanupErrors=[System.Collections.Generic.List[string]]::new()
    try{
        $child=New-Object System.Diagnostics.Process
        $child.StartInfo=$psi
        if(-not $child.Start()){throw 'attester child process failed to start.'}
        $stdoutTask=$child.StandardOutput.ReadToEndAsync()
        $stderrTask=$child.StandardError.ReadToEndAsync()
        $child.WaitForExit()
        $out=[string]$stdoutTask.Result
        $err=[string]$stderrTask.Result
        if(-not [string]::IsNullOrEmpty($err)){
            if(-not [string]::IsNullOrEmpty($out)){$out+=[Environment]::NewLine}
            $out+=$err
        }
        $exit=$child.ExitCode
    }catch{$primaryError=$_.Exception.Message}
    finally{
        if($null-ne $child){
            try{if(-not $child.HasExited){$child.Kill();$child.WaitForExit(2000)}}catch{$cleanupErrors.Add("attester child cleanup: $($_.Exception.Message)")}
            try{$child.Dispose()}catch{$cleanupErrors.Add("attester child dispose: $($_.Exception.Message)")}
        }
    }
    if($primaryError-or$cleanupErrors.Count){
        $messages=[System.Collections.Generic.List[string]]::new()
        if($primaryError){$messages.Add($primaryError)}
        foreach($message in $cleanupErrors){$messages.Add($message)}
        throw ($messages -join '; ')
    }
    return [pscustomobject]@{ExitCode=$exit;Output=$out}
}
function Assert-NoOutputs { param([hashtable]$Case);foreach($p in @($Case.CaptureAttestationPath,$Case.CaptureSignaturePath,$Case.CaptureSpkiPath)){Assert-True (-not(Test-Path -LiteralPath $p)) "output residue: $p"} }

if($Help){Write-Host 'Low-load integration-only native capture SelfTest; formal verification is never faked.';exit 0}

$root=Join-Path ([IO.Path]::GetTempPath()) ('mir2-native-attest-selftest-'+[guid]::NewGuid().ToString('N'))
$package=Join-Path $root 'package';$capture=Join-Path $root 'capture';$outputs=Join-Path $root 'outputs';$process=$null;$evidenceCert=$null;$releaseCert=$null
try {
    New-Item -ItemType Directory -Path $package,$capture,$outputs -Force|Out-Null
    $exe=Join-Path $package 'mir2-platform-windows.exe';Copy-Item -LiteralPath (Join-Path $env:WINDIR 'System32\WindowsPowerShell\v1.0\powershell.exe') -Destination $exe
    $manifest=Join-Path $package 'PACKAGE-MANIFEST.json';Write-Utf8Json $manifest ([ordered]@{fixture=$true;file='mir2-platform-windows.exe'});$exeHash=Sha256File $exe;$manifestHash=Sha256File $manifest;$candidate='WN-CANDIDATE-SELFTEST';$revision=('1'*40);$image=Join-Path $capture 'native.png';Write-TestPng $image;Write-Utf8Json (Join-Path $package 'VERSION.json') ([ordered]@{candidate=$candidate;gitRevision=$revision})
    $evidenceCert=New-SelfSignedCertificate -Type Custom -Subject ('CN=Mir2 Capture SelfTest '+[guid]::NewGuid().ToString('N')) -KeyAlgorithm RSA -KeyLength 3072 -KeyExportPolicy Exportable -KeyUsage DigitalSignature -CertStoreLocation 'Cert:\CurrentUser\My'
    $releaseCert=New-SelfSignedCertificate -Type Custom -Subject ('CN=Mir2 Release SelfTest '+[guid]::NewGuid().ToString('N')) -KeyAlgorithm RSA -KeyLength 3072 -KeyExportPolicy Exportable -KeyUsage DigitalSignature -CertStoreLocation 'Cert:\CurrentUser\My'
    $releaseThumb=$releaseCert.Thumbprint;$spki=SpkiSha256 $evidenceCert;$challenge=([guid]::NewGuid().ToString('N')+[guid]::NewGuid().ToString('N')).ToLowerInvariant();$issued=[DateTimeOffset]::UtcNow.AddMinutes(-1);$expires=[DateTimeOffset]::UtcNow.AddMinutes(10)
    $policyPath=Join-Path $root 'native-visual-policy.json';$policy=[ordered]@{schemaVersion='mir2.native-visual-policy.v1';policyId='selftest-integration-only';candidate=$candidate;trustedReleaseSignerThumbprint=$releaseThumb;evidenceSignerSpkiSha256=$spki;challenge=$challenge;challengeIssuedAt=$issued.ToString('o',[Globalization.CultureInfo]::InvariantCulture);challengeExpiresAt=$expires.ToString('o',[Globalization.CultureInfo]::InvariantCulture);challengeAuthority='external-one-time-required'};Write-Utf8Json $policyPath $policy;$env:MIR2_NATIVE_TRUSTED_POLICY_ROOT=$root;$env:MIR2_NATIVE_TRUSTED_POLICY_SHA256=Sha256File $policyPath
    $releasePath=Join-Path $package 'RELEASE-STATEMENT.json';$release=[ordered]@{schema='mir2.windows.release-statement.v1';candidate=$candidate;exeSha256=$exeHash;packageManifestSha256=$manifestHash;packageManifestAggregateSha256=('c'*64);versionSha256=('d'*64);buildAttestationSha256=('e'*64);gitRevision=$revision;worktreeDirty=$false;worktreeStatusSha256=('f'*64)};Write-Utf8Json $releasePath $release;Write-DetachedCms (Join-Path $package 'RELEASE-STATEMENT.p7s') $releaseCert ([IO.File]::ReadAllBytes($releasePath))
    $verificationPath=Join-Path $root 'package-verification.json';$verification=[ordered]@{schema='mir2.windows.package-verification.v4';packageRoot=$package;nonvisual=$true;launchRequested=$false;sourceRepoCheck='unavailable';currentSourceRevision=$revision;attestationPresent=$true;attestationSha256=('e'*64);packageManifestPresent=$true;packageManifestSha256=$manifestHash;packageManifestAggregateSha256=('c'*64);trustedSignerThumbprint=$releaseThumb;detachedSignatureValid=$true;exeSha256=$exeHash;peValid=$true;peImports=@();peDelayImports=@();packageFileCount=6;failures=@();passed=$true;visualAccepted=$false;selfTestOnly=$true;integrationOnly=$true};Write-Utf8Json $verificationPath $verification
    $statePath=Join-Path $capture 'native.json';$process=Start-Process -FilePath $exe -ArgumentList '-NoProfile','-NonInteractive','-Command','Start-Sleep -Seconds 120' -WindowStyle Hidden -PassThru;Start-Sleep -Milliseconds 400;$start=(Get-Process -Id $process.Id -ErrorAction Stop).StartTime.ToUniversalTime();$captured=[DateTimeOffset]::UtcNow
    $state=[ordered]@{schemaVersion='mir2-native-visual-capture-v1';producer='windows-native';runId='selftest-native-capture';scene='login';capturedAt=$captured.ToString('o',[Globalization.CultureInfo]::InvariantCulture);imagePath='native.png';imageSha256=(Sha256File $image);logicalSize=[ordered]@{width=1024;height=768};dpiScale=1.0;uiState='Login';world=$null;build=[ordered]@{sourceRevision=$revision;executableSha256=$exeHash;assetManifestSha256=$manifestHash};challenge=$challenge;producerPid=$process.Id;processStartUtc=$start.ToString('o',[Globalization.CultureInfo]::InvariantCulture)};Write-Utf8Json $statePath $state
    $common=@{PackageRoot=$package;CandidateImagePath=$image;CandidateStatePath=$statePath;ProcessId=$process.Id;TrustedReleaseSignerThumbprint=$releaseThumb;EvidenceSignerThumbprint=$evidenceCert.Thumbprint;PackageVerificationPath=$verificationPath;TrustedPolicyPath=$policyPath;ExpectedChallenge=$challenge;SelfTest=$true;CaptureAttestationPath=(Join-Path $outputs 'positive.json');CaptureSignaturePath=(Join-Path $outputs 'positive.sig');CaptureSpkiPath=(Join-Path $outputs 'positive.der')}
    $positive=Invoke-Attester $common;Assert-True ($positive.ExitCode-eq 0) "stage positive [fixtureExe=$exeHash currentExe=$(Sha256File $exe) verificationExe=$($verification.exeSha256)]: $($positive.Output)";foreach($p in @($common.CaptureAttestationPath,$common.CaptureSignaturePath,$common.CaptureSpkiPath)){Assert-True (Test-Path -LiteralPath $p -PathType Leaf) "positive output missing: $p"}
    $jsonStart=$positive.Output.IndexOf('{');if($jsonStart-lt 0){throw 'positive attester did not emit JSON'};$result=($positive.Output.Substring($jsonStart)|ConvertFrom-Json);Assert-True ($result.formalAcceptance -match 'BLOCKED_UNSIGNED_PACKAGE_VERIFICATION_RACE') 'unsigned verification blocker absent';Assert-True ($result.formalAcceptance -match 'BLOCKED_PATH_BASED_TOCTOU_NO_NOFOLLOW') 'path/no-follow blocker absent';Assert-True ($result.formalAcceptance -match 'BLOCKED_EXTERNAL_CHALLENGE_CONSUMPTION') 'challenge blocker absent';Assert-True ($result.formalAcceptance -notmatch 'Accepted') 'formal result claimed Accepted';Assert-True ($result.challengeConsumption-eq 'external-required-not-proven') 'challenge consumption overstated'
    $signed=[IO.File]::ReadAllBytes($common.CaptureAttestationPath);$sig=[IO.File]::ReadAllBytes($common.CaptureSignaturePath);$rsa=$evidenceCert.PublicKey.Key;Assert-True ($rsa.VerifyData($signed,'SHA256',$sig)) 'attestation signature invalid'
    Write-Host 'stage exact-hash';    $stateGood=[IO.File]::ReadAllBytes($statePath);$state.build.executableSha256=('0'*64);Write-Utf8Json $statePath $state;Assert-True ((Invoke-Attester (New-Case $common 'wrong-hash')).ExitCode-ne 0) 'wrong exact hash accepted';[IO.File]::WriteAllBytes($statePath,$stateGood);$state.build.executableSha256=$exeHash
    Write-Host 'stage strict-json';    [IO.File]::WriteAllText($statePath,'{"schemaVersion":"mir2-native-visual-capture-v1","schemaVersion":"mir2-native-visual-capture-v1"}',[Text.UTF8Encoding]::new($false));Assert-True ((Invoke-Attester (New-Case $common 'duplicate')).ExitCode-ne 0) 'duplicate key accepted';[IO.File]::WriteAllBytes($statePath,$stateGood)
    [IO.File]::WriteAllBytes($statePath,[byte[]](0x7b,0x22,0xff,0x22,0x3a,0x31,0x7d));Assert-True ((Invoke-Attester (New-Case $common 'utf8')).ExitCode-ne 0) 'invalid UTF8 accepted';[IO.File]::WriteAllBytes($statePath,$stateGood)
    $extra=[ordered]@{};foreach($k in $state.Keys){$extra[$k]=$state[$k]};$extra.extra=$true;Write-Utf8Json $statePath $extra;Assert-True ((Invoke-Attester (New-Case $common 'closed')).ExitCode-ne 0) 'closed schema accepted extra key';[IO.File]::WriteAllBytes($statePath,$stateGood)
    Write-Host 'stage verification-bindings';    $stale=[ordered]@{};foreach($k in $verification.Keys){$stale[$k]=$verification[$k]};$stale.exeSha256=('0'*64);$stalePath=Join-Path $root 'stale.json';Write-Utf8Json $stalePath $stale;$staleCase=New-Case $common 'stale';$staleCase.PackageVerificationPath=$stalePath;Assert-True ((Invoke-Attester $staleCase).ExitCode-ne 0) 'stale verification accepted'
    $fake=[ordered]@{};foreach($k in $verification.Keys){$fake[$k]=$verification[$k]};$fake.attestationSha256=('0'*64);$fakePath=Join-Path $root 'fake.json';Write-Utf8Json $fakePath $fake;$fakeCase=New-Case $common 'fake';$fakeCase.PackageVerificationPath=$fakePath;Assert-True ((Invoke-Attester $fakeCase).ExitCode-ne 0) 'fake verification binding accepted'
    Write-Host 'stage cms-pin';    $releaseGood=[IO.File]::ReadAllBytes($releasePath);$bad=[ordered]@{};foreach($k in $release.Keys){$bad[$k]=$release[$k]};$bad.buildAttestationSha256=('0'*64);Write-Utf8Json $releasePath $bad;Write-DetachedCms (Join-Path $package 'RELEASE-STATEMENT.p7s') $releaseCert ([IO.File]::ReadAllBytes($releasePath));Assert-True ((Invoke-Attester (New-Case $common 'bad-release-binding')).ExitCode-ne 0) 'bad release binding accepted';[IO.File]::WriteAllBytes($releasePath,$releaseGood);Write-DetachedCms (Join-Path $package 'RELEASE-STATEMENT.p7s') $releaseCert $releaseGood
    Write-DetachedCms (Join-Path $package 'RELEASE-STATEMENT.p7s') $evidenceCert $releaseGood;Assert-True ((Invoke-Attester (New-Case $common 'wrong-cms-signer')).ExitCode-ne 0) 'wrong CMS signer accepted';Write-DetachedCms (Join-Path $package 'RELEASE-STATEMENT.p7s') $releaseCert $releaseGood
    Write-Host 'stage png';    $pngGood=[IO.File]::ReadAllBytes($image);$broken=Corrupt-IdatWithValidCrc $pngGood;[IO.File]::WriteAllBytes($image,$broken);Assert-True ((Invoke-Attester (New-Case $common 'broken-idat')).ExitCode-ne 0) 'CRC-valid broken IDAT accepted';[IO.File]::WriteAllBytes($image,$pngGood)
    [IO.File]::WriteAllBytes($image,([byte[]]$pngGood[0..99]));Assert-True ((Invoke-Attester (New-Case $common 'truncated')).ExitCode-ne 0) 'truncated PNG accepted';[IO.File]::WriteAllBytes($image,$pngGood)
    [IO.File]::WriteAllBytes($image,(New-Object byte[] (32MB+1)));Assert-True ((Invoke-Attester (New-Case $common 'oversize')).ExitCode-ne 0) 'oversize PNG accepted';[IO.File]::WriteAllBytes($image,$pngGood)
    Write-Host 'stage rollback';    $state.build.executableSha256=$exeHash;$state.build.assetManifestSha256=$manifestHash;$state.capturedAt=([DateTimeOffset]::UtcNow).ToString('o',[Globalization.CultureInfo]::InvariantCulture);Write-Utf8Json $statePath $state;$old=$env:MIR2_NATIVE_ATTEST_SELFTEST_FAIL_AFTER_FIRST;try{$env:MIR2_NATIVE_ATTEST_SELFTEST_FAIL_AFTER_FIRST='1';$rollback=New-Case $common 'rollback';$rr=Invoke-Attester $rollback}finally{if($null-eq $old){Remove-Item Env:MIR2_NATIVE_ATTEST_SELFTEST_FAIL_AFTER_FIRST -ErrorAction SilentlyContinue}else{$env:MIR2_NATIVE_ATTEST_SELFTEST_FAIL_AFTER_FIRST=$old}};Assert-True ($rr.ExitCode-ne 0) 'normal rollback injection passed';Assert-NoOutputs $rollback
    $old=$env:MIR2_NATIVE_ATTEST_SELFTEST_FAIL_AFTER_FIRST;$old2=$env:MIR2_NATIVE_ATTEST_SELFTEST_FAIL_ROLLBACK;try{$env:MIR2_NATIVE_ATTEST_SELFTEST_FAIL_AFTER_FIRST='1';$env:MIR2_NATIVE_ATTEST_SELFTEST_FAIL_ROLLBACK='1';$rollbackFail=New-Case $common 'rollback-failure';$rf=Invoke-Attester $rollbackFail}finally{if($null-eq $old){Remove-Item Env:MIR2_NATIVE_ATTEST_SELFTEST_FAIL_AFTER_FIRST -ErrorAction SilentlyContinue}else{$env:MIR2_NATIVE_ATTEST_SELFTEST_FAIL_AFTER_FIRST=$old};if($null-eq $old2){Remove-Item Env:MIR2_NATIVE_ATTEST_SELFTEST_FAIL_ROLLBACK -ErrorAction SilentlyContinue}else{$env:MIR2_NATIVE_ATTEST_SELFTEST_FAIL_ROLLBACK=$old2}};Assert-True ($rf.ExitCode-ne 0) 'rollback failure passed';Assert-True ($rf.Output-match 'rollba\s*ck failed') 'rollback failure not reported';Assert-True (($rf.Output -replace '\s','').Contains($rollbackFail.CaptureAttestationPath)) 'residual path not reported';Assert-True (Test-Path -LiteralPath $rollbackFail.CaptureAttestationPath) 'residual missing';Assert-True (-not(Test-Path -LiteralPath $rollbackFail.CaptureSignaturePath)) 'rollback failure left signature';Assert-True (-not(Test-Path -LiteralPath $rollbackFail.CaptureSpkiPath)) 'rollback failure left SPKI';Remove-Item -LiteralPath $rollbackFail.CaptureAttestationPath -Force -ErrorAction Stop
    Write-Host 'stage output-preflight';    $partial=New-Case $common 'partial';[IO.File]::WriteAllBytes($partial.CaptureAttestationPath,[byte[]](1,2,3));$pr=Invoke-Attester $partial;Assert-True ($pr.ExitCode-ne 0) 'partial output accepted';Assert-True (-not(Test-Path -LiteralPath $partial.CaptureSignaturePath)) 'partial wrote signature';Assert-True (-not(Test-Path -LiteralPath $partial.CaptureSpkiPath)) 'partial wrote SPKI'
    $overwrite=Invoke-Attester $common;Assert-True ($overwrite.ExitCode-ne 0) 'overwrite accepted'
    Write-Host 'stage formal-block';    $formal=New-Case $common 'formal';$formal.SelfTest=$false;Assert-True ((Invoke-Attester $formal).ExitCode-ne 0) 'formal accepted caller verification path'
    Write-Host 'attest-native-visual-capture SelfTest passed: PS5 host, pwsh CMS child, exact bytes, strict JSON, stale/fake bindings, CMS pin, PNG full decode/CRC/size, formal blockers, rollback, rollback failure, partial/no-overwrite.'
    Write-Host 'Integration-only: formal verifier was not faked; formal acceptance is blocked by policy.'
    exit 0
} catch { Write-Error $_.Exception.Message; exit 1 }
finally {
    $cleanupFailures=New-Object System.Collections.Generic.List[string]
    if($process){try{Stop-Process -Id $process.Id -Force -ErrorAction Stop;Start-Sleep -Milliseconds 100;if(Get-Process -Id $process.Id -ErrorAction SilentlyContinue){[void]$cleanupFailures.Add("process PID $($process.Id) still running")}}catch{[void]$cleanupFailures.Add("process PID $($process.Id): $($_.Exception.Message)")}}
    foreach($cert in @($evidenceCert,$releaseCert)){if($null-ne $cert){$certPath='Cert:\CurrentUser\My\'+$cert.Thumbprint;try{Remove-Item -LiteralPath $certPath -Force -ErrorAction Stop;if(Test-Path -LiteralPath $certPath){[void]$cleanupFailures.Add("certificate remains: $certPath")}}catch{[void]$cleanupFailures.Add("certificate cleanup $certPath failed: $($_.Exception.Message)")}}}
    foreach($name in @('MIR2_NATIVE_TRUSTED_POLICY_ROOT','MIR2_NATIVE_TRUSTED_POLICY_SHA256','MIR2_NATIVE_ATTEST_SELFTEST_FAIL_AFTER_FIRST','MIR2_NATIVE_ATTEST_SELFTEST_FAIL_ROLLBACK','MIR2_SELFTEST_CMS_CONTENT_PATH','MIR2_SELFTEST_CMS_SIGNATURE_PATH','MIR2_SELFTEST_CMS_CERT_THUMBPRINT')){try{Remove-Item -LiteralPath ('Env:\'+$name) -ErrorAction Stop}catch{if(Test-Path -LiteralPath ('Env:\'+$name)){[void]$cleanupFailures.Add("environment cleanup $name failed: $($_.Exception.Message)")}}}
    if(Test-Path -LiteralPath $root){try{Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction Stop;if(Test-Path -LiteralPath $root){[void]$cleanupFailures.Add("temporary root remains: $root")}}catch{[void]$cleanupFailures.Add("temporary root cleanup $root failed: $($_.Exception.Message)")}}
    if($cleanupFailures.Count-gt 0){Write-Error ('SELFTEST cleanup failures: '+([string]::Join('; ',[string[]]$cleanupFailures)));exit 1}
}
