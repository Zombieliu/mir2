"""Synthetic in-memory File authority tests; no real accounts or passwords."""
import copy
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

SPEC=importlib.util.spec_from_file_location("modern_disk_tests",Path(__file__).with_name("read_modern_disk.py"))
m=importlib.util.module_from_spec(SPEC);SPEC.loader.exec_module(m)


def fixture():
    meta={"item_index":221,"awake_type":0,"awake_values":[],"refined_value":0,"refine_added":0,"refine_success_chance":0,
          "wedding_ring":-1,"expire_info":None,"rental_information":None,"sealed_info":None,"slots":[],"is_shop_item":False,"gm_made":False}
    item={"key":"crystal-item-221","slot":"weapon","quantity":1,"durability_current":4000,"durability_max":4000,
          "identified":True,"soul_bound_id":None,"cursed":False,"gem_count":0,"added_stats":[],"added_attack":0,"added_defence":0,
          "added_luck":0,"socketed":[],"socket_slots":0,"user_item_metadata":meta,"user_item_unique_id":12345,
          "awake_type":0,"awake_values":[],"rental_binding_flags":0,"rental_owner_name":"","rental_expiry_binary_datetime":0,
          "rental_locked":False,"sealed_expiry_time_binary_datetime":0,"sealed_next_time_binary_datetime":0}
    atom={"unique_id":12345,"item_index":221,"count":1,"current_dura":4000,"max_dura":4000,"added_stats":[],
          "awake_type":0,"awake_values":[],"cursed":False,"gem_count":0,"identified":True,"soul_bound_id":-1,
          "refined_value":0,"refine_added":0,"refine_success_chance":0,"wedding_ring":-1,"expire_info":None,
          "rental_information":None,"sealed_info":None,"slots":[],"is_shop_item":False,"gm_made":False}
    char={"index":101,"name":"SyntheticFixture","level":40,"class":"Warrior","gender":"Male"}
    save={"character":char,"map_file_name":"DogYoHyun","position":{"x":55,"y":51},"direction":"Left","hp":674,"mp":151,
          "gold":123,"inventory_capacity":46,"inventory_items_json":[],"belt_items_json":[],"equipment_items_json":[json.dumps(item)]}
    store={"accounts":{"private-account-key-canary":{"password":"PASSWORD-CANARY","storage_password":"AUTHHASH-CANARY",
           "ledger":"LEDGER-CANARY","gm_level":0,"is_banned":False,"characters":[char],"saves":{"101":save}}}}
    carried={"inventoryItems":[],"beltItems":[],"equipmentItems":[{"uniqueId":12345,"key":"crystal-item-221","slot":"weapon",
              "quantity":1,"durabilityCurrent":4000,"durabilityMax":4000,"tooltipSource":{"userItem":atom}}],
             "gold":123,"inventoryCapacity":46,"maxBagSlots":40}
    state={"name":char["name"],"level":40,"class":"Warrior","gender":"Male","map":"DogYoHyun","x":55,"y":51,
           "direction":"Left","hp":674,"mp":151,"carried":carried}
    return store,{0:state}


