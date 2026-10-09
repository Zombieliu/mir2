//! Bounded local Native mode reducer. No Core token, gameplay authority, send or retry.
use std::cell::RefCell;
use std::time::Duration;
use mir2_client_bevy::combat_mode_keys::{CombatModeInstant, CombatModes, ModeRequest};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const SAFE: u64 = 9_007_199_254_740_991;
const MAX_INPUT: usize = 16 * 1024;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Owner {
    generation: u64, connection_generation: u64, session_generation: u64,
    player_object_id: u32, scene_revision: u64, map_file_name: String,
}
impl Owner {
    fn valid(&self) -> bool {
        self.generation > 0 && self.generation <= SAFE
            && self.connection_generation > 0 && self.connection_generation <= SAFE
            && self.session_generation > 0 && self.session_generation <= SAFE
            && self.player_object_id > 0 && self.scene_revision <= SAFE
            && !self.map_file_name.is_empty() && self.map_file_name.len() <= 512
            && !self.map_file_name.chars().any(char::is_control)
    }
    fn physical(&self, other: &Self) -> bool {
        self.generation == other.generation && self.connection_generation == other.connection_generation
            && self.session_generation == other.session_generation && self.player_object_id == other.player_object_id
    }
    fn can_replace(&self, before: &Self) -> bool {
        self.generation > before.generation || self.generation == before.generation
            && self.connection_generation == before.connection_generation && self.session_generation > before.session_generation
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
enum Command {
    #[serde(rename = "changeAMode")] ChangeAMode { mode: u8 },
    #[serde(rename = "changePMode")] ChangePMode { mode: u8 },
}
impl Command {
    fn valid(&self) -> bool { match self { Self::ChangeAMode { mode } => *mode < 6, Self::ChangePMode { mode } => *mode < 5 } }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Proof { request_id: u64, owner: Owner, revision: u64, function: String, command: Command }
#[derive(Debug, Deserialize)]
#[serde(tag = "operation", rename_all = "camelCase", deny_unknown_fields)]
enum Input {
    Snapshot { version: u8, owner: Owner, revision: u64, #[serde(rename="clockMs")] clock_ms: u64,
        #[serde(rename="attackMode", deserialize_with="required_mode")] attack_mode: Option<u8>,
        #[serde(rename="petMode", deserialize_with="required_mode")] pet_mode: Option<u8> },
    Request { version: u8, owner: Owner, revision: u64, #[serde(rename="clockMs")] clock_ms: u64, function: String },
    Claim { version: u8, owner: Owner, revision: u64, #[serde(rename="clockMs")] clock_ms: u64, proof: Proof, command: Command },
    DefinitelyUnsent { version: u8, owner: Owner, #[serde(rename="clockMs")] clock_ms: u64, proof: Proof },
    OutcomeUnknown { version: u8, owner: Owner, #[serde(rename="clockMs")] clock_ms: u64, proof: Proof },
    Receipt { version: u8, owner: Owner, revision: u64, #[serde(rename="clockMs")] clock_ms: u64, packet: String, mode: u8 },
}
fn required_mode<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<u8>, D::Error> { Option::<u8>::deserialize(d) }
struct Host {
    owner: Option<Owner>, revision: u64, modes: CombatModes, attack_known: bool, pet_known: bool,
    origin: CombatModeInstant, clock_ms: Option<u64>, next_id: u64, claimed_through: u64,
    reserved: Option<Proof>, last_entered: Option<Proof>,
}
impl Default for Host { fn default() -> Self { Self::new(CombatModeInstant::now()) } }
impl Host {
    fn new(origin: CombatModeInstant) -> Self { Self { owner: None, revision: 0, modes: CombatModes::default(),
        attack_known: false, pet_known: false, origin, clock_ms: None, next_id: 0, claimed_through: 0, reserved: None, last_entered: None } }
    fn clock(&mut self, ms: u64) -> Result<CombatModeInstant, &'static str> {
        if ms > SAFE || self.clock_ms.is_some_and(|old| ms < old) { return Err("invalidClock"); }
        let now = self.origin.checked_add(Duration::from_millis(ms)).ok_or("invalidClock")?;
        self.clock_ms = Some(ms); Ok(now)
    }
    fn current(&self, owner: &Owner, revision: u64) -> bool {
        owner.valid() && revision <= SAFE && self.owner.as_ref() == Some(owner) && self.revision == revision
    }
    fn retire_reserved(&mut self) {
        if let Some(proof) = self.reserved.take() { self.modes.failed_cycle(&proof.function); }
    }
    fn run(&mut self, input: Input) -> Result<Value, &'static str> {
        match input {
            Input::Snapshot { version, owner, revision, clock_ms, attack_mode, pet_mode } => {
                if version != 1 || !owner.valid() || revision > SAFE || attack_mode.is_some_and(|m| m >= 6) || pet_mode.is_some_and(|m| m >= 5) { return Err("invalidSnapshot"); }
                let changed_physical = self.owner.as_ref().is_none_or(|old| !old.physical(&owner));
                if let Some(old) = &self.owner {
                    if changed_physical && !owner.can_replace(old) { return Err("retiredOwner"); }
                    if !changed_physical {
                        if revision < self.revision || owner.scene_revision < old.scene_revision
                            || owner.map_file_name != old.map_file_name && owner.scene_revision == old.scene_revision { return Err("staleSnapshot"); }
                        let previous_attack = self.attack_known.then_some(self.modes.attack);
                        let previous_pet = self.pet_known.then_some(self.modes.pet);
                        if revision == self.revision && (attack_mode != previous_attack || pet_mode != previous_pet) { return Err("reusedRevision"); }
                    }
                }
                self.clock(clock_ms)?;
                if changed_physical {
                    self.modes = CombatModes::default(); self.reserved = None; self.last_entered = None;
                } else if self.owner.as_ref() != Some(&owner) || revision > self.revision { self.retire_reserved(); }
                self.owner = Some(owner); self.revision = revision;
                self.attack_known = attack_mode.is_some(); self.pet_known = pet_mode.is_some();
                if let Some(mode) = attack_mode { self.modes.attack = mode; }
                if let Some(mode) = pet_mode { self.modes.pet = mode; }
                Ok(json!({"observedSnapshot":true}))
            }
            Input::Request { version, owner, revision, clock_ms, function } => {
                if version != 1 || !self.current(&owner, revision) || function.is_empty() || function.len() > 64 || !function.is_ascii() { return Err("invalidRequest"); }
                let now = self.clock(clock_ms)?;
                if self.reserved.is_some() { return Err("reservedBusy"); }
                if function == "ChangeAttackmode" && !self.attack_known || function == "ChangePetmode" && !self.pet_known { return Err("unknownMode"); }
                if self.next_id >= SAFE { return Err("requestIdExhausted"); }
                let command = match self.modes.request(&function, now).ok_or("notReady")? {
                    ModeRequest::Attack(mode) => Command::ChangeAMode { mode },
                    ModeRequest::Pet(mode) => Command::ChangePMode { mode },
                };
                self.next_id += 1;
                let proof = Proof { request_id: self.next_id, owner, revision, function, command: command.clone() };
                self.reserved = Some(proof.clone()); Ok(json!({"proof":proof,"command":command}))
            }
            Input::Claim { version, owner, revision, clock_ms, proof, command } => {
                if version != 1 || !self.current(&owner, revision) || !command.valid()
                    || proof.owner != owner || proof.revision != revision || proof.command != command
                    || proof.request_id <= self.claimed_through || self.reserved.as_ref() != Some(&proof) { return Err("invalidClaim"); }
                self.clock(clock_ms)?; self.claimed_through = proof.request_id;
                self.reserved = None; self.last_entered = Some(proof); Ok(json!({"entered":true}))
            }
            Input::DefinitelyUnsent { version, owner, clock_ms, proof } => {
                if version != 1 || owner != proof.owner || !self.owner.as_ref().is_some_and(|current| current.physical(&owner))
                    || self.reserved.as_ref() != Some(&proof) { return Err("notReserved"); }
                self.clock(clock_ms)?; self.retire_reserved(); Ok(json!({"cancelled":true}))
            }
            Input::OutcomeUnknown { version, owner, clock_ms, proof } => {
                if version != 1 || owner != proof.owner || !self.owner.as_ref().is_some_and(|current| current.physical(&owner))
                    || self.last_entered.as_ref() != Some(&proof) { return Err("notEntered"); }
                self.clock(clock_ms)?; Ok(json!({"entered":true,"outcomeUnknown":true}))
            }
            Input::Receipt { version, owner, revision, clock_ms, packet, mode } => {
                if version != 1 || !owner.valid() || revision > SAFE || revision <= self.revision
                    || !self.owner.as_ref().is_some_and(|current| current.physical(&owner)) { return Err("invalidReceiptOwner"); }
                let packet = match packet.as_str() {
                    "ChangeAMode" if mode < 6 => mir2_protocol::ServerPacket::ChangeAMode { mode },
                    "ChangePMode" if mode < 5 => mir2_protocol::ServerPacket::ChangePMode { mode },
                    _ => return Err("invalidReceipt"),
                };
                self.clock(clock_ms)?; self.retire_reserved(); self.revision = revision;
                match &packet { mir2_protocol::ServerPacket::ChangeAMode { .. } => self.attack_known = true,
                    mir2_protocol::ServerPacket::ChangePMode { .. } => self.pet_known = true, _ => unreachable!() }
                let notice = self.modes.observe(&packet).ok_or("invalidReceipt")?;
                // A packet observes actual state, never retires entered proofs or grants replay.
                Ok(json!({"observedReceipt":true,"notice":notice}))
            }
        }
    }
    fn process(&mut self, input: &str) -> String {
        let result = if input.len() > MAX_INPUT { Err("inputTooLarge") } else {
            crate::item_tooltip_query::strict_json_value::<256>(input)
                .and_then(|value| serde_json::from_value::<Input>(value).map_err(|_| "invalidInput"))
                .and_then(|input| self.run(input))
        };
        match result {
            Ok(Value::Object(mut fields)) => { fields.insert("version".into(),json!(1)); fields.insert("ok".into(),json!(true)); Value::Object(fields).to_string() },
            Ok(_) => unreachable!(),
            Err(error) => json!({"version":1,"ok":false,"error":error}).to_string(),
        }
    }
}
thread_local! { static HOST: RefCell<Host> = RefCell::new(Host::default()); }
pub fn process_json(input: &str) -> String { HOST.with(|host| host.borrow_mut().process(input)) }

#[cfg(test)]
mod tests {
    use super::*;
    fn owner() -> Value { json!({"generation":1,"connectionGeneration":1,"sessionGeneration":1,"playerObjectId":1000,"sceneRevision":1,"mapFileName":"0"}) }
    fn call(host: &mut Host, input: Value) -> Value { serde_json::from_str(&host.process(&input.to_string())).unwrap() }
    fn snapshot(host: &mut Host) { assert_eq!(call(host,json!({"version":1,"operation":"snapshot","owner":owner(),"revision":1,"clockMs":0,"attackMode":0,"petMode":0}))["ok"],true); }
    fn request(host: &mut Host, function: &str, clock: u64, revision: u64) -> Value { call(host,json!({"version":1,"operation":"request","owner":owner(),"revision":revision,"clockMs":clock,"function":function})) }
    fn claim(host: &mut Host, response: &Value, clock: u64) -> Value { call(host,json!({"version":1,"operation":"claim","owner":owner(),"revision":response["proof"]["revision"],"clockMs":clock,"proof":response["proof"],"command":response["command"]})) }
    #[test]
    fn mode_keys_native_cycles_keep_strict_independent_delays_without_ack_barrier() {
        let mut host=Host::default(); snapshot(&mut host);
        let first=request(&mut host,"ChangeAttackmode",0,1); assert_eq!(first["command"],json!({"type":"changeAMode","mode":1}));
        assert_eq!(claim(&mut host,&first,0)["entered"],true);
        assert_eq!(request(&mut host,"ChangeAttackmode",300,1)["ok"],false);
        let second=request(&mut host,"ChangeAttackmode",301,1); assert_eq!(second["command"],first["command"]);
        assert_ne!(second["proof"]["requestId"],first["proof"]["requestId"]); assert_eq!(claim(&mut host,&second,301)["ok"],true);
        let pet=request(&mut host,"ChangePetmode",301,1); assert_eq!(claim(&mut host,&pet,301)["ok"],true);
        assert_eq!(request(&mut host,"ChangePetmode",801,1)["ok"],false);
        assert_eq!(request(&mut host,"ChangePetmode",802,1)["ok"],true);
        assert_eq!(host.modes.attack,0); assert_eq!(host.modes.pet,0);
    }
    #[test]
    fn mode_keys_claim_is_exact_and_unknown_never_rolls_back_or_replays() {
        let mut host=Host::default(); snapshot(&mut host); let first=request(&mut host,"ChangeAttackmode",0,1);
        let mut bad=first.clone(); bad["command"]["mode"]=json!(2); assert_eq!(claim(&mut host,&bad,0)["ok"],false);
        assert_eq!(claim(&mut host,&first,0)["ok"],true); assert_eq!(claim(&mut host,&first,1)["ok"],false);
        assert_eq!(call(&mut host,json!({"version":1,"operation":"definitelyUnsent","owner":owner(),"clockMs":1,"proof":first["proof"]}))["ok"],false);
        assert_eq!(call(&mut host,json!({"version":1,"operation":"outcomeUnknown","owner":owner(),"clockMs":1,"proof":first["proof"]}))["outcomeUnknown"],true);
        assert_eq!(request(&mut host,"ChangeAttackmode",300,1)["ok"],false);
        let next=request(&mut host,"ChangeAttackmode",301,1); assert_eq!(next["ok"],true);
        assert_eq!(claim(&mut host,&first,301)["ok"],false); assert_eq!(claim(&mut host,&next,301)["ok"],true);
    }
    #[test]
    fn mode_keys_only_exact_unentered_proof_can_release_native_failed_cycle() {
        let mut host=Host::default(); snapshot(&mut host); let first=request(&mut host,"ChangePetmode",0,1);
        let mut proof=first["proof"].clone(); proof["requestId"]=json!(2);
        assert_eq!(call(&mut host,json!({"version":1,"operation":"definitelyUnsent","owner":owner(),"clockMs":1,"proof":proof}))["ok"],false);
        assert_eq!(call(&mut host,json!({"version":1,"operation":"definitelyUnsent","owner":owner(),"clockMs":1,"proof":first["proof"]}))["cancelled"],true);
        let next=request(&mut host,"ChangePetmode",1,1); assert_eq!(next["ok"],true);
        assert_eq!(claim(&mut host,&first,1)["ok"],false); assert_eq!(claim(&mut host,&next,1)["ok"],true);
    }
    #[test]
    fn mode_keys_real_receipt_updates_observation_and_snapshot_is_not_ack() {
        let mut host=Host::default(); snapshot(&mut host); let first=request(&mut host,"ChangeAttackmode",0,1); assert_eq!(claim(&mut host,&first,0)["ok"],true);
        let mut other=owner(); other["playerObjectId"]=json!(2000);
        assert_eq!(call(&mut host,json!({"version":1,"operation":"receipt","owner":other,"revision":2,"clockMs":100,"packet":"ChangeAMode","mode":5}))["ok"],false);
        let ack=call(&mut host,json!({"version":1,"operation":"receipt","owner":owner(),"revision":2,"clockMs":100,"packet":"ChangeAMode","mode":5}));
        assert_eq!(ack["observedReceipt"],true); assert_eq!(host.modes.attack,5);
        assert_eq!(request(&mut host,"ChangeAttackmode",300,2)["ok"],false);
        assert_eq!(request(&mut host,"ChangeAttackmode",301,2)["command"],json!({"type":"changeAMode","mode":0}));
        assert_eq!(host.last_entered.as_ref().unwrap().request_id,first["proof"]["requestId"].as_u64().unwrap());
    }
    #[test]
    fn mode_keys_unknown_cycle_fails_closed_but_direct_is_native_and_not_optimistic() {
        let mut host=Host::default();
        assert_eq!(call(&mut host,json!({"version":1,"operation":"snapshot","owner":owner(),"revision":1,"clockMs":0,"attackMode":null,"petMode":null}))["ok"],true);
        assert_eq!(request(&mut host,"ChangeAttackmode",0,1)["error"],"unknownMode");
        assert_eq!(request(&mut host,"ChangePetmode",0,1)["error"],"unknownMode");
        let direct=request(&mut host,"AttackmodeEnemyguild",0,1); assert_eq!(direct["command"],json!({"type":"changeAMode","mode":3}));
        assert_eq!(claim(&mut host,&direct,0)["ok"],true); assert!(!host.attack_known);
        let pet=request(&mut host,"PetmodeFocusMasterTarget",0,1); assert_eq!(pet["command"],json!({"type":"changePMode","mode":4}));
    }
    #[test]
    fn mode_keys_owner_scene_revision_clock_and_json_boundaries_are_strict() {
        let mut host=Host::default(); snapshot(&mut host); let first=request(&mut host,"ChangeAttackmode",0,1);
        let mut next_owner=owner(); next_owner["sceneRevision"]=json!(2); next_owner["mapFileName"]=json!("1");
        assert_eq!(call(&mut host,json!({"version":1,"operation":"snapshot","owner":next_owner,"revision":2,"clockMs":1,"attackMode":0,"petMode":0}))["ok"],true);
        assert_eq!(claim(&mut host,&first,1)["ok"],false);
        assert_eq!(request(&mut host,"ChangeAttackmode",1,2)["ok"],false);
        let mut next_owner=owner(); next_owner["generation"]=json!(2); next_owner["connectionGeneration"]=json!(2);
        assert_eq!(call(&mut host,json!({"version":1,"operation":"snapshot","owner":next_owner,"revision":0,"clockMs":2,"attackMode":0,"petMode":0}))["ok"],true);
        assert_eq!(call(&mut host,json!({"version":1,"operation":"snapshot","owner":owner(),"revision":99,"clockMs":3,"attackMode":0,"petMode":0}))["error"],"retiredOwner");
        for raw in [r#"{"version":1,"version":1,"operation":"snapshot"}"#, r#"{"version":1,"operation":"snapshot","extra":1}"#] { assert_eq!(serde_json::from_str::<Value>(&host.process(raw)).unwrap()["ok"],false); }
        assert_eq!(serde_json::from_str::<Value>(&host.process(&"x".repeat(16385))).unwrap()["error"],"inputTooLarge");
        let invalid=json!({"version":1,"operation":"snapshot","owner":next_owner,"revision":1,"clockMs":1,"attackMode":0,"petMode":0});
        assert_eq!(call(&mut host,invalid)["error"],"invalidClock");

        // Entered cycles retain their Native timers across a same-physical scene change.
        let mut entered=Host::default(); snapshot(&mut entered);
        let proof=request(&mut entered,"ChangeAttackmode",0,1); assert_eq!(claim(&mut entered,&proof,0)["ok"],true);
        let mut scene=owner(); scene["sceneRevision"]=json!(2); scene["mapFileName"]=json!("1");
        assert_eq!(call(&mut entered,json!({"version":1,"operation":"snapshot","owner":scene,"revision":2,"clockMs":1,"attackMode":0,"petMode":0}))["ok"],true);
        assert_eq!(call(&mut entered,json!({"version":1,"operation":"request","owner":scene,"revision":2,"clockMs":300,"function":"ChangeAttackmode"}))["error"],"notReady");
        assert_eq!(call(&mut entered,json!({"version":1,"operation":"request","owner":scene,"revision":2,"clockMs":301,"function":"ChangeAttackmode"}))["ok"],true);
        let mut session=scene.clone(); session["sessionGeneration"]=json!(2); session["playerObjectId"]=json!(2000); session["sceneRevision"]=json!(0);
        assert_eq!(call(&mut entered,json!({"version":1,"operation":"snapshot","owner":session,"revision":0,"clockMs":301,"attackMode":0,"petMode":0}))["ok"],true);
        assert_eq!(call(&mut entered,json!({"version":1,"operation":"request","owner":session,"revision":0,"clockMs":301,"function":"ChangeAttackmode"}))["ok"],true);
        let mut bounds=Host::default(); snapshot(&mut bounds);
        for (field,value) in [("generation",json!(SAFE+1)),("connectionGeneration",json!(0)),("sessionGeneration",json!(SAFE+1)),("playerObjectId",json!(0)),("sceneRevision",json!(SAFE+1)),("mapFileName",json!("x".repeat(513))),("mapFileName",json!("0\n"))] {
            let mut invalid_owner=owner(); invalid_owner[field]=value;
            assert_eq!(call(&mut bounds,json!({"version":1,"operation":"snapshot","owner":invalid_owner,"revision":2,"clockMs":0,"attackMode":0,"petMode":0}))["ok"],false,"invalid owner {field}");
        }
        for field in ["revision","clockMs"] {
            let mut input=json!({"version":1,"operation":"snapshot","owner":owner(),"revision":2,"clockMs":0,"attackMode":0,"petMode":0}); input[field]=json!(SAFE+1);
            assert_eq!(call(&mut bounds,input)["ok"],false,"unsafe integer {field}");
        }
        let mut missing=json!({"version":1,"operation":"snapshot","owner":owner(),"revision":2,"clockMs":0,"attackMode":null,"petMode":null}); missing.as_object_mut().unwrap().remove("attackMode");
        assert_eq!(call(&mut bounds,missing)["error"],"invalidInput");
        let reserved=request(&mut bounds,"ChangeAttackmode",0,1); let mut extended=reserved.clone(); extended["proof"]["extra"]=json!(true);
        assert_eq!(claim(&mut bounds,&extended,0)["error"],"invalidInput"); assert_eq!(claim(&mut bounds,&reserved,0)["ok"],true);
    }
}
