//! Ordinary, online party membership owned by one mailbox task.
//!
//! The caller derives `PartyPresence` from an authenticated game session. This
//! module never accepts a client account/character declaration, mutates assets,
//! or treats a personal-session name list as admission authority. Admission is
//! intentionally same-map for this MVP; it is not a cross-Gateway party service.

use std::collections::{BTreeMap, BTreeSet};
use std::time::{SystemTime, UNIX_EPOCH};

use mir2_protocol::{ChatType, GroupMember, MirClass, Point, ServerPacket};
use tokio::sync::{mpsc, oneshot};

/// Crystal Shared/Globals.cs: MaxGroup = 15.
pub const MAX_PARTY_MEMBERS: usize = 15;
pub const PARTY_INVITE_TTL_MS: u64 = 30_000;
/// Crystal Server/Settings.cs: GroupInviteDelay = 2000.
const INVITE_DELAY_MS: u64 = 2_000;
const MAILBOX_CAPACITY: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyPresence {
    pub session_id: String,
    pub account_id: String,
    pub character_index: i32,
    pub name: String,
    pub map_file_name: String,
    pub map_title: String,
    pub position: Point,
    pub class: MirClass,
    pub level: u16,
    pub hp: i32,
    pub max_hp: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PartyCommand {
    SetAllow(bool),
    Invite(String),
    Answer(bool),
    /// Internal Gateway policy fence, never populated from client fields.
    AnswerExpectedOwner {
        account_id: String,
        character_index: i32,
    },
    Remove(String),
}

#[derive(Debug, Clone)]
pub struct PartyUpdate {
    /// Fence late deliveries after logout or a different character's StartGame.
    pub recipient_session_id: String,
    pub recipient_account_id: String,
    pub recipient_character_index: i32,
    pub packets: Vec<ServerPacket>,
    pub allow_group: bool,
    pub members: Vec<String>,
    pub leader_name: String,
}

/// A clone is only a command sender; the task is the sole state writer.
#[derive(Clone)]
pub struct PartyHub {
    sender: mpsc::Sender<Request>,
}

impl Default for PartyHub {
    fn default() -> Self {
        Self::new()
    }
}

impl PartyHub {
    /// Construct inside the Gateway's Tokio runtime.
    pub fn new() -> Self {
        let (sender, mut receiver) = mpsc::channel::<Request>(MAILBOX_CAPACITY);
        tokio::spawn(async move {
            let mut state = PartyState::default();
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tokio::select! {
                    request = receiver.recv() => {
                        let Some(request) = request else { break };
                        let result = match request.operation {
                            Operation::Register(presence, outbound) => state.register(presence, outbound),
                            Operation::UpdatePresence(presence) => state.update_presence(presence),
                            Operation::Command { session_id, command, now_ms } => state.command(&session_id, command, now_ms),
                            Operation::Unregister(session_id) => state.unregister(&session_id),
                        };
                        let _ = request.reply.send(result);
                    }
                    _ = interval.tick() => {
                        // Delivery failures remove unavailable recipients; there
                        // is no unbounded queue or retry loop behind this timer.
                        let _ = state.tick(unix_ms());
                    }
                }
            }
        });
        Self { sender }
    }

    pub async fn register(
        &self,
        presence: PartyPresence,
        sender: mpsc::Sender<PartyUpdate>,
    ) -> Result<(), String> {
        self.request(Operation::Register(presence, sender)).await
    }

    pub async fn update_presence(&self, presence: PartyPresence) -> Result<(), String> {
        self.request(Operation::UpdatePresence(presence)).await
    }

    /// `now_ms` is a server-derived Unix-millisecond clock, not client input.
    pub async fn command(
        &self,
        session_id: &str,
        command: PartyCommand,
        now_ms: u64,
    ) -> Result<(), String> {
        self.request(Operation::Command {
            session_id: session_id.to_string(),
            command,
            now_ms,
        })
        .await
    }

    pub async fn unregister(&self, session_id: &str) {
        let _ = self
            .request(Operation::Unregister(session_id.to_string()))
            .await;
    }

    async fn request(&self, operation: Operation) -> Result<(), String> {
        let (reply, receive) = oneshot::channel();
        self.sender
            .send(Request { operation, reply })
            .await
            .map_err(|_| "Party authority is unavailable.".to_string())?;
        receive
            .await
            .map_err(|_| "Party authority did not confirm the command.".to_string())?
    }
}

struct Request {
    operation: Operation,
    reply: oneshot::Sender<Result<(), String>>,
}

enum Operation {
    Register(PartyPresence, mpsc::Sender<PartyUpdate>),
    UpdatePresence(PartyPresence),
    Command {
        session_id: String,
        command: PartyCommand,
        now_ms: u64,
    },
    Unregister(String),
}

#[derive(Debug, Clone)]
struct Invitation {
    inviter: String,
    expires_at_ms: u64,
}

struct OnlineMember {
    presence: PartyPresence,
    outbound: mpsc::Sender<PartyUpdate>,
    allow_group: bool,
    party: Option<u64>,
    invitation: Option<Invitation>,
    last_invite_at_ms: Option<u64>,
}

