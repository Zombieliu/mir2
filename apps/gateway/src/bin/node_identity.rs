use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use mir2_gateway::NodeSigningIdentity;
use rand::rngs::OsRng;
use rand::RngCore;
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PublicIdentity<'a> {
    node_id: &'a str,
    public_key: &'a str,
    public_key_bytes: Vec<u8>,
    key_file: String,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("node identity failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let command = args.next().ok_or_else(usage)?;
    let path = args.next().ok_or_else(usage)?;
    if args.next().is_some() {
        return Err(usage());
    }
    let path = Path::new(&path);
    let identity = match command.as_str() {
        "generate" => generate(path)?,
        "inspect" => NodeSigningIdentity::from_file(path)?,
        _ => return Err(usage()),
    };
    let output = PublicIdentity {
        node_id: identity.node_id(),
        public_key: identity.public_key(),
        public_key_bytes: URL_SAFE_NO_PAD
            .decode(identity.public_key())
            .map_err(|error| format!("failed to decode derived public key: {error}"))?,
        key_file: path
            .canonicalize()
            .unwrap_or_else(|_| path.to_path_buf())
            .display()
            .to_string(),
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&output)
            .map_err(|error| format!("failed to encode public identity: {error}"))?
    );
    Ok(())
}

fn generate(path: &Path) -> Result<NodeSigningIdentity, String> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    let mut seed = [0_u8; 32];
    OsRng.fill_bytes(&mut seed);
    let identity = NodeSigningIdentity::from_seed(seed);
    let encoded = URL_SAFE_NO_PAD.encode(seed);
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|error| format!("failed to create key file {}: {error}", path.display()))?;
    writeln!(file, "{encoded}")
        .map_err(|error| format!("failed to write key file {}: {error}", path.display()))?;
    file.sync_all()
        .map_err(|error| format!("failed to sync key file {}: {error}", path.display()))?;
    Ok(identity)
}

fn usage() -> String {
    "usage: node_identity generate <private-key-file> | node_identity inspect <private-key-file>"
        .to_string()
}
