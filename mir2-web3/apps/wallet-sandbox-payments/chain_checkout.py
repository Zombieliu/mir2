"""Numeron Sui Testnet QA adapter. No HTTP listener and no browser-supplied receipts."""

import argparse
import json
import os
import re
import uuid
from urllib.parse import urlsplit
from urllib.request import HTTPRedirectHandler, Request, build_opener

import psycopg
from service import Config, Receiver, Rejected, strict_json

SKU = "numeron-chain-qa-100"
USDC = "0xa1ec7fc00a6f40db9693ad1415d0c193ad3906494428cf252621037bd7117e29::usdc::USDC"
PACKAGE = "0x7e069abe383e80d32f2aec17b3793da82aabc8c2edf84abbf68dd7b719e71497"
ADDRESS = r"0x[0-9a-f]{64}"
HEX = r"[0-9a-f]{64}"
DIGEST = r"[1-9A-HJ-NP-Za-km-z]{32,44}"
NONCE = r"[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}"


def matches(value, pattern):
    return isinstance(value, str) and re.fullmatch(pattern, value) is not None


class NoRedirect(HTTPRedirectHandler):
    def redirect_request(self, *args, **kwargs):
        return None


class ChainCheckout:
    def __init__(self, receiver, origin, wallet_origin, token, registry, payee, payers, *, local=False):
        for url in (origin, wallet_origin):
            target = urlsplit(url)
            if not target.hostname or url != target.scheme + "://" + target.netloc or target.username or target.password or (
                target.scheme != "https" and not (local and target.scheme == "http" and target.hostname == "127.0.0.1")
            ):
                raise Rejected("chain_invalid_origin")
        if not matches(token, HEX) or token in {receiver.config.token, receiver.config.provisioning_key}:
            raise Rejected("chain_invalid_token")
        if not payers or not all(matches(addr, ADDRESS) for addr in (*payers, registry, payee)) or payee in payers:
            raise Rejected("chain_invalid_test_configuration")
        self.receiver, self.origin, self.wallet_origin, self.token = receiver, origin, wallet_origin, token
        self.registry, self.payee, self.payers = registry, payee, frozenset(payers)
        self.opener = build_opener(NoRedirect)

    def request(self, path, data=None):
        headers = {"Authorization": "Bearer " + self.token}
        payload = None
        if data is not None:
            headers["Content-Type"] = "application/json"
            payload = json.dumps(data).encode()
        with self.opener.open(Request(self.origin + path, data=payload, headers=headers), timeout=12) as response:
            raw = response.read(262145)
            if response.headers.get_content_type() != "application/json" or len(raw) > 262144:
                raise Rejected("chain_response_invalid")
            return strict_json(raw)

    @staticmethod
    def customer(order):
        return order["server_id"] + ":" + order["account_id"] + ":" + str(order["character_index"])

    def terms(self, receipt, order):
        return isinstance(receipt, dict) and all(receipt.get(k) == v for k, v in {
            "merchantOrderId": order["merchant_ref"], "customerRef": self.customer(order),
            "network": "testnet", "payer": order["payer"], "receiver": order["receiver"],
            "registry": order["registry"], "packageId": PACKAGE, "asset": USDC,
            "sku": SKU, "amountAtomic": "1000000", "credits": 100,
        }.items()) and type(receipt["credits"]) is int

    def prepare(self, owner, request_id):
        if owner not in self.payers or not matches(request_id, HEX):
            raise Rejected("chain_not_test_recipient", 403)
        receiver = self.receiver
        receiver.reserve(owner)
        with receiver.connect() as db:
            db.execute("SELECT pg_advisory_xact_lock(hashtextextended(%s,837522))", (request_id,))
            order = db.execute("SELECT * FROM dubhe_sandbox_chain_orders WHERE request_id=%s", (request_id,)).fetchone()
            if not order:
                binding = receiver.ready_binding(db, owner)
                order = db.execute(
                    "INSERT INTO dubhe_sandbox_chain_orders(merchant_ref,request_id,payer,account_id,character_index,server_id,registry,receiver) "
                    "VALUES(%s,%s,%s,%s,%s,%s,%s,%s) RETURNING *",
                    ("numchain_" + uuid.uuid4().hex, request_id, owner, binding["account_id"], binding["character_index"],
                     receiver.config.server_id, self.registry, self.payee),
                ).fetchone()
            if (order["payer"], order["registry"], order["receiver"], order["server_id"]) != (
                owner, self.registry, self.payee, receiver.config.server_id
            ):
                raise Rejected("chain_request_reused")
        result = self.request("/v1/orders", {"payer": owner, "sku": SKU, "merchantOrderId": order["merchant_ref"],
                                               "customerRef": self.customer(order)})
        payment, link = result.get("order"), result.get("checkoutUrl")
        if not self.terms(payment, order) or not matches(payment.get("nonce"), NONCE):
            raise Rejected("chain_order_response_mismatch")
        prefix = self.wallet_origin + "/checkout#order=" + payment["nonce"] + "."
        if not isinstance(link, str) or not link.startswith(prefix) or not matches(link[len(prefix):], r"[A-Za-z0-9_-]{43}"):
            raise Rejected("chain_checkout_link_invalid")
        with receiver.connect() as db:
            changed = db.execute("UPDATE dubhe_sandbox_chain_orders SET wallet_nonce=%s WHERE merchant_ref=%s AND (wallet_nonce IS NULL OR wallet_nonce=%s)",
                                 (payment["nonce"], order["merchant_ref"], payment["nonce"]))
            if changed.rowcount != 1:
                raise Rejected("chain_nonce_mismatch")
        return {"mode": "testnet", "checkoutUrl": link, "merchantOrderId": order["merchant_ref"],
                "characterId": order["account_id"] + ":" + str(order["character_index"]), "credits": 100}

    def grant(self, receipt):
        if not isinstance(receipt, dict) or receipt.get("status") != "confirmed" or not all(matches(receipt.get(k), pattern) for k, pattern in {
            "nonce": NONCE, "digest": DIGEST, "checkpoint": r"(0|[1-9][0-9]{0,19})", "merchantOrderId": r"numchain_[0-9a-f]{32}",
        }.items()):
            raise Rejected("chain_receipt_unconfirmed")
        receiver = self.receiver
        try:
            with receiver.connect() as db:
                order = db.execute("SELECT * FROM dubhe_sandbox_chain_orders WHERE merchant_ref=%s FOR UPDATE",
                                   (receipt["merchantOrderId"],)).fetchone()
                if (not order or order["payer"] not in self.payers or not self.terms(receipt, order) or
                    order["receiver"] != self.payee or order["registry"] != self.registry or order["server_id"] != receiver.config.server_id or
                    str(order["wallet_nonce"]) != receipt["nonce"]):
                    raise Rejected("chain_receipt_mismatch")
                prior = db.execute("SELECT * FROM dubhe_sandbox_chain_receipts WHERE merchant_ref=%s", (order["merchant_ref"],)).fetchone()
                if prior:
                    if prior["sui_digest"] != receipt["digest"] or str(prior["checkpoint"]) != receipt["checkpoint"]:
                        raise Rejected("chain_receipt_reused")
                    return prior["grant_id"]
                binding = db.execute("SELECT * FROM dubhe_sandbox_wallets WHERE owner_address=%s FOR UPDATE", (order["payer"],)).fetchone()
                if not binding or (order["account_id"], order["character_index"]) != (binding["account_id"], binding["character_index"]):
                    raise Rejected("chain_recipient_mismatch")
                grant_id = "qagrant_" + uuid.uuid4().hex

                def record(_, before, after):
                    db.execute("INSERT INTO dubhe_sandbox_chain_receipts(merchant_ref,wallet_nonce,sui_digest,grant_id,checkpoint,balance_before,balance_after) VALUES(%s,%s,%s,%s,%s,%s,%s)",
                               (order["merchant_ref"], receipt["nonce"], receipt["digest"], grant_id, receipt["checkpoint"], before, after))

                receiver.grant_locked_credits(db, binding, 100, record)
                return grant_id
        except psycopg.errors.UniqueViolation as error:
            raise Rejected("chain_receipt_reused") from error

    def drain(self):
        receipts = self.request("/v1/merchant/receipts").get("orders")
        if not isinstance(receipts, list) or len(receipts) > 100:
            raise Rejected("chain_receipts_invalid")
        delivered, deferred = 0, 0
        for receipt in receipts:
            try:
                grant_id = self.grant(receipt)  # Database commits before ACK.
                result = self.request("/v1/merchant/ack", {"nonce": receipt["nonce"], "digest": receipt["digest"],
                                                            "merchantOrderId": receipt["merchantOrderId"], "grantId": grant_id})
                if result.get("status") != "delivered" or result.get("grantId") != grant_id:
                    raise Rejected("chain_ack_mismatch")
                delivered += 1
            except (Rejected, OSError, psycopg.Error):
                deferred += 1
        return {"delivered": delivered, "deferred": deferred}


def main():
    parser = argparse.ArgumentParser(description="Numeron USDC Testnet QA only")
    parser.add_argument("command", choices=["prepare", "drain"])
    parser.add_argument("--payer")
    parser.add_argument("--request-id")
    args = parser.parse_args()
    if os.environ.get("NUMERON_CHAIN_QA_ENABLED") != "true":
        raise SystemExit("Numeron chain QA is disabled.")
    env = os.environ
    consumer = ChainCheckout(Receiver(Config.from_env()), env["NUMERON_CHAIN_SERVICE_ORIGIN"],
                             env["NUMERON_CHAIN_WALLET_ORIGIN"], env["NUMERON_CHAIN_MERCHANT_TOKEN"],
                             env["NUMERON_CHAIN_REGISTRY"], env["NUMERON_CHAIN_PAYEE"],
                             env["NUMERON_CHAIN_QA_PAYERS"].split(","), local=env.get("NUMERON_CHAIN_LOCAL") == "true")
    print(json.dumps(consumer.prepare(args.payer, args.request_id) if args.command == "prepare" else consumer.drain()))


if __name__ == "__main__":
    main()
