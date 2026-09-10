//! JSON-backed bookmarks & history for Mrowser.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Serialize, Deserialize)]
pub struct Item {
    #[serde(default)]
    pub title: String,
    pub url: String,
}

#[derive(Default, Serialize, Deserialize)]
pub struct List(pub Vec<Item>);

#[derive(Clone, Copy)]
pub enum Kind {
    Bookmarks,
    History,
}

impl List {
    pub fn push_unique(&mut self, title: &str, url: &str) {
        if !self.0.iter().any(|i| i.url == url) {
            self.0.push(Item { title: title.to_string(), url: url.to_string() });
        }
    }
    pub fn push_capped(&mut self, title: &str, url: &str, cap: usize) {
        if self.0.last().map(|i| i.url.as_str()) == Some(url) {
            return; // don't record the same page twice in a row
        }
        self.0.push(Item { title: title.to_string(), url: url.to_string() });
        if self.0.len() > cap {
            let excess = self.0.len() - cap;
            self.0.drain(0..excess);
        }
    }
}

fn path(k: Kind) -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
    let (base, file) = match k {
        Kind::Bookmarks => (
            std::env::var("XDG_CONFIG_HOME").ok().filter(|p| p.starts_with('/')),
            "bookmarks.json",
        ),
        Kind::History => (
            std::env::var("XDG_DATA_HOME").ok().filter(|p| p.starts_with('/')),
            "history.json",
        ),
    };
    let base = base.unwrap_or_else(|| match k {
        Kind::Bookmarks => format!("{home}/.config"),
        Kind::History => format!("{home}/.local/share"),
    });
    PathBuf::from(base).join("mrowser").join(file)
}

pub fn load(k: Kind) -> List {
    fs::read_to_string(path(k))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(k: Kind, l: &List) {
    let p = path(k);
    if let Some(d) = p.parent() {
        let _ = fs::create_dir_all(d);
    }
    if let Ok(s) = serde_json::to_string_pretty(l) {
        let _ = fs::write(p, s);
    }
}
