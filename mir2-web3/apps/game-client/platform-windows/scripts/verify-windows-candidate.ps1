# Verify a Windows Candidate package. Default mode is strictly nonvisual.
[CmdletBinding()]
param(
    [string]$PackageRoot = '',
    [string]$TrustedSignerThumbprint = '',
    [switch]$Launch,
    [int]$LaunchTimeoutMs = 45000,
    [switch]$AllowDirtyWorktree,
    [switch]$SelfTest
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$PlayerCombatSoundNames = @('70.wav','71.wav','72.wav','73.wav','80.wav','81.wav','82.wav','83.wav','138.wav','139.wav','144.wav','145.wav','tiger_struck_1.wav','tiger_struck_2.wav','wolf_struck1.wav')
$EntityAtlasClosureScript = Join-Path (Split-Path -Parent $PSCommandPath) 'entity-atlas-closure.ps1'
if (-not (Test-Path -LiteralPath $EntityAtlasClosureScript -PathType Leaf)) { throw "entity atlas closure validator missing: $EntityAtlasClosureScript" }
. $EntityAtlasClosureScript

function Write-Utf8NoBom { param([string]$Path, [string]$Text); [IO.File]::WriteAllText($Path, $Text, [Text.UTF8Encoding]::new($false)) }
function Get-TextSha256 { param([string]$Text); return ([BitConverter]::ToString([Security.Cryptography.SHA256]::Create().ComputeHash([Text.Encoding]::UTF8.GetBytes($Text)))).Replace('-', '') }
function Get-ByteSha256 { param([byte[]]$Bytes); return ([BitConverter]::ToString([Security.Cryptography.SHA256]::Create().ComputeHash($Bytes))).Replace('-', '') }
function ConvertFrom-JsonPreservingDateStrings { param([Parameter(Mandatory = $true)][string]$Text); $command=Get-Command ConvertFrom-Json -ErrorAction Stop;if($command.Parameters.ContainsKey('DateKind')){return($Text|ConvertFrom-Json -DateKind String)};return($Text|ConvertFrom-Json) }
function Get-OrdinalSortedStrings { param([object[]]$Values); $strings=New-Object System.Collections.Generic.List[string];foreach($value in @($Values)){[void]$strings.Add([string]$value)};$result=$strings.ToArray();[Array]::Sort($result,[StringComparer]::Ordinal);return $result }
function Get-OrdinalUniqueStrings { param([object[]]$Values); $set=[Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal);foreach($value in @($Values)){[void]$set.Add([string]$value)};return @(Get-OrdinalSortedStrings -Values $set) }
function Get-ManifestCanonicalText { param([object[]]$Entries);$records=New-Object System.Collections.Generic.List[string];foreach($entry in @($Entries)){[void]$records.Add([string]::Format([Globalization.CultureInfo]::InvariantCulture,"{0}`t{1}`t{2}",$entry.path,$entry.size,$entry.sha256))};$sorted=$records.ToArray();[Array]::Sort($sorted,[StringComparer]::Ordinal);return(($sorted-join"`n")+"`n") }
function Test-ExactProperties { param([object]$Value, [string[]]$Expected); if ($null -eq $Value) { return $false }; $actual=@(Get-OrdinalSortedStrings -Values $Value.PSObject.Properties.Name); $wanted=@(Get-OrdinalSortedStrings -Values $Expected); return (($actual.Count -eq $wanted.Count) -and (($actual -join "`n") -ceq ($wanted -join "`n"))) }
function Test-StructuredBuildContract {
    param([object]$Attestation)
    if (-not (Test-ExactProperties -Value $Attestation.buildCommand -Expected @('executable','toolchain','subcommand','manifestPath','bin','release','locked','target','profile','targetDir','extraArgs'))) { return $false }
    $command=$Attestation.buildCommand
    if ([string]$command.executable -cne 'cargo' -or [string]$command.toolchain -cne '+1.95.0' -or [string]$command.subcommand -cne 'build' -or [string]$command.manifestPath -cne 'apps/game-client/platform-windows/Cargo.toml' -or [string]$command.bin -cne 'mir2-platform-windows') { return $false }
    if (-not ($command.release -is [bool]) -or $command.release -ne $true -or -not ($command.locked -is [bool]) -or $command.locked -ne $true -or [string]$command.target -cne 'x86_64-pc-windows-msvc' -or [string]$command.profile -cne 'release' -or [string]$command.targetDir -cne 'target-attested-windows-candidate' -or @($command.extraArgs).Count -ne 0) { return $false }
    if (-not (Test-ExactProperties -Value $Attestation.pathRemapping -Expected @('enabled','environmentVariable','flags'))) { return $false }
    $remap=$Attestation.pathRemapping; $flags=@($remap.flags)
    if (-not ($remap.enabled -is [bool]) -or $remap.enabled -ne $true -or [string]$remap.environmentVariable -cne 'RUSTFLAGS' -or $flags.Count -ne 2) { return $false }
    foreach ($flag in $flags) { if (-not (Test-ExactProperties -Value $flag -Expected @('sourceToken','destination'))) { return $false } }
    return ([string]$flags[0].sourceToken -ceq '<REPO_ROOT>' -and [string]$flags[0].destination -ceq '.' -and [string]$flags[1].sourceToken -ceq '<CARGO_HOME>' -and [string]$flags[1].destination -ceq 'cargo-home')
}
function Initialize-Pkcs { if($null-eq('System.Security.Cryptography.Pkcs.SignedCms'-as[type])){try{Add-Type -AssemblyName System.Security -ErrorAction Stop}catch{try{Add-Type -AssemblyName System.Security.Cryptography.Pkcs -ErrorAction Stop}catch{throw "CMS/PKCS#7 support unavailable: $($_.Exception.Message)"}}};if($null-eq('System.Security.Cryptography.Pkcs.SignedCms'-as[type])){throw 'CMS/PKCS#7 support unavailable'} }
function Normalize-Thumbprint { param([string]$Thumbprint); return (($Thumbprint -replace '\s','').ToUpperInvariant()) }
function Test-CodeSigningCertificate { param([Security.Cryptography.X509Certificates.X509Certificate2]$Certificate); if($null-eq$Certificate){return $false};$now=[DateTime]::UtcNow;if($Certificate.NotBefore.ToUniversalTime()-gt$now-or$Certificate.NotAfter.ToUniversalTime()-lt$now){return $false};$eku=$Certificate.Extensions|Where-Object{$_.Oid.Value-eq'2.5.29.37'}|Select-Object -First 1;if($null-eq$eku-or @($eku.EnhancedKeyUsages|ForEach-Object{$_.Value})-notcontains'1.3.6.1.5.5.7.3.3'){return $false};$keyUsage=$Certificate.Extensions|Where-Object{$_.Oid.Value-eq'2.5.29.15'}|Select-Object -First 1;return($null-eq$keyUsage-or(($keyUsage.KeyUsages-band[Security.Cryptography.X509Certificates.X509KeyUsageFlags]::DigitalSignature)-ne 0)) }
function New-DetachedCmsSignature { param([byte[]]$Content,[Security.Cryptography.X509Certificates.X509Certificate2]$Certificate); Initialize-Pkcs; $cms=[Security.Cryptography.Pkcs.SignedCms]::new([Security.Cryptography.Pkcs.ContentInfo]::new($Content),$true); $signer=[Security.Cryptography.Pkcs.CmsSigner]::new([Security.Cryptography.Pkcs.SubjectIdentifierType]::IssuerAndSerialNumber,$Certificate); $signer.IncludeOption=[Security.Cryptography.X509Certificates.X509IncludeOption]::EndCertOnly; $cms.ComputeSignature($signer,$false); return $cms.Encode() }
function Test-DetachedCmsSignature {
    param([byte[]]$Content,[byte[]]$Signature,[string]$TrustedThumbprint)
    try { Initialize-Pkcs; $trusted=Normalize-Thumbprint $TrustedThumbprint; if ($trusted -notmatch '^[0-9A-F]{40}$') { return $false }; $cms=[Security.Cryptography.Pkcs.SignedCms]::new([Security.Cryptography.Pkcs.ContentInfo]::new($Content),$true); $cms.Decode($Signature); if ($cms.SignerInfos.Count -ne 1) { return $false }; $cms.CheckSignature($true); $certificate=$cms.SignerInfos[0].Certificate; return ($null -ne $certificate -and (Normalize-Thumbprint $certificate.Thumbprint) -ceq $trusted -and (Test-CodeSigningCertificate -Certificate $certificate)) } catch { return $false }
}
function New-ReleaseStatementText {
    param([string]$Candidate,[string]$ExeSha256,[string]$ManifestSha256,[string]$ManifestAggregateSha256,[string]$VersionSha256,[string]$AttestationSha256,[string]$GitRevision,[bool]$WorktreeDirty,[string]$DirtyDigest)
    foreach($hash in @($ExeSha256,$ManifestSha256,$ManifestAggregateSha256,$VersionSha256,$AttestationSha256,$DirtyDigest)){if($hash -notmatch '^[0-9A-F]{64}$'){throw 'release statement received an invalid SHA256'}}; if($Candidate -notmatch '^WN-CANDIDATE-[A-Za-z0-9._-]+$' -or $GitRevision -notmatch '^[0-9a-f]{40}$'){throw 'release statement identity is invalid'}; $dirty=if($WorktreeDirty){'true'}else{'false'}; return '{"schema":"mir2.windows.release-statement.v1","candidate":"'+$Candidate+'","exeSha256":"'+$ExeSha256+'","packageManifestSha256":"'+$ManifestSha256+'","packageManifestAggregateSha256":"'+$ManifestAggregateSha256+'","versionSha256":"'+$VersionSha256+'","buildAttestationSha256":"'+$AttestationSha256+'","gitRevision":"'+$GitRevision+'","worktreeDirty":'+$dirty+',"worktreeStatusSha256":"'+$DirtyDigest+'"}'
}
function Invoke-GitBytes { param([string]$Root, [string]$Arguments); $info = [Diagnostics.ProcessStartInfo]::new(); $info.FileName = 'git.exe'; $info.Arguments = $Arguments; $info.WorkingDirectory = $Root; $info.UseShellExecute = $false; $info.CreateNoWindow = $true; $info.RedirectStandardOutput = $true; $info.RedirectStandardError = $true; $process = [Diagnostics.Process]::new(); $process.StartInfo = $info; if (-not $process.Start()) { throw 'failed to start git' }; $memory = [IO.MemoryStream]::new(); try { $process.StandardOutput.BaseStream.CopyTo($memory); $stderr = $process.StandardError.ReadToEnd(); $process.WaitForExit(); if ($process.ExitCode -ne 0) { throw "git failed ($($process.ExitCode)): $stderr" }; return ,$memory.ToArray() } finally { $memory.Dispose(); $process.Dispose() } }
function ConvertFrom-NulUtf8 { param([byte[]]$Bytes); $result = New-Object System.Collections.Generic.List[string]; $start = 0; $utf8 = [Text.UTF8Encoding]::new($false, $true); for ($i = 0; $i -lt $Bytes.Length; $i++) { if ($Bytes[$i] -eq 0) { if ($i -gt $start) { [void]$result.Add($utf8.GetString($Bytes, $start, $i - $start)) }; $start = $i + 1 } }; if ($start -ne $Bytes.Length) { throw 'NUL-delimited git output is unterminated' }; return @($result) }
function Resolve-FullPath { param([string]$Path); return (Resolve-Path -LiteralPath $Path -ErrorAction Stop).Path.TrimEnd('\', '/') }

function Find-RepoRoot {
    param([string]$StartPath)
    $cursor = (Get-Item -LiteralPath $StartPath).FullName
    while ($true) {
        if ((Test-Path -LiteralPath (Join-Path $cursor '.git')) -or ((Test-Path -LiteralPath (Join-Path $cursor 'apps')) -and (Test-Path -LiteralPath (Join-Path $cursor 'docs')) -and (Test-Path -LiteralPath (Join-Path $cursor 'Cargo.toml')))) { return $cursor.TrimEnd('\', '/') }
        $parent = Split-Path -Parent $cursor
        if ([string]::IsNullOrWhiteSpace($parent) -or $parent -eq $cursor) { throw "repository root not found from $StartPath" }
        $cursor = $parent
    }
}

function Test-PathWithin {
    param([string]$Path, [string]$Root, [switch]$AllowRoot)
    $pathFull = [IO.Path]::GetFullPath($Path).TrimEnd('\', '/'); $rootFull = [IO.Path]::GetFullPath($Root).TrimEnd('\', '/')
    if ($AllowRoot -and $pathFull.Equals($rootFull, [StringComparison]::OrdinalIgnoreCase)) { return $true }
    return $pathFull.StartsWith($rootFull + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase) -or $pathFull.StartsWith($rootFull + [IO.Path]::AltDirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)
}

function Test-IsReparsePoint { param([IO.FileSystemInfo]$Item); return (($Item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) }

function Assert-NoReparseAncestors {
    param([string]$Path)
    $cursor = [IO.Path]::GetFullPath($Path).TrimEnd('\', '/')
    while (-not (Test-Path -LiteralPath $cursor)) { $parent = Split-Path -Parent $cursor; if ([string]::IsNullOrWhiteSpace($parent) -or $parent -eq $cursor) { break }; $cursor = $parent }
    while (-not [string]::IsNullOrWhiteSpace($cursor)) {
        if (Test-Path -LiteralPath $cursor) { $item = Get-Item -LiteralPath $cursor -Force; if (Test-IsReparsePoint -Item $item) { throw "reparse-point ancestor rejected: $($item.FullName)" } }
        $parent = Split-Path -Parent $cursor; if ([string]::IsNullOrWhiteSpace($parent) -or $parent -eq $cursor) { break }; $cursor = $parent
    }
}

function Assert-NoReparseTree {
    param([string]$Path)
    if (-not (Test-Path -LiteralPath $Path)) { return }
    Assert-NoReparseAncestors -Path $Path
    $root = Get-Item -LiteralPath $Path -Force; if (Test-IsReparsePoint -Item $root) { throw "reparse root rejected: $($root.FullName)" }
    $stack = New-Object System.Collections.Generic.Stack[string]; if ($root.PSIsContainer) { $stack.Push($root.FullName) }
    while ($stack.Count -gt 0) { foreach ($child in Get-ChildItem -LiteralPath $stack.Pop() -Force) { if (Test-IsReparsePoint -Item $child) { throw "reparse point in package rejected: $($child.FullName)" }; if ($child.PSIsContainer) { $stack.Push($child.FullName) } } }
}

function Initialize-NativeStreamEnumerator {
    if($null-ne('Mir2.Windows.NativeStreamEnumerator'-as[type])){return}
    $source=@'
using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Runtime.InteropServices;
namespace Mir2.Windows {
    public static class NativeStreamEnumerator {
        [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
        private struct StreamData { public long StreamSize; [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 296)] public string StreamName; }
        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)] private static extern IntPtr FindFirstStreamW(string fileName, int infoLevel, out StreamData data, uint flags);
        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)] [return: MarshalAs(UnmanagedType.Bool)] private static extern bool FindNextStreamW(IntPtr handle, out StreamData data);
        [DllImport("kernel32.dll", SetLastError = true)] [return: MarshalAs(UnmanagedType.Bool)] private static extern bool FindClose(IntPtr handle);
        public static string[] List(string path) { var names = new List<string>(); StreamData data; IntPtr handle = FindFirstStreamW(path, 0, out data, 0); if (handle == new IntPtr(-1)) { int error = Marshal.GetLastWin32Error(); if (error == 38) return names.ToArray(); throw new Win32Exception(error, "FindFirstStreamW failed for " + path); } try { names.Add(data.StreamName); while (FindNextStreamW(handle, out data)) names.Add(data.StreamName); int error = Marshal.GetLastWin32Error(); if (error != 38) throw new Win32Exception(error, "FindNextStreamW failed for " + path); } finally { FindClose(handle); } return names.ToArray(); }
    }
}
'@
    try{Add-Type -TypeDefinition $source -Language CSharp -ErrorAction Stop}catch{throw "native ADS enumeration facility unavailable: $($_.Exception.Message)"}
}

function Assert-NoAlternateDataStreams {
    param([string]$Path)
    if($env:OS-ne'Windows_NT'){throw 'ADS enumeration is mandatory and unavailable on this non-Windows host'}
    $resolved=(Resolve-Path -LiteralPath $Path -ErrorAction Stop).Path
    try{$drive=[IO.DriveInfo]::new([IO.Path]::GetPathRoot($resolved));$format=$drive.DriveFormat}catch{throw "ADS filesystem capability cannot be established for $resolved : $($_.Exception.Message)"}
    if($format-ne'NTFS'){throw "ADS enumeration requires NTFS and fails closed on '$format': $resolved"};Initialize-NativeStreamEnumerator
    $rootItem=Get-Item -LiteralPath $resolved -Force -ErrorAction Stop;$items=New-Object System.Collections.Generic.List[IO.FileSystemInfo];[void]$items.Add($rootItem);if($rootItem.PSIsContainer){foreach($child in Get-ChildItem -LiteralPath $resolved -Recurse -Force -ErrorAction Stop){[void]$items.Add($child)}}
    foreach($item in $items){try{$providerStreams=@(Microsoft.PowerShell.Management\Get-Item -LiteralPath $item.FullName -Stream * -Force -ErrorAction Stop);$nativeStreams=@([Mir2.Windows.NativeStreamEnumerator]::List($item.FullName))}catch{throw "ADS enumeration unavailable for $($item.FullName): $($_.Exception.Message)"};foreach($stream in $providerStreams){if([string]$stream.Stream-cne':$DATA'){throw "named NTFS stream rejected: $($item.FullName):$($stream.Stream)"}};foreach($streamName in $nativeStreams){if([string]$streamName-cne'::$DATA'-and[string]$streamName-cne':$DATA'){throw "named NTFS stream rejected: $($item.FullName)$streamName"}}}
}

function Remove-SafeTemporaryTree {
    param([string]$Path, [string]$RequiredPrefix)
    $tempBase = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\', '/')
    $target = [IO.Path]::GetFullPath($Path).TrimEnd('\', '/')
    if (-not (Test-PathWithin -Path $target -Root $tempBase) -or (Split-Path -Leaf $target) -notlike ($RequiredPrefix + '*')) { throw "unsafe temporary cleanup target: $target" }
    Assert-NoReparseAncestors -Path $target
    Assert-NoReparseTree -Path $target
    Remove-Item -LiteralPath $target -Recurse -Force
}

function Get-RelativeUnixPath {
    param([string]$Root, [string]$Path)
    $rootFull = [IO.Path]::GetFullPath($Root).TrimEnd('\', '/'); $pathFull = [IO.Path]::GetFullPath($Path)
    if (-not (Test-PathWithin -Path $pathFull -Root $rootFull)) { throw "path outside package: $pathFull" }
    return $pathFull.Substring($rootFull.Length).TrimStart('\', '/') -replace '\\', '/'
}

function Get-ManifestPayloadFiles {
    param([string]$Root)
    $excluded=@('PACKAGE-MANIFEST.json','VERSION.json','RELEASE-STATEMENT.json','RELEASE-STATEMENT.p7s')
    $map=[Collections.Generic.Dictionary[string,IO.FileInfo]]::new([StringComparer]::Ordinal);$paths=New-Object System.Collections.Generic.List[string];foreach($file in Get-ChildItem -LiteralPath $Root -Recurse -File -Force){$rel=Get-RelativeUnixPath -Root $Root -Path $file.FullName;if($excluded -cnotcontains $rel){if($map.ContainsKey($rel)){throw "duplicate package path: $rel"};$map.Add($rel,$file);[void]$paths.Add($rel)}};$pathArray=$paths.ToArray();[Array]::Sort($pathArray,[StringComparer]::Ordinal);return @($pathArray|ForEach-Object{$map[$_]})
}

function Get-OrdinalPackageFiles {
    param([string]$Root)
    $map=[Collections.Generic.Dictionary[string,IO.FileInfo]]::new([StringComparer]::Ordinal);$paths=New-Object System.Collections.Generic.List[string]
    foreach($file in Get-ChildItem -LiteralPath $Root -Recurse -File -Force){$rel=Get-RelativeUnixPath -Root $Root -Path $file.FullName;if($map.ContainsKey($rel)){throw "duplicate package path: $rel"};$map.Add($rel,$file);[void]$paths.Add($rel)}
    $pathArray=$paths.ToArray();[Array]::Sort($pathArray,[StringComparer]::Ordinal);return @($pathArray|ForEach-Object{$map[$_]})
}

function Test-PathContainsDangerousDotToken {
    param([string]$RelativePath)
    $dangerous=[Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach($token in @('exe','dll','com','scr','cpl','msi','msp','bat','cmd','ps1','psm1','psd1','vbs','vbe','js','jse','wsf','wsh','hta','reg','lnk','chm','jar','py','pyw','sh','bash','zsh','fish','pdb','ilk','dmp')){[void]$dangerous.Add($token)}
    foreach($segment in @($RelativePath -split '[\\/]')){if($segment.Length-gt 0-and($segment[$segment.Length-1]-eq[char]0x20-or$segment[$segment.Length-1]-eq[char]0x2E)){return $true};$dotTokens=$segment.Split([char]'.');foreach($dotToken in $dotTokens){if($dotToken.Length-gt 0-and$dotToken[$dotToken.Length-1]-eq[char]0x20){return $true}};for($index=1;$index-lt$dotTokens.Count;$index++){if($dangerous.Contains($dotTokens[$index])){return $true}}}
    return $false
}

function Test-PackageRelativeFileAllowed {
    param([string]$RelativePath,[string]$ExeName)
    if($RelativePath -ceq $ExeName){return $true}
    if(Test-PathContainsDangerousDotToken -RelativePath $RelativePath){return $false}
    $rootFiles=@('mir2-client.toml','README-START.txt','CONTROLS.txt','KNOWN-ISSUES.md','BUILD-ATTESTATION.json','PACKAGE-MANIFEST.json','VERSION.json','RELEASE-STATEMENT.json','RELEASE-STATEMENT.p7s')
    if($rootFiles -ccontains $RelativePath){return $true};if($RelativePath -ceq 'mir2-assets/original-ui/frame-sets.generated.json'){return $true}
    if(@('mir2-assets/original-ui/Sound/Login2.wav','mir2-assets/original-ui/Sound/Select2.wav','mir2-assets/original-ui/Sound/103.wav','mir2-assets/original-ui/Sound/70.wav','mir2-assets/original-ui/Sound/71.wav','mir2-assets/original-ui/Sound/72.wav','mir2-assets/original-ui/Sound/73.wav','mir2-assets/original-ui/Sound/80.wav','mir2-assets/original-ui/Sound/81.wav','mir2-assets/original-ui/Sound/82.wav','mir2-assets/original-ui/Sound/83.wav','mir2-assets/original-ui/Sound/138.wav','mir2-assets/original-ui/Sound/139.wav','mir2-assets/original-ui/Sound/144.wav','mir2-assets/original-ui/Sound/145.wav','mir2-assets/original-ui/Sound/tiger_struck_1.wav','mir2-assets/original-ui/Sound/tiger_struck_2.wav','mir2-assets/original-ui/Sound/wolf_struck1.wav','mir2-assets/original-ui/Sound/M8-1.wav','mir2-assets/original-ui/Sound/M31-0.wav','mir2-assets/original-ui/Sound/M31-1.wav','mir2-assets/original-ui/Sound/M31-2.wav','mir2-assets/original-ui/Sound/M34-0.wav','mir2-assets/original-ui/Sound/M34-1.wav','mir2-assets/original-ui/Sound/M34-2.wav','mir2-assets/original-ui/Sound/M39-0.wav','mir2-assets/original-ui/Sound/M39-1.wav','mir2-assets/original-ui/Sound/M40-0.wav','mir2-assets/original-ui/Sound/M61-0.wav','mir2-assets/original-ui/Sound/M61-1.wav','mir2-assets/original-ui/Sound/M64-0.wav','mir2-assets/original-ui/Sound/M64-1.wav','mir2-assets/original-ui/Sound/M64-2.wav','mir2-assets/original-ui/Sound/M79-1.wav') -ccontains $RelativePath){return $true}
    if($RelativePath.StartsWith('mir2-assets/crystal-map-pack/',[StringComparison]::Ordinal)){return $RelativePath.EndsWith('.map.gz',[StringComparison]::OrdinalIgnoreCase)}
    $imageJsonRoots=@('mir2-assets/bevy-entity-atlases/','mir2-assets/generated/map-atlas/','mir2-assets/generated/native-map-keyed/','mir2-assets/original-effects/','mir2-assets/original-ui/ChrSel/','mir2-assets/original-ui/Help/','mir2-assets/original-ui/MMap/','mir2-assets/original-ui/Prguse/','mir2-assets/original-ui/Prguse2/','mir2-assets/original-ui/UI_32bit/','mir2-assets/original-ui/Title/','mir2-assets/original-ui/AArmour/00/','mir2-assets/original-ui/Monster/000/','mir2-assets/original-ui/NPC/00/')
    foreach($prefix in $imageJsonRoots){if($RelativePath.StartsWith($prefix,[StringComparison]::Ordinal)){return $RelativePath.EndsWith('.json',[StringComparison]::OrdinalIgnoreCase)-or$RelativePath.EndsWith('.png',[StringComparison]::OrdinalIgnoreCase)}}
    return $false
}

function Test-PackageRelativeDirectoryAllowed {
    param([string]$RelativePath)
    if(Test-PathContainsDangerousDotToken -RelativePath $RelativePath){return $false}
    if(@('logs','mir2-assets','mir2-assets/generated','mir2-assets/original-ui','mir2-assets/original-ui/AArmour','mir2-assets/original-ui/Monster','mir2-assets/original-ui/NPC','mir2-assets/original-ui/Sound') -ccontains $RelativePath){return $true}
    $treeRoots=@('mir2-assets/bevy-entity-atlases','mir2-assets/generated/map-atlas','mir2-assets/generated/native-map-keyed','mir2-assets/crystal-map-pack','mir2-assets/original-effects','mir2-assets/original-ui/ChrSel','mir2-assets/original-ui/Help','mir2-assets/original-ui/MMap','mir2-assets/original-ui/Prguse','mir2-assets/original-ui/Prguse2','mir2-assets/original-ui/UI_32bit','mir2-assets/original-ui/Title','mir2-assets/original-ui/AArmour/00','mir2-assets/original-ui/Monster/000','mir2-assets/original-ui/NPC/00')
    foreach($root in $treeRoots){if($RelativePath -ceq $root -or $RelativePath.StartsWith($root+'/',[StringComparison]::Ordinal)){return $true}}
    return $false
}

function Assert-PackageAllowlist {
    param([string]$Root,[string]$ExeName)
    foreach($directory in Get-ChildItem -LiteralPath $Root -Recurse -Directory -Force){$rel=Get-RelativeUnixPath -Root $Root -Path $directory.FullName;if(-not(Test-PackageRelativeDirectoryAllowed -RelativePath $rel)){throw "package directory outside strict allowlist: $rel"}}
    foreach($file in Get-ChildItem -LiteralPath $Root -Recurse -File -Force){$rel=Get-RelativeUnixPath -Root $Root -Path $file.FullName;if(-not(Test-PackageRelativeFileAllowed -RelativePath $rel -ExeName $ExeName)){throw "package file outside strict path/extension allowlist: $rel"}}
}

function Get-WorktreeState {
    param([string]$Root)
    Push-Location $Root
    try {
        $revision = (& git rev-parse HEAD 2>$null).Trim().ToLowerInvariant(); if ($LASTEXITCODE -ne 0 -or $revision -notmatch '^[0-9a-f]{40}$') { throw 'git HEAD unavailable' }
        $gitOptions = '-c core.quotepath=false -c core.autocrlf=false -c core.safecrlf=false'; $statusBytes = Invoke-GitBytes -Root $Root -Arguments "$gitOptions status --porcelain=v1 -z --untracked-files=all"; $indexDiffBytes = Invoke-GitBytes -Root $Root -Arguments "$gitOptions diff --cached --no-ext-diff --binary --full-index --"; $worktreeDiffBytes = Invoke-GitBytes -Root $Root -Arguments "$gitOptions diff --no-ext-diff --binary --full-index --"; $untrackedPaths = @(ConvertFrom-NulUtf8 -Bytes (Invoke-GitBytes -Root $Root -Arguments "$gitOptions ls-files --others --exclude-standard -z"))
        $records = New-Object System.Collections.Generic.List[string]; $seen = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
        foreach ($gitPath in $untrackedPaths) { $normalized = $gitPath.Normalize([Text.NormalizationForm]::FormC).Replace('\', '/'); if ([IO.Path]::IsPathRooted($normalized) -or $normalized -eq '..' -or $normalized.StartsWith('../') -or $normalized.Contains('/../')) { throw "unsafe untracked path: $gitPath" }; if (-not $seen.Add($normalized)) { throw "duplicate normalized untracked path: $normalized" }; $full = [IO.Path]::GetFullPath((Join-Path $Root ($gitPath -replace '/', '\'))); if (-not (Test-PathWithin -Path $full -Root $Root)) { throw "untracked path escapes repository: $normalized" }; Assert-NoReparseAncestors -Path $full; if (-not (Test-Path -LiteralPath $full -PathType Leaf)) { throw "untracked regular file changed during digest: $normalized" }; $item = Get-Item -LiteralPath $full -Force; if (Test-IsReparsePoint -Item $item) { throw "untracked reparse file rejected: $normalized" }; $pathBase64 = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($normalized)); [void]$records.Add(("{0}`t{1}`t{2}" -f $pathBase64, [int64]$item.Length, (Get-FileHash -LiteralPath $full -Algorithm SHA256).Hash.ToUpperInvariant())) }
        $recordArray = $records.ToArray(); [Array]::Sort($recordArray, [StringComparer]::Ordinal); $scope = 'git-status-z+diff+all-untracked-content-v2'; $canonical = "SCOPE`n$scope`nREVISION`n$revision`nSTATUS-Z`n$($statusBytes.Length)`n$(Get-ByteSha256 $statusBytes)`nINDEX-DIFF`n$($indexDiffBytes.Length)`n$(Get-ByteSha256 $indexDiffBytes)`nWORKTREE-DIFF`n$($worktreeDiffBytes.Length)`n$(Get-ByteSha256 $worktreeDiffBytes)`nUNTRACKED`n$($recordArray.Count)`n" + ($recordArray -join "`n") + "`n"
        return [ordered]@{ revision = $revision; dirty = ($statusBytes.Length -gt 0); statusLineCount = @(ConvertFrom-NulUtf8 -Bytes $statusBytes).Count; statusScope = $scope; statusSha256 = Get-TextSha256 -Text $canonical }
    } finally { Pop-Location }
}

function Read-PeInfo {
    param([string]$Path)
    $stream = [IO.File]::OpenRead($Path); $reader = [IO.BinaryReader]::new($stream)
    try {
        if ($reader.ReadUInt16() -ne 0x5A4D) { throw 'missing MZ signature' }
        $stream.Position = 0x3c; $peOffset = $reader.ReadInt32(); if ($peOffset -lt 64 -or $peOffset -gt $stream.Length - 256) { throw 'invalid PE offset' }; $stream.Position = $peOffset
        if ($reader.ReadUInt32() -ne 0x00004550) { throw 'missing PE signature' }
        [void]$reader.ReadUInt16(); $sectionCount = $reader.ReadUInt16(); [void]$reader.ReadBytes(12); $optionalSize = $reader.ReadUInt16(); [void]$reader.ReadUInt16(); $optionalStart = $stream.Position; $magic = $reader.ReadUInt16()
        if ($magic -eq 0x10b) { $imageBaseOffset = $optionalStart + 28; $dataDirectoryStart = $optionalStart + 96; $numberDirectoryOffset = $optionalStart + 92; $is64 = $false }
        elseif ($magic -eq 0x20b) { $imageBaseOffset = $optionalStart + 24; $dataDirectoryStart = $optionalStart + 112; $numberDirectoryOffset = $optionalStart + 108; $is64 = $true }
        else { throw 'unknown PE optional header' }
        $stream.Position = $imageBaseOffset; $imageBase = if ($is64) { $reader.ReadUInt64() } else { [uint64]$reader.ReadUInt32() }
        $stream.Position = $numberDirectoryOffset; $directoryCount = $reader.ReadUInt32(); if ($directoryCount -lt 14) { throw 'PE data directories do not include delay imports' }
        $stream.Position = $dataDirectoryStart + 8; $importRva = $reader.ReadUInt32(); [void]$reader.ReadUInt32()
        $stream.Position = $dataDirectoryStart + (13 * 8); $delayRva = $reader.ReadUInt32(); [void]$reader.ReadUInt32()
        if ($sectionCount -lt 1 -or $sectionCount -gt 96) { throw 'invalid PE section count' }
        $sectionStart = $optionalStart + $optionalSize
        if ([uint64]$sectionStart + ([uint64]40 * $sectionCount) -gt [uint64]$stream.Length) { throw 'PE section table exceeds file bounds' }
        $sections = @()
        for ($i = 0; $i -lt $sectionCount; $i++) {
            $stream.Position = $sectionStart + (40 * $i)
            $name = ([Text.Encoding]::ASCII.GetString($reader.ReadBytes(8))).Trim([char]0)
            $virtualSize = $reader.ReadUInt32(); $virtualAddress = $reader.ReadUInt32(); $rawSize = $reader.ReadUInt32(); $rawPointer = $reader.ReadUInt32()
            [void]$reader.ReadBytes(12); $characteristics = $reader.ReadUInt32()
            if ($rawSize -gt 0 -and ($rawPointer -eq 0 -or [uint64]$rawPointer + [uint64]$rawSize -gt [uint64]$stream.Length)) { throw "PE section '$name' raw range exceeds file bounds" }
            $sections += [ordered]@{ name = $name; virtualSize = $virtualSize; virtualAddress = $virtualAddress; rawSize = $rawSize; rawPointer = $rawPointer; characteristics = $characteristics }
        }
        function Convert-RvaToOffset([uint32]$Rva) { foreach ($section in $sections) { $span = [Math]::Max([uint32]$section.virtualSize, [uint32]$section.rawSize); if ($Rva -ge $section.virtualAddress -and $Rva -lt ([uint64]$section.virtualAddress + $span)) { return [int64]$section.rawPointer + ($Rva - $section.virtualAddress) } }; throw "RVA 0x$('{0:X8}' -f $Rva) outside PE sections" }
        function Read-AsciiZ([uint32]$Rva) { $stream.Position = Convert-RvaToOffset $Rva; $bytes = New-Object System.Collections.Generic.List[byte]; for ($i = 0; $i -lt 1024; $i++) { $b = $reader.ReadByte(); if ($b -eq 0) { return [Text.Encoding]::ASCII.GetString($bytes.ToArray()) }; [void]$bytes.Add($b) }; throw 'unterminated PE import name' }
        $imports = New-Object System.Collections.Generic.List[string]
        if ($importRva -ne 0) { $offset = Convert-RvaToOffset $importRva; for ($i = 0; $i -lt 4096; $i++) { $stream.Position = $offset + (20 * $i); $fields = @($reader.ReadUInt32(), $reader.ReadUInt32(), $reader.ReadUInt32(), $reader.ReadUInt32(), $reader.ReadUInt32()); if (($fields | Measure-Object -Sum).Sum -eq 0) { break }; if ($fields[3] -eq 0) { throw 'PE import descriptor has no DLL name' }; [void]$imports.Add((Read-AsciiZ $fields[3])); if ($i -eq 4095) { throw 'PE import descriptor limit exceeded' } } }
        $delayImports = New-Object System.Collections.Generic.List[string]
        if ($delayRva -ne 0) { $offset = Convert-RvaToOffset $delayRva; for ($i = 0; $i -lt 4096; $i++) { $stream.Position = $offset + (32 * $i); $fields = @(); for ($j = 0; $j -lt 8; $j++) { $fields += $reader.ReadUInt32() }; if (($fields | Measure-Object -Sum).Sum -eq 0) { break }; $nameValue = [uint64]$fields[1]; if (($fields[0] -band 1) -eq 0) { if ($nameValue -lt $imageBase -or ($nameValue - $imageBase) -gt [uint32]::MaxValue) { throw 'invalid VA-based delay import name' }; $nameValue -= $imageBase }; [void]$delayImports.Add((Read-AsciiZ ([uint32]$nameValue))); if ($i -eq 4095) { throw 'PE delay-import descriptor limit exceeded' } } }
        return [ordered]@{ valid = $true; imports = @(Get-OrdinalUniqueStrings -Values $imports); delayImports = @(Get-OrdinalUniqueStrings -Values $delayImports); sections = @($sections) }
    } finally { $reader.Dispose(); $stream.Dispose() }
}

function Test-SystemDependency {
    param([string]$Name)
    $lower = $Name.ToLowerInvariant()
    if ($lower -like 'api-ms-win-*.dll' -or $lower -like 'ext-ms-win-*.dll') { return $true }
    $allowed = @('advapi32.dll','avrt.dll','bcrypt.dll','bcryptprimitives.dll','cfgmgr32.dll','combase.dll','comctl32.dll','comdlg32.dll','crypt32.dll','d3d11.dll','d3d12.dll','d3dcompiler_47.dll','dbghelp.dll','dwmapi.dll','dxgi.dll','gdi32.dll','hid.dll','imm32.dll','iphlpapi.dll','kernel32.dll','ksuser.dll','mf.dll','mfplat.dll','mfreadwrite.dll','mpr.dll','msvcp140.dll','ntdll.dll','ole32.dll','oleaut32.dll','powrprof.dll','propsys.dll','runtimeobject.dll','secur32.dll','setupapi.dll','shell32.dll','shlwapi.dll','uiautomationcore.dll','user32.dll','ucrtbase.dll','uxtheme.dll','version.dll','vcruntime140.dll','vcruntime140_1.dll','winhttp.dll','winmm.dll','wintrust.dll','ws2_32.dll')
    return $allowed -contains $lower
}

function New-BuildPathLeakRegex {
    param([switch]$Unicode)
    # ISO-8859-1 gives a one-byte-to-one-char view. Its ASCII pattern excludes
    # bytes >= 0x80, so arbitrary machine bytes cannot become replacement '?'
    # characters and accidentally form a path. UTF-8 and UTF-16 use the Unicode
    # pattern, which excludes controls, surrogates and decoder U+FFFD markers.
    $pathChar = if ($Unicode) { '[^\p{C}\p{Zl}\p{Zp}\uFFFD<>:"/\\|?*]' } else { '[\x20-\x21\x23-\x29\x2B-\x2E\x30-\x39\x3B\x3D\x40-\x5B\x5D-\x7B\x7D-\x7E]' }
    $segment = "(?:$pathChar){1,255}"
    $relative = "(?:$segment[\\/]){0,32}$segment"
    $hostName = '[A-Za-z0-9_](?:[A-Za-z0-9._-]{0,126}[A-Za-z0-9_])?'
    $minimumDriveTail = "(?=(?:$pathChar){3}|(?:$pathChar){1,255}[\\/])"
    $patterns = @(
        "(?<![A-Za-z0-9])[A-Z]:[\\/]$minimumDriveTail$relative",
        "\\\\[?.]\\(?:[A-Z]:\\$relative|UNC\\$hostName\\$segment(?:\\$segment){0,32})",
        "(?<!\\)\\\\$hostName\\$segment(?:\\$segment){0,32}",
        "(?<!:)//(?!rustc/[0-9a-f]{40}/library(?=$|[\\/]|[\x00-\x1F\x7F]))$hostName/$segment(?:/$segment){0,32}",
        "(?<![A-Za-z0-9:])/(?:var/tmp|home|Users|workspace|workspaces|build|builds|src|tmp|opt|private|mnt|Volumes)(?:/$segment){0,32}",
        "(?<!$pathChar)(?:$segment[\\/:]){1,32}$segment\.pdb\b"
    )
    $options = [Text.RegularExpressions.RegexOptions]::IgnoreCase -bor [Text.RegularExpressions.RegexOptions]::CultureInvariant
    return [regex]::new(('(?:' + ($patterns -join '|') + ')'), $options, [TimeSpan]::FromSeconds(30))
}

function Assert-NoBuildPathStrings {
    param([string]$ExePath, [object]$PeInfo = $null)
    $bytes = [IO.File]::ReadAllBytes($ExePath)
    $ranges = New-Object System.Collections.Generic.List[object]
    if ($null -eq $PeInfo) {
        [void]$ranges.Add([pscustomobject]@{ name = '<raw>'; offset = 0; count = $bytes.Length })
    } else {
        $sections = @($PeInfo.sections)
        if ($sections.Count -eq 0) { throw 'PE path inspection has no section metadata' }
        foreach ($section in $sections) {
            $offset = [int64]$section.rawPointer; $count = [int64]$section.rawSize
            if ($count -eq 0 -or (([uint32]$section.characteristics -band [uint32]0x20000000) -ne 0)) { continue }
            if ($offset -lt 0 -or $count -lt 0 -or [uint64]$offset + [uint64]$count -gt [uint64]$bytes.LongLength) { throw "PE section '$($section.name)' path-scan range exceeds file bounds" }
            [void]$ranges.Add([pscustomobject]@{ name = [string]$section.name; offset = [int]$offset; count = [int]$count })
        }
        if ($ranges.Count -eq 0) { throw 'PE path inspection has no non-executable section data' }
    }

    $asciiRegex = New-BuildPathLeakRegex
    $unicodeRegex = New-BuildPathLeakRegex -Unicode
    $singleByteEncoding = [Text.Encoding]::GetEncoding(28591)
    $utf8Encoding = [Text.UTF8Encoding]::new($false, $false)
    foreach ($range in $ranges) {
        $views = New-Object System.Collections.Generic.List[object]
        [void]$views.Add([pscustomobject]@{ kind = 'ASCII'; text = $singleByteEncoding.GetString($bytes, $range.offset, $range.count); regex = $asciiRegex })
        [void]$views.Add([pscustomobject]@{ kind = 'UTF-8'; text = $utf8Encoding.GetString($bytes, $range.offset, $range.count); regex = $unicodeRegex })
        $evenCount = $range.count - ($range.count % 2)
        if ($evenCount -ge 2) { [void]$views.Add([pscustomobject]@{ kind = 'UTF-16LE/even'; text = [Text.Encoding]::Unicode.GetString($bytes, $range.offset, $evenCount); regex = $unicodeRegex }) }
        $oddCount = $range.count - 1; $oddCount -= ($oddCount % 2)
        if ($oddCount -ge 2) { [void]$views.Add([pscustomobject]@{ kind = 'UTF-16LE/odd'; text = [Text.Encoding]::Unicode.GetString($bytes, $range.offset + 1, $oddCount); regex = $unicodeRegex }) }
        foreach ($view in $views) {
            try { $match = $view.regex.Match($view.text) } catch [Text.RegularExpressions.RegexMatchTimeoutException] { throw "PE path inspection timed out in section '$($range.name)' ($($view.kind))" }
            if ($match.Success) {
                $sample = $match.Value; if ($sample.Length -gt 180) { $sample = $sample.Substring(0, 180) }
                throw "EXE contains machine/CI absolute source path or non-basename PDB reference in section '$($range.name)' ($($view.kind)): $sample"
            }
        }
    }
}

function Clear-NativeDevEnv { foreach ($name in @('MIR2_NATIVE_ASSET_ROOT','MIR2_ASSET_ROOT','MIR2_NATIVE_ACCOUNT','MIR2_NATIVE_PASSWORD','MIR2_GATEWAY_WS_URL','MIR2_NATIVE_CAPTURE_DIR','MIR2_NATIVE_SCREENSHOT_DIR','MIR2_NATIVE_TRACE_RENDER','MIR2_NATIVE_SOAK_METRICS')) { Remove-Item -Path ('Env:' + $name) -ErrorAction SilentlyContinue } }

function Invoke-CandidateProcess {
    param([string]$WorkingDirectory, [string]$ExePath, [int]$TimeoutMs, [string]$LogPath)
    Clear-NativeDevEnv; $stdout = $LogPath + '.stdout.txt'; $stderr = $LogPath + '.stderr.txt'; $process = Start-Process -FilePath $ExePath -WorkingDirectory $WorkingDirectory -PassThru -RedirectStandardOutput $stdout -RedirectStandardError $stderr
    try { $deadline = (Get-Date).AddMilliseconds($TimeoutMs); $opened = $false; $combined = ''; while ((Get-Date) -lt $deadline -and -not $process.HasExited) { Start-Sleep -Milliseconds 400; $combined = ((Get-Content -LiteralPath $stdout -Raw -ErrorAction SilentlyContinue) + "`n" + (Get-Content -LiteralPath $stderr -Raw -ErrorAction SilentlyContinue)); if ($combined -match 'native window opened') { $opened = $true; break }; if ($combined -match 'FATAL:|configuration error') { break } }; Write-Utf8NoBom -Path $LogPath -Text $combined; return [ordered]@{ opened = $opened; exitCode = $process.ExitCode; log = $combined } }
    finally { if (-not $process.HasExited) { Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue; [void]$process.WaitForExit(5000) } }
}

function Fail([string]$Message) { [void]$script:Failures.Add($Message); Write-Host "FAIL: $Message" }

function Get-RequiredCandidateFiles {
    param([string]$ExeName)
    $required = @($ExeName, 'BUILD-ATTESTATION.json', 'PACKAGE-MANIFEST.json', 'VERSION.json', 'RELEASE-STATEMENT.json', 'RELEASE-STATEMENT.p7s', 'mir2-client.toml', 'README-START.txt', 'CONTROLS.txt', 'KNOWN-ISSUES.md', 'mir2-assets\bevy-entity-atlases\manifest.json', 'mir2-assets\generated\map-atlas\manifest.json', 'mir2-assets\generated\native-map-keyed\manifest.json', 'mir2-assets\original-effects\effects.generated.json', 'mir2-assets\crystal-map-pack\0.map.gz', 'mir2-assets\original-ui\frame-sets.generated.json', 'mir2-assets\original-ui\Prguse\20.png', 'mir2-assets\original-ui\Sound\Login2.wav', 'mir2-assets\original-ui\Sound\Select2.wav', 'mir2-assets\original-ui\Sound\103.wav', 'mir2-assets\original-ui\Sound\M8-1.wav', 'mir2-assets\original-ui\Sound\M31-0.wav', 'mir2-assets\original-ui\Sound\M31-1.wav', 'mir2-assets\original-ui\Sound\M31-2.wav', 'mir2-assets\original-ui\Sound\M34-0.wav', 'mir2-assets\original-ui\Sound\M34-1.wav', 'mir2-assets\original-ui\Sound\M34-2.wav', 'mir2-assets\original-ui\Sound\M39-0.wav', 'mir2-assets\original-ui\Sound\M39-1.wav', 'mir2-assets\original-ui\Sound\M40-0.wav', 'mir2-assets\original-ui\Sound\M61-0.wav', 'mir2-assets\original-ui\Sound\M61-1.wav', 'mir2-assets\original-ui\Sound\M64-0.wav', 'mir2-assets\original-ui\Sound\M64-1.wav', 'mir2-assets\original-ui\Sound\M64-2.wav', 'mir2-assets\original-ui\Sound\M79-1.wav')
    foreach($soundName in $PlayerCombatSoundNames){ $required += "mir2-assets\original-ui\Sound\$soundName" }
    foreach($index in @(197,205,207,360,431,1340,1350) + @(450..468)){ $required += "mir2-assets\original-ui\Prguse2\$index.png" }
    foreach($index in @(0..41)){ $required += "mir2-assets\original-ui\Help\$index.png" }
    foreach($index in @(57,411,567,633,634,635,636,637,638,820,821,822,823,824,827,848,850,851,853)){ $required += "mir2-assets\original-ui\Title\$index.png" }
    foreach($index in @(1,920,1903,1904,1905,1970,1973,1976,1979,1982,1985,1988,1991,1992,1993,1994,1995,1996,2000,2090,2096,2097,2098)){ $required += "mir2-assets\original-ui\Prguse\$index.png" }
    foreach($index in @(0..9) + @(170..179)){ $required += "mir2-assets\original-effects\Magic\$index.png" }
    foreach($index in @(200..209) + @(370..379)){ $required += "mir2-assets\original-effects\Magic\$index.png" }
    foreach($direction in 0..15){ foreach($frame in 0..5){ $index = 10 + ($direction * 10) + $frame; $required += "mir2-assets\original-effects\Magic\$index.png" } }
    foreach($index in @(400..409) + @(570..579)){ $required += "mir2-assets\original-effects\Magic\$index.png" }
    foreach($direction in 0..15){ foreach($frame in 0..5){ $index = 410 + ($direction * 10) + $frame; $required += "mir2-assets\original-effects\Magic\$index.png" } }
    foreach($index in 1360..1369){ $required += "mir2-assets\original-effects\Magic\$index.png" }
    foreach($direction in 0..15){ foreach($frame in 0..2){ $index = 1160 + ($direction * 10) + $frame; $required += "mir2-assets\original-effects\Magic\$index.png" } }
    foreach($index in 1620..1635){ $required += "mir2-assets\original-effects\Magic\$index.png" }
    foreach($direction in 0..7){ foreach($frame in 0..5){ $index = 3480 + ($direction * 10) + $frame; $required += "mir2-assets\original-effects\Magic\$index.png" } }
    foreach($index in 1220..1239){ $required += "mir2-assets\original-effects\Magic2\$index.png" }
    return $required
}

function Get-MissingRequiredCandidateFiles {
    param([string]$PackageRoot, [string]$ExeName)
    return @(Get-RequiredCandidateFiles -ExeName $ExeName | Where-Object { -not (Test-Path -LiteralPath (Join-Path $PackageRoot $_) -PathType Leaf) })
}

function Test-FileIdentity {
    param([string]$Path, [int64]$ExpectedSize, [string]$ExpectedSha256)
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { return $false }
    $item = Get-Item -LiteralPath $Path
    return [int64]$item.Length -eq $ExpectedSize -and
        (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToUpperInvariant() -ceq $ExpectedSha256
}

if ($SelfTest) {
    $dateVector = ConvertFrom-JsonPreservingDateStrings -Text '{"buildCompletedUtc":"2026-08-25T20:51:33.9697458+00:00"}'
    if (-not ($dateVector.buildCompletedUtc -is [string]) -or [string]$dateVector.buildCompletedUtc -cne '2026-08-25T20:51:33.9697458+00:00') { throw 'JSON parser changed an attestation UTC string into a locale-dependent value' }
    $selfRoot = Join-Path ([IO.Path]::GetTempPath()) ('mir2-verify-selftest-' + [guid]::NewGuid().ToString('N')); New-Item -ItemType Directory -Path $selfRoot | Out-Null
    $certificate=$null; $wrongCertificate=$null; $rsa=$null; $wrongRsa=$null
    try {
        if ($Launch) { throw 'SelfTest must preserve default nonlaunch behavior' }
        if (-not (Test-PathWithin -Path (Join-Path $selfRoot 'child') -Root $selfRoot)) { throw 'containment self-test failed' }; if (Test-PathWithin -Path (Join-Path $selfRoot '..\escape') -Root $selfRoot) { throw 'escape containment self-test failed' }
        $identityProbe = Join-Path $selfRoot '103.wav'; [IO.File]::WriteAllBytes($identityProbe, [byte[]](1,2,3)); $identityProbeHash = (Get-FileHash -LiteralPath $identityProbe -Algorithm SHA256).Hash.ToUpperInvariant(); if (Test-FileIdentity -Path $identityProbe -ExpectedSize 4 -ExpectedSha256 $identityProbeHash) { throw 'sound identity self-test accepted wrong size' }; if (Test-FileIdentity -Path $identityProbe -ExpectedSize 3 -ExpectedSha256 ('0' * 64)) { throw 'sound identity self-test accepted wrong hash' }; if (-not (Test-FileIdentity -Path $identityProbe -ExpectedSize 3 -ExpectedSha256 $identityProbeHash)) { throw 'sound identity self-test rejected exact file' }
        $adsNormal=Join-Path $selfRoot 'ads-normal';New-Item -ItemType Directory -Path $adsNormal|Out-Null;$adsNormalFile=Join-Path $adsNormal 'normal.bin';[IO.File]::WriteAllBytes($adsNormalFile,[byte[]](1,2,3));Assert-NoAlternateDataStreams -Path $adsNormal;$adsVerifyResult='creation-unsupported';try{Set-Content -LiteralPath $adsNormalFile -Stream Zone.Identifier -Value '[ZoneTransfer]' -NoNewline -ErrorAction Stop;$fileAdsCreated=$true}catch{$fileAdsCreated=$false};if($fileAdsCreated){$fileRejected=$false;try{Assert-NoAlternateDataStreams -Path $adsNormal}catch{$fileRejected=$true};if(-not$fileRejected){throw 'verify ADS inspection accepted file named stream'};$adsDirectory=Join-Path $selfRoot 'ads-directory';New-Item -ItemType Directory -Path $adsDirectory|Out-Null;Set-Content -LiteralPath $adsDirectory -Stream DirectoryMarker -Value marker -NoNewline -ErrorAction Stop;$directoryRejected=$false;try{Assert-NoAlternateDataStreams -Path $adsDirectory}catch{$directoryRejected=$true};if(-not$directoryRejected){throw 'verify ADS inspection accepted directory named stream'};$adsVerifyResult='passed'};Write-Host "VERIFY_ADS_SELFTEST=$adsVerifyResult"
        $cultureEntries=@([pscustomobject]@{path='I.png';size=[int64]1;sha256=('1'*64)},[pscustomobject]@{path='i.png';size=[int64]2;sha256=('2'*64)},[pscustomobject]@{path=(([string][char]0x0130)+'.png');size=[int64]3;sha256=('3'*64)},[pscustomobject]@{path=(([string][char]0x0131)+'.png');size=[int64]4;sha256=('4'*64)},[pscustomobject]@{path=(([string][char]0x4E2D)+([string][char]0x6587)+'.json');size=[int64]5;sha256=('5'*64)})
        $selfTestThread=[Threading.Thread]::CurrentThread;$savedCulture=$selfTestThread.CurrentCulture;$savedUiCulture=$selfTestThread.CurrentUICulture;$cultureAggregate=$null;try{foreach($cultureName in @('tr-TR','zh-CN','en-US')){$testCulture=[Globalization.CultureInfo]::GetCultureInfo($cultureName);$selfTestThread.CurrentCulture=$testCulture;$selfTestThread.CurrentUICulture=$testCulture;$candidateAggregate=Get-TextSha256 -Text (Get-ManifestCanonicalText -Entries $cultureEntries);if($null-eq$cultureAggregate){$cultureAggregate=$candidateAggregate}elseif($candidateAggregate-cne$cultureAggregate){throw "manifest aggregate changed under culture: $cultureName"}}}finally{$selfTestThread.CurrentCulture=$savedCulture;$selfTestThread.CurrentUICulture=$savedUiCulture}
        $unicodeNoBreakSpace=[string][char]0x00A0;$unicodeFullwidthFullStop=[string][char]0xFF0E;$allowedLayoutVectors=@('mir2-platform-windows.exe','mir2-client.toml','README-START.txt','CONTROLS.txt','KNOWN-ISSUES.md','BUILD-ATTESTATION.json','PACKAGE-MANIFEST.json','VERSION.json','RELEASE-STATEMENT.json','RELEASE-STATEMENT.p7s','mir2-assets/bevy-entity-atlases/manifest.json','mir2-assets/generated/map-atlas/0.png','mir2-assets/generated/native-map-keyed/manifest.json','mir2-assets/crystal-map-pack/0.map.gz','mir2-assets/original-effects/effects.generated.json','mir2-assets/original-effects/sprite.v1.final.PNG',('mir2-assets/original-effects/folder'+$unicodeNoBreakSpace+'/0.png'),('mir2-assets/original-effects/payload'+$unicodeFullwidthFullStop+'exe.png'),'mir2-assets/original-ui/frame-sets.generated.json','mir2-assets/original-ui/ChrSel/0.png','mir2-assets/original-ui/Sound/Login2.wav','mir2-assets/original-ui/Sound/Select2.wav','mir2-assets/original-ui/Sound/103.wav','mir2-assets/original-ui/Sound/M8-1.wav','mir2-assets/original-ui/Sound/M31-0.wav','mir2-assets/original-ui/Sound/M31-1.wav','mir2-assets/original-ui/Sound/M31-2.wav','mir2-assets/original-ui/Sound/M34-0.wav','mir2-assets/original-ui/Sound/M34-1.wav','mir2-assets/original-ui/Sound/M34-2.wav','mir2-assets/original-ui/Sound/M39-0.wav','mir2-assets/original-ui/Sound/M39-1.wav','mir2-assets/original-ui/Sound/M40-0.wav','mir2-assets/original-ui/Sound/M61-0.wav','mir2-assets/original-ui/Sound/M61-1.wav','mir2-assets/original-ui/Sound/M64-0.wav','mir2-assets/original-ui/Sound/M64-1.wav','mir2-assets/original-ui/Sound/M64-2.wav','mir2-assets/original-ui/Sound/M79-1.wav');foreach($soundName in $PlayerCombatSoundNames){$allowedLayoutVectors+='mir2-assets/original-ui/Sound/'+$soundName};foreach($rel in $allowedLayoutVectors){if(-not(Test-PackageRelativeFileAllowed -RelativePath $rel -ExeName 'mir2-platform-windows.exe')){throw "strict allowlist rejected valid path: $rel"}}
        $requiredProbe = Join-Path $selfRoot 'required-probe'; New-Item -ItemType Directory -Path $requiredProbe | Out-Null; foreach ($relative in Get-RequiredCandidateFiles -ExeName 'mir2-platform-windows.exe') { $probePath = Join-Path $requiredProbe $relative; New-Item -ItemType Directory -Path (Split-Path -Parent $probePath) -Force | Out-Null; [IO.File]::WriteAllBytes($probePath, [byte[]](1)) }; if (@(Get-MissingRequiredCandidateFiles -PackageRoot $requiredProbe -ExeName 'mir2-platform-windows.exe').Count -ne 0) { throw 'required-file self-test rejected a complete probe package' }; Remove-Item -LiteralPath (Join-Path $requiredProbe 'mir2-assets\original-ui\frame-sets.generated.json') -Force; $missingRequired = @(Get-MissingRequiredCandidateFiles -PackageRoot $requiredProbe -ExeName 'mir2-platform-windows.exe'); if ($missingRequired.Count -ne 1 -or $missingRequired[0] -cne 'mir2-assets\original-ui\frame-sets.generated.json') { throw 'required-file self-test did not fail closed for missing frame-set catalog' }
        [IO.File]::WriteAllBytes((Join-Path $requiredProbe 'mir2-assets\original-ui\frame-sets.generated.json'), [byte[]](1)); Remove-Item -LiteralPath (Join-Path $requiredProbe 'mir2-assets\original-ui\Sound\103.wav') -Force; $missingRequired = @(Get-MissingRequiredCandidateFiles -PackageRoot $requiredProbe -ExeName 'mir2-platform-windows.exe'); if ($missingRequired.Count -ne 1 -or $missingRequired[0] -cne 'mir2-assets\original-ui\Sound\103.wav') { throw 'required-file self-test did not fail closed for missing VIS-03 ButtonA sound' }
        [IO.File]::WriteAllBytes((Join-Path $requiredProbe 'mir2-assets\original-ui\Sound\103.wav'), [byte[]](1)); Remove-Item -LiteralPath (Join-Path $requiredProbe 'mir2-assets\original-ui\Title\823.png') -Force; $missingRequired = @(Get-MissingRequiredCandidateFiles -PackageRoot $requiredProbe -ExeName 'mir2-platform-windows.exe'); if ($missingRequired.Count -ne 1 -or $missingRequired[0] -cne 'mir2-assets\original-ui\Title\823.png') { throw 'required-file self-test did not fail closed for missing VIS-03 disabled Teleport asset' }
        [IO.File]::WriteAllBytes((Join-Path $requiredProbe 'mir2-assets\original-ui\Title\823.png'), [byte[]](1)); Remove-Item -LiteralPath (Join-Path $requiredProbe 'mir2-assets\original-ui\Help\41.png') -Force; $missingRequired = @(Get-MissingRequiredCandidateFiles -PackageRoot $requiredProbe -ExeName 'mir2-platform-windows.exe'); if ($missingRequired.Count -ne 1 -or $missingRequired[0] -cne 'mir2-assets\original-ui\Help\41.png') { throw 'required-file self-test did not fail closed for missing Help final page asset' }
        [IO.File]::WriteAllBytes((Join-Path $requiredProbe 'mir2-assets\original-ui\Help\41.png'), [byte[]](1)); Remove-Item -LiteralPath (Join-Path $requiredProbe 'mir2-assets\original-ui\Prguse\920.png') -Force; $missingRequired = @(Get-MissingRequiredCandidateFiles -PackageRoot $requiredProbe -ExeName 'mir2-platform-windows.exe'); if ($missingRequired.Count -ne 1 -or $missingRequired[0] -cne 'mir2-assets\original-ui\Prguse\920.png') { throw 'required-file self-test did not fail closed for missing Help frame asset' }
        [IO.File]::WriteAllBytes((Join-Path $requiredProbe 'mir2-assets\original-ui\Prguse\920.png'), [byte[]](1)); Remove-Item -LiteralPath (Join-Path $requiredProbe 'mir2-assets\original-ui\Title\57.png') -Force; $missingRequired = @(Get-MissingRequiredCandidateFiles -PackageRoot $requiredProbe -ExeName 'mir2-platform-windows.exe'); if ($missingRequired.Count -ne 1 -or $missingRequired[0] -cne 'mir2-assets\original-ui\Title\57.png') { throw 'required-file self-test did not fail closed for missing Help title asset' }
        [IO.File]::WriteAllBytes((Join-Path $requiredProbe 'mir2-assets\original-ui\Title\57.png'), [byte[]](1));
        [IO.File]::WriteAllBytes((Join-Path $requiredProbe 'mir2-assets\original-ui\Title\823.png'), [byte[]](1)); Remove-Item -LiteralPath (Join-Path $requiredProbe 'mir2-assets\original-ui\Sound\M31-2.wav') -Force; $missingRequired = @(Get-MissingRequiredCandidateFiles -PackageRoot $requiredProbe -ExeName 'mir2-platform-windows.exe'); if ($missingRequired.Count -ne 1 -or $missingRequired[0] -cne 'mir2-assets\original-ui\Sound\M31-2.wav') { throw 'required-file self-test did not fail closed for missing VIS-02 FireBall impact sound' }
        [IO.File]::WriteAllBytes((Join-Path $requiredProbe 'mir2-assets\original-ui\Sound\M31-2.wav'), [byte[]](1)); Remove-Item -LiteralPath (Join-Path $requiredProbe 'mir2-assets\original-effects\Magic\165.png') -Force; $missingRequired = @(Get-MissingRequiredCandidateFiles -PackageRoot $requiredProbe -ExeName 'mir2-platform-windows.exe'); if ($missingRequired.Count -ne 1 -or $missingRequired[0] -cne 'mir2-assets\original-effects\Magic\165.png') { throw 'required-file self-test did not fail closed for missing VIS-02 FireBall direction frame' }
        [IO.File]::WriteAllBytes((Join-Path $requiredProbe 'mir2-assets\original-effects\Magic\165.png'), [byte[]](1)); Remove-Item -LiteralPath (Join-Path $requiredProbe 'mir2-assets\original-ui\Sound\M34-2.wav') -Force; $missingRequired = @(Get-MissingRequiredCandidateFiles -PackageRoot $requiredProbe -ExeName 'mir2-platform-windows.exe'); if ($missingRequired.Count -ne 1 -or $missingRequired[0] -cne 'mir2-assets\original-ui\Sound\M34-2.wav') { throw 'required-file self-test did not fail closed for missing VIS-02 GreatFireBall impact sound' }
        [IO.File]::WriteAllBytes((Join-Path $requiredProbe 'mir2-assets\original-ui\Sound\M34-2.wav'), [byte[]](1)); Remove-Item -LiteralPath (Join-Path $requiredProbe 'mir2-assets\original-effects\Magic\565.png') -Force; $missingRequired = @(Get-MissingRequiredCandidateFiles -PackageRoot $requiredProbe -ExeName 'mir2-platform-windows.exe'); if ($missingRequired.Count -ne 1 -or $missingRequired[0] -cne 'mir2-assets\original-effects\Magic\565.png') { throw 'required-file self-test did not fail closed for missing VIS-02 GreatFireBall direction frame' }
        [IO.File]::WriteAllBytes((Join-Path $requiredProbe 'mir2-assets\original-effects\Magic\565.png'), [byte[]](1)); Remove-Item -LiteralPath (Join-Path $requiredProbe 'mir2-assets\original-ui\Sound\M64-2.wav') -Force; $missingRequired = @(Get-MissingRequiredCandidateFiles -PackageRoot $requiredProbe -ExeName 'mir2-platform-windows.exe'); if ($missingRequired.Count -ne 1 -or $missingRequired[0] -cne 'mir2-assets\original-ui\Sound\M64-2.wav') { throw 'required-file self-test did not fail closed for missing VIS-02 SoulFireBall impact sound' }
        [IO.File]::WriteAllBytes((Join-Path $requiredProbe 'mir2-assets\original-ui\Sound\M64-2.wav'), [byte[]](1)); Remove-Item -LiteralPath (Join-Path $requiredProbe 'mir2-assets\original-effects\Magic\1312.png') -Force; $missingRequired = @(Get-MissingRequiredCandidateFiles -PackageRoot $requiredProbe -ExeName 'mir2-platform-windows.exe'); if ($missingRequired.Count -ne 1 -or $missingRequired[0] -cne 'mir2-assets\original-effects\Magic\1312.png') { throw 'required-file self-test did not fail closed for missing VIS-02 SoulFireBall direction frame' }
        [IO.File]::WriteAllBytes((Join-Path $requiredProbe 'mir2-assets\original-effects\Magic\1312.png'), [byte[]](1)); Remove-Item -LiteralPath (Join-Path $requiredProbe 'mir2-assets\original-ui\Sound\M39-1.wav') -Force; $missingRequired = @(Get-MissingRequiredCandidateFiles -PackageRoot $requiredProbe -ExeName 'mir2-platform-windows.exe'); if ($missingRequired.Count -ne 1 -or $missingRequired[0] -cne 'mir2-assets\original-ui\Sound\M39-1.wav') { throw 'required-file self-test did not fail closed for missing VIS-02 FireWall completion sound' }
        [IO.File]::WriteAllBytes((Join-Path $requiredProbe 'mir2-assets\original-ui\Sound\M39-1.wav'), [byte[]](1)); Remove-Item -LiteralPath (Join-Path $requiredProbe 'mir2-assets\original-effects\Magic\1635.png') -Force; $missingRequired = @(Get-MissingRequiredCandidateFiles -PackageRoot $requiredProbe -ExeName 'mir2-platform-windows.exe'); if ($missingRequired.Count -ne 1 -or $missingRequired[0] -cne 'mir2-assets\original-effects\Magic\1635.png') { throw 'required-file self-test did not fail closed for missing VIS-02 FireWall ground frame' }
        [IO.File]::WriteAllBytes((Join-Path $requiredProbe 'mir2-assets\original-effects\Magic\1635.png'), [byte[]](1)); Remove-Item -LiteralPath (Join-Path $requiredProbe 'mir2-assets\original-ui\Sound\M61-0.wav') -Force; $missingRequired = @(Get-MissingRequiredCandidateFiles -PackageRoot $requiredProbe -ExeName 'mir2-platform-windows.exe'); if ($missingRequired.Count -ne 1 -or $missingRequired[0] -cne 'mir2-assets\original-ui\Sound\M61-0.wav') { throw 'required-file self-test did not fail closed for missing VIS-02 Healing cast sound' }
        [IO.File]::WriteAllBytes((Join-Path $requiredProbe 'mir2-assets\original-ui\Sound\M61-0.wav'), [byte[]](1)); Remove-Item -LiteralPath (Join-Path $requiredProbe 'mir2-assets\original-ui\Sound\M61-1.wav') -Force; $missingRequired = @(Get-MissingRequiredCandidateFiles -PackageRoot $requiredProbe -ExeName 'mir2-platform-windows.exe'); if ($missingRequired.Count -ne 1 -or $missingRequired[0] -cne 'mir2-assets\original-ui\Sound\M61-1.wav') { throw 'required-file self-test did not fail closed for missing VIS-02 Healing target sound' }
        [IO.File]::WriteAllBytes((Join-Path $requiredProbe 'mir2-assets\original-ui\Sound\M61-1.wav'), [byte[]](1)); Remove-Item -LiteralPath (Join-Path $requiredProbe 'mir2-assets\original-effects\Magic\200.png') -Force; $missingRequired = @(Get-MissingRequiredCandidateFiles -PackageRoot $requiredProbe -ExeName 'mir2-platform-windows.exe'); if ($missingRequired.Count -ne 1 -or $missingRequired[0] -cne 'mir2-assets\original-effects\Magic\200.png') { throw 'required-file self-test did not fail closed for missing VIS-02 Healing cast frame' }
        [IO.File]::WriteAllBytes((Join-Path $requiredProbe 'mir2-assets\original-effects\Magic\200.png'), [byte[]](1)); Remove-Item -LiteralPath (Join-Path $requiredProbe 'mir2-assets\original-effects\Magic\379.png') -Force; $missingRequired = @(Get-MissingRequiredCandidateFiles -PackageRoot $requiredProbe -ExeName 'mir2-platform-windows.exe'); if ($missingRequired.Count -ne 1 -or $missingRequired[0] -cne 'mir2-assets\original-effects\Magic\379.png') { throw 'required-file self-test did not fail closed for missing VIS-02 Healing target frame' }
        [IO.File]::WriteAllBytes((Join-Path $requiredProbe 'mir2-assets\original-effects\Magic\379.png'), [byte[]](1)); Remove-Item -LiteralPath (Join-Path $requiredProbe 'mir2-assets\original-ui\Sound\M8-1.wav') -Force; $missingRequired = @(Get-MissingRequiredCandidateFiles -PackageRoot $requiredProbe -ExeName 'mir2-platform-windows.exe'); if ($missingRequired.Count -ne 1 -or $missingRequired[0] -cne 'mir2-assets\original-ui\Sound\M8-1.wav') { throw 'required-file self-test did not fail closed for missing VIS-02 FlamingSword attack sound' }
        [IO.File]::WriteAllBytes((Join-Path $requiredProbe 'mir2-assets\original-ui\Sound\M8-1.wav'), [byte[]](1)); Remove-Item -LiteralPath (Join-Path $requiredProbe 'mir2-assets\original-effects\Magic\3555.png') -Force; $missingRequired = @(Get-MissingRequiredCandidateFiles -PackageRoot $requiredProbe -ExeName 'mir2-platform-windows.exe'); if ($missingRequired.Count -ne 1 -or $missingRequired[0] -cne 'mir2-assets\original-effects\Magic\3555.png') { throw 'required-file self-test did not fail closed for missing VIS-02 FlamingSword direction frame' }
        [IO.File]::WriteAllBytes((Join-Path $requiredProbe 'mir2-assets\original-effects\Magic\3555.png'), [byte[]](1)); Remove-Item -LiteralPath (Join-Path $requiredProbe 'mir2-assets\original-ui\Sound\M79-1.wav') -Force; $missingRequired = @(Get-MissingRequiredCandidateFiles -PackageRoot $requiredProbe -ExeName 'mir2-platform-windows.exe'); if ($missingRequired.Count -ne 1 -or $missingRequired[0] -cne 'mir2-assets\original-ui\Sound\M79-1.wav') { throw 'required-file self-test did not fail closed for missing player revive sound' }
        [IO.File]::WriteAllBytes((Join-Path $requiredProbe 'mir2-assets\original-ui\Sound\M79-1.wav'), [byte[]](1)); Remove-Item -LiteralPath (Join-Path $requiredProbe 'mir2-assets\original-ui\Sound\145.wav') -Force; $missingRequired = @(Get-MissingRequiredCandidateFiles -PackageRoot $requiredProbe -ExeName 'mir2-platform-windows.exe'); if ($missingRequired.Count -ne 1 -or $missingRequired[0] -cne 'mir2-assets\original-ui\Sound\145.wav') { throw 'required-file self-test did not fail closed for missing player death sound' }
        [IO.File]::WriteAllBytes((Join-Path $requiredProbe 'mir2-assets\original-ui\Sound\145.wav'), [byte[]](1)); Remove-Item -LiteralPath (Join-Path $requiredProbe 'mir2-assets\original-effects\Magic2\1239.png') -Force; $missingRequired = @(Get-MissingRequiredCandidateFiles -PackageRoot $requiredProbe -ExeName 'mir2-platform-windows.exe'); if ($missingRequired.Count -ne 1 -or $missingRequired[0] -cne 'mir2-assets\original-effects\Magic2\1239.png') { throw 'required-file self-test did not fail closed for missing player revive effect frame' }
        $atlasProbeRoot = Join-Path $selfRoot 'atlas-probe'; $atlasProbeDir = Join-Path $atlasProbeRoot 'bevy-entity-atlases'; New-Item -ItemType Directory -Path $atlasProbeDir -Force | Out-Null
        $atlasPageBytes = [Convert]::FromBase64String('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGNgYGD4DwABBAEAHnOcQAAAAABJRU5ErkJggg=='); $atlasPageHash = (Get-ByteSha256 -Bytes $atlasPageBytes).ToLowerInvariant()
        foreach ($name in @('atlas.png','atlas-p1.png')) { [IO.File]::WriteAllBytes((Join-Path $atlasProbeDir $name), $atlasPageBytes) }
        $atlasProbeManifest = [ordered]@{ schemaVersion = 2; kind = 'mir2-bevy-entity-atlas-manifest'; atlases = @([ordered]@{ key = 'self-test'; pages = @([ordered]@{ imageUrl = '/bevy-entity-atlases/atlas.png'; width = 1; height = 1; sha256 = $atlasPageHash; imageBytes = $atlasPageBytes.Length }, [ordered]@{ imageUrl = '/bevy-entity-atlases/atlas-p1.png'; width = 1; height = 1; sha256 = $atlasPageHash; imageBytes = $atlasPageBytes.Length }); rects = @([ordered]@{ key = '/self-test/0.png|1x1'; pageIndex = 1 }) }) }
        $atlasProbeManifestPath = Join-Path $atlasProbeDir 'manifest.json'; Write-Utf8NoBom -Path $atlasProbeManifestPath -Text (($atlasProbeManifest | ConvertTo-Json -Depth 8) + "`n")
        $atlasClosure = Assert-EntityAtlasClosure -ManifestPath $atlasProbeManifestPath -AssetRoot $atlasProbeRoot; if ($atlasClosure.pageCount -ne 2) { throw 'entity atlas closure self-test rejected a complete two-page manifest' }
        Remove-Item -LiteralPath (Join-Path $atlasProbeDir 'atlas-p1.png') -Force; $atlasMissingRejected = $false; try { [void](Assert-EntityAtlasClosure -ManifestPath $atlasProbeManifestPath -AssetRoot $atlasProbeRoot) } catch { $atlasMissingRejected = $true }; if (-not $atlasMissingRejected) { throw 'entity atlas closure self-test accepted a missing referenced page' }
        $unicodeDangerousName=([string][char]0x5B89)+([string][char]0x5168)+'.JsE.png';$blockedLayoutVectors=@('extra.txt','mir2-assets/unknown/0.png','mir2-assets/crystal-map-pack/0.gz','mir2-assets/crystal-map-pack/0.png','mir2-assets/original-effects/payload.map.gz','mir2-assets/original-ui/Sound/Other.wav','mir2-assets/original-effects/payload.exe.png','mir2-assets/original-effects/config.cmd.json','mir2-assets/crystal-map-pack/payload.ps1.map.gz','mir2-assets/original-effects/PAYLOAD.DLL.PNG',('mir2-assets/original-effects/'+$unicodeDangerousName),'mir2-assets/original-effects/folder.BAT/0.png','mir2-assets/original-effects/folder.BAT /0.png','mir2-assets/original-effects/folder.bAt./0.png','mir2-assets/original-effects/payload.exe .png','mir2-assets/original-effects/PAYLOAD.ExE .PNG');foreach($extension in @('.exe','.dll','.com','.scr','.cpl','.msi','.msp','.bat','.cmd','.ps1','.psm1','.psd1','.vbs','.vbe','.js','.jse','.wsf','.wsh','.hta','.reg','.lnk','.chm','.jar','.py','.pyw','.sh','.bash','.zsh','.fish','.pdb','.ilk','.dmp','.dat','.unknown')){$blockedLayoutVectors+='mir2-assets/original-effects/payload'+$extension};foreach($rel in $blockedLayoutVectors){if(Test-PackageRelativeFileAllowed -RelativePath $rel -ExeName 'mir2-platform-windows.exe'){throw "strict allowlist accepted blocked path: $rel"}};if(Test-PackageRelativeDirectoryAllowed -RelativePath 'mir2-assets/original-effects/folder.BAT'){throw 'strict directory allowlist accepted dangerous intermediate extension'};foreach($unsafeDirectory in @('mir2-assets/original-effects/folder.BAT ','mir2-assets/original-effects/folder.bAt.')){if(Test-PackageRelativeDirectoryAllowed -RelativePath $unsafeDirectory){throw "strict directory allowlist accepted Windows-normalized segment: $unsafeDirectory"}};foreach($unicodeDirectory in @(('mir2-assets/original-effects/folder'+$unicodeNoBreakSpace),('mir2-assets/original-effects/folder'+$unicodeFullwidthFullStop))){if(-not(Test-PackageRelativeDirectoryAllowed -RelativePath $unicodeDirectory)){throw "strict directory allowlist rejected Unicode-adjacent segment: $unicodeDirectory"}}
        $exactAttestation=[pscustomobject]@{buildCommand=[pscustomobject]@{executable='cargo';toolchain='+1.95.0';subcommand='build';manifestPath='apps/game-client/platform-windows/Cargo.toml';bin='mir2-platform-windows';release=$true;locked=$true;target='x86_64-pc-windows-msvc';profile='release';targetDir='target-attested-windows-candidate';extraArgs=@()};pathRemapping=[pscustomobject]@{enabled=$true;environmentVariable='RUSTFLAGS';flags=@([pscustomobject]@{sourceToken='<REPO_ROOT>';destination='.'},[pscustomobject]@{sourceToken='<CARGO_HOME>';destination='cargo-home'})}}
        if(-not(Test-StructuredBuildContract $exactAttestation)){throw 'exact build contract rejected'}; $nearMiss=$exactAttestation|ConvertTo-Json -Depth 8|ConvertFrom-Json; $nearMiss.buildCommand.target='i686-pc-windows-msvc'; if(Test-StructuredBuildContract $nearMiss){throw 'near-miss target accepted'}
        Initialize-Pkcs
        $pathScanFile = Join-Path $selfRoot 'path-scan.bin'
        $blockedPaths = @(
            'D:\buildfarm\obj\client.pdb','C:\release\mir2.exe','C:/release/mir2.exe',
            '\\server\share\build\mir2.pdb','//host/share/build/mir2.pdb',
            '\\?\C:\release\mir2.exe','\\?\UNC\server\share\mir2.exe','\\.\C:\release\mir2.exe',
            '/home/runner/work/client','/Users/builder/client','/workspace/mir2/target','/workspaces/mir2/target',
            '/build/output/client','/builds/worker/client','/src/mir2/main.rs','/tmp/cargo-build','/var/tmp/cargo-build',
            '/opt/build/client','/private/tmp/client','/mnt/build/client','/Volumes/build/client',
            'target\release\client.pdb','target/release/client.pdb','foo:client.pdb','C:client.pdb',
            ('dir\' + ('a' * 129) + '.pdb'),
            ('//rustc/' + ('a' * 39) + '/library/core/src/lib.rs'),
            ('//rustc/' + ('a' * 40) + '/library?evil'),
            ('//rustc/' + ('a' * 40) + '/library%evil'),
            ('//rustc/' + ('a' * 40) + '/library evil')
        )
        foreach ($encoding in @([Text.Encoding]::ASCII, [Text.Encoding]::Unicode)) {
            foreach ($candidatePath in $blockedPaths) {
                [IO.File]::WriteAllBytes($pathScanFile, $encoding.GetBytes($candidatePath))
                $rejected = $false; try { Assert-NoBuildPathStrings -ExePath $pathScanFile } catch { $rejected = $true }
                if (-not $rejected) { throw "build path scanner accepted: $candidatePath ($($encoding.WebName))" }
            }
        }
        $unicodeBlockedPath = 'C:\' + ([string][char]0x6784) + ([string][char]0x5EFA) + '\' + ([string][char]0x5BA2) + ([string][char]0x6237) + '.pdb'
        [IO.File]::WriteAllBytes($pathScanFile, [Text.Encoding]::Unicode.GetBytes($unicodeBlockedPath)); $rejected = $false; try { Assert-NoBuildPathStrings -ExePath $pathScanFile } catch { $rejected = $true }; if (-not $rejected) { throw 'build path scanner accepted a Unicode machine path' }
        [IO.File]::WriteAllBytes($pathScanFile, [Text.Encoding]::UTF8.GetBytes($unicodeBlockedPath)); $rejected = $false; try { Assert-NoBuildPathStrings -ExePath $pathScanFile } catch { $rejected = $true }; if (-not $rejected) { throw 'build path scanner accepted a UTF-8 machine path' }
        $oddUnicodeBytes = [Text.Encoding]::Unicode.GetBytes('C:\odd\build\client.pdb'); $oddUnicodeVector = New-Object byte[] ($oddUnicodeBytes.Length + 1); $oddUnicodeVector[0] = 0xA5; [Array]::Copy($oddUnicodeBytes, 0, $oddUnicodeVector, 1, $oddUnicodeBytes.Length)
        [IO.File]::WriteAllBytes($pathScanFile, $oddUnicodeVector); $rejected = $false; try { Assert-NoBuildPathStrings -ExePath $pathScanFile } catch { $rejected = $true }; if (-not $rejected) { throw 'build path scanner accepted an odd-aligned UTF-16LE machine path' }
        $allowedPaths = @(
            'https://example.invalid/build/resource.png','wss://gateway.example/workspace/socket','http://host/opt/resource',
            'original-ui/Title/30.png','Magic/10.png','assets\relative\sprite.png','assets/relative/sprite.png',
            'mir2_platform_windows.pdb','// comment',
            ('//rustc/' + ('a' * 40) + '/library/core/src/lib.rs'),
            ('//rustc/' + ('a' * 40) + '/library\std\src\io\mod.rs'),
            'cargo-home\registry\src\crate\src\lib.rs','E:\7'
        )
        foreach ($encoding in @([Text.Encoding]::ASCII, [Text.Encoding]::Unicode)) { foreach ($candidatePath in $allowedPaths) { [IO.File]::WriteAllBytes($pathScanFile, $encoding.GetBytes($candidatePath)); Assert-NoBuildPathStrings -ExePath $pathScanFile } }
        $nulTerminatedRustPath = [Text.Encoding]::ASCII.GetBytes(('//rustc/' + ('a' * 40) + '/library' + [char]0)); [IO.File]::WriteAllBytes($pathScanFile, $nulTerminatedRustPath); Assert-NoBuildPathStrings -ExePath $pathScanFile
        [IO.File]::WriteAllBytes($pathScanFile, [byte[]](0x42,0x3A,0x5C,0xA0,0xEC,0x75,0x18,0x40,0x84,0xED,0x0F)); Assert-NoBuildPathStrings -ExePath $pathScanFile
        $execLeak = [Text.Encoding]::ASCII.GetBytes('C:\buildfarm\client.pdb'); $safeData = [Text.Encoding]::ASCII.GetBytes('assets/relative/sprite.png'); $sectionBytes = New-Object byte[] ($execLeak.Length + $safeData.Length); [Array]::Copy($execLeak, 0, $sectionBytes, 0, $execLeak.Length); [Array]::Copy($safeData, 0, $sectionBytes, $execLeak.Length, $safeData.Length); [IO.File]::WriteAllBytes($pathScanFile, $sectionBytes)
        $sectionPe = [pscustomobject]@{ sections = @([pscustomobject]@{ name = '.text'; rawPointer = 0; rawSize = $execLeak.Length; characteristics = [uint32]0x60000020 }, [pscustomobject]@{ name = '.rdata'; rawPointer = $execLeak.Length; rawSize = $safeData.Length; characteristics = [uint32]0x40000040 }) }; Assert-NoBuildPathStrings -ExePath $pathScanFile -PeInfo $sectionPe
        $sectionPe.sections[0].characteristics = [uint32]0x40000040; $rejected = $false; try { Assert-NoBuildPathStrings -ExePath $pathScanFile -PeInfo $sectionPe } catch { $rejected = $true }; if (-not $rejected) { throw 'PE path scanner accepted a leak in a non-executable section' }
        $sectionPe.sections[0].characteristics = [uint32]0x60000020; $sectionPe.sections[1].rawPointer = $sectionBytes.Length; $sectionPe.sections[1].rawSize = 1; $rejected = $false; try { Assert-NoBuildPathStrings -ExePath $pathScanFile -PeInfo $sectionPe } catch { $rejected = $true }; if (-not $rejected) { throw 'PE path scanner accepted an out-of-bounds section range' }
        $sectionPe.sections = @($sectionPe.sections[0]); $rejected = $false; try { Assert-NoBuildPathStrings -ExePath $pathScanFile -PeInfo $sectionPe } catch { $rejected = $true }; if (-not $rejected) { throw 'PE path scanner accepted a PE with no non-executable section data' }
        function New-EphemeralCodeSigningCertificate([string]$Name,[ref]$RsaReference){$localRsa=[Security.Cryptography.RSA]::Create(2048);$request=[Security.Cryptography.X509Certificates.CertificateRequest]::new("CN=$Name",$localRsa,[Security.Cryptography.HashAlgorithmName]::SHA256,[Security.Cryptography.RSASignaturePadding]::Pkcs1);$oids=[Security.Cryptography.OidCollection]::new();[void]$oids.Add([Security.Cryptography.Oid]::new('1.3.6.1.5.5.7.3.3'));$request.CertificateExtensions.Add([Security.Cryptography.X509Certificates.X509EnhancedKeyUsageExtension]::new($oids,$true));$request.CertificateExtensions.Add([Security.Cryptography.X509Certificates.X509KeyUsageExtension]::new([Security.Cryptography.X509Certificates.X509KeyUsageFlags]::DigitalSignature,$true));$RsaReference.Value=$localRsa;return $request.CreateSelfSigned([DateTimeOffset]::UtcNow.AddMinutes(-5),[DateTimeOffset]::UtcNow.AddHours(1))}
        $certificate=New-EphemeralCodeSigningCertificate 'Mir2 Candidate SelfTest' ([ref]$rsa); $wrongCertificate=New-EphemeralCodeSigningCertificate 'Wrong Mir2 Signer' ([ref]$wrongRsa)
        $a='A'*64;$b='B'*64;$c='C'*64;$d='D'*64;$e='E'*64;$f='F'*64
        $statement=New-ReleaseStatementText -Candidate 'WN-CANDIDATE-SELFTEST' -ExeSha256 $a -ManifestSha256 $b -ManifestAggregateSha256 $c -VersionSha256 $d -AttestationSha256 $e -GitRevision ('1'*40) -WorktreeDirty $true -DirtyDigest $f; $bytes=[Text.Encoding]::UTF8.GetBytes($statement); $signature=New-DetachedCmsSignature -Content $bytes -Certificate $certificate
        if(-not(Test-DetachedCmsSignature -Content $bytes -Signature $signature -TrustedThumbprint $certificate.Thumbprint)){throw 'detached CMS valid signature rejected'}
        if(Test-DetachedCmsSignature -Content $bytes -Signature $signature -TrustedThumbprint $wrongCertificate.Thumbprint){throw 'wrong trusted signer was accepted'}
        $altered=New-ReleaseStatementText -Candidate 'WN-CANDIDATE-SELFTEST' -ExeSha256 $a -ManifestSha256 ('9'*64) -ManifestAggregateSha256 ('8'*64) -VersionSha256 ('7'*64) -AttestationSha256 $e -GitRevision ('1'*40) -WorktreeDirty $true -DirtyDigest $f
        if(Test-DetachedCmsSignature -Content ([Text.Encoding]::UTF8.GetBytes($altered)) -Signature $signature -TrustedThumbprint $certificate.Thumbprint){throw 'recomputed payload/manifest/version statement was accepted with the old signature'}
        Write-Host 'verify-windows-candidate self-test passed'
    }
    finally { if($null-ne$certificate){$certificate.Dispose()};if($null-ne$wrongCertificate){$wrongCertificate.Dispose()};if($null-ne$rsa){$rsa.Dispose()};if($null-ne$wrongRsa){$wrongRsa.Dispose()}; if (Test-Path -LiteralPath $selfRoot) { Remove-SafeTemporaryTree -Path $selfRoot -RequiredPrefix 'mir2-verify-selftest-' } }
    exit 0
}

if ([string]::IsNullOrWhiteSpace($PackageRoot)) { throw 'PackageRoot is mandatory; default stale packages are forbidden' }
if ((Normalize-Thumbprint -Thumbprint $TrustedSignerThumbprint) -notmatch '^[0-9A-F]{40}$') { throw 'TrustedSignerThumbprint is mandatory and must be supplied out-of-band' }
$ScriptDir = Split-Path -Parent $PSCommandPath; $RepoRoot = Find-RepoRoot -StartPath $ScriptDir; $PackageRoot = Resolve-FullPath -Path $PackageRoot; $EvidenceDir = Join-Path $RepoRoot 'docs\generated\player-qa\windows-package-preflight'; $ExeName = 'mir2-platform-windows.exe'; $Failures = New-Object System.Collections.Generic.List[string]
try { Assert-NoReparseTree -Path $PackageRoot; Assert-NoAlternateDataStreams -Path $PackageRoot } catch { Fail $_.Exception.Message; Write-Host 'verify-windows-candidate FAILED during pre-manifest filesystem inspection'; exit 1 }
$allFiles = @(Get-OrdinalPackageFiles -Root $PackageRoot)
try { Assert-PackageAllowlist -Root $PackageRoot -ExeName $ExeName } catch { Fail $_.Exception.Message }
$required = @(Get-RequiredCandidateFiles -ExeName $ExeName)
foreach ($relative in @(Get-MissingRequiredCandidateFiles -PackageRoot $PackageRoot -ExeName $ExeName)) { Fail "missing required file: $relative" }
$candidateSoundIdentities = @(
    [pscustomobject]@{ name='103.wav'; size=[int64]26546; sha256='7A55D27DEA18F70EB4FF4F324B682EFAB4996406EFAE3E94467D3C39CCCC674A' },
    [pscustomobject]@{ name='70.wav'; size=[int64]43452; sha256='B4987C79614A0D230A801082C7A82384FD658B428DA78E378D78758667F40A53' },
    [pscustomobject]@{ name='71.wav'; size=[int64]54204; sha256='18A002E22FF6F06DB25A9FB84BE02D1EE7521089733E07EEECFE8671C42A1AC0' },
    [pscustomobject]@{ name='72.wav'; size=[int64]32342; sha256='921FEE1C7775A7903AC276C72399D32F34F586C343A8EF732A79AD12F124B974' },
    [pscustomobject]@{ name='73.wav'; size=[int64]31778; sha256='8F114AE3EE416F8A25511CEBF917217ADEE713B88EEF4FF2AB17079ECF5D0022' },
    [pscustomobject]@{ name='80.wav'; size=[int64]55944; sha256='05FE847653D1588B59924D661484CAB31D9670A1089B61F57EE23D92822C4CC4' },
    [pscustomobject]@{ name='81.wav'; size=[int64]64106; sha256='83653E0D7B92FC91D1AAFC09446C43219EB321F9C40E4B9B5DE1B5364E95E1CC' },
    [pscustomobject]@{ name='82.wav'; size=[int64]38922; sha256='D5D2B29FDEBD498280CD79D4EF8B89A8C7E29A819D339575893F542C9CA8E280' },
    [pscustomobject]@{ name='83.wav'; size=[int64]34356; sha256='07174438CFB099C7995F57220936DA3C11B32212999FEA4C00BE391C1AFF3374' },
    [pscustomobject]@{ name='138.wav'; size=[int64]32684; sha256='6352611AFB6F702A4440F176E09CCF7A2F9239B91A3046AA2F92B4BDED9A0E98' },
    [pscustomobject]@{ name='139.wav'; size=[int64]57004; sha256='AAF891B18F6B06684FA1040C267DE16ED3646DE54908470C70CC5A9304FF3586' },
    [pscustomobject]@{ name='144.wav'; size=[int64]74218; sha256='96DA14D611A2255362CD61AB7FAE011513DCB49214EDF1515DD6176CA1EDACA1' },
    [pscustomobject]@{ name='145.wav'; size=[int64]85414; sha256='ADDDB2EDBB636D9099F76E06BB0F430B746FB3F8E0E5D5BCA1835CEF27A3E11A' },
    [pscustomobject]@{ name='tiger_struck_1.wav'; size=[int64]49140; sha256='A2FB7D3C1D7B35ABC0EABB5D0BE95235566329620C60559256F9565B4D9EFF59' },
    [pscustomobject]@{ name='tiger_struck_2.wav'; size=[int64]48650; sha256='C5036046F4E4C7E4DD8A61A41B510EBD70F15B29B725C273B025ED46C73CB8B4' },
    [pscustomobject]@{ name='wolf_struck1.wav'; size=[int64]32298; sha256='E906653D793E525C6694165BD31518F85BFCF57AC3330EF92E4DC0E8C6DCB1DF' },
    [pscustomobject]@{ name='M8-1.wav'; size=[int64]132720; sha256='6A4A29C45E6D9882DD63D67FD4825C9401481DF52383BB74C5FF0644A8EC1B0B' },
    [pscustomobject]@{ name='M31-0.wav'; size=[int64]364024; sha256='98C28FC920A35FE3C134607811760E4C49200239C2E3B9CCAE36B42EE083AA3E' },
    [pscustomobject]@{ name='M31-1.wav'; size=[int64]364028; sha256='FCC49A68343DB3E910A3A35F12CEA227CBEA058E199D048236A8D99831005A15' },
    [pscustomobject]@{ name='M31-2.wav'; size=[int64]128908; sha256='8732FD9131E228712071AABFED542618B9D1D6F269D748EC9857ECBFA4E59B05' },
    [pscustomobject]@{ name='M34-0.wav'; size=[int64]430124; sha256='0F25BB7CD8556726C8758C48CBF0BD2D1D3D4C205BE36C6CAE39251DE9D3068B' },
    [pscustomobject]@{ name='M34-1.wav'; size=[int64]319532; sha256='895C3855F35BB8BA543B2717F682617A85BBE5A6EA15170D1D5EB4196914429C' },
    [pscustomobject]@{ name='M34-2.wav'; size=[int64]229420; sha256='4482367380FFF4EDB7E1CD605ADD6EAD984B45497B254ECB3941AB6D6CC0DBAB' },
    [pscustomobject]@{ name='M39-0.wav'; size=[int64]246912; sha256='464F33258DDD963A9D969AC1B439EA0FEA0A39529B84D7CC6A762FF5B712F3AF' },
    [pscustomobject]@{ name='M39-1.wav'; size=[int64]525980; sha256='E6D5E62494DA3D2F83073D7D17FF168B251D94AED8B054B3931E7A360894E6BE' },
    [pscustomobject]@{ name='M40-0.wav'; size=[int64]247772; sha256='05E08C3AA3ADF166A3FDF9462279024898217F4F936BBD28A1FB6EA75BF92A4E' },
    [pscustomobject]@{ name='M61-0.wav'; size=[int64]194008; sha256='AADE9DB9A46762B8C319A2FD3611FBB4CDC86D444B5C3FD14DC92AEC812F94A1' },
    [pscustomobject]@{ name='M61-1.wav'; size=[int64]308496; sha256='9E3942A729F886197B30D1CA0084AA020179F62BCA64C6044E36D6E080D74ED5' },
    [pscustomobject]@{ name='M64-0.wav'; size=[int64]151328; sha256='2736DA89BADEEA678DD17BC903D6AAC7D63595405D82E8C0E0C9F2FAF3E684C3' },
    [pscustomobject]@{ name='M64-1.wav'; size=[int64]168768; sha256='3487AAA8B8218D68F34D9ACE7CFBD95A13667737216DED6BE16702CCE48E161E' },
    [pscustomobject]@{ name='M64-2.wav'; size=[int64]228532; sha256='2D3F6EC560E0F11C86C95EBCE1E78907A154C127103E1B226B04203041B5689E' },
    [pscustomobject]@{ name='M79-1.wav'; size=[int64]484496; sha256='9098F96106FB880720711FB829B9CCDFEB8DB1883132BC680629FCD0360EA83D' }
)
foreach ($identity in $candidateSoundIdentities) { $soundPath = Join-Path $PackageRoot ('mir2-assets\original-ui\Sound\' + $identity.name); if ((Test-Path -LiteralPath $soundPath -PathType Leaf) -and -not (Test-FileIdentity -Path $soundPath -ExpectedSize $identity.size -ExpectedSha256 $identity.sha256)) { Fail "$($identity.name) identity mismatch" } }
try {
    $entityAtlasClosure = Assert-EntityAtlasClosure -ManifestPath (Join-Path $PackageRoot 'mir2-assets\bevy-entity-atlases\manifest.json') -AssetRoot (Join-Path $PackageRoot 'mir2-assets')
    Write-Host "entityAtlasClosure=atlases:$($entityAtlasClosure.atlasCount),pages:$($entityAtlasClosure.pageCount)"
}
catch {
    Fail "entity atlas closure failed: $($_.Exception.Message)"
}

$textExtensions = @('.txt','.md','.toml','.json','.ini','.cfg','.yaml','.yml','.env')
foreach ($file in $allFiles | Where-Object { $textExtensions -contains $_.Extension.ToLowerInvariant() }) {
    $text = [IO.File]::ReadAllText($file.FullName)
    if ($text -match '(?im)(?:(?<![A-Za-z])[A-Za-z]:[\\/]|\\Users\\|/Users/|/home/|\.env(?:\.|\b))') { Fail "absolute/user path or .env reference: $(Get-RelativeUnixPath -Root $PackageRoot -Path $file.FullName)" }
    if ($text -match '(?im)^\s*(?:password|token|passkey|secret|authorization)\s*=|"(?:password|token|passkey|secret|authorization)"\s*:') { Fail "credential field: $(Get-RelativeUnixPath -Root $PackageRoot -Path $file.FullName)" }
    if ($text -match '(?im)qa\.giveItem|event\.spawn|crystal:[^\s"`]+') { Fail "QA/admin/debug command: $(Get-RelativeUnixPath -Root $PackageRoot -Path $file.FullName)" }
}
$tomlPath = Join-Path $PackageRoot 'mir2-client.toml'; if (Test-Path -LiteralPath $tomlPath -PathType Leaf) { $toml = Get-Content -LiteralPath $tomlPath -Raw; if ($toml -notmatch '(?m)^\s*gateway_ws_url\s*=\s*"wss://') { Fail 'mir2-client.toml must use wss://' } }

$exePath = Join-Path $PackageRoot $ExeName; $exeHash = ''; $exeItem = $null; $pe = [ordered]@{ valid = $false; imports = @(); delayImports = @() }
if (Test-Path -LiteralPath $exePath -PathType Leaf) {
    $exeItem = Get-Item -LiteralPath $exePath; $exeHash = (Get-FileHash -LiteralPath $exePath -Algorithm SHA256).Hash.ToUpperInvariant()
    try { $pe = Read-PeInfo -Path $exePath; foreach ($dependency in @($pe.imports) + @($pe.delayImports)) { if (-not (Test-SystemDependency -Name $dependency)) { Fail "non-system PE dependency rejected: $dependency" } } } catch { Fail "PE dependency inspection failed closed: $($_.Exception.Message)" }
    try { Assert-NoBuildPathStrings -ExePath $exePath -PeInfo $pe } catch { Fail $_.Exception.Message }
} else { Fail 'client EXE unavailable for PE verification' }

$attestationPath = Join-Path $PackageRoot 'BUILD-ATTESTATION.json'; $attestation = $null; $attestationHash = ''; $attestationShapeValid = $false; $buildCompleted = [DateTime]::MinValue
if (Test-Path -LiteralPath $attestationPath -PathType Leaf) {
    $attestationHash = (Get-FileHash -LiteralPath $attestationPath -Algorithm SHA256).Hash.ToUpperInvariant()
    try { $attestation = ConvertFrom-JsonPreservingDateStrings -Text (Get-Content -LiteralPath $attestationPath -Raw) } catch { Fail 'BUILD-ATTESTATION.json invalid JSON' }
}
if ($null -ne $attestation) {
    $attestationShapeValid = $true
    foreach ($field in @('schema','exeSha256','exeSizeBytes','gitRevision','worktreeDirty','worktreeStatusScope','worktreeStatusSha256','worktreeStatusLineCount','cargoVersion','rustcVersion','buildCommand','pathRemapping','buildCompletedUtc')) { if ($null -eq $attestation.PSObject.Properties[$field] -or [string]::IsNullOrWhiteSpace([string]$attestation.$field)) { Fail "attestation missing/empty field: $field"; $attestationShapeValid = $false } }
    if ($attestationShapeValid) {
        if ($attestation.schema -ne 'mir2.windows.build-attestation.v2') { Fail 'attestation schema mismatch' }
        if (-not ($attestation.worktreeDirty -is [bool]) -or [int]$attestation.worktreeStatusLineCount -lt 0) { Fail 'attestation dirty/status count types invalid' }
        if ($attestation.worktreeStatusScope -ne 'git-status-z+diff+all-untracked-content-v2') { Fail 'attestation worktreeStatusScope unsupported' }
        if ($attestation.exeSha256 -notmatch '^[0-9a-fA-F]{64}$' -or $attestation.exeSha256 -ne $exeHash -or [int64]$attestation.exeSizeBytes -le 0 -or ($null -ne $exeItem -and [int64]$attestation.exeSizeBytes -ne [int64]$exeItem.Length)) { Fail 'attestation EXE hash/size mismatch' }
        if ($attestation.gitRevision -notmatch '^[0-9a-fA-F]{40}$' -or $attestation.worktreeStatusSha256 -notmatch '^[0-9a-fA-F]{64}$') { Fail 'attestation source digest fields invalid' }
        if ($attestation.cargoVersion -notmatch '^cargo 1\.95\.0(?:\s|$)' -or $attestation.rustcVersion -notmatch '^rustc 1\.95\.0(?:\s|$)') { Fail 'attestation toolchain is not pinned 1.95.0' }
        if (-not (Test-StructuredBuildContract -Attestation $attestation)) { Fail 'attestation structured build/path-remapping contract is not exact' }
        if ([string]$attestation.buildCompletedUtc -notmatch '(?i)(?:Z|\+00:00)$') { Fail 'attestation buildCompletedUtc must carry UTC offset' } else { try { $buildCompleted = [DateTimeOffset]::Parse([string]$attestation.buildCompletedUtc).UtcDateTime; if ($buildCompleted -gt [DateTime]::UtcNow.AddMinutes(5)) { Fail 'attestation buildCompletedUtc is in the future' } } catch { Fail 'attestation buildCompletedUtc invalid' } }
        if ([bool]$attestation.worktreeDirty -and -not $AllowDirtyWorktree) { Fail 'attestation records dirty worktree; explicit -AllowDirtyWorktree required' }
    }
}

$manifestPath = Join-Path $PackageRoot 'PACKAGE-MANIFEST.json'; $manifest = $null; $manifestShapeValid = $false; $computedAggregate = ''; $manifestHash = ''; $payloadFiles = @(Get-ManifestPayloadFiles -Root $PackageRoot)
if (Test-Path -LiteralPath $manifestPath -PathType Leaf) { $manifestHash=(Get-FileHash -LiteralPath $manifestPath -Algorithm SHA256).Hash.ToUpperInvariant(); try { $manifest = ConvertFrom-JsonPreservingDateStrings -Text (Get-Content -LiteralPath $manifestPath -Raw) } catch { Fail 'PACKAGE-MANIFEST.json invalid JSON' } }
if ($null -ne $manifest) {
    $manifestShapeValid = $true
    foreach ($field in @('schema','coverage','fileCount','totalBytes','aggregateSha256','files')) { if ($null -eq $manifest.PSObject.Properties[$field]) { Fail "package manifest missing field: $field"; $manifestShapeValid = $false } }
    $actualEntries = @(); $actualPaths = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal); $totalBytes = [int64]0
    foreach ($file in $payloadFiles) { $rel = Get-RelativeUnixPath -Root $PackageRoot -Path $file.FullName; if (-not $actualPaths.Add($rel)) { Fail "duplicate actual package path: $rel" }; $hash = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToUpperInvariant(); $totalBytes += [int64]$file.Length; $actualEntries += [ordered]@{ path = $rel; size = [int64]$file.Length; sha256 = $hash } }
    $canonical = Get-ManifestCanonicalText -Entries $actualEntries; $computedAggregate = Get-TextSha256 -Text $canonical
    if ($manifestShapeValid) {
        if ($manifest.schema -ne 'mir2.windows.package-manifest.v4') { Fail 'package manifest schema mismatch' }
        $expectedExcludes=@('PACKAGE-MANIFEST.json','VERSION.json','RELEASE-STATEMENT.json','RELEASE-STATEMENT.p7s'); $actualExcludes=@($manifest.coverage.excludes)
        if ($null -eq $manifest.coverage.PSObject.Properties['excludes'] -or ($actualExcludes -join "`n") -cne ($expectedExcludes -join "`n")) { Fail 'package manifest exclusion contract mismatch' }
        if ([int]$manifest.fileCount -ne $actualEntries.Count -or [int64]$manifest.totalBytes -ne $totalBytes -or $manifest.aggregateSha256 -ne $computedAggregate) { Fail 'package manifest aggregate/count/bytes mismatch' }
        $declared = @{}; foreach ($entry in @($manifest.files)) { if ($null -eq $entry.PSObject.Properties['path'] -or $null -eq $entry.PSObject.Properties['size'] -or $null -eq $entry.PSObject.Properties['sha256']) { Fail 'malformed package manifest entry'; continue }; if ($declared.ContainsKey([string]$entry.path)) { Fail "duplicate manifest path: $($entry.path)" } else { $declared[[string]$entry.path] = $entry } }
        foreach ($entry in $actualEntries) { if (-not $declared.ContainsKey($entry.path)) { Fail "unmanifested package file: $($entry.path)" } else { $expected = $declared[$entry.path]; if ([int64]$expected.size -ne $entry.size -or $expected.sha256 -ne $entry.sha256) { Fail "manifest file mismatch: $($entry.path)" } } }
        foreach ($path in $declared.Keys) { if (-not $actualPaths.Contains([string]$path)) { Fail "manifest references missing file: $path" } }
    }
}

$versionPath = Join-Path $PackageRoot 'VERSION.json'; $version = $null; $versionShapeValid = $false; $versionHash=''
if (Test-Path -LiteralPath $versionPath -PathType Leaf) { $versionHash=(Get-FileHash -LiteralPath $versionPath -Algorithm SHA256).Hash.ToUpperInvariant(); try { $version = ConvertFrom-JsonPreservingDateStrings -Text (Get-Content -LiteralPath $versionPath -Raw) } catch { Fail 'VERSION.json invalid JSON' } }
if ($null -ne $version) {
    $versionShapeValid = $true
    foreach ($field in @('schema','candidate','gitRevision','worktreeDirty','worktreeStatusScope','worktreeStatusSha256','exeName','exeSha256','exeSizeBytes','buildAttestationSha256','buildCompletedUtc','packageManifestSchema','packageManifestSha256','packageManifestAggregateSha256','packageManifestFileCount','packageFileCount','releaseStatementSchema','signatureFormat','staged','builtByPackagingScript','accepted')) { if ($null -eq $version.PSObject.Properties[$field]) { Fail "VERSION missing field: $field"; $versionShapeValid = $false } }
    if ($versionShapeValid) {
        if ($version.schema -ne 'mir2.windows.candidate-version.v4' -or $version.candidate -notmatch '^WN-CANDIDATE-[A-Za-z0-9._-]+$') { Fail 'VERSION schema/candidate invalid' }
        if ($version.exeName -ne $ExeName -or $version.exeSha256 -ne $exeHash -or ($null -ne $exeItem -and [int64]$version.exeSizeBytes -ne [int64]$exeItem.Length)) { Fail 'VERSION EXE identity mismatch' }
        if ($version.buildAttestationSha256 -ne $attestationHash) { Fail 'VERSION attestation digest mismatch' }; if ($attestationShapeValid -and ($version.gitRevision -ne $attestation.gitRevision.ToLowerInvariant() -or [bool]$version.worktreeDirty -ne [bool]$attestation.worktreeDirty -or $version.worktreeStatusScope -ne $attestation.worktreeStatusScope -or $version.worktreeStatusSha256 -ne $attestation.worktreeStatusSha256.ToUpperInvariant())) { Fail 'VERSION source/worktree binding mismatch' }
        if ($attestationShapeValid -and $version.buildCompletedUtc -ne $attestation.buildCompletedUtc) { Fail 'VERSION buildCompletedUtc is not from attestation' }
        if ($version.packageManifestSchema -ne 'mir2.windows.package-manifest.v4' -or $version.packageManifestSha256 -ne $manifestHash -or $version.packageManifestAggregateSha256 -ne $computedAggregate -or ($manifestShapeValid -and [int]$version.packageManifestFileCount -ne [int]$manifest.fileCount)) { Fail 'VERSION package-manifest binding mismatch' }
        if ([int]$version.packageFileCount -ne $allFiles.Count -or $allFiles.Count -ne ($payloadFiles.Count + 4)) { Fail 'VERSION package file count mismatch' }
        if ($version.releaseStatementSchema -ne 'mir2.windows.release-statement.v1' -or $version.signatureFormat -ne 'CMS/PKCS7-detached') { Fail 'VERSION release-signature contract mismatch' }
        if ($version.staged -ne $true -or $version.builtByPackagingScript -ne $false -or $version.accepted -ne $false) { Fail 'VERSION truth fields invalid' }
    }
}

$statementPath=Join-Path $PackageRoot 'RELEASE-STATEMENT.json'; $signaturePath=Join-Path $PackageRoot 'RELEASE-STATEMENT.p7s'; $signatureValid=$false
if($attestationShapeValid -and $manifestShapeValid -and $versionShapeValid -and (Test-Path -LiteralPath $statementPath -PathType Leaf) -and (Test-Path -LiteralPath $signaturePath -PathType Leaf)){
    try {
        $expectedStatement=New-ReleaseStatementText -Candidate ([string]$version.candidate) -ExeSha256 $exeHash -ManifestSha256 $manifestHash -ManifestAggregateSha256 $computedAggregate -VersionSha256 $versionHash -AttestationSha256 $attestationHash -GitRevision ([string]$attestation.gitRevision).ToLowerInvariant() -WorktreeDirty ([bool]$attestation.worktreeDirty) -DirtyDigest ([string]$attestation.worktreeStatusSha256).ToUpperInvariant()
        $expectedBytes=[Text.Encoding]::UTF8.GetBytes($expectedStatement); $actualBytes=[IO.File]::ReadAllBytes($statementPath)
        if([Convert]::ToBase64String($actualBytes) -cne [Convert]::ToBase64String($expectedBytes)){Fail 'release statement is not the exact canonical binding'}
        else{$signatureValid=Test-DetachedCmsSignature -Content $actualBytes -Signature ([IO.File]::ReadAllBytes($signaturePath)) -TrustedThumbprint $TrustedSignerThumbprint; if(-not $signatureValid){Fail 'detached CMS signature or trusted signer thumbprint mismatch'}}
    }catch{Fail "release statement/signature validation failed closed: $($_.Exception.Message)"}
}else{Fail 'release statement/signature cannot be validated without valid attestation, manifest, and VERSION'}

$sourceRepoCheck = 'unavailable'; $currentHead = ''
if (Test-PathWithin -Path $PackageRoot -Root $RepoRoot) {
    if($attestationShapeValid){$sourceRepoCheck='checked';$worktree=Get-WorktreeState -Root $RepoRoot;$currentHead=$worktree.revision;if($attestation.gitRevision.ToLowerInvariant() -ne $worktree.revision -or [bool]$attestation.worktreeDirty -ne [bool]$worktree.dirty -or $attestation.worktreeStatusScope -ne $worktree.statusScope -or [int]$attestation.worktreeStatusLineCount -ne [int]$worktree.statusLineCount -or $attestation.worktreeStatusSha256.ToUpperInvariant() -ne $worktree.statusSha256){Fail 'attestation does not match current repository state'};$cargoVersion=(& cargo +1.95.0 --version 2>$null).Trim();$rustcVersion=(& rustc +1.95.0 --version 2>$null).Trim();if($attestation.cargoVersion -ne $cargoVersion -or $attestation.rustcVersion -ne $rustcVersion){Fail 'attestation toolchain differs from current pinned toolchain'}}else{$sourceRepoCheck='blocked-invalid-attestation'}
}

New-Item -ItemType Directory -Force -Path $EvidenceDir | Out-Null; $candidateName = if ($versionShapeValid) { [string]$version.candidate } else { 'unknown' }
$verification = [ordered]@{ schema = 'mir2.windows.package-verification.v4'; packageRoot = $PackageRoot; nonvisual = (-not $Launch); launchRequested = [bool]$Launch; sourceRepoCheck = $sourceRepoCheck; currentSourceRevision = $currentHead; attestationPresent = ($null -ne $attestation); attestationSha256 = $attestationHash; packageManifestPresent = ($null -ne $manifest); packageManifestSha256=$manifestHash; packageManifestAggregateSha256 = $computedAggregate; trustedSignerThumbprint=(Normalize-Thumbprint $TrustedSignerThumbprint); detachedSignatureValid=[bool]$signatureValid; exeSha256 = $exeHash; peValid = [bool]$pe.valid; peImports = @($pe.imports); peDelayImports = @($pe.delayImports); packageFileCount = $allFiles.Count; failures = @($Failures); passed = ($Failures.Count -eq 0); visualAccepted = $false }
Write-Utf8NoBom -Path (Join-Path $EvidenceDir ($candidateName + '-verification.json')) -Text ($verification | ConvertTo-Json -Depth 8)

if ($Launch -and $Failures.Count -eq 0) {
    $tempBase = [IO.Path]::GetTempPath().TrimEnd('\', '/'); Assert-NoReparseAncestors -Path $tempBase; $tempRoot = Join-Path $tempBase ('mir2-windows-candidate-launch-' + [guid]::NewGuid().ToString('N')); New-Item -ItemType Directory -Path $tempRoot | Out-Null
    try {
        Assert-NoReparseTree -Path $tempRoot; $outsideRoot = Join-Path $tempRoot 'candidate'; Copy-Item -LiteralPath $PackageRoot -Destination $outsideRoot -Recurse -Force; Assert-NoReparseTree -Path $outsideRoot
        $launchResult = Invoke-CandidateProcess -WorkingDirectory $outsideRoot -ExePath (Join-Path $outsideRoot $ExeName) -TimeoutMs $LaunchTimeoutMs -LogPath (Join-Path $EvidenceDir 'explicit-launch.log')
        $missingRoot = Join-Path $tempRoot 'missing-assets'; New-Item -ItemType Directory -Path $missingRoot | Out-Null; Copy-Item -LiteralPath (Join-Path $outsideRoot $ExeName) -Destination $missingRoot; Copy-Item -LiteralPath (Join-Path $outsideRoot 'mir2-client.toml') -Destination $missingRoot; $missing = Invoke-CandidateProcess -WorkingDirectory $missingRoot -ExePath (Join-Path $missingRoot $ExeName) -TimeoutMs 15000 -LogPath (Join-Path $EvidenceDir 'explicit-missing-assets.log'); if ($missing.opened -or $missing.log -notmatch 'FATAL:') { Fail 'missing-assets launch did not fail closed' }
        $badRoot = Join-Path $tempRoot 'bad-ws'; Copy-Item -LiteralPath $outsideRoot -Destination $badRoot -Recurse -Force; Write-Utf8NoBom -Path (Join-Path $badRoot 'mir2-client.toml') -Text "[server]`ngateway_ws_url = `"ws://gateway.example.com/ws`"`n[display]`nwidth = 1024`nheight = 768`n"; $bad = Invoke-CandidateProcess -WorkingDirectory $badRoot -ExePath (Join-Path $badRoot $ExeName) -TimeoutMs 15000 -LogPath (Join-Path $EvidenceDir 'explicit-bad-ws.log'); if ($bad.opened -or $bad.log -notmatch '(?i)(configuration error|wss://)') { Fail 'non-loopback ws:// launch did not fail closed' }
        Write-Host ("explicit launch opened={0}" -f $launchResult.opened)
    } finally { if (Test-Path -LiteralPath $tempRoot) { Remove-SafeTemporaryTree -Path $tempRoot -RequiredPrefix 'mir2-windows-candidate-launch-' } }
} elseif ($Launch) { Write-Host 'launch skipped because static verification failed' }

if ($Failures.Count -gt 0) { Write-Host ("verify-windows-candidate FAILED ({0})" -f $Failures.Count); $Failures | ForEach-Object { Write-Host " - $_" }; exit 1 }
Write-Host "verify-windows-candidate passed; sourceRepoCheck=$sourceRepoCheck; nonvisual=$(-not $Launch)"
exit 0
