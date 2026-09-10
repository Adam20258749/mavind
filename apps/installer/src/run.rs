//! Drives `pkexec mavind-install --plan <file>` and reports progress.
//!
//! A worker thread runs the backend and reads its stdout/stderr; messages come
//! back over an mpsc channel that the GTK side drains on a short timer (no glib
//! channel API — keeps this compatible across gtk4-rs versions).

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, Sender};
use std::thread;

pub enum Msg {
    Progress(u8, String),
    Log(String),
    Done(i32),
}

pub fn start(plan_path: &Path) -> Receiver<Msg> {
    let (tx, rx) = std::sync::mpsc::channel::<Msg>();
    let plan = plan_path.to_path_buf();
    thread::spawn(move || {
        let runner = pick_privileged_runner();
        let mut cmd = Command::new(&runner[0]);
        cmd.args(&runner[1..])
            .arg("mavind-install")
            .arg("--plan")
            .arg(&plan)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                let _ = tx.send(Msg::Log(format!("failed to launch {}: {e}", runner[0])));
                let _ = tx.send(Msg::Done(127));
                return;
            }
        };

        if let Some(err) = child.stderr.take() {
            let tx2: Sender<Msg> = tx.clone();
            thread::spawn(move || {
                for line in BufReader::new(err).lines().map_while(Result::ok) {
                    let _ = tx2.send(Msg::Log(line));
                }
            });
        }
        if let Some(out) = child.stdout.take() {
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                if let Some(rest) = line.strip_prefix("PROGRESS:") {
                    let rest = rest.trim();
                    let (pct, msg) = rest.split_once(' ').unwrap_or((rest, ""));
                    let pct = pct.trim().parse::<u8>().unwrap_or(0).min(100);
                    let _ = tx.send(Msg::Progress(pct, msg.trim().to_string()));
                } else if !line.is_empty() {
                    let _ = tx.send(Msg::Log(line));
                }
            }
        }
        let code = child.wait().map(|s| s.code().unwrap_or(-1)).unwrap_or(-1);
        let _ = tx.send(Msg::Done(code));
    });
    rx
}

/// Prefer pkexec (polkit prompt); fall back to sudo -n, then plain (already root).
fn pick_privileged_runner() -> Vec<String> {
    if which("pkexec").is_some() {
        return vec!["pkexec".into()];
    }
    if which("sudo").is_some() {
        return vec!["sudo".into()];
    }
    vec!["/bin/sh".into(), "-c".into(), "exec \"$@\"".into(), "sh".into()]
}

fn which(bin: &str) -> Option<()> {
    std::env::var_os("PATH").and_then(|p| {
        std::env::split_paths(&p)
            .any(|d| d.join(bin).is_file())
            .then_some(())
    })
}
