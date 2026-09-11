//! Minimal `.desktop` file scanning + launching — no external crate, so the
//! parser only understands the handful of keys Mavind actually needs.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Clone)]
pub struct AppEntry {
    pub name: String,
    pub exec: String,
    pub icon: String,
    pub terminal: bool,
    pub categories: Vec<String>,
}

pub fn scan() -> Vec<AppEntry> {
    let mut seen: HashMap<String, AppEntry> = HashMap::new(); // de-dup by name
    let home = std::env::var("HOME").unwrap_or_default();
    for dir in [
        "/usr/share/applications".to_string(),
        format!("{home}/.local/share/applications"),
    ] {
        let Ok(rd) = fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().map(|x| x == "desktop").unwrap_or(false) {
                if let Some(entry) = parse_desktop_file(&p) {
                    seen.insert(entry.name.clone(), entry);
                }
            }
        }
    }
    let mut v: Vec<AppEntry> = seen.into_values().collect();
    v.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    v
}

fn parse_desktop_file(path: &Path) -> Option<AppEntry> {
    let text = fs::read_to_string(path).ok()?;
    let mut in_main_group = false;
    let mut name = None;
    let mut exec = None;
    let mut icon = String::new();
    let mut categories = Vec::new();
    let mut no_display = false;
    let mut hidden = false;
    let mut terminal = false;

    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_main_group = line == "[Desktop Entry]";
            continue;
        }
        if !in_main_group || line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, val)) = line.split_once('=') else { continue };
        match key.trim() {
            "Name" if name.is_none() => name = Some(val.trim().to_string()),
            "Exec" => exec = Some(val.trim().to_string()),
            "Icon" => icon = val.trim().to_string(),
            "Categories" => {
                categories = val
                    .trim()
                    .split(';')
                    .filter(|c| !c.is_empty())
                    .map(|c| c.to_string())
                    .collect()
            }
            "NoDisplay" => no_display = val.trim().eq_ignore_ascii_case("true"),
            "Hidden" => hidden = val.trim().eq_ignore_ascii_case("true"),
            "Terminal" => terminal = val.trim().eq_ignore_ascii_case("true"),
            "Type" if val.trim() != "Application" => return None,
            _ => {}
        }
    }

    if no_display || hidden {
        return None;
    }
    Some(AppEntry {
        name: name?,
        exec: exec?,
        icon,
        terminal,
        categories,
    })
}

/// Strip desktop file field codes (%f %F %u %U %i %c %k ...) and spawn.
pub fn launch(entry: &AppEntry) {
    let clean = strip_field_codes(&entry.exec);
    let argv = split_argv(&clean);
    if argv.is_empty() {
        return;
    }
    let mut cmd = if entry.terminal {
        let mut c = Command::new("foot");
        c.arg("-e");
        c.args(&argv);
        c
    } else {
        let mut c = Command::new(&argv[0]);
        c.args(&argv[1..]);
        c
    };
    cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    let _ = cmd.spawn();
}

fn strip_field_codes(exec: &str) -> String {
    let mut out = String::with_capacity(exec.len());
    let mut chars = exec.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            match chars.peek() {
                Some('%') => {
                    out.push('%');
                    chars.next();
                }
                Some('f' | 'F' | 'u' | 'U' | 'i' | 'c' | 'k' | 'd' | 'D' | 'n' | 'N' | 'v' | 'm') => {
                    chars.next();
                }
                _ => out.push(c),
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// A small shell-like tokenizer: honours "..." and '...' quoting, nothing more.
fn split_argv(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    for c in s.chars() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => cur.push(c),
            None if c == '"' || c == '\'' => quote = Some(c),
            None if c.is_whitespace() => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            None => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

pub fn icon_path_or_name(icon: &str) -> IconRef {
    if icon.is_empty() {
        IconRef::Name("application-x-executable".into())
    } else if icon.starts_with('/') && Path::new(icon).is_file() {
        IconRef::Path(PathBuf::from(icon))
    } else {
        IconRef::Name(icon.to_string())
    }
}

pub enum IconRef {
    Name(String),
    Path(PathBuf),
}