class FileReadbackTests(unittest.TestCase):
    def test_persisted_character_save_and_complete_items_match(self):
        store,states=fixture();result=m.inspect_store(store,states)
        self.assertEqual(len(result),1);self.assertTrue(result[0]["completeItemsAndGridMatch"])
        self.assertEqual(result[0]["position"],{"x":55,"y":51})

    def test_password_auth_ledger_and_account_keys_never_exported(self):
        store,states=fixture();text=json.dumps(m.inspect_store(store,states))
        for value in ["PASSWORD-CANARY","AUTHHASH-CANARY","LEDGER-CANARY","private-account-key-canary","SyntheticFixture"]:
            self.assertNotIn(value,text)
        for key in ["password","storage_password","ledger","accountId"]:self.assertNotIn(key,text)

    def test_actual_persisted_position_map_gold_and_capacity_change_rejected(self):
        for field,value in [("position",{"x":56,"y":51}),("map_file_name","Other"),("gold",124),("inventory_capacity",54),("mp",152)]:
            store,states=fixture();store["accounts"]["private-account-key-canary"]["saves"]["101"][field]=value
            with self.subTest(field=field),self.assertRaises(m.v.EvidenceError):m.inspect_store(store,states)

    def test_uid_count_durability_and_full_metadata_cannot_be_hash_only(self):
        for field,value in [("user_item_unique_id",12346),("quantity",2),("durability_current",3999),("identified",False)]:
            store,states=fixture();save=store["accounts"]["private-account-key-canary"]["saves"]["101"]
            item=json.loads(save["equipment_items_json"][0]);item[field]=value;save["equipment_items_json"][0]=json.dumps(item)
            with self.subTest(field=field),self.assertRaises(m.v.EvidenceError):m.inspect_store(store,states)

    def test_complex_carrier_is_explicitly_unsupported_not_zero_filled(self):
        store,states=fixture();save=store["accounts"]["private-account-key-canary"]["saves"]["101"]
        item=json.loads(save["equipment_items_json"][0]);item["socket_slots"]=1;save["equipment_items_json"][0]=json.dumps(item)
        with self.assertRaisesRegex(m.v.EvidenceError,"unsupported_complex_persisted_item_carrier"):m.inspect_store(store,states)

    def test_ambiguous_character_or_duplicate_uid_fails(self):
        for mode in ["character","item"]:
            store,states=fixture();account=store["accounts"]["private-account-key-canary"]
            if mode=="character":store["accounts"]["second-private-key"]=copy.deepcopy(account)
            else:account["saves"]["101"]["equipment_items_json"]*=2
            with self.subTest(mode=mode),self.assertRaises(m.v.EvidenceError):m.inspect_store(store,states)

    def test_public_file_receipt_is_case_and_reader_source_bound(self):
        store,states=fixture();samples=m.inspect_store(store,states)
        metadata={key:{"sha256":str(i)*64} for i,key in enumerate(["plan","summary","events","controller"],1)}
        disk={"schema":"mir2.comparable.modern-file-disk-readback.v1","readAfterNormalStop":True,"accountStorePathBoundToRuntime":True,
              "privateValuesExported":False,"accountFileSha256":"a"*64,"accountFileBytes":123,"sampleCount":1,"samples":samples,
              **{label:metadata[key]["sha256"] for label,key in [("planSha256","plan"),("summarySha256","summary"),("eventsSha256","events"),("controllerSha256","controller")]},
              "readerSourceSha256":m.v.digest(Path(m.__file__).read_bytes()),"validatorSourceSha256":m.v.digest(Path(m.v.__file__).read_bytes())}
        events=[{"event":"savedVerificationState","actorOrdinal":0,"after":states[0]}]
        self.assertTrue(m.v.verify_modern_disk({"population":1},events,disk,metadata)["valid"])
        for mode in ["flag","case","source","sample"]:
            changed=copy.deepcopy(disk);changed["passed"]=True
            if mode=="flag":changed["samples"][0]["completeItemsAndGridMatch"]=False
            elif mode=="case":changed["eventsSha256"]="f"*64
            elif mode=="source":changed["readerSourceSha256"]="f"*64
            else:changed["samples"][0]["position"]["x"]+=1
            with self.subTest(mode=mode),self.assertRaises(m.v.EvidenceError):m.v.verify_modern_disk({"population":1},events,changed,metadata)


