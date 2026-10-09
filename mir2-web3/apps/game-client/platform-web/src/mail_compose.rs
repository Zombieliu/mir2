//! Bevy-independent strict JSON edge for the common compose policy.
use mir2_client_core::mail_compose::{normalize_message, prepare_send};
use serde_json::{json, Value};
use wasm_bindgen::prelude::*;

const SAFE: u64 = 9_007_199_254_740_991;
fn fields<'a>(value: &'a Value, names: &[&str]) -> Option<&'a serde_json::Map<String, Value>> {
    let object = value.as_object()?;
    (object.len() == names.len() && names.iter().all(|name| object.contains_key(*name))).then_some(object)
}
fn rejected(error: &str) -> String { json!({"ok":false,"error":error}).to_string() }

pub fn normalize_json(input: &str) -> String {
    if input.len() > 8192 { return rejected("Invalid mail input"); }
    let Ok(value) = serde_json::from_str::<Value>(input) else { return rejected("Invalid mail input"); };
    let Some(object) = fields(&value, &["message"]) else { return rejected("Invalid mail input"); };
    let Some(message) = object["message"].as_str() else { return rejected("Invalid mail input"); };
    match normalize_message(message) {
        Ok(message) => json!({"ok":true,"message":message}).to_string(),
        Err(error) => rejected(error.message()),
    }
}

pub fn prepare_json(input: &str) -> String {
    if input.len() > 8192 { return rejected("Invalid mail input"); }
    let Ok(value) = serde_json::from_str::<Value>(input) else { return rejected("Invalid mail input"); };
    let Some(object) = fields(&value, &["recipient","message","gold","attachmentUniqueIds","stamped"]) else {
        return rejected("Invalid mail input");
    };
    let (Some(recipient), Some(message), Some(gold), Some(ids), Some(stamped)) = (
        object["recipient"].as_str(), object["message"].as_str(), object["gold"].as_u64(),
        object["attachmentUniqueIds"].as_array(), object["stamped"].as_bool(),
    ) else { return rejected("Invalid mail input"); };
    if recipient.len() > 1024 || gold > u64::from(u32::MAX) || ids.len() > 5 {
        return rejected("Invalid mail input");
    }
    let Some(ids) = ids.iter().map(|id| id.as_u64().filter(|n| *n > 0 && *n <= SAFE)).collect::<Option<Vec<_>>>() else {
        return rejected("Invalid mail attachment selection");
    };
    match prepare_send(recipient, message, gold as u32, &ids) {
        Ok(payload) => {
            let mut items = payload.attachment_unique_ids;
            items.resize(5, 0);
            json!({"ok":true,"payload":{"name":payload.recipient,"message":payload.message,
                "gold":payload.gold,"itemsIdx":items,"stamped":stamped}}).to_string()
        }
        Err(error) => rejected(error.message()),
    }
}

#[wasm_bindgen]
pub fn mail_compose_abi_version() -> u32 { 1 }
#[wasm_bindgen]
pub fn normalize_mail_message(input: &str) -> String { normalize_json(input) }
#[wasm_bindgen]
pub fn prepare_mail_send(input: &str) -> String { prepare_json(input) }

// Additive ABI. The old compose exports/DTO and numeric wire remain unchanged.
use mir2_client_core::mail_compose::{MailDraftClock, MailSendSlot, MailSendStream};

// Reject duplicate keys before Value can overwrite them. This parser is used
// only by the additive ABI and its exact serialized-body proof.
pub(crate) use crate::strict_json::StrictMailValue;

