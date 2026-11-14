#!/usr/bin/env python3
"""Review before execution. Exact ELF, owned loopback/File prepared cohort only."""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import secrets
import shutil
import signal
import socket
import subprocess
import sys
import time
import urllib.request

from guarded_seed import ELF_SHA, ROOT, SOURCE, StoppedOwnedRealm, seed_stopped_realm


def utc():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def digest(file):
    result = hashlib.sha256()
    with Path(file).open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            result.update(chunk)
    return result.hexdigest()


def verify_inputs():
    manifest = json.loads((ROOT / "FROZEN-INPUTS.json").read_text(encoding="utf-8-sig"))
    for row in manifest["files"]:
        file = (ROOT / row["relativePath"]).resolve(strict=True)
        if not file.is_relative_to(ROOT) or file.is_symlink() or file.stat().st_size != row["bytes"] or digest(file) != row["sha256"]:
            raise RuntimeError("Frozen dense fixture input changed: " + row["relativePath"])
    return manifest


def port():
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


def false_acceptance():
    return {"fullP1Accepted": False, "nativeInputAccepted": False, "nativeVisualAccepted": False, "humanAccepted": False, "ordinaryProgressionAccepted": False, "loadAccepted": False}


def new_env(store, origin, tcp_port, memory_keys):
    env = {k: v for k, v in os.environ.items() if not k.upper().startswith(("MIR2_", "CRYSTAL_"))}
    env.update(memory_keys)
    env.update({
        "MIR2_GATEWAY_WEB_ADDR": origin.removeprefix("http://"),
        "MIR2_GATEWAY_TCP_ADDR": "127.0.0.1:" + str(tcp_port),
        "MIR2_ACCOUNT_STORE_PATH": str(store), "MIR2_ACCOUNT_STORE_BACKEND": "file",
        "MIR2_RUNTIME_ENV": "development", "MIR2_CONTENT_PROFILE": "platinum_176",
        "MIR2_SAVE_RECOVERY_DIR": str(store.parent / "save-recovery"),
        "MIR2_GATEWAY_ALLOW_UNSAFE_LOCAL_PLAYER_COMMANDS": "0",
        "MIR2_GATEWAY_STDIN_SHUTDOWN": "0", "MIR2_ALLOWED_WEB_ORIGINS": origin,
        "MIR2_CRYSTAL_MAP_PACK": str(ROOT / "maps"),
    })
    return env


def health(origin):
    with urllib.request.urlopen(origin + "/health", timeout=1.5) as response:
        return json.load(response)


def active_count(value):
    count = value.get("capacity", {}).get("currentActiveSessions")
    if type(count) is not int or count < 0:
        raise RuntimeError("Actual capacity.currentActiveSessions evidence is required; missing/unknown is not zero")
    return count


