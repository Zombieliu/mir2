//! Non-bearer metadata custody for one actual material WebSocket reader.
//!
//! Parsing this handshake does not authenticate another socket, authorize a
//! dispatch or prove a gameplay apply. The future reader must also verify its
//! actual bootstrap actor and current native fence. Never retain the token:
//! material operations use the already authenticated physical socket.
use serde::{
    de::{self, MapAccess, SeqAccess, Visitor},
    Deserialize, Deserializer,
};
use serde_json::Value;
use std::{collections::BTreeSet, fmt, sync::Arc};

const MAX_HANDSHAKE_BYTES: usize = 65_536;
const MAX_DEPTH: usize = 16;
const MAX_NODES: usize = 2048;
const MAX_FIELDS: usize = 64;
const MAX_STRING_BYTES: usize = 4096;
const MAX_EXACT_MS: u64 = 9_007_199_254_740_991;
const MAX_FUTURE_ISSUE_SKEW_MS: u64 = 30_000;
const PROTOCOL_VERSION: u16 = 1;
type Result<T> = std::result::Result<T, &'static str>;

/// Preserve duplicates after escape decoding instead of letting Value replace
/// earlier security fields. The same rule applies inside unrelated profile data.
struct Unique(Value);
impl<'de> Deserialize<'de> for Unique {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        struct UniqueVisitor;
        impl<'de> Visitor<'de> for UniqueVisitor {
            type Value = Unique;
            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("bounded unique JSON fields")
            }
            fn visit_bool<E: de::Error>(self, value: bool) -> std::result::Result<Unique, E> {
                Ok(Unique(value.into()))
            }
            fn visit_i64<E: de::Error>(self, value: i64) -> std::result::Result<Unique, E> {
                Ok(Unique(value.into()))
            }
            fn visit_u64<E: de::Error>(self, value: u64) -> std::result::Result<Unique, E> {
                Ok(Unique(value.into()))
            }
            fn visit_f64<E: de::Error>(self, value: f64) -> std::result::Result<Unique, E> {
                serde_json::Number::from_f64(value)
                    .map(|value| Unique(value.into()))
                    .ok_or_else(|| E::custom("invalid number"))
            }
            fn visit_str<E: de::Error>(self, value: &str) -> std::result::Result<Unique, E> {
                Ok(Unique(value.into()))
            }
            fn visit_string<E: de::Error>(self, value: String) -> std::result::Result<Unique, E> {
                Ok(Unique(value.into()))
            }
            fn visit_unit<E: de::Error>(self) -> std::result::Result<Unique, E> {
                Ok(Unique(Value::Null))
            }
            fn visit_none<E: de::Error>(self) -> std::result::Result<Unique, E> {
                Ok(Unique(Value::Null))
            }
            fn visit_seq<A: SeqAccess<'de>>(
                self,
                mut access: A,
            ) -> std::result::Result<Unique, A::Error> {
                let mut values = Vec::new();
                while let Some(Unique(value)) = access.next_element()? {
                    if values.len() >= MAX_NODES {
                        return Err(de::Error::custom("too many values"));
                    }
                    values.push(value);
                }
                Ok(Unique(values.into()))
            }
            fn visit_map<A: MapAccess<'de>>(
                self,
                mut access: A,
            ) -> std::result::Result<Unique, A::Error> {
                let mut values = serde_json::Map::new();
                let mut seen = BTreeSet::new();
                while let Some(key) = access.next_key::<String>()? {
                    if seen.len() >= MAX_FIELDS || key.len() > 128 || !seen.insert(key.clone()) {
                        return Err(de::Error::custom("invalid or duplicate field"));
                    }
                    let Unique(value) = access.next_value()?;
                    values.insert(key, value);
                }
                Ok(Unique(values.into()))
            }
        }
        deserializer.deserialize_any(UniqueVisitor)
    }
}

fn bounded(value: &Value, depth: usize, remaining: &mut usize) -> bool {
    if depth > MAX_DEPTH || *remaining == 0 {
        return false;
    }
    *remaining -= 1;
    match value {
        Value::String(value) => value.len() <= MAX_STRING_BYTES,
        Value::Array(values) => values
            .iter()
            .all(|value| bounded(value, depth + 1, remaining)),
        Value::Object(values) => values
            .values()
            .all(|value| bounded(value, depth + 1, remaining)),
        _ => true,
    }
}

