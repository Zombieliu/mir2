//! Pure in-memory session/domain tests. No network, receipt correlation or persistence claim.
use super::*;
use super::super::resources::{InventoryResource, NpcStateResource, PlayerRuntimeResource, SessionResource, Stage5SystemsResource};
use super::super::npc::{NpcBuyBackItemState, NpcBuyBackState, NpcUsedGoodsState};
use super::super::inventory::future_binary_datetime_minutes;
use crate::{ActiveSessionIdentity, InProcessWorldRuntime, SimulationConfig, SimulationSession,
    VisibleNpcRecord, WorldCommand, WorldRuntime, WorldSnapshot};
use mir2_protocol::{ClientPacket, MirDirection, Point, UserItem};

const SCRIPT: &str = "BichonProvince/NaturalCave/WickedTrader";
fn config() -> SimulationConfig {
    let mut config = SimulationConfig::default();
    config.visible_npcs.push(VisibleNpcRecord { object_id:4990,name:"Wicked Trader".into(),
        image:5,colour_argb:-1,position:Point{x:331,y:271},direction:MirDirection::Left,
        quest_ids:vec![],script_key:Some(SCRIPT.into()) });
    config
}
fn login(s: &mut SimulationSession) {
    assert!(s.handle_packet(ClientPacket::Login { account_id:"demo".into(),password:"demo".into() })
        .iter().any(|p|matches!(p,ServerPacket::LoginSuccess{..})));
}
fn session() -> (SimulationSession, NpcGoldBuyRequest) {
    let mut s = SimulationSession::new(config());
    login(&mut s);
    s.handle_packet(ClientPacket::StartGame{character_index:0});
    s.interact(4990);
    let packets = s.select_npc_dialog_target("@BuySell");
    let uid = packets.iter().find_map(|p|match p {
        ServerPacket::NPCGoods{list,..} => list.iter().find(|i|i.item_index==658 && i.count==1).map(|i|i.unique_id),
        _ => None,
    }).expect("actual trade catalogue");
    {
        let mut i=s.app.world_mut().resource_mut::<InventoryResource>();
        i.inventory_capacity=86; i.inventory_items.clear();i.belt_items.clear();
        i.equipment_items.clear();i.storage_items.clear();i.reserved_item_unique_ids.clear();
    }
    s.app.world_mut().resource_mut::<PlayerRuntimeResource>().gold=100_000;
    (s,NpcGoldBuyRequest{item_index:uid,count:2,panel_type:0})
}
fn snapshot(s:&SimulationSession)->serde_json::Value {
    let i=s.app.world().resource::<InventoryResource>();
    let n=s.app.world().resource::<NpcStateResource>();
    serde_json::json!({"gold":s.app.world().resource::<PlayerRuntimeResource>().gold,
        "pearls":s.app.world().resource::<Stage5SystemsResource>().stage5_systems.intelligent_creature_pearls,
        "bag":i.inventory_items,"belt":i.belt_items,"equipment":i.equipment_items,
        "storage":i.storage_items,"reserved":i.reserved_item_unique_ids,
        "used":n.npc_used_goods_items,"buyBack":n.npc_buy_back_items})
}
fn committed(e:&NpcGoldBuyProcessingExecution,r:NpcGoldBuyRequest)->u64 {
    let NpcGoldBuyProcessingOutcome::Committed{request,gold_spent,incoming_unique_id}=&e.outcome
        else {panic!("direct commit outcome");};
    assert_eq!(*request,r);assert_eq!(*gold_spent,160);assert_ne!(*incoming_unique_id,0);
    let delta=e.packets.iter().find_map(|p|match p {ServerPacket::GainedItem{item}=>Some(item),_=>None}).unwrap();
    assert_eq!(delta.unique_id,*incoming_unique_id);assert_eq!(delta.count,2);
    assert!(e.packets.iter().any(|p|matches!(p,ServerPacket::LoseGold{gold:160})));
    *incoming_unique_id
}
fn rejected(s:&mut SimulationSession,r:NpcGoldBuyRequest,reason:NpcGoldBuyRejection) {
    let before=snapshot(s);
    let execution=s.try_npc_gold_buy_with_outcome(r).unwrap();
    assert_eq!(execution.outcome,NpcGoldBuyProcessingOutcome::Rejected{request:r,reason});
    assert!(execution.packets.iter().all(|p|!matches!(p,ServerPacket::LoseGold{..}|ServerPacket::GainedItem{..})));
    assert_eq!(snapshot(s),before);
}
#[test]
fn npc_gold_buy_outcome_real_commit_is_current_memory_not_delivery_ack() {
    let (mut s,r)=session();let before=snapshot(&s);
    let e=s.try_npc_gold_buy_with_outcome(r).unwrap();committed(&e,r);
    assert_eq!(s.app.world().resource::<PlayerRuntimeResource>().gold,99_840);
    assert_ne!(snapshot(&s),before);
    assert_eq!(s.app.world().resource::<InventoryResource>().belt_items.iter()
        .chain(s.app.world().resource::<InventoryResource>().inventory_items.iter())
        .filter(|i|i.user_item_metadata.as_ref().and_then(|u|u.item_index)==Some(658)).map(|i|i.quantity).sum::<u32>(),2);
}
#[test]
fn npc_gold_buy_outcome_same_four_keys_success_reject_success_never_leaks_previous_result() {
    let (mut s,r)=session();let first=s.try_npc_gold_buy_with_outcome(r).unwrap();let uid=committed(&first,r);
    s.app.world_mut().resource_mut::<PlayerRuntimeResource>().gold=0;
    rejected(&mut s,r,NpcGoldBuyRejection::InsufficientGold);
    s.app.world_mut().resource_mut::<PlayerRuntimeResource>().gold=1_000;
    let next=s.try_npc_gold_buy_with_outcome(r).unwrap();let next_uid=committed(&next,r);assert_ne!(next_uid,uid);
    let i=s.app.world().resource::<InventoryResource>();
    assert!(i.belt_items.iter().chain(i.inventory_items.iter()).all(|item|item.unique_id!=next_uid),
        "merged incoming delta identity is not a delivered live entity");
    assert_eq!(s.app.world().resource::<PlayerRuntimeResource>().gold,840);
}
#[test]
fn npc_gold_buy_outcome_validation_rejections_do_not_mutate_purchase() {
    for case in 0..5 {
        let(mut s,mut r)=session();
        let reason=match case {
            0=>{r.count=0;NpcGoldBuyRejection::InvalidRequest},
            1=>{r.panel_type=1;NpcGoldBuyRejection::InvalidRequest},
            2=>{r.item_index=u64::MAX;NpcGoldBuyRejection::UnknownGood},
            3=>{r.count=u16::MAX;NpcGoldBuyRejection::InvalidQuantity},
            _=>{s.app.world_mut().resource_mut::<NpcStateResource>().active_npc_service=None;NpcGoldBuyRejection::ServiceUnavailable},
        };rejected(&mut s,r,reason);
    }
}
#[test]
fn npc_gold_buy_outcome_invalid_roster_rejects_without_debit_or_partial_carrier() {
    let(mut s,r)=session();
    s.app.world_mut().resource_mut::<InventoryResource>().inventory_capacity=0;
    rejected(&mut s,r,NpcGoldBuyRejection::InvalidDelivery);
}
#[test]
fn npc_gold_buy_outcome_legacy_packet_economics_and_full_incoming_carrier_unchanged() {
    let(mut typed,r)=session();let(mut legacy,l)=session();
    let e=typed.try_npc_gold_buy_with_outcome(r).unwrap();committed(&e,r);
    let packets=legacy.try_handle_packet(l.packet()).unwrap();
    assert_eq!(packets.iter().filter(|p|matches!(p,ServerPacket::LoseGold{gold:160})).count(),1);
    let mut old=packets.iter().find_map(|p|match p{ServerPacket::GainedItem{item}=>Some(item.clone()),_=>None}).unwrap();
    let mut new=e.packets.iter().find_map(|p|match p{ServerPacket::GainedItem{item}=>Some(item.clone()),_=>None}).unwrap();
    // Global fresh allocator may issue distinct IDs to these independent sessions.
    assert_ne!(old.unique_id,0);assert_ne!(new.unique_id,0);old.unique_id=0;new.unique_id=0;
    assert_eq!(old,new);assert_eq!(typed.app.world().resource::<PlayerRuntimeResource>().gold,
        legacy.app.world().resource::<PlayerRuntimeResource>().gold);
}
#[test]
fn npc_gold_buy_outcome_postprocessing_error_preserves_actual_committed_outcome() {
    let(mut s,r)=session();
    let error=s.npc_gold_buy_with_postprocessing_failure(r).unwrap_err();
    let NpcGoldBuyProcessingError::PostProcessing{outcome,detail}=error else{panic!("known outcome must survive");};
    assert!(matches!(outcome,NpcGoldBuyProcessingOutcome::Committed{request,gold_spent:160,..} if request==r));
    assert_eq!(detail,"controlled postprocessing failure");
    assert_eq!(s.app.world().resource::<PlayerRuntimeResource>().gold,99_840);
    s.app.world_mut().resource_mut::<PlayerRuntimeResource>().gold=0;
    rejected(&mut s,r,NpcGoldBuyRejection::InsufficientGold);
}
#[test]
fn npc_gold_buy_outcome_missing_capture_and_error_are_unknown_not_rejected() {
    assert!(matches!(finish_npc_gold_buy_processing(Ok(vec![]),None),Err(NpcGoldBuyProcessingError::Unknown{..})));
    assert_eq!(finish_npc_gold_buy_processing(Err("ambiguous execution".into()),None).unwrap_err(),
        NpcGoldBuyProcessingError::Unknown{detail:"ambiguous execution".into()});
}
#[test]
fn npc_gold_buy_outcome_session_authentication_and_start_game_are_both_required() {
    let r=NpcGoldBuyRequest{item_index:0,count:2,panel_type:0};
    let mut s=SimulationSession::new(config());let before=snapshot(&s);
    assert_eq!(s.try_npc_gold_buy_with_outcome(r).unwrap_err(),
        NpcGoldBuyProcessingError::BeforeExecution(NpcGoldBuyBeforeExecution::NotAuthenticated));
    assert_eq!(snapshot(&s),before);
    login(&mut s);let before=snapshot(&s);
    assert_eq!(s.try_npc_gold_buy_with_outcome(r).unwrap_err(),
        NpcGoldBuyProcessingError::BeforeExecution(NpcGoldBuyBeforeExecution::NotInGame));
    assert_eq!(snapshot(&s),before);
    s.handle_packet(ClientPacket::StartGame{character_index:0});
    s.app.world_mut().resource_mut::<SessionResource>().account_id=None;
    let before=snapshot(&s);
    assert_eq!(s.try_npc_gold_buy_with_outcome(r).unwrap_err(),
        NpcGoldBuyProcessingError::BeforeExecution(NpcGoldBuyBeforeExecution::NotAuthenticated));
    assert_eq!(snapshot(&s),before,"selected character cannot substitute for authentication");
}
#[test]
fn npc_gold_buy_outcome_trusted_true_cannot_upgrade_unlogged_or_pre_start_runtime() {
    let r=NpcGoldBuyRequest{item_index:0,count:2,panel_type:0};
    let mut runtime=InProcessWorldRuntime::new(config());
    assert!(runtime.supports_typed_npc_gold_buy_outcome());
    assert_eq!(runtime.execute_production_npc_gold_buy_requiring_typed_outcome(true,r).unwrap_err(),
        NpcGoldBuyProcessingError::BeforeExecution(NpcGoldBuyBeforeExecution::NotAuthenticated));
    runtime.execute(WorldCommand::ClientPacket(ClientPacket::Login{account_id:"demo".into(),password:"demo".into()})).unwrap();
    assert_eq!(runtime.execute_production_npc_gold_buy_requiring_typed_outcome(true,r).unwrap_err(),
        NpcGoldBuyProcessingError::BeforeExecution(NpcGoldBuyBeforeExecution::NotInGame));
    runtime.execute(WorldCommand::ClientPacket(ClientPacket::StartGame{character_index:0})).unwrap();
    assert_eq!(runtime.execute_production_npc_gold_buy_requiring_typed_outcome(false,r).unwrap_err(),
        NpcGoldBuyProcessingError::BeforeExecution(NpcGoldBuyBeforeExecution::NotAuthenticated));
    let e=runtime.execute_production_npc_gold_buy_requiring_typed_outcome(true,r).unwrap();
    assert_eq!(e.outcome,NpcGoldBuyProcessingOutcome::Rejected{request:r,reason:NpcGoldBuyRejection::ServiceUnavailable});
}
struct Unsupported { s:SimulationSession, executions:usize }
impl WorldRuntime for Unsupported {
    fn as_any(&self)->&dyn std::any::Any{self}
    fn as_any_mut(&mut self)->&mut dyn std::any::Any{self}
    fn on_connect(&self)->Vec<ServerPacket>{vec![]}
    fn execute(&mut self,_:WorldCommand)->Result<Vec<ServerPacket>,String>{self.executions+=1;Ok(vec![])}
    fn world_snapshot(&self)->WorldSnapshot{self.s.world_snapshot()}
    fn active_identity(&self)->Option<ActiveSessionIdentity>{self.s.active_identity()}
    fn save_active_character(&mut self)->Result<(),String>{panic!("must not save")}
    fn refresh_active_external_mail(&mut self)->bool{false}
}
#[test]
fn npc_gold_buy_outcome_unsupported_runtime_never_executes_legacy_fallback() {
    let mut runtime=Unsupported{s:SimulationSession::new(config()),executions:0};
    assert!(!runtime.supports_typed_npc_gold_buy_outcome());
    assert_eq!(runtime.execute_production_npc_gold_buy_requiring_typed_outcome(true,
        NpcGoldBuyRequest{item_index:0,count:2,panel_type:0}).unwrap_err(),
        NpcGoldBuyProcessingError::BeforeExecution(NpcGoldBuyBeforeExecution::UnsupportedRuntime));
    assert_eq!(runtime.executions,0);
}
#[test]
fn npc_gold_buy_outcome_pearl_service_rejects_typed_but_legacy_purchase_is_unchanged() {
    let(mut s,r)=session();
    s.app.world_mut().resource_mut::<NpcStateResource>().active_npc_service.as_mut().unwrap().label_key="PEARLBUY".into();
    s.app.world_mut().resource_mut::<Stage5SystemsResource>().stage5_systems.intelligent_creature_pearls=240;
    rejected(&mut s,r,NpcGoldBuyRejection::UnsupportedService);
    let packets=s.handle_packet(r.packet());
    assert!(packets.iter().any(|p|matches!(p,ServerPacket::GainedItem{item} if item.count==2)));
    assert!(!packets.iter().any(|p|matches!(p,ServerPacket::LoseGold{..})));
    assert_eq!(s.app.world().resource::<Stage5SystemsResource>().stage5_systems.intelligent_creature_pearls,80);
    assert_eq!(s.app.world().resource::<PlayerRuntimeResource>().gold,100_000);
}
#[test]
fn npc_gold_buy_outcome_resale_services_reject_typed_and_keep_legacy_carrier_path() {
    for label in ["BUYBACK","BUYUSED"] {
        let(mut s,mut r)=session();
        let template=mir2_game_data::crystal_item_by_index(658).unwrap();
        let item=UserItem{unique_id:444_000,item_index:658,current_dura:template.durability,
            max_dura:template.durability,count:2,soul_bound_id:-1,identified:false,cursed:false,
            slots:vec![None;usize::from(template.slots)],gem_count:0,added_stats:vec![],awake_type:0,
            awake_values:vec![],refined_value:0,refine_added:0,refine_success_chance:0,wedding_ring:-1,
            expire_info:None,rental_information:None,is_shop_item:false,sealed_info:None,gm_made:false};
        r.item_index=item.unique_id;
        let name=s.app.world().resource::<SessionResource>().selected_character.as_ref().unwrap().name.clone();
        {
            let mut n=s.app.world_mut().resource_mut::<NpcStateResource>();
            n.active_npc_service.as_mut().unwrap().label_key=label.into();
            if label=="BUYBACK" {n.npc_buy_back_items.push(NpcBuyBackState{script_key:SCRIPT.into(),player_name:name,
                items:vec![NpcBuyBackItemState{item:item.clone(),expires_at_binary_datetime:future_binary_datetime_minutes(60)}]});}
            else {n.npc_used_goods_items.push(NpcUsedGoodsState{script_key:SCRIPT.into(),items:vec![item.clone()]});}
        }
        rejected(&mut s,r,NpcGoldBuyRejection::UnsupportedService);
        let packets=s.handle_packet(r.packet());
        assert!(packets.iter().any(|p|matches!(p,ServerPacket::LoseGold{gold:160})));
        assert!(packets.iter().any(|p|matches!(p,ServerPacket::GainedItem{item} if item.count==2)));
        let n=s.app.world().resource::<NpcStateResource>();
        if label=="BUYBACK"{assert!(n.npc_buy_back_items.iter().all(|entry|entry.items.is_empty()));}
        else{assert!(n.npc_used_goods_items.iter().all(|entry|entry.items.is_empty()));}
    }
}

