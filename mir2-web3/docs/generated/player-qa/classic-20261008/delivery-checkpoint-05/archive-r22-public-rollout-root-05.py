import datetime
import hashlib
import json
from pathlib import Path
import subprocess
import zipfile

root = Path('C:/Users/Administrator/.codex/worktrees/classic-gateway-budget-20261008/mir2-player-journey')
base = Path('C:/mir2-playtest-releases/20261008-classic-r22')
out = root/'mir2-web3/docs/generated/player-qa/classic-20261008/delivery-checkpoint-05'
assert not out.exists()
out.mkdir()

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def write_new(path,data):
    path.parent.mkdir(parents=True,exist_ok=True)
    with path.open('xb') as file:
        file.write(data)

def load_json_log(path):
    rows=[line for line in path.read_text(encoding='utf-8-sig').splitlines() if line.startswith('{')]
    assert len(rows)==1
    return json.loads(rows[0])

build=Path('C:/mir2-build')
apply_path=build/'r22-gateway-apply-root-20261008-1138-01.log'
feed_path=build/'r22-origin-feed-promote-root-20261008-1142-01.log'
applied=load_json_log(apply_path)
feed=load_json_log(feed_path)
exit_proof=json.loads((build/'r22-normal-client-exit-root-20261008-01.json').read_bytes())
post=json.loads((build/'r22-paired-postflight-root-20261008-01.json').read_bytes())
smoke=json.loads((base/'public-smoke-01/RESULT.json').read_bytes())
promote=json.loads((base/'cdn-promote-ci-01/extracted/receipt.json').read_bytes())
authenticated=json.loads((base/'cdn-promote-ci-01/AUTHENTICATED-ROOT-01.json').read_bytes())
assert applied['ok'] and applied['applied'] and applied['revision']=='a41dfe72d7e5858f11169adf88673dc44ed24200'
assert applied['priorStop']['ExecMainStatus']=='0' and applied['candidateStop']['ExecMainStatus']=='0'
assert applied['candidateShutdownPanic'] is False and applied['originalPid']=='3855184'
assert applied['envBeforeSha256']==applied['envAfterSha256']
assert feed['passed'] and feed['sequence']==17 and feed['allServicePidsPreserved']
assert exit_proof['exitRecord']['processId']==181324 and exit_proof['exitRecord']['exitCode']==0
assert post['passed'] and len(post['checks'])==4 and not post['cmsReverifiedByThisCheck']
assert smoke['passed'] and len(smoke['rows'])==44 and len(smoke['normalLogouts'])==8
assert len(smoke['ownedCharacterVariants'])==6 and all(row['confirmed'] for row in smoke['normalLogouts'])
assert smoke['newAccountRegistrations']==smoke['newCharacterCreations']==0
assert smoke['adminCommands'] is False and smoke['qaStatePreparation'] is False
assert promote['passed'] and promote['aliasesVerified'] and promote['privatePointerVerified']
assert promote['pointerChanged'] and not promote['pointerOutcomeUnknown']
assert promote['verifiedObjectCount']==0 and promote['publicVerifiedBytes']==0
assert not promote['cmsReverified'] and not promote['publicVerified']
assert authenticated['runId']==37772462840 and authenticated['zipBytesMatchAuthenticatedDigest']

# These are already redacted public QA artifacts, not account/private sources.
# Never copy the private account ledger. Confirm no actual password occurs in
# any public smoke byte stream before writing the public evidence archive.
private=Path('C:/mir2-playtest-releases/20261007-remote-player-r21/public-wss-03/private/accounts.json')
cohort=json.loads(private.read_bytes())
secret_values={entry['password'].encode() for entry in [*cohort['cohort'],cohort['female']]}
public_files=[path for path in (base/'public-smoke-01').iterdir() if path.is_file()]
for path in public_files:
    data=path.read_bytes()
    assert all(secret not in data for secret in secret_values), 'Public artifact failed private-value exclusion'
del secret_values,cohort
smoke_archive=out/'ordinary-public-smoke-original.zip'
with zipfile.ZipFile(smoke_archive,'x',compression=zipfile.ZIP_DEFLATED,compresslevel=9) as zipped:
    for path in sorted(public_files):
        zipped.write(path,path.name)
smoke_entries=[]
with zipfile.ZipFile(smoke_archive) as zipped:
    assert zipped.testzip() is None
    for path in sorted(public_files):
        data=path.read_bytes()
        assert zipped.read(path.name)==data
        smoke_entries.append({'file':path.name,'bytes':len(data),'sha256':sha(path)})
