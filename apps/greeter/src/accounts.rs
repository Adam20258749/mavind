//! Local login-capable accounts, read straight from /etc/passwd (no PAM/NSS
//! calls needed just to list names — greetd/PAM does the real auth).

use std::fs;

#[derive(Clone)]
pub struct Account {
    pub username: String,
    pub full_name: String,
}

pub fn list() -> Vec<Account> {
    let Ok(text) = fs::read_to_string("/etc/passwd") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split(':').collect();
        if f.len() < 7 {
            continue;
        }
        let (username, uid, gecos, shell) = (f[0], f[2], f[4], f[6]);
        let Ok(uid) = uid.parse::<u32>() else { continue };
        if !(1000..60000).contains(&uid) {
            continue;
        }
        if shell.ends_with("nologin") || shell.ends_with("false") || shell.is_empty() {
            continue;
        }
        let full_name = gecos.split(',').next().unwrap_or("").trim();
        out.push(Account {
            username: username.to_string(),
            full_name: if full_name.is_empty() { username.to_string() } else { full_name.to_string() },
        });
    }
    out.sort_by(|a, b| a.username.cmp(&b.username));
    out
}
