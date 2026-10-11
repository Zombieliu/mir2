
import http.client,socket,ssl
CONFIG=pathlib.Path('/etc/caddy/Caddyfile')
CONFIG_PIN='139d2f48a77e2af9f1ff0b0baec81aef47c6d084c1ce95f6df32c23889dda5d1'
ENV_PIN='77f12d974ca32103673181669803c89283c68cf34a952c0a177b721c1d013729'
UNIT_PIN='5f5f8a4770fee45035d9b8c9c25b4a45a6d85c06ceea3f3b1204235d8bce7a51'
REVISION='4006d988b3922d0106278720abfa9b3d384c8d5e'
FINGERPRINT=('fc530898d8cbf563389fe708ac3a279846d28b56ae97e340af3ed7d100ad8202',84778528)
BACKUP=BACKUP_PARENT/'mir2-caddy-stripe-live-20261011-01'
CALLBACK='/playtest/v1/billing/stripe/webhook'
FEEDS={'latest.json':'a97dbb60c48a19e7f3daf28a9b9ea42c9dbfea42bb8566f160c47475516f6d62','latest.p7s':'01ac444bc13bd822ba8283db4be7f30a36562e84c651c9bb6553b10921285d43'}
class LocalTLS(http.client.HTTPSConnection):
    def connect(self):
        self.sock=self._context.wrap_socket(socket.create_connection(('127.0.0.1',self.port),self.timeout),server_hostname=self.host)
def public_read(path,method='GET',signature=None):
    c=LocalTLS('165.154.65.136.sslip.io',timeout=15,context=ssl.create_default_context())
    headers={'Accept-Encoding':'identity','Content-Type':'application/json'}
    if signature is not None:headers['Stripe-Signature']=signature
    try:
        c.request(method,path,body=b'{}' if method=='POST' else None,headers=headers)
        response=c.getresponse(); b=response.read(65537)
        require(len(b)<=65536,'Bounded public response required')
        return response.status,dict((k.lower(),v) for k,v in response.getheaders()),b
    finally:c.close()
def protected():
    pid=status()['MainPID']
    require(ready(health()) and health()['revision']==REVISION,'Qualified current Gateway changed')
    require(process_binary(pid)==(RELEASE_PARENT/REVISION/'mir2-gateway',FINGERPRINT),'Actual Gateway inode changed')
    process_env=pathlib.Path('/proc/'+pid+'/environ').read_bytes()
    values=dict(x.split(b'=',1) for x in process_env.split(b'\0') if b'=' in x)
    require(values.get(b'MIR2_STRIPE_ENABLED',b'0') in (b'0',b'false'),'Stripe provider baseline changed')
    require(not values.get(b'MIR2_STRIPE_SECRET_KEY') and not values.get(b'MIR2_STRIPE_WEBHOOK_SECRET'),'Unexpected current Stripe credentials')
    require(values.get(b'MIR2_MONTHLY_CARD_CREDIT_PRICE')==b'1000' and values.get(b'MIR2_MONTHLY_CARD_REQUIRED',b'0') in (b'0',b'false'),'Current monthly policy changed')
    require(show(('DropInPaths',))['DropInPaths']==str(UNITFILE)+' '+str(WATCH_UNITFILE),'Unexpected new service override')
    return {'original':original_identity(),'pid':pid,'binary':process_binary(pid),
        'processEnv':process_env,
        'env':checked(ENV,65536,ENV_PIN,private=True),'unit':checked(UNITFILE,32768,UNIT_PIN),
        'watch':checked(WATCH_UNITFILE,65536,WATCH_UNIT_SHA256),'base':checked(BASE_UNITFILE,32768,BASE_UNIT_SHA256),
        'caddyPid':run(['systemctl','show','caddy.service','-p','MainPID','--value']).stdout.strip()}
def feed_checks():
    for name,pin in FEEDS.items():
        code,headers,data=public_read('/client-updates/'+name)
        require(code==200 and hashlib.sha256(data).hexdigest()==pin and headers.get('cache-control')=='no-store','Signed update pointer changed')
def verify_closed_neighbors():
    for path in ('/playtest/v1/admin','/playtest/v1/billing/config','/playtest/v1/billing/webhook','/playtest/v1/billing/checkout','/playtest/v1/billing/status'):
        require(public_read(path)[0]==404,'An unapproved adjacent route is exposed')
