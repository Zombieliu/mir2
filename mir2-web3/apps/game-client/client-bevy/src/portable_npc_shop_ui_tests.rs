//! Pure shared-surface regressions. No Window, renderer, socket, or attempt producer.
use super::*;
use crate::inventory::{CrystalItemInfoModel, CrystalItemTooltipSourceModel, CrystalUserItemModel};
use crate::shop::{NpcShopServiceMode, ShopGood};
fn good(id:u64)->ShopGood {
    ShopGood { unique_id:id,name:format!("Potion {id}"),price:1,count:1,stock:-1,panel_type:0,
        purchase_rate:Some(1.5),requires_gold_buy_plan:true,
        tooltip_source:Some(CrystalItemTooltipSourceModel {
            info:CrystalItemInfoModel { item_index:658,name:"Potion".into(),price:1,stack_size:99,item_type:13,..Default::default() },
            user_item:Some(CrystalUserItemModel { unique_id:id,item_index:658,count:1,is_shop_item:true,..Default::default() }),
            ..Default::default() }),..Default::default() }
}

fn models()->NpcShopSurfaceReadModel {
    NpcShopSurfaceReadModel { shop:ShopModel { goods:(0..10).map(good).collect(),
        service_mode:NpcShopServiceMode::Buy,supports_buy:true,supports_sell:true,..Default::default() },
        inventory:InventoryModel {gold:100,capacity:46,..Default::default()},player:PlayerStats::default() }
}
fn context()->NpcShopSurfaceContext {
    NpcShopSurfaceContext { identity:NpcShopSurfaceIdentity {run_generation:1,connection_generation:1,
        session_generation:1,owner_revision:0,player_object_id:7},revision:1,model_revision:1,
        presentation_revision:1,service_revision:1,catalog_revision:1,core_authority_revision:Some(1),
        open:true,input_enabled:true,show_buy:true,ready:true,
        presentation:Some(NpcShopSurfacePresentation {logical_width:1024.,logical_height:768.,stage_css_scale:1.,touch:false}),
        language:"en".into(),feedback:NpcGoldBuyFeedback {can_reserve:true,..Default::default()} }
}
fn gesture(sequence:u64)->NpcShopSurfaceGesture {
    NpcShopSurfaceGesture {pointer_id:1,down_sequence:sequence,sequence,
        origin:NpcShopSurfacePointerOrigin::Shop,button:0}
}
fn state(c:&NpcShopSurfaceContext,r:&NpcShopSurfaceReadModel)->NpcShopSurfaceState {
    let mut s=NpcShopSurfaceState::default();s.reconcile(c,r);s
}
fn request(s:&mut NpcShopSurfaceState,q:&mut NpcShopSurfaceIntentQueue,action:NpcShopUiAction,
    c:&NpcShopSurfaceContext,r:&NpcShopSurfaceReadModel)->NpcShopSurfaceIntent {
    assert!(s.request_action(action,gesture(q.next+1),c,r,q));
    let mut actual=q.drain();assert_eq!(actual.len(),1);actual.remove(0)
}
fn select(s:&mut NpcShopSurfaceState,q:&mut NpcShopSurfaceIntentQueue,c:&NpcShopSurfaceContext,r:&NpcShopSurfaceReadModel) {
    let intent=request(s,q,NpcShopUiAction::Select(0),c,r);
    assert!(s.apply_accepted(&intent,c,r).is_some());
}

