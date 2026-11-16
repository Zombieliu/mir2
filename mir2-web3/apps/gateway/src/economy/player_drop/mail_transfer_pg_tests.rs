//! Real public player-mail source transactions on a strictly disposable PG.
//! Receipt evidence deliberately does not enable PG ExistingCustody collection.
use super::Database;
use mir2_protocol::{ClientPacket, ServerPacket};
use mir2_simulation::{
    deliver_stage5_system_mail, CharacterSaveRecord, InProcessWorldRuntime, SimulationConfig,
    Stage5MailAttachmentOrigin, Stage5MailDelivery, Stage5MailMessage, Stage5MailTargetKind,
    Stage5SystemsState, WorldCommand, WorldRuntime,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::{mpsc, Arc};
use std::time::Duration;

const SENDER: &str = "transfer-sender";
const RECIPIENT: &str = "transfer-recipient";
const SENDER_NAME: &str = "TransferSender";
const RECIPIENT_NAME: &str = "TransferRecipient";
const CENSUS: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const SENT_GOLD: u32 = 17;

fn relogin(config: SimulationConfig, account: &str, index: i32) -> InProcessWorldRuntime {
    let mut runtime = InProcessWorldRuntime::new(config);
    let packets = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::Login {
            account_id: account.into(),
            password: account.into(),
        }))
        .unwrap();
    assert!(packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    let packets = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
            character_index: index,
        }))
        .unwrap();
    assert!(packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
    runtime
}

fn stored(db: &Database, account: &str, index: i32) -> CharacterSaveRecord {
    let value: Value = db
        .connect()
        .query_one(
            "SELECT snapshot_json FROM character_saves WHERE account_id=$1 AND character_index=$2",
            &[&account, &index],
        )
        .unwrap()
        .get(0);
    serde_json::from_value(value).unwrap()
}

fn systems(save: &CharacterSaveRecord) -> Stage5SystemsState {
    serde_json::from_str(save.stage5_systems_json.as_deref().unwrap()).unwrap()
}

fn image(save: &CharacterSaveRecord) -> Value {
    serde_json::to_value(save).unwrap()
}

fn assets(save: &CharacterSaveRecord) -> Value {
    serde_json::json!({
        "revision":save.revision, "gold":save.gold, "credit":save.credit,
        "inventory":save.inventory_items_json, "belt":save.belt_items_json,
        "storage":save.storage_items_json, "equipment":save.equipment_items_json,
        "heroInventory":save.hero_inventory_items_json, "heroEquipment":save.hero_equipment_items_json,
        "mail":systems(save).mail,
    })
}

fn count_receipts(db: &Database) -> i64 {
    db.connect()
        .query_one(
            "SELECT count(*) FROM public.mail_item_transfer_receipts",
            &[],
        )
        .unwrap()
        .get(0)
}

fn send(runtime: &mut InProcessWorldRuntime, uid: u64) -> Vec<ServerPacket> {
    runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::SendMail {
            name: RECIPIENT_NAME.into(),
            message: r#"Ordinary player text {"unique_id":18446744073709551615}"#.into(),
            gold: SENT_GOLD,
            items_idx: [uid, 0, 0, 0, 0],
            stamped: false,
        }))
        .unwrap()
}

fn success(packets: &[ServerPacket]) -> bool {
    packets.contains(&ServerPacket::MailSent { result: 1 })
}

struct Fixture {
    db: Database,
    config: SimulationConfig,
    sender_index: i32,
    recipient_index: i32,
    uid: u64,
    original_json: String,
}