write_new(out/'ORDINARY-SMOKE-ZIP-ENTRIES.json',json.dumps(smoke_entries,indent=2).encode()+b'\n')
sources=[
    build/'r22-gateway-preflight-root-20261008-1127-01.log', apply_path, feed_path,
    build/'r22-normal-client-exit-root-20261008-01.json',
    build/'r22-paired-postflight-root-20261008-01.json',
    build/'r22-promote-dispatch-intent-root-20261008-01.json',
    build/'r22-promote-dispatch-root-20261008-01.log',
    build/'r22-native-launch-root-20261008-01.json',
    build/'r22-public-smoke-root-20261008-01.log',
    base/'public-smoke-01/RESULT.json', base/'public-smoke-01/PREPARATION.json',
    base/'cdn-promote-ci-01/artifact.zip',
    base/'cdn-promote-ci-01/RUN.json', base/'cdn-promote-ci-01/JOBS.json',
    base/'cdn-promote-ci-01/ARTIFACT-METADATA.json',
    base/'cdn-promote-ci-01/AUTHENTICATED-ROOT-01.json',
    base/'cdn-promote-ci-01/extracted/receipt.json',
    base/'cdn-promote-ci-01/extracted/stage/receipt.json',
    base/'cdn-promote-ci-01/extracted/stage/receipt.sha256',
    base/'cdn-promote-ci-01/extracted/stage-run.json',
    base/'cdn-stage-ci-01/INDEPENDENT-ROOT-02.json',
    base/'operators-01/OPERATOR-RECIPES.json',
    base/'operators-01/upgrade-classic-gateway-02.py',
    base/'operators-01/GATEWAY-PREFLIGHT-01.command',
    base/'operators-01/GATEWAY-APPLY-01.command',
    base/'operators-01/FEED-PROMOTE-01.command',
]
bindings=[]
for index,path in enumerate(sources):
    relative=f'{index:02}-{path.name}'
    write_new(out/relative,path.read_bytes())
    assert (out/relative).read_bytes()==path.read_bytes()
    bindings.append({'file':relative,'original':str(path),'bytes':path.stat().st_size,'sha256':sha(path)})
proof={'schema':'mir2.r22.actual-paired-rollout-root-05.v1',
       'recordedAtUtc':datetime.datetime.now(datetime.timezone.utc).isoformat(),
       'gameSource':'7fea5cdba8cd160d4f2c5536f14fb7ce98c6bfbc',
       'gatewaySource':applied['revision'],'publisherSource':authenticated['source'],
       'publicWindowsVersion':'WN-CANDIDATE-20261008-invited-22','sequence':17,
       'normalClientExitObserved':True,'newHumanExitMessageReceived':False,
       'strictGatewayAndTcpDrainPreflightPassed':True,'pairedGatewayApplied':True,
       'oldAndCandidateStopsZero':True,'candidateColdRestartPassed':True,
       'privateDatabaseStateAndConfigBackup':applied['backup'],
       'backupDatabaseDumpSha256':applied['databaseDumpSha256'],
       'environmentAndCapacityAndResourcesPreserved':True,
       'originalProductionIdentityPreserved':{'pid':applied['originalPid'],'release':applied['originalRelease']},
       'originFeed17Promoted':True,'r2Pointer17Promoted':True,
       'promotionRun':37772462840,'authenticatedPromotionArtifact':authenticated,
       'independentOriginAndCdnAliases':post['checks'],
       'completeFortyObjectChecksBelongToEarlierStage':37739769708,
       'promotionFullObjectReverification':False,'promotionCmsReverification':False,
       'ordinaryPublicChecks':44,'ordinaryClassGenderVariants':6,'normalOwnedLogouts':8,
       'newAccountOrCharacterCreation':False,'adminOrQaStatePreparation':False,
       'privateValuesExcludedFromPublicTraffic':True,'originalEvidence':bindings,
       'nativeLaunch':{'pid':187896,'source':'7fea5cdba8cd160d4f2c5536f14fb7ce98c6bfbc','manualLogin':True,
                       'observedAliveAndFirstMainUpdate':True,'humanVisualAccepted':False},
       'humanShopWheelAccepted':False,'crowdedCombatAccepted':False,
       'healthFollowupB235IncludedInFrozenR22':False,'fullP1P7Complete':False,
       'originalDeadlineMissed':'2026-10-08 03:13:35 UTC'}
write_new(out/'ROOT-FINAL-05.json',json.dumps(proof,indent=2).encode()+b'\n')
print(json.dumps({'archive':str(out),'originalFiles':len(bindings),'smokeZipOriginalEntries':len(smoke_entries),
                  'publicPairedRollout':True,'humanWheelAccepted':False,'rootProofSha256':sha(out/'ROOT-FINAL-05.json')}))