#[test]
fn npc_shop_surface_select_and_quantity_enqueue_before_local_commit() {
    let c=context();let r=models();let mut s=state(&c,&r);let mut q=NpcShopSurfaceIntentQueue::default();
    let original=serde_json::to_string(&r.inventory).unwrap();
    let intent=request(&mut s,&mut q,NpcShopUiAction::Select(0),&c,&r);
    assert_eq!(s.view(&c,&r).selected_id,None);assert_eq!(intent.selected_id,None);
    assert!(!s.request_action(NpcShopUiAction::Select(1),gesture(2),&c,&r,&mut q));
    assert!(s.allows(&intent,&c,&r));assert!(s.apply_accepted(&intent,&c,&r).unwrap().changed);
    assert_eq!(s.view(&c,&r).selected_id,Some(0));
    let intent=request(&mut s,&mut q,NpcShopUiAction::QuantityInc,&c,&r);
    assert_eq!(s.view(&c,&r).quantity,1);assert_eq!(intent.quantity,1);
    assert!(s.finish_rejected(&intent));assert_eq!(s.view(&c,&r).quantity,1);
    let next=request(&mut s,&mut q,NpcShopUiAction::QuantityInc,&c,&r);
    assert!(s.apply_accepted(&next,&c,&r).is_some());assert_eq!(s.view(&c,&r).quantity,2);
    assert_eq!(serde_json::to_string(&r.inventory).unwrap(),original,"no optimistic inventory or gold mutation");
}
#[test]
fn npc_shop_surface_old_duplicate_or_forged_receipt_cannot_retire_fresh_action() {
    let c=context();let r=models();let mut s=state(&c,&r);let mut q=NpcShopSurfaceIntentQueue::default();
    let old=request(&mut s,&mut q,NpcShopUiAction::Select(0),&c,&r);assert!(s.finish_rejected(&old));
    let now=request(&mut s,&mut q,NpcShopUiAction::Select(1),&c,&r);assert_ne!(old.intent_sequence,now.intent_sequence);
    assert!(!s.finish_rejected(&old));assert!(s.apply_accepted(&old,&c,&r).is_none());
    for changed in 0..4 {let mut fake=now.clone();match changed {
        0=>fake.action=NpcShopSurfaceAction::Select {unique_id:2},1=>fake.quantity+=1,
        2=>fake.gesture.down_sequence+=1,_=>fake.proof.control_revision+=1 }
        assert!(!s.allows(&fake,&c,&r));assert!(!s.finish_rejected(&fake));}
    assert!(s.allows(&now,&c,&r));assert!(s.apply_accepted(&now,&c,&r).is_some());
    assert!(s.apply_accepted(&now,&c,&r).is_none());assert!(!s.finish_rejected(&now));
    assert_eq!(s.view(&c,&r).selected_id,Some(1));
}
#[test]
fn npc_shop_surface_owner_model_layout_service_catalog_and_core_clock_fence_old_receipt() {
    for changed in 0..8 {let c=context();let r=models();let mut s=state(&c,&r);let mut q=NpcShopSurfaceIntentQueue::default();
        let old=request(&mut s,&mut q,NpcShopUiAction::Select(0),&c,&r);let mut next=c.clone();
        match changed {0=>next.identity.owner_revision+=1,1=>next.identity.session_generation+=1,
            2=>next.model_revision+=1,3=>next.presentation_revision+=1,4=>next.service_revision+=1,
            5=>next.catalog_revision+=1,6=>next.core_authority_revision=Some(2),
            _=>next.presentation.as_mut().unwrap().logical_width=1200.}
        assert!(!s.allows(&old,&next,&r));assert!(s.apply_accepted(&old,&next,&r).is_none());
        s.reconcile(&next,&r);assert!(!s.finish_rejected(&old));
        let fresh=request(&mut s,&mut q,NpcShopUiAction::Select(1),&next,&r);
        assert!(s.apply_accepted(&fresh,&next,&r).is_some());assert_eq!(s.view(&next,&r).selected_id,Some(1));
    }
}
#[test]
fn npc_shop_surface_complete_raw_model_change_is_checked_without_relying_on_clock_bump() {
    for changed in 0..4 {let c=context();let r=models();let mut s=state(&c,&r);let mut q=NpcShopSurfaceIntentQueue::default();
        let old=request(&mut s,&mut q,NpcShopUiAction::Select(0),&c,&r);let mut next=r.clone();
        match changed {0=>next.shop.goods[0].description.push_str(" replaced source"),
            1=>next.shop.goods[0].tooltip_source.as_mut().unwrap().info.price+=1,
            2=>next.inventory.gold+=1,_=>next.player.level+=1}
        assert!(!s.allows(&old,&c,&next));assert!(s.apply_accepted(&old,&c,&next).is_none());
        s.reconcile(&c,&next);assert!(!s.finish_rejected(&old));
        let fresh=request(&mut s,&mut q,NpcShopUiAction::Select(0),&c,&next);
        assert!(s.apply_accepted(&fresh,&c,&next).is_some());
    }
}
#[test]
fn npc_shop_surface_buy_uses_current_shared_planner_and_does_not_create_attempt_receipt() {
    let c=context();let r=models();let mut s=state(&c,&r);let mut q=NpcShopSurfaceIntentQueue::default();select(&mut s,&mut q,&c,&r);
    let inc=request(&mut s,&mut q,NpcShopUiAction::QuantityInc,&c,&r);s.apply_accepted(&inc,&c,&r).unwrap();
    let before=s.view(&c,&r);assert_eq!(before.plan.total_gold,Some(3));
    let intent=request(&mut s,&mut q,NpcShopUiAction::Buy,&c,&r);assert!(!intent.preentry_withdraw);
    assert_eq!(intent.command,before.plan.command);assert_eq!(intent.command.as_ref().unwrap().item_index,0);
    assert_eq!(intent.command.as_ref().unwrap().count,2);let effect=s.apply_accepted(&intent,&c,&r).unwrap();
    assert_eq!(effect.buy,intent.command);assert_eq!(s.view(&c,&r).quantity,2);
    assert_eq!(c.feedback,NpcGoldBuyFeedback {can_reserve:true,..Default::default()},"surface cannot forge Core progress");
}
#[test]
fn npc_shop_surface_feedback_barrier_is_readonly_and_invalidates_old_control_proof() {
    for phase in [crate::npc_gold_buy_attempt::NpcGoldBuyAttemptPhase::Entered,
        crate::npc_gold_buy_attempt::NpcGoldBuyAttemptPhase::Unknown,crate::npc_gold_buy_attempt::NpcGoldBuyAttemptPhase::Flushed] {
        let c=context();let r=models();let mut s=state(&c,&r);let mut q=NpcShopSurfaceIntentQueue::default();select(&mut s,&mut q,&c,&r);
        let old=request(&mut s,&mut q,NpcShopUiAction::QuantityInc,&c,&r);let mut pending=c.clone();
        pending.feedback=NpcGoldBuyFeedback {phase:Some(phase),pending:true,can_reserve:false,previous_unknown:1};
        s.reconcile(&pending,&r);assert!(!s.allows(&old,&pending,&r));assert!(!s.finish_rejected(&old));
        assert!(!s.view(&pending,&r).can_buy);assert!(!s.request_action(NpcShopUiAction::Buy,gesture(9),&pending,&r,&mut q));
        let inc=request(&mut s,&mut q,NpcShopUiAction::QuantityInc,&pending,&r);s.apply_accepted(&inc,&pending,&r).unwrap();
        assert_eq!(pending.feedback.phase,Some(phase));assert!(pending.feedback.pending);assert_eq!(s.view(&pending,&r).quantity,2);
    }
}
#[test]
fn npc_shop_surface_snapshot_heartbeat_preserves_local_selection_but_retires_queued_proof() {
    let c=context();let r=models();let mut s=state(&c,&r);let mut q=NpcShopSurfaceIntentQueue::default();select(&mut s,&mut q,&c,&r);
    let old=request(&mut s,&mut q,NpcShopUiAction::QuantityInc,&c,&r);let mut next=c.clone();next.revision+=1;s.reconcile(&next,&r);
    assert_eq!(s.view(&next,&r).selected_id,Some(0));assert_eq!(s.view(&next,&r).quantity,1);assert!(!s.allows(&old,&next,&r));
    let fresh=request(&mut s,&mut q,NpcShopUiAction::QuantityInc,&next,&r);s.apply_accepted(&fresh,&next,&r).unwrap();
    assert_eq!(s.view(&next,&r).quantity,2);
}
#[test]
fn npc_shop_surface_full_u64_proof_clocks_are_canonical_decimal_strings() {
    let mut c=context();c.core_authority_revision=Some(u64::MAX);let r=models();let s=state(&c,&r);let mut proof=s.proof(&c,&r).unwrap();
    proof.control_revision=u64::MAX;let value=serde_json::to_value(proof).unwrap();
    assert_eq!(value["coreAuthorityRevision"],u64::MAX.to_string());assert_eq!(value["controlRevision"],u64::MAX.to_string());
    assert_eq!(serde_json::from_value::<NpcShopSurfaceProof>(value.clone()).unwrap(),proof);
    for invalid in [serde_json::json!(9_007_199_254_740_992_u64),serde_json::json!("0"),serde_json::json!("01"),
        serde_json::json!("+1"),serde_json::json!("18446744073709551616")] {
        let mut next=value.clone();next["coreAuthorityRevision"]=invalid;assert!(serde_json::from_value::<NpcShopSurfaceProof>(next).is_err());
    }
}
#[test]
fn npc_shop_surface_queue_exhaustion_stays_closed_across_clear_and_invalidate() {
    let c=context();let r=models();let mut s=state(&c,&r);let mut q=NpcShopSurfaceIntentQueue {next:SAFE,..Default::default()};
    assert!(!s.request_action(NpcShopUiAction::Select(0),gesture(1),&c,&r,&mut q));assert!(s.error.is_some());
    q.clear();s.invalidate();s.reconcile(&c,&r);
    assert!(!s.request_action(NpcShopUiAction::Select(0),gesture(2),&c,&r,&mut q));assert!(q.drain().is_empty());
}
#[test]
fn npc_shop_surface_world_origin_or_unready_context_never_enqueues_actions() {
    let c=context();let r=models();let mut s=state(&c,&r);let mut q=NpcShopSurfaceIntentQueue::default();
    let mut world=gesture(1);world.origin=NpcShopSurfacePointerOrigin::World;
    assert!(!s.request_action(NpcShopUiAction::Select(0),world,&c,&r,&mut q));
    for field in 0..4 {let mut unavailable=c.clone();match field {0=>unavailable.ready=false,1=>unavailable.open=false,
        2=>unavailable.input_enabled=false,_=>unavailable.core_authority_revision=None}
        s.reconcile(&unavailable,&r);assert!(!s.request_action(NpcShopUiAction::Select(0),gesture(2),&unavailable,&r,&mut q));}
    assert!(q.drain().is_empty());
}

