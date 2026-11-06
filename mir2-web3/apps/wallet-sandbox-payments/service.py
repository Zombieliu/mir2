"""Fixed-price Stripe sandbox fulfillment into locked, adapter-owned QA characters.

This is deliberately not a general player billing service. It cannot accept a
player/account supplied at prepare time, unlock recipients, or modify an online
player. Stripe signature/payment verification happens in the wallet Worker.
"""

from __future__ import annotations

import argparse
import hashlib
import hmac
import json
import os
import re
import time
import uuid
from dataclasses import dataclass
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any
from urllib.parse import urlsplit

import psycopg
from psycopg.rows import dict_row
from websockets.sync.client import connect

CATALOG = {"sku": "numeron-test-100", "amount": 100, "currency": "usd", "credits": 100}
LOCK_MARKER = "dubhe_wallet_sandbox_locked_v1"
PREPARE_FIELDS = {"mode", "requestId", "ownerAddress", "network", *CATALOG}
FULFILL_FIELDS = {"mode", "orderId", "merchantOrderId", "ownerAddress", "accountId", "characterId", "serverId", "network", "paymentIntentId", *CATALOG}


class Rejected(Exception):
    def __init__(self, code: str, status: int = 409):
        super().__init__(code)
        self.code, self.status = code, status


def strict_json(raw: bytes) -> dict[str, Any]:
    def pairs(items):
        result = {}
        for key, value in items:
            if key in result:
                raise Rejected("duplicate_json_field", 400)
            result[key] = value
        return result
    try:
        value = json.loads(raw, object_pairs_hook=pairs, parse_constant=lambda _: (_ for _ in ()).throw(ValueError()))
    except (ValueError, UnicodeError) as error:
        raise Rejected("invalid_json", 400) from error
    if not isinstance(value, dict):
        raise Rejected("invalid_json", 400)
    return value


def validate_payload(body: dict[str, Any], fulfill: bool = False) -> None:
    fields = FULFILL_FIELDS if fulfill else PREPARE_FIELDS
    if set(body) != fields:
        raise Rejected("payment_fields_invalid", 400)
    if body.get("mode") != "test" or any(type(body.get(k)) is not type(v) or body[k] != v for k, v in CATALOG.items()):
        raise Rejected("sandbox_catalog_mismatch", 400)
    if body.get("network") not in ("mainnet", "testnet"):
        raise Rejected("payment_network_invalid", 400)
    checks = {"ownerAddress": r"0x[0-9a-f]{64}"}
    checks.update({"orderId": r"ord_[0-9a-f]{32}", "merchantOrderId": r"numqa_[0-9a-f]{32}", "paymentIntentId": r"pi_[A-Za-z0-9_]{1,200}",
                   "accountId": r"dq[0-9a-f]{14}", "characterId": r"dq[0-9a-f]{14}:[0-9]{1,10}", "serverId": r"[A-Za-z0-9_.:@-]{1,100}"} if fulfill else {"requestId": r"[0-9a-f]{64}"})
    if any(not isinstance(body.get(k), str) or re.fullmatch(pattern, body[k]) is None for k, pattern in checks.items()):
        raise Rejected("payment_identity_invalid", 400)


def payload_hash(body: dict[str, Any]) -> str:
    return hashlib.sha256(json.dumps(body, sort_keys=True, separators=(",", ":"), ensure_ascii=True).encode()).hexdigest()