impl Fixture {
    fn new() -> Self {
        let db = Database::new();
        db.store()
            .initialize_player_drop_item_uid_authority(
                5_000_000,
                CENSUS,
                "owned-mail-transfer-fixture",
            )
            .unwrap();
        let config = db
            .config("owner-a", 7)
            .with_postgres_item_uid_allocator()
            .unwrap();
        let mut sender =
            crate::economy::start_test_runtime_with_config(config.clone(), SENDER, SENDER_NAME)
                .unwrap();
        let sender_index = sender.active_identity().unwrap().character_index;
        sender.save_active_character().unwrap();
        let mut recipient = crate::economy::start_test_runtime_with_config(
            config.clone(),
            RECIPIENT,
            RECIPIENT_NAME,
        )
        .unwrap();
        let recipient_index = recipient.active_identity().unwrap().character_index;
        recipient.save_active_character().unwrap();
        drop(recipient);
        let template = mir2_game_data::crystal_item_by_name("BronzeHelmet").unwrap();
        let reward = deliver_stage5_system_mail(
            &config,
            Stage5MailDelivery {
                target_kind: Stage5MailTargetKind::Character,
                target_id: SENDER_NAME.into(),
                from: "Owned PG fixture".into(),
                subject: "Public fresh helmet".into(),
                body: "Claim the public source reward before transferring its original instance"
                    .into(),
                gold: 100_000,
                items: vec![format!("crystal-item-{}", template.item_index)],
            },
        )
        .unwrap();
        assert_eq!(reward.delivered_count, 1);
        assert_eq!(reward.mail_ids.len(), 1);
        let delivered = stored(&db, SENDER, sender_index);
        assert_eq!(
            systems(&delivered)
                .mail
                .iter()
                .find(|mail| mail.id == reward.mail_ids[0])
                .unwrap()
                .attachment_origin,
            Stage5MailAttachmentOrigin::FreshGrant
        );
        drop(sender);
        let mut sender = relogin(config.clone(), SENDER, sender_index);
        let packets = sender
            .execute(WorldCommand::ClientPacket(ClientPacket::CollectParcel {
                mail_id: u64::from(reward.mail_ids[0]),
            }))
            .unwrap();
        assert!(packets.contains(&ServerPacket::ParcelCollected { result: 1 }));
        let gained: Vec<_> = packets
            .iter()
            .filter_map(|packet| match packet {
                ServerPacket::GainedItem { item } if item.item_index == template.item_index => {
                    Some(item.unique_id)
                }
                _ => None,
            })
            .collect();
        assert_eq!(gained.len(), 1);
        let uid = gained[0];
        assert!(uid > 5_000_000);
        let committed = stored(&db, SENDER, sender_index);
        let original_json = committed
            .inventory_items_json
            .iter()
            .find(|encoded| {
                serde_json::from_str::<Value>(encoded).unwrap()["unique_id"].as_u64() == Some(uid)
            })
            .unwrap()
            .clone();
        assert_eq!(
            count_receipts(&db),
            0,
            "fresh system grants are not player transfer receipts"
        );
        drop(sender);
        Self {
            db,
            config,
            sender_index,
            recipient_index,
            uid,
            original_json,
        }
    }

    fn recipient_mail(&self) -> Stage5MailMessage {
        let save = stored(&self.db, RECIPIENT, self.recipient_index);
        let mails: Vec<_> = systems(&save)
            .mail
            .into_iter()
            .filter(|mail| mail.from == SENDER_NAME)
            .collect();
        assert_eq!(mails.len(), 1);
        mails.into_iter().next().unwrap()
    }

    fn assert_receipt(&self, mail: &Stage5MailMessage) -> (Value, String) {
        assert_eq!(count_receipts(&self.db), 1);
        let row = self.db.connect().query_one(
            "SELECT source_namespace,recipient_account_id,recipient_character_index,binding,binding_digest FROM public.mail_item_transfer_receipts WHERE delivery_nonce=$1",
            &[&mail.delivery_nonce],
        ).unwrap();
        assert_eq!(row.get::<_, String>(0), "public");
        assert_eq!(row.get::<_, String>(1), RECIPIENT);
        assert_eq!(row.get::<_, i32>(2), self.recipient_index);
        let binding: Value = row.get(3);
        let digest: String = row.get(4);
        let recomputed: String = Sha256::digest(serde_json::to_vec(&binding).unwrap())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(digest, recomputed);
        assert_eq!(binding["version"], 1);
        assert_eq!(binding["sender"]["accountId"], SENDER);
        assert_eq!(binding["sender"]["characterIndex"], self.sender_index);
        assert_eq!(binding["recipient"]["accountId"], RECIPIENT);
        assert_eq!(binding["recipient"]["characterIndex"], self.recipient_index);
        assert_eq!(binding["mail"]["deliveryNonce"], mail.delivery_nonce);
        assert_eq!(
            binding["mail"]["itemStatesJson"],
            serde_json::json!([self.original_json])
        );
        assert_eq!(
            binding["originalSourceItemJson"],
            serde_json::json!([self.original_json])
        );
        assert_eq!(
            binding["attachmentManifest"],
            serde_json::json!([serde_json::from_str::<Value>(&self.original_json).unwrap()])
        );
        assert_eq!(
            binding["liveUids"],
            serde_json::json!([self.uid.to_string()])
        );
        assert_eq!(
            binding["rootUids"],
            serde_json::json!([self.uid.to_string()])
        );
        (binding, digest)
    }
}

