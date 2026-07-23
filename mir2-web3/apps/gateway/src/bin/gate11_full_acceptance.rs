use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use mir2_gateway::run_gate11_full_acceptance;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = parse_output_path()?;
    let evidence = run_gate11_full_acceptance()?;
    let json = serde_json::to_string_pretty(&evidence)?;
    if let Some(path) = output_path {
        write_atomic(&path, json.as_bytes())?;
        eprintln!(
            "gate11: wrote accepted evidence manifest to {}",
            path.display()
        );
    }
    println!("{json}");
    Ok(())
}

fn parse_output_path() -> Result<Option<PathBuf>, String> {
    let mut args = env::args().skip(1);
    match args.next() {
        None => Ok(None),
        Some(flag) if flag == "--output" => {
            let path = args
                .next()
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| "--output requires a path".to_string())?;
            if args.next().is_some() {
                return Err("unexpected extra Gate 11 acceptance argument".to_string());
            }
            Ok(Some(PathBuf::from(path)))
        }
        Some(flag) => Err(format!("unsupported argument {flag}; use --output PATH")),
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or("Gate 11 evidence path must have a UTF-8 file name")?;
    let temporary = parent.join(format!(".{file_name}.tmp-{}", std::process::id()));
    fs::write(&temporary, bytes)?;
    fs::rename(temporary, path)?;
    Ok(())
}
