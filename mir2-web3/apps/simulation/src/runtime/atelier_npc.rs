//! Alice's ordinary displayed links, never a generic crafting/admin ingress.
use super::atelier::{self, Action, Command, Error};
use super::components::{entity_by_object_id, entity_position, Npc, NpcAgent};
use super::npc::{set_dialog, ActiveNpcDialogState, NpcDialogLinkState};
use super::resources::{
    current_language, MapRuntimeResource, NpcStateResource, PlayerRuntimeResource, SessionResource,
    Stage5SystemsResource,
};
use super::session::{ActiveSessionIdentity, SimulationSession};
use bevy_ecs::prelude::{Resource, World};
use mir2_game_data::{atelier as catalog, LanguageCode};
use mir2_protocol::ServerPacket;

const PREFIX: &str = "@atelier:v1:";
const HOME: &str = "@atelier:v1:home";

// Quotes belong to the final Source revision, not the atelier transaction's
// intermediate revision. The original displayed menu retains the normal NPC
// actor/entity/map permission until the outermost Source command has settled.
#[derive(Resource)]
struct PendingAtelier {
    active: ActiveNpcDialogState,
    origin_target: String,
    owner: ActiveSessionIdentity,
    generation: u64,
    recipe_id: Option<String>,
    notice: Option<String>,
}

pub(super) fn discard_pending(world: &mut World) {
    world.remove_resource::<PendingAtelier>();
}

fn request_page(
    session: &mut SimulationSession,
    active: ActiveNpcDialogState,
    origin_target: &str,
    recipe_id: Option<&str>,
    notice: Option<&str>,
    packets: &mut Vec<ServerPacket>,
) {
    let Some(owner) = session.active_identity() else {
        return;
    };
    let Some(generation) = session
        .app
        .world()
        .resource::<SessionResource>()
        .ranking_inspect_generation()
    else {
        return;
    };
    session.app.world_mut().insert_resource(PendingAtelier {
        active,
        origin_target: origin_target.into(),
        owner,
        generation,
        recipe_id: recipe_id.map(str::to_owned),
        notice: notice.map(str::to_owned),
    });
    // Private implementation callers have no outer Source owner. Production
    // public/WorldRuntime callers leave this queued until their final finish.
    publish_pending(session, packets);
}

fn response_page(dialog: &ActiveNpcDialogState) -> Vec<String> {
    let mut page = vec![dialog.title.clone()];
    page.extend(dialog.body.iter().cloned());
    page.extend(
        dialog
            .links
            .iter()
            .map(|link| format!("<{}/{}>", link.text, link.target)),
    );
    page.push(dialog.footer.clone());
    page
}

pub(super) fn publish_pending(session: &mut SimulationSession, packets: &mut Vec<ServerPacket>) {
    if super::shared_guild_experience::source_command_active(session.app.world()) {
        return;
    }
    let Some(pending) = session.app.world_mut().remove_resource::<PendingAtelier>() else {
        return;
    };
    // Never let a pre-settlement atelier page escape with the final response.
    packets.retain(|packet| {
        !matches!(packet, ServerPacket::NPCResponse { page }
        if page.iter().any(|line| line.contains(PREFIX)))
    });
    if session.active_identity().as_ref() != Some(&pending.owner)
        || session
            .app
            .world()
            .resource::<SessionResource>()
            .ranking_inspect_generation()
            != Some(pending.generation)
        || !atelier::allowed(session.app.world())
        || !super::npc_input::has_menu_target(session.app.world(), &pending.origin_target)
    {
        return;
    }
    let dialog = render(
        session,
        &pending.active,
        pending.recipe_id.as_deref(),
        pending.notice.as_deref(),
    );
    let page = response_page(&dialog);
    set_dialog(session.app.world_mut(), dialog);
    packets.push(ServerPacket::NPCResponse { page });
}

