use mir2_game_data::crystal_map_events::crystal_map_event_manifest;

#[test]
fn crystal_map_coordinate_bindings_and_nested_event_include_are_imported() {
    let manifest = crystal_map_event_manifest();

    assert_eq!(manifest.schema_version, 1);
    assert_eq!(manifest.map_coordinates.len(), 6);
    assert_eq!(manifest.events.len(), 18);
    assert!(manifest.diagnostics.dangling_paths.is_empty());
    assert!(manifest.diagnostics.path_traversal_rejected.is_empty());
    assert!(manifest.diagnostics.cycles.is_empty());
    assert!(manifest
        .references
        .iter()
        .all(|reference| reference.resolved));

    let penal = manifest
        .map_coordinates
        .iter()
        .find(|binding| binding.map_id == "3")
        .expect("Penal map-coordinate binding");
    assert_eq!((penal.x, penal.y), (861, 686));
    assert_eq!(penal.event_id, "@Main");
    assert_eq!(
        penal.include.target_file,
        "SystemScripts/00Default/MapCoords/PenalCavern.txt"
    );
    assert_eq!(penal.resolved_section.source_line, 1);
    assert!(penal
        .resolved_section
        .lines
        .iter()
        .any(|line| line.text.contains("ENTERMAP")));

    let event_41 = manifest
        .events
        .iter()
        .find(|file| file.source_file == "Events/gumi203-Event-41.txt")
        .expect("Event41 parent file");
    let reward = event_41
        .sections
        .iter()
        .find(|section| section.name.eq_ignore_ascii_case("@_EventReward"))
        .expect("Event41 reward section");
    assert!(reward
        .lines
        .iter()
        .any(|line| line.text.contains("GIVEITEM FortuneWeapon")));
    let nested_line = reward
        .lines
        .iter()
        .find(|line| line.text.contains("GIVEITEM FortuneWeapon"))
        .expect("nested Event41 line");
    assert_eq!(nested_line.source_file, "Events/Event/Event41.txt");
    assert_eq!(nested_line.source_line, 6);
    assert!(!nested_line.include_chain.is_empty());

    let nested_file = manifest
        .events
        .iter()
        .find(|file| file.source_file == "Events/Event/Event41.txt")
        .expect("Event41 nested include file");
    assert!(nested_file
        .sections
        .iter()
        .any(|section| section.name.eq_ignore_ascii_case("@Main")));
}
