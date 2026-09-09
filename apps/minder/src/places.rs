//! The Places sidebar: standard user dirs + currently-mounted removable drives.

use std::fs;
use std::path::PathBuf;

pub struct Place {
    pub label: String,
    pub path: PathBuf,
    pub removable: bool,
}

pub fn user_places() -> Vec<Place> {
    let home = PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/root".into()));
    let mut v = vec![Place {
        label: "Home".into(),
        path: home.clone(),
        removable: false,
    }];
    for (label, sub) in [
        ("Desktop", "Desktop"),
        ("Documents", "Documents"),
        ("Downloads", "Downloads"),
        ("Pictures", "Pictures"),
        ("Music", "Music"),
        ("Videos", "Videos"),
    ] {
        let p = home.join(sub);
        if p.is_dir() {
            v.push(Place {
                label: label.into(),
                path: p,
                removable: false,
            });
        }
    }
    v.push(Place {
        label: "File System".into(),
        path: PathBuf::from("/"),
        removable: false,
    });
    v
}

/// Removable/media mounts parsed from /proc/mounts (what udisks2 mounts land as).
pub fn drives() -> Vec<Place> {
    let Ok(mounts) = fs::read_to_string("/proc/mounts") else {
        return vec![];
    };
    let mut v = Vec::new();
    for line in mounts.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() < 3 {
            continue;
        }
        let (src, mnt, fstype) = (f[0], f[1], f[2]);
        let mnt_dec = mnt.replace("\\040", " ");
        let is_media = mnt_dec.starts_with("/media/")
            || mnt_dec.starts_with("/run/media/")
            || mnt_dec.starts_with("/mnt/");
        let real = src.starts_with("/dev/");
        if is_media && real && fstype != "iso9660" {
            let label = PathBuf::from(&mnt_dec)
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| src.to_string());
            v.push(Place {
                label,
                path: PathBuf::from(mnt_dec),
                removable: true,
            });
        }
    }
    v
}
