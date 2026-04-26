use crate::error::{PacketCodecError, Result};
use crate::frame::{decode_frame, encode_frame};
use crate::ids::{ClientPacketId, ServerPacketId};
use crate::io::{PacketReader, PacketWriter};
use crate::types::{
    ChatType, ItemInfo, MapInformation, MirClass, MirDirection, MirGender, MirGridType,
    MonsterInfo, NpcInfo, ObjectAttackInfo, ObjectDiedInfo, ObjectEffectInfo, ObjectGoldInfo,
    ObjectHealthInfo, ObjectItemInfo, ObjectMovement, ObjectPlayerInfo, ObjectRangeAttackInfo,
    ObjectRevivedInfo, ObjectSpellInfo, ObjectStruckInfo, Point, SelectInfo, Spell, StruckInfo,
    UserInformation, UserItem, UserLocation,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientPacket {
    ClientVersion {
        version_hash: Vec<u8>,
    },
    Disconnect,
    KeepAlive {
        time: i64,
    },
    NewAccount {
        account_id: String,
        password: String,
        birth_date_binary: i64,
        user_name: String,
        secret_question: String,
        secret_answer: String,
        email_address: String,
    },
    ChangePassword {
        account_id: String,
        current_password: String,
        new_password: String,
    },
    Login {
        account_id: String,
        password: String,
    },
    NewCharacter {
        name: String,
        gender: MirGender,
        class: MirClass,
    },
    DeleteCharacter {
        character_index: i32,
    },
    StartGame {
        character_index: i32,
    },
    LogOut,
    Turn {
        direction: MirDirection,
    },
    Walk {
        direction: MirDirection,
    },
    Run {
        direction: MirDirection,
    },
    Chat {
        message: String,
    },
    MoveItem {
        grid: MirGridType,
        from: i32,
        to: i32,
    },
    StoreItem {
        from: i32,
        to: i32,
    },
    TakeBackItem {
        from: i32,
        to: i32,
    },
    MergeItem {
        grid_from: MirGridType,
        grid_to: MirGridType,
        id_from: u64,
        id_to: u64,
    },
    EquipItem {
        grid: MirGridType,
        unique_id: u64,
        to: i32,
    },
    RemoveItem {
        grid: MirGridType,
        unique_id: u64,
        to: i32,
    },
    RemoveSlotItem {
        grid: MirGridType,
        grid_to: MirGridType,
        unique_id: u64,
        to: i32,
        from_unique_id: u64,
    },
    SplitItem {
        grid: MirGridType,
        unique_id: u64,
        count: u16,
    },
    UseItem {
        unique_id: u64,
        grid: MirGridType,
    },
    DropItem {
        unique_id: u64,
        count: u16,
        hero_inventory: bool,
    },
    DeleteItem {
        unique_id: u64,
        count: u16,
        hero_inventory: bool,
    },
    DropGold {
        amount: u32,
    },
    PickUp,
    RequestItemInfo {
        item_index: i32,
    },
    Attack {
        direction: MirDirection,
        spell: Spell,
    },
    RangeAttack {
        direction: MirDirection,
        location: Point,
        target_id: u32,
        target_location: Point,
    },
    Harvest {
        direction: MirDirection,
    },
    BuyItem {
        item_index: u64,
        count: u16,
        panel_type: u8,
    },
    SellItem {
        unique_id: u64,
        count: u16,
    },
    RepairItem {
        unique_id: u64,
    },
    SRepairItem {
        unique_id: u64,
    },
    CombineItem {
        grid: MirGridType,
        id_from: u64,
        id_to: u64,
    },
    UnlockStorage {
        password: String,
    },
    SetStoragePassword {
        current_password: String,
        new_password: String,
    },
    RemoveStoragePassword {
        current_password: String,
    },
}

impl ClientPacket {
    pub fn packet_id(&self) -> ClientPacketId {
        match self {
            Self::ClientVersion { .. } => ClientPacketId::ClientVersion,
            Self::Disconnect => ClientPacketId::Disconnect,
            Self::KeepAlive { .. } => ClientPacketId::KeepAlive,
            Self::NewAccount { .. } => ClientPacketId::NewAccount,
            Self::ChangePassword { .. } => ClientPacketId::ChangePassword,
            Self::Login { .. } => ClientPacketId::Login,
            Self::NewCharacter { .. } => ClientPacketId::NewCharacter,
            Self::DeleteCharacter { .. } => ClientPacketId::DeleteCharacter,
            Self::StartGame { .. } => ClientPacketId::StartGame,
            Self::LogOut => ClientPacketId::LogOut,
            Self::Turn { .. } => ClientPacketId::Turn,
            Self::Walk { .. } => ClientPacketId::Walk,
            Self::Run { .. } => ClientPacketId::Run,
            Self::Chat { .. } => ClientPacketId::Chat,
            Self::MoveItem { .. } => ClientPacketId::MoveItem,
            Self::StoreItem { .. } => ClientPacketId::StoreItem,
            Self::TakeBackItem { .. } => ClientPacketId::TakeBackItem,
            Self::MergeItem { .. } => ClientPacketId::MergeItem,
            Self::EquipItem { .. } => ClientPacketId::EquipItem,
            Self::RemoveItem { .. } => ClientPacketId::RemoveItem,
            Self::RemoveSlotItem { .. } => ClientPacketId::RemoveSlotItem,
            Self::SplitItem { .. } => ClientPacketId::SplitItem,
            Self::UseItem { .. } => ClientPacketId::UseItem,
            Self::DropItem { .. } => ClientPacketId::DropItem,
            Self::DeleteItem { .. } => ClientPacketId::DeleteItem,
            Self::DropGold { .. } => ClientPacketId::DropGold,
            Self::PickUp => ClientPacketId::PickUp,
            Self::RequestItemInfo { .. } => ClientPacketId::RequestItemInfo,
            Self::Attack { .. } => ClientPacketId::Attack,
            Self::RangeAttack { .. } => ClientPacketId::RangeAttack,
            Self::Harvest { .. } => ClientPacketId::Harvest,
            Self::BuyItem { .. } => ClientPacketId::BuyItem,
            Self::SellItem { .. } => ClientPacketId::SellItem,
            Self::RepairItem { .. } => ClientPacketId::RepairItem,
            Self::SRepairItem { .. } => ClientPacketId::SRepairItem,
            Self::CombineItem { .. } => ClientPacketId::CombineItem,
            Self::UnlockStorage { .. } => ClientPacketId::UnlockStorage,
            Self::SetStoragePassword { .. } => ClientPacketId::SetStoragePassword,
            Self::RemoveStoragePassword { .. } => ClientPacketId::RemoveStoragePassword,
        }
    }

    fn encode_payload(&self, writer: &mut PacketWriter) -> Result<()> {
        match self {
            Self::ClientVersion { version_hash } => {
                writer.write_i32(version_hash.len() as i32);
                writer.write_bytes(version_hash);
            }
            Self::Disconnect => {}
            Self::KeepAlive { time } => writer.write_i64(*time),
            Self::NewAccount {
                account_id,
                password,
                birth_date_binary,
                user_name,
                secret_question,
                secret_answer,
                email_address,
            } => {
                writer.write_string(account_id)?;
                writer.write_string(password)?;
                writer.write_i64(*birth_date_binary);
                writer.write_string(user_name)?;
                writer.write_string(secret_question)?;
                writer.write_string(secret_answer)?;
                writer.write_string(email_address)?;
            }
            Self::ChangePassword {
                account_id,
                current_password,
                new_password,
            } => {
                writer.write_string(account_id)?;
                writer.write_string(current_password)?;
                writer.write_string(new_password)?;
            }
            Self::Login {
                account_id,
                password,
            } => {
                writer.write_string(account_id)?;
                writer.write_string(password)?;
            }
            Self::NewCharacter {
                name,
                gender,
                class,
            } => {
                writer.write_string(name)?;
                writer.write_u8(*gender as u8);
                writer.write_u8(*class as u8);
            }
            Self::DeleteCharacter { character_index } => writer.write_i32(*character_index),
            Self::StartGame { character_index } => writer.write_i32(*character_index),
            Self::LogOut => {}
            Self::Turn { direction } | Self::Walk { direction } | Self::Run { direction } => {
                writer.write_u8(*direction as u8)
            }
            Self::Chat { message } => {
                writer.write_string(message)?;
                writer.write_i32(0);
            }
            Self::MoveItem { grid, from, to } => {
                writer.write_u8(*grid as u8);
                writer.write_i32(*from);
                writer.write_i32(*to);
            }
            Self::StoreItem { from, to } | Self::TakeBackItem { from, to } => {
                writer.write_i32(*from);
                writer.write_i32(*to);
            }
            Self::CombineItem {
                grid,
                id_from,
                id_to,
            } => {
                writer.write_u8(*grid as u8);
                writer.write_u64(*id_from);
                writer.write_u64(*id_to);
            }
            Self::MergeItem {
                grid_from,
                grid_to,
                id_from,
                id_to,
            } => {
                writer.write_u8(*grid_from as u8);
                writer.write_u8(*grid_to as u8);
                writer.write_u64(*id_from);
                writer.write_u64(*id_to);
            }
            Self::EquipItem {
                grid,
                unique_id,
                to,
            }
            | Self::RemoveItem {
                grid,
                unique_id,
                to,
            } => {
                writer.write_u8(*grid as u8);
                writer.write_u64(*unique_id);
                writer.write_i32(*to);
            }
            Self::RemoveSlotItem {
                grid,
                grid_to,
                unique_id,
                to,
                from_unique_id,
            } => {
                writer.write_u8(*grid as u8);
                writer.write_u8(*grid_to as u8);
                writer.write_u64(*unique_id);
                writer.write_i32(*to);
                writer.write_u64(*from_unique_id);
            }
            Self::SplitItem {
                grid,
                unique_id,
                count,
            } => {
                writer.write_u8(*grid as u8);
                writer.write_u64(*unique_id);
                writer.write_u16(*count);
            }
            Self::UseItem { unique_id, grid } => {
                writer.write_u64(*unique_id);
                writer.write_u8(*grid as u8);
            }
            Self::DropItem {
                unique_id,
                count,
                hero_inventory,
            }
            | Self::DeleteItem {
                unique_id,
                count,
                hero_inventory,
            } => {
                writer.write_u64(*unique_id);
                writer.write_u16(*count);
                writer.write_bool(*hero_inventory);
            }
            Self::DropGold { amount } => writer.write_u32(*amount),
            Self::PickUp => {}
            Self::RequestItemInfo { item_index } => writer.write_i32(*item_index),
            Self::Attack { direction, spell } => {
                writer.write_u8(*direction as u8);
                writer.write_u8(*spell as u8);
            }
            Self::RangeAttack {
                direction,
                location,
                target_id,
                target_location,
            } => {
                writer.write_u8(*direction as u8);
                location.encode(writer);
                writer.write_u32(*target_id);
                target_location.encode(writer);
            }
            Self::Harvest { direction } => writer.write_u8(*direction as u8),
            Self::BuyItem {
                item_index,
                count,
                panel_type,
            } => {
                writer.write_u64(*item_index);
                writer.write_u16(*count);
                writer.write_u8(*panel_type);
            }
            Self::SellItem { unique_id, count } => {
                writer.write_u64(*unique_id);
                writer.write_u16(*count);
            }
            Self::RepairItem { unique_id } | Self::SRepairItem { unique_id } => {
                writer.write_u64(*unique_id);
            }
            Self::UnlockStorage { password } => writer.write_string(password)?,
            Self::SetStoragePassword {
                current_password,
                new_password,
            } => {
                writer.write_string(current_password)?;
                writer.write_string(new_password)?;
            }
            Self::RemoveStoragePassword { current_password } => {
                writer.write_string(current_password)?;
            }
        }

        Ok(())
    }

    fn decode_payload(packet_id: ClientPacketId, reader: &mut PacketReader<'_>) -> Result<Self> {
        let packet = match packet_id {
            ClientPacketId::ClientVersion => {
                let hash_length = reader.read_i32()?;
                if hash_length < 0 {
                    return Err(PacketCodecError::NegativeLength {
                        field: "version_hash",
                        value: hash_length,
                    });
                }
                Self::ClientVersion {
                    version_hash: reader.read_bytes(hash_length as usize)?,
                }
            }
            ClientPacketId::Disconnect => Self::Disconnect,
            ClientPacketId::KeepAlive => Self::KeepAlive {
                time: reader.read_i64()?,
            },
            ClientPacketId::NewAccount => Self::NewAccount {
                account_id: reader.read_string()?,
                password: reader.read_string()?,
                birth_date_binary: reader.read_i64()?,
                user_name: reader.read_string()?,
                secret_question: reader.read_string()?,
                secret_answer: reader.read_string()?,
                email_address: reader.read_string()?,
            },
            ClientPacketId::ChangePassword => Self::ChangePassword {
                account_id: reader.read_string()?,
                current_password: reader.read_string()?,
                new_password: reader.read_string()?,
            },
            ClientPacketId::Login => Self::Login {
                account_id: reader.read_string()?,
                password: reader.read_string()?,
            },
            ClientPacketId::NewCharacter => Self::NewCharacter {
                name: reader.read_string()?,
                gender: MirGender::try_from(reader.read_u8()?)?,
                class: MirClass::try_from(reader.read_u8()?)?,
            },
            ClientPacketId::DeleteCharacter => Self::DeleteCharacter {
                character_index: reader.read_i32()?,
            },
            ClientPacketId::StartGame => Self::StartGame {
                character_index: reader.read_i32()?,
            },
            ClientPacketId::LogOut => Self::LogOut,
            ClientPacketId::Turn => Self::Turn {
                direction: MirDirection::try_from(reader.read_u8()?)?,
            },
            ClientPacketId::Walk => Self::Walk {
                direction: MirDirection::try_from(reader.read_u8()?)?,
            },
            ClientPacketId::Run => Self::Run {
                direction: MirDirection::try_from(reader.read_u8()?)?,
            },
            ClientPacketId::Chat => {
                let message = reader.read_string()?;
                let linked_item_count = reader.read_i32()?;

                if linked_item_count != 0 {
                    return Err(PacketCodecError::UnsupportedLinkedItemCount(
                        linked_item_count,
                    ));
                }

                Self::Chat { message }
            }
            ClientPacketId::MoveItem => Self::MoveItem {
                grid: MirGridType::try_from(reader.read_u8()?)?,
                from: reader.read_i32()?,
                to: reader.read_i32()?,
            },
            ClientPacketId::StoreItem => Self::StoreItem {
                from: reader.read_i32()?,
                to: reader.read_i32()?,
            },
            ClientPacketId::TakeBackItem => Self::TakeBackItem {
                from: reader.read_i32()?,
                to: reader.read_i32()?,
            },
            ClientPacketId::CombineItem => Self::CombineItem {
                grid: MirGridType::try_from(reader.read_u8()?)?,
                id_from: reader.read_u64()?,
                id_to: reader.read_u64()?,
            },
            ClientPacketId::MergeItem => Self::MergeItem {
                grid_from: MirGridType::try_from(reader.read_u8()?)?,
                grid_to: MirGridType::try_from(reader.read_u8()?)?,
                id_from: reader.read_u64()?,
                id_to: reader.read_u64()?,
            },
            ClientPacketId::EquipItem => Self::EquipItem {
                grid: MirGridType::try_from(reader.read_u8()?)?,
                unique_id: reader.read_u64()?,
                to: reader.read_i32()?,
            },
            ClientPacketId::RemoveItem => Self::RemoveItem {
                grid: MirGridType::try_from(reader.read_u8()?)?,
                unique_id: reader.read_u64()?,
                to: reader.read_i32()?,
            },
            ClientPacketId::RemoveSlotItem => Self::RemoveSlotItem {
                grid: MirGridType::try_from(reader.read_u8()?)?,
                grid_to: MirGridType::try_from(reader.read_u8()?)?,
                unique_id: reader.read_u64()?,
                to: reader.read_i32()?,
                from_unique_id: reader.read_u64()?,
            },
            ClientPacketId::SplitItem => Self::SplitItem {
                grid: MirGridType::try_from(reader.read_u8()?)?,
                unique_id: reader.read_u64()?,
                count: reader.read_u16()?,
            },
            ClientPacketId::UseItem => Self::UseItem {
                unique_id: reader.read_u64()?,
                grid: MirGridType::try_from(reader.read_u8()?)?,
            },
            ClientPacketId::DropItem => Self::DropItem {
                unique_id: reader.read_u64()?,
                count: reader.read_u16()?,
                hero_inventory: reader.read_bool()?,
            },
            ClientPacketId::DeleteItem => Self::DeleteItem {
                unique_id: reader.read_u64()?,
                count: reader.read_u16()?,
                hero_inventory: reader.read_bool()?,
            },
            ClientPacketId::DropGold => Self::DropGold {
                amount: reader.read_u32()?,
            },
            ClientPacketId::PickUp => Self::PickUp,
            ClientPacketId::RequestItemInfo => Self::RequestItemInfo {
                item_index: reader.read_i32()?,
            },
            ClientPacketId::Attack => Self::Attack {
                direction: MirDirection::try_from(reader.read_u8()?)?,
                spell: Spell::try_from(reader.read_u8()?)?,
            },
            ClientPacketId::RangeAttack => Self::RangeAttack {
                direction: MirDirection::try_from(reader.read_u8()?)?,
                location: Point::decode(reader)?,
                target_id: reader.read_u32()?,
                target_location: Point::decode(reader)?,
            },
            ClientPacketId::Harvest => Self::Harvest {
                direction: MirDirection::try_from(reader.read_u8()?)?,
            },
            ClientPacketId::BuyItem => Self::BuyItem {
                item_index: reader.read_u64()?,
                count: reader.read_u16()?,
                panel_type: reader.read_u8()?,
            },
            ClientPacketId::SellItem => Self::SellItem {
                unique_id: reader.read_u64()?,
                count: reader.read_u16()?,
            },
            ClientPacketId::RepairItem => Self::RepairItem {
                unique_id: reader.read_u64()?,
            },
            ClientPacketId::SRepairItem => Self::SRepairItem {
                unique_id: reader.read_u64()?,
            },
            ClientPacketId::UnlockStorage => Self::UnlockStorage {
                password: reader.read_string()?,
            },
            ClientPacketId::SetStoragePassword => Self::SetStoragePassword {
                current_password: reader.read_string()?,
                new_password: reader.read_string()?,
            },
            ClientPacketId::RemoveStoragePassword => Self::RemoveStoragePassword {
                current_password: reader.read_string()?,
            },
        };

        Ok(packet)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ServerPacket {
    Connected,
    ClientVersion {
        result: u8,
    },
    Disconnect {
        reason: u8,
    },
    KeepAlive {
        time: i64,
    },
    NewAccount {
        result: u8,
    },
    ChangePassword {
        result: u8,
    },
    ChangePasswordBanned {
        reason: String,
        expiry_binary_datetime: i64,
    },
    Login {
        result: u8,
    },
    LoginBanned {
        reason: String,
        expiry_binary_datetime: i64,
    },
    LoginSuccess {
        characters: Vec<SelectInfo>,
    },
    NewCharacter {
        result: u8,
    },
    NewCharacterSuccess {
        char_info: SelectInfo,
    },
    DeleteCharacter {
        result: u8,
    },
    DeleteCharacterSuccess {
        character_index: i32,
    },
    StartGame {
        result: u8,
        resolution: i32,
    },
    StartGameBanned {
        reason: String,
        expiry_binary_datetime: i64,
    },
    StartGameDelay {
        milliseconds: i64,
    },
    MapInformation {
        info: MapInformation,
    },
    UserInformation {
        info: UserInformation,
    },
    UserLocation {
        location: UserLocation,
    },
    ObjectPlayer {
        info: ObjectPlayerInfo,
    },
    ObjectRemove {
        object_id: u32,
    },
    ObjectTurn {
        movement: ObjectMovement,
    },
    ObjectWalk {
        movement: ObjectMovement,
    },
    ObjectRun {
        movement: ObjectMovement,
    },
    ObjectBackStep {
        movement: ObjectMovement,
        distance: i32,
    },
    ObjectSitDown {
        movement: ObjectMovement,
        sitting: bool,
    },
    Chat {
        message: String,
        chat_type: ChatType,
    },
    ObjectChat {
        object_id: u32,
        text: String,
        chat_type: ChatType,
    },
    NewItemInfo {
        info: ItemInfo,
    },
    MoveItem {
        grid: MirGridType,
        from: i32,
        to: i32,
        success: bool,
    },
    EquipItem {
        grid: MirGridType,
        unique_id: u64,
        to: i32,
        success: bool,
    },
    MergeItem {
        grid_from: MirGridType,
        grid_to: MirGridType,
        id_from: u64,
        id_to: u64,
        success: bool,
    },
    RemoveItem {
        grid: MirGridType,
        unique_id: u64,
        to: i32,
        success: bool,
    },
    RemoveSlotItem {
        grid: MirGridType,
        grid_to: MirGridType,
        unique_id: u64,
        to: i32,
        success: bool,
    },
    TakeBackItem {
        from: i32,
        to: i32,
        success: bool,
    },
    StoreItem {
        from: i32,
        to: i32,
        success: bool,
    },
    CombineItem {
        grid: MirGridType,
        id_from: u64,
        id_to: u64,
        success: bool,
        destroy: bool,
    },
    ItemUpgraded {
        item: UserItem,
    },
    SplitItem {
        item: Option<UserItem>,
        grid: MirGridType,
    },
    SplitItem1 {
        grid: MirGridType,
        unique_id: u64,
        count: u16,
        success: bool,
    },
    UseItem {
        unique_id: u64,
        success: bool,
        grid: MirGridType,
    },
    DropItem {
        unique_id: u64,
        count: u16,
        hero_inventory: bool,
        success: bool,
    },
    NewMonsterInfo {
        info: MonsterInfo,
    },
    NewNpcInfo {
        info: NpcInfo,
    },
    ObjectItem {
        info: ObjectItemInfo,
    },
    ObjectGold {
        info: ObjectGoldInfo,
    },
    GainedItem {
        item: UserItem,
    },
    GainedGold {
        gold: u32,
    },
    LoseGold {
        gold: u32,
    },
    GainedCredit {
        credit: u32,
    },
    LoseCredit {
        credit: u32,
    },
    ObjectMonster {
        info: MonsterInfo,
    },
    ObjectAttack {
        info: ObjectAttackInfo,
    },
    Struck {
        info: StruckInfo,
    },
    ObjectStruck {
        info: ObjectStruckInfo,
    },
    DuraChanged {
        unique_id: u64,
        current_dura: u16,
    },
    DeleteItem {
        unique_id: u64,
        count: u16,
    },
    ObjectDied {
        info: ObjectDiedInfo,
    },
    ObjectHarvest {
        movement: ObjectMovement,
    },
    ObjectHarvested {
        movement: ObjectMovement,
    },
    ObjectNpc {
        info: NpcInfo,
    },
    ObjectHide {
        object_id: u32,
    },
    ObjectShow {
        object_id: u32,
    },
    ObjectTeleportOut {
        object_id: u32,
        effect_type: u8,
    },
    ObjectTeleportIn {
        object_id: u32,
        effect_type: u8,
    },
    TeleportIn,
    NPCGoods {
        list: Vec<UserItem>,
        rate: f32,
        panel_type: u8,
        hide_added_stats: bool,
    },
    NPCSell,
    NPCRepair {
        rate: f32,
    },
    NPCSRepair {
        rate: f32,
    },
    NPCRefine {
        rate: f32,
        refining: bool,
    },
    NPCCheckRefine,
    NPCCollectRefine {
        success: bool,
    },
    NPCReplaceWedRing {
        rate: f32,
    },
    NPCStorage,
    UserStorage {
        storage: Option<Vec<Option<UserItem>>>,
    },
    ItemRepaired {
        unique_id: u64,
        max_dura: u16,
        current_dura: u16,
    },
    ItemSlotSizeChanged {
        unique_id: u64,
        slot_size: i32,
    },
    ItemSealChanged {
        unique_id: u64,
        expiry_date_binary_datetime: i64,
    },
    SellItem {
        unique_id: u64,
        count: u16,
        success: bool,
    },
    RepairItem {
        unique_id: u64,
    },
    CraftItem {
        success: bool,
    },
    ObjectRevived {
        info: ObjectRevivedInfo,
    },
    ObjectEffect {
        info: ObjectEffectInfo,
    },
    ObjectHealth {
        info: ObjectHealthInfo,
    },
    ObjectRangeAttack {
        info: ObjectRangeAttackInfo,
    },
    RefreshItem {
        item: UserItem,
    },
    ObjectSpell {
        info: ObjectSpellInfo,
    },
    ResizeStorage {
        size: i32,
        has_expanded_storage: bool,
        expiry_time_binary_datetime: i64,
    },
    StorageUnlockResult {
        result: u8,
        has_password: bool,
    },
    StoragePasswordResult {
        result: u8,
        removing: bool,
        has_password: bool,
        last_set_binary_datetime: i64,
    },
    LogOutSuccess {
        characters: Vec<SelectInfo>,
    },
    LogOutFailed,
}

impl ServerPacket {
    pub fn packet_id(&self) -> ServerPacketId {
        match self {
            Self::Connected => ServerPacketId::Connected,
            Self::ClientVersion { .. } => ServerPacketId::ClientVersion,
            Self::Disconnect { .. } => ServerPacketId::Disconnect,
            Self::KeepAlive { .. } => ServerPacketId::KeepAlive,
            Self::NewAccount { .. } => ServerPacketId::NewAccount,
            Self::ChangePassword { .. } => ServerPacketId::ChangePassword,
            Self::ChangePasswordBanned { .. } => ServerPacketId::ChangePasswordBanned,
            Self::Login { .. } => ServerPacketId::Login,
            Self::LoginBanned { .. } => ServerPacketId::LoginBanned,
            Self::LoginSuccess { .. } => ServerPacketId::LoginSuccess,
            Self::NewCharacter { .. } => ServerPacketId::NewCharacter,
            Self::NewCharacterSuccess { .. } => ServerPacketId::NewCharacterSuccess,
            Self::DeleteCharacter { .. } => ServerPacketId::DeleteCharacter,
            Self::DeleteCharacterSuccess { .. } => ServerPacketId::DeleteCharacterSuccess,
            Self::StartGame { .. } => ServerPacketId::StartGame,
            Self::StartGameBanned { .. } => ServerPacketId::StartGameBanned,
            Self::StartGameDelay { .. } => ServerPacketId::StartGameDelay,
            Self::MapInformation { .. } => ServerPacketId::MapInformation,
            Self::UserInformation { .. } => ServerPacketId::UserInformation,
            Self::UserLocation { .. } => ServerPacketId::UserLocation,
            Self::ObjectPlayer { .. } => ServerPacketId::ObjectPlayer,
            Self::ObjectRemove { .. } => ServerPacketId::ObjectRemove,
            Self::ObjectTurn { .. } => ServerPacketId::ObjectTurn,
            Self::ObjectWalk { .. } => ServerPacketId::ObjectWalk,
            Self::ObjectRun { .. } => ServerPacketId::ObjectRun,
            Self::ObjectBackStep { .. } => ServerPacketId::ObjectBackStep,
            Self::ObjectSitDown { .. } => ServerPacketId::ObjectSitDown,
            Self::Chat { .. } => ServerPacketId::Chat,
            Self::ObjectChat { .. } => ServerPacketId::ObjectChat,
            Self::NewItemInfo { .. } => ServerPacketId::NewItemInfo,
            Self::MoveItem { .. } => ServerPacketId::MoveItem,
            Self::EquipItem { .. } => ServerPacketId::EquipItem,
            Self::MergeItem { .. } => ServerPacketId::MergeItem,
            Self::RemoveItem { .. } => ServerPacketId::RemoveItem,
            Self::RemoveSlotItem { .. } => ServerPacketId::RemoveSlotItem,
            Self::TakeBackItem { .. } => ServerPacketId::TakeBackItem,
            Self::StoreItem { .. } => ServerPacketId::StoreItem,
            Self::CombineItem { .. } => ServerPacketId::CombineItem,
            Self::ItemUpgraded { .. } => ServerPacketId::ItemUpgraded,
            Self::SplitItem { .. } => ServerPacketId::SplitItem,
            Self::SplitItem1 { .. } => ServerPacketId::SplitItem1,
            Self::UseItem { .. } => ServerPacketId::UseItem,
            Self::DropItem { .. } => ServerPacketId::DropItem,
            Self::NewMonsterInfo { .. } => ServerPacketId::NewMonsterInfo,
            Self::NewNpcInfo { .. } => ServerPacketId::NewNpcInfo,
            Self::ObjectItem { .. } => ServerPacketId::ObjectItem,
            Self::ObjectGold { .. } => ServerPacketId::ObjectGold,
            Self::GainedItem { .. } => ServerPacketId::GainedItem,
            Self::GainedGold { .. } => ServerPacketId::GainedGold,
            Self::LoseGold { .. } => ServerPacketId::LoseGold,
            Self::GainedCredit { .. } => ServerPacketId::GainedCredit,
            Self::LoseCredit { .. } => ServerPacketId::LoseCredit,
            Self::ObjectMonster { .. } => ServerPacketId::ObjectMonster,
            Self::ObjectAttack { .. } => ServerPacketId::ObjectAttack,
            Self::Struck { .. } => ServerPacketId::Struck,
            Self::ObjectStruck { .. } => ServerPacketId::ObjectStruck,
            Self::DuraChanged { .. } => ServerPacketId::DuraChanged,
            Self::DeleteItem { .. } => ServerPacketId::DeleteItem,
            Self::ObjectDied { .. } => ServerPacketId::ObjectDied,
            Self::ObjectHarvest { .. } => ServerPacketId::ObjectHarvest,
            Self::ObjectHarvested { .. } => ServerPacketId::ObjectHarvested,
            Self::ObjectNpc { .. } => ServerPacketId::ObjectNpc,
            Self::ObjectHide { .. } => ServerPacketId::ObjectHide,
            Self::ObjectShow { .. } => ServerPacketId::ObjectShow,
            Self::ObjectTeleportOut { .. } => ServerPacketId::ObjectTeleportOut,
            Self::ObjectTeleportIn { .. } => ServerPacketId::ObjectTeleportIn,
            Self::TeleportIn => ServerPacketId::TeleportIn,
            Self::NPCGoods { .. } => ServerPacketId::NPCGoods,
            Self::NPCSell => ServerPacketId::NPCSell,
            Self::NPCRepair { .. } => ServerPacketId::NPCRepair,
            Self::NPCSRepair { .. } => ServerPacketId::NPCSRepair,
            Self::NPCRefine { .. } => ServerPacketId::NPCRefine,
            Self::NPCCheckRefine => ServerPacketId::NPCCheckRefine,
            Self::NPCCollectRefine { .. } => ServerPacketId::NPCCollectRefine,
            Self::NPCReplaceWedRing { .. } => ServerPacketId::NPCReplaceWedRing,
            Self::NPCStorage => ServerPacketId::NPCStorage,
            Self::UserStorage { .. } => ServerPacketId::UserStorage,
            Self::SellItem { .. } => ServerPacketId::SellItem,
            Self::RepairItem { .. } => ServerPacketId::RepairItem,
            Self::CraftItem { .. } => ServerPacketId::CraftItem,
            Self::ItemRepaired { .. } => ServerPacketId::ItemRepaired,
            Self::ItemSlotSizeChanged { .. } => ServerPacketId::ItemSlotSizeChanged,
            Self::ItemSealChanged { .. } => ServerPacketId::ItemSealChanged,
            Self::ObjectRevived { .. } => ServerPacketId::ObjectRevived,
            Self::ObjectEffect { .. } => ServerPacketId::ObjectEffect,
            Self::ObjectHealth { .. } => ServerPacketId::ObjectHealth,
            Self::ObjectRangeAttack { .. } => ServerPacketId::ObjectRangeAttack,
            Self::RefreshItem { .. } => ServerPacketId::RefreshItem,
            Self::ObjectSpell { .. } => ServerPacketId::ObjectSpell,
            Self::ResizeStorage { .. } => ServerPacketId::ResizeStorage,
            Self::StorageUnlockResult { .. } => ServerPacketId::StorageUnlockResult,
            Self::StoragePasswordResult { .. } => ServerPacketId::StoragePasswordResult,
            Self::LogOutSuccess { .. } => ServerPacketId::LogOutSuccess,
            Self::LogOutFailed => ServerPacketId::LogOutFailed,
        }
    }

    fn encode_payload(&self, writer: &mut PacketWriter) -> Result<()> {
        match self {
            Self::Connected => {}
            Self::ClientVersion { result }
            | Self::NewAccount { result }
            | Self::ChangePassword { result }
            | Self::Login { result }
            | Self::NewCharacter { result }
            | Self::DeleteCharacter { result } => writer.write_u8(*result),
            Self::Disconnect { reason } => writer.write_u8(*reason),
            Self::KeepAlive { time } => writer.write_i64(*time),
            Self::LoginBanned {
                reason,
                expiry_binary_datetime,
            }
            | Self::ChangePasswordBanned {
                reason,
                expiry_binary_datetime,
            }
            | Self::StartGameBanned {
                reason,
                expiry_binary_datetime,
            } => {
                writer.write_string(reason)?;
                writer.write_i64(*expiry_binary_datetime);
            }
            Self::LoginSuccess { characters } | Self::LogOutSuccess { characters } => {
                writer.write_i32(characters.len() as i32);
                for character in characters {
                    character.encode(writer)?;
                }
            }
            Self::NewCharacterSuccess { char_info } => char_info.encode(writer)?,
            Self::DeleteCharacterSuccess { character_index } => writer.write_i32(*character_index),
            Self::StartGame { result, resolution } => {
                writer.write_u8(*result);
                writer.write_i32(*resolution);
            }
            Self::StartGameDelay { milliseconds } => writer.write_i64(*milliseconds),
            Self::MapInformation { info } => info.encode(writer)?,
            Self::UserInformation { info } => info.encode(writer)?,
            Self::UserLocation { location } => location.encode(writer),
            Self::ObjectPlayer { info } => info.encode(writer)?,
            Self::ObjectRemove { object_id }
            | Self::ObjectHide { object_id }
            | Self::ObjectShow { object_id } => writer.write_u32(*object_id),
            Self::ObjectTeleportOut {
                object_id,
                effect_type,
            }
            | Self::ObjectTeleportIn {
                object_id,
                effect_type,
            } => {
                writer.write_u32(*object_id);
                writer.write_u8(*effect_type);
            }
            Self::TeleportIn | Self::NPCSell | Self::NPCCheckRefine | Self::NPCStorage => {}
            Self::UserStorage { storage } => {
                writer.write_bool(storage.is_some());
                let Some(storage) = storage else {
                    return Ok(());
                };

                writer.write_i32(storage.len() as i32);
                for item in storage {
                    writer.write_bool(item.is_some());
                    if let Some(item) = item {
                        item.encode(writer)?;
                    }
                }
            }
            Self::NPCGoods {
                list,
                rate,
                panel_type,
                hide_added_stats,
            } => {
                writer.write_i32(list.len() as i32);
                for item in list {
                    item.encode(writer)?;
                }
                writer.write_f32(*rate);
                writer.write_u8(*panel_type);
                writer.write_bool(*hide_added_stats);
            }
            Self::NPCRepair { rate }
            | Self::NPCSRepair { rate }
            | Self::NPCReplaceWedRing { rate } => writer.write_f32(*rate),
            Self::NPCRefine { rate, refining } => {
                writer.write_f32(*rate);
                writer.write_bool(*refining);
            }
            Self::NPCCollectRefine { success } | Self::CraftItem { success } => {
                writer.write_bool(*success)
            }
            Self::ObjectTurn { movement }
            | Self::ObjectWalk { movement }
            | Self::ObjectRun { movement } => movement.encode(writer),
            Self::ObjectBackStep { movement, distance } => {
                movement.encode(writer);
                writer.write_i32(*distance);
            }
            Self::ObjectSitDown { movement, sitting } => {
                movement.encode(writer);
                writer.write_bool(*sitting);
            }
            Self::Chat { message, chat_type } => {
                writer.write_string(message)?;
                writer.write_u8(*chat_type as u8);
            }
            Self::ObjectChat {
                object_id,
                text,
                chat_type,
            } => {
                writer.write_u32(*object_id);
                writer.write_string(text)?;
                writer.write_u8(*chat_type as u8);
            }
            Self::NewItemInfo { info } => info.encode(writer)?,
            Self::MoveItem {
                grid,
                from,
                to,
                success,
            } => {
                writer.write_u8(*grid as u8);
                writer.write_i32(*from);
                writer.write_i32(*to);
                writer.write_bool(*success);
            }
            Self::EquipItem {
                grid,
                unique_id,
                to,
                success,
            }
            | Self::RemoveItem {
                grid,
                unique_id,
                to,
                success,
            } => {
                writer.write_u8(*grid as u8);
                writer.write_u64(*unique_id);
                writer.write_i32(*to);
                writer.write_bool(*success);
            }
            Self::MergeItem {
                grid_from,
                grid_to,
                id_from,
                id_to,
                success,
            } => {
                writer.write_u8(*grid_from as u8);
                writer.write_u8(*grid_to as u8);
                writer.write_u64(*id_from);
                writer.write_u64(*id_to);
                writer.write_bool(*success);
            }
            Self::RemoveSlotItem {
                grid,
                grid_to,
                unique_id,
                to,
                success,
            } => {
                writer.write_u8(*grid as u8);
                writer.write_u8(*grid_to as u8);
                writer.write_u64(*unique_id);
                writer.write_i32(*to);
                writer.write_bool(*success);
            }
            Self::TakeBackItem { from, to, success } | Self::StoreItem { from, to, success } => {
                writer.write_i32(*from);
                writer.write_i32(*to);
                writer.write_bool(*success);
            }
            Self::CombineItem {
                grid,
                id_from,
                id_to,
                success,
                destroy,
            } => {
                writer.write_u8(*grid as u8);
                writer.write_u64(*id_from);
                writer.write_u64(*id_to);
                writer.write_bool(*success);
                writer.write_bool(*destroy);
            }
            Self::ItemUpgraded { item } => item.encode(writer)?,
            Self::SplitItem { item, grid } => {
                writer.write_bool(item.is_some());
                if let Some(item) = item {
                    item.encode(writer)?;
                }
                writer.write_u8(*grid as u8);
            }
            Self::SplitItem1 {
                grid,
                unique_id,
                count,
                success,
            } => {
                writer.write_u8(*grid as u8);
                writer.write_u64(*unique_id);
                writer.write_u16(*count);
                writer.write_bool(*success);
            }
            Self::UseItem {
                unique_id,
                success,
                grid,
            } => {
                writer.write_u64(*unique_id);
                writer.write_bool(*success);
                writer.write_u8(*grid as u8);
            }
            Self::DropItem {
                unique_id,
                count,
                hero_inventory,
                success,
            } => {
                writer.write_u64(*unique_id);
                writer.write_u16(*count);
                writer.write_bool(*hero_inventory);
                writer.write_bool(*success);
            }
            Self::NewMonsterInfo { info } => info.encode(writer)?,
            Self::NewNpcInfo { info } => info.encode(writer)?,
            Self::ObjectItem { info } => info.encode(writer)?,
            Self::ObjectGold { info } => info.encode(writer),
            Self::GainedItem { item } => item.encode(writer)?,
            Self::GainedGold { gold } | Self::LoseGold { gold } => writer.write_u32(*gold),
            Self::GainedCredit { credit } | Self::LoseCredit { credit } => {
                writer.write_u32(*credit)
            }
            Self::ObjectMonster { info } => info.encode(writer)?,
            Self::ObjectAttack { info } => info.encode(writer),
            Self::Struck { info } => info.encode(writer),
            Self::ObjectStruck { info } => info.encode(writer),
            Self::DuraChanged {
                unique_id,
                current_dura,
            } => {
                writer.write_u64(*unique_id);
                writer.write_u16(*current_dura);
            }
            Self::DeleteItem { unique_id, count } => {
                writer.write_u64(*unique_id);
                writer.write_u16(*count);
            }
            Self::ObjectDied { info } => info.encode(writer),
            Self::ObjectHarvest { movement } | Self::ObjectHarvested { movement } => {
                movement.encode(writer);
            }
            Self::ObjectNpc { info } => info.encode(writer)?,
            Self::ItemRepaired {
                unique_id,
                max_dura,
                current_dura,
            } => {
                writer.write_u64(*unique_id);
                writer.write_u16(*max_dura);
                writer.write_u16(*current_dura);
            }
            Self::ItemSlotSizeChanged {
                unique_id,
                slot_size,
            } => {
                writer.write_u64(*unique_id);
                writer.write_i32(*slot_size);
            }
            Self::ItemSealChanged {
                unique_id,
                expiry_date_binary_datetime,
            } => {
                writer.write_u64(*unique_id);
                writer.write_i64(*expiry_date_binary_datetime);
            }
            Self::SellItem {
                unique_id,
                count,
                success,
            } => {
                writer.write_u64(*unique_id);
                writer.write_u16(*count);
                writer.write_bool(*success);
            }
            Self::RepairItem { unique_id } => writer.write_u64(*unique_id),
            Self::ObjectRevived { info } => info.encode(writer),
            Self::ObjectEffect { info } => info.encode(writer),
            Self::ObjectHealth { info } => info.encode(writer),
            Self::ObjectRangeAttack { info } => info.encode(writer),
            Self::RefreshItem { item } => item.encode(writer)?,
            Self::ObjectSpell { info } => info.encode(writer),
            Self::ResizeStorage {
                size,
                has_expanded_storage,
                expiry_time_binary_datetime,
            } => {
                writer.write_i32(*size);
                writer.write_bool(*has_expanded_storage);
                writer.write_i64(*expiry_time_binary_datetime);
            }
            Self::StorageUnlockResult {
                result,
                has_password,
            } => {
                writer.write_u8(*result);
                writer.write_bool(*has_password);
            }
            Self::StoragePasswordResult {
                result,
                removing,
                has_password,
                last_set_binary_datetime,
            } => {
                writer.write_u8(*result);
                writer.write_bool(*removing);
                writer.write_bool(*has_password);
                writer.write_i64(*last_set_binary_datetime);
            }
            Self::LogOutFailed => {}
        }

        Ok(())
    }

    fn decode_payload(packet_id: ServerPacketId, reader: &mut PacketReader<'_>) -> Result<Self> {
        let packet = match packet_id {
            ServerPacketId::Connected => Self::Connected,
            ServerPacketId::ClientVersion => Self::ClientVersion {
                result: reader.read_u8()?,
            },
            ServerPacketId::Disconnect => Self::Disconnect {
                reason: reader.read_u8()?,
            },
            ServerPacketId::KeepAlive => Self::KeepAlive {
                time: reader.read_i64()?,
            },
            ServerPacketId::NewAccount => Self::NewAccount {
                result: reader.read_u8()?,
            },
            ServerPacketId::ChangePassword => Self::ChangePassword {
                result: reader.read_u8()?,
            },
            ServerPacketId::ChangePasswordBanned => Self::ChangePasswordBanned {
                reason: reader.read_string()?,
                expiry_binary_datetime: reader.read_i64()?,
            },
            ServerPacketId::Login => Self::Login {
                result: reader.read_u8()?,
            },
            ServerPacketId::LoginBanned => Self::LoginBanned {
                reason: reader.read_string()?,
                expiry_binary_datetime: reader.read_i64()?,
            },
            ServerPacketId::LoginSuccess => Self::LoginSuccess {
                characters: decode_select_info_vec(reader)?,
            },
            ServerPacketId::NewCharacter => Self::NewCharacter {
                result: reader.read_u8()?,
            },
            ServerPacketId::NewCharacterSuccess => Self::NewCharacterSuccess {
                char_info: SelectInfo::decode(reader)?,
            },
            ServerPacketId::DeleteCharacter => Self::DeleteCharacter {
                result: reader.read_u8()?,
            },
            ServerPacketId::DeleteCharacterSuccess => Self::DeleteCharacterSuccess {
                character_index: reader.read_i32()?,
            },
            ServerPacketId::StartGame => Self::StartGame {
                result: reader.read_u8()?,
                resolution: reader.read_i32()?,
            },
            ServerPacketId::StartGameBanned => Self::StartGameBanned {
                reason: reader.read_string()?,
                expiry_binary_datetime: reader.read_i64()?,
            },
            ServerPacketId::StartGameDelay => Self::StartGameDelay {
                milliseconds: reader.read_i64()?,
            },
            ServerPacketId::MapInformation => Self::MapInformation {
                info: MapInformation::decode(reader)?,
            },
            ServerPacketId::UserInformation => Self::UserInformation {
                info: UserInformation::decode(reader)?,
            },
            ServerPacketId::UserLocation => Self::UserLocation {
                location: UserLocation::decode(reader)?,
            },
            ServerPacketId::ObjectPlayer => Self::ObjectPlayer {
                info: ObjectPlayerInfo::decode(reader)?,
            },
            ServerPacketId::ObjectRemove => Self::ObjectRemove {
                object_id: reader.read_u32()?,
            },
            ServerPacketId::ObjectTurn => Self::ObjectTurn {
                movement: ObjectMovement::decode(reader)?,
            },
            ServerPacketId::ObjectWalk => Self::ObjectWalk {
                movement: ObjectMovement::decode(reader)?,
            },
            ServerPacketId::ObjectRun => Self::ObjectRun {
                movement: ObjectMovement::decode(reader)?,
            },
            ServerPacketId::ObjectBackStep => Self::ObjectBackStep {
                movement: ObjectMovement::decode(reader)?,
                distance: reader.read_i32()?,
            },
            ServerPacketId::ObjectSitDown => Self::ObjectSitDown {
                movement: ObjectMovement::decode(reader)?,
                sitting: reader.read_bool()?,
            },
            ServerPacketId::Chat => Self::Chat {
                message: reader.read_string()?,
                chat_type: ChatType::try_from(reader.read_u8()?)?,
            },
            ServerPacketId::ObjectChat => Self::ObjectChat {
                object_id: reader.read_u32()?,
                text: reader.read_string()?,
                chat_type: ChatType::try_from(reader.read_u8()?)?,
            },
            ServerPacketId::NewItemInfo => Self::NewItemInfo {
                info: ItemInfo::decode(reader)?,
            },
            ServerPacketId::MoveItem => Self::MoveItem {
                grid: MirGridType::try_from(reader.read_u8()?)?,
                from: reader.read_i32()?,
                to: reader.read_i32()?,
                success: reader.read_bool()?,
            },
            ServerPacketId::EquipItem => Self::EquipItem {
                grid: MirGridType::try_from(reader.read_u8()?)?,
                unique_id: reader.read_u64()?,
                to: reader.read_i32()?,
                success: reader.read_bool()?,
            },
            ServerPacketId::MergeItem => Self::MergeItem {
                grid_from: MirGridType::try_from(reader.read_u8()?)?,
                grid_to: MirGridType::try_from(reader.read_u8()?)?,
                id_from: reader.read_u64()?,
                id_to: reader.read_u64()?,
                success: reader.read_bool()?,
            },
            ServerPacketId::RemoveItem => Self::RemoveItem {
                grid: MirGridType::try_from(reader.read_u8()?)?,
                unique_id: reader.read_u64()?,
                to: reader.read_i32()?,
                success: reader.read_bool()?,
            },
            ServerPacketId::RemoveSlotItem => Self::RemoveSlotItem {
                grid: MirGridType::try_from(reader.read_u8()?)?,
                grid_to: MirGridType::try_from(reader.read_u8()?)?,
                unique_id: reader.read_u64()?,
                to: reader.read_i32()?,
                success: reader.read_bool()?,
            },
            ServerPacketId::TakeBackItem => Self::TakeBackItem {
                from: reader.read_i32()?,
                to: reader.read_i32()?,
                success: reader.read_bool()?,
            },
            ServerPacketId::StoreItem => Self::StoreItem {
                from: reader.read_i32()?,
                to: reader.read_i32()?,
                success: reader.read_bool()?,
            },
            ServerPacketId::CombineItem => Self::CombineItem {
                grid: MirGridType::try_from(reader.read_u8()?)?,
                id_from: reader.read_u64()?,
                id_to: reader.read_u64()?,
                success: reader.read_bool()?,
                destroy: reader.read_bool()?,
            },
            ServerPacketId::ItemUpgraded => Self::ItemUpgraded {
                item: UserItem::decode(reader)?,
            },
            ServerPacketId::SplitItem => {
                let item = if reader.read_bool()? {
                    Some(UserItem::decode(reader)?)
                } else {
                    None
                };
                Self::SplitItem {
                    item,
                    grid: MirGridType::try_from(reader.read_u8()?)?,
                }
            }
            ServerPacketId::SplitItem1 => Self::SplitItem1 {
                grid: MirGridType::try_from(reader.read_u8()?)?,
                unique_id: reader.read_u64()?,
                count: reader.read_u16()?,
                success: reader.read_bool()?,
            },
            ServerPacketId::UseItem => Self::UseItem {
                unique_id: reader.read_u64()?,
                success: reader.read_bool()?,
                grid: MirGridType::try_from(reader.read_u8()?)?,
            },
            ServerPacketId::DropItem => Self::DropItem {
                unique_id: reader.read_u64()?,
                count: reader.read_u16()?,
                hero_inventory: reader.read_bool()?,
                success: reader.read_bool()?,
            },
            ServerPacketId::NewMonsterInfo => Self::NewMonsterInfo {
                info: MonsterInfo::decode(reader)?,
            },
            ServerPacketId::NewNpcInfo => Self::NewNpcInfo {
                info: NpcInfo::decode(reader)?,
            },
            ServerPacketId::ObjectItem => Self::ObjectItem {
                info: ObjectItemInfo::decode(reader)?,
            },
            ServerPacketId::ObjectGold => Self::ObjectGold {
                info: ObjectGoldInfo::decode(reader)?,
            },
            ServerPacketId::GainedItem => Self::GainedItem {
                item: UserItem::decode(reader)?,
            },
            ServerPacketId::GainedGold => Self::GainedGold {
                gold: reader.read_u32()?,
            },
            ServerPacketId::LoseGold => Self::LoseGold {
                gold: reader.read_u32()?,
            },
            ServerPacketId::GainedCredit => Self::GainedCredit {
                credit: reader.read_u32()?,
            },
            ServerPacketId::LoseCredit => Self::LoseCredit {
                credit: reader.read_u32()?,
            },
            ServerPacketId::ObjectMonster => Self::ObjectMonster {
                info: MonsterInfo::decode(reader)?,
            },
            ServerPacketId::ObjectAttack => Self::ObjectAttack {
                info: ObjectAttackInfo::decode(reader)?,
            },
            ServerPacketId::Struck => Self::Struck {
                info: StruckInfo::decode(reader)?,
            },
            ServerPacketId::ObjectStruck => Self::ObjectStruck {
                info: ObjectStruckInfo::decode(reader)?,
            },
            ServerPacketId::DuraChanged => Self::DuraChanged {
                unique_id: reader.read_u64()?,
                current_dura: reader.read_u16()?,
            },
            ServerPacketId::DeleteItem => Self::DeleteItem {
                unique_id: reader.read_u64()?,
                count: reader.read_u16()?,
            },
            ServerPacketId::ObjectDied => Self::ObjectDied {
                info: ObjectDiedInfo::decode(reader)?,
            },
            ServerPacketId::ObjectHarvest => Self::ObjectHarvest {
                movement: ObjectMovement::decode(reader)?,
            },
            ServerPacketId::ObjectHarvested => Self::ObjectHarvested {
                movement: ObjectMovement::decode(reader)?,
            },
            ServerPacketId::ObjectNpc => Self::ObjectNpc {
                info: NpcInfo::decode(reader)?,
            },
            ServerPacketId::ObjectHide => Self::ObjectHide {
                object_id: reader.read_u32()?,
            },
            ServerPacketId::ObjectShow => Self::ObjectShow {
                object_id: reader.read_u32()?,
            },
            ServerPacketId::ObjectTeleportOut => Self::ObjectTeleportOut {
                object_id: reader.read_u32()?,
                effect_type: reader.read_u8()?,
            },
            ServerPacketId::ObjectTeleportIn => Self::ObjectTeleportIn {
                object_id: reader.read_u32()?,
                effect_type: reader.read_u8()?,
            },
            ServerPacketId::TeleportIn => Self::TeleportIn,
            ServerPacketId::NPCGoods => {
                let count = reader.read_i32()?;
                if count < 0 {
                    return Err(PacketCodecError::NegativeLength {
                        field: "npc_goods",
                        value: count,
                    });
                }
                let mut list = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    list.push(UserItem::decode(reader)?);
                }
                Self::NPCGoods {
                    list,
                    rate: reader.read_f32()?,
                    panel_type: reader.read_u8()?,
                    hide_added_stats: reader.read_bool()?,
                }
            }
            ServerPacketId::NPCSell => Self::NPCSell,
            ServerPacketId::NPCRepair => Self::NPCRepair {
                rate: reader.read_f32()?,
            },
            ServerPacketId::NPCSRepair => Self::NPCSRepair {
                rate: reader.read_f32()?,
            },
            ServerPacketId::NPCRefine => Self::NPCRefine {
                rate: reader.read_f32()?,
                refining: reader.read_bool()?,
            },
            ServerPacketId::NPCCheckRefine => Self::NPCCheckRefine,
            ServerPacketId::NPCCollectRefine => Self::NPCCollectRefine {
                success: reader.read_bool()?,
            },
            ServerPacketId::NPCReplaceWedRing => Self::NPCReplaceWedRing {
                rate: reader.read_f32()?,
            },
            ServerPacketId::NPCStorage => Self::NPCStorage,
            ServerPacketId::UserStorage => {
                let storage = if reader.read_bool()? {
                    let count = reader.read_i32()?;
                    if count < 0 {
                        return Err(PacketCodecError::NegativeLength {
                            field: "user_storage_count",
                            value: count,
                        });
                    }

                    let mut storage = Vec::with_capacity(count as usize);
                    for _ in 0..count {
                        if reader.read_bool()? {
                            storage.push(Some(UserItem::decode(reader)?));
                        } else {
                            storage.push(None);
                        }
                    }
                    Some(storage)
                } else {
                    None
                };

                Self::UserStorage { storage }
            }
            ServerPacketId::ItemRepaired => Self::ItemRepaired {
                unique_id: reader.read_u64()?,
                max_dura: reader.read_u16()?,
                current_dura: reader.read_u16()?,
            },
            ServerPacketId::ItemSlotSizeChanged => Self::ItemSlotSizeChanged {
                unique_id: reader.read_u64()?,
                slot_size: reader.read_i32()?,
            },
            ServerPacketId::ItemSealChanged => Self::ItemSealChanged {
                unique_id: reader.read_u64()?,
                expiry_date_binary_datetime: reader.read_i64()?,
            },
            ServerPacketId::SellItem => Self::SellItem {
                unique_id: reader.read_u64()?,
                count: reader.read_u16()?,
                success: reader.read_bool()?,
            },
            ServerPacketId::RepairItem => Self::RepairItem {
                unique_id: reader.read_u64()?,
            },
            ServerPacketId::CraftItem => Self::CraftItem {
                success: reader.read_bool()?,
            },
            ServerPacketId::ObjectRevived => Self::ObjectRevived {
                info: ObjectRevivedInfo::decode(reader)?,
            },
            ServerPacketId::ObjectEffect => Self::ObjectEffect {
                info: ObjectEffectInfo::decode(reader)?,
            },
            ServerPacketId::ObjectHealth => Self::ObjectHealth {
                info: ObjectHealthInfo::decode(reader)?,
            },
            ServerPacketId::ObjectRangeAttack => Self::ObjectRangeAttack {
                info: ObjectRangeAttackInfo::decode(reader)?,
            },
            ServerPacketId::RefreshItem => Self::RefreshItem {
                item: UserItem::decode(reader)?,
            },
            ServerPacketId::ObjectSpell => Self::ObjectSpell {
                info: ObjectSpellInfo::decode(reader)?,
            },
            ServerPacketId::ResizeStorage => Self::ResizeStorage {
                size: reader.read_i32()?,
                has_expanded_storage: reader.read_bool()?,
                expiry_time_binary_datetime: reader.read_i64()?,
            },
            ServerPacketId::StorageUnlockResult => Self::StorageUnlockResult {
                result: reader.read_u8()?,
                has_password: reader.read_bool()?,
            },
            ServerPacketId::StoragePasswordResult => Self::StoragePasswordResult {
                result: reader.read_u8()?,
                removing: reader.read_bool()?,
                has_password: reader.read_bool()?,
                last_set_binary_datetime: reader.read_i64()?,
            },
            ServerPacketId::LogOutSuccess => Self::LogOutSuccess {
                characters: decode_select_info_vec(reader)?,
            },
            ServerPacketId::LogOutFailed => Self::LogOutFailed,
        };

        Ok(packet)
    }
}

pub fn decode_client_packet(frame_bytes: &[u8]) -> Result<ClientPacket> {
    let frame = decode_frame(frame_bytes)?;
    let packet_id = ClientPacketId::try_from(frame.packet_id)?;
    let mut reader = PacketReader::new(&frame.payload);
    let packet = ClientPacket::decode_payload(packet_id, &mut reader)?;
    reader.finish(frame.packet_id)?;
    Ok(packet)
}

pub fn encode_client_packet(packet: &ClientPacket) -> Result<Vec<u8>> {
    let mut writer = PacketWriter::new();
    packet.encode_payload(&mut writer)?;
    encode_frame(packet.packet_id() as i16, &writer.into_inner())
}

pub fn decode_server_packet(frame_bytes: &[u8]) -> Result<ServerPacket> {
    let frame = decode_frame(frame_bytes)?;
    let packet_id = ServerPacketId::try_from(frame.packet_id)?;
    let mut reader = PacketReader::new(&frame.payload);
    let packet = ServerPacket::decode_payload(packet_id, &mut reader)?;
    reader.finish(frame.packet_id)?;
    Ok(packet)
}

pub fn encode_server_packet(packet: &ServerPacket) -> Result<Vec<u8>> {
    let mut writer = PacketWriter::new();
    packet.encode_payload(&mut writer)?;
    encode_frame(packet.packet_id() as i16, &writer.into_inner())
}

fn decode_select_info_vec(reader: &mut PacketReader<'_>) -> Result<Vec<SelectInfo>> {
    let count = reader.read_i32()?;
    if count < 0 {
        return Err(PacketCodecError::NegativeLength {
            field: "select_info_count",
            value: count,
        });
    }

    let mut values = Vec::with_capacity(count as usize);
    for _ in 0..count {
        values.push(SelectInfo::decode(reader)?);
    }
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_password_client_packets_round_trip_with_crystal_ids() {
        let packets = [
            ClientPacket::UnlockStorage {
                password: "vault".to_string(),
            },
            ClientPacket::SetStoragePassword {
                current_password: "vault".to_string(),
                new_password: "new-vault".to_string(),
            },
            ClientPacket::RemoveStoragePassword {
                current_password: "new-vault".to_string(),
            },
        ];

        assert_eq!(packets[0].packet_id() as i16, 150);
        assert_eq!(packets[1].packet_id() as i16, 151);
        assert_eq!(packets[2].packet_id() as i16, 152);

        for packet in packets {
            let encoded = encode_client_packet(&packet).expect("client packet should encode");
            let decoded = decode_client_packet(&encoded).expect("client packet should decode");
            assert_eq!(decoded, packet);
        }
    }

    #[test]
    fn storage_password_server_packets_round_trip_with_crystal_ids() {
        let packets = [
            ServerPacket::StorageUnlockResult {
                result: 0,
                has_password: true,
            },
            ServerPacket::StoragePasswordResult {
                result: 4,
                removing: false,
                has_password: true,
                last_set_binary_datetime: 638000000000000000,
            },
        ];

        assert_eq!(packets[0].packet_id() as i16, 277);
        assert_eq!(packets[1].packet_id() as i16, 278);

        for packet in packets {
            let encoded = encode_server_packet(&packet).expect("server packet should encode");
            let decoded = decode_server_packet(&encoded).expect("server packet should decode");
            assert_eq!(decoded, packet);
        }
    }

    #[test]
    fn resize_storage_server_packet_round_trip_with_crystal_id() {
        let packet = ServerPacket::ResizeStorage {
            size: 160,
            has_expanded_storage: true,
            expiry_time_binary_datetime: 638000000000000000,
        };

        assert_eq!(packet.packet_id() as i16, 238);

        let encoded = encode_server_packet(&packet).expect("server packet should encode");
        let decoded = decode_server_packet(&encoded).expect("server packet should decode");
        assert_eq!(decoded, packet);
    }
}
