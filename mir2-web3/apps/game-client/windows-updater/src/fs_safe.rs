use anyhow::{ensure, Result};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};
pub fn ancestors(path: &Path) -> Result<()> {
    ensure!(path.is_absolute(), "relative filesystem root");
    for p in path.ancestors() {
        match fs::symlink_metadata(p) {
            Ok(m) => {
                ensure!(!m.file_type().is_symlink(), "linked filesystem path");
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    ensure!(m.file_attributes() & 0x400 == 0, "reparse filesystem path");
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}
pub fn target(root: &Path, relative: &str) -> Result<PathBuf> {
    crate::model::relative(relative)?;
    let p = root.join(relative);
    ancestors(&p)?;
    Ok(p)
}
pub fn regular(path: &Path) -> Result<fs::Metadata> {
    ancestors(path)?;
    let meta = fs::symlink_metadata(path)?;
    ensure!(meta.is_file(), "nonregular file");
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        ensure!(meta.nlink() == 1, "hard-linked file");
    }
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::{
            GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
        };
        let file = File::open(path)?;
        let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
        ensure!(
            unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } != 0,
            "file information failed"
        );
        ensure!(info.nNumberOfLinks == 1, "hard-linked file");
    }
    Ok(meta)
}
pub fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>> {
    ensure!(regular(path)?.len() <= limit, "file too large");
    let mut data = Vec::new();
    File::open(path)?.take(limit + 1).read_to_end(&mut data)?;
    ensure!(data.len() as u64 <= limit, "file too large");
    Ok(data)
}
pub fn digest_file(path: &Path) -> Result<(u64, String)> {
    let meta = regular(path)?;
    let mut file = File::open(path)?;
    let mut sha = Sha256::new();
    let mut size = 0;
    let mut buffer = [0u8; 65536];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        sha.update(&buffer[..n]);
        size += n as u64;
    }
    ensure!(size == meta.len(), "file changed during hashing");
    Ok((size, format!("{:X}", sha.finalize())))
}
pub fn matches(path: &Path, entry: &crate::model::FileEntry) -> Result<bool> {
    let meta = match regular(path) {
        Ok(m) => m,
        Err(_) if !path.exists() => return Ok(false),
        Err(e) => return Err(e),
    };
    if meta.len() != entry.size {
        return Ok(false);
    }
    let (size, hash) = digest_file(path)?;
    Ok(size == entry.size && hash == entry.sha256)
}
pub fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    ancestors(path)?;
    let parent = path.parent().unwrap();
    fs::create_dir_all(parent)?;
    ancestors(parent)?;
    let mut f = OpenOptions::new().write(true).create_new(true).open(path)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    Ok(())
}
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    ancestors(path)?;
    fs::create_dir_all(path.parent().unwrap())?;
    let temp = path.with_extension("tmp");
    if temp.exists() {
        regular(&temp)?;
        fs::remove_file(&temp)?;
    }
    write_new(&temp, bytes)?;
    crate::platform::replace_file(&temp, path)?;
    Ok(())
}
/// Used only for fixed scratch paths after validating every descendant. Never
/// recursively follow a reparse point, and never delete a caller-supplied root.
pub fn clear_scratch(root: &Path, name: &str) -> Result<()> {
    ensure!(["staging", "backup"].contains(&name), "unbounded cleanup");
    let dir = target(&root.join(".update"), name)?;
    if !dir.exists() {
        return Ok(());
    }
    fn collect(dir: &Path, files: &mut Vec<PathBuf>, dirs: &mut Vec<PathBuf>) -> Result<()> {
        ancestors(dir)?;
        ensure!(
            fs::symlink_metadata(dir)?.is_dir(),
            "scratch is not directory"
        );
        for item in fs::read_dir(dir)? {
            let p = item?.path();
            ancestors(&p)?;
            if fs::symlink_metadata(&p)?.is_dir() {
                collect(&p, files, dirs)?
            } else {
                regular(&p)?;
                files.push(p);
            }
        }
        dirs.push(dir.to_owned());
        Ok(())
    }
    let mut files = vec![];
    let mut dirs = vec![];
    collect(&dir, &mut files, &mut dirs)?;
    for p in files {
        fs::remove_file(p)?
    }
    for p in dirs {
        fs::remove_dir(p)?
    }
    Ok(())
}
pub fn install_root(root: &Path) -> Result<PathBuf> {
    ancestors(root)?;
    let s = root.to_string_lossy();
    ensure!(
        (!s.starts_with(r"\\") || (s.starts_with(r"\\?\") && !s.starts_with(r"\\?\UNC\")))
            && !s.starts_with("//"),
        "network installation is unsupported"
    );
    let root = fs::canonicalize(root)?;
    // canonicalize on Windows adds the extended prefix; permit that local form.
    ancestors(&root)?;
    ensure!(root.is_dir(), "invalid installation");
    regular(&root.join("Mir2Launcher.exe"))?;
    // The trusted engine must be able to recover/repair a missing game EXE.
    regular(&root.join("game/VERSION.json"))?;
    Ok(root)
}
