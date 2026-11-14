from pathlib import Path
base=Path(__file__).resolve().parent
source=(base/'run-runtime-check.ps1').read_text()
start=source.index('    [ValidateSet(')
end=source.index('\n)',start)
source=source[:start]+"    [ValidateSet('native-tests','portable-tests','native-check')][string]$Kind='native-tests'"+source[end:]
source=source.replace('independent-stone-runtime-source-review-05.json','independent-stone-frontend-source-review-01.json')
start=source.index('$taskSourcePaths=@(')
end=source.index('$taskRows=',start)
source=source[:start]+'''$taskSourcePaths=@('mir2-web3/rust-toolchain.toml')
foreach($taskRelative in @('apps/game-client','packages/game-data','packages/protocol','config/quest-guidance')){
    $taskListed=@(& rg --files -- (Join-Path $taskProject $taskRelative))
    if($LASTEXITCODE -ne 0){throw "Source enumeration failed: $taskRelative"}
    $taskSourcePaths+=@($taskListed | ForEach-Object {$_.Substring($taskRepo.Length+1).Replace('\\','/')})
}
$taskSourcePaths+=@(& rg --files (Join-Path $taskProject 'apps/web/public/original-ui') -g '*.json' | ForEach-Object {$_.Substring($taskRepo.Length+1).Replace('\\','/')})
$taskSourcePaths+=@('mir2-web3/apps/web/public/original-ui/Prguse/4.png','mir2-web3/apps/web/public/original-ui/Prguse/6.png')
foreach($taskConfig in @('.cargo/config.toml','mir2-web3/.cargo/config.toml')){
    if(Test-Path -LiteralPath (Join-Path $taskRepo $taskConfig)){$taskSourcePaths+=$taskConfig}
}
'''+source[end:]
source=source.replace("scope='Bounded stone/runtime implementation and adjacent regressions only. Controlled File and fake PG receipt evidence does not establish actual PostgreSQL, shared gateway, client rendering, sidebar, deployment or public playability.'",
    "scope='Current Stone NPC sidebar headless layout/input-model tests and Native compile only. Original modality is retained. No renderer, screenshot, gateway, PostgreSQL, installer, live gameplay or human acceptance is established.'")
start=source.index('$taskArgv=switch($Kind){')
end=source.index('$taskStart=',start)
source=source[:start]+'''$taskArgv=switch($Kind){
    'native-tests' {@('+1.95.0','test','--manifest-path','apps/game-client/client-bevy/Cargo.toml','--features','native-ui','--lib','workshop_','--locked','--offline','--jobs','1','--','--test-threads=1')}
    'portable-tests' {@('+1.95.0','test','--manifest-path','apps/game-client/client-bevy/Cargo.toml','--no-default-features','--features','portable-quest-ui','--lib','workshop_','--locked','--offline','--jobs','1','--','--test-threads=1')}
    'native-check' {@('+1.95.0','check','--manifest-path','apps/game-client/platform-windows/Cargo.toml','--locked','--offline','--jobs','1')}
}
'''+source[end:]
source=source.replace("$env:CARGO_TARGET_DIR=Join-Path $PSScriptRoot 'target'", "$env:CARGO_TARGET_DIR='C:/mir2-cross-platform-storage-20261002/rust-target/windows'")
source=source.replace("controlledInventoryCommitTests=($Kind -eq 'simulation-tests')", "headlessSidebarTests=($Kind -in @('native-tests','portable-tests'))")
(base/'run-frontend-check.ps1').write_text(source)
print('Created frontend wrapper; original Guard/template/tool pins/thresholds and cleanup are unchanged.')
