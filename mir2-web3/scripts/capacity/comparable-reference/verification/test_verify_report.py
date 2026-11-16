"""Synthetic, in-memory contract tests. No processes, sockets or private stores."""
import copy
import importlib.util
import io
import json
import math
from pathlib import Path
import tempfile
import types
import unittest
from unittest import mock

SOURCE = Path(__file__).with_name("verify_report.py")
SPEC = importlib.util.spec_from_file_location("independent_verifier_tests", SOURCE)
v = importlib.util.module_from_spec(SPEC); SPEC.loader.exec_module(v)


def synthetic_plan(population=2,version=1):
    rules = json.loads((SOURCE.parent.parent / f"workload.v{version}.json").read_text())
    groups=[];cursor=0
    while cursor<population:
        size=3 if population-cursor==3 else 2;groups.append(list(range(cursor,cursor+size)));cursor+=size
    actors = [{"ordinal": i, "group": group_id, "peerOrdinals": [other for other in group if other!=i],
               "start": {"x": 20+2*(i%10), "y": 20+i//10},
               "lane": [{"x": 20+2*(i%10), "y": 20+i//10}, {"x": 21+2*(i%10), "y": 20+i//10}]}
              for group_id,group in enumerate(groups) for i in group]
    duration=60000 if population==2 else 300000
    plan = {"schema": "mir2.comparable.plan.v1", "planId": "12345678901234567890", "population": population,
            "purpose": "functional" if population==2 else "capacity", "durationMs": duration, "scenario": "dense-movement-chat", "qualificationLayer": "game-core-local",
            **{k: rules[k] for k in ["map", "actorTemplate", "thresholds", "resourceContract", "causality", "settleBeforeLogoutMs"]},
            "actors": actors, "equipmentActorOrdinals": list(range(min(3,population))), "equipmentRoundsPerActor": 2,
            "assessment": {"windowMs": 60000, "perActor": True, "allCompleteWindowsRequired": True,
                           "failedActionCanBecomeCompleted": False, "notSentIsTimeout": False,
                           "notSentIsCompleted": False, "safetyOrEvidenceFailureIsHardFailure": True},
            "capacityClaimAllowed": False}
    if version==2:
        plan["scheduling"]={"version":2,"chatPhaseRule":"same-primary-phase-with-midpoint-offset","primaryIntervalMs":2000,
                            "minimumSameActorOfferGapMs":1000,"chatOffsetWithinPrimaryIntervalMs":1000}
        plan["definitionSha256"]=v.digest((SOURCE.parent.parent/"workload.v2.json").read_bytes())
        plan["definitionCanonicalSha256"]=v.digest(v.encoded(rules,sorted_keys=True))
    actions = []
    for actor in actors:
        i = actor["ordinal"]
        rows = [{"actorOrdinal": i, "kind": "walk", "dueMs": due, "deadlineMs": due+2000,
                 "direction": "Right" if step % 2 == 0 else "Left", "target": actor["lane"][1 if step % 2 == 0 else 0]}
                for step, due in enumerate(range(i*2000//population, duration, 2000))]
        rows += [{"actorOrdinal": i, "kind": "chat", "dueMs": due, "deadlineMs": due+2500}
                 for due in range(15000+i*(2000 if version==2 else 1000)//population, duration, 30000)]
        for ticket, row in enumerate(sorted(rows, key=lambda r: (r["dueMs"], r["kind"])), 1):
            row["ticketSequence"] = ticket
            if row["kind"] == "chat":
                row["nonce"] = f"m2cmp:{plan['planId']}:{i}:{ticket}"
                row["nonceSha256"] = v.digest(row["nonce"].encode())
            actions.append(row)
    plan["actions"] = sorted(actions, key=lambda r: (r["dueMs"], r["actorOrdinal"], r["ticketSequence"]))
    return plan


def synthetic_actions(engine="modern", *, not_sent=(), late=None):
    plan, events, seq = synthetic_plan(), [], [1, 1]
    epoch = 100000
    report = {"schema": f"mir2.comparable.{engine}-report.v1", "synthetic": True, "planId": plan["planId"],
              "measurementEpochMs": epoch, "measurementStartedAtMs": epoch, "observationEndedAtMs": epoch+61000,
              "measurementEndedAtMs": epoch+60000,
              "processId": 7000, "ok": True, "stagePassed": True}
    for actor in range(2):
        if engine == "modern":
            events += [{"event": "admitted", "actorOrdinal": actor, "connection": 1, "atMs": epoch-1000},
                       {"event": "received", "actorOrdinal": actor, "connection": 1, "receiveSeq": 1,
                        "atMs": epoch-1000, "packet": "UserInformation", "payload": {"location": plan["actors"][actor]["start"]}}]
        else:
            events += [{"kind": "admitted", "actorOrdinal": actor, "connection": 1, "monotonicMs": epoch-1000},
                       {"kind": "packet", "direction": "received", "actorOrdinal": actor, "connection": 1,
                        "sequence": 1, "monotonicMs": epoch-1000, "packet": "UserInformation", "detail": {"position": plan["actors"][actor]["start"]}}]
    for action in plan["actions"]:
        actor, ticket = action["actorOrdinal"], action["ticketSequence"]
        key = actor, ticket
        when, deadline = epoch+action["dueMs"]+10, epoch+action["deadlineMs"]
        common = {**action, "dueAtMs": epoch+action["dueMs"], "plannedAtMs": epoch+action["dueMs"],
                  "deadlineAtMs": deadline, "expectedPeerOrdinals": [1-actor], "nonceHash": action.get("nonceSha256"),
                  "before": plan["actors"][actor]["lane"][0 if action.get("direction") == "Right" else 1],
                  "origin": plan["actors"][actor]["lane"][0 if action.get("direction") == "Right" else 1],
                  "action": action["kind"]}
        if engine == "crystal": events.append({**common, "kind": "offered", "offeredAtMs": when})
        if key in not_sent:
            result = {**common, "sentAtMs": None, "passed": False, "completedOnTime": False, "timeout": False, "timedOut": False}
            events.append({"event": "notSent", **result} if engine == "modern" else {"kind": "notSent", "result": result})
            if engine == "crystal": events.append({"kind": "actionOutcome", "result": result})
            continue
        floors = dict(enumerate(seq))
        if engine == "modern":
            events.append({"event": "sent", "actorOrdinal": actor, "connection": 1, "atMs": when,
                           "command": {"type": action["kind"], **({"direction": action["direction"]} if action["kind"] == "walk" else {"message": action["nonce"]})}})
            sent = {**common, "sentAtMs": when, "receiveSequenceFloors": [{"actorOrdinal": i, "connection": 1, "receiveSeq": s} for i, s in floors.items()],
                    "expectedSharedIds": [{"observerOrdinal": i, "connection": 1, "objectId": 50000+actor}
                                          for i in ([1-actor] if action["kind"] == "walk" else [actor, 1-actor])]}
            events.append({"event": "actionSent", **sent})
        else:
            seq[actor] += 1
            events.append({"kind": "packet", "direction": "sent", "actorOrdinal": actor, "connection": 1,
                           "sequence": seq[actor], "monotonicMs": when, "packet": "Walk" if action["kind"] == "walk" else "Chat",
                           "correlation": {"ticketSequence": ticket}})
            sent = {**common, "sentAtMs": when, "sentSequence": seq[actor], "expectedSharedId": 50000+actor, "receiveSequenceFloors": {str(i): s for i, s in floors.items()}}
            events.append({**sent, "kind": "sent"})
        receipts = []
        for observer in [actor, 1-actor]:
            seq[observer] += 1
            at = deadline+100 if key == late and observer != actor else when+100+(observer != actor)*10
            packet = "ObjectChat" if action["kind"] == "chat" else "UserLocation" if observer == actor else "ObjectWalk"
            body = ({"objectId": 50000+actor, "text": "synthetic: "+action["nonce"], "nonceHash": action["nonceSha256"]} if action["kind"] == "chat"
                    else {"objectId": 50000+actor, "location": action["target"], "position": action["target"]})
            if engine == "modern":
                events.append({"event": "received", "actorOrdinal": observer, "connection": 1, "receiveSeq": seq[observer], "atMs": at, "packet": packet, "payload": body})
                receipts.append({"observerOrdinal": observer, "connection": 1, "receiveSeq": seq[observer], "atMs": at, "packet": packet})
            else:
                events.append({"kind": "packet", "direction": "received", "actorOrdinal": observer, "connection": 1, "sequence": seq[observer], "monotonicMs": at, "packet": packet, "detail": body})
                detail = {"receiverActorOrdinal": observer, "connection": 1, "receiveSequence": seq[observer], "receivedAtMs": at, "packet": packet}
                events.append({"kind": "ownerReceipt" if observer == actor else "peerReceipt", "actorOrdinal": actor, "ticketSequence": ticket, "detail": detail})
        timed_out = key == late
        if timed_out:
            events.append({"event": "actionTimeout", **common, "atMs": deadline} if engine == "modern"
                          else {"kind": "timeout", "actorOrdinal": actor, "ticketSequence": ticket, "observedAtMs": deadline})
        outcome = {**sent, "receipts": receipts, "passed": not timed_out, "completedOnTime": not timed_out, "timedOut": timed_out,
                   "timeout": timed_out, "accepted": True, "completedAtMs": deadline+100 if timed_out else when+110}
        events.append({"event": "actionTerminal", **outcome} if engine == "modern" else {"kind": "actionOutcome", "result": outcome})
    return plan, report, events


def resource_row(role, pid, at, *, injector=False):
    cpus, cap, mask = (8, 4*2**30, "0xff0") if injector else (4, 8*2**30, "0xf")
    limits = {"logicalCpus": cpus, "hostLogicalCpus": 32, "affinityMaskHex": mask,
              "jobCommitLimitBytes": cap, "cpuRatePer10000": cpus*10000//32}
    return {"schema": "mir2.shared-service-job-sample.v1", "resourceSampleAtMs": at,
            "requestedLimits": limits, "effectiveLimits": {**limits, "limitFlags": 8720, "cpuControlFlags": 5},
            "membershipStable": True, "jobActiveProcesses": 1, "memberPids": [pid], "jobPeakCommitBytes": 100*2**20,
            "members": [{"pid": pid, "parentPid": 1, "role": role, "creationTime100ns": 100+pid,
                         "inOwnedJob": True, "queryError": None, "affinityMaskHex": mask}],
            "roles": {role: pid}, "roots": [{"role": role, "pid": pid, "creationTime100ns": 100+pid,
                "assignedBeforeResume": True, "exited": False}], "failedLaunches": [],
            "resourceQualification": {"qualified": True}}


def synthetic_resources(driver_duration=65000):
    ended=200+driver_duration; last=math.ceil(ended/1000)*1000
    server = [resource_row("gameServer", 6000, at) for at in [100]+list(range(1000,last+1,1000))]
    injector = [resource_row("injector", 7000, at, injector=True) for at in range(1000,int(ended//1000)*1000+1,1000)]
    meta = {key: {"sha256": str(i)*64, "bytes": 100+i} for i, key in enumerate(["plan", "summary", "events", "resources", "injector"], 1)}
    close = lambda role: {"closed": True, "allMembersExited": True, "killAppliedByClose": False, "failedLaunches": [], "rootExitCodes": {role: 0}}
    controller = {"schema": "mir2.comparable.stage-controller.v1", "qualificationLayer": "game-core-local", "engine": "modern",
                  "requiredRoles": ["gameServer"], "declaredDependencies": [], "resourceNominalIntervalMs": 1000,
                  "resourceSampleMaxGapMs": 2500, "actualExitCode": 0, "driverActualExitCode": 0, "serverActualExitCode": 0,
                  "normalStopProven": True, "abnormalCleanup": False, "serverJobClose": close("gameServer"),
                  "injectorJobClose": close("injector"), "gameJobRootPid": 6000, "injectorJobRootPid": 7000,
                  "planSha256": meta["plan"]["sha256"], "driverEvidence": {key: {**meta[key], "relativePath": key+".json"} for key in ["summary", "events"]},
                  "resourcesSha256": meta["resources"]["sha256"], "resourcesRowCount": len(server), "injectorResourcesSha256": meta["injector"]["sha256"],
                  "readyAtMs": 0, "driverStartedAtMs": 200, "driverExitedAtMs": ended, "normalShutdownStartedAtMs": last+100, "resourceEvidenceComplete": True}
    report = {"processId": 7000, "planSha256": meta["plan"]["sha256"], "eventsSha256": meta["events"]["sha256"],
              "measurementEpochMs":100000,"observationEndedAtMs":161000}
    return controller, server, injector, meta, report


def synthetic_native_lifecycle():
    plan, report, action_events = synthetic_actions("crystal")
    events, current = [], {0: 1, 1: 1}
    def state(actor, removed=False):
        slots = [{"grid": "inventory" if removed else "equipment", "slot": 6 if removed else 0,
                  "uniqueId": 100+actor, "itemIndex": 221, "count": 1, "currentDura": 4000,
                  "maxDura": 4000, "originalUserItemSaveSha256": v.digest(f"synthetic-item-{actor}".encode())}]
        owned = {"slots": slots, "gold": 0, "credit": 0,
                 "capacity": {"inventoryIncludingBelt": 46, "equipment": 14, "questInventory": 0, "belt": 6, "bag": 40},
                 "beltRule": "original Inventory indices0..5; alias of inventory, not duplicated item ownership"}
        return {"owned": owned, "fingerprintSha256": v.digest(v.encoded(owned))}
    def raw(actor, conn, sequence, at, name, body=None, direction="received"):
        events.append({"kind":"packet","direction":direction,"actorOrdinal":actor,"connection":conn,
                       "sequence":sequence,"monotonicMs":at,"packet":name,"detail":body or {}})
    def session(actor, conn, at, removed=False):
        raw(actor,conn,1,at,"Login",direction="sent")
        raw(actor,conn,2,at,"LoginSuccess")
        raw(actor,conn,3,at,"StartGame",direction="sent")
        raw(actor,conn,4,at,"StartGame")
        raw(actor,conn,5,at,"UserInformation",{"position":plan["actors"][actor]["start"],"hp":100,"level":40,
            "realCharacterIndex":actor+1,"nameHash":v.digest(f"synthetic-name-{actor}".encode()),
            "identityMatchesFixture":True,"authoritativeItemState":state(actor,removed)})
        events.append({"kind":"admitted","actorOrdinal":actor,"connection":conn,"monotonicMs":at})
    def exit(actor,conn,at,sequence):
        raw(actor,conn,sequence,at,"LogOut",direction="sent")
        raw(actor,conn,sequence+1,at+1,"LogOutSuccess")
        events.append({"kind":"transportClosed","result":{"actorOrdinal":actor,"connection":conn,
            "localTcpCloseCompleted":True,"receiveLoopCompleted":True,"startedAtMs":at+1,"closedAtMs":at+2}})
    for actor in range(2): session(actor,1,1000)
    for actor in range(2):
        for round_number in [1,2]:
            result={"actorOrdinal":actor,"round":round_number,"itemIdentityHash":v.digest(str(100+actor).encode())}
            for removed in [True,False]:
                at=2000+actor*1000+round_number*100+(0 if removed else 40)
                old=current[actor]; conn=old+1
                name="RemoveItem" if removed else "EquipItem"
                raw(actor,old,6,at,name,direction="sent")
                raw(actor,old,7,at+1,name,{"uniqueID":100+actor,"to":6 if removed else 0,"grid":1,"success":True})
                exit(actor,old,at+2,8)
                session(actor,conn,at+5,removed)
                ack={"sentSequence":6,"receiveSequence":7,"sentAtMs":at,"receivedAtMs":at+1,"deadlineAtMs":at+15000}
                fresh={"priorConnection":old,"connection":conn,"userInformationReceiveSequence":5,"userInformationReceivedAtMs":at+5}
                result["remove" if removed else "equip"]=ack
                result["removedState" if removed else "equippedState"]=fresh
                current[actor]=conn
            events.append({"kind":"equipmentRoundtrip","result":result})
    for event in action_events[4:]:
        if event.get("kind")=="packet": event["connection"]=current[event["actorOrdinal"]]; event["sequence"]+=4
        if event.get("kind")=="sent":
            event["sentSequence"]+=4
            event["receiveSequenceFloors"]={k:value+4 for k,value in event["receiveSequenceFloors"].items()}
        if event.get("kind") in ["ownerReceipt","peerReceipt"]:
            event["detail"]["connection"]=current[event["detail"]["receiverActorOrdinal"]]
            event["detail"]["receiveSequence"]+=4
        events.append(event)
    reentries=[]
    for actor in range(2):
        conn=current[actor]
        last=max(e["sequence"] for e in events if e.get("kind")=="packet" and e["actorOrdinal"]==actor and e["connection"]==conn)
        exit(actor,conn,200000,last+1)
        session(actor,conn+1,201000)
        exit(actor,conn+1,202000,6)
        reentries.append({"actorOrdinal":actor,"connection":conn+1,"passed":True})
    report.update(reentries=reentries,evidenceComplete=True,evidenceWriteFailed=False,evidenceBudgetExceeded=False,eventRows=len(events))
    disk={"schema":"mir2.comparable.crystal-disk-readback.v1","synthetic":True,"originalLoaderUsed":True,
          "accountFileSha256":"a"*64,"sampleCount":2,"samples":[{"actorOrdinal":actor,"identityMatchesFixture":True,
          "mapIndex":440,"position":plan["actors"][actor]["start"],"hp":100,"level":40,"itemState":state(actor)} for actor in range(2)]}
    return plan,report,events,disk


class ActionTests(unittest.TestCase):
    def verify(self, engine="modern", **kwargs):
        plan, report, events = synthetic_actions(engine, **kwargs)
        checks = v.Checks(); v.validate_plan(plan)
        windows, *unused = v.verify_actions(plan, report, events, engine, checks)
        return windows, checks, plan, report, events

    def test_both_adapter_schemas_recompute_full_minutes(self):
        for engine in ["modern", "crystal"]:
            windows, checks, *unused = self.verify(engine)
            self.assertEqual(checks.failures, [])
            self.assertEqual([(r["offered"], r["completed"], r["timeout"]) for r in windows], [(32, 32, 0)]*2)
            self.assertTrue(all(r["passed"] for r in windows))

    def test_one_notSent_chat_is_not_an_extra_veto_or_timeout(self):
        plan = synthetic_plan(); chat = next(a for a in plan["actions"] if a["actorOrdinal"] == 0 and a["kind"] == "chat")
        for engine in ["modern", "crystal"]:
            windows, checks, *unused = self.verify(engine, not_sent=[(0, chat["ticketSequence"])])
            self.assertEqual(checks.failures, [])
            self.assertEqual((windows[0]["completed"], windows[0]["notSent"], windows[0]["timeout"]), (31, 1, 0))
            self.assertTrue(windows[0]["passed"])

    def test_two_notSent_lower_completion_ratio(self):
        plan = synthetic_plan(); tickets = [(0, a["ticketSequence"]) for a in plan["actions"] if a["actorOrdinal"] == 0][-2:]
        windows, checks, *unused = self.verify(not_sent=tickets)
        self.assertEqual(checks.failures, [])
        self.assertFalse(windows[0]["passed"]); self.assertEqual(windows[0]["timeout"], 0)

    def test_late_complete_receipts_cannot_restore_original_success(self):
        last = [a for a in synthetic_plan()["actions"] if a["actorOrdinal"] == 0][-1]
        for engine in ["modern", "crystal"]:
            windows, checks, *unused = self.verify(engine, late=(0, last["ticketSequence"]))
            self.assertEqual(checks.failures, [])
            self.assertEqual((windows[0]["completed"], windows[0]["timeout"]), (31, 1))
            self.assertFalse(windows[0]["passed"])

    def test_lying_passed_flag_and_missing_anchor_fail(self):
        last = [a for a in synthetic_plan()["actions"] if a["actorOrdinal"] == 0][-1]
        p,r,e = synthetic_actions(late=(0,last["ticketSequence"]))
        next(x for x in reversed(e) if x.get("event") == "actionTerminal" and x["actorOrdinal"] == 0)["passed"] = True
        c=v.Checks(); v.verify_actions(p,r,e,"modern",c)
        self.assertIn("claimed_outcome_disagrees_with_raw_evidence", {x["code"] for x in c.failures})
        p,r,e=synthetic_actions(); next(x for x in e if x.get("event")=="actionSent").pop("dueAtMs")
        c=v.Checks(); v.verify_actions(p,r,e,"modern",c)
        self.assertIn("absolute_anchor_or_deadline_drift", {x["code"] for x in c.failures})

    def test_new_send_before_late_unique_terminal_is_ambiguous(self):
        p,r,e=synthetic_actions(late=(0,1)); c=v.Checks(); v.verify_actions(p,r,e,"modern",c)
        self.assertIn("new_send_before_previous_unique_terminal", {x["code"] for x in c.failures})

    def test_stale_connection_duplicate_packet_and_missing_raw_fail(self):
        for mode in ["connection", "duplicate", "missing"]:
            p,r,e=synthetic_actions(); row=next(x for x in e if x.get("event")=="actionSent")
            if mode=="connection": row["receiveSequenceFloors"][0]["connection"]=99
            elif mode=="duplicate": e.append(copy.deepcopy(next(x for x in e if x.get("event")=="received")))
            else: e.remove(next(x for x in e if x.get("event")=="received" and x.get("packet")=="ObjectWalk"))
            c=v.Checks(); v.verify_actions(p,r,e,"modern",c)
            self.assertTrue(c.failures)

    def test_p95_uses_nearest_rank_and_empty_is_unknown(self):
        self.assertEqual(v.p95(list(range(1,21))),19)
        self.assertIsNone(v.p95([]))


class ResourceTests(unittest.TestCase):
    def run_gate(self, change=None):
        controller, server, injector, meta, report = synthetic_resources()
        if change: change(controller,server,injector,meta,report)
        c=v.Checks(); out=v.verify_resources(synthetic_plan(),report,controller,server,injector,meta,c)
        return out,c

    def test_complete_declared_resource_gate(self):
        out,c=self.run_gate(); self.assertTrue(out["valid"]); self.assertEqual(c.failures,[])

    def test_resource_invalid_cannot_hide_behind_producer_true(self):
        for change in [lambda c,s,i,m,r:s[1].update(membershipStable=False),
                       lambda c,s,i,m,r:s[1].update(jobPeakCommitBytes=9*2**30),
                       lambda c,s,i,m,r:s[1]["members"][0].update(inOwnedJob=None),
                       lambda c,s,i,m,r:s[1]["effectiveLimits"].update(affinityMaskHex="0xff"),
                       lambda c,s,i,m,r:c.update(driverActualExitCode=1),
                       lambda c,s,i,m,r:c["serverJobClose"].update(killAppliedByClose=True),
                       lambda c,s,i,m,r:r.update(processId=7001),
                       lambda c,s,i,m,r:c["driverEvidence"]["events"].update(sha256="f"*64)]:
            out,c=self.run_gate(change); self.assertFalse(out["valid"]); self.assertTrue(c.failures)

    def test_missing_sample_gap_and_wrong_edges_fail(self):
        for change in [lambda c,s,i,m,r:s.__delitem__(slice(1,4)),
                       lambda c,s,i,m,r:s[0].update(resourceSampleAtMs=201),
                       lambda c,s,i,m,r:s[-1].update(resourceSampleAtMs=3499),
                       lambda c,s,i,m,r:i[0].update(resourceSampleAtMs=2900)]:
            out,c=self.run_gate(change); self.assertFalse(out["valid"])

    def test_faked_short_process_lifetime_cannot_support_a_minute(self):
        out,c=self.run_gate(lambda c,s,i,m,r:c.update(driverExitedAtMs=3000))
        self.assertFalse(out["valid"])
        self.assertIn("driver_lifetime_shorter_than_complete_workload_window",{x["code"] for x in c.failures})


class SchemaTests(unittest.TestCase):
    def test_duplicate_keys_nan_and_plan_budget_drift_fail_closed(self):
        for blob in [b'{"x":1,"x":2}',b'{"x":NaN}']:
            with self.assertRaises(v.EvidenceError): v.decode(blob)
        plan=synthetic_plan(); plan["thresholds"]["minimumCompletionRatio"]=.5
        with self.assertRaises(v.EvidenceError): v.validate_plan(plan)

    def test_full_carried_missing_or_duplicate_item_cannot_be_hash_only(self):
        with self.assertRaises(v.EvidenceError): v.carried({"fingerprint":"a"*64},"modern")
        item={"uniqueId":1,"slot":0,"quantity":1}
        with self.assertRaises(v.EvidenceError):
            v.carried({"inventoryItems":[item,item],"equipmentItems":[],"beltItems":[],"gold":0,"inventoryCapacity":46,"maxBagSlots":40},"modern")

    def test_partial_native_illustration_is_explicitly_insufficient(self):
        p=synthetic_plan(); controller,server,injector,meta,report=synthetic_resources()
        report.update(schema="mir2.comparable.crystal-report.v1",planId=p["planId"],synthetic=True)
        out=v.verify_bundle(p,report,[],controller,server,injector,meta)
        self.assertFalse(out["passed"])
        self.assertIn("actual_measurement_anchor_or_end_missing",{x["code"] for x in out["hardFailures"]})

    def test_bounded_stable_jsonl_read_rejects_partial_or_changed_input(self):
        class Input:
            def __init__(self,blob,changing=False,oversize=False):self.blob,self.changing,self.oversize,self.calls=blob,changing,oversize,0
            def is_symlink(self):return False
            def is_file(self):return True
            def stat(self):
                self.calls+=1
                return types.SimpleNamespace(st_size=v.MAX_FILE_BYTES+1 if self.oversize else len(self.blob),st_mtime_ns=self.calls if self.changing else 1)
            def open(self,mode):return io.BytesIO(self.blob)
        for fake in [Input(b'{"event":"synthetic"}'),Input(b'{}\n',changing=True),Input(b'',oversize=True)]:
            with mock.patch.object(v,"Path",lambda unused:fake),self.assertRaises(v.EvidenceError):v.read_file("synthetic",jsonl=True)
        fake=Input(b'{}\n')
        with mock.patch.object(v,"Path",lambda unused:fake):rows,meta=v.read_file("synthetic",jsonl=True)
        self.assertEqual(rows,[{}]);self.assertEqual(meta["bytes"],3)


class LifecycleTests(unittest.TestCase):
    def native(self, change=None):
        plan,report,events,disk=synthetic_native_lifecycle()
        if change: change(plan,report,events,disk)
        checks=v.Checks(); windows,actions,packets,admissions=v.verify_actions(plan,report,events,"crystal",checks)
        return v.verify_crystal_lifecycle(plan,report,events,packets,admissions,disk)

    def test_native_all_eight_strong_checks_and_all_disk_actors(self):
        result=self.native(); self.assertEqual(result["equipmentStrongChecks"],8)
        self.assertEqual(result["originalLoaderDiskActors"],2)

    def test_native_wrong_grid_item_fields_and_missing_disk_fail(self):
        changes=[lambda p,r,e,d:next(x for x in e if x.get("packet")=="RemoveItem" and x.get("direction")=="received")["detail"].update(grid=0),
                 lambda p,r,e,d:d["samples"][0]["itemState"]["owned"].update(gold=1),
                 lambda p,r,e,d:d.update(originalLoaderUsed=False),
                 lambda p,r,e,d:d["samples"].pop()]
        for change in changes:
            with self.subTest(change=change),self.assertRaises(v.EvidenceError): self.native(change)

    def test_native_same_socket_or_missing_normal_login_fail(self):
        for mode in ["socket","login"]:
            def change(p,r,e,d):
                if mode=="socket":next(x for x in e if x.get("kind")=="equipmentRoundtrip")["result"]["removedState"]["connection"]=1
                else:e.remove(next(x for x in e if x.get("kind")=="packet" and x.get("packet")=="Login"))
            with self.subTest(mode=mode),self.assertRaises(v.EvidenceError): self.native(change)

    def test_native_missing_strong_step_and_close_over_5s_fail(self):
        for mode in ["step","close"]:
            def change(p,r,e,d):
                if mode=="step":e.remove(next(x for x in e if x.get("kind")=="equipmentRoundtrip"))
                else:next(x for x in e if x.get("kind")=="transportClosed")["result"]["closedAtMs"]+=5001
            with self.subTest(mode=mode),self.assertRaises(v.EvidenceError): self.native(change)

    def test_full_synthetic_bundle_can_never_qualify_an_actual_run(self):
        p,r,e,d=synthetic_native_lifecycle()
        c,s,i,m,base=synthetic_resources(240000)
        c["engine"]="crystal";r.update(planSha256=base["planSha256"],eventsSha256=base["eventsSha256"])
        out=v.verify_bundle(p,r,e,c,s,i,m,disk=d)
        self.assertEqual(out["hardFailures"],[])
        self.assertTrue(out["consistentWithContract"])
        self.assertFalse(out["passed"]);self.assertFalse(out["capacityQualified"])


MODERN_SAMPLE = Path(tempfile.gettempdir()) / "mir2-comparable-capacity-20261113/modern-audit/synthetic-v2"


@unittest.skipUnless((MODERN_SAMPLE/"sample.report.json").is_file(),"Optional actual SDKMock schema fixture not installed")
class ModernSdkSchemaTests(unittest.TestCase):
    def data(self,complete_items=True):
        report=json.loads((MODERN_SAMPLE/"sample.report.json").read_text())
        self.assertTrue(report["synthetic"]);self.assertFalse(report["liveEvidence"])
        events=[json.loads(x) for x in (MODERN_SAMPLE/"sample.events.jsonl").read_text().splitlines()]
        if complete_items:
            # Build complete synthetic UserItem atoms in memory. The SDKMock's
            # original example intentionally has only a minimal item identity.
            # No file is rewritten and this is never actual-run evidence.
            states=[];hashes={}
            for e in events:
                if e.get("event")=="equipmentOriginalState":states.append(e["state"])
                if e.get("event")=="equipmentStep":states.append(e["freshState"])
                if e.get("event")=="savedVerificationState":states.extend([e["before"]["carried"],e["after"]["carried"]])
            for state in states:
                old=v.carried(state,"modern")["fingerprint"]
                for grid in ["inventoryItems","equipmentItems","beltItems"]:
                    for item in state[grid]:
                        atom={"unique_id":item["uniqueId"],"item_index":int(item["key"].rsplit("-",1)[1]),"count":item["quantity"],
                              "current_dura":item.get("durabilityCurrent",0),"max_dura":item.get("durabilityMax",0),"added_stats":[],
                              "awake_type":0,"awake_values":[],"cursed":False,"gem_count":0,"gm_made":False,"identified":True,
                              "is_shop_item":False,"refined_value":0,"refine_added":0,"refine_success_chance":0,
                              "rental_information":None,"sealed_info":None,"expire_info":None,"slots":[],"soul_bound_id":-1,"wedding_ring":-1}
                        item.setdefault("tooltipSource",{})["userItem"]=atom
                hashes[old]=v.carried(state,"modern")["fingerprint"]
            for e in events:
                if e.get("event")=="received" and e.get("type")=="worldSnapshot" and e.get("carriedFingerprint")in hashes:
                    e["carriedFingerprint"]=hashes[e["carriedFingerprint"]]
        return json.loads((MODERN_SAMPLE/"sample.plan.json").read_text()),report,events

    def lifecycle(self,change=None):
        p,r,e=self.data()
        if change:change(p,r,e)
        c=v.Checks();w,a,packets,admissions=v.verify_actions(p,r,e,"modern",c)
        self.assertEqual(c.failures,[])
        return v.verify_modern_lifecycle(p,r,e,packets,admissions,c)

    def test_actual_sdk_schema_all_eight_and_saved_rosters(self):
        self.assertEqual(self.lifecycle()["equipmentStrongChecks"],8)

    def test_incomplete_original_sdk_item_atom_is_insufficient(self):
        p,r,e=self.data(complete_items=False);c=v.Checks()
        w,a,packets,admissions=v.verify_actions(p,r,e,"modern",c)
        with self.assertRaisesRegex(v.EvidenceError,"complete_original_UserItem_projection_missing"):
            v.verify_modern_lifecycle(p,r,e,packets,admissions,c)

    def test_second_round_cannot_switch_from_initial_weapon_to_armour(self):
        def change(p,r,e):
            original=next(x for x in e if x.get("event")=="equipmentOriginalState" and x["actorOrdinal"]==0)
            armour=next(i for i in original["state"]["equipmentItems"] if i["slot"]=="armour")
            next(x for x in e if x.get("event")=="equipmentStep" and x["actorOrdinal"]==0 and x["round"]==1)["uniqueId"]=armour["uniqueId"]
        with self.assertRaisesRegex(v.EvidenceError,"both_equipment_rounds_must_use_initial_weapon_UID"):self.lifecycle(change)

    def test_changed_original_item_field_rejected_even_with_rebound_snapshot_hash(self):
        def change(p,r,e):
            step=next(x for x in e if x.get("event")=="equipmentStep")
            item=next(i for i in step["freshState"]["inventoryItems"] if i["uniqueId"]==step["uniqueId"])
            item["tooltipSource"]["userItem"]["refined_value"]+=1
            ref=step["state"]
            raw=next(x for x in e if x.get("event")=="received" and x["actorOrdinal"]==step["actorOrdinal"] and x["connection"]==ref["connection"] and x["receiveSeq"]==ref["receiveSeq"])
            raw["carriedFingerprint"]=v.carried(step["freshState"],"modern")["fingerprint"]
        with self.assertRaisesRegex(v.EvidenceError,"equipment_original_UserItem_fields_changed"):self.lifecycle(change)

    def test_prelogin_decoy_cannot_replace_authenticated_gear_baseline(self):
        def change(p,r,e):
            actor=0;first=next(x for x in e if x.get("event")=="equipmentStep" and x["actorOrdinal"]==actor)
            connection=first["ack"]["connection"]
            def shift(node):
                if isinstance(node,dict):
                    if node.get("connection")==connection and node.get("actorOrdinal",node.get("observerOrdinal"))==actor and type(node.get("receiveSeq"))is int:node["receiveSeq"]+=1
                    for value in node.values():shift(value)
                elif isinstance(node,list):
                    for value in node:shift(value)
            shift(e)
            decoy=copy.deepcopy(next(x for x in e if x.get("event")=="received" and x.get("type")=="worldSnapshot" and x["actorOrdinal"]==actor and x["connection"]==connection))
            decoy.update(receiveSeq=1,carriedFingerprint="0"*64)
            e.insert(0,decoy)
        self.assertEqual(self.lifecycle(change)["equipmentStrongChecks"],8)

    def test_newer_authenticated_state_cannot_be_ignored_before_mutation(self):
        def change(p,r,e):
            actor=0;first=next(x for x in e if x.get("event")=="equipmentStep" and x["actorOrdinal"]==actor)
            connection=first["ack"]["connection"]
            sent_index=next(i for i,x in enumerate(e) if x.get("event")=="sent" and x.get("actorOrdinal")==actor and x.get("connection")==connection and x.get("command",{}).get("type")=="removeItem")
            raw=next(x for x in reversed(e[:sent_index]) if x.get("event")=="received" and x.get("type")=="worldSnapshot" and x["actorOrdinal"]==actor and x["connection"]==connection)
            raw["carriedFingerprint"]="f"*64
        with self.assertRaises(v.EvidenceError):self.lifecycle(change)

    def test_sdk_hash_only_gear_and_fake_close_are_insufficient(self):
        for mode in ["roster","close","step","save"]:
            def change(p,r,e):
                if mode=="roster":next(x for x in e if x.get("event")=="equipmentStep").pop("freshState")
                elif mode=="close":next(x for x in e if x.get("event")=="transportClosed")["validCloseFrameReceived"]=False
                elif mode=="step":e.remove(next(x for x in e if x.get("event")=="equipmentStep"))
                else:next(x for x in e if x.get("event")=="savedVerificationState")["after"].pop("carried")
            with self.subTest(mode=mode),self.assertRaises(v.EvidenceError):self.lifecycle(change)

if __name__=="__main__": unittest.main()
