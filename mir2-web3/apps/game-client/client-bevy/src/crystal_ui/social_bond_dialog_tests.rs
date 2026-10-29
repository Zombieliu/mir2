use super::*;

fn guild_context() -> GuildCreationContext {
    GuildCreationContext {
        npc_object_id: 427,
        map_epoch: 7,
    }
}
fn guild_name_prompt() -> SocialBondDialogs {
    let mut ui = SocialBondDialogs::default();
    assert!(ui.begin_guild_creation_request(guild_context()));
    assert!(ui.observe(&ServerPacket::GuildNameRequest));
    assert!(matches!(
        ui.prompt.as_ref().unwrap().kind,
        BondPromptKind::GuildName { .. }
    ));
    ui
}

#[test]
fn guild_name_requires_the_sent_original_npc_link_and_consumes_request_once() {
    let mut ui = SocialBondDialogs::default();
    assert!(ui.observe(&ServerPacket::GuildNameRequest));
    assert!(
        ui.prompt.is_none(),
        "unsolicited packets cannot open the name modal"
    );
    assert!(ui.begin_guild_creation_request(guild_context()));
    assert!(
        !ui.begin_guild_creation_request(guild_context()),
        "duplicate click is suppressed"
    );
    ui.observe(&ServerPacket::GuildNameRequest);
    let revision = ui.prompt.as_ref().unwrap().revision;
    ui.input_name(revision, "Knights");
    ui.observe(&ServerPacket::GuildNameRequest);
    assert_eq!(ui.prompt.as_ref().unwrap().revision, revision);
    assert!(
        matches!(&ui.prompt.as_ref().unwrap().kind, BondPromptKind::GuildName { text } if text == "Knights")
    );
    let packet = ui.answer(revision, true).unwrap();
    assert!(matches!(&packet, ClientPacket::GuildNameReturn { name } if name == "Knights"));
    assert!(ui.can_dispatch_guild_name(&packet));
    assert!(ui.answer(revision, true).is_none());
    ui.guild_name_dispatched(&packet);
    assert!(!ui.can_dispatch_guild_name(&packet));
    ui.release_unsent(&packet);
    ui.observe(&ServerPacket::GuildNameRequest);
    assert!(
        ui.prompt.is_none(),
        "old server request cannot reopen a submitted prompt"
    );
}

#[test]
fn guild_name_cancel_and_stale_buttons_do_not_submit_or_restore() {
    let mut ui = guild_name_prompt();
    let revision = ui.prompt.as_ref().unwrap().revision;
    assert!(ui.answer(revision, false).is_none());
    assert!(ui.answer(revision, true).is_none());
    ui.observe(&ServerPacket::GuildNameRequest);
    assert!(ui.prompt.is_none());
    assert!(ui.begin_guild_creation_request(guild_context()));
    ui.observe(&ServerPacket::GuildNameRequest);
    let fresh = ui.prompt.as_ref().unwrap().revision;
    assert!(fresh > revision);
    assert!(ui.answer(revision, false).is_none());
    assert_eq!(ui.prompt.as_ref().unwrap().revision, fresh);
}

#[test]
fn guild_name_map_npc_exit_and_session_boundaries_retire_queued_name() {
    for boundary in 0..4 {
        let mut ui = guild_name_prompt();
        let revision = ui.prompt.as_ref().unwrap().revision;
        ui.input_name(revision, "Knights");
        let packet = ui.answer(revision, true).unwrap();
        match boundary {
            0 => ui.retain_guild_creation_map(8),
            1 => ui.retain_guild_creation_context(Some(GuildCreationContext {
                npc_object_id: 428,
                ..guild_context()
            })),
            2 => ui.retain_guild_creation_context(None),
            3 => ui.reset_session(),
            _ => unreachable!(),
        }
        assert!(!ui.can_dispatch_guild_name(&packet), "boundary={boundary}");
        ui.release_unsent(&packet);
        ui.observe(&ServerPacket::GuildNameRequest);
        assert!(ui.prompt.is_none(), "boundary={boundary}");
    }
}