#[test]
fn npc_shop_surface_page_close_and_sell_handoff_commit_only_after_matching_accept() {
    for action in [NpcShopUiAction::PageDown,NpcShopUiAction::Close,NpcShopUiAction::HandoffSell] {
        let c=context();let r=models();let mut s=state(&c,&r);let mut q=NpcShopSurfaceIntentQueue::default();select(&mut s,&mut q,&c,&r);
        let intent=request(&mut s,&mut q,action,&c,&r);assert!(intent.preentry_withdraw);
        assert_eq!(s.view(&c,&r).selected_id,Some(0));assert_eq!(s.view(&c,&r).start_index,0);
        let effect=s.apply_accepted(&intent,&c,&r).unwrap();
        match action {NpcShopUiAction::PageDown=>{assert_eq!(s.view(&c,&r).start_index,1);assert_eq!(s.view(&c,&r).selected_id,None);},
            NpcShopUiAction::Close=>assert!(effect.close),NpcShopUiAction::HandoffSell=>assert!(effect.handoff_sell),_=>unreachable!()}
        assert!(!s.finish_rejected(&intent));
    }
}
#[test]
fn npc_shop_surface_mixed_whole_catalog_is_legacy_but_marked_incomplete_remains_blocked_common() {
    let c=context();
    for kind in 0..3 {let mut r=models();match kind {0=>r.shop.goods[9].use_pearls=true,1=>r.shop.goods[9].stock=0,_=>r.shop.goods[9].panel_type=1}
        let mut s=state(&c,&r);let mut q=NpcShopSurfaceIntentQueue::default();assert!(s.proof(&c,&r).is_none());
        assert!(!s.request_action(NpcShopUiAction::Select(0),gesture(1),&c,&r,&mut q));assert!(q.drain().is_empty());}
    let mut r=models();r.shop.goods[0].tooltip_source=None;r.shop.goods[0].purchase_rate=None;
    let mut s=state(&c,&r);let mut q=NpcShopSurfaceIntentQueue::default();assert!(s.proof(&c,&r).is_some());select(&mut s,&mut q,&c,&r);
    assert!(!s.view(&c,&r).can_buy);assert!(!s.request_action(NpcShopUiAction::Buy,gesture(2),&c,&r,&mut q));
    assert!(q.drain().is_empty());
}

