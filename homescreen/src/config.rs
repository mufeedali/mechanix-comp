use std::ffi::{CString, OsStr, OsString};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const DEFAULT_TOML: &str = r#"# Mechanix homescreen. Add slots and icons here; edit mode writes back.
columns = 4
rows = 6

[[slots]]
page = 0
col = 0
row = 0
col_span = 2
row_span = 2
command = "weston-simple-egl"

[[slots]]
page = 0
col = 2
row = 0
col_span = 2
row_span = 2
command = "weston-simple-shm"

[[icons]]
page = 0
col = 0
row = 4
desktop = "/usr/share/applications/org.gtk.Demo4.desktop"

[[slots]]
page = 1
col = 0
row = 0
col_span = 2
row_span = 2
command = "weston-simple-shm"
"#;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HomeConfig {
    #[serde(default = "default_columns")]
    pub columns: u32,
    #[serde(default = "default_rows")]
    pub rows: u32,
    #[serde(default)]
    pub slots: Vec<SlotConfig>,
    #[serde(default)]
    pub icons: Vec<IconConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlotConfig {
    #[serde(default)]
    pub page: u32,
    pub col: u32,
    pub row: u32,
    #[serde(default = "default_span")]
    pub col_span: u32,
    #[serde(default = "default_span")]
    pub row_span: u32,
    pub command: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IconConfig {
    #[serde(default)]
    pub page: u32,
    pub col: u32,
    pub row: u32,
    pub desktop: String,
}

fn default_columns() -> u32 {
    4
}
fn default_rows() -> u32 {
    6
}
fn default_span() -> u32 {
    1
}

impl Default for HomeConfig {
    fn default() -> Self {
        toml::from_str(DEFAULT_TOML).unwrap_or(Self {
            columns: 4,
            rows: 6,
            slots: Vec::new(),
            icons: Vec::new(),
        })
    }
}

pub fn config_path() -> PathBuf {
    if let Some(p) = std::env::var_os("MECHANIX_HOME_CONFIG") {
        return PathBuf::from(p);
    }
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config"))
        })
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("mechanix/homescreen.toml")
}

pub fn load_or_init(path: &Path) -> std::io::Result<HomeConfig> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // FIXME: overwrite every launch so the checked-in defaults are what we run.
    write_atomic(path, DEFAULT_TOML.as_bytes())?;
    tracing::info!(?path, "overwrote homescreen config with defaults");
    Ok(HomeConfig::default())
}

pub fn load(path: &Path) -> std::io::Result<HomeConfig> {
    let text = std::fs::read_to_string(path)?;
    toml::from_str(&text).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

pub fn save(path: &Path, cfg: &HomeConfig) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = toml::to_string_pretty(cfg)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    write_atomic(path, text.as_bytes())
}

fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut tmp = path.as_os_str().to_os_string();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

/// Directory watch so atomic replace (`write` + `rename`) still notifies us.
pub struct ConfigWatch {
    fd: OwnedFd,
    name: OsString,
}

impl ConfigWatch {
    pub fn open(path: &Path) -> std::io::Result<Self> {
        let dir = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let name = path
            .file_name()
            .ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::InvalidInput, "config path has no file name")
            })?
            .to_os_string();
        let raw = unsafe { libc::inotify_init1(libc::IN_CLOEXEC | libc::IN_NONBLOCK) };
        if raw < 0 {
            return Err(std::io::Error::last_os_error());
        }
        let fd = unsafe { OwnedFd::from_raw_fd(raw) };
        let c_dir = CString::new(dir.as_os_str().as_bytes()).map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "config dir contains NUL")
        })?;
        let mask = libc::IN_CREATE
            | libc::IN_MOVED_TO
            | libc::IN_MODIFY
            | libc::IN_CLOSE_WRITE
            | libc::IN_ATTRIB;
        let wd = unsafe { libc::inotify_add_watch(fd.as_raw_fd(), c_dir.as_ptr(), mask as u32) };
        if wd < 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(Self { fd, name })
    }

    pub fn as_raw_fd(&self) -> RawFd {
        self.fd.as_raw_fd()
    }

    pub fn take_changed(&self) -> bool {
        let mut buf = [0u8; 4096];
        let mut changed = false;
        loop {
            let n = unsafe {
                libc::read(
                    self.fd.as_raw_fd(),
                    buf.as_mut_ptr() as *mut libc::c_void,
                    buf.len(),
                )
            };
            if n < 0 {
                let err = std::io::Error::last_os_error();
                if err.kind() != std::io::ErrorKind::WouldBlock {
                    tracing::warn!(%err, "config inotify read failed");
                }
                break;
            }
            if n == 0 {
                break;
            }
            let n = n as usize;
            let mut off = 0;
            let header = std::mem::size_of::<libc::inotify_event>();
            while off + header <= n {
                let ev = unsafe {
                    std::ptr::read_unaligned(buf.as_ptr().add(off) as *const libc::inotify_event)
                };
                let name_off = off + header;
                let name_len = ev.len as usize;
                if name_len > 0 && name_off + name_len <= n {
                    let bytes = &buf[name_off..name_off + name_len];
                    let end = bytes.iter().position(|&b| b == 0).unwrap_or(name_len);
                    if OsStr::from_bytes(&bytes[..end]) == self.name {
                        changed = true;
                    }
                }
                off = name_off + name_len;
            }
        }
        changed
    }
}