#[derive(Default)]
struct PartyState {
    online: BTreeMap<String, OnlineMember>,
    names: BTreeMap<String, String>,
    characters: BTreeMap<(String, i32), String>,
    parties: BTreeMap<u64, Vec<String>>,
    next_party_id: u64,
}

type Updates = BTreeMap<String, Vec<ServerPacket>>;

impl PartyState {
    fn register(
        &mut self,
        presence: PartyPresence,
        outbound: mpsc::Sender<PartyUpdate>,
    ) -> Result<(), String> {
        self.prune_closed()?;
        validate_presence(&presence)?;
        if outbound.is_closed() {
            return Err("Party recipient is not connected.".to_string());
        }
        if self.online.contains_key(&presence.session_id)
            || self.names.contains_key(&name_key(&presence.name))
            || self.characters.contains_key(&character_key(&presence))
        {
            return Err("That session, character, or name is already registered.".to_string());
        }
        let session_id = presence.session_id.clone();
        self.names
            .insert(name_key(&presence.name), session_id.clone());
        self.characters
            .insert(character_key(&presence), session_id.clone());
        self.online.insert(
            session_id.clone(),
            OnlineMember {
                presence,
                outbound,
                allow_group: true,
                party: None,
                invitation: None,
                last_invite_at_ms: None,
            },
        );
        self.publish(BTreeMap::from([(
            session_id,
            vec![ServerPacket::DeleteGroup],
        )]))
    }

    fn update_presence(&mut self, presence: PartyPresence) -> Result<(), String> {
        self.prune_closed()?;
        validate_presence(&presence)?;
        let old = self.member(&presence.session_id)?.presence.clone();
        if character_key(&presence) != character_key(&old) {
            return Err("Party presence cannot change its registered character.".to_string());
        }
        if old == presence {
            // This request is also an authorization barrier before an ordinary
            // player action. Even unchanged attributes must enqueue a fresh
            // caller view so the Gateway cannot reuse a stale personal roster.
            return self.publish(BTreeMap::from([(presence.session_id, Vec::new())]));
        }
        if self
            .names
            .get(&name_key(&presence.name))
            .is_some_and(|owner| owner != &presence.session_id)
        {
            return Err("That character name is already online.".to_string());
        }
        let session_id = presence.session_id.clone();
        let changed_map = old.map_file_name != presence.map_file_name;
        let renamed = old.name != presence.name;
        self.names.remove(&name_key(&old.name));
        self.names
            .insert(name_key(&presence.name), session_id.clone());
        self.online
            .get_mut(&session_id)
            .expect("member checked")
            .presence = presence;
        let mut updates = if changed_map {
            self.cancel_related_invitations(
                &session_id,
                "Group invitation cancelled after a map change.",
            )
        } else {
            Updates::new()
        };
        let party = self.member(&session_id)?.party;
        if let Some(party) = party {
            merge_updates(&mut updates, self.party_updates(party, renamed));
        } else {
            updates.entry(session_id).or_default();
        }
        self.publish(updates)
    }

    fn command(
        &mut self,
        session_id: &str,
        command: PartyCommand,
        now_ms: u64,
    ) -> Result<(), String> {
        self.tick(now_ms)?;
        self.member(session_id)?;
        let updates = match command {
            PartyCommand::SetAllow(allow) => {
                self.online
                    .get_mut(session_id)
                    .expect("member checked")
                    .allow_group = allow;
                let mut updates = Updates::new();
                if !allow {
                    merge_updates(&mut updates, self.detach_from_party(session_id));
                    merge_updates(
                        &mut updates,
                        self.cancel_related_invitations(session_id, "Grouping was disabled."),
                    );
                }
                updates.entry(session_id.to_string()).or_default();
                updates
            }
            PartyCommand::Invite(name) => self.invite(session_id, &name, now_ms)?,
            PartyCommand::Answer(accept) => self.answer(session_id, accept)?,
            PartyCommand::AnswerExpectedOwner {
                account_id,
                character_index,
            } => {
                let invitation = self
                    .member(session_id)?
                    .invitation
                    .as_ref()
                    .ok_or("There is no active group invitation.")?;
                let inviter = &self.member(&invitation.inviter)?.presence;
                if inviter.account_id != account_id || inviter.character_index != character_index {
                    return Err("The group invitation is not from the enrolled owner.".into());
                }
                self.answer(session_id, true)?
            }
            PartyCommand::Remove(name) => self.remove_member(session_id, &name)?,
        };
        self.publish(updates)
    }

