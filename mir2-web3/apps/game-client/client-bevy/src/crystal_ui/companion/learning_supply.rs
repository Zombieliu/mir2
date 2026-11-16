//! Teaching previews and one-order supply authorization. Text is never authority.
use super::*;
use crate::companion::{current_teaching_proposal_id, valid_companion_ref, CompanionOwnerView};

fn teaching_status(view: &CompanionOwnerView) -> &'static str {
    match view
        .runtime
        .pointer("/teachingProposal/status")
        .and_then(Value::as_str)
    {
        Some("planning") => "模型正在理解教学；当前习惯尚未改变。",
        Some("proposed") if current_teaching_proposal_id(view).is_some() => {
            "请核对下方改动，点击确认后才会保存。"
        }
        Some("proposed") => "提案或培养版本已变化，请刷新后重新生成。",
        Some("confirmed") => "教学已确认；请在下一次冒险观察实际行动。",
        Some("cancelled") => "这份提案已取消，没有生效。",
        Some("invalidated") => "记忆或培养版本已变化，旧提案已经失效。",
        Some("unsupported") => "这段教学暂不能转成现有的三项习惯，请重新表述。",
        Some("failed") => "未能生成教学提案；不会因此改变习惯。",
        _ => "先写下反馈，生成提案后再确认。",
    }
}

fn teaching_rows(view: &CompanionOwnerView) -> Vec<String> {
    let proposal = &view.runtime["teachingProposal"];
    let confirmed = proposal["status"].as_str() == Some("confirmed");
    if !confirmed && current_teaching_proposal_id(view).is_none() {
        return Vec::new();
    }
    let changes = &proposal["changes"];
    [
        (
            "assistBeforeLoot",
            "组队先支援",
            view.profile.party.assist_before_loot,
        ),
        (
            "lootOnlyAfterCombat",
            "战斗后再拾取",
            view.profile.party.loot_only_after_combat,
        ),
        (
            "askBeforeLeaving",
            "离队先询问",
            view.profile.party.ask_before_leaving,
        ),
    ]
    .into_iter()
    .filter_map(|(key, label, before)| {
        changes.get(key).and_then(Value::as_bool).map(|after| {
            if confirmed {
                format!("已保存 · {label}：{}", yes(after))
            } else {
                format!("{label}：{} → {}", yes(before), yes(after))
            }
        })
    })
    .collect()
}

