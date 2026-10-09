//! Pure host-contract tests. No browser, socket, renderer or WASM instance.
use super::*;
use mir2_client_bevy::{
    inventory::ItemModel,
    portable_storage_ui::{StoragePointerOrigin,StorageSelection},
    storage_interaction::StorageGrid,
};
fn identity(run:u64,owner:u64)->StorageIdentity {
    StorageIdentity{run_generation:run,connection_generation:1,session_generation:1,owner_revision:owner}
}
fn item(container:u8,slot:u32,id:Option<u64>)->ItemModel {
    ItemModel{container,slot,unique_id:id,key:"potion".into(),name:"Potion".into(),
        icon:116,quantity:1,description:String::new(),..Default::default()}
}
fn snapshot()->StorageSnapshot {
    StorageSnapshot{identity:identity(1,0),revision:1,model_revision:1,presentation_revision:1,service_revision:1,
        open:true,input_enabled:true,presentation:Some(StoragePresentation{
            logical_width:1024.,logical_height:768.,stage_css_scale:1.,touch:false}),
        inventory:InventoryModel{capacity:54,items:vec![item(0,43,Some(7))],..Default::default()},
        storage:StorageModel{items:vec![item(4,3,Some(8))],..StorageModel::new()},
        player:PlayerStats::default(),blocked_unique_ids:vec![],pending_cells:vec![],language:"en".into()}
}
fn value()->Value {serde_json::to_value(snapshot()).unwrap()}
fn parse(value:Value)->Result<StorageSnapshot,&'static str>{StorageSnapshot::parse(&value.to_string())}
fn edge(sequence:u64)->StoragePointerEdge {
    StoragePointerEdge{identity:identity(1,0),sequence,model_revision:1,presentation_revision:1,service_revision:1,
        pointer_id:1,down_sequence:sequence,phase:StoragePointerPhase::Down,origin:StoragePointerOrigin::Storage,x:10.,y:10.,button:0}
}
#[test]
fn storage_contract_accepts_snake_storage_camel_item_and_physical_bag2() {
    let parsed=parse(value()).unwrap();
    assert_eq!(parsed.inventory.items[0].slot,43);
    assert_eq!(parsed.storage.items[0].container,4);
    let mut missing=value();missing["storage"].as_object_mut().unwrap().remove("has_password");
    assert!(parse(missing).is_err());
    let mut display=value();display["inventory"]["items"][0].as_object_mut().unwrap().remove("uniqueId");
    assert!(parse(display).is_err(),"missing UID field is not authoritative empty");
    let mut display=value();display["inventory"]["items"][0]["uniqueId"]=Value::Null;
    assert_eq!(parse(display).unwrap().inventory.items[0].unique_id,None,
        "explicit display-only item may render but cannot become a source");
}
#[test]
fn storage_contract_rejects_duplicate_json_before_legacy_models_collapse_it() {
    let json=value().to_string();
    assert!(StorageSnapshot::parse(&json.replacen("\"revision\":1","\"revision\":1,\"revision\":2",1)).is_err());
    assert!(StorageSnapshot::parse(&json.replacen("\"slot\":43","\"slot\":43,\"slot\":44",1)).is_err());
    assert!(strict("{\"nested\":{\"a\":1,\"a\":2}}").is_err());
    assert!(strict("{\"items\":[{\"slot\":1,\"slot\":2}]}").is_err());
}
#[test]
fn storage_contract_rejects_duplicate_uid_cells_unsafe_numbers_and_capacity() {
    let mut invalid=value();invalid["storage"]["items"][0]["uniqueId"]=7.into();assert!(parse(invalid).is_err());
    let mut invalid=value();let row=invalid["inventory"]["items"][0].clone();
    invalid["inventory"]["items"].as_array_mut().unwrap().push(row);assert!(parse(invalid).is_err());
    for (field,number) in [("runGeneration",0),("revision",0),("modelRevision",SAFE+1),
        ("presentationRevision",0),("serviceRevision",SAFE+1)] {
        let mut invalid=value();invalid[field]=number.into();assert!(parse(invalid).is_err(),"{field}");
    }
    let mut invalid=value();invalid["inventory"]["items"][0]["uniqueId"]=(SAFE+1).into();assert!(parse(invalid).is_err());
    let mut invalid=value();invalid["inventory"]["capacity"]=46.into();assert!(parse(invalid).is_err(),"Bag2 capacity revoked");
    let mut invalid=value();invalid["inventory"]["capacity"]=55.into();assert!(parse(invalid).is_err());
    let mut invalid=value();invalid["storage"]["size"]=0.into();assert!(parse(invalid).is_err());
    let mut invalid=value();invalid["storage"]["size"]=3.into();assert!(parse(invalid).is_err(),"slot is beyond physical size");
    let mut invalid=value();invalid["storage"]["items"][0]["container"]=0.into();assert!(parse(invalid).is_err());
}
#[test]
fn storage_contract_preserves_large_and_negative_datetime_not_uid_rules() {
    for expiry in [638_900_000_000_000_000_i64,-8_584_900_000_000_000_000_i64] {
        let mut v=value();v["storage"]["expiry"]=expiry.into();
        assert_eq!(parse(v).unwrap().storage.expiry,expiry);
    }
    let mut v=value();v["storage"]["size"]=160.into();v["storage"]["items"][0]["slot"]=100.into();
    let parsed=parse(v).unwrap();assert!(!parsed.storage.is_valid_slot(100),"expired expanded backing item stays inaccessible");
}

