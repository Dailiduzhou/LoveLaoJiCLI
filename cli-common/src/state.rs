//! Private, versioned, authenticated records. Locks are OS locks, never PID files.
use crate::{error, Result};
use rand::RngCore;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
pub fn id() -> String {
    format!(
        "{:016x}{:016x}",
        rand::random::<u64>(),
        rand::random::<u64>()
    )
}
pub fn digest(data: &[u8]) -> String {
    blake3::hash(data).to_hex().to_string()
}
pub fn path_bytes(path: &Path) -> Vec<u8> {
    path.as_os_str().as_bytes().to_vec()
}
pub fn path_from(bytes: &[u8]) -> PathBuf {
    std::ffi::OsString::from_vec(bytes.to_vec()).into()
}
pub fn fields(key: &[u8; 32], values: impl IntoIterator<Item = impl AsRef<[u8]>>) -> String {
    let mut h = blake3::Hasher::new_keyed(key);
    for v in values {
        let v = v.as_ref();
        h.update(&(v.len() as u64).to_le_bytes());
        h.update(v);
    }
    h.finalize().to_hex().to_string()
}
fn no_links(path: &Path) -> Result<()> {
    let mut p = PathBuf::new();
    for c in path.components() {
        p.push(c);
        if fs::symlink_metadata(&p)?.file_type().is_symlink() {
            return Err(error(
                "Symlink in managed state path",
                "状态路径中存在符号链接",
            ));
        }
    }
    Ok(())
}
pub fn private_dir(path: &Path) -> Result<()> {
    if !path.exists() {
        let parent = path
            .parent()
            .ok_or_else(|| error("Invalid state path", "状态路径无效"))?;
        if !parent.exists() {
            private_dir(parent)?;
        }
        no_links(parent)?;
        match fs::DirBuilder::new().mode(0o700).create(path) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
            Err(e) => return Err(e),
        }
    }
    no_links(path)?;
    let m = fs::symlink_metadata(path)?;
    if !m.is_dir() || m.uid() != rustix::process::geteuid().as_raw() || m.mode() & 0o077 != 0 {
        return Err(error(
            "State directory must be owned by you and mode 0700",
            "状态目录必须归当前用户所有且权限为 0700",
        ));
    }
    Ok(())
}
fn open(path: &Path, create: bool) -> Result<File> {
    let parent = path.parent().unwrap();
    no_links(parent)?;
    let pm = fs::metadata(parent)?;
    if pm.uid() != rustix::process::geteuid().as_raw() || pm.mode() & 0o077 != 0 {
        return Err(error(
            "Unsafe state parent permissions",
            "状态父目录权限不安全",
        ));
    }
    let f = OpenOptions::new()
        .read(true)
        .write(create)
        .create(create)
        .mode(0o600)
        .custom_flags((rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK).bits() as i32)
        .open(path)?;
    let m = f.metadata()?;
    if !m.is_file()
        || m.uid() != rustix::process::geteuid().as_raw()
        || m.mode() & 0o077 != 0
        || m.nlink() != 1
    {
        return Err(error(
            "Unsafe state file permissions/type",
            "状态文件权限或类型不安全",
        ));
    }
    Ok(f)
}
/// Validate an existing private directory; never create a missing one.
pub fn existing_private_dir(path: &Path) -> Result<bool> {
    if let Err(e) = no_links(path) {
        return if e.kind() == std::io::ErrorKind::NotFound {
            Ok(false)
        } else {
            Err(e)
        };
    }
    let m = fs::metadata(path)?;
    if !m.is_dir() || m.uid() != rustix::process::geteuid().as_raw() || m.mode() & 0o077 != 0 {
        return Err(error("Unsafe state directory", "状态目录不安全"));
    }
    Ok(true)
}
pub fn lock_readonly(path: &Path) -> Result<File> {
    let f = open(path, false)?;
    f.try_lock_shared().map_err(|_| {
        error(
            "State is busy or locking is unsupported",
            "状态忙碌或文件系统不支持锁",
        )
    })?;
    Ok(f)
}
pub fn lock(path: &Path) -> Result<File> {
    let f = open(path, true)?;
    f.try_lock().map_err(|_| {
        error(
            "State is busy or locking is unsupported",
            "状态忙碌或文件系统不支持锁",
        )
    })?;
    Ok(f)
}
/// Same-directory replacement, leaving the old record intact on failure.
pub fn atomic(path: &Path, bytes: &[u8], mode: u32) -> Result<()> {
    let parent = path.parent().unwrap();
    no_links(parent)?;
    if let Ok(m) = fs::symlink_metadata(path) {
        if !m.is_file() || m.file_type().is_symlink() || m.nlink() != 1 {
            return Err(error("Unsafe replacement target", "替换目标不安全"));
        }
    }
    let temp = parent.join(format!(".humanutils-{}", id()));
    let result = (|| {
        let mut f = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(mode)
            .open(&temp)?;
        f.write_all(bytes)?;
        f.set_permissions(fs::Permissions::from_mode(mode))?;
        f.sync_all()?;
        fs::rename(&temp, path)?;
        File::open(parent)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}
#[derive(Serialize, Deserialize)]
struct Envelope {
    schema_version: u32,
    payload: String,
    checksum: String,
}
#[derive(Clone)]
pub struct Store {
    pub root: PathBuf,
    pub key: [u8; 32],
}
impl Store {
    fn base() -> Result<PathBuf> {
        std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .filter(|p| p.is_absolute())
                    .map(|p| p.join(".local/state"))
            })
            .ok_or_else(|| {
                error(
                    "No absolute XDG_STATE_HOME or HOME",
                    "缺少绝对路径的 XDG_STATE_HOME 或 HOME",
                )
            })
    }
    /// Inspect existing state without creating keys, directories, locks or records.
    /// A missing store is empty; a present store with missing identity is unavailable.
    pub fn open_readonly() -> Result<Option<Self>> {
        let base = Self::base()?;
        if let Err(e) = no_links(&base) {
            return if e.kind() == std::io::ErrorKind::NotFound {
                Ok(None)
            } else {
                Err(e)
            };
        }
        let metadata = fs::metadata(&base)?;
        if metadata.uid() != rustix::process::geteuid().as_raw() || metadata.mode() & 0o022 != 0 {
            return Err(error(
                "Untrusted state parent directory",
                "状态父目录不可信",
            ));
        }
        let root = base.join("lovelaojicli");
        if !existing_private_dir(&root)? {
            return Ok(None);
        }
        let _lock = lock_readonly(&root.join("identity.lock"))?;
        let mut bytes = Vec::new();
        open(&root.join("identity.key"), false)?
            .take(33)
            .read_to_end(&mut bytes)?;
        let key = bytes
            .try_into()
            .map_err(|_| error("Invalid identity key", "身份密钥无效"))?;
        Ok(Some(Self { root, key }))
    }
    pub fn open() -> Result<Self> {
        let base = Self::base()?;
        // XDG_STATE_HOME may legitimately be 0755; only our subtree must be private.
        if !base.exists() {
            private_dir(&base)?;
        } else {
            no_links(&base)?;
        }
        let metadata = fs::metadata(&base)?;
        if metadata.uid() != rustix::process::geteuid().as_raw() || metadata.mode() & 0o022 != 0 {
            return Err(error(
                "Untrusted state parent directory",
                "状态父目录不可信",
            ));
        }
        let root = base.join("lovelaojicli");
        private_dir(&root)?;
        let _lock = lock(&root.join("identity.lock"))?;
        let path = root.join("identity.key");
        let mut key = [0; 32];
        match open(&path, false) {
            Ok(f) => {
                let mut b = Vec::new();
                f.take(33).read_to_end(&mut b)?;
                if b.len() != 32 {
                    return Err(error(
                        "Invalid identity key; records are unavailable",
                        "身份密钥损坏，记录不可用",
                    ));
                }
                key.copy_from_slice(&b);
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                rand::rngs::OsRng
                    .try_fill_bytes(&mut key)
                    .map_err(std::io::Error::other)?;
                atomic(&path, &key, 0o600)?;
            }
            Err(e) => return Err(e),
        }
        Ok(Self { root, key })
    }
    pub fn dir(&self, relative: &str) -> Result<PathBuf> {
        let path = self.root.join(relative);
        private_dir(&path)?;
        Ok(path)
    }
    pub fn workspace_path(&self, path: &Path) -> PathBuf {
        self.root
            .join("workspaces")
            .join(fields(&self.key, [path.as_os_str().as_bytes()]))
    }
    pub fn workspace(&self, path: &Path) -> Result<PathBuf> {
        let dir = self.workspace_path(path);
        private_dir(&dir)?;
        Ok(dir)
    }
    pub fn read<T: DeserializeOwned>(&self, path: &Path) -> Result<Option<T>> {
        let mut f = match open(path, false) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };
        if f.metadata()?.len() > 16 * 1024 * 1024 {
            return Err(error("State record too large", "状态记录过大"));
        }
        let mut bytes = Vec::new();
        f.read_to_end(&mut bytes)?;
        let e: Envelope = serde_json::from_slice(&bytes)?;
        if e.schema_version != 1
            || fields(
                &self.key,
                [
                    path.strip_prefix(&self.root)
                        .map_err(std::io::Error::other)?
                        .as_os_str()
                        .as_bytes(),
                    e.payload.as_bytes(),
                ],
            ) != e.checksum
        {
            return Err(error(
                "Unsupported or damaged state record",
                "状态记录版本不支持或记录损坏",
            ));
        }
        Ok(Some(serde_json::from_str(&e.payload)?))
    }
    pub fn write<T: Serialize>(&self, path: &Path, value: &T) -> Result<()> {
        if path.exists() {
            let _ = open(path, false)?;
        }
        let payload = serde_json::to_string(value)?;
        let checksum = fields(
            &self.key,
            [
                path.strip_prefix(&self.root)
                    .map_err(std::io::Error::other)?
                    .as_os_str()
                    .as_bytes(),
                payload.as_bytes(),
            ],
        );
        let bytes = serde_json::to_vec(&Envelope {
            schema_version: 1,
            payload,
            checksum,
        })?;
        if bytes.len() > 16 * 1024 * 1024 {
            return Err(error("State record too large", "状态记录过大"));
        }
        atomic(path, &bytes, 0o600)
    }
}
