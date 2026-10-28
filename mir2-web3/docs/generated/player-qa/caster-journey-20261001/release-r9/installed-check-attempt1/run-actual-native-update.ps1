$ErrorActionPreference='Stop'
$taskRoot='C:/mir2-playtest-releases/20261001-native-r9'
$taskReceipt=Join-Path $taskRoot 'actual-native-update-process.json'
if(Test-Path -LiteralPath $taskReceipt){throw 'Fresh actual native update receipt required'}
$taskResult=[ordered]@{source='180b01d46f35a5e97f4c60ee2ae033c7b53717e5';candidate='WN-CANDIDATE-20261001-invited-09';startedUtc=[DateTimeOffset]::UtcNow.ToString('o');complete=$false;success=$false;pid=$PID;phase='wait-publication';gameStarted=$false}
function RecordPhase([string]$Phase){$taskResult.phase=$Phase;$taskResult | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $taskReceipt -Encoding utf8}
try {
    RecordPhase 'wait-publication'
    $taskDeadline=[DateTimeOffset]::UtcNow.AddMinutes(35)
    do {
        $taskPublished=Get-Content -LiteralPath (Join-Path $taskRoot 'publish-process.json') -Raw | ConvertFrom-Json -DateKind String
        if($taskPublished.complete){break}
        if([DateTimeOffset]::UtcNow-gt$taskDeadline){throw 'Publication wait deadline exceeded'}
        Start-Sleep -Seconds 5
    } while($true)
    if(-not$taskPublished.success-or$taskPublished.sequence-ne5-or$taskPublished.source-cne$taskResult.source){throw 'Exact successful sequence-five publication required'}
    RecordPhase 'native-https-r8-to-r9'
    & 'C:/Python313/python.exe' (Join-Path $taskRoot 'native-installed-update-r9.py') *> (Join-Path $taskRoot 'actual-native-update.log')
    if($LASTEXITCODE-ne0){throw 'Actual native HTTPS verification failed; inspect retained log'}
    $taskProof=Get-Content -LiteralPath (Join-Path $taskRoot 'actual-native-https-update-report.json') -Raw | ConvertFrom-Json -DateKind String
    if(-not$taskProof.passed-or$taskProof.sourceRevision-cne$taskResult.source-or$taskProof.sequence-ne5){throw 'Exact installed verification receipt required'}
    $taskResult.success=$true
    RecordPhase 'complete'
} catch {$taskResult.error=$_.Exception.Message} finally {
    $taskResult.complete=$true
    $taskResult.finishedUtc=[DateTimeOffset]::UtcNow.ToString('o')
    $taskResult | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $taskReceipt -Encoding utf8
}
if(-not$taskResult.success){exit 1}
