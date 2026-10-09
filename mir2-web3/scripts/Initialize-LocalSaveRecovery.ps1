[CmdletBinding()]
param([switch]$SelfTest, [switch]$SelfTestAbandonMutex, [string]$InitializationRoot = "")

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Assert-Mir2LocalProjectRoot {
    param([string]$ProjectRoot)
    $FullPath = [IO.Path]::GetFullPath($ProjectRoot)
    if ($FullPath.StartsWith('\\')) { throw "UNC/network project roots are not supported by the local durability contract." }
    $PathRoot = [IO.Path]::GetPathRoot($FullPath)
    if ($PathRoot -and (New-Object IO.DriveInfo($PathRoot)).DriveType -eq [IO.DriveType]::Network) {
        throw "Network-drive project roots are not supported by the local durability contract."
    }
}

function Assert-Mir2SafeLocalPath {
    param([string]$ProjectRoot, [string]$CandidatePath)
    $Root = [IO.Path]::GetFullPath($ProjectRoot).TrimEnd('\', '/')
    $Candidate = [IO.Path]::GetFullPath($CandidatePath)
    if ($Candidate -ne $Root -and -not $Candidate.StartsWith($Root + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing local save-recovery path outside project root: $Candidate"
    }
    $Current = $Root
    if (-not [IO.Directory]::Exists($Current)) { throw "Project root does not exist: $Root" }
    $Relative = $Candidate.Substring($Root.Length).TrimStart('\', '/')
    foreach ($Part in ($Relative -split '[\\/]' | Where-Object { $_ })) {
        if (([IO.Directory]::Exists($Current) -or [IO.File]::Exists($Current)) -and (([IO.File]::GetAttributes($Current) -band [IO.FileAttributes]::ReparsePoint) -ne 0)) {
            throw "Refusing symlink/reparse point in local save-recovery path: $Current"
        }
        $Current = Join-Path $Current $Part
    }
    if (([IO.Directory]::Exists($Current) -or [IO.File]::Exists($Current)) -and (([IO.File]::GetAttributes($Current) -band [IO.FileAttributes]::ReparsePoint) -ne 0)) {
        throw "Refusing symlink/reparse point in local save-recovery path: $Current"
    }
}

function Assert-Mir2CurrentUserOnlyAcl {
    param([string]$LiteralPath)
    $CurrentSid = [Security.Principal.WindowsIdentity]::GetCurrent().User.Value
    $Acl = Get-Acl -LiteralPath $LiteralPath
    if (-not $Acl.AreAccessRulesProtected) { throw "ACL inheritance remains enabled: $LiteralPath" }
    $AllowRules = @($Acl.GetAccessRules($true, $true, [Security.Principal.SecurityIdentifier]) | Where-Object { $_.AccessControlType -eq [Security.AccessControl.AccessControlType]::Allow })
    if ($AllowRules.Count -lt 1 -or @($AllowRules | Where-Object { $_.IdentityReference.Value -ne $CurrentSid }).Count -ne 0) {
        throw "ACL grants Allow access beyond the current user: $LiteralPath"
    }
}

function Set-Mir2CurrentUserOnlyAcl {
    param([Parameter(Mandatory = $true)][string]$LiteralPath, [Parameter(Mandatory = $true)][bool]$IsDirectory)
    if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) { throw "Use scripts/Initialize-LocalSaveRecovery.sh on Unix." }
    # Reapplying an identical protected ACL is unnecessary and can fail on an
    # otherwise healthy local volume under transient metadata pressure. Keep
    # the operation fail-closed: only skip the write after the full ownership /
    # inheritance / allow-list assertion already succeeds.
    try {
        Assert-Mir2CurrentUserOnlyAcl -LiteralPath $LiteralPath
        return
    }
    catch { }
    try {
        $Identity = [System.Security.Principal.WindowsIdentity]::GetCurrent()
        $Security = if ($IsDirectory) { New-Object System.Security.AccessControl.DirectorySecurity } else { New-Object System.Security.AccessControl.FileSecurity }
        $Security.SetOwner($Identity.User)
        $Security.SetAccessRuleProtection($true, $false)
        $Inheritance = if ($IsDirectory) { [System.Security.AccessControl.InheritanceFlags]::ContainerInherit -bor [System.Security.AccessControl.InheritanceFlags]::ObjectInherit } else { [System.Security.AccessControl.InheritanceFlags]::None }
        $Rule = New-Object System.Security.AccessControl.FileSystemAccessRule($Identity.User, [System.Security.AccessControl.FileSystemRights]::FullControl, $Inheritance, [System.Security.AccessControl.PropagationFlags]::None, [System.Security.AccessControl.AccessControlType]::Allow)
        $Security.AddAccessRule($Rule)
        # PowerShell 7 / modern .NET no longer exposes the legacy static
        # Directory.SetAccessControl/File.SetAccessControl methods. Set-Acl
        # accepts the same explicit FileSystemSecurity object on both paths.
        Set-Acl -LiteralPath $LiteralPath -AclObject $Security
        Assert-Mir2CurrentUserOnlyAcl -LiteralPath $LiteralPath
    }
    catch { throw "Failed to enforce current-user-only ACL on $LiteralPath" }
}

function Read-Mir2RecoveryMacKey {
    param([Parameter(Mandatory = $true)][string]$LiteralPath)
    $Key = [System.IO.File]::ReadAllText($LiteralPath).Trim()
    if ($Key -cnotmatch '^[0-9a-f]{64}$') { throw "Local save-recovery MAC key is invalid. Expected exactly 64 lowercase hexadecimal characters at: $LiteralPath" }
    return $Key
}

function Initialize-Mir2LocalSaveRecovery {
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)][string]$ProjectRoot)

    Assert-Mir2LocalProjectRoot -ProjectRoot $ProjectRoot
    $ResolvedProjectRoot = [System.IO.Path]::GetFullPath($ProjectRoot)
    $HashAlgorithm = [Security.Cryptography.SHA256]::Create()
    try { $RootHash = -join ($HashAlgorithm.ComputeHash([Text.Encoding]::UTF8.GetBytes($ResolvedProjectRoot)) | ForEach-Object { $_.ToString('x2') }) } finally { $HashAlgorithm.Dispose() }
    $Mutex = New-Object Threading.Mutex($false, "Local\Mir2SaveRecovery-$RootHash")
    $MutexAcquired = $false
    try {
    try { $MutexAcquired = $Mutex.WaitOne([TimeSpan]::FromSeconds(30)) }
    catch [Threading.AbandonedMutexException] { $MutexAcquired = $true }
    if (-not $MutexAcquired) { throw "Timed out acquiring local save-recovery initialization lock." }
    $SecretDirectory = Join-Path $ResolvedProjectRoot ".mir2-data\local-secrets"
    $KeyPath = Join-Path $SecretDirectory "save-recovery-mac-key.hex"
    $RecoveryDirectory = Join-Path $ResolvedProjectRoot ".mir2-data\save-recovery\v1\developer-gateway"
    foreach ($Path in @($SecretDirectory, $KeyPath, $RecoveryDirectory)) { Assert-Mir2SafeLocalPath -ProjectRoot $ResolvedProjectRoot -CandidatePath $Path }
    [System.IO.Directory]::CreateDirectory($SecretDirectory) | Out-Null
    Assert-Mir2SafeLocalPath -ProjectRoot $ResolvedProjectRoot -CandidatePath $SecretDirectory
    $null = Set-Mir2CurrentUserOnlyAcl -LiteralPath $SecretDirectory -IsDirectory $true

    if (-not [System.IO.File]::Exists($KeyPath)) {
        $RandomBytes = New-Object byte[] 32
        $Generator = [System.Security.Cryptography.RandomNumberGenerator]::Create()
        try { $Generator.GetBytes($RandomBytes) } finally { $Generator.Dispose() }
        $GeneratedKey = -join ($RandomBytes | ForEach-Object { $_.ToString("x2") })
        [Array]::Clear($RandomBytes, 0, $RandomBytes.Length)
        $TemporaryPath = Join-Path $SecretDirectory (".save-recovery-mac-key.{0}.tmp" -f [Guid]::NewGuid().ToString("N"))
        $Payload = (New-Object System.Text.UTF8Encoding($false)).GetBytes($GeneratedKey + [Environment]::NewLine)
        $Stream = $null
        try {
            $Stream = New-Object System.IO.FileStream($TemporaryPath, [System.IO.FileMode]::CreateNew, [System.IO.FileAccess]::Write, [System.IO.FileShare]::None)
            $Stream.Write($Payload, 0, $Payload.Length)
            $Stream.Flush($true)
            $Stream.Dispose()
            $Stream = $null
            $null = Set-Mir2CurrentUserOnlyAcl -LiteralPath $TemporaryPath -IsDirectory $false
            try { [System.IO.File]::Move($TemporaryPath, $KeyPath) }
            catch [System.IO.IOException] { if (-not [System.IO.File]::Exists($KeyPath)) { throw } }
        }
        finally {
            if ($null -ne $Stream) { $Stream.Dispose() }
            [Array]::Clear($Payload, 0, $Payload.Length)
            $GeneratedKey = $null
            if ([System.IO.File]::Exists($TemporaryPath)) { [System.IO.File]::Delete($TemporaryPath) }
        }
    }

    $null = Set-Mir2CurrentUserOnlyAcl -LiteralPath $KeyPath -IsDirectory $false
    [System.IO.Directory]::CreateDirectory($RecoveryDirectory) | Out-Null
    Assert-Mir2SafeLocalPath -ProjectRoot $ResolvedProjectRoot -CandidatePath $RecoveryDirectory
    $null = Set-Mir2CurrentUserOnlyAcl -LiteralPath $RecoveryDirectory -IsDirectory $true
    $MacKey = Read-Mir2RecoveryMacKey -LiteralPath $KeyPath
    return [PSCustomObject]@{ MacKey = $MacKey; KeyPath = $KeyPath; RecoveryDirectory = $RecoveryDirectory }
    }
    finally {
        if ($MutexAcquired) { $Mutex.ReleaseMutex() }
        $Mutex.Dispose()
    }
}

