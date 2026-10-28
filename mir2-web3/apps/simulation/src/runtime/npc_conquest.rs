//! Ordinary, range-bound NPC entry into the shared siege service.
//! Labels are capabilities displayed by the current server-owned NPC dialog;
//! they never carry a client-selected conquest index or actor identity.
use super::npc::{
    crystal_npc_object_in_data_range, set_dialog, ActiveNpcDialogState, NpcDialogLinkState,
};
use super::npc_script::NpcInteractionContext;
use super::resources::{MapRuntimeResource, RuntimeConfigResource, SessionResource};
use super::session::SimulationSession;
use crate::config::{ConquestManagementAction, Stage5FriendIdentity};
use crate::conquest::{ConquestDefenseKind, SharedConquestRecord};
use bevy_ecs::world::World;
use mir2_protocol::{ChatType, ServerPacket};

const INDEX: i32 = 1;
const REGISTRAR: u32 = 427;
const OFFICER: u32 = 1146;
fn now_ms() -> u64 {
    u64::try_from(super::stage5::now_unix_ms()).unwrap_or(u64::MAX)
}

pub(super) fn enabled(world: &World) -> bool {
    world
        .resource::<RuntimeConfigResource>()
        .config
        .conquest_policies
        .iter()
        .any(|p| p.index == INDEX)
}

/// Source script reads share the same durable record as the repaired public
/// entry. A legacy personal Stage5 snapshot cannot grant castle ownership.
pub(super) fn script_snapshot(world: &World) -> Option<SharedConquestRecord> {
    if !enabled(world) {
        return None;
    }
    world
        .resource::<RuntimeConfigResource>()
        .config
        .shared_conquest_snapshot_checked(INDEX)
        .ok()
        .flatten()
}

pub(super) fn script_owner_matches(world: &World) -> bool {
    let Some(record) = script_snapshot(world) else {
        return false;
    };
    let Some(owner) = record.owner_guild_id else {
        return false;
    };
    let Some(identity) = actor(world) else {
        return false;
    };
    let config = &world.resource::<RuntimeConfigResource>().config;
    let Ok(store) = config.account_store.lock() else {
        return false;
    };
    store
        .shared_guilds
        .get(&owner)
        .is_some_and(|guild| guild.member(&identity).is_some())
}

pub(super) fn script_owner_name(world: &World) -> String {
    let Some(record) = script_snapshot(world) else {
        return "No Owner".into();
    };
    let config = &world.resource::<RuntimeConfigResource>().config;
    let Ok(store) = config.account_store.lock() else {
        return "No Owner".into();
    };
    record
        .owner_guild_id
        .as_ref()
        .and_then(|id| store.shared_guilds.get(id))
        .map(|guild| guild.name.clone())
        .unwrap_or_else(|| "No Owner".into())
}

fn binding(world: &World, id: u32) -> bool {
    let config = &world.resource::<RuntimeConfigResource>().config;
    if !config.conquest_policies.iter().any(|p| p.index == INDEX) {
        return false;
    }
    let map = &world.resource::<MapRuntimeResource>().current_map.file_name;
    (id == REGISTRAR && map.eq_ignore_ascii_case("0122"))
        || (id == OFFICER && map.eq_ignore_ascii_case("0150"))
}
fn link(text: impl Into<String>, target: &str) -> NpcDialogLinkState {
    NpcDialogLinkState {
        text: text.into(),
        target: target.into(),
    }
}
pub(super) fn append_registrar_entry(
    world: &World,
    context: &NpcInteractionContext,
    dialog: &mut ActiveNpcDialogState,
) {
    if context.object_id == REGISTRAR && binding(world, context.object_id) {
        dialog
            .links
            .push(link("Siege registration", "@sabuk:status"));
    }
}
pub(super) fn officer_main(
    world: &mut World,
    context: &NpcInteractionContext,
) -> Option<Vec<ServerPacket>> {
    if context.object_id != OFFICER || !binding(world, context.object_id) {
        return None;
    }
    Some(show_page(
        world,
        context.object_id,
        &context.name,
        context.name_key.clone(),
        "status",
        None,
    ))
}
fn actor(world: &World) -> Option<Stage5FriendIdentity> {
    let session = world.resource::<SessionResource>();
    Some(Stage5FriendIdentity {
        account_id: session.account_id.clone()?,
        character_index: session.selected_character.as_ref()?.index,
    })
}

