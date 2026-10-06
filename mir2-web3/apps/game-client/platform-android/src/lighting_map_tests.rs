use super::*;
use crate::map_objects::MapObjectEntry;
fn binary(width: u16, height: u16, lights: &[(u8, u8, i16, i16)]) -> Vec<u8> {
    let mut bytes = vec![0; 8 + usize::from(width) * usize::from(height) * 26];
    bytes[2] = 0x43;
    bytes[3] = 0x23;
    bytes[4..6].copy_from_slice(&width.to_le_bytes());
    bytes[6..8].copy_from_slice(&height.to_le_bytes());
    for (i, (light, animation, index, image)) in lights.iter().enumerate() {
        let at = 8 + i * 26;
        bytes[at + 10..at + 12].copy_from_slice(&index.to_le_bytes());
        bytes[at + 12..at + 14].copy_from_slice(&image.to_le_bytes());
        bytes[at + 16] = *animation;
        bytes[at + 25] = *light;
    }
    bytes
}
fn objects() -> MapObjectPack {
    MapObjectPack {
        entries: HashMap::from([(
            "WemadeMir2/Objects#0".into(),
            MapObjectEntry {
                image_key: "checked-key".into(),
                asset_path: "checked-source.png".into(),
                width: 96,
                height: 160,
                offset: Some((-50, -100)),
            },
        )]),
    }
}
fn viewport(x: i32, y: i32) -> Viewport {
    Viewport {
        center_x: x,
        center_y: y,
        width: 19,
        height: 15,
    }
}
#[test]
fn actual_type100_light_byte_and_x_major_order_reach_shared_source_hook() {
    let map = parse_type100_map(&binary(
        2,
        2,
        &[(1, 0, 2, 1), (2, 1, 2, 1), (0, 0, 2, 1), (9, 0, 2, 1)],
    ))
    .unwrap();
    let cells = map_light_cells(&map, viewport(0, 0), &objects());
    assert_eq!(
        cells
            .iter()
            .map(|cell| cell.key.as_str())
            .collect::<Vec<_>>(),
        ["0:0:1", "0:1:2", "1:1:9"]
    );
    assert_eq!((cells[0].offset_x, cells[0].offset_y), (0, 0));
    assert_eq!((cells[1].offset_x, cells[1].offset_y), (-50, -100));
    assert_eq!((cells[2].offset_x, cells[2].offset_y), (0, 0));
}
#[test]
fn absent_offset_metadata_keeps_original_explicit_zero_not_synthetic_position() {
    let map = parse_type100_map(&binary(1, 1, &[(1, 1, 2, 1)])).unwrap();
    let cells = map_light_cells(
        &map,
        viewport(0, 0),
        &MapObjectPack {
            entries: HashMap::new(),
        },
    );
    assert_eq!((cells[0].offset_x, cells[0].offset_y), (0, 0));
}
#[test]
fn original_nonrenderable_cell_classes_do_not_create_map_lights() {
    let map = parse_type100_map(&binary(
        1,
        5,
        &[
            (0, 0, 2, 1),
            (10, 0, 2, 1),
            (1, 0, -1, 1),
            (1, 0, 2, 0),
            (9, 0, 2, 1),
        ],
    ))
    .unwrap();
    let cells = map_light_cells(&map, viewport(0, 0), &objects());
    assert_eq!(cells.len(), 1);
    assert_eq!(cells[0].key, "0:4:9");
}
#[test]
fn viewport_bounded_sources_keep_edge_and_discard_outside_range() {
    let values = vec![(1, 0, 2, 1); 83];
    let map = parse_type100_map(&binary(1, 83, &values)).unwrap();
    let cells = map_light_cells(&map, viewport(0, 0), &objects());
    assert_eq!(cells.len(), 42);
    assert_eq!(cells.last().unwrap().y, 41);
}
#[test]
fn source_hook_handles_empty_malformed_map_without_dividing_by_zero() {
    let map = ParsedMap {
        width: 0,
        height: 0,
        cells: Vec::new(),
    };
    assert!(map_light_cells(&map, viewport(0, 0), &objects()).is_empty());
}
