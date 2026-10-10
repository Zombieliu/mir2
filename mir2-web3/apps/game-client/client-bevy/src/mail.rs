//! Authoritative mail model shared by every host.

//! Mirrors the simulation's `Stage5MailMessage` / `ClientMail` so the native
//! Windows Mail panel shows the same inbox as the browser. UI selection is kept
//! alongside the authoritative list but is not overwritten by the server.

use crate::inventory::{CrystalItemInfoModel, CrystalItemTooltipSourceModel, CrystalUserItemModel};
use bevy::prelude::Resource;
use serde::de::{self, SeqAccess, Visitor};
use serde::{Deserialize, Serialize};
use std::fmt;

pub const MAX_MAIL_ATTACHMENTS: usize = 5;
pub const MAX_MAIL_MESSAGES: usize = 256;
pub const MAIL_PAGE_SIZE: usize = 10;

fn deserialize_bounded<'de, D, T, const MAX: usize>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: de::Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct BoundedVisitor<T, const MAX: usize>(std::marker::PhantomData<T>);

    impl<'de, T, const MAX: usize> Visitor<'de> for BoundedVisitor<T, MAX>
    where
        T: Deserialize<'de>,
    {
        type Value = Vec<T>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(formatter, "a sequence with at most {MAX} retained entries")
        }

        fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let mut values = Vec::with_capacity(MAX.min(sequence.size_hint().unwrap_or(0)));
            while let Some(value) = sequence.next_element::<T>()? {
                if values.len() < MAX {
                    values.push(value);
                }
            }
            Ok(values)
        }
    }

    deserializer.deserialize_seq(BoundedVisitor::<T, MAX>(std::marker::PhantomData))
}

fn deserialize_mail_attachments<'de, D>(deserializer: D) -> Result<Vec<MailAttachment>, D::Error>
where
    D: de::Deserializer<'de>,
{
    deserialize_bounded::<D, MailAttachment, MAX_MAIL_ATTACHMENTS>(deserializer)
}

fn deserialize_mail_messages<'de, D>(deserializer: D) -> Result<Vec<MailMessage>, D::Error>
where
    D: de::Deserializer<'de>,
{
    // The native bridge appends one transient operation receipt after the
    // mailbox. It must not compete with real mail for the 256-row limit:
    // dropping that receipt would leave a full inbox's operation pending.
    struct MailMessagesVisitor;
    impl<'de> Visitor<'de> for MailMessagesVisitor {
        type Value = Vec<MailMessage>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a bounded mailbox with at most one operation receipt")
        }

        fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let mut rows = Vec::with_capacity(MAX_MAIL_MESSAGES + 1);
            let mut messages = 0;
            let mut has_feedback = false;
            while let Some(row) = sequence.next_element::<MailMessage>()? {
                if row.operation.is_some() {
                    if !has_feedback {
                        rows.push(row);
                        has_feedback = true;
                    }
                } else if messages < MAX_MAIL_MESSAGES {
                    rows.push(row);
                    messages += 1;
                }
            }
            Ok(rows)
        }
    }
    deserializer.deserialize_seq(MailMessagesVisitor)
}

/// A server-owned mail attachment. `unique_id` is optional only for legacy
/// Stage5 snapshots that contain display names instead of a wire UserItem;
/// packet-originated ClientMail keeps the concrete id and metadata intact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct MailAttachment {
    pub image: Option<u32>,
    pub unique_id: Option<u64>,
    pub item_index: Option<i32>,
    pub key: Option<String>,
    pub name: Option<String>,
    pub count: u16,
    pub current_dura: u16,
    pub max_dura: u16,
    pub soul_bound_id: i32,
    pub identified: bool,
    pub cursed: bool,
    pub gem_count: u16,
    pub tooltip_source: Option<CrystalItemTooltipSourceModel>,
}

impl MailAttachment {
    pub fn is_monthly_card(&self) -> bool {
        // Identity comes from the reserved index/key, never a player's free
        // text that happens to resemble a shop item name.
        self.item_index == Some(mir2_game_data::BILLING_MONTHLY_CARD_ITEM_INDEX)
            || (self.item_index.is_none() && self.key.as_deref() == Some("crystal-item-1000001"))
    }