    fn invite(&mut self, inviter: &str, name: &str, now_ms: u64) -> Result<Updates, String> {
        let leader = self.member(inviter)?;
        self.require_leader(inviter)?;
        if leader
            .last_invite_at_ms
            .is_some_and(|last| now_ms < last.saturating_add(INVITE_DELAY_MS))
        {
            return Err("Wait before sending another group invitation.".to_string());
        }
        if self.party_size(leader.party) >= MAX_PARTY_MEMBERS {
            return Err("The group is full.".to_string());
        }
        let target = self
            .names
            .get(&name_key(name))
            .cloned()
            .ok_or_else(|| "The invited player is not online.".to_string())?;
        if target == inviter {
            return Err("A player cannot invite itself.".to_string());
        }
        let recipient = self.member(&target)?;
        if !recipient.allow_group {
            return Err("The invited player does not allow grouping.".to_string());
        }
        if recipient.party.is_some() || recipient.invitation.is_some() {
            return Err("The invited player already has a group or invitation.".to_string());
        }
        if leader.presence.map_file_name != recipient.presence.map_file_name {
            return Err("Group invitations require both players on the same map.".to_string());
        }
        let inviter_name = leader.presence.name.clone();
        let expires_at_ms = now_ms
            .checked_add(PARTY_INVITE_TTL_MS)
            .ok_or_else(|| "Invalid group invitation deadline.".to_string())?;
        let leader = self.online.get_mut(inviter).expect("member checked");
        leader.allow_group = true;
        leader.last_invite_at_ms = Some(now_ms);
        self.online
            .get_mut(&target)
            .expect("target checked")
            .invitation = Some(Invitation {
            inviter: inviter.to_string(),
            expires_at_ms,
        });
        Ok(BTreeMap::from([
            (inviter.to_string(), Vec::new()),
            (
                target,
                vec![ServerPacket::GroupInvite { name: inviter_name }],
            ),
        ]))
    }

    fn answer(&mut self, recipient: &str, accept: bool) -> Result<Updates, String> {
        let invitation = self
            .member(recipient)?
            .invitation
            .clone()
            .ok_or_else(|| "There is no active group invitation.".to_string())?;
        if !accept {
            self.online
                .get_mut(recipient)
                .expect("recipient checked")
                .invitation = None;
            return Ok(BTreeMap::from([
                (recipient.to_string(), Vec::new()),
                (
                    invitation.inviter,
                    vec![notice("The group invitation was declined.")],
                ),
            ]));
        }
        let validation = self.validate_acceptance(recipient, &invitation.inviter);
        if let Err(error) = validation {
            self.online
                .get_mut(recipient)
                .expect("recipient checked")
                .invitation = None;
            self.publish(BTreeMap::from([(
                recipient.to_string(),
                vec![notice("The group invitation is no longer valid.")],
            )]))?;
            return Err(error);
        }
        let inviter = invitation.inviter;
        let party_id = if let Some(id) = self.member(&inviter)?.party {
            id
        } else {
            let id = self.next_party_id;
            self.next_party_id = id
                .checked_add(1)
                .ok_or_else(|| "Party identifiers are exhausted.".to_string())?;
            self.parties.insert(id, vec![inviter.clone()]);
            self.online
                .get_mut(&inviter)
                .expect("inviter checked")
                .party = Some(id);
            id
        };
        self.parties
            .get_mut(&party_id)
            .expect("party created")
            .push(recipient.to_string());
        let recipient = self.online.get_mut(recipient).expect("recipient checked");
        recipient.party = Some(party_id);
        recipient.invitation = None;
        Ok(self.party_updates(party_id, true))
    }

    fn validate_acceptance(&self, recipient: &str, inviter: &str) -> Result<(), String> {
        let recipient = self.member(recipient)?;
        let leader = self.member(inviter)?;
        self.require_leader(inviter)?;
        if !recipient.allow_group || !leader.allow_group || recipient.party.is_some() {
            return Err("The invitation can no longer be accepted.".to_string());
        }
        if recipient.presence.map_file_name != leader.presence.map_file_name {
            return Err("Both players must remain on the same map.".to_string());
        }
        if self.party_size(leader.party) >= MAX_PARTY_MEMBERS {
            return Err("The group is full.".to_string());
        }
        Ok(())
    }

    fn remove_member(&mut self, requester: &str, name: &str) -> Result<Updates, String> {
        let party_id = self
            .member(requester)?
            .party
            .ok_or_else(|| "The player is not grouped.".to_string())?;
        let target = self
            .parties
            .get(&party_id)
            .into_iter()
            .flatten()
            .find(|id| {
                self.online
                    .get(*id)
                    .is_some_and(|member| name_key(&member.presence.name) == name_key(name))
            })
            .cloned()
            .ok_or_else(|| "That player is not a group member.".to_string())?;
        if target != requester {
            self.require_leader(requester)?;
        }
        let mut updates = self.detach_from_party(&target);
        merge_updates(
            &mut updates,
            self.cancel_related_invitations(&target, "The inviting player left the group."),
        );
        Ok(updates)
    }

    fn require_leader(&self, session_id: &str) -> Result<(), String> {
        if let Some(party_id) = self.member(session_id)?.party {
            if self
                .parties
                .get(&party_id)
                .and_then(|members| members.first())
                .map(String::as_str)
                != Some(session_id)
            {
                return Err("Only the group leader can perform that action.".to_string());
            }
        }
        Ok(())
    }

    fn party_size(&self, party: Option<u64>) -> usize {
        party
            .and_then(|id| self.parties.get(&id).map(Vec::len))
            .unwrap_or(0)
    }

    fn member(&self, session_id: &str) -> Result<&OnlineMember, String> {
        self.online
            .get(session_id)
            .ok_or_else(|| "The party session is not registered.".to_string())
    }

    fn unregister(&mut self, session_id: &str) -> Result<(), String> {
        let updates = self.remove_session(session_id);
        self.publish(updates)
    }

