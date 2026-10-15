//! Small, fail-closed Windows primitives used by both updater entry points.
//!
//! CMS trust is an application key pin, not a change to the machine trust store.

use std::{io, path::Path};

#[cfg(windows)]
mod windows {
    use super::*;
    use std::{
        ffi::CStr, fs::OpenOptions, mem::size_of, os::windows::ffi::OsStrExt,
        os::windows::fs::OpenOptionsExt, ptr,
    };
    use windows_sys::Win32::{
        Foundation::{
            CloseHandle, FILETIME, HANDLE, INVALID_HANDLE_VALUE, WAIT_ABANDONED, WAIT_OBJECT_0,
            WAIT_TIMEOUT,
        },
        Globalization::{CompareStringOrdinal, GetUserDefaultLocaleName, CSTR_EQUAL},
        Security::Cryptography::*,
        Storage::FileSystem::{
            GetDiskFreeSpaceExW, MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
        },
        System::{
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
                TH32CS_SNAPPROCESS,
            },
            Threading::{
                CreateMutexW, OpenProcess, QueryFullProcessImageNameW, ReleaseMutex,
                WaitForSingleObject, PROCESS_QUERY_LIMITED_INFORMATION,
            },
        },
    };

    fn wide(path: &Path) -> io::Result<Vec<u16>> {
        let mut value: Vec<_> = path.as_os_str().encode_wide().collect();
        if value.contains(&0) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "path contains a NUL",
            ));
        }
        value.push(0);
        Ok(value)
    }

    struct Handle(HANDLE);
    impl Drop for Handle {
        fn drop(&mut self) {
            if !self.0.is_null() && self.0 != INVALID_HANDLE_VALUE {
                // SAFETY: this wrapper owns a handle returned by a Windows API.
                unsafe {
                    CloseHandle(self.0);
                }
            }
        }
    }

    struct Message(*mut std::ffi::c_void);
    impl Drop for Message {
        fn drop(&mut self) {
            // SAFETY: the handle was returned by CryptMsgOpenToDecode.
            unsafe {
                CryptMsgClose(self.0);
            }
        }
    }

    struct Certificate(*mut CERT_CONTEXT);
    impl Drop for Certificate {
        fn drop(&mut self) {
            // SAFETY: CryptVerifyDetachedMessageSignature transfers this context.
            unsafe {
                CertFreeCertificateContext(self.0);
            }
        }
    }

    fn crypto_error(operation: &str) -> String {
        format!("{operation}: {}", io::Error::last_os_error())
    }

    fn sha256(bytes: &[u8]) -> Result<[u8; 32], String> {
        let length = u32::try_from(bytes.len()).map_err(|_| "hash input is too large")?;
        let mut output = [0_u8; 32];
        let mut output_length = output.len() as u32;
        // SAFETY: all buffers are valid for their stated lengths.
        let result = unsafe {
            CryptHashCertificate2(
                windows_sys::core::w!("SHA256"),
                0,
                ptr::null(),
                bytes.as_ptr(),
                length,
                output.as_mut_ptr(),
                &mut output_length,
            )
        };
        if result == 0 || output_length != 32 {
            return Err(crypto_error("SHA256"));
        }
        Ok(output)
    }

    fn parse_key_pin(pin: &str) -> Result<[u8; 32], String> {
        if pin.len() != 64 || !pin.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("trusted RSA key SHA256 must contain 64 hexadecimal characters".into());
        }
        let mut bytes = [0_u8; 32];
        for (index, item) in bytes.iter_mut().enumerate() {
            *item = u8::from_str_radix(&pin[index * 2..index * 2 + 2], 16)
                .map_err(|_| "invalid trusted RSA key SHA256")?;
        }
        Ok(bytes)
    }

    unsafe fn oid_equals(value: *const u8, expected: &[u8]) -> bool {
        !value.is_null() && unsafe { CStr::from_ptr(value.cast()).to_bytes() == expected }
    }

    fn verify_impl(
        content: &[u8],
        signature: &[u8],
        pinned_rsa_key_sha256: &str,
        authenticated_unix_time: Option<i64>,
    ) -> Result<(), String> {
        let expected_pin = parse_key_pin(pinned_rsa_key_sha256)?;
        if signature.is_empty() || signature.len() > 1024 * 1024 {
            return Err("detached CMS size is invalid".into());
        }
        let signature_length = u32::try_from(signature.len()).map_err(|_| "CMS is too large")?;
        let content_length =
            u32::try_from(content.len()).map_err(|_| "signed content is too large")?;
        // Decode the original signature without a detached-content flag so an
        // embedded-content CMS cannot silently masquerade as detached metadata.
        let handle = unsafe {
            CryptMsgOpenToDecode(
                X509_ASN_ENCODING | PKCS_7_ASN_ENCODING,
                0,
                0,
                0,
                ptr::null(),
                ptr::null(),
            )
        };
        if handle.is_null() {
            return Err(crypto_error("decode CMS"));
        }
        let message = Message(handle);
        if unsafe { CryptMsgUpdate(message.0, signature.as_ptr(), signature_length, 1) } == 0 {
            return Err(crypto_error("parse CMS"));
        }
        let mut message_type = 0_u32;
        let mut message_type_bytes = size_of::<u32>() as u32;
        if unsafe {
            CryptMsgGetParam(
                message.0,
                CMSG_TYPE_PARAM,
                0,
                (&mut message_type as *mut u32).cast(),
                &mut message_type_bytes,
            )
        } == 0
            || message_type_bytes != 4
            || message_type != CMSG_SIGNED
        {
            return Err("detached CMS must contain signed data".into());
        }
        let mut signer_count = 0_u32;
        let mut signer_count_bytes = size_of::<u32>() as u32;
        if unsafe {
            CryptMsgGetParam(
                message.0,
                CMSG_SIGNER_COUNT_PARAM,
                0,
                (&mut signer_count as *mut u32).cast(),
                &mut signer_count_bytes,
            )
        } == 0
            || signer_count_bytes != 4
            || signer_count != 1
        {
            return Err("detached CMS must contain exactly one signer".into());
        }
        let mut attached_content_bytes = 0_u32;
        if unsafe {
            CryptMsgGetParam(
                message.0,
                CMSG_CONTENT_PARAM,
                0,
                ptr::null_mut(),
                &mut attached_content_bytes,
            )
        } == 0
            || attached_content_bytes != 0
        {
            return Err("CMS contains attached content or invalid detached content".into());
        }
        let mut signer_info_bytes = 0_u32;
        if unsafe {
            CryptMsgGetParam(
                message.0,
                CMSG_SIGNER_INFO_PARAM,
                0,
                ptr::null_mut(),
                &mut signer_info_bytes,
            )
        } == 0
            || signer_info_bytes < size_of::<CMSG_SIGNER_INFO>() as u32
            || signer_info_bytes > 1024 * 1024
        {
            return Err(crypto_error("read CMS signer"));
        }
        // usize storage gives the C struct the alignment that Vec<u8> does not promise.
        let mut signer_storage =
            vec![0_usize; (signer_info_bytes as usize).div_ceil(size_of::<usize>())];
        if unsafe {
            CryptMsgGetParam(
                message.0,
                CMSG_SIGNER_INFO_PARAM,
                0,
                signer_storage.as_mut_ptr().cast(),
                &mut signer_info_bytes,
            )
        } == 0
        {
            return Err(crypto_error("decode CMS signer"));
        }
        let signer_info = unsafe { &*signer_storage.as_ptr().cast::<CMSG_SIGNER_INFO>() };
        if !unsafe {
            oid_equals(
                signer_info.HashAlgorithm.pszObjId,
                b"2.16.840.1.101.3.4.2.1",
            )
        } {
            return Err("CMS signer digest must be SHA256".into());
        }

        let parameters = CRYPT_VERIFY_MESSAGE_PARA {
            cbSize: size_of::<CRYPT_VERIFY_MESSAGE_PARA>() as u32,
            dwMsgAndCertEncodingType: X509_ASN_ENCODING | PKCS_7_ASN_ENCODING,
            ..Default::default()
        };
        let content_pointer = content.as_ptr();
        let mut certificate_pointer = ptr::null_mut();
        let valid = unsafe {
            CryptVerifyDetachedMessageSignature(
                &parameters,
                0,
                signature.as_ptr(),
                signature_length,
                1,
                &content_pointer,
                &content_length,
                &mut certificate_pointer,
            )
        };
        if valid == 0 || certificate_pointer.is_null() {
            // Some API failures may nevertheless return a certificate context.
            if !certificate_pointer.is_null() {
                drop(Certificate(certificate_pointer));
            }
            return Err(crypto_error("verify detached CMS signature"));
        }
        let certificate = Certificate(certificate_pointer);
        let context = unsafe { &*certificate.0 };
        if context.pCertInfo.is_null() {
            return Err("CMS signer certificate is missing".into());
        }
        let certificate_info = unsafe { &*context.pCertInfo };
        let public_key = &certificate_info.SubjectPublicKeyInfo;
        if !unsafe { oid_equals(public_key.Algorithm.pszObjId, b"1.2.840.113549.1.1.1") }
            || public_key.PublicKey.cUnusedBits != 0
            || public_key.PublicKey.pbData.is_null()
            || !(64..=65536).contains(&public_key.PublicKey.cbData)
        {
            return Err("CMS signer must use a valid RSA public key".into());
        }
        let public_key_bytes = unsafe {
            std::slice::from_raw_parts(
                public_key.PublicKey.pbData,
                public_key.PublicKey.cbData as usize,
            )
        };
        if sha256(public_key_bytes)? != expected_pin {
            return Err("CMS signer RSA key does not match the pinned publisher key".into());
        }

        let mut usage_bytes = 0_u32;
        if unsafe {
            CertGetEnhancedKeyUsage(
                certificate.0,
                CERT_FIND_EXT_ONLY_ENHKEY_USAGE_FLAG,
                ptr::null_mut(),
                &mut usage_bytes,
            )
        } == 0
            || usage_bytes < size_of::<CTL_USAGE>() as u32
            || usage_bytes > 1024 * 1024
        {
            return Err("CMS signer certificate requires a code-signing EKU extension".into());
        }
        let mut usage_storage = vec![0_usize; (usage_bytes as usize).div_ceil(size_of::<usize>())];
        if unsafe {
            CertGetEnhancedKeyUsage(
                certificate.0,
                CERT_FIND_EXT_ONLY_ENHKEY_USAGE_FLAG,
                usage_storage.as_mut_ptr().cast(),
                &mut usage_bytes,
            )
        } == 0
        {
            return Err(crypto_error("read signer EKU"));
        }
        let usage = unsafe { &*usage_storage.as_ptr().cast::<CTL_USAGE>() };
        if usage.cUsageIdentifier == 0
            || usage.cUsageIdentifier > 4096
            || usage.rgpszUsageIdentifier.is_null()
        {
            return Err("CMS signer certificate requires a code-signing EKU".into());
        }
        let usage_oids = unsafe {
            std::slice::from_raw_parts(usage.rgpszUsageIdentifier, usage.cUsageIdentifier as usize)
        };
        if !usage_oids
            .iter()
            .any(|oid| unsafe { oid_equals(*oid, b"1.3.6.1.5.5.7.3.3") })
        {
            return Err("CMS signer certificate is not a code-signing certificate".into());
        }

        if certificate_info.cExtension > 4096
            || (certificate_info.cExtension > 0 && certificate_info.rgExtension.is_null())
        {
            return Err("invalid CMS certificate extensions".into());
        }
        let extensions = if certificate_info.cExtension == 0 {
            &[][..]
        } else {
            unsafe {
                std::slice::from_raw_parts(
                    certificate_info.rgExtension,
                    certificate_info.cExtension as usize,
                )
            }
        };
        if extensions
            .iter()
            .any(|extension| unsafe { oid_equals(extension.pszObjId, b"2.5.29.15") })
        {
            let mut key_usage = [0_u8; 2];
            if unsafe {
                CertGetIntendedKeyUsage(
                    context.dwCertEncodingType,
                    context.pCertInfo,
                    key_usage.as_mut_ptr(),
                    key_usage.len() as u32,
                )
            } == 0
                || u32::from(key_usage[0]) & CERT_DIGITAL_SIGNATURE_KEY_USAGE == 0
            {
                return Err("CMS signer key usage does not permit digital signatures".into());
            }
        }
        // Historic engine/release signatures remain usable after renewal. When
        // the caller has authenticated the release time, validate at that time.
        if let Some(unix_time) = authenticated_unix_time {
            let ticks = unix_time
                .checked_add(11_644_473_600)
                .and_then(|seconds| u64::try_from(seconds).ok())
                .and_then(|seconds| seconds.checked_mul(10_000_000))
                .ok_or("authenticated signing time is outside the FILETIME range")?;
            let time = FILETIME {
                dwLowDateTime: ticks as u32,
                dwHighDateTime: (ticks >> 32) as u32,
            };
            if unsafe { CertVerifyTimeValidity(&time, context.pCertInfo) } != 0 {
                return Err(
                    "CMS certificate is not valid at the authenticated release time".into(),
                );
            }
        }
        Ok(())
    }

    pub fn verify_cms(
        content: &[u8],
        signature: &[u8],
        pinned_rsa_key_sha256: &str,
    ) -> Result<(), String> {
        verify_impl(content, signature, pinned_rsa_key_sha256, None)
    }

    pub fn verify_cms_at(
        content: &[u8],
        signature: &[u8],
        pinned_rsa_key_sha256: &str,
        authenticated_unix_time: i64,
    ) -> Result<(), String> {
        verify_impl(
            content,
            signature,
            pinned_rsa_key_sha256,
            Some(authenticated_unix_time),
        )
    }

    pub fn replace_file(source: &Path, destination: &Path) -> io::Result<()> {
        let source = wide(source)?;
        let destination = wide(destination)?;
        // No COPY_ALLOWED flag: cross-volume replacement must fail, preserving
        // the caller's same-volume staging/rollback invariant.
        if unsafe {
            MoveFileExW(
                source.as_ptr(),
                destination.as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        } == 0
        {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    pub fn free_bytes(path: &Path) -> io::Result<u64> {
        let path = wide(path)?;
        let mut bytes = 0;
        if unsafe {
            GetDiskFreeSpaceExW(path.as_ptr(), &mut bytes, ptr::null_mut(), ptr::null_mut())
        } == 0
        {
            Err(io::Error::last_os_error())
        } else {
            Ok(bytes)
        }
    }

    fn normalized_path(path: &Path) -> io::Result<Vec<u16>> {
        let canonical = match std::fs::canonicalize(path) {
            Ok(path) => path,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let parent = path.parent().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "missing path parent")
                })?;
                std::fs::canonicalize(parent)?.join(path.file_name().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "missing filename")
                })?)
            }
            Err(error) => return Err(error),
        };
        let rendered = canonical.as_os_str().to_string_lossy();
        let value = if let Some(unc) = rendered.strip_prefix(r"\\?\UNC\") {
            format!(r"\\{unc}")
        } else {
            rendered
                .strip_prefix(r"\\?\")
                .unwrap_or(&rendered)
                .to_string()
        };
        Ok(value.encode_utf16().collect())
    }

    fn equal_path(left: &[u16], right: &[u16]) -> bool {
        if left.len() > i32::MAX as usize || right.len() > i32::MAX as usize {
            return false;
        }
        unsafe {
            CompareStringOrdinal(
                left.as_ptr(),
                left.len() as i32,
                right.as_ptr(),
                right.len() as i32,
                1,
            ) == CSTR_EQUAL
        }
    }

    pub fn game_is_running(game_exe: &Path) -> io::Result<bool> {
        let target = normalized_path(game_exe)?;
        let name = game_exe
            .file_name()
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "game executable has no file name",
                )
            })?
            .encode_wide()
            .collect::<Vec<_>>();
        if let Some(running) = process_probe(&target, &name)? {
            return Ok(running);
        }
        // Unknown matching processes retain the original exact-file fallback.
        match OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(0)
            .open(game_exe)
        {
            Ok(_) => Ok(false),
            Err(error) if error.raw_os_error() == Some(32) => Ok(true),
            Err(error) => Err(error),
        }
    }

    fn process_probe(target: &[u16], name: &[u16]) -> io::Result<Option<bool>> {
        let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
        if snapshot == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        let snapshot = Handle(snapshot);
        let mut entry = PROCESSENTRY32W {
            dwSize: size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        if unsafe { Process32FirstW(snapshot.0, &mut entry) } == 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(18) {
                return Ok(Some(false));
            } // ERROR_NO_MORE_FILES.
            return Err(error);
        }
        let mut inaccessible_match = false;
        loop {
            let name_length = entry
                .szExeFile
                .iter()
                .position(|c| *c == 0)
                .unwrap_or(entry.szExeFile.len());
            if equal_path(&name, &entry.szExeFile[..name_length]) {
                let process = unsafe {
                    OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, entry.th32ProcessID)
                };
                if process.is_null() {
                    inaccessible_match = true;
                } else {
                    let process = Handle(process);
                    let mut buffer = vec![0_u16; 32768];
                    let mut length = buffer.len() as u32;
                    if unsafe {
                        QueryFullProcessImageNameW(process.0, 0, buffer.as_mut_ptr(), &mut length)
                    } == 0
                    {
                        inaccessible_match = true;
                    } else {
                        use std::os::windows::ffi::OsStringExt;
                        let queried = std::path::PathBuf::from(std::ffi::OsString::from_wide(
                            &buffer[..length as usize],
                        ));
                        if let Ok(queried) = normalized_path(&queried) {
                            if equal_path(&target, &queried) {
                                return Ok(Some(true));
                            }
                        } else {
                            inaccessible_match = true;
                        }
                    }
                }
            }
            if unsafe { Process32NextW(snapshot.0, &mut entry) } == 0 {
                let error = io::Error::last_os_error();
                if error.raw_os_error() != Some(18) {
                    return Err(error);
                }
                break;
            }
        }
        Ok(if inaccessible_match {
            None
        } else {
            Some(false)
        })
    }

    /// A live exclusion handle, not a cached process-list/absence result. A
    /// full exact-path process probe after acquiring the handle retains the
    /// original process/path checks, in addition to preventing new image maps.
    pub struct GameGuard {
        exe: std::path::PathBuf,
        target: Vec<u16>,
        name: Vec<u16>,
        held: Option<std::fs::File>,
    }
    impl GameGuard {
        pub fn new(exe: &Path) -> io::Result<Self> {
            Ok(Self {
                exe: exe.to_owned(),
                target: normalized_path(exe)?,
                name: exe
                    .file_name()
                    .ok_or_else(|| {
                        io::Error::new(io::ErrorKind::InvalidInput, "missing executable name")
                    })?
                    .encode_wide()
                    .collect(),
                held: None,
            })
        }
        pub fn release(&mut self) {
            self.held.take();
        }
        pub fn excluded(&self) -> bool {
            self.held.is_some()
        }
        pub fn verify_excluded(&self) -> io::Result<()> {
            use std::os::windows::io::AsRawHandle;
            use windows_sys::Win32::Storage::FileSystem::GetFinalPathNameByHandleW;
            let file = self
                .held
                .as_ref()
                .ok_or_else(|| io::Error::other("game is not excluded"))?;
            file.metadata()?;
            let mut buffer = [0u16; 512];
            let length = unsafe {
                GetFinalPathNameByHandleW(
                    file.as_raw_handle(),
                    buffer.as_mut_ptr(),
                    buffer.len() as u32,
                    0,
                )
            } as usize;
            let prefix = [92, 92, 63, 92];
            if length == 0 {
                return Err(io::Error::last_os_error());
            }
            if length < buffer.len() {
                let actual = buffer[..length]
                    .strip_prefix(&prefix)
                    .unwrap_or(&buffer[..length]);
                if equal_path(actual, &self.target) {
                    return Ok(());
                }
            } else if length < 32768 {
                let mut buffer = vec![0u16; length + 1];
                let length = unsafe {
                    GetFinalPathNameByHandleW(
                        file.as_raw_handle(),
                        buffer.as_mut_ptr(),
                        buffer.len() as u32,
                        0,
                    )
                } as usize;
                if length > 0 && length < buffer.len() {
                    let actual = buffer[..length]
                        .strip_prefix(&prefix)
                        .unwrap_or(&buffer[..length]);
                    if equal_path(actual, &self.target) {
                        return Ok(());
                    }
                }
            }
            Err(io::Error::other("excluded game executable path changed"))
        }
        pub fn is_running(&mut self) -> io::Result<bool> {
            if self.held.is_some() {
                // Validate the live handle on every step. Its share-zero write
                // access prevents read/image mapping, rename and replacement;
                // no new game can start until release. No bytes are written.
                self.verify_excluded()?;
                return Ok(false);
            }
            if let Ok(file) = OpenOptions::new()
                .read(true)
                .write(true)
                .share_mode(0)
                .open(&self.exe)
            {
                let result = process_probe(&self.target, &self.name);
                if matches!(result, Ok(Some(false))) {
                    self.held = Some(file);
                    return Ok(false);
                }
                // Release before the original probe's path/fallback opens so
                // our own handle cannot masquerade as an inaccessible game.
                drop(file);
                result?;
            }
            game_is_running(&self.exe)
        }
    }

    /// Owned by the acquiring thread; Windows mutex ownership is thread-affine.
    pub struct InstallLock {
        handle: HANDLE,
        file: Option<std::fs::File>,
    }
    impl InstallLock {
        pub fn acquire(root: &Path) -> Result<Self, String> {
            let path = normalized_path(root).map_err(|error| error.to_string())?;
            let fold = String::from_utf16_lossy(&path).to_uppercase();
            let hash = sha256(fold.as_bytes())?;
            let hash = hash
                .iter()
                .map(|byte| format!("{byte:02X}"))
                .collect::<String>();
            let name = format!("Local\\Numeron-Mir2-Update-{hash}\0")
                .encode_utf16()
                .collect::<Vec<_>>();
            let handle = unsafe { CreateMutexW(ptr::null(), 0, name.as_ptr()) };
            if handle.is_null() {
                return Err(crypto_error("acquire installation mutex"));
            }
            let wait = unsafe { WaitForSingleObject(handle, 0) };
            match wait {
                WAIT_OBJECT_0 | WAIT_ABANDONED => {
                    let file = (|| -> Result<std::fs::File, String> {
                        let lock = root.join(".update/install.lock");
                        crate::fs_safe::ancestors(&lock).map_err(|e| e.to_string())?;
                        std::fs::create_dir_all(lock.parent().unwrap())
                            .map_err(|e| e.to_string())?;
                        let file = OpenOptions::new()
                            .read(true)
                            .write(true)
                            .create(true)
                            .truncate(false)
                            .share_mode(0)
                            .open(&lock)
                            .map_err(|e| e.to_string())?;
                        use std::os::windows::io::AsRawHandle;
                        use windows_sys::Win32::Storage::FileSystem::{
                            GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
                        };
                        let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
                        if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) }
                            == 0
                            || info.nNumberOfLinks != 1
                            || info.dwFileAttributes & 0x400 != 0
                        {
                            return Err("installation lock must be a regular unlinked file".into());
                        }
                        Ok(file)
                    })();
                    match file {
                        Ok(file) => Ok(Self {
                            handle,
                            file: Some(file),
                        }),
                        Err(error) => {
                            unsafe {
                                ReleaseMutex(handle);
                                CloseHandle(handle);
                            }
                            Err(error)
                        }
                    }
                }
                WAIT_TIMEOUT => {
                    unsafe {
                        CloseHandle(handle);
                    }
                    Err("another updater is using this installation".into())
                }
                _ => {
                    let error = crypto_error("wait for installation mutex");
                    unsafe {
                        CloseHandle(handle);
                    }
                    Err(error)
                }
            }
        }
    }
    impl Drop for InstallLock {
        fn drop(&mut self) {
            drop(self.file.take());
            // SAFETY: the calling thread acquired and still owns this mutex.
            unsafe {
                ReleaseMutex(self.handle);
                CloseHandle(self.handle);
            }
        }
    }

    pub fn os_locale() -> String {
        let mut locale = [0_u16; 85]; // LOCALE_NAME_MAX_LENGTH includes the terminator.
        let length = unsafe { GetUserDefaultLocaleName(locale.as_mut_ptr(), locale.len() as i32) };
        if length > 1 {
            String::from_utf16_lossy(&locale[..length as usize - 1])
        } else {
            "en".into()
        }
    }
}