// Controlled measured nodes below exercise the real pointer system, separately
// from the actual painter/font/layout cases which establish tree completeness.
fn pointer_fixture()->App {
    let mut app=App::new();let c=context();let r=models();let s=state(&c,&r);let proof=s.proof(&c,&r).unwrap();
    let root=app.world_mut().spawn((NpcShopSurfaceRoot,NpcShopSurfaceTreeStamp {proof,selected_id:None,quantity:1,start_index:0})).id();
    for (id,x) in [(0,100.),(1,160.)] {let action=NpcShopUiAction::Select(id);
        app.world_mut().spawn((ChildOf(root),Button,action,ShopPaintControl {action,enabled:true},
            NpcShopUiStamp {source_revision:proof.core_authority_revision,presentation_revision:proof.control_revision},
            NpcShopSurfaceControl(proof),ComputedNode {size:Vec2::splat(40.),..Default::default()},
            bevy::ui::UiGlobalTransform::from_xy(x,100.),InheritedVisibility::VISIBLE));}
    app.insert_resource(c).insert_resource(r).insert_resource(s).init_resource::<NpcShopSurfacePointerEdges>()
        .init_resource::<NpcShopSurfaceIntentQueue>().add_systems(Update,process_pointer);app
}
fn pointer_edge(proof:NpcShopSurfaceProof,sequence:u64,down:u64,phase:NpcShopSurfacePointerPhase,origin:NpcShopSurfacePointerOrigin,x:f32)->NpcShopSurfacePointerEdge {
    NpcShopSurfacePointerEdge {proof,gesture:NpcShopSurfaceGesture {pointer_id:1,down_sequence:down,sequence,origin,button:0},phase,x,y:100.}
}
fn push_edge(app:&mut App,e:NpcShopSurfacePointerEdge) {app.world_mut().resource_mut::<NpcShopSurfacePointerEdges>().0.push(e);app.update();}
fn refresh_control_proof(app:&mut App) {
    let c=app.world().resource::<NpcShopSurfaceContext>().clone();let r=app.world().resource::<NpcShopSurfaceReadModel>().clone();
    let s=app.world().resource::<NpcShopSurfaceState>();let proof=s.proof(&c,&r).unwrap();let view=s.view(&c,&r);
    let root=app.world_mut().query_filtered::<Entity,With<NpcShopSurfaceRoot>>().single(app.world()).unwrap();
    app.world_mut().entity_mut(root).insert(NpcShopSurfaceTreeStamp {proof,selected_id:view.selected_id,quantity:view.quantity,start_index:view.start_index});
    let ids:Vec<_>=app.world_mut().query_filtered::<Entity,With<NpcShopSurfaceControl>>().iter(app.world()).collect();
    for id in ids {app.world_mut().entity_mut(id).insert((NpcShopSurfaceControl(proof),NpcShopUiStamp {
        source_revision:proof.core_authority_revision,presentation_revision:proof.control_revision}));}
}
#[test]
fn npc_shop_surface_actual_pointer_never_retargets_world_origin_into_shop_intent() {
    let mut app=pointer_fixture();let proof=app.world().resource::<NpcShopSurfaceState>().proof(
        app.world().resource::<NpcShopSurfaceContext>(),app.world().resource::<NpcShopSurfaceReadModel>()).unwrap();
    for (sequence,phase,x) in [(1,NpcShopSurfacePointerPhase::Down,100.),(2,NpcShopSurfacePointerPhase::Move,160.),(3,NpcShopSurfacePointerPhase::Up,160.)] {
        push_edge(&mut app,pointer_edge(proof,sequence,1,phase,NpcShopSurfacePointerOrigin::World,x));}
    assert!(app.world_mut().resource_mut::<NpcShopSurfaceIntentQueue>().drain().is_empty());
    assert!(app.world().resource::<NpcShopSurfaceState>().lease.is_none());
    assert_eq!(app.world().resource::<NpcShopSurfaceState>().view(app.world().resource::<NpcShopSurfaceContext>(),
        app.world().resource::<NpcShopSurfaceReadModel>()).selected_id,None);
    push_edge(&mut app,pointer_edge(proof,4,4,NpcShopSurfacePointerPhase::Down,NpcShopSurfacePointerOrigin::Shop,100.));
    push_edge(&mut app,pointer_edge(proof,5,4,NpcShopSurfacePointerPhase::Up,NpcShopSurfacePointerOrigin::Shop,100.));
    let actual=app.world_mut().resource_mut::<NpcShopSurfaceIntentQueue>().drain();assert_eq!(actual.len(),1);
    assert_eq!(actual[0].action,NpcShopSurfaceAction::Select {unique_id:0});
    assert_eq!(actual[0].gesture.down_sequence,4);
}
#[test]
fn npc_shop_surface_stale_terminal_cannot_clear_new_awaiting_or_advance_watermark() {
    let mut app=pointer_fixture();let c=app.world().resource::<NpcShopSurfaceContext>().clone();let r=models();
    let proof=app.world().resource::<NpcShopSurfaceState>().proof(&c,&r).unwrap();
    push_edge(&mut app,pointer_edge(proof,1,1,NpcShopSurfacePointerPhase::Down,NpcShopSurfacePointerOrigin::Shop,100.));
    push_edge(&mut app,pointer_edge(proof,2,1,NpcShopSurfacePointerPhase::Cancel,NpcShopSurfacePointerOrigin::Shop,100.));
    push_edge(&mut app,pointer_edge(proof,3,3,NpcShopSurfacePointerPhase::Down,NpcShopSurfacePointerOrigin::Shop,160.));
    push_edge(&mut app,pointer_edge(proof,4,3,NpcShopSurfacePointerPhase::Up,NpcShopSurfacePointerOrigin::Shop,160.));
    let actual=app.world_mut().resource_mut::<NpcShopSurfaceIntentQueue>().drain();assert_eq!(actual.len(),1);
    for phase in [NpcShopSurfacePointerPhase::Cancel,NpcShopSurfacePointerPhase::Blur,NpcShopSurfacePointerPhase::Up] {
        push_edge(&mut app,pointer_edge(proof,SAFE,1,phase,NpcShopSurfacePointerOrigin::Shop,100.));
        let s=app.world().resource::<NpcShopSurfaceState>();assert_eq!(s.last_pointer_sequence,4);assert!(s.allows(&actual[0],&c,&r));}
    assert!(app.world_mut().resource_mut::<NpcShopSurfaceState>().apply_accepted(&actual[0],&c,&r).is_some());
    assert_eq!(app.world().resource::<NpcShopSurfaceState>().view(&c,&r).selected_id,Some(1));
}
#[test]
fn npc_shop_surface_old_owner_terminal_does_not_retire_current_pointer_lease() {
    let mut app=pointer_fixture();let old_c=context();let r=models();let old=app.world().resource::<NpcShopSurfaceState>().proof(&old_c,&r).unwrap();
    push_edge(&mut app,pointer_edge(old,50,50,NpcShopSurfacePointerPhase::Down,NpcShopSurfacePointerOrigin::Shop,100.));
    push_edge(&mut app,pointer_edge(old,51,50,NpcShopSurfacePointerPhase::Up,NpcShopSurfacePointerOrigin::Shop,100.));
    let previous=app.world_mut().resource_mut::<NpcShopSurfaceIntentQueue>().drain();assert_eq!(previous.len(),1);
    let mut now=old_c.clone();now.identity.owner_revision+=1;now.service_revision+=1;now.catalog_revision+=1;
    app.insert_resource(now.clone());app.world_mut().resource_mut::<NpcShopSurfaceState>().reconcile(&now,&r);refresh_control_proof(&mut app);
    let current=app.world().resource::<NpcShopSurfaceState>().proof(&now,&r).unwrap();
    push_edge(&mut app,pointer_edge(current,1,1,NpcShopSurfacePointerPhase::Down,NpcShopSurfacePointerOrigin::Shop,160.));
    push_edge(&mut app,pointer_edge(old,100,50,NpcShopSurfacePointerPhase::Cancel,NpcShopSurfacePointerOrigin::Shop,100.));
    let s=app.world().resource::<NpcShopSurfaceState>();assert_eq!(s.last_pointer_sequence,1);assert_eq!(s.lease.unwrap().edge.proof,current);
    push_edge(&mut app,pointer_edge(current,2,1,NpcShopSurfacePointerPhase::Up,NpcShopSurfacePointerOrigin::Shop,160.));
    let actual=app.world_mut().resource_mut::<NpcShopSurfaceIntentQueue>().drain();assert_eq!(actual.len(),1);
    assert!(actual[0].intent_sequence>previous[0].intent_sequence,"identity reset affects pointer watermark, not intent token sequence");
    push_edge(&mut app,pointer_edge(old,101,50,NpcShopSurfacePointerPhase::Blur,NpcShopSurfacePointerOrigin::Shop,100.));
    assert_eq!(app.world().resource::<NpcShopSurfaceState>().last_pointer_sequence,2);
    assert!(app.world().resource::<NpcShopSurfaceState>().allows(&actual[0],&now,&r));
}

