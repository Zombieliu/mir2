import json
import os
import threading
import unittest
import urllib.error
import urllib.request
import uuid
from concurrent.futures import ThreadPoolExecutor
from dataclasses import replace
from http.server import ThreadingHTTPServer
from pathlib import Path
from unittest.mock import patch
from types import SimpleNamespace

import psycopg
from psycopg import sql
from psycopg.conninfo import make_conninfo
from psycopg.rows import dict_row
from psycopg.types.json import Jsonb
from websockets.sync.server import serve

from service import CATALOG, LOCK_MARKER, Config, Receiver, Rejected, authorize, gateway_provision, handler_for, strict_json, validate_payload


def config(**changes):
    return replace(Config(dsn="postgresql://unused", token="a" * 64, provisioning_key="b" * 64, enabled=True), **changes)


def prepare_body(owner=None):
    return {"mode": "test", "ownerAddress": owner or "0x" + "11" * 32, "requestId": uuid.uuid4().hex + uuid.uuid4().hex, "network": "testnet", **CATALOG}


def payment_body(order, **changes):
    return {"mode": "test", "ownerAddress": order["ownerAddress"], "network": "testnet", **CATALOG,
            "merchantOrderId": order["merchantOrderId"], "accountId": order["accountId"], "characterId": order["characterId"], "serverId": order["serverId"],
            "orderId": "ord_" + uuid.uuid4().hex, "paymentIntentId": "pi_" + uuid.uuid4().hex, **changes}


class ValidationTests(unittest.TestCase):
    def test_strict_catalog_mode_types_and_unknown_fields(self):
        good = prepare_body()
        validate_payload(good)
        for key, value in [("mode", "live"), ("amount", 101), ("amount", True), ("amount", 100.0), ("currency", "USD"), ("credits", 1), ("sku", "other"), ("ownerAddress", "0x123"), ("requestId", "z" * 64), ("network", "devnet"), ("accountId", "demo")]:
            with self.subTest(key=key, value=value), self.assertRaises(Rejected):
                validate_payload({**good, key: value})
        with self.assertRaises(Rejected):
            validate_payload({k: v for k, v in good.items() if k != "mode"})

    def test_json_duplicates_constants_and_non_objects_rejected(self):
        for value in [b'{"mode":"test","mode":"live"}', b'{"a":NaN}', b'[]', b'null', b'\xff']:
            with self.subTest(value=value), self.assertRaises(Rejected):
                strict_json(value)

    def test_bearer_constant_time_and_test_only_configuration(self):
        with patch("service.hmac.compare_digest", return_value=True) as comparison:
            authorize(config(), "Bearer " + "a" * 64)
            comparison.assert_called_once_with("a" * 64, "a" * 64)
        for header in [None, "Bearer " + "c" * 64, "Bearer " + "a" * 64 + " ", "bearer " + "a" * 64]:
            with self.subTest(header=header), self.assertRaises(Rejected) as failure:
                authorize(config(), header)
            self.assertEqual(failure.exception.status, 401)
        for options in [{"enabled": False}, {"mode": "live"}, {"host": "0.0.0.0"}, {"gateway_ws": "ws://example.org/ws"}, {"token": ""}, {"provisioning_key": "a" * 64}]:
            with self.subTest(options=options), self.assertRaises(Rejected):
                config(**options).validate()

    def test_gateway_flow_uses_real_commands_without_entering_world(self):
        commands = []
        def socket_handler(ws):
            for _ in range(4):
                command = json.loads(ws.recv(timeout=2))
                commands.append(command["type"])
                if command["type"] == "clientVersion":
                    response = {"packet": "ClientVersion", "payload": {"result": 1}}
                elif command["type"] == "newAccount":
                    self.assertGreater(len(command["password"]), 10)
                    response = {"packet": "NewAccount", "payload": {"result": 8}}
                elif command["type"] == "login":
                    response = {"packet": "LoginSuccess", "payload": {"characters": []}}
                else:
                    response = {"packet": "NewCharacterSuccess", "payload": {"character": {"index": 7, "name": "Dq1234567890"}}}
                ws.send(json.dumps({"type": "packet", **response}))
        with serve(socket_handler, "127.0.0.1", 0) as server:
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            port = server.socket.getsockname()[1]
            binding = {"owner_address": "0x" + "11" * 32, "account_id": "dq12345678901234", "character_name": "Dq1234567890"}
            self.assertEqual(gateway_provision(config(gateway_ws=f"ws://127.0.0.1:{port}/ws"), binding, False), 7)
            server.shutdown()
            thread.join(2)
        self.assertEqual(commands, ["clientVersion", "newAccount", "login", "newCharacter"])

    def test_http_rejects_unauthenticated_before_receiver(self):
        receiver = Receiver(config())
        with patch.object(receiver, "prepare") as prepare:
            server = ThreadingHTTPServer(("127.0.0.1", 0), handler_for(receiver))
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            try:
                request = urllib.request.Request(f"http://127.0.0.1:{server.server_port}/prepare", data=json.dumps(prepare_body()).encode(), headers={"Content-Type": "application/json"})
                with self.assertRaises(urllib.error.HTTPError) as error:
                    urllib.request.urlopen(request, timeout=2)
                self.assertEqual(error.exception.code, 401)
                self.assertEqual(json.load(error.exception)["error"], "sandbox_unauthorized")
                error.exception.close()
                prepare.assert_not_called()
            finally:
                server.shutdown()
                server.server_close()
                thread.join(2)