@dataclass(frozen=True)
class Config:
    dsn: str
    token: str
    provisioning_key: str
    server_id: str = "numeron-remote-qa"
    gateway_ws: str = "ws://127.0.0.1:7110/ws"
    gateway_origin: str = "https://mir2.obelisk.build"
    enabled: bool = False
    mode: str = "test"
    host: str = "127.0.0.1"
    port: int = 7431

    def validate(self):
        if self.mode != "test" or not self.enabled:
            raise Rejected("sandbox_receiver_disabled", 503)
        if not self.dsn or any(re.fullmatch(r"[0-9a-f]{64}", value or "") is None for value in (self.token, self.provisioning_key)) or self.token == self.provisioning_key:
            raise Rejected("sandbox_receiver_not_configured", 503)
        target = urlsplit(self.gateway_ws)
        origin = urlsplit(self.gateway_origin)
        if target.scheme != "ws" or target.hostname != "127.0.0.1" or target.path != "/ws" or target.query or target.fragment or target.username or target.password:
            raise Rejected("sandbox_gateway_invalid", 503)
        if origin.scheme != "https" or origin.username or origin.password or origin.query or origin.fragment or origin.path not in ("", "/"):
            raise Rejected("sandbox_gateway_origin_invalid", 503)
        if self.host != "127.0.0.1" or not 1024 <= self.port <= 65535 or re.fullmatch(r"[A-Za-z0-9_.:@-]{1,100}", self.server_id) is None:
            raise Rejected("sandbox_binding_invalid", 503)

    @classmethod
    def from_env(cls):
        return cls(dsn=os.environ.get("NUMERON_SANDBOX_DATABASE_URL", ""), token=os.environ.get("NUMERON_SANDBOX_TOKEN", ""),
                   provisioning_key=os.environ.get("NUMERON_SANDBOX_PROVISIONING_KEY", ""), server_id=os.environ.get("NUMERON_SANDBOX_SERVER_ID", "numeron-remote-qa"),
                   gateway_ws=os.environ.get("NUMERON_SANDBOX_GATEWAY_WS", "ws://127.0.0.1:7110/ws"), gateway_origin=os.environ.get("NUMERON_SANDBOX_GATEWAY_ORIGIN", "https://mir2.obelisk.build"),
                   enabled=os.environ.get("NUMERON_SANDBOX_ENABLED") == "true", mode=os.environ.get("NUMERON_SANDBOX_MODE", "test"), port=int(os.environ.get("NUMERON_SANDBOX_PORT", "7431")))


def authorize(config: Config, header: str | None):
    config.validate()
    match = re.fullmatch(r"Bearer ([0-9a-f]{64})", header or "")
    if not match or not hmac.compare_digest(config.token, match[1]):
        raise Rejected("sandbox_unauthorized", 401)


def gateway_provision(config: Config, binding: dict[str, Any], account_exists: bool) -> int:
    # This key is separate from the bearer token held by the wallet Worker.
    material = (binding["owner_address"] + ":" + binding["account_id"]).encode()
    password = "Dq!" + hmac.new(bytes.fromhex(config.provisioning_key), material, hashlib.sha256).hexdigest()
    deadline = time.monotonic() + 18

    def receive(ws, accepted: set[str]):
        for _ in range(60):
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise Rejected("sandbox_gateway_timeout", 503)
            event = json.loads(ws.recv(timeout=remaining))
            if event.get("type") == "error":
                raise Rejected("sandbox_gateway_rejected", 503)
            if event.get("type") == "packet" and event.get("packet") in accepted:
                return event
        raise Rejected("sandbox_gateway_protocol_invalid", 503)

    try:
        with connect(config.gateway_ws, origin=config.gateway_origin, proxy=None, open_timeout=4, close_timeout=2, max_size=1024 * 1024, compression=None) as ws:
            ws.send(json.dumps({"type": "clientVersion"}))
            if receive(ws, {"ClientVersion"})["payload"].get("result") != 1:
                raise Rejected("sandbox_gateway_version_rejected", 503)
            if not account_exists:
                ws.send(json.dumps({"type": "newAccount", "accountId": binding["account_id"], "password": password}))
                if receive(ws, {"NewAccount"})["payload"].get("result") != 8:
                    raise Rejected("sandbox_account_creation_rejected", 503)
            ws.send(json.dumps({"type": "login", "accountId": binding["account_id"], "password": password}))
            login = receive(ws, {"LoginSuccess", "Login", "LoginBanned"})
            if login["packet"] != "LoginSuccess":
                raise Rejected("sandbox_account_ownership_unverified", 503)
            characters = login["payload"].get("characters", [])
            if characters:
                if len(characters) != 1 or characters[0].get("name") != binding["character_name"]:
                    raise Rejected("sandbox_account_not_dedicated", 409)
                return int(characters[0]["index"])
            ws.send(json.dumps({"type": "newCharacter", "name": binding["character_name"], "gender": "male", "class": "warrior"}))
            character = receive(ws, {"NewCharacter", "NewCharacterSuccess"})
            if character["packet"] != "NewCharacterSuccess" or character["payload"].get("character", {}).get("name") != binding["character_name"]:
                raise Rejected("sandbox_character_creation_rejected", 503)
            return int(character["payload"]["character"]["index"])
    except Rejected:
        raise
    except Exception as error:
        # No remote packet, password, environment or driver diagnostic is logged.
        raise Rejected("sandbox_gateway_unavailable", 503) from error


