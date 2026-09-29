//! Hero ride state uses its own mount and socket custody. Only @RIDE mirrors
//! the owner's requested state; the owner's saddle/bells never grant abilities.
use super::super::{
    components::{hero_entity, ObjectId},
    items::try_user_item_from_item_state,
    resources::{HeroInventoryResource, MountResource, Stage5SystemsResource},
};
use bevy_ecs::prelude::{Resource, World};
use mir2_protocol::ServerPacket;
#[derive(Resource, Default, Clone)]
struct HeroMount {
    identity: String,
    riding: bool,
}
fn identity(world: &World) -> String {
    world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .hero
        .as_ref()
        .map(|hero| format!("{}:{:?}:{:?}", hero.name, hero.class, hero.gender))
        .unwrap_or_default()
}
#[derive(Default)]
struct Parts {
    mount_type: i16,
    saddle: bool,
    reins: bool,
    bells: bool,
}
fn parts(world: &World) -> Parts {
    let mut out = Parts {
        mount_type: -1,
        ..Default::default()
    };
    let Some(item) = world
        .resource::<HeroInventoryResource>()
        .equipment
        .iter()
        .find(|item| item.slot == 13)
    else {
        return out;
    };
    let Ok(item) = try_user_item_from_item_state(item) else {
        return out;
    };
    let Some(base) = mir2_game_data::crystal_item_by_index(item.item_index) else {
        return out;
    };
    if base.item_type != super::super::crystal_compat::CRYSTAL_ITEM_TYPE_MOUNT
        || item.current_dura == 0
    {
        return out;
    }
    out.mount_type = base.shape;
    out.reins = item.slots.first().is_some_and(Option::is_some);
    out.bells = item.slots.get(1).is_some_and(Option::is_some);
    out.saddle = item.slots.get(2).is_some_and(Option::is_some);
    out
}
fn eligible(world: &World, parts: &Parts) -> bool {
    parts.mount_type >= 0
        && parts.saddle
        && !super::super::map::current_map_disallows_mount(world)
        && (!super::super::map::current_map_requires_bridle(world) || parts.reins)
}
pub(in crate::runtime) fn riding(world: &World) -> bool {
    world
        .get_resource::<HeroMount>()
        .is_some_and(|state| state.identity == identity(world) && state.riding)
        && eligible(world, &parts(world))
}
pub(in crate::runtime) fn can_attack(world: &World) -> bool {
    !riding(world) || parts(world).bells
}
pub(in crate::runtime) fn reset(world: &mut World) {
    world.remove_resource::<HeroMount>();
}
pub(in crate::runtime) fn refresh(world: &mut World) -> Vec<ServerPacket> {
    let key = identity(world);
    let Some(old) = world.get_resource::<HeroMount>() else {
        return vec![];
    };
    let old_riding = old.riding;
    let next = old.identity == key && old_riding && eligible(world, &parts(world));
    let state = world.resource_mut::<HeroMount>().into_inner();
    state.identity = key;
    state.riding = next;
    if old_riding == next {
        return vec![];
    }
    packet(world)
}
fn packet(world: &World) -> Vec<ServerPacket> {
    hero_entity(world)
        .and_then(|entity| world.get::<ObjectId>(entity))
        .map(|id| {
            vec![ServerPacket::MountUpdate {
                object_id: id.0,
                mount_type: parts(world).mount_type,
                riding_mount: riding(world),
            }]
        })
        .unwrap_or_default()
}
pub(in crate::runtime) fn sync_owner_toggle(world: &mut World) -> Vec<ServerPacket> {
    if hero_entity(world).is_none()
        || !world
            .resource::<Stage5SystemsResource>()
            .stage5_systems
            .hero
            .as_ref()
            .is_some_and(|hero| hero.spawned)
    {
        return vec![];
    }
    let mut packets = refresh(world);
    let desired = world.resource::<MountResource>().riding_mount;
    let key = identity(world);
    let capability = parts(world);
    if capability.mount_type < 0 || riding(world) == desired {
        return packets;
    }
    let next = desired && eligible(world, &capability);
    world.insert_resource(HeroMount {
        identity: key,
        riding: next,
    });
    packets.extend(packet(world));
    packets
}