impl SimulationSession {
    /// Invoked only after active-dialog label membership and current DataRange.
    pub(super) fn select_shared_conquest_npc(&mut self, target: &str) -> Option<Vec<ServerPacket>> {
        if !target.to_ascii_lowercase().starts_with("@sabuk:") {
            return None;
        }
        if super::components::current_player_is_dead(self.app.world()) {
            return Some(Vec::new());
        }
        let dialog = self
            .app
            .world()
            .resource::<super::resources::NpcStateResource>()
            .active_npc_dialog
            .clone()?;
        if !binding(self.app.world(), dialog.npc_object_id)
            || !crystal_npc_object_in_data_range(self.app.world(), dialog.npc_object_id)
        {
            return Some(Vec::new());
        }
        let config = self
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .clone();
        let Some(identity) = actor(self.app.world()) else {
            return Some(Vec::new());
        };
        let action = target.strip_prefix("@sabuk:").unwrap_or("");
        let now = now_ms();
        let mut notice = None;
        let mut page = "status";
        match action {
            "status" => {}
            "manage" | "repairs" | "archers" => page = action,
            "register" => {
                notice = Some(
                    if config
                        .request_shared_conquest(
                            &identity.account_id,
                            identity.character_index,
                            INDEX,
                            now,
                        )
                        .is_ok()
                    {
                        "Registration saved."
                    } else {
                        "Siege registration failed. Reopen this page and try again."
                    },
                );
            }
            _ => {
                if dialog.npc_object_id != OFFICER {
                    return Some(Vec::new());
                }
                let intent = if let Some(rate) = action
                    .strip_prefix("tax:")
                    .and_then(|r| r.parse::<u8>().ok())
                {
                    Some(ConquestManagementAction::SetTax { rate })
                } else if action == "open" || action == "close" {
                    Some(ConquestManagementAction::SetGate {
                        key: "gate:1".into(),
                        open: action == "open",
                    })
                } else if let Some(key) = action.strip_prefix("repair:") {
                    Some(ConquestManagementAction::Repair { key: key.into() })
                } else if action == "withdraw" {
                    Some(ConquestManagementAction::WithdrawTax)
                } else {
                    None
                };
                let Some(intent) = intent else {
                    return Some(Vec::new());
                };
                notice = Some(
                    if config
                        .manage_shared_conquest(
                            &identity.account_id,
                            identity.character_index,
                            INDEX,
                            now,
                            intent,
                        )
                        .is_ok()
                    {
                        "Castle management saved."
                    } else {
                        "Castle management failed. Check permissions and guild funds."
                    },
                );
                page = "manage";
            }
        }
        Some(show_page(
            self.app.world_mut(),
            dialog.npc_object_id,
            &dialog.npc_name,
            dialog.npc_name_key,
            page,
            notice,
        ))
    }
}

#[cfg(test)]
#[path = "npc_conquest_tests.rs"]
mod tests;

