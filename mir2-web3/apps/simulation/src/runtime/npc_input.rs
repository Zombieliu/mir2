//! Local displayed-menu permission and single-use NPC input permission. Both
//! are separate from cloneable checkpoints and online/Zone/life qualification.
use bevy_ecs::{
    entity::Entity,
    prelude::{Resource, World},
};
use mir2_game_data::crystal_npc_script_by_key;
use sha2::{Digest, Sha256};

use super::components::{
    current_player_is_dead, entity_by_object_id, entity_position, player_entity, Npc, NpcAgent,
};
use super::npc::{
    crystal_npc_object_in_data_range, set_dialog, ActiveNpcDialogState, NpcDialogInputState,
};
use super::npc_script::crystal_npc_dialog_targets_match;
use super::resources::{is_in_world, MapRuntimeResource, NpcStateResource, SessionResource};
use super::save::active_session_mutating_account_id;

#[derive(Debug, PartialEq, Eq)]
struct NpcActorBinding {
    player: Entity,
    account: String,
    character_index: i32,
    character_name: String,
    map_index: i32,
    map_file_name: String,
    npc: Entity,
    npc_object_id: u32,
    npc_x: i32,
    npc_y: i32,
    declared_script_key: Option<String>,
    script: Option<NpcScriptBinding>,
}

#[derive(Debug, PartialEq, Eq)]
struct NpcScriptBinding {
    script_key: String,
    relative_path: String,
    raw_text_sha256: [u8; 32],
    imported_script_sha256: [u8; 32],
}

#[derive(Resource)]
struct PendingNpcInput {
    binding: NpcActorBinding,
    target: String,
    prompt: String,
}

// Only normal server display publication creates this slot. Cloneable UI
// strings or checkpoints alone cannot request a new input permission.
#[derive(Resource)]
struct DisplayedNpcMenu {
    binding: NpcActorBinding,
    display: ActiveNpcDialogState,
}

fn actor_binding(world: &World, npc_object_id: u32) -> Option<NpcActorBinding> {
    if !is_in_world(world) || current_player_is_dead(world) || npc_object_id == 0 {
        return None;
    }
    let player = player_entity(world)?;
    let session = world.get_resource::<SessionResource>()?;
    let account = active_session_mutating_account_id(session)?;
    let character = session.selected_character.as_ref()?;
    let npc = entity_by_object_id(world, npc_object_id)?;
    world.entity(npc).get::<Npc>()?;
    let script_key = world.entity(npc).get::<NpcAgent>()?.script_key.as_ref();
    let script = match script_key.filter(|key| !key.is_empty()) {
        Some(script_key) => {
            // A declared but unresolved script cannot become a generated menu.
            let script = crystal_npc_script_by_key(script_key)?;
            Some(NpcScriptBinding {
                script_key: script_key.clone(),
                relative_path: script.relative_path.clone(),
                // Imported representations, not disk bytes or world receipts.
                raw_text_sha256: Sha256::digest(script.raw_text.as_bytes()).into(),
                imported_script_sha256: Sha256::digest(serde_json::to_vec(&script).ok()?).into(),
            })
        }
        // The trusted periodic NPC catalog has no script. Ordinary spawn
        // stores None; the shared snapshot path stores its exact empty key.
        None => None,
    };
    let position = entity_position(world, npc)?;
    Some(NpcActorBinding {
        player,
        account,
        character_index: character.index,
        character_name: character.name.clone(),
        map_index: world
            .get_resource::<MapRuntimeResource>()?
            .current_map
            .map_index,
        map_file_name: world
            .get_resource::<MapRuntimeResource>()?
            .current_map
            .file_name
            .clone(),
        npc,
        npc_object_id,
        npc_x: position.x,
        npc_y: position.y,
        declared_script_key: script_key.cloned(),
        script,
    })
}

pub(super) fn invalidate(world: &mut World) {
    world.remove_resource::<PendingNpcInput>();
    world.remove_resource::<DisplayedNpcMenu>();
}

pub(super) fn record_display(world: &mut World, dialog: &ActiveNpcDialogState) {
    invalidate(world);
    if dialog.input.is_some()
        || dialog.links.is_empty()
        || !crystal_npc_object_in_data_range(world, dialog.npc_object_id)
    {
        return;
    }
    if let Some(binding) = actor_binding(world, dialog.npc_object_id) {
        world.insert_resource(DisplayedNpcMenu {
            binding,
            display: dialog.clone(),
        });
    }
}

