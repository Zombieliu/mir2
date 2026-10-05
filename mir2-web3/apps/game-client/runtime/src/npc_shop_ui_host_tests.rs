//! Strict host and synchronous callback seam tests. No JS eval, renderer or network.
use super::*;

#[test]
fn npc_shop_host_capability_is_independent_and_preserves_existing_exact_objects() {
    use crate::runtime_ui_capabilities::*;
    for (backend,wasm,host,shared,requested,compiled,startup) in [
        ("native",false,true,true,true,false,false),
        ("webgl2",true,true,false,true,false,false),
        ("webgl2",true,false,true,true,false,false),
        ("webgl2",true,true,true,false,true,false),
        ("webgl2",true,true,true,true,true,true),
        ("webgpu",true,true,false,true,true,true),
    ] {
        assert_eq!(describe_npc_shop(backend,wasm,host,shared,requested),serde_json::json!({
            "schemaVersion":1,"npcShopUiAbiVersion":u8::from(compiled),"npcShopIntentAbiVersion":u8::from(compiled),
            "compiled":compiled,"startup":startup}));
    }
    let old:serde_json::Value=serde_json::from_str(&current_json()).unwrap();
    assert_eq!(old.as_object().unwrap().len(),6);
    for existing in [current_hud_json(),current_character_json(),current_spells_json(),current_storage_json()] {
        let value:serde_json::Value=serde_json::from_str(&existing).unwrap();assert_eq!(value.as_object().unwrap().len(),5);
        assert!(!value.as_object().unwrap().contains_key("npcShopUiAbiVersion"));
    }
    #[cfg(not(target_arch="wasm32"))]
    {
        assert_eq!(old,serde_json::json!({"schemaVersion":1,"backend":"native","questUiAbiVersion":0,"bagUiAbiVersion":0,
            "primarySharedUiCompiled":false,"primarySharedUiStartup":false}));
        for (text,a,b) in [(current_hud_json(),"hudUiAbiVersion","characterStatsAbiVersion"),
            (current_character_json(),"characterPageAbiVersion","characterEquipmentIntentAbiVersion"),
            (current_spells_json(),"spellsPageAbiVersion","spellsIntentAbiVersion"),
            (current_storage_json(),"storageUiAbiVersion","storageIntentAbiVersion")] {
            let mut expected=serde_json::json!({"schemaVersion":1,"compiled":false,"startup":false});expected[a]=0.into();expected[b]=0.into();
            assert_eq!(serde_json::from_str::<serde_json::Value>(&text).unwrap(),expected);
        }
    }
    #[cfg(not(target_arch="wasm32"))]
    assert_eq!(serde_json::from_str::<serde_json::Value>(&current_npc_shop_json()).unwrap(),
        serde_json::json!({"schemaVersion":1,"npcShopUiAbiVersion":0,"npcShopIntentAbiVersion":0,"compiled":false,"startup":false}));
}

use mir2_client_bevy::{inventory::{CrystalItemInfoModel,CrystalItemTooltipSourceModel,CrystalUserItemModel,ItemModel},
    shop::{ShopGood,NpcShopServiceMode},npc_shop_ui::NpcShopUiAction,
    portable_npc_shop_ui::{NpcShopSurfaceIntentQueue,NpcShopSurfaceAction}};