def all_actor_logout_fixture(population=3):
    """Synthetic all-actor raw projections and in-memory File DTOs, no game."""
    events=[];cursor={};store={"accounts":{}};initial={};final={};snapshots={};selected={0,population-1}
    plan={"scenario":"dispersed-melee","population":population,"settleBeforeLogoutMs":11000}
    report={"measurementEpochMs":1000,"observationEndedAtMs":61000,"synthetic":True,"cleanup":[{"actorOrdinal":a,"arbitrarySummaryState":"IGNORED-SUMMARY-CANARY"} for a in range(population)]}
    def raw(actor,connection,name,body,at):
        key=actor,connection;cursor[key]=cursor.get(key,0)+1
        events.append({"event":"received","actorOrdinal":actor,"connection":connection,"receiveSeq":cursor[key],"atMs":at,"packet":name,"payload":body,"synthetic":True})
        return {"observerOrdinal":actor,"connection":connection,"receiveSeq":cursor[key],"atMs":at,"packet":name}
    def sent(actor,connection,kind,at):events.append({"event":"sent","actorOrdinal":actor,"connection":connection,"atMs":at,"command":{"type":kind},"synthetic":True})
    def session(actor,connection,at,state):
        sent(actor,connection,"login",at);raw(actor,connection,"LoginSuccess",{},at+1)
        sent(actor,connection,"startGame",at+2);raw(actor,connection,"StartGame",{"result":4},at+3)
        raw(actor,connection,"UserInformation",{**{k:state[k] for k in ["name","level","class","gender","hp","mp"]},"objectId":1000,"location":{"x":state["x"],"y":state["y"]}},at+4)
        snapshot=raw(actor,connection,"worldSnapshot",{"playerObjectId":1000,"map":"DogYoHyun","playerHp":state["hp"],"playerMp":state["mp"],
             "owner":{k:state[k] for k in ["name","level","class","gender","x","y","direction"]},"carriedFingerprint":m.v.carried(state["carried"],"modern")["fingerprint"]},at+5)
        events.append({"event":"ownedSnapshotEvidence","phase":"authenticatedBootstrap","actorOrdinal":actor,"connection":connection,"sourceReceipt":snapshot,"state":copy.deepcopy(state["carried"]),"synthetic":True})
        events.append({"event":"admitted","actorOrdinal":actor,"connection":connection,"atMs":at+6,"synthetic":True});snapshots[actor,connection]=snapshot
    def exit(actor,connection,at,state,patches,freeze):
        sent(actor,connection,"logOut",at);ack=raw(actor,connection,"LogOutSuccess",{},at+1)
        if freeze:
            value={k:v for k,v in state.items() if k!="carried"};value["items"]=m.v.carried(state["carried"],"modern")["fingerprint"]
            events.append({"event":"logoutOwnedState","actorOrdinal":actor,"connection":connection,"state":value,"carried":copy.deepcopy(state["carried"]),
                "source":{"snapshot":snapshots[actor,connection],"patches":patches,"logOutSuccess":ack},"synthetic":True})
        events.append({"event":"clientCloseStarted","actorOrdinal":actor,"connection":connection,"atMs":at+2,"synthetic":True})
        events.append({"event":"transportClosed","actorOrdinal":actor,"connection":connection,"atMs":at+3,"validCloseFrameReceived":True,"code":1000,"synthetic":True})
    for actor in range(population):
        private,public=fixture();account=copy.deepcopy(next(iter(private["accounts"].values())));state=copy.deepcopy(public[0]);name=f"SyntheticFixture{actor}"
        char=account["characters"][0];char.update(name=name,index=101+actor);save=account["saves"].pop("101");save["character"]=copy.deepcopy(char);account["saves"][str(101+actor)]=save
        weapon=json.loads(save["equipment_items_json"][0]);weapon["user_item_unique_id"]+=actor;save["equipment_items_json"][0]=json.dumps(weapon)
        bag=copy.deepcopy(weapon);bag.pop("user_item_unique_id");bag.update(unique_id=22345+actor,key="crystal-item-662",container="bag1",slot=0,quantity=20,durability_current=None,durability_max=None)
        bag["user_item_metadata"]["item_index"]=662;save["inventory_items_json"]=[json.dumps(bag)]
        state["name"]=name;state["x"]+=actor;state["carried"]["equipmentItems"][0]["uniqueId"]+=actor;state["carried"]["equipmentItems"][0]["tooltipSource"]["userItem"]["unique_id"]+=actor
        potion=copy.deepcopy(state["carried"]["equipmentItems"][0]);potion.update(uniqueId=22345+actor,key="crystal-item-662",container="bag1",slot=0,quantity=20);potion.pop("durabilityCurrent");potion.pop("durabilityMax")
        potion["tooltipSource"]["userItem"].update(unique_id=22345+actor,item_index=662,count=20,current_dura=0,max_dura=0);state["carried"]["inventoryItems"]=[potion]
        save["position"]={"x":state["x"],"y":state["y"]};store["accounts"][f"private-key-{actor}"]=account;initial[actor]=state
        session(actor,1,100+actor*10,state)
    for actor,state in initial.items():
        at=80000+actor*10;value=copy.deepcopy(state);value.update(hp=660-actor,mp=150)
        value["carried"]["equipmentItems"][0]["durabilityCurrent"]=3990;value["carried"]["equipmentItems"][0]["tooltipSource"]["userItem"]["current_dura"]=3990
        value["carried"]["inventoryItems"][0]["quantity"]=19;value["carried"]["inventoryItems"][0]["tooltipSource"]["userItem"]["count"]=19
        patches=[raw(actor,1,"HealthChanged",{"hp":value["hp"],"mp":150},at-10),raw(actor,1,"DuraChanged",{"uniqueId":12345+actor,"currentDura":3990},at-9),raw(actor,1,"DeleteItem",{"uniqueId":22345+actor,"count":1},at-8)]
        exit(actor,1,at,value,patches,True);final[actor]=value
    for actor in sorted(selected):
        at=90000+actor*10;prior=copy.deepcopy(final[actor]);session(actor,2,at,prior)
        events.append({"event":"savedVerificationState","actorOrdinal":actor,"connection":2,"before":copy.deepcopy(prior),"after":copy.deepcopy(prior),"synthetic":True})
        final[actor]["hp"]+=3;final[actor]["carried"]["equipmentItems"][0]["durabilityCurrent"]-=1;final[actor]["carried"]["equipmentItems"][0]["tooltipSource"]["userItem"]["current_dura"]-=1
        patches=[raw(actor,2,"HealthChanged",{"hp":final[actor]["hp"],"mp":150},at+10),raw(actor,2,"DuraChanged",{"uniqueId":12345+actor,"currentDura":3989},at+11)]
        exit(actor,2,at+20,final[actor],patches,False)
    for actor,state in final.items():
        save=next(iter(store["accounts"][f"private-key-{actor}"]["saves"].values()));save.update(hp=state["hp"],mp=state["mp"])
        weapon=json.loads(save["equipment_items_json"][0]);weapon["durability_current"]=state["carried"]["equipmentItems"][0]["durabilityCurrent"];save["equipment_items_json"][0]=json.dumps(weapon)
        potion=json.loads(save["inventory_items_json"][0]);potion["quantity"]=19;save["inventory_items_json"][0]=json.dumps(potion)
    return plan,report,events,store


