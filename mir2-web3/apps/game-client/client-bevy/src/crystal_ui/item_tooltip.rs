//! Native adapter for the shared Crystal item-label formatter.

use crate::inventory::{CrystalItemTooltipSourceModel, ItemModel};
use crate::read_model::PlayerStats;
use mir2_client_core::{item_tooltip as formatter, item_tooltip_types as dto};
use web_time::{SystemTime, UNIX_EPOCH};
pub use mir2_client_core::item_tooltip::{CrystalItemTooltipSectionKind, CrystalItemTooltipColour, CrystalItemTooltipLine, CrystalItemTooltipSection, CrystalItemTooltipDocument, CrystalStackSplitHint, CrystalItemTooltipOptions};
const DOTNET_TICKS_PER_SECOND: i64 = 10_000_000;
const DOTNET_TICKS_AT_UNIX_EPOCH: i64 = 621_355_968_000_000_000;

pub fn crystal_item_tooltip_document(item: &ItemModel, player: &PlayerStats) -> CrystalItemTooltipDocument {
    crystal_item_tooltip_document_with_options(item, player, CrystalItemTooltipOptions::default())
}

pub fn crystal_item_tooltip_document_with_options(item: &ItemModel, player: &PlayerStats, options: CrystalItemTooltipOptions) -> CrystalItemTooltipDocument {
    let now_dotnet_ticks = dotnet_ticks_now_utc();
    formatter::crystal_item_tooltip_document_with_options(&convert_item(item), &convert_player(player), now_dotnet_ticks, options)
}

pub fn crystal_item_tooltip_document_from_source(name: &str, icon: u16, quantity: u32, source: Option<&CrystalItemTooltipSourceModel>, player: &PlayerStats) -> Option<CrystalItemTooltipDocument> {
    crystal_item_tooltip_document_from_source_with_options(name, icon, quantity, source, player, CrystalItemTooltipOptions::default())
}

pub fn crystal_item_tooltip_document_from_source_with_options(name: &str, icon: u16, quantity: u32, source: Option<&CrystalItemTooltipSourceModel>, player: &PlayerStats, options: CrystalItemTooltipOptions) -> Option<CrystalItemTooltipDocument> {
    let source = source?;
    let now_dotnet_ticks = dotnet_ticks_now_utc();
    formatter::crystal_item_tooltip_document_from_source_with_options(name, icon, quantity, Some(&convert_crystal_item_tooltip_source_model(source)), &convert_player(player), now_dotnet_ticks, options)
}

fn convert_item(value: &ItemModel) -> dto::ItemTooltipItem {
    dto::ItemTooltipItem {
        unique_id: value.unique_id,
        key: value.key.clone(),
        name: value.name.clone(),
        icon: value.icon,
        quantity: value.quantity,
        grade: value.grade.clone(),
        durability_current: value.durability_current,
        durability_max: value.durability_max,
        attack: value.attack,
        defence: value.defence,
        added_attack: value.added_attack,
        added_defence: value.added_defence,
        added_luck: value.added_luck,
        socket_slots: value.socket_slots,
        description: value.description.clone(),
        sell_value: value.sell_value,
        tooltip_source: value.tooltip_source.as_ref().map(convert_crystal_item_tooltip_source_model),
    }
}

fn convert_player(value: &PlayerStats) -> dto::ItemTooltipPlayer {
    dto::ItemTooltipPlayer {
        level: value.level,
        class_name: value.class_name.clone(),
        crystal_stats: value.crystal_stats.as_ref().map(|stats| stats.iter().map(|stat| dto::CrystalItemStatModel { stat: stat.stat, value: stat.value }).collect()),
    }
}

