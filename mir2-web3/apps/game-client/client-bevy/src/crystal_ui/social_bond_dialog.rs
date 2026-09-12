//! Original MentorDialog / RelationshipDialog and their input/confirmation
//! dialogs. Only server updates change the displayed relationship state.
use mir2_protocol::{ClientPacket, ServerPacket};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BondPage {
    Mentor,
    Relationship,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BondAction {
    Close,
    Allow,
    AddMentor,
    CancelMentor,
    Marriage,
    Divorce,
    Mail,
    Whisper,
}

#[derive(Debug, PartialEq)]
pub enum BondEffect {
    Packet(ClientPacket),
    SystemChat(String),
    ComposeMail(String),
    Whisper(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct MentorDialogUi {
    pub open: bool,
    pub position: Option<[f32; 2]>,
    pub name: String,
    pub level: u16,
    pub online: bool,
    pub mentee_exp: i64,
    /// Raw normal frame, not a fabricated server permission acknowledgement.
    pub allow_frame: u16,
    drag_offset: Option<[f32; 2]>,
}

impl Default for MentorDialogUi {
    fn default() -> Self {
        Self {
            open: false,
            position: None,
            name: String::new(),
            level: 0,
            online: false,
            mentee_exp: 0,
            allow_frame: 114,
            drag_offset: None,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct RelationshipDialogUi {
    pub open: bool,
    pub position: Option<[f32; 2]>,
    pub name: String,
    pub date_binary_datetime: i64,
    pub map_name: String,
    pub married_days: i16,
    drag_offset: Option<[f32; 2]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BondPromptKind {
    MentorName { text: String },
    CancelMentor { name: String, level: u16 },
    MentorInvite { name: String, level: u16 },
    MarriageInvite { name: String },
    DivorceInvite { name: String },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BondPrompt {
    pub revision: u64,
    pub kind: BondPromptKind,
}

impl BondPrompt {
    pub fn text(&self, owner_class: &str) -> String {
        match &self.kind {
            BondPromptKind::MentorName { .. } => {
                "Please enter the name of the person you would like to be your Mentor.".into()
            }
            BondPromptKind::CancelMentor { .. } => {
                "Cancelling a Mentorship early will cause a cooldown. Are you sure?".into()
            }
            BondPromptKind::MentorInvite { name, level } => format!(
                "{name} (Level {level}) has requested you teach him the ways of the {owner_class}."
            ),
            BondPromptKind::MarriageInvite { name } => {
                format!("{name} has asked for your hand in marriage.")
            }
            BondPromptKind::DivorceInvite { name } => format!("{name} has requested a divorce"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SocialBondDialogs {
    pub mentor: MentorDialogUi,
    pub relationship: RelationshipDialogUi,
    pub prompt: Option<BondPrompt>,
    revision: u64,
    pub input: super::friend_dialog::FriendDialogUi,
    pub notice: Option<String>,
    pub input_consumed: bool,
    pending: Option<(ClientPacket, BondPrompt)>,
}
impl Default for SocialBondDialogs {
    fn default() -> Self {
        Self {
            mentor: MentorDialogUi {
                allow_frame: 114,
                ..Default::default()
            },
            relationship: Default::default(),
            prompt: None,
            revision: 0,
            input: Default::default(),
            notice: None,
            input_consumed: false,
            pending: None,
        }
    }
}

impl SocialBondDialogs {
    pub fn show(&mut self, page: BondPage) {
        match page {
            BondPage::Mentor => self.mentor.open = true,
            BondPage::Relationship => self.relationship.open = true,
        }
    }
    pub fn hide(&mut self, page: BondPage) {
        match page {
            BondPage::Mentor => {
                self.mentor.open = false;
                self.mentor.drag_offset = None;
            }
            BondPage::Relationship => {
                self.relationship.open = false;
                self.relationship.drag_offset = None;
            }
        }
    }
    pub fn reset_session(&mut self) {
        let revision = self.revision.saturating_add(1);
        *self = Self::default();
        self.revision = revision;
    }
    fn prompt(&mut self, kind: BondPromptKind) {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        self.revision = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.input = Default::default();
        self.prompt = Some(BondPrompt {
            revision: self.revision,
            kind,
        });
    }
    pub fn action(&mut self, page: BondPage, action: BondAction) -> Option<BondEffect> {
        let open = match page {
            BondPage::Mentor => self.mentor.open,
            BondPage::Relationship => self.relationship.open,
        };
        if !open || self.prompt.is_some() {
            return None;
        }
        if action == BondAction::Close {
            self.hide(page);
            return None;
        }
        let chat = |s: &str| Some(BondEffect::SystemChat(s.into()));
        match (page, action) {
            (BondPage::Mentor, BondAction::Allow) => {
                // Mirror the source's normal-frame branch. Do not infer an
                // authoritative permission boolean from this client button.
                self.mentor.allow_frame = if self.mentor.allow_frame == 116 {
                    117
                } else {
                    114
                };
                Some(BondEffect::Packet(ClientPacket::AllowMentor))
            }
            (BondPage::Mentor, BondAction::AddMentor) => {
                if self.mentor.level != 0 {
                    return chat("You already have a Mentor.");
                }
                self.prompt(BondPromptKind::MentorName {
                    text: String::new(),
                });
                None
            }
            (BondPage::Mentor, BondAction::CancelMentor) => {
                if self.mentor.name.is_empty() {
                    return chat("You don't currently have a Mentorship to cancel.");
                }
                self.prompt(BondPromptKind::CancelMentor {
                    name: self.mentor.name.clone(),
                    level: self.mentor.level,
                });
                None
            }
            (BondPage::Relationship, BondAction::Allow) => {
                Some(BondEffect::Packet(ClientPacket::ChangeMarriage))
            }
            (BondPage::Relationship, BondAction::Marriage) => {
                if !self.relationship.name.is_empty() {
                    return chat("You're already married.");
                }
                Some(BondEffect::Packet(ClientPacket::MarriageRequest))
            }
            (BondPage::Relationship, BondAction::Divorce) => {
                if self.relationship.name.is_empty() {
                    return chat("You're not married.");
                }
                Some(BondEffect::Packet(ClientPacket::DivorceRequest))
            }
            (BondPage::Relationship, BondAction::Mail) => {
                if self.relationship.name.is_empty() {
                    return chat("You're not married.");
                }
                Some(BondEffect::ComposeMail(self.relationship.name.clone()))
            }
            (BondPage::Relationship, BondAction::Whisper) => {
                if self.relationship.name.is_empty() {
                    return chat("You're not married.");
                }
                if self.relationship.map_name.is_empty() {
                    return chat("Lover is not online");
                }
                Some(BondEffect::Whisper(":)".into()))
            }
            _ => None,
        }
    }

    pub fn sync_editor(&mut self) {
        if let Some(BondPrompt {
            kind: BondPromptKind::MentorName { text },
            ..
        }) = &self.prompt
        {
            if self.input.modal.as_ref().is_none_or(
                |m| !matches!(m,super::friend_dialog::FriendModal::Add{text:old,..} if old==text),
            ) {
                self.input.modal = Some(super::friend_dialog::FriendModal::Add {
                    blocked: false,
                    text: text.clone(),
                });
            }
            self.input.open = true;
            self.input.sync_editor();
        } else {
            self.input.cancel_modal();
        }
    }
    pub fn sync_draft(&mut self) {
        if let (
            Some(BondPrompt {
                kind: BondPromptKind::MentorName { text },
                ..
            }),
            Some(e),
        ) = (&mut self.prompt, &self.input.editor)
        {
            *text = e.text().to_owned();
        }
    }
    pub fn input_name(&mut self, revision: u64, value: &str) {
        if self.prompt.as_ref().is_none_or(|p| p.revision != revision) {
            return;
        }
        self.sync_editor();
        self.input.paste(value);
        self.sync_draft();
    }
    pub fn backspace(&mut self, revision: u64) {
        if self.prompt.as_ref().is_none_or(|p| p.revision != revision) {
            return;
        }
        self.sync_editor();
        if let Some(e) = &mut self.input.editor {
            let r = e.delete(true);
            self.input.commit_editor(r);
        }
        self.sync_draft();
    }
    pub fn release_unsent(&mut self, packet: &ClientPacket) {
        if self
            .pending
            .as_ref()
            .is_some_and(|(sent, _)| sent == packet)
        {
            if let Some((_, prompt)) = self.pending.take() {
                if self.prompt.is_none() {
                    self.prompt = Some(prompt);
                    self.sync_editor();
                }
            }
        }
        self.notice = Some("Unable to send. Please try again.".into());
    }

    /// Returns a real ordinary packet, never updates names, dates, XP or
    /// permissions optimistically. Stale rendered buttons cannot answer a new
    /// invitation that replaced their modal.
    pub fn answer(&mut self, revision: u64, accept: bool) -> Option<ClientPacket> {
        if self.prompt.as_ref()?.revision != revision {
            return None;
        }
        if accept && self.prompt.as_ref().is_some_and(|p|matches!(&p.kind,BondPromptKind::MentorName{text} if text.is_empty() || text.encode_utf16().count()>50)) {
            self.input.edit_notice=Some("Enter a name of at most 50 characters.".into()); return None;
        }
        let prompt = self.prompt.take()?;
        let packet = match prompt.kind.clone() {
            BondPromptKind::MentorName { text } => {
                accept.then_some(ClientPacket::AddMentor { name: text })
            }
            BondPromptKind::CancelMentor { name, level } => {
                (accept && name == self.mentor.name && level == self.mentor.level)
                    .then_some(ClientPacket::CancelMentor)
            }
            BondPromptKind::MentorInvite { .. } => Some(ClientPacket::MentorReply {
                accept_invite: accept,
            }),
            BondPromptKind::MarriageInvite { .. } => Some(ClientPacket::MarriageReply {
                accept_invite: accept,
            }),
            BondPromptKind::DivorceInvite { .. } => Some(ClientPacket::DivorceReply {
                accept_invite: accept,
            }),
        };
        self.pending = packet.clone().map(|packet| (packet, prompt));
        packet
    }

    pub fn observe(&mut self, packet: &ServerPacket) -> bool {
        match packet {
            ServerPacket::MentorUpdate {
                name,
                level,
                online,
                mentee_exp,
            } => {
                self.pending = None;
                if self.mentor.name != *name || self.mentor.level != *level {
                    if self.prompt.as_ref().is_some_and(|p| {
                        matches!(
                            p.kind,
                            BondPromptKind::MentorName { .. }
                                | BondPromptKind::CancelMentor { .. }
                                | BondPromptKind::MentorInvite { .. }
                        )
                    }) {
                        self.prompt = None;
                    }
                }
                self.mentor.name = name.clone();
                self.mentor.level = *level;
                self.mentor.online = *online;
                self.mentor.mentee_exp = *mentee_exp;
            }
            ServerPacket::LoverUpdate {
                name,
                date_binary_datetime,
                map_name,
                married_days,
            } => {
                self.pending = None;
                if self.relationship.name != *name
                    || self.relationship.date_binary_datetime != *date_binary_datetime
                {
                    if self.prompt.as_ref().is_some_and(|p| {
                        matches!(
                            p.kind,
                            BondPromptKind::MarriageInvite { .. }
                                | BondPromptKind::DivorceInvite { .. }
                        )
                    }) {
                        self.prompt = None;
                    }
                }
                self.relationship.name = name.clone();
                self.relationship.date_binary_datetime = *date_binary_datetime;
                self.relationship.map_name = map_name.clone();
                self.relationship.married_days = *married_days;
            }
            ServerPacket::MentorRequest { name, level } => {
                self.prompt(BondPromptKind::MentorInvite {
                    name: name.clone(),
                    level: *level,
                })
            }
            ServerPacket::MarriageRequest { name } => {
                self.prompt(BondPromptKind::MarriageInvite { name: name.clone() })
            }
            ServerPacket::DivorceRequest { name } => {
                self.prompt(BondPromptKind::DivorceInvite { name: name.clone() })
            }
            _ => return false,
        }
        true
    }

    pub fn begin_drag(&mut self, page: BondPage, cursor: [f32; 2]) {
        if self.prompt.is_some() {
            return;
        }
        let (open, position, offset) = match page {
            BondPage::Mentor => (
                self.mentor.open,
                self.mentor.position,
                &mut self.mentor.drag_offset,
            ),
            BondPage::Relationship => (
                self.relationship.open,
                self.relationship.position,
                &mut self.relationship.drag_offset,
            ),
        };
        if open {
            if let Some(p) = position {
                *offset = Some([cursor[0] - p[0], cursor[1] - p[1]]);
            }
        }
    }
    pub fn drag_to(&mut self, page: BondPage, cursor: [f32; 2]) {
        let (position, offset) = match page {
            BondPage::Mentor => (&mut self.mentor.position, self.mentor.drag_offset),
            BondPage::Relationship => (
                &mut self.relationship.position,
                self.relationship.drag_offset,
            ),
        };
        if let Some(o) = offset {
            *position = Some([cursor[0] - o[0], cursor[1] - o[1]]);
        }
    }
    pub fn end_drag(&mut self) {
        self.mentor.drag_offset = None;
        self.relationship.drag_offset = None;
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct BondLabel {
    pub text: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub font_points: u8,
    pub tone: BondTone,
}
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum BondTone {
    DimGray,
    LightGray,
    Green,
}

impl MentorDialogUi {
    pub fn labels(&self, owner_name: &str, owner_level: u16) -> Vec<BondLabel> {
        let mut labels = vec![
            BondLabel {
                text: "MENTOR".into(),
                x: 15,
                y: 41,
                width: 200,
                font_points: 7,
                tone: BondTone::DimGray,
            },
            BondLabel {
                text: "MENTEE".into(),
                x: 15,
                y: 94,
                width: 200,
                font_points: 7,
                tone: BondTone::DimGray,
            },
        ];
        if self.level == 0 {
            return labels;
        }
        let owner_is_mentor = owner_level > self.level;
        let entries = if owner_is_mentor {
            [
                (owner_name, owner_level, false, 58),
                (self.name.as_str(), self.level, self.online, 112),
            ]
        } else {
            [
                (self.name.as_str(), self.level, self.online, 58),
                (owner_name, owner_level, false, 112),
            ]
        };
        for (name, level, online, y) in entries {
            labels.push(BondLabel {
                text: name.into(),
                x: 20,
                y,
                width: 200,
                font_points: 10,
                tone: BondTone::LightGray,
            });
            labels.push(BondLabel {
                text: format!("Lv {level}"),
                x: 170,
                y: if y == 112 { 111 } else { y },
                width: 200,
                font_points: 10,
                tone: BondTone::LightGray,
            });
            if online {
                labels.push(BondLabel {
                    text: "ONLINE".into(),
                    x: 125,
                    y,
                    width: 200,
                    font_points: 7,
                    tone: BondTone::Green,
                });
            }
        }
        if owner_is_mentor {
            labels.push(BondLabel {
                text: format!("MENTEE EXP: {}", self.mentee_exp),
                x: 15,
                y: 147,
                width: 200,
                font_points: 7,
                tone: BondTone::DimGray,
            });
        }
        labels
    }
}

impl RelationshipDialogUi {
    pub fn labels(&self, short_date: impl Fn(i64) -> String) -> Vec<BondLabel> {
        let ticks = (self.date_binary_datetime as u64 & 0x3fff_ffff_ffff_ffff) as i64;
        let (date, length, location) = if self.name.is_empty() && ticks != 0 {
            // Source really constructs DateTime(2000 ticks), not year 2000.
            let (date, length) = if ticks < 2000 {
                ("Date: ".into(), "Length: ".into())
            } else {
                (
                    format!("Divorced Date:  {}", short_date(self.date_binary_datetime)),
                    format!("Time Since: {} Days", self.married_days),
                )
            };
            (date, length, "Location: ".into())
        } else {
            (
                format!(
                    "Marriage Date:  {}",
                    if ticks == 0 {
                        String::new()
                    } else {
                        short_date(self.date_binary_datetime)
                    }
                ),
                format!("Length: {} Days", self.married_days),
                if self.map_name.is_empty() {
                    "Location:  Offline".into()
                } else {
                    format!("Location:  {}", self.map_name)
                },
            )
        };
        [format!("Lover:  {}", self.name), date, length, location]
            .into_iter()
            .enumerate()
            .map(|(i, text)| BondLabel {
                text,
                x: 30,
                y: 40 + i as i32 * 25,
                width: 200,
                font_points: 10,
                tone: BondTone::LightGray,
            })
            .collect()
    }
    pub fn allow_hint(&self) -> &'static str {
        if self.name.is_empty() && self.date_binary_datetime != 0 {
            "Allow/Block Marriage"
        } else {
            "Allow/Block Recall"
        }
    }
}

#[cfg(test)]
#[path = "social_bond_dialog_tests.rs"]
mod tests;
#[cfg(feature = "native-player-ui")]
#[path = "social_bond_dialog_render.rs"]
pub mod view;

#[cfg(feature = "native-player-ui")]
#[path = "social_bond_dialog_host.rs"]
pub mod host;
