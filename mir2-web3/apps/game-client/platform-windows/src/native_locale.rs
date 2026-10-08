//! Per-user language preference; never write into the signed game directory.

use std::{
    fs::{self, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        OnceLock,
    },
};

use bevy::prelude::{App, Last, ResMut, Resource};
use mir2_client_bevy::native_i18n::{self, Locale};
use serde::{Deserialize, Serialize};

const MAX_PREFERENCE_BYTES: u64 = 256;
const PREFERENCE_FILE: &str = "locale.json";
const SEED_FILE: &str = "locale-seed.json";
static DIRECTORY: OnceLock<Option<PathBuf>> = OnceLock::new();
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Preference {
    schema: u8,
    locale: String,
}

fn parse_preference(bytes: &[u8]) -> Option<Locale> {
    if bytes.len() as u64 > MAX_PREFERENCE_BYTES {
        return None;
    }
    let value: Preference = serde_json::from_slice(bytes).ok()?;
    if value.schema != 1
        || !matches!(
            value.locale.as_str(),
            "en" | "zh-TW" | "pt-BR" | "ru" | "hi" | "id" | "vi" | "th" | "ar"
        )
    {
        return None;
    }
    Locale::from_code(&value.locale)
}

fn is_link(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(target_os = "windows"))]
    false
}