#[test]
#[ignore = "requires dedicated disposable loopback PostgreSQL and CREATE DATABASE authority"]
fn player_drop_mail_transfer_pg_public_send_freezes_original_receipt_and_claim_stays_closed() {
    let f = Fixture::new();
    let mut sender = relogin(f.config.clone(), SENDER, f.sender_index);
    let before_sender = stored(&f.db, SENDER, f.sender_index);
    let before_recipient = stored(&f.db, RECIPIENT, f.recipient_index);
    let before_floor = f
        .config
        .server_item_uid_allocator()
        .unwrap()
        .issued_through()
        .unwrap();
    let packets = send(&mut sender, f.uid);
    assert!(success(&packets));
    let charged = packets
        .iter()
        .find_map(|packet| match packet {
            ServerPacket::LoseGold { gold } => Some(*gold),
            _ => None,
        })
        .unwrap();
    assert!(charged >= SENT_GOLD);
    let after_sender = stored(&f.db, SENDER, f.sender_index);
    let mut expected_inventory = before_sender.inventory_items_json.clone();
    expected_inventory.retain(|encoded| encoded != &f.original_json);
    assert_eq!(after_sender.inventory_items_json, expected_inventory);
    assert_eq!(after_sender.gold, before_sender.gold - charged);
    let after_recipient = stored(&f.db, RECIPIENT, f.recipient_index);
    assert_eq!(
        after_recipient.inventory_items_json,
        before_recipient.inventory_items_json
    );
    assert_eq!(after_recipient.gold, before_recipient.gold);
    let mail = f.recipient_mail();
    assert_eq!(
        mail.attachment_origin,
        Stage5MailAttachmentOrigin::ExistingCustody
    );
    assert_eq!(mail.item_states_json, vec![f.original_json.clone()]);
    assert_eq!(mail.gold, SENT_GOLD);
    let receipt = f.assert_receipt(&mail);
    assert_eq!(
        f.config
            .server_item_uid_allocator()
            .unwrap()
            .issued_through()
            .unwrap(),
        before_floor
    );
    let mut recipient = relogin(f.config.clone(), RECIPIENT, f.recipient_index);
    let before_claim = image(&stored(&f.db, RECIPIENT, f.recipient_index));
    for _ in 0..2 {
        let packets = recipient
            .execute(WorldCommand::ClientPacket(ClientPacket::CollectParcel {
                mail_id: u64::from(mail.id),
            }))
            .unwrap();
        assert!(!packets.contains(&ServerPacket::ParcelCollected { result: 1 }));
        assert!(!packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::GainedGold { .. } | ServerPacket::GainedItem { .. }
        )));
        assert_eq!(
            image(&stored(&f.db, RECIPIENT, f.recipient_index)),
            before_claim
        );
    }
    for packet in [
        ClientPacket::ReadMail {
            mail_id: u64::from(mail.id),
        },
        ClientPacket::LockMail {
            mail_id: u64::from(mail.id),
            lock: true,
        },
        ClientPacket::LockMail {
            mail_id: u64::from(mail.id),
            lock: false,
        },
        ClientPacket::DeleteMail {
            mail_id: u64::from(mail.id),
        },
    ] {
        recipient
            .execute(WorldCommand::ClientPacket(packet))
            .unwrap();
        assert_eq!(f.assert_receipt(&mail), receipt);
    }
    let terminal = f.recipient_mail();
    assert!(terminal.opened && terminal.deleted && !terminal.claimed);
    assert_eq!(terminal.item_states_json, vec![f.original_json.clone()]);
    let mut sql = f.db.connect();
    for statement in [
        "UPDATE public.mail_item_transfer_receipts SET binding_digest=repeat('a',64) WHERE delivery_nonce=$1",
        "DELETE FROM public.mail_item_transfer_receipts WHERE delivery_nonce=$1",
    ] {
        let error = sql.execute(statement, &[&mail.delivery_nonce]).unwrap_err();
        assert!(error.as_db_error().unwrap().message().contains("immutable mail item transfer receipt"));
        assert_eq!(f.assert_receipt(&mail), receipt);
    }
    let logout = recipient
        .execute(WorldCommand::ClientPacket(ClientPacket::LogOut))
        .unwrap();
    assert!(logout
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LogOutSuccess { .. })));
    let deleted = recipient
        .execute(WorldCommand::ClientPacket(ClientPacket::DeleteCharacter {
            character_index: f.recipient_index,
        }))
        .unwrap();
    assert!(deleted.contains(&ServerPacket::DeleteCharacterSuccess {
        character_index: f.recipient_index
    }));
    assert_eq!(
        f.db.connect()
            .query_one(
                "SELECT count(*) FROM character_saves WHERE account_id=$1 AND character_index=$2",
                &[&RECIPIENT, &f.recipient_index]
            )
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    assert_eq!(
        f.assert_receipt(&mail),
        receipt,
        "character deletion must not cascade immutable transfer evidence"
    );
    assert_eq!(
        f.config
            .server_item_uid_allocator()
            .unwrap()
            .issued_through()
            .unwrap(),
        before_floor
    );
}

