//! Renderer-neutral state for Crystal's Big Map dialog.
//!
//! This module deliberately stops at the authoritative client contract.  It
//! stores the current `MapInformation` identity plus `NewMapInfo`,
//! `WorldMapSetup`, and `SearchMapResult` data, exposes the same 18-row NPC
//! window as Crystal, and produces metadata for a server-gated teleport
//! intent. It never changes the player's map or reports teleport success; a
//! gateway/renderer can consume the intent later.

use std::collections::{BTreeMap, VecDeque};

use bevy::prelude::Resource;
use serde::{Deserialize, Deserializer, Serialize};

pub const BIG_MAP_NPC_ROW_COUNT: usize = 18;
pub const MAX_MAP_MOVEMENTS: usize = 128;
pub const MAX_MAP_NPCS: usize = 128;
pub const MAX_WORLD_MAP_ICONS: usize = 256;
pub const MAX_CACHED_MAPS: usize = 128;
pub const MAX_BIG_MAP_GATEWAY_INTENTS: usize = 16;
pub const MIN_SEARCH_QUERY_CHARS: usize = 3;
pub const MAX_SEARCH_QUERY_CHARS: usize = 64;
pub const MAX_DISPLAY_TEXT_CHARS: usize = 256;
pub const WORLD_MAP_IMAGE_URL: &str = "original-ui/Prguse2/1360.png";

pub fn big_map_image_url(image_index: u32) -> String {
    format!("original-ui/MMap/{image_index}.png")
}

/// The protocol's `Point`, kept independent of the protocol crate so this
/// model remains usable by Windows, Web, and Android renderers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BigMapPoint {
    pub x: i32,
    pub y: i32,
}

/// One `ClientMovementInfo` from `ClientMapInfo.movements`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BigMapMovement {
    #[serde(default)]
    pub destination: i32,
    #[serde(default, deserialize_with = "deserialize_bounded_text")]
    pub title: String,
    #[serde(default)]
    pub location: BigMapPoint,
    #[serde(default)]
    pub icon: i32,
}

/// One `ClientNpcInfo` from `ClientMapInfo.npcs`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BigMapNpc {
    #[serde(default)]
    pub index: i32,
    #[serde(default, deserialize_with = "deserialize_bounded_text")]
    pub file_name: String,
    #[serde(default, deserialize_with = "deserialize_bounded_text")]
    pub name: String,
    #[serde(default)]
    pub map_index: i32,
    #[serde(default)]
    pub location: BigMapPoint,
    #[serde(default)]
    pub image: u16,
    #[serde(default)]
    pub rate: u16,
    #[serde(default)]
    pub show_on_big_map: bool,
    #[serde(default)]
    pub big_map_icon: i32,
    /// Zero is not a valid teleport target and is rejected by the intent
    /// builder below.
    #[serde(default)]
    pub object_id: u32,
    #[serde(default)]
    pub icon: i32,
    #[serde(default)]
    pub can_teleport_to: bool,
}

/// The typed payload carried by a protocol `NewMapInfo` packet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BigMapInfo {
    #[serde(default, deserialize_with = "deserialize_bounded_text")]
    pub title: String,
    #[serde(default)]
    pub width: i32,
    #[serde(default)]
    pub height: i32,
    /// Crystal uses zero when no Big Map image is available.  Keep the raw
    /// protocol value; `big_map_image_index` exposes the renderer-friendly
    /// optional form.
    #[serde(default)]
    pub big_map: i32,
    #[serde(default, deserialize_with = "deserialize_bounded_movements")]
    pub movements: Vec<BigMapMovement>,
    #[serde(default, deserialize_with = "deserialize_bounded_npcs")]
    pub npcs: Vec<BigMapNpc>,
}

impl BigMapInfo {
    pub fn big_map_image_index(&self) -> Option<u32> {
        u32::try_from(self.big_map).ok().filter(|index| *index > 0)
    }

    fn bounded(mut self) -> Self {
        self.title = truncate_text(self.title);
        self.width = self.width.max(0);
        self.height = self.height.max(0);
        self.movements.truncate(MAX_MAP_MOVEMENTS);
        self.npcs.truncate(MAX_MAP_NPCS);
        for movement in &mut self.movements {
            movement.title = truncate_text(std::mem::take(&mut movement.title));
        }
        for npc in &mut self.npcs {
            npc.file_name = truncate_text(std::mem::take(&mut npc.file_name));
            npc.name = truncate_text(std::mem::take(&mut npc.name));
        }
        self
    }
}

/// A cached map definition, with the `mapIndex` supplied by `NewMapInfo`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BigMapEntry {
    pub map_index: i32,
    #[serde(flatten)]
    pub info: BigMapInfo,
}