    pub fn image_index(&self) -> Option<u16> {
        self.tooltip_source
            .as_ref()
            .map(|source| source.user_item_image(u32::from(self.count)))
            .or_else(|| self.image.and_then(|image| u16::try_from(image).ok()))
            .or_else(|| {
                self.is_monthly_card()
                    .then(|| mir2_game_data::billing_monthly_card_item_template().image)
            })
    }

    pub fn display_tooltip_source(&self) -> Option<CrystalItemTooltipSourceModel> {
        self.tooltip_source.clone().or_else(|| {
            if !self.is_monthly_card() {
                return None;
            }
            let info: CrystalItemInfoModel = serde_json::from_value(
                serde_json::to_value(mir2_game_data::billing_monthly_card_item_template()).ok()?,
            )
            .ok()?;
            Some(CrystalItemTooltipSourceModel {
                real_info: Some(info.clone()),
                info,
                user_item: self.unique_id.map(|unique_id| CrystalUserItemModel {
                    unique_id,
                    item_index: mir2_game_data::BILLING_MONTHLY_CARD_ITEM_INDEX,
                    count: self.count,
                    current_dura: self.current_dura,
                    max_dura: self.max_dura,
                    soul_bound_id: self.soul_bound_id,
                    identified: self.identified,
                    cursed: self.cursed,
                    gem_count: self.gem_count,
                    ..Default::default()
                }),
                ..Default::default()
            })
        })
    }