    fn remove_session(&mut self, session_id: &str) -> Updates {
        let mut updates = self.detach_from_party(session_id);
        merge_updates(
            &mut updates,
            self.cancel_related_invitations(session_id, "The inviting player is no longer online."),
        );
        if let Some(member) = self.online.remove(session_id) {
            self.names.remove(&name_key(&member.presence.name));
            self.characters.remove(&character_key(&member.presence));
        }
        updates.remove(session_id);
        updates
    }

    fn detach_from_party(&mut self, session_id: &str) -> Updates {
        let Some(party_id) = self
            .online
            .get_mut(session_id)
            .and_then(|member| member.party.take())
        else {
            return Updates::new();
        };
        let mut members = self.parties.remove(&party_id).unwrap_or_default();
        members.retain(|member| member != session_id);
        let mut updates =
            BTreeMap::from([(session_id.to_string(), vec![ServerPacket::DeleteGroup])]);
        if members.len() <= 1 {
            for survivor in members {
                if let Some(member) = self.online.get_mut(&survivor) {
                    member.party = None;
                }
                updates.insert(survivor, vec![ServerPacket::DeleteGroup]);
            }
        } else {
            self.parties.insert(party_id, members);
            merge_updates(&mut updates, self.party_updates(party_id, true));
        }
        updates
    }

    fn cancel_related_invitations(&mut self, session_id: &str, message: &str) -> Updates {
        let cancelled = self
            .online
            .iter()
            .filter_map(|(recipient, member)| {
                member
                    .invitation
                    .as_ref()
                    .filter(|invitation| {
                        recipient == session_id || invitation.inviter == session_id
                    })
                    .map(|invitation| (recipient.clone(), invitation.inviter.clone()))
            })
            .collect::<Vec<_>>();
        let mut updates = Updates::new();
        for (recipient, inviter) in cancelled {
            self.online
                .get_mut(&recipient)
                .expect("recipient collected")
                .invitation = None;
            updates.entry(recipient).or_default().push(notice(message));
            updates.entry(inviter).or_default().push(notice(message));
        }
        updates
    }

    fn tick(&mut self, now_ms: u64) -> Result<(), String> {
        self.prune_closed()?;
        let expired = self
            .online
            .iter()
            .filter_map(|(recipient, member)| {
                member
                    .invitation
                    .as_ref()
                    .filter(|invitation| now_ms >= invitation.expires_at_ms)
                    .map(|invitation| (recipient.clone(), invitation.inviter.clone()))
            })
            .collect::<Vec<_>>();
        let mut updates = Updates::new();
        for (recipient, inviter) in expired {
            self.online
                .get_mut(&recipient)
                .expect("recipient collected")
                .invitation = None;
            updates
                .entry(recipient)
                .or_default()
                .push(notice("The group invitation expired."));
            updates
                .entry(inviter)
                .or_default()
                .push(notice("The group invitation expired."));
        }
        self.publish(updates)
    }

    fn prune_closed(&mut self) -> Result<(), String> {
        let closed = self
            .online
            .iter()
            .filter(|(_, member)| member.outbound.is_closed())
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();
        let mut updates = Updates::new();
        for id in closed {
            merge_updates(&mut updates, self.remove_session(&id));
        }
        self.publish(updates)
    }

    fn party_updates(&self, party_id: u64, rebuild: bool) -> Updates {
        let Some(members) = self.parties.get(&party_id) else {
            return Updates::new();
        };
        let names = members
            .iter()
            .filter_map(|id| self.online.get(id))
            .map(|member| member.presence.name.clone())
            .collect::<Vec<_>>();
        members
            .iter()
            .map(|id| {
                let mut packets = Vec::new();
                if rebuild {
                    // Clear incremental legacy roster entries before rebuilding;
                    // the envelope also supplies the complete authoritative view.
                    packets.push(ServerPacket::DeleteGroup);
                    packets.extend(
                        names
                            .iter()
                            .map(|name| ServerPacket::AddMember { name: name.clone() }),
                    );
                }
                (id.clone(), packets)
            })
            .collect()
    }

    fn view(&self, session_id: &str, mut packets: Vec<ServerPacket>) -> Option<PartyUpdate> {
        let recipient = self.online.get(session_id)?;
        let members = recipient
            .party
            .and_then(|id| self.parties.get(&id))
            .into_iter()
            .flatten()
            .filter_map(|id| self.online.get(id))
            .collect::<Vec<_>>();
        let names = members
            .iter()
            .map(|member| member.presence.name.clone())
            .collect::<Vec<_>>();
        let leader_name = names.first().cloned().unwrap_or_default();
        packets.insert(
            0,
            ServerPacket::SwitchGroup {
                allow_group: recipient.allow_group,
            },
        );
        if !members.is_empty() {
            packets.push(ServerPacket::GroupMemberInfo {
                members: members
                    .iter()
                    .map(|member| GroupMember {
                        name: member.presence.name.clone(),
                        level: Some(member.presence.level),
                        class: Some(member.presence.class as u8),
                        hp: Some(member.presence.hp),
                        max_hp: Some(member.presence.max_hp),
                        online: Some(true),
                    })
                    .collect(),
                leader_name: leader_name.clone(),
            });
            for member in members {
                packets.push(ServerPacket::GroupMembersMap {
                    player_name: member.presence.name.clone(),
                    player_map: member.presence.map_title.clone(),
                });
                packets.push(ServerPacket::SendMemberLocation {
                    member_name: member.presence.name.clone(),
                    member_location: member.presence.position.clone(),
                });
            }
        }
        Some(PartyUpdate {
            recipient_session_id: session_id.to_string(),
            recipient_account_id: recipient.presence.account_id.clone(),
            recipient_character_index: recipient.presence.character_index,
            packets,
            allow_group: recipient.allow_group,
            members: names,
            leader_name,
        })
    }

