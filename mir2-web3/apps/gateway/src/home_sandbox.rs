use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{verify_ed25519_signature, NodeSigningIdentity};

pub const HOME_SANDBOX_SCHEMA: &str = "obelisk.home-zone-sandbox.v1";
const HOME_SANDBOX_SIGNATURE_ALGORITHM: &str = "ed25519-zip215";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeSandboxRuntimeLimits {
    pub memory_bytes: u64,
    pub nano_cpus: u64,
    pub pids_limit: i64,
    pub maximum_open_files: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeSandboxManifestPayload {
    pub schema: String,
    pub workload_id: String,
    pub image_digest: String,
    pub node_id: String,
    pub placement_generation: u64,
    pub issued_at_ms: u64,
    pub expires_at_ms: u64,
    pub run_as_uid: u32,
    pub run_as_gid: u32,
    pub read_only_root_filesystem: bool,
    pub no_new_privileges: bool,
    pub drop_all_capabilities: bool,
    pub seccomp_profile_sha256: String,
    pub allowed_networks: BTreeSet<String>,
    pub writable_paths: BTreeSet<String>,
    pub runtime_limits: HomeSandboxRuntimeLimits,
    pub allowed_environment_names: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeSandboxManifest {
    pub payload: HomeSandboxManifestPayload,
    pub signature_algorithm: String,
    pub signature: String,
}

impl HomeSandboxManifest {
    pub fn sign(
        payload: HomeSandboxManifestPayload,
        issuer: &NodeSigningIdentity,
    ) -> Result<Self, String> {
        validate_payload(&payload)?;
        let bytes = serde_json::to_vec(&payload)
            .map_err(|error| format!("serialize Home Sandbox payload: {error}"))?;
        Ok(Self {
            payload,
            signature_algorithm: HOME_SANDBOX_SIGNATURE_ALGORITHM.to_string(),
            signature: issuer.sign(&bytes),
        })
    }

    pub fn verify(
        &self,
        trusted_issuer: &str,
        expected_node_id: &str,
        minimum_generation: u64,
        now_ms: u64,
    ) -> Result<(), String> {
        validate_payload(&self.payload)?;
        if self.signature_algorithm != HOME_SANDBOX_SIGNATURE_ALGORITHM {
            return Err("unsupported Home Sandbox signature algorithm".to_string());
        }
        let bytes = serde_json::to_vec(&self.payload)
            .map_err(|error| format!("serialize Home Sandbox payload: {error}"))?;
        verify_ed25519_signature(trusted_issuer, &bytes, &self.signature)?;
        if self.payload.node_id != expected_node_id {
            return Err("Home Sandbox manifest targets another node".to_string());
        }
        if self.payload.placement_generation < minimum_generation {
            return Err("Home Sandbox placement generation is stale".to_string());
        }
        if now_ms < self.payload.issued_at_ms || now_ms > self.payload.expires_at_ms {
            return Err("Home Sandbox manifest is not currently valid".to_string());
        }
        Ok(())
    }

    pub fn attest_docker_inspect(
        &self,
        inspect: &Value,
    ) -> Result<HomeSandboxRuntimeAttestation, String> {
        let inspect = inspect
            .as_array()
            .and_then(|items| items.first())
            .unwrap_or(inspect);
        let image = string_at(inspect, &["Image"])?;
        if image != self.payload.image_digest {
            return Err(format!(
                "sandbox image {image} does not match signed digest {}",
                self.payload.image_digest
            ));
        }
        let user = string_at(inspect, &["Config", "User"])?;
        let expected_user = format!("{}:{}", self.payload.run_as_uid, self.payload.run_as_gid);
        if user != expected_user {
            return Err(format!(
                "sandbox user {user} does not match signed {expected_user}"
            ));
        }
        let read_only = bool_at(inspect, &["HostConfig", "ReadonlyRootfs"])?;
        let privileged = bool_at(inspect, &["HostConfig", "Privileged"])?;
        if !read_only || privileged {
            return Err("sandbox root filesystem/privileged mode violates policy".to_string());
        }
        let capabilities = strings_at(inspect, &["HostConfig", "CapDrop"])?;
        if !capabilities
            .iter()
            .any(|capability| capability.eq_ignore_ascii_case("ALL"))
        {
            return Err("sandbox must drop all Linux capabilities".to_string());
        }
        let security_options = strings_at(inspect, &["HostConfig", "SecurityOpt"])?;
        if !security_options
            .iter()
            .any(|option| option.contains("no-new-privileges"))
        {
            return Err("sandbox must enable no-new-privileges".to_string());
        }
        if !security_options
            .iter()
            .any(|option| option.to_ascii_lowercase().contains("seccomp"))
        {
            return Err("sandbox must install an explicit seccomp profile".to_string());
        }
        let seccomp_label = string_at(
            inspect,
            &["Config", "Labels", "com.obelisk.sandbox.seccomp-sha256"],
        )?;
        if seccomp_label != self.payload.seccomp_profile_sha256 {
            return Err("sandbox seccomp profile digest does not match signed policy".to_string());
        }
        let memory_bytes = unsigned_at(inspect, &["HostConfig", "Memory"])?;
        let nano_cpus = unsigned_at(inspect, &["HostConfig", "NanoCpus"])?;
        let pids_limit = signed_at(inspect, &["HostConfig", "PidsLimit"])?;
        if memory_bytes == 0
            || memory_bytes > self.payload.runtime_limits.memory_bytes
            || nano_cpus == 0
            || nano_cpus > self.payload.runtime_limits.nano_cpus
            || pids_limit <= 0
            || pids_limit > self.payload.runtime_limits.pids_limit
        {
            return Err("sandbox runtime resource limit exceeds signed policy".to_string());
        }
        let networks = inspect
            .pointer("/NetworkSettings/Networks")
            .and_then(Value::as_object)
            .ok_or_else(|| "sandbox inspect has no network map".to_string())?
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>();
        if networks.is_empty() || !networks.is_subset(&self.payload.allowed_networks) {
            return Err("sandbox is attached to an unauthorized network".to_string());
        }
        let mounts = inspect
            .get("Mounts")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for mount in &mounts {
            let destination = string_at(mount, &["Destination"])?;
            if destination == "/var/run/docker.sock"
                || destination == "/run/containerd/containerd.sock"
                || !self.payload.writable_paths.contains(destination)
            {
                return Err(format!("sandbox mount {destination} is not authorized"));
            }
        }
        let environment = strings_at(inspect, &["Config", "Env"])?;
        let environment_names = environment
            .iter()
            .filter_map(|entry| entry.split_once('=').map(|(name, _)| name.to_string()))
            .collect::<BTreeSet<_>>();
        for name in &environment_names {
            if forbidden_secret_name(name)
                || (!self.payload.allowed_environment_names.is_empty()
                    && !self.payload.allowed_environment_names.contains(name))
            {
                return Err(format!(
                    "sandbox environment variable {name} is not authorized"
                ));
            }
        }
        Ok(HomeSandboxRuntimeAttestation {
            accepted: true,
            workload_id: self.payload.workload_id.clone(),
            image_digest: image.to_string(),
            run_as_user: user.to_string(),
            read_only_root_filesystem: read_only,
            privileged,
            capabilities_dropped: capabilities,
            security_options,
            networks,
            environment_names,
            memory_bytes,
            nano_cpus,
            pids_limit,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeSandboxRuntimeAttestation {
    pub accepted: bool,
    pub workload_id: String,
    pub image_digest: String,
    pub run_as_user: String,
    pub read_only_root_filesystem: bool,
    pub privileged: bool,
    pub capabilities_dropped: Vec<String>,
    pub security_options: Vec<String>,
    pub networks: BTreeSet<String>,
    pub environment_names: BTreeSet<String>,
    pub memory_bytes: u64,
    pub nano_cpus: u64,
    pub pids_limit: i64,
}

fn validate_payload(payload: &HomeSandboxManifestPayload) -> Result<(), String> {
    if payload.schema != HOME_SANDBOX_SCHEMA
        || payload.workload_id.trim().is_empty()
        || payload.workload_id.len() > 160
        || !valid_sha256_digest(&payload.image_digest)
        || payload.node_id.trim().is_empty()
        || payload.placement_generation == 0
        || payload.expires_at_ms <= payload.issued_at_ms
        || payload.run_as_uid == 0
        || payload.run_as_gid == 0
        || !payload.read_only_root_filesystem
        || !payload.no_new_privileges
        || !payload.drop_all_capabilities
        || !valid_hex_sha256(&payload.seccomp_profile_sha256)
        || payload.allowed_networks.is_empty()
        || payload
            .allowed_networks
            .iter()
            .any(|network| network.trim().is_empty() || network.len() > 160)
        || payload.runtime_limits.memory_bytes < 64 * 1024 * 1024
        || payload.runtime_limits.nano_cpus == 0
        || payload.runtime_limits.pids_limit <= 0
        || payload.runtime_limits.maximum_open_files < 64
        || payload
            .allowed_environment_names
            .iter()
            .any(|name| forbidden_secret_name(name))
    {
        return Err("Home Sandbox manifest contains an unsafe or invalid field".to_string());
    }
    Ok(())
}

fn forbidden_secret_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    [
        "DATABASE_URL",
        "POSTGRES",
        "REDIS_URL",
        "SUI_PRIVATE",
        "SUI_KEYSTORE",
        "REWARD_ISSUER",
        "SETTLEMENT_KEY",
        "ADMIN_TOKEN",
        "DOCKER_HOST",
        "AWS_SECRET",
        "GITHUB_TOKEN",
    ]
    .iter()
    .any(|fragment| upper.contains(fragment))
}

fn valid_sha256_digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(valid_hex_sha256)
}

fn valid_hex_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn string_at<'a>(value: &'a Value, path: &[&str]) -> Result<&'a str, String> {
    value_at(value, path)?
        .as_str()
        .ok_or_else(|| format!("sandbox inspect {} is not a string", path.join(".")))
}

fn bool_at(value: &Value, path: &[&str]) -> Result<bool, String> {
    value_at(value, path)?
        .as_bool()
        .ok_or_else(|| format!("sandbox inspect {} is not a boolean", path.join(".")))
}

fn unsigned_at(value: &Value, path: &[&str]) -> Result<u64, String> {
    value_at(value, path)?
        .as_u64()
        .ok_or_else(|| format!("sandbox inspect {} is not unsigned", path.join(".")))
}

fn signed_at(value: &Value, path: &[&str]) -> Result<i64, String> {
    value_at(value, path)?
        .as_i64()
        .ok_or_else(|| format!("sandbox inspect {} is not signed", path.join(".")))
}

fn strings_at(value: &Value, path: &[&str]) -> Result<Vec<String>, String> {
    value_at(value, path)?
        .as_array()
        .ok_or_else(|| format!("sandbox inspect {} is not an array", path.join(".")))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_string)
                .ok_or_else(|| format!("sandbox inspect {} has a non-string", path.join(".")))
        })
        .collect()
}

