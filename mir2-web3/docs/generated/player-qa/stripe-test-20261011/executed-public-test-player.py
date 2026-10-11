"""Owned ordinary public WSS Stripe-test trial; no admin/provider payment APIs."""
import argparse
import asyncio
import hashlib
import importlib.util
import inspect
import json
import os
from pathlib import Path
import subprocess
import sys
import urllib.request

HELPER = Path('C:/mir2-build/stripe-sandbox-20261010/qa-player.py')
PIN = 'd57c9eeea7bc1cbb31f0cf23ca0e6dc3999bcbdb02ec5bce355eb33a53680ea0'
ROOT = Path('C:/Users/Administrator/.mir2-private/stripe-public-test-trial-20261011')
COHORT = Path('C:/mir2-playtest-releases/20261007-remote-player-r21/public-wss-03/private/accounts.json')
REVISION = '4006d988b3922d0106278720abfa9b3d384c8d5e'
BASE = 'https://165.154.65.136.sslip.io/playtest'
WS = 'wss://165.154.65.136.sslip.io/playtest/ws'
assert hashlib.sha256(HELPER.read_bytes()).hexdigest() == PIN
spec = importlib.util.spec_from_file_location('pinned_local_qa', HELPER)
qa = importlib.util.module_from_spec(spec)
spec.loader.exec_module(qa)
need = qa.need