fn show_page(
    world: &mut World,
    id: u32,
    name: &str,
    name_key: Option<String>,
    page: &str,
    notice: Option<&str>,
) -> Vec<ServerPacket> {
    let config = world.resource::<RuntimeConfigResource>().config.clone();
    let Some(policy) = config.conquest_policies.iter().find(|p| p.index == INDEX) else {
        return Vec::new();
    };
    let identity = actor(world);
    let result = config.shared_conquest_snapshot_checked(INDEX);
    let Ok(snapshot) = result else {
        return vec![ServerPacket::Chat {
            message: "Siege registration failed. Reopen this page and try again.".into(),
            chat_type: ChatType::System,
        }];
    };
    let record = snapshot.unwrap_or_else(|| SharedConquestRecord::new(INDEX));
    let Ok(store) = config.account_store.lock() else {
        return Vec::new();
    };
    let own = identity.as_ref().and_then(|actor| {
        store
            .shared_guilds
            .values()
            .find(|g| g.member(actor).is_some())
    });
    let leader = own.is_some_and(|g| {
        identity
            .as_ref()
            .and_then(|actor| g.member(actor))
            .is_some_and(|m| m.rank_index == 0)
    });
    let owner_leader =
        leader && own.is_some_and(|g| record.owner_guild_id.as_deref() == Some(g.id.as_str()));
    let guild_name = |id: Option<&str>, none: &str| {
        id.and_then(|id| store.shared_guilds.get(id))
            .map(|g| g.name.as_str())
            .unwrap_or(none)
            .to_string()
    };
    let owner = guild_name(record.owner_guild_id.as_deref(), "No guild");
    let attacker = guild_name(record.attacker_guild_id.as_deref(), "No applicant");
    let mut body = vec![
        if record.owner_guild_id.is_some() {
            format!("Castle owner: {owner}")
        } else {
            "Castle is unowned.".into()
        },
        if record.attacker_guild_id.is_some() {
            format!("Registered attacker: {attacker}")
        } else {
            "No siege applicant.".into()
        },
    ];
    let time = |minute: u16| format!("{:02}:{:02}", minute / 60, minute % 60);
    let offset = policy.utc_offset_minutes;
    body.push(format!(
        "Siege schedule: daily {}-{} (UTC{}{}:{:02}).",
        time(policy.start_minute),
        time(policy.start_minute + policy.duration_minutes),
        if offset >= 0 { "+" } else { "-" },
        offset.abs() / 60,
        offset.abs() % 60
    ));
    body.push(
        if record.war_active && now_ms() < record.war_ends_ms {
            "War in progress."
        } else {
            "War has not started."
        }
        .into(),
    );
    if let Some(notice) = notice {
        body.insert(0, notice.into());
    }
    let mut links = Vec::new();
    let management = id == OFFICER && owner_leader && page != "status";
    let title = if management {
        "Castle management"
    } else {
        "Siege status"
    };
    if management {
        body.push(format!("Tax rate: {}%", record.tax_rate_percent));
        body.push(format!("Tax treasury: {} gold", record.gold));
        body.push(format!(
            "Guild bank: {} gold",
            own.map(|g| g.gold).unwrap_or(0)
        ));
        match page {
            "repairs" | "archers" => {
                for (key, defense) in &record.defenses {
                    let archer = matches!(defense.kind, ConquestDefenseKind::Archer);
                    if archer != (page == "archers") {
                        continue;
                    }
                    let caption = match defense.kind {
                        ConquestDefenseKind::Gate => {
                            format!("Gate {}: {}/{}", defense.slot, defense.hp, defense.max_hp)
                        }
                        ConquestDefenseKind::Wall => {
                            format!("Wall {}: {}/{}", defense.slot, defense.hp, defense.max_hp)
                        }
                        ConquestDefenseKind::Archer => {
                            format!("Archer {}: {}/{}", defense.slot, defense.hp, defense.max_hp)
                        }
                    };
                    body.push(caption);
                    if let Ok(cost) = record.repair_cost(key) {
                        body.push(format!("Repair cost: {cost} gold."));
                    }
                    let caption = match defense.kind {
                        ConquestDefenseKind::Gate => "Repair gate".into(),
                        ConquestDefenseKind::Wall => format!("Repair wall {}", defense.slot),
                        ConquestDefenseKind::Archer => format!("Restore archer {}", defense.slot),
                    };
                    links.push(link(caption, &format!("@sabuk:repair:{key}")));
                }
            }
            _ => {
                for rate in [10, 15, 20, 25] {
                    links.push(link(
                        format!("Set tax: {rate}%"),
                        &format!("@sabuk:tax:{rate}"),
                    ));
                }
                links.push(link("Open gate", "@sabuk:open"));
                links.push(link("Close gate", "@sabuk:close"));
                links.push(link("Repair defenses", "@sabuk:repairs"));
                links.push(link("Restore archers", "@sabuk:archers"));
                links.push(link("Withdraw taxes to guild bank", "@sabuk:withdraw"));
            }
        }
        links.push(link("Back", "@sabuk:manage"));
    } else {
        if id == REGISTRAR
            && leader
            && !record.war_active
            && record.attacker_guild_id.is_none()
            && !owner_leader
        {
            links.push(link("Apply for the next siege", "@sabuk:register"));
        }
        if id == OFFICER && owner_leader {
            links.push(link("Castle management", "@sabuk:manage"));
        }
    }
    links.push(link("Close", "@Exit"));
    drop(store);
    set_dialog(
        world,
        ActiveNpcDialogState {
            npc_object_id: id,
            npc_name: name.into(),
            npc_name_key: name_key,
            stage: None,
            current: 0,
            required: 0,
            title: title.into(),
            body,
            footer: "Sabuk Wall".into(),
            links,
            input: None,
        },
    );
    Vec::new()
}