class AllActorLastAckTests(unittest.TestCase):
    def data(self,population=3):
        p,r,e,store=all_actor_logout_fixture(population);states,anchors=m.v.modern_disk_expected_states(p,r,e)
        return p,r,e,store,states,anchors

    def test_all25_from_raw_last_ack_with_only_two_fresh_states(self):
        p,r,e,store,states,anchors=self.data(25)
        self.assertEqual(len([x for x in e if x.get("event")=="savedVerificationState"]),2)
        self.assertEqual(set(states),set(range(25)));self.assertEqual(len(anchors),25)
        self.assertEqual(states[12]["hp"],648);self.assertEqual(states[0]["hp"],663)
        self.assertEqual(states[0]["carried"]["equipmentItems"][0]["durabilityCurrent"],3989)
        self.assertEqual([a["actorOrdinal"] for a in anchors if not a["producerFrozenStateCrossChecked"]],[0,24])
        samples=m.inspect_store(store,states);self.assertEqual(len(samples),25);self.assertEqual(samples[0]["hp"],663)

    def test_final_fresh_regen_is_exact_raw_operation_not_initial_UI_or_tolerance(self):
        p,r,e,store,states,anchors=self.data()
        fresh=next(x for x in e if x.get("event")=="savedVerificationState" and x["actorOrdinal"]==0)
        self.assertEqual(fresh["after"]["hp"],660);self.assertEqual(states[0]["hp"],663)
        save=next(iter(store["accounts"]["private-key-0"]["saves"].values()));save["hp"]=660
        with self.assertRaisesRegex(m.v.EvidenceError,"persisted_final_transform_or_vitals_differs"):m.inspect_store(store,states)

    def test_unrecorded_fresh_hp_change_cannot_be_filled_from_private_save(self):
        p,r,e,store=all_actor_logout_fixture()
        e=[x for x in e if not(x.get("event")=="received" and x.get("actorOrdinal")==0 and x.get("connection")==2 and x.get("packet")=="HealthChanged")]
        states,anchors=m.v.modern_disk_expected_states(p,r,e)
        self.assertEqual(states[0]["hp"],660)
        with self.assertRaisesRegex(m.v.EvidenceError,"persisted_final_transform_or_vitals_differs"):m.inspect_store(store,states)

    def test_nonfresh_actor_private_hp_mp_durability_and_stock_are_all_checked(self):
        for mode in ["hp","mp","dura","stock"]:
            p,r,e,store,states,anchors=self.data();save=next(iter(store["accounts"]["private-key-1"]["saves"].values()))
            if mode in ["hp","mp"]:save[mode]+=1
            else:
                field="equipment_items_json" if mode=="dura" else "inventory_items_json";item=json.loads(save[field][0]);item["durability_current" if mode=="dura" else "quantity"]+=1;save[field][0]=json.dumps(item)
            with self.subTest(mode=mode),self.assertRaises(m.v.EvidenceError):m.inspect_store(store,states)

    def test_missing_nonfresh_owned_checkpoint_and_false_source_reference_fail(self):
        for mode in ["missing","wrongref"]:
            p,r,e,store=all_actor_logout_fixture()
            row=next(x for x in e if x.get("event")=="ownedSnapshotEvidence" and x.get("actorOrdinal")==1)
            if mode=="missing":e.remove(row)
            else:row["sourceReceipt"]["connection"]=999
            with self.subTest(mode=mode),self.assertRaises(m.v.EvidenceError):m.v.modern_disk_expected_states(p,r,e)

    def test_missing_last_fresh_ACK_or_real_close_does_not_fall_back_to_first(self):
        for mode in ["ack","close"]:
            p,r,e,store=all_actor_logout_fixture()
            e=[x for x in e if not(x.get("actorOrdinal")==0 and x.get("connection")==2 and
               (x.get("packet")=="LogOutSuccess" if mode=="ack" else x.get("event")=="transportClosed"))]
            with self.subTest(mode=mode),self.assertRaises(m.v.EvidenceError):m.v.modern_disk_expected_states(p,r,e)

    def test_summary_values_are_ignored_but_wrong_producer_state_rejected(self):
        p,r,e,store,states,anchors=self.data();self.assertEqual(states[1]["hp"],659)
        row=next(x for x in e if x.get("event")=="logoutOwnedState" and x.get("actorOrdinal")==1);row["state"]["hp"]=99999
        with self.assertRaises(m.v.EvidenceError):m.v.modern_disk_expected_states(p,r,e)

    def test_unknown_refresh_requires_full_evidence_not_a_guessed_item(self):
        p,r,e,store=all_actor_logout_fixture();row=next(x for x in e if x.get("event")=="received" and x.get("actorOrdinal")==0 and x.get("connection")==2 and x.get("packet")=="DuraChanged")
        row.update(packet="RefreshItem",payload={})
        with self.assertRaisesRegex(m.v.EvidenceError,"mutation_without_complete_checkpoint"):m.v.modern_disk_expected_states(p,r,e)

    def receipt(self):
        p,r,e,store,states,anchors=self.data();samples=m.inspect_store(store,states)
        metadata={key:{"sha256":str(i)*64} for i,key in enumerate(["plan","summary","events","controller"],1)}
        disk={"schema":"mir2.comparable.modern-file-disk-readback.v2","passed":True,"readAfterNormalStop":True,"accountStorePathBoundToRuntime":True,
              "privateValuesExported":False,"accountFileSha256":"a"*64,"accountFileBytes":123,"sampleCount":3,"samples":samples,
              "expectedStateBasis":"all-actor-last-normal-LogOutSuccess-raw-reconstruction","allActorFinalStateAnchors":anchors,
              **{label:metadata[key]["sha256"] for label,key in [("planSha256","plan"),("summarySha256","summary"),("eventsSha256","events"),("controllerSha256","controller")]},
              "readerSourceSha256":m.v.digest(Path(m.__file__).read_bytes()),"validatorSourceSha256":m.v.digest(Path(m.v.__file__).read_bytes())}
        return p,r,e,disk,metadata

    def test_v2_public_receipt_recomputed_anchors_numeric_pools_and_UserItems(self):
        p,r,e,disk,metadata=self.receipt();result=m.v.verify_modern_disk(p,e,disk,metadata,report=r)
        self.assertTrue(result["valid"]);self.assertEqual(result["actors"],3)
        for field in ["hp","mp","direction","persistedItemSemanticsSha256"]:
            changed=copy.deepcopy(disk);changed["samples"][1][field]="wrong" if field in ["direction","persistedItemSemanticsSha256"] else changed["samples"][1][field]+1
            with self.subTest(field=field),self.assertRaises(m.v.EvidenceError):m.v.verify_modern_disk(p,e,changed,metadata,report=r)
        changed=copy.deepcopy(disk);changed["allActorFinalStateAnchors"][1]["logOutSuccess"]["receiveSeq"]+=1
        with self.assertRaisesRegex(m.v.EvidenceError,"anchors_unbound"):m.v.verify_modern_disk(p,e,changed,metadata,report=r)
        for field,value in [("hp",float(disk["samples"][1]["hp"])),("actorOrdinal",True)]:
            changed=copy.deepcopy(disk);changed["samples"][1][field]=value
            with self.subTest(typed_field=field),self.assertRaises(m.v.EvidenceError):m.v.verify_modern_disk(p,e,changed,metadata,report=r)

    def test_incomplete_actors_or_legacy_receipt_cannot_qualify_melee(self):
        p,r,e,disk,metadata=self.receipt()
        for mode in ["sample","anchor","schema"]:
            changed=copy.deepcopy(disk)
            if mode=="sample":changed["samples"].pop(1)
            elif mode=="anchor":changed["allActorFinalStateAnchors"].pop(1)
            else:changed["schema"]="mir2.comparable.modern-file-disk-readback.v1"
            with self.subTest(mode=mode),self.assertRaises(m.v.EvidenceError):m.v.verify_modern_disk(p,e,changed,metadata,report=r)

    def test_all_actor_outputs_export_no_names_credentials_ledger_UIDs_or_payload(self):
        p,r,e,disk,metadata=self.receipt();text=json.dumps(disk)
        for value in ["PASSWORD-CANARY","AUTHHASH-CANARY","LEDGER-CANARY","private-key-","SyntheticFixture","IGNORED-SUMMARY-CANARY","tooltipSource","userItem"]:self.assertNotIn(value,text)
        self.assertTrue(all("payload" not in json.dumps(anchor) for anchor in disk["allActorFinalStateAnchors"]))