fn text(
    language: LanguageCode,
    en: &'static str,
    zh: &'static str,
    pt: &'static str,
    es: &'static str,
) -> &'static str {
    match language {
        LanguageCode::English => en,
        LanguageCode::ChineseSimplified => zh,
        LanguageCode::Portuguese => pt,
        LanguageCode::Spanish => es,
    }
}
fn title(language: LanguageCode) -> &'static str {
    text(
        language,
        "Crafting workshop",
        "制作工坊",
        "Oficina de criação",
        "Taller de creación",
    )
}
fn link(label: impl Into<String>, target: impl Into<String>) -> NpcDialogLinkState {
    NpcDialogLinkState {
        text: label.into(),
        target: target.into(),
    }
}

pub(super) fn approved_npc(world: &World, object_id: u32) -> bool {
    if object_id != 33 {
        return false;
    }
    let Some(map) = world.get_resource::<MapRuntimeResource>() else {
        return false;
    };
    if map.current_map.file_name != "0" {
        return false;
    }
    let Some(entity) = entity_by_object_id(world, object_id) else {
        return false;
    };
    let entry = world.entity(entity);
    entry.contains::<Npc>()
        && entry
            .get::<NpcAgent>()
            .and_then(|agent| agent.script_key.as_deref())
            == Some("BichonProvince/BichonWall/CraftsLady")
        && entity_position(world, entity).is_some_and(|p| p.x == 321 && p.y == 265)
}

pub(super) fn decorate_dialog(world: &World, dialog: &mut ActiveNpcDialogState) {
    if !atelier::enabled(world)
        || !approved_npc(world, dialog.npc_object_id)
        || dialog.input.is_some()
        || dialog.title == title(current_language(world))
        || dialog.links.iter().any(|l| l.target.starts_with(PREFIX))
    {
        return;
    }
    dialog
        .links
        .push(link(title(current_language(world)), HOME));
}

fn target(command: &Command) -> String {
    let action = match &command.action {
        Action::Craft { recipe } => format!("craft:{recipe}"),
        Action::Wear { fashion } => format!("wear:{fashion}"),
        Action::Clear => "clear".into(),
    };
    format!(
        "{PREFIX}{action}:{}:{}",
        command.expected_revision, command.request_id
    )
}

fn parse_command(target: &str) -> Option<Command> {
    if target.len() > 256 {
        return None;
    }
    let parts: Vec<_> = target.strip_prefix(PREFIX)?.split(':').collect();
    let (action, revision, request) = match parts.as_slice() {
        ["craft", id, revision, request] if catalog::recipe(id).is_some() => (
            Action::Craft {
                recipe: (*id).into(),
            },
            *revision,
            *request,
        ),
        ["wear", id, revision, request] if catalog::fashion(id).is_some() => (
            Action::Wear {
                fashion: (*id).into(),
            },
            *revision,
            *request,
        ),
        ["clear", revision, request] => (Action::Clear, *revision, *request),
        _ => return None,
    };
    let expected_revision = revision.parse::<u64>().ok()?;
    if expected_revision == 0
        || expected_revision.to_string() != revision
        || request.len() != 64
        || !request
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return None;
    }
    Some(Command {
        action,
        expected_revision,
        request_id: request.into(),
    })
}