fn value_at<'a>(mut value: &'a Value, path: &[&str]) -> Result<&'a Value, String> {
    for component in path {
        value = value
            .get(component)
            .ok_or_else(|| format!("sandbox inspect is missing {}", path.join(".")))?;
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload() -> HomeSandboxManifestPayload {
        HomeSandboxManifestPayload {
            schema: HOME_SANDBOX_SCHEMA.to_string(),
            workload_id: "home-zone-primary".to_string(),
            image_digest: format!("sha256:{}", "11".repeat(32)),
            node_id: "ed25519:test-home-node".to_string(),
            placement_generation: 7,
            issued_at_ms: 1_000,
            expires_at_ms: 3_000,
            run_as_uid: 65_534,
            run_as_gid: 65_534,
            read_only_root_filesystem: true,
            no_new_privileges: true,
            drop_all_capabilities: true,
            seccomp_profile_sha256: "22".repeat(32),
            allowed_networks: BTreeSet::from(["gate24_home-private".to_string()]),
            writable_paths: BTreeSet::new(),
            runtime_limits: HomeSandboxRuntimeLimits {
                memory_bytes: 1024 * 1024 * 1024,
                nano_cpus: 2_000_000_000,
                pids_limit: 128,
                maximum_open_files: 1024,
            },
            allowed_environment_names: BTreeSet::from([
                "MIR2_ZONE_HOST_ADDR".to_string(),
                "PATH".to_string(),
            ]),
        }
    }

    fn inspect() -> Value {
        serde_json::json!([{
            "Image": format!("sha256:{}", "11".repeat(32)),
            "HostConfig": {
                "ReadonlyRootfs": true,
                "Privileged": false,
                "CapDrop": ["ALL"],
                "SecurityOpt": ["no-new-privileges:true", "seccomp=fixture-profile"],
                "Memory": 536870912_u64,
                "NanoCpus": 1_000_000_000_u64,
                "PidsLimit": 64
            },
            "Config": {
                "User": "65534:65534",
                "Env": ["MIR2_ZONE_HOST_ADDR=0.0.0.0:7020", "PATH=/usr/bin"],
                "Labels": {
                    "com.obelisk.sandbox.seccomp-sha256": "2222222222222222222222222222222222222222222222222222222222222222"
                }
            },
            "NetworkSettings": {
                "Networks": {"gate24_home-private": {}}
            },
            "Mounts": []
        }])
    }

    #[test]
    fn signed_sandbox_attests_hardened_runtime() {
        let issuer = NodeSigningIdentity::from_seed([91; 32]);
        let manifest = HomeSandboxManifest::sign(payload(), &issuer).unwrap();
        manifest
            .verify(issuer.public_key(), "ed25519:test-home-node", 7, 2_000)
            .unwrap();
        assert!(manifest.attest_docker_inspect(&inspect()).unwrap().accepted);
    }

    #[test]
    fn sandbox_rejects_root_writable_privileged_secret_and_stale_generation() {
        let issuer = NodeSigningIdentity::from_seed([92; 32]);
        let manifest = HomeSandboxManifest::sign(payload(), &issuer).unwrap();
        let mut root = inspect();
        root[0]["Config"]["User"] = Value::String("0:0".to_string());
        assert!(manifest.attest_docker_inspect(&root).is_err());
        let mut writable = inspect();
        writable[0]["HostConfig"]["ReadonlyRootfs"] = Value::Bool(false);
        assert!(manifest.attest_docker_inspect(&writable).is_err());
        let mut privileged = inspect();
        privileged[0]["HostConfig"]["Privileged"] = Value::Bool(true);
        assert!(manifest.attest_docker_inspect(&privileged).is_err());
        let mut secret = inspect();
        secret[0]["Config"]["Env"] = serde_json::json!(["DATABASE_URL=postgres://forbidden"]);
        assert!(manifest.attest_docker_inspect(&secret).is_err());
        assert!(manifest
            .verify(issuer.public_key(), "ed25519:test-home-node", 8, 2_000,)
            .is_err());
    }
}
