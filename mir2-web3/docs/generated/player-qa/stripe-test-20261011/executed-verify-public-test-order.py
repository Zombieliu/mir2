"""Read actual owned sandbox objects and replay their signed completed event."""
import datetime
import hashlib
import hmac
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time
import urllib.error
import urllib.request
from urllib.parse import urlsplit

ROOT=Path('C:/Users/Administrator/.mir2-private')
TRIAL=ROOT/'stripe-public-test-trial-20261011'
OUT=Path('C:/mir2-playtest-releases/20261011-stripe-test/PROVIDER-PAID-READBACK.json')
VERSION='2026-09-30.endive'
CALLBACK='https://165.154.65.136.sslip.io/playtest/v1/billing/stripe/webhook'
class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self,*args):return None
class Stop(Exception):pass
def need(value,message):
    if not value:raise Stop(message)
def sha(value):return hashlib.sha256(value.encode()).hexdigest()

def main():
    need(sys.argv[1:]==['--verify-and-replay-test-event'], 'explicit-test-verification-required')
    env={}
    for line in (ROOT/'stripe-public-test.env').read_text(encoding='utf-8-sig').splitlines():
        if line and not line.startswith('#'):
            k,v=line.split('=',1);env[k]=v.strip().strip("'").strip('"')
    key=env['MIR2_STRIPE_SECRET_KEY'];secret=env['MIR2_STRIPE_WEBHOOK_SECRET']
    need(re.fullmatch(r'(sk|rk)_test_[A-Za-z0-9_]+',key) and secret.startswith('whsec_'),'test-only-private-credentials-required')
    state=json.loads((TRIAL/'state.json').read_text())
    checkout=json.loads((TRIAL/'checkout.json').read_text())
    url=urlsplit(checkout['checkoutUrl'])
    need(url.scheme=='https' and url.netloc=='checkout.stripe.com','owned-stripe-checkout-required')
    match=re.fullmatch(r'/c/pay/(cs_test_[A-Za-z0-9]+)',url.path)
    need(match is not None,'test-checkout-identity-required');sid=match.group(1)
    opener=urllib.request.build_opener(NoRedirect())
    def api(path):
        request=urllib.request.Request('https://api.stripe.com/v1/'+path,
           headers={'Authorization':'Bearer '+key,'Stripe-Version':VERSION},method='GET')
        with opener.open(request,timeout=20) as response:
            raw=response.read(1024*1024+1)
            need(response.status==200 and len(raw)<=1024*1024,'bounded-provider-read-required')
            return json.loads(raw)
    session=api('checkout/sessions/'+sid)
    expected={'mir2_order_id':checkout['orderId'],'mir2_account_id':state['accountId'],
              'mir2_character_index':str(state['characterIndex'])}
    need(session['id']==sid and session['livemode'] is False and session['status']=='complete'
         and session['payment_status']=='paid' and session['amount_total']==1000 and session['currency']=='usd'
         and session['client_reference_id']==checkout['orderId']
         and all(session['metadata'].get(k)==v for k,v in expected.items()),'actual-paid-owned-checkout-mismatch')
    pid=session['payment_intent']
    need(re.fullmatch(r'pi_[A-Za-z0-9]+',pid),'actual-payment-intent-required')
    intent=api('payment_intents/'+pid)
    need(intent['livemode'] is False and intent['status']=='succeeded' and intent['currency']=='usd'
         and intent['amount']==intent['amount_received']==1000
         and all(intent['metadata'].get(k)==v for k,v in expected.items()),'actual-succeeded-test-intent-mismatch')
    events=api('events?type=checkout.session.completed&limit=30')
    matching=[e for e in events['data'] if e.get('data',{}).get('object',{}).get('id')==sid]
    need(len(matching)==1,'actual-exact-checkout-completed-event-required')
    event=api('events/'+matching[0]['id'])
    provision=json.loads((ROOT/'stripe-public-test-provision.json').read_text())
    endpoint=api('webhook_endpoints/'+provision['endpointId'])
    need(endpoint['livemode'] is False and endpoint['status']=='enabled'
         and endpoint['api_version']==VERSION and endpoint['url']==CALLBACK,'exact-test-endpoint-required')
    need(event['livemode'] is False
         and event['type']=='checkout.session.completed' and not event.get('account'),'matching-signed-event-mode-required')
    def post(body,valid=True):
        raw=json.dumps(body,separators=(',',':')).encode();now=str(int(time.time()))
        digest=hmac.new(secret.encode(),now.encode()+b'.'+raw,hashlib.sha256).hexdigest() if valid else '0'*64
        request=urllib.request.Request(CALLBACK,data=raw,
            headers={'Content-Type':'application/json','Stripe-Signature':'t='+now+',v1='+digest},method='POST')
        try:
            response=opener.open(request,timeout=20)
        except urllib.error.HTTPError as error:response=error
        with response:
            result=response.read(65537);need(len(result)<=65536,'bounded-callback-response-required')
            return response.status,json.loads(result),response.headers.get('Cache-Control')
    outcomes=[]
    status,body,cache=post(event,False)
    need(status==400 and body.get('error')=='billingWebhookRejected' and cache=='no-store', 'invalid-signature-must-refuse')
    outcomes.append({'kind':'invalidSignature','httpStatus':status,'error':body['error'],'cacheControl':cache})
    wrong=dict(event);wrong['livemode']=True;wrong['api_version']=VERSION
    status,body,cache=post(wrong)
    need(status==400 and body.get('error')=='billingWebhookRejected' and cache=='no-store','signed-wrong-live-mode-must-refuse')
    outcomes.append({'kind':'wrongLiveMode','httpStatus':status,'error':body.get('error'),'cacheControl':cache})
    # GET Event retains the account's creation-time version. The permanent
    # endpoint receives its own version. Ask Stripe to resend its actual event
    # rather than relabel/re-sign an older API snapshot as a genuine callback.
    cli=Path('C:/mir2-build/stripe-sandbox-20261010/stripe-cli-1.53.1/stripe.exe')
    need(cli.is_file() and not cli.is_symlink(),'previously-qualified-official-cli-required')
    child=os.environ.copy();child['STRIPE_API_KEY']=key
    for number in (1,2):
        log=TRIAL/('provider-resend-'+str(number)+'.private.log')
        if log.exists():
            repeated,_=json.JSONDecoder().raw_decode(log.read_text(encoding='utf-8').lstrip())
        else:
            result=subprocess.run([str(cli),'events','resend',event['id'],
                '--webhook-endpoint='+endpoint['id'],'--stripe-version='+VERSION,'--confirm',
                '--color','off','--config',str(TRIAL/'stripe-resend-config.toml')],
                env=child,capture_output=True,text=True,timeout=30,
                creationflags=subprocess.CREATE_NO_WINDOW)
            log.write_text(result.stdout+'\n'+result.stderr,encoding='utf-8')
            need(result.returncode==0,'official-test-event-resend-failed')
            repeated=json.loads(result.stdout)
        need(repeated.get('id')==event['id'] and repeated.get('livemode') is False,
             'resend-must-return-original-test-event')
        outcomes.append({'kind':'providerActualCompletedResend','number':number,
            'officialCliExitCode':0,'sameEventIdentity':True,'endpointApiVersion':VERSION})
    for attempt in range(10):
        settled=api('events/'+event['id'])
        if settled['pending_webhooks']==0:break
        if attempt<9:time.sleep(2)
    need(settled['pending_webhooks']==0,'provider-delivery-not-yet-complete')
    need(state.get('nativeMonthly',{}).get('completed') is True,'ordinary-native-monthly-lifecycle-required')
    report={'schema':'mir2.public-stripe-test.provider-verification.v1','passed':True,
       'verifiedAtUtc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'sourceRevision':'4006d988b3922d0106278720abfa9b3d384c8d5e',
       'apiVersion':VERSION,'livemode':False,'realMoneyTransferred':False,'providerReads':6,
       'checkout':{'idSha256':sha(sid),'status':'complete','paymentStatus':'paid','amountTotal':1000,'currency':'usd','frozenOwnerTupleMatches':True},
       'paymentIntent':{'idSha256':sha(pid),'status':'succeeded','amount':1000,'amountReceived':1000,'currency':'usd','frozenOwnerTupleMatches':True},
       'providerEvent':{'idSha256':sha(event['id']),'type':event['type'],'pendingWebhooks':settled['pending_webhooks'],
           'retrievedEventApiVersion':event['api_version'],'endpointApiVersion':endpoint['api_version'],
           'officialCliSha256':hashlib.sha256(cli.read_bytes()).hexdigest()},
       'publicCallbackOutcomes':outcomes,'monthlyCard':{'priceCredits':1000,'durationMs':2592000000,
           'ordinaryNativeShopParcelClaimAndUse':True,'expiresAtMs':state['nativeMonthly']['expiresAtMs']},
       'scopeLimits':['No live money','Native visual human acceptance remains open','No actual refund/dispute trial','No Gateway cold restart after financial trial']}
    OUT.write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8',newline='\n')
    print(json.dumps(report))

if __name__=='__main__':
    try:main()
    except Exception as error:
        print(json.dumps({'passed':False,'error':str(error) if isinstance(error,Stop) else type(error).__name__}));sys.exit(1)
