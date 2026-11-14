//! Windows generation lifetime exclusion and authenticated local readiness.
//! The installation lock remains the launcher's responsibility. This module
//! deliberately does not acquire a legacy EXE guard: the game's own lease must
//! never exclude its own process. A readiness message is not health acceptance.
use anyhow::Result;
use std::path::Path;

#[cfg(windows)]
pub struct GameLease {
    _file: std::fs::File,
}
#[cfg(not(windows))]
pub struct GameLease {
    _unsupported: (),
}

impl GameLease {
    pub fn ensure_idle(root: &Path) -> Result<()> {
        ensure_idle(root)
    }
    pub fn acquire(root: &Path) -> Result<Self> {
        #[cfg(windows)]
        {
            let path = crate::fs_safe::target(root, ".update/pack-game.lock")?;
            // open_download checks the actual no-reparse, single-link handle
            // and uses share_mode(0). The handle, not a marker, is the lease.
            Ok(Self {
                _file: crate::fs_safe::open_download(&path)?,
            })
        }
        #[cfg(not(windows))]
        {
            let _ = root;
            anyhow::bail!("Windows generation game lease is unsupported on this platform")
        }
    }
}

pub fn ensure_idle(root: &Path) -> Result<()> {
    drop(GameLease::acquire(root)?);
    Ok(())
}

#[cfg(windows)]
mod windows {
    use anyhow::{bail, ensure, Context, Result};
    use serde::{Deserialize, Serialize};
    use std::{
        ffi::c_void,
        io,
        os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
        ptr,
        time::{Duration, Instant},
    };
    use windows_sys::Win32::{
        Foundation::{
            GetLastError, LocalFree, ERROR_FILE_NOT_FOUND, ERROR_MORE_DATA, ERROR_NO_DATA,
            ERROR_PIPE_BUSY, ERROR_PIPE_CONNECTED, ERROR_PIPE_LISTENING, GENERIC_READ,
            GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE,
        },
        Security::{
            Authorization::{
                ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
                SDDL_REVISION_1,
            },
            GetTokenInformation, TokenUser, PSID, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER,
        },
        Storage::FileSystem::{
            CreateFileW, ReadFile, WriteFile, FILE_FLAG_FIRST_PIPE_INSTANCE, OPEN_EXISTING,
            PIPE_ACCESS_DUPLEX,
        },
        System::{
            Pipes::{
                ConnectNamedPipe, CreateNamedPipeW, GetNamedPipeClientProcessId,
                GetNamedPipeServerProcessId, PeekNamedPipe, SetNamedPipeHandleState, PIPE_NOWAIT,
                PIPE_READMODE_MESSAGE, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_MESSAGE,
            },
            Threading::{GetCurrentProcess, OpenProcessToken},
        },
    };

