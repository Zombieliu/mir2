//! Owner-confirmed life sessions. Resources and outcomes are observations only.
use super::*;
use crate::companion::{valid_companion_ref, CompanionOwnerView};

const SAFE_INTEGER: u64 = 9_007_199_254_740_991;
fn number(value: &Value) -> Option<u64> {
    value.as_u64().filter(|n| *n <= SAFE_INTEGER)
}
fn shown(value: &Value) -> String {
    number(value).map_or_else(|| "待核对".into(), |n| n.to_string())
}
fn unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |t| t.as_millis().min(u128::from(u64::MAX)) as u64)
}
pub(super) fn duration(ui: &CompanionUi) -> u16 {
    if ui.life_duration_seconds == 1800 {
        1800
    } else {
        1200
    }
}
fn unresolved(view: &CompanionOwnerView) -> bool {
    view.runtime
        .pointer("/resources/medicineUnresolved")
        .and_then(Value::as_bool)
        != Some(false)
        || number(&view.runtime["lifeTask"]["reservedGold"]).is_none_or(|n| n > 0)
}
fn gift_ready(view: &CompanionOwnerView) -> bool {
    let gift = &view.runtime["giftTask"];
    gift.is_null()
        || matches!(
            gift["status"].as_str(),
            Some("idle" | "completed" | "cancelled")
        )
}
pub(super) fn can_authorize(view: &CompanionOwnerView) -> bool {
    let task = &view.runtime["lifeTask"];
    if !gift_ready(view)
        || view
            .runtime
            .pointer("/resources/medicineUnresolved")
            .and_then(Value::as_bool)
            == Some(true)
    {
        return false;
    }
    let supply = view
        .runtime
        .pointer("/supplyTask/status")
        .and_then(Value::as_str);
    if supply.is_some_and(|s| !matches!(s, "idle" | "completed" | "cancelled" | "failed")) {
        return false;
    }
    task.is_null()
        || task["status"].as_str() == Some("idle")
        || matches!(
            task["status"].as_str(),
            Some("ended" | "completed" | "cancelled" | "failed" | "expired")
        ) && !unresolved(view)
}
pub(super) fn resume_id(view: &CompanionOwnerView) -> Option<&str> {
    let task = &view.runtime["lifeTask"];
    if !gift_ready(view)
        || task["status"].as_str() != Some("paused")
        || number(&task["budgetGold"]) != Some(50)
        || number(&task["maxPurchasedQuantity"]) != Some(1)
        || number(&task["expiresAtMs"]).is_none_or(|expiry| unix_ms() == 0 || expiry <= unix_ms())
    {
        return None;
    }
    task["lifePermitId"]
        .as_str()
        .filter(|id| valid_companion_ref(id))
}
pub(super) fn can_stop(view: &CompanionOwnerView) -> bool {
    !view.runtime["lifeTask"].is_null()
        && !matches!(
            view.runtime["lifeTask"]["status"].as_str(),
            Some("idle" | "ended" | "completed" | "cancelled" | "failed" | "expired")
        )
}
fn phase_label(task: &Value) -> &'static str {
    match task["status"].as_str() {
        None if task.is_null() => return "待命",
        Some("idle") => return "待命",
        Some("paused") => return "暂停",
        Some("unknown") => return "暂停 · 原购买结果待核对",
        Some("ended" | "completed" | "cancelled" | "expired" | "failed") => return "结束",
        _ => {}
    }
    match task["phase"].as_str() {
        Some("explore" | "exploring" | "exploration" | "patrolling") => "探索",
        Some("combat" | "fight" | "fighting" | "hunt" | "hunting") => "战斗",
        Some("return" | "returning" | "return-home" | "returning-home" | "retreat") => "回城",
        Some(
            "supply" | "purchase" | "resupply" | "buying" | "awaiting-supply" | "claiming-supply"
            | "supplying",
        ) => "补给",
        Some("rest" | "resting" | "recovering") => "休息",
        Some("awaiting-owner") => "等待主人补给",
        Some("awaiting-observation") => "等待资源核对",
        Some("awaiting-server-activation") => "等待服务器激活",
        Some("idle" | "wait" | "waiting" | "standby") => "待命",
        Some("paused") => "暂停",
        Some("ended" | "completed" | "finished") => "结束",
        _ => "阶段待核对",
    }
}
fn status_line(view: &CompanionOwnerView) -> String {
    let task = &view.runtime["lifeTask"];
    if matches!(
        task["status"].as_str(),
        Some("ended" | "completed" | "cancelled" | "expired")
    ) && unresolved(view)
    {
        "自主生活 · 已停止，购药结果仍待核对".into()
    } else {
        format!("自主生活 · {}", phase_label(task))
    }
}
fn expiry(task: &Value) -> String {
    let Some(at) = number(&task["expiresAtMs"]) else {
        return "有效期待核对".into();
    };
    let now = unix_ms();
    if now == 0 {
        return "本机时间未确认".into();
    }
    if at <= now {
        "授权已到期".into()
    } else {
        format!("约余 {} 分钟（本机估算）", (at - now).div_ceil(60_000))
    }
}
fn budget_lines(task: &Value) -> String {
    if task.is_null() || task["status"].as_str() == Some("idle") {
        return "本会话采购额度：尚未授权；累计上限 50 游戏金币 / 1 瓶。".into();
    }
    format!(
        "采购额度 {} · 剩余 {} · 预留 {} · 实扣 {} 游戏金币\n已购 {} / {} 瓶 · 观察击杀 {} · {}",
        shown(&task["budgetGold"]),
        shown(&task["remainingBudgetGold"]),
        shown(&task["reservedGold"]),
        shown(&task["chargedGold"]),
        shown(&task["purchasedQuantity"]),
        shown(&task["maxPurchasedQuantity"]),
        shown(&task["killsObserved"]),
        expiry(task)
    )
}
fn resource_lines(resources: &Value) -> String {
    if resources["known"].as_bool() != Some(true) || number(&resources["observedAtMs"]).is_none() {
        return "阿洛自己的金币：待核对 · 血量：待核对\n血药与可用药量：待核对 · 背包空位与负重：待核对\n资源观察尚未确认，请刷新；不会把未知数量显示为 0。".into();
    }
    let inventory = resources["inventoryCurrent"].as_bool() == Some(true);
    let inventory_value = |key: &str| {
        if inventory {
            shown(&resources[key])
        } else {
            "待核对".into()
        }
    };
    format!("阿洛自己的金币：{} · 血量 {}/{}\n血药 {}（可用 {}）· 背包空位 {} · 负重 {}/{}\n背包{} · 资源观察：{}",
        shown(&resources["gold"]), shown(&resources["hp"]), shown(&resources["maxHp"]),
        inventory_value("hpPotionCount"), inventory_value("usableHpPotionCount"), inventory_value("freeBagSlots"), inventory_value("currentWeight"), inventory_value("maxWeight"),
        if inventory { "已核对" } else { "待核对" }, short_time(number(&resources["observedAtMs"])))
}
fn model_label(task: &Value) -> &'static str {
    if task["status"].as_str() == Some("paused") {
        return "暂停";
    }
    if task["activationPending"].as_bool() == Some(true) {
        return "等待服务器激活";
    }
    match task["modelStatus"]
        .as_str()
        .or_else(|| task["decisionMode"].as_str())
    {
        Some("rules") => "规则执行",
        Some("planning" | "thinking" | "model-planning") => "规划中",
        Some("model") => "已有模型计划",
        Some("running" | "active") => "执行中",
        Some("paused") => "暂停",
        Some("disabled" | "off") => "未启用模型",
        Some("failed") => "暂不可用",
        Some("idle") => "待命",
        _ => "待核对",
    }
}
fn life_note(task: &Value) -> String {
    let reason = task["pausedReason"]
        .as_str()
        .filter(|s| !s.is_empty())
        .or_else(|| task["reason"].as_str())
        .unwrap_or("暂无已确认的行动说明。");
    // Model replies are intentions. Only observed counters become an activity
    // summary; absent counters stay unknown, including an absent charge.
    let summary =
        if number(&task["killsObserved"]).is_none() && number(&task["chargedGold"]).is_none() {
            "暂无已核对的击杀或消费统计。".into()
        } else {
            format!(
                "已观察击杀 {} · 实扣 {} 游戏金币",
                shown(&task["killsObserved"]),
                shown(&task["chargedGold"])
            )
        };
    // Three independent lines fit the 57px box. Keeping the reason separate
    // from its model label avoids four wrapped lines at the largest CJK width.
    format!(
        "模型：{}\n{}\n{}",
        model_label(task),
        bounded_display(reason, 45),
        bounded_display(&summary, 45)
    )
}
pub(super) fn confirmation_message(control: &Control) -> Option<String> {
    match control {
        Control::AuthorizeLife { duration_seconds, .. } => Some(format!(
            "授权阿洛独立生活 {} 分钟：可在比奇地图 0 的 x270–310、y595–645 内探索、战斗、自保、回城（288,616）、补给与休息，独立于主人的视野。本次会话累计最多花费 50 游戏金币，只能向鲁本购买 1 瓶小型生命药水；阿洛自己的金币另计。随时停止或召回；已提交或未知购买仍须核对。",
            duration_seconds / 60)),
        Control::ResumeLife { .. } => Some("恢复这份原生活授权：沿用原到期时间、累计 50 游戏金币与最多 1 瓶的额度。不会延长时间、补回已花或已预留的额度，也不会重买未知订单；已提交结果先按原操作核对。".into()),
        _ => None,
    }
}
pub(super) fn render(
    p: &mut ChildSpawnerCommands,
    f: &FontSource,
    ui: &mut CompanionUi,
    view: &CompanionOwnerView,
    enabled: bool,
) {
    let task = &view.runtime["lifeTask"];
    text(p, f, &status_line(view), 22., 180., 690., 28., 17., GOLD);
    for (index, seconds) in [1200_u16, 1800].into_iter().enumerate() {
        let label = format!(
            "{} 分钟{}",
            seconds / 60,
            if duration(ui) == seconds {
                " · 已选"
            } else {
                ""
            }
        );
        button(
            p,
            f,
            ui,
            &label,
            22. + index as f32 * 120.,
            214.,
            110.,
            Control::LifeDuration(seconds),
            enabled,
        );
    }
    button(
        p,
        f,
        ui,
        "查看并确认生活授权",
        262.,
        214.,
        212.,
        Control::PrepareLife,
        enabled && can_authorize(view),
    );
    button(
        p,
        f,
        ui,
        "停止生活",
        486.,
        214.,
        108.,
        Control::CancelLife,
        enabled && can_stop(view),
    );
    button(p, f, ui, "召回", 604., 214., 108., Control::Recall, enabled);
    text(p,f,"单会话累计最多 50 游戏金币 / 1 瓶血药；可独立于主人视野行动。\n范围：地图0 x270–310、y595–645；回城点（288,616）。",22.,251.,690.,38.,13.,MUTED);
    text(p, f, &budget_lines(task), 22., 292., 690., 40., 14., INK);
    text(
        p,
        f,
        &resource_lines(&view.runtime["resources"]),
        22.,
        335.,
        690.,
        51.,
        13.,
        INK,
    );
    text(p,f,"靠近并面对阿洛，用原交易窗口赠送补给；阿洛只接收，不付出。\n采购额度按原授权记录，金币与药品按阿洛自己的实际资源显示。",22.,391.,690.,35.,12.,MUTED);
    text(p, f, &life_note(task), 22., 431., 690., 57., 12., MUTED);
    button(
        p,
        f,
        ui,
        "确认恢复原生活授权",
        22.,
        494.,
        334.,
        Control::PrepareResumeLife,
        enabled && resume_id(view).is_some(),
    );
    button(
        p,
        f,
        ui,
        "刷新生活与资源",
        368.,
        494.,
        344.,
        Control::RefreshTask,
        enabled,
    );
    let warning = match view
        .runtime
        .pointer("/resources/medicineUnresolved")
        .and_then(Value::as_bool)
    {
        Some(true) => "购药已提交或结果未知：只核对原订单，尚未算作完成。",
        Some(false) => "行动、击杀与消费均来自实际观察；发送授权不等于行动已完成。",
        None => "购药结果待核对；未知金额与数量保持待确认。",
    };
    text(p, f, warning, 22., 529., 690., 23., 12., GOLD);
}

