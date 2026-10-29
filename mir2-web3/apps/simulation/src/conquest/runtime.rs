use super::*;

impl SharedConquestRecord {
    /// Called after an ordinary in-range registrar link and current guild-leader
    /// permission check. Duplicate requests are a successful zero-change retry.
    pub fn request(&mut self, guild_id: &str, now_ms: u64) -> Result<bool, String> {
        self.validate()?;
        if !valid_guild_id(guild_id) {
            return Err("invalid conquest applicant".into());
        }
        if self.war_active {
            return Err("conquest war already active".into());
        }
        if self.owner_guild_id.as_deref() == Some(guild_id) {
            return Err("castle owner cannot challenge own castle".into());
        }
        if let Some(attacker) = &self.attacker_guild_id {
            if attacker == guild_id {
                return Ok(false);
            }
            return Err("another guild already applied for conquest".into());
        }
        let mut next = self.clone();
        next.attacker_guild_id = Some(guild_id.into());
        next.event(now_ms, ConquestEventKind::Requested, Some(guild_id.into()))?;
        next.changed()?;
        next.validate()?;
        *self = next;
        Ok(true)
    }

    /// One durable lease fences overlapping server schedulers. No session activity
    /// is needed to open or settle the calendar window.
    pub fn tick(
        &mut self,
        policy: &ConquestPolicy,
        now_ms: u64,
        owner: &str,
        palace: &[ConquestPalacePresence],
    ) -> Result<Vec<ConquestEvent>, String> {
        self.validate()?;
        policy.validate()?;
        if self.index != policy.index || !valid_guild_id(owner) || now_ms < self.last_observed_ms {
            return Err("invalid conquest clock observation".into());
        }
        if self
            .lease
            .as_ref()
            .is_some_and(|lease| lease.owner != owner && lease.expires_ms > now_ms)
        {
            return Ok(Vec::new());
        }
        let mut next = self.clone();
        let first_event = next.next_event_sequence;
        let expires = now_ms
            .checked_add(30_000)
            .filter(|ms| *ms <= i64::MAX as u64)
            .ok_or("conquest lease exhausted")?;
        if next
            .lease
            .as_ref()
            .is_none_or(|lease| lease.owner != owner || lease.expires_ms <= now_ms)
        {
            next.clock_generation = next
                .clock_generation
                .checked_add(1)
                .filter(|generation| *generation <= i64::MAX as u64)
                .ok_or("conquest clock generation exhausted")?;
        }
        next.lease = Some(ConquestLease {
            owner: owner.into(),
            generation: next.clock_generation,
            expires_ms: expires,
        });
        next.last_observed_ms = now_ms;
        let window = policy.window(now_ms)?;
        if next.war_active
            && (now_ms >= next.war_ends_ms || next.battle_day != Some(window.day) || !window.open)
        {
            next.war_active = false;
            next.attacker_guild_id = None;
            next.event(
                now_ms,
                ConquestEventKind::WarEnded,
                next.owner_guild_id.clone(),
            )?;
        }
        if !next.war_active
            && window.open
            && next.battle_day != Some(window.day)
            && next.attacker_guild_id.is_some()
        {
            next.war_active = true;
            next.battle_day = Some(window.day);
            next.war_ends_ms = window.end_ms;
            next.last_capture_ms = 0;
            next.event(
                now_ms,
                ConquestEventKind::WarStarted,
                next.attacker_guild_id.clone(),
            )?;
        }
        if next.war_active
            && (next.last_capture_ms == 0
                || now_ms.saturating_sub(next.last_capture_ms) >= policy.capture_interval_ms)
        {
            next.last_capture_ms = now_ms;
            let living: Vec<_> = palace
                .iter()
                .filter(|p| {
                    p.alive
                        && p.map_file_name
                            .eq_ignore_ascii_case(&policy.palace_file_name)
                })
                .collect();
            // Living unguilded occupants contest, avoiding the source null-guild crash.
            if let Some(guild) = living.first().and_then(|p| p.guild_id.as_ref()) {
                let exclusive = living.iter().all(|p| p.guild_id.as_ref() == Some(guild));
                if exclusive
                    && next.owner_guild_id.as_ref() != Some(guild)
                    && next.attacker_guild_id.as_ref() == Some(guild)
                {
                    if let Some(previous) = next.owner_guild_id.replace(guild.clone()) {
                        next.attacker_guild_id = Some(previous);
                    }
                    next.event(now_ms, ConquestEventKind::Captured, Some(guild.clone()))?;
                }
            }
        }
        if next == *self {
            return Ok(Vec::new());
        }
        next.changed()?;
        next.validate()?;
        let events = next
            .events
            .iter()
            .filter(|event| event.sequence >= first_event)
            .cloned()
            .collect();
        *self = next;
        Ok(events)
    }
}
