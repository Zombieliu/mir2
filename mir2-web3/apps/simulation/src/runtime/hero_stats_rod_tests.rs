use super::*;
#[test]
fn hero_stats_fishing_rod_root_weight_counts_but_socket_stats_and_weight_do_not() {
    use crate::{ItemContainer,SimulationConfig,SimulationSession};
    use mir2_protocol::{ClientPacket,MirClass,MirGender};
    use super::super::items::{embedded_item_state_from_template,try_item_state_from_user_item};
    let mut s=SimulationSession::new(SimulationConfig::default());
    s.handle_packet(ClientPacket::Login{account_id:"demo".into(),password:"demo".into()});
    s.handle_packet(ClientPacket::StartGame{character_index:0});
    s.handle_packet(ClientPacket::NewHero{name:"Aide".into(),gender:MirGender::Female,class:MirClass::Taoist});
    let manifest=mir2_game_data::crystal_item_manifest();
    let rod=manifest.items.iter().find(|t|t.item_type==1&&matches!(t.shape,49|50)).unwrap();
    let mut item=embedded_item_state_from_template(rod,ItemContainer::Bag1,0);item.unique_id=91001;
    let mut wire=try_user_item_from_item_state(&item).unwrap();wire.current_dura=wire.max_dura;
    let item=try_item_state_from_user_item(item,&wire).unwrap();
    s.app.world_mut().resource_mut::<HeroInventoryResource>().equipment=vec![item.clone()];
    let base=compute(s.app.world());assert_eq!(base.hand,u32::from(rod.weight));
    let child_template=mir2_game_data::crystal_item_by_index(1).unwrap();
    let mut child=embedded_item_state_from_template(&child_template,ItemContainer::Bag1,0);child.unique_id=91002;
    let mut child_wire=try_user_item_from_item_state(&child).unwrap();
    child_wire.added_stats=vec![UserItemStat{stat:12,value:1000}];
    wire.slots=vec![Some(child_wire)];
    s.app.world_mut().resource_mut::<HeroInventoryResource>().equipment=vec![try_item_state_from_user_item(item,&wire).unwrap()];
    let with_socket=compute(s.app.world());
    assert_eq!(base.values,with_socket.values);
    assert_eq!((base.bag,base.wear,base.hand),(with_socket.bag,with_socket.wear,with_socket.hand));
}