def stage(binary, node, accounts, store, output, phase, keys, args):
    if digest(binary) != ELF_SHA:
        raise RuntimeError("Exact ELF changed between isolated stages")
    web_port, tcp_port = port(), port()
    while tcp_port == web_port:
        tcp_port = port()
    origin = "http://127.0.0.1:" + str(web_port)
    env = new_env(store, origin, tcp_port, keys)
    raw_log = store.parent / (phase + ".gateway.private.log")
    stdout = output / (phase + ".driver.stdout.log")
    phase_receipts = output / (phase + "-receipts")
    process = None
    stage_report = {"phase": phase, "origin": origin, "startedUtc": utc(), "binarySha256": ELF_SHA,
                    "ownedGatewayNormalExitCode": None, "qaEnabled": False,
                    "randomSecretsPersisted": False, "driverReport": None,
                    "gatewaysRawLogPublic": False, **false_acceptance()}
    try:
        with raw_log.open("xb") as log:
            process = subprocess.Popen([str(binary)], cwd=store.parent, env=env,
                                       stdin=subprocess.DEVNULL, stdout=log,
                                       stderr=subprocess.STDOUT, start_new_session=True)
            stage_report["ownedGatewayPid"] = process.pid
            deadline = time.monotonic() + 45
            while time.monotonic() < deadline:
                if process.poll() is not None:
                    raise RuntimeError("Owned Gateway exited before readiness")
                try:
                    initial = health(origin)
                    if initial.get("ok"):
                        if active_count(initial) != 0:
                            raise RuntimeError("Fresh isolated listener already has an active session")
                        stage_report["initialHealth"] = initial
                        break
                except (OSError, ValueError):
                    time.sleep(0.1)
            else:
                raise RuntimeError("Owned Gateway readiness exceeded 45 seconds")
            request = {
                "phase": phase, "binarySha256": ELF_SHA, "origin": origin,
                "output": str(phase_receipts), "accounts": accounts,
                "formationBudgetMs": args.formation_seconds * 1000,
                "travelBudgetMs": args.travel_seconds * 1000,
                "deathBudgetMs": args.death_seconds * 1000,
                "trials": args.trials, "includeSurround": args.include_surround,
            }
            if phase == "dense":
                timeout = 3 * ((4 * args.trials + int(args.include_surround)) * (args.formation_seconds + 35)) + 180
            elif phase == "travel":
                timeout = 3 * (2 * args.travel_seconds + 140) + 120
            elif phase == "death":
                timeout = 3 * (args.death_seconds + 75) + 120
            else:
                timeout = 1500
            # Neither account passwords nor random identity keys appear in argv.
            with stdout.open("xb") as log_out:
                driver = subprocess.run([node, str(ROOT / "dense_network_driver.mjs")],
                                        input=json.dumps(request).encode("utf-8"),
                                        stdout=log_out, stderr=subprocess.STDOUT,
                                        timeout=timeout, cwd=ROOT, check=False)
            stage_report["driverExitCode"] = driver.returncode
            report_file = phase_receipts / "report.json"
            if not report_file.is_file():
                raise RuntimeError("Driver produced no structured report; retain failure logs")
            driver_report = json.loads(report_file.read_text(encoding="utf-8-sig"))
            stage_report["driverReport"] = {"relativePath": str(report_file.relative_to(output)), "sha256": digest(report_file)}
            stage_report["classes"] = [{"className": row["className"], "status": row["status"], "normalLogoutConfirmed": row["normalLogoutConfirmed"]} for row in driver_report["classes"]]
            stage_report["normalLogoutConfirmed"] = driver_report.get("normalLogoutConfirmed") is True
            deadline = time.monotonic() + 15
            while time.monotonic() < deadline:
                settled = health(origin)
                if active_count(settled) == 0:
                    stage_report["settledHealth"] = settled
                    break
                time.sleep(0.1)
            else:
                raise RuntimeError("Owned fixture did not drain normal sessions before offline work")
            if driver.returncode != 0 or not stage_report["normalLogoutConfirmed"]:
                raise RuntimeError("Driver/socket cleanup did not complete normally; offline seed prohibited")
    except Exception as error:
        stage_report["executionError"] = str(error)
    finally:
        if process is not None:
            if process.poll() is None:
                process.send_signal(signal.SIGTERM)
                try:
                    process.wait(timeout=15)
                except subprocess.TimeoutExpired:
                    stage_report["operatorAttention"] = "Owned PID did not stop on SIGTERM. No force-kill/seed/other process action performed."
            stage_report["ownedGatewayNormalExitCode"] = process.poll()
        if raw_log.is_file():
            stage_report["privateRawLogHash"] = digest(raw_log)
        if store.is_file():
            stage_report["privateStoreSha256"] = digest(store)
        stage_report["completedUtc"] = utc()
        (output / (phase + "-stage.json")).write_text(json.dumps(stage_report, indent=2) + "\n", encoding="utf-8")
    if stage_report.get("executionError") or stage_report["ownedGatewayNormalExitCode"] != 0 or not stage_report.get("normalLogoutConfirmed"):
        raise RuntimeError("Stage " + phase + " incomplete; review retained per-class receipts and owned exit")
    guard = StoppedOwnedRealm(process, store, store.parent, ELF_SHA)
    guard.check()
    return guard, driver_report, stage_report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--verify-only", action="store_true")
    parser.add_argument("--binary", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--node", default="node")
    parser.add_argument("--travel-mode", choices=["legal", "prepared"], default="legal")
    parser.add_argument("--formation-seconds", type=int, default=90)
    parser.add_argument("--travel-seconds", type=int, default=600)
    parser.add_argument("--death-seconds", type=int, default=180)
    parser.add_argument("--trials", type=int, default=3)
    parser.add_argument("--include-surround", action="store_true")
    parser.add_argument("--include-death", action="store_true")
    args = parser.parse_args()
    frozen = verify_inputs()
    if args.verify_only:
        print(json.dumps({"frozenInputsVerified": True, "fileCount": len(frozen["files"]), "noGatewayStarted": True, **false_acceptance()}))
        return 0
    if os.name != "posix" or not args.binary or not args.output:
        raise RuntimeError("This owned SIGTERM wrapper requires Linux, an exact ELF and a NEW output child")
    if not 1 <= args.trials <= 5 or not 30 <= args.formation_seconds <= 300 or not 30 <= args.travel_seconds <= 1200 or not 30 <= args.death_seconds <= 600:
        raise RuntimeError("Declared time/trial bounds are outside fixture limits")
    binary = args.binary.resolve(strict=True)
    if digest(binary) != ELF_SHA or binary.read_bytes()[:4] != b"\x7fELF":
        raise RuntimeError("Only authenticated unchanged R17 release ELF is accepted")
    node = shutil.which(args.node)
    if not node:
        raise RuntimeError("Node 22.18+ required")
    output = args.output.resolve()
    if output == ROOT or not output.is_relative_to(ROOT) or output.exists():
        raise RuntimeError("Output must be a fresh child of this NEW external fixture directory")
    os.umask(0o077)
    output.mkdir(mode=0o700)
    private = output / "private-realm"
    private.mkdir(mode=0o700)
    store = private / "accounts.json"
    nonce = secrets.token_hex(5)
    accounts = [{"className": name, "accountId": "dn" + nonce + str(i), "name": "D" + str(i) + nonce,
                 "password": secrets.token_hex(16)} for i, name in enumerate(["Warrior", "Wizard", "Taoist"])]
    keys = {name: secrets.token_hex(32) for name in ["MIR2_SAVE_RECOVERY_MAC_KEY", "MIR2_IDENTITY_SESSION_SECRET", "MIR2_IDENTITY_RECOVERY_PEPPER", "MIR2_PASSKEY_AUTH_SECRET"]}
    report = {"schema": "mir2.prepared-dense-network-wrapper.v2", "sourceRevision": SOURCE, "binarySha256": ELF_SHA,
              "startedUtc": utc(), "preparedState": True, "freshFileRealm": True,
              "noProductionRedisPostgres": True, "liveStoresTouched": False,
              "loopbackOnly": True, "unsafeQaEnabled": False, "credentialsInMemoryOnly": True,
              "originalMonsterStatsClock": True, "trainingCadenceOverridesCleared": True,
              "travelMode": args.travel_mode, "frozenInputsSha256": digest(ROOT / "FROZEN-INPUTS.json"),
              "stages": [], "offlineWrites": [], "r16Comparison": {"status": "blocked", "reason": "Authenticated R16 ELF exists; a separately reviewed R16 allowlist wrapper and identical-checkpoint comparison have not been executed", "authenticatedR16Sha256": "7a8b7305d7f5b748d81e502683a4f5d6580cfecbe5bf87cf8462b930224f690f", "r17ExecutableAllowlistUnchanged": True},
              "normalProcessCompletion": False, "sevenMonsterMechanicsAccepted": False, **false_acceptance()}
    try:
        guard, bootstrap, receipt = stage(binary, node, accounts, store, output, "bootstrap", keys, args)
        report["stages"].append(receipt)
        if any(row["status"] != "completed" for row in bootstrap["classes"]):
            raise RuntimeError("Fresh ordinary bootstrap failed; no prepared fields may be written")
        level_diff = seed_stopped_realm(guard, digest(store), accounts, "level")
        report["offlineWrites"].append(level_diff)
        (output / "level-field-diff.json").write_text(json.dumps(level_diff, indent=2) + "\n", encoding="utf-8")
        guard, purchases, receipt = stage(binary, node, accounts, store, output, "purchases", keys, args)
        report["stages"].append(receipt)
        if args.travel_mode == "legal":
            guard, travel, receipt = stage(binary, node, accounts, store, output, "travel", keys, args)
            report["stages"].append(receipt)
            if any(row["status"] != "completed" for row in travel["classes"]):
                raise RuntimeError("Normal route did not finish for every class; no silent prepared travel fallback")
        else:
            planning = subprocess.run([node, str(ROOT / "dense_network_driver.mjs"), "--plan-staging"], stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=20, check=True, cwd=ROOT)
            staging = json.loads(planning.stdout)
            travel_diff = seed_stopped_realm(guard, digest(store), accounts, "travel", staging=staging)
            report["offlineWrites"].append(travel_diff)
            report["preparedStaging"] = staging
            (output / "travel-field-diff.json").write_text(json.dumps(travel_diff, indent=2) + "\n", encoding="utf-8")
        # Sealed private checkpoint is server-produced ordinary purchases/books
        # plus the declared offline field writes. No monster or item writes.
        checkpoint = private / "sealed-prepared-checkpoint.private.json"
        shutil.copyfile(store, checkpoint)
        os.chmod(checkpoint, 0o600)
        report["sealedCheckpointSha256"] = digest(checkpoint)
        checkpoint_data = json.loads(checkpoint.read_text(encoding="utf-8-sig"))
        for account in accounts:
            record = checkpoint_data["accounts"][account["accountId"]]
            account["expectedBindPoint"] = next(iter(record["saves"].values()))["bind_point"]
        guard, dense, receipt = stage(binary, node, accounts, store, output, "dense", keys, args)
        report["stages"].append(receipt)
        report["sevenMonsterMechanicsAccepted"] = dense.get("mechanicsAllCasesPassed") is True
        if args.include_death:
            # Separate copy preserves dense results, original stats and item UID
            # inventory. Death is actual AI damage, never an offline HP edit.
            death_private = output / "private-death-clone"
            death_private.mkdir(mode=0o700)
            death_store = death_private / "accounts.json"
            shutil.copyfile(checkpoint, death_store)
            report["deathCloneInputSha256"] = digest(death_store)
            _, death, receipt = stage(binary, node, accounts, death_store, output, "death", keys, args)
            report["stages"].append(receipt)
        report["normalProcessCompletion"] = True
    except Exception as error:
        report["executionError"] = str(error)
    finally:
        report["completedUtc"] = utc()
        # Public receipts are an allowlisted package. Keep private hashes only;
        # never publish complete stores, password hashes, random signing keys or
        # recovery files. Public JSONL events retain all sanitized failed trials.
        public = [p for p in output.rglob("*") if p.is_file() and not any(part.startswith("private-") for part in p.relative_to(output).parts)]
        report["publicReceiptFiles"] = [{"relativePath": str(p.relative_to(output)), "sha256": digest(p), "bytes": p.stat().st_size} for p in sorted(public)]
        (output / "wrapper-report.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"normalProcessCompletion": report["normalProcessCompletion"], "sevenMonsterMechanicsAccepted": report["sevenMonsterMechanicsAccepted"], "executionError": report.get("executionError"), **false_acceptance()}))
    return 0 if report["normalProcessCompletion"] else 2


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as error:
        print(json.dumps({"fatalFixtureError": str(error), **false_acceptance()}))
        sys.exit(2)