class Receiver:
    def __init__(self, config: Config, provision=gateway_provision):
        config.validate()
        self.config, self.provision = config, provision

    def connect(self):
        return psycopg.connect(self.config.dsn, row_factory=dict_row, connect_timeout=4, client_encoding="utf8", options="-c statement_timeout=8000 -c lock_timeout=5000")

    def migrate(self):
        with self.connect() as db:
            db.execute(Path(__file__).with_name("migration.sql").read_text())

    def reserve(self, owner: str):
        # Persist ownership before Gateway I/O so a process interruption cannot
        # turn a newly created account into an unowned, silently adopted account.
        with self.connect() as db:
            db.execute("SELECT pg_advisory_xact_lock(hashtextextended(%s, 837521))", (owner,))
            existing = db.execute("SELECT * FROM dubhe_sandbox_wallets WHERE owner_address=%s", (owner,)).fetchone()
            if existing:
                return
            suffix = uuid.uuid4().hex
            account, name = "dq" + suffix[:14], "Dq" + suffix[14:24]
            if db.execute("SELECT public.dubhe_sandbox_candidate_exists(%s) AS occupied", (account,)).fetchone()["occupied"]:
                raise Rejected("sandbox_account_collision", 503)
            db.execute("INSERT INTO dubhe_sandbox_wallets(owner_address,account_id,character_name) VALUES(%s,%s,%s)", (owner, account, name))

    def prepare(self, body: dict[str, Any]):
        validate_payload(body)
        owner = body["ownerAddress"]
        self.reserve(owner)
        with self.connect() as db:
            binding = self.ready_binding(db, owner)
            previous = db.execute("SELECT * FROM dubhe_sandbox_payment_orders WHERE request_id=%s", (body["requestId"],)).fetchone()
            if previous:
                if previous["owner_address"] != owner or previous["network"] != body["network"]:
                    raise Rejected("sandbox_request_reused")
                return self.public_order(previous)
            merchant = "numqa_" + uuid.uuid4().hex
            row = db.execute("INSERT INTO dubhe_sandbox_payment_orders(merchant_order_id,request_id,owner_address,account_id,character_index,server_id,network,sku,amount,currency,credits) VALUES(%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s) RETURNING *", (merchant, body["requestId"], owner, binding["account_id"], binding["character_index"], self.config.server_id, body["network"], *CATALOG.values())).fetchone()
            return self.public_order(row)

    def ready_binding(self, db, owner):
        binding = db.execute("SELECT * FROM dubhe_sandbox_wallets WHERE owner_address=%s FOR UPDATE", (owner,)).fetchone()
        if binding["state"] != "ready":
            existing = db.execute("SELECT created_at FROM accounts WHERE account_id=%s", (binding["account_id"],)).fetchone()
            if existing and existing["created_at"] < binding["created_at"]:
                raise Rejected("sandbox_account_collision")
            index = self.provision(self.config, binding, existing is not None)
            if type(index) is not int or not 0 <= index <= 2147483647:
                raise Rejected("sandbox_character_invalid")
            account = db.execute("SELECT raw_json FROM accounts WHERE account_id=%s FOR UPDATE", (binding["account_id"],)).fetchone()
            save = db.execute("SELECT credit,snapshot_json FROM character_saves WHERE account_id=%s AND character_index=%s FOR UPDATE", (binding["account_id"], index)).fetchone()
            raw = account["raw_json"] if account else {}
            characters = raw.get("characters", [])
            if not save or len(characters) != 1 or characters[0].get("name") != binding["character_name"] or characters[0].get("index") != index or save["credit"] != 0 or save["snapshot_json"].get("credit") != 0 or raw.get("saves", {}).get(str(index), {}).get("credit") != 0:
                raise Rejected("sandbox_new_recipient_invalid")
            # This recipient has never entered the world. Lock it before
            # accepting payments, so gameplay and payment writers cannot race.
            if self.active_lease(db, binding["account_id"]):
                raise Rejected("sandbox_recipient_online")
            db.execute("UPDATE accounts SET is_banned=true,ban_reason=%s,ban_until_ms=NULL,banned_at_ms=%s,store_version=store_version+1,updated_at=now(),raw_json=jsonb_set(jsonb_set(jsonb_set(jsonb_set(raw_json,'{is_banned}','true'::jsonb),'{ban_reason}',to_jsonb(%s::text)),'{ban_until_ms}','null'::jsonb),'{banned_at_ms}',to_jsonb(%s::bigint)) WHERE account_id=%s", (LOCK_MARKER, int(time.time() * 1000), LOCK_MARKER, int(time.time() * 1000), binding["account_id"]))
            db.execute("UPDATE dubhe_sandbox_wallets SET state='ready',character_index=%s,ready_at=now() WHERE owner_address=%s", (index, owner))
            binding.update(state="ready", character_index=index)
        self.locked_recipient(db, binding)
        return binding

    @staticmethod
    def public_order(row):
        return {"mode": "test", "dedicatedTestAccount": True, "ownerAddress": row["owner_address"], **CATALOG,
                "merchantOrderId": row["merchant_order_id"], "accountId": row["account_id"], "characterId": row["account_id"] + ":" + str(row["character_index"]), "serverId": row["server_id"]}

    @staticmethod
    def active_lease(db, account):
        # Missing authority tables are an error, never interpreted as "offline".
        return db.execute("SELECT 1 FROM gate14_session_leases WHERE account_id=%s AND expires_at_ms>%s LIMIT 1", (account, int(time.time() * 1000))).fetchone() is not None

    def locked_recipient(self, db, binding):
        account = db.execute("SELECT raw_json,is_banned,ban_reason FROM accounts WHERE account_id=%s FOR UPDATE", (binding["account_id"],)).fetchone()
        raw = account["raw_json"] if account else {}
        if not account or binding["state"] != "ready" or account["is_banned"] is not True or account["ban_reason"] != LOCK_MARKER or raw.get("is_banned") is not True or raw.get("ban_reason") != LOCK_MARKER:
            raise Rejected("sandbox_recipient_not_locked")
        characters = raw.get("characters", [])
        if len(characters) != 1 or characters[0].get("name") != binding["character_name"] or characters[0].get("index") != binding["character_index"] or self.active_lease(db, binding["account_id"]):
            raise Rejected("sandbox_recipient_not_offline")
        return account

    def fulfill(self, body: dict[str, Any]):
        validate_payload(body, fulfill=True)
        digest = payload_hash(body)
        try:
            with self.connect() as db:
                # Serialize same wallet and same prepared order; unique Stripe PI
                # is a second independent guard across different wallets/orders.
                binding = db.execute("SELECT * FROM dubhe_sandbox_wallets WHERE owner_address=%s FOR UPDATE", (body["ownerAddress"],)).fetchone()
                order = db.execute("SELECT * FROM dubhe_sandbox_payment_orders WHERE merchant_order_id=%s FOR UPDATE", (body["merchantOrderId"],)).fetchone()
                if not binding or not order:
                    raise Rejected("sandbox_order_not_found", 404)
                expected = self.public_order(order)
                for key in ("ownerAddress", "accountId", "characterId", "serverId", *CATALOG):
                    if body[key] != expected[key]:
                        raise Rejected("sandbox_order_mismatch")
                if body["serverId"] != self.config.server_id or body["network"] != order["network"] or binding["account_id"] != order["account_id"] or binding["character_index"] != order["character_index"]:
                    raise Rejected("sandbox_order_mismatch")
                receipt = db.execute("SELECT * FROM dubhe_sandbox_payment_receipts WHERE order_id=%s OR payment_intent_id=%s OR merchant_order_id=%s", (body["orderId"], body["paymentIntentId"], body["merchantOrderId"])).fetchone()
                if receipt:
                    if receipt["order_id"] != body["orderId"] or receipt["payload_hash"] != digest:
                        raise Rejected("sandbox_payment_already_used")
                    return self.receipt(body, receipt["balance_after"], True)
                def record(args, before, after):
                    db.execute("INSERT INTO dubhe_sandbox_payment_receipts(order_id,merchant_order_id,payment_intent_id,payload_hash,account_id,character_index,credits,balance_before,balance_after) VALUES(%s,%s,%s,%s,%s,%s,%s,%s,%s)", (body["orderId"], body["merchantOrderId"], body["paymentIntentId"], digest, *args, CATALOG["credits"], before, after))
                after = self.grant_locked_credits(db, binding, CATALOG["credits"], record)
                return self.receipt(body, after, False)
        except psycopg.errors.UniqueViolation as error:
            raise Rejected("sandbox_payment_already_used") from error

    def grant_locked_credits(self, db, binding, credits, record):
        account = self.locked_recipient(db, binding)
        args = (binding["account_id"], binding["character_index"])
        save = db.execute("SELECT credit,snapshot_json,save_version FROM character_saves WHERE account_id=%s AND character_index=%s FOR UPDATE", args).fetchone()
        projection = db.execute("SELECT credit,save_version FROM character_state WHERE account_id=%s AND character_index=%s FOR UPDATE", args).fetchone()
        if not save or not projection:
            raise Rejected("sandbox_projection_missing")
        raw_credit = account["raw_json"].get("saves", {}).get(str(binding["character_index"]), {}).get("credit")
        values = [raw_credit, save["snapshot_json"].get("credit"), save["credit"], projection["credit"]]
        if any(type(value) is not int or value < 0 for value in values) or len(set(values)) != 1 or save["save_version"] != projection["save_version"]:
            raise Rejected("sandbox_projection_mismatch")
        before, after = values[0], values[0] + credits
        if after > 4294967295:
            raise Rejected("sandbox_credit_overflow")
        record(args, before, after)
        db.execute("UPDATE accounts SET raw_json=jsonb_set(raw_json,%s,to_jsonb(%s::bigint),false),store_version=store_version+1,updated_at=now() WHERE account_id=%s", (["saves", str(binding["character_index"]), "credit"], after, binding["account_id"]))
        db.execute("UPDATE character_saves SET credit=%s,snapshot_json=jsonb_set(snapshot_json,'{credit}',to_jsonb(%s::bigint),false),save_version=save_version+1,updated_at=now() WHERE account_id=%s AND character_index=%s", (after, after, *args))
        db.execute("UPDATE character_state SET credit=%s,save_version=save_version+1,updated_at_ms=%s,updated_at=now() WHERE account_id=%s AND character_index=%s", (after, int(time.time() * 1000), *args))
        return after

    @staticmethod
    def receipt(body, after, duplicate):
        return {"mode": "test", "delivered": True, "orderId": body["orderId"], "merchantOrderId": body["merchantOrderId"], "accountId": body["accountId"], "characterId": body["characterId"], "credits": CATALOG["credits"], "duplicate": duplicate, "balanceAfter": after}


