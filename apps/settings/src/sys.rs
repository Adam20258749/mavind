//! Small helpers that read real system state. No fabricated values.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn os_release() -> HashMap<String, String> {
    let mut m = HashMap::new();
    for p in ["/etc/os-release", "/usr/lib/os-release"] {
        if let Ok(s) = fs::read_to_string(p) {
            for line in s.lines() {
                if let Some((k, v)) = line.split_once('=') {
                    m.insert(k.trim().to_string(), v.trim().trim_matches('"').to_string());
                }
            }
            break;
        }
    }
    m
}

pub fn uname_r() -> String {
    run("uname", &["-r"]).unwrap_or_else(|| "?".into())
}

pub fn cpu_model() -> String {
    fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("model name"))
                .and_then(|l| l.split_once(':'))
                .map(|(_, v)| v.trim().to_string())
        })
        .unwrap_or_else(|| "unknown".into())
}

pub fn mem_total_kb() -> u64 {
    fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("MemTotal"))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|v| v.parse().ok())
        })
        .unwrap_or(0)
}

pub fn mem_used_kb() -> (u64, u64) {
    let s = fs::read_to_string("/proc/meminfo").unwrap_or_default();
    let get = |k: &str| -> u64 {
        s.lines()
            .find(|l| l.starts_with(k))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|v| v.parse().ok())
            .unwrap_or(0)
    };
    let total = get("MemTotal");
    let avail = get("MemAvailable");
    (total.saturating_sub(avail), total)
}

pub struct Fs {
    pub mount: String,
    pub total: u64,
    pub free: u64,
}

pub fn root_fs() -> Option<Fs> {
    disk_usage("/")
}

pub fn disk_usage(path: &str) -> Option<Fs> {
    // parse `df -B1 --output=target,size,avail <path>`
    let out = Command::new("df")
        .args(["-B1", "--output=target,size,avail", path])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let line = text.lines().nth(1)?;
    let f: Vec<&str> = line.split_whitespace().collect();
    if f.len() < 3 {
        return None;
    }
    Some(Fs {
        mount: f[0].to_string(),
        total: f[1].parse().ok()?,
        free: f[2].parse().ok()?,
    })
}

pub fn du_h(path: &str) -> Option<String> {
    if !Path::new(path).exists() {
        return None;
    }
    let out = Command::new("du").args(["-sxh", path]).output().ok()?;
    String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .next()
        .map(|s| s.to_string())
}

pub fn wine_version() -> Option<String> {
    run("wine", &["--version"])
}

pub fn prefixes_count() -> usize {
    let home = std::env::var("HOME").unwrap_or_default();
    fs::read_dir(PathBuf::from(home).join(".local/share/mavind/prefixes"))
        .map(|rd| rd.flatten().filter(|e| e.path().is_dir()).count())
        .unwrap_or(0)
}

pub fn nm_devices() -> Vec<(String, String, String)> {
    // (device, type, state)
    run("nmcli", &["-t", "-f", "DEVICE,TYPE,STATE", "device"])
        .unwrap_or_default()
        .lines()
        .filter_map(|l| {
            let mut it = l.split(':');
            Some((
                it.next()?.to_string(),
                it.next().unwrap_or("").to_string(),
                it.next().unwrap_or("").to_string(),
            ))
        })
        .collect()
}

pub fn wifi_list() -> Vec<(String, String)> {
    // (ssid, signal)
    run("nmcli", &["-t", "-f", "SSID,SIGNAL", "device", "wifi", "list"])
        .unwrap_or_default()
        .lines()
        .filter_map(|l| {
            let (ssid, sig) = l.rsplit_once(':')?;
            if ssid.is_empty() {
                None
            } else {
                Some((ssid.to_string(), sig.to_string()))
            }
        })
        .collect()
}

pub fn outputs() -> Vec<String> {
    run("wlr-randr", &[])
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.starts_with(char::is_whitespace) && !l.is_empty())
        .map(|l| l.to_string())
        .collect()
}

pub fn volume_percent() -> Option<i32> {
    let s = run("wpctl", &["get-volume", "@DEFAULT_AUDIO_SINK@"])?;
    s.split_whitespace()
        .nth(1)
        .and_then(|v| v.parse::<f32>().ok())
        .map(|v| (v * 100.0).round() as i32)
}

pub fn set_volume_percent(p: i32) {
    let _ = Command::new("wpctl")
        .args(["set-volume", "@DEFAULT_AUDIO_SINK@", &format!("{}%", p.clamp(0, 150))])
        .status();
}

pub fn upgradable_count() -> usize {
    run("apt", &["list", "--upgradable"])
        .map(|s| s.lines().filter(|l| l.contains("/")).count())
        .unwrap_or(0)
}

pub fn users() -> Vec<String> {
    fs::read_to_string("/etc/passwd")
        .unwrap_or_default()
        .lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split(':').collect();
            let uid: u32 = f.get(2)?.parse().ok()?;
            if (1000..60000).contains(&uid) {
                Some(format!("{} (uid {uid})", f[0]))
            } else {
                None
            }
        })
        .collect()
}

pub fn firewall_status() -> String {
    if let Some(s) = run("ufw", &["status"]) {
        return s.lines().next().unwrap_or("").to_string();
    }
    if run("nft", &["list", "ruleset"]).map(|s| !s.trim().is_empty()).unwrap_or(false) {
        return "nftables ruleset present".into();
    }
    "no firewall configured".into()
}

fn run(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!s.is_empty()).then_some(s)
}

// ---- Performance Mode config (shared with the shell & session) ----------
pub fn perf_conf_path() -> PathBuf {
    let base = std::env::var("XDG_CONFIG_HOME")
        .ok()
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config"));
    base.join("mavind/performance.conf")
}

pub fn perf_enabled() -> bool {
    fs::read_to_string(perf_conf_path())
        .map(|s| {
            s.lines().any(|l| {
                let l = l.trim();
                l.starts_with("enabled") && l.contains("true")
            })
        })
        .unwrap_or(false)
}

pub fn set_perf_enabled(on: bool) -> std::io::Result<()> {
    let p = perf_conf_path();
    if let Some(d) = p.parent() {
        fs::create_dir_all(d)?;
    }
    let mut body = fs::read_to_string(&p).unwrap_or_default();
    if body.is_empty() {
        body = "# Mavind Performance Mode\nenabled = false\n".into();
    }
    let mut wrote = false;
    let new: String = body
        .lines()
        .map(|l| {
            if l.trim_start().starts_with("enabled") {
                wrote = true;
                format!("enabled = {}", on)
            } else {
                l.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let new = if wrote {
        format!("{new}\n")
    } else {
        format!("{new}\nenabled = {on}\n")
    };
    fs::write(&p, new)?;
    // Nudge the running shell to re-read (best effort).
    let _ = Command::new("pkill").args(["-USR1", "-f", "mavind-shell"]).status();
    Ok(())
}
