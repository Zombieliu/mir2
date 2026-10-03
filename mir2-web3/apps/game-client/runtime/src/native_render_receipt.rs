//! Native render receipts for hosts that must not reveal gameplay until the
//! matching map and actor frame has actually committed in Bevy.

use bevy::prelude::Resource;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MapApply {
    request_id: u64,
    center_x: i32,
    center_y: i32,
    tile_count: usize,
    unresolved_draw_count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct EntityApply {
    request_id: u64,
    center_x: i32,
    center_y: i32,
    entity_count: usize,
    layer_count: usize,
    unresolved_entity_count: usize,
    self_visible: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeRenderReady {
    pub request_id: u64,
    pub center_x: i32,
    pub center_y: i32,
    pub map_tile_count: usize,
    pub entity_count: usize,
    pub entity_layer_count: usize,
}

#[derive(Resource, Default, Debug)]
pub struct NativeRenderReceipt {
    map: Option<MapApply>,
    entities: Option<EntityApply>,
    candidate: Option<(NativeRenderReady, u8)>,
    ready: Option<NativeRenderReady>,
}

impl NativeRenderReceipt {
    pub fn ready_for(&self, request_id: u64) -> Option<NativeRenderReady> {
        self.ready.filter(|ready| ready.request_id == request_id)
    }

    fn complete_frame(&self) -> Option<NativeRenderReady> {
        let map = self.map?;
        let entities = self.entities?;
        (map.request_id != 0
            && map.request_id == entities.request_id
            && (map.center_x, map.center_y) == (entities.center_x, entities.center_y)
            && map.tile_count > 0
            && map.unresolved_draw_count == 0
            && entities.entity_count > 0
            && entities.layer_count > 0
            && entities.unresolved_entity_count == 0
            && entities.self_visible)
            .then_some(NativeRenderReady {
                request_id: map.request_id,
                center_x: map.center_x,
                center_y: map.center_y,
                map_tile_count: map.tile_count,
                entity_count: entities.entity_count,
                entity_layer_count: entities.layer_count,
            })
    }

    /// Promote a complete map/entity frame only after the same render state was
    /// live for a previous update. The first complete update creates/spawns the
    /// render objects; requiring a second consecutive frame means hosts cannot
    /// leave their transition surface before those objects have had a render
    /// opportunity on the native swapchain.
    pub(crate) fn finish_frame(&mut self) {
        let Some(current) = self.complete_frame() else {
            self.candidate = None;
            self.ready = None;
            return;
        };
        let stable_frames = match self.candidate {
            Some((candidate, frames)) if candidate == current => frames.saturating_add(1),
            _ => 1,
        };
        self.candidate = Some((current, stable_frames));
        self.ready = (stable_frames >= 2).then_some(current);
    }

    pub(crate) fn mark_map(
        &mut self,
        request_id: u64,
        center: Option<(i32, i32)>,
        tile_count: usize,
        unresolved_draw_count: usize,
    ) {
        self.map = request_id
            .checked_sub(1)
            .and_then(|_| center)
            .map(|(center_x, center_y)| MapApply {
                request_id,
                center_x,
                center_y,
                tile_count,
                unresolved_draw_count,
            });
    }

    pub(crate) fn mark_entities(
        &mut self,
        request_id: u64,
        center: Option<(i32, i32)>,
        entity_count: usize,
        layer_count: usize,
        unresolved_entity_count: usize,
        self_visible: bool,
    ) {
        self.entities =
            request_id
                .checked_sub(1)
                .and_then(|_| center)
                .map(|(center_x, center_y)| EntityApply {
                    request_id,
                    center_x,
                    center_y,
                    entity_count,
                    layer_count,
                    unresolved_entity_count,
                    self_visible,
                });
    }

    pub(crate) fn clear_map(&mut self) {
        self.map = None;
    }

    pub(crate) fn clear_entities(&mut self) {
        self.entities = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ready_requires_exact_complete_same_center_frame() {
        let mut receipt = NativeRenderReceipt::default();
        receipt.mark_map(7, Some((302, 634)), 600, 0);
        receipt.mark_entities(7, Some((302, 634)), 2, 3, 0, true);
        receipt.finish_frame();
        assert_eq!(receipt.ready_for(6), None);
        assert_eq!(receipt.ready_for(7), None);
        receipt.finish_frame();
        assert_eq!(receipt.ready_for(7).unwrap().entity_layer_count, 3);

        receipt.mark_map(7, Some((303, 634)), 600, 0);
        receipt.finish_frame();
        assert_eq!(receipt.ready_for(7), None);
        receipt.mark_map(7, Some((302, 634)), 600, 1);
        receipt.finish_frame();
        assert_eq!(receipt.ready_for(7), None);
        receipt.mark_map(7, Some((302, 634)), 600, 0);
        receipt.mark_entities(7, Some((302, 634)), 2, 3, 1, true);
        receipt.finish_frame();
        assert_eq!(receipt.ready_for(7), None);
        receipt.mark_entities(7, Some((302, 634)), 2, 3, 0, false);
        receipt.finish_frame();
        assert_eq!(receipt.ready_for(7), None);
    }

    #[test]
    fn changing_request_restarts_the_two_frame_presentation_gate() {
        let mut receipt = NativeRenderReceipt::default();
        for _ in 0..2 {
            receipt.mark_map(7, Some((302, 634)), 600, 0);
            receipt.mark_entities(7, Some((302, 634)), 2, 3, 0, true);
            receipt.finish_frame();
        }
        assert!(receipt.ready_for(7).is_some());
        receipt.mark_map(8, Some((303, 634)), 600, 0);
        receipt.mark_entities(8, Some((303, 634)), 2, 3, 0, true);
        receipt.finish_frame();
        assert_eq!(receipt.ready_for(7), None);
        assert_eq!(receipt.ready_for(8), None);
        receipt.finish_frame();
        assert!(receipt.ready_for(8).is_some());
    }
}
