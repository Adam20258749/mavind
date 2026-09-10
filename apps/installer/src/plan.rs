//! The install plan the GUI builds and hands to `mavind-install --plan`.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize)]
pub struct InstallPlan {
    pub disk: String,
    pub firmware: String,
    pub locale: String,
    pub keymap: String,
    pub timezone: String,
    pub hostname: String,
    pub version_id: String,
    /// The graphical installer leaves account creation to the OOBE.
    pub create_user: bool,
}

impl Default for InstallPlan {
    fn default() -> Self {
        InstallPlan {
            disk: String::new(),
            firmware: if std::path::Path::new("/sys/firmware/efi").exists() {
                "uefi"
            } else {
                "bios"
            }
            .into(),
            locale: "en_US.UTF-8".into(),
            keymap: "us".into(),
            timezone: "UTC".into(),
            hostname: "mavind".into(),
            version_id: current_version(),
            create_user: false,
        }
    }
}

impl InstallPlan {
    /// Write the plan JSON somewhere the privileged backend can read it.
    pub fn write(&self) -> std::io::Result<PathBuf> {
        let dir = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/tmp"));
        let path = dir.join("mavind-install-plan.json");
        fs::write(&path, serde_json::to_vec_pretty(self).unwrap_or_default())?;
        Ok(path)
    }
}

/// Read the version stamped into the running ISO.
pub fn current_version() -> String {
    // /usr/lib/mavind/release.json  -> {"version_id": "...", ...}
    if let Ok(s) = fs::read_to_string("/usr/lib/mavind/release.json") {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&s) {
            if let Some(id) = v.get("version_id").and_then(|x| x.as_str()) {
                return id.to_string();
            }
        }
    }
    // fall back to os-release
    for p in ["/etc/os-release", "/usr/lib/os-release"] {
        if let Ok(s) = fs::read_to_string(p) {
            for line in s.lines() {
                if let Some(rest) = line.strip_prefix("VERSION_ID=") {
                    return rest.trim().trim_matches('"').to_string();
                }
            }
        }
    }
    "0.1.0".into()
}

/// Releases offered on the "choose version" page. Multi-version media ship
/// /usr/lib/mavind/releases.json (an array of {version_id, pretty, channel});
/// otherwise it's just the one on this ISO.
#[derive(Debug, Clone, Deserialize)]
pub struct Release {
    pub version_id: String,
    #[serde(default)]
    pub pretty: String,
    #[serde(default = "default_channel")]
    pub channel: String,
}
fn default_channel() -> String {
    "core".into()
}

pub fn available_releases() -> Vec<Release> {
    if let Ok(s) = fs::read_to_string("/usr/lib/mavind/releases.json") {
        if let Ok(list) = serde_json::from_str::<Vec<Release>>(&s) {
            if !list.is_empty() {
                return list;
            }
        }
    }
    let v = current_version();
    vec![Release {
        pretty: format!("Mavind {v}"),
        version_id: v,
        channel: "core".into(),
    }]
}
