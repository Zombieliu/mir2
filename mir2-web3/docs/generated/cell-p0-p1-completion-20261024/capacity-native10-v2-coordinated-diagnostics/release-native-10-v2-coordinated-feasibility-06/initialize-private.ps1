param([Parameter(Mandatory=$true)][string]$PrivateRoot)
$ErrorActionPreference='Stop'
$taskFull=[IO.Path]::GetFullPath($PrivateRoot)
$taskExpected=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot 'private'))
$taskRecordings=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot 'recordings'))
if($taskFull -ne $taskExpected -and $taskFull -ne $taskRecordings){throw 'Only this unique private/recordings directory may be initialized'}
if(Test-Path -LiteralPath $taskFull){throw 'Refusing existing private directory'}
[IO.Directory]::CreateDirectory($taskFull)|Out-Null
$taskOwner=[Security.Principal.WindowsIdentity]::GetCurrent().User
$taskSystem=[Security.Principal.SecurityIdentifier]::new('S-1-5-18')
$taskAcl=[Security.AccessControl.DirectorySecurity]::new()
$taskAcl.SetOwner($taskOwner)
$taskAcl.SetAccessRuleProtection($true,$false)
$taskInheritance=[Security.AccessControl.InheritanceFlags]::ContainerInherit -bor [Security.AccessControl.InheritanceFlags]::ObjectInherit
foreach($taskPrincipal in @($taskOwner,$taskSystem)){
 $taskRule=[Security.AccessControl.FileSystemAccessRule]::new($taskPrincipal,[Security.AccessControl.FileSystemRights]::FullControl,$taskInheritance,[Security.AccessControl.PropagationFlags]::None,[Security.AccessControl.AccessControlType]::Allow)
 $taskAcl.AddAccessRule($taskRule)
}
Set-Acl -LiteralPath $taskFull -AclObject $taskAcl
$taskApplied=Get-Acl -LiteralPath $taskFull
$taskAllowed=@($taskOwner.Value,$taskSystem.Value)
$taskRules=@($taskApplied.GetAccessRules($true,$true,[Security.Principal.SecurityIdentifier]))
if(-not $taskApplied.AreAccessRulesProtected -or $taskRules.Count -ne 2 -or @($taskRules|Where-Object {$_.IsInherited -or $_.IdentityReference.Value -notin $taskAllowed -or $_.AccessControlType -ne [Security.AccessControl.AccessControlType]::Allow}).Count){throw 'Private ACL verification failed'}
@{ownerOnlyVerified=$true;allowedPrincipalCount=$taskRules.Count}|ConvertTo-Json -Compress
