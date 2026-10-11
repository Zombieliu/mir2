use super::*;
use super::shared_mentor::{identity, MentorPresence};
use mir2_simulation::{
    SharedMentorAccountingReceipt, SharedMentorBreakReason, SimulationConfig,
    Stage5FriendIdentity, ZoneMentorBankAttribution,
    SHARED_MENTOR_DURATION_MS, SHARED_MENTOR_LEVEL_GAP,
};

impl SharedInProcessZoneSessionRuntime {
    pub(super) fn project_mentor_bank_participants(
        &self, config: &SimulationConfig, participants: &[Stage5FriendIdentity],
        presences: &[MentorPresence],
    ) -> Result<(), String> {
        for presence in presences.iter().filter(|p| participants.contains(&identity(&p.key))) {
            let pupil = identity(&presence.key);
            let profile = config.shared_mentor_profile_for(&pupil)?;
            let attribution = if !profile.mentor.is_mentor {
                profile.mentor.partner_identity.map(|teacher| ZoneMentorBankAttribution {
                    pupil, teacher, relationship_epoch: profile.mentor.ledger.relationship_epoch,
                })
            } else { None };
            let mut zone = presence.zone.lock().map_err(|_| "mentor Zone unavailable")?;
            // Bind the projection to this exact online registration.
            if zone.zone_sessions.get(&presence.key) != Some(&presence.session)
                || zone.players.get(&presence.key).is_none_or(|p| p.zone_object_id != presence.object_id)
            { continue; }
            zone.zone_manager.handle(ZoneCommand::UpdateMentorBank {
                session_id: presence.session.clone(), attribution,
            });
        }
        Ok(())
    }

    pub(super) fn notify_mentor_end(
        &self, receipt: &SharedMentorAccountingReceipt, presences: &[MentorPresence],
    ) {
        let owner = self.current_presence_key();
        for peer in presences.iter().filter(|p|
            receipt.participants.contains(&identity(&p.key)) && Some(&p.key) != owner.as_ref())
        {
            if let Ok(mut zone) = peer.zone.lock() {
                if zone.zone_sessions.get(&peer.key) != Some(&peer.session) { continue; }
                zone.queue_zone_packets(peer.key.clone(), vec![
                    ServerPacket::MentorUpdate { name: String::new(), level: 0, online: false, mentee_exp: 0 },
                    self.inner.shared_social_message("server.MentorshipExpired", &[]),
                ]);
            }
        }
    }

    /// No peer private-runtime/Zone FIFO lane is acquired. Store transactions
    /// finish before updating trusted Zone projections or queuing notifications.
    pub(super) fn reconcile_mentor_accounting(
        &mut self, login_expiry: bool, flush_bank: bool,
    ) -> Result<Vec<ServerPacket>, String> {
        let Some(key) = self.current_presence_key() else { return Ok(Vec::new()); };
        let Some(config) = self.inner.shared_mentor_config() else { return Ok(Vec::new()); };
        let vital_basis = self.prepare_personal_reward_vital_command()?;
        let mut packets = self.inner.consume_shared_mentor_credits()?;
        if packets.iter().any(|p| matches!(p, ServerPacket::LevelChanged { .. })) {
            if let Some(basis) = vital_basis {
                self.commit_shared_personal_vital_command(basis, &mut packets)?;
            }
            self.refresh_current_experience_profile()?;
        }
        let actor = identity(&key);
        let profile = config.shared_mentor_profile_for(&actor)?;
        let Some(peer) = profile.mentor.partner_identity.as_ref() else { return Ok(packets); };
        let partner = config.shared_mentor_profile_for(peer)?;
        let (teacher, pupil) = if profile.mentor.is_mentor { (&profile, &partner) } else { (&partner, &profile) };
        let presences = self.mentor_presences()?;
        // The current recipient may have just committed a Source level-up, or
        // be excluded from presences by its own serialized teardown fence.
        let actor_level = self.inner.active_zone_experience_profile()?.map(|p| p.level);
        let live_level = |participant: &Stage5FriendIdentity| {
            if participant == &actor { actor_level } else { presences.iter()
                .find(|p| identity(&p.key) == *participant).and_then(|p| p.entity.level) }
        };
        let live_levels = (live_level(&pupil.identity), live_level(&teacher.identity));
        let now = Self::zone_now_ms();
        let end = if login_expiry && now > teacher.mentor.established_at_ms
            .checked_add(SHARED_MENTOR_DURATION_MS).ok_or("mentor expiry exhausted")?
        { Some(SharedMentorBreakReason::LoginExpired) }
        else if !profile.mentor.is_mentor
            && live_levels.0.unwrap_or(pupil.level).saturating_add(SHARED_MENTOR_LEVEL_GAP)
                > live_levels.1.unwrap_or(teacher.level)
        { Some(SharedMentorBreakReason::Graduated) }
        else { None };
        if !flush_bank && end.is_none() { return Ok(packets); }
        let teacher_online = presences.iter().any(|p| identity(&p.key) == teacher.identity);
        let receipt = config.settle_shared_mentor_bank_with_live_levels(
            &actor, profile.mentor.ledger.relationship_epoch, end, now, teacher_online, live_levels,
        )?;
        if receipt.transferred > 0 || receipt.ended {
            self.shared_social_generation.fetch_add(1, Ordering::AcqRel);
            self.project_mentor_bank_participants(&config, &receipt.participants, &presences)?;
        }
        if receipt.ended {
            self.notify_mentor_end(&receipt, &presences);
            packets.push(ServerPacket::MentorUpdate { name: String::new(), level: 0, online: false, mentee_exp: 0 });
            packets.push(self.inner.shared_social_message("server.MentorshipExpired", &[]));
        }
        let vital_basis = self.prepare_personal_reward_vital_command()?;
        let mut credits = self.inner.consume_shared_mentor_credits()?;
        if credits.iter().any(|p| matches!(p, ServerPacket::LevelChanged { .. })) {
            if let Some(basis) = vital_basis {
                self.commit_shared_personal_vital_command(basis, &mut credits)?;
            }
            self.refresh_current_experience_profile()?;
        }
        packets.extend(credits);
        Ok(packets)
    }

    pub(super) fn refresh_mentor_bank_projection(&self) -> Result<(), String> {
        let (Some(key), Some(config)) = (self.current_presence_key(), self.inner.shared_mentor_config())
            else { return Ok(()); };
        self.project_mentor_bank_participants(&config, &[identity(&key)], &self.mentor_presences()?)
    }
}
