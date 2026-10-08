//! Persist causal PK effects on the killer's serialized private runtime.
use super::*;

impl SharedInProcessZoneSessionRuntime {
    pub(super) fn consume_pending_player_kills(&mut self) -> Result<Vec<ServerPacket>, String> {
        let Some(key) = self.current_presence_key() else { return Ok(Vec::new()); };
        let pending = {
            let state = self.zone_state.lock().map_err(|_| "PK Zone unavailable")?;
            state.pending_zone_player_kills.get(&key).cloned().unwrap_or_default()
        };
        let mut packets = Vec::new();
        for receipt in pending {
            {
                let state = self.zone_state.lock().map_err(|_| "PK Zone unavailable")?;
                if !state.zone_manager.issued_owned_pet_kill_is_current(&receipt) {
                    return Err("pending PK receipt has lost its issuing authority".into());
                }
            }
            // The complete personal save and idempotency marker publish before
            // the issuing proof is acknowledged. Failed publication keeps both.
            let applied = self.inner.apply_shared_owned_pet_player_kill(&receipt)?;
            let owner_name = self.inner.active_identity()
                .ok_or("PK owner disappeared during settlement")?.character_name;
            let notice = self.inner.shared_social_message("server.MurderedByPlayer", &[owner_name]);
            let pk_points = self.inner.world_snapshot().player_pk_points;
            {
                let mut state = self.zone_state.lock().map_err(|_| "PK Zone unavailable")?;
                if !state.zone_manager.issued_owned_pet_kill_is_current(&receipt) {
                    return Err("PK issuing authority changed before acknowledgement".into());
                }
                // The durable penalty is immediately the shared attack-mode
                // authority, including commands which skip a broad tail sync.
                if let Some(session) = state.zone_sessions.get(&key).cloned() {
                    let chat = state.zone_manager.zone_key_for_session(&session)
                        .and_then(|zone| state.zone_manager.zone(&zone))
                        .and_then(|zone| zone.player_chat_profile(&session));
                    if let Some(mut chat) = chat {
                        chat.pk_points = pk_points;
                        let out = state.zone_manager.handle(ZoneCommand::UpdateChatProfile {
                            session_id: session, profile: chat,
                        });
                        let (owner_packets, ..) = state.dispatch_zone_outbounds(out, Some(&key));
                        packets.extend(owner_packets);
                    }
                }
                if let Some(presence) = state.players.get_mut(&key) {
                    presence.pk_points = pk_points;
                }
                state.zone_manager.acknowledge_owned_pet_kill(&receipt);
                if let Some(queue) = state.pending_zone_player_kills.get_mut(&key) {
                    queue.retain(|queued| queued != &receipt);
                    if queue.is_empty() { state.pending_zone_player_kills.remove(&key); }
                }
                if !applied.is_empty() {
                    if let Some(victim) = state.zone_session_keys.get(&receipt.victim_session_id).cloned() {
                        state.queue_zone_packets(victim, vec![notice]);
                    }
                }
            }
            if let Some(id) = self.local_self_object_id() {
                packets.extend(self.dispatch_zone_observer_packets(id, &applied));
            }
            packets.extend(applied);
        }
        Ok(packets)
    }
}