class Player(qa.Player):
    def credit_result(self, status):
        if self.state.get('nativeMonthly',{}).get('receipt',{}).get('success') is True:
            intent = self.state['checkout']
            need(next(o['state'] for o in status['orders'] if o['id']==intent['orderId'])=='applied'
                 and status['credit']==0 and status['pendingCredit']==0, 'nativeShopDebitSettlementMismatch')
            self.receipt('explicitNativeSettlementStatus', sourceApplied=True, credit=0, pendingCredit=0)
            return True
        return super().credit_result(status)

    async def until(self, predicate, label):
        def remembered(event):
            self.raw.append(event)
            return predicate(event)
        return await super().until(remembered, label)

    async def monthly_shop(self):
        before = await self.status()
        need(self.credit_result(before), 'checkoutMustBeSourceApplied')
        intent = self.state.get('nativeMonthly')
        if intent is None:
            need(before['credit'] == 1000 and not before['monthlyCard']['active']
                 and not before['ownedMonthlyCards'], 'initialPublicMonthlyStateUnexpected')
            intent = {'requestId':'public-stripe-test-monthly-20261011-01', 'before':before}
            self.state['nativeMonthly'] = intent
            self.save()
        need(not intent.get('completed'), 'monthlyCompleteUseStatusInstead')
        await self.send({'type':'clientCapabilities','capabilities':['nativeGameShopReceiptV1','nativeGameShopGiftReceiptV1']})
        await self.send({'type':'startGame','characterIndex':self.state['characterIndex']})
        entered = await self.packet('StartGame')
        self.active = entered['payload']['result'] == 4
        need(self.active, 'normalPublicStartGameRejected')
        await self.barrier('PublicShopBootstrapBarrier')
        catalogue = [e['payload']['item'] for e in self.raw if e.get('packet')=='GameShopInfo'
                     and e.get('payload',{}).get('item',{}).get('g_index')==1000001]
        need(catalogue and catalogue[-1]['credit_price']==1000
             and catalogue[-1]['can_buy_credit'] and not catalogue[-1]['can_buy_gold'],
             'ordinaryPublicMonthlyCatalogueMismatch')
        command = {'type':'gameShopBuy','requestId':intent['requestId'],
                   'gIndex':1000001,'quantity':1,'priceType':0}
        receipt = intent.get('receipt')
        if receipt is None:
            need(not intent.get('sent'), 'unknownShopBuyMustReconcileNoAutomaticResend')
            intent['sent'] = True
            self.save()
            await self.send(command)
            receipt = await self.until(lambda e:e.get('type')=='gameShopReceipt'
                                      and e.get('requestId')==intent['requestId'], 'NativeMonthlyShopReceipt')
        need(receipt.get('success') is True and receipt.get('protocol')=='nativeGameShopReceiptV1'
             and receipt.get('gIndex')==1000001 and receipt.get('quantity')==1
             and receipt.get('priceType')==0 and type(receipt.get('mailId')) is int,
             'nativeMonthlyPurchaseRejected')
        need(intent.get('receipt',receipt)==receipt, 'immutableShopReceiptChanged')
        intent['receipt'] = receipt
        self.save()
        await self.barrier('PurchaseMailBarrier')
        mail = [m for e in self.raw if e.get('packet')=='ReceiveMail'
                for m in e.get('payload',{}).get('mail',[]) if m.get('mailId')==receipt['mailId']]
        need(mail and mail[-1].get('senderName')=='Gameshop' and not mail[-1].get('collected')
             and len(mail[-1].get('items',[]))==1, 'purchasedParcelMissingRealItem')
        key = mail[-1]['items'][0]['unique_id']
        need(type(key) is int and 0 < key < 2**64 and mail[-1]['items'][0]['item_index']==1000001,
             'monthlyParcelItemIdentityMismatch')
        intent['itemUniqueId'] = str(key)
        self.save()
        bought = await self.status()
        need(bought['credit']==0 and bought['pendingCredit']==0 and not bought['monthlyCard']['active'],
             'monthlyDebitedOrActivatedIncorrectly')
        self.receipt('nativeMonthlyPurchased', mailId=receipt['mailId'], credit=0,
                     itemUniqueId=str(key), realParcelItemPresent=True, activatedOnPurchase=False)
        # Ordinary BUY uses a new server nonce per completed request. Only the
        # gift protocol binds retries; never pretend BUY requestId is that key.
        # A confirmed purchase resumes from its existing parcel, not a new BUY.
        self.receipt('confirmedBuyResumedFromExistingParcel', noAutomaticResend=True)
        await self.send({'type':'collectParcel','mailId':receipt['mailId']})
        collected = await self.packet('ParcelCollected')
        need(collected.get('payload',{}).get('result')==1, 'ordinaryPurchasedParcelClaimFailed')
        await self.barrier('ParcelClaimBarrier')
        gained = [e for e in self.raw if e.get('packet')=='GainedItem'
                  and e.get('payload',{}).get('item',{}).get('unique_id')==key]
        need(len(gained)==1, 'parcelClaimDidNotYieldExactOneCard')
        claimed = await self.status()
        need(claimed['credit']==0 and claimed['ownedMonthlyCards']==[{'uniqueId':str(key)}],
             'claimedExactCardNotInInventory')
        intent['activationLowerServerNowMs'] = claimed['monthlyCard']['serverNowMs']
        intent['activationSent'] = True
        self.save()
        await self.send({'type':'useItem','uniqueId':key,'grid':'inventory'})
        used = await self.until(lambda e:e.get('packet')=='UseItem'
                               and e.get('payload',{}).get('uniqueId')==key, 'OrdinaryMonthlyUseItem')
        need(used['payload'].get('success') is True, 'ordinaryMonthlyUseFailed')
        activated = await self.status()
        month = activated['monthlyCard']
        need(activated['credit']==0 and not activated['ownedMonthlyCards'] and month['active']
             and month['canEnterGame'] and not month['required'], 'monthlyUsageStateMismatch')
        expiry = month['expiresAtMs']
        need(intent['activationLowerServerNowMs']+qa.MONTH_MS <= expiry <= month['serverNowMs']+qa.MONTH_MS,
             'monthlyThirtyDaysMismatch')
        intent['expiresAtMs'] = expiry
        self.save()
        _, replayed = await self.billing('activateMonthlyCard','billingActivateMonthlyCard',
                                        {'itemUniqueId':str(key)})
        need(replayed['credit']==0 and not replayed['ownedMonthlyCards']
             and replayed['monthlyCard']['expiresAtMs']==expiry, 'consumedCardReplayChangedExpiry')
        self.receipt('nativeMonthlyLifecycleConfirmed', credit=0, itemUniqueId=str(key),
                     expiresAtMs=expiry, durationMs=qa.MONTH_MS, confirmedBuyResumedWithoutResend=True,
                     consumedUidReplayPreserved=True, nativeVisualAcceptance=False)
        await self.logout()
        intent['completed'] = True
        self.save()
        self.report['finalStatus'] = replayed