function Wait-Mir2SelfTestChild {
    param([Diagnostics.Process]$Process, [string]$Label)
    if (-not $Process.WaitForExit(15000)) {
        if (-not $Process.HasExited) { Stop-Process -Id $Process.Id -Force; $null = $Process.WaitForExit(5000) }
        throw "$Label exceeded the 15 second selftest limit and was terminated."
    }
    $Process.Refresh()
    if ($null -ne $Process.ExitCode -and $Process.ExitCode -ne 0) { throw "$Label exited with code $($Process.ExitCode)." }
}

function Invoke-Mir2LocalSaveRecoverySelfTest {
    $TestRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("mir2-local-save-recovery-selftest-{0}" -f [Guid]::NewGuid().ToString("N"))
    $ConcurrentRoot = "$TestRoot-concurrent"
    $AbandonedRoot = "$TestRoot-abandoned"
    $Processes = @()
    try {
        [System.IO.Directory]::CreateDirectory($TestRoot) | Out-Null
        $UncRejected = $false
        try { Assert-Mir2LocalProjectRoot -ProjectRoot '\\server\share\mir2' } catch { $UncRejected = $true }
        if (-not $UncRejected) { throw "UNC project root was accepted." }
        $First = Initialize-Mir2LocalSaveRecovery -ProjectRoot $TestRoot
        if ($First.MacKey -cnotmatch '^[0-9a-f]{64}$') { throw "Generated key did not match the required format." }
        if (-not [System.IO.Directory]::Exists($First.RecoveryDirectory)) { throw "Recovery directory was not created." }
        $Second = Initialize-Mir2LocalSaveRecovery -ProjectRoot $TestRoot
        if ($First.MacKey -cne $Second.MacKey) { throw "Key changed across restart initialization." }
        if ([System.IO.Directory]::GetFiles((Split-Path -Parent $First.KeyPath), '*.tmp').Count -ne 0) { throw "Atomic publication left a temporary key file behind." }
        if ([Environment]::OSVersion.Platform -eq [PlatformID]::Win32NT) {
            $CurrentSid = [System.Security.Principal.WindowsIdentity]::GetCurrent().User.Value
            $Acl = Get-Acl -LiteralPath $First.KeyPath
            if (-not $Acl.AreAccessRulesProtected) { throw "Key ACL still inherits access rules." }
            $UnexpectedAllow = $Acl.GetAccessRules($true, $true, [System.Security.Principal.SecurityIdentifier]) | Where-Object {
                $_.AccessControlType -eq [System.Security.AccessControl.AccessControlType]::Allow -and $_.IdentityReference.Value -ne $CurrentSid
            }
            if ($UnexpectedAllow) { throw "Key ACL grants access to an identity other than the current user." }
            $RecoveryAcl = Get-Acl -LiteralPath $First.RecoveryDirectory
            if (-not $RecoveryAcl.AreAccessRulesProtected) { throw "Recovery-directory ACL still inherits access rules." }
            $UnexpectedRecoveryAllow = $RecoveryAcl.GetAccessRules($true, $true, [System.Security.Principal.SecurityIdentifier]) | Where-Object {
                $_.AccessControlType -eq [System.Security.AccessControl.AccessControlType]::Allow -and $_.IdentityReference.Value -ne $CurrentSid
            }
            if ($UnexpectedRecoveryAllow) { throw "Recovery-directory ACL grants access to an identity other than the current user." }
        }
        $StableKey = ('ab' * 32)
        [System.IO.File]::WriteAllText($First.KeyPath, $StableKey + [Environment]::NewLine, (New-Object System.Text.UTF8Encoding($false)))
        $Existing = Initialize-Mir2LocalSaveRecovery -ProjectRoot $TestRoot
        if ($Existing.MacKey -cne $StableKey) { throw "Existing valid key was overwritten." }
        [System.IO.File]::WriteAllText($First.KeyPath, 'invalid', (New-Object System.Text.UTF8Encoding($false)))
        $InvalidRejected = $false
        try { $null = Initialize-Mir2LocalSaveRecovery -ProjectRoot $TestRoot } catch { $InvalidRejected = $true }
        if (-not $InvalidRejected) { throw "Invalid existing key did not fail closed." }
        [System.IO.Directory]::CreateDirectory($ConcurrentRoot) | Out-Null
        for ($Index = 0; $Index -lt 2; $Index++) {
            $Out = Join-Path $TestRoot "direct-$Index.out"
            $Err = Join-Path $TestRoot "direct-$Index.err"
            $Arguments = @("-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File", "`"$PSCommandPath`"", "-InitializationRoot", "`"$ConcurrentRoot`"")
            $Processes += Start-Process -FilePath ([Diagnostics.Process]::GetCurrentProcess().MainModule.FileName) -ArgumentList $Arguments -WindowStyle Hidden -RedirectStandardOutput $Out -RedirectStandardError $Err -PassThru
        }
        foreach ($Process in $Processes) {
            Wait-Mir2SelfTestChild -Process $Process -Label "Concurrent initializer"
            if ($null -ne $Process.ExitCode -and $Process.ExitCode -ne 0) {
                $ChildError = Get-Content -LiteralPath (Join-Path $TestRoot "direct-$($Processes.IndexOf($Process)).err") -Raw -ErrorAction SilentlyContinue
                $ChildOutput = Get-Content -LiteralPath (Join-Path $TestRoot "direct-$($Processes.IndexOf($Process)).out") -Raw -ErrorAction SilentlyContinue
                throw "Concurrent direct initialization failed with exit $($Process.ExitCode): $ChildError $ChildOutput"
            }
        }
        $Concurrent = Initialize-Mir2LocalSaveRecovery -ProjectRoot $ConcurrentRoot
        if ($Concurrent.MacKey -cnotmatch '^[0-9a-f]{64}$') { throw "Concurrent direct initialization produced an invalid key." }
        [System.IO.Directory]::CreateDirectory($AbandonedRoot) | Out-Null
        $AbandonOut = Join-Path $TestRoot "abandon.out"
        $AbandonErr = Join-Path $TestRoot "abandon.err"
        $AbandonArguments = @("-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File", "`"$PSCommandPath`"", "-InitializationRoot", "`"$AbandonedRoot`"", "-SelfTestAbandonMutex")
        $AbandonProcess = Start-Process -FilePath ([Diagnostics.Process]::GetCurrentProcess().MainModule.FileName) -ArgumentList $AbandonArguments -WindowStyle Hidden -RedirectStandardOutput $AbandonOut -RedirectStandardError $AbandonErr -PassThru
        $Processes += $AbandonProcess
        Wait-Mir2SelfTestChild -Process $AbandonProcess -Label "Abandoned-mutex owner"
        $AfterAbandon = Initialize-Mir2LocalSaveRecovery -ProjectRoot $AbandonedRoot
        if ($AfterAbandon.MacKey -cnotmatch '^[0-9a-f]{64}$') { throw "Recovery after abandoned mutex failed." }
        $LinkRoot = Join-Path $TestRoot "link-case"
        $Outside = Join-Path $TestRoot "outside"
        [System.IO.Directory]::CreateDirectory($LinkRoot) | Out-Null
        [System.IO.Directory]::CreateDirectory($Outside) | Out-Null
        $Link = Join-Path $LinkRoot ".mir2-data"
        $LinkCreated = $false
        try { $null = New-Item -ItemType SymbolicLink -Path $Link -Target $Outside -ErrorAction Stop; $LinkCreated = $true } catch { }
        if ($LinkCreated) {
            $LinkRejected = $false
            try { $null = Initialize-Mir2LocalSaveRecovery -ProjectRoot $LinkRoot } catch { $LinkRejected = $true }
            if (-not $LinkRejected) { throw "Symlink/reparse path did not fail closed." }
        }
        Write-Output "SAVE-RECOVERY-LAUNCH-LOCAL helper selftest: PASS"
    }
    finally {
        foreach ($Process in $Processes) { if (-not $Process.HasExited) { Stop-Process -Id $Process.Id -Force } }
        $ResolvedTestRoot = [System.IO.Path]::GetFullPath($TestRoot)
        $ResolvedTempRoot = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath())
        $Relative = $ResolvedTestRoot.Substring($ResolvedTempRoot.Length).TrimStart('\', '/')
        if ($Relative -and -not $Relative.Contains('..') -and $Relative.StartsWith('mir2-local-save-recovery-selftest-')) { Remove-Item -LiteralPath $ResolvedTestRoot -Recurse -Force -ErrorAction SilentlyContinue }
        if ([System.IO.Directory]::Exists($ConcurrentRoot)) { Remove-Item -LiteralPath $ConcurrentRoot -Recurse -Force -ErrorAction SilentlyContinue }
        if ([System.IO.Directory]::Exists($AbandonedRoot)) { Remove-Item -LiteralPath $AbandonedRoot -Recurse -Force -ErrorAction SilentlyContinue }
    }
}

$InvokedDirectly = $MyInvocation.InvocationName -ne "."
if ($SelfTestAbandonMutex) {
    if (-not $InitializationRoot) { throw "Abandoned-mutex selftest requires InitializationRoot." }
    $ResolvedRoot = [IO.Path]::GetFullPath($InitializationRoot)
    $Hasher = [Security.Cryptography.SHA256]::Create()
    try { $Hash = -join ($Hasher.ComputeHash([Text.Encoding]::UTF8.GetBytes($ResolvedRoot)) | ForEach-Object { $_.ToString('x2') }) } finally { $Hasher.Dispose() }
    $OwnedMutex = New-Object Threading.Mutex($false, "Local\Mir2SaveRecovery-$Hash")
    if (-not $OwnedMutex.WaitOne([TimeSpan]::FromSeconds(10))) { throw "Could not acquire mutex for abandoned-mutex selftest." }
    [Environment]::Exit(0)
}
elseif ($SelfTest) { Invoke-Mir2LocalSaveRecoverySelfTest }
elseif ($InvokedDirectly) {
    $Root = if ($InitializationRoot) { $InitializationRoot } else { [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot "..")) }
    $Result = Initialize-Mir2LocalSaveRecovery -ProjectRoot $Root
    $Result.MacKey = $null
    Write-Output "Local save-recovery secret is ready."
}
