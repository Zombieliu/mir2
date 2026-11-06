#!/usr/bin/env python3
"""Postflight for an operator-owned, already paid USD1 sandbox browser trial.

No payment creation, callback replay, admin command, player provisioning or
direct save access. The browser must sign out before the ordinary game trial.
Private inputs remain outside the repository; stdout/report contain hashes and
allowlisted facts only. The sole POST Checkout probe targets a foreign owned
QA character and must fail before an order is created.
"""
import argparse
import asyncio
import datetime
import hashlib
import json
from pathlib import Path
import re
import sys
import time
import traceback
import urllib.error
import urllib.request
from urllib.parse import urlsplit
import uuid

ORIGIN = "https://165.154.65.136.sslip.io"
API = ORIGIN + "/playtest/v1/billing"
VERSION = "2026-09-30.endive"


class Stop(Exception):
    pass


def need(value, code):
    if not value:
        raise Stop(code)


def sha(value):
    return hashlib.sha256(value.encode() if isinstance(value, str) else value).hexdigest()


def read(path):
    return json.loads(Path(path).read_text(encoding="utf-8-sig"))


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *args):
        return None


def http(url, body=None, token=None, headers=None):
    values = {"Accept": "application/json", **(headers or {})}
    if token:
        values["Authorization"] = "Bearer " + token
    if body is not None:
        values["Content-Type"] = "application/json"
    request = urllib.request.Request(url, headers=values,
        data=json.dumps(body).encode() if body is not None else None)
    opener = urllib.request.build_opener(NoRedirect())
    try:
        response = opener.open(request, timeout=20)
    except urllib.error.HTTPError as error:
        response = error
    with response:
        raw = response.read(1024 * 1024 + 1)
        need(len(raw) <= 1024 * 1024, "httpResponseTooLarge")
        return response.status, raw, dict(response.headers.items())


def json_http(url, **kwargs):
    status, raw, headers = http(url, **kwargs)
    return status, json.loads(raw), {k.lower(): v for k, v in headers.items()}


class Player:
    def __init__(self, account):
        self.account = account
        self.token = None
        self.session_id = None
        self.active = False
        self.raw = []

    async def send(self, value):
        await asyncio.wait_for(self.socket.send(json.dumps(value)), 10)

    async def until(self, predicate, code):
        end = asyncio.get_running_loop().time() + 25
        for _ in range(4096):
            remaining = end - asyncio.get_running_loop().time()
            need(remaining > 0, code)
            value = json.loads(await asyncio.wait_for(self.socket.recv(), remaining))
            need(value.get("type") != "error", "gatewayErrorRedacted")
            self.raw.append(value)
            if predicate(value):
                return value
        raise Stop("gatewayFrameBudgetReached")

    async def barrier(self):
        stamp = time.time_ns() // 1000000
        await self.send({"type": "keepAlive", "time": stamp})
        await self.until(lambda e: e.get("packet") == "KeepAlive"
                         and e.get("payload", {}).get("time") == stamp, "barrierTimeout")

    async def login(self):
        await self.send({"type": "login", "accountId": self.account["accountId"],
                         "password": self.account["password"]})
        event = await self.until(lambda e: e.get("packet") in
            {"LoginSuccess", "Login", "LoginBanned"}, "loginTimeout")
        need(event.get("packet") == "LoginSuccess", "ownedPasswordLoginFailed")
        characters = event["payload"]["characters"]
        need(any(c["index"] == self.account["characterIndex"] and c["name"] == self.account["name"]
                 for c in characters), "ownedExistingCharacterMissing")
        await self.barrier()
        grants = [e for e in self.raw if e.get("type") == "identitySession"]
        need(len(grants) == 1 and grants[0]["session"]["accountId"] == self.account["accountId"]
             and grants[0]["session"]["authMethod"] == "password"
             and grants[0]["session"]["revokedAtMs"] is None, "identityGrantMismatch")
        self.token = grants[0]["token"]
        self.session_id = grants[0]["session"]["sessionId"]

    async def logout(self):
        await self.send({"type": "logOut"})
        if self.active:
            await self.until(lambda e: e.get("packet") == "LogOutSuccess", "logoutTimeout")
            self.active = False
        else:
            await self.barrier()
        code, body, _ = await asyncio.to_thread(json_http, ORIGIN + "/playtest/v1/identity/sessions/revoke",
            token=self.token, body={"sessionId": self.session_id, "reason": "recharge_website_qa_sign_out"})
        need(code == 200 and body.get("accepted") is True and body.get("affected") == 1,
             "ownWebsiteSessionRevokeFailed")
        from websockets.exceptions import ConnectionClosed
        try:
            await self.send({"type": "disconnect"})
            # Confirm FIFO processing if revocation hasn't already closed WSS.
            await self.barrier()
        except ConnectionClosed:
            # The own identity was explicitly revoked above. Active game
            # logout/save already completed before that revocation.
            pass