try:
    require(os.geteuid()==0 and sys.platform=='linux' and sys.argv[1:] in (['--preflight'],['--apply']),'Explicit root route mode required')
    directory(CONFIG.parent);directory(BACKUP_PARENT)
    require(not BACKUP.exists() and not BACKUP.is_symlink(),'Existing backup requires inspection, never repeated apply')
    config=checked(CONFIG,131072,CONFIG_PIN)
    old=b'@mir2_playtest_public path /ws /health /spectator/matches /spectator/ws /ai-live/status /v1/billing/return'
    require(config.count(old)==1 and b'/v1/billing/stripe/webhook' not in config,'Pinned existing whitelist required')
    candidate=config.replace(old,old+b' /v1/billing/stripe/webhook')
    before=protected();feed_checks();verify_closed_neighbors()
    require(before['original']=={'pid':'3855184','release':'/opt/mir2/gateway/releases/79ba815c0-checkpoint-base64'},'Original realm baseline changed')
    require(public_read(CALLBACK,'POST')[0]==404,'Callback already public, inspect before changing')
    code,headers,html=public_read('/playtest/v1/billing/return?result=paid&lang=zh-Hant')
    require(code==200 and hashlib.sha256(html).hexdigest()=='b3476392d221966a03ac09a6bebd02b33c44b3a10cce991f9cb7b99e1196fc94','Styled return changed')
    report={'schema':'mir2.stripe-live.webhook-route.v1','passed':True,'applied':False,'sourceRevision':REVISION,'oldCaddySha256':CONFIG_PIN,
        'newCaddySha256':hashlib.sha256(candidate).hexdigest(),'onlyRouteAdded':CALLBACK,'providerStillDisabled':True,'gatewayRestarted':False,'keysCopied':False}
    if sys.argv[1:]==['--apply']:
        BACKUP.mkdir(mode=0o700);newfile(BACKUP/'Caddyfile.before',config);newfile(BACKUP/'Caddyfile.candidate',candidate)
        caddy_pid=before['caddyPid'].decode()
        caddy_env=dict(x.split(b'=',1) for x in pathlib.Path('/proc/'+caddy_pid+'/environ').read_bytes().split(b'\0') if b'=' in x)
        required=set(re.findall(rb'\{\$([A-Z][A-Z0-9_]*)\}',config))
        require(all(caddy_env.get(key) for key in required),'Current Caddy matcher environment missing')
        subprocess.run(['/usr/bin/caddy','validate','--config',str(BACKUP/'Caddyfile.candidate'),'--adapter','caddyfile'],env=caddy_env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=25,check=True)
        require(checked(CONFIG,131072,CONFIG_PIN)==config and protected()==before,'Pre-write config/runtime conflict')
        replace(CONFIG,candidate,0o644,REVISION,'stripe-webhook-only')
        run(['systemctl','reload','caddy.service'],timeout=30)
        require(checked(CONFIG,131072)==candidate and protected()==before,'Protected runtime or config changed during reload')
        code,headers,response=public_read(CALLBACK,'POST')
        require(code==400 and json.loads(response)=={'error':'billingWebhookSignatureInvalid'} and headers.get('cache-control')=='no-store','Unsigned callback must reject before provider parsing')
        require(public_read(CALLBACK,'POST','t=0,v1='+'0'*64)[0]==503,'Disabled provider must not accept forged callback')
        require(public_read('/playtest/v1/billing/return?result=paid&lang=zh-Hant')[2]==html,'Cosmetic return bytes changed')
        feed_checks();verify_closed_neighbors()
        report.update(applied=True,unsignedStatus=400,forgedWhileDisabledStatus=503,allServicePidsPreserved=True,signedFeedsPreserved=True,privateBackup=str(BACKUP))
        write_json(BACKUP/'APPLIED.json',report)
    print(json.dumps(report,sort_keys=True))
except SafetyError as exc:
    print(json.dumps({'passed':False,'error':str(exc),'repeatApplyAllowed':False}),flush=True);sys.exit(1)
except Exception as exc:
    print(json.dumps({'passed':False,'errorType':type(exc).__name__,'manualInspectionRequired':True,'repeatApplyAllowed':False}),flush=True);sys.exit(1)
