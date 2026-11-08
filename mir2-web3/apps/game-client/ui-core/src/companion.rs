//! Private owner controls, never part of the public world replica or persisted UI settings.
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};

use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const CHUNK_BYTES: usize = 16 * 1024;
const MAX_EXPORT_BYTES: usize = 2 * 1024 * 1024;
static INSTANCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CompanionOperation {
    List,
    View,
    Teach,
    RevokeFeedback,
    DeleteFeedback,
    DeleteEpisode,
    ClearGrowth,
    Recall,
    Resume,
    ExportBegin,
    ExportChunk,
    TrialEvent,
}
impl CompanionOperation {
    pub fn mutation(self) -> bool {
        !matches!(
            self,
            Self::List | Self::View | Self::ExportBegin | Self::ExportChunk
        )
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompanionRequest {
    pub request_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resident_id: Option<String>,
    pub operation: CompanionOperation,
    pub payload: Value,
}
impl std::fmt::Debug for CompanionRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompanionRequest")
            .field("request_id", &self.request_id)
            .field("operation", &self.operation)
            .field("payload", &"<private>")
            .finish()
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CompanionStatus {
    Ok,
    Denied,
    Offline,
    Unknown,
    Invalid,
    Conflict,
    Error,
}
#[derive(Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanionResult {
    pub request_id: String,
    #[serde(default)]
    pub resident_id: Option<String>,
    pub operation: CompanionOperation,
    pub status: CompanionStatus,
    #[serde(default)]
    pub result: Value,
}
impl std::fmt::Debug for CompanionResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompanionResult")
            .field("request_id", &self.request_id)
            .field("operation", &self.operation)
            .field("status", &self.status)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanionEnrollment {
    pub resident_id: String,
    pub archive_id: String,
    pub display_name: String,
    pub online: bool,
}
#[derive(Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanionArchive {
    pub archive_id: String,
    pub resident_id: String,
    pub display_name: String,
    pub character_name: String,
    pub realm_id: String,
    pub world_epoch: String,
    #[serde(default)]
    pub base_setting: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanionHabits {
    pub assist_before_loot: bool,
    pub loot_only_after_combat: bool,
    pub ask_before_leaving: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanionProfile {
    pub policy_version: u64,
    pub style: String,
    pub party: CompanionHabits,
}
#[derive(Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanionFeedback {
    pub feedback_id: String,
    pub status: String,
    #[serde(default)]
    pub recorded_at: Option<u64>,
    #[serde(default)]
    pub summary: String,
}
#[derive(Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanionEpisode {
    pub episode_id: String,
    #[serde(default)]
    pub recorded_at: Option<u64>,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub action: String,
    #[serde(default)]
    pub outcome: String,
    #[serde(default)]
    pub policy_version: Option<u64>,
}
#[derive(Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanionOwnerView {
    pub archive: CompanionArchive,
    pub profile: CompanionProfile,
    pub growth_revision: u64,
    #[serde(default)]
    pub feedback: Vec<CompanionFeedback>,
    #[serde(default)]
    pub episodes: Vec<CompanionEpisode>,
    #[serde(default)]
    pub next_cursor: Option<String>,
    #[serde(default)]
    pub runtime: Value,
    #[serde(default)]
    pub trial: Value,
}

