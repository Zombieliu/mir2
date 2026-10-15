sudo /usr/bin/python3 - <<'PYREMOTE'
import datetime,hashlib,json,pathlib,subprocess,urllib.request,re
sha=lambda p:hashlib.file_digest(pathlib.Path(p).open('rb'),'sha256').hexdigest()
def show(unit):
    text=subprocess.check_output(['systemctl','show',unit,'--property=MainPID','--property=ActiveState','--property=Result','--property=NRestarts','--property=FragmentPath','--property=DropInPaths','--property=EnvironmentFiles'],text=True)
    values=dict(line.split('=',1) for line in text.splitlines() if '=' in line)
    files=[]
    for key in ['FragmentPath','DropInPaths','EnvironmentFiles']:
        for name in re.findall(r'/[^\s;()]+',values.pop(key,'')):
            path=pathlib.Path(name)
            if path.is_file():files.append({'path':name,'sha256':sha(path),'bytes':path.stat().st_size})
    return {'properties':values,'files':files}
keys=['currentWsConnections','currentActiveSessions','currentReconnectLeases','currentLoginInFlight','currentNewCharacterInFlight','currentStartGameInFlight']
opener=urllib.request.build_opener(urllib.request.ProxyHandler({}));realms=[]
for port,tcp,unit in [(7210,7200,'mir2-playtest.service'),(7110,7100,'mir2-gateway.service')]:
    with opener.open('http://127.0.0.1:'+str(port)+'/health',timeout=8) as response:health=json.load(response)
    assert health.get('ok') is True and health.get('http')=='ready' and health.get('ws')=='ready'
    counters={k:health.get('capacity',{}).get(k) for k in keys};assert all(type(n) is int and n>=0 for n in counters.values())
    service=show(unit);pid=int(service['properties']['MainPID']);assert service['properties']['ActiveState']=='active' and pid>0
    exe=pathlib.Path('/proc')/str(pid)/'exe'
    established=subprocess.check_output(['ss','-Htn','state','established','sport = :'+str(tcp)],text=True)
    realms.append({'port':port,'tcpPort':tcp,'unit':unit,'service':service,'exe':str(exe.resolve()),'exeSha256':sha(exe),'revision':health.get('revision'),'ready':True,'sixCounters':counters,'tcpEstablishedCount':len(established.splitlines())})
caddy=show('caddy.service');assert caddy['properties']['ActiveState']=='active'
config=[]
for name in ['/etc/mir2-playtest/gateway.env','/etc/systemd/system/mir2-playtest.service','/etc/caddy/Caddyfile','/etc/systemd/system/mir2-playtest.service.d/50-periodic-b71.conf']:
    path=pathlib.Path(name);assert path.is_file();config.append({'path':name,'sha256':sha(path),'bytes':path.stat().st_size})
feed=pathlib.Path('/srv/mir2-client-updates/feed-current/latest.json');value=json.loads(feed.read_text())
print(json.dumps({'schema':'mir2.r18-isolated-execution-live-readonly.v1','generatedUtc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'realms':realms,'caddy':caddy,'configHashes':config,'feed':{'path':str(feed.resolve()),'sha256':sha(feed),'sequence':value.get('sequence'),'candidate':value.get('game',{}).get('identity')},'serviceStoreOrPlayerMutation':False,'environmentContentsPrinted':False,'capacityAccepted':False}))
PYREMOTE