    const SCHEMA: &str = "mir2.generation.ready.v1";
    const PREFIX: &str = r"\\.\pipe\mir2-generation-ready-";
    const MAX_MESSAGE: usize = 1024;
    const ACK: &[u8] = b"MIR2READY1";
    const CLIENT_TIMEOUT: Duration = Duration::from_secs(3);

    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields, rename_all = "camelCase")]
    struct Message {
        schema: String,
        attempt: String,
        generation_sha256: String,
        pid: u32,
    }

    fn identities(attempt: &str, generation_sha: &str) -> Result<()> {
        ensure!(
            crate::model::valid_hash(attempt),
            "invalid ready attempt nonce"
        );
        ensure!(
            crate::model::valid_hash(generation_sha),
            "invalid ready generation SHA256"
        );
        Ok(())
    }
    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(Some(0)).collect()
    }
    fn raw(handle: &OwnedHandle) -> HANDLE {
        handle.as_raw_handle().cast()
    }
    fn owned(handle: HANDLE, context: &str) -> Result<OwnedHandle> {
        ensure!(
            !handle.is_null() && handle != INVALID_HANDLE_VALUE,
            "{context}: {}",
            io::Error::last_os_error()
        );
        Ok(unsafe { OwnedHandle::from_raw_handle(handle.cast()) })
    }
    struct LocalMemory(*mut c_void);
    impl Drop for LocalMemory {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    LocalFree(self.0);
                }
            }
        }
    }
    fn sid_string(sid: PSID) -> Result<String> {
        let mut value = ptr::null_mut();
        ensure!(
            unsafe { ConvertSidToStringSidW(sid, &mut value) } != 0,
            "convert current token SID: {}",
            io::Error::last_os_error()
        );
        let allocation = LocalMemory(value.cast());
        let mut length = 0;
        while length < 256 && unsafe { *value.add(length) } != 0 {
            length += 1;
        }
        ensure!(length > 0 && length < 256, "current SID text bound");
        let result = String::from_utf16(unsafe { std::slice::from_raw_parts(value, length) })?;
        drop(allocation);
        Ok(result)
    }
    fn current_sid() -> Result<String> {
        let mut token = ptr::null_mut();
        ensure!(
            unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } != 0,
            "open current process token: {}",
            io::Error::last_os_error()
        );
        let token = owned(token, "current token")?;
        let mut required = 0;
        unsafe {
            GetTokenInformation(raw(&token), TokenUser, ptr::null_mut(), 0, &mut required);
        }
        ensure!(
            (std::mem::size_of::<TOKEN_USER>() as u32..=65536).contains(&required),
            "token user byte bound"
        );
        // Use an aligned allocation; TOKEN_USER contains a pointer.
        let mut buffer = vec![0usize; (required as usize).div_ceil(std::mem::size_of::<usize>())];
        ensure!(
            unsafe {
                GetTokenInformation(
                    raw(&token),
                    TokenUser,
                    buffer.as_mut_ptr().cast(),
                    required,
                    &mut required,
                )
            } != 0,
            "read current token SID: {}",
            io::Error::last_os_error()
        );
        let user = unsafe { &*buffer.as_ptr().cast::<TOKEN_USER>() };
        sid_string(user.User.Sid)
    }
    fn pipe_security() -> Result<LocalMemory> {
        let sid = current_sid()?;
        // Protected DACL, explicit current-token owner. No Everyone, network,
        // Administrators or inherited ACEs. SYSTEM remains available for OS use.
        let sddl = wide(&format!("O:{sid}D:P(A;;GA;;;SY)(A;;GA;;;{sid})"));
        let mut descriptor = ptr::null_mut();
        ensure!(
            unsafe {
                ConvertStringSecurityDescriptorToSecurityDescriptorW(
                    sddl.as_ptr(),
                    SDDL_REVISION_1,
                    &mut descriptor,
                    ptr::null_mut(),
                )
            } != 0,
            "create local readiness security descriptor: {}",
            io::Error::last_os_error()
        );
        Ok(LocalMemory(descriptor))
    }

    pub struct ReadyServer {
        handle: OwnedHandle,
        name: String,
        attempt: String,
        generation_sha: String,
        expected_pid: Option<u32>,
        connected: bool,
        accepted: bool,
        failed: bool,
    }
    impl ReadyServer {
        /// Caller supplies a fresh unpredictable 256-bit attempt nonce.
        pub fn new(attempt: &str, generation_sha: &str) -> Result<Self> {
            identities(attempt, generation_sha)?;
            let name = format!("{PREFIX}{}-{attempt}", std::process::id());
            let security = pipe_security()?;
            let attributes = SECURITY_ATTRIBUTES {
                nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: security.0,
                bInheritHandle: 0,
            };
            let pipe = unsafe {
                CreateNamedPipeW(
                    wide(&name).as_ptr(),
                    PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE,
                    PIPE_TYPE_MESSAGE
                        | PIPE_READMODE_MESSAGE
                        | PIPE_NOWAIT
                        | PIPE_REJECT_REMOTE_CLIENTS,
                    1,
                    1024,
                    4096,
                    0,
                    &attributes,
                )
            };
            let handle = owned(pipe, "create readiness pipe")?;
            Ok(Self {
                handle,
                name,
                attempt: attempt.into(),
                generation_sha: generation_sha.into(),
                expected_pid: None,
                connected: false,
                accepted: false,
                failed: false,
            })
        }
        pub fn pipe_name(&self) -> &str {
            &self.name
        }
        /// Never waits for connection or data. A malformed or foreign peer
        /// permanently poisons this attempt; callers must not retry around it.
        pub fn poll(&mut self, expected_child_pid: u32) -> Result<bool> {
            ensure!(!self.failed, "readiness attempt previously rejected");
            let result = self.poll_inner(expected_child_pid);
            if result.is_err() {
                self.failed = true;
            }
            result
        }
        fn poll_inner(&mut self, expected_child_pid: u32) -> Result<bool> {
            ensure!(expected_child_pid != 0, "missing expected child PID");
            if let Some(expected) = self.expected_pid {
                ensure!(
                    expected == expected_child_pid,
                    "expected child identity changed"
                );
            } else {
                self.expected_pid = Some(expected_child_pid);
            }
            if self.accepted {
                return Ok(true);
            }
            if !self.connected {
                if unsafe { ConnectNamedPipe(raw(&self.handle), ptr::null_mut()) } == 0 {
                    match unsafe { GetLastError() } {
                        ERROR_PIPE_LISTENING => return Ok(false),
                        ERROR_PIPE_CONNECTED => {}
                        error => bail!(
                            "connect readiness peer: {}",
                            io::Error::from_raw_os_error(error as i32)
                        ),
                    }
                }
                self.connected = true;
            }
            let mut actual_pid = 0;
            ensure!(
                unsafe { GetNamedPipeClientProcessId(raw(&self.handle), &mut actual_pid) } != 0,
                "read readiness client identity: {}",
                io::Error::last_os_error()
            );
            ensure!(
                actual_pid == expected_child_pid,
                "readiness kernel client PID mismatch"
            );
            let mut available = 0;
            let mut message_left = 0;
            ensure!(
                unsafe {
                    PeekNamedPipe(
                        raw(&self.handle),
                        ptr::null_mut(),
                        0,
                        ptr::null_mut(),
                        &mut available,
                        &mut message_left,
                    )
                } != 0,
                "peek readiness message: {}",
                io::Error::last_os_error()
            );
            ensure!(
                available <= MAX_MESSAGE as u32 && message_left <= MAX_MESSAGE as u32,
                "oversized/multiple readiness message"
            );
            if available == 0 {
                return Ok(false);
            }
            let mut bytes = [0u8; MAX_MESSAGE + 1];
            let mut received = 0;
            if unsafe {
                ReadFile(
                    raw(&self.handle),
                    bytes.as_mut_ptr(),
                    bytes.len() as u32,
                    &mut received,
                    ptr::null_mut(),
                )
            } == 0
            {
                let error = unsafe { GetLastError() };
                if error == ERROR_NO_DATA {
                    return Ok(false);
                }
                ensure!(error != ERROR_MORE_DATA, "oversized readiness JSON");
                bail!(
                    "read readiness JSON: {}",
                    io::Error::from_raw_os_error(error as i32)
                );
            }
            ensure!(
                received > 0 && received as usize <= MAX_MESSAGE,
                "readiness message byte bound"
            );
            let message: Message = serde_json::from_slice(&bytes[..received as usize])
                .context("readiness JSON schema")?;
            ensure!(
                message.schema == SCHEMA
                    && message.attempt == self.attempt
                    && message.generation_sha256 == self.generation_sha
                    && message.pid == actual_pid,
                "readiness attempt/generation/message PID mismatch"
            );
            write_message(&self.handle, ACK)?;
            self.accepted = true;
            Ok(true)
        }
    }

    fn parse_pipe_identity(name: &str, attempt: &str) -> Result<u32> {
        let tail = name
            .strip_prefix(PREFIX)
            .context("not a local generation readiness pipe")?;
        let (pid, nonce) = tail.split_once('-').context("readiness pipe identity")?;
        let pid: u32 = pid.parse().context("readiness server PID")?;
        ensure!(
            pid != 0 && nonce == attempt && name == format!("{PREFIX}{pid}-{attempt}"),
            "readiness pipe nonce/identity mismatch"
        );
        Ok(pid)
    }
    fn verify_server(handle: &OwnedHandle, expected: u32) -> Result<()> {
        let mut actual = 0;
        ensure!(
            unsafe { GetNamedPipeServerProcessId(raw(handle), &mut actual) } != 0,
            "read readiness server PID: {}",
            io::Error::last_os_error()
        );
        ensure!(actual == expected, "readiness kernel server PID mismatch");
        Ok(())
    }
    fn connect_client(name: &str, attempt: &str, deadline: Instant) -> Result<OwnedHandle> {
        let expected_server = parse_pipe_identity(name, attempt)?;
        let name = wide(name);
        loop {
            let handle = unsafe {
                CreateFileW(
                    name.as_ptr(),
                    GENERIC_READ | GENERIC_WRITE,
                    0,
                    ptr::null(),
                    OPEN_EXISTING,
                    0,
                    ptr::null_mut(),
                )
            };
            if handle != INVALID_HANDLE_VALUE && !handle.is_null() {
                let handle = owned(handle, "open readiness peer")?;
                verify_server(&handle, expected_server)?;
                let mode = PIPE_READMODE_MESSAGE | PIPE_NOWAIT;
                ensure!(
                    unsafe {
                        SetNamedPipeHandleState(raw(&handle), &mode, ptr::null(), ptr::null())
                    } != 0,
                    "set nonblocking readiness client: {}",
                    io::Error::last_os_error()
                );
                return Ok(handle);
            }
            let error = unsafe { GetLastError() };
            ensure!(
                error == ERROR_PIPE_BUSY || error == ERROR_FILE_NOT_FOUND,
                "open readiness pipe: {}",
                io::Error::from_raw_os_error(error as i32)
            );
            ensure!(
                Instant::now() < deadline,
                "readiness pipe connection deadline"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    fn write_message(handle: &OwnedHandle, bytes: &[u8]) -> Result<()> {
        let mut written = 0;
        ensure!(
            unsafe {
                WriteFile(
                    raw(handle),
                    bytes.as_ptr(),
                    bytes.len() as u32,
                    &mut written,
                    ptr::null_mut(),
                )
            } != 0,
            "write readiness message: {}",
            io::Error::last_os_error()
        );
        ensure!(written as usize == bytes.len(), "partial readiness message");
        Ok(())
    }
    /// Holds the client connection until the authenticated server acknowledges
    /// it, so the server can query the actual peer PID before the handle closes.
    /// Busy connection retries and acknowledgement share one three-second bound.
    pub fn send_ready(pipe_name: &str, attempt: &str, generation_sha: &str) -> Result<()> {
        identities(attempt, generation_sha)?;
        let deadline = Instant::now() + CLIENT_TIMEOUT;
        let handle = connect_client(pipe_name, attempt, deadline)?;
        let bytes = serde_json::to_vec(&Message {
            schema: SCHEMA.into(),
            attempt: attempt.into(),
            generation_sha256: generation_sha.into(),
            pid: std::process::id(),
        })?;
        ensure!(bytes.len() <= MAX_MESSAGE, "readiness JSON byte bound");
        write_message(&handle, &bytes)?;
        loop {
            let mut bytes = [0u8; 32];
            let mut received = 0;
            if unsafe {
                ReadFile(
                    raw(&handle),
                    bytes.as_mut_ptr(),
                    bytes.len() as u32,
                    &mut received,
                    ptr::null_mut(),
                )
            } != 0
            {
                ensure!(
                    &bytes[..received as usize] == ACK,
                    "invalid readiness acknowledgement"
                );
                return Ok(());
            }
            let error = unsafe { GetLastError() };
            ensure!(
                error == ERROR_NO_DATA,
                "read readiness acknowledgement: {}",
                io::Error::from_raw_os_error(error as i32)
            );
            ensure!(
                Instant::now() < deadline,
                "readiness acknowledgement deadline"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::sync::atomic::{AtomicU64, Ordering};
        fn nonce() -> String {
            static COUNTER: AtomicU64 = AtomicU64::new(0);
            crate::model::hash(
                format!(
                    "runtime-fixture:{}:{}",
                    std::process::id(),
                    COUNTER.fetch_add(1, Ordering::Relaxed)
                )
                .as_bytes(),
            )
        }
        fn ready(attempt: &str, sha: &str) -> Message {
            Message {
                schema: SCHEMA.into(),
                attempt: attempt.into(),
                generation_sha256: sha.into(),
                pid: std::process::id(),
            }
        }
        fn client(server: &ReadyServer, attempt: &str) -> OwnedHandle {
            connect_client(server.pipe_name(), attempt, Instant::now() + CLIENT_TIMEOUT).unwrap()
        }
        #[test]
        fn ready_nonblocking_same_kernel_pid_and_first_instance() {
            let attempt = nonce();
            let sha = nonce();
            let mut server = ReadyServer::new(&attempt, &sha).unwrap();
            let before = Instant::now();
            assert!(!server.poll(std::process::id()).unwrap());
            assert!(before.elapsed() < Duration::from_secs(1));
            assert!(ReadyServer::new(&attempt, &sha).is_err());
            let pipe = server.pipe_name().to_owned();
            let sender = std::thread::spawn(move || send_ready(&pipe, &attempt, &sha));
            let deadline = Instant::now() + Duration::from_secs(4);
            while !server.poll(std::process::id()).unwrap() {
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(2));
            }
            sender.join().unwrap().unwrap();
            assert!(server.poll(std::process::id()).unwrap());
            assert!(server.poll(std::process::id().wrapping_add(1)).is_err());
        }
        #[test]
        fn wrong_kernel_pid_nonce_generation_and_message_pid_are_terminal() {
            for kind in 0..4 {
                let attempt = nonce();
                let sha = nonce();
                let mut server = ReadyServer::new(&attempt, &sha).unwrap();
                let handle = client(&server, &attempt);
                let mut message = ready(&attempt, &sha);
                match kind {
                    1 => message.attempt = nonce(),
                    2 => message.generation_sha256 = nonce(),
                    3 => message.pid = message.pid.wrapping_add(1),
                    _ => {}
                }
                write_message(&handle, &serde_json::to_vec(&message).unwrap()).unwrap();
                let expected = if kind == 0 {
                    std::process::id().wrapping_add(1)
                } else {
                    std::process::id()
                };
                assert!(server.poll(expected).is_err(), "kind {kind}");
                assert!(server.poll(std::process::id()).is_err());
            }
        }
        #[test]
        fn oversized_unknown_duplicate_and_foreign_pipe_identity_rejected() {
            for kind in 0..4 {
                let attempt = nonce();
                let sha = nonce();
                let mut server = ReadyServer::new(&attempt, &sha).unwrap();
                let handle = client(&server, &attempt);
                let mut message = serde_json::to_value(ready(&attempt, &sha)).unwrap();
                let bytes = match kind {
                    0 => vec![b'X'; MAX_MESSAGE + 1],
                    1 => {
                        message["unknown"] = true.into();
                        serde_json::to_vec(&message).unwrap()
                    }
                    2 => {
                        let text = serde_json::to_string(&message).unwrap();
                        format!("{{\"schema\":\"{SCHEMA}\",{}", &text[1..]).into_bytes()
                    }
                    _ => b"{}".to_vec(),
                };
                write_message(&handle, &bytes).unwrap();
                assert!(server.poll(std::process::id()).is_err(), "kind {kind}");
            }
            let attempt = nonce();
            let sha = nonce();
            let server = ReadyServer::new(&attempt, &sha).unwrap();
            assert!(send_ready(server.pipe_name(), &nonce(), &sha).is_err());
            assert!(send_ready(r"\\remote\pipe\other", &attempt, &sha).is_err());
            let handle = client(&server, &attempt);
            assert!(verify_server(&handle, std::process::id().wrapping_add(1)).is_err());
        }
        #[test]
        fn pipe_actual_protected_acl_has_only_system_and_current_user() {
            use windows_sys::Win32::Security::{
                Authorization::{GetSecurityInfo, SE_KERNEL_OBJECT},
                GetAce, GetSecurityDescriptorControl, ACCESS_ALLOWED_ACE, ACL,
                DACL_SECURITY_INFORMATION, SE_DACL_PROTECTED,
            };
            let server = ReadyServer::new(&nonce(), &nonce()).unwrap();
            let mut acl: *mut ACL = ptr::null_mut();
            let mut descriptor = ptr::null_mut();
            assert_eq!(
                unsafe {
                    GetSecurityInfo(
                        raw(&server.handle),
                        SE_KERNEL_OBJECT,
                        DACL_SECURITY_INFORMATION,
                        ptr::null_mut(),
                        ptr::null_mut(),
                        &mut acl,
                        ptr::null_mut(),
                        &mut descriptor,
                    )
                },
                0
            );
            let _allocation = LocalMemory(descriptor);
            let mut control = 0;
            let mut revision = 0;
            assert_ne!(
                unsafe { GetSecurityDescriptorControl(descriptor, &mut control, &mut revision) },
                0
            );
            assert_ne!(control & SE_DACL_PROTECTED, 0);
            assert!(!acl.is_null());
            assert_eq!(unsafe { (*acl).AceCount }, 2);
            let mut sids = std::collections::BTreeSet::new();
            for index in 0..2 {
                let mut ace = ptr::null_mut();
                assert_ne!(unsafe { GetAce(acl, index, &mut ace) }, 0);
                let ace = unsafe { &*ace.cast::<ACCESS_ALLOWED_ACE>() };
                assert_eq!(ace.Header.AceType, 0);
                sids.insert(sid_string((&ace.SidStart as *const u32).cast_mut().cast()).unwrap());
            }
            assert_eq!(
                sids,
                ["S-1-5-18".into(), current_sid().unwrap()]
                    .into_iter()
                    .collect()
            );
        }
    }
}

#[cfg(windows)]
pub use windows::{send_ready, ReadyServer};

#[cfg(not(windows))]
pub struct ReadyServer {
    _unsupported: (),
}
#[cfg(not(windows))]
impl ReadyServer {
    pub fn new(_: &str, _: &str) -> Result<Self> {
        anyhow::bail!("Windows generation readiness is unsupported on this platform")
    }
    pub fn pipe_name(&self) -> &str {
        ""
    }
    pub fn poll(&mut self, _: u32) -> Result<bool> {
        anyhow::bail!("Windows generation readiness is unsupported on this platform")
    }
}
#[cfg(not(windows))]
pub fn send_ready(_: &str, _: &str, _: &str) -> Result<()> {
    anyhow::bail!("Windows generation readiness is unsupported on this platform")
}

#[cfg(all(test, windows))]
mod lease_tests {
    use super::*;
    #[test]
    fn actual_file_handle_lease_excludes_a_second_game_and_idle_probe() {
        let root = tempfile::tempdir().unwrap();
        ensure_idle(root.path()).unwrap();
        let lease = GameLease::acquire(root.path()).unwrap();
        assert!(GameLease::acquire(root.path()).is_err());
        assert!(ensure_idle(root.path()).is_err());
        assert!(std::fs::remove_file(root.path().join(".update/pack-game.lock")).is_err());
        drop(lease);
        ensure_idle(root.path()).unwrap();
    }
    #[test]
    fn hardlinked_lock_is_rejected_without_touching_other_file() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join(".update")).unwrap();
        let original = root.path().join("personal-file.txt");
        std::fs::write(&original, b"preserve personal").unwrap();
        std::fs::hard_link(&original, root.path().join(".update/pack-game.lock")).unwrap();
        assert!(GameLease::acquire(root.path()).is_err());
        assert_eq!(std::fs::read(original).unwrap(), b"preserve personal");
    }
}
