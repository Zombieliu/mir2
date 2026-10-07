//! Fresh owner checkpoint projection for the indivisible native economic turn.
//! No packet cursor, retained UI snapshot, or previous model is an input.

use serde_json::Value;
use mir2_bevy_runtime::npc_purchase_economy::{NativeNpcEconomyProjection, owner_hero_model, owner_social_model, owner_mail_model, owner_shop_catalog};

pub(crate) fn project_owner(owner: &Value) -> Result<NativeNpcEconomyProjection,String> {
    let mut cursor=crate::gateway::NativeUiPlayerCursor::default();
    cursor.observe_world_snapshot(owner);
    let _=crate::gateway::try_transform_mail_model_from_snapshot(owner).ok_or("incomplete owner mail")?;
    let mail=owner_mail_model(owner).map_err(|e|format!("invalid owner mail: {e:?}"))?;
    let storage=crate::gateway::try_transform_storage_model_from_snapshot(owner).ok_or("incomplete owner storage")?;
    let shop=project_shop(owner)?;
    let projection=NativeNpcEconomyProjection {
        owner_json:owner.to_string(),
        world_json:crate::gateway::transform_world_snapshot(owner).to_string(),
        ui_json:cursor.to_read_model_json().to_string(),
        inventory_json:crate::gateway::transform_inventory_model(owner).to_string(),
        mail_json:serde_json::to_string(&mail).map_err(|e|e.to_string())?,storage_json:storage.to_string(),
        shop_json:serde_json::to_string(&shop).map_err(|e|e.to_string())?,
        skill_json:crate::gateway::transform_skill_model(owner).to_string(),
        hero_json:serde_json::to_string(&project_hero(owner)?).map_err(|e|e.to_string())?,
        social_json:serde_json::to_string(&owner_social_model(owner).map_err(|e|format!("invalid owner Social: {e:?}"))?).map_err(|e|e.to_string())?,
    };
    projection.validate_source().map_err(|e|format!("incomplete owner projection: {e:?}"))?;
    Ok(projection)
}

/// Fresh actual owner Hero fields, before any packet cursor overlay.
pub(crate) fn project_hero(owner:&Value)->Result<mir2_client_bevy::hero_model::HeroModel,String> {
    owner_hero_model(owner).map_err(|e|format!("invalid owner Hero: {e:?}"))
}