#[cfg(windows)]
pub use windows::*;

#[cfg(not(windows))]
pub fn verify_cms(_: &[u8], _: &[u8], _: &str) -> Result<(), String> {
    Err("Windows CMS verification is unavailable on this platform".into())
}
#[cfg(not(windows))]
pub fn verify_cms_at(content: &[u8], signature: &[u8], pin: &str, _: i64) -> Result<(), String> {
    verify_cms(content, signature, pin)
}
#[cfg(not(windows))]
pub fn replace_file(source: &Path, destination: &Path) -> io::Result<()> {
    std::fs::rename(source, destination)
}
#[cfg(not(windows))]
pub fn free_bytes(_: &Path) -> io::Result<u64> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Windows free-space probe is unavailable",
    ))
}
#[cfg(not(windows))]
pub fn game_is_running(_: &Path) -> io::Result<bool> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Windows process probe is unavailable",
    ))
}
#[cfg(not(windows))]
pub struct InstallLock;
#[cfg(not(windows))]
pub struct GameGuard;
#[cfg(not(windows))]
impl GameGuard {
    pub fn new(_: &Path) -> io::Result<Self> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Windows game exclusion is unavailable",
        ))
    }
    pub fn release(&mut self) {}
    pub fn excluded(&self) -> bool {
        false
    }
    pub fn verify_excluded(&self) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Windows game exclusion is unavailable",
        ))
    }
    pub fn is_running(&mut self) -> io::Result<bool> {
        game_is_running(Path::new(""))
    }
}
#[cfg(not(windows))]
impl InstallLock {
    pub fn acquire(_: &Path) -> Result<Self, String> {
        Err("Windows install mutex is unavailable".into())
    }
}
#[cfg(not(windows))]
pub fn os_locale() -> String {
    "en".into()
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };
    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct FixtureRoot(PathBuf);
    impl FixtureRoot {
        fn new() -> Self {
            let clock = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "mir2-updater-platform-{}-{clock}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&root).unwrap();
            Self(root)
        }
    }
    impl Drop for FixtureRoot {
        fn drop(&mut self) {
            // Only remove the exact newly-created test root; no paths are taken
            // from signatures, metadata or user preferences here.
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn atomic_replacement_and_free_space_probe_use_the_exact_target() {
        let fixture = FixtureRoot::new();
        let source = fixture.0.join("new.bin");
        let destination = fixture.0.join("active.bin");
        fs::write(&source, b"new").unwrap();
        fs::write(&destination, b"old").unwrap();
        replace_file(&source, &destination).unwrap();
        assert_eq!(fs::read(&destination).unwrap(), b"new");
        assert!(!source.exists());
        assert!(free_bytes(&fixture.0).unwrap() > 0);
    }

    #[test]
    fn mutex_blocks_another_thread_and_releases_on_drop() {
        let fixture = FixtureRoot::new();
        let first = InstallLock::acquire(&fixture.0).unwrap();
        let root = fs::canonicalize(&fixture.0).unwrap();
        assert!(
            std::thread::spawn(move || InstallLock::acquire(&root).is_err())
                .join()
                .unwrap()
        );
        drop(first);
        let root = fixture.0.clone();
        assert!(
            std::thread::spawn(move || InstallLock::acquire(&root).is_ok())
                .join()
                .unwrap()
        );
    }

    #[test]
    fn process_probe_does_not_confuse_another_install_with_this_executable() {
        let running = std::env::current_exe().unwrap();
        assert!(game_is_running(&running).unwrap());
        assert!(game_is_running(&fs::canonicalize(&running).unwrap()).unwrap());
        let fixture = FixtureRoot::new();
        let other_install = fixture.0.join(running.file_name().unwrap());
        fs::copy(&running, &other_install).unwrap();
        assert!(!game_is_running(&other_install).unwrap());
    }

    #[test]
    fn fresh_probe_detects_a_later_image_mapping_and_keeps_locked_missing_readonly_fallbacks() {
        use std::os::windows::process::CommandExt;
        use std::time::{Duration, Instant};
        const CHILD: &str = "MIR2_UPDATER_OWNED_PROCESS_PROBE";
        if let Some(root) = std::env::var_os(CHILD) {
            let root = PathBuf::from(root);
            fs::write(root.join("ready"), b"owned test process ready").unwrap();
            let deadline = Instant::now() + Duration::from_secs(10);
            while !root.join("stop").exists() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(10));
            }
            return;
        }
        let fixture = FixtureRoot::new();
        let exe = fixture.0.join("mir2-platform-windows.exe");
        assert!(!game_is_running(&exe).unwrap()); // Missing target retains snapshot.
        fs::copy(std::env::current_exe().unwrap(), &exe).unwrap();
        assert!(!game_is_running(&exe).unwrap());
        let mut permissions = fs::metadata(&exe).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&exe, permissions).unwrap();
        assert!(!game_is_running(&exe).unwrap()); // Access denied is not "running".
        let mut permissions = fs::metadata(&exe).unwrap().permissions();
        permissions.set_readonly(false);
        fs::set_permissions(&exe, permissions).unwrap();
        let locked = std::fs::OpenOptions::new().read(true).open(&exe).unwrap();
        assert!(!game_is_running(&exe).unwrap()); // A data handle is not an image.
        drop(locked);
        let mut child = std::process::Command::new(&exe)
            .args(["--exact", "platform::tests::fresh_probe_detects_a_later_image_mapping_and_keeps_locked_missing_readonly_fallbacks", "--test-threads=1"])
            .env(CHILD, &fixture.0)
            .creation_flags(0x08000000) // CREATE_NO_WINDOW, only an owned test child.
            .stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null())
            .spawn().unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !fixture.0.join("ready").exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        let ready = fixture.0.join("ready").exists();
        let running = game_is_running(&exe);
        let guarded_running = GameGuard::new(&exe).unwrap().is_running();
        fs::write(fixture.0.join("stop"), b"normal owned child exit").unwrap();
        assert!(child.wait().unwrap().success());
        assert!(ready && running.unwrap());
        assert!(guarded_running.unwrap());
        assert!(!game_is_running(&exe).unwrap()); // No cached running result either.
    }

    #[test]
    fn malformed_signature_and_unconfigured_key_fail_closed() {
        assert!(verify_cms(b"payload", b"not CMS", &"0".repeat(64)).is_err());
        assert!(verify_cms(b"payload", b"not CMS", "").is_err());
        assert!(verify_cms(b"payload", b"", &"0".repeat(64)).is_err());
    }

    /// Public-only CMS fixtures are generated outside the repository with an
    /// ephemeral key. Explicitly ignored without that integration fixture.
    #[test]
    #[ignore = "requires MIR2_UPDATER_CRYPTO_FIXTURES generated by the Windows CMS integration probe"]
    fn cms_math_key_pin_usage_detached_and_renewal_contract() {
        let root = PathBuf::from(
            std::env::var_os("MIR2_UPDATER_CRYPTO_FIXTURES").expect("fixture path required"),
        );
        let content = fs::read(root.join("content.bin")).unwrap();
        let pin = fs::read_to_string(root.join("key-pin.txt")).unwrap();
        let pin = pin.trim();
        let signature = |name: &str| fs::read(root.join(format!("{name}.p7s"))).unwrap();
        verify_cms(&content, &signature("valid"), pin).unwrap();
        verify_cms(&content, &signature("renewed"), pin).unwrap();
        verify_cms(&content, &signature("expired"), pin).unwrap();
        assert!(verify_cms(b"tampered", &signature("valid"), pin).is_err());
        assert!(verify_cms(&content, &signature("valid"), &"0".repeat(64)).is_err());
        for invalid in [
            "multiple",
            "attached",
            "sha1",
            "no-eku",
            "wrong-usage",
            "wrong-key",
        ] {
            assert!(
                verify_cms(&content, &signature(invalid), pin).is_err(),
                "accepted {invalid}"
            );
        }
        let authenticated_time = fs::read_to_string(root.join("valid-time.txt"))
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        verify_cms_at(&content, &signature("valid"), pin, authenticated_time).unwrap();
        assert!(verify_cms_at(&content, &signature("valid"), pin, 0).is_err());
    }
}
