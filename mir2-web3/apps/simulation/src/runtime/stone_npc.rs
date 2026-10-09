//! The first stone-workshop UI uses normal live NPC dialog links and snapshots.
//! Trusted execution and persistence belong to `stones`, not to dialog targets.
//! This module never changes movement, attack queues or the client's input rules.
use std::collections::BTreeSet;

use bevy_ecs::prelude::World;
use mir2_game_data::LanguageCode;
use mir2_production::{PublicStone, StoneError, StoneGrade, StoneLifecycle, StoneMaterial, StoneOwner};
use mir2_protocol::ServerPacket;
use sha2::{Digest, Sha256};

use super::components::{entity_by_object_id, entity_position, Npc, NpcAgent};
use super::npc::{set_dialog, ActiveNpcDialogState, NpcDialogLinkState};
use super::packets::system_message;
use super::resources::{current_language, MapRuntimeResource, NpcStateResource};
use super::session::SimulationSession;
use super::stones::{workshop_allowed, workshop_enabled};

const PREFIX: &str = "@stone:v1:";
const HOME: &str = "@stone:v1:home";
const PAGE_SIZE: usize = 3;
const MAX_ITEMS: usize = 80;
const MAX_PAGE: u64 = ((MAX_ITEMS - 1) / PAGE_SIZE) as u64;
const MAX_TARGET_BYTES: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoneOreView {
    pub uid: String,
    pub template_id: i32,
    pub grade: StoneGrade,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoneWorkshopView {
    pub owner: StoneOwner,
    pub inventory_revision: u64,
    pub ores: Vec<StoneOreView>,
    pub stones: Vec<PublicStone>,
    pub gold: u32,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoneWorkshopOperation {
    Seal { uid: u64 },
    Appraise { serial: u64, expected_stone_revision: u64 },
    Cut { serial: u64, expected_stone_revision: u64 },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoneWorkshopCommand {
    pub request_id: String,
    pub expected_inventory_revision: u64,
    pub operation: StoneWorkshopOperation,
}
#[derive(Debug, Clone)]
pub struct StoneWorkshopExecution {
    pub packets: Vec<ServerPacket>,
    pub replayed: bool,
    /// A localized committed Cut receipt description. Ignored for Seal/Appraise
    /// so those operations cannot expose a hidden result via an incidental string.
    pub public_result: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoneWorkshopError {
    Unavailable,
    InvalidRequest,
    StaleInventory,
    Rejected(StoneError),
    AuthorityRejected,
    OutcomeUnknown,
}
impl From<StoneError> for StoneWorkshopError {
    fn from(error: StoneError) -> Self { Self::Rejected(error) }
}
impl std::fmt::Display for StoneWorkshopError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for StoneWorkshopError {}

fn text(language: LanguageCode, en: &'static str, zh: &'static str,
    pt: &'static str, es: &'static str) -> &'static str
{
    match language {
        LanguageCode::English => en,
        LanguageCode::ChineseSimplified => zh,
        LanguageCode::Portuguese => pt,
        LanguageCode::Spanish => es,
    }
}
fn title(language: LanguageCode) -> &'static str {
    text(language, "Stone workshop", "原石工坊", "Oficina de pedras", "Taller de piedras")
}
fn grade_name(language: LanguageCode, grade: StoneGrade) -> &'static str {
    match grade {
        StoneGrade::Rough => text(language, "Rough raw stone", "粗糙原石", "Pedra bruta comum", "Piedra bruta común"),
        StoneGrade::Fine => text(language, "Fine raw stone", "精致原石", "Pedra bruta refinada", "Piedra bruta fina"),
        StoneGrade::Precious => text(language, "Precious raw stone", "珍贵原石", "Pedra bruta preciosa", "Piedra bruta preciosa"),
    }
}
fn material_name(language: LanguageCode, material: StoneMaterial) -> &'static str {
    match material {
        StoneMaterial::Copper => text(language, "Copper ore", "铜矿", "Minério de cobre", "Mineral de cobre"),
        StoneMaterial::Silver => text(language, "Silver ore", "银矿", "Minério de prata", "Mineral de plata"),
        StoneMaterial::JadeCrystal => text(language, "Jade crystal", "玉晶", "Cristal de jade", "Cristal de jade"),
        StoneMaterial::BraveryGem => text(language, "Bravery gem", "勇气宝石", "Gema da coragem", "Gema de valentía"),
        StoneMaterial::MagicGem => text(language, "Magic gem", "魔法宝石", "Gema mágica", "Gema mágica"),
        StoneMaterial::SoulGem => text(language, "Soul gem", "灵魂宝石", "Gema da alma", "Gema del alma"),
    }
}
fn ore_name(language: LanguageCode, template: i32) -> &'static str {
    match template {
        824 => material_name(language, StoneMaterial::Copper),
        826 => material_name(language, StoneMaterial::Silver),
        827 => text(language, "Gold ore", "金矿", "Minério de ouro", "Mineral de oro"),
        _ => text(language, "Ore", "矿石", "Minério", "Mineral"),
    }
}
fn gold_label(language: LanguageCode) -> &'static str {
    text(language, "Gold", "金币", "Ouro", "Oro")
}
fn link(label: impl Into<String>, target: impl Into<String>) -> NpcDialogLinkState {
    NpcDialogLinkState { text: label.into(), target: target.into() }
}
fn is_workshop_target(target: &str) -> bool {
    target.get(..7).is_some_and(|head| head.eq_ignore_ascii_case("@stone:"))
}
fn canonical_decimal(value: &str, nonzero: bool) -> Option<u64> {
    if value.is_empty() || value.len() > 20 || !value.bytes().all(|byte| byte.is_ascii_digit())
        || value.len() > 1 && value.starts_with('0')
    { return None; }
    let parsed = value.parse::<u64>().ok()?;
    (!nonzero || parsed != 0).then_some(parsed)
}
fn valid_request_id(value: &str) -> bool {
    value.len() == 70 && value.starts_with("stone-")
        && value[6..].bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Items { Ore, Stone }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page { Home, List(Items, u64), Ore(u64), Stone(u64) }
#[derive(Debug, Clone, PartialEq, Eq)]
enum Target { Page(Page), Action(StoneWorkshopCommand) }
fn parse_target(target: &str) -> Option<Target> {
    if target.len() > MAX_TARGET_BYTES || !target.starts_with(PREFIX) { return None; }
    let parts: Vec<_> = target.split(':').collect();
    let page = match parts.as_slice() {
        ["@stone", "v1", "home"] => Some(Page::Home),
        ["@stone", "v1", "list", kind, index] => {
            let kind = match *kind { "ore" => Items::Ore, "stone" => Items::Stone, _ => return None };
            let index = canonical_decimal(index, false)?;
            if index > MAX_PAGE { return None; }
            Some(Page::List(kind, index))
        },
        ["@stone", "v1", "ore", uid, inventory_revision] => {
            canonical_decimal(inventory_revision, false)?;
            Some(Page::Ore(canonical_decimal(uid, true)?))
        },
        ["@stone", "v1", "stone", serial, stone_revision, inventory_revision] => {
            canonical_decimal(stone_revision, false)?;
            canonical_decimal(inventory_revision, false)?;
            Some(Page::Stone(canonical_decimal(serial, true)?))
        },
        _ => None,
    };
    if let Some(page) = page { return Some(Target::Page(page)); }
    let (operation, inventory_revision, request_id) = match parts.as_slice() {
        ["@stone", "v1", "seal", uid, inventory_revision, request_id] =>
            (StoneWorkshopOperation::Seal { uid: canonical_decimal(uid, true)? }, *inventory_revision, *request_id),
        ["@stone", "v1", operation, serial, stone_revision, inventory_revision, request_id] => {
            let serial = canonical_decimal(serial, true)?;
            let expected_stone_revision = canonical_decimal(stone_revision, false)?;
            let operation = match *operation {
                "appraise" => StoneWorkshopOperation::Appraise { serial, expected_stone_revision },
                "cut" => StoneWorkshopOperation::Cut { serial, expected_stone_revision },
                _ => return None,
            };
            (operation, *inventory_revision, *request_id)
        },
        _ => return None,
    };
    if !valid_request_id(request_id) { return None; }
    Some(Target::Action(StoneWorkshopCommand { operation, request_id: request_id.into(),
        expected_inventory_revision: canonical_decimal(inventory_revision, false)? }))
}
fn operation_fields(operation: &StoneWorkshopOperation) -> (&'static str, u64, u64) {
    match *operation {
        StoneWorkshopOperation::Seal { uid } => ("seal", uid, 0),
        StoneWorkshopOperation::Appraise { serial, expected_stone_revision } => ("appraise", serial, expected_stone_revision),
        StoneWorkshopOperation::Cut { serial, expected_stone_revision } => ("cut", serial, expected_stone_revision),
    }
}
/// Deterministic request identity, not a secret or a possession authorization.
/// The exact displayed-link check, current holder and atomic authority are required.
fn quote_request_id(owner: &StoneOwner, inventory_revision: u64, operation: &StoneWorkshopOperation) -> String {
    let (kind, selector, stone_revision) = operation_fields(operation);
    let mut hash = Sha256::new();
    hash.update(b"stone-workshop-npc-v1");
    for value in [&owner.account_id, &owner.character_incarnation] {
        hash.update((value.len() as u64).to_le_bytes());
        hash.update(value.as_bytes());
    }
    hash.update(kind.as_bytes());
    hash.update(selector.to_le_bytes());
    hash.update(stone_revision.to_le_bytes());
    hash.update(inventory_revision.to_le_bytes());
    let digest: String = hash.finalize().iter().map(|byte| format!("{byte:02x}")).collect();
    format!("stone-{digest}")
}
fn action_target(view: &StoneWorkshopView, operation: StoneWorkshopOperation) -> String {
    let request = quote_request_id(&view.owner, view.inventory_revision, &operation);
    let (kind, selector, stone_revision) = operation_fields(&operation);
    if kind == "seal" {
        format!("{PREFIX}{kind}:{selector}:{}:{request}", view.inventory_revision)
    } else {
        format!("{PREFIX}{kind}:{selector}:{stone_revision}:{}:{request}", view.inventory_revision)
    }
}

fn allowed_location(script_key: &str, map: &str, x: i32, y: i32) -> bool {
    matches!((script_key, map, x, y),
        ("BichonProvince/BichonWall/Blacksmith", "0", 302, 221)
        | ("MongchonProvince/MudWall/Blacksmith", "0159", 5, 9))
}
pub(super) fn approved_npc(world: &World, object_id: u32) -> bool {
    let Some(map) = world.get_resource::<MapRuntimeResource>() else { return false; };
    let Some(entity) = entity_by_object_id(world, object_id) else { return false; };
    let entry = world.entity(entity);
    if entry.get::<Npc>().is_none() { return false; }
    let Some(script_key) = entry.get::<NpcAgent>().and_then(|agent| agent.script_key.as_deref()) else { return false; };
    let Some(position) = entity_position(world, entity) else { return false; };
    allowed_location(script_key, &map.current_map.file_name, position.x, position.y)
}
pub(super) fn decorate_dialog(world: &World, dialog: &mut ActiveNpcDialogState) {
    if !workshop_enabled(world) || !approved_npc(world, dialog.npc_object_id)
        || dialog.input.is_some() || dialog.links.iter().any(|link| is_workshop_target(&link.target))
        || dialog.title == title(current_language(world))
    { return; }
    let language = current_language(world);
    dialog.body.push(text(language, "Seal mined ore, appraise raw stones, or cut them into materials.",
        "可将矿石封成原石，鉴定线索或切割获得材料。", "Sele minérios, avalie pedras brutas ou corte-as para obter materiais.",
        "Sella minerales, evalúa piedras brutas o córtalas para obtener materiales.").into());
    dialog.links.push(link(title(language), HOME));
}

fn valid_view(view: &StoneWorkshopView) -> bool {
    if view.owner.account_id.is_empty() || view.owner.account_id.len() > 256
        || view.owner.character_incarnation.is_empty() || view.owner.character_incarnation.len() > 128
        || view.ores.len() > MAX_ITEMS || view.stones.len() > MAX_ITEMS
        || view.ores.len() + view.stones.len() > MAX_ITEMS
    { return false; }
    let mut uids = BTreeSet::new();
    let mut serials = BTreeSet::new();
    for ore in &view.ores {
        let Some(uid) = canonical_decimal(&ore.uid, true) else { return false; };
        if !uids.insert(uid) || ore.template_id != match ore.grade {
            StoneGrade::Rough => 824, StoneGrade::Fine => 826, StoneGrade::Precious => 827,
        } { return false; }
    }
    for stone in &view.stones {
        let Some(uid) = canonical_decimal(&stone.uid, true) else { return false; };
        let Some(serial) = canonical_decimal(&stone.serial, true) else { return false; };
        if canonical_decimal(&stone.revision, false).is_none() || !uids.insert(uid) || !serials.insert(serial)
            || stone.lifecycle == StoneLifecycle::Cut || stone.possibilities.len() != 6
            || stone.catalog_version == 0 || stone.appraisal_gold == 0 || stone.cutting_gold == 0
            || stone.appraisal_gold > 10_000_000 || stone.cutting_gold > 10_000_000
            || stone.lifecycle == StoneLifecycle::Sealed && (stone.revision != "0" || stone.appraisal_clue.is_some())
            || stone.lifecycle == StoneLifecycle::Appraised && (stone.revision != "1" || stone.appraisal_clue.is_none())
        { return false; }
        let mut sum = 0_u32;
        for (material, row) in StoneMaterial::ALL.into_iter().zip(&stone.possibilities) {
            if row.material != material || row.template_id != material.template_id()
                || row.quantity == 0 || row.quantity > 16 || row.basis_points == 0
            { return false; }
            sum += u32::from(row.basis_points);
        }
        if sum != 10_000 { return false; }
    }
    true
}
fn base_dialog(active: &ActiveNpcDialogState, language: LanguageCode) -> ActiveNpcDialogState {
    let mut dialog = active.clone();
    dialog.title = title(language).into();
    dialog.stage = None;
    dialog.current = 0;
    dialog.required = 0;
    dialog.input = None;
    dialog.body.clear();
    dialog.footer = text(language, "Materials and raw stones stay in your physical Bag.",
        "原石和产出的材料均保存在实际背包中。", "As pedras e os materiais ficam na sua Bolsa.",
        "Las piedras y los materiales se guardan en tu Bolsa.").into();
    dialog.links.retain(|link| !is_workshop_target(&link.target));
    if !dialog.links.iter().any(|link| link.target.eq_ignore_ascii_case("@Main")) {
        dialog.links.push(link(text(language, "Merchant menu", "商人菜单", "Menu do comerciante", "Menú del comerciante"), "@Main"));
    }
    if !dialog.links.iter().any(|link| link.target.eq_ignore_ascii_case("@Exit")) {
        dialog.links.push(link(text(language, "Exit", "离开", "Sair", "Salir"), "@Exit"));
    }
    dialog
}
fn add_home(dialog: &mut ActiveNpcDialogState, language: LanguageCode) {
    dialog.links.push(link(text(language, "Workshop menu", "工坊菜单", "Menu da oficina", "Menú del taller"), HOME));
}
fn possibilities_body(language: LanguageCode, stone: &PublicStone) -> Vec<String> {
    let mut body = vec![format!("{} · #{}", grade_name(language, stone.grade), stone.uid),
        text(language, "The result was sealed when this stone was created. Cutting does not reroll it.",
            "结果在原石生成时已封存，切割不会重新抽取。", "O resultado foi fixado ao criar a pedra. O corte não sorteia novamente.",
            "El resultado se fijó al crear la piedra. El corte no vuelve a sortearlo.").into(),
        text(language, "Possible materials (generation probabilities):", "可能产物（生成概率）：",
            "Materiais possíveis (probabilidades na criação):", "Materiales posibles (probabilidades al crear):").into()];
    for row in &stone.possibilities {
        body.push(format!("{} ×{} — {}.{:02}%", material_name(language, row.material), row.quantity,
            row.basis_points / 100, row.basis_points % 100));
    }
    body.push(format!("{}: {} · {}: {} {}",
        text(language, "Appraise", "鉴定", "Avaliar", "Evaluar"), stone.appraisal_gold,
        text(language, "Cut", "切割", "Cortar", "Cortar"), stone.cutting_gold, gold_label(language)));
    let clue = match stone.appraisal_clue {
        None => text(language, "Not appraised", "尚未鉴定", "Ainda não avaliada", "Sin evaluar"),
        Some(mir2_production::StoneClue::Metallic) => text(language, "Metallic: copper or silver", "金属纹理：铜矿或银矿",
            "Metálica: cobre ou prata", "Metálica: cobre o plata"),
        Some(mir2_production::StoneClue::Crystalline) => text(language, "Crystalline: jade crystal or a gem", "晶体纹理：玉晶或宝石",
            "Cristalina: cristal de jade ou uma gema", "Cristalina: cristal de jade o una gema"),
    };
    body.push(format!("{}: {clue}", text(language, "Appraisal clue", "鉴定线索", "Indício da avaliação", "Pista de evaluación")));
    body
}
fn render_page(active: &ActiveNpcDialogState, language: LanguageCode, view: &StoneWorkshopView,
    page: Page, notice: Option<&str>) -> ActiveNpcDialogState
{
    let mut dialog = base_dialog(active, language);
    dialog.body.push(format!("{}: {}", gold_label(language), view.gold));
    match page {
        Page::Home => {
            dialog.body.push(text(language, "One Bag ore becomes one raw stone of the matching grade. No gold is charged to seal it.",
                "一块背包矿石可封成对应档位的原石，封存不收取金币。", "Um minério da Bolsa vira uma pedra bruta da classe correspondente. Selar não custa ouro.",
                "Un mineral de la Bolsa se convierte en una piedra de su categoría. Sellar no cuesta oro.").into());
            dialog.body.push(text(language, "Copper → Rough · Silver → Fine · Gold → Precious", "铜矿 → 粗糙 · 银矿 → 精致 · 金矿 → 珍贵",
                "Cobre → Comum · Prata → Refinada · Ouro → Preciosa", "Cobre → Común · Plata → Fina · Oro → Preciosa").into());
            dialog.body.push(text(language, "Appraisal gives a broad clue. Cutting consumes the raw stone and yields its fixed material in your Bag.",
                "鉴定仅提供粗略线索；切割消耗原石，将固定产物放入背包。", "A avaliação dá um indício geral. O corte consome a pedra e coloca o material fixado na Bolsa.",
                "La evaluación da una pista general. El corte consume la piedra y coloca su material fijo en la Bolsa.").into());
            dialog.links.push(link(format!("{} ({})", text(language, "Seal Bag ore", "封存背包矿石", "Selar minério da Bolsa", "Sellar mineral de la Bolsa"), view.ores.len()), format!("{PREFIX}list:ore:0")));
            dialog.links.push(link(format!("{} ({})", text(language, "Manage Bag raw stones", "管理背包原石", "Gerir pedras brutas da Bolsa", "Gestionar piedras de la Bolsa"), view.stones.len()), format!("{PREFIX}list:stone:0")));
        },
        Page::List(kind, requested) => {
            let count = match kind { Items::Ore => view.ores.len(), Items::Stone => view.stones.len() };
            let last_page = count.saturating_sub(1) / PAGE_SIZE;
            let index = (requested as usize).min(last_page);
            let start = index * PAGE_SIZE;
            dialog.body.push(format!("{} {}/{}", text(language, "Page", "页", "Página", "Página"), index + 1, last_page + 1));
            if count == 0 {
                dialog.body.push(text(language, "No eligible items in your Bag. Ore must be one unlocked physical item.",
                    "背包中没有可用物品，矿石必须为未锁定的单件实体物品。", "Não há itens elegíveis na Bolsa. O minério deve ser um item físico desbloqueado.",
                    "No hay objetos válidos en la Bolsa. El mineral debe ser un objeto físico sin bloquear.").into());
            }
            match kind {
                Items::Ore => for ore in view.ores.iter().skip(start).take(PAGE_SIZE) {
                    dialog.links.push(link(format!("{} → {} · #{}", ore_name(language, ore.template_id), grade_name(language, ore.grade), ore.uid),
                        format!("{PREFIX}ore:{}:{}", ore.uid, view.inventory_revision)));
                },
                Items::Stone => for stone in view.stones.iter().skip(start).take(PAGE_SIZE) {
                    dialog.links.push(link(format!("{} · #{}", grade_name(language, stone.grade), stone.uid),
                        format!("{PREFIX}stone:{}:{}:{}", stone.serial, stone.revision, view.inventory_revision)));
                },
            }
            let kind_name = match kind { Items::Ore => "ore", Items::Stone => "stone" };
            if index > 0 { dialog.links.push(link(text(language, "Previous", "上一页", "Anterior", "Anterior"), format!("{PREFIX}list:{kind_name}:{}", index - 1))); }
            if index < last_page { dialog.links.push(link(text(language, "Next", "下一页", "Seguinte", "Siguiente"), format!("{PREFIX}list:{kind_name}:{}", index + 1))); }
            add_home(&mut dialog, language);
        },
        Page::Ore(uid) => {
            if let Some(ore) = view.ores.iter().find(|ore| canonical_decimal(&ore.uid, true) == Some(uid)) {
                dialog.body.push(format!("{} ×1 · #{} → {}", ore_name(language, ore.template_id), ore.uid, grade_name(language, ore.grade)));
                dialog.body.push(text(language, "Sealing consumes this ore and keeps its physical Bag slot. Its hidden result is fixed now.",
                    "封存将消耗此矿石并沿用背包位置，此时固定隐藏结果。", "Selar consome este minério e mantém o espaço na Bolsa. O resultado oculto é fixado agora.",
                    "Sellar consume este mineral y conserva su espacio en la Bolsa. El resultado oculto se fija ahora.").into());
                dialog.body.push(text(language, "Sealing fee: 0 Gold. Review the raw stone before paying to appraise or cut it.",
                    "封存费用：0 金币。封存后可查看原石，再决定是否付费鉴定或切割。", "Custo de selar: 0 Ouro. Consulte a pedra antes de pagar pela avaliação ou pelo corte.",
                    "Coste de sellar: 0 Oro. Consulta la piedra antes de pagar para evaluarla o cortarla.").into());
                dialog.links.push(link(text(language, "Seal this ore", "封存此矿石", "Selar este minério", "Sellar este mineral"), action_target(view, StoneWorkshopOperation::Seal { uid })));
            } else { dialog.body.push(error_notice(language, &StoneWorkshopError::Rejected(StoneError::InvalidPossession)).into()); }
            dialog.links.push(link(text(language, "Back to ore", "返回矿石列表", "Voltar aos minérios", "Volver a los minerales"), format!("{PREFIX}list:ore:0")));
            add_home(&mut dialog, language);
        },
        Page::Stone(serial) => {
            if let Some(stone) = view.stones.iter().find(|stone| canonical_decimal(&stone.serial, true) == Some(serial)) {
                dialog.body.extend(possibilities_body(language, stone));
                let Some(revision) = canonical_decimal(&stone.revision, false) else {
                    return notice_dialog(active, language, error_notice(language, &StoneWorkshopError::Unavailable));
                };
                if stone.lifecycle == StoneLifecycle::Sealed {
                    dialog.links.push(link(format!("{} · {} {}", text(language, "Appraise", "鉴定", "Avaliar", "Evaluar"), stone.appraisal_gold, gold_label(language)),
                        action_target(view, StoneWorkshopOperation::Appraise { serial, expected_stone_revision: revision })));
                }
                dialog.links.push(link(format!("{} · {} {}", text(language, "Cut", "切割", "Cortar", "Cortar"), stone.cutting_gold, gold_label(language)),
                    action_target(view, StoneWorkshopOperation::Cut { serial, expected_stone_revision: revision })));
            } else { dialog.body.push(error_notice(language, &StoneWorkshopError::Rejected(StoneError::InvalidPossession)).into()); }
            dialog.links.push(link(text(language, "Back to raw stones", "返回原石列表", "Voltar às pedras brutas", "Volver a las piedras"), format!("{PREFIX}list:stone:0")));
            add_home(&mut dialog, language);
        },
    }
    if let Some(notice) = notice { dialog.body.insert(0, notice.into()); }
    dialog
}

fn error_notice(language: LanguageCode, error: &StoneWorkshopError) -> &'static str {
    match error {
        StoneWorkshopError::OutcomeUnknown => text(language,
            "The result is not confirmed. Reconnect and check your Bag before another action.", "结果尚未确认。请重连检查背包，确认前不要再次操作。",
            "O resultado não foi confirmado. Reconecte e verifique a Bolsa antes de outra ação.", "El resultado no está confirmado. Reconecta y revisa la Bolsa antes de otra acción."),
        StoneWorkshopError::StaleInventory | StoneWorkshopError::Rejected(StoneError::StaleRevision) => text(language,
            "Your Bag or stone changed. Review the updated item before choosing again.", "背包或原石状态已变化，请查看更新后的物品再选择操作。",
            "A Bolsa ou a pedra mudou. Reveja o item atualizado antes de escolher novamente.", "La Bolsa o la piedra cambió. Revisa el objeto actualizado antes de volver a elegir."),
        StoneWorkshopError::Rejected(StoneError::InsufficientGold) => text(language,
            "Not enough Gold. Your ore or raw stone was kept.", "金币不足，矿石或原石已保留。", "Ouro insuficiente. O minério ou a pedra foi mantido.", "Oro insuficiente. Se conservó el mineral o la piedra."),
        StoneWorkshopError::Rejected(StoneError::InsufficientCapacity) => text(language,
            "Not enough Bag space or carrying capacity. Your raw stone was kept.", "背包空间或负重不足，原石已保留。", "Falta espaço ou capacidade de carga na Bolsa. A pedra foi mantida.", "Falta espacio o capacidad de carga en la Bolsa. Se conservó la piedra."),
        StoneWorkshopError::Rejected(StoneError::AlreadyAppraised) => text(language,
            "This stone was already appraised. Review its existing clue.", "此原石已鉴定，请查看已有线索。", "Esta pedra já foi avaliada. Consulte o indício existente.", "Esta piedra ya fue evaluada. Consulta su pista."),
        StoneWorkshopError::Rejected(StoneError::AlreadyCut) => text(language,
            "This stone was already cut. Check your Bag for its material.", "此原石已切割，请检查背包中的材料。", "Esta pedra já foi cortada. Procure o material na Bolsa.", "Esta piedra ya fue cortada. Busca el material en la Bolsa."),
        StoneWorkshopError::Rejected(StoneError::InvalidPossession | StoneError::UnknownStone) => text(language,
            "This physical item is unavailable or locked. Review your Bag.", "此实体物品已不可用或被锁定，请检查背包。", "Este item físico está indisponível ou bloqueado. Verifique a Bolsa.", "Este objeto físico no está disponible o está bloqueado. Revisa la Bolsa."),
        StoneWorkshopError::InvalidRequest | StoneWorkshopError::Rejected(StoneError::InvalidRequest | StoneError::RequestConflict | StoneError::SourceConflict) => text(language,
            "This choice has expired or conflicts with an earlier action. Review the workshop again.", "此选项已失效或与先前操作冲突，请重新查看工坊。", "Esta escolha expirou ou conflita com uma ação anterior. Consulte a oficina novamente.", "Esta opción caducó o entra en conflicto con una acción anterior. Revisa el taller."),
        _ => text(language, "The workshop is unavailable. No new action was confirmed.", "工坊暂不可用，未确认新的操作。",
            "A oficina está indisponível. Nenhuma nova ação foi confirmada.", "El taller no está disponible. No se confirmó ninguna acción nueva."),
    }
}
fn notice_dialog(active: &ActiveNpcDialogState, language: LanguageCode, notice: &str) -> ActiveNpcDialogState {
    let mut dialog = base_dialog(active, language);
    dialog.body.push(notice.into());
    dialog
}
fn command_page(operation: &StoneWorkshopOperation) -> Page {
    match *operation {
        StoneWorkshopOperation::Seal { uid } => Page::Ore(uid),
        StoneWorkshopOperation::Appraise { serial, .. } | StoneWorkshopOperation::Cut { serial, .. } => Page::Stone(serial),
    }
}
fn current_command(view: &StoneWorkshopView, command: &StoneWorkshopCommand) -> bool {
    if command.expected_inventory_revision != view.inventory_revision
        || command.request_id != quote_request_id(&view.owner, command.expected_inventory_revision, &command.operation)
    { return false; }
    match command.operation {
        StoneWorkshopOperation::Seal { uid } => view.ores.iter().any(|ore| canonical_decimal(&ore.uid, true) == Some(uid)),
        StoneWorkshopOperation::Appraise { serial, expected_stone_revision } | StoneWorkshopOperation::Cut { serial, expected_stone_revision } =>
            view.stones.iter().any(|stone| canonical_decimal(&stone.serial, true) == Some(serial)
                && canonical_decimal(&stone.revision, false) == Some(expected_stone_revision)),
    }
}
fn success_notice(language: LanguageCode, operation: &StoneWorkshopOperation, execution: &StoneWorkshopExecution) -> String {
    if execution.replayed {
        return text(language, "This action was already completed. No second fee or item was issued.", "此操作已完成，未重复扣费或产出物品。",
            "Esta ação já foi concluída. Não houve nova cobrança nem novo item.", "Esta acción ya se completó. No se cobró otra vez ni se entregó otro objeto.").into();
    }
    match operation {
        StoneWorkshopOperation::Seal { .. } => text(language, "Ore sealed. The raw stone is in your Bag.", "矿石已封存，原石已放入背包。",
            "Minério selado. A pedra bruta está na Bolsa.", "Mineral sellado. La piedra bruta está en la Bolsa.").into(),
        StoneWorkshopOperation::Appraise { .. } => text(language, "Appraisal completed. Review the clue below.", "鉴定已完成，请查看下方线索。",
            "Avaliação concluída. Consulte o indício abaixo.", "Evaluación completada. Consulta la pista indicada.").into(),
        StoneWorkshopOperation::Cut { .. } => {
            let label = text(language, "Cutting completed. Material is in your Bag.", "切割已完成，材料已放入背包。",
                "Corte concluído. O material está na Bolsa.", "Corte completado. El material está en la Bolsa.");
            let result: String = execution.public_result.chars().filter(|ch| !ch.is_control() && *ch != '<' && *ch != '>').take(160).collect();
            let result=StoneMaterial::ALL.iter().find(|m| format!("{m:?}")==result)
                .map(|m|material_name(language,*m).to_string()).unwrap_or(result);
            if result.is_empty() { label.into() } else { format!("{label} {result}") }
        },
    }
}
/// Hook only after the original live-dialog link and DataRange checks. Raw
/// target must be passed unchanged: do not strip parentheses or trailing fields.
pub(super) fn handle_target(session: &mut SimulationSession, target: &str) -> Option<Vec<ServerPacket>> {
    if !is_workshop_target(target) { return None; }
    let Some(active) = session.app.world().resource::<NpcStateResource>().active_npc_dialog.clone() else { return Some(Vec::new()); };
    if !active.links.iter().any(|link| link.target == target)
        || !approved_npc(session.app.world(), active.npc_object_id)
    { return Some(Vec::new()); }
    let language = current_language(session.app.world());
    if !workshop_allowed(session.app.world()) {
        let notice = error_notice(language, &StoneWorkshopError::Unavailable);
        set_dialog(session.app.world_mut(), notice_dialog(&active, language, notice));
        return Some(vec![system_message(notice)]);
    }
    let Some(parsed) = parse_target(target) else {
        let notice = error_notice(language, &StoneWorkshopError::InvalidRequest);
        set_dialog(session.app.world_mut(), notice_dialog(&active, language, notice));
        return Some(vec![system_message(notice)]);
    };
    let view = match session.stone_workshop_view() {
        Ok(view) if valid_view(&view) => view,
        _ => {
            let notice = error_notice(language, &StoneWorkshopError::Unavailable);
            set_dialog(session.app.world_mut(), notice_dialog(&active, language, notice));
            return Some(vec![system_message(notice)]);
        },
    };
    match parsed {
        Target::Page(page) => {
            set_dialog(session.app.world_mut(), render_page(&active, language, &view, page, None));
            Some(Vec::new())
        },
        Target::Action(command) => {
            if !current_command(&view, &command) {
                let notice = error_notice(language, &StoneWorkshopError::StaleInventory);
                set_dialog(session.app.world_mut(), render_page(&active, language, &view, command_page(&command.operation), Some(notice)));
                return Some(vec![system_message(notice)]);
            }
            let operation = command.operation.clone();
            match session.execute_stone_workshop(command) {
                Ok(mut execution) => {
                    let notice = success_notice(language, &operation, &execution);
                    let updated = session.stone_workshop_view();
                    let dialog = match updated {
                        Ok(view) if valid_view(&view) => {
                            let page = match operation {
                                StoneWorkshopOperation::Seal { uid } => view.stones.iter().find(|stone| canonical_decimal(&stone.uid, true) == Some(uid))
                                    .and_then(|stone| canonical_decimal(&stone.serial, true)).map(Page::Stone).unwrap_or(Page::Home),
                                StoneWorkshopOperation::Appraise { serial, .. } => Page::Stone(serial),
                                StoneWorkshopOperation::Cut { .. } => Page::Home,
                            };
                            render_page(&active, language, &view, page, Some(&notice))
                        },
                        _ => notice_dialog(&active, language, &notice),
                    };
                    set_dialog(session.app.world_mut(), dialog);
                    execution.packets.push(system_message(&notice));
                    Some(execution.packets)
                },
                Err(error) => {
                    let notice = error_notice(language, &error);
                    // An unknown commit result never triggers refresh, execution,
                    // reroll or replacement action links. Original menu/exit remain.
                    let dialog = if error == StoneWorkshopError::OutcomeUnknown {
                        notice_dialog(&active, language, notice)
                    } else {
                        match session.stone_workshop_view() {
                            Ok(view) if valid_view(&view) => render_page(&active, language, &view, command_page(&operation), Some(notice)),
                            _ => notice_dialog(&active, language, notice),
                        }
                    };
                    set_dialog(session.app.world_mut(), dialog);
                    Some(vec![system_message(notice)])
                },
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mir2_production::StoneCatalog;

    fn owner() -> StoneOwner {
        StoneOwner { account_id: "account-a".into(), character_incarnation: "character-a".into() }
    }
    fn view(count: usize) -> StoneWorkshopView {
        StoneWorkshopView { owner: owner(), inventory_revision: 7, gold: 5000,
            ores: (1..=count).map(|uid| StoneOreView { uid: uid.to_string(), template_id: 824, grade: StoneGrade::Rough }).collect(), stones: vec![] }
    }
    fn active() -> ActiveNpcDialogState {
        ActiveNpcDialogState { npc_object_id: 19, npc_name: "Bill".into(), npc_name_key: None,
            stage: None, current: 0, required: 0, title: "Bill".into(), body: vec!["original".into()], footer: String::new(),
            input: None, links: vec![link("Buy", "@Buy"), link("Back", "@Main"), link("Exit", "@Exit")] }
    }
    fn stone() -> PublicStone {
        let catalog = StoneCatalog::v1();
        let rule = catalog.rule(StoneGrade::Rough).unwrap();
        PublicStone { uid: "9007199254740993".into(), serial: "18446744073709551615".into(), grade: StoneGrade::Rough,
            lifecycle: StoneLifecycle::Sealed, revision: "0".into(), catalog_version: 1, appraisal_gold: rule.appraisal_gold,
            cutting_gold: rule.cutting_gold, possibilities: rule.outcomes.clone(), appraisal_clue: None }
    }
    #[test]
    fn exact_target_parser_roundtrips_max_u64_and_refuses_ambiguous_inputs() {
        let view = view(1);
        for operation in [StoneWorkshopOperation::Seal { uid: u64::MAX },
            StoneWorkshopOperation::Appraise { serial: u64::MAX, expected_stone_revision: u64::MAX },
            StoneWorkshopOperation::Cut { serial: u64::MAX, expected_stone_revision: u64::MAX }]
        {
            let target = action_target(&view, operation.clone());
            assert!(target.len() <= MAX_TARGET_BYTES);
            assert_eq!(parse_target(&target), Some(Target::Action(StoneWorkshopCommand { operation,
                expected_inventory_revision: 7, request_id: target.rsplit(':').next().unwrap().into() })));
            for bad in [format!("{target}:extra"), format!(" {target}"), format!("{target} "), format!("{target}(ignored)"), target.to_ascii_uppercase()] {
                assert_eq!(parse_target(&bad), None);
            }
        }
        for target in ["@stone:v1:list:ore:27", "@stone:v1:list:ore:01", "@stone:v1:list:unknown:0", "@stone:v2:home",
            "@stone:v1:ore:0:7", "@stone:v1:ore:-1:7", "@stone:v1:ore:18446744073709551616:7", "@stone:v1:ore:1:07", "qa.giveItem"]
        { assert_eq!(parse_target(target), None); }
        assert_eq!(parse_target(&format!("{PREFIX}{}", "x".repeat(257))), None);
        assert_eq!(parse_target(HOME), Some(Target::Page(Page::Home)));
    }
    #[test]
    fn quote_binds_owner_incarnation_item_operation_and_both_revisions() {
        let view = view(1);
        let op = StoneWorkshopOperation::Cut { serial: 8, expected_stone_revision: 2 };
        let quote = quote_request_id(&view.owner, 7, &op);
        assert!(valid_request_id(&quote));
        assert_eq!(quote, quote_request_id(&view.owner, 7, &op));
        let mut owner = view.owner.clone(); owner.character_incarnation.push_str("-new");
        assert_ne!(quote, quote_request_id(&owner, 7, &op));
        owner = view.owner.clone(); owner.account_id.push_str("-new");
        assert_ne!(quote, quote_request_id(&owner, 7, &op));
        for changed in [StoneWorkshopOperation::Cut { serial: 9, expected_stone_revision: 2 },
            StoneWorkshopOperation::Cut { serial: 8, expected_stone_revision: 3 },
            StoneWorkshopOperation::Appraise { serial: 8, expected_stone_revision: 2 }]
        { assert_ne!(quote, quote_request_id(&view.owner, 7, &changed)); }
        assert_ne!(quote, quote_request_id(&view.owner, 8, &op));
    }
    #[test]
    fn eighty_physical_items_are_paginated_three_at_a_time_and_keep_original_links() {
        let view = view(80);
        assert!(valid_view(&view));
        let mut seen = BTreeSet::new();
        for page in 0..=MAX_PAGE {
            let dialog = render_page(&active(), LanguageCode::English, &view, Page::List(Items::Ore, page), None);
            let choices: Vec<_> = dialog.links.iter().filter(|link| link.target.starts_with("@stone:v1:ore:")).collect();
            assert!(!choices.is_empty() && choices.len() <= 3);
            for link in choices { assert!(seen.insert(link.target.clone())); }
            for target in ["@Buy", "@Main", "@Exit"] { assert!(dialog.links.iter().any(|link| link.target == target)); }
        }
        assert_eq!(seen.len(), 80);
    }
    #[test]
    fn stone_detail_shows_six_exact_probabilities_frozen_fees_and_only_coarse_clue() {
        let mut view = view(0); view.stones.push(stone());
        assert!(valid_view(&view));
        let dialog = render_page(&active(), LanguageCode::English, &view, Page::Stone(u64::MAX), None);
        assert_eq!(dialog.body.iter().filter(|line| line.ends_with('%')).count(), 6);
        assert!(dialog.body.iter().any(|line| line.contains("80.00%")));
        assert!(dialog.body.iter().any(|line| line.contains("0.50%")));
        assert!(dialog.body.iter().any(|line| line.contains("100") && line.contains("1500")));
        assert!(dialog.body.iter().any(|line| line.contains("Not appraised")));
        assert_eq!(dialog.links.iter().filter(|link| link.target.starts_with("@stone:v1:appraise:")).count(), 1);
        view.stones[0].lifecycle = StoneLifecycle::Appraised;
        view.stones[0].revision = "1".into(); view.stones[0].appraisal_clue = Some(mir2_production::StoneClue::Crystalline);
        let dialog = render_page(&active(), LanguageCode::English, &view, Page::Stone(u64::MAX), None);
        assert!(dialog.body.iter().any(|line| line.contains("jade crystal or a gem")));
        assert_eq!(dialog.links.iter().filter(|link| link.target.starts_with("@stone:v1:appraise:")).count(), 0);
        assert_eq!(dialog.links.iter().filter(|link| link.target.starts_with("@stone:v1:cut:")).count(), 1);
    }
    #[test]
    fn unknown_result_page_has_no_action_or_automatic_refresh_links() {
        let mut active = active(); active.links.push(link("Cut", "@stone:v1:cut:1:0:7:old"));
        for language in [LanguageCode::English, LanguageCode::ChineseSimplified, LanguageCode::Portuguese, LanguageCode::Spanish] {
            let notice = error_notice(language, &StoneWorkshopError::OutcomeUnknown);
            let dialog = notice_dialog(&active, language, notice);
            assert_eq!(dialog.body, vec![notice.to_string()]);
            assert!(dialog.links.iter().all(|link| !is_workshop_target(&link.target)));
            assert!(dialog.links.iter().any(|link| link.target == "@Exit"));
        }
    }
    #[test]
    fn only_two_actual_whitelisted_script_locations_are_approved() {
        assert!(allowed_location("BichonProvince/BichonWall/Blacksmith", "0", 302, 221));
        assert!(allowed_location("MongchonProvince/MudWall/Blacksmith", "0159", 5, 9));
        for tuple in [("BichonProvince/BichonWall/Blacksmith1", "0", 302, 221),
            ("BichonProvince/BichonWall/Blacksmith", "0", 302, 222),
            ("BichonProvince/BichonWall/Blacksmith", "0103", 302, 221),
            ("MongchonProvince/MudWall/Blacksmith-0159", "0159", 5, 9)]
        { assert!(!allowed_location(tuple.0, tuple.1, tuple.2, tuple.3)); }
    }
    #[test]
    fn displayed_command_is_bound_to_current_inventory_and_public_stone_revision() {
        let mut view = view(0); view.stones.push(stone());
        let op = StoneWorkshopOperation::Cut { serial: u64::MAX, expected_stone_revision: 0 };
        let request_id = quote_request_id(&view.owner, view.inventory_revision, &op);
        let command = StoneWorkshopCommand { operation: op, request_id, expected_inventory_revision: view.inventory_revision };
        assert!(current_command(&view, &command));
        view.inventory_revision += 1; assert!(!current_command(&view, &command));
        view.inventory_revision -= 1; view.stones[0].revision = "1".into(); assert!(!current_command(&view, &command));
        view.stones[0].revision = "0".into(); view.owner.character_incarnation = "replacement".into();
        assert!(!current_command(&view, &command));
    }
    #[test]
    fn duplicate_carriers_bad_decimal_and_forged_material_probabilities_are_not_rendered() {
        let mut view = view(1); view.stones.push(stone());
        assert!(valid_view(&view));
        view.stones[0].uid = "1".into(); assert!(!valid_view(&view));
        view.stones[0].uid = "9007199254740993".into(); view.stones[0].serial = "01".into(); assert!(!valid_view(&view));
        view.stones[0].serial = "1".into(); view.stones[0].possibilities[0].template_id = 1; assert!(!valid_view(&view));
        assert!(!valid_view(&super::tests::view(81)));
    }
    #[test]
    fn four_supported_languages_render_localized_workshop_and_failure_notices() {
        let view = view(1);
        let mut titles = BTreeSet::new();
        for language in [LanguageCode::English, LanguageCode::ChineseSimplified, LanguageCode::Portuguese, LanguageCode::Spanish] {
            let dialog = render_page(&active(), language, &view, Page::Home, None);
            titles.insert(dialog.title);
            assert!(!error_notice(language, &StoneWorkshopError::Rejected(StoneError::InsufficientGold)).is_empty());
            assert!(!material_name(language, StoneMaterial::SoulGem).is_empty());
        }
        assert_eq!(titles.len(), 4);
    }
    #[test]
    fn seal_and_appraisal_success_never_forward_an_incidental_private_result_string() {
        let execution = StoneWorkshopExecution { packets: vec![], replayed: false, public_result: "private-gem-name".into() };
        for op in [StoneWorkshopOperation::Seal { uid: 1 }, StoneWorkshopOperation::Appraise { serial: 1, expected_stone_revision: 0 }] {
            assert!(!success_notice(LanguageCode::English, &op, &execution).contains("private-gem-name"));
        }
        assert!(success_notice(LanguageCode::English, &StoneWorkshopOperation::Cut { serial: 1, expected_stone_revision: 0 }, &execution).contains("private-gem-name"));
    }
}
