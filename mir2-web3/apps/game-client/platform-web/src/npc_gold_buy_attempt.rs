//! Browser transport bridge for the shared local attempt slot; never a purchase ACK.
use mir2_client_core::npc_gold_buy_attempt::{NpcGoldBuyAttempt, NpcGoldBuyAttemptOutcome,
    NpcGoldBuyAttemptPhase, NpcGoldBuyAttemptSlot, NpcGoldBuyAttemptToken, NpcGoldBuyConnectionEpoch,
    NPC_GOLD_BUY_AUTHORITY_MAX_BYTES};
use serde_json::Value;
use wasm_bindgen::prelude::*;
use crate::mail_compose::StrictMailValue;

const BODY_LIMIT: usize = 4096;
// JSON escaping can expand a bounded string to six bytes per source byte.
const REQUEST_LIMIT: usize = 6 * (NPC_GOLD_BUY_AUTHORITY_MAX_BYTES + BODY_LIMIT) + 256;
const SAFE_ITEM_INDEX: u64 = 9_007_199_254_740_991;

#[derive(Clone, Debug, PartialEq, Eq)]
struct WebTicket { transport: String, body: String }

fn fields<'a>(value: &'a Value, names: &[&str]) -> Option<&'a serde_json::Map<String, Value>> {
    let object = value.as_object()?;
    (object.len() == names.len() && names.iter().all(|name| object.contains_key(*name))).then_some(object)
}
fn decimal(value: &Value) -> Option<u64> {
    let text = value.as_str()?;
    if text.is_empty() || text.len() > 20 || text.starts_with('0')
        || !text.bytes().all(|c| c.is_ascii_digit()) { return None; }
    text.parse::<u64>().ok().filter(|number| *number != 0)
}
fn ticket(value: &Value) -> Option<WebTicket> {
    let value = fields(value, &["transport", "body"])?;
    decimal(&value["transport"])?;
    let body = value["body"].as_str()?;
    if body.len() > BODY_LIMIT { return None; }
    let StrictMailValue(raw) = serde_json::from_str(body).ok()?;
    let raw = fields(&raw, &["type", "itemIndex", "count", "panelType"])?;
    if raw["type"].as_str()? != "buyItem" || raw["panelType"].as_u64()? != 0
        || raw["itemIndex"].as_u64()? > SAFE_ITEM_INDEX
        || !(1..=u64::from(u16::MAX)).contains(&raw["count"].as_u64()?) { return None; }
    Some(WebTicket { transport: value["transport"].as_str()?.to_owned(), body: body.to_owned() })
}
fn phase(phase: NpcGoldBuyAttemptPhase) -> &'static str {
    use NpcGoldBuyAttemptPhase::*;
    match phase { Queued => "queued", Bound => "bound", Entered => "entered",
        Flushed => "flushed", Unknown => "unknown", DefinitelyUnsent => "definitelyUnsent" }
}
fn quoted(output: &mut String, value: &str) {
    output.push_str(&serde_json::to_string(value).expect("JSON string serialization"));
}
fn encoded_flight(output: &mut String, attempt: &NpcGoldBuyAttempt<WebTicket>) {
    output.push_str("{\"authorityRevision\":");
    quoted(output, &attempt.authority_revision.to_string());
    output.push_str(",\"phase\":"); quoted(output, phase(attempt.phase));
    output.push_str(",\"ticket\":");
    if let Some(ticket) = &attempt.ticket {
        output.push_str("{\"body\":"); quoted(output, &ticket.body);
        output.push_str(",\"transport\":"); quoted(output, &ticket.transport);
        output.push('}');
    } else { output.push_str("null"); }
    output.push_str(",\"token\":"); quoted(output, &attempt.token.value().to_string());
    output.push('}');
}