async def run(phase):
    from websockets import connect
    need(os.environ.get('NODE_TLS_REJECT_UNAUTHORIZED') != '0', 'normalTlsRequired')
    with urllib.request.urlopen(BASE+'/health', timeout=15) as response:
        health = json.load(response)
    need(health.get('ok') is True and health.get('revision')==REVISION, 'gatewayRevisionMismatch')
    need(health['capacity']['currentActiveSessions'] < 50, 'ordinaryCapacityRequired')
    if not ROOT.exists():
        ROOT.mkdir()
        who = subprocess.check_output(['whoami.exe'],text=True).strip()
        done = subprocess.run(['icacls.exe',str(ROOT),'/inheritance:r','/grant:r',who+':(OI)(CI)(F)',
                               '*S-1-5-18:(OI)(CI)(F)'],capture_output=True,timeout=10)
        need(done.returncode==0, 'privateTrialAclFailed')
    paths = {k:ROOT/v for k,v in {'statePath':'state.json','checkoutPath':'checkout.json','reportPath':'player-report.json'}.items()}
    account = json.loads(COHORT.read_text(encoding='utf-8'))['cohort'][0]
    config = {'accountId':account['accountId'],'password':account['password'],
              'character':{k:account[k] for k in ('name','class','gender')}}
    state = json.loads(paths['statePath'].read_text()) if paths['statePath'].exists() else {
        'accountId':account['accountId'],'characterIndex':account['characterIndex'],'nextRequestId':110001}
    need(state['accountId']==account['accountId'] and state['characterIndex']==account['characterIndex'],
         'ownedTrialIdentityMismatch')
    with qa.exclusive_run(paths['statePath']):
        player = Player(config,state,paths,phase)
        player.raw = []
        direct = {'proxy':None} if 'proxy' in inspect.signature(connect).parameters else {}
        try:
            async with connect(WS, origin='https://165.154.65.136.sslip.io',open_timeout=15,
                               close_timeout=5,max_size=8*1024*1024,max_queue=64,
                               ping_interval=15,ping_timeout=10,**direct) as socket:
                player.ws = socket
                async def keep_alive():
                    import time
                    while True:
                        await asyncio.sleep(2)
                        await player.send({'type':'keepAlive','time':time.time_ns()//1000000})
                heartbeat = asyncio.create_task(keep_alive())
                try:
                    await player.send({'type':'clientVersion'})
                    characters = await player.login()
                    need(any(c.get('index')==state['characterIndex'] and c.get('name')==config['character']['name']
                             for c in characters), 'ownedExistingCharacterRequiredNoCreation')
                    if phase=='checkout':
                        before = await player.status()
                        need(before['checkoutEnabled'] and before['monthlyCardCreditPrice']==1000
                             and not before['monthlyCard']['required'], 'authorizedCatalogMismatch')
                        need([(o['amountMinor'],o['credits']) for o in before['offers']]==
                             [(100,100),(500,500),(1000,1000),(2000,2000)], 'fourUsdOffersMismatch')
                        if 'checkout' not in state:
                            need(before['credit']==0 and not before['orders'] and not before['ownedMonthlyCards']
                                 and not before['monthlyCard']['active'], 'ownedActorInitialEconomyNotEmpty')
                        await player.checkout(before)
                    elif phase=='shop-replay':
                        await player.send({'type':'clientCapabilities','capabilities':['nativeGameShopReceiptV1','nativeGameShopGiftReceiptV1']})
                        await player.send({'type':'startGame','characterIndex':state['characterIndex']})
                        player.active = (await player.packet('StartGame'))['payload']['result']==4
                        need(player.active, 'normalShopReplayStartGameFailed')
                        intent=state['nativeMonthly']
                        await player.send({'type':'gameShopBuy','requestId':intent['requestId'],'gIndex':1000001,'quantity':1,'priceType':0})
                        observed=await player.until(lambda e:e.get('type')=='gameShopReceipt' and e.get('requestId')==intent['requestId'], 'DiagnosticSameShopIntent')
                        player.receipt('diagnosticNativeReplay', expected=intent['receipt'], observed=observed)
                        print(json.dumps({'expected':intent['receipt'],'observed':observed}))
                    elif phase=='monthly':
                        await player.monthly_shop()
                    else:
                        status = await player.status()
                        if state.get('nativeMonthly',{}).get('completed'):
                            need(status['credit']==0 and status['pendingCredit']==0 and not status['ownedMonthlyCards']
                                 and status['monthlyCard']['expiresAtMs']==state['nativeMonthly']['expiresAtMs']
                                 and next(o['state'] for o in status['orders'] if o['id']==state['checkout']['orderId'])=='applied',
                                 'reloginFinancialStateChanged')
                            player.receipt('reloginPreservedMonthlyWalletAndOrder', confirmed=True)
                        else:
                            player.credit_result(status)
                        player.report['finalStatus'] = status
                    if player.active or phase!='monthly':
                        await player.logout()
                except BaseException:
                    try: await player.logout()
                    except BaseException: pass
                    raise
                finally:
                    heartbeat.cancel()
                    try: await heartbeat
                    except BaseException: pass
            player.report['ok'] = True
        except BaseException as error:
            player.report['error'] = str(error) if isinstance(error,qa.QAError) else type(error).__name__
            raise
        finally:
            qa.private_json(paths['reportPath'],player.evidence)
        final = player.report.get('finalStatus',{})
        print(json.dumps({'passed':True,'phase':phase,'sourceRevision':REVISION,'tlsVerified':True,
                          'privateEvidenceSaved':True,'credit':final.get('credit'),
                          'pendingCredit':final.get('pendingCredit'),'monthlyCard':final.get('monthlyCard')}))


if __name__=='__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--phase',required=True,choices=['checkout','status','monthly','shop-replay'])
    args = parser.parse_args()
    try: asyncio.run(run(args.phase))
    except BaseException as error:
        print(json.dumps({'passed':False,'error':str(error) if isinstance(error,qa.QAError) else type(error).__name__}))
        sys.exit(1)