def handler_for(receiver: Receiver):
    class Handler(BaseHTTPRequestHandler):
        server_version = "DubheNumeronSandbox/1"

        def setup(self):
            super().setup()
            self.connection.settimeout(5)

        def log_message(self, *_):
            pass

        def reply(self, status, value):
            raw = json.dumps(value, separators=(",", ":")).encode()
            self.send_response(status)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(raw)))
            self.send_header("Cache-Control", "no-store, private")
            self.send_header("X-Content-Type-Options", "nosniff")
            self.end_headers()
            self.wfile.write(raw)

        def do_GET(self):
            if self.path != "/health":
                return self.reply(404, {"error": "not_found"})
            try:
                receiver.config.validate()
                with receiver.connect() as db:
                    db.execute("SELECT 1 FROM dubhe_sandbox_payment_receipts LIMIT 1")
                self.reply(200, {"ok": True, "mode": "test", "service": "numeron-wallet-sandbox-payments"})
            except Exception:
                self.reply(503, {"ok": False, "mode": "test"})

        def do_POST(self):
            try:
                authorize(receiver.config, self.headers.get("Authorization"))
                if self.path not in ("/prepare", "/fulfill"):
                    raise Rejected("not_found", 404)
                length = self.headers.get("Content-Length", "")
                if self.headers.get("Transfer-Encoding") or re.fullmatch(r"[0-9]{1,6}", length) is None or not 1 <= int(length) <= 16384:
                    raise Rejected("payload_too_large_or_unbounded", 413)
                if self.headers.get("Content-Type", "").split(";", 1)[0].strip() != "application/json":
                    raise Rejected("json_required", 415)
                self.connection.settimeout(5)
                raw = self.rfile.read(int(length))
                if len(raw) != int(length):
                    raise Rejected("payload_incomplete", 400)
                body = strict_json(raw)
                result = receiver.prepare(body) if self.path == "/prepare" else receiver.fulfill(body)
                self.reply(200, result)
            except Rejected as error:
                self.reply(error.status, {"error": error.code, "mode": "test"})
            except Exception as error:
                print(json.dumps({"event": "sandbox_receiver_error", "type": type(error).__name__}), flush=True)
                self.reply(503, {"error": "sandbox_receiver_unavailable", "mode": "test"})

    return Handler


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=["serve", "migrate"], default="serve", nargs="?")
    arguments = parser.parse_args()
    receiver = Receiver(Config.from_env())
    if arguments.command == "migrate":
        receiver.migrate()
    else:
        server = ThreadingHTTPServer((receiver.config.host, receiver.config.port), handler_for(receiver))
        server.daemon_threads = True
        server.serve_forever()


if __name__ == "__main__":
    main()
