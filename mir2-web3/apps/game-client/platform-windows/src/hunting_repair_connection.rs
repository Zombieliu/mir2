//! Private custody of one physical connection's repair realm and identity.
//! Wallet HTTP is observation only; it never authorizes a repair operation.
use mir2_client_bevy::hunting_repair::HuntingRepairMode;
use serde::{
    de::{self, MapAccess, SeqAccess, Visitor},
    Deserialize, Deserializer,
};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    io::Read,
    time::Duration,
};

const MAX_HANDSHAKE: usize = 65_536;
const MAX_WALLET: usize = 16_384;
type Result<T> = std::result::Result<T, &'static str>;

// Keep duplicates visible after escape decoding, before a Value can overwrite
// them. This boundary also applies to nested identity and HTTP wallet fields.
struct Unique(Value);
impl<'de> Deserialize<'de> for Unique {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Unique;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("unique JSON fields")
            }
            fn visit_bool<E: de::Error>(self, v: bool) -> std::result::Result<Unique, E> {
                Ok(Unique(v.into()))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> std::result::Result<Unique, E> {
                Ok(Unique(v.into()))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> std::result::Result<Unique, E> {
                Ok(Unique(v.into()))
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> std::result::Result<Unique, E> {
                serde_json::Number::from_f64(v)
                    .map(|n| Unique(n.into()))
                    .ok_or_else(|| E::custom("invalid number"))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> std::result::Result<Unique, E> {
                Ok(Unique(v.into()))
            }
            fn visit_string<E: de::Error>(self, v: String) -> std::result::Result<Unique, E> {
                Ok(Unique(v.into()))
            }
            fn visit_unit<E: de::Error>(self) -> std::result::Result<Unique, E> {
                Ok(Unique(Value::Null))
            }
            fn visit_none<E: de::Error>(self) -> std::result::Result<Unique, E> {
                Ok(Unique(Value::Null))
            }
            fn visit_seq<A: SeqAccess<'de>>(
                self,
                mut a: A,
            ) -> std::result::Result<Unique, A::Error> {
                let mut out = Vec::new();
                while let Some(Unique(v)) = a.next_element()? {
                    out.push(v);
                }
                Ok(Unique(out.into()))
            }
            fn visit_map<A: MapAccess<'de>>(
                self,
                mut a: A,
            ) -> std::result::Result<Unique, A::Error> {
                let mut out = serde_json::Map::new();
                let mut seen = BTreeSet::new();
                while let Some(k) = a.next_key::<String>()? {
                    if !seen.insert(k.clone()) {
                        return Err(de::Error::custom("duplicate field"));
                    }
                    let Unique(v) = a.next_value()?;
                    out.insert(k, v);
                }
                Ok(Unique(out.into()))
            }
        }
        d.deserialize_any(V)
    }
}
fn parse(raw: &str, max: usize) -> Result<Value> {
    if raw.is_empty() || raw.len() > max {
        return Err("Invalid native repair data");
    }
    serde_json::from_str::<Unique>(raw)
        .map(|v| v.0)
        .map_err(|_| "Invalid native repair data")
}
fn object_fields(value: &Value, names: &[&str]) -> bool {
    value
        .as_object()
        .is_some_and(|v| v.len() == names.len() && names.iter().all(|n| v.contains_key(*n)))
}
fn text(value: &Value, max: usize) -> Option<&str> {
    value
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= max && !s.chars().any(char::is_control))
}
fn canonical_decimal(value: &Value) -> Option<u64> {
    let v = value.as_str()?;
    if v.is_empty()
        || v.len() > 20
        || !v.bytes().all(|b| b.is_ascii_digit())
        || v.len() > 1 && v.starts_with('0')
    {
        return None;
    }
    v.parse().ok()
}
fn exact_ms(value: &Value) -> Option<u64> {
    value.as_u64().filter(|v| *v <= 9_007_199_254_740_991)
}
fn marker(raw: &str) -> bool {
    struct M;
    impl<'de> Deserialize<'de> for M {
        fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
            struct V;
            impl<'de> Visitor<'de> for V {
                type Value = M;
                fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    f.write_str("native envelope")
                }
                fn visit_map<A: MapAccess<'de>>(
                    self,
                    mut a: A,
                ) -> std::result::Result<M, A::Error> {
                    while let Some(k) = a.next_key::<String>()? {
                        if k == "type" {
                            let v = a.next_value::<Value>()?;
                            if matches!(v.as_str(), Some("realmInfo" | "identitySession")) {
                                return Err(de::Error::custom("repair connection marker"));
                            }
                        } else {
                            let _ = a.next_value::<de::IgnoredAny>()?;
                        }
                    }
                    Ok(M)
                }
            }
            d.deserialize_map(V)
        }
    }
    serde_json::from_str::<M>(raw)
        .is_err_and(|e| e.to_string().starts_with("repair connection marker"))
}

