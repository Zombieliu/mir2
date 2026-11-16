from pathlib import Path

work = Path(__file__).resolve().parent
previous = work.parent / 'slg-production-p02b2a-20261008'
text = (previous / 'package-evidence.ps1').read_text(encoding='utf-8')

def replace(old, new):
    global text
    assert old in text, f'Missing original packaging fragment: {old[:100]}'
    text = text.replace(old, new)

replace('slg-production-p02b2a-20261008', 'slg-production-p02b2b-20261009')
replace("$taskAudit.actualPassingExecutions -ne 208 -or $taskAudit.distinctNamedPassingTests -ne 204 -or $taskAudit.newUniqueTests -ne 16",
        "$taskAudit.actualPassingExecutions -le 11 -or $taskAudit.distinctNamedPassingTests -lt 11 -or $taskAudit.newUniqueTests -ne 11")
replace("@{'grant-tests-01'=@(101,0,0);'grant-tests-02'=@(101,4,4)}[$taskRun]",
        "@{'drop-uid-tests-01'=@(101,10,1)}[$taskRun]")
replace("note=if($taskRun -eq 'grant-tests-01'){'Four ItemGrade root-path compile errors, zero executed tests; product enum path corrected'}else{'New fixture used a nonexistent potion name; five literals corrected to original (HP)DrugSmall, all assertions unchanged'}",
        "note='New carried fixture inherited template description while incoming used a drop description; original functional identity correctly refused merge. Only fixture descriptions unified; all original assertions retained, positive merge checks added.'")
replace("mir2.production-grants-history.v1", "mir2.production-drops-history.v1")
replace("serialPreflightOrWaitCount=2", "serialPreflightOrWaitCount=@(Get-ChildItem -LiteralPath $PSScriptRoot -File -Filter 'serial-preflight-*.json').Count")
old = """foreach($taskFile in Get-ChildItem -LiteralPath $PSScriptRoot -File|Where-Object{$_.Name -match '^(authored-source-|independent-source-review-|serial-|.*preflight.*\\.console\\.log$)' -or $_.Name -in @('run-check-01.ps1','run-check-02.ps1','run-check.ps1','audit-results.ps1','package-evidence.ps1','result-audit-01.json','private-cache.json','protected-parent-qualification.json','historical-results-audit.json')}){Copy-TaskProof $taskFile.FullName $taskFile.Name}"""
new = """foreach($taskFile in Get-ChildItem -LiteralPath $PSScriptRoot -File|Where-Object{$_.Name -match '^(authored-source-|independent-.*\\.json$|run-check.*\\.ps1$|relink-probe-.*\\.json$|serial-)' -or $_.Name -in @('audit-results.ps1','package-evidence.ps1','prepare-evidence.py','qualify-parent.ps1','result-audit-01.json','private-cache.json','protected-parent-qualification.json','final-protected-parent-qualification.json','historical-results-audit.json','preserve-failed-fixture.py','failed-source06-exact-drop-uid-tests.rs','inventory-before.rs')}){Copy-TaskProof $taskFile.FullName $taskFile.Name}"""
replace(old, new)
replace('mir2.production-grants-packaging.v1', 'mir2.production-drops-packaging.v1')
replace('actualPassingExecutions=208;distinctNamedPassingTests=204;newUniqueTests=16',
        'actualPassingExecutions=$taskAudit.actualPassingExecutions;distinctNamedPassingTests=$taskAudit.distinctNamedPassingTests;newUniqueTests=$taskAudit.newUniqueTests')
out = work / 'package-evidence.ps1'
assert not out.exists(), 'Preserve existing packaging helper'
out.write_text(text, encoding='utf-8', newline='\n')
print('Prepared bounded exact-byte packaging helper; no product source or test mutation.')
