use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use mir2_gateway::NodeSigningIdentity;
use rcgen::{
    BasicConstraints, Certificate, CertificateParams, ExtendedKeyUsagePurpose, IsCa, KeyPair,
    KeyUsagePurpose,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("HOME_LOCAL_STACK_FIXTURE_FATAL {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let output_dir = env::args()
        .nth(1)
        .map(PathBuf::from)
        .ok_or_else(|| "usage: home_local_stack_fixture <output-directory>".to_string())?;
    fs::create_dir_all(&output_dir).map_err(|error| {
        format!(
            "create Home local stack fixture directory {}: {error}",
            output_dir.display()
        )
    })?;
    let output_dir = output_dir.canonicalize().map_err(|error| {
        format!(
            "canonicalize Home local stack fixture directory {}: {error}",
            output_dir.display()
        )
    })?;

    let enrollment_seed = [71_u8; 32];
    let control_seed = [72_u8; 32];
    let relay_seed = [73_u8; 32];
    let enrollment = NodeSigningIdentity::from_seed(enrollment_seed);
    let control = NodeSigningIdentity::from_seed(control_seed);
    let relay = NodeSigningIdentity::from_seed(relay_seed);

    let enrollment_key = output_dir.join("enrollment-signing.key");
    let control_key = output_dir.join("control-signing.key");
    let relay_key = output_dir.join("relay-signing.key");
    write_secret(
        &enrollment_key,
        format!("{}\n", URL_SAFE_NO_PAD.encode(enrollment_seed)).as_bytes(),
    )?;
    write_secret(
        &control_key,
        format!("{}\n", URL_SAFE_NO_PAD.encode(control_seed)).as_bytes(),
    )?;
    write_secret(
        &relay_key,
        format!("{}\n", URL_SAFE_NO_PAD.encode(relay_seed)).as_bytes(),
    )?;

    let (ca, ca_key, server, server_key) = relay_tls_material()?;
    let ca_certificate = output_dir.join("relay-ca.der");
    let ca_private_key = output_dir.join("relay-ca-key.der");
    let server_certificate = output_dir.join("relay-server.der");
    let server_private_key = output_dir.join("relay-server-key.der");
    fs::write(&ca_certificate, ca.der()).map_err(|error| {
        format!(
            "write Home local Relay CA {}: {error}",
            ca_certificate.display()
        )
    })?;
    write_secret(&ca_private_key, &ca_key.serialize_der())?;
    fs::write(&server_certificate, server.der()).map_err(|error| {
        format!(
            "write Home local Relay server certificate {}: {error}",
            server_certificate.display()
        )
    })?;
    write_secret(&server_private_key, &server_key.serialize_der())?;

    let placements = output_dir.join("placements.json");
    let admissions = output_dir.join("admissions.json");
    fs::write(&placements, b"[]\n")
        .map_err(|error| format!("write {}: {error}", placements.display()))?;
    fs::write(&admissions, b"[]\n")
        .map_err(|error| format!("write {}: {error}", admissions.display()))?;
    let operator_token = output_dir.join("telemetry-operator.token");
    write_secret(
        &operator_token,
        b"local-home-telemetry-operator-token-000000000001\n",
    )?;

    let environment = format!(
        "MIR2_HOME_ENROLLMENT_SIGNING_KEY_FILE={}\n\
         MIR2_HOME_ENROLLMENT_CONTROL_SIGNING_KEY_FILE={}\n\
         MIR2_HOME_ENROLLMENT_RELAY_PUBLIC_KEY={}\n\
         MIR2_HOME_ENROLLMENT_CONTROL_ISSUER_PUBLIC_KEY={}\n\
         MIR2_HOME_ENROLLMENT_TLS_CA_CERTIFICATE_DER={}\n\
         MIR2_HOME_ENROLLMENT_TLS_CA_KEY_DER={}\n\
         MIR2_HOME_ENROLLMENT_PLACEMENTS_FILE={}\n\
         MIR2_HOME_ENROLLMENT_ADMISSIONS_FILE={}\n\
         MIR2_HOME_RELAY_SIGNING_KEY_FILE={}\n\
         MIR2_HOME_RELAY_TLS_CA_DER={}\n\
         MIR2_HOME_RELAY_TLS_CERT_CHAIN_DER={}\n\
         MIR2_HOME_RELAY_TLS_KEY_DER={}\n\
         MIR2_HOME_RELAY_PUBLIC_KEY={}\n\
         MIR2_HOME_CONTROL_ISSUER_PUBLIC_KEY={}\n\
         MIR2_HOME_CAPACITY_ISSUER_PUBLIC_KEY={}\n\
         MIR2_HOME_PLACEMENTS_FILE={}\n\
         MIR2_HOME_TELEMETRY_ADMISSIONS_FILE={}\n\
         MIR2_HOME_TELEMETRY_ENROLLMENT_ISSUER_PUBLIC_KEY={}\n\
         MIR2_HOME_TELEMETRY_OPERATOR_TOKEN_FILE={}\n",
        enrollment_key.display(),
        control_key.display(),
        relay.public_key(),
        control.public_key(),
        ca_certificate.display(),
        ca_private_key.display(),
        placements.display(),
        admissions.display(),
        relay_key.display(),
        ca_certificate.display(),
        server_certificate.display(),
        server_private_key.display(),
        relay.public_key(),
        control.public_key(),
        enrollment.public_key(),
        placements.display(),
        admissions.display(),
        enrollment.public_key(),
        operator_token.display(),
    );
    let env_file = output_dir.join("fixture.env");
    fs::write(&env_file, environment)
        .map_err(|error| format!("write {}: {error}", env_file.display()))?;

    println!(
        "HOME_LOCAL_STACK_FIXTURE_READY output={} enrollment_issuer={} relay_issuer={} control_issuer={}",
        output_dir.display(),
        enrollment.public_key(),
        relay.public_key(),
        control.public_key(),
    );
    Ok(())
}

fn relay_tls_material() -> Result<(Certificate, KeyPair, Certificate, KeyPair), String> {
    let mut ca_params = CertificateParams::new(Vec::<String>::new())
        .map_err(|error| format!("create Home local Relay CA parameters: {error}"))?;
    ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    ca_params.key_usages = vec![
        KeyUsagePurpose::DigitalSignature,
        KeyUsagePurpose::KeyCertSign,
        KeyUsagePurpose::CrlSign,
    ];
    let ca_key = KeyPair::generate()
        .map_err(|error| format!("generate Home local Relay CA key: {error}"))?;
    let ca = ca_params
        .self_signed(&ca_key)
        .map_err(|error| format!("sign Home local Relay CA: {error}"))?;

    let mut server_params = CertificateParams::new(vec!["localhost".to_string()])
        .map_err(|error| format!("create Home local Relay server parameters: {error}"))?;
    server_params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
    server_params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    let server_key = KeyPair::generate()
        .map_err(|error| format!("generate Home local Relay server key: {error}"))?;
    let server = server_params
        .signed_by(&server_key, &ca, &ca_key)
        .map_err(|error| format!("sign Home local Relay server certificate: {error}"))?;
    Ok((ca, ca_key, server, server_key))
}

fn write_secret(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|error| format!("open secret fixture {}: {error}", path.display()))?;
    file.write_all(bytes)
        .map_err(|error| format!("write secret fixture {}: {error}", path.display()))
}
