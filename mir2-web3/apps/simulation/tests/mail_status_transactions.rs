use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use mir2_protocol::{ClientPacket, ServerPacket};
use mir2_simulation::{
    deliver_stage5_system_mail, SimulationConfig, SimulationSession, Stage5MailDelivery,
    Stage5MailTargetKind,
};

fn unique_store_path(label: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "mir2-mail-status-{label}-{}-{suffix}.json",
        std::process::id()
    ))
}

fn prepared_mailbox(label: &str) -> (SimulationConfig, u32) {
    let path = unique_store_path(label);
    let config = SimulationConfig::default().with_account_store_path(path);
    let receipt = deliver_stage5_system_mail(
        &config,
        Stage5MailDelivery {
            target_kind: Stage5MailTargetKind::Character,
            target_id: "Scout".to_string(),
            from: "System".to_string(),
            subject: "Status test".to_string(),
            body: "Durable mailbox".to_string(),
            gold: 0,
            items: Vec::new(),
        },
    )
    .expect("mail fixture should persist");
    assert_eq!(receipt.delivered_count, 1);
    (config, receipt.mail_ids[0])
}

fn started_session(config: SimulationConfig) -> SimulationSession {
    let mut session = SimulationSession::new(config);
    assert!(session
        .handle_packet(ClientPacket::Login {
            account_id: "demo".to_string(),
            password: "demo".to_string(),
        })
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    assert!(session
        .handle_packet(ClientPacket::StartGame { character_index: 0 })
        .iter()
        .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
    session
}

fn mail_flags(packets: &[ServerPacket], mail_id: u32) -> (bool, bool, bool) {
    let mail = packets
        .iter()
        .find_map(|packet| match packet {
            ServerPacket::ReceiveMail { mail } => {
                mail.iter().find(|mail| mail.mail_id == u64::from(mail_id))
            }
            _ => None,
        })
        .expect("ReceiveMail should contain the requested mail");
    (mail.opened, mail.locked, mail.collected)
}

#[test]
fn read_and_lock_are_durable_and_reloadable() {
    let (config, mail_id) = prepared_mailbox("read-lock");
    let mut session = started_session(config.clone());

    let read = session.handle_packet(ClientPacket::ReadMail {
        mail_id: u64::from(mail_id),
    });
    assert_eq!(mail_flags(&read, mail_id), (true, false, false));

    let lock = session.handle_packet(ClientPacket::LockMail {
        mail_id: u64::from(mail_id),
        lock: true,
    });
    assert_eq!(mail_flags(&lock, mail_id), (true, true, false));

    let mut reloaded = started_session(config);
    let snapshot = reloaded.handle_packet(ClientPacket::ReadMail {
        mail_id: u64::from(mail_id),
    });
    assert_eq!(mail_flags(&snapshot, mail_id), (true, true, false));
}

#[test]
fn stale_session_cannot_delete_lock_but_can_unlock_then_delete_durably() {
    let (config, mail_id) = prepared_mailbox("stale-lock-delete");
    let mut current = started_session(config.clone());
    let mut stale = started_session(config.clone());

    let locked = current.handle_packet(ClientPacket::LockMail {
        mail_id: u64::from(mail_id),
        lock: true,
    });
    assert_eq!(mail_flags(&locked, mail_id), (false, true, false));

    let rejected = stale.handle_packet(ClientPacket::DeleteMail {
        mail_id: u64::from(mail_id),
    });
    assert!(
        rejected.is_empty(),
        "a rejected stale delete must not expose the cached mailbox: {rejected:?}"
    );

    let still_locked = current.handle_packet(ClientPacket::LockMail {
        mail_id: u64::from(mail_id),
        lock: true,
    });
    assert_eq!(mail_flags(&still_locked, mail_id), (false, true, false));

    let unlock = stale.handle_packet(ClientPacket::LockMail {
        mail_id: u64::from(mail_id),
        lock: false,
    });
    assert_eq!(mail_flags(&unlock, mail_id), (false, false, false));

    let deleted = stale.handle_packet(ClientPacket::DeleteMail {
        mail_id: u64::from(mail_id),
    });
    assert!(!deleted.iter().any(|packet| {
        matches!(packet, ServerPacket::ReceiveMail { mail } if mail.iter().any(|mail| mail.mail_id == u64::from(mail_id)))
    }));

    let mut reloaded = started_session(config);
    let after_reload = reloaded.handle_packet(ClientPacket::ReadMail {
        mail_id: u64::from(mail_id),
    });
    assert!(!after_reload.iter().any(|packet| {
        matches!(packet, ServerPacket::ReceiveMail { mail } if mail.iter().any(|mail| mail.mail_id == u64::from(mail_id)))
    }));
}

#[test]
fn malformed_missing_deleted_and_invalid_identity_are_no_ops() {
    let (config, mail_id) = prepared_mailbox("invalid-no-op");
    let mut session = started_session(config.clone());
    let baseline = session.handle_packet(ClientPacket::ReadMail {
        mail_id: u64::from(mail_id),
    });
    assert_eq!(mail_flags(&baseline, mail_id), (true, false, false));

    let missing = session.handle_packet(ClientPacket::DeleteMail { mail_id: 999_999 });
    assert!(
        missing.is_empty(),
        "a missing mail identity must be a silent no-op: {missing:?}"
    );

    let oversized = session.handle_packet(ClientPacket::LockMail {
        mail_id: u64::from(u32::MAX) + 1,
        lock: true,
    });
    assert!(
        oversized.is_empty(),
        "an oversized mail identity must be a silent no-op: {oversized:?}"
    );

    {
        let mut store = config
            .account_store
            .lock()
            .expect("account store should not be poisoned");
        store
            .accounts
            .get_mut("demo")
            .and_then(|account| account.saves.get_mut(&0))
            .expect("demo save should exist")
            .stage5_systems_json = Some("{malformed-mail-state".to_string());
    }
    config
        .save_account_store()
        .expect("malformed fixture should persist");

    let malformed = session.handle_packet(ClientPacket::LockMail {
        mail_id: u64::from(mail_id),
        lock: true,
    });
    assert!(
        malformed.is_empty(),
        "malformed durable mail state must fail closed: {malformed:?}"
    );
    assert_eq!(
        config
            .account_store
            .lock()
            .expect("account store should not be poisoned")
            .accounts["demo"]
            .saves[&0]
            .stage5_systems_json
            .as_deref(),
        Some("{malformed-mail-state")
    );
}
