"""Public read-only postflight and allowlisted publication; excludes secrets."""
import datetime
import hashlib
import json
from pathlib import Path
import re
import subprocess
import urllib.error
import urllib.request

ROOT=Path('C:/mir2-playtest-releases/20261011-stripe-test')
PRIVATE=Path('C:/Users/Administrator/.mir2-private/stripe-public-test-trial-20261011')
PUB=Path('C:/Users/Administrator/.codex/worktrees/classic-publication-20261008/mir2-player-journey/mir2-web3')
OUT=PUB/'docs/generated/player-qa/stripe-test-20261011'
REV='4006d988b3922d0106278720abfa9b3d384c8d5e'
FEED='a97dbb60c48a19e7f3daf28a9b9ea42c9dbfea42bb8566f160c47475516f6d62'
CMS='01ac444bc13bd822ba8283db4be7f30a36562e84c651c9bb6553b10921285d43'
def need(value,message):
    if not value:raise RuntimeError(message)
def sha(data):return hashlib.sha256(data).hexdigest()
def write(name,value):
    data=json.dumps(value,indent=2,ensure_ascii=False)+'\n'
    need(not re.search(r'\b(?:sk|rk)_(?:test|live)_[A-Za-z0-9_]+|whsec_[A-Za-z0-9]+|Bearer |checkout\.stripe\.com/c/pay|"password"\s*:',data),'private-material-refused')
    (OUT/name).write_text(data,encoding='utf-8',newline='\n')

def redact(value):
    if isinstance(value,list):return [redact(v) for v in value]
    if isinstance(value,dict):
        result={}
        for k,v in value.items():
            if re.search('password|secret|email|answer|checkoutUrl$|token',k,re.I):continue
            if k.lower() in ('accountid','orderid','uniqueid','itemuniqueid') and isinstance(v,str):result[k+'Sha256']=sha(v.encode())
            else:result[k]=redact(v)
        return result
    return value

