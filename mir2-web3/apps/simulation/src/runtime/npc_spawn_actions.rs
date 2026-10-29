//! Source declaration/ActList rules only. Valid MONGEN still uses the existing
//! personal compatibility executor; this is not a shared-world transaction.
use super::npc_script::{
    crystal_npc_mongen, resolve_crystal_npc_runtime_tokens, CrystalNpcActionControl,
    CrystalNpcExecutionState,
};
use bevy_ecs::prelude::World;

pub(super) fn execute(
    world: &mut World,
    line: &str,
    state: &mut CrystalNpcExecutionState,
) -> Option<CrystalNpcActionControl> {
    let raw: Vec<_> = line.split(' ').filter(|part| !part.is_empty()).collect();
    let command = raw.first()?.to_ascii_uppercase();
    if !matches!(command.as_str(), "PARAM1" | "PARAM2" | "PARAM3" | "MONGEN") {
        return None;
    }
    // NPCSegment.ParseAct omits the declaration before substitutions can add
    // text. Optional instance/count defaults apply only to absent arguments.
    if raw.len() < 2 {
        return Some(CrystalNpcActionControl::Continue);
    }
    let first = resolve_crystal_npc_runtime_tokens(world, raw[1], state);
    let number = |value: &str| resolve_crystal_npc_runtime_tokens(world, value, state);
    let returned = CrystalNpcActionControl::ReturnFromActList;
    match command.as_str() {
        "PARAM1" => {
            let Ok(instance) = number(raw.get(2).copied().unwrap_or("1"))
                .trim()
                .parse::<i32>()
            else {
                return Some(returned);
            };
            state.map_file_name = Some(first);
            state.map_instance = instance;
        }
        "PARAM2" | "PARAM3" => {
            let Ok(value) = first.trim().parse::<i32>() else {
                return Some(returned);
            };
            if command == "PARAM2" {
                state.x = value;
            } else {
                state.y = value;
            }
        }
        "MONGEN" => {
            if state.map_file_name.is_none() || state.x == 0 || state.y == 0 {
                return Some(returned);
            }
            let Ok(count) = number(raw.get(2).copied().unwrap_or("1"))
                .trim()
                .parse::<u8>()
            else {
                return Some(returned);
            };
            // Envir.GetMonsterInfo checks exact and space-free names together
            // in original list order. Substituted spaces are never new args.
            let compact = first.replace(' ', "");
            let Some(monster) = mir2_game_data::crystal_monster_manifest_ref()
                .monsters
                .iter()
                .find(|monster| {
                    monster.name.eq_ignore_ascii_case(&first)
                        || monster.name.replace(' ', "").eq_ignore_ascii_case(&compact)
                })
            else {
                return Some(returned);
            };
            // A valid zero still resolves the selector/template, then executes
            // zero births. Spawn(false) is not the Source factory-null return.
            // Target-map/instance authority, other factory overrides/hooks and
            // durable ordered application remain separate work.
            crystal_npc_mongen(world, &[monster.name.as_str(), &count.to_string()], state);
        }
        _ => unreachable!(),
    }
    Some(CrystalNpcActionControl::Continue)
}