fn painted(p:NpcShopSurfacePresentation,dpr:f32,with_font:bool)->App {
    use bevy::asset::AssetApp;
    use bevy::camera::{ComputedCameraValues,RenderTargetInfo,Viewport};
    let mut app=App::new();app.add_plugins((MinimalPlugins,bevy::asset::AssetPlugin::default(),bevy::input::InputPlugin,
        bevy::image::ImagePlugin::default(),bevy::transform::TransformPlugin,bevy::camera::visibility::VisibilityPlugin,
        bevy::text::TextPlugin,bevy::ui::UiPlugin));
    app.init_asset::<Image>().init_asset::<Font>().init_asset::<bevy::image::TextureAtlasLayout>()
        .init_asset::<bevy::mesh::Mesh>().init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>();
    let font=Font::from_bytes(std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/original-ui/fonts/NotoSansCJKsc-Regular.otf")).expect("packaged font"));
    let font=app.world_mut().resource_mut::<Assets<Font>>().add(font);
    let physical=UVec2::new((p.logical_width*dpr) as u32,(p.logical_height*dpr) as u32);
    let camera=app.world_mut().spawn((Camera2d,Camera {computed:ComputedCameraValues {
        target_info:Some(RenderTargetInfo {physical_size:physical,scale_factor:dpr}),..Default::default()},
        viewport:Some(Viewport {physical_size:physical,..Default::default()}),..Default::default()})).id();
    app.insert_resource(QuestUiTargetCamera(camera));if with_font {app.insert_resource(QuestUiFont(font));}
    let mut c=context();c.presentation=Some(p);app.insert_resource(c).insert_resource(models()).add_plugins(Mir2PortableNpcShopUiPlugin);
    for _ in 0..4 {app.update();}app
}
fn observe(app:&mut App)->NpcShopSurfaceTreeObservation {
    let mut system=bevy::ecs::system::SystemState::<NpcShopSurfaceTreeObserver>::new(app.world_mut());
    let tree=system.get(app.world()).expect("actual NPC shop tree queries");
    tree.observe(app.world().resource::<NpcShopSurfaceContext>(),app.world().resource::<NpcShopSurfaceState>(),
        app.world().resource::<NpcShopSurfaceReadModel>())
}
fn near(a:f32,b:f32) {assert!((a-b).abs()<0.1,"{a} != {b}");}
fn painted_tap(app:&mut App,wanted:NpcShopUiAction,sequence:u64)->NpcShopSurfaceIntent {
    let c=app.world().resource::<NpcShopSurfaceContext>().clone();let r=app.world().resource::<NpcShopSurfaceReadModel>().clone();
    let proof=app.world().resource::<NpcShopSurfaceState>().proof(&c,&r).unwrap();
    let point={let world=app.world_mut();world.query::<(&NpcShopUiAction,&ComputedNode,&bevy::ui::UiGlobalTransform)>()
        .iter(world).find_map(|(a,n,t)|(*a==wanted).then(||logical_rect(n,t).unwrap().center())).expect("actual enabled painted control")};
    let mut down=pointer_edge(proof,sequence,sequence,NpcShopSurfacePointerPhase::Down,NpcShopSurfacePointerOrigin::Shop,point.x);down.y=point.y;
    let mut up=down;up.phase=NpcShopSurfacePointerPhase::Up;up.gesture.sequence+=1;
    app.world_mut().resource_mut::<NpcShopSurfacePointerEdges>().0.extend([down,up]);app.update();
    let mut intents=app.world_mut().resource_mut::<NpcShopSurfaceIntentQueue>().drain();assert_eq!(intents.len(),1);intents.remove(0)
}
#[test]
fn npc_shop_surface_actual_tree_complete_desktop_touch_fractional_and_dpr_two() {
    for (touch,dpr) in [(false,1.),(true,1.),(true,2.)] {
        let p=NpcShopSurfacePresentation {logical_width:1024.,logical_height:768.,stage_css_scale:1.,touch};
        let mut app=painted(p,dpr,true);let observation=observe(&mut app);assert!(observation.complete,"touch={touch} dpr={dpr}");
        assert_eq!(observation.input_regions.len(),2);let world=app.world_mut();
        let root=world.query_filtered::<&bevy::ui::LayoutConfig,With<NpcShopSurfaceRoot>>().single(world).unwrap();assert!(!root.use_rounding);
        let (node,transform)=world.query_filtered::<(&ComputedNode,&bevy::ui::UiGlobalTransform),With<NpcShopSurfaceViewport>>().single(world).unwrap();
        let viewport=logical_rect(node,transform).unwrap();near(viewport.width(),584.*p.scale());near(viewport.height(),334.*p.scale());
        near(viewport.width(),node.unrounded_size().x*node.inverse_scale_factor);
        assert_eq!(node.size(),node.unrounded_size(),"explicit root layout policy inherits fraction-preserving geometry");
        assert_eq!(world.query::<&ShopPaintGoodCell>().iter(world).count(),8);
        for node in world.query_filtered::<&ComputedNode,With<ShopPaintGoodCell>>().iter(world) {
            near(node.size().x*node.inverse_scale_factor,205.*p.scale());near(node.size().y*node.inverse_scale_factor,32.*p.scale());
            assert_eq!(node.size(),node.unrounded_size());}
        let (notice,transform)=world.query_filtered::<(&ComputedNode,&bevy::ui::UiGlobalTransform),With<ShopPaintFeedback>>().single(world).unwrap();
        let notice=logical_rect(notice,transform).unwrap();near(notice.max.x,viewport.max.x);near(notice.width(),330.*p.scale());
        assert!(notice.max.x<=p.logical_width && notice.max.y<=p.logical_height,"full notice rather than only244px panel fits");
        let font=world.resource::<QuestUiFont>().0.clone();
        for f in world.query::<&TextFont>().iter(world) {assert_eq!(f.font,bevy::text::FontSource::Handle(font.clone()));}
    }
}
#[test]
fn npc_shop_surface_actual_measured_pointer_queues_then_accept_or_reject_commits_local_state() {
    let p=NpcShopSurfacePresentation {logical_width:1024.,logical_height:768.,stage_css_scale:1.,touch:false};
    let mut app=painted(p,1.,true);assert!(observe(&mut app).complete);
    let select=painted_tap(&mut app,NpcShopUiAction::Select(0),1);let c=context();let r=models();
    assert_eq!(app.world().resource::<NpcShopSurfaceState>().view(&c,&r).selected_id,None);
    assert!(app.world_mut().resource_mut::<NpcShopSurfaceState>().apply_accepted(&select,&c,&r).is_some());app.update();app.update();
    assert!(observe(&mut app).complete);let inc=painted_tap(&mut app,NpcShopUiAction::QuantityInc,3);
    assert_eq!(app.world().resource::<NpcShopSurfaceState>().view(&c,&r).quantity,1);
    assert!(app.world_mut().resource_mut::<NpcShopSurfaceState>().finish_rejected(&inc));app.update();
    let retry=painted_tap(&mut app,NpcShopUiAction::QuantityInc,5);
    assert!(app.world_mut().resource_mut::<NpcShopSurfaceState>().apply_accepted(&retry,&c,&r).is_some());app.update();app.update();
    assert_eq!(app.world().resource::<NpcShopSurfaceState>().view(&c,&r).quantity,2);assert!(observe(&mut app).complete);
}
#[test]
fn npc_shop_surface_actual_clip_hidden_control_or_stale_tree_cannot_establish_readiness() {
    let p=NpcShopSurfacePresentation {logical_width:1024.,logical_height:768.,stage_css_scale:1.,touch:false};
    let mut app=painted(p,1.,true);assert!(observe(&mut app).complete);
    let viewport=app.world_mut().query_filtered::<Entity,With<NpcShopSurfaceViewport>>().single(app.world()).unwrap();
    app.world_mut().get_mut::<Node>(viewport).unwrap().width=Val::Px(244.);app.update();app.update();
    assert!(!observe(&mut app).complete,"clipped full notice rejects even when panel remains");
    app.world_mut().get_mut::<Node>(viewport).unwrap().width=Val::Px(584.);app.update();app.update();assert!(observe(&mut app).complete);
    let cell=app.world_mut().query_filtered::<Entity,With<ShopPaintGoodCell>>().iter(app.world()).next().unwrap();
    app.world_mut().get_mut::<Node>(cell).unwrap().display=Display::None;app.update();assert!(!observe(&mut app).complete);
    app.world_mut().get_mut::<Node>(cell).unwrap().display=Display::Flex;app.update();assert!(observe(&mut app).complete);
    app.world_mut().resource_mut::<NpcShopSurfaceContext>().presentation_revision+=1;
    assert!(!observe(&mut app).complete,"old painted proof is not the new applied presentation");
}
#[test]
fn npc_shop_surface_missing_font_or_narrow_touch_never_fabricates_complete_surface() {
    let p=NpcShopSurfacePresentation {logical_width:1024.,logical_height:768.,stage_css_scale:1.,touch:false};
    let mut app=painted(p,1.,false);assert!(!observe(&mut app).complete);
    let narrow=NpcShopSurfacePresentation {logical_width:640.,logical_height:480.,stage_css_scale:0.5,touch:true};
    assert!(!narrow.valid());let mut app=painted(narrow,1.,true);assert!(!observe(&mut app).complete);
    assert_eq!(app.world_mut().query::<&ShopPaintPanel>().iter(app.world()).count(),0);
}

#[test]
fn npc_shop_surface_full_bag_attested_capacity_is_shared_and_evidence_fences_pending_intent() {
    use crate::inventory::{ItemModel,NpcGoldTradeCapacity};
    let c=context();let mut r=models();
    r.inventory.items=(0..40).map(|slot|ItemModel {unique_id:Some(1000+u64::from(slot)),slot,container:0,quantity:1,..Default::default()})
        .chain((0..6).map(|slot|ItemModel {unique_id:Some(2000+u64::from(slot)),slot,container:1,quantity:1,..Default::default()})).collect();
    r.inventory.items[0]=ItemModel {unique_id:Some(77),slot:0,container:0,quantity:97,
        tooltip_source:Some(CrystalItemTooltipSourceModel {
            info:CrystalItemInfoModel {item_index:658,name:"Potion".into(),price:1,stack_size:99,item_type:13,..Default::default()},
            user_item:Some(CrystalUserItemModel {unique_id:77,item_index:658,count:97,identified:false,
                soul_bound_id:-1,wedding_ring:-1,..Default::default()}),..Default::default()}),..Default::default()};
    r.inventory.items[1].unique_id=Some(0);r.inventory.items[40].unique_id=Some(0);
    r.inventory.npc_gold_trade_capacity=Some(NpcGoldTradeCapacity {roster_valid:true,fresh_compatible_unique_ids:vec![77]});
    let before=serde_json::to_value(&r.inventory).unwrap();let mut s=state(&c,&r);let mut q=NpcShopSurfaceIntentQueue::default();
    select(&mut s,&mut q,&c,&r);
    let increment=request(&mut s,&mut q,NpcShopUiAction::QuantityInc,&c,&r);assert!(s.apply_accepted(&increment,&c,&r).is_some());
    assert_eq!(s.view(&c,&r).quantity,2);assert!(s.view(&c,&r).plan.can_buy);
    let buy=request(&mut s,&mut q,NpcShopUiAction::Buy,&c,&r);assert_eq!(buy.command.as_ref().unwrap().count,2);
    let mut stale=r.clone();stale.inventory.npc_gold_trade_capacity.as_mut().unwrap().fresh_compatible_unique_ids.clear();
    assert!(!s.allows(&buy,&c,&stale));assert!(s.apply_accepted(&buy,&c,&stale).is_none());
    assert!(s.allows(&buy,&c,&r));assert!(s.finish_rejected(&buy));
    assert_eq!(serde_json::to_value(&r.inventory).unwrap(),before,"no optimistic merge or debit");
}