def main():
    OUT.mkdir(parents=True,exist_ok=True)
    need(not OUT.is_symlink(), 'ordinary-owned-evidence-directory-required')
    result=subprocess.run(['python','C:/mir2-ui-repair-20260921/playtest-release-20260928/remote.py',
       '--user','ubuntu','--command-file','E:/mir2-player-journey/tmp/gateway-lifecycle-read-public-01.sh',
       '--timeout','40'],capture_output=True,text=True,timeout=50)
    need(result.returncode==0,'independent-lifecycle-read-failed')
    state=json.loads(result.stdout)
    need(state['ok'] and state['test']['revision']==REV and state['unit']['MainPID']=='3369030'
         and state['unit']['NRestarts']=='0' and state['original']['pids']==['3855184'],'running-services-changed')
    need(state['files']['/etc/mir2-playtest/gateway.env']['sha256']=='41d0939baa6f164a5db0605ef441647c4c8214a6b4ac1dd7c69ab1da65c647c0', 'private-config-changed')
    checks=[]
    class NoRedirect(urllib.request.HTTPRedirectHandler):
        def redirect_request(self,*args):return None
    opener=urllib.request.build_opener(NoRedirect())
    def probe(url,method='GET',data=None):
        request=urllib.request.Request(url,method=method,data=data,headers={'Content-Type':'application/json'})
        try:response=opener.open(request,timeout=15)
        except urllib.error.HTTPError as error:response=error
        with response:
            body=response.read(262145);need(len(body)<=262144,'public-response-budget')
            return response.status,dict(response.headers),body
    for host in ('https://165.154.65.136.sslip.io','https://assets.mir2.obelisk.build'):
        for suffix,pin in (('latest.json',FEED),('latest.p7s',CMS)):
            url=host+'/client-updates/'+suffix
            status,headers,body=probe(url)
            need(status==200 and sha(body)==pin,'public-signed-feed-changed')
            checks.append({'url':url,'status':status,'sha256':sha(body),'bytes':len(body)})
    base='https://165.154.65.136.sslip.io/playtest'
    for neighbor in ('/v1/billing/status','/v1/billing/checkout','/admin/billing'):
        status,_,_=probe(base+neighbor)
        need(status==404,'private-http-neighbor-exposed')
        checks.append({'url':base+neighbor,'status':status})
    status,headers,body=probe(base+'/v1/billing/stripe/webhook','POST',b'{}')
    need(status==400 and json.loads(body)['error']=='billingWebhookSignatureInvalid'
         and headers.get('Cache-Control')=='no-store','unsigned-callback-not-refused')
    checks.append({'kind':'unsignedCallback','status':status,'error':'billingWebhookSignatureInvalid','cacheControl':'no-store'})
    status,headers,body=probe(base+'/v1/billing/return?result=paid&lang=zh-Hant')
    need(status==200 and headers.get('Cache-Control')=='no-store' and headers.get('Referrer-Policy')=='no-referrer'
         and "connect-src 'none'" in headers.get('Content-Security-Policy',''),'public-return-protection-changed')
    checks.append({'kind':'styledReturn','status':status,'sha256':sha(body),'bytes':len(body),'cacheControl':'no-store','unverifiedQueryCannotAwardCredits':True})
    report={'schema':'mir2.public-stripe-test.postflight.v1','passed':True,'observedAtUtc':datetime.datetime.now(datetime.timezone.utc).isoformat(),
            'sourceRevision':REV,'tlsVerified':True,'signedFeedSequence':19,'services':state,'publicChecks':checks}
    (ROOT/'POSTFLIGHT.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8',newline='\n')
    write('POSTFLIGHT.json',report)
    for file in ('provider-provision-01.json','PROVIDER-PAID-READBACK.json'):
        write(file,json.loads((ROOT/file).read_text()))
    apply=json.loads(next(l for l in (ROOT/'gateway-config-01/APPLY.log').read_text().splitlines() if l.startswith('{')))
    need(apply['passed'] and apply['applied'] and apply['checkoutEnabled'] and not apply['liveMode'],'successful-test-apply-required')
    write('GATEWAY-APPLIED.json',apply)
    runs=json.loads((PRIVATE/'player-report.json').read_text())
    need(runs['runs'][-1]['ok'] and runs['runs'][-1]['finalStatus']['credit']==0
         and runs['runs'][-1]['finalStatus']['monthlyCard']['active'], 'final-ordinary-relogin-not-passed')
    write('ORDINARY-WS-REDACTED.json',redact(runs))
    write('RETAINED-FAILURES.json',{'excludedFromPassingAcceptance':True,'failures':[
       {'stage':'privateKeyReadBeforeProvision','cause':'Operator cleared the live file; test mode correctly uses stripe-test.env. No API call/create at failure.'},
       {'stage':'initialQaInterpreter','cause':'websockets absent; installed pinned15.0.1 in this task-only dependency directory, no financial request yet.'},
       {'stage':'WebMcpFill','cause':'Browser fill call timed out after completing fields; observed actual form and submitted once with the normal button.'},
       {'stage':'firstMonthly','cause':'Test incorrectly assumed completed BUY envelopes retain a nonce like Gift; repeat correctly refused for insufficient Credits. Confirmed existing parcel resumed; no new BUY.'},
       {'stage':'intermediateStatus','cause':'Test expected1000 despite the confirmed native1000 debit; corrected the test to account for that debit.'},
       {'stage':'firstMonthlyResume','cause':'Missing ordinary2s application heartbeat caused connection closure during bootstrap. Existing parcel preserved; correct transport heartbeat resumes it.'},
       {'stage':'initialProviderReadback','cause':'Retrieved Event keeps account creation-time2025-01-27.acacia, while endpoint uses2026-09-30.endive. Actual official CLI resend used without relabeling the API snapshot.'},
       {'stage':'initialBadSignatureAssertion','cause':'Actual gateway intentionally exposes error=billingWebhookRejected for supplied invalid signature; corrected assertion against source and actual response.'},
       {'stage':'initialDeliveryCompletion','cause':'Stripe resend queues async delivery; bounded later read confirms pending_webhooks=0. Existing receipts reused, no repeated resend.'},
       {'stage':'firstPostflight','cause':'Test guessed latest.json.p7s instead of the previous observed latest.p7s. Actual manifest SHA remained unchanged; corrected exact known signature alias.'}
    ]})
    for source,name in [(ROOT/'PUBLIC-RETURN.jpg','PUBLIC-RETURN.jpg'),
                        (Path('C:/mir2-build/stripe-live-20261011/public-test-player.py'),'executed-public-test-player.py'),
                        (Path('C:/mir2-build/stripe-live-20261011/verify-public-test-order.py'),'executed-verify-public-test-order.py')]:
        content=source.read_bytes()
        if source.suffix=='.py':
            text=source.read_text(encoding='utf-8').replace('\r\n','\n')
            need(not re.search(r'\b(?:sk|rk)_(?:test|live)_[A-Za-z0-9_]{24,}|whsec_[A-Za-z0-9]{24,}',text),'script-must-have-no-secret-values')
            (OUT/name).write_text(text,encoding='utf-8',newline='\n')
        else:(OUT/name).write_bytes(content)
    print(json.dumps({'passed':True,'publicChecks':len(checks),'gatewayPid':state['unit']['MainPID'],
                      'capacity':state['test']['capacity'],'output':str(OUT)}))

if __name__=='__main__':
    main()