#[test]
fn guild_name_local_transport_failure_restores_only_current_authorization() {
    let mut ui = guild_name_prompt();
    let revision = ui.prompt.as_ref().unwrap().revision;
    ui.input_name(revision, "Knights");
    let packet = ui.answer(revision, true).unwrap();
    ui.release_unsent(&packet);
    assert_eq!(ui.prompt.as_ref().unwrap().revision, revision);
    let retried = ui.answer(revision, true).unwrap();
    assert_eq!(retried, packet);
    assert!(ui.can_dispatch_guild_name(&retried));
    ui.retain_guild_creation_map(8);
    ui.release_unsent(&retried);
    assert!(ui.prompt.is_none());
}

#[test]
fn guild_name_editor_applies_twenty_utf16_units_and_validates_before_submission() {
    let mut ui = guild_name_prompt();
    let revision = ui.prompt.as_ref().unwrap().revision;
    assert!(ui.answer(revision, true).is_none());
    ui.input_name(revision, "ab");
    assert!(ui.answer(revision, true).is_none());
    ui.input.editor.as_mut().unwrap().select_all();
    ui.input_name(revision, &"🦀".repeat(11));
    let packet = ui.answer(revision, true).unwrap();
    assert!(
        matches!(packet, ClientPacket::GuildNameReturn { name } if name.encode_utf16().count() == 20)
    );
    let mut ui = guild_name_prompt();
    let revision = ui.prompt.as_ref().unwrap().revision;
    ui.input_name(revision, "bad\\name");
    assert!(ui.answer(revision, true).is_none());
    assert!(ui.input.edit_notice.is_some());
}

#[test]
fn guild_name_preserves_existing_bonds_and_never_handles_guild_war_as_sabuk() {
    let mut ui = guild_name_prompt();
    let revision = ui.prompt.as_ref().unwrap().revision;
    ui.input_name(revision, "Knights");
    let packet = ui.answer(revision, true).unwrap();
    ui.observe(&ServerPacket::MentorUpdate {
        name: "Mentor".into(),
        level: 40,
        online: true,
        mentee_exp: 5,
    });
    ui.observe(&ServerPacket::LoverUpdate {
        name: "Lover".into(),
        date_binary_datetime: 123,
        map_name: "Bichon".into(),
        married_days: 1,
    });
    assert!(ui.can_dispatch_guild_name(&packet));
    assert_eq!(ui.mentor.name, "Mentor");
    assert_eq!(ui.relationship.name, "Lover");
    assert!(!ui.observe(&ServerPacket::GuildRequestWar));
    ui.observe(&ServerPacket::MarriageRequest { name: "New".into() });
    assert!(!ui.can_dispatch_guild_name(&packet));
    ui.release_unsent(&packet);
    assert!(
        matches!(&ui.prompt.as_ref().unwrap().kind, BondPromptKind::MarriageInvite { name } if name == "New")
    );
}

#[test]
fn guild_name_prompt_has_copy_in_all_nine_locales_without_translating_names() {
    let prompt = BondPrompt {
        revision: 1,
        kind: BondPromptKind::GuildName {
            text: "Knights".into(),
        },
    };
    for locale in crate::native_i18n::Locale::ALL {
        crate::native_i18n::with_locale(locale, || {
            let text = prompt.text("Warrior");
            assert!(!text.is_empty());
            assert!(
                text.contains('3') && text.contains("20"),
                "locale={locale:?}"
            );
            assert!(!text.contains("Knights"));
            if locale != crate::native_i18n::Locale::English {
                assert_ne!(
                    text,
                    "Please enter a guild name, length must be 3~20 characters."
                );
            }
        });
    }
}

#[test]
fn rejected_transport_restores_prompt_and_stale_reply_cannot_replace_new_invitation() {
    let mut b = SocialBondDialogs::default();
    b.observe(&ServerPacket::MarriageRequest {
        name: "First".into(),
    });
    let rev = b.prompt.as_ref().unwrap().revision;
    let p = b.answer(rev, false).unwrap();
    b.release_unsent(&p);
    assert!(
        matches!(&b.prompt.as_ref().unwrap().kind,BondPromptKind::MarriageInvite{name} if name=="First")
    );
    let p = b.answer(rev, true).unwrap();
    b.observe(&ServerPacket::MarriageRequest {
        name: "Second".into(),
    });
    b.release_unsent(&p);
    assert!(
        matches!(&b.prompt.as_ref().unwrap().kind,BondPromptKind::MarriageInvite{name} if name=="Second")
    );
    assert!(b.relationship.name.is_empty());
}

