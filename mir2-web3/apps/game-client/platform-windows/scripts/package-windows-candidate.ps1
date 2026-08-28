# Stage a Windows Candidate package from an explicitly attested Release EXE.
# The script never builds. -DryRun validates only and never writes dist/target.
[CmdletBinding()]
param(
    [string]$ReleaseExe = '',
    [string]$BuildAttestation = '',
    [string]$CandidateVersion = '',
    [string]$SourceRevision = '',
    [string]$SignerThumbprint = '',
    [string]$OutputRoot = '',
    [switch]$AllowDirtyWorktree,
    [switch]$DryRun,
    [switch]$SelfTest
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$EntityAtlasClosureScript = Join-Path (Split-Path -Parent $PSCommandPath) 'entity-atlas-closure.ps1'
if (-not (Test-Path -LiteralPath $EntityAtlasClosureScript -PathType Leaf)) { throw "entity atlas closure validator missing: $EntityAtlasClosureScript" }
. $EntityAtlasClosureScript

function Write-Utf8NoBom {
    param([Parameter(Mandatory = $true)][string]$Path, [Parameter(Mandatory = $true)][string]$Text)
    [IO.File]::WriteAllText($Path, $Text, [Text.UTF8Encoding]::new($false))
}

function Get-TextSha256 {
    param([Parameter(Mandatory = $true)][string]$Text)
    return ([BitConverter]::ToString([Security.Cryptography.SHA256]::Create().ComputeHash([Text.Encoding]::UTF8.GetBytes($Text)))).Replace('-', '')
}

function Get-ByteSha256 {
    param([byte[]]$Bytes)
    return ([BitConverter]::ToString([Security.Cryptography.SHA256]::Create().ComputeHash($Bytes))).Replace('-', '')
}

function ConvertFrom-JsonPreservingDateStrings {
    param([Parameter(Mandatory = $true)][string]$Text)
    $command = Get-Command ConvertFrom-Json -ErrorAction Stop
    if ($command.Parameters.ContainsKey('DateKind')) {
        return ($Text | ConvertFrom-Json -DateKind String)
    }
    return ($Text | ConvertFrom-Json)
}

function Get-OrdinalSortedStrings {
    param([object[]]$Values)
    $strings = New-Object System.Collections.Generic.List[string]
    foreach ($value in @($Values)) { [void]$strings.Add([string]$value) }
    $result = $strings.ToArray()
    [Array]::Sort($result, [StringComparer]::Ordinal)
    return $result
}

function Get-ManifestCanonicalText {
    param([object[]]$Entries)
    $records = New-Object System.Collections.Generic.List[string]
    foreach ($entry in @($Entries)) { [void]$records.Add([string]::Format([Globalization.CultureInfo]::InvariantCulture, "{0}`t{1}`t{2}", $entry.path, $entry.size, $entry.sha256)) }
    $sorted = $records.ToArray()
    [Array]::Sort($sorted, [StringComparer]::Ordinal)
    return (($sorted -join "`n") + "`n")
}

function Test-ExactProperties {
    param([object]$Value, [string[]]$Expected)
    if ($null -eq $Value) { return $false }
    $actual = @(Get-OrdinalSortedStrings -Values $Value.PSObject.Properties.Name)
    $wanted = @(Get-OrdinalSortedStrings -Values $Expected)
    return (($actual.Count -eq $wanted.Count) -and (($actual -join "`n") -ceq ($wanted -join "`n")))
}

function Test-StructuredBuildContract {
    param([object]$Attestation)
    if (-not (Test-ExactProperties -Value $Attestation.buildCommand -Expected @('executable','toolchain','subcommand','manifestPath','bin','release','locked','target','profile','targetDir','extraArgs'))) { return $false }
    $command = $Attestation.buildCommand
    if ([string]$command.executable -cne 'cargo' -or [string]$command.toolchain -cne '+1.95.0' -or [string]$command.subcommand -cne 'build') { return $false }
    if ([string]$command.manifestPath -cne 'apps/game-client/platform-windows/Cargo.toml' -or [string]$command.bin -cne 'mir2-platform-windows') { return $false }
    if (-not ($command.release -is [bool]) -or $command.release -ne $true -or -not ($command.locked -is [bool]) -or $command.locked -ne $true) { return $false }
    if ([string]$command.target -cne 'x86_64-pc-windows-msvc' -or [string]$command.profile -cne 'release' -or [string]$command.targetDir -cne 'target-attested-windows-candidate') { return $false }
    if (@($command.extraArgs).Count -ne 0) { return $false }
    if (-not (Test-ExactProperties -Value $Attestation.pathRemapping -Expected @('enabled','environmentVariable','flags'))) { return $false }
    $remap = $Attestation.pathRemapping
    if (-not ($remap.enabled -is [bool]) -or $remap.enabled -ne $true -or [string]$remap.environmentVariable -cne 'RUSTFLAGS') { return $false }
    $flags = @($remap.flags)
    if ($flags.Count -ne 2) { return $false }
    foreach ($flag in $flags) { if (-not (Test-ExactProperties -Value $flag -Expected @('sourceToken','destination'))) { return $false } }
    return ([string]$flags[0].sourceToken -ceq '<REPO_ROOT>' -and [string]$flags[0].destination -ceq '.' -and
            [string]$flags[1].sourceToken -ceq '<CARGO_HOME>' -and [string]$flags[1].destination -ceq 'cargo-home')
}

function Initialize-Pkcs {
    if ($null -eq ('System.Security.Cryptography.Pkcs.SignedCms' -as [type])) {
        try { Add-Type -AssemblyName System.Security -ErrorAction Stop } catch {
            try { Add-Type -AssemblyName System.Security.Cryptography.Pkcs -ErrorAction Stop } catch { throw "CMS/PKCS#7 support unavailable: $($_.Exception.Message)" }
        }
    }
    if ($null -eq ('System.Security.Cryptography.Pkcs.SignedCms' -as [type])) { throw 'CMS/PKCS#7 support unavailable' }
}

function Normalize-Thumbprint {
    param([string]$Thumbprint)
    return (($Thumbprint -replace '\s', '').ToUpperInvariant())
}

function Test-CodeSigningCertificate {
    param([Security.Cryptography.X509Certificates.X509Certificate2]$Certificate, [switch]$RequirePrivateKey)
    if ($null -eq $Certificate) { return $false }
    if ($RequirePrivateKey -and -not $Certificate.HasPrivateKey) { return $false }
    $now = [DateTime]::UtcNow
    if ($Certificate.NotBefore.ToUniversalTime() -gt $now -or $Certificate.NotAfter.ToUniversalTime() -lt $now) { return $false }
    $eku = $Certificate.Extensions | Where-Object { $_.Oid.Value -eq '2.5.29.37' } | Select-Object -First 1
    if ($null -eq $eku) { return $false }
    if (@($eku.EnhancedKeyUsages | ForEach-Object { $_.Value }) -notcontains '1.3.6.1.5.5.7.3.3') { return $false }
    $keyUsage = $Certificate.Extensions | Where-Object { $_.Oid.Value -eq '2.5.29.15' } | Select-Object -First 1
    return ($null -eq $keyUsage -or (($keyUsage.KeyUsages -band [Security.Cryptography.X509Certificates.X509KeyUsageFlags]::DigitalSignature) -ne 0))
}

function Get-SigningCertificate {
    param([string]$Thumbprint)
    $normalized = Normalize-Thumbprint -Thumbprint $Thumbprint
    if ($normalized -notmatch '^[0-9A-F]{40}$') { throw 'SignerThumbprint is invalid' }
    $matches = @(Get-ChildItem -LiteralPath 'Cert:\CurrentUser\My' | Where-Object { (Normalize-Thumbprint -Thumbprint $_.Thumbprint) -ceq $normalized })
    if ($matches.Count -ne 1) { throw "SignerThumbprint must identify exactly one CurrentUser/My certificate: $normalized" }
    if (-not (Test-CodeSigningCertificate -Certificate $matches[0] -RequirePrivateKey)) { throw 'signer certificate must be current, have a private key, and carry the Code Signing EKU' }
    return $matches[0]
}

function New-DetachedCmsSignature {
    param([byte[]]$Content, [Security.Cryptography.X509Certificates.X509Certificate2]$Certificate)
    Initialize-Pkcs
    $cms = [Security.Cryptography.Pkcs.SignedCms]::new([Security.Cryptography.Pkcs.ContentInfo]::new($Content), $true)
    $signer = [Security.Cryptography.Pkcs.CmsSigner]::new([Security.Cryptography.Pkcs.SubjectIdentifierType]::IssuerAndSerialNumber, $Certificate)
    $signer.IncludeOption = [Security.Cryptography.X509Certificates.X509IncludeOption]::EndCertOnly
    $cms.ComputeSignature($signer, $false)
    return $cms.Encode()
}

function New-ReleaseStatementText {
    param([string]$Candidate, [string]$ExeSha256, [string]$ManifestSha256, [string]$ManifestAggregateSha256, [string]$VersionSha256, [string]$AttestationSha256, [string]$GitRevision, [bool]$WorktreeDirty, [string]$DirtyDigest)
    foreach ($hash in @($ExeSha256,$ManifestSha256,$ManifestAggregateSha256,$VersionSha256,$AttestationSha256,$DirtyDigest)) { if ($hash -notmatch '^[0-9A-F]{64}$') { throw 'release statement received an invalid SHA256' } }
    if ($Candidate -notmatch '^WN-CANDIDATE-[A-Za-z0-9._-]+$' -or $GitRevision -notmatch '^[0-9a-f]{40}$') { throw 'release statement identity is invalid' }
    $dirty = if ($WorktreeDirty) { 'true' } else { 'false' }
    return '{"schema":"mir2.windows.release-statement.v1","candidate":"' + $Candidate + '","exeSha256":"' + $ExeSha256 + '","packageManifestSha256":"' + $ManifestSha256 + '","packageManifestAggregateSha256":"' + $ManifestAggregateSha256 + '","versionSha256":"' + $VersionSha256 + '","buildAttestationSha256":"' + $AttestationSha256 + '","gitRevision":"' + $GitRevision + '","worktreeDirty":' + $dirty + ',"worktreeStatusSha256":"' + $DirtyDigest + '"}'
}

function Invoke-GitBytes {
    param([string]$Root, [string]$Arguments)
    $info = [Diagnostics.ProcessStartInfo]::new()
    $info.FileName = 'git.exe'; $info.Arguments = $Arguments; $info.WorkingDirectory = $Root; $info.UseShellExecute = $false; $info.CreateNoWindow = $true; $info.RedirectStandardOutput = $true; $info.RedirectStandardError = $true
    $process = [Diagnostics.Process]::new(); $process.StartInfo = $info
    if (-not $process.Start()) { throw 'failed to start git' }
    $memory = [IO.MemoryStream]::new()
    try { $process.StandardOutput.BaseStream.CopyTo($memory); $stderr = $process.StandardError.ReadToEnd(); $process.WaitForExit(); if ($process.ExitCode -ne 0) { throw "git failed ($($process.ExitCode)): $stderr" }; return ,$memory.ToArray() }
    finally { $memory.Dispose(); $process.Dispose() }
}

function ConvertFrom-NulUtf8 {
    param([byte[]]$Bytes)
    $result = New-Object System.Collections.Generic.List[string]; $start = 0; $utf8 = [Text.UTF8Encoding]::new($false, $true)
    for ($i = 0; $i -lt $Bytes.Length; $i++) { if ($Bytes[$i] -eq 0) { if ($i -gt $start) { [void]$result.Add($utf8.GetString($Bytes, $start, $i - $start)) }; $start = $i + 1 } }
    if ($start -ne $Bytes.Length) { throw 'NUL-delimited git output is unterminated' }
    return @($result)
}

function Resolve-FullPath {
    param([Parameter(Mandatory = $true)][string]$Path)
    return (Resolve-Path -LiteralPath $Path -ErrorAction Stop).Path.TrimEnd('\', '/')
}

function Find-RepoRoot {
    param([Parameter(Mandatory = $true)][string]$StartPath)
    $cursor = (Get-Item -LiteralPath $StartPath).FullName
    while ($true) {
        if ((Test-Path -LiteralPath (Join-Path $cursor '.git')) -or
            ((Test-Path -LiteralPath (Join-Path $cursor 'apps')) -and
             (Test-Path -LiteralPath (Join-Path $cursor 'docs')) -and
             (Test-Path -LiteralPath (Join-Path $cursor 'Cargo.toml')))) {
            return $cursor.TrimEnd('\', '/')
        }
        $parent = Split-Path -Parent $cursor
        if ([string]::IsNullOrWhiteSpace($parent) -or $parent -eq $cursor) { throw "repository root not found from $StartPath" }
        $cursor = $parent
    }
}

function Test-PathWithin {
    param([Parameter(Mandatory = $true)][string]$Path, [Parameter(Mandatory = $true)][string]$Root, [switch]$AllowRoot)
    $pathFull = [IO.Path]::GetFullPath($Path).TrimEnd('\', '/')
    $rootFull = [IO.Path]::GetFullPath($Root).TrimEnd('\', '/')
    if ($AllowRoot -and $pathFull.Equals($rootFull, [StringComparison]::OrdinalIgnoreCase)) { return $true }
    return $pathFull.StartsWith($rootFull + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase) -or
        $pathFull.StartsWith($rootFull + [IO.Path]::AltDirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)
}

function Test-IsReparsePoint {
    param([Parameter(Mandatory = $true)][IO.FileSystemInfo]$Item)
    return (($Item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0)
}

function Assert-NoReparseAncestors {
    param([Parameter(Mandatory = $true)][string]$Path)
    $cursor = [IO.Path]::GetFullPath($Path).TrimEnd('\', '/')
    while (-not (Test-Path -LiteralPath $cursor)) {
        $parent = Split-Path -Parent $cursor
        if ([string]::IsNullOrWhiteSpace($parent) -or $parent -eq $cursor) { break }
        $cursor = $parent
    }
    while (-not [string]::IsNullOrWhiteSpace($cursor)) {
        if (Test-Path -LiteralPath $cursor) {
            $item = Get-Item -LiteralPath $cursor -Force
            if (Test-IsReparsePoint -Item $item) { throw "reparse-point ancestor rejected: $($item.FullName)" }
        }
        $parent = Split-Path -Parent $cursor
        if ([string]::IsNullOrWhiteSpace($parent) -or $parent -eq $cursor) { break }
        $cursor = $parent
    }
}

function Assert-NoReparseTree {
    param([Parameter(Mandatory = $true)][string]$Path)
    if (-not (Test-Path -LiteralPath $Path)) { return }
    Assert-NoReparseAncestors -Path $Path
    $stack = New-Object System.Collections.Generic.Stack[string]
    $rootItem = Get-Item -LiteralPath $Path -Force
    if (Test-IsReparsePoint -Item $rootItem) { throw "reparse-point root rejected: $($rootItem.FullName)" }
    if ($rootItem.PSIsContainer) { $stack.Push($rootItem.FullName) }
    while ($stack.Count -gt 0) {
        $current = $stack.Pop()
        foreach ($child in Get-ChildItem -LiteralPath $current -Force) {
            if (Test-IsReparsePoint -Item $child) { throw "reparse point in tree rejected: $($child.FullName)" }
            if ($child.PSIsContainer) { $stack.Push($child.FullName) }
        }
    }
}

function Initialize-NativeStreamEnumerator {
    if ($null -ne ('Mir2.Windows.NativeStreamEnumerator' -as [type])) { return }
    $source = @'
using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Runtime.InteropServices;
namespace Mir2.Windows {
    public static class NativeStreamEnumerator {
        [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
        private struct StreamData {
            public long StreamSize;
            [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 296)] public string StreamName;
        }
        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        private static extern IntPtr FindFirstStreamW(string fileName, int infoLevel, out StreamData data, uint flags);
        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        private static extern bool FindNextStreamW(IntPtr handle, out StreamData data);
        [DllImport("kernel32.dll", SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        private static extern bool FindClose(IntPtr handle);
        public static string[] List(string path) {
            var names = new List<string>(); StreamData data;
            IntPtr handle = FindFirstStreamW(path, 0, out data, 0);
            if (handle == new IntPtr(-1)) {
                int error = Marshal.GetLastWin32Error();
                if (error == 38) return names.ToArray();
                throw new Win32Exception(error, "FindFirstStreamW failed for " + path);
            }
            try {
                names.Add(data.StreamName);
                while (FindNextStreamW(handle, out data)) names.Add(data.StreamName);
                int error = Marshal.GetLastWin32Error();
                if (error != 38) throw new Win32Exception(error, "FindNextStreamW failed for " + path);
            } finally { FindClose(handle); }
            return names.ToArray();
        }
    }
}
'@
    try { Add-Type -TypeDefinition $source -Language CSharp -ErrorAction Stop } catch { throw "native ADS enumeration facility unavailable: $($_.Exception.Message)" }
}

function Assert-NoAlternateDataStreams {
    param([Parameter(Mandatory = $true)][string]$Path)
    if ($env:OS -ne 'Windows_NT') { throw 'ADS enumeration is mandatory and unavailable on this non-Windows host' }
    $resolved = (Resolve-Path -LiteralPath $Path -ErrorAction Stop).Path
    try { $drive = [IO.DriveInfo]::new([IO.Path]::GetPathRoot($resolved)); $format = $drive.DriveFormat } catch { throw "ADS filesystem capability cannot be established for $resolved : $($_.Exception.Message)" }
    if ($format -ne 'NTFS') { throw "ADS enumeration requires NTFS and fails closed on '$format': $resolved" }
    Initialize-NativeStreamEnumerator
    $rootItem = Get-Item -LiteralPath $resolved -Force -ErrorAction Stop
    $items = New-Object System.Collections.Generic.List[IO.FileSystemInfo]; [void]$items.Add($rootItem)
    if ($rootItem.PSIsContainer) { foreach ($child in Get-ChildItem -LiteralPath $resolved -Recurse -Force -ErrorAction Stop) { [void]$items.Add($child) } }
    foreach ($item in $items) {
        try { $providerStreams = @(Microsoft.PowerShell.Management\Get-Item -LiteralPath $item.FullName -Stream * -Force -ErrorAction Stop); $nativeStreams = @([Mir2.Windows.NativeStreamEnumerator]::List($item.FullName)) } catch { throw "ADS enumeration unavailable for $($item.FullName): $($_.Exception.Message)" }
        foreach ($stream in $providerStreams) { if ([string]$stream.Stream -cne ':$DATA') { throw "named NTFS stream rejected: $($item.FullName):$($stream.Stream)" } }
        foreach ($streamName in $nativeStreams) { if ([string]$streamName -cne '::$DATA' -and [string]$streamName -cne ':$DATA') { throw "named NTFS stream rejected: $($item.FullName)$streamName" } }
    }
}

function Assert-SafeDistTarget {
    param([Parameter(Mandatory = $true)][string]$Path, [Parameter(Mandatory = $true)][string]$DistRoot)
    $full = [IO.Path]::GetFullPath($Path).TrimEnd('\', '/')
    $dist = [IO.Path]::GetFullPath($DistRoot).TrimEnd('\', '/')
    if (-not (Test-PathWithin -Path $full -Root $dist)) { throw "output must be a strict child of dist: $full" }
    if ($full -match '[*?]') { throw "wildcards are forbidden in output path: $full" }
    Assert-NoReparseAncestors -Path $dist
    Assert-NoReparseAncestors -Path $full
    if (Test-Path -LiteralPath $full) { Assert-NoReparseTree -Path $full }
    return $full
}

function Ensure-SafeDirectory {
    param([Parameter(Mandatory = $true)][string]$Path, [Parameter(Mandatory = $true)][string]$BoundaryRoot)
    $target = [IO.Path]::GetFullPath($Path).TrimEnd('\', '/')
    $boundary = [IO.Path]::GetFullPath($BoundaryRoot).TrimEnd('\', '/')
    if (-not (Test-PathWithin -Path $target -Root $boundary -AllowRoot)) { throw "directory creation escapes boundary: $target" }
    Assert-NoReparseAncestors -Path $boundary
    if (-not (Test-Path -LiteralPath $boundary)) { New-Item -ItemType Directory -Path $boundary | Out-Null }
    Assert-NoReparseTree -Path $boundary
    $relative = $target.Substring($boundary.Length).TrimStart('\', '/')
    $current = $boundary
    foreach ($segment in @($relative -split '[\\/]' | Where-Object { $_ })) {
        $current = Join-Path $current $segment
        Assert-NoReparseAncestors -Path $current
        if (-not (Test-Path -LiteralPath $current)) { New-Item -ItemType Directory -Path $current | Out-Null }
        Assert-NoReparseTree -Path $current
    }
    return $target
}

function Remove-SafeTree {
    param([Parameter(Mandatory = $true)][string]$Path, [Parameter(Mandatory = $true)][string]$DistRoot)
    $target = Assert-SafeDistTarget -Path $Path -DistRoot $DistRoot
    if (-not (Test-Path -LiteralPath $target)) { return }
    Assert-NoReparseAncestors -Path $target
    Assert-NoReparseTree -Path $target
    Remove-Item -LiteralPath $target -Recurse -Force
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

function Test-RobocopyAdsExclusionSupport {
    if (Get-Variable -Name Mir2RobocopyAdsExclusionSupported -Scope Script -ErrorAction SilentlyContinue) { return [bool]$script:Mir2RobocopyAdsExclusionSupported }
    $help = ((& robocopy /? 2>&1) -join "`n")
    $supported = $help.Contains('/COPY:copyflag') -and $help.Contains('/DCOPY:copyflag') -and ([regex]::Matches($help, 'X=Skip alt data streams').Count -ge 2)
    $script:Mir2RobocopyAdsExclusionSupported = $supported
    return $supported
}

function Copy-FileDefaultDataOnly {
    param([Parameter(Mandatory = $true)][string]$Source, [Parameter(Mandatory = $true)][string]$Destination)
    Assert-NoAlternateDataStreams -Path $Source
    $parent = Split-Path -Parent $Destination; if (-not (Test-Path -LiteralPath $parent -PathType Container)) { New-Item -ItemType Directory -Path $parent -Force | Out-Null }
    [IO.File]::Copy($Source, $Destination, $true)
    [IO.File]::SetLastWriteTimeUtc($Destination, (Get-Item -LiteralPath $Source -Force).LastWriteTimeUtc)
    Assert-NoAlternateDataStreams -Path $Source; Assert-NoAlternateDataStreams -Path $Destination
}

function Copy-CandidateAssetFile {
    param([Parameter(Mandatory = $true)][string]$Source, [Parameter(Mandatory = $true)][string]$Destination)
    if (-not (Test-Path -LiteralPath $Source -PathType Leaf)) { throw "candidate asset source missing: $Source" }
    $destinationParent = Split-Path -Parent $Destination
    if (-not (Test-Path -LiteralPath $destinationParent -PathType Container)) { New-Item -ItemType Directory -Path $destinationParent -Force | Out-Null }
    Copy-FileDefaultDataOnly -Source $Source -Destination $Destination
}

function Copy-TreeDefaultDataOnly {
    param([Parameter(Mandatory = $true)][string]$Source, [Parameter(Mandatory = $true)][string]$Destination)
    if (-not (Test-Path -LiteralPath $Destination -PathType Container)) { New-Item -ItemType Directory -Path $Destination -Force | Out-Null }
    foreach ($directory in Get-ChildItem -LiteralPath $Source -Recurse -Directory -Force) { $relative = Get-RelativeUnixPath -Root $Source -Path $directory.FullName; New-Item -ItemType Directory -Path (Join-Path $Destination ($relative -replace '/', '\')) -Force | Out-Null }
    foreach ($file in Get-ChildItem -LiteralPath $Source -Recurse -File -Force) { $relative = Get-RelativeUnixPath -Root $Source -Path $file.FullName; Copy-FileDefaultDataOnly -Source $file.FullName -Destination (Join-Path $Destination ($relative -replace '/', '\')) }
}

function Copy-Tree {
    param(
        [Parameter(Mandatory = $true)][string]$Source,
        [Parameter(Mandatory = $true)][string]$Destination,
        [switch]$ForceNoRobocopy
    )
    if (-not (Test-Path -LiteralPath $Source -PathType Container)) { throw "missing source tree: $Source" }
    Assert-NoReparseTree -Path $Source
    Assert-NoAlternateDataStreams -Path $Source
    Assert-NoReparseAncestors -Path $Destination
    New-Item -ItemType Directory -Force -Path $Destination | Out-Null
    Assert-NoReparseTree -Path $Destination
    $robocopy = if ($ForceNoRobocopy) { $null } else { Get-Command robocopy -ErrorAction SilentlyContinue }
    if ($null -ne $robocopy -and (Test-RobocopyAdsExclusionSupport)) {
        & $robocopy.Source $Source $Destination /E /COPY:DATX /DCOPY:DATX /R:2 /W:1 /NFL /NDL /NJH /NJS /nc /ns /np | Out-Null
        if ($LASTEXITCODE -ge 8) { throw "robocopy failed: $Source -> $Destination (exit $LASTEXITCODE)" }
    } else {
        Copy-TreeDefaultDataOnly -Source $Source -Destination $Destination
    }
    Assert-NoReparseTree -Path $Source
    Assert-NoReparseTree -Path $Destination
    Assert-NoAlternateDataStreams -Path $Source
    Assert-NoAlternateDataStreams -Path $Destination
}

function Get-WorktreeState {
    param([Parameter(Mandatory = $true)][string]$Root)
    Push-Location $Root
    try {
        $revision = (& git rev-parse HEAD 2>$null).Trim().ToLowerInvariant()
        if ($LASTEXITCODE -ne 0 -or $revision -notmatch '^[0-9a-f]{40}$') { throw 'git HEAD is unavailable' }
        $gitOptions = '-c core.quotepath=false -c core.autocrlf=false -c core.safecrlf=false'
        $statusBytes = Invoke-GitBytes -Root $Root -Arguments "$gitOptions status --porcelain=v1 -z --untracked-files=all"
        $indexDiffBytes = Invoke-GitBytes -Root $Root -Arguments "$gitOptions diff --cached --no-ext-diff --binary --full-index --"
        $worktreeDiffBytes = Invoke-GitBytes -Root $Root -Arguments "$gitOptions diff --no-ext-diff --binary --full-index --"
        $untrackedPaths = @(ConvertFrom-NulUtf8 -Bytes (Invoke-GitBytes -Root $Root -Arguments "$gitOptions ls-files --others --exclude-standard -z"))
        $records = New-Object System.Collections.Generic.List[string]; $seen = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
        foreach ($gitPath in $untrackedPaths) {
            $normalized = $gitPath.Normalize([Text.NormalizationForm]::FormC).Replace('\', '/')
            if ([IO.Path]::IsPathRooted($normalized) -or $normalized -eq '..' -or $normalized.StartsWith('../') -or $normalized.Contains('/../')) { throw "unsafe untracked path: $gitPath" }
            if (-not $seen.Add($normalized)) { throw "duplicate normalized untracked path: $normalized" }
            $full = [IO.Path]::GetFullPath((Join-Path $Root ($gitPath -replace '/', '\')))
            if (-not (Test-PathWithin -Path $full -Root $Root)) { throw "untracked path escapes repository: $normalized" }
            Assert-NoReparseAncestors -Path $full
            if (-not (Test-Path -LiteralPath $full -PathType Leaf)) { throw "untracked regular file changed during digest: $normalized" }
            $item = Get-Item -LiteralPath $full -Force; if (Test-IsReparsePoint -Item $item) { throw "untracked reparse file rejected: $normalized" }
            $pathBase64 = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($normalized))
            [void]$records.Add(("{0}`t{1}`t{2}" -f $pathBase64, [int64]$item.Length, (Get-FileHash -LiteralPath $full -Algorithm SHA256).Hash.ToUpperInvariant()))
        }
        $recordArray = $records.ToArray(); [Array]::Sort($recordArray, [StringComparer]::Ordinal)
        $scope = 'git-status-z+diff+all-untracked-content-v2'
        $canonical = "SCOPE`n$scope`nREVISION`n$revision`nSTATUS-Z`n$($statusBytes.Length)`n$(Get-ByteSha256 $statusBytes)`nINDEX-DIFF`n$($indexDiffBytes.Length)`n$(Get-ByteSha256 $indexDiffBytes)`nWORKTREE-DIFF`n$($worktreeDiffBytes.Length)`n$(Get-ByteSha256 $worktreeDiffBytes)`nUNTRACKED`n$($recordArray.Count)`n" + ($recordArray -join "`n") + "`n"
        return [ordered]@{ revision = $revision; dirty = ($statusBytes.Length -gt 0); statusLineCount = @(ConvertFrom-NulUtf8 -Bytes $statusBytes).Count; statusScope = $scope; statusSha256 = Get-TextSha256 -Text $canonical }
    } finally { Pop-Location }
}

function Read-BuildAttestation {
    param([Parameter(Mandatory = $true)][string]$Path)
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { throw "build attestation missing: $Path" }
    try { $value = ConvertFrom-JsonPreservingDateStrings -Text (Get-Content -LiteralPath $Path -Raw) } catch { throw "invalid build attestation JSON: $Path" }
    $required = @('schema', 'exeSha256', 'exeSizeBytes', 'gitRevision', 'worktreeDirty', 'worktreeStatusScope', 'worktreeStatusSha256', 'worktreeStatusLineCount', 'cargoVersion', 'rustcVersion', 'buildCommand', 'pathRemapping', 'buildCompletedUtc')
    foreach ($name in $required) { if ($null -eq $value.PSObject.Properties[$name] -or [string]::IsNullOrWhiteSpace([string]$value.$name)) { throw "build attestation missing field: $name" } }
    if ($value.schema -ne 'mir2.windows.build-attestation.v2') { throw 'unsupported build attestation schema' }
    return $value
}

function Assert-Attestation {
    param([object]$Attestation, [string]$AttestationPath, [IO.FileInfo]$Exe, [object]$Worktree, [switch]$DirtyAllowed)
    $exeHash = (Get-FileHash -LiteralPath $Exe.FullName -Algorithm SHA256).Hash.ToUpperInvariant()
    if (-not ($Attestation.worktreeDirty -is [bool]) -or [int]$Attestation.worktreeStatusLineCount -lt 0) { throw 'build attestation dirty/status count types are invalid' }
    if ($Attestation.worktreeStatusScope -ne 'git-status-z+diff+all-untracked-content-v2') { throw 'build attestation worktreeStatusScope is unsupported' }
    if ($Attestation.exeSha256 -ne $exeHash -or [int64]$Attestation.exeSizeBytes -ne [int64]$Exe.Length) { throw 'build attestation does not bind the exact EXE hash/size' }
    if ($Attestation.gitRevision.ToLowerInvariant() -ne $Worktree.revision) { throw 'build attestation gitRevision differs from current repository' }
    if ([bool]$Attestation.worktreeDirty -ne [bool]$Worktree.dirty -or $Attestation.worktreeStatusScope -ne $Worktree.statusScope -or [int]$Attestation.worktreeStatusLineCount -ne [int]$Worktree.statusLineCount -or $Attestation.worktreeStatusSha256.ToUpperInvariant() -ne $Worktree.statusSha256) { throw 'build attestation worktree digest differs from current repository' }
    if ($Worktree.dirty -and -not $DirtyAllowed) { throw 'attested worktree is dirty; pass -AllowDirtyWorktree explicitly' }
    $cargoVersion = (& cargo +1.95.0 --version 2>$null).Trim(); if ($LASTEXITCODE -ne 0 -or $Attestation.cargoVersion -ne $cargoVersion) { throw 'build attestation cargoVersion mismatch' }
    $rustcVersion = (& rustc +1.95.0 --version 2>$null).Trim(); if ($LASTEXITCODE -ne 0 -or $Attestation.rustcVersion -ne $rustcVersion) { throw 'build attestation rustcVersion mismatch' }
    if (-not (Test-StructuredBuildContract -Attestation $Attestation)) { throw 'build attestation structured build/path-remapping contract is not exact' }
    if ([string]$Attestation.buildCompletedUtc -notmatch '(?i)(?:Z|\+00:00)$') { throw 'buildCompletedUtc must carry an explicit UTC offset' }
    try { $completed = [DateTimeOffset]::Parse([string]$Attestation.buildCompletedUtc).UtcDateTime } catch { throw 'buildCompletedUtc is invalid' }
    if ($completed -gt [DateTime]::UtcNow.AddMinutes(5)) { throw 'buildCompletedUtc is in the future' }
    return [ordered]@{ exeSha256 = $exeHash; cargoVersion = $cargoVersion; rustcVersion = $rustcVersion; buildCompletedUtc = $completed.ToString('o'); attestationSha256 = (Get-FileHash -LiteralPath $AttestationPath -Algorithm SHA256).Hash.ToUpperInvariant() }
}

function Get-RelativeUnixPath {
    param([string]$Root, [string]$Path)
    $rootFull = [IO.Path]::GetFullPath($Root).TrimEnd('\', '/'); $pathFull = [IO.Path]::GetFullPath($Path)
    if (-not (Test-PathWithin -Path $pathFull -Root $rootFull)) { throw "path outside package: $pathFull" }
    return $pathFull.Substring($rootFull.Length).TrimStart('\', '/') -replace '\\', '/'
}

function Get-ManifestPayloadFiles {
    param([string]$Root)
    $excluded = @('PACKAGE-MANIFEST.json', 'VERSION.json', 'RELEASE-STATEMENT.json', 'RELEASE-STATEMENT.p7s')
    $map = [Collections.Generic.Dictionary[string,IO.FileInfo]]::new([StringComparer]::Ordinal); $paths = New-Object System.Collections.Generic.List[string]
    foreach ($file in Get-ChildItem -LiteralPath $Root -Recurse -File -Force) { $rel = Get-RelativeUnixPath -Root $Root -Path $file.FullName; if ($excluded -cnotcontains $rel) { if ($map.ContainsKey($rel)) { throw "duplicate package path: $rel" }; $map.Add($rel, $file); [void]$paths.Add($rel) } }
    $pathArray = $paths.ToArray(); [Array]::Sort($pathArray, [StringComparer]::Ordinal)
    return @($pathArray | ForEach-Object { $map[$_] })
}

function Write-PackageManifest {
    param([string]$Root, [string]$OutputPath)
    $entries = @(); $totalBytes = [int64]0
    foreach ($file in Get-ManifestPayloadFiles -Root $Root) {
        $rel = Get-RelativeUnixPath -Root $Root -Path $file.FullName; $hash = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToUpperInvariant(); $totalBytes += [int64]$file.Length
        $entries += [ordered]@{ path = $rel; size = [int64]$file.Length; sha256 = $hash }
    }
    $canonical = Get-ManifestCanonicalText -Entries $entries
    $manifest = [ordered]@{ schema = 'mir2.windows.package-manifest.v4'; coverage = [ordered]@{ excludes = @('PACKAGE-MANIFEST.json', 'VERSION.json', 'RELEASE-STATEMENT.json', 'RELEASE-STATEMENT.p7s'); rule = 'Payload files are hashed; manifest/version are bound by the detached signed release statement; statement/signature are excluded to avoid self-reference.' }; fileCount = $entries.Count; totalBytes = $totalBytes; aggregateSha256 = Get-TextSha256 -Text $canonical; files = $entries }
    Write-Utf8NoBom -Path $OutputPath -Text ($manifest | ConvertTo-Json -Depth 8)
    return $manifest
}

function Test-FileIdentity {
    param([string]$Path, [int64]$ExpectedSize, [string]$ExpectedSha256)
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { return $false }
    $item = Get-Item -LiteralPath $Path
    return [int64]$item.Length -eq $ExpectedSize -and
        (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToUpperInvariant() -ceq $ExpectedSha256
}

function Test-PathContainsDangerousDotToken {
    param([string]$RelativePath)
    $dangerous = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($token in @('exe','dll','com','scr','cpl','msi','msp','bat','cmd','ps1','psm1','psd1','vbs','vbe','js','jse','wsf','wsh','hta','reg','lnk','chm','jar','py','pyw','sh','bash','zsh','fish','pdb','ilk','dmp')) { [void]$dangerous.Add($token) }
    foreach ($segment in @($RelativePath -split '[\\/]')) {
        if ($segment.Length -gt 0 -and ($segment[$segment.Length - 1] -eq [char]0x20 -or $segment[$segment.Length - 1] -eq [char]0x2E)) { return $true }
        $dotTokens = $segment.Split([char]'.')
        foreach ($dotToken in $dotTokens) { if ($dotToken.Length -gt 0 -and $dotToken[$dotToken.Length - 1] -eq [char]0x20) { return $true } }
        for ($index = 1; $index -lt $dotTokens.Count; $index++) { if ($dangerous.Contains($dotTokens[$index])) { return $true } }
    }
    return $false
}

function Test-PackageRelativeFileAllowed {
    param([string]$RelativePath, [string]$ExeName)
    if ($RelativePath -ceq $ExeName) { return $true }
    if (Test-PathContainsDangerousDotToken -RelativePath $RelativePath) { return $false }
    $rootFiles = @('mir2-client.toml', 'README-START.txt', 'CONTROLS.txt', 'KNOWN-ISSUES.md', 'BUILD-ATTESTATION.json', 'PACKAGE-MANIFEST.json', 'VERSION.json', 'RELEASE-STATEMENT.json', 'RELEASE-STATEMENT.p7s')
    if ($rootFiles -ccontains $RelativePath) { return $true }
    if ($RelativePath -ceq 'mir2-assets/original-ui/frame-sets.generated.json') { return $true }
    if (@('mir2-assets/original-ui/Sound/005-1.wav','mir2-assets/original-ui/Sound/005-2.wav','mir2-assets/original-ui/Sound/005-3.wav','mir2-assets/original-ui/Sound/60.wav','mir2-assets/original-ui/Sound/61.wav','mir2-assets/original-ui/Sound/62.wav','mir2-assets/original-ui/Sound/63.wav','mir2-assets/original-ui/Sound/64.wav','mir2-assets/original-ui/Sound/65.wav') -ccontains $RelativePath) { return $true }
    if (@('mir2-assets/original-ui/Sound/Login2.wav','mir2-assets/original-ui/Sound/Select2.wav','mir2-assets/original-ui/Sound/103.wav','mir2-assets/original-ui/Sound/70.wav','mir2-assets/original-ui/Sound/71.wav','mir2-assets/original-ui/Sound/72.wav','mir2-assets/original-ui/Sound/73.wav','mir2-assets/original-ui/Sound/80.wav','mir2-assets/original-ui/Sound/81.wav','mir2-assets/original-ui/Sound/82.wav','mir2-assets/original-ui/Sound/83.wav','mir2-assets/original-ui/Sound/138.wav','mir2-assets/original-ui/Sound/139.wav','mir2-assets/original-ui/Sound/144.wav','mir2-assets/original-ui/Sound/145.wav','mir2-assets/original-ui/Sound/tiger_struck_1.wav','mir2-assets/original-ui/Sound/tiger_struck_2.wav','mir2-assets/original-ui/Sound/wolf_struck1.wav','mir2-assets/original-ui/Sound/M8-1.wav','mir2-assets/original-ui/Sound/M31-0.wav','mir2-assets/original-ui/Sound/M31-1.wav','mir2-assets/original-ui/Sound/M31-2.wav','mir2-assets/original-ui/Sound/M34-0.wav','mir2-assets/original-ui/Sound/M34-1.wav','mir2-assets/original-ui/Sound/M34-2.wav','mir2-assets/original-ui/Sound/M39-0.wav','mir2-assets/original-ui/Sound/M39-1.wav','mir2-assets/original-ui/Sound/M40-0.wav','mir2-assets/original-ui/Sound/M61-0.wav','mir2-assets/original-ui/Sound/M61-1.wav','mir2-assets/original-ui/Sound/M64-0.wav','mir2-assets/original-ui/Sound/M64-1.wav','mir2-assets/original-ui/Sound/M64-2.wav','mir2-assets/original-ui/Sound/M79-1.wav') -ccontains $RelativePath) { return $true }
    if ($RelativePath.StartsWith('mir2-assets/crystal-map-pack/', [StringComparison]::Ordinal)) { return $RelativePath.EndsWith('.map.gz', [StringComparison]::OrdinalIgnoreCase) }
    $imageJsonRoots = @('mir2-assets/bevy-entity-atlases/', 'mir2-assets/generated/map-atlas/', 'mir2-assets/generated/native-map-keyed/', 'mir2-assets/original-effects/', 'mir2-assets/original-ui/ChrSel/', 'mir2-assets/original-ui/Help/', 'mir2-assets/original-ui/MMap/', 'mir2-assets/original-ui/Prguse/', 'mir2-assets/original-ui/Prguse2/', 'mir2-assets/original-ui/UI_32bit/', 'mir2-assets/original-ui/Title/', 'mir2-assets/original-ui/AArmour/00/', 'mir2-assets/original-ui/Monster/000/', 'mir2-assets/original-ui/NPC/00/')
    foreach ($prefix in $imageJsonRoots) { if ($RelativePath.StartsWith($prefix, [StringComparison]::Ordinal)) { return $RelativePath.EndsWith('.json', [StringComparison]::OrdinalIgnoreCase) -or $RelativePath.EndsWith('.png', [StringComparison]::OrdinalIgnoreCase) } }
    return $false
}

function Test-PackageRelativeDirectoryAllowed {
    param([string]$RelativePath)
    if (Test-PathContainsDangerousDotToken -RelativePath $RelativePath) { return $false }
    if (@('logs', 'mir2-assets', 'mir2-assets/generated', 'mir2-assets/original-ui', 'mir2-assets/original-ui/AArmour', 'mir2-assets/original-ui/Monster', 'mir2-assets/original-ui/NPC', 'mir2-assets/original-ui/Sound') -ccontains $RelativePath) { return $true }
    $treeRoots = @('mir2-assets/bevy-entity-atlases', 'mir2-assets/generated/map-atlas', 'mir2-assets/generated/native-map-keyed', 'mir2-assets/crystal-map-pack', 'mir2-assets/original-effects', 'mir2-assets/original-ui/ChrSel', 'mir2-assets/original-ui/Help', 'mir2-assets/original-ui/MMap', 'mir2-assets/original-ui/Prguse', 'mir2-assets/original-ui/Prguse2', 'mir2-assets/original-ui/UI_32bit', 'mir2-assets/original-ui/Title', 'mir2-assets/original-ui/AArmour/00', 'mir2-assets/original-ui/Monster/000', 'mir2-assets/original-ui/NPC/00')
    foreach ($root in $treeRoots) { if ($RelativePath -ceq $root -or $RelativePath.StartsWith($root + '/', [StringComparison]::Ordinal)) { return $true } }
    return $false
}

function Assert-PackageAllowlist {
    param([string]$Root, [string]$ExeName)
    foreach ($directory in Get-ChildItem -LiteralPath $Root -Recurse -Directory -Force) { $rel = Get-RelativeUnixPath -Root $Root -Path $directory.FullName; if (-not (Test-PackageRelativeDirectoryAllowed -RelativePath $rel)) { throw "package directory outside strict allowlist: $rel" } }
    foreach ($file in Get-ChildItem -LiteralPath $Root -Recurse -File -Force) { $rel = Get-RelativeUnixPath -Root $Root -Path $file.FullName; if (-not (Test-PackageRelativeFileAllowed -RelativePath $rel -ExeName $ExeName)) { throw "package file outside strict path/extension allowlist: $rel" } }
}

if ($SelfTest) {
    $dateVector = ConvertFrom-JsonPreservingDateStrings -Text '{"buildCompletedUtc":"2026-08-25T20:51:33.9697458+00:00"}'
    if (-not ($dateVector.buildCompletedUtc -is [string]) -or [string]$dateVector.buildCompletedUtc -cne '2026-08-25T20:51:33.9697458+00:00') {
        throw 'JSON parser changed an attestation UTC string into a locale-dependent value'
    }
    $selfRoot = Join-Path ([IO.Path]::GetTempPath()) ('mir2-package-selftest-' + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $selfRoot | Out-Null
    $junctionPath = Join-Path $selfRoot 'dist\junction'
    try {
        $selfDist = Join-Path $selfRoot 'dist'; New-Item -ItemType Directory -Path $selfDist | Out-Null
        $escaped = $false; try { Assert-SafeDistTarget -Path (Join-Path $selfDist '..\outside') -DistRoot $selfDist | Out-Null } catch { $escaped = $true }; if (-not $escaped) { throw 'malicious output escape was accepted' }
        $rootAccepted = $false; try { Assert-SafeDistTarget -Path $selfDist -DistRoot $selfDist | Out-Null; $rootAccepted = $true } catch { }; if ($rootAccepted) { throw 'dist root was accepted as output' }
        $source = Join-Path $selfRoot 'source'; $nested = Join-Path $source 'nested'; New-Item -ItemType Directory -Path $nested | Out-Null; [IO.File]::WriteAllBytes((Join-Path $source 'a.bin'), [byte[]](1,2,3,4)); [IO.File]::WriteAllText((Join-Path $nested 'b.txt'), 'fallback-copy', [Text.UTF8Encoding]::new($false)); $destination = Join-Path $selfRoot 'fallback-copy'; Copy-Tree -Source $source -Destination $destination -ForceNoRobocopy
        foreach ($rel in @('a.bin', 'nested\b.txt')) { $left = (Get-FileHash -LiteralPath (Join-Path $source $rel) -Algorithm SHA256).Hash; $right = (Get-FileHash -LiteralPath (Join-Path $destination $rel) -Algorithm SHA256).Hash; if ($left -ne $right) { throw "fallback copy hash mismatch: $rel" } }
        $frameSource = Join-Path $selfRoot 'frame-sets.generated.json'; $frameDestination = Join-Path $selfRoot 'copied\original-ui\frame-sets.generated.json'; [IO.File]::WriteAllText($frameSource, '{"selftest":true}', [Text.UTF8Encoding]::new($false)); Copy-CandidateAssetFile -Source $frameSource -Destination $frameDestination; if ((Get-FileHash -LiteralPath $frameSource -Algorithm SHA256).Hash -ne (Get-FileHash -LiteralPath $frameDestination -Algorithm SHA256).Hash) { throw 'frame-set catalog copy hash mismatch' }
        $identityProbe = Join-Path $selfRoot '103.wav'; [IO.File]::WriteAllBytes($identityProbe, [byte[]](1,2,3)); $identityProbeHash = (Get-FileHash -LiteralPath $identityProbe -Algorithm SHA256).Hash.ToUpperInvariant(); if (Test-FileIdentity -Path $identityProbe -ExpectedSize 4 -ExpectedSha256 $identityProbeHash) { throw 'sound identity self-test accepted wrong size' }; if (Test-FileIdentity -Path $identityProbe -ExpectedSize 3 -ExpectedSha256 ('0' * 64)) { throw 'sound identity self-test accepted wrong hash' }; if (-not (Test-FileIdentity -Path $identityProbe -ExpectedSize 3 -ExpectedSha256 $identityProbeHash)) { throw 'sound identity self-test rejected exact file' }
        Assert-NoAlternateDataStreams -Path $source; Assert-NoAlternateDataStreams -Path $destination
        $adsResult='creation-unsupported';$adsSource=Join-Path $selfRoot 'ads-source';New-Item -ItemType Directory -Path $adsSource|Out-Null;$adsFile=Join-Path $adsSource 'payload.bin';[IO.File]::WriteAllBytes($adsFile,[byte[]](1,2,3));$adsCreated=$false
        try{Set-Content -LiteralPath $adsFile -Stream Zone.Identifier -Value '[ZoneTransfer]' -NoNewline -ErrorAction Stop;Set-Content -LiteralPath $adsSource -Stream DirectoryMarker -Value marker -NoNewline -ErrorAction Stop;$adsCreated=$true}catch{$adsCreated=$false}
        if($adsCreated){$sourceRejected=$false;try{Assert-NoAlternateDataStreams -Path $adsSource}catch{$sourceRejected=$true};if(-not$sourceRejected){throw 'source file/directory ADS was accepted'};$adsStaging=Join-Path $selfRoot 'ads-staging';New-Item -ItemType Directory -Path $adsStaging|Out-Null;$stagingFile=Join-Path $adsStaging 'candidate.bin';[IO.File]::WriteAllBytes($stagingFile,[byte[]](4,5,6));Set-Content -LiteralPath $stagingFile -Stream StageMarker -Value marker -NoNewline;$stagingRejected=$false;try{Assert-NoAlternateDataStreams -Path $adsStaging}catch{$stagingRejected=$true};if(-not$stagingRejected){throw 'staging ADS was accepted'};if(-not(Test-RobocopyAdsExclusionSupport)){throw 'current robocopy does not advertise DATX ADS exclusion'};$adsCopy=Join-Path $selfRoot 'ads-robocopy-destination';New-Item -ItemType Directory -Path $adsCopy|Out-Null;& robocopy $adsSource $adsCopy /E /COPY:DATX /DCOPY:DATX /R:0 /W:0 /NFL /NDL /NJH /NJS /NP|Out-Null;if($LASTEXITCODE-ge 8){throw "ADS robocopy self-test failed: $LASTEXITCODE"};Assert-NoAlternateDataStreams -Path $adsCopy;$adsResult='passed'}
        Write-Host "ADS_SELFTEST=$adsResult"
        $digestRepo = Join-Path $selfRoot 'digest repo'; New-Item -ItemType Directory -Path $digestRepo | Out-Null
        & git -C $digestRepo init -q; if ($LASTEXITCODE -ne 0) { throw 'git init failed in dirty-digest self-test' }
        & git -C $digestRepo config user.email 'selftest@example.invalid'; & git -C $digestRepo config user.name 'Candidate SelfTest'; & git -C $digestRepo commit --allow-empty -q -m baseline
        if ($LASTEXITCODE -ne 0) { throw 'baseline commit failed in dirty-digest self-test' }
        [IO.File]::WriteAllBytes((Join-Path $digestRepo 'payload.bin'), [byte[]](1,2,3,4))
        [IO.File]::WriteAllBytes((Join-Path $digestRepo 'space name.dat'), [byte[]](5,6,7,8))
        $unicodeName = ([string][char]0x4E2D) + ([string][char]0x6587) + ' name.dat'
        [IO.File]::WriteAllBytes((Join-Path $digestRepo $unicodeName), [byte[]](9,10,11,12))
        $digest1 = Get-WorktreeState -Root $digestRepo
        [IO.File]::WriteAllBytes((Join-Path $digestRepo 'payload.bin'), [byte[]](4,3,2,1)); $digest2 = Get-WorktreeState -Root $digestRepo
        if ($digest1.statusSha256 -eq $digest2.statusSha256) { throw '.bin untracked content change did not alter dirty digest' }
        [IO.File]::WriteAllBytes((Join-Path $digestRepo 'space name.dat'), [byte[]](8,7,6,5)); $digest3 = Get-WorktreeState -Root $digestRepo
        if ($digest2.statusSha256 -eq $digest3.statusSha256 -or $digest3.statusScope -ne 'git-status-z+diff+all-untracked-content-v2') { throw '.dat/space/Unicode dirty-digest self-test failed' }
        $exactAttestation = [pscustomobject]@{
            buildCommand = [pscustomobject]@{ executable='cargo'; toolchain='+1.95.0'; subcommand='build'; manifestPath='apps/game-client/platform-windows/Cargo.toml'; bin='mir2-platform-windows'; release=$true; locked=$true; target='x86_64-pc-windows-msvc'; profile='release'; targetDir='target-attested-windows-candidate'; extraArgs=@() }
            pathRemapping = [pscustomobject]@{ enabled=$true; environmentVariable='RUSTFLAGS'; flags=@([pscustomobject]@{ sourceToken='<REPO_ROOT>'; destination='.' }, [pscustomobject]@{ sourceToken='<CARGO_HOME>'; destination='cargo-home' }) }
        }
        if (-not (Test-StructuredBuildContract -Attestation $exactAttestation)) { throw 'exact structured build contract was rejected' }
        $nearMiss = $exactAttestation | ConvertTo-Json -Depth 8 | ConvertFrom-Json; $nearMiss.buildCommand.bin = 'different-bin'
        if (Test-StructuredBuildContract -Attestation $nearMiss) { throw 'near-miss bin was accepted' }
        $nearMiss = $exactAttestation | ConvertTo-Json -Depth 8 | ConvertFrom-Json; $nearMiss.buildCommand.manifestPath = 'Cargo.toml'
        if (Test-StructuredBuildContract -Attestation $nearMiss) { throw 'near-miss manifest path was accepted' }
        $nearMiss = $exactAttestation | ConvertTo-Json -Depth 8 | ConvertFrom-Json; $nearMiss.buildCommand.extraArgs = @('--package','other')
        if (Test-StructuredBuildContract -Attestation $nearMiss) { throw 'extra build arguments were accepted' }
        $manifestRoot = Join-Path $selfRoot 'manifest-root'; New-Item -ItemType Directory -Path $manifestRoot | Out-Null
        [IO.File]::WriteAllBytes((Join-Path $manifestRoot 'z.bin'), [byte[]](1)); [IO.File]::WriteAllBytes((Join-Path $manifestRoot 'A.bin'), [byte[]](2))
        $selfManifest = Write-PackageManifest -Root $manifestRoot -OutputPath (Join-Path $manifestRoot 'PACKAGE-MANIFEST.json')
        if ($selfManifest.schema -ne 'mir2.windows.package-manifest.v4' -or (@($selfManifest.files | ForEach-Object { $_.path }) -join ',') -cne 'A.bin,z.bin') { throw 'ordinal deterministic manifest self-test failed' }
        $cultureEntries = @(
            [pscustomobject]@{ path='I.png'; size=[int64]1; sha256=('1'*64) },
            [pscustomobject]@{ path='i.png'; size=[int64]2; sha256=('2'*64) },
            [pscustomobject]@{ path=(([string][char]0x0130)+'.png'); size=[int64]3; sha256=('3'*64) },
            [pscustomobject]@{ path=(([string][char]0x0131)+'.png'); size=[int64]4; sha256=('4'*64) },
            [pscustomobject]@{ path=(([string][char]0x4E2D)+([string][char]0x6587)+'.json'); size=[int64]5; sha256=('5'*64) }
        )
        $selfTestThread = [Threading.Thread]::CurrentThread; $savedCulture = $selfTestThread.CurrentCulture; $savedUiCulture = $selfTestThread.CurrentUICulture; $cultureAggregate = $null
        try {
            foreach ($cultureName in @('tr-TR','zh-CN','en-US')) {
                $testCulture = [Globalization.CultureInfo]::GetCultureInfo($cultureName); $selfTestThread.CurrentCulture = $testCulture; $selfTestThread.CurrentUICulture = $testCulture
                $candidateAggregate = Get-TextSha256 -Text (Get-ManifestCanonicalText -Entries $cultureEntries)
                if ($null -eq $cultureAggregate) { $cultureAggregate = $candidateAggregate } elseif ($candidateAggregate -cne $cultureAggregate) { throw "manifest aggregate changed under culture: $cultureName" }
            }
        } finally { $selfTestThread.CurrentCulture = $savedCulture; $selfTestThread.CurrentUICulture = $savedUiCulture }
        $unicodeNoBreakSpace = [string][char]0x00A0; $unicodeFullwidthFullStop = [string][char]0xFF0E
        $allowedLayoutVectors = @('mir2-platform-windows.exe','mir2-client.toml','README-START.txt','CONTROLS.txt','KNOWN-ISSUES.md','BUILD-ATTESTATION.json','PACKAGE-MANIFEST.json','VERSION.json','RELEASE-STATEMENT.json','RELEASE-STATEMENT.p7s','mir2-assets/bevy-entity-atlases/manifest.json','mir2-assets/generated/map-atlas/0.png','mir2-assets/generated/native-map-keyed/manifest.json','mir2-assets/crystal-map-pack/0.map.gz','mir2-assets/original-effects/effects.generated.json','mir2-assets/original-effects/sprite.v1.final.PNG',('mir2-assets/original-effects/folder'+$unicodeNoBreakSpace+'/0.png'),('mir2-assets/original-effects/payload'+$unicodeFullwidthFullStop+'exe.png'),'mir2-assets/original-ui/frame-sets.generated.json','mir2-assets/original-ui/ChrSel/0.png','mir2-assets/original-ui/Sound/Login2.wav','mir2-assets/original-ui/Sound/Select2.wav','mir2-assets/original-ui/Sound/103.wav','mir2-assets/original-ui/Sound/70.wav','mir2-assets/original-ui/Sound/71.wav','mir2-assets/original-ui/Sound/72.wav','mir2-assets/original-ui/Sound/73.wav','mir2-assets/original-ui/Sound/80.wav','mir2-assets/original-ui/Sound/81.wav','mir2-assets/original-ui/Sound/82.wav','mir2-assets/original-ui/Sound/83.wav','mir2-assets/original-ui/Sound/138.wav','mir2-assets/original-ui/Sound/139.wav','mir2-assets/original-ui/Sound/144.wav','mir2-assets/original-ui/Sound/145.wav','mir2-assets/original-ui/Sound/tiger_struck_1.wav','mir2-assets/original-ui/Sound/tiger_struck_2.wav','mir2-assets/original-ui/Sound/wolf_struck1.wav','mir2-assets/original-ui/Sound/M8-1.wav','mir2-assets/original-ui/Sound/M31-0.wav','mir2-assets/original-ui/Sound/M31-1.wav','mir2-assets/original-ui/Sound/M31-2.wav','mir2-assets/original-ui/Sound/M34-0.wav','mir2-assets/original-ui/Sound/M34-1.wav','mir2-assets/original-ui/Sound/M34-2.wav','mir2-assets/original-ui/Sound/M39-0.wav','mir2-assets/original-ui/Sound/M39-1.wav','mir2-assets/original-ui/Sound/M40-0.wav','mir2-assets/original-ui/Sound/M61-0.wav','mir2-assets/original-ui/Sound/M61-1.wav','mir2-assets/original-ui/Sound/M64-0.wav','mir2-assets/original-ui/Sound/M64-1.wav','mir2-assets/original-ui/Sound/M64-2.wav','mir2-assets/original-ui/Sound/M79-1.wav')
        $allowedLayoutVectors += @('mir2-assets/original-ui/Sound/005-1.wav','mir2-assets/original-ui/Sound/005-2.wav','mir2-assets/original-ui/Sound/005-3.wav','mir2-assets/original-ui/Sound/60.wav','mir2-assets/original-ui/Sound/61.wav','mir2-assets/original-ui/Sound/62.wav','mir2-assets/original-ui/Sound/63.wav','mir2-assets/original-ui/Sound/64.wav','mir2-assets/original-ui/Sound/65.wav')
        foreach ($rel in $allowedLayoutVectors) { if (-not (Test-PackageRelativeFileAllowed -RelativePath $rel -ExeName 'mir2-platform-windows.exe')) { throw "strict allowlist rejected valid path: $rel" } }
        $unicodeDangerousName = ([string][char]0x5B89) + ([string][char]0x5168) + '.JsE.png'
        $blockedLayoutVectors = @('extra.txt','mir2-assets/unknown/0.png','mir2-assets/crystal-map-pack/0.gz','mir2-assets/crystal-map-pack/0.png','mir2-assets/original-effects/payload.map.gz','mir2-assets/original-ui/Sound/Other.wav','mir2-assets/original-effects/payload.exe.png','mir2-assets/original-effects/config.cmd.json','mir2-assets/crystal-map-pack/payload.ps1.map.gz','mir2-assets/original-effects/PAYLOAD.DLL.PNG',('mir2-assets/original-effects/'+$unicodeDangerousName),'mir2-assets/original-effects/folder.BAT/0.png','mir2-assets/original-effects/folder.BAT /0.png','mir2-assets/original-effects/folder.bAt./0.png','mir2-assets/original-effects/payload.exe .png','mir2-assets/original-effects/PAYLOAD.ExE .PNG')
        foreach ($extension in @('.exe','.dll','.com','.scr','.cpl','.msi','.msp','.bat','.cmd','.ps1','.psm1','.psd1','.vbs','.vbe','.js','.jse','.wsf','.wsh','.hta','.reg','.lnk','.chm','.jar','.py','.pyw','.sh','.bash','.zsh','.fish','.pdb','.ilk','.dmp','.dat','.unknown')) { $blockedLayoutVectors += 'mir2-assets/original-effects/payload' + $extension }
        foreach ($rel in $blockedLayoutVectors) { if (Test-PackageRelativeFileAllowed -RelativePath $rel -ExeName 'mir2-platform-windows.exe') { throw "strict allowlist accepted blocked path: $rel" } }
        if (Test-PackageRelativeDirectoryAllowed -RelativePath 'mir2-assets/original-effects/folder.BAT') { throw 'strict directory allowlist accepted dangerous intermediate extension' }
        foreach ($unsafeDirectory in @('mir2-assets/original-effects/folder.BAT ','mir2-assets/original-effects/folder.bAt.')) { if (Test-PackageRelativeDirectoryAllowed -RelativePath $unsafeDirectory) { throw "strict directory allowlist accepted Windows-normalized segment: $unsafeDirectory" } }
        foreach ($unicodeDirectory in @(('mir2-assets/original-effects/folder'+$unicodeNoBreakSpace),('mir2-assets/original-effects/folder'+$unicodeFullwidthFullStop))) { if (-not (Test-PackageRelativeDirectoryAllowed -RelativePath $unicodeDirectory)) { throw "strict directory allowlist rejected Unicode-adjacent segment: $unicodeDirectory" } }
        $selfRepo = Find-RepoRoot -StartPath (Split-Path -Parent $PSCommandPath)
        $sourceMappings = @(
            [pscustomobject]@{ source='apps\web\public\bevy-entity-atlases'; destination='mir2-assets/bevy-entity-atlases' }, [pscustomobject]@{ source='apps\web\public\generated\map-atlas'; destination='mir2-assets/generated/map-atlas' }, [pscustomobject]@{ source='apps\web\public\generated\native-map-keyed'; destination='mir2-assets/generated/native-map-keyed' }, [pscustomobject]@{ source='apps\web\lib\generated\crystal-map-pack'; destination='mir2-assets/crystal-map-pack' }, [pscustomobject]@{ source='apps\web\public\original-effects'; destination='mir2-assets/original-effects' }, [pscustomobject]@{ source='apps\web\public\original-ui\ChrSel'; destination='mir2-assets/original-ui/ChrSel' }, [pscustomobject]@{ source='apps\web\public\original-ui\Help'; destination='mir2-assets/original-ui/Help' }, [pscustomobject]@{ source='apps\web\public\original-ui\MMap'; destination='mir2-assets/original-ui/MMap' }, [pscustomobject]@{ source='apps\web\public\original-ui\Prguse'; destination='mir2-assets/original-ui/Prguse' }, [pscustomobject]@{ source='apps\web\public\original-ui\Prguse2'; destination='mir2-assets/original-ui/Prguse2' }, [pscustomobject]@{ source='apps\web\public\original-ui\UI_32bit'; destination='mir2-assets/original-ui/UI_32bit' }, [pscustomobject]@{ source='apps\web\public\original-ui\Title'; destination='mir2-assets/original-ui/Title' }, [pscustomobject]@{ source='apps\web\public\original-ui\AArmour\00'; destination='mir2-assets/original-ui/AArmour/00' }, [pscustomobject]@{ source='apps\web\public\original-ui\Monster\000'; destination='mir2-assets/original-ui/Monster/000' }, [pscustomobject]@{ source='apps\web\public\original-ui\NPC\00'; destination='mir2-assets/original-ui/NPC/00' }
         )
         foreach ($mapping in $sourceMappings) { $sourceRoot=Join-Path $selfRepo $mapping.source; if(-not(Test-Path -LiteralPath $sourceRoot -PathType Container)){throw "allowlist source tree missing: $($mapping.source)"}; foreach($sourceFile in Get-ChildItem -LiteralPath $sourceRoot -Recurse -File -Force){$sourceRel=Get-RelativeUnixPath -Root $sourceRoot -Path $sourceFile.FullName;$targetRel=$mapping.destination+'/'+$sourceRel;if(-not(Test-PackageRelativeFileAllowed -RelativePath $targetRel -ExeName 'mir2-platform-windows.exe')){throw "actual required resource rejected by strict allowlist: $targetRel"}} }
         $frameSetSource = Join-Path $selfRepo 'apps\web\public\original-ui\frame-sets.generated.json'; if (-not (Test-Path -LiteralPath $frameSetSource -PathType Leaf)) { throw 'frame-set catalog source file missing from repository' }; if (-not (Test-PackageRelativeFileAllowed -RelativePath 'mir2-assets/original-ui/frame-sets.generated.json' -ExeName 'mir2-platform-windows.exe')) { throw 'frame-set catalog was rejected by strict allowlist' }
        $layoutRoot = Join-Path $selfRoot 'package-layout'; New-Item -ItemType Directory -Path (Join-Path $layoutRoot 'logs') -Force | Out-Null; New-Item -ItemType Directory -Path (Join-Path $layoutRoot 'mir2-assets\original-effects') -Force | Out-Null; New-Item -ItemType Directory -Path (Join-Path $layoutRoot 'mir2-assets\crystal-map-pack') -Force | Out-Null
        [IO.File]::WriteAllText((Join-Path $layoutRoot 'README-START.txt'),'ok',[Text.UTF8Encoding]::new($false)); [IO.File]::WriteAllBytes((Join-Path $layoutRoot 'mir2-assets\original-effects\0.png'),[byte[]](1)); [IO.File]::WriteAllBytes((Join-Path $layoutRoot 'mir2-assets\crystal-map-pack\0.map.gz'),[byte[]](2)); Assert-PackageAllowlist -Root $layoutRoot -ExeName 'mir2-platform-windows.exe'
        $badLayoutFile = Join-Path $layoutRoot 'mir2-assets\original-effects\payload.cmd'; [IO.File]::WriteAllBytes($badLayoutFile,[byte[]](3)); $layoutRejected=$false; try { Assert-PackageAllowlist -Root $layoutRoot -ExeName 'mir2-platform-windows.exe' } catch { $layoutRejected=$true }; if(-not$layoutRejected){throw 'filesystem strict allowlist accepted executable payload'}; Remove-Item -LiteralPath $badLayoutFile -Force
        $junctionTarget = Join-Path $selfRoot 'junction-target'; New-Item -ItemType Directory -Path $junctionTarget | Out-Null
        $reparseResult = 'skipped'
        $junctionCreated = $false
        try { New-Item -ItemType Junction -Path $junctionPath -Target $junctionTarget -ErrorAction Stop | Out-Null; $junctionCreated = $true } catch { $junctionCreated = $false }
        if ($junctionCreated) {
            $treeRejected = $false; try { Assert-NoReparseTree -Path $selfDist } catch { $treeRejected = $true }; if (-not $treeRejected) { throw 'reparse tree was accepted' }
            $outputRejected = $false; try { Assert-SafeDistTarget -Path (Join-Path $junctionPath 'candidate') -DistRoot $selfDist | Out-Null } catch { $outputRejected = $true }; if (-not $outputRejected) { throw 'reparse output path was accepted' }
            $reparseResult = 'passed'
        }
        Write-Host "REPARSE_SELFTEST=$reparseResult"
        Write-Host 'package-windows-candidate self-test passed'
    } finally {
        if (Test-Path -LiteralPath $junctionPath) { if (-not (Test-PathWithin -Path $junctionPath -Root $selfRoot)) { throw 'unsafe self-test junction cleanup target' }; Remove-Item -LiteralPath $junctionPath -Force }
        if (Test-Path -LiteralPath $selfRoot) { Remove-SafeTemporaryTree -Path $selfRoot -RequiredPrefix 'mir2-package-selftest-' }
    }
    exit 0
}

$invalid = @()
if ([string]::IsNullOrWhiteSpace($ReleaseExe)) { $invalid += 'ReleaseExe' }
if ([string]::IsNullOrWhiteSpace($BuildAttestation)) { $invalid += 'BuildAttestation' }
if ($CandidateVersion -notmatch '^WN-CANDIDATE-[A-Za-z0-9._-]+$') { $invalid += 'CandidateVersion' }
if ($SourceRevision -notmatch '^[0-9a-fA-F]{40}$') { $invalid += 'SourceRevision' }
if ((Normalize-Thumbprint -Thumbprint $SignerThumbprint) -notmatch '^[0-9A-F]{40}$') { $invalid += 'SignerThumbprint' }
if ($invalid.Count -gt 0) { throw ('mandatory attested inputs missing or invalid: ' + ($invalid -join ', ')) }

$ScriptDir = Split-Path -Parent $PSCommandPath
$RepoRoot = Find-RepoRoot -StartPath $ScriptDir
$PublicRoot = Join-Path $RepoRoot 'apps\web\public'
$MapPackRoot = Join-Path $RepoRoot 'apps\web\lib\generated\crystal-map-pack'
$DistRoot = Join-Path $RepoRoot 'dist'
$EvidenceDir = Join-Path $RepoRoot 'docs\generated\player-qa\windows-package-preflight'
$ExeName = 'mir2-platform-windows.exe'
$releaseExeFull = Resolve-FullPath -Path $ReleaseExe
$attestationFull = Resolve-FullPath -Path $BuildAttestation
Assert-NoReparseAncestors -Path $releaseExeFull
Assert-NoReparseTree -Path $releaseExeFull
Assert-NoReparseAncestors -Path $attestationFull
Assert-NoReparseTree -Path $attestationFull
Assert-NoAlternateDataStreams -Path $releaseExeFull
Assert-NoAlternateDataStreams -Path $attestationFull
$exe = Get-Item -LiteralPath $releaseExeFull
if ($exe.Name -ne $ExeName -or $releaseExeFull -notmatch '(?i)[\\/]release[\\/][^\\/]+\.exe$') { throw "ReleaseExe must be $ExeName from an explicit release directory" }
$signingCertificate = Get-SigningCertificate -Thumbprint $SignerThumbprint
$signingProbe = New-DetachedCmsSignature -Content ([Text.Encoding]::UTF8.GetBytes('mir2-candidate-signing-capability-v1')) -Certificate $signingCertificate
if ($null -eq $signingProbe -or $signingProbe.Length -eq 0) { throw 'signer private key could not produce a detached CMS signature' }
$worktree = Get-WorktreeState -Root $RepoRoot
if ($worktree.revision -ne $SourceRevision.ToLowerInvariant()) { throw 'SourceRevision differs from current repository HEAD' }
$attestation = Read-BuildAttestation -Path $attestationFull
$attested = Assert-Attestation -Attestation $attestation -AttestationPath $attestationFull -Exe $exe -Worktree $worktree -DirtyAllowed:$AllowDirtyWorktree

$webRoot = Join-Path $RepoRoot 'apps\web'
$dryRunNativeKeyedRoot = $null
try {
    if ($DryRun) {
        $dryRunNativeKeyedRoot = Join-Path ([IO.Path]::GetTempPath()) ('native-keyed-map-dryrun-' + [guid]::NewGuid().ToString('N'))
        & npm.cmd --prefix $webRoot run assets:native-map-keyed:build -- --outputRoot $dryRunNativeKeyedRoot
    } else {
        & npm.cmd --prefix $webRoot run assets:native-map-keyed:build
    }
    if ($LASTEXITCODE -ne 0) { throw "native keyed map generation failed with exit code $LASTEXITCODE" }
    $nativeKeyedMapRoot = if ($DryRun) { $dryRunNativeKeyedRoot } else { Join-Path $PublicRoot 'generated\native-map-keyed' }

 $playerCombatSoundNames = @('70.wav','71.wav','72.wav','73.wav','80.wav','81.wav','82.wav','83.wav','138.wav','139.wav','144.wav','145.wav','tiger_struck_1.wav','tiger_struck_2.wav','wolf_struck1.wav')
 $monsterCombatSoundNames = @('005-1.wav','005-2.wav','005-3.wav','60.wav','61.wav','62.wav','63.wav','64.wav','65.wav')
 $requiredSources = @((Join-Path $PublicRoot 'bevy-entity-atlases\manifest.json'), (Join-Path $PublicRoot 'generated\map-atlas\manifest.json'), (Join-Path $nativeKeyedMapRoot 'manifest.json'), (Join-Path $PublicRoot 'original-effects\effects.generated.json'), (Join-Path $MapPackRoot '0.map.gz'), (Join-Path $PublicRoot 'original-ui\frame-sets.generated.json'), (Join-Path $PublicRoot 'original-ui\ChrSel\0.png'), (Join-Path $PublicRoot 'original-ui\MMap\101.png'), (Join-Path $PublicRoot 'original-ui\Prguse\20.png'), (Join-Path $PublicRoot 'original-ui\Prguse\1084.png'), (Join-Path $PublicRoot 'original-ui\UI_32bit\472.png'), (Join-Path $PublicRoot 'original-ui\Title\30.png'), (Join-Path $PublicRoot 'original-ui\Title\411.png'), (Join-Path $PublicRoot 'original-ui\AArmour\00\0.png'), (Join-Path $PublicRoot 'original-ui\Monster\000\0.png'), (Join-Path $PublicRoot 'original-ui\NPC\00\0.png'), (Join-Path $PublicRoot 'original-ui\Sound\Login2.wav'), (Join-Path $PublicRoot 'original-ui\Sound\Select2.wav'), (Join-Path $PublicRoot 'original-ui\Sound\103.wav'), (Join-Path $PublicRoot 'original-ui\Sound\M8-1.wav'), (Join-Path $PublicRoot 'original-ui\Sound\M31-0.wav'), (Join-Path $PublicRoot 'original-ui\Sound\M31-1.wav'), (Join-Path $PublicRoot 'original-ui\Sound\M31-2.wav'), (Join-Path $PublicRoot 'original-ui\Sound\M34-0.wav'), (Join-Path $PublicRoot 'original-ui\Sound\M34-1.wav'), (Join-Path $PublicRoot 'original-ui\Sound\M34-2.wav'), (Join-Path $PublicRoot 'original-ui\Sound\M39-0.wav'), (Join-Path $PublicRoot 'original-ui\Sound\M39-1.wav'), (Join-Path $PublicRoot 'original-ui\Sound\M40-0.wav'), (Join-Path $PublicRoot 'original-ui\Sound\M61-0.wav'), (Join-Path $PublicRoot 'original-ui\Sound\M61-1.wav'), (Join-Path $PublicRoot 'original-ui\Sound\M64-0.wav'), (Join-Path $PublicRoot 'original-ui\Sound\M64-1.wav'), (Join-Path $PublicRoot 'original-ui\Sound\M64-2.wav'), (Join-Path $PublicRoot 'original-ui\Sound\M79-1.wav'))
 foreach ($soundName in $playerCombatSoundNames) { $requiredSources += (Join-Path $PublicRoot ('original-ui\Sound\' + $soundName)) }
 foreach ($soundName in $monsterCombatSoundNames) { $requiredSources += (Join-Path $PublicRoot ('original-ui\Sound\' + $soundName)) }
 foreach ($index in @(197,205,207,360,431,1340,1350) + @(450..468)) { $requiredSources += (Join-Path $PublicRoot "original-ui\Prguse2\$index.png") }
 foreach ($index in @(0..41)) { $requiredSources += (Join-Path $PublicRoot "original-ui\Help\$index.png") }
 foreach ($index in @(57,168,169,196,197,198,411,567,633,634,635,636,637,638,737,738,739,820,821,822,823,824,827,848,850,851,853)) { $requiredSources += (Join-Path $PublicRoot "original-ui\Title\$index.png") }
 foreach ($index in @(1,920,1903,1904,1905,1970,1973,1976,1979,1982,1985,1988,1991,1992,1993,1994,1995,1996,2000,2090,2096,2097,2098)) { $requiredSources += (Join-Path $PublicRoot "original-ui\Prguse\$index.png") }
 foreach ($index in @(0..9) + @(170..179)) { $requiredSources += (Join-Path $PublicRoot "original-effects\Magic\$index.png") }
 foreach ($index in @(200..209) + @(370..379)) { $requiredSources += (Join-Path $PublicRoot "original-effects\Magic\$index.png") }
 foreach ($direction in 0..15) { foreach ($frame in 0..5) { $index = 10 + ($direction * 10) + $frame; $requiredSources += (Join-Path $PublicRoot "original-effects\Magic\$index.png") } }
 foreach ($index in @(400..409) + @(570..579)) { $requiredSources += (Join-Path $PublicRoot "original-effects\Magic\$index.png") }
 foreach ($direction in 0..15) { foreach ($frame in 0..5) { $index = 410 + ($direction * 10) + $frame; $requiredSources += (Join-Path $PublicRoot "original-effects\Magic\$index.png") } }
 foreach ($index in 1360..1369) { $requiredSources += (Join-Path $PublicRoot "original-effects\Magic\$index.png") }
 foreach ($direction in 0..15) { foreach ($frame in 0..2) { $index = 1160 + ($direction * 10) + $frame; $requiredSources += (Join-Path $PublicRoot "original-effects\Magic\$index.png") } }
 foreach ($index in 1620..1635) { $requiredSources += (Join-Path $PublicRoot "original-effects\Magic\$index.png") }
 foreach ($direction in 0..7) { foreach ($frame in 0..5) { $index = 3480 + ($direction * 10) + $frame; $requiredSources += (Join-Path $PublicRoot "original-effects\Magic\$index.png") } }
 foreach ($index in 1220..1239) { $requiredSources += (Join-Path $PublicRoot "original-effects\Magic2\$index.png") }
foreach ($required in $requiredSources) { if (-not (Test-Path -LiteralPath $required -PathType Leaf)) { throw "required asset missing: $required" } }
$candidateSoundIdentities = @(
    [pscustomobject]@{ name='005-1.wav'; size=[int64]90118; sha256='966E4163FC0000CF769B63C0F3379F1E9863645F43C1CCADEEE8066B73E6AE9A' },
    [pscustomobject]@{ name='005-2.wav'; size=[int64]36726; sha256='23DCD6D10BFBA3935FB3FEC8E7551B8AC9EE832CD40B1921B0399C829893A376' },
    [pscustomobject]@{ name='005-3.wav'; size=[int64]198168; sha256='CF1FAF157B49D1E014E9B3A56367234FDCFD54088F93F04BB653CB27A67B9FF7' },
    [pscustomobject]@{ name='60.wav'; size=[int64]71200; sha256='2EFA4E2AE9101364F96D404A2F487C0010EFD026648AC30D0D6C9FC464437C94' },
    [pscustomobject]@{ name='61.wav'; size=[int64]47674; sha256='C48C3836EDDDAC4688310F6906ED08388DD8BCCE628C8D439A1E69101ACC3942' },
    [pscustomobject]@{ name='62.wav'; size=[int64]58252; sha256='23F2A8312C0979E338B8F1B482E606247A653DF8C23277D0EAFD71CB4E58630B' },
    [pscustomobject]@{ name='63.wav'; size=[int64]67092; sha256='F729C4FC85E7F09318463FEC9C689BFD652B0B3FD12F2E4F6A991B9272A10F36' },
    [pscustomobject]@{ name='64.wav'; size=[int64]70818; sha256='BC8EDB1BB3367B888006FD3AC9C909208E2B623E3B4814CF02DB37F151ADF400' },
    [pscustomobject]@{ name='65.wav'; size=[int64]68848; sha256='19A905F01B171898044C5374997A5390160E80C113EA0EB110270457D1262EAA' },
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
foreach ($identity in $candidateSoundIdentities) { $soundPath = Join-Path $PublicRoot ('original-ui\Sound\' + $identity.name); if (-not (Test-FileIdentity -Path $soundPath -ExpectedSize $identity.size -ExpectedSha256 $identity.sha256)) { throw "$($identity.name) source identity mismatch" } }
$requiredSourceTrees = @((Join-Path $PublicRoot 'bevy-entity-atlases'), (Join-Path $PublicRoot 'generated\map-atlas'), $nativeKeyedMapRoot, $MapPackRoot, (Join-Path $PublicRoot 'original-effects'), (Join-Path $PublicRoot 'original-ui\ChrSel'), (Join-Path $PublicRoot 'original-ui\Help'), (Join-Path $PublicRoot 'original-ui\MMap'), (Join-Path $PublicRoot 'original-ui\Prguse'), (Join-Path $PublicRoot 'original-ui\Prguse2'), (Join-Path $PublicRoot 'original-ui\UI_32bit'), (Join-Path $PublicRoot 'original-ui\Title'), (Join-Path $PublicRoot 'original-ui\AArmour\00'), (Join-Path $PublicRoot 'original-ui\Monster\000'), (Join-Path $PublicRoot 'original-ui\NPC\00'))
foreach ($sourceTree in $requiredSourceTrees) { if (-not (Test-Path -LiteralPath $sourceTree -PathType Container)) { throw "required source tree missing: $sourceTree" }; Assert-NoReparseTree -Path $sourceTree; Assert-NoAlternateDataStreams -Path $sourceTree }
foreach ($soundName in @('Login2.wav','Select2.wav','103.wav','M8-1.wav','M31-0.wav','M31-1.wav','M31-2.wav','M34-0.wav','M34-1.wav','M34-2.wav','M39-0.wav','M39-1.wav','M40-0.wav','M61-0.wav','M61-1.wav','M64-0.wav','M64-1.wav','M64-2.wav','M79-1.wav') + $playerCombatSoundNames + $monsterCombatSoundNames) { $sound = Join-Path $PublicRoot ('original-ui\Sound\' + $soundName); Assert-NoReparseTree -Path $sound; Assert-NoAlternateDataStreams -Path $sound }
 foreach ($manifestPath in $requiredSources | Where-Object { $_ -like '*.json' }) { try { $json = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json } catch { throw "invalid required JSON manifest: $manifestPath" }; if ($null -eq $json) { throw "empty required JSON manifest: $manifestPath" } }
 $entityAtlasClosure = Assert-EntityAtlasClosure -ManifestPath (Join-Path $PublicRoot 'bevy-entity-atlases\manifest.json') -AssetRoot $PublicRoot
 Write-Host "entityAtlasClosure=atlases:$($entityAtlasClosure.atlasCount),pages:$($entityAtlasClosure.pageCount)"

$output = if ([string]::IsNullOrWhiteSpace($OutputRoot)) { Join-Path $DistRoot ('mir2-windows-candidate\' + $CandidateVersion) } else { $OutputRoot }
$output = Assert-SafeDistTarget -Path $output -DistRoot $DistRoot
if ($DryRun) {
    Write-Host 'attested nonvisual preflight passed'
    Write-Host "releaseExeSha256=$($attested.exeSha256)"
    Write-Host "attestationSha256=$($attested.attestationSha256)"
    Write-Host "sourceRevision=$($worktree.revision) dirty=$($worktree.dirty) statusSha256=$($worktree.statusSha256)"
    Write-Host "outputPlan=$output"
    Write-Host 'no build, package, dist write, GUI, or repository target creation performed'
}
} finally {
    if ($null -ne $dryRunNativeKeyedRoot -and (Test-Path -LiteralPath $dryRunNativeKeyedRoot)) {
        Remove-SafeTemporaryTree -Path $dryRunNativeKeyedRoot -RequiredPrefix 'native-keyed-map-dryrun-'
    }
}
if ($DryRun) { exit 0 }

Assert-NoReparseAncestors -Path $RepoRoot
if (-not (Test-Path -LiteralPath $DistRoot)) { New-Item -ItemType Directory -Path $DistRoot | Out-Null }
Assert-NoReparseTree -Path $DistRoot
$outputParent = Split-Path -Parent $output
Ensure-SafeDirectory -Path $outputParent -BoundaryRoot $DistRoot | Out-Null
$staging = Join-Path $outputParent ('.' + (Split-Path -Leaf $output) + '.staging-' + [guid]::NewGuid().ToString('N'))
$staging = Assert-SafeDistTarget -Path $staging -DistRoot $DistRoot
New-Item -ItemType Directory -Path $staging | Out-Null
Assert-NoReparseTree -Path $staging
$completed = $false
try {
    $assetDest = Join-Path $staging 'mir2-assets'; New-Item -ItemType Directory -Path $assetDest | Out-Null
     $treeCopies = @(
        @((Join-Path $PublicRoot 'bevy-entity-atlases'), (Join-Path $assetDest 'bevy-entity-atlases')),
        @((Join-Path $PublicRoot 'generated\map-atlas'), (Join-Path $assetDest 'generated\map-atlas')),
        @((Join-Path $PublicRoot 'generated\native-map-keyed'), (Join-Path $assetDest 'generated\native-map-keyed')),
        @($MapPackRoot, (Join-Path $assetDest 'crystal-map-pack')),
         @((Join-Path $PublicRoot 'original-effects'), (Join-Path $assetDest 'original-effects')),
        @((Join-Path $PublicRoot 'original-ui\frame-sets.generated.json'), (Join-Path $assetDest 'original-ui\frame-sets.generated.json')),
         @((Join-Path $PublicRoot 'original-ui\ChrSel'), (Join-Path $assetDest 'original-ui\ChrSel')),
        @((Join-Path $PublicRoot 'original-ui\Help'), (Join-Path $assetDest 'original-ui\Help')),
        @((Join-Path $PublicRoot 'original-ui\MMap'), (Join-Path $assetDest 'original-ui\MMap')),
        @((Join-Path $PublicRoot 'original-ui\Prguse'), (Join-Path $assetDest 'original-ui\Prguse')),
        @((Join-Path $PublicRoot 'original-ui\Prguse2'), (Join-Path $assetDest 'original-ui\Prguse2')),
        @((Join-Path $PublicRoot 'original-ui\UI_32bit'), (Join-Path $assetDest 'original-ui\UI_32bit')),
        @((Join-Path $PublicRoot 'original-ui\Title'), (Join-Path $assetDest 'original-ui\Title')),
        @((Join-Path $PublicRoot 'original-ui\AArmour\00'), (Join-Path $assetDest 'original-ui\AArmour\00')),
        @((Join-Path $PublicRoot 'original-ui\Monster\000'), (Join-Path $assetDest 'original-ui\Monster\000')),
        @((Join-Path $PublicRoot 'original-ui\NPC\00'), (Join-Path $assetDest 'original-ui\NPC\00'))
    )
     foreach ($copy in $treeCopies) {
         if ((Get-Item -LiteralPath $copy[0]).PSIsContainer) { Copy-Tree -Source $copy[0] -Destination $copy[1] } else { Copy-CandidateAssetFile -Source $copy[0] -Destination $copy[1] }
     }
    $soundSource = Join-Path $PublicRoot 'original-ui\Sound'; Assert-NoReparseTree -Path $soundSource
    $soundDest = Join-Path $assetDest 'original-ui\Sound'; New-Item -ItemType Directory -Path $soundDest | Out-Null
    foreach ($sound in @('Login2.wav', 'Select2.wav', '103.wav', 'M8-1.wav', 'M31-0.wav', 'M31-1.wav', 'M31-2.wav', 'M34-0.wav', 'M34-1.wav', 'M34-2.wav', 'M39-0.wav', 'M39-1.wav', 'M40-0.wav', 'M61-0.wav', 'M61-1.wav', 'M64-0.wav', 'M64-1.wav', 'M64-2.wav', 'M79-1.wav') + $playerCombatSoundNames + $monsterCombatSoundNames) { Copy-FileDefaultDataOnly -Source (Join-Path $PublicRoot ('original-ui\Sound\' + $sound)) -Destination (Join-Path $soundDest $sound) }
    Assert-NoReparseTree -Path $soundSource
    Assert-NoReparseTree -Path $soundDest
    Assert-NoReparseTree -Path $releaseExeFull
    Assert-NoReparseTree -Path $attestationFull
    if ((Get-FileHash -LiteralPath $releaseExeFull -Algorithm SHA256).Hash.ToUpperInvariant() -ne $attested.exeSha256) { throw 'Release EXE changed before staging copy' }
    if ((Get-FileHash -LiteralPath $attestationFull -Algorithm SHA256).Hash.ToUpperInvariant() -ne $attested.attestationSha256) { throw 'build attestation changed before staging copy' }
    Copy-FileDefaultDataOnly -Source $releaseExeFull -Destination (Join-Path $staging $ExeName)
    Copy-FileDefaultDataOnly -Source $attestationFull -Destination (Join-Path $staging 'BUILD-ATTESTATION.json')
    Assert-NoReparseTree -Path $releaseExeFull; Assert-NoReparseTree -Path $attestationFull; Assert-NoReparseTree -Path $staging
    if ((Get-FileHash -LiteralPath $releaseExeFull -Algorithm SHA256).Hash.ToUpperInvariant() -ne $attested.exeSha256 -or (Get-FileHash -LiteralPath (Join-Path $staging $ExeName) -Algorithm SHA256).Hash.ToUpperInvariant() -ne $attested.exeSha256) { throw 'Release EXE changed during staging copy' }
    if ((Get-FileHash -LiteralPath $attestationFull -Algorithm SHA256).Hash.ToUpperInvariant() -ne $attested.attestationSha256 -or (Get-FileHash -LiteralPath (Join-Path $staging 'BUILD-ATTESTATION.json') -Algorithm SHA256).Hash.ToUpperInvariant() -ne $attested.attestationSha256) { throw 'build attestation changed during staging copy' }
    Assert-NoAlternateDataStreams -Path $staging

    $toml = "# Candidate client configuration; credentials are forbidden.`n[server]`ngateway_ws_url = `"wss://candidate-gateway.example/ws`"`n[display]`nwidth = 1024`nheight = 768`n"
    Write-Utf8NoBom -Path (Join-Path $staging 'mir2-client.toml') -Text $toml
    Write-Utf8NoBom -Path (Join-Path $staging 'README-START.txt') -Text "Mir2 Windows Native Candidate — client only`nThis package is staged, not Accepted. It includes no server or credentials.`nRemote gateways require wss://. Visual, human, WebSocket black-box, DPI and soak gates remain open.`n"
    Write-Utf8NoBom -Path (Join-Path $staging 'CONTROLS.txt') -Text "Windows Native Candidate controls`nLogin: Tab/Shift+Tab, Enter, Escape, mouse fields/buttons.`nGame: WASD/arrows walk; Shift run; E turns right while gameplay input is enabled; Q opens/closes Quest Log; T talk; F attack; R pick up; V revive; 1-6 belt; F1-F8 skills; I bag; C character/equipment; Escape close/menu; Enter chat; U use; G equip; L logout from menu.`nF12 capture requires an explicit capture directory. Mouse click-to-move is not claimed.`n"
    Write-Utf8NoBom -Path (Join-Path $staging 'KNOWN-ISSUES.md') -Text "# Known acceptance gaps`nThis client-only Candidate is not Crystal 1:1 Accepted.`nVisual parity, human play feel, authenticated WebSocket black-box, 125%/150% DPI, lighting and 30-minute soak are not certified by this package.`n"
    New-Item -ItemType Directory -Path (Join-Path $staging 'logs') | Out-Null

    Assert-NoReparseTree -Path $staging
    Assert-NoAlternateDataStreams -Path $staging
    Assert-PackageAllowlist -Root $staging -ExeName $ExeName
    $manifestPath = Join-Path $staging 'PACKAGE-MANIFEST.json'
    $manifest = Write-PackageManifest -Root $staging -OutputPath $manifestPath
    $manifestSha256 = (Get-FileHash -LiteralPath $manifestPath -Algorithm SHA256).Hash.ToUpperInvariant()
    $version = [ordered]@{ schema = 'mir2.windows.candidate-version.v4'; candidate = $CandidateVersion; gitRevision = $worktree.revision; worktreeDirty = [bool]$worktree.dirty; worktreeStatusScope = $worktree.statusScope; worktreeStatusSha256 = $worktree.statusSha256; buildAttestationSha256 = $attested.attestationSha256; buildCompletedUtc = [string]$attestation.buildCompletedUtc; exeName = $ExeName; exeSha256 = $attested.exeSha256; exeSizeBytes = [int64]$exe.Length; packageManifestSchema = $manifest.schema; packageManifestSha256 = $manifestSha256; packageManifestAggregateSha256 = $manifest.aggregateSha256; packageManifestFileCount = [int]$manifest.fileCount; packageFileCount = [int]$manifest.fileCount + 4; releaseStatementSchema = 'mir2.windows.release-statement.v1'; signatureFormat = 'CMS/PKCS7-detached'; staged = $true; builtByPackagingScript = $false; clientOnly = $true; accepted = $false }
    $versionPath = Join-Path $staging 'VERSION.json'
    Write-Utf8NoBom -Path $versionPath -Text ($version | ConvertTo-Json -Depth 8)
    $versionSha256 = (Get-FileHash -LiteralPath $versionPath -Algorithm SHA256).Hash.ToUpperInvariant()
    $statementText = New-ReleaseStatementText -Candidate $CandidateVersion -ExeSha256 $attested.exeSha256 -ManifestSha256 $manifestSha256 -ManifestAggregateSha256 $manifest.aggregateSha256 -VersionSha256 $versionSha256 -AttestationSha256 $attested.attestationSha256 -GitRevision $worktree.revision -WorktreeDirty ([bool]$worktree.dirty) -DirtyDigest $worktree.statusSha256
    $statementBytes = [Text.Encoding]::UTF8.GetBytes($statementText)
    [IO.File]::WriteAllBytes((Join-Path $staging 'RELEASE-STATEMENT.json'), $statementBytes)
    [IO.File]::WriteAllBytes((Join-Path $staging 'RELEASE-STATEMENT.p7s'), (New-DetachedCmsSignature -Content $statementBytes -Certificate $signingCertificate))
    Assert-NoReparseTree -Path $staging
    Assert-NoAlternateDataStreams -Path $staging
    Assert-PackageAllowlist -Root $staging -ExeName $ExeName

    $verifyArgs = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', (Join-Path $ScriptDir 'verify-windows-candidate.ps1'), '-PackageRoot', $staging, '-TrustedSignerThumbprint', (Normalize-Thumbprint -Thumbprint $SignerThumbprint))
    if ($AllowDirtyWorktree) { $verifyArgs += '-AllowDirtyWorktree' }
    & powershell @verifyArgs
    if ($LASTEXITCODE -ne 0) { throw "nonvisual staged-package verification failed: exit $LASTEXITCODE" }

    Assert-NoReparseTree -Path $staging
    Assert-NoReparseAncestors -Path $output
    if (Test-Path -LiteralPath $output) { Remove-SafeTree -Path $output -DistRoot $DistRoot }
    Assert-NoReparseAncestors -Path $outputParent
    Assert-NoReparseTree -Path $staging
    Move-Item -LiteralPath $staging -Destination $output
    Assert-NoReparseTree -Path $output
    try { Assert-NoAlternateDataStreams -Path $output } catch { if (Test-Path -LiteralPath $output) { Remove-SafeTree -Path $output -DistRoot $DistRoot }; throw }
    $completed = $true

    New-Item -ItemType Directory -Force -Path $EvidenceDir | Out-Null
    $summary = [ordered]@{ schema = 'mir2.windows.package-preflight.v4'; candidate = $CandidateVersion; packageRoot = $output; sourceRevision = $worktree.revision; worktreeDirty = [bool]$worktree.dirty; buildAttestationSha256 = $attested.attestationSha256; packageManifestSha256 = $manifestSha256; packageManifestAggregateSha256 = $manifest.aggregateSha256; packageManifestFileCount = [int]$manifest.fileCount; signerThumbprint = (Normalize-Thumbprint -Thumbprint $SignerThumbprint); staged = $true; builtByPackagingScript = $false; visual = $false; accepted = $false }
    Write-Utf8NoBom -Path (Join-Path $EvidenceDir ($CandidateVersion + '-package-summary.json')) -Text ($summary | ConvertTo-Json -Depth 6)
    Write-Host "staged attested Candidate at $output"
} finally {
    if (-not $completed -and (Test-Path -LiteralPath $staging)) { Remove-SafeTree -Path $staging -DistRoot $DistRoot }
}
