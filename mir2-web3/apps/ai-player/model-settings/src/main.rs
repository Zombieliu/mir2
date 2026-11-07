#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#[cfg(windows)]
mod windows;

fn main() {
    #[cfg(windows)]
    let result = windows::run();
    #[cfg(not(windows))]
    let result: std::result::Result<(), &'static str> =
        Err(numeron_alo_model_settings::FIXED_ERROR);
    if result.is_err() {
        // Fixed text only. In --read-key mode stderr is captured by Node.
        eprintln!("{}", numeron_alo_model_settings::FIXED_ERROR);
        std::process::exit(1);
    }
}
