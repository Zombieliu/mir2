python3 - <<'PYREMOTE'
import pathlib,json,datetime
root=pathlib.Path('/home/ubuntu/mir2-playtest-preflight-20260929/r18-dense-20261004-161010');output=root/'r18-execution-01';result={'generatedUtc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'root':str(root),'wrapperFinished':False,'phaseReceipts':[],'traces':[]}
launch=root/'EXECUTION-LAUNCH-01.json'
if launch.is_file():
    row=json.loads(launch.read_text());result.update({'wrapperPid':row['wrapperPid'],'wrapperExitCode':row['wrapperExitCode'],'wrapperProcExists':pathlib.Path('/proc/'+str(row['wrapperPid'])).exists(),'wrapperFinished':row['wrapperExitCode'] is not None})
for p in sorted(output.glob('*-stage.json')):
    row=json.loads(p.read_text());result.setdefault('completedStages',[]).append({k:row.get(k) for k in ['phase','classes','executionError','ownedGatewayNormalExitCode','normalLogoutConfirmed']})
for p in sorted(output.glob('*-receipts/report.json')):
    row=json.loads(p.read_text());result['phaseReceipts'].append({'phase':row['phase'],'classes':[{'className':c['className'],'status':c['status'],'error':c.get('error'),'cases':[{'kind':t['kind'],'trial':t.get('trial'),'status':t.get('status')} for t in c.get('cases',[])],'deathStatus':c.get('death',{}).get('status'),'deadActions':c.get('death',{}).get('deadActions',{}).get('status'),'freshLoad':c.get('freshLoad',{}).get('status'),'genericPersistence':c.get('genericAlivePersistence',{}).get('status')} for c in row['classes']]})
for p in sorted(output.glob('*-receipts/*.jsonl')):
    with p.open('rb') as h:
        h.seek(max(0,p.stat().st_size-65536));lines=h.read().splitlines()
    last=None
    for line in reversed(lines):
        try:last=json.loads(line);break
        except ValueError:continue
    if last:result['traces'].append({'file':str(p.relative_to(output)),'bytes':p.stat().st_size,'lastSequence':last.get('sequence'),'lastType':last.get('type'),'lastPacket':last.get('packet'),'lastMs':last.get('tsMs'),'state':{k:last.get('state',{}).get(k) for k in ['map','x','y','hp','mp','dead']}})
for p in sorted(output.glob('*.driver.stdout.log')):
    result.setdefault('publicDriverStdout',[]).append({'file':p.name,'tail':p.read_text()[-1800:]})
print(json.dumps(result))
PYREMOTE