fn good(id:u64)->ShopGood {
    ShopGood {unique_id:id,name:format!("Potion {id}"),price:1,count:1,stock:-1,panel_type:0,
        purchase_rate:Some(1.5),requires_gold_buy_plan:true,
        tooltip_source:Some(CrystalItemTooltipSourceModel {
            info:CrystalItemInfoModel {item_index:658,name:"Potion".into(),price:1,stack_size:99,item_type:13,..Default::default()},
            user_item:Some(CrystalUserItemModel {unique_id:id,item_index:658,count:1,is_shop_item:true,..Default::default()}),
            ..Default::default()}),..Default::default()}
}
fn identity(run:u64,owner:u64)->NpcShopSurfaceIdentity {
    NpcShopSurfaceIdentity {run_generation:run,connection_generation:1,session_generation:1,owner_revision:owner,player_object_id:7}
}
fn snapshot()->NpcShopSnapshot {
    NpcShopSnapshot {identity:identity(1,0),revision:1,model_revision:1,presentation_revision:1,service_revision:1,catalog_revision:1,
        core_authority_revision:Some(1),open:true,input_enabled:true,show_buy:true,
        presentation:Some(NpcShopSurfacePresentation {logical_width:1024.,logical_height:768.,stage_css_scale:1.,touch:false}),
        shop:ShopModel {goods:(0..10).map(good).collect(),selected_id:Some(0),service_mode:NpcShopServiceMode::Buy,
            supports_buy:true,supports_sell:true,..Default::default()},
        inventory:InventoryModel {gold:100,capacity:46,..Default::default()},player:PlayerStats::default(),language:"en".into(),
        feedback:NpcShopFeedback {can_reserve:true,..Default::default()}}
}
fn value()->Value {serde_json::to_value(snapshot()).unwrap()}
fn parse(value:Value)->Result<NpcShopSnapshot,&'static str> {NpcShopSnapshot::parse(&value.to_string())}
fn read_model(s:&NpcShopSnapshot)->NpcShopSurfaceReadModel {
    NpcShopSurfaceReadModel {shop:s.shop.clone(),inventory:s.inventory.clone(),player:s.player.clone()}
}
fn settle(i:&mut NpcShopIngress) {i.pending.take();i.cancel_pointer=false;i.withdrawn.take();}
fn edge(proof:NpcShopSurfaceProof,sequence:u64,down:u64,phase:NpcShopSurfacePointerPhase)->NpcShopSurfacePointerEdge {
    NpcShopSurfacePointerEdge {proof,gesture:NpcShopSurfaceGesture {pointer_id:1,down_sequence:down,sequence,
        origin:NpcShopSurfacePointerOrigin::Shop,button:0},phase,x:50.,y:50.}
}
fn flow()->(NpcShopIngress,NpcShopSurfaceContext,NpcShopSurfaceReadModel,NpcShopSurfaceState,NpcShopSurfaceIntent,NpcShopStatus) {
    let snap=snapshot();let mut ingress=NpcShopIngress::default();assert!(ingress.accept(snap.clone()));settle(&mut ingress);
    let mut c=snap.context();c.ready=true;let r=read_model(&snap);let mut state=NpcShopSurfaceState::default();state.reconcile(&c,&r);
    let proof=state.proof(&c,&r).unwrap();let down=edge(proof,1,1,NpcShopSurfacePointerPhase::Down);
    let up=edge(proof,2,1,NpcShopSurfacePointerPhase::Up);assert!(ingress.pointer(down,true));assert!(ingress.pointer(up,true));
    let mut q=NpcShopSurfaceIntentQueue::default();assert!(state.request_action(NpcShopUiAction::Select(0),up.gesture,&c,&r,&mut q));
    let mut intents=q.drain();assert_eq!(intents.len(),1);let intent=intents.remove(0);
    let status=NpcShopStatus::from_context(&c,&state,&r);(ingress,c,r,state,intent,status)
}