#[test]
fn storage_contract_exact_i64_text_reaches_ingress_without_loosening_uid_bounds() {
    for expiry in [0_i64, 1000, 639_028_224_000_000_001, 639_028_224_000_000_002,
        -8_584_900_000_000_000_001, i64::MIN, i64::MAX] {
        let mut v=value();v["storage"]["expiry"]=expiry.to_string().into();
        let parsed=parse(v.clone()).unwrap();
        assert_eq!(parsed.storage.expiry,expiry);
        assert_eq!(parsed.storage.page(1).expiry,expiry);
        let mut ingress=StorageIngress::default();assert!(ingress.accept(parsed));
        assert_eq!(ingress.pending.as_ref().unwrap().storage.expiry,expiry);
        assert!(ingress.withdraw(identity(1,0)));assert!(ingress.pending.is_none());
        v["inventory"]["items"][0]["uniqueId"]=(SAFE+1).into();
        assert!(parse(v).is_err(),"an exact date never authorizes an unsafe UID");
    }
}
#[test]
fn storage_contract_rejects_malformed_or_out_of_range_expiry_text() {
    for expiry in ["+1","-0","01","-01"," 1","1 ","","1e3","1.0",
        "9223372036854775808","-9223372036854775809"] {
        let mut v=value();v["storage"]["expiry"]=expiry.into();assert!(parse(v).is_err(),"{expiry}");
    }
    for expiry in [Value::Null,serde_json::json!(true),serde_json::json!({}),serde_json::json!([])] {
        let mut v=value();v["storage"]["expiry"]=expiry;assert!(parse(v).is_err());
    }
}

