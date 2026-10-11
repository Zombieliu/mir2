use super::*;

impl SharedInProcessZoneState {
    pub(super) fn five_class_practice_owner_is_current(&self, key: &ZonePresenceKey,
        receipt: &mir2_simulation::ZoneFiveClassPracticeReceipt) -> bool {
        receipt.sequence > 0 && receipt.object_id != 0
            && key.account_id == receipt.account_id
            && key.character_index == receipt.character_index
            && self.zone_sessions.get(key) == Some(&receipt.session_id)
            && self.players.get(key).is_some_and(|player| player.zone_object_id == receipt.object_id
                && ZoneKey::for_map(&player.map_file_name) == receipt.zone_key)
            && self.zone_manager.player_life_generation(&receipt.session_id) == Some(receipt.life_generation)
    }
}

impl SharedInProcessZoneSessionRuntime {
    pub(super) fn apply_five_class_practice_receipt(&mut self,
        receipt: mir2_simulation::ZoneFiveClassPracticeReceipt) -> Vec<ServerPacket> {
        self.apply_captured_five_class_practice_receipt(receipt, false)
    }

    // Only prepare_teardown_checkpoint may allow its closed, already queued
    // receipt batch after setting the fence. Normal/late deliveries still fail.
    pub(super) fn apply_captured_five_class_practice_receipt(&mut self,
        receipt: mir2_simulation::ZoneFiveClassPracticeReceipt,
        captured_before_teardown: bool) -> Vec<ServerPacket> {
        let Some(key) = self.current_presence_key() else { return Vec::new(); };
        let learned = self.inner.zone_player_combat_stats().five_class_learned_mask;
        if learned & receipt.spell.learned_bit() == 0 { return Vec::new(); }
        let accepted = {
            let mut state = self.zone_state.lock().expect("shared zone presence mutex should not be poisoned");
            if (state.teardown_fenced(&key) && !captured_before_teardown)
                || !state.five_class_practice_owner_is_current(&key, &receipt) {
                false
            } else {
                let progress = state.vital_receipt_progress.entry(key).or_default();
                if progress.object_id != receipt.object_id {
                    *progress = ZoneVitalReceiptProgress { object_id: receipt.object_id, ..Default::default() };
                }
                let accepted = receipt.sequence > progress.highest_five_class_practice_sequence;
                progress.highest_five_class_practice_sequence = progress.highest_five_class_practice_sequence.max(receipt.sequence);
                accepted
            }
        };
        if accepted { self.inner.commit_zone_five_class_practice(&receipt) } else { Vec::new() }
    }

    pub(super) fn apply_five_class_mana_receipt(&mut self, settlement: mir2_simulation::ZoneManaSettlement) -> bool {
        if settlement.receipt_sequence == 0 || settlement.mp_before < 0 || settlement.mp_after < 0
            || settlement.mp_before == settlement.mp_after { return false; }
        let max_mp = self.inner.zone_player_combat_stats().max_mp;
        if max_mp <= 0 || settlement.mp_after > max_mp { return false; }
        let Some(key) = self.current_presence_key() else { return false; };
        let accepted = {
            let mut state = self.zone_state.lock().expect("shared zone presence mutex should not be poisoned");
            let current = state.zone_sessions.get(&key).and_then(|session| Some((
                state.players.get(&key)?.zone_object_id,
                state.zone_manager.player_life_generation(session)?,
            )));
            if state.teardown_fenced(&key) || current != Some((settlement.object_id, settlement.life_generation)) { false }
            else {
                let progress = state.vital_receipt_progress.entry(key).or_default();
                if progress.object_id != settlement.object_id {
                    *progress = ZoneVitalReceiptProgress { object_id: settlement.object_id, ..Default::default() };
                }
                let accepted = settlement.receipt_sequence > progress.highest_mana_sequence;
                progress.highest_mana_sequence = progress.highest_mana_sequence.max(settlement.receipt_sequence);
                accepted
            }
        };
        if accepted { self.inner.force_authoritative_player_vitals(None, Some(settlement.mp_after)); }
        accepted
    }
}
