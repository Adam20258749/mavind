//! Status readouts for the panel. Every value comes from the real system:
//! `/sys/class/power_supply` for battery, `wpctl` for volume, `nmcli` for
//! network. Nothing here is synthesised.

use std::fs;
use std::process::Command;

/// Local time as `HH:MM  ·  ddd DD Mon`.
pub fn clock_text() -> String {
    // Avoid a chrono dependency: ask `date`. It's coreutils, always present.
    run("date", &["+%H:%M  ·  %a %d %b"]).unwrap_or_else(|| "--:--".into())
}

/// Default sink volume, e.g. `♪ 42%` or `♪ mute`.
pub fn volume_text() -> String {
    match run("wpctl", &["get-volume", "@DEFAULT_AUDIO_SINK@"]) {
        // "Volume: 0.42" or "Volume: 0.42 [MUTED]"
        Some(s) => {
            let muted = s.contains("[MUTED]") || s.to_lowercase().contains("muted");
            let pct = s
                .split_whitespace()
                .nth(1)
                .and_then(|v| v.parse::<f32>().ok())
                .map(|v| (v * 100.0).round() as i32);
            match (muted, pct) {
                (true, _) => "♪ mute".into(),
                (false, Some(p)) => format!("♪ {p}%"),
                _ => "♪ —".into(),
            }
        }
        None => String::new(),
    }
}

/// Network state, e.g. `⇅ wifi MyAP` / `⇅ eth` / `⇅ off`.
pub fn network_text() -> String {
    // `nmcli -t -f TYPE,STATE,CONNECTION dev` -> lines like `wifi:connected:MyAP`
    let Some(out) = run("nmcli", &["-t", "-f", "TYPE,STATE,CONNECTION", "device"]) else {
        return String::new();
    };
    let mut best: Option<String> = None;
    for line in out.lines() {
        let mut it = line.split(':');
        let (ty, st, conn) = (it.next().unwrap_or(""), it.next().unwrap_or(""), it.next().unwrap_or(""));
        if st != "connected" {
            continue;
        }
        match ty {
            "wifi" => return format!("⇅ {}", if conn.is_empty() { "wifi" } else { conn }),
            "ethernet" => best = Some("⇅ eth".into()),
            _ => {}
        }
    }
    best.unwrap_or_else(|| "⇅ off".into())
}

/// Battery percentage + charge state, or `None` on desktops (no battery).
pub fn battery_text() -> Option<String> {
    let base = "/sys/class/power_supply";
    let entries = fs::read_dir(base).ok()?;
    for e in entries.flatten() {
        let p = e.path();
        let is_batt = fs::read_to_string(p.join("type"))
            .map(|t| t.trim() == "Battery")
            .unwrap_or(false);
        if !is_batt {
            continue;
        }
        let cap = fs::read_to_string(p.join("capacity")).ok()?;
        let cap: i32 = cap.trim().parse().ok()?;
        let st = fs::read_to_string(p.join("status"))
            .unwrap_or_default()
            .trim()
            .to_string();
        let glyph = match st.as_str() {
            "Charging" => "⚡",
            "Full" => "▪",
            _ if cap <= 10 => "!",
            _ => "",
        };
        return Some(format!("{glyph}{cap}%"));
    }
    None
}

fn run(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}