#[cfg(test)]
mod tests {
    use super::*;
    fn owner_view() -> CompanionOwnerView {
        serde_json::from_value(json!({
            "archive":{"archiveId":"a","residentId":"r","displayName":"Alo","characterName":"Alo","realmId":"test","worldEpoch":"w"},
            "profile":{"policyVersion":1,"style":"collection","party":{"assistBeforeLoot":false,"lootOnlyAfterCombat":false,"askBeforeLeaving":false}},
            "growthRevision":0,"runtime":{"resources":{"medicineUnresolved":false},"lifeTask":{"status":"idle"}}
        })).unwrap()
    }
    #[test]
    fn missing_or_stale_resources_are_not_zero_and_wallet_is_separate_from_budget() {
        assert!(resource_lines(&Value::Null).contains("金币：待核对"));
        let mut resources = json!({"known":true,"observedAtMs":1,"gold":7,"hp":10,"maxHp":18,
            "inventoryCurrent":true,"hpPotionCount":2,"usableHpPotionCount":1,"freeBagSlots":8,"currentWeight":3,"maxWeight":50});
        assert!(resource_lines(&resources).contains("金币：7"));
        assert!(resource_lines(&resources).contains("血药 2（可用 1）"));
        resources["inventoryCurrent"] = json!(false);
        assert!(resource_lines(&resources).contains("血药 待核对"));
        assert!(!resource_lines(&resources).contains("血药 0"));
        assert!(
            budget_lines(&json!({"status":"running","budgetGold":50,"chargedGold":null}))
                .contains("实扣 待核对")
        );
    }
    #[test]
    fn unknown_phase_never_becomes_completed_and_pending_medicine_stays_unresolved() {
        assert_eq!(
            phase_label(&json!({"status":"future","phase":"future"})),
            "阶段待核对"
        );
        let mut view:CompanionOwnerView=serde_json::from_value(json!({"archive":{"archiveId":"a","residentId":"r","displayName":"Alo","characterName":"Alo","realmId":"test","worldEpoch":"w"},
            "profile":{"policyVersion":1,"style":"collection","party":{"assistBeforeLoot":false,"lootOnlyAfterCombat":false,"askBeforeLeaving":false}},"growthRevision":0,
            "runtime":{"lifeTask":{"status":"completed","reservedGold":50},"resources":{"medicineUnresolved":true}}})).unwrap();
        assert!(status_line(&view).contains("仍待核对"));
        assert!(!can_authorize(&view));
        view.runtime["lifeTask"]["reservedGold"] = json!(0);
        view.runtime["resources"]["medicineUnresolved"] = json!(false);
        assert!(can_authorize(&view));
    }
    #[test]
    fn runtime_decision_modes_and_both_supply_handshake_phases_have_explicit_labels() {
        for (mode, expected) in [
            ("rules", "规则执行"),
            ("model-planning", "规划中"),
            ("model", "已有模型计划"),
            ("future-mode", "待核对"),
        ] {
            assert_eq!(model_label(&json!({"decisionMode":mode})), expected);
        }
        assert_eq!(
            model_label(&json!({"activationPending":true,"decisionMode":"rules"})),
            "等待服务器激活"
        );
        assert_eq!(
            model_label(
                &json!({"status":"paused","activationPending":true,"decisionMode":"model"})
            ),
            "暂停"
        );
        for phase in ["awaiting-supply", "claiming-supply", "supplying"] {
            assert_eq!(
                phase_label(&json!({"status":"running","phase":phase})),
                "补给"
            );
        }
    }
    #[test]
    fn notes_fit_three_bounded_lines_and_use_observed_counts_instead_of_model_prose() {
        let note = life_note(
            &json!({"decisionMode":"model","reason":"长".repeat(100),"killsObserved":3,"chargedGold":40,
            "reply":"模型声称已经买到十瓶药","summary":"没有观察依据的自由摘要"}),
        );
        let lines: Vec<_> = note.lines().collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "模型：已有模型计划");
        assert_eq!(lines[1].chars().count(), 45);
        assert!(lines.iter().all(|line| line.chars().count() <= 45));
        assert!(lines[2].contains("已观察击杀 3"));
        assert!(lines[2].contains("实扣 40 游戏金币"));
        assert!(!note.contains("十瓶药"));
        assert!(!note.contains("自由摘要"));
        let unknown =
            life_note(&json!({"decisionMode":"rules","killsObserved":0,"chargedGold":null}));
        assert!(unknown.contains("实扣 待核对"));
        assert!(!unknown.contains("实扣 0"));
    }
    #[test]
    fn unresolved_gifts_disable_new_life_and_original_life_resume() {
        let mut view = owner_view();
        for status in [
            "accepting",
            "receiving",
            "unknown",
            "paused",
            "future-status",
        ] {
            view.runtime["giftTask"] = json!({"status":status});
            view.runtime["lifeTask"] = json!({"status":"idle"});
            assert!(!can_authorize(&view));
            view.runtime["lifeTask"] = json!({"status":"paused","lifePermitId":"life:old","budgetGold":50,
                "maxPurchasedQuantity":1,"expiresAtMs":unix_ms()+60_000});
            assert!(resume_id(&view).is_none());
        }
        for status in ["idle", "completed", "cancelled"] {
            view.runtime["giftTask"] = json!({"status":status});
            assert_eq!(resume_id(&view), Some("life:old"));
            view.runtime["lifeTask"] = json!({"status":"idle"});
            assert!(can_authorize(&view));
            view.runtime["lifeTask"] = json!({"status":"paused","lifePermitId":"life:old","budgetGold":50,
                "maxPurchasedQuantity":1,"expiresAtMs":unix_ms()+60_000});
        }
    }
}