#[derive(Debug, Clone)]
struct Pending {
    request: CompanionRequest,
    deadline_ms: u64,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanionExportManifest {
    pub export_id: String,
    pub total_bytes: usize,
    pub sha256: String,
    pub chunk_count: usize,
}
#[derive(Clone)]
pub struct CompletedCompanionExport {
    pub manifest: CompanionExportManifest,
    pub bytes: Vec<u8>,
}
impl Drop for CompletedCompanionExport {
    fn drop(&mut self) {
        self.bytes.fill(0);
    }
}
struct ExportAssembly {
    manifest: CompanionExportManifest,
    bytes: Vec<u8>,
    next_index: usize,
}
impl Drop for ExportAssembly {
    fn drop(&mut self) {
        self.bytes.fill(0);
    }
}

#[derive(Resource)]
pub struct CompanionControls {
    pub enrollments: Vec<CompanionEnrollment>,
    pub selected: Option<String>,
    pub view: Option<CompanionOwnerView>,
    pub notice: String,
    pub uncertain: bool,
    pub revision: u64,
    pub completed_export: Option<CompletedCompanionExport>,
    pub export_path: Option<String>,
    pending: Option<Pending>,
    outbound: VecDeque<CompanionRequest>,
    export: Option<ExportAssembly>,
    nonce: String,
    sequence: u64,
}
impl std::fmt::Debug for CompanionControls {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompanionControls")
            .field("enrollment_count", &self.enrollments.len())
            .field("pending", &self.pending)
            .field("revision", &self.revision)
            .finish()
    }
}
impl Default for CompanionControls {
    fn default() -> Self {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        Self {
            enrollments: vec![],
            selected: None,
            view: None,
            notice: String::new(),
            uncertain: false,
            revision: 0,
            completed_export: None,
            export_path: None,
            pending: None,
            outbound: VecDeque::new(),
            export: None,
            nonce: format!("{stamp:x}-{:x}", INSTANCE.fetch_add(1, Ordering::Relaxed)),
            sequence: 0,
        }
    }
}
pub fn valid_companion_ref(value: &str) -> bool {
    !value.is_empty() && value.len() <= 100 && value.bytes().all(|b| b.is_ascii_graphic())
}
impl CompanionControls {
    pub fn next_visit_id(&self) -> String {
        format!("visit-{}-{}", self.nonce, self.sequence.saturating_add(1))
    }
    pub fn busy(&self) -> bool {
        self.pending.is_some() || self.export.is_some()
    }
    pub fn selected_enrollment(&self) -> Option<&CompanionEnrollment> {
        self.enrollments
            .iter()
            .find(|entry| Some(&entry.resident_id) == self.selected.as_ref())
    }
    pub fn reset(&mut self) {
        self.enrollments.clear();
        self.selected = None;
        self.view = None;
        self.pending = None;
        self.outbound.clear();
        self.export = None;
        self.completed_export = None;
        self.export_path = None;
        self.notice.clear();
        self.uncertain = false;
        self.revision += 1;
    }
    pub fn begin(&mut self, operation: CompanionOperation, payload: Value, now_ms: u64) -> bool {
        if self.pending.is_some() || (self.uncertain && operation.mutation()) {
            return false;
        }
        let resident_id = if operation == CompanionOperation::List {
            None
        } else {
            let Some(entry) = self.selected_enrollment() else {
                return false;
            };
            Some(entry.resident_id.clone())
        };
        let Some(sequence) = self.sequence.checked_add(1) else {
            self.notice = "本次连接的请求序列已耗尽，请重新启动客户端。".into();
            self.revision += 1;
            return false;
        };
        self.sequence = sequence;
        let request = CompanionRequest {
            request_id: format!("ncp-{}-{}", self.nonce, self.sequence),
            resident_id,
            operation,
            payload,
        };
        self.pending = Some(Pending {
            request: request.clone(),
            deadline_ms: now_ms.saturating_add(18_000),
        });
        self.outbound.push_back(request);
        self.notice = "等待服务器确认…".into();
        self.revision += 1;
        true
    }
    pub fn choose(&mut self, resident_id: &str, now_ms: u64) -> bool {
        if self.busy()
            || !self
                .enrollments
                .iter()
                .any(|e| e.resident_id == resident_id)
        {
            return false;
        }
        self.selected = Some(resident_id.into());
        self.view = None;
        self.uncertain = false;
        self.export = None;
        self.completed_export = None;
        self.export_path = None;
        self.begin(CompanionOperation::View, json!({"limit": 20}), now_ms)
    }
    pub fn version_payload(&self) -> Option<Value> {
        self.view
            .as_ref()
            .map(|view| json!({"expectedPolicyVersion": view.profile.policy_version}))
    }
    pub fn drain_requests(&mut self) -> Vec<CompanionRequest> {
        self.outbound.drain(..).collect()
    }
    pub fn tick(&mut self, now_ms: u64) {
        if self
            .pending
            .as_ref()
            .is_some_and(|pending| now_ms >= pending.deadline_ms)
        {
            let mutation = self.pending.take().unwrap().request.operation.mutation();
            self.outbound.clear();
            self.export = None;
            self.completed_export = None;
            self.uncertain |= mutation;
            self.notice = "结果未确认。请刷新查看，操作不会自动重发。".into();
            self.revision += 1;
        }
    }
    pub fn send_failed(&mut self, request_id: &str) {
        if self
            .pending
            .as_ref()
            .is_some_and(|pending| pending.request.request_id == request_id)
        {
            self.pending = None;
            self.export = None;
            self.notice = "连接不可用，请重新连接后查看。".into();
            self.revision += 1;
        }
    }
    fn accept_view(&mut self, value: Value) -> bool {
        let Ok(view) = serde_json::from_value::<CompanionOwnerView>(value) else {
            return false;
        };
        let Some(enrollment) = self.selected_enrollment() else {
            return false;
        };
        if view.archive.resident_id != enrollment.resident_id
            || view.archive.archive_id != enrollment.archive_id
            || view.profile.policy_version == 0
            || !valid_companion_ref(&view.archive.archive_id)
        {
            return false;
        }
        self.view = Some(view);
        true
    }
    pub fn receive(&mut self, reply: CompanionResult, now_ms: u64) -> bool {
        let Some(pending) = &self.pending else {
            return false;
        };
        if reply.request_id != pending.request.request_id
            || reply.operation != pending.request.operation
            || reply.resident_id != pending.request.resident_id
        {
            return false;
        }
        let pending = self.pending.take().unwrap();
        self.revision += 1;
        if reply.status != CompanionStatus::Ok {
            self.export = None;
            self.completed_export = None;
            self.notice = match reply.status {
                CompanionStatus::Denied => "本角色没有此伙伴的操作权限。",
                CompanionStatus::Offline => "伙伴当前离线，请稍后刷新。",
                CompanionStatus::Unknown => "结果未确认。请刷新查看，操作不会自动重发。",
                CompanionStatus::Conflict => "培养版本已变化，已请求显示当前版本；请检查后再操作。",
                CompanionStatus::Invalid => "请求无效，请刷新伙伴资料。",
                _ => "伙伴服务暂不可用，请稍后刷新。",
            }
            .into();
            if reply.status == CompanionStatus::Unknown && pending.request.operation.mutation() {
                self.uncertain = true;
            }
            if reply.status == CompanionStatus::Conflict {
                self.accept_view(reply.result);
            }
            if reply.status == CompanionStatus::Denied {
                self.reset();
                self.notice = "本角色没有此伙伴的操作权限。".into();
            }
            return true;
        }
        match reply.operation {
            CompanionOperation::List => {
                let rows = reply
                    .result
                    .get("companions")
                    .cloned()
                    .unwrap_or(Value::Null);
                let Ok(enrollments) = serde_json::from_value::<Vec<CompanionEnrollment>>(rows)
                else {
                    return self.invalid_reply();
                };
                if enrollments.iter().any(|e| {
                    !valid_companion_ref(&e.resident_id) || !valid_companion_ref(&e.archive_id)
                }) {
                    return self.invalid_reply();
                }
                let mut seen = std::collections::HashSet::new();
                if enrollments
                    .iter()
                    .any(|e| !seen.insert(e.resident_id.clone()))
                {
                    return self.invalid_reply();
                }
                self.enrollments = enrollments;
                if !self
                    .enrollments
                    .iter()
                    .any(|e| Some(&e.resident_id) == self.selected.as_ref())
                {
                    self.selected = self.enrollments.first().map(|e| e.resident_id.clone());
                    self.view = None;
                }
                if self.selected.is_some() {
                    self.begin(CompanionOperation::View, json!({"limit":20}), now_ms);
                } else {
                    self.notice = "本角色没有已授权的伙伴。".into();
                }
            }
            CompanionOperation::ExportBegin => {
                let Ok(manifest) = serde_json::from_value::<CompanionExportManifest>(reply.result)
                else {
                    return self.invalid_reply();
                };
                if !valid_companion_ref(&manifest.export_id)
                    || manifest.total_bytes == 0
                    || manifest.total_bytes > MAX_EXPORT_BYTES
                    || manifest.chunk_count != manifest.total_bytes.div_ceil(CHUNK_BYTES)
                    || manifest.sha256.len() != 64
                    || !manifest.sha256.bytes().all(|b| b.is_ascii_hexdigit())
                {
                    return self.invalid_reply();
                }
                self.export = Some(ExportAssembly {
                    bytes: Vec::with_capacity(manifest.total_bytes),
                    manifest,
                    next_index: 0,
                });
                self.queue_export_chunk(now_ms);
            }
            CompanionOperation::ExportChunk => {
                if !self.accept_chunk(reply.result) {
                    return self.invalid_reply();
                }
                if self
                    .export
                    .as_ref()
                    .is_some_and(|export| export.next_index == export.manifest.chunk_count)
                {
                    let mut export = self.export.take().unwrap();
                    self.completed_export = Some(CompletedCompanionExport {
                        manifest: export.manifest.clone(),
                        bytes: std::mem::take(&mut export.bytes),
                    });
                    self.notice = "正在校验完整导出…".into();
                } else {
                    self.queue_export_chunk(now_ms);
                }
            }
            operation => {
                if !self.accept_view(reply.result) {
                    self.uncertain |= operation.mutation();
                    return self.invalid_reply();
                }
                self.uncertain = false;
                let version = self.view.as_ref().unwrap().profile.policy_version;
                self.notice = match operation {
                    CompanionOperation::Teach => format!("教学已保存，培养版本 v{version}。"),
                    CompanionOperation::View => format!("已读取服务器记录，培养版本 v{version}。"),
                    CompanionOperation::Recall => "已确认召回请求；请观察伙伴实际行动。".into(),
                    CompanionOperation::Resume => "已确认恢复行动。".into(),
                    CompanionOperation::TrialEvent => {
                        "体验自述已记录；不代表付款或服务完成。".into()
                    }
                    _ => format!("操作已保存，培养版本 v{version}。"),
                };
            }
        }
        true
    }
    fn invalid_reply(&mut self) -> bool {
        self.export = None;
        self.completed_export = None;
        self.notice = "服务器资料不匹配，未显示成功；请刷新。".into();
        true
    }
    fn queue_export_chunk(&mut self, now_ms: u64) {
        let Some(export) = &self.export else {
            return;
        };
        let payload = json!({"exportId":export.manifest.export_id,"index":export.next_index});
        self.begin(CompanionOperation::ExportChunk, payload, now_ms);
        if let Some(export) = &self.export {
            self.notice = format!(
                "下载完整成长 {}/{}…",
                export.next_index, export.manifest.chunk_count
            );
        }
    }
    fn accept_chunk(&mut self, result: Value) -> bool {
        let Some(export) = self.export.as_mut() else {
            return false;
        };
        if result.get("exportId").and_then(Value::as_str)
            != Some(export.manifest.export_id.as_str())
            || result.get("index").and_then(Value::as_u64) != Some(export.next_index as u64)
            || result.get("chunkCount").and_then(Value::as_u64)
                != Some(export.manifest.chunk_count as u64)
        {
            return false;
        }
        let Some(data) = result.get("dataBase64").and_then(Value::as_str) else {
            return false;
        };
        let Some(mut bytes) = decode_export_base64(data) else {
            return false;
        };
        let expected = (export.manifest.total_bytes - export.bytes.len()).min(CHUNK_BYTES);
        if bytes.len() != expected {
            bytes.fill(0);
            return false;
        }
        export.bytes.append(&mut bytes);
        export.next_index += 1;
        true
    }
}

