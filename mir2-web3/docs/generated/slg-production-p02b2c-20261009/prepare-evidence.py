from pathlib import Path

work = Path(__file__).resolve().parent
previous = work.parent / 'slg-production-p02b2b-20261009'
text = (previous / 'package-evidence.ps1').read_text(encoding='utf-8')


def replace(old, new):
    global text
    assert old in text, f'Missing original fragment: {old[:100]}'
    text = text.replace(old, new)


replace('slg-production-p02b2b-20261009', 'slg-production-p02b2c-20261009')
replace('$taskAudit.actualPassingExecutions -le 11 -or $taskAudit.distinctNamedPassingTests -lt 11 -or $taskAudit.newUniqueTests -ne 11',
        '$taskAudit.actualPassingExecutions -le 8 -or $taskAudit.distinctNamedPassingTests -lt 8 -or $taskAudit.newUniqueTests -ne 8')
replace("@{'drop-uid-tests-01'=@(101,10,1)}[$taskRun]",
        "@{'creature-reward-tests-01'=@(0,8,0);'creature-regression-01'=@(0,55,0);'use-item-regression-01'=@(101,46,1)}[$taskRun]")
replace("note='New carried fixture inherited template description while incoming used a drop description; original functional identity correctly refused merge. Only fixture descriptions unified; all original assertions retained, positive merge checks added.'",
        "note=if($taskRun -eq 'use-item-regression-01'){'Original TownTeleport test expected configured spawn after its movement fixture had rebound to the nearest safety zone. Source02 pins this routing fixture bind point explicitly; all original assertions and normal return behavior retained. Source01 46pass/1fail is historical only.'}else{'Original Source01 positive checks retained as history; final Source02 checks are counted separately, not added together.'}")
replace('mir2.production-drops-history.v1', 'mir2.production-creature-box-history.v1')
replace('mir2.production-drops-packaging.v1', 'mir2.production-creature-box-packaging.v1')
old = "foreach($taskFile in Get-ChildItem -LiteralPath $PSScriptRoot -File|Where-Object{$_.Name -match '^(authored-source-|independent-.*\\.json$|run-check.*\\.ps1$|relink-probe-.*\\.json$|serial-)' -or $_.Name -in @('audit-results.ps1','audit-results-01.ps1','audit-helper-error-01.json','package-evidence.ps1','prepare-evidence.py','qualify-parent.ps1','result-audit-01.json','private-cache.json','protected-parent-qualification.json','final-protected-parent-qualification.json','historical-results-audit.json','preserve-failed-fixture.py','failed-source06-exact-drop-uid-tests.rs','inventory-before.rs')}){Copy-TaskProof $taskFile.FullName $taskFile.Name}"
new = """foreach($taskFile in Get-ChildItem -LiteralPath $PSScriptRoot -File|Where-Object{$_.Name -match '^(authored-source-|independent-.*\\.json$|run-check.*\\.ps1$|relink-probe-.*\\.json$|serial-)' -or $_.Name -in @('audit-results.ps1','package-evidence.ps1','prepare-evidence.py','qualify-parent.ps1','qualify-parent-final.ps1','result-audit-01.json','private-cache.json','protected-parent-qualification.json','final-protected-parent-qualification.json','historical-results-audit.json','wait-and-check.ps1','fixture-repair.json','repair-fixture.py','runner-byte-restoration.json','update-docs.py','audit-helper-error-01.json')}){Copy-TaskProof $taskFile.FullName $taskFile.Name}
foreach($taskArchive in Get-ChildItem -LiteralPath $PSScriptRoot -Directory|Where-Object{$_.Name -match '^authored-bytes-\\d+$'}){
    foreach($taskFile in Get-ChildItem -LiteralPath $taskArchive.FullName -Recurse -File){Copy-TaskProof $taskFile.FullName ($taskArchive.Name+'/'+$taskFile.FullName.Substring($taskArchive.FullName.Length+1))}
}"""
replace(old, new)
out = work / 'package-evidence.ps1'
assert not out.exists(), 'Preserve existing packaging helper'
out.write_text(text, encoding='utf-8', newline='\n')
print('Prepared bounded exact-byte packaging helper; no product source or Cargo mutation.')
