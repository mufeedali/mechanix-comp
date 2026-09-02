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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    if path.exists() {
        let text = std::fs::read_to_string(path)?;
        return toml::from_str(&text)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, DEFAULT_TOML)?;
    tracing::info!(?path, "wrote default homescreen config");
    Ok(HomeConfig::default())
}

pub fn save(path: &Path, cfg: &HomeConfig) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = toml::to_string_pretty(cfg)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    std::fs::write(path, text)
}