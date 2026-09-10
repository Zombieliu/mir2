use super::*;

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