#[test]
fn storage_contract_rejects_oversized_arrays_before_truncation_and_bad_locks() {
    let mut invalid=value();
    invalid["storage"]["items"]=Value::Array((0..161).map(|slot|serde_json::to_value(item(4,slot,Some(100+u64::from(slot)))).unwrap()).collect());
    assert_eq!(parse(invalid).unwrap_err(),"storage item array too large");
    let mut invalid=value();invalid["pendingCells"]=serde_json::json!([{"container":4,"slot":3},{"container":4,"slot":3}]);
    assert!(parse(invalid).is_err());
    let mut invalid=value();invalid["blockedUniqueIds"]=serde_json::json!([7,7]);assert!(parse(invalid).is_err());
    let mut invalid=value();invalid["pendingCells"]=serde_json::json!([{"container":1,"slot":0}]);assert!(parse(invalid).is_err());
}
#[test]
fn storage_contract_presentation_language_and_closed_input_fail_closed() {
    let mut invalid=value();invalid["open"]=false.into();assert!(parse(invalid).is_err());
    let mut closed=value();closed["open"]=false.into();closed["inputEnabled"]=false.into();closed["presentation"]=Value::Null;
    assert!(!parse(closed).unwrap().open);
    let mut invalid=value();invalid["language"]="other".into();assert!(parse(invalid).is_err());
    let mut touch=value();touch["presentation"]=serde_json::json!({"logicalWidth":640,"logicalHeight":480,"stageCssScale":0.5,"touch":true});
    assert!(parse(touch.clone()).is_ok());
    touch["presentation"]["logicalWidth"]=500.into();assert!(parse(touch).is_err());
    let mut invalid=value();invalid["presentation"]["stageCssScale"]=0.into();assert!(parse(invalid).is_err());
    let mut invalid=value();invalid["unexpected"]=true.into();assert!(parse(invalid).is_err());
}
#[test]
fn storage_ingress_monotonic_owner_and_three_revision_clocks() {
    let mut ingress=StorageIngress::default();assert!(ingress.accept(snapshot()));
    let mut stale=snapshot();stale.revision=2;stale.model_revision=0;assert!(!ingress.accept(stale));
    let mut next=snapshot();next.revision=2;next.model_revision=2;assert!(ingress.accept(next.clone()));
    next.revision=3;next.model_revision=1;assert!(!ingress.accept(next));
    let mut next=snapshot();next.identity.owner_revision=1;next.revision=3;next.model_revision=2;assert!(ingress.accept(next));
    let mut old=snapshot();old.revision=4;old.model_revision=2;assert!(!ingress.accept(old));
    let mut newer=snapshot();newer.identity=identity(2,0);assert!(ingress.accept(newer));
}
#[test]
fn storage_withdraw_exact_identity_never_retires_new_run_and_keeps_watermark() {
    let mut ingress=StorageIngress::default();assert!(ingress.accept(snapshot()));
    assert!(!ingress.withdraw(identity(1,1)));assert!(ingress.pending.is_some());
    assert!(ingress.withdraw(identity(1,0)));assert!(ingress.pending.is_none());assert!(ingress.accept(snapshot())==false);
    let mut newer=snapshot();newer.identity=identity(2,0);assert!(ingress.accept(newer));
    assert!(!ingress.withdraw(identity(1,0)));assert!(ingress.pending.is_some());
}
#[test]
fn storage_pointer_old_model_service_owner_and_duplicate_edges_do_not_poison_sequence() {
    let mut ingress=StorageIngress::default();assert!(ingress.accept(snapshot()));ingress.pending.take();
    let mut old=edge(SAFE);old.model_revision=2;assert!(!ingress.pointer(old,true));
    old=edge(SAFE);old.service_revision=2;assert!(!ingress.pointer(old,true));
    old=edge(SAFE);old.identity.owner_revision=1;assert!(!ingress.pointer(old,true));
    assert_eq!(ingress.last_edge_sequence,0);
    assert!(!ingress.pointer(edge(1),false));assert_eq!(ingress.last_edge_sequence,0);
    assert!(ingress.pointer(edge(1),true));assert!(!ingress.pointer(edge(1),true));
    let mut cleanup=edge(2);cleanup.phase=StoragePointerPhase::Cancel;cleanup.down_sequence=1;assert!(ingress.pointer(cleanup,false));
    let mut changed=snapshot();changed.revision=2;changed.model_revision=2;assert!(ingress.accept(changed));ingress.pending.take();
    assert!(!ingress.pointer(edge(3),true));
    let mut now=edge(3);now.model_revision=2;assert!(ingress.pointer(now,true));
}
#[test]
fn storage_pointer_capacity_and_grammar_are_bounded() {
    let mut ingress=StorageIngress::default();assert!(ingress.accept(snapshot()));
    assert!(!ingress.pointer(edge(1),true),"setter acceptance is not an applied input tree");ingress.pending.take();
    for sequence in 1..=256 {let mut event=edge(sequence);if sequence>1{event.phase=StoragePointerPhase::Move;event.down_sequence=1;}
        assert!(ingress.pointer(event,true));}
    let mut full=edge(257);full.phase=StoragePointerPhase::Move;full.down_sequence=1;assert!(!ingress.pointer(full,true));assert_eq!(ingress.last_edge_sequence,256);
    let mut invalid=edge(258);invalid.x=f32::NAN;assert!(!invalid.valid());
    let mut v=serde_json::json!({"runGeneration":1,"connectionGeneration":1,"sessionGeneration":1,
        "ownerRevision":0,"sequence":1,"modelRevision":1,"presentationRevision":1,"serviceRevision":1,
        "pointerId":0,"downSequence":1,"phase":"down","origin":"storage","x":10,"y":10,"button":0});
    assert!(serde_json::from_value::<StoragePointerEdge>(v.clone()).unwrap().valid());
    v.as_object_mut().unwrap().remove("serviceRevision");assert!(serde_json::from_value::<StoragePointerEdge>(v).is_err());
}
#[test]
fn storage_intent_whole_contract_preserves_physical_cells_and_both_merge_grids() {
    let mut intent=StorageUiIntent{identity:identity(1,2),intent_sequence:7,model_revision:3,
        presentation_revision:4,service_revision:5,kind:StorageUiIntentKind::Transfer{kind:"takeBackItem",
            source:StorageSelection{container:4,slot:83,unique_id:9},target:StorageCell{container:0,slot:43}}};
    let actual:Value=serde_json::from_str(&encode_intent(intent)).unwrap();
    assert_eq!(actual,serde_json::json!({"runGeneration":1,"connectionGeneration":1,"sessionGeneration":1,"ownerRevision":2,
        "intentSequence":7,"modelRevision":3,"presentationRevision":4,"serviceRevision":5,"type":"takeBackItem",
        "source":{"container":4,"slot":83,"uniqueId":9},"target":{"container":0,"slot":43}}));
    intent.kind=StorageUiIntentKind::Merge{source:StorageSelection{container:0,slot:43,unique_id:7},
        target:StorageSelection{container:4,slot:3,unique_id:8},grid_from:StorageGrid::Inventory,grid_to:StorageGrid::Storage};
    let actual:Value=serde_json::from_str(&encode_intent(intent)).unwrap();
    assert_eq!(actual["gridFrom"],"inventory");assert_eq!(actual["gridTo"],"storage");
    assert_eq!(actual["source"]["uniqueId"],7);assert_eq!(actual["target"]["uniqueId"],8);
    intent.kind=StorageUiIntentKind::Transfer{kind:"moveItem",source:StorageSelection{container:0,slot:43,unique_id:7},
        target:StorageCell{container:0,slot:44}};
    let actual:Value=serde_json::from_str(&encode_intent(intent)).unwrap();
    assert_eq!(actual["grid"],"inventory");assert_eq!(actual["source"]["slot"],43);assert_eq!(actual["target"]["slot"],44);
    for kind in [StorageUiIntentKind::Close,StorageUiIntentKind::Password,StorageUiIntentKind::Rent] {
        intent.kind=kind;let actual:Value=serde_json::from_str(&encode_intent(intent)).unwrap();
        assert!(!actual.as_object().unwrap().contains_key("source"));
    }
}
#[test]
fn storage_status_disable_never_turns_setter_acceptance_into_ready() {
    let mut ingress=StorageIngress::default();assert!(ingress.accept(snapshot()));
    let mut status=StorageStatus::default();assert!(!status.ready);assert!(!status.input_enabled);
    status.ready=true;status.input_enabled=true;status.input_regions.push(InputRegion{left:1.,top:1.,width:40.,height:40.});
    status.disable();assert!(!status.ready);assert!(!status.input_enabled);assert!(status.input_regions.is_empty());
}