pub(super) fn render_teaching(
    p: &mut ChildSpawnerCommands,
    f: &FontSource,
    ui: &mut CompanionUi,
    view: &CompanionOwnerView,
    enabled: bool,
) {
    text(p, f, "用自己的话教阿洛", 22., 181., 470., 28., 17., GOLD);
    button(
        p,
        f,
        ui,
        "返回培养",
        520.,
        180.,
        192.,
        Control::CloseTeaching,
        true,
    );
    text(
        p,
        f,
        "支持支援、拾取与离队习惯。生成会调用模型；确认前不生效，也不会扩大花费权限。",
        22.,
        216.,
        690.,
        27.,
        12.,
        MUTED,
    );
    ui.focus_actions.push(Control::FocusInstruction);
    p.spawn((
        Name::new("自然语言教学输入"),
        InstructionField,
        Button,
        Control::FocusInstruction,
        Node {
            position_type: PositionType::Absolute,
            left: px(22.),
            top: px(249.),
            width: px(690.),
            height: px(65.),
            padding: UiRect::all(px(8.)),
            border: UiRect::all(px(1.)),
            overflow: Overflow::clip(),
            ..default()
        },
        BackgroundColor(Color::srgb(0.09, 0.075, 0.052)),
        BorderColor::all(if ui.instruction.focused { GOLD } else { MUTED }),
    ))
    .with_children(|p| {
        p.spawn((
            Text::new(instruction_preview(ui)),
            TextFont {
                font: f.clone(),
                font_size: FontSize::Px(15.),
                ..default()
            },
            TextColor(INK),
        ));
    });
    let proposal = &view.runtime["teachingProposal"];
    let planning = proposal["status"].as_str() == Some("planning");
    button(
        p,
        f,
        ui,
        "生成教学提案 · Enter",
        22.,
        323.,
        334.,
        Control::GenerateTeaching,
        enabled
            && model_ready(&view.runtime)
            && !planning
            && !ui.instruction.editor.text().trim().is_empty(),
    );
    let cancel_id = proposal["proposalId"]
        .as_str()
        .filter(|id| valid_companion_ref(id));
    button(
        p,
        f,
        ui,
        "取消当前提案",
        368.,
        323.,
        344.,
        Control::CancelTeaching(cancel_id.unwrap_or_default().to_owned()),
        enabled
            && cancel_id.is_some()
            && matches!(proposal["status"].as_str(), Some("planning" | "proposed")),
    );
    text(
        p,
        f,
        if model_ready(&view.runtime) {
            teaching_status(view)
        } else {
            "请先打开模型设置并启用真实模型；原有固定教学仍可使用。"
        },
        22.,
        365.,
        690.,
        30.,
        14.,
        GOLD,
    );
    if let Some(reply) = proposal["reply"].as_str() {
        text(
            p,
            f,
            &bounded_display(reply, 500),
            22.,
            398.,
            690.,
            40.,
            13.,
            INK,
        );
    }
    for (index, row) in teaching_rows(view).iter().enumerate() {
        text(
            p,
            f,
            row,
            22.,
            439. + index as f32 * 23.,
            690.,
            23.,
            14.,
            INK,
        );
    }
    let confirm_id = current_teaching_proposal_id(view);
    button(
        p,
        f,
        ui,
        "确认这些习惯改动",
        22.,
        515.,
        334.,
        Control::ConfirmTeaching(confirm_id.unwrap_or_default().to_owned()),
        enabled && confirm_id.is_some(),
    );
    let hint = if ui.instruction.notice.is_empty() {
        format!(
            "{} / 1000 字符 · 中文候选词确认后再提交。",
            ui.instruction.editor.text().chars().count()
        )
    } else {
        ui.instruction.notice.clone()
    };
    text(p, f, &hint, 368., 515., 344., 29., 12., MUTED);
}

fn supply_complete(task: &Value) -> bool {
    task["status"].as_str() == Some("completed")
        && task["receiptKnown"].as_bool() == Some(true)
        && task["returnedToAnchor"].as_bool() == Some(true)
        && task["acquiredQuantity"].as_u64() == Some(1)
        && task["budgetGold"].as_u64() == Some(50)
        && task["chargedGold"].as_u64().is_some_and(|n| n <= 50)
}

fn supply_status(task: &Value) -> &'static str {
    match task["status"].as_str() {
        Some("authorized") => "服务器已授权；购买与返程尚未完成。",
        Some("running") => "正在执行补给；实际扣款和入药以服务器回执为准。",
        Some("paused") => "补给已暂停；不会自动恢复支出。",
        Some("unknown") => "购买结果未知；仅核对原订单，不会重复下单。",
        Some("returning") => "正在返程；到达授权点旁前不显示完成。",
        Some("completed") if supply_complete(task) => "补给完成：购买回执和实际返程均已确认。",
        Some("completed") => "完成证据不足，请刷新核对实际购买与返程。",
        Some("cancelled") => "补给已撤销；已提交购买的结果仍需核对，不会自动退款。",
        Some("failed") => "本次补给未完成，请核对原因和已有购买回执。",
        Some("idle") | None => "暂无独立补给任务。",
        _ => "补给状态暂未分类，请刷新记录。",
    }
}

fn supply_step(task: &Value) -> &'static str {
    match task["step"].as_str() {
        Some("travel" | "travelling") => "前往杂货商",
        Some("open-shop" | "opening-service") => "打开当前商店服务",
        Some("quote" | "quoted") => "核对服务器报价",
        Some("purchase-pending") => "等待原购买订单结果",
        Some("reconcile") => "核对原订单与当前背包",
        Some("return" | "returning") => "回到授权点旁",
        Some("completed") => "本次补给已结束",
        _ => "等待实际行动记录",
    }
}