#[test]
fn mentor_editor_preserves_grapheme_selection_and_source_name_packet() {
    let mut b = SocialBondDialogs::default();
    b.show(BondPage::Mentor);
    b.action(BondPage::Mentor, BondAction::AddMentor);
    let rev = b.prompt.as_ref().unwrap().revision;
    b.input_name(rev, "Old😀");
    b.input.editor.as_mut().unwrap().select_all();
    b.input_name(rev, "New");
    assert!(matches!(b.answer(rev,true),Some(ClientPacket::AddMentor{name}) if name=="New"));
}

#[test]
fn mentor_add_cancel_and_allow_do_not_fabricate_server_state() {
    let mut model = SocialBondDialogs::default();
    model.show(BondPage::Mentor);
    assert!(matches!(
        model.action(BondPage::Mentor, BondAction::Allow),
        Some(BondEffect::Packet(ClientPacket::AllowMentor))
    ));
    model.action(BondPage::Mentor, BondAction::AddMentor);
    let rev = model.prompt.as_ref().unwrap().revision;
    model.input_name(rev, "Teacher");
    assert!(matches!(model.answer(rev,true),Some(ClientPacket::AddMentor{name})if name=="Teacher"));
    assert_eq!(model.mentor.level, 0);
    assert!(model.mentor.name.is_empty());
    model.observe(&ServerPacket::MentorUpdate {
        name: "Teacher".into(),
        level: 40,
        online: true,
        mentee_exp: 55,
    });
    assert!(matches!(
        model.action(BondPage::Mentor, BondAction::AddMentor),
        Some(BondEffect::SystemChat(_))
    ));
    model.action(BondPage::Mentor, BondAction::CancelMentor);
    let rev = model.prompt.as_ref().unwrap().revision;
    assert!(model
        .prompt
        .as_ref()
        .unwrap()
        .text("Warrior")
        .contains("cooldown"));
    assert!(model.answer(rev, false).is_none());
    assert_eq!(model.mentor.name, "Teacher");
    model.action(BondPage::Mentor, BondAction::CancelMentor);
    let rev = model.prompt.as_ref().unwrap().revision;
    assert!(matches!(
        model.answer(rev, true),
        Some(ClientPacket::CancelMentor)
    ));
    assert_eq!(model.mentor.name, "Teacher");
}

#[test]
fn server_change_invalidates_stale_mentorship_confirmation() {
    let mut model = SocialBondDialogs::default();
    model.show(BondPage::Mentor);
    model.observe(&ServerPacket::MentorUpdate {
        name: "Old".into(),
        level: 40,
        online: true,
        mentee_exp: 0,
    });
    model.action(BondPage::Mentor, BondAction::CancelMentor);
    let rev = model.prompt.as_ref().unwrap().revision;
    model.observe(&ServerPacket::MentorUpdate {
        name: "New".into(),
        level: 42,
        online: true,
        mentee_exp: 0,
    });
    assert!(model.answer(rev, true).is_none());
}

#[test]
fn mentor_rows_follow_source_level_order_and_authoritative_online_xp() {
    let model = MentorDialogUi {
        name: "Partner".into(),
        level: 30,
        online: true,
        mentee_exp: 99,
        ..Default::default()
    };
    let labels = model.labels("Owner", 40);
    assert!(labels.iter().any(|l| l.text == "Owner" && l.y == 58));
    assert!(labels.iter().any(|l| l.text == "ONLINE" && l.y == 112));
    assert!(labels.iter().any(|l| l.text == "MENTEE EXP: 99"));
    let labels = model.labels("Owner", 20);
    assert!(labels.iter().any(|l| l.text == "Partner" && l.y == 58));
    assert!(!labels.iter().any(|l| l.text.starts_with("MENTEE EXP")));
    assert_eq!(MentorDialogUi::default().labels("Owner", 20).len(), 2);
}

