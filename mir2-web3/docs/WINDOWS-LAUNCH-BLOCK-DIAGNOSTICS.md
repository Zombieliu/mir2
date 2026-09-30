# Windows launch-block diagnostics

Use this when the installed game fails to start with an application-control error
such as `CreateProcess 4551`. The error alone does not identify Smart App Control
(SAC), a managed App Control policy, or the precise file that was rejected.
Run the diagnostic on the affected computer, against the installed game EXE.

## Read-only collection

In Windows PowerShell 5.1 or newer, replace the two input paths below:

```powershell
powershell.exe -NoProfile -File ".\diagnose-launch-block.ps1" -GameExePath "C:\Program Files\Mir2 Invite\mir2-platform-windows.exe" -OutputPath ".\launch-block-report.json"
```

The script is at `apps/game-client/platform-windows/scripts/diagnose-launch-block.ps1`.
`GameExePath` must be a fully qualified existing EXE path; the example installation
directory is not an autodetection rule. Omit `OutputPath` to display JSON only.
An explicit output creates one new report and refuses to overwrite an existing
file. Nothing is uploaded automatically. Review the report before sharing it.

The script does not run the game, elevate, change registry values or policies,
import certificates, alter antivirus settings, or change PowerShell execution
policy. If the computer refuses to run the script, use the small fallback below;
do not disable its security policy to collect diagnostics.

It collects only:

- Selected file name, size, SHA-256, file/product version, Authenticode status and
  public signer/timestamp-certificate metadata. No certificate private key.
- Windows build, update revision, display version and edition; no registered
  owner, machine name, license identifier or account listing.
- The SAC `VerifiedAndReputablePolicyState` registry value. Microsoft documents
  `0 = Off`, `1 = Enforcement`, `2 = Evaluation`; this value does not enumerate
  other application-control policies. A missing/unreadable value stays unknown.
  [Microsoft SAC testing documentation](https://learn.microsoft.com/en-us/windows/apps/develop/smart-app-control/test-your-app-with-smart-app-control).
- The last 48 hours of CodeIntegrity events `3077`, `3033`, `3089`, filtered to
  the selected file. Exported fields are event time/ID, matching basis, selected
  file marker, policy identifiers/name, hashes and numeric signing/status codes.
  No event message, full XML, process path, user SID or unrelated program log is
  exported. [Microsoft event-field and correlation documentation](https://learn.microsoft.com/en-us/windows/security/application-security/application-control/app-control-for-business/operations/appcontrol-debugging-and-troubleshooting).

## Matching and interpretation

Full normalized file paths or exact `SHA256FlatHash` identify a matching event;
a matching filename alone or the parent `ProcessName` does not. Windows kernel
device paths without a flat hash are accepted only when their complete path tail
matches and a read-only hash of that device file equals the selected file hash.
There are at most 16 such reads. `SHA256Hash` is the Authenticode hash and is not
treated as the flat-file SHA-256.

Signature event `3089` may instead be associated with a matched `3077`/`3033`
event by its nonzero ActivityID. An explicitly conflicting filename is excluded.
The report exposes the matching basis; a historical exact-path event might refer
to bytes that were since replaced. Compare available event hashes and timestamps
with the current file before attributing it to this release.

Reads are limited to the newest 4,096 candidate events and the report contains at
most 128 matching events. Limit flags, parse failures, device-file read failures,
and other read failures remain visible. `state: unavailable` is different from a
successful read with no matching events. Neither an empty result nor SAC `Off`
proves that the application is permitted by every policy.

`NotSigned` describes the EXE's Authenticode status. A separate internal CMS
package signature does not Authenticode-sign that EXE. Conversely, a valid EXE
signature does not guarantee acceptance by every machine's policy. Keep these
facts separate from the event evidence identifying the actual launch block.

## Small fallback when a script is blocked

Paste this into an already available PowerShell window, changing only `$p`.
These are built-in read operations, with fixed failure messages that avoid
printing personal paths. They do not collect event logs or establish which policy
blocked the game. No execution-policy override is required or recommended.

```powershell
$p = 'C:\Program Files\Mir2 Invite\mir2-platform-windows.exe'
try { Get-FileHash -LiteralPath $p -Algorithm SHA256 -ErrorAction Stop | Select-Object Hash } catch { 'SHA256: read failed' }
try { Get-AuthenticodeSignature -LiteralPath $p -ErrorAction Stop | Select-Object Status,SignatureType,@{Name='SignerThumbprint';Expression={if ($null -ne $_.SignerCertificate) { $_.SignerCertificate.Thumbprint }}} } catch { 'Authenticode: read failed' }
try { Get-ItemProperty -LiteralPath 'HKLM:\SYSTEM\CurrentControlSet\Control\CI\Policy' -Name VerifiedAndReputablePolicyState -ErrorAction Stop | Select-Object VerifiedAndReputablePolicyState } catch { 'SAC state: read failed' }
try { Get-ItemProperty -LiteralPath 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion' -ErrorAction Stop | Select-Object CurrentBuildNumber,UBR,DisplayVersion } catch { 'OS build: read failed' }
```

## Verification scope

Run the controlled fixtures with:

```powershell
powershell.exe -NoProfile -File apps/game-client/platform-windows/scripts/test-diagnose-launch-block.ps1
```

The fixtures mock file/signature/registry/event readers; they do not query the
affected laptop, launch a game, or alter system configuration. They verify
matching, correlation, time and count bounds, rejected ambiguous/XML inputs,
privacy filtering, and the distinction between a failed read and no records.
Passing them is not evidence that the user's launch problem has been diagnosed
or that this computer will permit a particular publisher or game build.