fn decimal(value: &Value, positive: bool) -> Option<u64> {
    let text = value.as_str()?;
    if text.is_empty() || !text.bytes().all(|c| c.is_ascii_digit())
        || text.len() > 20 || text.len() > 1 && text.starts_with('0') { return None; }
    let number = text.parse::<u64>().ok()?;
    (!positive || number != 0).then_some(number)
}
fn owner_value(value: &Value) -> Option<Value> {
    let owner = fields(value, &["connectionGeneration", "sessionGeneration", "ownerRevision", "playerObjectId"])?;
    for name in ["connectionGeneration", "sessionGeneration", "ownerRevision", "playerObjectId"] {
        decimal(&owner[name], name != "ownerRevision")?;
    }
    Some(value.clone())
}
fn raw_value(value: &Value) -> Option<Value> {
    let raw = fields(value, &["to", "subject", "body", "goldText", "items", "attachmentUniqueIds", "stamped",
        "attachmentUniqueIdsPresent", "stampedPresent"])?;
    for (name, limit) in [("to", 1024), ("subject", 4096), ("body", 8192), ("goldText", 256)] {
        if raw[name].as_str()?.len() > limit { return None; }
    }
    let items = raw["items"].as_array()?;
    if items.len() > 5 || items.iter().any(|item| item.as_str().is_none_or(|s| s.len() > 1024)) { return None; }
    let ids = raw["attachmentUniqueIds"].as_array()?;
    if ids.len() > 5 { return None; }
    let ids = ids.iter().map(|id| decimal(id, true)).collect::<Option<Vec<_>>>()?;
    if ids.iter().enumerate().any(|(i, id)| *id > SAFE || ids[..i].contains(id)) { return None; }
    let stamped=raw["stamped"].as_bool()?;
    if !raw["attachmentUniqueIdsPresent"].as_bool()? && !ids.is_empty() || !raw["stampedPresent"].as_bool()? && stamped {return None;}
    Some(value.clone())
}
fn payload_value(value: &Value) -> Option<Value> {
    let payload = fields(value, &["name", "message", "gold", "itemsIdx", "stamped"])?;
    payload["name"].as_str()?; payload["message"].as_str()?; payload["stamped"].as_bool()?;
    if payload["gold"].as_u64()? > u64::from(u32::MAX) { return None; }
    let ids = payload["itemsIdx"].as_array()?;
    if ids.len() != 5 { return None; }
    for id in ids { if decimal(id, false)? > SAFE { return None; } }
    Some(value.clone())
}
fn raw_gold(text: &str) -> Option<u32> {
    let text = text.trim_matches(|c:char|c.is_whitespace()||c=='\u{feff}');
    if text.is_empty() { return Some(0); }
    for (prefix, radix) in [("0x", 16), ("0b", 2), ("0o", 8)] {
        if text.to_ascii_lowercase().starts_with(prefix) { return u32::from_str_radix(&text[2..], radix).ok(); }
    }
    let value = text.parse::<f64>().ok()?;
    (value.is_finite() && value >= 0.0 && value <= f64::from(u32::MAX) && value.fract() == 0.0).then_some(value as u32)
}
fn stream_value(stream: MailSendStream) -> Value { json!({"run":stream.run.to_string(),"connection":stream.connection.to_string()}) }
fn parsed_stream(value: &Value) -> Option<MailSendStream> {
    let v = fields(value, &["run", "connection"])?;
    Some(MailSendStream { run: decimal(&v["run"], true)?, connection: decimal(&v["connection"], true)? })
}
fn allocate(next: &mut Option<u64>) -> Option<u64> { let value = (*next)?; *next = value.checked_add(1); Some(value) }

#[derive(Clone, Debug, PartialEq, Eq)]
struct WebMailFlight { incarnation: u64, generation: u64, owner: Value, key: String, raw: Value, payload: Value, body: String }