#[wasm_bindgen]
pub struct NpcGoldBuyAttemptBridge { slot: NpcGoldBuyAttemptSlot<WebTicket> }
impl Default for NpcGoldBuyAttemptBridge {
    fn default() -> Self { Self { slot: NpcGoldBuyAttemptSlot::new() } }
}
#[wasm_bindgen]
pub fn npc_gold_buy_attempt_abi_version() -> u32 { 1 }
#[wasm_bindgen]
impl NpcGoldBuyAttemptBridge {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self { Self::default() }
    pub fn transact(&mut self, input: &str) -> String {
        let matched = (input.len() <= REQUEST_LIMIT)
            .then(|| serde_json::from_str::<StrictMailValue>(input).ok()).flatten()
            .and_then(|StrictMailValue(value)| self.apply(&value));
        match matched {
            Some(matched) => self.response(matched),
            None => "{\"error\":\"Invalid NPC gold-buy attempt request\",\"ok\":false}".to_owned(),
        }
    }
}
impl NpcGoldBuyAttemptBridge {
    fn token(&self, number: u64) -> Option<NpcGoldBuyAttemptToken> {
        self.slot.flight().into_iter().chain(self.slot.history().iter())
            .find(|attempt| attempt.token.value() == number).map(|attempt| attempt.token)
    }
    fn response(&self, matched: bool) -> String {
        // Preserve the original Value/Map lexical field order, without building
        // response maps or cloning the complete ticket body into another Value.
        let mut output = String::from("{\"matched\":");
        output.push_str(if matched { "true" } else { "false" });
        output.push_str(",\"ok\":true,\"state\":{\"authorityRevision\":");
        quoted(&mut output, &self.slot.authority_revision().to_string());
        output.push_str(",\"canReserve\":");
        output.push_str(if self.slot.can_reserve() { "true" } else { "false" });
        output.push_str(",\"flight\":");
        if let Some(attempt) = self.slot.flight() { encoded_flight(&mut output, attempt); }
        else { output.push_str("null"); }
        output.push_str(",\"lastPhase\":");
        if let Some(last) = self.slot.flight().or_else(|| self.slot.history().back()) {
            quoted(&mut output, phase(last.phase));
        } else { output.push_str("null"); }
        output.push_str("}}"); output
    }
    fn apply(&mut self, value: &Value) -> Option<bool> {
        let op = value.get("op")?.as_str()?;
        match op {
            "status" | "reserve" => {
                fields(value, &["op"])?;
                Some(op == "status" || self.slot.reserve().is_some())
            }
            "connection" => {
                let value = fields(value, &["op", "run", "connection"])?;
                Some(self.slot.observe_connection(NpcGoldBuyConnectionEpoch {
                    run: decimal(&value["run"])? , connection: decimal(&value["connection"])? ,
                }))
            }
            "observe" => {
                let value = fields(value, &["op", "authority"])?;
                let authority = value["authority"].as_str()?;
                if authority.is_empty() || authority.len() > NPC_GOLD_BUY_AUTHORITY_MAX_BYTES { return None; }
                Some(self.slot.observe_authority(authority))
            }
            "availability" => {
                let value = fields(value, &["op", "available"])?;
                self.slot.set_available(value["available"].as_bool()?); Some(true)
            }
            "rejectUnpublished" => {
                let value = fields(value, &["op", "token"])?;
                let token = self.token(decimal(&value["token"])?);
                Some(token.is_some_and(|token| self.slot.reject_unpublished(token)))
            }
            "bind" | "allows" | "enter" | "receipt" => {
                let names = if op == "receipt" { &["op", "token", "ticket", "outcome"][..] }
                    else { &["op", "token", "ticket"][..] };
                let value = fields(value, names)?;
                let number = decimal(&value["token"])?;
                let ticket = ticket(&value["ticket"])?;
                if op != "receipt" && self.slot.connection_epoch().is_none_or(|epoch|
                    ticket.transport != epoch.connection.to_string()) { return Some(false); }
                let outcome = if op == "receipt" {
                    Some(match value["outcome"].as_str()? {
                        "definitelyUnsent" => NpcGoldBuyAttemptOutcome::DefinitelyUnsent,
                        "flushed" => NpcGoldBuyAttemptOutcome::Flushed,
                        "unknown" => NpcGoldBuyAttemptOutcome::Unknown, _ => return None,
                    })
                } else { None };
                let Some(token) = self.token(number) else { return Some(false); };
                Some(match op {
                    "bind" => self.slot.bind(token, ticket),
                    "allows" => self.slot.can_begin_entry(token, &ticket),
                    "enter" => self.slot.begin_entry(token, &ticket),
                    _ => self.slot.apply_receipt(token, &ticket, outcome?),
                })
            }
            _ => None,
        }
    }
}

#[cfg(test)]
#[path = "npc_gold_buy_attempt_tests.rs"]
mod tests;