fn safe_ancestors(path: &Path) -> io::Result<()> {
    if !path.is_absolute() {
        return Err(io::Error::from(io::ErrorKind::InvalidInput));
    }
    for ancestor in path.ancestors() {
        match fs::symlink_metadata(ancestor) {
            Ok(metadata) if is_link(&metadata) => {
                return Err(io::Error::from(io::ErrorKind::PermissionDenied));
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn read_preference(path: &Path) -> Option<Locale> {
    safe_ancestors(path).ok()?;
    let metadata = fs::symlink_metadata(path).ok()?;
    if !metadata.is_file() || metadata.len() > MAX_PREFERENCE_BYTES {
        return None;
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .ok()?
        .take(MAX_PREFERENCE_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    parse_preference(&bytes)
}

fn supported_os_locale(name: &str) -> Locale {
    let name = name.replace('_', "-").to_ascii_lowercase();
    if name == "pt-br" {
        Locale::BrazilianPortuguese
    } else if matches!(name.as_str(), "zh-tw" | "zh-hk" | "zh-mo" | "zh-hant")
        || name.starts_with("zh-hant-")
    {
        Locale::TraditionalChinese
    } else {
        // Match a complete BCP-47 language subtag, not an arbitrary prefix
        // ("arbitrary" must never select Arabic). Regional variants share the
        // bundled translation; unsupported scripts/locales fall back to English.
        match name.split('-').next().unwrap_or_default() {
            "ru" => Locale::Russian,
            "hi" => Locale::Hindi,
            "id" => Locale::Indonesian,
            "vi" => Locale::Vietnamese,
            "th" => Locale::Thai,
            "ar" => Locale::Arabic,
            _ => Locale::English,
        }
    }
}

fn operating_system_locale() -> Locale {
    #[cfg(target_os = "windows")]
    {
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn GetUserDefaultLocaleName(buffer: *mut u16, count: i32) -> i32;
        }
        let mut name = [0u16; 85];
        // SAFETY: Windows writes at most the supplied UTF-16 buffer length.
        let length = unsafe { GetUserDefaultLocaleName(name.as_mut_ptr(), name.len() as i32) };
        if length > 1 && length as usize <= name.len() {
            return supported_os_locale(&String::from_utf16_lossy(&name[..length as usize - 1]));
        }
    }
    Locale::English
}

fn choose_locale(directory: Option<&Path>, os_locale: Locale) -> Locale {
    directory
        .and_then(|root| read_preference(&root.join(PREFERENCE_FILE)))
        .or_else(|| directory.and_then(|root| read_preference(&root.join(SEED_FILE))))
        .unwrap_or(os_locale)
}

fn replace_file(source: &Path, destination: &Path) -> io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::ffi::OsStrExt;
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn MoveFileExW(source: *const u16, destination: *const u16, flags: u32) -> i32;
        }
        let source: Vec<_> = source.as_os_str().encode_wide().chain(Some(0)).collect();
        let destination: Vec<_> = destination
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect();
        // Same-directory replacement is atomic. Do not use COPY_ALLOWED or a
        // delete-then-rename gap. WRITE_THROUGH completes the small preference.
        let result = unsafe { MoveFileExW(source.as_ptr(), destination.as_ptr(), 0x1 | 0x8) };
        if result == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
    #[cfg(not(target_os = "windows"))]
    fs::rename(source, destination)
}

fn write_preference(directory: &Path, locale: Locale) -> io::Result<()> {
    safe_ancestors(directory)?;
    fs::create_dir_all(directory)?;
    safe_ancestors(directory)?;
    let destination = directory.join(PREFERENCE_FILE);
    safe_ancestors(&destination)?;
    if let Ok(metadata) = fs::symlink_metadata(&destination) {
        if !metadata.is_file() {
            return Err(io::Error::from(io::ErrorKind::PermissionDenied));
        }
    }
    let bytes = serde_json::to_vec(&Preference {
        schema: 1,
        locale: locale.code().to_owned(),
    })?;
    let temporary = directory.join(format!(
        ".locale-{}-{}.tmp",
        std::process::id(),
        TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    let result = (|| {
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        safe_ancestors(&destination)?;
        replace_file(&temporary, &destination)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

/// Call before diagnostics so even early failure dialogs use the chosen language.
pub(crate) fn initialize() {
    let directory = DIRECTORY.get_or_init(|| {
        let path = PathBuf::from(std::env::var_os("APPDATA")?).join("mir2-web3");
        safe_ancestors(&path).ok()?;
        Some(path)
    });
    let selected = choose_locale(directory.as_deref(), operating_system_locale());
    native_i18n::activate(selected);
    if let Some(directory) = directory {
        if read_preference(&directory.join(PREFERENCE_FILE)) != Some(selected)
            && write_preference(directory, selected).is_err()
        {
            eprintln!("[native-locale] preference could not be saved; using in-memory selection");
        }
    }
}

#[derive(Resource)]
struct LocalePersistence {
    directory: Option<PathBuf>,
    observed_revision: u64,
}

fn persist_locale_changes(mut state: ResMut<LocalePersistence>) {
    let revision = native_i18n::revision();
    if revision == state.observed_revision {
        return;
    }
    state.observed_revision = revision;
    if let Some(directory) = &state.directory {
        if write_preference(directory, native_i18n::locale()).is_err() {
            eprintln!("[native-locale] preference could not be saved; using in-memory selection");
        }
    }
}

pub(crate) fn install(app: &mut App) {
    app.insert_resource(LocalePersistence {
        directory: DIRECTORY.get().cloned().flatten(),
        observed_revision: native_i18n::revision(),
    })
    // The selector runs in Update. Persist its final choice in the same frame,
    // including when the user changes language and closes the window together.
    .add_systems(Last, persist_locale_changes);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_directory() -> PathBuf {
        std::env::temp_dir().join(format!(
            "mir2-locale-test-{}-{}",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn locale_preference_rejects_untrusted_schema_codes_and_oversized_content() {
        for bad in [
            r#"{"schema":2,"locale":"en"}"#,
            r#"{"schema":1,"locale":"zh-CN"}"#,
            r#"{"schema":1,"locale":"en","password":"secret"}"#,
            r#"{"schema":1,"locale":"en","locale":"pt-BR"}"#,
            r#"{"schema":1,"locale":"ar-SA"}"#,
            r#"{"schema":1,"locale":"RU"}"#,
        ] {
            assert!(parse_preference(bad.as_bytes()).is_none());
        }
        assert!(parse_preference(&vec![b' '; 257]).is_none());
        assert_eq!(
            parse_preference(br#"{"schema":1,"locale":"zh-TW"}"#),
            Some(Locale::TraditionalChinese)
        );
    }

    #[test]
    fn all_nine_preferences_round_trip_without_replacing_the_install_seed() {
        let directory = temp_directory();
        fs::create_dir_all(&directory).unwrap();
        let seed = br#"{"schema":1,"locale":"ar"}"#;
        fs::write(directory.join(SEED_FILE), seed).unwrap();
        assert_eq!(
            choose_locale(Some(&directory), Locale::English),
            Locale::Arabic
        );
        assert_eq!(Locale::ALL.len(), 9);
        for locale in Locale::ALL {
            write_preference(&directory, locale).unwrap();
            assert_eq!(
                read_preference(&directory.join(PREFERENCE_FILE)),
                Some(locale)
            );
            assert_eq!(choose_locale(Some(&directory), Locale::English), locale);
            assert_eq!(fs::read(directory.join(SEED_FILE)).unwrap(), seed);
            assert_eq!(fs::read_dir(&directory).unwrap().count(), 2);
        }
        fs::remove_file(directory.join(PREFERENCE_FILE)).unwrap();
        fs::remove_file(directory.join(SEED_FILE)).unwrap();
        fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn locale_preference_priority_and_atomic_replacement_leave_no_temporary_files() {
        let directory = temp_directory();
        fs::create_dir_all(&directory).unwrap();
        assert_eq!(
            choose_locale(Some(&directory), Locale::English),
            Locale::English
        );
        fs::write(
            directory.join(SEED_FILE),
            br#"{"schema":1,"locale":"pt-BR"}"#,
        )
        .unwrap();
        assert_eq!(
            choose_locale(Some(&directory), Locale::English),
            Locale::BrazilianPortuguese
        );
        write_preference(&directory, Locale::TraditionalChinese).unwrap();
        assert_eq!(
            choose_locale(Some(&directory), Locale::English),
            Locale::TraditionalChinese
        );
        write_preference(&directory, Locale::English).unwrap();
        assert_eq!(
            choose_locale(Some(&directory), Locale::BrazilianPortuguese),
            Locale::English
        );
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 2);
        fs::remove_file(directory.join(PREFERENCE_FILE)).unwrap();
        fs::remove_file(directory.join(SEED_FILE)).unwrap();
        fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn locale_os_mapping_has_an_explicit_supported_fallback() {
        assert_eq!(
            supported_os_locale("zh-Hant-TW"),
            Locale::TraditionalChinese
        );
        assert_eq!(supported_os_locale("pt_BR"), Locale::BrazilianPortuguese);
        assert_eq!(supported_os_locale("pt-PT"), Locale::English);
        assert_eq!(supported_os_locale("de-DE"), Locale::English);
        assert_eq!(supported_os_locale("en-US"), Locale::English);
        for (name, expected) in [
            ("ru-RU", Locale::Russian),
            ("ru", Locale::Russian),
            ("hi-IN", Locale::Hindi),
            ("id_ID", Locale::Indonesian),
            ("vi-VN", Locale::Vietnamese),
            ("th-TH", Locale::Thai),
            ("ar-SA", Locale::Arabic),
            ("ar-EG", Locale::Arabic),
            ("ar", Locale::Arabic),
            ("arbitrary", Locale::English),
            ("zh-CN", Locale::English),
        ] {
            assert_eq!(supported_os_locale(name), expected, "{name}");
        }
    }
}