    pub fn label(&self) -> String {
        if self.is_monthly_card() {
            return crate::native_i18n::for_locale(
                crate::native_i18n::locale(),
                "billing.monthlyCardItemName",
                mir2_game_data::BILLING_MONTHLY_CARD_ITEM_NAME,
            );
        }
        self.name
            .as_deref()
            .filter(|name| !name.trim().is_empty())
            .or(self.key.as_deref().filter(|key| !key.trim().is_empty()))
            .map(str::to_owned)
            .or_else(|| self.item_index.map(|index| format!("Item #{index}")))
            .unwrap_or_else(|| "Item".to_owned())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MailOperationKind {
    Send,
    Read,
    Collect,
    Delete,
}

/// One-shot result transported through the existing mail model channel. The
/// runtime keeps this transient row long enough for pending reconciliation;
/// the overlay consumes and hides it from the inbox.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MailOperationFeedback {
    pub kind: MailOperationKind,
    pub success: bool,
    pub mail_id: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct MailMessage {
    #[serde(alias = "mailId")]
    pub id: u64,
    #[serde(alias = "from", alias = "senderName")]
    pub sender: String,
    #[serde(alias = "canReply")]
    pub can_reply: bool,
    #[serde(alias = "dateSentBinaryDatetime")]
    pub date_sent_binary_datetime: i64,
    pub metadata_known: bool,
    pub subject: String,
    pub body: String,
    pub gold: u32,
    #[serde(default, deserialize_with = "deserialize_mail_attachments")]
    pub items: Vec<MailAttachment>,
    #[serde(default)]
    pub operation: Option<MailOperationFeedback>,
    pub claimed: bool,
    pub locked: bool,
    #[serde(alias = "opened")]
    pub read: bool,
}

impl MailMessage {
    pub fn display_body(&self) -> String {
        self.display_body_for_locale(crate::native_i18n::locale())
    }

    fn display_body_for_locale(&self, language: crate::native_i18n::Locale) -> String {
        let body = self.body.replace("\\r\\n", "\r\n");
        let quantity: u32 = self
            .items
            .iter()
            .filter(|item| item.is_monthly_card())
            .map(|item| u32::from(item.count))
            .sum();
        if self.sender != "Gameshop" || quantity == 0 {
            return body;
        }
        // Match complete Source-authored templates and the actual attachment
        // quantity. Never rewrite substrings in opaque letters/player text.
        let name = mir2_game_data::BILLING_MONTHLY_CARD_ITEM_NAME;
        let localized_name =
            crate::native_i18n::for_locale(language, "billing.monthlyCardItemName", name);
        let quantity = quantity.to_string();
        let bare = format!("{name} x{quantity}");
        let purchase = format!("{name} x {quantity} was sent from the game shop.");
        let gift_suffix = format!(" sent {name} x {quantity} from the game shop.");
        body.split('\n')
            .map(|line| {
                let trimmed = line.strip_suffix('\r').unwrap_or(line);
                let replacement = if trimmed == bare {
                    Some(format!("{localized_name} x{quantity}"))
                } else if trimmed == purchase {
                    Some(crate::native_i18n::format_for(
                        language,
                        "game.mail.shopPurchaseBody",
                        "{item} x {quantity} was sent from the game shop.",
                        &[("item", &localized_name), ("quantity", &quantity)],
                    ))
                } else if let Some(sender) = trimmed
                    .strip_suffix(&gift_suffix)
                    .filter(|sender| mir2_ui_core::game_shop::is_valid_recipient_name(sender))
                {
                    Some(crate::native_i18n::format_for(
                        language,
                        "game.mail.shopGiftBody",
                        "{sender} sent {item} x {quantity} from the game shop.",
                        &[
                            ("sender", sender),
                            ("item", &localized_name),
                            ("quantity", &quantity),
                        ],
                    ))
                } else {
                    None
                };
                match replacement {
                    Some(mut text) => {
                        if line.ends_with('\r') {
                            text.push('\r');
                        }
                        text
                    }
                    None => line.to_owned(),
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn has_attachment(&self) -> bool {
        self.gold > 0 || !self.items.is_empty()
    }

    pub fn attachment_summary(&self) -> String {
        let mut parts = Vec::new();
        if self.gold > 0 {
            parts.push(format!("{} Gold", self.gold));
        }
        if !self.items.is_empty() {
            parts.push(
                self.items
                    .iter()
                    .map(MailAttachment::label)
                    .collect::<Vec<_>>()
                    .join(", "),
            );
        }
        if parts.is_empty() {
            String::new()
        } else {
            parts.join(" · ")
        }
    }
}

#[derive(Debug, Clone, Default, Resource, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct MailModel {
    #[serde(default, deserialize_with = "deserialize_mail_messages")]
    pub mails: Vec<MailMessage>,
    pub selected_id: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct MailPageCursor {
    pub page: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MailPage<'a> {
    pub page: usize,
    pub page_count: usize,
    pub entries: Vec<&'a MailMessage>,
}

impl MailModel {
    pub fn selected(&self) -> Option<&MailMessage> {
        self.selected_id.and_then(|id| {
            self.mails
                .iter()
                .find(|m| m.id == id && m.operation.is_none())
        })
    }

    pub fn unread_count(&self) -> usize {
        self.mails
            .iter()
            .filter(|m| m.operation.is_none() && !m.read)
            .count()
    }

    pub fn visible_mails(&self) -> Vec<&MailMessage> {
        self.mails
            .iter()
            .filter(|mail| mail.operation.is_none())
            .collect()
    }

    pub fn operation_feedback(&self) -> Option<&MailOperationFeedback> {
        self.mails.iter().find_map(|mail| mail.operation.as_ref())
    }

    /// Clamp local pagination and selection after an authoritative refresh.
    /// A delete is not guessed locally: the next server snapshot determines
    /// which rows remain, while this method prevents stale UI state.
    pub fn clamp_after_refresh(&mut self, cursor: &mut MailPageCursor) {
        self.selected_id = self.selected_id.filter(|id| {
            self.mails
                .iter()
                .any(|mail| mail.id == *id && mail.operation.is_none())
        });
        cursor.page = self.clamp_page(cursor.page);
    }

    pub fn page_count(&self) -> usize {
        self.page_count_for(MAIL_PAGE_SIZE)
    }

    pub fn page_count_for(&self, page_size: usize) -> usize {
        let page_size = page_size.max(1);
        self.visible_mails().len().div_ceil(page_size).max(1)
    }

    pub fn clamp_page(&self, page: usize) -> usize {
        page.min(self.page_count().saturating_sub(1))
    }

    pub fn page(&self, page: usize) -> MailPage<'_> {
        self.page_with_size(page, MAIL_PAGE_SIZE)
    }

    pub fn page_with_size(&self, page: usize, page_size: usize) -> MailPage<'_> {
        let page_size = page_size.max(1);
        let page_count = self.page_count_for(page_size);
        let page = page.min(page_count.saturating_sub(1));
        let start = page.saturating_mul(page_size);
        let entries = self
            .visible_mails()
            .into_iter()
            .skip(start)
            .take(page_size)
            .collect();
        MailPage {
            page,
            page_count,
            entries,
        }
    }

    /// Selection is identity-based. Unknown or operation-only rows are
    /// rejected so a stale pressed row cannot address a different message.
    pub fn select_visible(&mut self, id: u64) -> bool {
        if self.visible_mails().iter().any(|mail| mail.id == id) {
            self.selected_id = Some(id);
            true
        } else {
            self.selected_id = None;
            false
        }
    }
}

pub fn mail_attachment_label(item: &MailAttachment) -> String {
    item.label()
}

/// Crystal DateTime.FromBinary display; unknown/invalid dates remain blank.
pub fn mail_date_display_label(binary: i64) -> String {
    let label = mail_date_label(binary);
    if label.is_empty() {
        crate::native_i18n::for_locale(
            crate::native_i18n::locale(),
            "game.mail.dateUnknown",
            "Not recorded",
        )
    } else {
        label
    }
}

pub fn mail_date_label(binary: i64) -> String {
    const EPOCH_TICKS: i64 = 621_355_968_000_000_000;
    const MAX_TICKS: u64 = 3_155_378_975_999_999_999;
    let bits = binary as u64;
    let ticks = bits & 0x3fff_ffff_ffff_ffff;
    if ticks == 0 || ticks > MAX_TICKS {
        return String::new();
    }
    let unix_ticks = ticks as i64 - EPOCH_TICKS;
    let Some(date) = chrono::DateTime::from_timestamp(
        unix_ticks.div_euclid(10_000_000),
        (unix_ticks.rem_euclid(10_000_000) * 100) as u32,
    ) else {
        return String::new();
    };
    if bits >> 62 >= 2 {
        date.with_timezone(&chrono::Local)
            .format("%d/%m/%y %-H:%M:%S")
            .to_string()
    } else {
        date.format("%d/%m/%y %-H:%M:%S").to_string()
    }
}

pub fn mail_claim_enabled(msg: &MailMessage) -> bool {
    msg.operation.is_none() && !msg.claimed && !msg.locked && msg.has_attachment()
}

pub fn mail_delete_enabled(msg: &MailMessage) -> bool {
    msg.operation.is_none() && !msg.locked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monthly_card_mail_display_uses_exact_identity_and_does_not_rewrite_letters() {
        let attachment = MailAttachment {
            key: Some("crystal-item-1000001".into()),
            name: Some("MonthlyCard30Days".into()),
            unique_id: Some(71),
            count: 1,
            ..Default::default()
        };
        assert!(attachment.is_monthly_card());
        assert_eq!(attachment.image_index(), Some(1813));
        let source = attachment.display_tooltip_source().unwrap();
        assert_eq!(source.info.item_index, 1_000_001);
        assert_eq!(source.user_item.as_ref().unwrap().unique_id, 71);
        let mut mail = MailMessage {
            sender: "Gameshop".into(),
            body: "Purchase\r\nMonthlyCard30Days x1\r\nOpaque MonthlyCard30Days text".into(),
            items: vec![attachment.clone()],
            ..Default::default()
        };
        let expected_name = crate::native_i18n::for_locale(
            crate::native_i18n::locale(),
            "billing.monthlyCardItemName",
            "MonthlyCard30Days",
        );
        assert_eq!(attachment.label(), expected_name);
        assert_eq!(
            mail.display_body(),
            format!("Purchase\r\n{expected_name} x1\r\nOpaque MonthlyCard30Days text")
        );
        mail.sender = "Friend".into();
        assert_eq!(mail.display_body(), mail.body);
        let mut unknown = attachment;
        unknown.key = Some("unknown".into());
        assert!(
            !unknown.is_monthly_card(),
            "display names cannot supply item identity"
        );
        assert!(unknown.image_index().is_none());
        assert!(unknown.display_tooltip_source().is_none());
        unknown.key = Some("crystal-item-1000001".into());
        unknown.item_index = Some(1_000_002);
        assert!(
            !unknown.is_monthly_card(),
            "an explicit other index cannot become a monthly card"
        );
    }

    #[test]
    fn monthly_card_mail_localizes_actual_source_purchase_and_gift_templates_in_all_languages() {
        use crate::native_i18n::{for_locale, format_for, Locale};
        for quantity in [1, 2] {
            let mut mail = MailMessage {
                sender: "Gameshop".into(),
                items: (0..quantity)
                    .map(|slot| MailAttachment {
                        key: Some("crystal-item-1000001".into()),
                        item_index: Some(1_000_001),
                        unique_id: Some(90 + slot),
                        count: 1,
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            };
            let count = quantity.to_string();
            for language in Locale::ALL {
                let name = for_locale(language, "billing.monthlyCardItemName", "MonthlyCard30Days");
                mail.body = format!("Game shop purchase\r\nMonthlyCard30Days x {quantity} was sent from the game shop.\r\nOpaque MonthlyCard30Days text");
                let original_body = mail.body.clone();
                let purchase = format_for(
                    language,
                    "game.mail.shopPurchaseBody",
                    "{item} x {quantity} was sent from the game shop.",
                    &[("item", &name), ("quantity", &count)],
                );
                assert!(purchase.contains(&name));
                assert_eq!(
                    mail.display_body_for_locale(language),
                    format!("Game shop purchase\r\n{purchase}\r\nOpaque MonthlyCard30Days text")
                );
                assert_eq!(
                    mail.body, original_body,
                    "display cannot mutate Source mail"
                );
                if language != Locale::English {
                    assert!(!purchase.contains("was sent from the game shop."));
                }
                for sender in ["GiftSender", "贈送者"] {
                    mail.body = format!("Game shop gift\n{sender} sent MonthlyCard30Days x {quantity} from the game shop.");
                    let gift = format_for(
                        language,
                        "game.mail.shopGiftBody",
                        "{sender} sent {item} x {quantity} from the game shop.",
                        &[("sender", sender), ("item", &name), ("quantity", &count)],
                    );
                    assert!(gift.contains(sender), "a character name remains literal");
                    assert_eq!(
                        mail.display_body_for_locale(language),
                        format!("Game shop gift\n{gift}")
                    );
                }
            }
        }
    }

    #[test]
    fn monthly_card_mail_keeps_wrong_quantity_and_opaque_shop_text_unchanged() {
        let mut mail = MailMessage {
            sender: "Gameshop".into(),
            items: vec![MailAttachment {
                item_index: Some(1_000_001),
                key: Some("crystal-item-1000001".into()),
                count: 1,
                ..Default::default()
            }],
            ..Default::default()
        };
        for body in [
            "MonthlyCard30Days x2",
            "MonthlyCard30Days x 2 was sent from the game shop.",
            "GiftSender sent MonthlyCard30Days x 2 from the game shop.",
            "MonthlyCard30Days x 01 was sent from the game shop.",
            "Story: MonthlyCard30Days x 1 was sent from the game shop.",
            "A story says GiftSender sent MonthlyCard30Days x 1 from the game shop.",
            " sent MonthlyCard30Days x 1 from the game shop.",
            "GiftSender sent MonthlyCard30Days x 1 from the game shop. Extra text.",
            "MonthlyCard30DaysFromPlayer x 1 was sent from the game shop.",
        ] {
            mail.body = body.into();
            assert_eq!(
                mail.display_body_for_locale(crate::native_i18n::Locale::TraditionalChinese),
                body
            );
        }
        mail.body = "MonthlyCard30Days x 1 was sent from the game shop.".into();
        mail.sender = "Friend".into();
        assert_eq!(mail.display_body(), mail.body);
        mail.sender = "Gameshop".into();
        mail.items[0].item_index = Some(1_000_002);
        assert_eq!(
            mail.display_body(),
            mail.body,
            "exact attachment identity is required"
        );
        mail.items.clear();
        assert_eq!(mail.display_body(), mail.body);
    }

    #[test]
    fn missing_mail_date_has_a_visible_placeholder_without_inventing_a_timestamp() {
        assert_eq!(mail_date_label(0), "");
        assert_eq!(
            mail_date_display_label(0),
            crate::native_i18n::for_locale(
                crate::native_i18n::locale(),
                "game.mail.dateUnknown",
                "Not recorded"
            )
        );
        let timestamp = 621_355_968_000_000_000;
        assert_eq!(
            mail_date_display_label(timestamp),
            mail_date_label(timestamp)
        );
        assert!(!mail_date_display_label(-1).is_empty());
    }

    #[test]
    fn mail_dates_follow_source_format_and_leave_unknown_values_blank() {
        assert_eq!(mail_date_label(0), "");
        assert_eq!(mail_date_label(i64::MAX), "");
        assert_eq!(mail_date_label(-1), "");
        assert_eq!(mail_date_label(621_355_968_000_000_000), "01/01/70 0:00:00");
        assert_eq!(
            mail_date_label(621_355_968_000_000_000 | (1i64 << 62)),
            "01/01/70 0:00:00"
        );
        let parsed: MailMessage = serde_json::from_str(
            r#"{"id":3,"canReply":true,"dateSentBinaryDatetime":621355968000000000}"#,
        )
        .unwrap();
        assert!(parsed.can_reply);
        assert_eq!(parsed.date_sent_binary_datetime, 621_355_968_000_000_000);
    }

    #[test]
    fn full_mailbox_keeps_appended_receipt_without_evicting_real_mail() {
        use crate::pending_operations::{
            reconcile_mail_refresh, PendingOperationKey, PendingOperations,
        };
        let rows: Vec<_> = (0..MAX_MAIL_MESSAGES)
            .map(|id| serde_json::json!({"id":id,"read":true}))
            .collect();
        let old: MailModel = serde_json::from_value(serde_json::json!({"mails":rows})).unwrap();
        let mut rows = rows;
        rows.push(serde_json::json!({"id":u64::MAX,"operation":{"kind":"collect","success":false,"mailId":255}}));
        let new: MailModel = serde_json::from_value(serde_json::json!({"mails":rows})).unwrap();
        assert_eq!(new.visible_mails(), old.visible_mails());
        assert_eq!(new.mails.len(), MAX_MAIL_MESSAGES + 1);
        assert_eq!(new.unread_count(), 0);
        let feedback = new.mails.last().unwrap();
        assert!(!mail_delete_enabled(feedback));
        assert!(!mail_claim_enabled(feedback));
        let mut pending = PendingOperations::default();
        assert!(pending.try_begin(PendingOperationKey::ClaimMail(255)));
        assert!(pending.try_begin(PendingOperationKey::ClaimMail(254)));
        assert_eq!(reconcile_mail_refresh(&mut pending, &old, &new), 1);
        assert!(!pending.contains(&PendingOperationKey::ClaimMail(255)));
        assert!(pending.contains(&PendingOperationKey::ClaimMail(254)));
    }

    #[test]
    fn mailbox_limits_real_messages_and_feedback_independently() {
        let receipt = serde_json::json!({"id":u64::MAX,"operation":{"kind":"send","success":false,"mailId":null}});
        let mut rows = vec![receipt.clone(); 50];
        rows.extend((0..MAX_MAIL_MESSAGES + 20).map(|id| serde_json::json!({"id":id})));
        rows.push(receipt);
        let model: MailModel = serde_json::from_value(serde_json::json!({"mails":rows})).unwrap();
        assert_eq!(model.mails.len(), MAX_MAIL_MESSAGES + 1);
        assert_eq!(model.visible_mails().len(), MAX_MAIL_MESSAGES);
        assert_eq!(
            model.mails.iter().filter(|m| m.operation.is_some()).count(),
            1
        );
        assert_eq!(model.unread_count(), MAX_MAIL_MESSAGES);
    }

    fn msg(id: u64, claimed: bool, locked: bool, gold: u32) -> MailMessage {
        MailMessage {
            id,
            sender: "System".to_owned(),
            subject: format!("Test {}", id),
            body: "Hello".to_owned(),
            gold,
            items: if gold > 0 {
                vec![MailAttachment {
                    name: Some("Gold".to_owned()),
                    ..Default::default()
                }]
            } else {
                vec![]
            },
            operation: None,
            claimed,
            locked,
            read: false,
            ..Default::default()
        }
    }

    #[test]
    fn mail_claim_and_delete_rules() {
        let unclaimed = msg(1, false, false, 100);
        let claimed = msg(2, true, false, 100);
        let locked = msg(3, false, true, 100);
        let no_attach = msg(4, false, false, 0);
        assert!(mail_claim_enabled(&unclaimed));
        assert!(!mail_claim_enabled(&claimed));
        assert!(!mail_claim_enabled(&locked));
        assert!(!mail_claim_enabled(&no_attach));
        assert!(mail_delete_enabled(&unclaimed));
        assert!(!mail_delete_enabled(&locked));
    }

    #[test]
    fn serde_roundtrip() {
        let model = MailModel {
            mails: vec![msg(1, false, false, 10)],
            selected_id: Some(1),
        };
        let json = serde_json::to_string(&model).expect("serialize");
        let restored: MailModel = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(model, restored);
    }

    #[test]
    fn user_item_metadata_roundtrips_without_string_loss() {
        let attachment = MailAttachment {
            unique_id: Some(77),
            item_index: Some(1001),
            count: 2,
            current_dura: 8,
            max_dura: 10,
            soul_bound_id: 3,
            identified: true,
            cursed: true,
            gem_count: 1,
            ..Default::default()
        };
        let json = serde_json::to_string(&attachment).expect("serialize");
        let restored: MailAttachment = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored, attachment);
    }

    #[test]
    fn mail_pages_are_ten_rows_and_clamp_after_refresh_delete() {
        let mut model = MailModel {
            mails: (0..21).map(|id| msg(id, false, false, 0)).collect(),
            selected_id: Some(20),
        };
        let mut cursor = MailPageCursor { page: 2 };
        assert_eq!(model.page_count(), 3);
        assert_eq!(model.page(2).entries.len(), 1);

        model.mails.retain(|mail| mail.id != 20);
        model.clamp_after_refresh(&mut cursor);
        assert_eq!(cursor.page, 1);
        assert_eq!(model.selected_id, None);
        assert_eq!(model.page(cursor.page).entries.len(), 10);
    }

    #[test]
    fn mail_deserialization_is_backward_compatible_and_bounded() {
        let oversized = serde_json::json!({
            "mails": (0..(MAX_MAIL_MESSAGES + 7)).map(|id| serde_json::json!({
                "mailId": id,
                "from": "System",
                "subject": "Subject",
                "body": "Body",
                "gold": 0,
                "items": (0..(MAX_MAIL_ATTACHMENTS + 3)).map(|_| serde_json::json!({"name":"Potion"})).collect::<Vec<_>>(),
                "claimed": false,
                "locked": false,
                "opened": false
            })).collect::<Vec<_>>()
        });
        let model: MailModel = serde_json::from_value(oversized).expect("legacy mail");
        assert_eq!(model.mails.len(), MAX_MAIL_MESSAGES);
        assert_eq!(model.mails[0].items.len(), MAX_MAIL_ATTACHMENTS);
        assert_eq!(model.mails[0].sender, "System");
    }

    #[test]
    fn stale_mail_selection_fails_closed() {
        let mut model = MailModel {
            mails: vec![msg(1, false, false, 0)],
            selected_id: Some(99),
        };
        assert!(!model.select_visible(99));
        assert_eq!(model.selected_id, None);
        assert!(model.select_visible(1));
        assert_eq!(model.selected_id, Some(1));
    }
}