#[test]
fn npc_gold_buy_outcome_ordinary_buy_used_fallback_is_typed_rejected_before_legacy_economics() {
    let(mut s,mut r)=session();
    let first=s.try_npc_gold_buy_with_outcome(r).unwrap();
    let mut item=first.packets.iter().find_map(|p|match p{ServerPacket::GainedItem{item}=>Some(item.clone()),_=>None}).unwrap();
    item.unique_id=444_001;r.item_index=item.unique_id;
    s.app.world_mut().resource_mut::<InventoryResource>().inventory_items.clear();
    s.app.world_mut().resource_mut::<InventoryResource>().belt_items.clear();
    s.app.world_mut().resource_mut::<PlayerRuntimeResource>().gold=100_000;
    {
        let mut n=s.app.world_mut().resource_mut::<NpcStateResource>();
        n.active_npc_service.as_mut().unwrap().label_key="BUY".into();
        n.npc_used_goods_items.push(NpcUsedGoodsState{script_key:SCRIPT.into(),items:vec![item]});
    }
    rejected(&mut s,r,NpcGoldBuyRejection::UnsupportedService);
    let packets=s.handle_packet(r.packet());
    assert!(packets.iter().any(|p|matches!(p,ServerPacket::LoseGold{gold:160})));
    assert!(packets.iter().any(|p|matches!(p,ServerPacket::GainedItem{item} if item.count==2)));
    assert!(s.app.world().resource::<NpcStateResource>().npc_used_goods_items.iter().all(|entry|entry.items.is_empty()));
}