fn error_notice(language: LanguageCode, error: &Error) -> &'static str {
    match error {
        Error::OutcomeUnknown => text(language,
            "The result is unknown. Do not repeat this action; reconnect to inspect the saved result.",
            "结果尚不确定。请勿重复操作；重新登录后核对已保存结果。",
            "O resultado é desconhecido. Não repita; reconecte para verificar o resultado salvo.",
            "El resultado es desconocido. No repitas; vuelve a conectar para revisar lo guardado."),
        Error::Stale => text(language, "This quote expired. Review the current inventory before trying again.",
            "报价已失效，请核对当前背包后重新选择。", "Esta oferta expirou. Confira a Bolsa antes de tentar novamente.",
            "Esta oferta caducó. Revisa la Bolsa antes de volver a intentarlo."),
        Error::Materials => text(language, "Eligible Bag materials or carrying capacity are insufficient.",
            "背包内合格材料或负重容量不足。", "Faltam materiais válidos na Bolsa ou capacidade de carga.",
            "Faltan materiales válidos en la Bolsa o capacidad de carga."),
        Error::Gold => text(language, "Not enough Gold. Nothing was consumed.", "金币不足，未消耗材料。",
            "Ouro insuficiente. Nada foi consumido.", "Oro insuficiente. No se consumió nada."),
        Error::AlreadyOwned => text(language, "This fashion is already unlocked. Nothing was consumed.",
            "已解锁此时装，未重复消耗。", "Este traje já foi desbloqueado. Nada foi consumido.",
            "Este atuendo ya está desbloqueado. No se consumió nada."),
        Error::UnsupportedClass => text(language, "This fashion does not support your class.", "此时装不支持当前职业。",
            "Este traje não é compatível com sua classe.", "Este atuendo no es compatible con tu clase."),
        Error::NoChange => text(language, "The requested appearance is already active.", "当前外观无需更改。",
            "A aparência solicitada já está ativa.", "La apariencia solicitada ya está activa."),
        _ => text(language, "The workshop rejected this action. No success was confirmed.",
            "工坊拒绝了此操作，未确认成功。", "A oficina recusou esta ação. Nenhum sucesso foi confirmado.",
            "El taller rechazó esta acción. No se confirmó el éxito."),
    }
}

fn base(active: &ActiveNpcDialogState, language: LanguageCode) -> ActiveNpcDialogState {
    let mut dialog = active.clone();
    dialog.title = title(language).into();
    dialog.stage = None;
    dialog.current = 0;
    dialog.required = 0;
    dialog.input = None;
    dialog.body.clear();
    dialog.links.clear();
    dialog.footer = text(
        language,
        "Cosmetics change appearance only.",
        "时装仅改变外观，不增加战斗属性。",
        "Os trajes alteram apenas a aparência.",
        "Los trajes solo cambian la apariencia.",
    )
    .into();
    dialog
}

fn render(
    session: &mut SimulationSession,
    active: &ActiveNpcDialogState,
    recipe_id: Option<&str>,
    notice: Option<&str>,
) -> ActiveNpcDialogState {
    let language = current_language(session.app.world());
    let mut dialog = base(active, language);
    if let Some(notice) = notice {
        dialog.body.push(notice.into());
    }
    let gold = session.app.world().resource::<PlayerRuntimeResource>().gold;
    dialog.body.push(format!("Gold: {gold}"));
    if let Some(recipe) = recipe_id.and_then(catalog::recipe) {
        dialog.body.push(recipe.name(language).into());
        for ingredient in recipe.ingredients {
            let label = match ingredient.template_id {
                824 => text(
                    language,
                    "Copper ore",
                    "铜矿",
                    "Minério de cobre",
                    "Mineral de cobre",
                ),
                826 => text(
                    language,
                    "Silver ore",
                    "银矿",
                    "Minério de prata",
                    "Mineral de plata",
                ),
                904 => text(
                    language,
                    "Jade crystal",
                    "玉晶",
                    "Cristal de jade",
                    "Cristal de jade",
                ),
                _ => "?",
            };
            dialog.body.push(format!("{label} ×{}", ingredient.count));
        }
        dialog.body.push(format!("Gold: {}", recipe.gold));
        match session.quote_atelier(Action::Craft {
            recipe: recipe.id.into(),
        }) {
            Ok(command) => dialog.links.push(link(
                text(language, "Make", "制作", "Criar", "Crear"),
                target(&command),
            )),
            Err(error) => dialog.body.push(error_notice(language, &error).into()),
        }
        dialog.links.push(link(title(language), HOME));
    } else {
        for recipe in catalog::RECIPES {
            dialog.links.push(link(
                recipe.name(language),
                format!("{PREFIX}recipe:{}", recipe.id),
            ));
        }
        let appearance = session
            .app
            .world()
            .resource::<Stage5SystemsResource>()
            .stage5_systems
            .appearance
            .clone();
        for id in &appearance.owned_fashions {
            let Some(style) = catalog::fashion(id) else {
                continue;
            };
            if let Ok(command) = session.quote_atelier(Action::Wear {
                fashion: id.clone(),
            }) {
                dialog.links.push(link(
                    format!(
                        "{} · {}",
                        text(language, "Wear", "穿戴", "Vestir", "Vestir"),
                        style.name(language)
                    ),
                    target(&command),
                ));
            }
        }
        if let Ok(command) = session.quote_atelier(Action::Clear) {
            dialog.links.push(link(
                text(
                    language,
                    "Remove fashion",
                    "脱下时装",
                    "Remover traje",
                    "Quitar atuendo",
                ),
                target(&command),
            ));
        }
    }
    dialog.links.push(link(
        text(
            language,
            "Alice menu",
            "返回艾丽丝",
            "Menu de Alice",
            "Menú de Alice",
        ),
        "@Main",
    ));
    dialog.links.push(link(
        text(language, "Exit", "离开", "Sair", "Salir"),
        "@Exit",
    ));
    dialog
}

