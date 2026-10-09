//! Shared Crystal template join for the actual packet and full owner snapshot.
use crate::{crystal_item_by_index, crystal_user_item_image};
use mir2_protocol::UserItem;
use serde_json::{json, Value};

pub fn crystal_npc_goods_item_json(item: &UserItem, rate: f32) -> Value {
    let mut entry = serde_json::to_value(item)
        .ok()
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    entry.insert("id".into(), json!(item.unique_id));
    entry.insert("uniqueId".into(), json!(item.unique_id));
    // Exact goods selector for the durable protocol, before any JS Number conversion.
    entry.insert("purchaseItemIndex".into(), json!(item.unique_id.to_string()));
    entry.insert("itemIndex".into(), json!(item.item_index));
    entry.insert("count".into(), json!(item.count));

    if let Some(template) = crystal_item_by_index(item.item_index) {
        entry.insert("name".into(), json!(template.name));
        entry.insert(
            "icon".into(),
            json!(crystal_user_item_image(
                template.item_type,
                template.shape,
                template.stack_size,
                template.image,
                u32::from(item.count),
            )),
        );
        entry.insert(
            "price".into(),
            json!(((template.price as f32) * rate).floor() as u32),
        );
        // The displayed unit price cannot reconstruct a multi-item f32 total.
        let socket_infos: Vec<_> = item.slots.iter().map(|slot| {
            slot.as_ref().and_then(|socket| crystal_item_by_index(socket.item_index))
        }).collect();
        entry.insert("tooltipSource".into(), json!({
            "info": &template, "realInfo": null, "userItem": item,
            "socketInfos": socket_infos, "realSocketInfos": [],
        }));
        entry.insert("grade".into(), json!(template.grade));
        if let Some(description) = template.tooltip.filter(|value| !value.trim().is_empty()) {
            entry.insert("description".into(), json!(description));
        }
    } else {
        entry.insert("name".into(), json!(format!("Item #{}", item.item_index)));
        entry.insert("icon".into(), json!(0));
        entry.insert("price".into(), json!(0));
    }
    Value::Object(entry)
}

pub fn crystal_npc_goods_list_json(list: &[UserItem], rate: f32) -> Value {
    Value::Array(
        list.iter()
            .map(|item| crystal_npc_goods_item_json(item, rate))
            .collect(),
    )
}


#[cfg(test)]
mod tests {
    use super::*;
    fn item(id: u64, index: i32) -> UserItem {
        UserItem { unique_id:id, item_index:index, current_dura:100, max_dura:100, count:3,
            soul_bound_id:0, identified:true, cursed:false, slots:vec![], gem_count:0,
            added_stats:vec![], awake_type:0, awake_values:vec![], refined_value:0,
            refine_added:0, refine_success_chance:0, wedding_ring:0, expire_info:None,
            rental_information:None, is_shop_item:false, sealed_info:None, gm_made:false }
    }
    #[test]
    fn npc_goods_snapshot_projection_keeps_max_identity_and_template_facts() {
        let mut item = item(u64::MAX,658);
        item.slots = vec![None,Some(self::item(u64::MAX-1,659))];
        let template = crystal_item_by_index(658).unwrap();
        let row = crystal_npc_goods_item_json(&item,1.25);
        let raw = serde_json::to_value(&item).unwrap();
        for (key,value) in raw.as_object().unwrap() { assert_eq!(&row[key],value); }
        assert_eq!(row["id"].as_u64(),Some(u64::MAX));
        assert_eq!(row["purchaseItemIndex"].as_str(),Some("18446744073709551615"));
        assert_eq!(row["tooltipSource"]["userItem"],raw);
        assert_eq!(row["tooltipSource"]["info"],serde_json::to_value(&template).unwrap());
        assert_eq!(row["tooltipSource"]["socketInfos"][0],Value::Null);
        assert_eq!(row["tooltipSource"]["socketInfos"][1],serde_json::to_value(crystal_item_by_index(659).unwrap()).unwrap());
        assert_eq!(row["name"],json!(template.name));
        assert_eq!(row["icon"],json!(crystal_user_item_image(template.item_type,template.shape,template.stack_size,template.image,3)));
        assert_eq!(row["price"],json!(((template.price as f32)*1.25_f32).floor() as u32));
    }
    #[test]
    fn npc_goods_snapshot_projection_list_and_unknown_template_preserve_wire() {
        let list = vec![item(0,658),item(u64::MAX,i32::MAX)];
        let rows = crystal_npc_goods_list_json(&list,1.0);
        assert_eq!(rows.as_array().unwrap().len(),2);
        for (source,row) in list.iter().zip(rows.as_array().unwrap()) {
            assert_eq!(row,&crystal_npc_goods_item_json(source,1.0));
            assert_eq!(row["unique_id"].as_u64(),Some(source.unique_id));
            assert_eq!(row["purchaseItemIndex"].as_str(),Some(source.unique_id.to_string().as_str()));
        }
        assert!(rows[1].get("tooltipSource").is_none());
        assert_eq!(rows[1]["name"],json!(format!("Item #{}",i32::MAX)));
        assert_eq!(rows[1]["price"],json!(0));
    }
}
