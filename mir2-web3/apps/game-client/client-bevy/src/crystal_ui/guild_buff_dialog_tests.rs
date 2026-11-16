use super::*;

fn info(id: i32) -> GuildBuffInfo {
    GuildBuffInfo {
        id,
        icon: id * 3,
        name: format!("Buff {id}"),
        level_requirement: 2,
        points_requirement: 3,
        time_limit: 30,
        activation_cost: 100,
        stats: Vec::new(),
    }
}
fn authority() -> GuildBuffAuthority {
    GuildBuffAuthority {
        level: 3,
        spare_points: 5,
        gold: 200,
        may_activate: true,
    }
}

#[test]
fn ordinary_deltas_preserve_catalog_and_other_buffs() {
    let mut ui = GuildBuffDialog::default();
    ui.apply_packet(
        0,
        &[GuildBuff {
            id: 9,
            active: true,
            active_time_remaining: 12,
        }],
        &[info(9), info(12)],
    );
    ui.apply_packet(
        0,
        &[GuildBuff {
            id: 12,
            active: false,
            active_time_remaining: 0,
        }],
        &[],
    );
    assert_eq!(ui.catalog.len(), 2);
    assert_eq!(ui.enabled.len(), 2);
    ui.apply_packet(
        1,
        &[GuildBuff {
            id: 9,
            active: false,
            active_time_remaining: 0,
        }],
        &[],
    );
    assert_eq!(
        ui.enabled.iter().map(|b| b.id).collect::<Vec<_>>(),
        vec![12]
    );
    ui.apply_packet(
        1,
        &[GuildBuff {
            id: 99,
            active: false,
            active_time_remaining: 0,
        }],
        &[],
    );
    assert_eq!(ui.enabled.len(), 1);
}

#[test]
fn acquisition_activation_and_permissions_use_authority_without_optimism() {
    let mut ui = GuildBuffDialog::default();
    ui.apply_packet(0, &[], &[info(50)]);
    let mut denied = authority();
    denied.may_activate = false;
    denied.level = 0;
    denied.spare_points = 0;
    assert_eq!(
        ui.request_row(0, denied, 1_000),
        Err(GuildBuffError::GuildRankNoBuffActivation)
    );
    denied.may_activate = true;
    assert_eq!(
        ui.request_row(0, denied, 1_000),
        Err(GuildBuffError::GuildLevelTooLow)
    );
    denied.level = 3;
    assert_eq!(
        ui.request_row(0, denied, 1_000),
        Err(GuildBuffError::InsufficientPointsAvailable)
    );
    assert_eq!(
        ui.request_row(0, authority(), 1_000).unwrap(),
        Some(GuildBuffRequest { action: 1, id: 50 })
    );
    assert!(ui.enabled.is_empty());
    assert_eq!(ui.request_row(0, authority(), 1_099).unwrap(), None);
    ui.apply_packet(
        0,
        &[GuildBuff {
            id: 50,
            active: false,
            active_time_remaining: 0,
        }],
        &[],
    );
    assert_eq!(
        ui.request_row(0, authority(), 1_100).unwrap(),
        Some(GuildBuffRequest { action: 2, id: 50 })
    );
    ui.apply_packet(
        0,
        &[GuildBuff {
            id: 50,
            active: true,
            active_time_remaining: 30,
        }],
        &[],
    );
    assert_eq!(
        ui.request_row(0, authority(), 2_000),
        Err(GuildBuffError::BuffIsActive)
    );
    let mut poor = authority();
    poor.gold = 99;
    assert_eq!(
        ui.request_row(0, poor, 2_000),
        Err(GuildBuffError::GuildFundsInsufficient)
    );
}

#[test]
fn list_request_failure_retries_and_character_change_clears_authority() {
    let mut ui = GuildBuffDialog::default();
    let req = ui.request_list().unwrap();
    assert_eq!(req.action, 0);
    assert!(ui.request_list().is_none());
    ui.send_failed(req);
    assert!(ui.request_list().is_some());
    ui.apply_packet(
        0,
        &[GuildBuff {
            id: 1,
            active: true,
            active_time_remaining: 8,
        }],
        &[info(1)],
    );
    ui.reset();
    assert!(ui.enabled.is_empty());
    assert!(ui.catalog.is_empty());
    assert!(ui.request_list().is_some());
}

#[test]
fn source_row_states_use_guildskill_icon_triplets() {
    let mut ui = GuildBuffDialog::default();
    let mut permanent = info(4);
    permanent.time_limit = 0;
    ui.apply_packet(
        0,
        &[
            GuildBuff {
                id: 2,
                active: true,
                active_time_remaining: 9,
            },
            GuildBuff {
                id: 3,
                active: false,
                active_time_remaining: 0,
            },
            GuildBuff {
                id: 4,
                active: true,
                active_time_remaining: 0,
            },
        ],
        &[info(1), info(2), info(3), permanent],
    );
    let rows = ui.rows(1);
    assert_eq!(
        rows.iter()
            .map(|r| (r.status, r.icon, r.active))
            .collect::<Vec<_>>(),
        vec![
            (GuildBuffRowStatus::InsufficientLevel, 5, None),
            (GuildBuffRowStatus::CountingDown, 7, Some(true)),
            (GuildBuffRowStatus::Expired, 9, Some(false)),
            (GuildBuffRowStatus::Obtained, 13, Some(true)),
        ]
    );
    assert!(rows[0].warning_red);
    assert_eq!(ui.rows(3)[0].status, GuildBuffRowStatus::Available);
}

#[test]
fn scroll_drag_and_catalog_shrink_are_bounded_with_stable_ids() {
    let mut ui = GuildBuffDialog::default();
    ui.apply_packet(0, &[], &(10..22).map(info).collect::<Vec<_>>());
    ui.wheel(-360);
    assert_eq!(ui.start_index, 1);
    ui.drag_thumb(296, 20);
    assert_eq!(ui.start_index, 4);
    assert_eq!(ui.thumb_y(20), 296);
    assert_eq!(ui.rows(3)[0].id, 14);
    assert_eq!(ui.request_row(0, authority(), 500).unwrap().unwrap().id, 14);
    assert!(ui.request_row(8, authority(), 700).unwrap().is_none());
    ui.apply_packet(0, &[], &[info(99)]);
    assert_eq!(ui.start_index, 0);
    ui.drag_thumb(296, 20);
    assert_eq!(ui.start_index, 0);
    assert_eq!(ui.thumb_y(20), 39);
    assert_eq!(ui.rows(3).len(), 1);
}