struct Identity {
    token: String,
    account: String,
    session: String,
    expires: u64,
}
impl fmt::Debug for Identity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Identity(<private>)")
    }
}

#[derive(Debug)]
pub(crate) struct HuntingRepairConnection {
    mode: HuntingRepairMode,
    realm: Option<String>,
    blocked: bool,
    identity: Option<Identity>,
    generation: u64,
    balance: Option<(i32, u64, u64)>,
    revisions: BTreeMap<i32, u64>,
}
impl HuntingRepairConnection {
    pub fn new() -> Self {
        Self {
            mode: HuntingRepairMode::Unknown,
            realm: None,
            blocked: false,
            identity: None,
            generation: 0,
            balance: None,
            revisions: BTreeMap::new(),
        }
    }
    pub fn mode(&self) -> HuntingRepairMode {
        if self.blocked {
            HuntingRepairMode::Unknown
        } else {
            self.mode
        }
    }
    pub fn realm_id(&self) -> Option<&str> {
        (!self.blocked).then_some(self.realm.as_deref()).flatten()
    }
    pub fn account_id(&self) -> Option<&str> {
        (!self.blocked)
            .then_some(self.identity.as_ref().map(|i| i.account.as_str()))
            .flatten()
    }
    pub fn clear_identity(&mut self) {
        self.identity = None;
        self.revisions.clear();
        self.invalidate_wallet();
    }
    pub fn invalidate_wallet(&mut self) {
        self.balance = None;
        match self.generation.checked_add(1) {
            Some(v) => self.generation = v,
            None => {
                self.blocked = true;
                self.identity = None;
            }
        }
    }
    pub fn identity_matches(&self, account: &str, now_ms: u64) -> bool {
        !self.blocked
            && self
                .identity
                .as_ref()
                .is_some_and(|i| i.account.eq_ignore_ascii_case(account) && now_ms < i.expires)
    }
    /// Only a physical socket's original reader may call this method.
    pub fn observe(&mut self, raw: &str) -> Result<bool> {
        if !marker(raw) {
            return Ok(false);
        }
        if self.blocked {
            return Err("Native repair connection retired");
        }
        match self.observe_inner(raw) {
            Ok(()) => Ok(true),
            Err(e) => {
                self.blocked = true;
                self.clear_identity();
                self.mode = HuntingRepairMode::Unknown;
                Err(e)
            }
        }
    }
    fn observe_inner(&mut self, raw: &str) -> Result<()> {
        let v = parse(raw, MAX_HANDSHAKE)?;
        match v["type"].as_str() {
            Some("realmInfo") => {
                if !object_fields(&v, &["type", "payload"])
                    || v["payload"]["schema"] != "mir2-realm-handshake/1"
                {
                    return Err("Invalid native repair realm");
                }
                let realm = text(&v["payload"]["realmId"], 128)
                    .filter(|s| s.trim() == *s)
                    .ok_or("Invalid native repair realm")?;
                let mode = match v["payload"]["npcRepairMode"].as_str() {
                    Some("legacy") => HuntingRepairMode::Legacy,
                    Some("hunting-v1") => HuntingRepairMode::HuntingV1,
                    _ => return Err("Invalid native repair mode"),
                };
                if self
                    .realm
                    .as_ref()
                    .is_some_and(|prior| prior != realm || self.mode != mode)
                {
                    return Err("Native repair realm changed");
                }
                self.realm = Some(realm.into());
                self.mode = mode;
                Ok(())
            }
            Some("identitySession") => {
                if !object_fields(&v, &["type", "token", "session"]) {
                    return Err("Invalid native repair identity");
                }
                let token = v["token"]
                    .as_str()
                    .filter(|s| {
                        !s.is_empty()
                            && s.len() <= 4089
                            && s.bytes()
                                .all(|b| b.is_ascii_alphanumeric() || b"._~-".contains(&b))
                    })
                    .ok_or("Invalid native repair identity")?;
                let session = &v["session"];
                let account =
                    text(&session["accountId"], 128).ok_or("Invalid native repair identity")?;
                let session_id =
                    text(&session["sessionId"], 256).ok_or("Invalid native repair identity")?;
                let expires = exact_ms(&session["expiresAtMs"])
                    .filter(|v| *v > 0)
                    .ok_or("Invalid native repair identity")?;
                if session
                    .as_object()
                    .is_none_or(|v| !v.contains_key("revokedAtMs"))
                    || !session["revokedAtMs"].is_null()
                {
                    return Err("Revoked native repair identity");
                }
                self.clear_identity();
                self.identity = Some(Identity {
                    token: token.into(),
                    account: account.into(),
                    session: session_id.into(),
                    expires,
                });
                Ok(())
            }
            _ => Err("Invalid native repair envelope"),
        }
    }
    pub fn wallet_request(
        &mut self,
        gateway_ws_url: &str,
        public_origin: &str,
        account: &str,
        character_index: i32,
        now_ms: u64,
    ) -> Option<WalletRequest> {
        if self.mode() != HuntingRepairMode::HuntingV1
            || character_index < 0
            || !self.identity_matches(account, now_ms)
        {
            return None;
        }
        let url = wallet_url(gateway_ws_url, character_index)?;
        let origin = valid_origin(public_origin)?;
        self.invalidate_wallet();
        if self.blocked {
            return None;
        }
        let i = self.identity.as_ref()?;
        Some(WalletRequest {
            url,
            origin,
            token: i.token.clone(),
            account: i.account.clone(),
            session: i.session.clone(),
            expires: i.expires,
            generation: self.generation,
            character_index,
        })
    }
    pub fn accept_wallet(
        &mut self,
        response: WalletResponse,
        account: &str,
        character_index: i32,
        now_ms: u64,
    ) -> bool {
        if self.mode() != HuntingRepairMode::HuntingV1
            || !self.identity_matches(account, now_ms)
            || response.character_index != character_index
            || response.generation != self.generation
        {
            return false;
        }
        let Some(i) = self.identity.as_ref() else {
            return false;
        };
        if i.account != response.account
            || i.session != response.session
            || i.expires != response.expires
            || now_ms >= response.expires
        {
            return false;
        }
        if self
            .revisions
            .get(&character_index)
            .is_some_and(|v| response.revision < *v)
            || !self.revisions.contains_key(&character_index) && self.revisions.len() >= 32
        {
            return false;
        }
        self.revisions.insert(character_index, response.revision);
        self.balance = Some((character_index, response.bound, response.unbound));
        true
    }
    pub fn wallet_balance(
        &self,
        account: &str,
        character_index: i32,
        now_ms: u64,
    ) -> Option<(u64, u64)> {
        if self.mode() != HuntingRepairMode::HuntingV1 || !self.identity_matches(account, now_ms) {
            return None;
        }
        self.balance
            .filter(|(index, _, _)| *index == character_index)
            .map(|(_, b, u)| (b, u))
    }
}