fn parse(raw: &str) -> Result<Value> {
    if raw.is_empty() || raw.len() > MAX_HANDSHAKE_BYTES {
        return Err("Invalid native material metadata");
    }
    let Unique(value) =
        serde_json::from_str::<Unique>(raw).map_err(|_| "Invalid native material metadata")?;
    let mut remaining = MAX_NODES;
    if !bounded(&value, 0, &mut remaining) {
        return Err("Native material metadata exceeds limits");
    }
    Ok(value)
}

fn quoted_end(bytes: &[u8], start: usize) -> Option<usize> {
    if bytes.get(start) != Some(&b'"') {
        return None;
    }
    let mut index = start + 1;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => return Some(index + 1),
            b'\\' => index += 2,
            _ => index += 1,
        }
    }
    None
}

fn whitespace(bytes: &[u8], mut index: usize) -> usize {
    while bytes.get(index).is_some_and(u8::is_ascii_whitespace) {
        index += 1;
    }
    index
}

/// Recognize decoded root type keys even when another field is malformed. A
/// complete Value probe can otherwise stop before a later claimed metadata key.
/// Nested type fields and quoted chat strings never claim this envelope.
fn claims_metadata(raw: &str) -> bool {
    let bytes = raw.as_bytes();
    let mut depth = 0usize;
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'{' | b'[' => {
                depth += 1;
                index += 1;
            }
            b'}' | b']' => {
                depth = depth.saturating_sub(1);
                index += 1;
            }
            b'"' => {
                let Some(end) = quoted_end(bytes, index) else {
                    return false;
                };
                if depth == 1 && end - index <= 32 {
                    let key = serde_json::from_str::<String>(&raw[index..end]);
                    let colon = whitespace(bytes, end);
                    if matches!(key.as_deref(), Ok("type")) && bytes.get(colon) == Some(&b':') {
                        let start = whitespace(bytes, colon + 1);
                        if let Some(value_end) = quoted_end(bytes, start) {
                            if value_end - start <= 128 {
                                let value = serde_json::from_str::<String>(&raw[start..value_end]);
                                if matches!(value.as_deref(), Ok("realmInfo" | "identitySession")) {
                                    return true;
                                }
                            }
                        }
                    }
                }
                index = end;
            }
            _ => index += 1,
        }
    }
    false
}

fn fields(value: &Value, required: &[&str], allowed: &[&str]) -> bool {
    value.as_object().is_some_and(|value| {
        required.iter().all(|key| value.contains_key(*key))
            && value.keys().all(|key| allowed.contains(&key.as_str()))
    })
}

fn text(value: &Value, max: usize) -> Option<&str> {
    value.as_str().filter(|value| {
        !value.is_empty()
            && value.len() <= max
            && value.trim() == *value
            && !value.chars().any(char::is_control)
    })
}

fn exact_ms(value: &Value) -> Option<u64> {
    value.as_u64().filter(|value| *value <= MAX_EXACT_MS)
}

fn base64url_digit(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'-' => Some(62),
        b'_' => Some(63),
        _ => None,
    }
}

fn canonical_base64url(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|byte| base64url_digit(byte).is_some())
        && match value.len() % 4 {
            0 => true,
            2 => value
                .bytes()
                .last()
                .and_then(base64url_digit)
                .is_some_and(|value| value % 16 == 0),
            3 => value
                .bytes()
                .last()
                .and_then(base64url_digit)
                .is_some_and(|value| value % 4 == 0),
            _ => false,
        }
}

fn valid_token_format(value: &Value) -> bool {
    let Some(token) = value.as_str().filter(|token| token.len() <= 4089) else {
        return false;
    };
    let Some((payload, signature)) = token.split_once('.') else {
        return false;
    };
    // Shape only, not a client-side signature verification. The signature is a
    // 32-byte HMAC encoded without padding. No bearer enters connection state.
    canonical_base64url(payload) && signature.len() == 43 && canonical_base64url(signature)
}

#[derive(Clone, PartialEq, Eq)]
struct Identity {
    account: String,
    session: String,
    expires: u64,
}