#[wasm_bindgen]
pub struct MailSendSlotBridge {
    slot: MailSendSlot<WebMailFlight>, clock: MailDraftClock,
    stream: Option<MailSendStream>, next_connection: Option<u64>, next_incarnation: Option<u64>, next_owner: Option<u64>,
    incarnation: Option<u64>, owner_tag: Option<u64>, owner: Option<Value>, key: Option<String>, raw: Option<Value>, notice: Option<String>,
}
impl Default for MailSendSlotBridge {
    fn default() -> Self { Self { slot: MailSendSlot::default(), clock: MailDraftClock::default(), stream: None,
        next_connection: Some(1), next_incarnation: Some(1), next_owner: Some(1), incarnation: None, owner_tag: None,
        owner: None, key: None, raw: None, notice: None } }
}
#[wasm_bindgen]
pub fn mail_send_slot_abi_version() -> u32 { 1 }
#[wasm_bindgen]
impl MailSendSlotBridge {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self { Self::default() }
    pub fn transact(&mut self, input: &str) -> String { self.apply(input).to_string() }
    /// Read the current Core draft's gold with the same parser used by reserve.
    /// This does not allocate a flight, advance the clock, or change transact/status.
    pub fn draft_gold(&self) -> Option<u32> {
        self.incarnation?; self.owner_tag?; self.owner.as_ref()?; self.key.as_ref()?; self.clock.generation()?;
        raw_gold(self.raw.as_ref()?.get("goldText")?.as_str()?)
    }
}
impl MailSendSlotBridge {
    fn response(&self, matched: bool, completion: Option<&str>) -> Value {
        let flight = self.slot.flight().map(|f| json!({"token":f.token.value().to_string(),"stream":stream_value(f.stream),
            "owner":f.value.owner,"ownerTag":f.owner.to_string(),"incarnation":f.value.incarnation.to_string(),
            "generation":f.value.generation.to_string(),"key":f.value.key,"draft":f.value.raw,"payload":f.value.payload,
            "body":f.value.body,"entered":f.entered,"retired":f.retired}));
        json!({"ok":true,"matched":matched,"completion":completion,"state":{"stream":self.stream.map(stream_value),
            "incarnation":self.incarnation.map(|v|v.to_string()),"owner":self.owner,"ownerTag":self.owner_tag.map(|v|v.to_string()),
            "generation":self.clock.generation().map(|v|v.to_string()),"key":self.key,"draft":self.raw,"flight":flight,"notice":self.notice}})
    }
    fn invalid() -> Value { json!({"ok":false,"error":"Invalid mail send-slot input"}) }
    fn current(&self, value: &Value) -> bool { decimal(value, true).is_some_and(|v| self.incarnation == Some(v)) }
    fn apply(&mut self, input: &str) -> Value {
        if input.len() > 65536 { return Self::invalid(); }
        let Ok(StrictMailValue(value)) = serde_json::from_str::<StrictMailValue>(input) else { return Self::invalid(); };
        let Some(op) = value.get("op").and_then(Value::as_str) else { return Self::invalid(); };
        let names: &[&str] = match op {
            "status" | "mount" | "open" => &["op"], "sync" => &["op","incarnation","owner"],
            "remember" => &["op","incarnation","key","draft"], "reserve" => &["op","incarnation","key","payload","body"],
            "allows" | "cancel" | "enter" => &["op","incarnation","token","stream","body"],
            "ack" => &["op","incarnation","stream","result"], _ => return Self::invalid(),
        };
        let Some(v) = fields(&value, names) else { return Self::invalid(); };
        match op {
            "status" => self.response(false, None),
            "mount" => {
                self.incarnation = allocate(&mut self.next_incarnation); self.owner_tag = allocate(&mut self.next_owner);
                self.slot.retire_owner(self.owner_tag.unwrap_or(0)); self.clock.advance();
                self.response(self.incarnation.is_some() && self.owner_tag.is_some() && self.clock.generation().is_some(), None)
            }
            "open" => {
                let Some(connection) = allocate(&mut self.next_connection) else { return self.response(false, None); };
                let stream = MailSendStream { run: 1, connection };
                if !self.slot.start_stream(stream) { return self.response(false, None); }
                self.stream = Some(stream); self.notice = None; self.response(true, None)
            }
            "sync" => {
                if decimal(&v["incarnation"], true).is_none() || !v["owner"].is_null() && owner_value(&v["owner"]).is_none() { return Self::invalid(); }
                if !self.current(&v["incarnation"]) { return self.response(false, None); }
                let owner = (!v["owner"].is_null()).then(||v["owner"].clone());
                if self.owner != owner {
                    self.owner_tag = allocate(&mut self.next_owner); self.slot.retire_owner(self.owner_tag.unwrap_or(0));
                    self.owner = owner; self.raw = None; self.key = None; self.clock.advance();
                    self.notice = self.slot.flight().map(|_|"Waiting for earlier mail delivery on this connection".to_owned());
                }
                self.response(true, None)
            }
            "remember" => {
                let (Some(key), Some(raw)) = (v["key"].as_str().filter(|s|s.len()<=4096), raw_value(&v["draft"])) else { return Self::invalid(); };
                if decimal(&v["incarnation"], true).is_none() { return Self::invalid(); }
                if !self.current(&v["incarnation"]) || self.owner.is_none() { return self.response(false, None); }
                if self.raw.as_ref() != Some(&raw) || self.key.as_deref() != Some(key) { self.clock.advance(); }
                self.raw = Some(raw); self.key = Some(key.to_owned());
                if self.slot.flight().is_none() { self.notice = None; } self.response(true, None)
            }
            "reserve" => {
                let (Some(key), Some(payload), Some(body)) = (v["key"].as_str(), payload_value(&v["payload"]), v["body"].as_str().filter(|s|s.len()<=32768)) else { return Self::invalid(); };
                if decimal(&v["incarnation"], true).is_none() { return Self::invalid(); }
                let Some(raw) = self.raw.clone() else { return self.response(false, None); };
                let Some(gold) = raw_gold(raw["goldText"].as_str().unwrap()) else { return self.response(false, None); };
                let ids = raw["attachmentUniqueIds"].as_array().unwrap().iter().map(|id|decimal(id,true).unwrap()).collect::<Vec<_>>();
                if !raw["items"].as_array().unwrap().is_empty() || ids.iter().any(|id|*id>SAFE) { return self.response(false,None); }
                let Ok(prepared) = prepare_send(raw["to"].as_str().unwrap(), raw["body"].as_str().unwrap(), gold, &ids) else { return self.response(false,None); };
                let mut wire_ids = prepared.attachment_unique_ids; wire_ids.resize(5,0);
                let expected_payload = json!({"name":prepared.recipient,"message":prepared.message,"gold":prepared.gold,"itemsIdx":wire_ids.iter().map(u64::to_string).collect::<Vec<_>>(),"stamped":raw["stamped"]});
                let wire = json!({"name":expected_payload["name"],"message":expected_payload["message"],"gold":expected_payload["gold"],"itemsIdx":wire_ids,"stamped":expected_payload["stamped"],"type":"sendMail"});
                let parsed_body = serde_json::from_str::<StrictMailValue>(body).ok().map(|v|v.0);
                if payload != expected_payload || parsed_body.as_ref() != Some(&wire) || !self.current(&v["incarnation"]) || self.key.as_deref()!=Some(key) { return self.response(false,None); }
                let (Some(stream),Some(owner_tag),Some(owner),Some(incarnation),Some(generation)) = (self.stream,self.owner_tag,self.owner.clone(),self.incarnation,self.clock.generation()) else { return self.response(false,None); };
                let capture=WebMailFlight {incarnation,generation,owner,key:key.to_owned(),raw,payload,body:body.to_owned()};
                let matched=self.slot.reserve(stream,owner_tag,capture).is_some();
                if matched { self.notice=Some("Sending mail…".to_owned()); } self.response(matched,None)
            }
            "allows" | "enter" | "cancel" => {
                let (Some(token_value),Some(stream),Some(body))=(decimal(&v["token"],true),parsed_stream(&v["stream"]),v["body"].as_str()) else {return Self::invalid();};
                if decimal(&v["incarnation"],true).is_none(){return Self::invalid();}
                // Obtain the opaque token from the actual Core flight; never reconstruct its private type.
                let Some(flight)=self.slot.flight() else {return self.response(false,None);};
                if flight.token.value()!=token_value || flight.stream!=stream || flight.value.body!=body {return self.response(false,None);}
                let token=flight.token;
                if op=="cancel" {let matched=self.slot.cancel_unsent(token).is_some();if matched{self.notice=Some("Mail was not sent; draft kept".to_owned());}return self.response(matched,None);}
                let matches=self.current(&v["incarnation"]) && !flight.entered && !flight.retired && self.owner_tag==Some(flight.owner)
                    && self.clock.generation()==Some(flight.value.generation) && self.raw.as_ref()==Some(&flight.value.raw) && self.key.as_deref()==Some(&flight.value.key);
                let matched=matches && (op=="allows" || self.slot.enter(token)); self.response(matched,None)
            }
            "ack" => {
                let (Some(stream),Some(result))=(parsed_stream(&v["stream"]),v["result"].as_i64().filter(|n|matches!(*n,1|-1))) else {return Self::invalid();};
                if !v["incarnation"].is_null() && decimal(&v["incarnation"],true).is_none(){return Self::invalid();}
                let Some(flight)=self.slot.flight() else {return self.response(false,None);};
                if flight.stream!=stream{return self.response(false,None);}
                let token=flight.token;
                let Some(old)=self.slot.acknowledge(token,result as i32) else {return self.response(false,None);};
                let current=!old.retired && self.current(&v["incarnation"]) && self.incarnation==Some(old.value.incarnation)
                    && self.owner_tag==Some(old.owner) && self.owner.as_ref()==Some(&old.value.owner) && self.clock.generation()==Some(old.value.generation)
                    && self.raw.as_ref()==Some(&old.value.raw) && self.key.as_deref()==Some(&old.value.key);
                let completion=if !current{"retired"}else if result==1{self.raw=None;self.key=None;self.clock.advance();self.notice=None;"success"}
                    else{self.notice=Some("Mail was rejected; draft kept".to_owned());"failure"};
                self.response(true,Some(completion))
            }
            _=>Self::invalid(),
        }
    }
}