#[test]
fn marriage_actions_guard_missing_offline_partner_without_optimistic_updates() {
    let mut model = SocialBondDialogs::default();
    model.show(BondPage::Relationship);
    assert!(matches!(
        model.action(BondPage::Relationship, BondAction::Marriage),
        Some(BondEffect::Packet(ClientPacket::MarriageRequest))
    ));
    assert!(model.relationship.name.is_empty());
    assert!(matches!(
        model.action(BondPage::Relationship, BondAction::Divorce),
        Some(BondEffect::SystemChat(_))
    ));
    model.observe(&ServerPacket::LoverUpdate {
        name: "Lover".into(),
        date_binary_datetime: 9000,
        map_name: "".into(),
        married_days: 5,
    });
    assert!(matches!(
        model.action(BondPage::Relationship, BondAction::Marriage),
        Some(BondEffect::SystemChat(_))
    ));
    assert!(matches!(
        model.action(BondPage::Relationship, BondAction::Whisper),
        Some(BondEffect::SystemChat(_))
    ));
    assert_eq!(
        model.action(BondPage::Relationship, BondAction::Mail),
        Some(BondEffect::ComposeMail("Lover".into()))
    );
    model.relationship.map_name = "Bichon".into();
    assert_eq!(
        model.action(BondPage::Relationship, BondAction::Whisper),
        Some(BondEffect::Whisper(":)".into()))
    );
    assert!(matches!(
        model.action(BondPage::Relationship, BondAction::Divorce),
        Some(BondEffect::Packet(ClientPacket::DivorceRequest))
    ));
    assert_eq!(model.relationship.name, "Lover");
}

#[test]
fn invitation_revision_reply_and_session_boundary_are_exact() {
    let mut model = SocialBondDialogs::default();
    model.observe(&ServerPacket::MarriageRequest { name: "Old".into() });
    let old = model.prompt.as_ref().unwrap().revision;
    model.observe(&ServerPacket::MentorRequest {
        name: "New".into(),
        level: 15,
    });
    let current = model.prompt.as_ref().unwrap().revision;
    assert!(model.answer(old, true).is_none());
    assert_eq!(model.prompt.as_ref().unwrap().revision, current);
    assert!(matches!(
        model.answer(current, false),
        Some(ClientPacket::MentorReply {
            accept_invite: false
        })
    ));
    model.observe(&ServerPacket::DivorceRequest {
        name: "Partner".into(),
    });
    let rev = model.prompt.as_ref().unwrap().revision;
    model.reset_session();
    assert!(model.answer(rev, true).is_none());
    model.observe(&ServerPacket::MarriageRequest {
        name: "Other".into(),
    });
    assert!(model.prompt.as_ref().unwrap().revision > rev);
}

#[test]
fn relationship_dates_preserve_source_zero_and_divorce_tick_branches() {
    let mut model = RelationshipDialogUi::default();
    let labels = model.labels(|_| "9/10/2026".into());
    assert_eq!(labels[1].text, "Marriage Date:  ");
    assert_eq!(labels[3].text, "Location:  Offline");
    model.date_binary_datetime = 1;
    let labels = model.labels(|_| "unused".into());
    assert_eq!(labels[1].text, "Date: ");
    assert_eq!(labels[2].text, "Length: ");
    model.date_binary_datetime = 2000;
    model.married_days = 10;
    let labels = model.labels(|_| "9/10/2026".into());
    assert_eq!(labels[1].text, "Divorced Date:  9/10/2026");
    assert_eq!(labels[2].text, "Time Since: 10 Days");
    assert_eq!(labels[3].text, "Location: ");
    assert_eq!(model.allow_hint(), "Allow/Block Marriage");
    model.name = "Lover".into();
    assert_eq!(model.allow_hint(), "Allow/Block Recall");
}

#[test]
fn mentor_input_enforces_winforms_utf16_limit_and_rejects_stale_text() {
    let mut model = SocialBondDialogs::default();
    model.show(BondPage::Mentor);
    model.action(BondPage::Mentor, BondAction::AddMentor);
    let rev = model.prompt.as_ref().unwrap().revision;
    model.input_name(rev + 1, "stale");
    model.input_name(rev, &"🦀".repeat(26));
    let Some(ClientPacket::AddMentor { name }) = model.answer(rev, true) else {
        panic!()
    };
    assert_eq!(name.encode_utf16().count(), 50);
}