#[test]
fn storage_runtime_exact_down_nonce_and_world_origin_are_preserved() {
    let mut ingress=StorageIngress::default();assert!(ingress.accept(snapshot()));ingress.pending.take();
    let mut old=edge(1);old.origin=StoragePointerOrigin::World;assert!(ingress.pointer(old,true));
    let mut up=old;up.sequence=2;up.phase=StoragePointerPhase::Up;assert!(ingress.pointer(up,true));
    let current=edge(3);assert!(ingress.pointer(current,true));
    let mut stale=old;stale.phase=StoragePointerPhase::Cancel;stale.sequence=SAFE;
    assert!(!ingress.pointer(stale,true));assert_eq!(ingress.last_edge_sequence,3);
    assert_eq!(ingress.lease.unwrap().down_sequence,3);
    let mut mismatch=current;mismatch.sequence=4;mismatch.phase=StoragePointerPhase::Blur;
    mismatch.origin=StoragePointerOrigin::World;assert!(!ingress.pointer(mismatch,true));
    mismatch=current;mismatch.sequence=4;mismatch.phase=StoragePointerPhase::Cancel;
    assert!(ingress.pointer(mismatch,false));assert!(ingress.lease.is_none());
    assert_eq!(ingress.edges[0].origin,StoragePointerOrigin::World);
    let mut forged=edge(5);forged.down_sequence=4;assert!(!forged.valid());
}