fn convert_crystal_item_stat_model(value: &crate::inventory::CrystalItemStatModel) -> dto::CrystalItemStatModel {
    dto::CrystalItemStatModel {
        stat: value.stat,
        value: value.value,
    }
}
fn convert_crystal_item_info_model(value: &crate::inventory::CrystalItemInfoModel) -> dto::CrystalItemInfoModel {
    dto::CrystalItemInfoModel {
        item_index: value.item_index,
        name: value.name.clone(),
        item_type: value.item_type,
        grade: value.grade.clone(),
        required_type: value.required_type,
        required_class: value.required_class,
        required_gender: value.required_gender,
        item_set: value.item_set,
        shape: value.shape,
        weight: value.weight,
        light: value.light,
        required_amount: value.required_amount,
        image: value.image,
        durability: value.durability,
        stack_size: value.stack_size,
        price: value.price,
        start_item: value.start_item,
        effect: value.effect,
        need_identify: value.need_identify,
        show_group_pickup: value.show_group_pickup,
        class_based: value.class_based,
        level_based: value.level_based,
        can_mine: value.can_mine,
        global_drop_notify: value.global_drop_notify,
        bind: value.bind,
        unique: value.unique,
        random_stats_id: value.random_stats_id,
        can_fast_run: value.can_fast_run,
        can_awakening: value.can_awakening,
        slots: value.slots,
        stats: value.stats.iter().map(convert_crystal_item_stat_model).collect(),
        tooltip: value.tooltip.clone(),
    }
}
fn convert_crystal_user_item_expire_model(value: &crate::inventory::CrystalUserItemExpireModel) -> dto::CrystalUserItemExpireModel {
    dto::CrystalUserItemExpireModel {
        expiry_binary_datetime: value.expiry_binary_datetime,
    }
}
fn convert_crystal_user_item_rental_model(value: &crate::inventory::CrystalUserItemRentalModel) -> dto::CrystalUserItemRentalModel {
    dto::CrystalUserItemRentalModel {
        owner_name: value.owner_name.clone(),
        binding_flags: value.binding_flags,
        expiry_binary_datetime: value.expiry_binary_datetime,
        rental_locked: value.rental_locked,
    }
}
fn convert_crystal_user_item_sealed_model(value: &crate::inventory::CrystalUserItemSealedModel) -> dto::CrystalUserItemSealedModel {
    dto::CrystalUserItemSealedModel {
        expiry_binary_datetime: value.expiry_binary_datetime,
        next_seal_binary_datetime: value.next_seal_binary_datetime,
    }
}
fn convert_crystal_user_item_model(value: &crate::inventory::CrystalUserItemModel) -> dto::CrystalUserItemModel {
    dto::CrystalUserItemModel {
        unique_id: value.unique_id,
        item_index: value.item_index,
        current_dura: value.current_dura,
        max_dura: value.max_dura,
        count: value.count,
        soul_bound_id: value.soul_bound_id,
        identified: value.identified,
        cursed: value.cursed,
        slots: value.slots.iter().map(|socket| socket.as_ref().map(convert_crystal_user_item_model)).collect(),
        gem_count: value.gem_count,
        added_stats: value.added_stats.iter().map(convert_crystal_item_stat_model).collect(),
        awake_type: value.awake_type,
        awake_values: value.awake_values.clone(),
        refined_value: value.refined_value,
        refine_added: value.refine_added,
        refine_success_chance: value.refine_success_chance,
        wedding_ring: value.wedding_ring,
        expire_info: value.expire_info.as_ref().map(convert_crystal_user_item_expire_model),
        rental_information: value.rental_information.as_ref().map(convert_crystal_user_item_rental_model),
        is_shop_item: value.is_shop_item,
        sealed_info: value.sealed_info.as_ref().map(convert_crystal_user_item_sealed_model),
        gm_made: value.gm_made,
    }
}
fn convert_crystal_item_tooltip_source_model(value: &crate::inventory::CrystalItemTooltipSourceModel) -> dto::CrystalItemTooltipSourceModel {
    dto::CrystalItemTooltipSourceModel {
        info: convert_crystal_item_info_model(&value.info),
        real_info: value.real_info.as_ref().map(convert_crystal_item_info_model),
        user_item: value.user_item.as_ref().map(convert_crystal_user_item_model),
        socket_infos: value.socket_infos.iter().map(|info| info.as_ref().map(convert_crystal_item_info_model)).collect(),
        real_socket_infos: value.real_socket_infos.iter().map(|info| info.as_ref().map(convert_crystal_item_info_model)).collect(),
    }
}