fn same_display(left: &ActiveNpcDialogState, right: &ActiveNpcDialogState) -> bool {
    left.npc_object_id == right.npc_object_id
        && left.npc_name == right.npc_name
        && left.npc_name_key == right.npc_name_key
        && left.stage == right.stage
        && left.current == right.current
        && left.required == right.required
        && left.title == right.title
        && left.body == right.body
        && left.footer == right.footer
        && left.links == right.links
        && left.input == right.input
}

fn current_menu(world: &World) -> Option<&DisplayedNpcMenu> {
    let dialog = world
        .get_resource::<NpcStateResource>()
        .and_then(|state| state.active_npc_dialog.as_ref())?;
    let menu = world.get_resource::<DisplayedNpcMenu>()?;
    if !same_display(dialog, &menu.display)
        || !crystal_npc_object_in_data_range(world, dialog.npc_object_id)
        || actor_binding(world, dialog.npc_object_id).as_ref() != Some(&menu.binding)
    {
        return None;
    }
    Some(menu)
}

pub(super) fn has_menu_target(world: &World, target: &str) -> bool {
    current_menu(world).is_some_and(|menu| {
        menu.display
            .links
            .iter()
            .any(|link| crystal_npc_dialog_targets_match(&link.target, target))
    })
}

pub(super) fn permits_menu_target(world: &mut World, target: &str) -> bool {
    let Some(menu) = current_menu(world) else {
        invalidate(world);
        return false;
    };
    // Wrong targets preserve the actual displayed page for a valid retry.
    menu.display
        .links
        .iter()
        .any(|link| crystal_npc_dialog_targets_match(&link.target, target))
}

pub(super) fn request(world: &mut World, target: &str) -> bool {
    if !permits_menu_target(world, target) {
        return false;
    }
    if world
        .resource::<DisplayedNpcMenu>()
        .binding
        .script
        .is_none()
    {
        // Script input still needs a resolved canonical script, unlike a
        // generated ordinary menu. Do not invent a script from UI strings.
        return false;
    }
    let menu = world.remove_resource::<DisplayedNpcMenu>().unwrap();
    let link = menu
        .display
        .links
        .iter()
        .find(|link| crystal_npc_dialog_targets_match(&link.target, target))
        .unwrap();
    let prompt = link.text.clone();
    let target = link.target.clone();
    let pending = PendingNpcInput {
        binding: menu.binding,
        target: target.clone(),
        prompt: prompt.clone(),
    };
    let dialog = menu.display;
    set_dialog(
        world,
        ActiveNpcDialogState {
            npc_object_id: dialog.npc_object_id,
            npc_name: dialog.npc_name.clone(),
            npc_name_key: dialog.npc_name_key,
            stage: None,
            current: 0,
            required: 0,
            title: dialog.npc_name.clone(),
            body: vec![prompt.clone()],
            footer: dialog.npc_name,
            links: vec![],
            input: Some(NpcDialogInputState { target, prompt }),
        },
    );
    world.insert_resource(pending);
    true
}

pub(super) fn consume(world: &mut World, npc_object_id: u32, target: &str) -> bool {
    let Some(pending) = world.get_resource::<PendingNpcInput>() else {
        return false;
    };
    if pending.binding.npc_object_id != npc_object_id
        || !crystal_npc_dialog_targets_match(&pending.target, target)
    {
        // An unrelated bad packet must not consume the legitimate prompt.
        return false;
    }
    let dialog_matches = world
        .get_resource::<NpcStateResource>()
        .and_then(|state| state.active_npc_dialog.as_ref())
        .is_some_and(|dialog| {
            dialog.npc_object_id == npc_object_id
                && dialog.input.as_ref().is_some_and(|input| {
                    input.target == pending.target && input.prompt == pending.prompt
                })
        });
    let valid = dialog_matches
        && actor_binding(world, npc_object_id).as_ref() == Some(&pending.binding)
        && crystal_npc_object_in_data_range(world, npc_object_id);
    // Consume before any script mutation or fallible save. A checkpoint of
    // NpcStateResource cannot resurrect this non-Clone, non-serialized slot.
    invalidate(world);
    valid
}