async def game_trial(account, order_id, expiry):
    from websockets import connect
    receipts = []
    for number in (1, 2):
        player = Player(account)
        async with connect(ORIGIN.replace("https:", "wss:") + "/playtest/ws", origin=ORIGIN,
                           proxy=None, open_timeout=15, close_timeout=5,
                           max_size=8 * 1024 * 1024, max_queue=64) as socket:
            player.socket = socket
            async def heartbeat():
                while True:
                    await asyncio.sleep(2)
                    await player.send({"type": "keepAlive", "time": time.time_ns() // 1000000})
            task = asyncio.create_task(heartbeat())
            try:
                await player.login()
                code, status, _ = await asyncio.to_thread(json_http,
                    API + f"/status?characterIndex={account['characterIndex']}&requestId={9000+number}",
                    token=player.token)
                need(code == 200 and status["credit"] == 100 and status["pendingCredit"] == 0
                     and status["monthlyCard"]["expiresAtMs"] == expiry
                     and next(o["state"] for o in status["orders"] if o["id"] == order_id) == "applied",
                     "persistedWalletMismatch")
                await player.send({"type": "startGame", "characterIndex": account["characterIndex"]})
                entered = await player.until(lambda e: e.get("packet") == "StartGame", "startGameTimeout")
                player.active = entered["payload"]["result"] == 4
                need(player.active, "ordinaryStartGameRejected")
                await player.barrier()
                info = [e["payload"] for e in player.raw if e.get("packet") == "UserInformation"]
                need(info and info[-1]["credit"] == 100 and info[-1]["name"] == account["name"],
                     "gameBootstrapCreditMismatch")
                await player.logout()
                receipts.append({"round": number, "passwordLogin": True, "sourceCredit": 100,
                    "userInformationCredit": 100, "ordinaryStartGame": True, "ordinaryLogout": True,
                    "orderApplied": True, "monthlyExpiryPreserved": True})
            finally:
                if player.active:
                    await player.logout()
                task.cancel()
                try:
                    await task
                except asyncio.CancelledError:
                    pass
    return receipts


async def ownership_trial(owner, foreign):
    from websockets import connect
    player = Player(foreign)
    async with connect(ORIGIN.replace("https:", "wss:") + "/playtest/ws", origin=ORIGIN,
                       proxy=None, open_timeout=15, close_timeout=5, max_size=8 * 1024 * 1024) as socket:
        player.socket = socket
        await player.login()
        idx = foreign["characterIndex"]
        async def own_status():
            code, body, _ = await asyncio.to_thread(json_http,
                API + f"/status?characterIndex={idx}&requestId=9010", token=player.token)
            need(code == 200, "foreignActorOwnStatusFailed")
            return body
        before = await own_status()
        target = owner["characterIndex"]
        code, body, _ = await asyncio.to_thread(json_http,
            API + f"/status?characterIndex={target}&requestId=9011", token=player.token)
        need(code == 503 and body.get("error") == "billingCharacterMissing"
             and set(body) == {"error"}, "crossAccountStatusNotRejected")
        quote = before["offers"][0]
        code, body, _ = await asyncio.to_thread(json_http, API + "/checkout", token=player.token,
            body={"characterIndex": target, "requestId": 9012,
                  "orderRequestId": "web-" + str(uuid.uuid4()), "offerId": quote["id"]})
        need(code == 503 and body.get("error") == "billingCharacterMissing"
             and set(body) == {"error"}, "crossAccountCheckoutNotRejected")
        after = await own_status()
        need(before["orders"] == after["orders"] and before["credit"] == after["credit"]
             and before["pendingCredit"] == after["pendingCredit"], "foreignActorStateChanged")
        other = Player(owner)
        async with connect(ORIGIN.replace("https:", "wss:") + "/playtest/ws", origin=ORIGIN,
                           proxy=None, open_timeout=15, close_timeout=5, max_size=8 * 1024 * 1024) as other_socket:
            other.socket = other_socket
            await other.login()
            code, receipt, _ = await asyncio.to_thread(json_http,
                ORIGIN + "/playtest/v1/identity/sessions/revoke", token=player.token,
                body={"sessionId": other.session_id, "reason": "cross_account_negative_qa"})
            need(code == 200 and receipt == {"accepted": False, "affected": 0}, "foreignSessionRevoked")
            code, value, _ = await asyncio.to_thread(json_http,
                API + f"/status?characterIndex={target}&requestId=9014", token=other.token)
            need(code == 200 and value["credit"] == 100, "otherAccountGrantChanged")
            await other.logout()
        old_token = player.token
        await player.logout()
    code, body, _ = await asyncio.to_thread(json_http,
        API + f"/status?characterIndex={idx}&requestId=9013", token=old_token)
    need(code == 401 and body.get("error") == "billingAuthenticationRequired", "logoutTokenStillAccepted")
    return {"foreignStatus": 503, "foreignCheckout": 503, "noFinancialStateChange": True,
            "foreignSessionRevoke": {"accepted": False, "affected": 0},
            "otherAccountGrantPreserved": True, "disconnectedToken": 401}


def public_probes(manifest_dir):
    manifest_bytes = (manifest_dir / "release.json").read_bytes()
    manifest = json.loads(manifest_bytes)
    receipts = []
    for file in manifest["files"] + [{"name": "release.json", "sha256": sha(manifest_bytes)}]:
        status, raw, headers = http(ORIGIN + "/recharge/" + file["name"])
        headers = {k.lower(): v for k, v in headers.items()}
        need(status == 200 and sha(raw) == file["sha256"] and headers.get("cache-control") == "no-store"
             and headers.get("referrer-policy") == "no-referrer"
             and headers.get("x-content-type-options") == "nosniff"
             and headers.get("x-frame-options") == "DENY"
             and "frame-ancestors 'none'" in headers.get("content-security-policy", ""),
             "staticPublicationOrHeadersMismatch")
        mime = headers.get("content-type", "").split(";")[0]
        expected_mime = {".html": {"text/html"}, ".css": {"text/css"},
            ".js": {"text/javascript", "application/javascript"}, ".svg": {"image/svg+xml"},
            ".json": {"application/json"}}[Path(file["name"]).suffix]
        need(mime in expected_mime, "staticMimeMismatch")
        receipts.append({"path": "/recharge/" + file["name"], "status": status,
                         "sha256": sha(raw), "bytes": len(raw), "privateHeaders": True})
    status, _, headers = http(ORIGIN + "/recharge")
    need(status == 308 and headers.get("Location") == "/recharge/", "canonicalUrlMismatch")
    need(http(ORIGIN + "/recharge/not-present.js")[0] == 404, "missingFileFallbackWrong")
    target = API + "/status?characterIndex=26&requestId=1"
    for token in (None, "forged-session"):
        code, body, headers = json_http(target, token=token)
        need(code == 401 and body.get("error") == "billingAuthenticationRequired"
             and headers.get("cache-control") == "no-store", "anonymousBillingStatusAccepted")
    probe = {"characterIndex": 26, "requestId": 1, "orderRequestId": "web-" + str(uuid.uuid4()), "offerId": "usd1"}
    code, body, _ = json_http(API + "/checkout", body=probe)
    need(code == 401 and body.get("error") == "billingAuthenticationRequired", "anonymousCheckoutAccepted")
    amount_code, _, _ = http(API + "/checkout", body={**probe, "amountMinor": 1, "credits": 99999999})
    need(amount_code in (400, 422), "clientQuotedAmountNotRejected")
    blocked = ["/playtest/admin", "/playtest/v1/identity/session", "/playtest/v1/billing/buyMonthlyCard",
               "/playtest/v1/billing/activateMonthlyCard"]
    need(all(http(ORIGIN + path)[0] == 404 for path in blocked), "privateNeighborExposed")
    code, body, _ = json_http(API + "/stripe/webhook", body={})
    need(code == 400 and body.get("error") == "billingWebhookSignatureInvalid", "unsignedWebhookAccepted")
    code, _, _ = json_http(ORIGIN + "/playtest/v1/identity/sessions/revoke",
                          body={"sessionId": "not-an-owned-session", "reason": "anonymous_qa"})
    need(code == 401, "anonymousSessionRevokeAccepted")
    return {"files": receipts, "releaseSha256": sha(manifest_bytes), "canonicalRedirect": 308,
        "missingFile": 404, "anonymousStatus": 401, "forgedToken": 401, "anonymousCheckout": 401,
        "clientQuotedAmount": amount_code, "unsignedWebhook": 400, "anonymousSessionRevoke": 401,
        "privateNeighbors": [{"path": path, "status": 404} for path in blocked]}


def provider_trial(env_path, cohort, trial):
    values = {}
    for line in env_path.read_text(encoding="utf-8-sig").splitlines():
        if line and not line.startswith("#"):
            k, v = line.split("=", 1)
            values[k] = v.strip().strip("'").strip('"')
    key = values["MIR2_STRIPE_SECRET_KEY"]
    need(re.fullmatch(r"(sk|rk)_test_[A-Za-z0-9_]+", key), "testKeyRequired")
    before, after, checkout = (read(trial / name) for name in ["before.json", "after.json", "checkout.json"])
    url = urlsplit(checkout["url"])
    need(url.scheme == "https" and url.netloc == "checkout.stripe.com", "ownedCheckoutHostMismatch")
    match = re.fullmatch(r"/c/pay/(cs_test_[A-Za-z0-9]+)", url.path)
    need(match, "sandboxCheckoutRequired")
    sid = match.group(1)
    def stripe(path):
        code, body, _ = json_http("https://api.stripe.com/v1/" + path,
            token=key, headers={"Stripe-Version": VERSION})
        need(code == 200, "providerReadFailed")
        return body
    session = stripe("checkout/sessions/" + sid)
    before_ids = set()
    for row in before["orders"]:
        matches = re.findall(r"rch_[a-f0-9]{64}", row)
        need(len(matches) == 1, "browserBeforeOrderIdentityInvalid")
        before_ids.add(matches[0])
    new_ids = {row["id"] for row in after["orders"]} - before_ids
    need(len(new_ids) == 1, "exactOneNewOrderRequired")
    order_id = new_ids.pop()
    expected = {"mir2_order_id": order_id, "mir2_account_id": cohort[0]["accountId"],
                "mir2_character_index": str(cohort[0]["characterIndex"])}
    need(session["livemode"] is False and session["status"] == "complete"
         and session["payment_status"] == "paid" and session["currency"] == "usd"
         and session["amount_total"] == 100 and session["client_reference_id"] == order_id
         and all(session["metadata"].get(k) == v for k, v in expected.items()), "paidOwnedSessionMismatch")
    pid = session["payment_intent"]
    need(re.fullmatch(r"pi_[A-Za-z0-9]+", pid), "paymentIntentMissing")
    intent = stripe("payment_intents/" + pid)
    need(intent["livemode"] is False and intent["status"] == "succeeded"
         and intent["amount"] == intent["amount_received"] == 100 and intent["currency"] == "usd"
         and all(intent["metadata"].get(k) == v for k, v in expected.items()), "paidOwnedIntentMismatch")
    need(before["credit"] == "0" and after["credit"] == "100"
         and before["expiry"] == after["monthlyExpiry"] and after["pendingHidden"] is True
         and checkout["sameCheckoutOnRetry"] is True, "browserEconomyProofMismatch")
    return {"livemode": False, "checkoutSha256": sha(sid), "paymentIntentSha256": sha(pid),
        "orderSha256": sha(order_id), "sessionStatus": "complete", "paymentStatus": "paid",
        "paymentIntentStatus": "succeeded", "currency": "usd", "amountTotal": 100,
        "amountReceived": 100, "ownerTupleMatches": True, "browserCreditBefore": 0,
        "browserCreditAfter": 100, "sameOfferRetrySameCheckout": True,
        "monthlyExpiryPreserved": True, "providerReplayUsed": False}, order_id


async def main(args):
    if args.deps:
        sys.path.insert(0, str(args.deps))
    cohort = read(args.cohort)["cohort"]
    need(len(cohort) >= 2 and cohort[0]["accountId"] != cohort[1]["accountId"], "ownedQaCohortRequired")
    code, health, _ = json_http(ORIGIN + "/playtest/health")
    need(code == 200 and health["ok"] is True and health["revision"] == args.gateway_revision,
         "currentGatewayRevisionMismatch")
    need(health["capacity"]["currentActiveSessions"] < 50, "ordinaryCapacityRequired")
    provider, order_id = provider_trial(args.test_env, cohort, args.trial_dir)
    public = public_probes(args.manifest_dir)
    ownership = await ownership_trial(cohort[0], cohort[1])
    game = await game_trial(cohort[0], order_id, args.monthly_expiry_ms)
    code, final_health, _ = json_http(ORIGIN + "/playtest/health")
    need(code == 200 and final_health["ok"] and final_health["revision"] == args.gateway_revision,
         "gatewayChangedDuringTrial")
    report = {"schema": "mir2.recharge-web.acceptance.v1", "passed": True,
        "verifiedAtUtc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "gatewayRevision": args.gateway_revision, "site": ORIGIN + "/recharge/", "tlsVerified": True,
        "provider": provider, "public": public, "ownership": ownership, "game": game,
        "scopeLimits": ["Stripe TEST only; no real money", "No live refund/dispute trial",
                        "No game binary or Gateway rollout", "No native visual human acceptance claim"]}
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8", newline="\n")
    print(json.dumps(report))


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    for name in ("cohort", "trial-dir", "test-env", "manifest-dir", "report", "deps"):
        parser.add_argument("--" + name, type=Path, required=name != "deps")
    parser.add_argument("--gateway-revision", required=True)
    parser.add_argument("--monthly-expiry-ms", required=True, type=int)
    args = parser.parse_args()
    try:
        asyncio.run(main(args))
    except Exception as error:
        locations = [{"function": f.name, "line": f.lineno}
                     for f in traceback.extract_tb(error.__traceback__) if Path(f.filename).name == "acceptance.py"]
        print(json.dumps({"passed": False, "error": str(error) if isinstance(error, Stop) else type(error).__name__,
                          "locations": locations}))
        raise SystemExit(1)