    /// Never await a socket writer. A slow/closed recipient loses its online
    /// admission immediately, then surviving members get the corrected view.
    fn publish(&mut self, mut updates: Updates) -> Result<(), String> {
        let mut unavailable = false;
        while !updates.is_empty() {
            let mut failed = BTreeSet::new();
            for (id, packets) in updates {
                let Some(update) = self.view(&id, packets) else {
                    continue;
                };
                let Some(member) = self.online.get(&id) else {
                    continue;
                };
                if member.outbound.try_send(update).is_err() {
                    failed.insert(id);
                }
            }
            updates = Updates::new();
            for id in failed {
                unavailable = true;
                merge_updates(&mut updates, self.remove_session(&id));
            }
        }
        if unavailable {
            Err("A party recipient was unavailable; its membership was removed.".to_string())
        } else {
            Ok(())
        }
    }
}

fn validate_presence(presence: &PartyPresence) -> Result<(), String> {
    let valid_text = |text: &str| {
        !text.trim().is_empty() && text.len() <= 256 && !text.chars().any(char::is_control)
    };
    if !valid_text(&presence.session_id)
        || !valid_text(&presence.account_id)
        || !valid_text(&presence.name)
        || presence.name.trim() != presence.name
        || !valid_text(&presence.map_file_name)
        || !valid_text(&presence.map_title)
        || presence.character_index < 0
        || presence.level == 0
        || presence.max_hp <= 0
        || presence.hp < 0
        || presence.hp > presence.max_hp
    {
        return Err("Invalid server-derived party presence.".to_string());
    }
    Ok(())
}

fn character_key(presence: &PartyPresence) -> (String, i32) {
    (
        presence.account_id.to_ascii_lowercase(),
        presence.character_index,
    )
}

fn name_key(name: &str) -> String {
    name.trim().to_lowercase()
}

fn notice(message: &str) -> ServerPacket {
    ServerPacket::Chat {
        message: message.to_string(),
        chat_type: ChatType::System,
    }
}