fn decode_export_base64(text: &str) -> Option<Vec<u8>> {
    if text.is_empty() || text.len() > 21848 || text.len() % 4 != 0 {
        return None;
    }
    let alphabet = |byte| match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    };
    let mut output = Vec::with_capacity(text.len() / 4 * 3);
    for (index, chunk) in text.as_bytes().chunks_exact(4).enumerate() {
        let a = alphabet(chunk[0])?;
        let b = alphabet(chunk[1])?;
        let last = index + 1 == text.len() / 4;
        output.push((a << 2) | (b >> 4));
        if chunk[2] == b'=' {
            if !last || chunk[3] != b'=' || b & 15 != 0 {
                return None;
            }
            continue;
        }
        let c = alphabet(chunk[2])?;
        output.push((b << 4) | (c >> 2));
        if chunk[3] == b'=' {
            if !last || c & 3 != 0 {
                return None;
            }
            continue;
        }
        let d = alphabet(chunk[3])?;
        output.push((c << 6) | d);
    }
    Some(output)
}

pub fn format_companion_micros(amount: Option<&str>, currency: Option<&str>) -> String {
    let (Some(amount), Some(currency)) = (amount, currency) else {
        return "未计量".into();
    };
    if currency.len() != 3 || !currency.bytes().all(|b| b.is_ascii_uppercase()) {
        return "未计量".into();
    }
    let negative = amount.starts_with('-');
    let digits = amount.strip_prefix('-').unwrap_or(amount);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) || digits.len() > 80 {
        return "未计量".into();
    }
    let padded = format!("{:0>7}", digits);
    let (whole, fraction) = padded.split_at(padded.len() - 6);
    let whole = whole.trim_start_matches('0');
    let whole = if whole.is_empty() { "0" } else { whole };
    let fraction = fraction.trim_end_matches('0');
    format!(
        "{}{whole}{} {currency}",
        if negative { "−" } else { "" },
        if fraction.is_empty() {
            String::new()
        } else {
            format!(".{fraction}")
        }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn owner_view(version: u64) -> Value {
        json!({"archive":{"archiveId":"archive-1","residentId":"alo-1","displayName":"阿洛","characterName":"AloAI","realmId":"trial","worldEpoch":"world-1"},
            "profile":{"policyVersion":version,"style":"collection","party":{"assistBeforeLoot":version>1,"lootOnlyAfterCombat":version>1,"askBeforeLeaving":version>1}},
            "growthRevision":version,"feedback":[{"feedbackId":"feedback-1","status":"active","summary":"private teaching"}],
            "episodes":[{"episodeId":"episode-1","summary":"private episode"}],"trial":{},"runtime":{"modelEnabled":false}})
    }
    fn reply(
        request: &CompanionRequest,
        status: CompanionStatus,
        result: Value,
    ) -> CompanionResult {
        CompanionResult {
            request_id: request.request_id.clone(),
            resident_id: request.resident_id.clone(),
            operation: request.operation,
            status,
            result,
        }
    }
    fn loaded() -> CompanionControls {
        let mut controls = CompanionControls::default();
        controls.begin(CompanionOperation::List, json!({}), 0);
        let request = controls.drain_requests().pop().unwrap();
        assert!(request.resident_id.is_none());
        controls.receive(reply(&request,CompanionStatus::Ok,json!({"companions":[{"residentId":"alo-1","archiveId":"archive-1","displayName":"阿洛","online":true}]})),1);
        let request = controls.drain_requests().pop().unwrap();
        assert_eq!(request.operation, CompanionOperation::View);
        controls.receive(reply(&request, CompanionStatus::Ok, owner_view(1)), 2);
        controls
    }
    #[test]
    fn teach_waits_for_exact_ack_and_replaces_committed_version() {
        let mut controls = loaded();
        let payload = controls.version_payload().unwrap();
        assert_eq!(payload["expectedPolicyVersion"], 1);
        assert!(controls.begin(CompanionOperation::Teach, payload, 10));
        assert_eq!(controls.view.as_ref().unwrap().profile.policy_version, 1);
        let request = controls.drain_requests().pop().unwrap();
        let mut wrong = reply(&request, CompanionStatus::Ok, owner_view(2));
        wrong.resident_id = Some("stranger".into());
        assert!(!controls.receive(wrong, 11));
        assert!(controls.busy());
        controls.receive(reply(&request, CompanionStatus::Ok, owner_view(2)), 12);
        assert_eq!(controls.view.as_ref().unwrap().profile.policy_version, 2);
        assert!(
            controls
                .view
                .as_ref()
                .unwrap()
                .profile
                .party
                .assist_before_loot
        );
        assert!(controls.notice.contains("已保存"));
        assert!(!controls.busy());
    }
    #[test]
    fn timeout_never_replays_and_read_refresh_releases_uncertain_guard() {
        let mut controls = loaded();
        controls.begin(
            CompanionOperation::Teach,
            controls.version_payload().unwrap(),
            10,
        );
        let old = controls.drain_requests().pop().unwrap();
        controls.tick(18_010);
        assert!(controls.uncertain);
        assert!(controls.drain_requests().is_empty());
        assert!(!controls.begin(
            CompanionOperation::Teach,
            json!({"expectedPolicyVersion":1}),
            18_011
        ));
        assert!(!controls.receive(reply(&old, CompanionStatus::Ok, owner_view(2)), 18_012));
        assert_eq!(controls.view.as_ref().unwrap().profile.policy_version, 1);
        assert!(controls.begin(CompanionOperation::View, json!({}), 18_013));
        let fresh = controls.drain_requests().pop().unwrap();
        assert_ne!(fresh.request_id, old.request_id);
        controls.receive(reply(&fresh, CompanionStatus::Ok, owner_view(2)), 18_014);
        assert!(!controls.uncertain);
    }
    #[test]
    fn conflict_displays_current_version_but_does_not_repeat() {
        let mut controls = loaded();
        controls.begin(
            CompanionOperation::Teach,
            controls.version_payload().unwrap(),
            10,
        );
        let request = controls.drain_requests().pop().unwrap();
        controls.receive(
            reply(&request, CompanionStatus::Conflict, owner_view(3)),
            11,
        );
        assert_eq!(controls.view.as_ref().unwrap().profile.policy_version, 3);
        assert!(controls.drain_requests().is_empty());
        assert!(!controls.notice.contains("已保存"));
    }
    #[test]
    fn binding_mismatch_and_malformed_success_do_not_commit_or_claim_success() {
        let mut controls = loaded();
        controls.begin(
            CompanionOperation::Teach,
            controls.version_payload().unwrap(),
            10,
        );
        let request = controls.drain_requests().pop().unwrap();
        let mut foreign = owner_view(2);
        foreign["archive"]["archiveId"] = json!("foreign");
        controls.receive(reply(&request, CompanionStatus::Ok, foreign), 11);
        assert_eq!(controls.view.as_ref().unwrap().profile.policy_version, 1);
        assert!(controls.uncertain);
    }
    #[test]
    fn session_reset_drops_all_private_views_downloads_and_old_correlations() {
        let mut controls = loaded();
        controls.begin(
            CompanionOperation::Teach,
            controls.version_payload().unwrap(),
            10,
        );
        let request = controls.drain_requests().pop().unwrap();
        controls.reset();
        assert!(controls.view.is_none());
        assert!(controls.selected.is_none());
        assert!(controls.enrollments.is_empty());
        assert!(!controls.busy());
        assert!(!controls.receive(reply(&request, CompanionStatus::Ok, owner_view(2)), 11));
        controls.begin(CompanionOperation::List, json!({}), 12);
        assert_ne!(controls.drain_requests()[0].request_id, request.request_id);
    }
    #[test]
    fn private_request_and_reply_debug_never_include_memory_or_trial_text() {
        let request = CompanionRequest {
            request_id: "id-1".into(),
            resident_id: Some("alo-1".into()),
            operation: CompanionOperation::TrialEvent,
            payload: json!({"reasonText":"private sentinel"}),
        };
        assert!(!format!("{request:?}").contains("private sentinel"));
        let response = reply(&request, CompanionStatus::Ok, owner_view(2));
        let diagnostic = format!("{response:?}");
        assert!(!diagnostic.contains("private teaching"));
        assert!(!diagnostic.contains("private episode"));
        assert_eq!(
            serde_json::to_value(&request).unwrap()["payload"]["reasonText"],
            "private sentinel"
        );
    }
    #[test]
    fn export_manifest_and_chunks_must_match_and_complete_without_truncation() {
        let mut controls = loaded();
        controls.begin(CompanionOperation::ExportBegin, json!({}), 10);
        let request = controls.drain_requests().pop().unwrap();
        controls.receive(reply(&request,CompanionStatus::Ok,json!({"exportId":"export-1","totalBytes":3,"sha256":"a".repeat(64),"chunkCount":1})),11);
        let chunk = controls.drain_requests().pop().unwrap();
        assert_eq!(chunk.payload["index"], 0);
        controls.receive(
            reply(
                &chunk,
                CompanionStatus::Ok,
                json!({"exportId":"export-1","index":0,"chunkCount":1,"dataBase64":"YWJj"}),
            ),
            12,
        );
        assert_eq!(controls.completed_export.as_ref().unwrap().bytes, b"abc");
        assert!(controls.drain_requests().is_empty());
        assert!(!controls.busy());
    }
    #[test]
    fn oversized_export_or_wrong_chunk_is_discarded() {
        let mut controls = loaded();
        controls.begin(CompanionOperation::ExportBegin, json!({}), 10);
        let request = controls.drain_requests().pop().unwrap();
        controls.receive(reply(&request,CompanionStatus::Ok,json!({"exportId":"export-1","totalBytes":2097153,"sha256":"a".repeat(64),"chunkCount":129})),11);
        assert!(!controls.busy());
        assert!(controls.completed_export.is_none());
        assert!(controls.drain_requests().is_empty());
        controls.begin(CompanionOperation::ExportBegin, json!({}), 12);
        let request = controls.drain_requests().pop().unwrap();
        controls.receive(reply(&request,CompanionStatus::Ok,json!({"exportId":"export-1","totalBytes":3,"sha256":"a".repeat(64),"chunkCount":1})),13);
        let chunk = controls.drain_requests().pop().unwrap();
        controls.receive(
            reply(
                &chunk,
                CompanionStatus::Ok,
                json!({"exportId":"another-export","index":0,"chunkCount":1,"dataBase64":"YWJj"}),
            ),
            14,
        );
        assert!(!controls.busy());
        assert!(controls.completed_export.is_none());
    }
    #[test]
    fn base64_padding_and_exact_decimal_costs_are_checked() {
        assert_eq!(decode_export_base64("YQ=="), Some(b"a".to_vec()));
        assert!(decode_export_base64("YR==").is_none());
        assert!(decode_export_base64("YQ==YWJj").is_none());
        assert!(decode_export_base64("$$$=").is_none());
        assert_eq!(
            format_companion_micros(Some("9007199254740993123456"), Some("CNY")),
            "9007199254740993.123456 CNY"
        );
        assert_eq!(
            format_companion_micros(Some("-1"), Some("CNY")),
            "−0.000001 CNY"
        );
        assert_eq!(format_companion_micros(None, Some("CNY")), "未计量");
    }
    #[test]
    fn companion_panel_is_in_game_only_and_blocks_gameplay_input() {
        let login = crate::UiState::default();
        let refused = crate::reduce(&login, crate::UiAction::OpenCompanion);
        assert_eq!(refused.state.panel, crate::UiPanel::None);
        let state = crate::UiState {
            screen: crate::UiScreen::InGame,
            ..Default::default()
        };
        let opened = crate::reduce(&state, crate::UiAction::OpenCompanion).state;
        assert_eq!(opened.panel, crate::UiPanel::Companion);
        assert!(opened.blocks_gameplay_keys());
        assert!(opened.blocks_world_click());
        assert!(!crate::reduce(&opened, crate::UiAction::ClosePanel)
            .state
            .blocks_gameplay_keys());
    }
}