#[test]
fn npc_gold_buy_outcome_post_commit_panic_preserves_known_commit_and_actual_inventory() {
    let(mut s,r)=session();
    let before=snapshot(&s);
    let error=s.npc_gold_buy_with_postprocessing_panic(r).unwrap_err();
    let NpcGoldBuyProcessingError::PostProcessing{outcome,detail}=error else{panic!("known commit survives unwind");};
    assert!(matches!(outcome,NpcGoldBuyProcessingOutcome::Committed{request,gold_spent:160,incoming_unique_id}
        if request==r && incoming_unique_id!=0));
    assert_eq!(detail,"purchase processing panicked; outcome may be unknown");
    assert_eq!(s.app.world().resource::<PlayerRuntimeResource>().gold,99_840);
    assert_ne!(snapshot(&s),before);
    let i=s.app.world().resource::<InventoryResource>();
    assert_eq!(i.belt_items.iter().chain(i.inventory_items.iter())
        .filter(|item|item.user_item_metadata.as_ref().and_then(|u|u.item_index)==Some(658))
        .map(|item|item.quantity).sum::<u32>(),2);
}

#[test]
fn npc_gold_buy_outcome_actual_precommit_panic_is_unknown_without_gold_debit() {
    let(mut s,r)=session();
    let gold=s.app.world().resource::<PlayerRuntimeResource>().gold;
    // Actual handler/pipeline cannot obtain this required resource. No simulated
    // outcome or packet oracle supplies a rejection and no live purchase commits.
    let inventory=s.app.world_mut().remove_resource::<InventoryResource>().unwrap();
    assert_eq!(s.try_npc_gold_buy_with_outcome(r).unwrap_err(),
        NpcGoldBuyProcessingError::Unknown{detail:"purchase processing panicked; outcome may be unknown".into()});
    assert_eq!(s.app.world().resource::<PlayerRuntimeResource>().gold,gold);
    assert!(s.app.world().get_resource::<InventoryResource>().is_none());
    s.app.world_mut().insert_resource(inventory);
    let e=s.try_npc_gold_buy_with_outcome(r).unwrap();
    committed(&e,r);
}