pub(super) fn handle_target(
    session: &mut SimulationSession,
    target_text: &str,
) -> Option<Vec<ServerPacket>> {
    discard_pending(session.app.world_mut());
    if !target_text.starts_with(PREFIX) {
        return None;
    }
    let Some(active) = session
        .app
        .world()
        .resource::<NpcStateResource>()
        .active_npc_dialog
        .clone()
    else {
        return Some(Vec::new());
    };
    if !active.links.iter().any(|link| link.target == target_text)
        || !approved_npc(session.app.world(), active.npc_object_id)
        || !super::npc::crystal_npc_object_in_data_range(session.app.world(), active.npc_object_id)
        || !atelier::allowed(session.app.world())
    {
        return Some(Vec::new());
    }
    if target_text == HOME {
        let mut packets = Vec::new();
        request_page(session, active, target_text, None, None, &mut packets);
        return Some(packets);
    }
    if let Some(id) = target_text
        .strip_prefix("@atelier:v1:recipe:")
        .filter(|id| catalog::recipe(id).is_some())
    {
        let mut packets = Vec::new();
        request_page(session, active, target_text, Some(id), None, &mut packets);
        return Some(packets);
    }
    let language = current_language(session.app.world());
    let Some(command) = parse_command(target_text) else {
        return Some(Vec::new());
    };
    match session.execute_atelier(command) {
        Ok(mut execution) => {
            let notice = if execution.replayed {
                text(
                    language,
                    "This action was already completed. Nothing was spent or issued again.",
                    "此操作已完成，未重复扣费或发放。",
                    "Esta ação já foi concluída. Nada foi gasto ou emitido novamente.",
                    "Esta acción ya se completó. No se volvió a gastar ni entregar nada.",
                )
            } else {
                text(
                    language,
                    "Completed. Review your Bag or wardrobe.",
                    "制作完成，请查看背包或衣橱。",
                    "Concluído. Confira a Bolsa ou o guarda-roupa.",
                    "Completado. Revisa la Bolsa o el guardarropa.",
                )
            };
            request_page(
                session,
                active,
                target_text,
                None,
                Some(notice),
                &mut execution.packets,
            );
            execution
                .packets
                .push(super::packets::system_message(notice));
            Some(execution.packets)
        }
        Err(error) => {
            let notice = error_notice(language, &error);
            let mut packets = vec![super::packets::system_message(notice)];
            if error == Error::OutcomeUnknown {
                // No new quote or automatic retry after an ambiguous commit.
                let mut dialog = base(&active, language);
                dialog.body.push(notice.into());
                dialog.links.push(link(
                    text(language, "Exit", "离开", "Sair", "Salir"),
                    "@Exit",
                ));
                let page = response_page(&dialog);
                set_dialog(session.app.world_mut(), dialog);
                packets.push(ServerPacket::NPCResponse { page });
            } else {
                request_page(
                    session,
                    active,
                    target_text,
                    None,
                    Some(notice),
                    &mut packets,
                );
            }
            Some(packets)
        }
    }
}