#[test]
#[ignore = "requires dedicated disposable loopback PostgreSQL and CREATE DATABASE authority"]
fn player_drop_mail_transfer_pg_failed_commit_freezes_and_fresh_binding_can_retry() {
    let f = Fixture::new();
    let mut sender = relogin(f.config.clone(), SENDER, f.sender_index);
    let before_sender = image(&stored(&f.db, SENDER, f.sender_index));
    let before_recipient = image(&stored(&f.db, RECIPIENT, f.recipient_index));
    let before_live = assets(&sender.active_character_checkpoint().unwrap());
    f.db.connect().batch_execute(
        "CREATE FUNCTION public.mail_fixture_reject_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'owned deferred mail commit rejection'; END $$; \
         CREATE CONSTRAINT TRIGGER mail_fixture_reject_commit AFTER UPDATE ON character_saves DEFERRABLE INITIALLY DEFERRED FOR EACH ROW WHEN (NEW.account_id='transfer-recipient') EXECUTE FUNCTION public.mail_fixture_reject_commit();"
    ).unwrap();
    let packets = send(&mut sender, f.uid);
    assert!(
        packets.is_empty(),
        "failed COMMIT must not report a known rejection"
    );
    assert!(f
        .config
        .save_account_store_account(SENDER)
        .unwrap_err()
        .contains("writes are frozen"));
    assert_eq!(image(&stored(&f.db, SENDER, f.sender_index)), before_sender);
    assert_eq!(
        image(&stored(&f.db, RECIPIENT, f.recipient_index)),
        before_recipient
    );
    assert_eq!(
        assets(&sender.active_character_checkpoint().unwrap()),
        before_live
    );
    assert_eq!(count_receipts(&f.db), 0);
    f.db.connect().batch_execute("DROP TRIGGER mail_fixture_reject_commit ON character_saves; DROP FUNCTION public.mail_fixture_reject_commit();").unwrap();
    assert!(send(&mut sender, f.uid).is_empty());
    // The original writer remains frozen. This independent test binding starts
    // only after direct canonical reads confirmed the failed transaction left
    // both snapshots and the receipt absent; this is not an automatic unfreeze.
    let recovered =
        f.db.config("owner-a", 7)
            .with_postgres_account_store(f.db.url())
            .unwrap()
            .with_postgres_item_uid_allocator()
            .unwrap();
    let mut sender = relogin(recovered, SENDER, f.sender_index);
    let packets = send(&mut sender, f.uid);
    assert!(success(&packets));
    let mail = f.recipient_mail();
    f.assert_receipt(&mail);
    assert_eq!(mail.item_states_json, vec![f.original_json.clone()]);
    assert!(!success(&send(&mut sender, f.uid)));
    assert_eq!(count_receipts(&f.db), 1);
    assert_eq!(f.recipient_mail(), mail);
}