fn merge_updates(target: &mut Updates, source: Updates) {
    for (id, packets) in source {
        target.entry(id).or_default().extend(packets);
    }
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|time| time.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn presence(id: &str, name: &str) -> PartyPresence {
        PartyPresence {
            session_id: id.to_string(),
            account_id: format!("account-{id}"),
            character_index: 1,
            name: name.to_string(),
            map_file_name: "0".to_string(),
            map_title: "Bichon Province".to_string(),
            position: Point { x: 330, y: 270 },
            class: MirClass::Warrior,
            level: 1,
            hp: 18,
            max_hp: 18,
        }
    }

    fn add(state: &mut PartyState, id: &str, name: &str) -> mpsc::Receiver<PartyUpdate> {
        let (tx, mut rx) = mpsc::channel(64);
        state.register(presence(id, name), tx).unwrap();
        drain(&mut rx);
        rx
    }

    fn drain(rx: &mut mpsc::Receiver<PartyUpdate>) -> Vec<PartyUpdate> {
        let mut updates = Vec::new();
        while let Ok(update) = rx.try_recv() {
            updates.push(update);
        }
        updates
    }

    fn join(state: &mut PartyState, leader: &str, target: &str, now_ms: u64) {
        let name = state.online[target].presence.name.clone();
        state
            .command(leader, PartyCommand::Invite(name), now_ms)
            .unwrap();
        state
            .command(target, PartyCommand::Answer(true), now_ms + 1)
            .unwrap();
    }

    #[test]
    fn admission_requires_a_real_invitation_and_updates_both_rosters() {
        let mut state = PartyState::default();
        let mut leader = add(&mut state, "a", "Leader");
        let mut target = add(&mut state, "b", "Aluo");
        assert!(state
            .command("b", PartyCommand::Answer(true), 10_000)
            .is_err());
        state
            .command("a", PartyCommand::Invite("Aluo".into()), 10_000)
            .unwrap();
        assert!(state.online["a"].party.is_none());
        assert!(state.online["b"].party.is_none());
        let invited = drain(&mut target);
        assert!(invited.iter().all(|update| update.members.is_empty()));
        assert!(invited.iter().any(|update| update.packets.iter().any(
            |packet| matches!(packet, ServerPacket::GroupInvite { name } if name == "Leader")
        )));
        drain(&mut leader);
        state
            .command("b", PartyCommand::Answer(true), 10_001)
            .unwrap();
        for (id, rx) in [("a", &mut leader), ("b", &mut target)] {
            let update = drain(rx).pop().unwrap();
            assert_eq!(update.recipient_session_id, id);
            assert_eq!(update.recipient_account_id, format!("account-{id}"));
            assert_eq!(update.recipient_character_index, 1);
            assert_eq!(update.members, ["Leader", "Aluo"]);
            assert_eq!(update.leader_name, "Leader");
        }
    }

    #[test]
    fn false_names_self_invites_and_non_leader_actions_are_rejected() {
        let mut state = PartyState::default();
        let _a = add(&mut state, "a", "Leader");
        let _b = add(&mut state, "b", "Aluo");
        let _c = add(&mut state, "c", "Other");
        assert!(state
            .command("a", PartyCommand::Invite("NotOnline".into()), 10_000)
            .is_err());
        assert!(state
            .command("a", PartyCommand::Invite("Leader".into()), 10_000)
            .is_err());
        join(&mut state, "a", "b", 10_000);
        assert!(state
            .command("b", PartyCommand::Invite("Other".into()), 13_000)
            .is_err());
        assert!(state
            .command("b", PartyCommand::Remove("Leader".into()), 13_000)
            .is_err());
        assert!(state.online["c"].invitation.is_none());
        assert_eq!(state.parties.len(), 1);
    }

    #[test]
    fn companion_acceptance_requires_pending_inviter_exact_owner_identity() {
        let mut state = PartyState::default();
        let _owner = add(&mut state, "owner", "TrustedDisplay");
        let _actor = add(&mut state, "actor", "Companion");
        let _foreign = add(&mut state, "foreign", "OtherDisplay");
        state
            .command("foreign", PartyCommand::Invite("Companion".into()), 10_000)
            .unwrap();
        let expected = PartyCommand::AnswerExpectedOwner {
            account_id: "account-owner".into(),
            character_index: 1,
        };
        assert!(state.command("actor", expected.clone(), 10_001).is_err());
        assert!(state.online["actor"].party.is_none());
        state
            .command("actor", PartyCommand::Answer(false), 10_002)
            .unwrap();
        state
            .command("owner", PartyCommand::Invite("Companion".into()), 10_003)
            .unwrap();
        assert!(state
            .command(
                "actor",
                PartyCommand::AnswerExpectedOwner {
                    account_id: "account-owner".into(),
                    character_index: 2
                },
                10_004
            )
            .is_err());
        state.command("actor", expected, 10_005).unwrap();
        assert_eq!(state.online["actor"].party, state.online["owner"].party);
        assert!(state.online["actor"].party.is_some());
    }

    #[test]
    fn allow_flags_decline_expiry_and_map_changes_do_not_admit_members() {
        let mut state = PartyState::default();
        let mut a = add(&mut state, "a", "Leader");
        let mut b = add(&mut state, "b", "Aluo");
        state
            .command("b", PartyCommand::SetAllow(false), 10_000)
            .unwrap();
        assert!(state
            .command("a", PartyCommand::Invite("Aluo".into()), 10_000)
            .is_err());
        state
            .command("b", PartyCommand::SetAllow(true), 10_000)
            .unwrap();
        state
            .command("a", PartyCommand::Invite("Aluo".into()), 10_000)
            .unwrap();
        state
            .command("b", PartyCommand::Answer(false), 10_001)
            .unwrap();
        assert!(state.online["b"].invitation.is_none());
        state
            .command("a", PartyCommand::Invite("Aluo".into()), 13_000)
            .unwrap();
        assert!(state
            .command("b", PartyCommand::Answer(true), 43_000)
            .is_err());
        assert!(state.online["b"].invitation.is_none());
        state
            .command("a", PartyCommand::Invite("Aluo".into()), 44_000)
            .unwrap();
        let mut moved = presence("a", "Leader");
        moved.map_file_name = "0141".into();
        moved.map_title = "Bichon House".into();
        state.update_presence(moved).unwrap();
        assert!(state
            .command("b", PartyCommand::Answer(true), 44_001)
            .is_err());
        assert!(state.parties.is_empty());
        drain(&mut a);
        drain(&mut b);
    }

    #[test]
    fn duplicate_sessions_characters_and_names_do_not_replace_registration() {
        let mut state = PartyState::default();
        let _a = add(&mut state, "a", "Leader");
        for candidate in [
            presence("a", "Other"),
            presence("b", "leader"),
            PartyPresence {
                session_id: "c".into(),
                name: "Other".into(),
                ..presence("a", "Leader")
            },
        ] {
            let (tx, _rx) = mpsc::channel(8);
            assert!(state.register(candidate, tx).is_err());
        }
        let mut changed_identity = presence("a", "Leader");
        changed_identity.account_id = "another-account".into();
        assert!(state.update_presence(changed_identity).is_err());
        assert_eq!(state.online.len(), 1);
        assert_eq!(state.online["a"].presence.account_id, "account-a");
    }

    #[test]
    fn an_invitation_does_not_survive_the_inviters_loss_of_leadership() {
        let mut state = PartyState::default();
        let mut a = add(&mut state, "a", "Leader");
        let mut b = add(&mut state, "b", "Aluo");
        let mut c = add(&mut state, "c", "Other");
        state
            .command("a", PartyCommand::Invite("Aluo".into()), 10_000)
            .unwrap();
        state
            .command("c", PartyCommand::Invite("Leader".into()), 10_000)
            .unwrap();
        state
            .command("a", PartyCommand::Answer(true), 10_001)
            .unwrap();
        assert!(state
            .command("b", PartyCommand::Answer(true), 10_002)
            .is_err());
        assert!(state.online["b"].party.is_none());
        assert!(state.online["b"].invitation.is_none());
        assert_eq!(
            state.view("c", Vec::new()).unwrap().members,
            ["Other", "Leader"]
        );
        drain(&mut a);
        drain(&mut b);
        drain(&mut c);
    }

    #[test]
    fn leader_removal_and_disabling_grouping_notify_both_sides() {
        let mut state = PartyState::default();
        let mut a = add(&mut state, "a", "Leader");
        let mut b = add(&mut state, "b", "Aluo");
        join(&mut state, "a", "b", 10_000);
        drain(&mut a);
        drain(&mut b);
        state
            .command("a", PartyCommand::Remove("Aluo".into()), 10_002)
            .unwrap();
        for rx in [&mut a, &mut b] {
            assert!(drain(rx).last().unwrap().members.is_empty());
        }
        join(&mut state, "a", "b", 13_000);
        drain(&mut a);
        drain(&mut b);
        state
            .command("b", PartyCommand::SetAllow(false), 13_002)
            .unwrap();
        let update = drain(&mut b).pop().unwrap();
        assert!(!update.allow_group);
        assert!(update.members.is_empty());
        assert!(drain(&mut a).last().unwrap().members.is_empty());
        assert!(state.parties.is_empty());
    }

    #[test]
    fn disconnect_and_self_leave_clear_membership_and_pending_invites() {
        let mut state = PartyState::default();
        let mut a = add(&mut state, "a", "Leader");
        let mut b = add(&mut state, "b", "Aluo");
        let mut c = add(&mut state, "c", "Other");
        join(&mut state, "a", "b", 10_000);
        drain(&mut a);
        drain(&mut b);
        state
            .command("a", PartyCommand::Invite("Other".into()), 13_000)
            .unwrap();
        state.unregister("a").unwrap();
        assert!(!state.online.contains_key("a"));
        assert!(state.online["b"].party.is_none());
        assert!(state.online["c"].invitation.is_none());
        assert!(state.parties.is_empty());
        let latest = drain(&mut b).pop().unwrap();
        assert!(latest.members.is_empty());
        assert!(latest
            .packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::DeleteGroup)));
        drain(&mut c);
        join(&mut state, "b", "c", 15_000);
        state
            .command("c", PartyCommand::Remove("Other".into()), 15_001)
            .unwrap();
        assert!(state.online["b"].party.is_none());
        assert!(state.online["c"].party.is_none());
    }

    #[test]
    fn leader_departure_promotes_the_first_remaining_member() {
        let mut state = PartyState::default();
        let mut a = add(&mut state, "a", "Leader");
        let mut b = add(&mut state, "b", "Aluo");
        let mut c = add(&mut state, "c", "Other");
        join(&mut state, "a", "b", 10_000);
        drain(&mut a);
        drain(&mut b);
        join(&mut state, "a", "c", 13_000);
        drain(&mut a);
        drain(&mut b);
        drain(&mut c);
        state.unregister("a").unwrap();
        for rx in [&mut b, &mut c] {
            let latest = drain(rx).pop().unwrap();
            assert_eq!(latest.members, ["Aluo", "Other"]);
            assert_eq!(latest.leader_name, "Aluo");
        }
    }

    #[test]
    fn current_attributes_and_map_titles_are_broadcast_from_server_presence() {
        let mut state = PartyState::default();
        let mut a = add(&mut state, "a", "Leader");
        let mut b = add(&mut state, "b", "Aluo");
        join(&mut state, "a", "b", 10_000);
        drain(&mut a);
        drain(&mut b);
        let mut changed = presence("b", "Aluo");
        changed.position = Point { x: 331, y: 271 };
        changed.hp = 7;
        changed.map_file_name = "0141".into();
        changed.map_title = "Bichon House".into();
        state.update_presence(changed).unwrap();
        let update = drain(&mut a).pop().unwrap();
        assert!(update.packets.iter().any(|packet| matches!(packet, ServerPacket::GroupMemberInfo { members, .. } if members.iter().any(|member| member.name == "Aluo" && member.hp == Some(7)))));
        assert!(update.packets.iter().any(|packet| matches!(packet, ServerPacket::GroupMembersMap { player_name, player_map } if player_name == "Aluo" && player_map == "Bichon House")));
        assert!(update.packets.iter().any(|packet| matches!(packet, ServerPacket::SendMemberLocation { member_name, member_location } if member_name == "Aluo" && member_location == &Point { x: 331, y: 271 })));
    }

    #[test]
    fn closed_or_full_outbound_cannot_leave_ghost_admission() {
        let mut state = PartyState::default();
        let mut a = add(&mut state, "a", "Leader");
        let (tx, mut b) = mpsc::channel(1);
        state.register(presence("b", "Aluo"), tx).unwrap();
        drain(&mut b);
        state
            .command("a", PartyCommand::Invite("Aluo".into()), 10_000)
            .unwrap();
        // Keep the invitation queued: the accepted roster cannot be delivered.
        assert!(state
            .command("b", PartyCommand::Answer(true), 10_001)
            .is_err());
        assert!(!state.online.contains_key("b"));
        assert!(state.online["a"].party.is_none());
        assert!(state.parties.is_empty());
        assert!(drain(&mut a).last().unwrap().members.is_empty());
        let (tx, b) = mpsc::channel(8);
        state.register(presence("b", "Aluo"), tx).unwrap();
        drop(b);
        assert!(state
            .command("a", PartyCommand::Invite("Aluo".into()), 13_000)
            .is_err());
        assert!(!state.online.contains_key("b"));
        assert!(state.online["a"].invitation.is_none());
    }

    #[test]
    fn unavailable_registration_does_not_reserve_a_name_or_character() {
        let mut state = PartyState::default();
        let (tx, rx) = mpsc::channel(1);
        drop(rx);
        assert!(state.register(presence("a", "Aluo"), tx).is_err());
        assert!(state.names.is_empty());
        assert!(state.characters.is_empty());
        let _a = add(&mut state, "a", "Aluo");
        assert_eq!(state.online.len(), 1);
    }

    #[test]
    fn unchanged_presence_refreshes_the_caller_and_full_outbound_revokes_old_membership() {
        let mut state = PartyState::default();
        let mut a = add(&mut state, "a", "Leader");
        let (tx, mut b) = mpsc::channel(1);
        state.register(presence("b", "Aluo"), tx).unwrap();
        drain(&mut b);
        state
            .command("a", PartyCommand::Invite("Aluo".into()), 10_000)
            .unwrap();
        drain(&mut b);
        state
            .command("b", PartyCommand::Answer(true), 10_001)
            .unwrap();
        drain(&mut a);
        let old_view = drain(&mut b).pop().unwrap();
        assert_eq!(old_view.members, ["Leader", "Aluo"]);

        state.update_presence(presence("b", "Aluo")).unwrap();
        // No attribute changed: only the caller receives the barrier view.
        assert!(drain(&mut a).is_empty());
        let fresh_view = drain(&mut b).pop().unwrap();
        assert_eq!(fresh_view.members, old_view.members);
        assert_eq!(fresh_view.recipient_session_id, "b");

        // Leave this fresh view queued. The next unchanged-presence barrier
        // must fail closed rather than authorize using the cached old roster.
        state.update_presence(presence("b", "Aluo")).unwrap();
        assert!(state.update_presence(presence("b", "Aluo")).is_err());
        assert!(!state.online.contains_key("b"));
        assert!(state.parties.is_empty());
        assert!(state.online["a"].party.is_none());
        assert!(drain(&mut a).last().unwrap().members.is_empty());
        assert!(state.update_presence(presence("b", "Aluo")).is_err());
    }

    #[test]
    fn maximum_group_and_invitation_rate_are_bounded() {
        let mut state = PartyState::default();
        let mut receivers = vec![add(&mut state, "a", "Leader")];
        for index in 1..MAX_PARTY_MEMBERS {
            let id = format!("s{index}");
            receivers.push(add(&mut state, &id, &format!("Member{index}")));
            join(&mut state, "a", &id, 10_000 + index as u64 * 3_000);
            for receiver in &mut receivers {
                drain(receiver);
            }
        }
        receivers.push(add(&mut state, "extra", "Extra"));
        assert!(state
            .command("a", PartyCommand::Invite("Extra".into()), 100_000)
            .is_err());
        assert!(state.online["extra"].invitation.is_none());
        assert_eq!(
            state.parties.values().next().unwrap().len(),
            MAX_PARTY_MEMBERS
        );

        let mut state = PartyState::default();
        let _a = add(&mut state, "a", "Leader");
        let _b = add(&mut state, "b", "Aluo");
        let _c = add(&mut state, "c", "Other");
        state
            .command("a", PartyCommand::Invite("Aluo".into()), 10_000)
            .unwrap();
        assert!(state
            .command("a", PartyCommand::Invite("Other".into()), 10_001)
            .is_err());
        assert!(state.online["c"].invitation.is_none());
    }

    #[tokio::test]
    async fn public_mailbox_serializes_registration_commands_and_teardown() {
        let hub = PartyHub::new();
        let (a_tx, mut a_rx) = mpsc::channel(32);
        let (b_tx, mut b_rx) = mpsc::channel(32);
        hub.register(presence("a", "Leader"), a_tx).await.unwrap();
        hub.register(presence("b", "Aluo"), b_tx).await.unwrap();
        hub.command("a", PartyCommand::Invite("Aluo".into()), unix_ms())
            .await
            .unwrap();
        hub.command("b", PartyCommand::Answer(true), unix_ms())
            .await
            .unwrap();
        assert_eq!(drain(&mut a_rx).last().unwrap().members, ["Leader", "Aluo"]);
        assert_eq!(drain(&mut b_rx).last().unwrap().members, ["Leader", "Aluo"]);
        hub.unregister("a").await;
        assert!(drain(&mut b_rx).last().unwrap().members.is_empty());
    }
}
