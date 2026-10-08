//! Trusted pet snapshot/XP producer on the owner's serialized private lane.
//! No client pet record, peer private lock, or durable online ID is admitted.
use super::*;

impl SharedInProcessZoneSessionRuntime {
    pub(super) fn refresh_pet_progress_context(&mut self)->Result<(),String> {
        // A prior durable source must reach the same typed pet before another
        // source captures its growth. A deleted/changed pet cannot be revived.
        self.publish_committed_pet_progress()?;
        let Some(key)=self.current_presence_key() else{return Ok(());};
        let captured={
            let state=self.zone_state.lock().map_err(|_|"pet capture Zone unavailable")?;
            state.zone_sessions.get(&key).map(|session|(
                state.zone_manager.capture_player_saved_pets(session,Self::zone_now_ms()),
                state.zone_manager.capture_player_pet_experience(session,Self::zone_now_ms())
            ))
        };
        if let Some((snapshot,admission))=captured {
            self.inner.set_shared_pet_experience_context(snapshot,admission)?;
        }
        Ok(())
    }
    pub(super) fn publish_committed_pet_progress(&mut self)->Result<(),String> {
        // Do not consume committed progress while Zone/identity admission is
        // unavailable: a retry must retain the exact already-saved source.
        let Some((admission,steps))=self.inner.pending_shared_pet_experience_commit() else{return Ok(());};
        let Some(key)=self.current_presence_key() else{return Err("pet growth lost active Human identity".into());};
        let mut state=self.zone_state.lock().map_err(|_|"pet progress Zone unavailable")?;
        let Some(session)=state.zone_sessions.get(&key).cloned() else{return Err("pet growth lost online Human".into());};
        let outbounds=state.zone_manager.mirror_player_pet_earned_steps(&session,&admission,&steps,Self::zone_now_ms());
        // Keep self and observers on the ordinary FIFO; no guessed local ID.
        let (packets,_,_,_,_,_,_)=state.dispatch_zone_outbounds(outbounds,Some(&key));
        state.pending_zone_packets.entry(key).or_default().extend(packets);
        drop(state);
        self.inner.acknowledge_shared_pet_experience_commit(&steps)?;
        Ok(())
    }
    pub(super) fn remember_warm_pet_snapshot(&self,key:&ZonePresenceKey,snapshot:mir2_simulation::ZoneSavedPetSnapshot) {
        if let Ok(mut state)=self.zone_state.lock(){state.warm_pet_snapshots.insert(key.clone(),snapshot);}
    }
}