/// Ordinary owner updates use the same fresh catalogue source and mapping.
pub(crate) fn project_shop(owner: &Value) -> Result<mir2_client_bevy::shop::ShopModel,String> {
    let mut cursor=crate::gateway::NativeUiPlayerCursor::default();
    cursor.observe_world_snapshot(owner);
    let mut shop=owner_shop_catalog(owner).map_err(|e|format!("invalid owner shop: {e:?}"))?;
    if let Some(source)=owner.get("nativeNpcShop").filter(|source|!source.is_null()) {
        let packet=source["packetType"].as_str().ok_or("missing owner catalog packet")?;
        let mapped=crate::gateway::transform_npc_catalog_packet(packet,source,&mut cursor).ok_or("invalid owner catalog")?;
        let mapped:mir2_client_bevy::shop::ShopModel=serde_json::from_value(mapped).map_err(|e|e.to_string())?;
        if mapped.goods.len()!=shop.goods.len() {return Err("incomplete owner catalog".into());}
        for (target,mapped) in shop.goods.iter_mut().zip(&mapped.goods) {
            if target.unique_id!=mapped.unique_id {return Err("owner catalog identity mismatch".into());}
            target.icon_width=mapped.icon_width;target.icon_height=mapped.icon_height;
        }
    }
    Ok(shop)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    /// Ordinary complete public WorldSnapshot shape, including intentional
    /// nullable fields. No private save/journal or receipt factory is used.
    pub(crate) fn owner() -> Value {
        serde_json::from_str(r#"{
            "tick":7,"mapTitle":"Bichon","mapFileName":"0","inSafeZone":false,"lightSetting":0,
            "playerObjectId":3,"playerHp":10,"playerMaxHp":20,"playerMp":5,"playerMaxMp":10,"playerCrystalStats":[],"playerPkPoints":0,
            "playerExperience":50,"playerMaxExperience":100,"gold":90,"credit":20,"cityCurrencies":{"bichon":0,"feitian":0},
            "currentWeight":0,"playerWeights":null,"maxWeight":20,"freeBagSlots":40,"maxBagSlots":40,"inventoryCapacity":46,"npcGoldTradeCapacity":null,
            "storageSize":80,"hasExpandedStorage":false,"hasStoragePassword":false,"requireStoragePassword":false,
            "storagePasswordLastSetBinaryDatetime":0,"expandedStorageExpiryTimeBinaryDatetime":0,"sceneView":null,"terrainPatches":[],"decorObjects":[],
            "entities":[{"objectId":3,"kind":"selfPlayer","name":"Alice","ownerName":null,"ai":null,"x":4,"y":5,"direction":"Down",
                "class":"Warrior","gender":"Male","level":8,"hp":10,"maxHp":20,"light":0,"nameColourArgb":0,"dead":false,
                "disposition":"friendly","sprite":null,"questIds":[]}],"groundDrops":[],"beltItems":[],"inventoryItems":[],"equipmentItems":[],"storageItems":[],
            "heroInventoryItems":[],"heroEquipmentItems":[],"heroInventoryCapacity":10,"heroStats":[],"heroVitals":null,"heroWeights":{"bag":0,"wear":0,"hand":0},
            "heroMaxExperience":null,"nativeNpcShop":null,"questLog":[],"activeNpcDialog":null,"npcScriptDiagnostics":[],"knownSkills":[],"activeBuffs":[],
            "stage5Systems":{"group":{"allowGroup":true,"members":[],"lootMode":"free"},
                "guild":{"name":"","members":[],"rank":"","permissions":[],"chatLog":[],"knownGuilds":[],"activeWars":[],"activeWarTicksRemaining":{},
                    "alliedGuilds":[],"allyCount":0,"allianceBroadcasts":[],"warBroadcasts":[],"notice":[],"storageGold":0,"storageItems":{},"storageItemStates":{},"storageItemUsers":{}},
                "social":{"friends":[],"blocked":[],"memos":{}},
                "relationship":{"allowLoverRecall":false,"cooldownUntilMs":0,"allowMarriage":false,"partnerName":"","marriedDateBinaryDatetime":0,
                    "mapName":"","marriedDays":0,"pendingRequestFrom":null,"pendingDivorceFrom":null},
                "mentor":{"isMentor":false,"cooldownUntilMs":0,"allowMentor":false,"name":"","level":0,"online":false,"menteeExp":0,"pendingRequestFrom":null,"pendingRequestLevel":0},
                "mail":[],"gameShopIndividualPurchases":{},"economyProjectionEventIds":[],"trade":null,"auction":[],
                "refine":{"ovenItemStateJson":null,"remainingMs":0,"clockEpoch":null,"collectDeadlineMs":null,"itemStates":{},"slots":{},"currentItem":null,
                    "refining":false,"ready":false,"pendingUniqueId":0,"pendingChance":0,"pendingStat":0},
                "conquest":{"castleOwner":"","activeWars":[],"eventLog":[],"taxRatePercent":0,"gold":0,"guards":[],"walls":[],"gates":[],"openGates":[]},
                "guildTerritory":{"owned":false,"mapFileName":"GA0","owner":"","leader":"","leader2":"","price":0,"rentalDaysLeft":0,"begin":0,"recallLog":[]},
                "hero":null,"heroLearnedMagics":[],"profession":{"miningLevel":0,"ore":0,"craftedItems":[]},"appearance":{"hair":0},"nameLists":[],
                "intelligentCreatures":[],"summonedIntelligentCreatureType":99,"intelligentCreaturePearls":12,
                "itemRental":{"partnerName":null,"fee":0,"days":0,"hasDepositedItem":false,"depositedItemName":null,"goldLocked":false,"itemLocked":false,"recordCount":0,"rentedItems":[]},
                "attackMode":0,"petMode":0,"pkDecayElapsedTicks":0},"mapTransfers":[],"interactionHints":[]
        }"#).unwrap()
    }
    #[test]
    fn native_npc_projection_complete_owner_is_lossless_and_uses_fresh_models() {
        let owner=owner();let projection=project_owner(&owner).unwrap();
        assert_eq!(serde_json::from_str::<Value>(&projection.owner_json).unwrap(),owner);
        projection.validate_source().unwrap();
        let ui:mir2_client_bevy::read_model::UiReadModel=serde_json::from_str(&projection.ui_json).unwrap();
        assert_eq!(ui.player.name.as_deref(),Some("Alice"));assert_eq!((ui.player.experience,ui.player.max_experience),(50,100));
        let shop:mir2_client_bevy::shop::ShopModel=serde_json::from_str(&projection.shop_json).unwrap();assert!(shop.goods.is_empty());assert!(!shop.supports_buy);
    }
    #[test]
    fn native_npc_projection_missing_economic_source_cannot_default_or_overlay() {
        let original=owner();
        for key in ["gold","inventoryItems","heroEquipmentItems","cityCurrencies","heroMaxExperience","nativeNpcShop"] {
            let mut partial=original.clone();partial.as_object_mut().unwrap().remove(key);
            assert!(project_owner(&partial).is_err(),"{key}");
        }
        let mut partial=original;partial["stage5Systems"]["mail"]=serde_json::json!([{"id":3,"gold":7,"items":[]}]);
        assert!(project_owner(&partial).is_err());
    }
    #[test]
    fn native_npc_projection_actual_raw_finite_catalogue_keeps_uid_and_source_stock() {
        let mut owner=owner();
        let item=mir2_client_bevy::inventory::CrystalUserItemModel{unique_id:u64::MAX,item_index:658,count:3,is_shop_item:false,..Default::default()};
        owner["nativeNpcShop"]=serde_json::json!({"npcObjectId":4990,"scriptKey":"BichonProvince/NaturalCave/WickedTrader","service":"BUYBACK",
            "packetType":"NPCGoods","list":[item],"rate":1.337_f32,"panelType":0,"hideAddedStats":false});
        let projection=project_owner(&owner).unwrap();let shop:mir2_client_bevy::shop::ShopModel=serde_json::from_str(&projection.shop_json).unwrap();
        assert_eq!(shop.goods[0].unique_id,u64::MAX);assert_eq!(shop.goods[0].stock,3);assert!(shop.supports_buy);
        assert_eq!(shop.goods[0].name,mir2_game_data::crystal_item_by_index(658).unwrap().name);assert!(shop.goods[0].price>0);
    }
    #[test]
    fn native_npc_projection_hero_panel_epoch_survives_receipt_and_old_packet_models() {
        use mir2_bevy_runtime::npc_purchase_economy::{NativeHeroOwnerEpoch,NativeHeroOwnerGate,NativeNpcEconomyGate,
            apply_native_hero_owner_update,apply_native_hero_packet_model,apply_native_npc_economy_bundle};
        use mir2_client_bevy::{hero_model::HeroModel,crystal_ui::overlays::hero_dialog::{HeroDialogModel,HeroPage}};
        use mir2_client_core::{npc_purchase_host::NpcPurchaseReceiptHost,npc_purchase_receipt::{ActorKey,OwnerScope,SnapshotWitness}};
        let mut owner=owner();
        owner["stage5Systems"]["hero"]=serde_json::json!({"name":"Hero","class":"Warrior","gender":"Male","level":2,
            "experience":77,"behaviour":0,"spawned":true,"autoPot":false,"autoHpPercent":30,"autoMpPercent":30,"hpItemIndex":0,"mpItemIndex":0});
        owner["heroMaxExperience"]=200.into();
        let packet=mir2_protocol::HeroUserInformation{object_id:12,name:"Hero".into(),class:mir2_protocol::MirClass::Warrior,
            gender:mir2_protocol::MirGender::Male,level:2,hair:0,hp:10,mp:5,experience:1,max_experience:100,
            inventory:Some(vec![None;10]),equipment:Some(vec![None;14]),magics:vec![],auto_pot:false,
            auto_hp_percent:30,auto_mp_percent:30,hp_item_index:0,mp_item_index:0};
        let mut packet_model=HeroModel::default();assert!(packet_model.apply_packet("HeroInformation",&serde_json::json!({"info":packet})));
        packet_model.session_epoch=41;packet_model.revision=5;packet_model.inventory_view.capacity=10;
        let generation=packet_model.hero_generation;
        let mut world=bevy::prelude::World::new();world.insert_resource(packet_model.clone());
        let display_gate=NativeHeroOwnerGate::new(NativeHeroOwnerEpoch{run:1,connection:1,procedure:1,owner_epoch:1,scene_epoch:1,cancellation:1,actor:3,map:0}).unwrap();
        assert!(apply_native_hero_owner_update(&mut world,display_gate.prepare(&owner).unwrap()));
        let mut panel=HeroDialogModel::default();panel.observe(world.resource::<HeroModel>());panel.toggle_page(HeroPage::Status);
        assert!(panel.character_open);
        let mut host=NpcPurchaseReceiptHost::new().unwrap();let connection=host.open_connection().unwrap();let ticket=host.request_begin(connection).unwrap();
        let binding=host.bind(ticket,ActorKey::from_server_bytes([1;32]).unwrap(),OwnerScope::from_server_bytes([2;32]).unwrap()).unwrap();
        let gate=NativeNpcEconomyGate::new(binding);let witness=SnapshotWitness{actor:binding.actor(),producer_scope:binding.producer_scope(),server_revision:7,complete:true};
        owner["stage5Systems"]["hero"]["experience"]=88.into();
        let projection=project_owner(&owner).unwrap();
        assert_eq!(serde_json::from_str::<HeroModel>(&projection.hero_json).unwrap().session_epoch,0);
        let bundle=gate.prepare(witness,projection).unwrap().with_hero_owner_gate(&display_gate).unwrap();
        assert!(apply_native_npc_economy_bundle(&mut world,bundle));
        let model=world.resource::<HeroModel>();assert_eq!((model.session_epoch,model.hero_generation),(41,generation));
        assert!(model.info.is_none());assert_eq!(model.snapshot_read_model().unwrap().player.experience,88);
        panel.observe(model);assert!(panel.character_open);assert!(panel.info.is_none());
        let mut old=packet_model;old.session_epoch=2;old.hero_generation=0;old.revision=1;
        apply_native_hero_packet_model(&mut world,old);
        let model=world.resource::<HeroModel>();assert_eq!((model.session_epoch,model.hero_generation),(41,generation));
        assert_eq!(model.snapshot_read_model().unwrap().player.experience,88);panel.observe(model);assert!(panel.character_open);
        owner["stage5Systems"]["hero"]["experience"]=99.into();
        assert!(apply_native_hero_owner_update(&mut world,display_gate.prepare(&owner).unwrap()));
        panel.observe(world.resource::<HeroModel>());assert!(panel.character_open);
        assert_eq!(world.resource::<HeroModel>().snapshot_read_model().unwrap().player.experience,99);
        assert_eq!(gate.try_recv_applied().unwrap().witness(),witness);assert!(gate.try_recv_applied().is_none());
    }
}