impl BigMapEntry {
    fn bounded(mut self) -> Self {
        self.info = self.info.bounded();
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BigMapWorldIcon {
    #[serde(default)]
    pub image_index: i32,
    #[serde(default, deserialize_with = "deserialize_bounded_text")]
    pub title: String,
    #[serde(default)]
    pub map_index: i32,
}

impl BigMapWorldIcon {
    fn bounded(mut self) -> Self {
        self.title = truncate_text(self.title);
        self
    }
}

/// Authoritative world-map setup.  `teleport_to_npc_cost` is only metadata;
/// the server remains the authority for whether a request succeeds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BigMapWorldState {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default, deserialize_with = "deserialize_bounded_world_icons")]
    pub icons: Vec<BigMapWorldIcon>,
    #[serde(default)]
    pub teleport_to_npc_cost: i32,
}

impl Default for BigMapWorldState {
    fn default() -> Self {
        Self {
            enabled: false,
            icons: Vec::new(),
            teleport_to_npc_cost: 0,
        }
    }
}

impl BigMapWorldState {
    fn bounded(mut self) -> Self {
        self.icons.truncate(MAX_WORLD_MAP_ICONS);
        self.teleport_to_npc_cost = self.teleport_to_npc_cost.max(0);
        for icon in &mut self.icons {
            *icon = icon.clone().bounded();
        }
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BigMapView {
    CurrentMap,
    WorldMap,
}

impl Default for BigMapView {
    fn default() -> Self {
        Self::CurrentMap
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BigMapSearchResult {
    Map { map_index: i32 },
    Npc { map_index: i32, object_id: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BigMapSearchState {
    #[serde(default, deserialize_with = "deserialize_bounded_query")]
    pub draft: String,
    #[serde(default)]
    pub page: usize,
    #[serde(default)]
    pub cooldown_until_ms: Option<u64>,
    #[serde(default, deserialize_with = "deserialize_optional_bounded_query")]
    pub last_submitted: Option<String>,
    #[serde(default)]
    pub last_result: Option<BigMapSearchResult>,
}

impl Default for BigMapSearchState {
    fn default() -> Self {
        Self {
            draft: String::new(),
            page: 0,
            cooldown_until_ms: None,
            last_submitted: None,
            last_result: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BigMapSearchError {
    TooShort,
    TooLong,
    Cooldown { remaining_ms: u64 },
    QueueFull,
}

/// Metadata for a `TeleportToNpc` intent.  This is deliberately not an
/// acknowledgement and contains no destination transform or success flag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BigMapTeleportIntent {
    pub object_id: u32,
    pub npc_map_index: i32,
    pub source_map_index: Option<i32>,
    pub teleport_to_npc_cost: i32,
    pub requires_server_authorization: bool,
}

/// Renderer-facing projection of the authoritative Big Map model. Every map
/// image index, coordinate and NPC row in this value originates in a received
/// MapInformation/NewMapInfo/WorldMapSetup packet; it contains no map-name
/// lookup or client-authored NPC fallback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BigMapRenderSnapshot {
    pub map_index: Option<i32>,
    pub title: Option<String>,
    pub map_image_url: Option<String>,
    pub width: i32,
    pub height: i32,
    pub player_location: Option<BigMapPoint>,
    pub npcs: Vec<BigMapNpc>,
}

/// Shared state consumed by a renderer and later adapted by a gateway.
#[derive(Debug, Clone, PartialEq, Eq, Resource, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BigMapModel {
    /// Advances at every authoritative map or session boundary. Renderer
    /// requests bind to this value so an old NPC object id cannot cross into a
    /// different map/session.
    #[serde(default)]
    pub reset_epoch: u64,
    #[serde(default, deserialize_with = "deserialize_bounded_map_entries")]
    pub maps: BTreeMap<i32, BigMapEntry>,
    #[serde(default)]
    pub current_map_index: Option<i32>,
    #[serde(default)]
    pub active_map_index: Option<i32>,
    #[serde(default)]
    pub world: BigMapWorldState,
    #[serde(default)]
    pub view: BigMapView,
    #[serde(default)]
    pub player_location: Option<BigMapPoint>,
    #[serde(default)]
    pub search: BigMapSearchState,
    #[serde(default)]
    pub npc_scroll_row: usize,
    #[serde(default)]
    pub selected_npc_object_id: Option<u32>,
}

impl Default for BigMapModel {
    fn default() -> Self {
        Self {
            reset_epoch: 0,
            maps: BTreeMap::new(),
            current_map_index: None,
            active_map_index: None,
            world: BigMapWorldState::default(),
            view: BigMapView::CurrentMap,
            player_location: None,
            search: BigMapSearchState::default(),
            npc_scroll_row: 0,
            selected_npc_object_id: None,
        }
    }
}

impl BigMapModel {
    /// Drop all account/character-scoped Big Map data.  This is called on a
    /// true session boundary; retaining NPC object ids across accounts would
    /// make a later teleport request ambiguous and unsafe.
    pub fn reset_for_session(&mut self) {
        let reset_epoch = self.reset_epoch.saturating_add(1);
        *self = Self {
            reset_epoch,
            ..Self::default()
        };
    }

    /// Start a new map scene without carrying map-local definitions, search
    /// selection, or NPC object ids over from the prior map.  WorldMapSetup
    /// is connection-scoped and remains valid until an authoritative setup
    /// packet replaces it or the session ends.
    pub fn reset_for_map(&mut self, map_index: i32, location: Option<BigMapPoint>) {
        let world = self.world.clone();
        let reset_epoch = self.reset_epoch.saturating_add(1);
        *self = Self {
            reset_epoch,
            world,
            current_map_index: Some(map_index),
            active_map_index: Some(map_index),
            player_location: location,
            ..Self::default()
        };
    }

    /// Apply the typed payload from `NewMapInfo`.  This updates cached map
    /// data, but does not claim that the player moved to that map.
    pub fn apply_new_map_info(&mut self, map_index: i32, info: BigMapInfo) {
        if !self.maps.contains_key(&map_index) && self.maps.len() >= MAX_CACHED_MAPS {
            if let Some(evict) = self
                .maps
                .keys()
                .copied()
                .find(|candidate| Some(*candidate) != self.current_map_index)
            {
                self.maps.remove(&evict);
            }
        }
        self.maps.insert(
            map_index,
            BigMapEntry {
                map_index,
                info: info.bounded(),
            },
        );
        if self.active_map_index.is_none() {
            self.active_map_index = Some(self.current_map_index.unwrap_or(map_index));
        }
        if self.current_map_index == Some(map_index) {
            self.active_map_index = Some(map_index);
            self.clamp_selection_and_scroll();
        }
    }

    /// Reconcile the authoritative current map from `MapInformation` or a
    /// world snapshot.  It intentionally does not fabricate a map definition.
    pub fn set_current_map(&mut self, map_index: i32) {
        let changed = self.current_map_index != Some(map_index);
        self.current_map_index = Some(map_index);
        self.active_map_index = Some(map_index);
        self.view = BigMapView::CurrentMap;
        if changed {
            self.reset_epoch = self.reset_epoch.saturating_add(1);
            self.npc_scroll_row = 0;
            self.selected_npc_object_id = None;
            self.search.page = 0;
            self.search.last_result = None;
        }
        self.clamp_selection_and_scroll();
    }

    pub fn apply_world_map_setup(
        &mut self,
        enabled: bool,
        icons: Vec<BigMapWorldIcon>,
        teleport_to_npc_cost: i32,
    ) {
        self.world = BigMapWorldState {
            enabled,
            icons,
            teleport_to_npc_cost,
        }
        .bounded();
        if !enabled {
            self.view = BigMapView::CurrentMap;
        }
    }

    pub fn set_view(&mut self, view: BigMapView) {
        self.view = if view == BigMapView::WorldMap && self.world.enabled {
            view
        } else {
            BigMapView::CurrentMap
        };
    }

    pub fn set_player_location(&mut self, map_index: Option<i32>, location: BigMapPoint) {
        if let Some(map_index) = map_index {
            self.set_current_map(map_index);
        }
        self.player_location = Some(location);
    }

    pub fn active_map(&self) -> Option<&BigMapEntry> {
        self.active_map_index
            .and_then(|index| self.maps.get(&index))
    }

    pub fn current_map(&self) -> Option<&BigMapEntry> {
        self.current_map_index
            .and_then(|index| self.maps.get(&index))
    }

    pub fn missing_current_map_index(&self) -> Option<i32> {
        self.current_map_index
            .filter(|map_index| !self.maps.contains_key(map_index))
    }

    /// Build the exact read model consumed by the native Big Map ECS. The
    /// current-map image comes only from ClientMapInfo.bigMap and the NPC rows
    /// come only from ClientMapInfo.npcs. World-map art is enabled only by the
    /// authoritative WorldMapSetup packet.
    pub fn render_snapshot(&self) -> BigMapRenderSnapshot {
        if self.view == BigMapView::WorldMap && self.world.enabled {
            return BigMapRenderSnapshot {
                map_index: None,
                title: None,
                map_image_url: Some(WORLD_MAP_IMAGE_URL.to_owned()),
                width: 0,
                height: 0,
                player_location: None,
                npcs: Vec::new(),
            };
        }

        let Some(entry) = self.active_map() else {
            return BigMapRenderSnapshot {
                map_index: self.active_map_index,
                title: None,
                map_image_url: None,
                width: 0,
                height: 0,
                player_location: None,
                npcs: Vec::new(),
            };
        };
        BigMapRenderSnapshot {
            map_index: Some(entry.map_index),
            title: (!entry.info.title.trim().is_empty()).then(|| entry.info.title.clone()),
            map_image_url: entry.info.big_map_image_index().map(big_map_image_url),
            width: entry.info.width,
            height: entry.info.height,
            player_location: (Some(entry.map_index) == self.current_map_index)
                .then_some(self.player_location)
                .flatten(),
            npcs: self.visible_npcs().into_iter().cloned().collect(),
        }
    }

    pub fn set_search_draft(&mut self, draft: impl Into<String>) {
        self.search.draft = truncate_query(draft.into());
        self.search.page = 0;
        self.npc_scroll_row = 0;
        self.selected_npc_object_id = None;
    }

    /// Validate and mark a search request.  The returned string is the only
    /// client-side request value; the eventual result must come from the
    /// server's `SearchMapResult` packet.
    pub fn submit_search(
        &mut self,
        now_ms: u64,
        cooldown_ms: u64,
    ) -> Result<String, BigMapSearchError> {
        if let Some(until) = self.search.cooldown_until_ms {
            if until > now_ms {
                return Err(BigMapSearchError::Cooldown {
                    remaining_ms: until - now_ms,
                });
            }
        }
        let query = normalize_query(&self.search.draft)?;
        self.search.last_submitted = Some(query.clone());
        self.search.cooldown_until_ms = Some(now_ms.saturating_add(cooldown_ms));
        self.search.last_result = None;
        self.search.page = 0;
        self.npc_scroll_row = 0;
        Ok(query)
    }

    pub fn search_cooldown_remaining_ms(&self, now_ms: u64) -> u64 {
        self.search
            .cooldown_until_ms
            .unwrap_or_default()
            .saturating_sub(now_ms)
    }

    /// Apply the authoritative `SearchMapResult`.  A zero NPC index is the
    /// Crystal map-hit encoding; a non-zero value is the NPC object id.
    pub fn apply_search_result(&mut self, map_index: i32, npc_index: u32) {
        self.search.last_result = Some(if npc_index == 0 {
            BigMapSearchResult::Map { map_index }
        } else {
            BigMapSearchResult::Npc {
                map_index,
                object_id: npc_index,
            }
        });
        if self.maps.contains_key(&map_index) {
            self.active_map_index = Some(map_index);
        }
        if npc_index != 0 && self.select_npc(npc_index) {
            self.npc_scroll_row = self
                .filtered_npc_index(npc_index)
                .unwrap_or_default()
                .min(self.max_scroll_row());
            self.search.page = self.npc_scroll_row / BIG_MAP_NPC_ROW_COUNT;
        }
    }

    pub fn select_npc(&mut self, object_id: u32) -> bool {
        let valid = self
            .active_map()
            .map(|map| {
                map.info
                    .npcs
                    .iter()
                    .any(|npc| npc.object_id == object_id && npc.show_on_big_map)
            })
            .unwrap_or(false);
        self.selected_npc_object_id = valid.then_some(object_id);
        valid
    }

    pub fn selected_npc(&self) -> Option<&BigMapNpc> {
        let selected = self.selected_npc_object_id?;
        self.active_map()?
            .info
            .npcs
            .iter()
            .find(|npc| npc.object_id == selected && npc.show_on_big_map)
    }

    pub fn filtered_npcs(&self) -> Vec<&BigMapNpc> {
        let query = normalize_filter_query(&self.search.draft);
        self.active_map()
            .into_iter()
            .flat_map(|map| map.info.npcs.iter())
            .filter(|npc| npc.show_on_big_map)
            .filter(|npc| query.is_empty() || normalize_filter_query(&npc.name).contains(&query))
            .collect()
    }

    pub fn visible_npcs(&self) -> Vec<&BigMapNpc> {
        let filtered = self.filtered_npcs();
        filtered
            .into_iter()
            .skip(self.npc_scroll_row)
            .take(BIG_MAP_NPC_ROW_COUNT)
            .collect()
    }

    pub fn npc_page_count(&self) -> usize {
        self.filtered_npcs()
            .len()
            .div_ceil(BIG_MAP_NPC_ROW_COUNT)
            .max(1)
    }

    pub fn set_npc_page(&mut self, page: usize) {
        self.search.page = page.min(self.npc_page_count().saturating_sub(1));
        self.npc_scroll_row = (self.search.page * BIG_MAP_NPC_ROW_COUNT).min(self.max_scroll_row());
    }

    pub fn set_npc_scroll_row(&mut self, row: usize) {
        self.npc_scroll_row = row.min(self.max_scroll_row());
        self.search.page = self.npc_scroll_row / BIG_MAP_NPC_ROW_COUNT;
    }

    /// Build metadata for a server-gated teleport request.  No local map
    /// change occurs and no success is implied by this method.
    pub fn selected_teleport_intent(&self) -> Option<BigMapTeleportIntent> {
        let npc = self.selected_npc()?;
        if !self.world.enabled || !npc.can_teleport_to || npc.object_id == 0 {
            return None;
        }
        Some(BigMapTeleportIntent {
            object_id: npc.object_id,
            npc_map_index: npc.map_index,
            source_map_index: self.current_map_index,
            teleport_to_npc_cost: self.world.teleport_to_npc_cost.max(0),
            requires_server_authorization: true,
        })
    }

    fn filtered_npc_index(&self, object_id: u32) -> Option<usize> {
        self.filtered_npcs()
            .iter()
            .position(|npc| npc.object_id == object_id)
    }

    fn max_scroll_row(&self) -> usize {
        self.filtered_npcs()
            .len()
            .saturating_sub(BIG_MAP_NPC_ROW_COUNT)
    }

    fn clamp_selection_and_scroll(&mut self) {
        let max_row = self.max_scroll_row();
        self.npc_scroll_row = self.npc_scroll_row.min(max_row);
        if self
            .selected_npc_object_id
            .is_some_and(|_| self.selected_npc().is_none())
        {
            self.selected_npc_object_id = None;
        }
        self.search.page = self.npc_scroll_row / BIG_MAP_NPC_ROW_COUNT;
    }
}

/// One renderer-originated Big Map request.  These are requests only: no
/// variant encodes a local movement, map change, or transport success.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BigMapGatewayIntent {
    RequestMapInfo { map_index: i32 },
    SearchMap { text: String },
    TeleportToNpc { object_id: u32 },
}

/// Bounded native intent queue owned by a future Big Map renderer.  Keeping it
/// separate from the player-panel queue lets the renderer ask the Windows
/// gateway for data without depending on Crystal UI implementation details.
#[derive(Debug, Default, Resource)]
pub struct BigMapGatewayIntentQueue {
    /// Queue contents are valid only for this authoritative model epoch.
    reset_epoch: Option<u64>,
    intents: VecDeque<BigMapGatewayIntent>,
}

impl BigMapGatewayIntentQueue {
    /// Discard requests from an earlier map/session. The Windows bridge calls
    /// this immediately before wire forwarding; renderers call it while
    /// enqueueing so the queue is safe even when the UI opens during a reset.
    pub fn sync_model(&mut self, model: &BigMapModel) {
        if self.reset_epoch != Some(model.reset_epoch) {
            self.intents.clear();
            self.reset_epoch = Some(model.reset_epoch);
        }
    }

    pub fn request_map_info(&mut self, model: &BigMapModel, map_index: i32) -> bool {
        self.sync_model(model);
        if map_index <= 0 {
            return false;
        }
        self.push(BigMapGatewayIntent::RequestMapInfo { map_index })
    }

    /// Validate the renderer draft and enqueue the normalized query.  The
    /// actual result remains exclusively packet-driven.
    pub fn search(
        &mut self,
        model: &mut BigMapModel,
        now_ms: u64,
        cooldown_ms: u64,
    ) -> Result<(), BigMapSearchError> {
        self.sync_model(model);
        let text = model.submit_search(now_ms, cooldown_ms)?;
        if self.push(BigMapGatewayIntent::SearchMap { text }) {
            Ok(())
        } else {
            // The request was not accepted by the bounded local transport
            // queue, so do not leave a client-side cooldown pretending it was
            // sent. The server remains the only result authority.
            model.search.last_submitted = None;
            model.search.cooldown_until_ms = None;
            Err(BigMapSearchError::QueueFull)
        }
    }

    /// Queue only an intent derived from a visible, authoritative NPC record.
    /// No destination or local teleport result crosses this boundary.
    pub fn teleport_selected(&mut self, model: &BigMapModel) -> bool {
        self.sync_model(model);
        let Some(intent) = model.selected_teleport_intent() else {
            return false;
        };
        self.push(BigMapGatewayIntent::TeleportToNpc {
            object_id: intent.object_id,
        })
    }

    pub fn drain_intents(&mut self) -> Vec<BigMapGatewayIntent> {
        self.intents.drain(..).collect()
    }

    pub fn len(&self) -> usize {
        self.intents.len()
    }

    fn push(&mut self, intent: BigMapGatewayIntent) -> bool {
        // A duplicate request already waiting for the gateway carries no new
        // authority. Coalescing it protects the bounded command lane without
        // changing the server's semantics.
        if self.intents.iter().any(|pending| pending == &intent) {
            return true;
        }
        if self.intents.len() >= MAX_BIG_MAP_GATEWAY_INTENTS {
            return false;
        }
        self.intents.push_back(intent);
        true
    }
}

fn normalize_query(text: &str) -> Result<String, BigMapSearchError> {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let length = normalized.chars().count();
    if length < MIN_SEARCH_QUERY_CHARS {
        return Err(BigMapSearchError::TooShort);
    }
    if length > MAX_SEARCH_QUERY_CHARS {
        return Err(BigMapSearchError::TooLong);
    }
    Ok(normalized.to_lowercase())
}

fn normalize_filter_query(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn truncate_query(text: String) -> String {
    text.chars().take(MAX_SEARCH_QUERY_CHARS).collect()
}

fn truncate_text(text: String) -> String {
    text.chars().take(MAX_DISPLAY_TEXT_CHARS).collect()
}

fn deserialize_bounded_text<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(truncate_text(String::deserialize(deserializer)?))
}

fn deserialize_bounded_query<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(truncate_query(String::deserialize(deserializer)?))
}

fn deserialize_optional_bounded_query<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Option::<String>::deserialize(deserializer)?.map(truncate_query))
}

fn deserialize_bounded_movements<'de, D>(deserializer: D) -> Result<Vec<BigMapMovement>, D::Error>
where
    D: Deserializer<'de>,
{
    let mut values = Vec::<BigMapMovement>::deserialize(deserializer)?;
    values.truncate(MAX_MAP_MOVEMENTS);
    Ok(values)
}

fn deserialize_bounded_npcs<'de, D>(deserializer: D) -> Result<Vec<BigMapNpc>, D::Error>
where
    D: Deserializer<'de>,
{
    let mut values = Vec::<BigMapNpc>::deserialize(deserializer)?;
    values.truncate(MAX_MAP_NPCS);
    Ok(values)
}

fn deserialize_bounded_world_icons<'de, D>(
    deserializer: D,
) -> Result<Vec<BigMapWorldIcon>, D::Error>
where
    D: Deserializer<'de>,
{
    let mut values = Vec::<BigMapWorldIcon>::deserialize(deserializer)?;
    values.truncate(MAX_WORLD_MAP_ICONS);
    Ok(values)
}

fn deserialize_bounded_map_entries<'de, D>(
    deserializer: D,
) -> Result<BTreeMap<i32, BigMapEntry>, D::Error>
where
    D: Deserializer<'de>,
{
    let values = BTreeMap::<i32, BigMapEntry>::deserialize(deserializer)?;
    Ok(values
        .into_iter()
        .take(MAX_CACHED_MAPS)
        .map(|(index, entry)| (index, entry.bounded()))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn npc(object_id: u32, name: &str, can_teleport_to: bool) -> BigMapNpc {
        BigMapNpc {
            index: object_id as i32,
            file_name: "NPC/00".into(),
            name: name.into(),
            map_index: 1,
            location: BigMapPoint { x: 10, y: 20 },
            image: 1,
            rate: 1,
            show_on_big_map: true,
            big_map_icon: 1,
            object_id,
            icon: 7,
            can_teleport_to,
        }
    }

    fn info(npcs: Vec<BigMapNpc>) -> BigMapInfo {
        BigMapInfo {
            title: "BichonProvince".into(),
            width: 700,
            height: 700,
            big_map: 101,
            movements: vec![BigMapMovement {
                destination: 34,
                title: "NaturalCave".into(),
                location: BigMapPoint { x: 8, y: 9 },
                icon: 2,
            }],
            npcs,
        }
    }

    #[test]
    fn serde_is_backward_compatible_with_missing_optional_fields() {
        let model: BigMapModel = serde_json::from_str(r#"{"maps":{}}"#).unwrap();
        assert_eq!(model, BigMapModel::default());

        let info: BigMapInfo =
            serde_json::from_str(r#"{"title":"Old","width":32,"height":32,"bigMap":0}"#).unwrap();
        assert!(info.movements.is_empty());
        assert!(info.npcs.is_empty());
        assert_eq!(info.big_map_image_index(), None);
    }

    #[test]
    fn oversized_payloads_are_bounded_and_text_is_truncated() {
        let movements = (0..(MAX_MAP_MOVEMENTS + 20))
            .map(|_| BigMapMovement {
                destination: 1,
                title: "x".into(),
                location: BigMapPoint::default(),
                icon: 1,
            })
            .collect();
        let npcs = (1..=(MAX_MAP_NPCS as u32 + 20))
            .map(|id| npc(id, &"n".repeat(MAX_DISPLAY_TEXT_CHARS + 20), false))
            .collect();
        let mut model = BigMapModel::default();
        model.apply_new_map_info(
            1,
            BigMapInfo {
                title: "t".repeat(MAX_DISPLAY_TEXT_CHARS + 20),
                width: -4,
                height: -9,
                big_map: 101,
                movements,
                npcs,
            },
        );
        let map = model.maps.get(&1).unwrap();
        assert_eq!(map.info.movements.len(), MAX_MAP_MOVEMENTS);
        assert_eq!(map.info.npcs.len(), MAX_MAP_NPCS);
        assert_eq!(map.info.title.chars().count(), MAX_DISPLAY_TEXT_CHARS);
        assert_eq!(map.info.width, 0);
        assert_eq!(map.info.height, 0);
        assert_eq!(
            map.info.npcs[0].name.chars().count(),
            MAX_DISPLAY_TEXT_CHARS
        );
    }

    #[test]
    fn search_normalizes_draft_and_enforces_cooldown() {
        let mut model = BigMapModel::default();
        model.set_search_draft("  Natural   Cave  ");
        assert_eq!(model.submit_search(100, 500).unwrap(), "natural cave");
        assert_eq!(model.search_cooldown_remaining_ms(200), 400);
        assert_eq!(
            model.submit_search(200, 500),
            Err(BigMapSearchError::Cooldown { remaining_ms: 400 })
        );
        assert_eq!(model.submit_search(600, 500).unwrap(), "natural cave");
    }

    #[test]
    fn npc_selection_and_eighteen_row_paging_are_stable() {
        let npcs = (1..=40)
            .map(|id| npc(id, &format!("NPC {id}"), id == 20))
            .collect();
        let mut model = BigMapModel::default();
        model.apply_new_map_info(1, info(npcs));
        model.set_current_map(1);
        assert_eq!(model.visible_npcs().len(), BIG_MAP_NPC_ROW_COUNT);
        assert_eq!(model.npc_page_count(), 3);
        model.set_npc_page(1);
        assert_eq!(model.visible_npcs()[0].object_id, 19);
        assert!(model.select_npc(20));
        assert_eq!(model.selected_npc().unwrap().icon, 7);
        model.set_search_draft("NPC 20");
        assert_eq!(model.visible_npcs().len(), 1);
        assert!(model.selected_npc().is_none());
    }

    #[test]
    fn teleport_intent_is_server_gated_and_never_changes_map() {
        let mut model = BigMapModel::default();
        model.apply_new_map_info(1, info(vec![npc(42, "Teleporter", true)]));
        model.set_current_map(1);
        model.apply_world_map_setup(true, Vec::new(), 3_000);
        assert!(model.select_npc(42));
        let intent = model.selected_teleport_intent().unwrap();
        assert_eq!(intent.object_id, 42);
        assert_eq!(intent.source_map_index, Some(1));
        assert!(intent.requires_server_authorization);
        assert_eq!(model.current_map_index, Some(1));
        assert_eq!(model.view, BigMapView::CurrentMap);

        model.apply_world_map_setup(false, Vec::new(), 3_000);
        assert!(model.selected_teleport_intent().is_none());
    }

    #[test]
    fn search_result_is_authoritative_and_selects_only_a_known_npc() {
        let mut model = BigMapModel::default();
        model.apply_new_map_info(1, info(vec![npc(77, "Guide", false)]));
        model.apply_search_result(1, 77);
        assert_eq!(
            model.search.last_result,
            Some(BigMapSearchResult::Npc {
                map_index: 1,
                object_id: 77
            })
        );
        assert_eq!(model.selected_npc_object_id, Some(77));
        assert_eq!(model.active_map_index, Some(1));

        model.apply_search_result(99, 0);
        assert_eq!(
            model.search.last_result,
            Some(BigMapSearchResult::Map { map_index: 99 })
        );
        assert_eq!(model.active_map_index, Some(1));
    }

    #[test]
    fn session_and_map_resets_drop_stale_npcs_without_losing_connection_setup() {
        let mut model = BigMapModel::default();
        model.apply_world_map_setup(
            true,
            vec![BigMapWorldIcon {
                image_index: 7,
                title: "Bichon".into(),
                map_index: 1,
            }],
            3_000,
        );
        model.apply_new_map_info(1, info(vec![npc(77, "Guide", true)]));
        model.set_current_map(1);
        assert!(model.select_npc(77));

        model.reset_for_map(2, Some(BigMapPoint { x: 40, y: 41 }));
        assert!(model.maps.is_empty());
        assert_eq!(model.current_map_index, Some(2));
        assert_eq!(model.player_location, Some(BigMapPoint { x: 40, y: 41 }));
        assert!(model.selected_npc().is_none());
        assert!(model.world.enabled);
        assert_eq!(model.world.teleport_to_npc_cost, 3_000);

        model.reset_for_session();
        assert!(model.maps.is_empty());
        assert_eq!(model.current_map_index, None);
        assert_eq!(model.player_location, None);
        assert!(!model.world.enabled);
    }

    #[test]
    fn gateway_intents_are_bounded_deduplicated_and_never_mutate_player_map() {
        let mut model = BigMapModel::default();
        model.apply_new_map_info(1, info(vec![npc(42, "Teleporter", true)]));
        model.set_player_location(Some(1), BigMapPoint { x: 12, y: 13 });
        model.apply_world_map_setup(true, Vec::new(), 3_000);
        assert!(model.select_npc(42));
        let before = model.clone();

        let mut queue = BigMapGatewayIntentQueue::default();
        assert!(!queue.request_map_info(&model, 0));
        assert!(queue.request_map_info(&model, 1));
        assert!(queue.request_map_info(&model, 1));
        assert_eq!(queue.len(), 1);
        model.set_search_draft("Natural Cave");
        assert_eq!(queue.search(&mut model, 100, 500), Ok(()));
        // Search edits intentionally clear a stale selection. A renderer
        // must receive/search-select an authoritative NPC before teleporting.
        model.set_search_draft("");
        assert!(model.select_npc(42));
        assert!(queue.teleport_selected(&model));
        assert_eq!(
            queue.drain_intents(),
            vec![
                BigMapGatewayIntent::RequestMapInfo { map_index: 1 },
                BigMapGatewayIntent::SearchMap {
                    text: "natural cave".into()
                },
                BigMapGatewayIntent::TeleportToNpc { object_id: 42 },
            ]
        );
        assert_eq!(model.current_map_index, before.current_map_index);
        assert_eq!(model.player_location, before.player_location);
    }

    #[test]
    fn full_queue_rolls_back_local_search_submission() {
        let mut model = BigMapModel::default();
        let mut queue = BigMapGatewayIntentQueue::default();
        for map_index in 1..=i32::try_from(MAX_BIG_MAP_GATEWAY_INTENTS).unwrap() {
            assert!(queue.request_map_info(&model, map_index));
        }
        model.set_search_draft("Natural Cave");
        assert_eq!(
            queue.search(&mut model, 10, 1_000),
            Err(BigMapSearchError::QueueFull)
        );
        assert_eq!(model.search.last_submitted, None);
        assert_eq!(model.search.cooldown_until_ms, None);
        assert_eq!(queue.len(), MAX_BIG_MAP_GATEWAY_INTENTS);
    }

    #[test]
    fn map_or_session_reset_invalidates_pending_gateway_intents() {
        let mut model = BigMapModel::default();
        let mut queue = BigMapGatewayIntentQueue::default();
        assert!(queue.request_map_info(&model, 1));
        assert_eq!(queue.len(), 1);

        model.reset_for_map(2, Some(BigMapPoint { x: 5, y: 6 }));
        queue.sync_model(&model);
        assert_eq!(queue.len(), 0);

        assert!(queue.request_map_info(&model, 2));
        model.reset_for_session();
        queue.sync_model(&model);
        assert_eq!(queue.len(), 0);
    }

    #[test]
    fn authoritative_render_snapshot_exposes_map_image_player_and_npc_rows() {
        let mut model = BigMapModel::default();
        model.set_current_map(1);
        model.set_player_location(Some(1), BigMapPoint { x: 257, y: 594 });
        model.apply_new_map_info(1, info(vec![npc(77, "Village Guide", false)]));

        let rendered = model.render_snapshot();
        assert_eq!(rendered.map_index, Some(1));
        assert_eq!(rendered.title.as_deref(), Some("BichonProvince"));
        assert_eq!(
            rendered.map_image_url.as_deref(),
            Some("original-ui/MMap/101.png")
        );
        assert_eq!(rendered.width, 700);
        assert_eq!(rendered.height, 700);
        assert_eq!(
            rendered.player_location,
            Some(BigMapPoint { x: 257, y: 594 })
        );
        assert_eq!(rendered.npcs.len(), 1);
        assert_eq!(rendered.npcs[0].object_id, 77);
        assert_eq!(rendered.npcs[0].name, "Village Guide");
    }

    #[test]
    fn render_snapshot_never_invents_missing_map_or_npc_data() {
        let mut model = BigMapModel::default();
        model.set_current_map(1);
        model.set_player_location(Some(1), BigMapPoint { x: 257, y: 594 });

        let rendered = model.render_snapshot();
        assert_eq!(rendered.map_index, Some(1));
        assert_eq!(rendered.map_image_url, None);
        assert_eq!(rendered.player_location, None);
        assert!(rendered.npcs.is_empty());

        model.apply_world_map_setup(true, Vec::new(), 3_000);
        model.set_view(BigMapView::WorldMap);
        let world = model.render_snapshot();
        assert_eq!(world.map_image_url.as_deref(), Some(WORLD_MAP_IMAGE_URL));
        assert_eq!(world.player_location, None);
        assert!(world.npcs.is_empty());
    }
}