def dense_logout_fixture(population=3):
    """Dense has no melee freeze event: only actual raw state operations count."""
    plan,report,events,store=all_actor_logout_fixture(population)
    plan["scenario"]="dense-movement-chat"
    events=[row for row in events if row.get("event")!="logoutOwnedState"]
    actor=1;ack=next(row for row in events if row.get("event")=="received" and row.get("packet")=="LogOutSuccess" and row["actorOrdinal"]==actor)
    seq=ack["receiveSeq"];ack["receiveSeq"]+=1
    position={"x":57,"y":52};move={"event":"received","actorOrdinal":actor,"connection":1,"receiveSeq":seq,
        "atMs":ack["atMs"]-.5,"packet":"UserLocation","payload":{**position,"direction":"Right"},"synthetic":True}
    events.insert(events.index(ack),move)
    save=next(iter(store["accounts"]["private-key-1"]["saves"].values()));save.update(position=position,direction="Right")
    return plan,report,events,store


class AllActorDenseLastAckTests(unittest.TestCase):
    def data(self,population=3):
        p,r,e,store=dense_logout_fixture(population)
        states,anchors=m.v.modern_disk_expected_states(p,r,e,all_actor_dense=True)
        return p,r,e,store,states,anchors

    def receipt(self):
        p,r,e,store=dense_logout_fixture()
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory)/"synthetic-events.jsonl";path.write_bytes(b"".join(m.v.encoded(row)+b"\n" for row in e))
            selected,source=m.v.read_dense_disk_state_events(path)
            scoring,core=m.v.read_events(path,"modern",p["population"])
        m.v.validate_dense_state_binding(selected,core)
        states,anchors=m.v.modern_disk_expected_states(p,r,selected,all_actor_dense=True)
        metadata={key:{"sha256":str(i)*64} for i,key in enumerate(["plan","summary","events","controller"],1)};metadata["events"]=core
        disk={"schema":"mir2.comparable.modern-file-disk-readback.v3","passed":True,"readAfterNormalStop":True,"accountStorePathBoundToRuntime":True,
              "privateValuesExported":False,"accountFileSha256":"a"*64,"accountFileBytes":123,"sampleCount":3,"samples":m.inspect_store(store,states),
              "expectedStateBasis":"all-actor-last-normal-LogOutSuccess-raw-reconstruction","allActorFinalStateAnchors":anchors,
              **{label:metadata[key]["sha256"] for label,key in [("planSha256","plan"),("summarySha256","summary"),("eventsSha256","events"),("controllerSha256","controller")]},
              "denseStateSourceSha256":source["sha256"],"denseStateSourceBytes":source["bytes"],"denseStateSourceRows":source["rows"],
              "readerSourceSha256":m.v.digest(Path(m.__file__).read_bytes()),"validatorSourceSha256":m.v.digest(Path(m.v.__file__).read_bytes())}
        return p,r,scoring,selected,disk,metadata

    def test_dense25_all_final_states_without_melee_freeze_or_summary_fallback(self):
        p,r,e,store,states,anchors=self.data(25)
        self.assertEqual(len([x for x in e if x.get("event")=="savedVerificationState"]),2)
        self.assertFalse(any(x.get("event")=="logoutOwnedState" for x in e))
        self.assertEqual(set(states),set(range(25)));self.assertEqual(len(anchors),25)
        self.assertTrue(all(not x["producerFrozenStateCrossChecked"] for x in anchors))
        self.assertEqual((states[1]["x"],states[1]["y"],states[1]["direction"]),(57,52,"Right"))
        self.assertEqual((states[12]["hp"],states[12]["mp"]),(648,150))
        self.assertEqual(states[1]["carried"]["inventoryItems"][0]["quantity"],19)
        self.assertEqual(states[1]["carried"]["equipmentItems"][0]["durabilityCurrent"],3990)
        self.assertEqual(len(m.inspect_store(store,states)),25)
        with self.assertRaisesRegex(m.v.EvidenceError,"all_actor_fresh_full_states_required"):
            m.v.modern_disk_expected_states(p,r,e)

    def test_dense_additional_pass_retains_unreferenced_operations_without_changing_scorer(self):
        p,r,scoring,state_events,disk,metadata=self.receipt()
        for packet in ["HealthChanged","DuraChanged","DeleteItem"]:
            self.assertTrue(any(row.get("packet")==packet for row in state_events))
            self.assertFalse(any(row.get("packet")==packet for row in scoring))
        self.assertFalse(state_events.audit["scoringStreamChanged"])
        self.assertEqual(state_events.audit["fullRows"],metadata["events"]["rows"])
        self.assertGreater(metadata["events"]["rows"],0)
        self.assertTrue(m.v.verify_modern_disk(p,scoring,disk,metadata,report=r,dense_state_events=state_events)["valid"])

    def test_v3_requires_complete_source_pass_of_the_exact_scored_file(self):
        p,r,e,state_events,disk,metadata=self.receipt()
        with self.assertRaisesRegex(m.v.EvidenceError,"state_pass_missing"):
            m.v.verify_modern_disk(p,e,disk,metadata,report=r)
        for key in ["sha256","bytes","rows"]:
            changed=copy.deepcopy(state_events);changed.full_source_metadata[key]="f"*64 if key=="sha256" else changed.full_source_metadata[key]+1
            with self.subTest(source=key),self.assertRaisesRegex(m.v.EvidenceError,"state_pass_not_same_events_file"):
                m.v.verify_modern_disk(p,e,disk,metadata,report=r,dense_state_events=changed)
        for key in ["denseStateSourceSha256","denseStateSourceBytes","denseStateSourceRows"]:
            bad=copy.deepcopy(disk);bad[key]="f"*64 if key.endswith("Sha256") else bad[key]+1
            with self.subTest(receipt=key),self.assertRaisesRegex(m.v.EvidenceError,"state_file_binding_missing"):
                m.v.verify_modern_disk(p,e,bad,metadata,report=r,dense_state_events=state_events)

    def test_v3_rechecks_actor_set_last_ack_anchor_and_numeric_item_semantics(self):
        p,r,e,state_events,disk,metadata=self.receipt()
        for mode in ["actor","anchor","hp","mp","position","direction","itemHash","count","source"]:
            bad=copy.deepcopy(disk)
            if mode=="actor":bad["samples"].pop(1)
            elif mode=="anchor":bad["allActorFinalStateAnchors"][1]["logOutSuccess"]["receiveSeq"]+=1
            elif mode in ["hp","mp"]:bad["samples"][1][mode]+=1
            elif mode=="position":bad["samples"][1]["position"]["x"]+=1
            elif mode=="direction":bad["samples"][1]["direction"]="Left"
            elif mode=="itemHash":bad["samples"][1]["persistedItemSemanticsSha256"]="f"*64
            elif mode=="count":bad["samples"][1]["itemCounts"]["inventoryItems"]+=1
            else:bad["readerSourceSha256"]="f"*64
            with self.subTest(mode=mode),self.assertRaises(m.v.EvidenceError):
                m.v.verify_modern_disk(p,e,bad,metadata,report=r,dense_state_events=state_events)

    def test_dense_cannot_use_private_save_to_fill_missing_or_false_raw_checkpoint(self):
        for mode in ["missing","source","ack","close"]:
            p,r,e,store=dense_logout_fixture()
            if mode in ["missing","source"]:
                row=next(x for x in e if x.get("event")=="ownedSnapshotEvidence" and x["actorOrdinal"]==1)
                if mode=="missing":e.remove(row)
                else:row["sourceReceipt"]["receiveSeq"]+=1
            else:
                e=[x for x in e if not(x.get("actorOrdinal")==1 and (x.get("packet")=="LogOutSuccess" if mode=="ack" else x.get("event")=="transportClosed"))]
            with self.subTest(mode=mode),self.assertRaises(m.v.EvidenceError):m.v.modern_disk_expected_states(p,r,e,all_actor_dense=True)

    def test_dense_stale_summary_or_fresh2_does_not_replace_nonfresh_last_raw_values(self):
        p,r,e,store,states,anchors=self.data()
        r["cleanup"][1].update(hp=99999,mp=99999,x=99999,y=99999)
        again,_=m.v.modern_disk_expected_states(p,r,e,all_actor_dense=True)
        self.assertEqual(states,again)
        for field in ["hp","mp","direction","position"]:
            private=copy.deepcopy(store);save=next(iter(private["accounts"]["private-key-1"]["saves"].values()))
            save[field]="Left" if field=="direction" else {"x":55,"y":51} if field=="position" else save[field]+1
            with self.subTest(field=field),self.assertRaises(m.v.EvidenceError):m.inspect_store(private,states)

    def test_dense_mutation_without_checkpoint_fails_instead_of_guessing(self):
        p,r,e,store=dense_logout_fixture()
        raw=next(x for x in e if x.get("packet")=="DuraChanged" and x.get("actorOrdinal")==1)
        raw.update(packet="RefreshItem",payload={})
        with self.assertRaisesRegex(m.v.EvidenceError,"mutation_without_complete_checkpoint"):
            m.v.modern_disk_expected_states(p,r,e,all_actor_dense=True)

    def test_dense_unsupported_gold_credit_and_item_operations_are_not_silent_noops(self):
        for packet in ["GainedGold","LoseGold","GainedCredit","LoseCredit","AddItem","BagSizeChanged","Death"]:
            p,r,e,store=dense_logout_fixture()
            row=next(x for x in e if x.get("packet")=="DuraChanged" and x.get("actorOrdinal")==1)
            row.update(packet=packet,payload={})
            with self.subTest(packet=packet),self.assertRaisesRegex(m.v.EvidenceError,"unsupported_state_operation_without_checkpoint"):
                m.v.modern_disk_expected_states(p,r,e,all_actor_dense=True)

    def test_dense_state_pass_bounds_and_truncation_are_explicit_not_partial_success(self):
        from unittest.mock import patch
        p,r,e,store=dense_logout_fixture()
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory)/"synthetic-events.jsonl";blob=b"".join(m.v.encoded(row)+b"\n" for row in e);path.write_bytes(blob)
            with patch.object(m.v,"MAX_SELECTED_ROWS",1),self.assertRaisesRegex(m.v.EvidenceError,"dense_state_evidence_bound"):
                m.v.read_dense_disk_state_events(path)
            path.write_bytes(blob[:-1])
            with self.assertRaisesRegex(m.v.EvidenceError,"truncated_jsonl_last_line"):m.v.read_dense_disk_state_events(path)

    def test_dense_v3_receipt_excludes_private_identity_items_and_summary_canaries(self):
        p,r,e,state_events,disk,metadata=self.receipt();text=json.dumps(disk)
        for value in ["PASSWORD-CANARY","AUTHHASH-CANARY","LEDGER-CANARY","private-key-","SyntheticFixture","IGNORED-SUMMARY-CANARY","tooltipSource","userItem"]:
            self.assertNotIn(value,text)


