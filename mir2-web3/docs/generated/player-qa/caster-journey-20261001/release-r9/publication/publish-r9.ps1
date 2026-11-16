$ErrorActionPreference='Stop'
$taskRoot='C:/mir2-playtest-releases/20261001-native-r9'
$taskPlan=Join-Path $taskRoot 'deployment'
$taskRemote='C:/mir2-ui-repair-20260921/playtest-release-20260928/remote.py'
$taskSource='180b01d46f35a5e97f4c60ee2ae033c7b53717e5'
$taskReceipt=Join-Path $taskRoot 'publish-process.json'
if(Test-Path -LiteralPath $taskReceipt){throw 'Fresh publication receipt required'}
$taskResult=[ordered]@{sequence=5;source=$taskSource;startedUtc=[DateTimeOffset]::UtcNow.ToString('o');complete=$false;success=$false;pid=$PID;phase='preflight';gameServicesChanged=$false}
function RecordPhase([string]$Phase){$taskResult.phase=$Phase;$taskResult | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $taskReceipt -Encoding utf8}
try {
    foreach($taskName in @('build-package-process.json','release-process.json','real-integration-final-process.json')){
        $taskGate=Get-Content -LiteralPath (Join-Path $taskRoot $taskName) -Raw | ConvertFrom-Json -DateKind String
        if(-not$taskGate.complete -or -not$taskGate.success -or $taskGate.source-cne$taskSource){throw ('Exact successful release gate required: '+$taskName)}
    }
    $taskLocal=Get-Content -LiteralPath (Join-Path $taskRoot 'r9-release-receipt.json') -Raw | ConvertFrom-Json -DateKind String
    if(-not$taskLocal.passed -or $taskLocal.sourceRevision-cne$taskSource -or $taskLocal.updaterSequence-ne5){throw 'Verified local installer/update artifacts required'}
    RecordPhase 'preflight'
    & 'C:/Python313/python.exe' $taskRemote --command-file (Join-Path $taskPlan 'deploy-preflight.sh') --timeout 60 *> (Join-Path $taskPlan 'preflight.log')
    if($LASTEXITCODE-ne0){throw 'Server preflight failed'}
    RecordPhase 'create-upload-stage'
    & 'C:/Python313/python.exe' $taskRemote --command-file (Join-Path $taskPlan 'deploy-create-stage.sh') --timeout 60 *> (Join-Path $taskPlan 'create-stage.log')
    if($LASTEXITCODE-ne0){throw 'Upload stage creation failed'}
    RecordPhase 'upload'
    & 'C:/Python313/python.exe' $taskRemote --upload-batch (Join-Path $taskPlan 'deploy-upload.json') --timeout 1800 *> (Join-Path $taskPlan 'upload.log')
    if($LASTEXITCODE-ne0){throw 'Pinned release upload failed'}
    RecordPhase 'verify-and-publish'
    & 'C:/Python313/python.exe' $taskRemote --command-file (Join-Path $taskPlan 'deploy-apply.sh') --timeout 1800 *> (Join-Path $taskPlan 'apply.log')
    if($LASTEXITCODE-ne0){throw 'Static update publication failed; retained stage requires inspection'}
    $taskResult.success=$true
    RecordPhase 'complete'
} catch {$taskResult.error=$_.Exception.Message} finally {
    $taskResult.complete=$true
    $taskResult.finishedUtc=[DateTimeOffset]::UtcNow.ToString('o')
    $taskResult | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $taskReceipt -Encoding utf8
}
if(-not$taskResult.success){exit 1}
