use crate::error::{PacketCodecError, Result};
use crate::frame::{decode_frame, encode_frame};
use crate::ids::{ClientPacketId, ServerPacketId};
use crate::io::{PacketReader, PacketWriter};
use crate::types::{
    AwakeningMaterial, BaseStats, ChatItem, ChatType, ClientAuction, ClientBuff, ClientFriend,
    ClientGtMap, ClientHeroInformation, ClientIntelligentCreature, ClientMagic, ClientMail,
    ClientMapInfo, ClientQuestInfo, ClientRecipeInfo, GameShopItem, GuildBuff, GuildBuffInfo,
    GuildRank, GuildStorageItem, HeroUserInformation, ItemInfo, ItemRentalInformation,
    MapInformation, MirClass, MirDirection, MirGender, MirGridType, MonsterInfo, Notice, NpcInfo,
    ObjectAttackInfo, ObjectDiedInfo, ObjectEffectInfo, ObjectGoldInfo, ObjectHealthInfo,
    ObjectItemInfo, ObjectManaInfo, ObjectMovement, ObjectPlayerInfo, ObjectRangeAttackInfo,
    ObjectRevivedInfo, ObjectSpellInfo, ObjectStruckInfo, PlayerInspectInfo, Point,
    RankCharacterInfo, SelectInfo, Spell, StruckInfo, UserInformation, UserItem, UserLocation,
    WorldMapSetup,
};
use serde::Serialize;

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
        linked_items: Vec<ChatItem>,
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
    DepositRefineItem {
        from: i32,
        to: i32,
    },
    RetrieveRefineItem {
        from: i32,
        to: i32,
    },
    RefineCancel,
    RefineItem {
        unique_id: u64,
    },
    CheckRefine {
        unique_id: u64,
    },
    ReplaceWedRing {
        unique_id: u64,
    },
    DepositTradeItem {
        from: i32,
        to: i32,
    },
    RetrieveTradeItem {
        from: i32,
        to: i32,
    },
    TakeBackHeroItem {
        from: i32,
        to: i32,
    },
    TransferHeroItem {
        from: i32,
        to: i32,
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
    RequestMapInfo {
        map_index: i32,
    },
    RequestMonsterInfo {
        monster_index: i32,
    },
    RequestNpcInfo {
        npc_index: i32,
    },
    RequestItemInfo {
        item_index: i32,
    },
    TeleportToNpc {
        object_id: u32,
    },
    SearchMap {
        text: String,
    },
    Inspect {
        object_id: u32,
        ranking: bool,
        hero: bool,
    },
    Observe {
        name: String,
    },
    ChangeAMode {
        mode: u8,
    },
    ChangePMode {
        mode: u8,
    },
    ChangeTrade {
        allow_trade: bool,
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
    CallNpc {
        object_id: u32,
        key: String,
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
    CraftItem {
        unique_id: u64,
        count: u16,
        slots: Vec<i32>,
    },
    RepairItem {
        unique_id: u64,
    },
    BuyItemBack {
        unique_id: u64,
        count: u16,
    },
    SRepairItem {
        unique_id: u64,
    },
    MagicKey {
        spell: Spell,
        key: u8,
        old_key: u8,
    },
    Magic {
        object_id: u32,
        spell: Spell,
        direction: MirDirection,
        target_id: u32,
        location: Point,
        spell_target_lock: bool,
    },
    SwitchGroup {
        allow_group: bool,
    },
    AddMember {
        name: String,
    },
    DelMember {
        name: String,
    },
    GroupInvite {
        accept_invite: bool,
    },
    NewHero {
        name: String,
        gender: MirGender,
        class: MirClass,
    },
    SetAutoPotValue {
        stat: u8,
        value: u32,
    },
    SetAutoPotItem {
        grid: MirGridType,
        item_index: i32,
    },
    SetHeroBehaviour {
        behaviour: u8,
    },
    ChangeHero {
        list_index: i32,
    },
    TownRevive,
    SpellToggle {
        spell: Spell,
        toggle_state: i8,
    },
    ConsignItem {
        unique_id: u64,
        price: u32,
        market_type: u8,
    },
    MarketSearch {
        match_text: String,
        item_type: u8,
        user_mode: bool,
        min_shape: i16,
        max_shape: i16,
        market_type: u8,
    },
    MarketRefresh,
    MarketPage {
        page: i32,
    },
    MarketBuy {
        auction_id: u64,
        bid_price: u32,
    },
    MarketGetBack {
        mode: u8,
        auction_id: u64,
    },
    MarketSellNow {
        auction_id: u64,
    },
    RequestUserName {
        user_id: u32,
    },
    RequestChatItem {
        chat_item_id: u64,
    },
    EditGuildMember {
        change_type: u8,
        rank_index: u8,
        name: String,
        rank_name: String,
    },
    EditGuildNotice {
        notice: Vec<String>,
    },
    GuildInvite {
        accept_invite: bool,
    },
    GuildNameReturn {
        name: String,
    },
    RequestGuildInfo {
        info_type: u8,
    },
    GuildStorageGoldChange {
        change_type: u8,
        amount: u32,
    },
    GuildStorageItemChange {
        change_type: u8,
        from: i32,
        to: i32,
    },
    GuildWarReturn {
        name: String,
    },
    MarriageRequest,
    MarriageReply {
        accept_invite: bool,
    },
    ChangeMarriage,
    DivorceRequest,
    DivorceReply {
        accept_invite: bool,
    },
    AddMentor {
        name: String,
    },
    MentorReply {
        accept_invite: bool,
    },
    AllowMentor,
    CancelMentor,
    TradeRequest,
    TradeReply {
        accept_invite: bool,
    },
    TradeGold {
        amount: u32,
    },
    TradeConfirm {
        locked: bool,
    },
    TradeCancel,
    EquipSlotItem {
        grid: MirGridType,
        unique_id: u64,
        to: i32,
        grid_to: MirGridType,
        to_unique_id: u64,
    },
    FishingCast {
        cast_out: bool,
    },
    FishingChangeAutocast {
        auto_cast: bool,
    },
    AcceptQuest {
        npc_index: u32,
        quest_index: i32,
    },
    FinishQuest {
        quest_index: i32,
        selected_item_index: i32,
    },
    AbandonQuest {
        quest_index: i32,
    },
    ShareQuest {
        quest_index: i32,
    },
    AcceptReincarnation,
    CancelReincarnation,
    CombineItem {
        grid: MirGridType,
        id_from: u64,
        id_to: u64,
    },
    AwakeningNeedMaterials {
        unique_id: u64,
        awake_type: u8,
    },
    AwakeningLockedItem {
        unique_id: u64,
        locked: bool,
    },
    Awakening {
        unique_id: u64,
        awake_type: u8,
        position_index: u32,
    },
    DisassembleItem {
        unique_id: u64,
    },
    DowngradeAwakening {
        unique_id: u64,
    },
    ResetAddedItem {
        unique_id: u64,
    },
    SendMail {
        name: String,
        message: String,
        gold: u32,
        items_idx: [u64; 5],
        stamped: bool,
    },
    ReadMail {
        mail_id: u64,
    },
    CollectParcel {
        mail_id: u64,
    },
    DeleteMail {
        mail_id: u64,
    },
    LockMail {
        mail_id: u64,
        lock: bool,
    },
    MailLockedItem {
        unique_id: u64,
        locked: bool,
    },
    MailCost {
        gold: u32,
        items_idx: [u64; 5],
        stamped: bool,
    },
    UpdateIntelligentCreature {
        creature: ClientIntelligentCreature,
        summon_me: bool,
        unsummon_me: bool,
        release_me: bool,
    },
    IntelligentCreaturePickup {
        mouse_mode: bool,
        location: Point,
    },
    RequestIntelligentCreatureUpdates {
        update: bool,
    },
    AddFriend {
        name: String,
        blocked: bool,
    },
    RemoveFriend {
        character_index: i32,
    },
    RefreshFriends,
    AddMemo {
        character_index: i32,
        memo: String,
    },
    GuildBuffUpdate {
        action: u8,
        id: i32,
    },
    NpcConfirmInput {
        npc_id: u32,
        page_name: String,
        value: String,
    },
    GameShopBuy {
        g_index: i32,
        quantity: u8,
        price_type: i32,
    },
    ReportIssue {
        image: Vec<u8>,
        image_size: i32,
        image_chunk: i32,
    },
    GetRanking {
        rank_type: u8,
        rank_index: i32,
        online_only: bool,
    },
    OpenDoor {
        door_index: u8,
    },
    GetRentedItems,
    ItemRentalRequest,
    ItemRentalFee {
        amount: u32,
    },
    ItemRentalPeriod {
        days: u32,
    },
    DepositRentalItem {
        from: i32,
        to: i32,
    },
    RetrieveRentalItem {
        from: i32,
        to: i32,
    },
    CancelItemRental,
    ItemRentalLockFee,
    ItemRentalLockItem,
    ConfirmItemRental,
    GuildTerritoryPage {
        page: i32,
    },
    PurchaseGuildTerritory {
        owner: String,
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
            Self::DepositRefineItem { .. } => ClientPacketId::DepositRefineItem,
            Self::RetrieveRefineItem { .. } => ClientPacketId::RetrieveRefineItem,
            Self::RefineCancel => ClientPacketId::RefineCancel,
            Self::RefineItem { .. } => ClientPacketId::RefineItem,
            Self::CheckRefine { .. } => ClientPacketId::CheckRefine,
            Self::ReplaceWedRing { .. } => ClientPacketId::ReplaceWedRing,
            Self::DepositTradeItem { .. } => ClientPacketId::DepositTradeItem,
            Self::RetrieveTradeItem { .. } => ClientPacketId::RetrieveTradeItem,
            Self::TakeBackHeroItem { .. } => ClientPacketId::TakeBackHeroItem,
            Self::TransferHeroItem { .. } => ClientPacketId::TransferHeroItem,
            Self::DeleteItem { .. } => ClientPacketId::DeleteItem,
            Self::DropGold { .. } => ClientPacketId::DropGold,
            Self::PickUp => ClientPacketId::PickUp,
            Self::RequestMapInfo { .. } => ClientPacketId::RequestMapInfo,
            Self::RequestMonsterInfo { .. } => ClientPacketId::RequestMonsterInfo,
            Self::RequestNpcInfo { .. } => ClientPacketId::RequestNpcInfo,
            Self::RequestItemInfo { .. } => ClientPacketId::RequestItemInfo,
            Self::TeleportToNpc { .. } => ClientPacketId::TeleportToNpc,
            Self::SearchMap { .. } => ClientPacketId::SearchMap,
            Self::Inspect { .. } => ClientPacketId::Inspect,
            Self::Observe { .. } => ClientPacketId::Observe,
            Self::ChangeAMode { .. } => ClientPacketId::ChangeAMode,
            Self::ChangePMode { .. } => ClientPacketId::ChangePMode,
            Self::ChangeTrade { .. } => ClientPacketId::ChangeTrade,
            Self::Attack { .. } => ClientPacketId::Attack,
            Self::RangeAttack { .. } => ClientPacketId::RangeAttack,
            Self::Harvest { .. } => ClientPacketId::Harvest,
            Self::CallNpc { .. } => ClientPacketId::CallNpc,
            Self::BuyItem { .. } => ClientPacketId::BuyItem,
            Self::SellItem { .. } => ClientPacketId::SellItem,
            Self::CraftItem { .. } => ClientPacketId::CraftItem,
            Self::RepairItem { .. } => ClientPacketId::RepairItem,
            Self::BuyItemBack { .. } => ClientPacketId::BuyItemBack,
            Self::SRepairItem { .. } => ClientPacketId::SRepairItem,
            Self::MagicKey { .. } => ClientPacketId::MagicKey,
            Self::Magic { .. } => ClientPacketId::Magic,
            Self::SwitchGroup { .. } => ClientPacketId::SwitchGroup,
            Self::AddMember { .. } => ClientPacketId::AddMember,
            Self::DelMember { .. } => ClientPacketId::DelMember,
            Self::GroupInvite { .. } => ClientPacketId::GroupInvite,
            Self::NewHero { .. } => ClientPacketId::NewHero,
            Self::SetAutoPotValue { .. } => ClientPacketId::SetAutoPotValue,
            Self::SetAutoPotItem { .. } => ClientPacketId::SetAutoPotItem,
            Self::SetHeroBehaviour { .. } => ClientPacketId::SetHeroBehaviour,
            Self::ChangeHero { .. } => ClientPacketId::ChangeHero,
            Self::TownRevive => ClientPacketId::TownRevive,
            Self::SpellToggle { .. } => ClientPacketId::SpellToggle,
            Self::ConsignItem { .. } => ClientPacketId::ConsignItem,
            Self::MarketSearch { .. } => ClientPacketId::MarketSearch,
            Self::MarketRefresh => ClientPacketId::MarketRefresh,
            Self::MarketPage { .. } => ClientPacketId::MarketPage,
            Self::MarketBuy { .. } => ClientPacketId::MarketBuy,
            Self::MarketGetBack { .. } => ClientPacketId::MarketGetBack,
            Self::MarketSellNow { .. } => ClientPacketId::MarketSellNow,
            Self::RequestUserName { .. } => ClientPacketId::RequestUserName,
            Self::RequestChatItem { .. } => ClientPacketId::RequestChatItem,
            Self::EditGuildMember { .. } => ClientPacketId::EditGuildMember,
            Self::EditGuildNotice { .. } => ClientPacketId::EditGuildNotice,
            Self::GuildInvite { .. } => ClientPacketId::GuildInvite,
            Self::GuildNameReturn { .. } => ClientPacketId::GuildNameReturn,
            Self::RequestGuildInfo { .. } => ClientPacketId::RequestGuildInfo,
            Self::GuildStorageGoldChange { .. } => ClientPacketId::GuildStorageGoldChange,
            Self::GuildStorageItemChange { .. } => ClientPacketId::GuildStorageItemChange,
            Self::GuildWarReturn { .. } => ClientPacketId::GuildWarReturn,
            Self::MarriageRequest => ClientPacketId::MarriageRequest,
            Self::MarriageReply { .. } => ClientPacketId::MarriageReply,
            Self::ChangeMarriage => ClientPacketId::ChangeMarriage,
            Self::DivorceRequest => ClientPacketId::DivorceRequest,
            Self::DivorceReply { .. } => ClientPacketId::DivorceReply,
            Self::AddMentor { .. } => ClientPacketId::AddMentor,
            Self::MentorReply { .. } => ClientPacketId::MentorReply,
            Self::AllowMentor => ClientPacketId::AllowMentor,
            Self::CancelMentor => ClientPacketId::CancelMentor,
            Self::TradeRequest => ClientPacketId::TradeRequest,
            Self::TradeReply { .. } => ClientPacketId::TradeReply,
            Self::TradeGold { .. } => ClientPacketId::TradeGold,
            Self::TradeConfirm { .. } => ClientPacketId::TradeConfirm,
            Self::TradeCancel => ClientPacketId::TradeCancel,
            Self::EquipSlotItem { .. } => ClientPacketId::EquipSlotItem,
            Self::FishingCast { .. } => ClientPacketId::FishingCast,
            Self::FishingChangeAutocast { .. } => ClientPacketId::FishingChangeAutocast,
            Self::AcceptQuest { .. } => ClientPacketId::AcceptQuest,
            Self::FinishQuest { .. } => ClientPacketId::FinishQuest,
            Self::AbandonQuest { .. } => ClientPacketId::AbandonQuest,
            Self::ShareQuest { .. } => ClientPacketId::ShareQuest,
            Self::AcceptReincarnation => ClientPacketId::AcceptReincarnation,
            Self::CancelReincarnation => ClientPacketId::CancelReincarnation,
            Self::CombineItem { .. } => ClientPacketId::CombineItem,
            Self::AwakeningNeedMaterials { .. } => ClientPacketId::AwakeningNeedMaterials,
            Self::AwakeningLockedItem { .. } => ClientPacketId::AwakeningLockedItem,
            Self::Awakening { .. } => ClientPacketId::Awakening,
            Self::DisassembleItem { .. } => ClientPacketId::DisassembleItem,
            Self::DowngradeAwakening { .. } => ClientPacketId::DowngradeAwakening,
            Self::ResetAddedItem { .. } => ClientPacketId::ResetAddedItem,
            Self::SendMail { .. } => ClientPacketId::SendMail,
            Self::ReadMail { .. } => ClientPacketId::ReadMail,
            Self::CollectParcel { .. } => ClientPacketId::CollectParcel,
            Self::DeleteMail { .. } => ClientPacketId::DeleteMail,
            Self::LockMail { .. } => ClientPacketId::LockMail,
            Self::MailLockedItem { .. } => ClientPacketId::MailLockedItem,
            Self::MailCost { .. } => ClientPacketId::MailCost,
            Self::UpdateIntelligentCreature { .. } => ClientPacketId::UpdateIntelligentCreature,
            Self::IntelligentCreaturePickup { .. } => ClientPacketId::IntelligentCreaturePickup,
            Self::RequestIntelligentCreatureUpdates { .. } => {
                ClientPacketId::RequestIntelligentCreatureUpdates
            }
            Self::AddFriend { .. } => ClientPacketId::AddFriend,
            Self::RemoveFriend { .. } => ClientPacketId::RemoveFriend,
            Self::RefreshFriends => ClientPacketId::RefreshFriends,
            Self::AddMemo { .. } => ClientPacketId::AddMemo,
            Self::GuildBuffUpdate { .. } => ClientPacketId::GuildBuffUpdate,
            Self::NpcConfirmInput { .. } => ClientPacketId::NpcConfirmInput,
            Self::GameShopBuy { .. } => ClientPacketId::GameShopBuy,
            Self::ReportIssue { .. } => ClientPacketId::ReportIssue,
            Self::GetRanking { .. } => ClientPacketId::GetRanking,
            Self::OpenDoor { .. } => ClientPacketId::OpenDoor,
            Self::GetRentedItems => ClientPacketId::GetRentedItems,
            Self::ItemRentalRequest => ClientPacketId::ItemRentalRequest,
            Self::ItemRentalFee { .. } => ClientPacketId::ItemRentalFee,
            Self::ItemRentalPeriod { .. } => ClientPacketId::ItemRentalPeriod,
            Self::DepositRentalItem { .. } => ClientPacketId::DepositRentalItem,
            Self::RetrieveRentalItem { .. } => ClientPacketId::RetrieveRentalItem,
            Self::CancelItemRental => ClientPacketId::CancelItemRental,
            Self::ItemRentalLockFee => ClientPacketId::ItemRentalLockFee,
            Self::ItemRentalLockItem => ClientPacketId::ItemRentalLockItem,
            Self::ConfirmItemRental => ClientPacketId::ConfirmItemRental,
            Self::GuildTerritoryPage { .. } => ClientPacketId::GuildTerritoryPage,
            Self::PurchaseGuildTerritory { .. } => ClientPacketId::PurchaseGuildTerritory,
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
            }
            | Self::NewHero {
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
            Self::Chat {
                message,
                linked_items,
            } => {
                writer.write_string(message)?;
                writer.write_i32(linked_items.len() as i32);
                for item in linked_items {
                    item.encode(writer)?;
                }
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
            Self::SendMail {
                name,
                message,
                gold,
                items_idx,
                stamped,
            } => {
                writer.write_string(name)?;
                writer.write_string(message)?;
                writer.write_u32(*gold);
                for item_idx in items_idx {
                    writer.write_u64(*item_idx);
                }
                writer.write_bool(*stamped);
            }
            Self::ReadMail { mail_id }
            | Self::CollectParcel { mail_id }
            | Self::DeleteMail { mail_id } => writer.write_u64(*mail_id),
            Self::LockMail { mail_id, lock } => {
                writer.write_u64(*mail_id);
                writer.write_bool(*lock);
            }
            Self::MailLockedItem { unique_id, locked } => {
                writer.write_u64(*unique_id);
                writer.write_bool(*locked);
            }
            Self::MailCost {
                gold,
                items_idx,
                stamped,
            } => {
                writer.write_u32(*gold);
                for item_idx in items_idx {
                    writer.write_u64(*item_idx);
                }
                writer.write_bool(*stamped);
            }
            Self::UpdateIntelligentCreature {
                creature,
                summon_me,
                unsummon_me,
                release_me,
            } => {
                creature.encode(writer)?;
                writer.write_bool(*summon_me);
                writer.write_bool(*unsummon_me);
                writer.write_bool(*release_me);
            }
            Self::IntelligentCreaturePickup {
                mouse_mode,
                location,
            } => {
                writer.write_bool(*mouse_mode);
                location.encode(writer);
            }
            Self::RequestIntelligentCreatureUpdates { update } => writer.write_bool(*update),
            Self::AddFriend { name, blocked } => {
                writer.write_string(name)?;
                writer.write_bool(*blocked);
            }
            Self::RemoveFriend { character_index } => writer.write_i32(*character_index),
            Self::RefreshFriends => {}
            Self::AddMemo {
                character_index,
                memo,
            } => {
                writer.write_i32(*character_index);
                writer.write_string(memo)?;
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
            Self::DepositRefineItem { from, to } | Self::RetrieveRefineItem { from, to } => {
                writer.write_i32(*from);
                writer.write_i32(*to);
            }
            Self::RefineCancel => {}
            Self::RefineItem { unique_id }
            | Self::CheckRefine { unique_id }
            | Self::ReplaceWedRing { unique_id } => writer.write_u64(*unique_id),
            Self::DropGold { amount } => writer.write_u32(*amount),
            Self::DepositTradeItem { from, to }
            | Self::RetrieveTradeItem { from, to }
            | Self::TakeBackHeroItem { from, to }
            | Self::TransferHeroItem { from, to } => {
                writer.write_i32(*from);
                writer.write_i32(*to);
            }
            Self::PickUp => {}
            Self::RequestMapInfo { map_index } => writer.write_i32(*map_index),
            Self::RequestMonsterInfo { monster_index } => writer.write_i32(*monster_index),
            Self::RequestNpcInfo { npc_index } => writer.write_i32(*npc_index),
            Self::RequestItemInfo { item_index } => writer.write_i32(*item_index),
            Self::TeleportToNpc { object_id } => writer.write_u32(*object_id),
            Self::SearchMap { text } | Self::Observe { name: text } => {
                writer.write_string(text)?;
            }
            Self::Inspect {
                object_id,
                ranking,
                hero,
            } => {
                writer.write_u32(*object_id);
                writer.write_bool(*ranking);
                writer.write_bool(*hero);
            }
            Self::ChangeAMode { mode } | Self::ChangePMode { mode } => writer.write_u8(*mode),
            Self::ChangeTrade { allow_trade } => writer.write_bool(*allow_trade),
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
            Self::CallNpc { object_id, key } => {
                writer.write_u32(*object_id);
                writer.write_string(key)?;
            }
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
            Self::CraftItem {
                unique_id,
                count,
                slots,
            } => {
                writer.write_u64(*unique_id);
                writer.write_u16(*count);
                writer.write_i32(slots.len() as i32);
                for slot in slots {
                    writer.write_i32(*slot);
                }
            }
            Self::BuyItemBack { unique_id, count } => {
                writer.write_u64(*unique_id);
                writer.write_u16(*count);
            }
            Self::RepairItem { unique_id } | Self::SRepairItem { unique_id } => {
                writer.write_u64(*unique_id);
            }
            Self::MagicKey {
                spell,
                key,
                old_key,
            } => {
                writer.write_u8(*spell as u8);
                writer.write_u8(*key);
                writer.write_u8(*old_key);
            }
            Self::Magic {
                object_id,
                spell,
                direction,
                target_id,
                location,
                spell_target_lock,
            } => {
                writer.write_u32(*object_id);
                writer.write_u8(*spell as u8);
                writer.write_u8(*direction as u8);
                writer.write_u32(*target_id);
                location.encode(writer);
                writer.write_bool(*spell_target_lock);
            }
            Self::SwitchGroup { allow_group } => writer.write_bool(*allow_group),
            Self::AddMember { name } | Self::DelMember { name } => {
                writer.write_string(name)?;
            }
            Self::GroupInvite { accept_invite } => writer.write_bool(*accept_invite),
            Self::SetAutoPotValue { stat, value } => {
                writer.write_u8(*stat);
                writer.write_u32(*value);
            }
            Self::SetAutoPotItem { grid, item_index } => {
                writer.write_u8(*grid as u8);
                writer.write_i32(*item_index);
            }
            Self::SpellToggle {
                spell,
                toggle_state,
            } => {
                writer.write_u8(*spell as u8);
                writer.write_u8(*toggle_state as u8);
            }
            Self::SetHeroBehaviour { behaviour } => writer.write_u8(*behaviour),
            Self::ChangeHero { list_index } => writer.write_i32(*list_index),
            Self::TownRevive => {}
            Self::ConsignItem {
                unique_id,
                price,
                market_type,
            } => {
                writer.write_u64(*unique_id);
                writer.write_u32(*price);
                writer.write_u8(*market_type);
            }
            Self::MarketSearch {
                match_text,
                item_type,
                user_mode,
                min_shape,
                max_shape,
                market_type,
            } => {
                writer.write_string(match_text)?;
                writer.write_u8(*item_type);
                writer.write_bool(*user_mode);
                writer.write_i16(*min_shape);
                writer.write_i16(*max_shape);
                writer.write_u8(*market_type);
            }
            Self::MarketRefresh => {}
            Self::MarketPage { page } => writer.write_i32(*page),
            Self::MarketBuy {
                auction_id,
                bid_price,
            } => {
                writer.write_u64(*auction_id);
                writer.write_u32(*bid_price);
            }
            Self::MarketGetBack { mode, auction_id } => {
                writer.write_u8(*mode);
                writer.write_u64(*auction_id);
            }
            Self::MarketSellNow { auction_id } => writer.write_u64(*auction_id),
            Self::RequestUserName { user_id } => writer.write_u32(*user_id),
            Self::RequestChatItem { chat_item_id } => writer.write_u64(*chat_item_id),
            Self::EditGuildMember {
                change_type,
                rank_index,
                name,
                rank_name,
            } => {
                writer.write_u8(*change_type);
                writer.write_u8(*rank_index);
                writer.write_string(name)?;
                writer.write_string(rank_name)?;
            }
            Self::EditGuildNotice { notice } => {
                writer.write_i32(notice.len() as i32);
                for line in notice {
                    writer.write_string(line)?;
                }
            }
            Self::GuildInvite { accept_invite } => writer.write_bool(*accept_invite),
            Self::GuildNameReturn { name } | Self::GuildWarReturn { name } => {
                writer.write_string(name)?;
            }
            Self::RequestGuildInfo { info_type } => writer.write_u8(*info_type),
            Self::GuildStorageGoldChange {
                change_type,
                amount,
            } => {
                writer.write_u8(*change_type);
                writer.write_u32(*amount);
            }
            Self::GuildStorageItemChange {
                change_type,
                from,
                to,
            } => {
                writer.write_u8(*change_type);
                writer.write_i32(*from);
                writer.write_i32(*to);
            }
            Self::MarriageRequest
            | Self::ChangeMarriage
            | Self::DivorceRequest
            | Self::AllowMentor
            | Self::CancelMentor
            | Self::TradeRequest
            | Self::TradeCancel => {}
            Self::MarriageReply { accept_invite }
            | Self::DivorceReply { accept_invite }
            | Self::MentorReply { accept_invite }
            | Self::TradeReply { accept_invite } => writer.write_bool(*accept_invite),
            Self::AddMentor { name } => writer.write_string(name)?,
            Self::TradeGold { amount } => writer.write_u32(*amount),
            Self::TradeConfirm { locked } => writer.write_bool(*locked),
            Self::EquipSlotItem {
                grid,
                unique_id,
                to,
                grid_to,
                to_unique_id,
            } => {
                writer.write_u8(*grid as u8);
                writer.write_u64(*unique_id);
                writer.write_i32(*to);
                writer.write_u8(*grid_to as u8);
                writer.write_u64(*to_unique_id);
            }
            Self::FishingCast { cast_out } => writer.write_bool(*cast_out),
            Self::FishingChangeAutocast { auto_cast } => writer.write_bool(*auto_cast),
            Self::AcceptQuest {
                npc_index,
                quest_index,
            } => {
                writer.write_u32(*npc_index);
                writer.write_i32(*quest_index);
            }
            Self::FinishQuest {
                quest_index,
                selected_item_index,
            } => {
                writer.write_i32(*quest_index);
                writer.write_i32(*selected_item_index);
            }
            Self::AbandonQuest { quest_index } | Self::ShareQuest { quest_index } => {
                writer.write_i32(*quest_index);
            }
            Self::AcceptReincarnation | Self::CancelReincarnation => {}
            Self::GetRentedItems | Self::ItemRentalRequest => {}
            Self::AwakeningNeedMaterials {
                unique_id,
                awake_type,
            } => {
                writer.write_u64(*unique_id);
                writer.write_u8(*awake_type);
            }
            Self::AwakeningLockedItem { unique_id, locked } => {
                writer.write_u64(*unique_id);
                writer.write_bool(*locked);
            }
            Self::Awakening {
                unique_id,
                awake_type,
                position_index,
            } => {
                writer.write_u64(*unique_id);
                writer.write_u8(*awake_type);
                writer.write_u32(*position_index);
            }
            Self::DisassembleItem { unique_id }
            | Self::DowngradeAwakening { unique_id }
            | Self::ResetAddedItem { unique_id } => writer.write_u64(*unique_id),
            Self::ItemRentalFee { amount } => writer.write_u32(*amount),
            Self::ItemRentalPeriod { days } => writer.write_u32(*days),
            Self::DepositRentalItem { from, to } | Self::RetrieveRentalItem { from, to } => {
                writer.write_i32(*from);
                writer.write_i32(*to);
            }
            Self::CancelItemRental
            | Self::ItemRentalLockFee
            | Self::ItemRentalLockItem
            | Self::ConfirmItemRental => {}
            Self::GuildBuffUpdate { action, id } => {
                writer.write_u8(*action);
                writer.write_i32(*id);
            }
            Self::NpcConfirmInput {
                npc_id,
                page_name,
                value,
            } => {
                writer.write_u32(*npc_id);
                writer.write_string(page_name)?;
                writer.write_string(value)?;
            }
            Self::GameShopBuy {
                g_index,
                quantity,
                price_type,
            } => {
                writer.write_i32(*g_index);
                writer.write_u8(*quantity);
                writer.write_i32(*price_type);
            }
            Self::ReportIssue {
                image,
                image_size,
                image_chunk,
            } => {
                writer.write_i32(image.len() as i32);
                writer.write_bytes(image);
                writer.write_i32(*image_size);
                writer.write_i32(*image_chunk);
            }
            Self::GetRanking {
                rank_type,
                rank_index,
                online_only,
            } => {
                writer.write_u8(*rank_type);
                writer.write_i32(*rank_index);
                writer.write_bool(*online_only);
            }
            Self::OpenDoor { door_index } => writer.write_u8(*door_index),
            Self::GuildTerritoryPage { page } => writer.write_i32(*page),
            Self::PurchaseGuildTerritory { owner } => writer.write_string(owner)?,
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
                if linked_item_count < 0 {
                    return Err(PacketCodecError::NegativeLength {
                        field: "chat_linked_items",
                        value: linked_item_count,
                    });
                }
                let mut linked_items = Vec::with_capacity(linked_item_count as usize);
                for _ in 0..linked_item_count {
                    linked_items.push(ChatItem::decode(reader)?);
                }

                Self::Chat {
                    message,
                    linked_items,
                }
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
            ClientPacketId::SendMail => Self::SendMail {
                name: reader.read_string()?,
                message: reader.read_string()?,
                gold: reader.read_u32()?,
                items_idx: [
                    reader.read_u64()?,
                    reader.read_u64()?,
                    reader.read_u64()?,
                    reader.read_u64()?,
                    reader.read_u64()?,
                ],
                stamped: reader.read_bool()?,
            },
            ClientPacketId::ReadMail => Self::ReadMail {
                mail_id: reader.read_u64()?,
            },
            ClientPacketId::CollectParcel => Self::CollectParcel {
                mail_id: reader.read_u64()?,
            },
            ClientPacketId::DeleteMail => Self::DeleteMail {
                mail_id: reader.read_u64()?,
            },
            ClientPacketId::LockMail => Self::LockMail {
                mail_id: reader.read_u64()?,
                lock: reader.read_bool()?,
            },
            ClientPacketId::MailLockedItem => Self::MailLockedItem {
                unique_id: reader.read_u64()?,
                locked: reader.read_bool()?,
            },
            ClientPacketId::MailCost => Self::MailCost {
                gold: reader.read_u32()?,
                items_idx: [
                    reader.read_u64()?,
                    reader.read_u64()?,
                    reader.read_u64()?,
                    reader.read_u64()?,
                    reader.read_u64()?,
                ],
                stamped: reader.read_bool()?,
            },
            ClientPacketId::UpdateIntelligentCreature => Self::UpdateIntelligentCreature {
                creature: ClientIntelligentCreature::decode(reader)?,
                summon_me: reader.read_bool()?,
                unsummon_me: reader.read_bool()?,
                release_me: reader.read_bool()?,
            },
            ClientPacketId::IntelligentCreaturePickup => Self::IntelligentCreaturePickup {
                mouse_mode: reader.read_bool()?,
                location: Point::decode(reader)?,
            },
            ClientPacketId::RequestIntelligentCreatureUpdates => {
                Self::RequestIntelligentCreatureUpdates {
                    update: reader.read_bool()?,
                }
            }
            ClientPacketId::AddFriend => Self::AddFriend {
                name: reader.read_string()?,
                blocked: reader.read_bool()?,
            },
            ClientPacketId::RemoveFriend => Self::RemoveFriend {
                character_index: reader.read_i32()?,
            },
            ClientPacketId::RefreshFriends => Self::RefreshFriends,
            ClientPacketId::AddMemo => Self::AddMemo {
                character_index: reader.read_i32()?,
                memo: reader.read_string()?,
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
            ClientPacketId::DepositRefineItem => Self::DepositRefineItem {
                from: reader.read_i32()?,
                to: reader.read_i32()?,
            },
            ClientPacketId::RetrieveRefineItem => Self::RetrieveRefineItem {
                from: reader.read_i32()?,
                to: reader.read_i32()?,
            },
            ClientPacketId::RefineCancel => Self::RefineCancel,
            ClientPacketId::RefineItem => Self::RefineItem {
                unique_id: reader.read_u64()?,
            },
            ClientPacketId::CheckRefine => Self::CheckRefine {
                unique_id: reader.read_u64()?,
            },
            ClientPacketId::ReplaceWedRing => Self::ReplaceWedRing {
                unique_id: reader.read_u64()?,
            },
            ClientPacketId::DeleteItem => Self::DeleteItem {
                unique_id: reader.read_u64()?,
                count: reader.read_u16()?,
                hero_inventory: reader.read_bool()?,
            },
            ClientPacketId::DropGold => Self::DropGold {
                amount: reader.read_u32()?,
            },
            ClientPacketId::DepositTradeItem => Self::DepositTradeItem {
                from: reader.read_i32()?,
                to: reader.read_i32()?,
            },
            ClientPacketId::RetrieveTradeItem => Self::RetrieveTradeItem {
                from: reader.read_i32()?,
                to: reader.read_i32()?,
            },
            ClientPacketId::TakeBackHeroItem => Self::TakeBackHeroItem {
                from: reader.read_i32()?,
                to: reader.read_i32()?,
            },
            ClientPacketId::TransferHeroItem => Self::TransferHeroItem {
                from: reader.read_i32()?,
                to: reader.read_i32()?,
            },
            ClientPacketId::PickUp => Self::PickUp,
            ClientPacketId::RequestMapInfo => Self::RequestMapInfo {
                map_index: reader.read_i32()?,
            },
            ClientPacketId::RequestMonsterInfo => Self::RequestMonsterInfo {
                monster_index: reader.read_i32()?,
            },
            ClientPacketId::RequestNpcInfo => Self::RequestNpcInfo {
                npc_index: reader.read_i32()?,
            },
            ClientPacketId::RequestItemInfo => Self::RequestItemInfo {
                item_index: reader.read_i32()?,
            },
            ClientPacketId::TeleportToNpc => Self::TeleportToNpc {
                object_id: reader.read_u32()?,
            },
            ClientPacketId::SearchMap => Self::SearchMap {
                text: reader.read_string()?,
            },
            ClientPacketId::Inspect => Self::Inspect {
                object_id: reader.read_u32()?,
                ranking: reader.read_bool()?,
                hero: reader.read_bool()?,
            },
            ClientPacketId::Observe => Self::Observe {
                name: reader.read_string()?,
            },
            ClientPacketId::ChangeAMode => Self::ChangeAMode {
                mode: reader.read_u8()?,
            },
            ClientPacketId::ChangePMode => Self::ChangePMode {
                mode: reader.read_u8()?,
            },
            ClientPacketId::ChangeTrade => Self::ChangeTrade {
                allow_trade: reader.read_bool()?,
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
            ClientPacketId::CallNpc => Self::CallNpc {
                object_id: reader.read_u32()?,
                key: reader.read_string()?,
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
            ClientPacketId::CraftItem => {
                let unique_id = reader.read_u64()?;
                let count = reader.read_u16()?;
                let slot_count = reader.read_i32()?;
                if slot_count < 0 {
                    return Err(PacketCodecError::NegativeLength {
                        field: "slots",
                        value: slot_count,
                    });
                }
                let mut slots = Vec::with_capacity(slot_count as usize);
                for _ in 0..slot_count {
                    slots.push(reader.read_i32()?);
                }
                Self::CraftItem {
                    unique_id,
                    count,
                    slots,
                }
            }
            ClientPacketId::RepairItem => Self::RepairItem {
                unique_id: reader.read_u64()?,
            },
            ClientPacketId::BuyItemBack => Self::BuyItemBack {
                unique_id: reader.read_u64()?,
                count: reader.read_u16()?,
            },
            ClientPacketId::SRepairItem => Self::SRepairItem {
                unique_id: reader.read_u64()?,
            },
            ClientPacketId::MagicKey => Self::MagicKey {
                spell: Spell::try_from(reader.read_u8()?)?,
                key: reader.read_u8()?,
                old_key: reader.read_u8()?,
            },
            ClientPacketId::Magic => Self::Magic {
                object_id: reader.read_u32()?,
                spell: Spell::try_from(reader.read_u8()?)?,
                direction: MirDirection::try_from(reader.read_u8()?)?,
                target_id: reader.read_u32()?,
                location: Point::decode(reader)?,
                spell_target_lock: reader.read_bool()?,
            },
            ClientPacketId::SwitchGroup => Self::SwitchGroup {
                allow_group: reader.read_bool()?,
            },
            ClientPacketId::AddMember => Self::AddMember {
                name: reader.read_string()?,
            },
            ClientPacketId::DelMember => Self::DelMember {
                name: reader.read_string()?,
            },
            ClientPacketId::GroupInvite => Self::GroupInvite {
                accept_invite: reader.read_bool()?,
            },
            ClientPacketId::NewHero => Self::NewHero {
                name: reader.read_string()?,
                gender: MirGender::try_from(reader.read_u8()?)?,
                class: MirClass::try_from(reader.read_u8()?)?,
            },
            ClientPacketId::SetAutoPotValue => Self::SetAutoPotValue {
                stat: reader.read_u8()?,
                value: reader.read_u32()?,
            },
            ClientPacketId::SetAutoPotItem => Self::SetAutoPotItem {
                grid: MirGridType::try_from(reader.read_u8()?)?,
                item_index: reader.read_i32()?,
            },
            ClientPacketId::SetHeroBehaviour => Self::SetHeroBehaviour {
                behaviour: reader.read_u8()?,
            },
            ClientPacketId::ChangeHero => Self::ChangeHero {
                list_index: reader.read_i32()?,
            },
            ClientPacketId::TownRevive => Self::TownRevive,
            ClientPacketId::SpellToggle => Self::SpellToggle {
                spell: Spell::try_from(reader.read_u8()?)?,
                toggle_state: reader.read_u8()? as i8,
            },
            ClientPacketId::ConsignItem => Self::ConsignItem {
                unique_id: reader.read_u64()?,
                price: reader.read_u32()?,
                market_type: reader.read_u8()?,
            },
            ClientPacketId::MarketSearch => Self::MarketSearch {
                match_text: reader.read_string()?,
                item_type: reader.read_u8()?,
                user_mode: reader.read_bool()?,
                min_shape: reader.read_i16()?,
                max_shape: reader.read_i16()?,
                market_type: reader.read_u8()?,
            },
            ClientPacketId::MarketRefresh => Self::MarketRefresh,
            ClientPacketId::MarketPage => Self::MarketPage {
                page: reader.read_i32()?,
            },
            ClientPacketId::MarketBuy => Self::MarketBuy {
                auction_id: reader.read_u64()?,
                bid_price: reader.read_u32()?,
            },
            ClientPacketId::MarketGetBack => Self::MarketGetBack {
                mode: reader.read_u8()?,
                auction_id: reader.read_u64()?,
            },
            ClientPacketId::MarketSellNow => Self::MarketSellNow {
                auction_id: reader.read_u64()?,
            },
            ClientPacketId::RequestUserName => Self::RequestUserName {
                user_id: reader.read_u32()?,
            },
            ClientPacketId::RequestChatItem => Self::RequestChatItem {
                chat_item_id: reader.read_u64()?,
            },
            ClientPacketId::EditGuildMember => Self::EditGuildMember {
                change_type: reader.read_u8()?,
                rank_index: reader.read_u8()?,
                name: reader.read_string()?,
                rank_name: reader.read_string()?,
            },
            ClientPacketId::EditGuildNotice => {
                let line_count = reader.read_i32()?;
                if line_count < 0 {
                    return Err(PacketCodecError::NegativeLength {
                        field: "notice",
                        value: line_count,
                    });
                }
                let mut notice = Vec::with_capacity(line_count as usize);
                for _ in 0..line_count {
                    notice.push(reader.read_string()?);
                }
                Self::EditGuildNotice { notice }
            }
            ClientPacketId::GuildInvite => Self::GuildInvite {
                accept_invite: reader.read_bool()?,
            },
            ClientPacketId::GuildNameReturn => Self::GuildNameReturn {
                name: reader.read_string()?,
            },
            ClientPacketId::RequestGuildInfo => Self::RequestGuildInfo {
                info_type: reader.read_u8()?,
            },
            ClientPacketId::GuildStorageGoldChange => Self::GuildStorageGoldChange {
                change_type: reader.read_u8()?,
                amount: reader.read_u32()?,
            },
            ClientPacketId::GuildStorageItemChange => Self::GuildStorageItemChange {
                change_type: reader.read_u8()?,
                from: reader.read_i32()?,
                to: reader.read_i32()?,
            },
            ClientPacketId::GuildWarReturn => Self::GuildWarReturn {
                name: reader.read_string()?,
            },
            ClientPacketId::MarriageRequest => Self::MarriageRequest,
            ClientPacketId::MarriageReply => Self::MarriageReply {
                accept_invite: reader.read_bool()?,
            },
            ClientPacketId::ChangeMarriage => Self::ChangeMarriage,
            ClientPacketId::DivorceRequest => Self::DivorceRequest,
            ClientPacketId::DivorceReply => Self::DivorceReply {
                accept_invite: reader.read_bool()?,
            },
            ClientPacketId::AddMentor => Self::AddMentor {
                name: reader.read_string()?,
            },
            ClientPacketId::MentorReply => Self::MentorReply {
                accept_invite: reader.read_bool()?,
            },
            ClientPacketId::AllowMentor => Self::AllowMentor,
            ClientPacketId::CancelMentor => Self::CancelMentor,
            ClientPacketId::TradeRequest => Self::TradeRequest,
            ClientPacketId::TradeReply => Self::TradeReply {
                accept_invite: reader.read_bool()?,
            },
            ClientPacketId::TradeGold => Self::TradeGold {
                amount: reader.read_u32()?,
            },
            ClientPacketId::TradeConfirm => Self::TradeConfirm {
                locked: reader.read_bool()?,
            },
            ClientPacketId::TradeCancel => Self::TradeCancel,
            ClientPacketId::EquipSlotItem => Self::EquipSlotItem {
                grid: MirGridType::try_from(reader.read_u8()?)?,
                unique_id: reader.read_u64()?,
                to: reader.read_i32()?,
                grid_to: MirGridType::try_from(reader.read_u8()?)?,
                to_unique_id: reader.read_u64()?,
            },
            ClientPacketId::FishingCast => Self::FishingCast {
                cast_out: reader.read_bool()?,
            },
            ClientPacketId::FishingChangeAutocast => Self::FishingChangeAutocast {
                auto_cast: reader.read_bool()?,
            },
            ClientPacketId::AcceptQuest => Self::AcceptQuest {
                npc_index: reader.read_u32()?,
                quest_index: reader.read_i32()?,
            },
            ClientPacketId::FinishQuest => Self::FinishQuest {
                quest_index: reader.read_i32()?,
                selected_item_index: reader.read_i32()?,
            },
            ClientPacketId::AbandonQuest => Self::AbandonQuest {
                quest_index: reader.read_i32()?,
            },
            ClientPacketId::ShareQuest => Self::ShareQuest {
                quest_index: reader.read_i32()?,
            },
            ClientPacketId::AcceptReincarnation => Self::AcceptReincarnation,
            ClientPacketId::CancelReincarnation => Self::CancelReincarnation,
            ClientPacketId::CombineItem => Self::CombineItem {
                grid: MirGridType::try_from(reader.read_u8()?)?,
                id_from: reader.read_u64()?,
                id_to: reader.read_u64()?,
            },
            ClientPacketId::AwakeningNeedMaterials => Self::AwakeningNeedMaterials {
                unique_id: reader.read_u64()?,
                awake_type: reader.read_u8()?,
            },
            ClientPacketId::AwakeningLockedItem => Self::AwakeningLockedItem {
                unique_id: reader.read_u64()?,
                locked: reader.read_bool()?,
            },
            ClientPacketId::Awakening => Self::Awakening {
                unique_id: reader.read_u64()?,
                awake_type: reader.read_u8()?,
                position_index: reader.read_u32()?,
            },
            ClientPacketId::DisassembleItem => Self::DisassembleItem {
                unique_id: reader.read_u64()?,
            },
            ClientPacketId::DowngradeAwakening => Self::DowngradeAwakening {
                unique_id: reader.read_u64()?,
            },
            ClientPacketId::ResetAddedItem => Self::ResetAddedItem {
                unique_id: reader.read_u64()?,
            },
            ClientPacketId::GetRentedItems => Self::GetRentedItems,
            ClientPacketId::ItemRentalRequest => Self::ItemRentalRequest,
            ClientPacketId::ItemRentalFee => Self::ItemRentalFee {
                amount: reader.read_u32()?,
            },
            ClientPacketId::ItemRentalPeriod => Self::ItemRentalPeriod {
                days: reader.read_u32()?,
            },
            ClientPacketId::DepositRentalItem => Self::DepositRentalItem {
                from: reader.read_i32()?,
                to: reader.read_i32()?,
            },
            ClientPacketId::RetrieveRentalItem => Self::RetrieveRentalItem {
                from: reader.read_i32()?,
                to: reader.read_i32()?,
            },
            ClientPacketId::CancelItemRental => Self::CancelItemRental,
            ClientPacketId::ItemRentalLockFee => Self::ItemRentalLockFee,
            ClientPacketId::ItemRentalLockItem => Self::ItemRentalLockItem,
            ClientPacketId::ConfirmItemRental => Self::ConfirmItemRental,
            ClientPacketId::GuildBuffUpdate => Self::GuildBuffUpdate {
                action: reader.read_u8()?,
                id: reader.read_i32()?,
            },
            ClientPacketId::NpcConfirmInput => Self::NpcConfirmInput {
                npc_id: reader.read_u32()?,
                page_name: reader.read_string()?,
                value: reader.read_string()?,
            },
            ClientPacketId::GameShopBuy => Self::GameShopBuy {
                g_index: reader.read_i32()?,
                quantity: reader.read_u8()?,
                price_type: reader.read_i32()?,
            },
            ClientPacketId::ReportIssue => {
                let image_len = reader.read_i32()?;
                if image_len < 0 {
                    return Err(PacketCodecError::NegativeLength {
                        field: "image",
                        value: image_len,
                    });
                }
                Self::ReportIssue {
                    image: reader.read_bytes(image_len as usize)?,
                    image_size: reader.read_i32()?,
                    image_chunk: reader.read_i32()?,
                }
            }
            ClientPacketId::GetRanking => Self::GetRanking {
                rank_type: reader.read_u8()?,
                rank_index: reader.read_i32()?,
                online_only: reader.read_bool()?,
            },
            ClientPacketId::OpenDoor => Self::OpenDoor {
                door_index: reader.read_u8()?,
            },
            ClientPacketId::GuildTerritoryPage => Self::GuildTerritoryPage {
                page: reader.read_i32()?,
            },
            ClientPacketId::PurchaseGuildTerritory => Self::PurchaseGuildTerritory {
                owner: reader.read_string()?,
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

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all_fields = "camelCase")]
pub enum ServerPacket {
    Raw {
        packet_id: ServerPacketId,
        payload: Vec<u8>,
    },
    TimeOfDay {
        lights: u8,
    },
    ChangeAMode {
        mode: u8,
    },
    ChangePMode {
        mode: u8,
    },
    BaseStatsInfo {
        stats: BaseStats,
    },
    HeroBaseStatsInfo {
        stats: BaseStats,
    },
    HeroInformation {
        info: HeroUserInformation,
    },
    NPCMarket {
        listings: Vec<ClientAuction>,
        pages: i32,
        user_mode: bool,
    },
    NPCMarketPage {
        listings: Vec<ClientAuction>,
    },
    NewMapInfo {
        map_index: i32,
        info: ClientMapInfo,
    },
    WorldMapSetup {
        setup: WorldMapSetup,
        teleport_to_npc_cost: i32,
    },
    SearchMapResult {
        map_index: i32,
        npc_index: u32,
    },
    UserSlotsRefresh {
        inventory: Option<Vec<Option<UserItem>>>,
        equipment: Option<Vec<Option<UserItem>>>,
    },
    NewChatItem {
        item: UserItem,
    },
    PlayerUpdate {
        object_id: u32,
        light: u8,
        weapon: i16,
        weapon_effect: i16,
        armour: i16,
        wing_effect: u8,
    },
    PlayerInspect {
        info: PlayerInspectInfo,
    },
    ReturnToLogin,
    DamageIndicator {
        damage: i32,
        damage_type: u8,
        object_id: u32,
    },
    HealthChanged {
        hp: i32,
        mp: i32,
    },
    Death {
        location: Point,
        direction: MirDirection,
    },
    ColourChanged {
        name_colour_argb: i32,
    },
    ObjectColourChanged {
        object_id: u32,
        name_colour_argb: i32,
    },
    ObjectGuildNameChanged {
        object_id: u32,
        guild_name: String,
    },
    GainExperience {
        amount: u32,
    },
    LevelChanged {
        level: u16,
        experience: i64,
        max_experience: i64,
    },
    ObjectLeveled {
        object_id: u32,
    },
    Poisoned {
        poison: u16,
    },
    ObjectPoisoned {
        object_id: u32,
        poison: u16,
    },
    MapChanged {
        map_index: i32,
        file_name: String,
        title: String,
        mini_map: u16,
        big_map: u16,
        lights: u8,
        location: Point,
        direction: MirDirection,
        map_dark_light: u8,
        music: u16,
        weather: u16,
    },
    ObjectName {
        object_id: u32,
        name: String,
    },
    Revived,
    NPCConsign,
    InTrapRock {
        trapped: bool,
    },
    UserName {
        id: u32,
        name: String,
    },
    ChatItemStats {
        chat_item_id: u64,
        stats: Option<UserItem>,
    },
    GuildNoticeChange {
        update: i32,
        notice: Vec<String>,
    },
    GuildMemberChange {
        name: String,
        rank_index: u8,
        status: u8,
        ranks: Vec<GuildRank>,
    },
    GuildStatus {
        guild_name: String,
        guild_rank_name: String,
        level: u8,
        experience: i64,
        max_experience: i64,
        gold: u32,
        spare_points: u8,
        member_count: i32,
        max_members: i32,
        voting: bool,
        item_count: u8,
        buff_count: u8,
        my_options: u8,
        my_rank_id: i32,
    },
    GuildInvite {
        name: String,
    },
    GuildExpGain {
        amount: u32,
    },
    GuildNameRequest,
    GuildStorageGoldChange {
        amount: u32,
        change_type: u8,
        name: String,
    },
    GuildStorageItemChange {
        change_type: u8,
        to: i32,
        from: i32,
        user: i32,
        item: Option<GuildStorageItem>,
    },
    GuildStorageList {
        items: Vec<Option<GuildStorageItem>>,
    },
    GuildRequestWar,
    SetAutoPotValue {
        stat: u8,
        value: u32,
    },
    SetAutoPotItem {
        grid: u8,
        item_index: i32,
    },
    NPCImageUpdate {
        object_id: u32,
        image: u16,
        colour_argb: i32,
    },
    EquipSlotItem {
        grid: MirGridType,
        unique_id: u64,
        to: i32,
        grid_to: MirGridType,
        success: bool,
    },
    GainedQuestItem {
        item: UserItem,
    },
    DeleteQuestItem {
        unique_id: u64,
        count: u16,
    },
    CancelReincarnation,
    RequestReincarnation,
    UserBackStep {
        location: Point,
        direction: MirDirection,
    },
    UserDashAttack {
        location: Point,
        direction: MirDirection,
    },
    ObjectDashAttack {
        object_id: u32,
        location: Point,
        direction: MirDirection,
        distance: i32,
    },
    UserAttackMove {
        location: Point,
        direction: MirDirection,
    },
    SetConcentration {
        object_id: u32,
        enabled: bool,
        interrupted: bool,
    },
    SetElemental {
        object_id: u32,
        enabled: bool,
        casted: bool,
        value: u32,
        element_type: u32,
        exp_last: u32,
    },
    AwakeningNeedMaterials {
        materials: Option<Vec<Option<AwakeningMaterial>>>,
    },
    NPCPearlGoods {
        list: Vec<UserItem>,
        rate: f32,
        panel_type: u8,
    },
    TransformUpdate {
        object_id: u32,
        transform_type: i16,
    },
    NPCRequestInput {
        npc_id: u32,
        page_name: String,
    },
    GameShopStock {
        g_index: i32,
        stock_level: i32,
    },
    Rankings {
        rank_type: u8,
        my_rank: i32,
        listing_details: Vec<RankCharacterInfo>,
        listings: Vec<i64>,
        count: i32,
    },
    UpdateNotice {
        notice: Notice,
    },
    GuildTerritoryPage {
        length: i32,
        listings: Vec<ClientGtMap>,
    },
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
    ObjectHero {
        info: ObjectPlayerInfo,
        owner_name: String,
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
    DepositRefineItem {
        from: i32,
        to: i32,
        success: bool,
    },
    RetrieveRefineItem {
        from: i32,
        to: i32,
        success: bool,
    },
    RefineCancel,
    RefineItem {
        unique_id: u64,
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
    DepositTradeItem {
        from: i32,
        to: i32,
        success: bool,
    },
    RetrieveTradeItem {
        from: i32,
        to: i32,
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
    TakeBackHeroItem {
        from: i32,
        to: i32,
        success: bool,
    },
    TransferHeroItem {
        from: i32,
        to: i32,
        success: bool,
    },
    NewMonsterInfo {
        info: MonsterInfo,
    },
    NewNpcInfo {
        info: NpcInfo,
    },
    NewHeroInfo {
        info: ClientHeroInformation,
        storage_index: i32,
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
    HeroHealthChanged {
        hp: i32,
        mp: i32,
    },
    DeleteItem {
        unique_id: u64,
        count: u16,
    },
    ObjectDied {
        info: ObjectDiedInfo,
    },
    GainHeroExperience {
        amount: u32,
    },
    HeroLevelChanged {
        level: u16,
        experience: i64,
        max_experience: i64,
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
    NPCResponse {
        page: Vec<String>,
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
    NewMagic {
        magic: ClientMagic,
        hero: bool,
    },
    RemoveMagic {
        place_id: i32,
    },
    MagicLeveled {
        object_id: u32,
        spell: Spell,
        level: u8,
        experience: u16,
    },
    Magic {
        spell: Spell,
        target_id: u32,
        target: Point,
        cast: bool,
        level: u8,
        secondary_target_ids: Vec<u32>,
    },
    MagicDelay {
        object_id: u32,
        spell: Spell,
        delay: i64,
    },
    MagicCast {
        spell: Spell,
    },
    ObjectMagic {
        object_id: u32,
        location: Point,
        direction: MirDirection,
        spell: Spell,
        target_id: u32,
        target: Point,
        cast: bool,
        level: u8,
        self_broadcast: bool,
        secondary_target_ids: Vec<u32>,
    },
    ObjectProjectile {
        spell: Spell,
        source_id: u32,
        destination_id: u32,
    },
    RangeAttack {
        target_id: u32,
        target: Point,
        spell: Spell,
    },
    Pushed {
        location: Point,
        direction: MirDirection,
    },
    ObjectPushed {
        object_id: u32,
        location: Point,
        direction: MirDirection,
    },
    SpellToggle {
        object_id: u32,
        spell: Spell,
        can_use: bool,
    },
    SwitchGroup {
        allow_group: bool,
    },
    DeleteGroup,
    DeleteMember {
        name: String,
    },
    GroupInvite {
        name: String,
    },
    AddMember {
        name: String,
    },
    GroupMembersMap {
        player_name: String,
        player_map: String,
    },
    SendMemberLocation {
        member_name: String,
        member_location: Point,
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
    ObjectMana {
        info: ObjectManaInfo,
    },
    MapEffect {
        location: Point,
        effect: u8,
        value: u8,
    },
    AllowObserve {
        allow: bool,
    },
    ObjectRangeAttack {
        info: ObjectRangeAttackInfo,
    },
    AddBuff {
        buff: ClientBuff,
    },
    RemoveBuff {
        buff_type: u8,
        object_id: u32,
    },
    PauseBuff {
        buff_type: u8,
        object_id: u32,
        paused: bool,
    },
    ObjectHidden {
        object_id: u32,
        hidden: bool,
    },
    RefreshItem {
        item: UserItem,
    },
    ObjectSpell {
        info: ObjectSpellInfo,
    },
    UserDash {
        location: Point,
        direction: MirDirection,
    },
    ObjectDash {
        object_id: u32,
        location: Point,
        direction: MirDirection,
    },
    UserDashFail {
        location: Point,
        direction: MirDirection,
    },
    ObjectDashFail {
        object_id: u32,
        location: Point,
        direction: MirDirection,
    },
    ConsignItem {
        unique_id: u64,
        success: bool,
    },
    MarketFail {
        reason: u8,
    },
    MarketSuccess {
        message: String,
    },
    HeroCreateRequest {
        can_create_class: Vec<bool>,
    },
    NewHero {
        result: u8,
    },
    UpdateHeroSpawnState {
        state: u8,
    },
    UnlockHeroAutoPot,
    SetHeroBehaviour {
        behaviour: u8,
    },
    ManageHeroes {
        maximum_count: i32,
        current_hero: Option<ClientHeroInformation>,
        heroes: Option<Vec<Option<ClientHeroInformation>>>,
    },
    ChangeHero {
        from_index: i32,
    },
    DefaultNPC {
        object_id: u32,
    },
    NPCUpdate {
        npc_id: u32,
    },
    MarriageRequest {
        name: String,
    },
    DivorceRequest {
        name: String,
    },
    MentorRequest {
        name: String,
        level: u16,
    },
    TradeRequest {
        name: String,
    },
    TradeAccept {
        name: String,
    },
    TradeGold {
        amount: u32,
    },
    TradeItem {
        trade_items: Vec<Option<UserItem>>,
    },
    TradeConfirm,
    TradeCancel {
        unlock: bool,
    },
    MountUpdate {
        object_id: u32,
        mount_type: i16,
        riding_mount: bool,
    },
    FishingUpdate {
        object_id: u32,
        fishing: bool,
        progress_percent: i32,
        chance_percent: i32,
        fishing_point: Point,
        found_fish: bool,
    },
    RemoveDelayedExplosion {
        object_id: u32,
    },
    ObjectDeco {
        object_id: u32,
        location: Point,
        image: i32,
    },
    ObjectSneaking {
        object_id: u32,
        sneaking_active: bool,
    },
    ObjectLevelEffects {
        object_id: u32,
        level_effects: u16,
    },
    SetBindingShot {
        object_id: u32,
        enabled: bool,
        value: i64,
    },
    SendOutputMessage {
        message: String,
        output_type: u8,
    },
    OpenDoor {
        door_index: u8,
        close: bool,
    },
    OpenBrowser {
        url: String,
    },
    PlaySound {
        sound: i32,
    },
    SetTimer {
        key: String,
        timer_type: u8,
        seconds: i32,
    },
    ExpireTimer {
        key: String,
    },
    Roll {
        roll_type: i32,
        page: String,
        result: i32,
        auto_roll: bool,
    },
    SetCompass {
        location: Point,
    },
    NPCAwakening,
    NPCDisassemble,
    NPCDowngrade,
    NPCReset,
    AwakeningLockedItem {
        unique_id: u64,
        locked: bool,
    },
    Awakening {
        result: i32,
        remove_id: i64,
    },
    ReceiveMail {
        mail: Vec<ClientMail>,
    },
    MailLockedItem {
        unique_id: u64,
        locked: bool,
    },
    MailSendRequest,
    MailSent {
        result: i8,
    },
    ParcelCollected {
        result: i8,
    },
    MailCost {
        cost: u32,
    },
    FriendUpdate {
        friends: Vec<ClientFriend>,
    },
    LoverUpdate {
        name: String,
        date_binary_datetime: i64,
        map_name: String,
        married_days: i16,
    },
    MentorUpdate {
        name: String,
        level: u16,
        online: bool,
        mentee_exp: i64,
    },
    GuildBuffList {
        remove: u8,
        active_buffs: Vec<GuildBuff>,
        guild_buffs: Vec<GuildBuffInfo>,
    },
    GameShopInfo {
        item: GameShopItem,
        stock_level: i32,
    },
    NewIntelligentCreature {
        creature: ClientIntelligentCreature,
    },
    UpdateIntelligentCreatureList {
        creature_list: Vec<ClientIntelligentCreature>,
        creature_summoned: bool,
        summoned_creature_type: u8,
        pearl_count: i32,
    },
    IntelligentCreatureEnableRename,
    IntelligentCreaturePickup {
        object_id: u32,
    },
    GetRentedItems {
        rented_items: Vec<ItemRentalInformation>,
    },
    ItemRentalRequest {
        name: String,
        renting: bool,
    },
    ItemRentalFee {
        amount: u32,
    },
    ItemRentalPeriod {
        days: u32,
    },
    DepositRentalItem {
        from: i32,
        to: i32,
        success: bool,
    },
    RetrieveRentalItem {
        from: i32,
        to: i32,
        success: bool,
    },
    UpdateRentalItem {
        loan_item: Option<UserItem>,
    },
    CancelItemRental,
    ItemRentalLock {
        success: bool,
        gold_locked: bool,
        item_locked: bool,
    },
    ItemRentalPartnerLock {
        gold_locked: bool,
        item_locked: bool,
    },
    CanConfirmItemRental,
    ConfirmItemRental,
    ChangeQuest {
        quest_id: i32,
        task_list: Vec<String>,
        taken: bool,
        completed: bool,
        new: bool,
        quest_state: u8,
        track_quest: bool,
    },
    CompleteQuest {
        completed_quests: Vec<i32>,
    },
    ShareQuest {
        quest_index: i32,
        sharer_name: String,
    },
    NewQuestInfo {
        info: ClientQuestInfo,
    },
    NewRecipeInfo {
        info: ClientRecipeInfo,
    },
    ResizeInventory {
        size: i32,
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
            Self::Raw { packet_id, .. } => *packet_id,
            Self::TimeOfDay { .. } => ServerPacketId::TimeOfDay,
            Self::ChangeAMode { .. } => ServerPacketId::ChangeAMode,
            Self::ChangePMode { .. } => ServerPacketId::ChangePMode,
            Self::BaseStatsInfo { .. } => ServerPacketId::BaseStatsInfo,
            Self::HeroBaseStatsInfo { .. } => ServerPacketId::HeroBaseStatsInfo,
            Self::HeroInformation { .. } => ServerPacketId::HeroInformation,
            Self::NPCMarket { .. } => ServerPacketId::NPCMarket,
            Self::NPCMarketPage { .. } => ServerPacketId::NPCMarketPage,
            Self::NewMapInfo { .. } => ServerPacketId::NewMapInfo,
            Self::WorldMapSetup { .. } => ServerPacketId::WorldMapSetup,
            Self::SearchMapResult { .. } => ServerPacketId::SearchMapResult,
            Self::UserSlotsRefresh { .. } => ServerPacketId::UserSlotsRefresh,
            Self::NewChatItem { .. } => ServerPacketId::NewChatItem,
            Self::PlayerUpdate { .. } => ServerPacketId::PlayerUpdate,
            Self::PlayerInspect { .. } => ServerPacketId::PlayerInspect,
            Self::ReturnToLogin => ServerPacketId::ReturnToLogin,
            Self::DamageIndicator { .. } => ServerPacketId::DamageIndicator,
            Self::HealthChanged { .. } => ServerPacketId::HealthChanged,
            Self::Death { .. } => ServerPacketId::Death,
            Self::ColourChanged { .. } => ServerPacketId::ColourChanged,
            Self::ObjectColourChanged { .. } => ServerPacketId::ObjectColourChanged,
            Self::ObjectGuildNameChanged { .. } => ServerPacketId::ObjectGuildNameChanged,
            Self::GainExperience { .. } => ServerPacketId::GainExperience,
            Self::LevelChanged { .. } => ServerPacketId::LevelChanged,
            Self::ObjectLeveled { .. } => ServerPacketId::ObjectLeveled,
            Self::Poisoned { .. } => ServerPacketId::Poisoned,
            Self::ObjectPoisoned { .. } => ServerPacketId::ObjectPoisoned,
            Self::MapChanged { .. } => ServerPacketId::MapChanged,
            Self::ObjectName { .. } => ServerPacketId::ObjectName,
            Self::Revived => ServerPacketId::Revived,
            Self::NPCConsign => ServerPacketId::NPCConsign,
            Self::InTrapRock { .. } => ServerPacketId::InTrapRock,
            Self::UserName { .. } => ServerPacketId::UserName,
            Self::ChatItemStats { .. } => ServerPacketId::ChatItemStats,
            Self::GuildNoticeChange { .. } => ServerPacketId::GuildNoticeChange,
            Self::GuildMemberChange { .. } => ServerPacketId::GuildMemberChange,
            Self::GuildStatus { .. } => ServerPacketId::GuildStatus,
            Self::GuildInvite { .. } => ServerPacketId::GuildInvite,
            Self::GuildExpGain { .. } => ServerPacketId::GuildExpGain,
            Self::GuildNameRequest => ServerPacketId::GuildNameRequest,
            Self::GuildStorageGoldChange { .. } => ServerPacketId::GuildStorageGoldChange,
            Self::GuildStorageItemChange { .. } => ServerPacketId::GuildStorageItemChange,
            Self::GuildStorageList { .. } => ServerPacketId::GuildStorageList,
            Self::GuildRequestWar => ServerPacketId::GuildRequestWar,
            Self::SetAutoPotValue { .. } => ServerPacketId::SetAutoPotValue,
            Self::SetAutoPotItem { .. } => ServerPacketId::SetAutoPotItem,
            Self::NPCImageUpdate { .. } => ServerPacketId::NPCImageUpdate,
            Self::EquipSlotItem { .. } => ServerPacketId::EquipSlotItem,
            Self::GainedQuestItem { .. } => ServerPacketId::GainedQuestItem,
            Self::DeleteQuestItem { .. } => ServerPacketId::DeleteQuestItem,
            Self::CancelReincarnation => ServerPacketId::CancelReincarnation,
            Self::RequestReincarnation => ServerPacketId::RequestReincarnation,
            Self::UserBackStep { .. } => ServerPacketId::UserBackStep,
            Self::UserDashAttack { .. } => ServerPacketId::UserDashAttack,
            Self::ObjectDashAttack { .. } => ServerPacketId::ObjectDashAttack,
            Self::UserAttackMove { .. } => ServerPacketId::UserAttackMove,
            Self::SetConcentration { .. } => ServerPacketId::SetConcentration,
            Self::SetElemental { .. } => ServerPacketId::SetElemental,
            Self::AwakeningNeedMaterials { .. } => ServerPacketId::AwakeningNeedMaterials,
            Self::NPCPearlGoods { .. } => ServerPacketId::NPCPearlGoods,
            Self::TransformUpdate { .. } => ServerPacketId::TransformUpdate,
            Self::NPCRequestInput { .. } => ServerPacketId::NPCRequestInput,
            Self::GameShopStock { .. } => ServerPacketId::GameShopStock,
            Self::Rankings { .. } => ServerPacketId::Rankings,
            Self::UpdateNotice { .. } => ServerPacketId::UpdateNotice,
            Self::GuildTerritoryPage { .. } => ServerPacketId::GuildTerritoryPage,
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
            Self::ObjectHero { .. } => ServerPacketId::ObjectHero,
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
            Self::DepositTradeItem { .. } => ServerPacketId::DepositTradeItem,
            Self::RetrieveTradeItem { .. } => ServerPacketId::RetrieveTradeItem,
            Self::UseItem { .. } => ServerPacketId::UseItem,
            Self::DropItem { .. } => ServerPacketId::DropItem,
            Self::TakeBackHeroItem { .. } => ServerPacketId::TakeBackHeroItem,
            Self::TransferHeroItem { .. } => ServerPacketId::TransferHeroItem,
            Self::NewMonsterInfo { .. } => ServerPacketId::NewMonsterInfo,
            Self::NewNpcInfo { .. } => ServerPacketId::NewNpcInfo,
            Self::NewHeroInfo { .. } => ServerPacketId::NewHeroInfo,
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
            Self::HeroHealthChanged { .. } => ServerPacketId::HeroHealthChanged,
            Self::DeleteItem { .. } => ServerPacketId::DeleteItem,
            Self::ObjectDied { .. } => ServerPacketId::ObjectDied,
            Self::GainHeroExperience { .. } => ServerPacketId::GainHeroExperience,
            Self::HeroLevelChanged { .. } => ServerPacketId::HeroLevelChanged,
            Self::ObjectHarvest { .. } => ServerPacketId::ObjectHarvest,
            Self::ObjectHarvested { .. } => ServerPacketId::ObjectHarvested,
            Self::ObjectNpc { .. } => ServerPacketId::ObjectNpc,
            Self::NPCResponse { .. } => ServerPacketId::NPCResponse,
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
            Self::DepositRefineItem { .. } => ServerPacketId::DepositRefineItem,
            Self::RetrieveRefineItem { .. } => ServerPacketId::RetrieveRefineItem,
            Self::RefineCancel => ServerPacketId::RefineCancel,
            Self::RefineItem { .. } => ServerPacketId::RefineItem,
            Self::NewMagic { .. } => ServerPacketId::NewMagic,
            Self::RemoveMagic { .. } => ServerPacketId::RemoveMagic,
            Self::MagicLeveled { .. } => ServerPacketId::MagicLeveled,
            Self::Magic { .. } => ServerPacketId::Magic,
            Self::MagicDelay { .. } => ServerPacketId::MagicDelay,
            Self::MagicCast { .. } => ServerPacketId::MagicCast,
            Self::ObjectMagic { .. } => ServerPacketId::ObjectMagic,
            Self::ObjectProjectile { .. } => ServerPacketId::ObjectProjectile,
            Self::RangeAttack { .. } => ServerPacketId::RangeAttack,
            Self::Pushed { .. } => ServerPacketId::Pushed,
            Self::ObjectPushed { .. } => ServerPacketId::ObjectPushed,
            Self::SpellToggle { .. } => ServerPacketId::SpellToggle,
            Self::SwitchGroup { .. } => ServerPacketId::SwitchGroup,
            Self::DeleteGroup => ServerPacketId::DeleteGroup,
            Self::DeleteMember { .. } => ServerPacketId::DeleteMember,
            Self::GroupInvite { .. } => ServerPacketId::GroupInvite,
            Self::AddMember { .. } => ServerPacketId::AddMember,
            Self::GroupMembersMap { .. } => ServerPacketId::GroupMembersMap,
            Self::SendMemberLocation { .. } => ServerPacketId::SendMemberLocation,
            Self::ObjectRevived { .. } => ServerPacketId::ObjectRevived,
            Self::ObjectEffect { .. } => ServerPacketId::ObjectEffect,
            Self::ObjectHealth { .. } => ServerPacketId::ObjectHealth,
            Self::ObjectMana { .. } => ServerPacketId::ObjectMana,
            Self::MapEffect { .. } => ServerPacketId::MapEffect,
            Self::AllowObserve { .. } => ServerPacketId::AllowObserve,
            Self::ObjectRangeAttack { .. } => ServerPacketId::ObjectRangeAttack,
            Self::AddBuff { .. } => ServerPacketId::AddBuff,
            Self::RemoveBuff { .. } => ServerPacketId::RemoveBuff,
            Self::PauseBuff { .. } => ServerPacketId::PauseBuff,
            Self::ObjectHidden { .. } => ServerPacketId::ObjectHidden,
            Self::RefreshItem { .. } => ServerPacketId::RefreshItem,
            Self::ObjectSpell { .. } => ServerPacketId::ObjectSpell,
            Self::UserDash { .. } => ServerPacketId::UserDash,
            Self::ObjectDash { .. } => ServerPacketId::ObjectDash,
            Self::UserDashFail { .. } => ServerPacketId::UserDashFail,
            Self::ObjectDashFail { .. } => ServerPacketId::ObjectDashFail,
            Self::ConsignItem { .. } => ServerPacketId::ConsignItem,
            Self::MarketFail { .. } => ServerPacketId::MarketFail,
            Self::MarketSuccess { .. } => ServerPacketId::MarketSuccess,
            Self::HeroCreateRequest { .. } => ServerPacketId::HeroCreateRequest,
            Self::NewHero { .. } => ServerPacketId::NewHero,
            Self::UpdateHeroSpawnState { .. } => ServerPacketId::UpdateHeroSpawnState,
            Self::UnlockHeroAutoPot => ServerPacketId::UnlockHeroAutoPot,
            Self::SetHeroBehaviour { .. } => ServerPacketId::SetHeroBehaviour,
            Self::ManageHeroes { .. } => ServerPacketId::ManageHeroes,
            Self::ChangeHero { .. } => ServerPacketId::ChangeHero,
            Self::DefaultNPC { .. } => ServerPacketId::DefaultNPC,
            Self::NPCUpdate { .. } => ServerPacketId::NPCUpdate,
            Self::MarriageRequest { .. } => ServerPacketId::MarriageRequest,
            Self::DivorceRequest { .. } => ServerPacketId::DivorceRequest,
            Self::MentorRequest { .. } => ServerPacketId::MentorRequest,
            Self::TradeRequest { .. } => ServerPacketId::TradeRequest,
            Self::TradeAccept { .. } => ServerPacketId::TradeAccept,
            Self::TradeGold { .. } => ServerPacketId::TradeGold,
            Self::TradeItem { .. } => ServerPacketId::TradeItem,
            Self::TradeConfirm => ServerPacketId::TradeConfirm,
            Self::TradeCancel { .. } => ServerPacketId::TradeCancel,
            Self::MountUpdate { .. } => ServerPacketId::MountUpdate,
            Self::FishingUpdate { .. } => ServerPacketId::FishingUpdate,
            Self::RemoveDelayedExplosion { .. } => ServerPacketId::RemoveDelayedExplosion,
            Self::ObjectDeco { .. } => ServerPacketId::ObjectDeco,
            Self::ObjectSneaking { .. } => ServerPacketId::ObjectSneaking,
            Self::ObjectLevelEffects { .. } => ServerPacketId::ObjectLevelEffects,
            Self::SetBindingShot { .. } => ServerPacketId::SetBindingShot,
            Self::SendOutputMessage { .. } => ServerPacketId::SendOutputMessage,
            Self::OpenDoor { .. } => ServerPacketId::OpenDoor,
            Self::OpenBrowser { .. } => ServerPacketId::OpenBrowser,
            Self::PlaySound { .. } => ServerPacketId::PlaySound,
            Self::SetTimer { .. } => ServerPacketId::SetTimer,
            Self::ExpireTimer { .. } => ServerPacketId::ExpireTimer,
            Self::Roll { .. } => ServerPacketId::Roll,
            Self::SetCompass { .. } => ServerPacketId::SetCompass,
            Self::NPCAwakening => ServerPacketId::NPCAwakening,
            Self::NPCDisassemble => ServerPacketId::NPCDisassemble,
            Self::NPCDowngrade => ServerPacketId::NPCDowngrade,
            Self::NPCReset => ServerPacketId::NPCReset,
            Self::AwakeningLockedItem { .. } => ServerPacketId::AwakeningLockedItem,
            Self::Awakening { .. } => ServerPacketId::Awakening,
            Self::ReceiveMail { .. } => ServerPacketId::ReceiveMail,
            Self::MailLockedItem { .. } => ServerPacketId::MailLockedItem,
            Self::MailSendRequest => ServerPacketId::MailSendRequest,
            Self::MailSent { .. } => ServerPacketId::MailSent,
            Self::ParcelCollected { .. } => ServerPacketId::ParcelCollected,
            Self::MailCost { .. } => ServerPacketId::MailCost,
            Self::FriendUpdate { .. } => ServerPacketId::FriendUpdate,
            Self::LoverUpdate { .. } => ServerPacketId::LoverUpdate,
            Self::MentorUpdate { .. } => ServerPacketId::MentorUpdate,
            Self::GuildBuffList { .. } => ServerPacketId::GuildBuffList,
            Self::GameShopInfo { .. } => ServerPacketId::GameShopInfo,
            Self::NewIntelligentCreature { .. } => ServerPacketId::NewIntelligentCreature,
            Self::UpdateIntelligentCreatureList { .. } => {
                ServerPacketId::UpdateIntelligentCreatureList
            }
            Self::IntelligentCreatureEnableRename => {
                ServerPacketId::IntelligentCreatureEnableRename
            }
            Self::IntelligentCreaturePickup { .. } => ServerPacketId::IntelligentCreaturePickup,
            Self::GetRentedItems { .. } => ServerPacketId::GetRentedItems,
            Self::ItemRentalRequest { .. } => ServerPacketId::ItemRentalRequest,
            Self::ItemRentalFee { .. } => ServerPacketId::ItemRentalFee,
            Self::ItemRentalPeriod { .. } => ServerPacketId::ItemRentalPeriod,
            Self::DepositRentalItem { .. } => ServerPacketId::DepositRentalItem,
            Self::RetrieveRentalItem { .. } => ServerPacketId::RetrieveRentalItem,
            Self::UpdateRentalItem { .. } => ServerPacketId::UpdateRentalItem,
            Self::CancelItemRental => ServerPacketId::CancelItemRental,
            Self::ItemRentalLock { .. } => ServerPacketId::ItemRentalLock,
            Self::ItemRentalPartnerLock { .. } => ServerPacketId::ItemRentalPartnerLock,
            Self::CanConfirmItemRental => ServerPacketId::CanConfirmItemRental,
            Self::ConfirmItemRental => ServerPacketId::ConfirmItemRental,
            Self::ChangeQuest { .. } => ServerPacketId::ChangeQuest,
            Self::CompleteQuest { .. } => ServerPacketId::CompleteQuest,
            Self::ShareQuest { .. } => ServerPacketId::ShareQuest,
            Self::NewQuestInfo { .. } => ServerPacketId::NewQuestInfo,
            Self::NewRecipeInfo { .. } => ServerPacketId::NewRecipeInfo,
            Self::ResizeInventory { .. } => ServerPacketId::ResizeInventory,
            Self::ResizeStorage { .. } => ServerPacketId::ResizeStorage,
            Self::StorageUnlockResult { .. } => ServerPacketId::StorageUnlockResult,
            Self::StoragePasswordResult { .. } => ServerPacketId::StoragePasswordResult,
            Self::LogOutSuccess { .. } => ServerPacketId::LogOutSuccess,
            Self::LogOutFailed => ServerPacketId::LogOutFailed,
        }
    }

    fn encode_payload(&self, writer: &mut PacketWriter) -> Result<()> {
        match self {
            Self::Raw { payload, .. } => writer.write_bytes(payload),
            Self::TimeOfDay { lights } => writer.write_u8(*lights),
            Self::ChangeAMode { mode } | Self::ChangePMode { mode } => writer.write_u8(*mode),
            Self::BaseStatsInfo { stats } | Self::HeroBaseStatsInfo { stats } => {
                stats.encode(writer);
            }
            Self::HeroInformation { info } => info.encode(writer)?,
            Self::NPCMarket {
                listings,
                pages,
                user_mode,
            } => {
                writer.write_i32(listings.len() as i32);
                for listing in listings {
                    listing.encode(writer)?;
                }
                writer.write_i32(*pages);
                writer.write_bool(*user_mode);
            }
            Self::NPCMarketPage { listings } => {
                writer.write_i32(listings.len() as i32);
                for listing in listings {
                    listing.encode(writer)?;
                }
            }
            Self::NewMapInfo { map_index, info } => {
                writer.write_i32(*map_index);
                info.encode(writer)?;
            }
            Self::WorldMapSetup {
                setup,
                teleport_to_npc_cost,
            } => {
                setup.encode(writer)?;
                writer.write_i32(*teleport_to_npc_cost);
            }
            Self::SearchMapResult {
                map_index,
                npc_index,
            } => {
                writer.write_i32(*map_index);
                writer.write_u32(*npc_index);
            }
            Self::UserSlotsRefresh {
                inventory,
                equipment,
            } => {
                encode_optional_user_item_slots(writer, inventory.as_deref())?;
                encode_optional_user_item_slots(writer, equipment.as_deref())?;
            }
            Self::NewChatItem { item } => item.encode(writer)?,
            Self::PlayerUpdate {
                object_id,
                light,
                weapon,
                weapon_effect,
                armour,
                wing_effect,
            } => {
                writer.write_u32(*object_id);
                writer.write_u8(*light);
                writer.write_i16(*weapon);
                writer.write_i16(*weapon_effect);
                writer.write_i16(*armour);
                writer.write_u8(*wing_effect);
            }
            Self::PlayerInspect { info } => info.encode(writer)?,
            Self::ReturnToLogin | Self::Revived | Self::NPCConsign => {}
            Self::DamageIndicator {
                damage,
                damage_type,
                object_id,
            } => {
                writer.write_i32(*damage);
                writer.write_u8(*damage_type);
                writer.write_u32(*object_id);
            }
            Self::HealthChanged { hp, mp } => {
                writer.write_i32(*hp);
                writer.write_i32(*mp);
            }
            Self::Death {
                location,
                direction,
            }
            | Self::UserBackStep {
                location,
                direction,
            }
            | Self::UserDashAttack {
                location,
                direction,
            }
            | Self::UserAttackMove {
                location,
                direction,
            } => {
                location.encode(writer);
                writer.write_u8(*direction as u8);
            }
            Self::ColourChanged { name_colour_argb } => writer.write_i32(*name_colour_argb),
            Self::ObjectColourChanged {
                object_id,
                name_colour_argb,
            } => {
                writer.write_u32(*object_id);
                writer.write_i32(*name_colour_argb);
            }
            Self::ObjectGuildNameChanged {
                object_id,
                guild_name,
            } => {
                writer.write_u32(*object_id);
                writer.write_string(guild_name)?;
            }
            Self::GainExperience { amount } | Self::GuildExpGain { amount } => {
                writer.write_u32(*amount)
            }
            Self::LevelChanged {
                level,
                experience,
                max_experience,
            } => {
                writer.write_u16(*level);
                writer.write_i64(*experience);
                writer.write_i64(*max_experience);
            }
            Self::ObjectLeveled { object_id } => writer.write_u32(*object_id),
            Self::Poisoned { poison } => writer.write_u16(*poison),
            Self::ObjectPoisoned { object_id, poison } => {
                writer.write_u32(*object_id);
                writer.write_u16(*poison);
            }
            Self::MapChanged {
                map_index,
                file_name,
                title,
                mini_map,
                big_map,
                lights,
                location,
                direction,
                map_dark_light,
                music,
                weather,
            } => {
                writer.write_i32(*map_index);
                writer.write_string(file_name)?;
                writer.write_string(title)?;
                writer.write_u16(*mini_map);
                writer.write_u16(*big_map);
                writer.write_u8(*lights);
                location.encode(writer);
                writer.write_u8(*direction as u8);
                writer.write_u8(*map_dark_light);
                writer.write_u16(*music);
                writer.write_u16(*weather);
            }
            Self::ObjectName { object_id, name }
            | Self::UserName {
                id: object_id,
                name,
            } => {
                writer.write_u32(*object_id);
                writer.write_string(name)?;
            }
            Self::InTrapRock { trapped } => writer.write_bool(*trapped),
            Self::ChatItemStats {
                chat_item_id,
                stats,
            } => {
                writer.write_u64(*chat_item_id);
                if let Some(stats) = stats {
                    stats.encode(writer)?;
                }
            }
            Self::GuildNoticeChange { update, notice } => {
                if *update < 0 {
                    writer.write_i32(*update);
                } else {
                    writer.write_i32(notice.len() as i32);
                    for line in notice {
                        writer.write_string(line)?;
                    }
                }
            }
            Self::GuildMemberChange {
                name,
                rank_index,
                status,
                ranks,
            } => {
                writer.write_string(name)?;
                writer.write_u8(*rank_index);
                writer.write_u8(*status);
                if *status > 5 {
                    writer.write_i32(ranks.len() as i32);
                    for rank in ranks {
                        rank.encode(writer)?;
                    }
                }
            }
            Self::GuildStatus {
                guild_name,
                guild_rank_name,
                level,
                experience,
                max_experience,
                gold,
                spare_points,
                member_count,
                max_members,
                voting,
                item_count,
                buff_count,
                my_options,
                my_rank_id,
            } => {
                writer.write_string(guild_name)?;
                writer.write_string(guild_rank_name)?;
                writer.write_u8(*level);
                writer.write_i64(*experience);
                writer.write_i64(*max_experience);
                writer.write_u32(*gold);
                writer.write_u8(*spare_points);
                writer.write_i32(*member_count);
                writer.write_i32(*max_members);
                writer.write_bool(*voting);
                writer.write_u8(*item_count);
                writer.write_u8(*buff_count);
                writer.write_u8(*my_options);
                writer.write_i32(*my_rank_id);
            }
            Self::GuildInvite { name } => writer.write_string(name)?,
            Self::GuildNameRequest | Self::GuildRequestWar => {}
            Self::GuildStorageGoldChange {
                amount,
                change_type,
                name,
            } => {
                writer.write_u32(*amount);
                writer.write_u8(*change_type);
                writer.write_string(name)?;
            }
            Self::GuildStorageItemChange {
                change_type,
                to,
                from,
                user,
                item,
            } => {
                writer.write_u8(*change_type);
                writer.write_i32(*to);
                writer.write_i32(*from);
                writer.write_i32(*user);
                writer.write_bool(item.is_some());
                if let Some(item) = item {
                    writer.write_i64(item.user_id);
                    item.item.encode(writer)?;
                }
            }
            Self::GuildStorageList { items } => {
                writer.write_i32(items.len() as i32);
                for item in items {
                    writer.write_bool(item.is_some());
                    if let Some(item) = item {
                        item.encode(writer)?;
                    }
                }
            }
            Self::SetAutoPotValue { stat, value } => {
                writer.write_u8(*stat);
                writer.write_u32(*value);
            }
            Self::SetAutoPotItem { grid, item_index } => {
                writer.write_u8(*grid);
                writer.write_i32(*item_index);
            }
            Self::NPCImageUpdate {
                object_id,
                image,
                colour_argb,
            } => {
                writer.write_u32(*object_id);
                writer.write_u16(*image);
                writer.write_i32(*colour_argb);
            }
            Self::EquipSlotItem {
                grid,
                unique_id,
                to,
                grid_to,
                success,
            } => {
                writer.write_u8(*grid as u8);
                writer.write_u64(*unique_id);
                writer.write_i32(*to);
                writer.write_u8(*grid_to as u8);
                writer.write_bool(*success);
            }
            Self::GainedQuestItem { item } => item.encode(writer)?,
            Self::DeleteQuestItem { unique_id, count } => {
                writer.write_u64(*unique_id);
                writer.write_u16(*count);
            }
            Self::CancelReincarnation | Self::RequestReincarnation => {}
            Self::ObjectDashAttack {
                object_id,
                location,
                direction,
                distance,
            } => {
                writer.write_u32(*object_id);
                location.encode(writer);
                writer.write_u8(*direction as u8);
                writer.write_i32(*distance);
            }
            Self::SetConcentration {
                object_id,
                enabled,
                interrupted,
            } => {
                writer.write_u32(*object_id);
                writer.write_bool(*enabled);
                writer.write_bool(*interrupted);
            }
            Self::SetElemental {
                object_id,
                enabled,
                casted,
                value,
                element_type,
                exp_last,
            } => {
                writer.write_u32(*object_id);
                writer.write_bool(*enabled);
                writer.write_bool(*casted);
                writer.write_u32(*value);
                writer.write_u32(*element_type);
                writer.write_u32(*exp_last);
            }
            Self::AwakeningNeedMaterials { materials } => {
                writer.write_bool(materials.is_some());
                if let Some(materials) = materials {
                    writer.write_i32(materials.len() as i32);
                    for material in materials {
                        writer.write_bool(material.is_some());
                        if let Some(material) = material {
                            material.encode(writer)?;
                        }
                    }
                }
            }
            Self::NPCPearlGoods {
                list,
                rate,
                panel_type,
            } => {
                writer.write_i32(list.len() as i32);
                for item in list {
                    item.encode(writer)?;
                }
                writer.write_f32(*rate);
                writer.write_u8(*panel_type);
            }
            Self::TransformUpdate {
                object_id,
                transform_type,
            } => {
                writer.write_u32(*object_id);
                writer.write_i16(*transform_type);
            }
            Self::NPCRequestInput { npc_id, page_name } => {
                writer.write_u32(*npc_id);
                writer.write_string(page_name)?;
            }
            Self::GameShopStock {
                g_index,
                stock_level,
            } => {
                writer.write_i32(*g_index);
                writer.write_i32(*stock_level);
            }
            Self::Rankings {
                rank_type,
                my_rank,
                listing_details,
                listings,
                count,
            } => {
                writer.write_u8(*rank_type);
                writer.write_i32(*my_rank);
                writer.write_i32(listing_details.len() as i32);
                for details in listing_details {
                    details.encode(writer)?;
                }
                writer.write_i32(listings.len() as i32);
                for listing in listings {
                    writer.write_i64(*listing);
                }
                writer.write_i32(*count);
            }
            Self::UpdateNotice { notice } => notice.encode(writer)?,
            Self::GuildTerritoryPage { length, listings } => {
                writer.write_i32(*length);
                writer.write_i32(listings.len() as i32);
                for listing in listings {
                    listing.encode(writer)?;
                }
            }
            Self::Connected => {}
            Self::ClientVersion { result }
            | Self::NewAccount { result }
            | Self::ChangePassword { result }
            | Self::Login { result }
            | Self::NewCharacter { result }
            | Self::NewHero { result }
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
            Self::ObjectHero { info, owner_name } => {
                info.encode(writer)?;
                writer.write_string(owner_name)?;
            }
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
            Self::DepositRefineItem { from, to, success }
            | Self::RetrieveRefineItem { from, to, success }
            | Self::TakeBackItem { from, to, success }
            | Self::StoreItem { from, to, success }
            | Self::TakeBackHeroItem { from, to, success }
            | Self::TransferHeroItem { from, to, success } => {
                writer.write_i32(*from);
                writer.write_i32(*to);
                writer.write_bool(*success);
            }
            Self::RefineCancel => {}
            Self::RefineItem { unique_id } => writer.write_u64(*unique_id),
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
            Self::DepositTradeItem { from, to, success }
            | Self::RetrieveTradeItem { from, to, success } => {
                writer.write_i32(*from);
                writer.write_i32(*to);
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
            Self::NewHeroInfo {
                info,
                storage_index,
            } => {
                info.encode(writer)?;
                writer.write_i32(*storage_index);
            }
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
            Self::HeroHealthChanged { hp, mp } => {
                writer.write_i32(*hp);
                writer.write_i32(*mp);
            }
            Self::DeleteItem { unique_id, count } => {
                writer.write_u64(*unique_id);
                writer.write_u16(*count);
            }
            Self::GainHeroExperience { amount } => writer.write_u32(*amount),
            Self::HeroLevelChanged {
                level,
                experience,
                max_experience,
            } => {
                writer.write_u16(*level);
                writer.write_i64(*experience);
                writer.write_i64(*max_experience);
            }
            Self::ObjectDied { info } => info.encode(writer),
            Self::ObjectHarvest { movement } | Self::ObjectHarvested { movement } => {
                movement.encode(writer);
            }
            Self::ObjectNpc { info } => info.encode(writer)?,
            Self::NPCResponse { page } => {
                writer.write_i32(page.len() as i32);
                for line in page {
                    writer.write_string(line)?;
                }
            }
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
            Self::NewMagic { magic, hero } => {
                magic.encode(writer)?;
                writer.write_bool(*hero);
            }
            Self::RemoveMagic { place_id } => writer.write_i32(*place_id),
            Self::MagicLeveled {
                object_id,
                spell,
                level,
                experience,
            } => {
                writer.write_u32(*object_id);
                writer.write_u8(*spell as u8);
                writer.write_u8(*level);
                writer.write_u16(*experience);
            }
            Self::Magic {
                spell,
                target_id,
                target,
                cast,
                level,
                secondary_target_ids,
            } => {
                writer.write_u8(*spell as u8);
                writer.write_u32(*target_id);
                target.encode(writer);
                writer.write_bool(*cast);
                writer.write_u8(*level);
                writer.write_i32(secondary_target_ids.len() as i32);
                for target_id in secondary_target_ids {
                    writer.write_u32(*target_id);
                }
            }
            Self::MagicDelay {
                object_id,
                spell,
                delay,
            } => {
                writer.write_u32(*object_id);
                writer.write_u8(*spell as u8);
                writer.write_i64(*delay);
            }
            Self::MagicCast { spell } => writer.write_u8(*spell as u8),
            Self::ObjectMagic {
                object_id,
                location,
                direction,
                spell,
                target_id,
                target,
                cast,
                level,
                self_broadcast,
                secondary_target_ids,
            } => {
                writer.write_u32(*object_id);
                location.encode(writer);
                writer.write_u8(*direction as u8);
                writer.write_u8(*spell as u8);
                writer.write_u32(*target_id);
                target.encode(writer);
                writer.write_bool(*cast);
                writer.write_u8(*level);
                writer.write_bool(*self_broadcast);
                writer.write_i32(secondary_target_ids.len() as i32);
                for target_id in secondary_target_ids {
                    writer.write_u32(*target_id);
                }
            }
            Self::ObjectProjectile {
                spell,
                source_id,
                destination_id,
            } => {
                writer.write_u8(*spell as u8);
                writer.write_u32(*source_id);
                writer.write_u32(*destination_id);
            }
            Self::RangeAttack {
                target_id,
                target,
                spell,
            } => {
                writer.write_u32(*target_id);
                target.encode(writer);
                writer.write_u8(*spell as u8);
            }
            Self::Pushed {
                location,
                direction,
            }
            | Self::UserDash {
                location,
                direction,
            }
            | Self::UserDashFail {
                location,
                direction,
            } => {
                location.encode(writer);
                writer.write_u8(*direction as u8);
            }
            Self::ObjectPushed {
                object_id,
                location,
                direction,
            }
            | Self::ObjectDash {
                object_id,
                location,
                direction,
            }
            | Self::ObjectDashFail {
                object_id,
                location,
                direction,
            } => {
                writer.write_u32(*object_id);
                location.encode(writer);
                writer.write_u8(*direction as u8);
            }
            Self::SpellToggle {
                object_id,
                spell,
                can_use,
            } => {
                writer.write_u32(*object_id);
                writer.write_u8(*spell as u8);
                writer.write_bool(*can_use);
            }
            Self::SwitchGroup { allow_group } => writer.write_bool(*allow_group),
            Self::DeleteGroup => {}
            Self::DeleteMember { name } | Self::GroupInvite { name } | Self::AddMember { name } => {
                writer.write_string(name)?;
            }
            Self::GroupMembersMap {
                player_name,
                player_map,
            } => {
                writer.write_string(player_name)?;
                writer.write_string(player_map)?;
            }
            Self::SendMemberLocation {
                member_name,
                member_location,
            } => {
                writer.write_string(member_name)?;
                member_location.encode(writer);
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
            Self::ObjectMana { info } => info.encode(writer),
            Self::MapEffect {
                location,
                effect,
                value,
            } => {
                location.encode(writer);
                writer.write_u8(*effect);
                writer.write_u8(*value);
            }
            Self::AllowObserve { allow } => writer.write_bool(*allow),
            Self::ObjectRangeAttack { info } => info.encode(writer),
            Self::AddBuff { buff } => buff.encode(writer),
            Self::RemoveBuff {
                buff_type,
                object_id,
            } => {
                writer.write_u8(*buff_type);
                writer.write_u32(*object_id);
            }
            Self::PauseBuff {
                buff_type,
                object_id,
                paused,
            } => {
                writer.write_u8(*buff_type);
                writer.write_u32(*object_id);
                writer.write_bool(*paused);
            }
            Self::ObjectHidden { object_id, hidden } => {
                writer.write_u32(*object_id);
                writer.write_bool(*hidden);
            }
            Self::RefreshItem { item } => item.encode(writer)?,
            Self::ObjectSpell { info } => info.encode(writer),
            Self::ConsignItem { unique_id, success } => {
                writer.write_u64(*unique_id);
                writer.write_bool(*success);
            }
            Self::MarketFail { reason } => writer.write_u8(*reason),
            Self::MarketSuccess { message } => writer.write_string(message)?,
            Self::HeroCreateRequest { can_create_class } => {
                writer.write_i32(can_create_class.len() as i32);
                for can_create in can_create_class {
                    writer.write_bool(*can_create);
                }
            }
            Self::UpdateHeroSpawnState { state } | Self::SetHeroBehaviour { behaviour: state } => {
                writer.write_u8(*state);
            }
            Self::UnlockHeroAutoPot => {}
            Self::ManageHeroes {
                maximum_count,
                current_hero,
                heroes,
            } => {
                writer.write_i32(*maximum_count);
                writer.write_bool(current_hero.is_some());
                if let Some(current_hero) = current_hero {
                    current_hero.encode(writer)?;
                }
                writer.write_bool(heroes.is_some());
                if let Some(heroes) = heroes {
                    writer.write_i32(heroes.len() as i32);
                    for hero in heroes {
                        writer.write_bool(hero.is_some());
                        if let Some(hero) = hero {
                            hero.encode(writer)?;
                        }
                    }
                }
            }
            Self::ChangeHero { from_index } => writer.write_i32(*from_index),
            Self::DefaultNPC { object_id } => writer.write_u32(*object_id),
            Self::NPCUpdate { npc_id } => writer.write_u32(*npc_id),
            Self::MarriageRequest { name }
            | Self::DivorceRequest { name }
            | Self::TradeRequest { name }
            | Self::TradeAccept { name } => {
                writer.write_string(name)?;
            }
            Self::MentorRequest { name, level } => {
                writer.write_string(name)?;
                writer.write_u16(*level);
            }
            Self::TradeGold { amount } => writer.write_u32(*amount),
            Self::TradeItem { trade_items } => {
                writer.write_i32(trade_items.len() as i32);
                for item in trade_items {
                    writer.write_bool(item.is_some());
                    if let Some(item) = item {
                        item.encode(writer)?;
                    }
                }
            }
            Self::TradeConfirm => {}
            Self::TradeCancel { unlock } => writer.write_bool(*unlock),
            Self::MountUpdate {
                object_id,
                mount_type,
                riding_mount,
            } => {
                writer.write_u32(*object_id);
                writer.write_i16(*mount_type);
                writer.write_bool(*riding_mount);
            }
            Self::FishingUpdate {
                object_id,
                fishing,
                progress_percent,
                chance_percent,
                fishing_point,
                found_fish,
            } => {
                writer.write_u32(*object_id);
                writer.write_bool(*fishing);
                writer.write_i32(*progress_percent);
                writer.write_i32(*chance_percent);
                fishing_point.encode(writer);
                writer.write_bool(*found_fish);
            }
            Self::RemoveDelayedExplosion { object_id } => writer.write_u32(*object_id),
            Self::ObjectDeco {
                object_id,
                location,
                image,
            } => {
                writer.write_u32(*object_id);
                location.encode(writer);
                writer.write_i32(*image);
            }
            Self::ObjectSneaking {
                object_id,
                sneaking_active,
            } => {
                writer.write_u32(*object_id);
                writer.write_bool(*sneaking_active);
            }
            Self::ObjectLevelEffects {
                object_id,
                level_effects,
            } => {
                writer.write_u32(*object_id);
                writer.write_u16(*level_effects);
            }
            Self::SetBindingShot {
                object_id,
                enabled,
                value,
            } => {
                writer.write_u32(*object_id);
                writer.write_bool(*enabled);
                writer.write_i64(*value);
            }
            Self::SendOutputMessage {
                message,
                output_type,
            } => {
                writer.write_string(message)?;
                writer.write_u8(*output_type);
            }
            Self::OpenDoor { door_index, close } => {
                writer.write_u8(*door_index);
                writer.write_bool(*close);
            }
            Self::OpenBrowser { url } => writer.write_string(url)?,
            Self::PlaySound { sound } => writer.write_i32(*sound),
            Self::SetTimer {
                key,
                timer_type,
                seconds,
            } => {
                writer.write_string(key)?;
                writer.write_u8(*timer_type);
                writer.write_i32(*seconds);
            }
            Self::ExpireTimer { key } => writer.write_string(key)?,
            Self::Roll {
                roll_type,
                page,
                result,
                auto_roll,
            } => {
                writer.write_i32(*roll_type);
                writer.write_string(page)?;
                writer.write_i32(*result);
                writer.write_bool(*auto_roll);
            }
            Self::SetCompass { location } => location.encode(writer),
            Self::NPCAwakening | Self::NPCDisassemble | Self::NPCDowngrade | Self::NPCReset => {}
            Self::AwakeningLockedItem { unique_id, locked } => {
                writer.write_u64(*unique_id);
                writer.write_bool(*locked);
            }
            Self::Awakening { result, remove_id } => {
                writer.write_i32(*result);
                writer.write_i64(*remove_id);
            }
            Self::ReceiveMail { mail } => {
                writer.write_i32(mail.len() as i32);
                for mail in mail {
                    mail.encode(writer)?;
                }
            }
            Self::MailLockedItem { unique_id, locked } => {
                writer.write_u64(*unique_id);
                writer.write_bool(*locked);
            }
            Self::MailSendRequest => {}
            Self::MailSent { result } | Self::ParcelCollected { result } => {
                writer.write_u8(*result as u8);
            }
            Self::MailCost { cost } => writer.write_u32(*cost),
            Self::FriendUpdate { friends } => {
                writer.write_i32(friends.len() as i32);
                for friend in friends {
                    friend.encode(writer)?;
                }
            }
            Self::LoverUpdate {
                name,
                date_binary_datetime,
                map_name,
                married_days,
            } => {
                writer.write_string(name)?;
                writer.write_i64(*date_binary_datetime);
                writer.write_string(map_name)?;
                writer.write_i16(*married_days);
            }
            Self::MentorUpdate {
                name,
                level,
                online,
                mentee_exp,
            } => {
                writer.write_string(name)?;
                writer.write_u16(*level);
                writer.write_bool(*online);
                writer.write_i64(*mentee_exp);
            }
            Self::GuildBuffList {
                remove,
                active_buffs,
                guild_buffs,
            } => {
                writer.write_u8(*remove);
                writer.write_i32(active_buffs.len() as i32);
                for buff in active_buffs {
                    buff.encode(writer);
                }
                writer.write_i32(guild_buffs.len() as i32);
                for buff in guild_buffs {
                    buff.encode(writer)?;
                }
            }
            Self::GameShopInfo { item, stock_level } => {
                item.encode(writer)?;
                writer.write_i32(*stock_level);
            }
            Self::NewIntelligentCreature { creature } => creature.encode(writer)?,
            Self::UpdateIntelligentCreatureList {
                creature_list,
                creature_summoned,
                summoned_creature_type,
                pearl_count,
            } => {
                writer.write_i32(creature_list.len() as i32);
                for creature in creature_list {
                    creature.encode(writer)?;
                }
                writer.write_bool(*creature_summoned);
                writer.write_u8(*summoned_creature_type);
                writer.write_i32(*pearl_count);
            }
            Self::IntelligentCreatureEnableRename => {}
            Self::IntelligentCreaturePickup { object_id } => writer.write_u32(*object_id),
            Self::GetRentedItems { rented_items } => {
                writer.write_i32(rented_items.len() as i32);
                for rented_item in rented_items {
                    rented_item.encode(writer)?;
                }
            }
            Self::ItemRentalRequest { name, renting } => {
                writer.write_string(name)?;
                writer.write_bool(*renting);
            }
            Self::ItemRentalFee { amount } => writer.write_u32(*amount),
            Self::ItemRentalPeriod { days } => writer.write_u32(*days),
            Self::DepositRentalItem { from, to, success }
            | Self::RetrieveRentalItem { from, to, success } => {
                writer.write_i32(*from);
                writer.write_i32(*to);
                writer.write_bool(*success);
            }
            Self::UpdateRentalItem { loan_item } => {
                writer.write_bool(loan_item.is_some());
                if let Some(item) = loan_item {
                    item.encode(writer)?;
                }
            }
            Self::CancelItemRental | Self::CanConfirmItemRental | Self::ConfirmItemRental => {}
            Self::ItemRentalLock {
                success,
                gold_locked,
                item_locked,
            } => {
                writer.write_bool(*success);
                writer.write_bool(*gold_locked);
                writer.write_bool(*item_locked);
            }
            Self::ItemRentalPartnerLock {
                gold_locked,
                item_locked,
            } => {
                writer.write_bool(*gold_locked);
                writer.write_bool(*item_locked);
            }
            Self::ChangeQuest {
                quest_id,
                task_list,
                taken,
                completed,
                new,
                quest_state,
                track_quest,
            } => {
                writer.write_i32(*quest_id);
                writer.write_i32(task_list.len() as i32);
                for task in task_list {
                    writer.write_string(task)?;
                }
                writer.write_bool(*taken);
                writer.write_bool(*completed);
                writer.write_bool(*new);
                writer.write_u8(*quest_state);
                writer.write_bool(*track_quest);
            }
            Self::CompleteQuest { completed_quests } => {
                writer.write_i32(completed_quests.len() as i32);
                for quest_id in completed_quests {
                    writer.write_i32(*quest_id);
                }
            }
            Self::ShareQuest {
                quest_index,
                sharer_name,
            } => {
                writer.write_i32(*quest_index);
                writer.write_string(sharer_name)?;
            }
            Self::NewQuestInfo { info } => info.encode(writer)?,
            Self::NewRecipeInfo { info } => info.encode(writer)?,
            Self::ResizeInventory { size } => writer.write_i32(*size),
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
            ServerPacketId::TimeOfDay => Self::TimeOfDay {
                lights: reader.read_u8()?,
            },
            ServerPacketId::ChangeAMode => Self::ChangeAMode {
                mode: reader.read_u8()?,
            },
            ServerPacketId::ChangePMode => Self::ChangePMode {
                mode: reader.read_u8()?,
            },
            ServerPacketId::BaseStatsInfo => Self::BaseStatsInfo {
                stats: BaseStats::decode(reader)?,
            },
            ServerPacketId::HeroBaseStatsInfo => Self::HeroBaseStatsInfo {
                stats: BaseStats::decode(reader)?,
            },
            ServerPacketId::HeroInformation => Self::HeroInformation {
                info: HeroUserInformation::decode(reader)?,
            },
            ServerPacketId::NPCMarket => {
                let count = reader.read_i32()?;
                if count < 0 {
                    return Err(PacketCodecError::NegativeLength {
                        field: "npc_market_listings",
                        value: count,
                    });
                }
                let mut listings = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    listings.push(ClientAuction::decode(reader)?);
                }
                Self::NPCMarket {
                    listings,
                    pages: reader.read_i32()?,
                    user_mode: reader.read_bool()?,
                }
            }
            ServerPacketId::NPCMarketPage => {
                let count = reader.read_i32()?;
                if count < 0 {
                    return Err(PacketCodecError::NegativeLength {
                        field: "npc_market_page_listings",
                        value: count,
                    });
                }
                let mut listings = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    listings.push(ClientAuction::decode(reader)?);
                }
                Self::NPCMarketPage { listings }
            }
            ServerPacketId::NewMapInfo => Self::NewMapInfo {
                map_index: reader.read_i32()?,
                info: ClientMapInfo::decode(reader)?,
            },
            ServerPacketId::WorldMapSetup => Self::WorldMapSetup {
                setup: WorldMapSetup::decode(reader)?,
                teleport_to_npc_cost: reader.read_i32()?,
            },
            ServerPacketId::SearchMapResult => Self::SearchMapResult {
                map_index: reader.read_i32()?,
                npc_index: reader.read_u32()?,
            },
            ServerPacketId::UserSlotsRefresh => Self::UserSlotsRefresh {
                inventory: decode_optional_user_item_slots(reader, "user_slots_inventory")?,
                equipment: decode_optional_user_item_slots(reader, "user_slots_equipment")?,
            },
            ServerPacketId::NewChatItem => Self::NewChatItem {
                item: UserItem::decode(reader)?,
            },
            ServerPacketId::PlayerUpdate => Self::PlayerUpdate {
                object_id: reader.read_u32()?,
                light: reader.read_u8()?,
                weapon: reader.read_i16()?,
                weapon_effect: reader.read_i16()?,
                armour: reader.read_i16()?,
                wing_effect: reader.read_u8()?,
            },
            ServerPacketId::PlayerInspect => Self::PlayerInspect {
                info: PlayerInspectInfo::decode(reader)?,
            },
            ServerPacketId::ReturnToLogin => Self::ReturnToLogin,
            ServerPacketId::DamageIndicator => Self::DamageIndicator {
                damage: reader.read_i32()?,
                damage_type: reader.read_u8()?,
                object_id: reader.read_u32()?,
            },
            ServerPacketId::HealthChanged => Self::HealthChanged {
                hp: reader.read_i32()?,
                mp: reader.read_i32()?,
            },
            ServerPacketId::Death => Self::Death {
                location: Point::decode(reader)?,
                direction: MirDirection::try_from(reader.read_u8()?)?,
            },
            ServerPacketId::ColourChanged => Self::ColourChanged {
                name_colour_argb: reader.read_i32()?,
            },
            ServerPacketId::ObjectColourChanged => Self::ObjectColourChanged {
                object_id: reader.read_u32()?,
                name_colour_argb: reader.read_i32()?,
            },
            ServerPacketId::ObjectGuildNameChanged => Self::ObjectGuildNameChanged {
                object_id: reader.read_u32()?,
                guild_name: reader.read_string()?,
            },
            ServerPacketId::GainExperience => Self::GainExperience {
                amount: reader.read_u32()?,
            },
            ServerPacketId::LevelChanged => Self::LevelChanged {
                level: reader.read_u16()?,
                experience: reader.read_i64()?,
                max_experience: reader.read_i64()?,
            },
            ServerPacketId::ObjectLeveled => Self::ObjectLeveled {
                object_id: reader.read_u32()?,
            },
            ServerPacketId::Poisoned => Self::Poisoned {
                poison: reader.read_u16()?,
            },
            ServerPacketId::ObjectPoisoned => Self::ObjectPoisoned {
                object_id: reader.read_u32()?,
                poison: reader.read_u16()?,
            },
            ServerPacketId::MapChanged => Self::MapChanged {
                map_index: reader.read_i32()?,
                file_name: reader.read_string()?,
                title: reader.read_string()?,
                mini_map: reader.read_u16()?,
                big_map: reader.read_u16()?,
                lights: reader.read_u8()?,
                location: Point::decode(reader)?,
                direction: MirDirection::try_from(reader.read_u8()?)?,
                map_dark_light: reader.read_u8()?,
                music: reader.read_u16()?,
                weather: reader.read_u16()?,
            },
            ServerPacketId::ObjectName => Self::ObjectName {
                object_id: reader.read_u32()?,
                name: reader.read_string()?,
            },
            ServerPacketId::Revived => Self::Revived,
            ServerPacketId::NPCConsign => Self::NPCConsign,
            ServerPacketId::InTrapRock => Self::InTrapRock {
                trapped: reader.read_bool()?,
            },
            ServerPacketId::UserName => Self::UserName {
                id: reader.read_u32()?,
                name: reader.read_string()?,
            },
            ServerPacketId::ChatItemStats => {
                let chat_item_id = reader.read_u64()?;
                let stats = if reader.remaining() == 0 {
                    None
                } else {
                    Some(UserItem::decode(reader)?)
                };
                Self::ChatItemStats {
                    chat_item_id,
                    stats,
                }
            }
            ServerPacketId::GuildNoticeChange => {
                let update = reader.read_i32()?;
                let mut notice = Vec::new();
                if update > 0 {
                    notice.reserve(update as usize);
                    for _ in 0..update {
                        notice.push(reader.read_string()?);
                    }
                }
                Self::GuildNoticeChange { update, notice }
            }
            ServerPacketId::GuildMemberChange => {
                let name = reader.read_string()?;
                let rank_index = reader.read_u8()?;
                let status = reader.read_u8()?;
                let ranks = if status > 5 {
                    let count = reader.read_i32()?;
                    if count < 0 {
                        return Err(PacketCodecError::NegativeLength {
                            field: "guild_member_change_ranks",
                            value: count,
                        });
                    }
                    let mut ranks = Vec::with_capacity(count as usize);
                    for _ in 0..count {
                        ranks.push(GuildRank::decode(reader)?);
                    }
                    ranks
                } else {
                    Vec::new()
                };
                Self::GuildMemberChange {
                    name,
                    rank_index,
                    status,
                    ranks,
                }
            }
            ServerPacketId::GuildStatus => Self::GuildStatus {
                guild_name: reader.read_string()?,
                guild_rank_name: reader.read_string()?,
                level: reader.read_u8()?,
                experience: reader.read_i64()?,
                max_experience: reader.read_i64()?,
                gold: reader.read_u32()?,
                spare_points: reader.read_u8()?,
                member_count: reader.read_i32()?,
                max_members: reader.read_i32()?,
                voting: reader.read_bool()?,
                item_count: reader.read_u8()?,
                buff_count: reader.read_u8()?,
                my_options: reader.read_u8()?,
                my_rank_id: reader.read_i32()?,
            },
            ServerPacketId::GuildInvite => Self::GuildInvite {
                name: reader.read_string()?,
            },
            ServerPacketId::GuildExpGain => Self::GuildExpGain {
                amount: reader.read_u32()?,
            },
            ServerPacketId::GuildNameRequest => Self::GuildNameRequest,
            ServerPacketId::GuildStorageGoldChange => Self::GuildStorageGoldChange {
                amount: reader.read_u32()?,
                change_type: reader.read_u8()?,
                name: reader.read_string()?,
            },
            ServerPacketId::GuildStorageItemChange => {
                let change_type = reader.read_u8()?;
                let to = reader.read_i32()?;
                let from = reader.read_i32()?;
                let user = reader.read_i32()?;
                let item = if reader.read_bool()? {
                    let user_id = reader.read_i64()?;
                    Some(GuildStorageItem {
                        user_id,
                        item: UserItem::decode(reader)?,
                    })
                } else {
                    None
                };
                Self::GuildStorageItemChange {
                    change_type,
                    to,
                    from,
                    user,
                    item,
                }
            }
            ServerPacketId::GuildStorageList => {
                let count = reader.read_i32()?;
                if count < 0 {
                    return Err(PacketCodecError::NegativeLength {
                        field: "guild_storage_items",
                        value: count,
                    });
                }
                let mut items = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    if reader.read_bool()? {
                        items.push(Some(GuildStorageItem::decode(reader)?));
                    } else {
                        items.push(None);
                    }
                }
                Self::GuildStorageList { items }
            }
            ServerPacketId::GuildRequestWar => Self::GuildRequestWar,
            ServerPacketId::SetAutoPotValue => Self::SetAutoPotValue {
                stat: reader.read_u8()?,
                value: reader.read_u32()?,
            },
            ServerPacketId::SetAutoPotItem => Self::SetAutoPotItem {
                grid: reader.read_u8()?,
                item_index: reader.read_i32()?,
            },
            ServerPacketId::NPCImageUpdate => Self::NPCImageUpdate {
                object_id: reader.read_u32()?,
                image: reader.read_u16()?,
                colour_argb: reader.read_i32()?,
            },
            ServerPacketId::EquipSlotItem => Self::EquipSlotItem {
                grid: MirGridType::try_from(reader.read_u8()?)?,
                unique_id: reader.read_u64()?,
                to: reader.read_i32()?,
                grid_to: MirGridType::try_from(reader.read_u8()?)?,
                success: reader.read_bool()?,
            },
            ServerPacketId::GainedQuestItem => Self::GainedQuestItem {
                item: UserItem::decode(reader)?,
            },
            ServerPacketId::DeleteQuestItem => Self::DeleteQuestItem {
                unique_id: reader.read_u64()?,
                count: reader.read_u16()?,
            },
            ServerPacketId::CancelReincarnation => Self::CancelReincarnation,
            ServerPacketId::RequestReincarnation => Self::RequestReincarnation,
            ServerPacketId::UserBackStep => Self::UserBackStep {
                location: Point::decode(reader)?,
                direction: MirDirection::try_from(reader.read_u8()?)?,
            },
            ServerPacketId::UserDashAttack => Self::UserDashAttack {
                location: Point::decode(reader)?,
                direction: MirDirection::try_from(reader.read_u8()?)?,
            },
            ServerPacketId::ObjectDashAttack => Self::ObjectDashAttack {
                object_id: reader.read_u32()?,
                location: Point::decode(reader)?,
                direction: MirDirection::try_from(reader.read_u8()?)?,
                distance: reader.read_i32()?,
            },
            ServerPacketId::UserAttackMove => Self::UserAttackMove {
                location: Point::decode(reader)?,
                direction: MirDirection::try_from(reader.read_u8()?)?,
            },
            ServerPacketId::SetConcentration => Self::SetConcentration {
                object_id: reader.read_u32()?,
                enabled: reader.read_bool()?,
                interrupted: reader.read_bool()?,
            },
            ServerPacketId::SetElemental => Self::SetElemental {
                object_id: reader.read_u32()?,
                enabled: reader.read_bool()?,
                casted: reader.read_bool()?,
                value: reader.read_u32()?,
                element_type: reader.read_u32()?,
                exp_last: reader.read_u32()?,
            },
            ServerPacketId::AwakeningNeedMaterials => {
                let materials = if reader.read_bool()? {
                    let count = reader.read_i32()?;
                    if count < 0 {
                        return Err(PacketCodecError::NegativeLength {
                            field: "awakening_materials",
                            value: count,
                        });
                    }
                    let mut materials = Vec::with_capacity(count as usize);
                    for _ in 0..count {
                        if reader.read_bool()? {
                            materials.push(Some(AwakeningMaterial::decode(reader)?));
                        } else {
                            materials.push(None);
                        }
                    }
                    Some(materials)
                } else {
                    None
                };
                Self::AwakeningNeedMaterials { materials }
            }
            ServerPacketId::NPCPearlGoods => {
                let count = reader.read_i32()?;
                if count < 0 {
                    return Err(PacketCodecError::NegativeLength {
                        field: "npc_pearl_goods",
                        value: count,
                    });
                }
                let mut list = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    list.push(UserItem::decode(reader)?);
                }
                Self::NPCPearlGoods {
                    list,
                    rate: reader.read_f32()?,
                    panel_type: reader.read_u8()?,
                }
            }
            ServerPacketId::TransformUpdate => Self::TransformUpdate {
                object_id: reader.read_u32()?,
                transform_type: reader.read_i16()?,
            },
            ServerPacketId::NPCRequestInput => Self::NPCRequestInput {
                npc_id: reader.read_u32()?,
                page_name: reader.read_string()?,
            },
            ServerPacketId::GameShopStock => Self::GameShopStock {
                g_index: reader.read_i32()?,
                stock_level: reader.read_i32()?,
            },
            ServerPacketId::Rankings => {
                let rank_type = reader.read_u8()?;
                let my_rank = reader.read_i32()?;
                let detail_count = reader.read_i32()?;
                if detail_count < 0 {
                    return Err(PacketCodecError::NegativeLength {
                        field: "ranking_details",
                        value: detail_count,
                    });
                }
                let mut listing_details = Vec::with_capacity(detail_count as usize);
                for _ in 0..detail_count {
                    listing_details.push(RankCharacterInfo::decode(reader)?);
                }
                let listing_count = reader.read_i32()?;
                if listing_count < 0 {
                    return Err(PacketCodecError::NegativeLength {
                        field: "ranking_listings",
                        value: listing_count,
                    });
                }
                let mut listings = Vec::with_capacity(listing_count as usize);
                for _ in 0..listing_count {
                    listings.push(reader.read_i64()?);
                }
                Self::Rankings {
                    rank_type,
                    my_rank,
                    listing_details,
                    listings,
                    count: reader.read_i32()?,
                }
            }
            ServerPacketId::UpdateNotice => Self::UpdateNotice {
                notice: Notice::decode(reader)?,
            },
            ServerPacketId::GuildTerritoryPage => {
                let length = reader.read_i32()?;
                let count = reader.read_i32()?;
                if count < 0 {
                    return Err(PacketCodecError::NegativeLength {
                        field: "guild_territory_page",
                        value: count,
                    });
                }
                let mut listings = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    listings.push(ClientGtMap::decode(reader)?);
                }
                Self::GuildTerritoryPage { length, listings }
            }
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
            ServerPacketId::ObjectHero => Self::ObjectHero {
                info: ObjectPlayerInfo::decode(reader)?,
                owner_name: reader.read_string()?,
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
            ServerPacketId::DepositRefineItem => Self::DepositRefineItem {
                from: reader.read_i32()?,
                to: reader.read_i32()?,
                success: reader.read_bool()?,
            },
            ServerPacketId::RetrieveRefineItem => Self::RetrieveRefineItem {
                from: reader.read_i32()?,
                to: reader.read_i32()?,
                success: reader.read_bool()?,
            },
            ServerPacketId::RefineCancel => Self::RefineCancel,
            ServerPacketId::RefineItem => Self::RefineItem {
                unique_id: reader.read_u64()?,
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
            ServerPacketId::DepositTradeItem => Self::DepositTradeItem {
                from: reader.read_i32()?,
                to: reader.read_i32()?,
                success: reader.read_bool()?,
            },
            ServerPacketId::RetrieveTradeItem => Self::RetrieveTradeItem {
                from: reader.read_i32()?,
                to: reader.read_i32()?,
                success: reader.read_bool()?,
            },
            ServerPacketId::TakeBackHeroItem => Self::TakeBackHeroItem {
                from: reader.read_i32()?,
                to: reader.read_i32()?,
                success: reader.read_bool()?,
            },
            ServerPacketId::TransferHeroItem => Self::TransferHeroItem {
                from: reader.read_i32()?,
                to: reader.read_i32()?,
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
            ServerPacketId::NewHeroInfo => Self::NewHeroInfo {
                info: ClientHeroInformation::decode(reader)?,
                storage_index: reader.read_i32()?,
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
            ServerPacketId::HeroHealthChanged => Self::HeroHealthChanged {
                hp: reader.read_i32()?,
                mp: reader.read_i32()?,
            },
            ServerPacketId::DeleteItem => Self::DeleteItem {
                unique_id: reader.read_u64()?,
                count: reader.read_u16()?,
            },
            ServerPacketId::ObjectDied => Self::ObjectDied {
                info: ObjectDiedInfo::decode(reader)?,
            },
            ServerPacketId::GainHeroExperience => Self::GainHeroExperience {
                amount: reader.read_u32()?,
            },
            ServerPacketId::HeroLevelChanged => Self::HeroLevelChanged {
                level: reader.read_u16()?,
                experience: reader.read_i64()?,
                max_experience: reader.read_i64()?,
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
            ServerPacketId::NPCResponse => {
                let count = reader.read_i32()?;
                if count < 0 {
                    return Err(PacketCodecError::NegativeLength {
                        field: "npc_response_page",
                        value: count,
                    });
                }
                let mut page = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    page.push(reader.read_string()?);
                }
                Self::NPCResponse { page }
            }
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
            ServerPacketId::NewMagic => Self::NewMagic {
                magic: ClientMagic::decode(reader)?,
                hero: reader.read_bool()?,
            },
            ServerPacketId::RemoveMagic => Self::RemoveMagic {
                place_id: reader.read_i32()?,
            },
            ServerPacketId::MagicLeveled => Self::MagicLeveled {
                object_id: reader.read_u32()?,
                spell: Spell::try_from(reader.read_u8()?)?,
                level: reader.read_u8()?,
                experience: reader.read_u16()?,
            },
            ServerPacketId::Magic => {
                let spell = Spell::try_from(reader.read_u8()?)?;
                let target_id = reader.read_u32()?;
                let target = Point::decode(reader)?;
                let cast = reader.read_bool()?;
                let level = reader.read_u8()?;
                let count = reader.read_i32()?;
                if count < 0 {
                    return Err(PacketCodecError::NegativeLength {
                        field: "magic_secondary_target_ids",
                        value: count,
                    });
                }
                let mut secondary_target_ids = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    secondary_target_ids.push(reader.read_u32()?);
                }
                Self::Magic {
                    spell,
                    target_id,
                    target,
                    cast,
                    level,
                    secondary_target_ids,
                }
            }
            ServerPacketId::MagicDelay => Self::MagicDelay {
                object_id: reader.read_u32()?,
                spell: Spell::try_from(reader.read_u8()?)?,
                delay: reader.read_i64()?,
            },
            ServerPacketId::MagicCast => Self::MagicCast {
                spell: Spell::try_from(reader.read_u8()?)?,
            },
            ServerPacketId::ObjectMagic => {
                let object_id = reader.read_u32()?;
                let location = Point::decode(reader)?;
                let direction = MirDirection::try_from(reader.read_u8()?)?;
                let spell = Spell::try_from(reader.read_u8()?)?;
                let target_id = reader.read_u32()?;
                let target = Point::decode(reader)?;
                let cast = reader.read_bool()?;
                let level = reader.read_u8()?;
                let self_broadcast = reader.read_bool()?;
                let count = reader.read_i32()?;
                if count < 0 {
                    return Err(PacketCodecError::NegativeLength {
                        field: "object_magic_secondary_target_ids",
                        value: count,
                    });
                }
                let mut secondary_target_ids = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    secondary_target_ids.push(reader.read_u32()?);
                }
                Self::ObjectMagic {
                    object_id,
                    location,
                    direction,
                    spell,
                    target_id,
                    target,
                    cast,
                    level,
                    self_broadcast,
                    secondary_target_ids,
                }
            }
            ServerPacketId::ObjectProjectile => Self::ObjectProjectile {
                spell: Spell::try_from(reader.read_u8()?)?,
                source_id: reader.read_u32()?,
                destination_id: reader.read_u32()?,
            },
            ServerPacketId::RangeAttack => Self::RangeAttack {
                target_id: reader.read_u32()?,
                target: Point::decode(reader)?,
                spell: Spell::try_from(reader.read_u8()?)?,
            },
            ServerPacketId::Pushed => Self::Pushed {
                location: Point::decode(reader)?,
                direction: MirDirection::try_from(reader.read_u8()?)?,
            },
            ServerPacketId::ObjectPushed => Self::ObjectPushed {
                object_id: reader.read_u32()?,
                location: Point::decode(reader)?,
                direction: MirDirection::try_from(reader.read_u8()?)?,
            },
            ServerPacketId::SpellToggle => Self::SpellToggle {
                object_id: reader.read_u32()?,
                spell: Spell::try_from(reader.read_u8()?)?,
                can_use: reader.read_bool()?,
            },
            ServerPacketId::SwitchGroup => Self::SwitchGroup {
                allow_group: reader.read_bool()?,
            },
            ServerPacketId::DeleteGroup => Self::DeleteGroup,
            ServerPacketId::DeleteMember => Self::DeleteMember {
                name: reader.read_string()?,
            },
            ServerPacketId::GroupInvite => Self::GroupInvite {
                name: reader.read_string()?,
            },
            ServerPacketId::AddMember => Self::AddMember {
                name: reader.read_string()?,
            },
            ServerPacketId::GroupMembersMap => Self::GroupMembersMap {
                player_name: reader.read_string()?,
                player_map: reader.read_string()?,
            },
            ServerPacketId::SendMemberLocation => Self::SendMemberLocation {
                member_name: reader.read_string()?,
                member_location: Point::decode(reader)?,
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
            ServerPacketId::ObjectMana => Self::ObjectMana {
                info: ObjectManaInfo::decode(reader)?,
            },
            ServerPacketId::MapEffect => Self::MapEffect {
                location: Point::decode(reader)?,
                effect: reader.read_u8()?,
                value: reader.read_u8()?,
            },
            ServerPacketId::AllowObserve => Self::AllowObserve {
                allow: reader.read_bool()?,
            },
            ServerPacketId::ObjectRangeAttack => Self::ObjectRangeAttack {
                info: ObjectRangeAttackInfo::decode(reader)?,
            },
            ServerPacketId::AddBuff => Self::AddBuff {
                buff: ClientBuff::decode(reader)?,
            },
            ServerPacketId::RemoveBuff => Self::RemoveBuff {
                buff_type: reader.read_u8()?,
                object_id: reader.read_u32()?,
            },
            ServerPacketId::PauseBuff => Self::PauseBuff {
                buff_type: reader.read_u8()?,
                object_id: reader.read_u32()?,
                paused: reader.read_bool()?,
            },
            ServerPacketId::ObjectHidden => Self::ObjectHidden {
                object_id: reader.read_u32()?,
                hidden: reader.read_bool()?,
            },
            ServerPacketId::RefreshItem => Self::RefreshItem {
                item: UserItem::decode(reader)?,
            },
            ServerPacketId::ObjectSpell => Self::ObjectSpell {
                info: ObjectSpellInfo::decode(reader)?,
            },
            ServerPacketId::UserDash => Self::UserDash {
                location: Point::decode(reader)?,
                direction: MirDirection::try_from(reader.read_u8()?)?,
            },
            ServerPacketId::ObjectDash => Self::ObjectDash {
                object_id: reader.read_u32()?,
                location: Point::decode(reader)?,
                direction: MirDirection::try_from(reader.read_u8()?)?,
            },
            ServerPacketId::UserDashFail => Self::UserDashFail {
                location: Point::decode(reader)?,
                direction: MirDirection::try_from(reader.read_u8()?)?,
            },
            ServerPacketId::ObjectDashFail => Self::ObjectDashFail {
                object_id: reader.read_u32()?,
                location: Point::decode(reader)?,
                direction: MirDirection::try_from(reader.read_u8()?)?,
            },
            ServerPacketId::ConsignItem => Self::ConsignItem {
                unique_id: reader.read_u64()?,
                success: reader.read_bool()?,
            },
            ServerPacketId::MarketFail => Self::MarketFail {
                reason: reader.read_u8()?,
            },
            ServerPacketId::MarketSuccess => Self::MarketSuccess {
                message: reader.read_string()?,
            },
            ServerPacketId::HeroCreateRequest => {
                let count = reader.read_i32()?;
                if count < 0 {
                    return Err(PacketCodecError::NegativeLength {
                        field: "hero_create_class_count",
                        value: count,
                    });
                }
                let mut can_create_class = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    can_create_class.push(reader.read_bool()?);
                }
                Self::HeroCreateRequest { can_create_class }
            }
            ServerPacketId::NewHero => Self::NewHero {
                result: reader.read_u8()?,
            },
            ServerPacketId::UpdateHeroSpawnState => Self::UpdateHeroSpawnState {
                state: reader.read_u8()?,
            },
            ServerPacketId::UnlockHeroAutoPot => Self::UnlockHeroAutoPot,
            ServerPacketId::SetHeroBehaviour => Self::SetHeroBehaviour {
                behaviour: reader.read_u8()?,
            },
            ServerPacketId::ManageHeroes => {
                let maximum_count = reader.read_i32()?;
                let current_hero = if reader.read_bool()? {
                    Some(ClientHeroInformation::decode(reader)?)
                } else {
                    None
                };
                let heroes = if reader.read_bool()? {
                    let count = reader.read_i32()?;
                    if count < 0 {
                        return Err(PacketCodecError::NegativeLength {
                            field: "manage_heroes",
                            value: count,
                        });
                    }
                    let mut heroes = Vec::with_capacity(count as usize);
                    for _ in 0..count {
                        let hero = if reader.read_bool()? {
                            Some(ClientHeroInformation::decode(reader)?)
                        } else {
                            None
                        };
                        heroes.push(hero);
                    }
                    Some(heroes)
                } else {
                    None
                };
                Self::ManageHeroes {
                    maximum_count,
                    current_hero,
                    heroes,
                }
            }
            ServerPacketId::ChangeHero => Self::ChangeHero {
                from_index: reader.read_i32()?,
            },
            ServerPacketId::DefaultNPC => Self::DefaultNPC {
                object_id: reader.read_u32()?,
            },
            ServerPacketId::NPCUpdate => Self::NPCUpdate {
                npc_id: reader.read_u32()?,
            },
            ServerPacketId::MarriageRequest => Self::MarriageRequest {
                name: reader.read_string()?,
            },
            ServerPacketId::DivorceRequest => Self::DivorceRequest {
                name: reader.read_string()?,
            },
            ServerPacketId::MentorRequest => Self::MentorRequest {
                name: reader.read_string()?,
                level: reader.read_u16()?,
            },
            ServerPacketId::TradeRequest => Self::TradeRequest {
                name: reader.read_string()?,
            },
            ServerPacketId::TradeAccept => Self::TradeAccept {
                name: reader.read_string()?,
            },
            ServerPacketId::TradeGold => Self::TradeGold {
                amount: reader.read_u32()?,
            },
            ServerPacketId::TradeItem => {
                let count = reader.read_i32()?;
                if count < 0 {
                    return Err(PacketCodecError::NegativeLength {
                        field: "trade_items",
                        value: count,
                    });
                }
                let mut trade_items = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    let item = if reader.read_bool()? {
                        Some(UserItem::decode(reader)?)
                    } else {
                        None
                    };
                    trade_items.push(item);
                }
                Self::TradeItem { trade_items }
            }
            ServerPacketId::TradeConfirm => Self::TradeConfirm,
            ServerPacketId::TradeCancel => Self::TradeCancel {
                unlock: reader.read_bool()?,
            },
            ServerPacketId::MountUpdate => Self::MountUpdate {
                object_id: reader.read_u32()?,
                mount_type: reader.read_i16()?,
                riding_mount: reader.read_bool()?,
            },
            ServerPacketId::FishingUpdate => Self::FishingUpdate {
                object_id: reader.read_u32()?,
                fishing: reader.read_bool()?,
                progress_percent: reader.read_i32()?,
                chance_percent: reader.read_i32()?,
                fishing_point: Point::decode(reader)?,
                found_fish: reader.read_bool()?,
            },
            ServerPacketId::RemoveDelayedExplosion => Self::RemoveDelayedExplosion {
                object_id: reader.read_u32()?,
            },
            ServerPacketId::ObjectDeco => Self::ObjectDeco {
                object_id: reader.read_u32()?,
                location: Point::decode(reader)?,
                image: reader.read_i32()?,
            },
            ServerPacketId::ObjectSneaking => Self::ObjectSneaking {
                object_id: reader.read_u32()?,
                sneaking_active: reader.read_bool()?,
            },
            ServerPacketId::ObjectLevelEffects => Self::ObjectLevelEffects {
                object_id: reader.read_u32()?,
                level_effects: reader.read_u16()?,
            },
            ServerPacketId::SetBindingShot => Self::SetBindingShot {
                object_id: reader.read_u32()?,
                enabled: reader.read_bool()?,
                value: reader.read_i64()?,
            },
            ServerPacketId::SendOutputMessage => Self::SendOutputMessage {
                message: reader.read_string()?,
                output_type: reader.read_u8()?,
            },
            ServerPacketId::OpenDoor => Self::OpenDoor {
                door_index: reader.read_u8()?,
                close: reader.read_bool()?,
            },
            ServerPacketId::OpenBrowser => Self::OpenBrowser {
                url: reader.read_string()?,
            },
            ServerPacketId::PlaySound => Self::PlaySound {
                sound: reader.read_i32()?,
            },
            ServerPacketId::SetTimer => Self::SetTimer {
                key: reader.read_string()?,
                timer_type: reader.read_u8()?,
                seconds: reader.read_i32()?,
            },
            ServerPacketId::ExpireTimer => Self::ExpireTimer {
                key: reader.read_string()?,
            },
            ServerPacketId::Roll => Self::Roll {
                roll_type: reader.read_i32()?,
                page: reader.read_string()?,
                result: reader.read_i32()?,
                auto_roll: reader.read_bool()?,
            },
            ServerPacketId::SetCompass => Self::SetCompass {
                location: Point::decode(reader)?,
            },
            ServerPacketId::NPCAwakening => Self::NPCAwakening,
            ServerPacketId::NPCDisassemble => Self::NPCDisassemble,
            ServerPacketId::NPCDowngrade => Self::NPCDowngrade,
            ServerPacketId::NPCReset => Self::NPCReset,
            ServerPacketId::AwakeningLockedItem => Self::AwakeningLockedItem {
                unique_id: reader.read_u64()?,
                locked: reader.read_bool()?,
            },
            ServerPacketId::Awakening => Self::Awakening {
                result: reader.read_i32()?,
                remove_id: reader.read_i64()?,
            },
            ServerPacketId::ReceiveMail => {
                let count = reader.read_i32()?;
                if count < 0 {
                    return Err(PacketCodecError::NegativeLength {
                        field: "receive_mail",
                        value: count,
                    });
                }
                let mut mail = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    mail.push(ClientMail::decode(reader)?);
                }
                Self::ReceiveMail { mail }
            }
            ServerPacketId::MailLockedItem => Self::MailLockedItem {
                unique_id: reader.read_u64()?,
                locked: reader.read_bool()?,
            },
            ServerPacketId::MailSendRequest => Self::MailSendRequest,
            ServerPacketId::MailSent => Self::MailSent {
                result: reader.read_u8()? as i8,
            },
            ServerPacketId::ParcelCollected => Self::ParcelCollected {
                result: reader.read_u8()? as i8,
            },
            ServerPacketId::MailCost => Self::MailCost {
                cost: reader.read_u32()?,
            },
            ServerPacketId::FriendUpdate => {
                let count = reader.read_i32()?;
                if count < 0 {
                    return Err(PacketCodecError::NegativeLength {
                        field: "friend_update",
                        value: count,
                    });
                }
                let mut friends = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    friends.push(ClientFriend::decode(reader)?);
                }
                Self::FriendUpdate { friends }
            }
            ServerPacketId::LoverUpdate => Self::LoverUpdate {
                name: reader.read_string()?,
                date_binary_datetime: reader.read_i64()?,
                map_name: reader.read_string()?,
                married_days: reader.read_i16()?,
            },
            ServerPacketId::MentorUpdate => Self::MentorUpdate {
                name: reader.read_string()?,
                level: reader.read_u16()?,
                online: reader.read_bool()?,
                mentee_exp: reader.read_i64()?,
            },
            ServerPacketId::GuildBuffList => {
                let remove = reader.read_u8()?;
                let active_count = reader.read_i32()?;
                if active_count < 0 {
                    return Err(PacketCodecError::NegativeLength {
                        field: "guild_buff_active",
                        value: active_count,
                    });
                }
                let mut active_buffs = Vec::with_capacity(active_count as usize);
                for _ in 0..active_count {
                    active_buffs.push(GuildBuff::decode(reader)?);
                }
                let buff_count = reader.read_i32()?;
                if buff_count < 0 {
                    return Err(PacketCodecError::NegativeLength {
                        field: "guild_buff_info",
                        value: buff_count,
                    });
                }
                let mut guild_buffs = Vec::with_capacity(buff_count as usize);
                for _ in 0..buff_count {
                    guild_buffs.push(GuildBuffInfo::decode(reader)?);
                }
                Self::GuildBuffList {
                    remove,
                    active_buffs,
                    guild_buffs,
                }
            }
            ServerPacketId::GameShopInfo => Self::GameShopInfo {
                item: GameShopItem::decode(reader)?,
                stock_level: reader.read_i32()?,
            },
            ServerPacketId::NewIntelligentCreature => Self::NewIntelligentCreature {
                creature: ClientIntelligentCreature::decode(reader)?,
            },
            ServerPacketId::UpdateIntelligentCreatureList => {
                let count = reader.read_i32()?;
                if count < 0 {
                    return Err(PacketCodecError::NegativeLength {
                        field: "intelligent_creature_list",
                        value: count,
                    });
                }
                let mut creature_list = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    creature_list.push(ClientIntelligentCreature::decode(reader)?);
                }
                Self::UpdateIntelligentCreatureList {
                    creature_list,
                    creature_summoned: reader.read_bool()?,
                    summoned_creature_type: reader.read_u8()?,
                    pearl_count: reader.read_i32()?,
                }
            }
            ServerPacketId::IntelligentCreatureEnableRename => {
                Self::IntelligentCreatureEnableRename
            }
            ServerPacketId::IntelligentCreaturePickup => Self::IntelligentCreaturePickup {
                object_id: reader.read_u32()?,
            },
            ServerPacketId::GetRentedItems => {
                let count = reader.read_i32()?;
                if count < 0 {
                    return Err(PacketCodecError::NegativeLength {
                        field: "rented_items",
                        value: count,
                    });
                }
                let mut rented_items = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    rented_items.push(ItemRentalInformation::decode(reader)?);
                }
                Self::GetRentedItems { rented_items }
            }
            ServerPacketId::ItemRentalRequest => Self::ItemRentalRequest {
                name: reader.read_string()?,
                renting: reader.read_bool()?,
            },
            ServerPacketId::ItemRentalFee => Self::ItemRentalFee {
                amount: reader.read_u32()?,
            },
            ServerPacketId::ItemRentalPeriod => Self::ItemRentalPeriod {
                days: reader.read_u32()?,
            },
            ServerPacketId::DepositRentalItem => Self::DepositRentalItem {
                from: reader.read_i32()?,
                to: reader.read_i32()?,
                success: reader.read_bool()?,
            },
            ServerPacketId::RetrieveRentalItem => Self::RetrieveRentalItem {
                from: reader.read_i32()?,
                to: reader.read_i32()?,
                success: reader.read_bool()?,
            },
            ServerPacketId::UpdateRentalItem => {
                let loan_item = if reader.read_bool()? {
                    Some(UserItem::decode(reader)?)
                } else {
                    None
                };
                Self::UpdateRentalItem { loan_item }
            }
            ServerPacketId::CancelItemRental => Self::CancelItemRental,
            ServerPacketId::ItemRentalLock => Self::ItemRentalLock {
                success: reader.read_bool()?,
                gold_locked: reader.read_bool()?,
                item_locked: reader.read_bool()?,
            },
            ServerPacketId::ItemRentalPartnerLock => Self::ItemRentalPartnerLock {
                gold_locked: reader.read_bool()?,
                item_locked: reader.read_bool()?,
            },
            ServerPacketId::CanConfirmItemRental => Self::CanConfirmItemRental,
            ServerPacketId::ConfirmItemRental => Self::ConfirmItemRental,
            ServerPacketId::ChangeQuest => {
                let quest_id = reader.read_i32()?;
                let task_count = reader.read_i32()?.max(0);
                let mut task_list = Vec::with_capacity(task_count as usize);
                for _ in 0..task_count {
                    task_list.push(reader.read_string()?);
                }
                Self::ChangeQuest {
                    quest_id,
                    task_list,
                    taken: reader.read_bool()?,
                    completed: reader.read_bool()?,
                    new: reader.read_bool()?,
                    quest_state: reader.read_u8()?,
                    track_quest: reader.read_bool()?,
                }
            }
            ServerPacketId::CompleteQuest => {
                let count = reader.read_i32()?.max(0);
                let mut completed_quests = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    completed_quests.push(reader.read_i32()?);
                }
                Self::CompleteQuest { completed_quests }
            }
            ServerPacketId::ShareQuest => Self::ShareQuest {
                quest_index: reader.read_i32()?,
                sharer_name: reader.read_string()?,
            },
            ServerPacketId::NewQuestInfo => Self::NewQuestInfo {
                info: ClientQuestInfo::decode(reader)?,
            },
            ServerPacketId::NewRecipeInfo => Self::NewRecipeInfo {
                info: ClientRecipeInfo::decode(reader)?,
            },
            ServerPacketId::ResizeInventory => Self::ResizeInventory {
                size: reader.read_i32()?,
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

fn decode_optional_user_item_slots(
    reader: &mut PacketReader<'_>,
    field: &'static str,
) -> Result<Option<Vec<Option<UserItem>>>> {
    if !reader.read_bool()? {
        return Ok(None);
    }

    let count = reader.read_i32()?;
    if count < 0 {
        return Err(PacketCodecError::NegativeLength {
            field,
            value: count,
        });
    }

    let mut values = Vec::with_capacity(count as usize);
    for _ in 0..count {
        if reader.read_bool()? {
            values.push(Some(UserItem::decode(reader)?));
        } else {
            values.push(None);
        }
    }
    Ok(Some(values))
}

fn encode_optional_user_item_slots(
    writer: &mut PacketWriter,
    values: Option<&[Option<UserItem>]>,
) -> Result<()> {
    writer.write_bool(values.is_some());
    let Some(values) = values else {
        return Ok(());
    };
    writer.write_i32(values.len() as i32);
    for item in values {
        writer.write_bool(item.is_some());
        if let Some(item) = item {
            item.encode(writer)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_user_item(unique_id: u64) -> UserItem {
        UserItem {
            unique_id,
            item_index: 1,
            current_dura: 1_000,
            max_dura: 1_000,
            count: 1,
            soul_bound_id: -1,
            identified: true,
            cursed: false,
            slots: Vec::new(),
            gem_count: 0,
            added_stats: Vec::new(),
            awake_type: 0,
            awake_values: Vec::new(),
            refined_value: 0,
            refine_added: 0,
            refine_success_chance: 0,
            wedding_ring: -1,
            expire_info: None,
            rental_information: None,
            is_shop_item: false,
            sealed_info: None,
            gm_made: false,
        }
    }

    fn test_item_info(index: i32) -> ItemInfo {
        ItemInfo {
            index,
            name: format!("Item{index}"),
            item_type: 0,
            grade: 0,
            required_type: 0,
            required_class: 0,
            required_gender: 0,
            item_set: 0,
            shape: 1,
            weight: 1,
            light: 0,
            required_amount: 0,
            image: 1,
            durability: 1_000,
            stack_size: 1,
            price: 10,
            start_item: false,
            effect: 0,
            need_identify: false,
            show_group_pickup: false,
            class_based: false,
            level_based: false,
            can_mine: false,
            global_drop_notify: false,
            bind: 0,
            unique: 0,
            random_stats_id: 0,
            can_fast_run: false,
            can_awakening: false,
            slots: 0,
            stats: Vec::new(),
            tooltip: None,
        }
    }

    fn test_intelligent_creature(slot_index: i32) -> ClientIntelligentCreature {
        ClientIntelligentCreature {
            pet_type: 1,
            icon: 44,
            custom_name: "Buddy".to_string(),
            fullness: 50,
            slot_index,
            expire_binary_datetime: 638000000000000000,
            blackstone_time: 12_000,
            pet_mode: 1,
            creature_rules: crate::types::IntelligentCreatureRules {
                minimal_fullness: 1,
                mouse_pickup_enabled: true,
                mouse_pickup_range: 6,
                auto_pickup_enabled: false,
                auto_pickup_range: 0,
                semi_auto_pickup_enabled: true,
                semi_auto_pickup_range: 4,
                can_produce_blackstone: true,
            },
            filter: crate::types::IntelligentCreatureItemFilter {
                pet_pickup_all: false,
                pet_pickup_gold: true,
                pet_pickup_weapons: false,
                pet_pickup_armours: false,
                pet_pickup_helmets: false,
                pet_pickup_boots: false,
                pet_pickup_belts: false,
                pet_pickup_accessories: false,
                pet_pickup_others: true,
            },
            pickup_grade: 2,
            maintain_food_time: 24_000,
        }
    }

    fn test_hero_info(index: i32) -> ClientHeroInformation {
        ClientHeroInformation {
            index,
            name: format!("Hero{index}"),
            level: 12,
            class: MirClass::Taoist,
            gender: MirGender::Female,
        }
    }

    fn test_object_player_info(object_id: u32) -> ObjectPlayerInfo {
        ObjectPlayerInfo {
            object_id,
            name: "HeroOne".to_string(),
            guild_name: String::new(),
            guild_rank_name: String::new(),
            name_colour_argb: -1,
            class: MirClass::Warrior,
            gender: MirGender::Male,
            level: 12,
            location: Point { x: 330, y: 270 },
            direction: MirDirection::Down,
            hair: 0,
            light: 0,
            weapon: -1,
            weapon_effect: 0,
            armour: -1,
            poison: 0,
            dead: false,
            hidden: false,
            effect: 0,
            wing_effect: 0,
            extra: false,
            mount_type: -1,
            riding_mount: false,
            fishing: false,
            transform_type: -1,
            element_orb_effect: 0,
            element_orb_level: 0,
            element_orb_max: 0,
            buffs: vec![1, 2],
            level_effects: 0,
        }
    }

    #[test]
    fn crystal_packet_ids_cover_full_ranges_and_raw_server_frame_encoding() {
        for packet_id in 0..=152 {
            assert!(
                ClientPacketId::try_from(packet_id).is_ok(),
                "Crystal client packet id {packet_id} should be known"
            );
        }
        assert_eq!(ClientPacketId::CombineItem as i16, 110);

        for packet_id in 0..=278 {
            assert!(
                ServerPacketId::try_from(packet_id).is_ok(),
                "Crystal server packet id {packet_id} should be known"
            );
        }
        assert_eq!(ServerPacketId::CombineItem as i16, 214);
        assert_eq!(ServerPacketId::ItemUpgraded as i16, 215);

        let packet = ServerPacket::Raw {
            packet_id: ServerPacketId::Connected,
            payload: vec![1, 2, 3, 4],
        };
        let encoded = encode_server_packet(&packet).expect("raw server packet should encode");
        let frame = decode_frame(&encoded).expect("raw server packet frame should decode");
        assert_eq!(frame.packet_id, ServerPacketId::Connected as i16);
        assert_eq!(frame.payload, vec![1, 2, 3, 4]);
    }

    #[test]
    fn magic_client_packets_round_trip_with_crystal_ids() {
        let packets = vec![
            ClientPacket::MagicKey {
                spell: Spell::Fury,
                key: 7,
                old_key: 0,
            },
            ClientPacket::Magic {
                object_id: 1_001,
                spell: Spell::Healing,
                direction: MirDirection::DownLeft,
                target_id: 2_002,
                location: Point { x: 330, y: 270 },
                spell_target_lock: true,
            },
            ClientPacket::SpellToggle {
                spell: Spell::Slaying,
                toggle_state: -1,
            },
        ];

        assert_eq!(packets[0].packet_id() as i16, 57);
        assert_eq!(packets[1].packet_id() as i16, 58);
        assert_eq!(packets[2].packet_id() as i16, 69);

        for packet in packets {
            let encoded = encode_client_packet(&packet).expect("client packet should encode");
            let decoded = decode_client_packet(&encoded).expect("client packet should decode");
            assert_eq!(decoded, packet);
        }
    }

    #[test]
    fn magic_server_packets_round_trip_with_crystal_ids() {
        let magic = ClientMagic {
            name: "Fury".to_string(),
            spell: Spell::Fury,
            base_cost: 10,
            level_cost: 4,
            icon: 76,
            level1: 45,
            level2: 48,
            level3: 51,
            need1: 8_000,
            need2: 14_000,
            need3: 20_000,
            level: 3,
            key: 7,
            experience: 123,
            delay: 240_000,
            range: 0,
            cast_time: 42,
        };
        let packets = vec![
            ServerPacket::NewMagic { magic, hero: false },
            ServerPacket::RemoveMagic { place_id: 2 },
            ServerPacket::MagicLeveled {
                object_id: 1_001,
                spell: Spell::Fury,
                level: 3,
                experience: 123,
            },
            ServerPacket::Magic {
                spell: Spell::Healing,
                target_id: 1_001,
                target: Point { x: 330, y: 270 },
                cast: true,
                level: 2,
                secondary_target_ids: vec![1_002, 1_003],
            },
            ServerPacket::MagicDelay {
                object_id: 1_001,
                spell: Spell::Fury,
                delay: 240_000,
            },
            ServerPacket::MagicCast { spell: Spell::Fury },
            ServerPacket::ObjectMagic {
                object_id: 1_001,
                location: Point { x: 329, y: 269 },
                direction: MirDirection::Down,
                spell: Spell::Fury,
                target_id: 0,
                target: Point { x: 330, y: 270 },
                cast: true,
                level: 3,
                self_broadcast: false,
                secondary_target_ids: vec![1_002],
            },
            ServerPacket::SpellToggle {
                object_id: 1_001,
                spell: Spell::Slaying,
                can_use: true,
            },
            ServerPacket::ObjectMana {
                info: ObjectManaInfo {
                    object_id: 1_001,
                    percent: 76,
                },
            },
            ServerPacket::AddBuff {
                buff: ClientBuff {
                    buff_type: 5,
                    visible: true,
                    object_id: 1_001,
                    expire_time: 60_000,
                    infinite: false,
                    paused: false,
                    stats: vec![crate::types::UserItemStat { stat: 14, value: 4 }],
                    values: vec![7],
                },
            },
            ServerPacket::RemoveBuff {
                buff_type: 5,
                object_id: 1_001,
            },
        ];

        let expected_ids = [117, 118, 119, 120, 121, 122, 123, 138, 140, 144, 145];
        for (packet, expected_id) in packets.into_iter().zip(expected_ids) {
            assert_eq!(packet.packet_id() as i16, expected_id);
            let encoded = encode_server_packet(&packet).expect("server packet should encode");
            let decoded = decode_server_packet(&encoded).expect("server packet should decode");
            assert_eq!(decoded, packet);
        }
    }

    #[test]
    fn magic_visual_server_packets_round_trip_with_crystal_ids() {
        let packets = vec![
            ServerPacket::ObjectProjectile {
                spell: Spell::FireBall,
                source_id: 1,
                destination_id: 2,
            },
            ServerPacket::RangeAttack {
                target_id: 3,
                target: Point { x: 10, y: 11 },
                spell: Spell::StraightShot,
            },
            ServerPacket::Pushed {
                location: Point { x: 12, y: 13 },
                direction: MirDirection::Up,
            },
            ServerPacket::ObjectPushed {
                object_id: 4,
                location: Point { x: 14, y: 15 },
                direction: MirDirection::Down,
            },
            ServerPacket::MapEffect {
                location: Point { x: 16, y: 17 },
                effect: 2,
                value: 3,
            },
            ServerPacket::AllowObserve { allow: true },
            ServerPacket::PauseBuff {
                buff_type: 1,
                object_id: 5,
                paused: true,
            },
            ServerPacket::ObjectHidden {
                object_id: 6,
                hidden: false,
            },
            ServerPacket::UserDash {
                location: Point { x: 18, y: 19 },
                direction: MirDirection::Left,
            },
            ServerPacket::ObjectDash {
                object_id: 7,
                location: Point { x: 20, y: 21 },
                direction: MirDirection::Right,
            },
            ServerPacket::UserDashFail {
                location: Point { x: 22, y: 23 },
                direction: MirDirection::UpRight,
            },
            ServerPacket::ObjectDashFail {
                object_id: 8,
                location: Point { x: 24, y: 25 },
                direction: MirDirection::DownLeft,
            },
        ];

        let expected_ids = [125, 126, 127, 128, 141, 142, 146, 147, 150, 151, 152, 153];
        for (packet, expected_id) in packets.into_iter().zip(expected_ids) {
            assert_eq!(packet.packet_id() as i16, expected_id);
            let encoded = encode_server_packet(&packet).expect("server packet should encode");
            let decoded = decode_server_packet(&encoded).expect("server packet should decode");
            assert_eq!(decoded, packet);
        }
    }

    #[test]
    fn simple_bootstrap_social_server_packets_round_trip_with_crystal_ids() {
        let packets = vec![
            ServerPacket::TimeOfDay { lights: 4 },
            ServerPacket::ChangeAMode { mode: 1 },
            ServerPacket::ChangePMode { mode: 2 },
            ServerPacket::NPCResponse {
                page: vec!["@main".to_string(), "Welcome".to_string()],
            },
            ServerPacket::DefaultNPC { object_id: 100 },
            ServerPacket::NPCUpdate { npc_id: 101 },
            ServerPacket::LoverUpdate {
                name: "Partner".to_string(),
                date_binary_datetime: 42,
                map_name: "Bichon".to_string(),
                married_days: 7,
            },
            ServerPacket::MentorUpdate {
                name: "Mentor".to_string(),
                level: 45,
                online: true,
                mentee_exp: 1234,
            },
        ];

        let expected_ids = [61, 62, 63, 93, 186, 187, 246, 247];
        for (packet, expected_id) in packets.into_iter().zip(expected_ids) {
            assert_eq!(packet.packet_id() as i16, expected_id);
            let encoded = encode_server_packet(&packet).expect("server packet should encode");
            let decoded = decode_server_packet(&encoded).expect("server packet should decode");
            assert_eq!(decoded, packet);
        }
    }

    #[test]
    fn complex_bootstrap_market_guild_server_packets_round_trip_with_crystal_ids() {
        let base_stats = BaseStats {
            job: MirClass::Warrior,
            stats: vec![crate::types::BaseStat {
                stat: 12,
                formula_type: 0,
                base: 14,
                gain: 4.0,
                gain_rate: 4.5,
                max: 0,
            }],
            caps: vec![crate::types::UserItemStat {
                stat: 35,
                value: 18,
            }],
        };
        let auction = ClientAuction {
            auction_id: 77,
            item: test_user_item(77),
            seller: "Seller".to_string(),
            price: 1_000,
            consignment_date_binary_datetime: 638_000_000_000_000_000,
            item_type: 0,
        };
        let game_shop_item = GameShopItem {
            item_index: 1,
            g_index: 20,
            info: test_item_info(1),
            gold_price: 500,
            credit_price: 25,
            count: 2,
            class: "All".to_string(),
            category: "Potion".to_string(),
            stock: 10,
            i_stock: true,
            deal: false,
            top_item: true,
            date_binary_datetime: 638_000_000_000_000_000,
            can_buy_credit: true,
            can_buy_gold: true,
        };
        let packets = vec![
            ServerPacket::NPCMarket {
                listings: vec![auction.clone()],
                pages: 3,
                user_mode: true,
            },
            ServerPacket::NPCMarketPage {
                listings: vec![auction],
            },
            ServerPacket::BaseStatsInfo {
                stats: base_stats.clone(),
            },
            ServerPacket::HeroBaseStatsInfo { stats: base_stats },
            ServerPacket::HeroInformation {
                info: HeroUserInformation {
                    object_id: 1_001,
                    name: "Hero".to_string(),
                    class: MirClass::Taoist,
                    gender: MirGender::Female,
                    level: 22,
                    hair: 3,
                    hp: 300,
                    mp: 200,
                    experience: 12_345,
                    max_experience: 67_890,
                    inventory: Some(vec![Some(test_user_item(1)), None]),
                    equipment: Some(vec![Some(test_user_item(2))]),
                    magics: Vec::new(),
                    auto_pot: true,
                    auto_hp_percent: 70,
                    auto_mp_percent: 60,
                    hp_item_index: 1,
                    mp_item_index: 2,
                },
            },
            ServerPacket::GuildBuffList {
                remove: 0,
                active_buffs: vec![GuildBuff {
                    id: 1,
                    active: true,
                    active_time_remaining: 600,
                }],
                guild_buffs: vec![GuildBuffInfo {
                    id: 1,
                    icon: 10,
                    name: "Guard".to_string(),
                    level_requirement: 2,
                    points_requirement: 1,
                    time_limit: 3_600,
                    activation_cost: 100,
                    stats: vec![crate::types::UserItemStat { stat: 1, value: 2 }],
                }],
            },
            ServerPacket::GameShopInfo {
                item: game_shop_item,
                stock_level: 9,
            },
            ServerPacket::NewQuestInfo {
                info: ClientQuestInfo {
                    index: 1,
                    npc_index: 1_001,
                    name: "Field Wasp".to_string(),
                    group: "Starter".to_string(),
                    description: vec!["Help the town".to_string()],
                    task_description: vec!["Defeat one Wasp".to_string()],
                    return_description: vec!["Return to the guard".to_string()],
                    completion_description: vec!["Good work".to_string()],
                    min_level_needed: 1,
                    max_level_needed: 0,
                    quest_needed: 0,
                    class_needed: 31,
                    quest_type: 0,
                    time_limit_in_seconds: 0,
                    reward_gold: 100,
                    reward_exp: 200,
                    reward_credit: 0,
                    rewards_fixed_item: vec![crate::types::QuestItemReward {
                        item: test_item_info(2),
                        count: 1,
                    }],
                    rewards_select_item: Vec::new(),
                    finish_npc_index: 1_001,
                },
            },
            ServerPacket::NewRecipeInfo {
                info: ClientRecipeInfo {
                    gold: 50,
                    chance: 75,
                    item: test_user_item(3),
                    tools: vec![test_user_item(4)],
                    ingredients: vec![test_user_item(5)],
                },
            },
        ];

        let expected_ids = [155, 156, 162, 163, 178, 248, 250, 204, 266];
        for (packet, expected_id) in packets.into_iter().zip(expected_ids) {
            assert_eq!(packet.packet_id() as i16, expected_id);
            let encoded = encode_server_packet(&packet).expect("server packet should encode");
            let decoded = decode_server_packet(&encoded).expect("server packet should decode");
            assert_eq!(decoded, packet);
        }
    }

    #[test]
    fn remaining_crystal_server_packets_round_trip_with_typed_payloads() {
        let map_info = ClientMapInfo {
            title: "Bichon Province".to_string(),
            width: 800,
            height: 600,
            big_map: 1,
            movements: vec![crate::types::ClientMovementInfo {
                destination: 2,
                title: "Border Village".to_string(),
                location: Point { x: 330, y: 270 },
                icon: 4,
            }],
            npcs: vec![crate::types::ClientNpcInfo {
                index: 9,
                file_name: "merchant".to_string(),
                name: "Merchant".to_string(),
                map_index: 0,
                location: Point { x: 320, y: 260 },
                image: 12,
                rate: 10,
                show_on_big_map: true,
                big_map_icon: 3,
                object_id: 99,
                icon: 5,
                can_teleport_to: true,
            }],
        };
        let guild_member = crate::types::GuildMember {
            name: "Member".to_string(),
            id: 10,
            last_login_binary_datetime: 638_000_000_000_000_000,
            has_voted: true,
            online: false,
        };
        let guild_rank = GuildRank {
            name: "Leader".to_string(),
            options: 0x7f,
            index: 1,
            members: vec![guild_member],
        };
        let guild_storage_item = GuildStorageItem {
            item: test_user_item(30),
            user_id: 55,
        };
        let packets = vec![
            ServerPacket::NewMapInfo {
                map_index: 0,
                info: map_info,
            },
            ServerPacket::WorldMapSetup {
                setup: WorldMapSetup {
                    enabled: true,
                    icons: vec![crate::types::WorldMapIcon {
                        image_index: 1,
                        title: "Bichon".to_string(),
                        map_index: 0,
                    }],
                },
                teleport_to_npc_cost: 500,
            },
            ServerPacket::SearchMapResult {
                map_index: 0,
                npc_index: 9,
            },
            ServerPacket::UserSlotsRefresh {
                inventory: Some(vec![Some(test_user_item(31)), None]),
                equipment: Some(vec![Some(test_user_item(32))]),
            },
            ServerPacket::NewChatItem {
                item: test_user_item(33),
            },
            ServerPacket::PlayerUpdate {
                object_id: 1_001,
                light: 4,
                weapon: 1,
                weapon_effect: 2,
                armour: 3,
                wing_effect: 5,
            },
            ServerPacket::PlayerInspect {
                info: PlayerInspectInfo {
                    name: "Inspector".to_string(),
                    guild_name: "Guild".to_string(),
                    guild_rank: "Leader".to_string(),
                    equipment: vec![Some(test_user_item(34)), None],
                    class: MirClass::Wizard,
                    gender: MirGender::Female,
                    hair: 2,
                    level: 42,
                    lover_name: "Lover".to_string(),
                    allow_observe: true,
                    is_hero: false,
                },
            },
            ServerPacket::ReturnToLogin,
            ServerPacket::DamageIndicator {
                damage: 123,
                damage_type: 2,
                object_id: 1_001,
            },
            ServerPacket::HealthChanged { hp: 100, mp: 90 },
            ServerPacket::Death {
                location: Point { x: 10, y: 11 },
                direction: MirDirection::Down,
            },
            ServerPacket::ColourChanged {
                name_colour_argb: -1,
            },
            ServerPacket::ObjectColourChanged {
                object_id: 1_002,
                name_colour_argb: -2,
            },
            ServerPacket::ObjectGuildNameChanged {
                object_id: 1_003,
                guild_name: "Guild".to_string(),
            },
            ServerPacket::GainExperience { amount: 1_000 },
            ServerPacket::LevelChanged {
                level: 43,
                experience: 12_345,
                max_experience: 67_890,
            },
            ServerPacket::ObjectLeveled { object_id: 1_004 },
            ServerPacket::Poisoned { poison: 3 },
            ServerPacket::ObjectPoisoned {
                object_id: 1_005,
                poison: 4,
            },
            ServerPacket::MapChanged {
                map_index: 1,
                file_name: "0.map".to_string(),
                title: "Bichon".to_string(),
                mini_map: 100,
                big_map: 1,
                lights: 2,
                location: Point { x: 330, y: 270 },
                direction: MirDirection::UpRight,
                map_dark_light: 3,
                music: 4,
                weather: 5,
            },
            ServerPacket::ObjectName {
                object_id: 1_006,
                name: "Target".to_string(),
            },
            ServerPacket::Revived,
            ServerPacket::NPCConsign,
            ServerPacket::InTrapRock { trapped: true },
            ServerPacket::UserName {
                id: 1_007,
                name: "KnownUser".to_string(),
            },
            ServerPacket::ChatItemStats {
                chat_item_id: 77,
                stats: Some(test_user_item(35)),
            },
            ServerPacket::GuildNoticeChange {
                update: 2,
                notice: vec!["Line1".to_string(), "Line2".to_string()],
            },
            ServerPacket::GuildMemberChange {
                name: "Member".to_string(),
                rank_index: 1,
                status: 6,
                ranks: vec![guild_rank],
            },
            ServerPacket::GuildStatus {
                guild_name: "Guild".to_string(),
                guild_rank_name: "Leader".to_string(),
                level: 3,
                experience: 100,
                max_experience: 1_000,
                gold: 500,
                spare_points: 2,
                member_count: 10,
                max_members: 30,
                voting: true,
                item_count: 5,
                buff_count: 1,
                my_options: 0x7f,
                my_rank_id: 1,
            },
            ServerPacket::GuildInvite {
                name: "Invitee".to_string(),
            },
            ServerPacket::GuildExpGain { amount: 123 },
            ServerPacket::GuildNameRequest,
            ServerPacket::GuildStorageGoldChange {
                amount: 100,
                change_type: 1,
                name: "Member".to_string(),
            },
            ServerPacket::GuildStorageItemChange {
                change_type: 2,
                to: 1,
                from: 0,
                user: 10,
                item: Some(guild_storage_item.clone()),
            },
            ServerPacket::GuildStorageList {
                items: vec![Some(guild_storage_item), None],
            },
            ServerPacket::GuildRequestWar,
            ServerPacket::SetAutoPotValue { stat: 1, value: 75 },
            ServerPacket::SetAutoPotItem {
                grid: 1,
                item_index: 2,
            },
            ServerPacket::NPCImageUpdate {
                object_id: 1_008,
                image: 12,
                colour_argb: -3,
            },
            ServerPacket::EquipSlotItem {
                grid: MirGridType::Equipment,
                unique_id: 99,
                to: 1,
                grid_to: MirGridType::Inventory,
                success: true,
            },
            ServerPacket::GainedQuestItem {
                item: test_user_item(36),
            },
            ServerPacket::DeleteQuestItem {
                unique_id: 36,
                count: 1,
            },
            ServerPacket::CancelReincarnation,
            ServerPacket::RequestReincarnation,
            ServerPacket::UserBackStep {
                location: Point { x: 12, y: 13 },
                direction: MirDirection::Left,
            },
            ServerPacket::UserDashAttack {
                location: Point { x: 14, y: 15 },
                direction: MirDirection::Right,
            },
            ServerPacket::ObjectDashAttack {
                object_id: 1_009,
                location: Point { x: 16, y: 17 },
                direction: MirDirection::Up,
                distance: 3,
            },
            ServerPacket::UserAttackMove {
                location: Point { x: 18, y: 19 },
                direction: MirDirection::DownLeft,
            },
            ServerPacket::SetConcentration {
                object_id: 1_010,
                enabled: true,
                interrupted: false,
            },
            ServerPacket::SetElemental {
                object_id: 1_011,
                enabled: true,
                casted: true,
                value: 10,
                element_type: 2,
                exp_last: 30,
            },
            ServerPacket::AwakeningNeedMaterials {
                materials: Some(vec![
                    Some(AwakeningMaterial {
                        item: test_item_info(37),
                        count: 2,
                    }),
                    None,
                ]),
            },
            ServerPacket::NPCPearlGoods {
                list: vec![test_user_item(38)],
                rate: 1.25,
                panel_type: 4,
            },
            ServerPacket::TransformUpdate {
                object_id: 1_012,
                transform_type: 5,
            },
            ServerPacket::NPCRequestInput {
                npc_id: 1_013,
                page_name: "@confirm".to_string(),
            },
            ServerPacket::GameShopStock {
                g_index: 10,
                stock_level: 3,
            },
            ServerPacket::Rankings {
                rank_type: 1,
                my_rank: 2,
                listing_details: vec![RankCharacterInfo {
                    player_id: 3,
                    name: "Ranked".to_string(),
                    level: 50,
                    class: MirClass::Assassin,
                }],
                listings: vec![3],
                count: 1,
            },
            ServerPacket::UpdateNotice {
                notice: Notice {
                    title: "Notice".to_string(),
                    message: "Message".to_string(),
                },
            },
            ServerPacket::GuildTerritoryPage {
                length: 1,
                listings: vec![ClientGtMap {
                    index: 1,
                    name: "Castle".to_string(),
                    owner: "Guild".to_string(),
                    leader: "Leader".to_string(),
                    leader2: "Deputy".to_string(),
                    price: 10_000,
                    days: 7,
                    begin: 0,
                }],
            },
        ];

        let expected_ids = [
            18, 19, 20, 22, 36, 56, 57, 60, 75, 77, 80, 82, 83, 84, 85, 87, 89, 96, 97, 98, 129,
            136, 154, 161, 164, 165, 166, 167, 168, 169, 170, 171, 172, 173, 174, 175, 181, 182,
            188, 199, 205, 206, 207, 208, 209, 211, 212, 213, 216, 217, 228, 243, 244, 249, 251,
            252, 271, 276,
        ];
        assert_eq!(packets.len(), expected_ids.len());

        for (packet, expected_id) in packets.into_iter().zip(expected_ids) {
            assert_eq!(packet.packet_id() as i16, expected_id);
            let encoded = encode_server_packet(&packet).expect("server packet should encode");
            let decoded = decode_server_packet(&encoded).expect("server packet should decode");
            assert_eq!(decoded, packet);
        }
    }

    #[test]
    fn group_utility_server_packets_round_trip_with_crystal_ids() {
        let packets = vec![
            ServerPacket::SwitchGroup { allow_group: true },
            ServerPacket::DeleteGroup,
            ServerPacket::DeleteMember {
                name: "PartyMate".to_string(),
            },
            ServerPacket::GroupInvite {
                name: "Leader".to_string(),
            },
            ServerPacket::AddMember {
                name: "Scout".to_string(),
            },
            ServerPacket::OpenDoor {
                door_index: 4,
                close: false,
            },
            ServerPacket::OpenBrowser {
                url: "https://example.invalid".to_string(),
            },
            ServerPacket::PlaySound { sound: 12 },
            ServerPacket::SetTimer {
                key: "quest".to_string(),
                timer_type: 1,
                seconds: 60,
            },
            ServerPacket::ExpireTimer {
                key: "quest".to_string(),
            },
            ServerPacket::Roll {
                roll_type: 2,
                page: "@roll".to_string(),
                result: 17,
                auto_roll: true,
            },
            ServerPacket::SetCompass {
                location: Point { x: 330, y: 270 },
            },
            ServerPacket::GroupMembersMap {
                player_name: "Scout".to_string(),
                player_map: "Bichon Province".to_string(),
            },
            ServerPacket::SendMemberLocation {
                member_name: "Scout".to_string(),
                member_location: Point { x: 331, y: 271 },
            },
        ];

        let expected_ids = [
            131, 132, 133, 134, 135, 253, 267, 268, 269, 270, 272, 273, 274, 275,
        ];
        for (packet, expected_id) in packets.into_iter().zip(expected_ids) {
            assert_eq!(packet.packet_id() as i16, expected_id);
            let encoded = encode_server_packet(&packet).expect("server packet should encode");
            let decoded = decode_server_packet(&encoded).expect("server packet should decode");
            assert_eq!(decoded, packet);
        }
    }

    #[test]
    fn quest_server_packets_round_trip_with_crystal_ids() {
        let packets = vec![
            ServerPacket::ChangeQuest {
                quest_id: 1001,
                task_list: vec![
                    "Collect Wasp Stinger 0/1".to_string(),
                    "Return to Assistant".to_string(),
                ],
                taken: true,
                completed: false,
                new: true,
                quest_state: 0,
                track_quest: true,
            },
            ServerPacket::CompleteQuest {
                completed_quests: vec![1001, 1002],
            },
            ServerPacket::ShareQuest {
                quest_index: 1001,
                sharer_name: "Scout".to_string(),
            },
        ];

        let expected_ids = [201, 202, 203];
        for (packet, expected_id) in packets.into_iter().zip(expected_ids) {
            assert_eq!(packet.packet_id() as i16, expected_id);
            let encoded = encode_server_packet(&packet).expect("server packet should encode");
            let decoded = decode_server_packet(&encoded).expect("server packet should decode");
            assert_eq!(decoded, packet);
        }
    }

    #[test]
    fn refine_server_packets_round_trip_with_crystal_ids() {
        let packets = vec![
            ServerPacket::DepositRefineItem {
                from: 4,
                to: 0,
                success: true,
            },
            ServerPacket::RetrieveRefineItem {
                from: 0,
                to: 4,
                success: true,
            },
            ServerPacket::RefineCancel,
            ServerPacket::RefineItem { unique_id: 4 },
        ];

        let expected_ids = [46, 47, 48, 49];
        for (packet, expected_id) in packets.into_iter().zip(expected_ids) {
            assert_eq!(packet.packet_id() as i16, expected_id);
            let encoded = encode_server_packet(&packet).expect("server packet should encode");
            let decoded = decode_server_packet(&encoded).expect("server packet should decode");
            assert_eq!(decoded, packet);
        }
    }

    #[test]
    fn late_magic_awake_server_packets_round_trip_with_crystal_ids() {
        let packets = vec![
            ServerPacket::RemoveDelayedExplosion { object_id: 10 },
            ServerPacket::ObjectDeco {
                object_id: 11,
                location: Point { x: 26, y: 27 },
                image: 88,
            },
            ServerPacket::ObjectSneaking {
                object_id: 12,
                sneaking_active: true,
            },
            ServerPacket::ObjectLevelEffects {
                object_id: 13,
                level_effects: 4,
            },
            ServerPacket::SetBindingShot {
                object_id: 14,
                enabled: true,
                value: 1_000,
            },
            ServerPacket::SendOutputMessage {
                message: "output".to_string(),
                output_type: 1,
            },
            ServerPacket::NPCAwakening,
            ServerPacket::NPCDisassemble,
            ServerPacket::NPCDowngrade,
            ServerPacket::NPCReset,
            ServerPacket::AwakeningLockedItem {
                unique_id: 15,
                locked: true,
            },
            ServerPacket::Awakening {
                result: 1,
                remove_id: -1,
            },
            ServerPacket::ResizeInventory { size: 64 },
        ];

        let expected_ids = [
            218, 219, 220, 221, 222, 223, 224, 225, 226, 227, 229, 230, 237,
        ];
        for (packet, expected_id) in packets.into_iter().zip(expected_ids) {
            assert_eq!(packet.packet_id() as i16, expected_id);
            let encoded = encode_server_packet(&packet).expect("server packet should encode");
            let decoded = decode_server_packet(&encoded).expect("server packet should decode");
            assert_eq!(decoded, packet);
        }
    }

    #[test]
    fn hero_client_packets_round_trip_with_crystal_ids() {
        let packets = vec![
            ClientPacket::TakeBackHeroItem { from: 3, to: 1 },
            ClientPacket::TransferHeroItem { from: 1, to: 3 },
            ClientPacket::NewHero {
                name: "Aide".to_string(),
                gender: MirGender::Female,
                class: MirClass::Taoist,
            },
            ClientPacket::SetHeroBehaviour { behaviour: 2 },
            ClientPacket::ChangeHero { list_index: 1 },
        ];

        let expected_ids = [32, 33, 63, 66, 67];
        for (packet, expected_id) in packets.into_iter().zip(expected_ids) {
            assert_eq!(packet.packet_id() as i16, expected_id);
            let encoded = encode_client_packet(&packet).expect("client packet should encode");
            let decoded = decode_client_packet(&encoded).expect("client packet should decode");
            assert_eq!(decoded, packet);
        }
    }

    #[test]
    fn hero_server_packets_round_trip_with_crystal_ids() {
        let current_hero = test_hero_info(0);
        let stored_hero = test_hero_info(1);
        let packets = vec![
            ServerPacket::ObjectHero {
                info: test_object_player_info(1_001),
                owner_name: "Scout".to_string(),
            },
            ServerPacket::NewHeroInfo {
                info: current_hero.clone(),
                storage_index: -1,
            },
            ServerPacket::TakeBackHeroItem {
                from: 3,
                to: 1,
                success: false,
            },
            ServerPacket::TransferHeroItem {
                from: 1,
                to: 3,
                success: true,
            },
            ServerPacket::HeroHealthChanged { hp: 42, mp: 24 },
            ServerPacket::GainHeroExperience { amount: 1_500 },
            ServerPacket::HeroLevelChanged {
                level: 13,
                experience: 1_500,
                max_experience: 5_000,
            },
            ServerPacket::HeroCreateRequest {
                can_create_class: vec![true, true, true, false, false],
            },
            ServerPacket::NewHero { result: 0 },
            ServerPacket::UpdateHeroSpawnState { state: 1 },
            ServerPacket::UnlockHeroAutoPot,
            ServerPacket::SetHeroBehaviour { behaviour: 2 },
            ServerPacket::ManageHeroes {
                maximum_count: 3,
                current_hero: Some(current_hero),
                heroes: Some(vec![Some(stored_hero), None]),
            },
            ServerPacket::ChangeHero { from_index: 0 },
        ];

        let expected_ids = [
            25, 35, 54, 55, 78, 86, 88, 176, 177, 179, 180, 183, 184, 185,
        ];
        for (packet, expected_id) in packets.into_iter().zip(expected_ids) {
            assert_eq!(packet.packet_id() as i16, expected_id);
            let encoded = encode_server_packet(&packet).expect("server packet should encode");
            let decoded = decode_server_packet(&encoded).expect("server packet should decode");
            assert_eq!(decoded, packet);
        }
    }

    #[test]
    fn market_client_packets_round_trip_with_crystal_ids() {
        let packets = vec![
            ClientPacket::ConsignItem {
                unique_id: 77,
                price: 1_500,
                market_type: 0,
            },
            ClientPacket::MarketSearch {
                match_text: "blade".to_string(),
                item_type: 5,
                user_mode: true,
                min_shape: 1,
                max_shape: 99,
                market_type: 0,
            },
            ClientPacket::MarketRefresh,
            ClientPacket::MarketPage { page: 2 },
            ClientPacket::MarketBuy {
                auction_id: 88,
                bid_price: 2_000,
            },
            ClientPacket::MarketGetBack {
                mode: 1,
                auction_id: 88,
            },
            ClientPacket::MarketSellNow { auction_id: 88 },
        ];

        let expected_ids = [70, 71, 72, 73, 74, 75, 76];
        for (packet, expected_id) in packets.into_iter().zip(expected_ids) {
            assert_eq!(packet.packet_id() as i16, expected_id);
            let encoded = encode_client_packet(&packet).expect("client packet should encode");
            let decoded = decode_client_packet(&encoded).expect("client packet should decode");
            assert_eq!(decoded, packet);
        }
    }

    #[test]
    fn chat_client_packet_round_trips_linked_items_with_crystal_payload() {
        let packet = ClientPacket::Chat {
            message: "show <Bronze Ring>".to_string(),
            linked_items: vec![ChatItem {
                unique_id: 77,
                title: "Bronze Ring".to_string(),
                grid: MirGridType::Inventory,
            }],
        };

        assert_eq!(packet.packet_id() as i16, 13);
        let encoded = encode_client_packet(&packet).expect("chat packet should encode");
        let decoded = decode_client_packet(&encoded).expect("chat packet should decode");
        assert_eq!(decoded, packet);
    }

    #[test]
    fn crystal_chat_type_enum_accepts_late_channel_values() {
        assert_eq!(ChatType::try_from(10), Ok(ChatType::LevelUp));
        assert_eq!(ChatType::try_from(11), Ok(ChatType::System2));
        assert_eq!(ChatType::try_from(12), Ok(ChatType::Relationship));
        assert_eq!(ChatType::try_from(13), Ok(ChatType::Mentor));
        assert_eq!(ChatType::try_from(14), Ok(ChatType::Shout2));
        assert_eq!(ChatType::try_from(15), Ok(ChatType::Shout3));
        assert_eq!(ChatType::try_from(16), Ok(ChatType::LineMessage));
    }

    #[test]
    fn late_utility_client_packets_round_trip_with_crystal_ids() {
        let packets = vec![
            ClientPacket::DepositRefineItem { from: 1, to: 2 },
            ClientPacket::RetrieveRefineItem { from: 3, to: 4 },
            ClientPacket::RefineCancel,
            ClientPacket::RefineItem { unique_id: 40 },
            ClientPacket::CheckRefine { unique_id: 41 },
            ClientPacket::ReplaceWedRing { unique_id: 42 },
            ClientPacket::RequestMapInfo { map_index: 10 },
            ClientPacket::RequestMonsterInfo { monster_index: 11 },
            ClientPacket::RequestNpcInfo { npc_index: 12 },
            ClientPacket::TeleportToNpc { object_id: 99 },
            ClientPacket::SearchMap {
                text: "bichon".to_string(),
            },
            ClientPacket::Inspect {
                object_id: 123,
                ranking: true,
                hero: false,
            },
            ClientPacket::Observe {
                name: "Scout".to_string(),
            },
            ClientPacket::ChangeAMode { mode: 2 },
            ClientPacket::ChangePMode { mode: 1 },
            ClientPacket::ChangeTrade { allow_trade: true },
            ClientPacket::CallNpc {
                object_id: 777,
                key: "@main".to_string(),
            },
            ClientPacket::CraftItem {
                unique_id: 91,
                count: 2,
                slots: vec![1, 2, 3],
            },
            ClientPacket::BuyItemBack {
                unique_id: 92,
                count: 1,
            },
            ClientPacket::SwitchGroup { allow_group: true },
            ClientPacket::AddMember {
                name: "PartyMate".to_string(),
            },
            ClientPacket::DelMember {
                name: "PartyMate".to_string(),
            },
            ClientPacket::GroupInvite {
                accept_invite: false,
            },
            ClientPacket::SetAutoPotValue {
                stat: 1,
                value: 500,
            },
            ClientPacket::SetAutoPotItem {
                grid: MirGridType::Inventory,
                item_index: 3,
            },
            ClientPacket::TownRevive,
            ClientPacket::EquipSlotItem {
                grid: MirGridType::Inventory,
                unique_id: 100,
                to: 1,
                grid_to: MirGridType::Equipment,
                to_unique_id: 101,
            },
            ClientPacket::AcceptQuest {
                npc_index: 500,
                quest_index: 1,
            },
            ClientPacket::FinishQuest {
                quest_index: 1,
                selected_item_index: 2,
            },
            ClientPacket::AbandonQuest { quest_index: 3 },
            ClientPacket::ShareQuest { quest_index: 4 },
            ClientPacket::AcceptReincarnation,
            ClientPacket::CancelReincarnation,
            ClientPacket::CombineItem {
                grid: MirGridType::Inventory,
                id_from: 200,
                id_to: 201,
            },
            ClientPacket::AwakeningNeedMaterials {
                unique_id: 300,
                awake_type: 1,
            },
            ClientPacket::AwakeningLockedItem {
                unique_id: 301,
                locked: true,
            },
            ClientPacket::Awakening {
                unique_id: 302,
                awake_type: 2,
                position_index: 3,
            },
            ClientPacket::DisassembleItem { unique_id: 303 },
            ClientPacket::DowngradeAwakening { unique_id: 304 },
            ClientPacket::ResetAddedItem { unique_id: 305 },
        ];

        let expected_ids = [
            24, 25, 26, 27, 28, 29, 36, 37, 38, 40, 41, 42, 43, 44, 45, 46, 50, 53, 55, 59, 60, 61,
            62, 64, 65, 68, 101, 104, 105, 106, 107, 108, 109, 110, 111, 112, 113, 114, 115, 116,
        ];
        for (packet, expected_id) in packets.into_iter().zip(expected_ids) {
            assert_eq!(packet.packet_id() as i16, expected_id);
            let encoded = encode_client_packet(&packet).expect("client packet should encode");
            let decoded = decode_client_packet(&encoded).expect("client packet should decode");
            assert_eq!(decoded, packet);
        }
    }

    #[test]
    fn guild_service_client_packets_round_trip_with_crystal_ids() {
        let packets = vec![
            ClientPacket::RequestUserName { user_id: 9 },
            ClientPacket::RequestChatItem { chat_item_id: 10 },
            ClientPacket::EditGuildMember {
                change_type: 1,
                rank_index: 2,
                name: "Member".to_string(),
                rank_name: "Officer".to_string(),
            },
            ClientPacket::EditGuildNotice {
                notice: vec!["first".to_string(), "second".to_string()],
            },
            ClientPacket::GuildInvite {
                accept_invite: true,
            },
            ClientPacket::GuildNameReturn {
                name: "Guild".to_string(),
            },
            ClientPacket::RequestGuildInfo { info_type: 3 },
            ClientPacket::GuildStorageGoldChange {
                change_type: 1,
                amount: 1_000,
            },
            ClientPacket::GuildStorageItemChange {
                change_type: 2,
                from: 1,
                to: 2,
            },
            ClientPacket::GuildWarReturn {
                name: "Enemy".to_string(),
            },
            ClientPacket::GuildBuffUpdate { action: 1, id: 50 },
            ClientPacket::NpcConfirmInput {
                npc_id: 1,
                page_name: "@input".to_string(),
                value: "yes".to_string(),
            },
            ClientPacket::GameShopBuy {
                g_index: 7,
                quantity: 3,
                price_type: 1,
            },
            ClientPacket::ReportIssue {
                image: vec![1, 2, 3],
                image_size: 3,
                image_chunk: 1,
            },
            ClientPacket::GetRanking {
                rank_type: 1,
                rank_index: 10,
                online_only: true,
            },
            ClientPacket::OpenDoor { door_index: 4 },
            ClientPacket::GuildTerritoryPage { page: 2 },
            ClientPacket::PurchaseGuildTerritory {
                owner: "Guild".to_string(),
            },
        ];

        let expected_ids = [
            77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 131, 132, 133, 134, 135, 136, 147, 148,
        ];
        for (packet, expected_id) in packets.into_iter().zip(expected_ids) {
            assert_eq!(packet.packet_id() as i16, expected_id);
            let encoded = encode_client_packet(&packet).expect("client packet should encode");
            let decoded = decode_client_packet(&encoded).expect("client packet should decode");
            assert_eq!(decoded, packet);
        }
    }

    #[test]
    fn relationship_client_packets_round_trip_with_crystal_ids() {
        let packets = vec![
            ClientPacket::MarriageRequest,
            ClientPacket::MarriageReply {
                accept_invite: true,
            },
            ClientPacket::ChangeMarriage,
            ClientPacket::DivorceRequest,
            ClientPacket::DivorceReply {
                accept_invite: false,
            },
            ClientPacket::AddMentor {
                name: "Master".to_string(),
            },
            ClientPacket::MentorReply {
                accept_invite: true,
            },
            ClientPacket::AllowMentor,
            ClientPacket::CancelMentor,
        ];

        let expected_ids = [87, 88, 89, 90, 91, 92, 93, 94, 95];
        for (packet, expected_id) in packets.into_iter().zip(expected_ids) {
            assert_eq!(packet.packet_id() as i16, expected_id);
            let encoded = encode_client_packet(&packet).expect("client packet should encode");
            let decoded = decode_client_packet(&encoded).expect("client packet should decode");
            assert_eq!(decoded, packet);
        }
    }

    #[test]
    fn market_relationship_server_packets_round_trip_with_crystal_ids() {
        let packets = vec![
            ServerPacket::ConsignItem {
                unique_id: 77,
                success: false,
            },
            ServerPacket::MarketFail { reason: 1 },
            ServerPacket::MarketSuccess {
                message: "Listed".to_string(),
            },
            ServerPacket::MarriageRequest {
                name: "Partner".to_string(),
            },
            ServerPacket::DivorceRequest {
                name: "Partner".to_string(),
            },
            ServerPacket::MentorRequest {
                name: "Master".to_string(),
                level: 45,
            },
        ];

        let expected_ids = [157, 158, 159, 189, 190, 191];
        for (packet, expected_id) in packets.into_iter().zip(expected_ids) {
            assert_eq!(packet.packet_id() as i16, expected_id);
            let encoded = encode_server_packet(&packet).expect("server packet should encode");
            let decoded = decode_server_packet(&encoded).expect("server packet should decode");
            assert_eq!(decoded, packet);
        }
    }

    #[test]
    fn trade_client_packets_round_trip_with_crystal_ids() {
        let packets = vec![
            ClientPacket::DepositTradeItem { from: 4, to: 0 },
            ClientPacket::RetrieveTradeItem { from: 0, to: 4 },
            ClientPacket::TradeRequest,
            ClientPacket::TradeReply {
                accept_invite: true,
            },
            ClientPacket::TradeGold { amount: 1_000 },
            ClientPacket::TradeConfirm { locked: true },
            ClientPacket::TradeCancel,
        ];

        let expected_ids = [30, 31, 96, 97, 98, 99, 100];
        for (packet, expected_id) in packets.into_iter().zip(expected_ids) {
            assert_eq!(packet.packet_id() as i16, expected_id);
            let encoded = encode_client_packet(&packet).expect("client packet should encode");
            let decoded = decode_client_packet(&encoded).expect("client packet should decode");
            assert_eq!(decoded, packet);
        }
    }

    #[test]
    fn trade_server_packets_round_trip_with_crystal_ids() {
        let trade_item = UserItem {
            unique_id: 7,
            item_index: 1,
            current_dura: 1_000,
            max_dura: 1_000,
            count: 1,
            soul_bound_id: -1,
            identified: true,
            cursed: false,
            slots: Vec::new(),
            gem_count: 0,
            added_stats: Vec::new(),
            awake_type: 0,
            awake_values: Vec::new(),
            refined_value: 0,
            refine_added: 0,
            refine_success_chance: 0,
            wedding_ring: -1,
            expire_info: None,
            rental_information: None,
            is_shop_item: false,
            sealed_info: None,
            gm_made: false,
        };
        let packets = vec![
            ServerPacket::DepositTradeItem {
                from: 4,
                to: 0,
                success: true,
            },
            ServerPacket::RetrieveTradeItem {
                from: 0,
                to: 4,
                success: true,
            },
            ServerPacket::TradeRequest {
                name: "Scout".to_string(),
            },
            ServerPacket::TradeAccept {
                name: "Blade".to_string(),
            },
            ServerPacket::TradeGold { amount: 1_000 },
            ServerPacket::TradeItem {
                trade_items: vec![Some(trade_item), None],
            },
            ServerPacket::TradeConfirm,
            ServerPacket::TradeCancel { unlock: false },
        ];

        let expected_ids = [50, 51, 192, 193, 194, 195, 196, 197];
        for (packet, expected_id) in packets.into_iter().zip(expected_ids) {
            assert_eq!(packet.packet_id() as i16, expected_id);
            let encoded = encode_server_packet(&packet).expect("server packet should encode");
            let decoded = decode_server_packet(&encoded).expect("server packet should decode");
            assert_eq!(decoded, packet);
        }
    }

    #[test]
    fn fishing_client_packets_round_trip_with_crystal_ids() {
        let packets = vec![
            ClientPacket::FishingCast { cast_out: true },
            ClientPacket::FishingChangeAutocast { auto_cast: true },
        ];

        let expected_ids = [102, 103];
        for (packet, expected_id) in packets.into_iter().zip(expected_ids) {
            assert_eq!(packet.packet_id() as i16, expected_id);
            let encoded = encode_client_packet(&packet).expect("client packet should encode");
            let decoded = decode_client_packet(&encoded).expect("client packet should decode");
            assert_eq!(decoded, packet);
        }
    }

    #[test]
    fn mount_and_fishing_server_packets_round_trip_with_crystal_ids() {
        let packets = vec![
            ServerPacket::MountUpdate {
                object_id: 1_001,
                mount_type: 12,
                riding_mount: true,
            },
            ServerPacket::FishingUpdate {
                object_id: 1_001,
                fishing: true,
                progress_percent: 33,
                chance_percent: 12,
                fishing_point: Point { x: 331, y: 270 },
                found_fish: false,
            },
        ];

        let expected_ids = [198, 200];
        for (packet, expected_id) in packets.into_iter().zip(expected_ids) {
            assert_eq!(packet.packet_id() as i16, expected_id);
            let encoded = encode_server_packet(&packet).expect("server packet should encode");
            let decoded = decode_server_packet(&encoded).expect("server packet should decode");
            assert_eq!(decoded, packet);
        }
    }

    #[test]
    fn mail_friend_client_packets_round_trip_with_crystal_ids() {
        let packets = vec![
            ClientPacket::SendMail {
                name: "Blade".to_string(),
                message: "Delivery".to_string(),
                gold: 2_500,
                items_idx: [7, 8, 0, 0, 0],
                stamped: true,
            },
            ClientPacket::ReadMail { mail_id: 11 },
            ClientPacket::CollectParcel { mail_id: 11 },
            ClientPacket::DeleteMail { mail_id: 11 },
            ClientPacket::LockMail {
                mail_id: 11,
                lock: true,
            },
            ClientPacket::MailLockedItem {
                unique_id: 7,
                locked: true,
            },
            ClientPacket::MailCost {
                gold: 2_500,
                items_idx: [7, 8, 0, 0, 0],
                stamped: true,
            },
            ClientPacket::AddFriend {
                name: "Blade".to_string(),
                blocked: false,
            },
            ClientPacket::RemoveFriend {
                character_index: 42,
            },
            ClientPacket::RefreshFriends,
            ClientPacket::AddMemo {
                character_index: 42,
                memo: "party lead".to_string(),
            },
        ];

        let expected_ids = [117, 118, 119, 120, 121, 122, 123, 127, 128, 129, 130];
        for (packet, expected_id) in packets.into_iter().zip(expected_ids) {
            assert_eq!(packet.packet_id() as i16, expected_id);
            let encoded = encode_client_packet(&packet).expect("client packet should encode");
            let decoded = decode_client_packet(&encoded).expect("client packet should decode");
            assert_eq!(decoded, packet);
        }
    }

    #[test]
    fn mail_friend_server_packets_round_trip_with_crystal_ids() {
        let packets = vec![
            ServerPacket::ReceiveMail {
                mail: vec![ClientMail {
                    mail_id: 11,
                    sender_name: "GM".to_string(),
                    message: "Welcome".to_string(),
                    opened: false,
                    locked: true,
                    can_reply: false,
                    collected: false,
                    date_sent_binary_datetime: 638000000000000000,
                    gold: 2_500,
                    items: vec![test_user_item(7)],
                }],
            },
            ServerPacket::MailLockedItem {
                unique_id: 7,
                locked: true,
            },
            ServerPacket::MailSendRequest,
            ServerPacket::MailSent { result: -1 },
            ServerPacket::ParcelCollected { result: 1 },
            ServerPacket::MailCost { cost: 200 },
            ServerPacket::FriendUpdate {
                friends: vec![ClientFriend {
                    index: 42,
                    name: "Blade".to_string(),
                    memo: "party lead".to_string(),
                    blocked: false,
                    online: true,
                }],
            },
        ];

        let expected_ids = [231, 232, 233, 234, 235, 236, 245];
        for (packet, expected_id) in packets.into_iter().zip(expected_ids) {
            assert_eq!(packet.packet_id() as i16, expected_id);
            let encoded = encode_server_packet(&packet).expect("server packet should encode");
            let decoded = decode_server_packet(&encoded).expect("server packet should decode");
            assert_eq!(decoded, packet);
        }
    }

    #[test]
    fn intelligent_creature_client_packets_round_trip_with_crystal_ids() {
        let packets = vec![
            ClientPacket::UpdateIntelligentCreature {
                creature: test_intelligent_creature(0),
                summon_me: true,
                unsummon_me: false,
                release_me: false,
            },
            ClientPacket::IntelligentCreaturePickup {
                mouse_mode: true,
                location: Point { x: 330, y: 270 },
            },
            ClientPacket::RequestIntelligentCreatureUpdates { update: true },
        ];

        let expected_ids = [124, 125, 126];
        for (packet, expected_id) in packets.into_iter().zip(expected_ids) {
            assert_eq!(packet.packet_id() as i16, expected_id);
            let encoded = encode_client_packet(&packet).expect("client packet should encode");
            let decoded = decode_client_packet(&encoded).expect("client packet should decode");
            assert_eq!(decoded, packet);
        }
    }

    #[test]
    fn intelligent_creature_server_packets_round_trip_with_crystal_ids() {
        let packets = vec![
            ServerPacket::NewIntelligentCreature {
                creature: test_intelligent_creature(0),
            },
            ServerPacket::UpdateIntelligentCreatureList {
                creature_list: vec![test_intelligent_creature(0)],
                creature_summoned: true,
                summoned_creature_type: 1,
                pearl_count: 3,
            },
            ServerPacket::IntelligentCreatureEnableRename,
            ServerPacket::IntelligentCreaturePickup { object_id: 1_001 },
        ];

        let expected_ids = [239, 240, 241, 242];
        for (packet, expected_id) in packets.into_iter().zip(expected_ids) {
            assert_eq!(packet.packet_id() as i16, expected_id);
            let encoded = encode_server_packet(&packet).expect("server packet should encode");
            let decoded = decode_server_packet(&encoded).expect("server packet should decode");
            assert_eq!(decoded, packet);
        }
    }

    #[test]
    fn item_rental_client_packets_round_trip_with_crystal_ids() {
        let packets = vec![
            ClientPacket::GetRentedItems,
            ClientPacket::ItemRentalRequest,
            ClientPacket::ItemRentalFee { amount: 750 },
            ClientPacket::ItemRentalPeriod { days: 7 },
            ClientPacket::DepositRentalItem { from: 1, to: 0 },
            ClientPacket::RetrieveRentalItem { from: 0, to: 1 },
            ClientPacket::CancelItemRental,
            ClientPacket::ItemRentalLockFee,
            ClientPacket::ItemRentalLockItem,
            ClientPacket::ConfirmItemRental,
        ];

        let expected_ids = [137, 138, 139, 140, 141, 142, 143, 144, 145, 146];
        for (packet, expected_id) in packets.into_iter().zip(expected_ids) {
            assert_eq!(packet.packet_id() as i16, expected_id);
            let encoded = encode_client_packet(&packet).expect("client packet should encode");
            let decoded = decode_client_packet(&encoded).expect("client packet should decode");
            assert_eq!(decoded, packet);
        }
    }

    #[test]
    fn item_rental_server_packets_round_trip_with_crystal_ids() {
        let loan_item = UserItem {
            unique_id: 42,
            item_index: 1,
            current_dura: 1_000,
            max_dura: 1_000,
            count: 1,
            soul_bound_id: -1,
            identified: true,
            cursed: false,
            slots: Vec::new(),
            gem_count: 0,
            added_stats: Vec::new(),
            awake_type: 0,
            awake_values: Vec::new(),
            refined_value: 0,
            refine_added: 0,
            refine_success_chance: 0,
            wedding_ring: -1,
            expire_info: None,
            rental_information: None,
            is_shop_item: false,
            sealed_info: None,
            gm_made: false,
        };
        let packets = vec![
            ServerPacket::GetRentedItems {
                rented_items: vec![ItemRentalInformation {
                    item_id: 42,
                    item_name: "Iron Sword".to_string(),
                    renting_player_name: "Blade".to_string(),
                    item_return_date_binary_datetime: 638000000000000000,
                }],
            },
            ServerPacket::ItemRentalRequest {
                name: "Blade".to_string(),
                renting: false,
            },
            ServerPacket::ItemRentalFee { amount: 750 },
            ServerPacket::ItemRentalPeriod { days: 7 },
            ServerPacket::DepositRentalItem {
                from: 1,
                to: 0,
                success: true,
            },
            ServerPacket::RetrieveRentalItem {
                from: 0,
                to: 1,
                success: true,
            },
            ServerPacket::UpdateRentalItem {
                loan_item: Some(loan_item),
            },
            ServerPacket::CancelItemRental,
            ServerPacket::ItemRentalLock {
                success: true,
                gold_locked: true,
                item_locked: false,
            },
            ServerPacket::ItemRentalPartnerLock {
                gold_locked: true,
                item_locked: true,
            },
            ServerPacket::CanConfirmItemRental,
            ServerPacket::ConfirmItemRental,
        ];

        let expected_ids = [254, 255, 256, 257, 258, 259, 260, 261, 262, 263, 264, 265];
        for (packet, expected_id) in packets.into_iter().zip(expected_ids) {
            assert_eq!(packet.packet_id() as i16, expected_id);
            let encoded = encode_server_packet(&packet).expect("server packet should encode");
            let decoded = decode_server_packet(&encoded).expect("server packet should decode");
            assert_eq!(decoded, packet);
        }
    }

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
