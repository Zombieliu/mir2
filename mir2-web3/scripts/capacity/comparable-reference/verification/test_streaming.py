"""Small, in-memory files only. No games, sockets, writes or large case reads."""
import copy
import importlib.util
import io
import json
from pathlib import Path
import types
import unittest
from unittest import mock

SPEC=importlib.util.spec_from_file_location("stream_fixtures",Path(__file__).with_name("test_verify_report.py"))
t=importlib.util.module_from_spec(SPEC);SPEC.loader.exec_module(t)
v=t.v


class FakeFile:
    def __init__(self,blob,second=None):self.blob,self.second,self.opens=blob,second,0
    def is_symlink(self):return False
    def is_file(self):return True
    def stat(self):return types.SimpleNamespace(st_size=len(self.blob),st_mtime_ns=1,st_ino=2,st_dev=3)
    def open(self,mode):
        self.opens+=1
        return io.BytesIO(self.second if self.opens>1 and self.second is not None else self.blob)


def load(events,engine="crystal",second=None):
    blob=b"".join(v.encoded(e)+b"\n" for e in events)
    assert len(blob)<2**20  # This suite never opens an actual large file.
    fake=FakeFile(blob,second)
    with mock.patch.object(v,"Path",lambda unused:fake):rows,meta=v.read_events("in-memory",engine,2)
    return rows,meta,fake


class PhaseTests(unittest.TestCase):
    def test_corrected_phase_all_populations_keeps_counts_and_midpoint(self):
        for population in [2,25,50,100]:
            plan=t.synthetic_plan(population,2);v.validate_plan(plan)
            for actor in range(population):
                rows=[a for a in plan["actions"] if a["actorOrdinal"]==actor]
                self.assertEqual(len(rows),32 if population==2 else 160)
                times=sorted(a["dueMs"] for a in rows)
                self.assertGreaterEqual(min(b-a for a,b in zip(times,times[1:])),1000)
                self.assertEqual(next(a["dueMs"] for a in rows if a["kind"]=="chat"),15000+actor*2000//population)

    def test_legacy_ten_ms_plan_stays_legacy_not_promoted(self):
        legacy=t.synthetic_plan(100,1);v.validate_plan(legacy)
        rows=[a["dueMs"] for a in legacy["actions"] if a["actorOrdinal"]==99]
        self.assertEqual(min(b-a for a,b in zip(sorted(rows),sorted(rows)[1:])),10)
        disguised=copy.deepcopy(legacy);disguised["scheduling"]=t.synthetic_plan(100,2)["scheduling"]
        with self.assertRaises(v.EvidenceError):v.validate_plan(disguised)

    def test_unknown_or_reduced_phase_declaration_rejected(self):
        for key,value in [("version",3),("minimumSameActorOfferGapMs",10),("chatPhaseRule","legacy")]:
            plan=t.synthetic_plan(2,2);plan["scheduling"][key]=value
            with self.subTest(key=key),self.assertRaises(v.EvidenceError):v.validate_plan(plan)


class StreamingTests(unittest.TestCase):
    def test_two_pass_filter_retains_verdict_full_count_order_and_hash(self):
        p,r,e,d=t.synthetic_native_lifecycle()
        noise=[{"kind":"packet","direction":"received","actorOrdinal":0,"connection":99,"sequence":i+1,
                "monotonicMs":1100+i,"packet":"ObjectWalk","detail":{"objectId":999,"position":{"x":700,"y":700}},
                "synthetic":True} for i in range(500)]
        full=noise+e;rows,meta,fake=load(full)
        self.assertEqual(fake.opens,2);self.assertEqual(meta["rows"],len(full));self.assertEqual(rows.total_rows,len(full))
        self.assertEqual(len(rows),len(e));self.assertEqual(rows,list(e));self.assertTrue(rows.any_synthetic)
        self.assertFalse(any(x.get("synthetic") for x in rows))
        c,s,i,m,base=t.synthetic_resources(240000);c["engine"]="crystal"
        m["events"]=meta;c["driverEvidence"]["events"].update(sha256=meta["sha256"],bytes=meta["bytes"])
        r.update(eventRows=len(full),planSha256=base["planSha256"],eventsSha256=meta["sha256"])
        a=v.verify_bundle(p,r,full,c,s,i,m,disk=d);b=v.verify_bundle(p,r,rows,c,s,i,m,disk=d)
        self.assertTrue(a["consistentWithContract"],a["hardFailures"]);self.assertTrue(b["consistentWithContract"],b["hardFailures"])
        self.assertEqual(a["windows"],b["windows"]);self.assertEqual(a["lifecycle"],b["lifecycle"])
        self.assertFalse(b["passed"]);self.assertEqual(b["eventReadAudit"]["fullRows"],len(full))

    def test_required_action_receipt_is_retained_for_both_schemas(self):
        for engine in ["modern","crystal"]:
            p,r,e=t.synthetic_actions(engine);rows,meta,fake=load(e,engine)
            c=v.Checks();windows,*unused=v.verify_actions(p,r,rows,engine,c)
            self.assertEqual(c.failures,[]);self.assertTrue(all(w["passed"] for w in windows))
            self.assertGreater(meta["streamAudit"]["referenceKeys"],0)

    def test_unselected_raw_duplicate_or_bad_json_still_fails(self):
        noise={"kind":"packet","direction":"received","actorOrdinal":0,"connection":99,"sequence":1,
               "monotonicMs":1,"packet":"ObjectWalk","detail":{}}
        with self.assertRaises(v.EvidenceError):load([noise,noise])
        for blob in [b'{"unused":{"x":1,"x":2}}\n',b'{"unused":1e400}\n',b'{}',b'\n']:
            fake=FakeFile(blob)
            with mock.patch.object(v,"Path",lambda unused:fake),self.assertRaises(v.EvidenceError):v.read_events("memory","crystal",2)

    def test_changed_between_passes_and_bounds_never_truncate(self):
        event={"kind":"note","value":"noise-a"};blob=v.encoded(event)+b"\n"
        fake=FakeFile(blob,blob.replace(b'noise-a',b'noise-b'))
        with mock.patch.object(v,"Path",lambda unused:fake),self.assertRaisesRegex(v.EvidenceError,"event_file_changed_between_full_scans"):
            v.read_events("memory","crystal",2)
        for name,value in [("MAX_EVENT_LINE_BYTES",3),("MAX_EVENT_ROWS",0),("MAX_SELECTED_ROWS",0),("MAX_SELECTED_BYTES",1)]:
            with mock.patch.object(v,name,value),self.subTest(bound=name),self.assertRaises(v.EvidenceError):load([event])


if __name__=="__main__":unittest.main()