#[test]
#[ignore = "requires dedicated disposable loopback PostgreSQL and CREATE DATABASE authority"]
fn player_drop_mail_transfer_pg_late_source_rejection_rolls_back_and_retries_without_freeze() {
    let f = Fixture::new();
    let mut sender = relogin(f.config.clone(), SENDER, f.sender_index);
    let before_sender = image(&stored(&f.db, SENDER, f.sender_index));
    let before_recipient = image(&stored(&f.db, RECIPIENT, f.recipient_index));
    f.db.connect().batch_execute(
        "CREATE FUNCTION public.mail_fixture_reject_source() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'owned late source rejection'; END $$; \
         CREATE TRIGGER mail_fixture_reject_source AFTER UPDATE ON character_saves FOR EACH ROW WHEN (NEW.account_id='transfer-recipient') EXECUTE FUNCTION public.mail_fixture_reject_source();"
    ).unwrap();
    assert_eq!(
        send(&mut sender, f.uid),
        vec![ServerPacket::MailSent { result: -1 }]
    );
    assert_eq!(image(&stored(&f.db, SENDER, f.sender_index)), before_sender);
    assert_eq!(
        image(&stored(&f.db, RECIPIENT, f.recipient_index)),
        before_recipient
    );
    assert_eq!(count_receipts(&f.db), 0);
    f.db.connect().batch_execute("DROP TRIGGER mail_fixture_reject_source ON character_saves; DROP FUNCTION public.mail_fixture_reject_source();").unwrap();
    assert!(success(&send(&mut sender, f.uid)));
    f.assert_receipt(&f.recipient_mail());
    assert!(!success(&send(&mut sender, f.uid)));
    assert_eq!(count_receipts(&f.db), 1);
}

#[test]
#[ignore = "requires dedicated disposable loopback PostgreSQL and CREATE DATABASE authority"]
fn player_drop_mail_transfer_pg_independent_runtime_competition_has_one_withdrawal_and_receipt() {
    let f = Fixture::new();
    let before_sender = stored(&f.db, SENDER, f.sender_index);
    let (ready_tx, ready_rx) = mpsc::channel();
    let mut workers = Vec::new();
    let mut both_ready = true;
    for ordinal in 0..2 {
        // StartGame itself publishes a source checkpoint. Finish each login
        // before loading the next independent cache, then race only SendMail.
        let config = if ordinal == 0 {
            f.config.clone()
        } else {
            let independent =
                f.db.config("owner-a", 7)
                    .with_postgres_account_store(f.db.url())
                    .unwrap()
                    .with_postgres_item_uid_allocator()
                    .unwrap();
            assert!(!Arc::ptr_eq(
                &independent.account_store,
                &f.config.account_store
            ));
            independent
        };
        let ready = ready_tx.clone();
        let (start_tx, start_rx) = mpsc::channel();
        let index = f.sender_index;
        let uid = f.uid;
        let worker =
            std::thread::spawn(move || {
                let mut runtime = relogin(config, SENDER, index);
                let checkpoint = runtime.active_character_checkpoint().unwrap();
                assert!(checkpoint.inventory_items_json.iter().any(
                    |encoded| serde_json::from_str::<Value>(encoded).unwrap()["unique_id"].as_u64()
                        == Some(uid)
                ));
                ready.send(()).unwrap();
                if start_rx.recv_timeout(Duration::from_secs(30)) != Ok(true) {
                    return Vec::new();
                }
                send(&mut runtime, uid)
            });
        workers.push((worker, start_tx));
        if ready_rx.recv_timeout(Duration::from_secs(10)).is_err() {
            both_ready = false;
            break;
        }
    }
    drop(ready_tx);
    for (_, start) in &workers {
        let _ = start.send(both_ready);
    }
    let joined: Vec<_> = workers
        .into_iter()
        .map(|(worker, _)| worker.join())
        .collect();
    assert!(
        both_ready,
        "both public runtimes must load the original instance before racing SendMail"
    );
    let outcomes: Vec<_> = joined
        .into_iter()
        .map(|result| result.expect("owned mail sender worker"))
        .collect();
    assert_eq!(
        outcomes.iter().filter(|packets| success(packets)).count(),
        1
    );
    let charged: Vec<_> = outcomes
        .iter()
        .flat_map(|packets| packets.iter())
        .filter_map(|packet| match packet {
            ServerPacket::LoseGold { gold } => Some(*gold),
            _ => None,
        })
        .collect();
    assert_eq!(charged.len(), 1);
    let after_sender = stored(&f.db, SENDER, f.sender_index);
    assert_eq!(after_sender.gold, before_sender.gold - charged[0]);
    let mut expected_inventory = before_sender.inventory_items_json;
    expected_inventory.retain(|encoded| encoded != &f.original_json);
    assert_eq!(after_sender.inventory_items_json, expected_inventory);
    let mail = f.recipient_mail();
    assert_eq!(mail.item_states_json, vec![f.original_json.clone()]);
    f.assert_receipt(&mail);
}
