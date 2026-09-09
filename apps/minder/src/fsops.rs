//! Filesystem operations for Minder — plain `std::fs`, no external helpers.

use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub size: u64,
    pub mtime: i64,
}

pub fn list_dir(dir: &Path, show_hidden: bool) -> Result<Vec<Entry>> {
    let mut out = Vec::new();
    for e in fs::read_dir(dir).with_context(|| format!("open {}", dir.display()))? {
        let e = e?;
        let name = e.file_name().to_string_lossy().into_owned();
        if !show_hidden && name.starts_with('.') {
            continue;
        }
        let md = e.metadata().ok();
        let lmd = fs::symlink_metadata(e.path()).ok();
        out.push(Entry {
            path: e.path(),
            name,
            is_dir: md.as_ref().map(|m| m.is_dir()).unwrap_or(false),
            is_symlink: lmd.as_ref().map(|m| m.file_type().is_symlink()).unwrap_or(false),
            size: md.as_ref().map(|m| m.len()).unwrap_or(0),
            mtime: md
                .as_ref()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0),
        });
    }
    out.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Ok(out)
}

pub fn copy_into(src: &Path, dst_dir: &Path) -> Result<()> {
    let name = src.file_name().context("source has no name")?;
    let dst = unique(dst_dir.join(name));
    if src.is_dir() {
        copy_dir_recursive(src, &dst)
    } else {
        fs::copy(src, &dst).map(|_| ()).with_context(|| format!("copy {}", src.display()))
    }
}

pub fn move_into(src: &Path, dst_dir: &Path) -> Result<()> {
    let name = src.file_name().context("source has no name")?;
    let dst = unique(dst_dir.join(name));
    if fs::rename(src, &dst).is_ok() {
        return Ok(());
    }
    // cross-device: copy then remove
    if src.is_dir() {
        copy_dir_recursive(src, &dst)?;
        fs::remove_dir_all(src)?;
    } else {
        fs::copy(src, &dst)?;
        fs::remove_file(src)?;
    }
    Ok(())
}

pub fn rename(src: &Path, new_name: &str) -> Result<PathBuf> {
    if new_name.is_empty() || new_name.contains('/') {
        bail!("invalid name");
    }
    let dst = src.with_file_name(new_name);
    if dst.exists() {
        bail!("'{new_name}' already exists");
    }
    fs::rename(src, &dst)?;
    Ok(dst)
}

pub fn trash(path: &Path) -> Result<()> {
    let home = std::env::var("HOME").context("HOME not set")?;
    let tdir = PathBuf::from(home).join(".local/share/mavind/trash");
    fs::create_dir_all(&tdir)?;
    move_into(path, &tdir)
}

pub fn delete_permanent(path: &Path) -> Result<()> {
    if path.is_dir() {
        fs::remove_dir_all(path)?;
    } else {
        fs::remove_file(path)?;
    }
    Ok(())
}

pub fn new_folder(parent: &Path, name: &str) -> Result<PathBuf> {
    if name.is_empty() || name.contains('/') {
        bail!("invalid folder name");
    }
    let p = parent.join(name);
    fs::create_dir(&p).with_context(|| format!("create {}", p.display()))?;
    Ok(p)
}

pub fn dir_size(path: &Path, cap_entries: usize) -> (u64, bool) {
    let mut total = 0u64;
    let mut n = 0usize;
    let mut stack = vec![path.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            n += 1;
            if n > cap_entries {
                return (total, true);
            }
            match e.metadata() {
                Ok(m) if m.is_dir() => stack.push(e.path()),
                Ok(m) => total += m.len(),
                Err(_) => {}
            }
        }
    }
    (total, false)
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for e in fs::read_dir(src)? {
        let e = e?;
        let to = dst.join(e.file_name());
        if e.file_type()?.is_dir() {
            copy_dir_recursive(&e.path(), &to)?;
        } else {
            fs::copy(e.path(), &to)?;
        }
    }
    Ok(())
}

fn unique(mut p: PathBuf) -> PathBuf {
    if !p.exists() {
        return p;
    }
    let stem = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = p.extension().map(|s| s.to_string_lossy().into_owned());
    let parent = p.parent().map(Path::to_path_buf).unwrap_or_default();
    for i in 1..10_000 {
        let name = match &ext {
            Some(e) => format!("{stem} (copy {i}).{e}"),
            None => format!("{stem} (copy {i})"),
        };
        let cand = parent.join(name);
        if !cand.exists() {
            p = cand;
            break;
        }
    }
    p
}

pub fn fmt_size(b: u64) -> String {
    const U: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut v = b as f64;
    let mut i = 0;
    while v >= 1024.0 && i < 4 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{b} B")
    } else {
        format!("{v:.1} {}", U[i])
    }
}
