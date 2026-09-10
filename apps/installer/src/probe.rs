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
}

impl Facts {
    pub fn gather() -> Facts {
        if let Some(f) = Self::from_backend() {
            return f;
        }
        Self::fallback()
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
        }
    }

    fn lsblk_disks() -> Vec<Disk> {
        let Ok(out) = Command::new("lsblk")
            .args(["-dnb", "-o", "NAME,SIZE,MODEL,TRAN,TYPE"])
            .output()
        else {
            return vec![];
        };
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(|l| {
                let f: Vec<&str> = l.split_whitespace().collect();
                if f.len() < 2 || f.last() != Some(&"disk") {
                    return None;
                }
                Some(Disk {
                    name: format!("/dev/{}", f[0]),
                    size: f[1].parse().unwrap_or(0),
                    model: f.get(2).map(|s| s.to_string()).unwrap_or_default(),
                    bus: f.get(3).map(|s| s.to_string()).unwrap_or_default(),
                })
            })
            .collect()
    }
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

    // firmware
    match f.firmware.as_str() {
        "uefi" => v.push(Check::Pass("Firmware: UEFI".into())),
        _ => v.push(Check::Warn("Firmware: BIOS/legacy — supported, UEFI recommended".into())),
    }

    // a usable disk of a sane size
    let biggest = f.disks.iter().map(|d| d.size).max().unwrap_or(0);
    let dgib = biggest as f64 / 1e9;
    if f.disks.is_empty() {
        v.push(Check::Fail("No disks detected to install onto".into()));
    } else if dgib >= 8.0 {
        v.push(Check::Pass(format!("Disk available: {dgib:.0} GB")));
    } else if dgib >= 4.0 {
        v.push(Check::Warn(format!("Largest disk is {dgib:.1} GB — tight; 8 GB+ recommended")));
    } else {
        v.push(Check::Fail(format!("Largest disk is only {dgib:.1} GB — need ~4 GB minimum")));
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