#[test]
fn npc_shop_host_strict_complete_models_delegate_original_raw_to_shared_planner() {
    let parsed=parse(value()).unwrap();assert_eq!(parsed.shop.goods[0].unique_id,0);assert_eq!(parsed.shop.goods[0].purchase_rate,Some(1.5));
    let raw=serde_json::json!({"shop":parsed.shop,"inventory":parsed.inventory,"quantity":2});
    let plan:NpcGoldBuyPlan=serde_json::from_str(&plan_npc_gold_buy_json(&raw.to_string())).unwrap();
    assert!(plan.can_buy);assert_eq!(plan.total_gold,Some(3));assert_eq!(plan.command.unwrap().item_index,0);
    for field in ["purchase_rate","requires_gold_buy_plan","tooltip_source","icon_width","icon_height"] {
        let mut incomplete=value();incomplete["shop"]["goods"][0].as_object_mut().unwrap().remove(field);
        assert!(parse(incomplete).is_err(),"missing raw good field {field}");
    }
    for field in ["gold","capacity","items"] {let mut incomplete=value();incomplete["inventory"].as_object_mut().unwrap().remove(field);
        assert!(parse(incomplete).is_err(),"missing inventory field {field}");}
    for (carrier,field) in [("info","price"),("info","stats"),("userItem","count"),("userItem","unique_id")] {
        let mut incomplete=value();incomplete["shop"]["goods"][0]["tooltip_source"][carrier].as_object_mut().unwrap().remove(field);
        assert!(parse(incomplete).is_err(),"partial nested {carrier}.{field} cannot be default-filled");
    }
    let row=serde_json::to_value(ItemModel {container:0,slot:0,unique_id:Some(9),key:"potion".into(),name:"Potion".into(),quantity:1,..Default::default()}).unwrap();
    for field in ["uniqueId","quantity","slot","container"] {
        let mut v=value();let mut partial=row.clone();partial.as_object_mut().unwrap().remove(field);v["inventory"]["items"]=serde_json::json!([partial]);
        assert!(parse(v).is_err(),"partial inventory {field} cannot be authority");
    }
    for field in ["hp","weights","crystalStats","name","guildName"] {let mut incomplete=value();incomplete["player"].as_object_mut().unwrap().remove(field);
        assert!(parse(incomplete).is_err(),"missing readonly player field {field}");}
}
#[test]
fn npc_shop_host_duplicate_unknown_nested_and_trailing_json_are_rejected() {
    let json=value().to_string();assert!(NpcShopSnapshot::parse(&json.replacen(r#""revision":1"#,r#""revision":1,"revision":2"#,1)).is_err());
    let mut unknown=value();unknown["unknown"]=true.into();assert!(parse(unknown).is_err());
    let mut unknown=value();unknown["presentation"]["unknown"]=true.into();assert!(parse(unknown).is_err());
    let mut unknown=value();unknown["feedback"]["unknown"]=true.into();assert!(parse(unknown).is_err());
    assert!(strict(r#"{"model":{"x":1,"x":2}}"#).is_err());assert!(strict(r#"{"a":[{"x":1,"x":2}]}"#).is_err());
    assert!(NpcShopSnapshot::parse(&(json+" null")).is_err());
    assert!(strict(&("[".repeat(40)+"0"+&"]".repeat(40))).is_err());
}
#[test]
fn npc_shop_host_complete_but_planner_blocked_models_do_not_manufacture_raw_source() {
    for cause in 0..3 {let mut v=value();match cause {0=>v["shop"]["goods"][0]["tooltip_source"]=Value::Null,
        1=>v["shop"]["goods"][0]["purchase_rate"]=Value::Null,_=>v["inventory"]["gold"]=0.into()}
        let parsed=parse(v).unwrap();let raw=serde_json::json!({"shop":parsed.shop,"inventory":parsed.inventory,"quantity":1});
        let plan:NpcGoldBuyPlan=serde_json::from_str(&plan_npc_gold_buy_json(&raw.to_string())).unwrap();
        assert!(!plan.can_buy);assert!(plan.command.is_none());assert_ne!(plan.block_reason,Some(NpcGoldBuyBlockReason::InvalidInput));}
    for kind in 0..3 {let mut v=value();match kind {0=>v["shop"]["goods"][9]["use_pearls"]=true.into(),
        1=>v["shop"]["goods"][9]["stock"]=0.into(),_=>v["shop"]["goods"][9]["panel_type"]=1.into()}
        assert!(parse(v).is_err(),"whole mixed catalog needs legacy, no partial shared admission");}
}
#[test]
fn npc_shop_host_decimal_authority_and_control_clocks_preserve_full_u64() {
    for clock in [9_007_199_254_740_992_u64,u64::MAX] {let mut v=value();v["coreAuthorityRevision"]=clock.to_string().into();
        let parsed=parse(v).unwrap();assert_eq!(parsed.core_authority_revision,Some(clock));
        let (mut i,mut c,r,mut state,_,_)=flow();c.core_authority_revision=Some(clock);state.reconcile(&c,&r);
        let mut proof=state.proof(&c,&r).unwrap();proof.control_revision=clock;
        let encoded=serde_json::to_value(proof).unwrap();assert_eq!(encoded["controlRevision"],clock.to_string());
        assert_eq!(serde_json::from_value::<NpcShopSurfaceProof>(encoded).unwrap(),proof);i.cancel();}
    for malformed in [serde_json::json!(9_007_199_254_740_992_u64),serde_json::json!("0"),serde_json::json!("01"),
        serde_json::json!("1.0"),serde_json::json!("18446744073709551616")] {
        let mut v=value();v["coreAuthorityRevision"]=malformed;assert!(parse(v).is_err());}
}
#[test]
fn npc_shop_host_safe_identity_capacity_uid_and_duplicate_cells_fail_closed() {
    for (field,number) in [("runGeneration",0),("playerObjectId",0),("catalogRevision",SAFE+1),("serviceRevision",0)] {
        let mut v=value();v[field]=number.into();assert!(parse(v).is_err(),"{field}");}
    let mut v=value();v["shop"]["goods"][1]["unique_id"]=0.into();assert!(parse(v).is_err());
    let mut v=value();v["inventory"]["capacity"]=47.into();assert!(parse(v).is_err());
    let row=serde_json::to_value(ItemModel {container:0,slot:0,unique_id:Some(9),key:"potion".into(),name:"Potion".into(),quantity:1,..Default::default()}).unwrap();
    let mut v=value();v["inventory"]["items"]=serde_json::json!([row.clone(),row]);assert!(parse(v).is_err());
    let mut v=value();v["feedback"]["pending"]=true.into();assert!(parse(v).is_err(),"pending with canReserve is incoherent");
}
#[test]
fn npc_shop_host_ingress_monotonic_owner_clocks_and_exact_withdrawal() {
    let mut i=NpcShopIngress::default();assert!(i.accept(snapshot()));assert!(!i.accept(snapshot()));
    for field in 0..5 {let mut old=snapshot();old.revision=2;match field {0=>old.model_revision=0,1=>old.presentation_revision=0,
        2=>old.service_revision=0,3=>old.catalog_revision=0,_=>old.identity.player_object_id=8}assert!(!i.accept(old));}
    let mut next=snapshot();next.revision=2;next.identity.owner_revision=1;assert!(i.accept(next.clone()));
    assert!(!i.withdraw(identity(1,0)));assert!(i.pending.is_some());assert!(i.withdraw(next.identity));assert!(i.pending.is_none());
    let mut newer=snapshot();newer.identity.run_generation=2;assert!(i.accept(newer));assert!(!i.withdraw(next.identity));assert!(i.pending.is_some());
}
#[test]
fn npc_shop_host_snapshot_acceptance_never_is_ready_or_allows_pending_ingress_pointer() {
    let snap=snapshot();assert!(!snap.context().ready);let r=read_model(&snap);let mut c=snap.context();let mut s=NpcShopSurfaceState::default();s.reconcile(&c,&r);
    let mut status=NpcShopStatus::from_context(&c,&s,&r);assert!(!status.ready && !status.input_enabled);
    let mut i=NpcShopIngress::default();assert!(i.accept(snap));let proof=s.proof(&c,&r).unwrap();
    assert!(!i.pointer(edge(proof,1,1,NpcShopSurfacePointerPhase::Down),true));
    settle(&mut i);c.ready=true;s.reconcile(&c,&r);status=NpcShopStatus::from_context(&c,&s,&r);
    assert!(status.ready);status.input_regions.push(InputRegion {left:1.,top:1.,width:10.,height:10.});status.disable();
    assert!(!status.ready && !status.input_enabled);assert!(status.input_regions.is_empty());
}
#[test]
fn npc_shop_host_world_origin_and_stale_down_nonce_never_forward_or_retire_new_lease() {
    let (mut i,c,r,s,_,status)=flow();i.edges.clear();let proof=s.proof(&c,&r).unwrap();
    let mut world=edge(proof,3,3,NpcShopSurfacePointerPhase::Down);world.gesture.origin=NpcShopSurfacePointerOrigin::World;
    assert!(i.pointer(world,true));let mut up=world;up.phase=NpcShopSurfacePointerPhase::Up;up.gesture.sequence=4;assert!(i.pointer(up,true));
    let mut q=NpcShopSurfaceIntentQueue::default();let mut now=NpcShopSurfaceState::default();now.reconcile(&c,&r);
    assert!(!now.request_action(NpcShopUiAction::Select(0),up.gesture,&c,&r,&mut q));
    let down=edge(proof,5,5,NpcShopSurfacePointerPhase::Down);assert!(i.pointer(down,true));
    let mut old=world;old.phase=NpcShopSurfacePointerPhase::Blur;old.gesture.sequence=SAFE;
    assert!(!i.pointer(old,false));assert_eq!(i.last_edge_sequence,5);assert_eq!(i.lease.unwrap().gesture.down_sequence,5);
    let cancel=edge(proof,6,5,NpcShopSurfacePointerPhase::Cancel);assert!(i.pointer(cancel,false));assert!(i.lease.is_none());
    assert!(i.checkpoint(&flow().4,&status,1).is_none(),"old gesture sequence cannot be forwarded");
}
#[test]
fn npc_shop_host_callback_checkpoint_before_and_after_is_exact_and_applies_once() {
    let (i,c,r,mut s,intent,status)=flow();let cp=i.checkpoint(&intent,&status,7).unwrap();
    assert_eq!(s.view(&c,&r).selected_id,None,"queued callback has not selected yet");
    assert!(i.checkpoint_current(cp,&intent,&status,7));assert!(s.allows(&intent,&c,&r));
    assert!(s.apply_accepted(&intent,&c,&r).is_some());assert_eq!(s.view(&c,&r).selected_id,Some(0));
    assert!(s.apply_accepted(&intent,&c,&r).is_none());
}
#[test]
fn npc_shop_host_callback_reentry_ingress_withdraw_error_sink_or_status_change_cannot_commit() {
    for changed in 0..7 {let (mut i,c,r,mut s,intent,mut status)=flow();let cp=i.checkpoint(&intent,&status,7).unwrap();
        let mut sink=7;match changed {0=>{let mut newer=snapshot();newer.revision=2;assert!(i.accept(newer));},
            1=>{assert!(i.withdraw(c.identity));},2=>i.reject_current(&value().to_string(),"callback injected malformed current"),
            3=>sink=8,4=>status.disable(),5=>status.control_revision=Some(u64::MAX),
            _=>{assert!(i.pointer(edge(intent.proof,3,3,NpcShopSurfacePointerPhase::Down),true));}}
        let allowed=i.checkpoint_current(cp,&intent,&status,sink)&&s.allows(&intent,&c,&r);
        assert!(!allowed);if allowed{s.apply_accepted(&intent,&c,&r);}
        assert_eq!(s.view(&c,&r).selected_id,None);assert!(s.finish_rejected(&intent));
        assert!(s.apply_accepted(&intent,&c,&r).is_none());}
}
#[test]
fn npc_shop_host_old_owner_malformed_ingress_cannot_poison_new_owner_callback() {
    let (mut i,c,r,s,intent,status)=flow();let cp=i.checkpoint(&intent,&status,3).unwrap();
    let mut old=value();old["ownerRevision"]=1.into();i.reject_current(&old.to_string(),"not current identity");
    assert!(i.error.is_none());assert!(i.checkpoint_current(cp,&intent,&status,3));assert!(s.allows(&intent,&c,&r));
    let mut next=snapshot();next.revision=2;next.identity.owner_revision=1;assert!(i.accept(next));settle(&mut i);
    i.reject_current(&value().to_string(),"late old owner malformed");assert!(i.error.is_none());
}
#[test]
fn npc_shop_host_feedback_transport_progress_serializes_readonly_without_buy_ack() {
    for phase in [NpcShopFeedbackPhase::Entered,NpcShopFeedbackPhase::Unknown,NpcShopFeedbackPhase::Flushed] {
        let mut snap=snapshot();snap.feedback=NpcShopFeedback {phase:Some(phase),pending:true,can_reserve:false,previous_unknown:1};
        let json=serde_json::to_string(&snap).unwrap();let parsed=NpcShopSnapshot::parse(&json).unwrap();let mut c=parsed.context();c.ready=true;
        let r=read_model(&parsed);let mut s=NpcShopSurfaceState::default();s.reconcile(&c,&r);let mut q=NpcShopSurfaceIntentQueue::default();
        let g=NpcShopSurfaceGesture {pointer_id:1,down_sequence:1,sequence:1,origin:NpcShopSurfacePointerOrigin::Shop,button:0};
        assert!(s.request_action(NpcShopUiAction::Select(0),g,&c,&r,&mut q));let intent=q.drain().remove(0);s.apply_accepted(&intent,&c,&r).unwrap();
        assert!(!s.view(&c,&r).can_buy);assert_eq!(c.feedback.phase,Some(phase.to_common()));assert!(c.feedback.pending);
        let status=NpcShopStatus::from_context(&c,&s,&r);let v=serde_json::to_value(status).unwrap();
        assert_eq!(v["feedback"]["pending"],true);assert_eq!(v["selectedId"],0);assert!(v["coreAuthorityRevision"].is_string());assert!(v["controlRevision"].is_string());
        assert_eq!(r.inventory.gold,100);assert!(!v.as_object().unwrap().contains_key("ack"));}
}

#[test]
fn npc_shop_host_pointer_and_identity_parse_require_exact_fields_and_full_decimal_strings() {
    let (_,c,r,s,_,_)=flow();let mut proof=s.proof(&c,&r).unwrap();proof.core_authority_revision=u64::MAX;proof.control_revision=9_007_199_254_740_992;
    let actual=edge(proof,1,1,NpcShopSurfacePointerPhase::Down);let v=serde_json::to_value(actual).unwrap();
    assert_eq!(parse_pointer(&v.to_string()).unwrap(),actual);
    for field in ["controlRevision","coreAuthorityRevision"] {
        for wrong in [serde_json::json!(1),serde_json::json!("01"),serde_json::json!("18446744073709551616")] {
            let mut invalid=v.clone();invalid[field]=wrong;assert!(parse_pointer(&invalid.to_string()).is_err());}
    }
    let mut invalid=v.clone();invalid["extra"]=true.into();assert!(parse_pointer(&invalid.to_string()).is_err());
    let mut invalid=v.clone();invalid.as_object_mut().unwrap().remove("downSequence");assert!(parse_pointer(&invalid.to_string()).is_err());
    let json=v.to_string().replacen(r#""sequence":1"#,r#""sequence":1,"sequence":2"#,1);
    assert!(parse_pointer(&json).is_err());
    let id=serde_json::to_value(c.identity).unwrap();assert_eq!(parse_identity(&id.to_string()).unwrap(),c.identity);
    let mut invalid=id.clone();invalid["extra"]=true.into();assert!(parse_identity(&invalid.to_string()).is_err());
    let mut invalid=id;invalid["playerObjectId"]=0.into();assert!(parse_identity(&invalid.to_string()).is_err());
}
#[test]
fn npc_shop_host_none_core_closed_surface_and_input_flags_remain_fail_closed() {
    let mut closed=value();closed["open"]=false.into();closed["inputEnabled"]=false.into();closed["showBuy"]=false.into();
    closed["presentation"]=Value::Null;closed["coreAuthorityRevision"]=Value::Null;
    let closed=parse(closed).unwrap();assert!(!closed.context().ready);assert!(closed.core_authority_revision.is_none());
    let mut wrong=value();wrong["coreAuthorityRevision"]=Value::Null;assert!(parse(wrong).is_err());
    let mut wrong=value();wrong["language"]="unknown".into();assert!(parse(wrong).is_err());
    let mut wrong=value();wrong["feedback"]["phase"]="unknown".into();wrong["feedback"]["pending"]=true.into();
    wrong["feedback"]["canReserve"]=false.into();assert!(parse(wrong).is_ok(),"Unknown is feedback, not success or implicit retry");
}
#[test]
fn npc_shop_host_ingress_exhaustion_and_old_owner_terminal_cannot_touch_new_lease() {
    let mut exhausted=NpcShopIngress {generation:u64::MAX,..Default::default()};assert!(!exhausted.accept(snapshot()));
    assert!(exhausted.exhausted);assert!(!exhausted.accept(snapshot()));assert!(exhausted.pending.is_none());
    let (mut i,old_c,r,s,_,_)=flow();let old=s.proof(&old_c,&r).unwrap();let mut next=snapshot();next.identity.owner_revision=1;next.revision=2;
    assert!(i.accept(next.clone()));settle(&mut i);let mut c=next.context();c.ready=true;let mut state=NpcShopSurfaceState::default();state.reconcile(&c,&r);
    let current=state.proof(&c,&r).unwrap();assert!(i.pointer(edge(current,1,1,NpcShopSurfacePointerPhase::Down),true));
    for phase in [NpcShopSurfacePointerPhase::Cancel,NpcShopSurfacePointerPhase::Blur,NpcShopSurfacePointerPhase::Up] {
        assert!(!i.pointer(edge(old,SAFE,1,phase),false));assert_eq!(i.last_edge_sequence,1);assert_eq!(i.lease.unwrap().proof,current);}
    assert!(i.pointer(edge(current,2,1,NpcShopSurfacePointerPhase::Up),true));assert!(i.lease.is_none());
}

#[test]
fn npc_shop_host_catalog_asset_paths_follow_actual_current_stack_image_not_raw_preview_icon() {
    let mut snap=snapshot();snap.shop.goods[0].icon=71;
    {let source=snap.shop.goods[0].tooltip_source.as_mut().unwrap();source.info.item_type=8;source.info.shape=0;
        source.info.stack_size=300;source.info.image=2960;}
    for (count,index) in [(199,3660),(200,3661),(300,3662)] {
        {let good=&mut snap.shop.goods[0];good.count=count;
            good.tooltip_source.as_mut().unwrap().user_item.as_mut().unwrap().count=count;
            assert_eq!(good.user_item_image_index(),Some(index));}
        let expected=format!("original-ui/Items/{index}.png");let paths=catalog_icon_paths(&snap.shop).unwrap();
        assert!(paths.contains(&expected));assert!(!paths.contains(&"original-ui/Items/71.png".to_string()));
        assert!(!paths.contains(&"original-ui/Items/2960.png".to_string()));
    }
    snap.shop.goods[0].tooltip_source=None;snap.shop.goods[0].icon=0;
    assert!(catalog_icon_paths(&snap.shop).is_err(),"unavailable image cannot create ready asset proof");
}

fn npc_capacity_snapshot() -> NpcShopSnapshot {
    let mut s=snapshot();
    s.inventory.items=(0..40).map(|slot|ItemModel {unique_id:Some(1000+u64::from(slot)),slot,container:0,quantity:1,..Default::default()})
        .chain((0..6).map(|slot|ItemModel {unique_id:Some(2000+u64::from(slot)),slot,container:1,quantity:1,..Default::default()})).collect();
    s.inventory.items[0]=ItemModel {unique_id:Some(77),slot:0,container:0,quantity:97,
        tooltip_source:Some(CrystalItemTooltipSourceModel {
            info:CrystalItemInfoModel {item_index:658,name:"Potion".into(),price:1,stack_size:99,item_type:13,..Default::default()},
            user_item:Some(CrystalUserItemModel {unique_id:77,item_index:658,count:97,identified:false,
                soul_bound_id:-1,wedding_ring:-1,..Default::default()}),..Default::default()}),..Default::default()};
    s.inventory.items[1].unique_id=Some(0);s.inventory.items[40].unique_id=Some(0);
    s.inventory.npc_gold_trade_capacity=Some(mir2_client_bevy::inventory::NpcGoldTradeCapacity {
        roster_valid:true,fresh_compatible_unique_ids:vec![77]});s
}
#[test]
fn npc_shop_host_capacity_reuses_shared_alias_and_complete_fresh_admission() {
    let s=npc_capacity_snapshot();let raw=serde_json::to_value(&s).unwrap();let parsed=parse(raw.clone()).unwrap();
    let plan:NpcGoldBuyPlan=serde_json::from_str(&plan_npc_gold_buy_json(&serde_json::json!({
        "shop":parsed.shop,"inventory":parsed.inventory,"quantity":2}).to_string())).unwrap();
    assert!(plan.can_buy);assert_eq!(plan.total_gold,Some(3));assert_eq!(plan.command.unwrap().count,2);
    for changed in 0..4 {let mut bad=raw.clone();match changed {
        0=>{bad["inventory"].as_object_mut().unwrap().remove("npcGoldTradeCapacity");},
        1=>bad["inventory"]["npcGoldTradeCapacity"]["rosterValid"]=false.into(),
        2=>bad["inventory"]["items"][40]["uniqueId"]=77.into(),
        _=>bad["inventory"]["items"][1]["uniqueId"]=77.into() }
        assert!(parse(bad).is_err());}
}
#[test]
fn npc_shop_host_capacity_raw_inner_and_listed_carrier_cannot_gain_serde_defaults() {
    let raw=serde_json::to_value(npc_capacity_snapshot()).unwrap();
    for changed in 0..4 {let mut bad=raw.clone();match changed {
        0=>{bad["inventory"]["npcGoldTradeCapacity"].as_object_mut().unwrap().remove("rosterValid");},
        1=>bad["inventory"]["npcGoldTradeCapacity"]["extra"]=true.into(),
        2=>{bad["inventory"]["items"][0]["tooltipSource"]["info"].as_object_mut().unwrap().remove("stack_size");},
        _=>{bad["inventory"]["items"][0]["tooltipSource"]["userItem"].as_object_mut().unwrap().remove("soul_bound_id");} }
        assert!(parse(bad).is_err());}
}
