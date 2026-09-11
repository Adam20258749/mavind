//! Machine facts for the "system check" page. Sourced from
//! `mavind-install --probe` (JSON), with a pure-Rust fallback so the page still
//! works if the backend script is missing.

use serde::Deserialize;
use std::fs;
use std::process::Command;

#[derive(Debug, Clone, Deserialize)]
pub struct Disk {
    pub name: String,   // /dev/sda
    pub size: u64,      // bytes
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub bus: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Facts {
    pub mem_kb: u64,
    pub arch: String,
    pub cores: u32,
    pub firmware: String, // "uefi" | "bios"
    pub network: bool,
    pub disks: Vec<Disk>,
    /// Installed size of THIS image (core/compat/full differ by GBs) — read
    /// from a sidecar next to the live squashfs. Defaults to 1 GiB (the core
    /// tier's target) if the backend is too old to report it.
    #[serde(default = "default_required_bytes")]
    pub required_bytes: u64,
}
fn default_required_bytes() -> u64 {
    1024 * 1024 * 1024
}

impl Facts {
    pub fn gather() -> Facts {
        let mut f = Self::from_backend().unwrap_or_else(Self::fallback);
        // The backend's disk list can come back empty (older probe parser,
        // odd lsblk output). Re-scan directly before trusting "no disks".
        if f.disks.is_empty() {
            f.disks = Self::lsblk_disks();
        }
        f
    }

    fn from_backend() -> Option<Facts> {
        for bin in ["mavind-install", "/usr/bin/mavind-install"] {
            if let Ok(out) = Command::new(bin).arg("--probe").output() {
                if out.status.success() {
                    if let Ok(f) = serde_json::from_slice::<Facts>(&out.stdout) {
                        return Some(f);
                    }
                }
            }
        }
        None
    }

    fn fallback() -> Facts {
        let mem_kb = fs::read_to_string("/proc/meminfo")
            .ok()
            .and_then(|s| {
                s.lines()
                    .find(|l| l.starts_with("MemTotal:"))
                    .and_then(|l| l.split_whitespace().nth(1))
                    .and_then(|v| v.parse().ok())
            })
            .unwrap_or(0);
        let cores = fs::read_to_string("/proc/cpuinfo")
            .map(|s| s.matches("processor").count().max(1) as u32)
            .unwrap_or(1);
        let firmware = if std::path::Path::new("/sys/firmware/efi").exists() {
            "uefi"
        } else {
            "bios"
        }
        .to_string();
        let disks = Self::lsblk_disks();
        Facts {
            mem_kb,
            arch: std::env::consts::ARCH.to_string(),
            cores,
            firmware,
            network: Command::new("ip")
                .args(["route", "get", "1.1.1.1"])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false),
            disks,
            required_bytes: default_required_bytes(),
        }
    }

    fn lsblk_disks() -> Vec<Disk> {
        // `-P` pairs output: values are quoted, so `MODEL="VBOX HARDDISK"`
        // (with a space) doesn't shift the columns.
        let Ok(out) = Command::new("lsblk")
            .args(["-dnb", "-P", "-o", "NAME,SIZE,TYPE,MODEL,TRAN"])
            .output()
        else {
            return vec![];
        };
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(|l| {
                let get = |k: &str| pair_value(l, k);
                if get("TYPE").as_deref() != Some("disk") {
                    return None;
                }
                let name = get("NAME")?;
                Some(Disk {
                    name: format!("/dev/{name}"),
                    size: get("SIZE").and_then(|s| s.parse().ok()).unwrap_or(0),
                    model: get("MODEL").unwrap_or_default(),
                    bus: get("TRAN").unwrap_or_default(),
                })
            })
            .collect()
    }
}

/// Extract `value` from a `KEY="value"` token in an `lsblk -P` line.
fn pair_value(line: &str, key: &str) -> Option<String> {
    let needle = format!("{key}=\"");
    let start = line.find(&needle)? + needle.len();
    let end = line[start..].find('"')? + start;
    Some(line[start..end].to_string())
}

/// A single row on the system-check page.
pub enum Check {
    Pass(String),
    Warn(String),
    Fail(String),
}

impl Check {
    pub fn is_blocker(&self) -> bool {
        matches!(self, Check::Fail(_))
    }
}

pub fn evaluate(f: &Facts) -> Vec<Check> {
    let mut v = Vec::new();

    // architecture — hard requirement
    if f.arch == "x86_64" || f.arch == "amd64" {
        v.push(Check::Pass(format!("64-bit CPU ({}, {} cores)", f.arch, f.cores)));
    } else {
        v.push(Check::Fail(format!("Mavind needs a 64-bit x86 CPU (found {})", f.arch)));
    }

    // memory — 2 GB target, 1.5 GB hard floor for the live installer
    let gib = f.mem_kb as f64 / 1024.0 / 1024.0;
    if gib >= 2.0 {
        v.push(Check::Pass(format!("Memory: {gib:.1} GiB")));
    } else if gib >= 1.4 {
        v.push(Check::Warn(format!("Memory: {gib:.1} GiB — Mavind wants 2 GiB; it will be slow")));
    } else {
        v.push(Check::Fail(format!("Memory: {gib:.1} GiB — need at least 1.5 GiB to install")));
    }

    // Firmware is NOT a requirement — Mavind boots UEFI *and* BIOS. (The plan
    // still records which one, to pick the right GRUB target.)

    // a usable disk — sized against what THIS image actually needs (core,
    // compat and full differ by GBs), not a number only right for one tier.
    // Same 10% + 300 MB margin the install backend uses for its own
    // pre-flight check, so the two never disagree.
    let req_gib = f.required_bytes as f64 / 1e9;
    let need_gib = req_gib * 1.1 + 0.3;
    let biggest = f.disks.iter().map(|d| d.size).max().unwrap_or(0);
    let dgib = biggest as f64 / 1e9;
    if f.disks.is_empty() {
        v.push(Check::Fail("No disks detected to install onto".into()));
    } else if dgib >= need_gib {
        v.push(Check::Pass(format!("Disk available: {dgib:.1} GB (this image needs ~{need_gib:.1} GB)")));
    } else if dgib >= req_gib {
        v.push(Check::Warn(format!(
            "Disk is only {dgib:.1} GB — this image needs ~{need_gib:.1} GB with safety margin; it may just barely fit"
        )));
    } else {
        v.push(Check::Fail(format!(
            "Disk is only {dgib:.1} GB — this image needs about {need_gib:.1} GB minimum"
        )));
    }

    // network — informational
    if f.network {
        v.push(Check::Pass("Network: connected".into()));
    } else {
        v.push(Check::Warn("Network: offline — install works, updates come later".into()));
    }

    v
}

pub fn human_size(bytes: u64) -> String {
    let gb = bytes as f64 / 1e9;
    if gb >= 1.0 {
        format!("{gb:.1} GB")
    } else {
        format!("{:.0} MB", bytes as f64 / 1e6)
    }
}