fn wallet_url(raw: &str, index: i32) -> Option<String> {
    let uri: tokio_tungstenite::tungstenite::http::Uri = raw.parse().ok()?;
    let scheme = match uri.scheme_str()? {
        "ws" => "http",
        "wss" => "https",
        _ => return None,
    };
    let authority = uri.authority()?.as_str();
    if authority.contains('@') || uri.host()?.is_empty() {
        return None;
    }
    Some(format!(
        "{scheme}://{authority}/v1/economy/hunting-wallet?characterIndex={index}"
    ))
}
fn valid_origin(raw: &str) -> Option<String> {
    let uri: tokio_tungstenite::tungstenite::http::Uri = raw.parse().ok()?;
    if !matches!(uri.scheme_str(), Some("http" | "https"))
        || uri.authority()?.as_str().contains('@')
        || uri.query().is_some()
        || !matches!(uri.path(), "" | "/")
    {
        return None;
    }
    Some(format!(
        "{}://{}",
        uri.scheme_str()?,
        uri.authority()?.as_str()
    ))
}

pub(crate) struct WalletRequest {
    url: String,
    origin: String,
    token: String,
    account: String,
    session: String,
    expires: u64,
    generation: u64,
    character_index: i32,
}
impl fmt::Debug for WalletRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("WalletRequest(<private>)")
    }
}
pub(crate) struct WalletResponse {
    account: String,
    session: String,
    expires: u64,
    generation: u64,
    character_index: i32,
    revision: u64,
    bound: u64,
    unbound: u64,
}
impl fmt::Debug for WalletResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("WalletResponse(<private>)")
    }
}
impl WalletRequest {
    /// Call only on a bounded worker. No redirects, cookies, mutation or private
    /// response/error text are exposed to the host, UI or log.
    pub fn fetch(self) -> Result<WalletResponse> {
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(8))
            .redirects(0)
            .build();
        let response = agent
            .get(&self.url)
            .set("authorization", &format!("Bearer {}", self.token))
            .set("origin", &self.origin)
            .set("accept", "application/json")
            .call()
            .map_err(|_| "Native wallet unavailable")?;
        if response.status() != 200 {
            return Err("Native wallet unavailable");
        }
        let mut body = Vec::new();
        response
            .into_reader()
            .take((MAX_WALLET + 1) as u64)
            .read_to_end(&mut body)
            .map_err(|_| "Native wallet unavailable")?;
        let raw = std::str::from_utf8(&body).map_err(|_| "Invalid native wallet")?;
        self.decode(raw)
    }
    fn decode(self, raw: &str) -> Result<WalletResponse> {
        let v = parse(raw, MAX_WALLET)?;
        if !object_fields(
            &v,
            &[
                "characterIndex",
                "initialized",
                "enabled",
                "boundGold",
                "unboundGold",
                "walletRevision",
                "permitStockAtoms",
                "policyVersion",
                "settledThroughDay",
            ],
        ) || v["characterIndex"].as_i64() != Some(i64::from(self.character_index))
            || v["initialized"].as_bool().is_none()
            || v["enabled"].as_bool().is_none()
            || v["settledThroughDay"].as_i64().is_none()
            || text(&v["policyVersion"], 256).is_none()
        {
            return Err("Invalid native wallet");
        }
        let bound = canonical_decimal(&v["boundGold"]).ok_or("Invalid native wallet")?;
        let unbound = canonical_decimal(&v["unboundGold"]).ok_or("Invalid native wallet")?;
        let revision = canonical_decimal(&v["walletRevision"]).ok_or("Invalid native wallet")?;
        canonical_decimal(&v["permitStockAtoms"]).ok_or("Invalid native wallet")?;
        if v["initialized"] == false && (bound != 0 || unbound != 0 || revision != 0) {
            return Err("Invalid native wallet");
        }
        Ok(WalletResponse {
            account: self.account,
            session: self.session,
            expires: self.expires,
            generation: self.generation,
            character_index: self.character_index,
            revision,
            bound,
            unbound,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn realm(mode: &str) -> String {
        json!({"type":"realmInfo","payload":{"schema":"mir2-realm-handshake/1","realmId":"fixture","npcRepairMode":mode,"profileId":"test"}}).to_string()
    }
    fn identity(token: &str, session: &str) -> String {
        json!({"type":"identitySession","token":token,"session":{"accountId":"alice","sessionId":session,"expiresAtMs":9000,"revokedAtMs":null}}).to_string()
    }
    fn live() -> HuntingRepairConnection {
        let mut c = HuntingRepairConnection::new();
        c.observe(&realm("hunting-v1")).unwrap();
        c.observe(&identity("private-bearer", "session1")).unwrap();
        c
    }
    fn wallet(bound: &str, revision: &str) -> String {
        json!({"characterIndex":0,"initialized":true,"enabled":true,"boundGold":bound,"unboundGold":"0","walletRevision":revision,"permitStockAtoms":"0","policyVersion":"test","settledThroughDay":1}).to_string()
    }
    #[test]
    fn native_hunting_connection_mode_is_physical_and_cannot_downgrade() {
        let mut c = live();
        assert_eq!(c.mode(), HuntingRepairMode::HuntingV1);
        assert!(c.observe(&realm("hunting-v1")).unwrap());
        assert!(c.observe(&realm("legacy")).is_err());
        assert_eq!(c.mode(), HuntingRepairMode::Unknown);
        assert!(!c.identity_matches("alice", 1));
        assert!(c.observe(&realm("hunting-v1")).is_err());
    }
    #[test]
    fn native_hunting_connection_rejects_escaped_duplicates_before_value() {
        for raw in [
            r#"{"type":"realmInfo","t\u0079pe":"realmInfo","payload":{}}"#,
            r#"{"type":"identitySession","token":"a","session":{"accountId":"alice","account\u0049d":"alice"}}"#,
        ] {
            let mut c = live();
            assert!(c.observe(raw).is_err());
            assert_eq!(c.mode(), HuntingRepairMode::Unknown);
        }
    }
    #[test]
    fn native_hunting_connection_identity_is_private_exact_and_expiring() {
        let mut c = live();
        assert!(c.identity_matches("ALICE", 1));
        assert_eq!(c.account_id(), Some("alice"));
        assert!(!c.identity_matches("bob", 1));
        assert!(!c.identity_matches("alice", 9000));
        let debug = format!("{c:?}");
        assert!(!debug.contains("private-bearer"));
        for token in ["", "secret\n", "a b", "a/b"] {
            let mut c = live();
            assert!(c.observe(&identity(token, "new")).is_err());
            assert!(!c.identity_matches("alice", 1));
        }
        c.clear_identity();
        assert_eq!(c.mode(), HuntingRepairMode::HuntingV1);
    }
    #[test]
    fn native_hunting_wallet_is_fixed_origin_and_exact_u64_observation() {
        let mut c = live();
        let q = c
            .wallet_request(
                "wss://example.com/custom/ws?x=1",
                "https://play.example.com/",
                "ALICE",
                0,
                1,
            )
            .unwrap();
        assert_eq!(
            q.url,
            "https://example.com/v1/economy/hunting-wallet?characterIndex=0"
        );
        assert_eq!(q.origin, "https://play.example.com");
        assert!(!format!("{q:?}").contains("private-bearer"));
        let response = q.decode(&wallet(&u64::MAX.to_string(), "9")).unwrap();
        assert!(c.accept_wallet(response, "alice", 0, 2));
        assert_eq!(c.wallet_balance("alice", 0, 2), Some((u64::MAX, 0)));
        assert_eq!(c.wallet_balance("alice", 1, 2), None);
        assert_eq!(c.wallet_balance("alice", 0, 9000), None);
    }
    #[test]
    fn native_hunting_wallet_rejects_late_and_revision_downgrade() {
        let mut c = live();
        let q = c
            .wallet_request("ws://127.0.0.1:1/ws", "http://127.0.0.1:2", "alice", 0, 1)
            .unwrap();
        let response = q.decode(&wallet("10", "9")).unwrap();
        assert!(c.accept_wallet(response, "alice", 0, 2));
        let q = c
            .wallet_request("ws://127.0.0.1:1/ws", "http://127.0.0.1:2", "alice", 0, 3)
            .unwrap();
        let low = q.decode(&wallet("99", "8")).unwrap();
        assert!(!c.accept_wallet(low, "alice", 0, 4));
        assert_eq!(c.wallet_balance("alice", 0, 4), None);
        let q = c
            .wallet_request("ws://127.0.0.1:1/ws", "http://127.0.0.1:2", "alice", 0, 5)
            .unwrap();
        let response = q.decode(&wallet("11", "10")).unwrap();
        c.invalidate_wallet();
        assert!(!c.accept_wallet(response, "alice", 0, 6));
    }
    #[test]
    fn native_hunting_wallet_never_accepts_foreign_session_or_partial_json() {
        for raw in [
            wallet("01", "1"),
            wallet("18446744073709551616", "1"),
            wallet("1", "1").replace("\"boundGold\":\"1\"", "\"boundGold\":1"),
            "{}".into(),
        ] {
            let mut c = live();
            let q = c
                .wallet_request("ws://127.0.0.1:1/ws", "http://127.0.0.1:2", "alice", 0, 1)
                .unwrap();
            assert!(q.decode(&raw).is_err());
        }
        let mut c = live();
        let q = c
            .wallet_request("ws://127.0.0.1:1/ws", "http://127.0.0.1:2", "alice", 0, 1)
            .unwrap();
        let r = q.decode(&wallet("10", "9")).unwrap();
        c.observe(&identity("other-private", "session2")).unwrap();
        assert!(!c.accept_wallet(r, "alice", 0, 2));
    }
    #[test]
    fn native_hunting_wallet_rejects_origin_paths_credentials_and_legacy_mode() {
        for origin in [
            "https://a.example/path",
            "https://a.example?q=1",
            "https://user@a.example",
            "ftp://a.example",
            "",
        ] {
            assert!(valid_origin(origin).is_none());
        }
        assert!(wallet_url("ws://user:pass@example.com/ws", 0).is_none());
        assert!(wallet_url("https://example.com/ws", 0).is_none());
        let mut c = live();
        assert!(c
            .wallet_request("ws://127.0.0.1:1/ws", "http://127.0.0.1:2", "bob", 0, 1)
            .is_none());
        let mut c = HuntingRepairConnection::new();
        c.observe(&realm("legacy")).unwrap();
        c.observe(&identity("a", "s")).unwrap();
        assert!(c
            .wallet_request("ws://127.0.0.1:1/ws", "http://127.0.0.1:2", "alice", 0, 1)
            .is_none());
    }
    #[test]
    fn native_hunting_connection_ordinary_data_is_not_identity_authority() {
        let mut c = HuntingRepairConnection::new();
        assert!(!c
            .observe(r#"{"type":"packet","payload":{"type":"identitySession"}}"#)
            .unwrap());
        assert!(!c.identity_matches("alice", 1));
        assert_eq!(c.mode(), HuntingRepairMode::Unknown);
    }
}