#[cfg(test)]
mod tests {
    use super::super::super::items::{
        embedded_item_state_from_template, try_item_state_from_user_item,
    };
    use super::*;
    use mir2_protocol::{ClientPacket, MirClass, MirGender};
    fn scene() -> crate::SimulationSession {
        let mut s = crate::SimulationSession::new(crate::SimulationConfig::default());
        s.handle_packet(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into(),
        });
        s.handle_packet(ClientPacket::StartGame { character_index: 0 });
        s.handle_packet(ClientPacket::NewHero {
            name: "Aide".into(),
            gender: MirGender::Female,
            class: MirClass::Warrior,
        });
        s
    }
    fn mount(saddle: bool, bells: bool) -> super::super::super::items::ItemState {
        let base = mir2_game_data::crystal_item_by_index(764).unwrap();
        let mut item = embedded_item_state_from_template(&base, crate::ItemContainer::Bag1, 13);
        item.unique_id = 88001;
        let mut wire = try_user_item_from_item_state(&item).unwrap();
        wire.current_dura = wire.max_dura;
        wire.slots.resize(5, None);
        for (present, slot, index, uid) in [(saddle, 2, 782, 88002), (bells, 1, 778, 88003)] {
            if present {
                let template = mir2_game_data::crystal_item_by_index(index).unwrap();
                let mut child = embedded_item_state_from_template(
                    &template,
                    crate::ItemContainer::Bag1,
                    slot as u8,
                );
                child.unique_id = uid;
                wire.slots[slot] = Some(try_user_item_from_item_state(&child).unwrap());
            }
        }
        try_item_state_from_user_item(item, &wire).unwrap()
    }
    #[test]
    fn hero_mount_never_borrows_owner_saddle_or_bells_and_uses_exact_own_slots() {
        let mut s = scene();
        s.app
            .world_mut()
            .resource_mut::<MountResource>()
            .riding_mount = true;
        s.app.world_mut().resource_mut::<MountResource>().has_bells = true;
        s.app
            .world_mut()
            .resource_mut::<HeroInventoryResource>()
            .equipment = vec![mount(false, false)];
        sync_owner_toggle(s.app.world_mut());
        assert!(!riding(s.app.world()));
        s.app
            .world_mut()
            .resource_mut::<HeroInventoryResource>()
            .equipment = vec![mount(true, false)];
        let packets = sync_owner_toggle(s.app.world_mut());
        assert!(riding(s.app.world()));
        assert!(!can_attack(s.app.world()));
        assert!(packets.iter().any(|p| matches!(
            p,
            ServerPacket::MountUpdate {
                object_id: 1001,
                riding_mount: true,
                mount_type: 0
            }
        )));
        s.app
            .world_mut()
            .resource_mut::<HeroInventoryResource>()
            .equipment = vec![mount(true, true)];
        assert!(can_attack(s.app.world()));
        s.app
            .world_mut()
            .resource_mut::<HeroInventoryResource>()
            .equipment
            .clear();
        let packets = refresh(s.app.world_mut());
        assert!(!riding(s.app.world()));
        assert!(packets.iter().any(|p| matches!(
            p,
            ServerPacket::MountUpdate {
                riding_mount: false,
                ..
            }
        )));
    }
    #[test]
    fn hero_mount_switch_identity_and_relogin_reset_cannot_restore_previous_ride() {
        let mut s = scene();
        s.app
            .world_mut()
            .resource_mut::<MountResource>()
            .riding_mount = true;
        s.app
            .world_mut()
            .resource_mut::<HeroInventoryResource>()
            .equipment = vec![mount(true, true)];
        sync_owner_toggle(s.app.world_mut());
        assert!(riding(s.app.world()));
        s.app
            .world_mut()
            .resource_mut::<Stage5SystemsResource>()
            .stage5_systems
            .hero
            .as_mut()
            .unwrap()
            .name = "Other".into();
        assert!(!riding(s.app.world()));
        reset(s.app.world_mut());
        assert!(!riding(s.app.world()));
    }
    #[test]
    fn hero_mount_ordinary_ride_emits_distinct_owner_and_hero_updates() {
        use super::super::super::{equipment::equipment_state_from_item_state, resources::InventoryResource};
        let mut s=scene();
        let mut own=mount(true,true);own.unique_id=99001;
        let equipment=equipment_state_from_item_state(&own,crate::EquipmentSlot::Mount);
        s.app.world_mut().resource_mut::<InventoryResource>().equipment_items.push(equipment);
        s.app.world_mut().resource_mut::<MountResource>().has_saddle=true;
        s.app.world_mut().resource_mut::<HeroInventoryResource>().equipment=vec![mount(true,true)];
        for expected in [true,false] {
            let packets=s.handle_packet(ClientPacket::Chat{message:"@RIDE".into(),linked_items:vec![]});
            for id in [1000,1001] {
                assert!(packets.iter().any(|packet|matches!(packet,ServerPacket::MountUpdate{object_id,riding_mount,..} if *object_id==id&&*riding_mount==expected)),"{packets:?}");
            }
            assert_eq!(riding(s.app.world()),expected);
        }
    }
    #[test]
    fn hero_mount_ordinary_pet_modes_gate_attack_and_move_independently() {
        let mut s=scene();
        for mode in 0..=4 {
            s.handle_packet(ClientPacket::ChangePMode{mode});
            assert_eq!(super::super::hero_attack_mode_allowed(s.app.world()),matches!(mode,0|2|4));
            assert_eq!(super::super::hero_move_mode_allowed(s.app.world()),matches!(mode,0|1|4));
        }
    }

}

#[derive(Clone)]
pub(super) struct TransientSnapshot(Option<HeroMount>);
pub(super) fn capture_transient(world:&World)->TransientSnapshot {TransientSnapshot(world.get_resource::<HeroMount>().cloned())}
pub(super) fn restore_transient(world:&mut World,snapshot:&TransientSnapshot){world.remove_resource::<HeroMount>();if let Some(state)=&snapshot.0 {world.insert_resource(state.clone());}}