fn dotnet_ticks_now_utc() -> i64 {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    DOTNET_TICKS_AT_UNIX_EPOCH
        .saturating_add(
            i64::try_from(duration.as_secs())
                .unwrap_or(i64::MAX)
                .saturating_mul(DOTNET_TICKS_PER_SECOND),
        )
        .saturating_add(i64::from(duration.subsec_nanos() / 100))
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::*;
    fn crystal_item_tooltip_document_at(item: &ItemModel, player: &PlayerStats, now: i64) -> CrystalItemTooltipDocument {
        formatter::crystal_item_tooltip_document(&convert_item(item), &convert_player(player), now)
    }
    fn potion() -> ItemModel {
        ItemModel {
            unique_id: Some(42),
            key: "hp-drug-small".to_owned(),
            name: "Small HP Drug".to_owned(),
            quantity: 5,
            slot: 0,
            container: 0,
            icon: 398,
            sell_value: 20,
            tooltip_source: Some(CrystalItemTooltipSourceModel {
                info: CrystalItemInfoModel {
                    item_index: 658,
                    name: "(HP)DrugSmall".to_owned(),
                    item_type: 13,
                    grade: 0,
                    weight: 1,
                    stack_size: 20,
                    stats: vec![CrystalItemStatModel {
                        stat: 12,
                        value: 30,
                    }],
                    ..Default::default()
                },
                user_item: Some(CrystalUserItemModel {
                    unique_id: 42,
                    item_index: 658,
                    count: 5,
                    soul_bound_id: -1,
                    identified: true,
                    ..Default::default()
                }),
                socket_infos: Vec::new(),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn npc_gold_expiry_exact_json_max_value_does_not_wrap_to_expired() {
        let now = DOTNET_TICKS_AT_UNIX_EPOCH;
        for ticks in [3_155_378_975_999_999_999_i64, 7_767_064_994_427_387_903] {
            let encoded = format!("{{\"expiry_binary_datetime\":\"{ticks}\"}}");
            let expiry: CrystalUserItemExpireModel = serde_json::from_str(&encoded).unwrap();
            let mut item = potion();
            item.tooltip_source.as_mut().unwrap().user_item.as_mut().unwrap().expire_info = Some(expiry);
            let text = crystal_item_tooltip_document_at(&item,&PlayerStats::default(),now).plain_text();
            assert!(text.contains("Expires in"));
            assert!(!text.contains("Expired"), "exact MaxTicks must not enter Local wrap");
        }
    }

    #[test]
    fn conversion_preserves_independent_fields_wide_identity_dates_and_holes() {
        let mut item=potion();
        item.unique_id=Some(u64::MAX);
        item.icon=u16::MAX;
        item.quantity=u32::MAX;
        item.grade=Some("rare".into());
        item.description="independent story".into();
        item.attack=-10; item.defence=20; item.added_attack=3; item.added_defence=-4; item.added_luck=5; item.socket_slots=2;
        item.durability_current=Some(0); item.durability_max=Some(u16::MAX);
        let source=item.tooltip_source.as_mut().unwrap();
        source.info.item_index=i32::MIN;
        let user=source.user_item.as_mut().unwrap();
        user.unique_id=u64::MAX;
        user.item_index=i32::MIN;
        user.expire_info=Some(CrystalUserItemExpireModel { expiry_binary_datetime:i64::MIN });
        user.slots=vec![None,Some(CrystalUserItemModel { unique_id:1,count:1,..Default::default() })];
        source.socket_infos=vec![None,Some(CrystalItemInfoModel { name:"Ruby".into(),..Default::default() })];
        source.real_socket_infos=vec![None,None];
        let converted=convert_item(&item);
        assert_eq!(converted.unique_id,Some(u64::MAX));
        assert_eq!(converted.key,item.key);
        assert_eq!(converted.name,item.name);
        assert_eq!(converted.icon,u16::MAX);
        assert_eq!(converted.quantity,u32::MAX);
        assert_eq!(converted.grade,item.grade);
        assert_eq!(converted.description,item.description);
        assert_eq!((converted.attack,converted.defence,converted.added_attack,converted.added_defence,converted.added_luck,converted.socket_slots),(-10,20,3,-4,5,2));
        assert_eq!((converted.durability_current,converted.durability_max),(Some(0),Some(u16::MAX)));
        assert_eq!(converted.sell_value,item.sell_value);
        let source=converted.tooltip_source.as_ref().unwrap();
        assert_eq!(source.info.item_index,i32::MIN);
        assert_eq!(source.user_item.as_ref().unwrap().item_index,i32::MIN);
        assert_eq!(source.user_item.as_ref().unwrap().expire_info.unwrap().expiry_binary_datetime,i64::MIN);
        assert!(source.user_item.as_ref().unwrap().slots[0].is_none());
        assert!(source.socket_infos[0].is_none());
        assert_eq!(source.socket_infos[1].as_ref().unwrap().name,"Ruby");
        assert_eq!(source.real_socket_infos,vec![None,None]);
        let viewer=convert_player(&PlayerStats::default());
        assert_eq!(viewer.crystal_stats,None);
        assert_eq!(viewer.class_name,None);
    }

    #[test]
    fn native_four_public_entry_points_delegate_and_keep_options() {
        let item=potion();
        let player=PlayerStats::default();
        let core=formatter::crystal_item_tooltip_document(&convert_item(&item),&convert_player(&player),0);
        assert_eq!(crystal_item_tooltip_document(&item,&player),core);
        let options=CrystalItemTooltipOptions { hide_added_stats:true,stack_split_hint:CrystalStackSplitHint::MoreActions };
        assert_eq!(crystal_item_tooltip_document_with_options(&item,&player,options),formatter::crystal_item_tooltip_document_with_options(&convert_item(&item),&convert_player(&player),0,options));
        let source=item.tooltip_source.as_ref();
        assert_eq!(crystal_item_tooltip_document_from_source(&item.name,item.icon,item.quantity,source,&player),formatter::crystal_item_tooltip_document_from_source(&item.name,item.icon,item.quantity,source.map(convert_crystal_item_tooltip_source_model).as_ref(),&convert_player(&player),0));
        assert_eq!(crystal_item_tooltip_document_from_source_with_options(&item.name,item.icon,item.quantity,source,&player,options),formatter::crystal_item_tooltip_document_from_source_with_options(&item.name,item.icon,item.quantity,source.map(convert_crystal_item_tooltip_source_model).as_ref(),&convert_player(&player),0,options));
        assert_eq!(crystal_item_tooltip_document_from_source("unknown",0,1,None,&player),None);
    }

}