class ReviewedLegacyAndSourceSchemaTests(unittest.TestCase):
    def test_only_exact_reviewed_v6_reader_validator_pair_remains_legacy_compatible(self):
        store,states=fixture();metadata={key:{"sha256":str(i)*64} for i,key in enumerate(["plan","summary","events","controller"],1)}
        disk={"schema":"mir2.comparable.modern-file-disk-readback.v1","readAfterNormalStop":True,"accountStorePathBoundToRuntime":True,
              "privateValuesExported":False,"accountFileSha256":"a"*64,"accountFileBytes":123,"sampleCount":1,"samples":m.inspect_store(store,states),
              **{label:metadata[key]["sha256"] for label,key in [("planSha256","plan"),("summarySha256","summary"),("eventsSha256","events"),("controllerSha256","controller")]},
              "readerSourceSha256":"d3eec8d840501c66abf7f37128554d1016c9aa3fd26cd65aeaa04f2e9b9d5a3c",
              "validatorSourceSha256":"762218c45e0c88beb319ce3765960cd25b69f177c8cb9ac58667d6eb87a0c73f"}
        events=[{"event":"savedVerificationState","actorOrdinal":0,"after":states[0]}]
        self.assertTrue(m.v.verify_modern_disk({"population":1},events,disk,metadata)["valid"])
        disk["validatorSourceSha256"]="f"*64
        with self.assertRaisesRegex(m.v.EvidenceError,"reader_source_not_current_reviewed_code"):
            m.v.verify_modern_disk({"population":1},events,disk,metadata)

    def test_actual_declared_adapter_source_schema_stays_exact(self):
        from test_combat_verification import SourceAndStreamTests
        with tempfile.TemporaryDirectory() as directory:
            args,report,controller,meta,artifacts=SourceAndStreamTests().source_fixture(directory)
            report["adapterSource"]["schema"]=artifacts["manifest"]["schema"]
            self.assertTrue(m.v.verify_combat_sources(report,controller,meta,artifacts,"modern")["valid"])
            for field,value in [("schema","unreviewed.schema"),("unknownExtra",True)]:
                bad=copy.deepcopy(report);bad["adapterSource"][field]=value
                with self.subTest(field=field),self.assertRaisesRegex(m.v.EvidenceError,"source_manifest_not_bound"):
                    m.v.verify_combat_sources(bad,controller,meta,artifacts,"modern")


    def test_actual_f42_optional_timing_field_and_public_source_chain(self):
        import types
        base=Path(tempfile.gettempdir())/"mir2-comparable-capacity-20261113"
        fixture=base/"modern-audit/fixture-scene-functional2-v1";case=fixture/"runs/calibration-2-v1"
        metadata={};plan,metadata["plan"]=m.v.read_file(fixture/"plan.json")
        report,_=m.v.read_file(case/"modern.report.json");controller,_=m.v.read_file(case/"controller-result.json")
        args=types.SimpleNamespace(adapter_sources=base/"modern-adapter-v5/adapter-sources.json",runtime_receipt=case/"runtime-receipt.json",
            build_manifest=base/"modern-release-spectator-scene-v1/build-manifest.json",native_build=None,server_ready=None)
        artifacts=m.v.read_combat_sources(args,"modern",metadata)
        self.assertEqual(metadata["buildManifest"]["sha256"],"f42d482f98d8520544731a86ecad42aff47639a7d1dfa7dc2fd989c8df01def1")
        self.assertNotIn("observerTimingOnly",artifacts["buildManifest"])
        self.assertIs(report["additionalSource"]["observerTimingOnly"],False)
        self.assertEqual(metadata["sourceInputManifest"]["sha256"],"0eb1e809d6278b922d4279cdb5f2c353c615ba1e95533c9fb10994597a17c1ee")
        self.assertTrue(m.v.verify_combat_sources(report,controller,metadata,artifacts,"modern",melee=False)["valid"])
        for key,value in [("observerTimingOnly",True),("observerTimingOnly",0),("schema","unreviewed.schema"),
                          ("changesKind","observer-timing-only"),("sourceInputManifestSha256","f"*64),("cleanBaseBuild",True)]:
            changed=copy.deepcopy(artifacts);changed["buildManifest"][key]=value
            with self.subTest(key=key,value=value),self.assertRaises(m.v.EvidenceError):
                m.v.verify_combat_sources(report,controller,metadata,changed,"modern",melee=False)
        with tempfile.TemporaryDirectory() as directory:
            args.build_manifest=Path(directory)/"synthetic-bad-build.json"
            bad=copy.deepcopy(artifacts["buildManifest"]);bad["sourceInputManifestSha256"]="f"*64;args.build_manifest.write_text(json.dumps(bad))
            with self.assertRaisesRegex(m.v.EvidenceError,"source_input_manifest_hash_mismatch"):m.v.read_combat_sources(args,"modern",{})

    def test_both_reviewed_schema_kind_pairs_derive_only_their_optional_boolean(self):
        from test_combat_verification import SourceAndStreamTests
        for schema,kind,timing,proof in [("mir2.comparable.observer-timing-build.v1","observer-timing-only",True,"base-plus-observer-timing-overlay"),
                                        ("mir2.comparable.spectator-scene-build.v1","spectator-scene-projection",False,"base-plus-spectator-scene-projection")]:
            with self.subTest(schema=schema),tempfile.TemporaryDirectory() as directory:
                args,report,controller,meta,artifacts=SourceAndStreamTests().source_fixture(directory)
                source_input=Path(directory)/"synthetic-source-inputs.json";source_input.write_text('{"synthetic":true}')
                fact=m.v.hash_source_artifact(source_input);meta["sourceInputManifest"]=fact;artifacts["verifiedBuildSourceInput"]=fact
                build={"schema":schema,"changesKind":kind,"baseRevision":"31e9b6a74661c31d6515dd7322b466285f975d71","binarySha256":"b"*64,
                    "sourceInputManifest":str(source_input),"sourceInputManifestSha256":fact["sha256"],"buildActualExitCode":0,"cleanBaseBuild":False}
                meta["buildManifest"]={"sha256":"e"*64};artifacts["buildManifest"]=build;controller["buildManifestSha256"]="e"*64
                artifacts["runtime"].update(sourceRevision=build["baseRevision"],buildManifestSha256="e"*64,changesKind=kind,sourceInputManifestSha256=fact["sha256"],buildActualExitCode=0)
                report.update(sourceRevision=build["baseRevision"],sourceRevisionIsBaseOnly=True,sourceProof=proof,additionalSource={
                    **{k:build[k] for k in ["changesKind","baseRevision","binarySha256","sourceInputManifestSha256","buildActualExitCode"]},
                    "observerTimingOnly":timing,"buildManifestSchema":schema,"buildManifestSha256":"e"*64,"cleanBaseBuild":False,"behaviorFixClaimed":False,"capacityClaimAllowed":False})
                self.assertTrue(m.v.verify_combat_sources(report,controller,meta,artifacts,"modern")["valid"])
                artifacts["buildManifest"]["observerTimingOnly"]=not timing
                with self.assertRaisesRegex(m.v.EvidenceError,"explicit_timing_flag_conflict"):m.v.verify_combat_sources(report,controller,meta,artifacts,"modern")


if __name__=="__main__":unittest.main()