@unittest.skipUnless(os.environ.get("WALLET_SANDBOX_TEST_DSN"), "Set a disposable PostgreSQL test DSN to run transaction tests")
class PostgresTests(unittest.TestCase):
    def setUp(self):
        self.base_dsn = os.environ["WALLET_SANDBOX_TEST_DSN"]
        self.database = "dubhe_test_" + uuid.uuid4().hex
        with psycopg.connect(self.base_dsn, autocommit=True) as db:
            db.execute(sql.SQL("CREATE DATABASE {} TEMPLATE template0 ENCODING 'UTF8'").format(sql.Identifier(self.database)))
        self.dsn = make_conninfo(self.base_dsn, dbname=self.database)
        # The minimal fixture uses the same authoritative column types and keeps
        # unrelated data that must survive a field-specific payment update.
        with self.db() as db:
            # PostgreSQL14 defaults PUBLIC to CREATE in public; use the modern
            # owner-only namespace policy expected by this production module.
            db.execute("REVOKE CREATE ON SCHEMA public FROM PUBLIC")
            db.execute("CREATE TABLE accounts(account_id text PRIMARY KEY,password_snapshot text NOT NULL DEFAULT 'private-player-password-hash',raw_json jsonb NOT NULL,store_version bigint NOT NULL DEFAULT 1,is_banned boolean NOT NULL DEFAULT false,ban_reason text NOT NULL DEFAULT '',ban_until_ms bigint,banned_at_ms bigint,created_at timestamptz NOT NULL DEFAULT now(),updated_at timestamptz NOT NULL DEFAULT now())")
            db.execute("CREATE TABLE character_saves(account_id text,character_index int,credit bigint,snapshot_json jsonb,save_version bigint DEFAULT 1,updated_at timestamptz DEFAULT now(),PRIMARY KEY(account_id,character_index))")
            db.execute("CREATE TABLE character_state(account_id text,character_index int,credit bigint,save_version bigint DEFAULT 1,updated_at_ms bigint DEFAULT 0,updated_at timestamptz DEFAULT now(),unknown_state text DEFAULT 'preserve',PRIMARY KEY(account_id,character_index))")
            db.execute("CREATE TABLE gate14_session_leases(session_id text,account_id text,character_id text,expires_at_ms bigint)")
        self.provisions = 0
        self.receiver = Receiver(config(dsn=self.dsn), self.provision)
        # Keep the fixture search_path while retaining runtime timeouts.
        self.receiver.connect = self.db
        self.receiver.migrate()

    def db(self):
        return psycopg.connect(self.dsn, row_factory=dict_row, client_encoding="utf8", options="-c statement_timeout=8000 -c lock_timeout=5000")

    def tearDown(self):
        with psycopg.connect(self.base_dsn, autocommit=True) as db:
            db.execute(sql.SQL("DROP DATABASE {}").format(sql.Identifier(self.database)))

    def provision(self, _config, binding, exists):
        self.provisions += 1
        self.assertFalse(exists)
        save = {"credit": 0, "future_sensitive_extension": {"nested": ["unchanged", 23]}, "character": {"index": 7, "name": binding["character_name"]}}
        raw = {"characters": [save["character"]], "saves": {"7": save}, "is_banned": False, "ban_reason": "", "unknown_account": {"keep": True}}
        with self.db() as db:
            db.execute("INSERT INTO accounts(account_id,raw_json) VALUES(%s,%s)", (binding["account_id"], Jsonb(raw)))
            db.execute("INSERT INTO character_saves(account_id,character_index,credit,snapshot_json) VALUES(%s,7,0,%s)", (binding["account_id"], Jsonb(save)))
            db.execute("INSERT INTO character_state(account_id,character_index,credit) VALUES(%s,7,0)", (binding["account_id"],))
        return 7

    def credits(self, account):
        with self.db() as db:
            row = db.execute("SELECT a.raw_json,s.snapshot_json,s.credit AS save_credit,p.credit AS projection_credit,a.store_version,s.save_version,p.save_version AS projection_version,p.unknown_state FROM accounts a JOIN character_saves s USING(account_id) JOIN character_state p USING(account_id,character_index) WHERE a.account_id=%s", (account,)).fetchone()
            row["receipts"] = db.execute("SELECT COUNT(*) AS n FROM dubhe_sandbox_payment_receipts").fetchone()["n"]
            return row

    def restricted_receiver(self):
        with self.db() as db:
            db.execute(Path(__file__).with_name("runtime_access.sql").read_text())
            # Disposable loopback trust-auth PostgreSQL only: test a real login
            # as the restricted role without storing any role password.
            db.execute("ALTER ROLE mir2_wallet_sandbox LOGIN")
        return Receiver(config(dsn=make_conninfo(self.dsn, user="mir2_wallet_sandbox")), self.provision)

    def test_restricted_login_can_prepare_and_fulfill_without_base_table_access(self):
        ordinary = {"characters": [], "saves": {}, "private_player": {"keep": True}}
        with self.db() as db:
            db.execute("INSERT INTO accounts(account_id,raw_json) VALUES('ordinary_player',%s)", (Jsonb(ordinary),))
        runtime = self.restricted_receiver()
        with runtime.connect() as db:
            self.assertEqual(db.execute("SELECT current_user AS name").fetchone()["name"], "mir2_wallet_sandbox")
            self.assertEqual(db.execute("SHOW search_path").fetchone()["search_path"], "dubhe_wallet_sandbox, public")
            self.assertIsNone(db.execute("SELECT raw_json,password_snapshot FROM accounts WHERE account_id='ordinary_player'").fetchone())
        for statement in ["SELECT raw_json,password_snapshot FROM public.accounts", "UPDATE public.accounts SET raw_json='{}'::jsonb", "CREATE TABLE public.unwanted(id integer)"]:
            with self.subTest(statement=statement), self.assertRaises(psycopg.errors.InsufficientPrivilege):
                with runtime.connect() as db:
                    db.execute(statement)
        order = runtime.prepare(prepare_body())
        payment = payment_body(order)
        with ThreadPoolExecutor(max_workers=5) as pool:
            results = list(pool.map(lambda _: runtime.fulfill(payment), range(5)))
        self.assertEqual(sum(result["duplicate"] is False for result in results), 1)
        self.assertEqual(self.credits(order["accountId"])["save_credit"], 100)
        with runtime.connect() as db:
            self.assertEqual(db.execute("UPDATE accounts SET raw_json='{}'::jsonb WHERE account_id='ordinary_player'").rowcount, 0)
        with self.assertRaises(psycopg.errors.InsufficientPrivilege):
            with runtime.connect() as db:
                db.execute("UPDATE accounts SET password_snapshot='changed' WHERE account_id=%s", (order["accountId"],))
        with self.db() as db:
            self.assertEqual(db.execute("SELECT raw_json FROM accounts WHERE account_id='ordinary_player'").fetchone()["raw_json"], ordinary)

    def test_restricted_candidate_check_cannot_adopt_unowned_qa_account(self):
        candidate = "dq" + "a" * 14
        original = {"unowned_qa": True, "characters": [], "saves": {}}
        with self.db() as db:
            db.execute("INSERT INTO accounts(account_id,raw_json) VALUES(%s,%s)", (candidate, Jsonb(original)))
        runtime = self.restricted_receiver()
        with runtime.connect() as db:
            self.assertIsNone(db.execute("SELECT * FROM accounts WHERE account_id=%s", (candidate,)).fetchone())
            self.assertTrue(db.execute("SELECT public.dubhe_sandbox_candidate_exists(%s) AS occupied", (candidate,)).fetchone()["occupied"])
        with self.assertRaises(psycopg.errors.InvalidParameterValue):
            with runtime.connect() as db:
                db.execute("SELECT public.dubhe_sandbox_candidate_exists('ordinary_player')")
        with patch("service.uuid.uuid4", return_value=SimpleNamespace(hex="a" * 32)), self.assertRaisesRegex(Rejected, "sandbox_account_collision"):
            runtime.prepare(prepare_body())
        with self.db() as db:
            self.assertEqual(db.execute("SELECT COUNT(*) AS n FROM dubhe_sandbox_wallets").fetchone()["n"], 0)
            self.assertEqual(db.execute("SELECT raw_json FROM accounts WHERE account_id=%s", (candidate,)).fetchone()["raw_json"], original)

    def test_prepare_retries_concurrently_create_one_owned_recipient_order(self):
        body = prepare_body()
        with ThreadPoolExecutor(max_workers=4) as pool:
            orders = list(pool.map(lambda _: self.receiver.prepare(body), range(4)))
        self.assertTrue(all(order == orders[0] for order in orders))
        self.assertEqual(self.provisions, 1)
        self.assertTrue(orders[0]["dedicatedTestAccount"])
        with self.db() as db:
            row = db.execute("SELECT is_banned,ban_reason FROM accounts").fetchone()
            self.assertEqual(row, {"is_banned": True, "ban_reason": LOCK_MARKER})

    def test_exactly_once_concurrent_replay_and_unknown_fields_preserved(self):
        order = self.receiver.prepare(prepare_body())
        payment = payment_body(order)
        with ThreadPoolExecutor(max_workers=6) as pool:
            results = list(pool.map(lambda _: self.receiver.fulfill(payment), range(6)))
        self.assertEqual(sum(result["duplicate"] is False for result in results), 1)
        self.assertTrue(all(result["balanceAfter"] == 100 for result in results))
        current = self.credits(order["accountId"])
        self.assertEqual([current["raw_json"]["saves"]["7"]["credit"], current["snapshot_json"]["credit"], current["save_credit"], current["projection_credit"]], [100] * 4)
        self.assertEqual(current["raw_json"]["unknown_account"], {"keep": True})
        self.assertEqual(current["snapshot_json"]["future_sensitive_extension"], {"nested": ["unchanged", 23]})
        self.assertEqual(current["unknown_state"], "preserve")
        self.assertEqual(current["save_version"], current["projection_version"])
        self.assertEqual(current["receipts"], 1)

    def test_wrong_identity_or_changed_contract_does_not_issue(self):
        order = self.receiver.prepare(prepare_body())
        payment = payment_body(order)
        for key, value in [("ownerAddress", "0x" + "22" * 32), ("accountId", "dq" + "a" * 14), ("characterId", order["accountId"] + ":8"), ("serverId", "other"), ("network", "mainnet"), ("credits", 200), ("amount", 1), ("mode", "live")]:
            with self.subTest(key=key), self.assertRaises(Rejected):
                self.receiver.fulfill({**payment, key: value})
        self.assertEqual(self.credits(order["accountId"])["save_credit"], 0)

    def test_unique_payment_intent_cannot_credit_another_wallet_or_order(self):
        first = self.receiver.prepare(prepare_body())
        second = self.receiver.prepare(prepare_body("0x" + "22" * 32))
        payment = payment_body(first)
        self.receiver.fulfill(payment)
        with self.assertRaisesRegex(Rejected, "sandbox_payment_already_used"):
            self.receiver.fulfill(payment_body(second, paymentIntentId=payment["paymentIntentId"]))
        with self.assertRaisesRegex(Rejected, "sandbox_payment_already_used"):
            self.receiver.fulfill({**payment, "paymentIntentId": "pi_different"})
        self.assertEqual(self.credits(second["accountId"])["save_credit"], 0)
        self.assertEqual(self.credits(first["accountId"])["receipts"], 1)

    def test_concurrent_cross_wallet_payment_intent_race_has_one_winner(self):
        orders = [self.receiver.prepare(prepare_body("0x" + f"{number:02x}" * 32)) for number in range(1, 5)]
        barrier = threading.Barrier(4)
        def deliver(order):
            barrier.wait(timeout=3)
            try:
                return self.receiver.fulfill(payment_body(order, paymentIntentId="pi_concurrent_shared"))
            except Rejected as error:
                self.assertEqual(error.code, "sandbox_payment_already_used")
                return None
        with ThreadPoolExecutor(max_workers=4) as pool:
            results = list(pool.map(deliver, orders))
        self.assertEqual(sum(value is not None for value in results), 1)
        self.assertEqual(sum(self.credits(order["accountId"])["save_credit"] for order in orders), 100)
        self.assertEqual(self.credits(orders[0]["accountId"])["receipts"], 1)

    def test_interrupted_gateway_provision_retains_reservation_and_recovers(self):
        body = prepare_body()
        def interrupted(cfg, binding, exists):
            self.provision(cfg, binding, exists)
            raise Rejected("sandbox_gateway_unavailable", 503)
        self.receiver.provision = interrupted
        with self.assertRaisesRegex(Rejected, "sandbox_gateway_unavailable"):
            self.receiver.prepare(body)
        with self.db() as db:
            binding = db.execute("SELECT * FROM dubhe_sandbox_wallets WHERE owner_address=%s", (body["ownerAddress"],)).fetchone()
            self.assertEqual(binding["state"], "provisioning")
            self.assertIsNotNone(db.execute("SELECT 1 FROM accounts WHERE account_id=%s", (binding["account_id"],)).fetchone())
        def recovered(_cfg, reserved, exists):
            self.assertTrue(exists)
            self.assertEqual(reserved["account_id"], binding["account_id"])
            return 7
        self.receiver.provision = recovered
        order = self.receiver.prepare(body)
        self.assertEqual(order["accountId"], binding["account_id"])
        self.assertEqual(self.receiver.fulfill(payment_body(order))["balanceAfter"], 100)

    def test_projection_discrepancy_rolls_back_and_can_retry_after_repair(self):
        order = self.receiver.prepare(prepare_body())
        with self.db() as db:
            db.execute("UPDATE character_state SET credit=1 WHERE account_id=%s", (order["accountId"],))
        payment = payment_body(order)
        with self.assertRaisesRegex(Rejected, "sandbox_projection_mismatch"):
            self.receiver.fulfill(payment)
        current = self.credits(order["accountId"])
        self.assertEqual(current["save_credit"], 0)
        self.assertEqual(current["receipts"], 0)
        with self.db() as db:
            db.execute("UPDATE character_state SET credit=0 WHERE account_id=%s", (order["accountId"],))
        self.assertEqual(self.receiver.fulfill(payment)["balanceAfter"], 100)

    def test_database_failure_after_receipt_insert_rolls_back_all_writes(self):
        order = self.receiver.prepare(prepare_body())
        with self.db() as db:
            db.execute("ALTER TABLE character_state ADD CONSTRAINT force_rollback CHECK(credit=0)")
        with self.assertRaises(psycopg.errors.CheckViolation):
            self.receiver.fulfill(payment_body(order))
        current = self.credits(order["accountId"])
        self.assertEqual(current["save_credit"], 0)
        self.assertEqual(current["raw_json"]["saves"]["7"]["credit"], 0)
        self.assertEqual(current["receipts"], 0)

    def test_online_or_unlocked_recipient_fails_closed(self):
        order = self.receiver.prepare(prepare_body())
        payment = payment_body(order)
        with self.db() as db:
            db.execute("INSERT INTO gate14_session_leases VALUES('unexpected',%s,'7',9999999999999)", (order["accountId"],))
        with self.assertRaisesRegex(Rejected, "sandbox_recipient_not_offline"):
            self.receiver.fulfill(payment)
        with self.db() as db:
            db.execute("DELETE FROM gate14_session_leases")
            db.execute("UPDATE accounts SET is_banned=false WHERE account_id=%s", (order["accountId"],))
        with self.assertRaisesRegex(Rejected, "sandbox_recipient_not_locked"):
            self.receiver.fulfill(payment)
        self.assertEqual(self.credits(order["accountId"])["receipts"], 0)

    def test_existing_player_is_untouched_and_cannot_be_supplied(self):
        original = {"characters": [], "saves": {}, "valuable_unknown": {"keep": 42}}
        with self.db() as db:
            db.execute("INSERT INTO accounts(account_id,raw_json) VALUES('existing_player',%s)", (Jsonb(original),))
        order = self.receiver.prepare(prepare_body())
        self.receiver.fulfill(payment_body(order))
        with self.db() as db:
            row = db.execute("SELECT raw_json,store_version FROM accounts WHERE account_id='existing_player'").fetchone()
            self.assertEqual(row, {"raw_json": original, "store_version": 1})


if __name__ == "__main__":
    unittest.main(verbosity=2)
