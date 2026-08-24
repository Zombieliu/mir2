use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrystalMapEventManifest {
    pub schema_version: u32,
    pub source: CrystalMapEventSource,
    pub limits: CrystalMapEventLimits,
    pub map_coordinates: Vec<CrystalMapCoordinateBinding>,
    pub events: Vec<CrystalEventFile>,
    pub references: Vec<CrystalEventReference>,
    pub diagnostics: CrystalMapEventDiagnostics,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrystalMapEventSource {
    pub envir_root: String,
    pub map_coordinates: String,
    pub events: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrystalMapEventLimits {
    pub max_depth: u32,
    pub max_file_bytes: u64,
    pub max_total_bytes: u64,
    pub max_resolved_lines: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrystalMapCoordinateBinding {
    pub map_id: String,
    pub x: i32,
    pub y: i32,
    pub event_id: String,
    pub event_name: String,
    pub binding_source_file: String,
    pub binding_source_line: u32,
    pub include: CrystalEventInclude,
    pub resolved_section: CrystalResolvedSection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrystalEventInclude {
    pub source_file: String,
    pub source_line: u32,
    pub target_file: String,
    pub section: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrystalEventFile {
    pub source_file: String,
    pub bytes: u64,
    pub resolved_lines: Vec<CrystalSourceLine>,
    pub sections: Vec<CrystalResolvedSection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrystalResolvedSection {
    pub name: String,
    pub header: String,
    pub source_file: String,
    pub source_line: u32,
    pub braced: bool,
    pub lines: Vec<CrystalSourceLine>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrystalSourceLine {
    pub text: String,
    pub source_file: String,
    pub source_line: u32,
    pub include_chain: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrystalEventReference {
    pub kind: String,
    pub source_file: String,
    pub source_line: u32,
    pub target_file: String,
    pub section: Option<String>,
    pub resolved: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrystalMapEventDiagnostics {
    pub dangling_paths: Vec<serde_json::Value>,
    pub path_traversal_rejected: Vec<serde_json::Value>,
    pub cycles: Vec<serde_json::Value>,
    pub warnings: Vec<serde_json::Value>,
}

pub fn crystal_map_event_manifest_ref() -> &'static CrystalMapEventManifest {
    static MANIFEST: OnceLock<CrystalMapEventManifest> = OnceLock::new();
    MANIFEST.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../data/generated/crystal_map_event_manifest.json"
        ))
        .expect("crystal map event manifest json should be valid")
    })
}

pub fn crystal_map_event_manifest() -> CrystalMapEventManifest {
    crystal_map_event_manifest_ref().clone()
}

pub fn crystal_map_event_bindings_for_map(map_id: &str) -> Vec<CrystalMapCoordinateBinding> {
    crystal_map_event_manifest_ref()
        .map_coordinates
        .iter()
        .filter(|binding| binding.map_id == map_id)
        .cloned()
        .collect()
}