#[cfg(test)]
mod mail_send_slot_tests {
    use super::*;
    fn call(bridge:&mut MailSendSlotBridge,input:Value)->Value { serde_json::from_str(&bridge.transact(&input.to_string())).unwrap() }
    fn owner()->Value {json!({"connectionGeneration":"1","sessionGeneration":"2","ownerRevision":"0","playerObjectId":"3"})}
    fn raw()->Value {json!({"to":" R ","subject":"local subject","body":" body\r\n ","goldText":"0007","items":[],"attachmentUniqueIds":[],"stamped":false,"attachmentUniqueIdsPresent":false,"stampedPresent":false})}
    fn payload()->Value {json!({"name":"R","message":"body","gold":7,"itemsIdx":["0","0","0","0","0"],"stamped":false})}
    fn body()->String {json!({"name":"R","message":"body","gold":7,"itemsIdx":[0,0,0,0,0],"stamped":false,"type":"sendMail"}).to_string()}
    fn ready()->MailSendSlotBridge {
        let mut b=MailSendSlotBridge::new();assert!(call(&mut b,json!({"op":"mount"}))["matched"].as_bool().unwrap());
        call(&mut b,json!({"op":"open"}));call(&mut b,json!({"op":"sync","incarnation":"1","owner":owner()}));
        call(&mut b,json!({"op":"remember","incarnation":"1","key":"A","draft":raw()}));b
    }
    fn reserve(b:&mut MailSendSlotBridge)->Value {call(b,json!({"op":"reserve","incarnation":"1","key":"A","payload":payload(),"body":body()}))}
    fn proof(op:&str,f:&Value)->Value {json!({"op":op,"incarnation":f["incarnation"],"token":f["token"],"stream":f["stream"],"body":f["body"]})}
    fn ack(b:&mut MailSendSlotBridge,f:&Value,result:i32)->Value {call(b,json!({"op":"ack","incarnation":"1","stream":f["stream"],"result":result}))}
    #[test]
    fn mail_send_slot_draft_gold_is_read_only_and_uses_existing_raw_parser_without_a_stream(){
        let mut b=MailSendSlotBridge::new();assert_eq!(b.draft_gold(),None);
        call(&mut b,json!({"op":"mount"}));assert_eq!(b.draft_gold(),None);
        call(&mut b,json!({"op":"sync","incarnation":"1","owner":owner()}));assert_eq!(b.draft_gold(),None);
        call(&mut b,json!({"op":"remember","incarnation":"1","key":"A","draft":raw()}));
        let before=call(&mut b,json!({"op":"status"}))["state"].clone();
        assert!(before["stream"].is_null());assert_eq!(b.draft_gold(),Some(7));assert_eq!(b.draft_gold(),Some(7));
        assert_eq!(call(&mut b,json!({"op":"status"}))["state"],before);
        let mut edited=raw();edited["body"]=json!("changed");edited["attachmentUniqueIds"]=json!(["41"]);
        edited["attachmentUniqueIdsPresent"]=json!(true);edited["stamped"]=json!(true);edited["stampedPresent"]=json!(true);
        call(&mut b,json!({"op":"remember","incarnation":"1","key":"A","draft":edited.clone()}));assert_eq!(b.draft_gold(),Some(7));
        edited["goldText"]=json!("0x7");call(&mut b,json!({"op":"remember","incarnation":"1","key":"A","draft":edited.clone()}));assert_eq!(b.draft_gold(),Some(7));
        edited["goldText"]=json!("");call(&mut b,json!({"op":"remember","incarnation":"1","key":"A","draft":edited.clone()}));assert_eq!(b.draft_gold(),Some(0));
        edited["goldText"]=json!("bad");call(&mut b,json!({"op":"remember","incarnation":"1","key":"A","draft":edited}));assert_eq!(b.draft_gold(),None);
        call(&mut b,json!({"op":"sync","incarnation":"1","owner":null}));assert_eq!(b.draft_gold(),None);
    }
    #[test]
    fn mail_send_slot_draft_gold_withdraws_on_missing_owner_incarnation_or_poisoned_core_generation(){
        let mut b=ready();assert_eq!(b.draft_gold(),Some(7));
        b.owner_tag=None;assert_eq!(b.draft_gold(),None);b.owner_tag=Some(2);
        b.incarnation=None;assert_eq!(b.draft_gold(),None);b.incarnation=Some(1);
        b.clock=MailDraftClock::from_highwater(u64::MAX);let mut edited=raw();edited["body"]=json!("changed");
        call(&mut b,json!({"op":"remember","incarnation":"1","key":"A","draft":edited}));
        assert!(call(&mut b,json!({"op":"status"}))["state"]["generation"].is_null());assert_eq!(b.draft_gold(),None);
    }
    #[test]
    fn mail_send_slot_bridge_strict_schemas_reject_unknown_missing_type_noncanonical_and_overflow_without_mutation(){
        let mut b=ready();let before=call(&mut b,json!({"op":"status"}))["state"].clone();
        let mut inputs=vec![json!({}),json!({"op":"status","extra":0}),json!({"op":"remember","incarnation":"1","key":"A"}),json!({"op":"unknown"})];
        for value in [json!(1),json!(null),json!(""),json!("01"),json!("+1"),json!("-1"),json!(" 1"),json!("18446744073709551616"),json!("0")]{inputs.push(json!({"op":"sync","incarnation":value,"owner":owner()}));}
        for value in [json!(1),json!("1"),json!(true),json!(null)] {let mut draft=raw();draft["stamped"]=value;inputs.push(json!({"op":"remember","incarnation":"1","key":"A","draft":draft}));}
        // true is the one valid boolean in the above type table.
        inputs.retain(|v|v.get("draft").is_none_or(|d|d["stamped"]!=true));
        let mut extra=raw();extra["unknown"]=json!(0);inputs.push(json!({"op":"remember","incarnation":"1","key":"A","draft":extra}));
        for input in inputs {assert_eq!(call(&mut b,input)["ok"],false);assert_eq!(call(&mut b,json!({"op":"status"}))["state"],before);}
        for text in ["{","[]","null","{\"op\":\"open\",\"op\":\"status\"}"]{assert_eq!(serde_json::from_str::<Value>(&b.transact(text)).unwrap()["ok"],false);assert_eq!(call(&mut b,json!({"op":"status"}))["state"],before);}
        let mut future=owner();future["playerObjectId"]=json!("18446744073709551615");future["ownerRevision"]=json!("9007199254740993");
        let accepted=call(&mut b,json!({"op":"sync","incarnation":"1","owner":future.clone()}));assert_eq!(accepted["state"]["owner"],future);
    }
    #[test]
    fn mail_send_slot_duplicate_key_scan_preserves_nested_and_escaped_key_identity() {
        fn strict_mail_value(input: &str) -> Option<Value> {
            serde_json::from_str::<StrictMailValue>(input).ok().map(|value| value.0)
        }
        for input in [
            r#"{"op":"status","op":"mount"}"#,
            r#"{"op":"status","\u006fp":"mount"}"#,
            r#"{"outer":{"key":1,"key":2}}"#,
            r#"{"outer":[{"key":1,"\u006bey":2}]}"#,
            r#"{"outer":{"\uD83D\uDE00":1,"😀":2}}"#,
            r#"{"outer":{"a\\b":1,"a\u005cb":2}}"#,
            r#"{"outer":{"a\"b":1,"a\u0022b":2}}"#,
        ] { assert!(strict_mail_value(input).is_none(), "duplicate key input {input}"); }
        for input in [
            r#"{"left":{"key":1},"right":{"key":2}}"#,
            r#"[{"key":1},{"key":2}]"#,
            r#"{"outer":["} { : \\\"",{"key":"value: with } and {"}]}"#,
            r#"{"outer":{"\uD83D\uDE00":1,"😀x":2}}"#,
        ] {
            let expected = serde_json::from_str::<Value>(input).expect("valid unique-key fixture");
            assert_eq!(strict_mail_value(input), Some(expected), "valid input {input}");
        }
        for input in ["{", "[", "\"\\ud800\"", "1e9999"] { assert!(strict_mail_value(input).is_none()); }
        let mut b = ready();
        let before = call(&mut b, json!({"op":"status"}))["state"].clone();
        let input = r#"{"op":"sync","incarnation":"1","owner":{"connectionGeneration":"1","sessionGeneration":"2","ownerRevision":"0","playerObjectId":"3","\u0070layerObjectId":"4"}}"#;
        assert_eq!(serde_json::from_str::<Value>(&b.transact(input)).unwrap()["ok"], false);
        assert_eq!(call(&mut b, json!({"op":"status"}))["state"], before);
    }
    #[test]
    fn mail_send_slot_bridge_exact_cancel_entry_unknown_and_legal_ack_are_core_owned(){
        let mut b=ready();let reserved=reserve(&mut b);let f=reserved["state"]["flight"].clone();assert_eq!(f["token"],"1");
        let mut bad=proof("cancel",&f);bad["token"]=json!("18446744073709551615");assert_eq!(call(&mut b,bad)["matched"],false);
        assert_eq!(call(&mut b,proof("cancel",&f))["matched"],true);assert_eq!(call(&mut b,proof("cancel",&f))["matched"],false);
        let f=reserve(&mut b)["state"]["flight"].clone();assert_eq!(f["token"],"2");
        assert_eq!(ack(&mut b,&f,1)["matched"],false);assert_eq!(call(&mut b,proof("enter",&f))["matched"],true);
        assert_eq!(call(&mut b,proof("enter",&f))["matched"],false);assert_eq!(call(&mut b,proof("cancel",&f))["matched"],false);assert_eq!(reserve(&mut b)["matched"],false);
        for result in [0,2] {assert_eq!(ack(&mut b,&f,result)["ok"],false);assert!(!call(&mut b,json!({"op":"status"}))["state"]["flight"].is_null());}
        assert_eq!(ack(&mut b,&f,-1)["completion"],"failure");assert_eq!(call(&mut b,json!({"op":"status"}))["state"]["draft"],raw());
        let f=reserve(&mut b)["state"]["flight"].clone();call(&mut b,proof("enter",&f));assert_eq!(ack(&mut b,&f,1)["completion"],"success");
        assert!(call(&mut b,json!({"op":"status"}))["state"]["draft"].is_null());
    }
    #[test]
    fn mail_send_slot_bridge_raw_aba_owner_aba_remount_retire_old_transport_without_new_draft_effects(){
        let mut b=ready();let f=reserve(&mut b)["state"]["flight"].clone();call(&mut b,proof("enter",&f));
        let mut edited=raw();edited["subject"]=json!("B");call(&mut b,json!({"op":"remember","incarnation":"1","key":"A","draft":edited}));
        let state=call(&mut b,json!({"op":"remember","incarnation":"1","key":"A","draft":raw()}));assert_ne!(state["state"]["generation"],f["generation"]);
        assert_eq!(ack(&mut b,&f,1)["completion"],"retired");assert_eq!(call(&mut b,json!({"op":"status"}))["state"]["draft"],raw());
        let f=reserve(&mut b)["state"]["flight"].clone();call(&mut b,proof("enter",&f));
        call(&mut b,json!({"op":"sync","incarnation":"1","owner":null}));call(&mut b,json!({"op":"sync","incarnation":"1","owner":owner()}));
        call(&mut b,json!({"op":"remember","incarnation":"1","key":"A","draft":raw()}));
        assert_eq!(reserve(&mut b)["matched"],false);assert_eq!(ack(&mut b,&f,-1)["completion"],"retired");
        let f=reserve(&mut b)["state"]["flight"].clone();call(&mut b,proof("enter",&f));
        let mounted=call(&mut b,json!({"op":"mount"}));assert_eq!(mounted["state"]["incarnation"],"2");assert!(!mounted["state"]["flight"].is_null());
        let completion=call(&mut b,json!({"op":"ack","incarnation":"2","stream":f["stream"],"result":1}));assert_eq!(completion["completion"],"retired");assert_eq!(completion["state"]["draft"],raw());
    }
    #[test]
    fn mail_send_slot_bridge_checked_highwaters_exhaust_permanently_and_new_stream_only_clears_barrier(){
        let mut b=ready();let f=reserve(&mut b)["state"]["flight"].clone();call(&mut b,proof("enter",&f));
        b.clock=MailDraftClock::from_highwater(u64::MAX);let mut edited=raw();edited["body"]=json!("B");call(&mut b,json!({"op":"remember","incarnation":"1","key":"A","draft":edited}));
        assert!(call(&mut b,json!({"op":"status"}))["state"]["generation"].is_null());assert_eq!(ack(&mut b,&f,1)["completion"],"retired");
        call(&mut b,json!({"op":"remember","incarnation":"1","key":"A","draft":raw()}));
        assert_eq!(reserve(&mut b)["matched"],false);let next=call(&mut b,json!({"op":"open"}));assert!(next["state"]["generation"].is_null());assert_eq!(reserve(&mut b)["matched"],false);
        let mut b=ready();b.next_connection=Some(u64::MAX);let last=call(&mut b,json!({"op":"open"}));assert_eq!(last["state"]["stream"]["connection"],u64::MAX.to_string());
        let snapshot=last["state"].clone();assert_eq!(call(&mut b,json!({"op":"open"}))["matched"],false);assert_eq!(call(&mut b,json!({"op":"status"}))["state"],snapshot);
        b.next_owner=None;let mut changed=owner();changed["sessionGeneration"]=json!("9");call(&mut b,json!({"op":"sync","incarnation":"1","owner":changed}));
        call(&mut b,json!({"op":"remember","incarnation":"1","key":"A","draft":raw()}));assert_eq!(reserve(&mut b)["matched"],false);
        let mut b=ready();b.next_incarnation=None;assert_eq!(call(&mut b,json!({"op":"mount"}))["matched"],false);assert!(call(&mut b,json!({"op":"status"}))["state"]["incarnation"].is_null());assert_eq!(reserve(&mut b)["matched"],false);
    }
    #[test]
    #[ignore="Root supplies complete actual Page/compiled bridge transcript; not counted by default"]
    fn mail_actual_page_send_slot_whole_transcript_matches_compiled_core_bridge(){
        let path=std::env::var("MIR2_MAIL_SEND_SLOT_FIXTURE_PATH").expect("explicit Root compiled fixture path");
        let sessions:Value=serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();let sessions=sessions.as_array().expect("sessions");assert!(!sessions.is_empty());
        let mut entered=0;let mut acked=0;
        for session in sessions {let mut bridge=MailSendSlotBridge::new();let rows=session.as_array().expect("rows");assert!(!rows.is_empty());
            for row in rows {let input=row["inputJson"].as_str().expect("exact input");let expected=row["outputJson"].as_str().expect("exact output");
                assert_eq!(serde_json::from_str::<Value>(input).unwrap(),row["input"]);assert_eq!(serde_json::from_str::<Value>(expected).unwrap(),row["output"]);
                let actual=bridge.transact(input);assert_eq!(actual,expected,"whole transcript op {}",row["input"]["op"]);
                if row["input"]["op"]=="enter"&&row["output"]["matched"]==true{entered+=1;}
                if row["input"]["op"]=="ack"&&row["output"]["matched"]==true{acked+=1;}
            }
        }assert!(entered>0);assert!(acked>0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input() -> Value { json!({"recipient":" R ","message":" hello\r\n世界 ","gold":100,
        "attachmentUniqueIds":[9,7],"stamped":true}) }
    #[test]
    fn actual_edge_uses_common_policy_and_preserves_gold_and_ids() {
        let result:Value=serde_json::from_str(&prepare_json(&input().to_string())).unwrap();
        assert_eq!(result,json!({"ok":true,"payload":{"name":"R","message":"hello\n世界",
            "gold":100,"itemsIdx":[9,7,0,0,0],"stamped":true}}));
        assert_eq!(normalize_json(&json!({"message":"a\r\nb\rc\t"}).to_string()),json!({"ok":true,"message":"a\nb\nc"}).to_string());
    }
    #[test]
    fn numeric_schema_and_unicode_boundaries_fail_closed() {
        for (key,bad) in [("gold",json!(-1)),("gold",json!(1.5)),("gold",json!(4294967296_u64)),
            ("attachmentUniqueIds",json!([0])),("attachmentUniqueIds",json!([9,9])),
            ("attachmentUniqueIds",json!([9007199254740992_u64])),("attachmentUniqueIds",json!([1.5])),
            ("stamped",json!(1)),("message",json!("😀".repeat(251)))] {
            let mut value=input();value[key]=bad;
            assert_eq!(serde_json::from_str::<Value>(&prepare_json(&value.to_string())).unwrap()["ok"],false);
        }
        let mut value=input();value["subject"]=json!("ignored?");
        assert_eq!(serde_json::from_str::<Value>(&prepare_json(&value.to_string())).unwrap()["ok"],false);
        assert_eq!(serde_json::from_str::<Value>(&normalize_json(&json!({"message":"😀".repeat(250)}).to_string())).unwrap()["ok"],true);
        assert_eq!(serde_json::from_str::<Value>(&prepare_json("{\"message\":\"\\ud800\"}")).unwrap()["ok"],false);
    }

    #[test]
    #[ignore = "Root supplies the actual Page controlled-port capture; not counted by default"]
    fn actual_page_compose_port_fixture_matches_compiled_shared_policy() {
        let path=std::env::var("MIR2_MAIL_COMPOSE_FIXTURE_PATH").expect("explicit Root fixture path");
        let captures:Value=serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let captures=captures.as_array().expect("capture array");assert!(!captures.is_empty());
        for capture in captures {
            let input=capture["input"].to_string();
            let actual=match capture["kind"].as_str().unwrap() {
                "send"=>prepare_json(&input),"message"=>normalize_json(&input),_=>panic!("unknown capture kind"),
            };
            assert_eq!(serde_json::from_str::<Value>(&actual).unwrap(),capture["output"]);
        }
    }
}