/// Detached non-bearer evidence from this exact observer. Private namespace and
/// revision also reject same-value clear/reissue ABA across identity handshakes.
#[derive(Clone)]
pub(crate) struct MaterialIdentityEvidence {
    realm: String,
    identity: Identity,
    namespace: Arc<()>,
    revision: u64,
}
impl fmt::Debug for MaterialIdentityEvidence {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("MaterialIdentityEvidence(<private>)")
    }
}
impl MaterialIdentityEvidence {
    pub(crate) fn realm_id(&self) -> &str {
        &self.realm
    }
    pub(crate) fn account_id(&self) -> &str {
        &self.identity.account
    }
    pub(crate) fn session_id(&self) -> &str {
        &self.identity.session
    }
    pub(crate) fn expires_at_ms(&self) -> u64 {
        self.identity.expires
    }
    pub(crate) fn is_live(&self, now_ms: u64) -> bool {
        now_ms < self.identity.expires
    }
}

pub(crate) struct HuntingMaterialConnection {
    realm: Option<String>,
    protocol: Option<u16>,
    retired: bool,
    identity: Option<Identity>,
    namespace: Arc<()>,
    revision: u64,
}
impl fmt::Debug for HuntingMaterialConnection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HuntingMaterialConnection")
            .field("protocol_v1", &self.supports_protocol_v1())
            .field("retired", &self.retired)
            .finish_non_exhaustive()
    }
}
impl Default for HuntingMaterialConnection {
    fn default() -> Self {
        Self::new()
    }
}
impl HuntingMaterialConnection {
    pub(crate) fn new() -> Self {
        Self {
            realm: None,
            protocol: None,
            retired: false,
            identity: None,
            namespace: Arc::new(()),
            revision: 0,
        }
    }
    pub(crate) fn is_retired(&self) -> bool {
        self.retired
    }
    pub(crate) fn supports_protocol_v1(&self) -> bool {
        !self.retired && self.realm.is_some() && self.protocol == Some(PROTOCOL_VERSION)
    }
    pub(crate) fn realm_id(&self) -> Option<&str> {
        (!self.retired).then_some(self.realm.as_deref()).flatten()
    }
    /// Metadata accessors do not assert liveness. Use identity()/matches() at
    /// the actual current time before deriving a physical connection context.
    pub(crate) fn account_id(&self) -> Option<&str> {
        (!self.retired)
            .then_some(
                self.identity
                    .as_ref()
                    .map(|identity| identity.account.as_str()),
            )
            .flatten()
    }
    pub(crate) fn session_id(&self) -> Option<&str> {
        (!self.retired)
            .then_some(
                self.identity
                    .as_ref()
                    .map(|identity| identity.session.as_str()),
            )
            .flatten()
    }
    pub(crate) fn expires_at_ms(&self) -> Option<u64> {
        (!self.retired)
            .then_some(self.identity.as_ref().map(|identity| identity.expires))
            .flatten()
    }
    pub(crate) fn clear_identity(&mut self) {
        self.identity = None;
        match self.revision.checked_add(1) {
            Some(revision) => self.revision = revision,
            None => self.retired = true,
        }
    }
    pub(crate) fn identity_matches(&self, account: &str, session: &str, now_ms: u64) -> bool {
        self.supports_protocol_v1()
            && self.identity.as_ref().is_some_and(|identity| {
                identity.account == account
                    && identity.session == session
                    && now_ms < identity.expires
            })
    }
    pub(crate) fn identity(&self, now_ms: u64) -> Option<MaterialIdentityEvidence> {
        if !self.supports_protocol_v1() {
            return None;
        }
        let identity = self
            .identity
            .as_ref()
            .filter(|identity| now_ms < identity.expires)?;
        Some(MaterialIdentityEvidence {
            realm: self.realm.as_ref()?.clone(),
            identity: identity.clone(),
            namespace: Arc::clone(&self.namespace),
            revision: self.revision,
        })
    }
    pub(crate) fn matches(&self, evidence: &MaterialIdentityEvidence, now_ms: u64) -> bool {
        self.supports_protocol_v1()
            && Arc::ptr_eq(&self.namespace, &evidence.namespace)
            && self.revision == evidence.revision
            && self.realm.as_deref() == Some(evidence.realm.as_str())
            && self.identity.as_ref() == Some(&evidence.identity)
            && evidence.is_live(now_ms)
    }
    /// Call only from this actual socket's reader. Ordinary packet envelopes
    /// are ignored; any malformed claimed metadata permanently retires custody.
    pub(crate) fn observe(&mut self, raw: &str, now_ms: u64) -> Result<bool> {
        if !claims_metadata(raw) {
            return Ok(false);
        }
        if self.retired {
            return Err("Native material connection retired");
        }
        match self.observe_inner(raw, now_ms) {
            Ok(()) => Ok(true),
            Err(error) => {
                self.retired = true;
                self.identity = None;
                Err(error)
            }
        }
    }
    fn observe_inner(&mut self, raw: &str, now_ms: u64) -> Result<()> {
        let value = parse(raw)?;
        match value["type"].as_str() {
            Some("realmInfo") => self.observe_realm(&value),
            Some("identitySession") => self.observe_identity(&value, now_ms),
            _ => Err("Invalid native material envelope"),
        }
    }
    fn observe_realm(&mut self, value: &Value) -> Result<()> {
        if !fields(value, &["type", "payload"], &["type", "payload"])
            || !fields(
                &value["payload"],
                &["schema", "realmId"],
                &[
                    "schema",
                    "realmId",
                    "huntingMaterialOwnerProtocolVersion",
                    "npcRepairMode",
                    "profileId",
                    "profileVersion",
                    "acceptanceLevel",
                    "source",
                    "ratePolicy",
                    "bundleHash",
                    "bundleBuiltAt",
                    "sourceData",
                ],
            )
            || value["payload"]["schema"] != "mir2-realm-handshake/1"
        {
            return Err("Invalid native material realm");
        }
        let realm =
            text(&value["payload"]["realmId"], 128).ok_or("Invalid native material realm")?;
        let protocol = match value["payload"].get("huntingMaterialOwnerProtocolVersion") {
            None | Some(Value::Null) => None,
            Some(version) if version.as_u64() == Some(u64::from(PROTOCOL_VERSION)) => {
                Some(PROTOCOL_VERSION)
            }
            _ => return Err("Unsupported native material protocol"),
        };
        if self
            .realm
            .as_ref()
            .is_some_and(|prior| prior != realm || self.protocol != protocol)
        {
            return Err("Native material realm or protocol changed");
        }
        self.realm = Some(realm.into());
        self.protocol = protocol;
        Ok(())
    }
    fn observe_identity(&mut self, value: &Value, now_ms: u64) -> Result<()> {
        if !fields(
            value,
            &["type", "token", "session"],
            &["type", "token", "session"],
        ) || !valid_token_format(&value["token"])
            || !fields(
                &value["session"],
                &["accountId", "sessionId", "expiresAtMs", "revokedAtMs"],
                &[
                    "accountId",
                    "sessionId",
                    "expiresAtMs",
                    "revokedAtMs",
                    "authMethod",
                    "credentialId",
                    "issuedAtMs",
                    "lastSeenAtMs",
                    "revokedReason",
                    "userAgentSummary",
                    "gatewayId",
                    "current",
                ],
            )
        {
            return Err("Invalid native material identity");
        }
        let session = &value["session"];
        let account = text(&session["accountId"], 160).ok_or("Invalid native material identity")?;
        let session_id =
            text(&session["sessionId"], 256).ok_or("Invalid native material identity")?;
        let expires = exact_ms(&session["expiresAtMs"])
            .filter(|expires| *expires > now_ms)
            .ok_or("Expired native material identity")?;
        if !session["revokedAtMs"].is_null()
            || session
                .get("revokedReason")
                .is_some_and(|value| !value.is_null())
            || session
                .get("current")
                .is_some_and(|value| value.as_bool() != Some(true))
            || session.get("issuedAtMs").is_some_and(|value| {
                exact_ms(value).is_none_or(|issued| {
                    issued >= expires || issued > now_ms.saturating_add(MAX_FUTURE_ISSUE_SKEW_MS)
                })
            })
            || session
                .get("lastSeenAtMs")
                .is_some_and(|value| exact_ms(value).is_none())
            || session
                .get("authMethod")
                .is_some_and(|value| text(value, 64).is_none())
            || session
                .get("credentialId")
                .is_some_and(|value| !value.is_null() && text(value, 256).is_none())
            || session
                .get("gatewayId")
                .is_some_and(|value| text(value, 160).is_none())
            || session.get("userAgentSummary").is_some_and(|value| {
                value
                    .as_str()
                    .is_none_or(|value| value.len() > 640 || value.chars().any(char::is_control))
            })
        {
            return Err("Invalid or revoked native material identity");
        }
        let identity = Identity {
            account: account.into(),
            session: session_id.into(),
            expires,
        };
        self.clear_identity();
        if self.retired {
            return Err("Native material identity revision exhausted");
        }
        self.identity = Some(identity);
        Ok(())
    }
}

#[cfg(test)]
#[path = "hunting_material_connection_tests.rs"]
mod tests;