fn supply_reason(task: &Value) -> String {
    let reason = task["lastError"]
        .as_str()
        .or_else(|| task["reason"].as_str())
        .unwrap_or_default();
    match reason {
        "insufficient-gold" | "insufficient-funds" | "not-enough-gold" => {
            "游戏金币不足，未完成购买。".into()
        }
        "budget-exceeded" | "over-budget" => "真实报价超过本次预算，未获准购买。".into(),
        "expired" | "permit-expired" => "补给授权已到期，停止新的支出。".into(),
        "revoked" | "permit-revoked" => "补给授权已撤销，停止新的支出。".into(),
        "inventory-full" | "overweight" => "背包空间或负重不足，未完成购买。".into(),
        "" => String::new(),
        other => format!("补给记录：{}", bounded_display(other, 350)),
    }
}

pub(super) fn render_supply(
    p: &mut ChildSpawnerCommands,
    f: &FontSource,
    ui: &mut CompanionUi,
    view: &CompanionOwnerView,
    enabled: bool,
) {
    let task = &view.runtime["supplyTask"];
    text(
        p,
        f,
        "独立补给 · 单次受限采购",
        22.,
        181.,
        690.,
        28.,
        17.,
        GOLD,
    );
    text(p, f, "目的地：比奇杂货商鲁本。购买 1 瓶小型生命药水，再回到授权点旁。\n最多 50 游戏金币，权限 5 分钟有效。禁止拾取、出售、交易和换图。",
        22., 217., 690., 60., 14., INK);
    let active = matches!(
        task["status"].as_str(),
        Some("authorized" | "running" | "paused" | "unknown" | "returning")
    );
    button(
        p,
        f,
        ui,
        "查看并确认这次授权",
        22.,
        286.,
        334.,
        Control::PrepareSupply,
        enabled && !active,
    );
    button(
        p,
        f,
        ui,
        "撤销本次补给",
        368.,
        286.,
        344.,
        Control::CancelSupply,
        enabled && active,
    );
    text(p, f, supply_status(task), 22., 330., 690., 44., 15., GOLD);
    text(
        p,
        f,
        &format!("当前步骤：{}", supply_step(task)),
        22.,
        377.,
        690.,
        25.,
        14.,
        INK,
    );
    let receipt = task["receiptKnown"].as_bool() == Some(true);
    let charged = if receipt {
        task["chargedGold"]
            .as_u64()
            .map(|n| format!("实际扣款：{n} 游戏金币"))
    } else {
        None
    }
    .unwrap_or_else(|| "实际扣款：尚未确认".into());
    let acquired = if receipt {
        task["acquiredQuantity"]
            .as_u64()
            .map(|n| format!("实际入药：{n} 瓶"))
    } else {
        None
    }
    .unwrap_or_else(|| "实际入药：尚未确认".into());
    text(
        p,
        f,
        &format!(
            "{charged} · {acquired}\n返程到达：{}",
            if task["returnedToAnchor"].as_bool() == Some(true) {
                "已确认到达授权点旁"
            } else {
                "尚未确认"
            }
        ),
        22.,
        409.,
        690.,
        49.,
        14.,
        INK,
    );
    text(p, f, &supply_reason(task), 22., 466., 690., 39., 13., MUTED);
    button(
        p,
        f,
        ui,
        "刷新服务器补给记录",
        22.,
        515.,
        690.,
        Control::RefreshTask,
        true,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn complete_supply_requires_actual_receipt_quantity_cost_and_anchor_arrival() {
        let task = json!({"status":"completed","budgetGold":50,"chargedGold":40,
            "receiptKnown":true,"returnedToAnchor":true,"acquiredQuantity":1});
        assert!(supply_status(&task).starts_with("补给完成："));
        for (key, value) in [
            ("receiptKnown", json!(false)),
            ("returnedToAnchor", json!(false)),
            ("acquiredQuantity", json!(0)),
            ("chargedGold", Value::Null),
            ("chargedGold", json!(51)),
            ("budgetGold", Value::Null),
        ] {
            let mut incomplete = task.clone();
            incomplete[key] = value;
            assert!(
                !supply_status(&incomplete).starts_with("补给完成："),
                "missing/mismatched {key}"
            );
        }
    }
}
