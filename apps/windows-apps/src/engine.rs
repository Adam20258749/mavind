//! The Windows-apps engine. All state lives under
//! `~/.local/share/mavind/`. This module shells out to `wine`, `wineboot`,
//! `winetricks` and coreutils; it has no Wine-specific dependencies so it
//! builds and unit-tests without Wine installed.

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

pub const HINTS_PATH: &str = "/usr/lib/mavind/wine/hints.tsv";
pub const DXVK_INSTALLER: &str = "/usr/lib/mavind/wine/install-dxvk.sh";
pub const VKD3D_INSTALLER: &str = "/usr/lib/mavind/wine/install-vkd3d.sh";

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Arch {
    Win64,
    Win32,
}
impl Arch {
    pub fn as_wine(&self) -> &'static str {
        match self {
            Arch::Win64 => "win64",
            Arch::Win32 => "win32",
        }
    }
    pub fn parse(s: &str) -> Result<Arch> {
        match s {
            "win64" | "64" => Ok(Arch::Win64),
            "win32" | "32" => Ok(Arch::Win32),
            _ => bail!("arch must be win64 or win32"),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AppEntry {
    pub id: String,
    pub name: String,
    /// Windows path to the executable *inside* the prefix, e.g.
    /// `C:\\Program Files\\Foo\\foo.exe`, or a unix path for portable exes.
    pub exe: String,
    pub prefix: String, // prefix name (dir under prefixes/)
    pub arch: String,   // win64 | win32
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub dxvk: Option<String>,
    #[serde(default)]
    pub added: u64,
    #[serde(default)]
    pub last_run: u64,
    #[serde(default)]
    pub isolated: bool,
}

#[derive(Serialize, Deserialize, Default)]
pub struct Registry {
    pub apps: BTreeMap<String, AppEntry>,
}

pub struct Engine {
    pub root: PathBuf, // ~/.local/share/mavind
}

impl Engine {
    pub fn new() -> Result<Engine> {
        let data = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
            .ok_or_else(|| anyhow!("neither XDG_DATA_HOME nor HOME is set"))?;
        let root = data.join("mavind");
        for sub in ["prefixes", "logs"] {
            fs::create_dir_all(root.join(sub))?;
        }
        Ok(Engine { root })
    }

    fn registry_path(&self) -> PathBuf {
        self.root.join("windows-apps/apps.json")
    }

    pub fn load(&self) -> Result<Registry> {
        let p = self.registry_path();
        if !p.exists() {
            return Ok(Registry::default());
        }
        let s = fs::read_to_string(&p).with_context(|| format!("read {}", p.display()))?;
        Ok(serde_json::from_str(&s).unwrap_or_default())
    }

    pub fn save(&self, reg: &Registry) -> Result<()> {
        let p = self.registry_path();
        fs::create_dir_all(p.parent().unwrap())?;
        let tmp = p.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_vec_pretty(reg)?)?;
        fs::rename(&tmp, &p)?;
        Ok(())
    }

    pub fn prefix_dir(&self, name: &str) -> PathBuf {
        self.root.join("prefixes").join(name)
    }

    pub fn wine_available() -> bool {
        which("wine").is_some()
    }

    // -- prefixes ---------------------------------------------------------
    pub fn prefix_create(&self, name: &str, arch: Arch) -> Result<PathBuf> {
        if !Self::wine_available() {
            bail!("Wine is not installed. Install the compatibility tier:  mpk install-tier compat");
        }
        let dir = self.prefix_dir(name);
        if dir.join("system.reg").exists() {
            return Ok(dir);
        }
        fs::create_dir_all(&dir)?;
        eprintln!("Creating Wine prefix '{name}' ({}) — first run downloads Mono/Gecko…", arch.as_wine());
        let status = Command::new("wineboot")
            .arg("--init")
            .env("WINEPREFIX", &dir)
            .env("WINEARCH", arch.as_wine())
            .env("WINEDLLOVERRIDES", "mscoree,mshtml=") // skip nag dialogs in headless
            .stdin(Stdio::null())
            .status()
            .context("failed to launch wineboot")?;
        if !status.success() {
            bail!("wineboot exited with {status}");
        }
        // Apply the Mavind baseline (fonts, no crash dialog, etc.)
        let baseline = fs::read_to_string("/usr/lib/mavind/wine/prefix-baseline.txt").unwrap_or_default();
        let verbs: Vec<&str> = baseline
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .collect();
        if !verbs.is_empty() && which("winetricks").is_some() {
            let _ = Command::new("winetricks")
                .arg("-q")
                .args(&verbs)
                .env("WINEPREFIX", &dir)
                .status();
        }
        Ok(dir)
    }

    pub fn prefix_list(&self) -> Vec<String> {
        fs::read_dir(self.root.join("prefixes"))
            .map(|rd| {
                rd.flatten()
                    .filter(|e| e.path().join("system.reg").exists() || e.path().is_dir())
                    .filter_map(|e| e.file_name().into_string().ok())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn prefix_remove(&self, name: &str) -> Result<()> {
        let dir = self.prefix_dir(name);
        if !dir.starts_with(self.root.join("prefixes")) || name.is_empty() || name.contains('/') {
            bail!("refusing to remove suspicious path");
        }
        fs::remove_dir_all(&dir).with_context(|| format!("remove {}", dir.display()))?;
        // drop apps that referenced it
        let mut reg = self.load()?;
        reg.apps.retain(|_, a| a.prefix != name);
        self.save(&reg)?;
        Ok(())
    }

    /// Install an .exe or .msi. Returns the new app id.
    pub fn install(
        &self,
        file: &Path,
        prefix: Option<&str>,
        isolated: bool,
        name: Option<&str>,
    ) -> Result<String> {
        let file = file
            .canonicalize()
            .with_context(|| format!("no such file: {}", file.display()))?;
        let stem = file
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("app")
            .to_string();
        let display = name.map(str::to_string).unwrap_or_else(|| titlecase(&stem));
        let id = slugify(&display);

        let prefix_name = if isolated {
            id.clone()
        } else {
            prefix.unwrap_or("default").to_string()
        };
        let arch = Arch::Win64;
        let pdir = self.prefix_create(&prefix_name, arch)?;

        let ext = file
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        eprintln!("Installing {} into prefix '{prefix_name}'…", file.display());
        let status = match ext.as_str() {
            "msi" => Command::new("wine")
                .args(["msiexec", "/i"])
                .arg(&file)
                .env("WINEPREFIX", &pdir)
                .status()?,
            "exe" => Command::new("wine")
                .arg(&file)
                .env("WINEPREFIX", &pdir)
                .status()?,
            other => bail!("unsupported file type: .{other} (expected .exe or .msi)"),
        };
        if !status.success() {
            eprintln!("note: installer exited with {status} — it may still have installed");
        }

        let mut reg = self.load()?;
        let entry = AppEntry {
            id: id.clone(),
            name: display,
            exe: file.to_string_lossy().into_owned(),
            prefix: prefix_name,
            arch: arch.as_wine().into(),
            args: vec![],
            dxvk: None,
            added: now(),
            last_run: 0,
            isolated,
        };
        reg.apps.insert(id.clone(), entry);
        self.save(&reg)?;
        Ok(id)
    }

    pub fn resolve<'r>(&self, reg: &'r Registry, id_or_name: &str) -> Result<&'r AppEntry> {
        if let Some(a) = reg.apps.get(id_or_name) {
            return Ok(a);
        }
        let want = id_or_name.to_lowercase();
        reg.apps
            .values()
            .find(|a| a.name.to_lowercase() == want || a.name.to_lowercase().contains(&want))
            .ok_or_else(|| anyhow!("no installed Windows app matches '{id_or_name}'"))
    }

    pub fn run(&self, id_or_name: &str) -> Result<()> {
        let mut reg = self.load()?;
        let app = self.resolve(&reg, id_or_name)?.clone();
        let pdir = self.prefix_dir(&app.prefix);
        if !pdir.exists() {
            bail!("prefix '{}' is missing — try:  mavind-wine repair {}", app.prefix, app.id);
        }
        let log = self.root.join("logs").join(format!("{}.log", app.id));
        let logf = fs::File::create(&log).ok();
        eprintln!("Launching {} (log: {})", app.name, log.display());

        let mut cmd = Command::new("wine");
        cmd.arg(&app.exe)
            .args(&app.args)
            .env("WINEPREFIX", &pdir)
            .env("WINEARCH", &app.arch)
            .stdin(Stdio::null());
        if let Some(f) = logf {
            let f2 = f.try_clone().ok();
            cmd.stdout(Stdio::from(f));
            if let Some(f2) = f2 {
                cmd.stderr(Stdio::from(f2));
            }
        }
        cmd.spawn().context("failed to start wine")?;

        if let Some(a) = reg.apps.get_mut(&app.id) {
            a.last_run = now();
        }
        self.save(&reg)?;
        Ok(())
    }

    pub fn make_shortcut(&self, id_or_name: &str, menu: bool, desktop: bool) -> Result<PathBuf> {
        let reg = self.load()?;
        let app = self.resolve(&reg, id_or_name)?;
        let content = format!(
            "[Desktop Entry]\n\
             Type=Application\n\
             Name={name}\n\
             Comment=Windows application (Wine, prefix: {prefix})\n\
             Exec=mavind-wine run {id}\n\
             Icon=wine\n\
             Terminal=false\n\
             Categories=Wine;X-Windows-Apps;\n\
             X-Mavind-Prefix={prefix}\n",
            name = app.name,
            id = app.id,
            prefix = app.prefix
        );
        let apps_dir = dirs_data().join("applications");
        fs::create_dir_all(&apps_dir)?;
        let path = apps_dir.join(format!("mavind-app-{}.desktop", app.id));
        if menu || !desktop {
            fs::write(&path, &content)?;
        }
        if desktop {
            if let Some(d) = dirs_desktop() {
                fs::create_dir_all(&d)?;
                let dp = d.join(format!("{}.desktop", app.name));
                fs::write(&dp, &content)?;
                let _ = Command::new("chmod").arg("+x").arg(&dp).status();
            }
        }
        Ok(path)
    }

    pub fn configure(&self, id_or_name: &str) -> Result<()> {
        let reg = self.load()?;
        let app = self.resolve(&reg, id_or_name)?;
        Command::new("winecfg")
            .env("WINEPREFIX", self.prefix_dir(&app.prefix))
            .spawn()
            .context("winecfg not found (install the compatibility tier)")?;
        Ok(())
    }

    pub fn repair(&self, id_or_name: &str) -> Result<()> {
        let reg = self.load()?;
        let app = self.resolve(&reg, id_or_name)?;
        let pdir = self.prefix_dir(&app.prefix);
        fs::create_dir_all(&pdir)?;
        eprintln!("Repairing prefix '{}' …", app.prefix);
        let st = Command::new("wineboot")
            .arg("-u")
            .env("WINEPREFIX", &pdir)
            .status()?;
        if !st.success() {
            bail!("wineboot -u failed ({st})");
        }
        Ok(())
    }

    pub fn uninstall(&self, id_or_name: &str, purge_prefix: bool) -> Result<()> {
        let mut reg = self.load()?;
        let app = self.resolve(&reg, id_or_name)?.clone();
        // best-effort Windows-side uninstall
        let _ = Command::new("wine")
            .args(["uninstaller", "--list"])
            .env("WINEPREFIX", self.prefix_dir(&app.prefix))
            .status();
        for p in [
            dirs_data().join("applications").join(format!("mavind-app-{}.desktop", app.id)),
        ] {
            let _ = fs::remove_file(p);
        }
        if purge_prefix && app.isolated {
            let _ = self.prefix_remove(&app.prefix);
        }
        reg.apps.remove(&app.id);
        self.save(&reg)?;
        Ok(())
    }

    pub fn dxvk(&self, id_or_name: &str, uninstall: bool) -> Result<()> {
        self.overlay_installer(id_or_name, DXVK_INSTALLER, "dxvk", uninstall)
    }
    pub fn vkd3d(&self, id_or_name: &str, uninstall: bool) -> Result<()> {
        self.overlay_installer(id_or_name, VKD3D_INSTALLER, "vkd3d", uninstall)
    }

    fn overlay_installer(&self, id_or_name: &str, script: &str, tag: &str, uninstall: bool) -> Result<()> {
        let mut reg = self.load()?;
        let app = self.resolve(&reg, id_or_name)?.clone();
        if !Path::new(script).exists() {
            bail!("{script} not present (compatibility tier not fully installed)");
        }
        let mut c = Command::new("sh");
        c.arg(script).arg(self.prefix_dir(&app.prefix));
        if uninstall {
            c.arg("--uninstall");
        }
        let st = c.status().with_context(|| format!("run {script}"))?;
        if !st.success() {
            bail!("{tag} installer failed ({st})");
        }
        if let Some(a) = reg.apps.get_mut(&app.id) {
            a.dxvk = if uninstall { None } else { Some(read_version_file(tag)) };
        }
        self.save(&reg)?;
        Ok(())
    }
}

// -- safety + hints -----------------------------------------------------
pub struct FileFacts {
    pub path: PathBuf,
    pub size: u64,
    pub sha256: String,
    pub is_pe: bool,
}

pub fn inspect_file(p: &Path) -> Result<FileFacts> {
    let meta = fs::metadata(p).with_context(|| format!("stat {}", p.display()))?;
    let sha = Command::new("sha256sum")
        .arg(p)
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.split_whitespace().next().map(str::to_string))
        .unwrap_or_else(|| "unknown".into());
    let ftype = Command::new("file")
        .args(["-b", "--mime-type"])
        .arg(p)
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default();
    let is_pe = ftype.contains("dosexec")
        || ftype.contains("x-msi")
        || p.extension().map(|e| e.eq_ignore_ascii_case("exe") || e.eq_ignore_ascii_case("msi")).unwrap_or(false);
    Ok(FileFacts {
        path: p.to_path_buf(),
        size: meta.len(),
        sha256: sha,
        is_pe,
    })
}

/// Look up a local, static compatibility hint. No network.
pub fn hint(query: &str) -> Option<(String, String, String)> {
    let s = fs::read_to_string(HINTS_PATH).ok()?;
    let q = query.to_lowercase();
    for line in s.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let mut it = line.splitn(3, '\t');
        let name = it.next()?.trim();
        let verdict = it.next().unwrap_or("unknown").trim();
        let note = it.next().unwrap_or("").trim();
        if q.contains(&name.to_lowercase()) || name.to_lowercase().contains(&q) {
            return Some((name.to_string(), verdict.to_string(), note.to_string()));
        }
    }
    None
}

// -- helpers ----------------------------------------------------------
pub fn which(bin: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|d| d.join(bin))
            .find(|p| p.is_file())
    })
}
fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}
fn dirs_data() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".local/share"))
}
fn dirs_desktop() -> Option<PathBuf> {
    let h = std::env::var_os("HOME")?;
    let d = PathBuf::from(&h).join("Desktop");
    Some(d)
}
fn read_version_file(tag: &str) -> String {
    fs::read_to_string(format!("/usr/lib/mavind/wine/{tag}.version"))
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "installed".into())
}
pub fn slugify(s: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    out.trim_matches('-').to_string()
}
fn titlecase(s: &str) -> String {
    s.split(|c: char| c == '-' || c == '_' || c == ' ')
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut ch = w.chars();
            match ch.next() {
                Some(f) => f.to_uppercase().collect::<String>() + ch.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn slug() {
        assert_eq!(slugify("Notepad++ 8.6"), "notepad-8-6");
        assert_eq!(slugify("  7-Zip "), "7-zip");
    }
    #[test]
    fn title() {
        assert_eq!(titlecase("cool_app-name"), "Cool App Name");
    }
}
