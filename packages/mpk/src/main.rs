//! mpk — Mavind package tool.
//!
//! Mavind does not ship a bespoke package manager. `mpk` is a small, opinionated
//! front-end over Debian's `apt`/`dpkg` that (a) always installs without
//! recommends, (b) knows the core/compat/optional tiers, and (c) can report the
//! real on-disk size per tier. Anything `mpk` can't do, `apt` still can.

use anyhow::{bail, Context, Result};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, ExitCode};

const TIER_DIR: &str = "/usr/lib/mavind/packages"; // core.list / compat.list / optional.list

const USAGE: &str = "\
mpk — Mavind package tool

  mpk install <pkg>...        install packages (no recommends)
  mpk remove  <pkg>...        purge packages + autoremove orphans
  mpk search  <query>         search available packages
  mpk info    <pkg>           show package details
  mpk list    [--tier]        list installed packages
  mpk update                  refresh package indexes
  mpk upgrade                 apply security/bugfix updates
  mpk install-tier <compat|optional>
                              install every package from a Mavind tier list
  mpk size                    real disk usage, broken down by area
  mpk help
";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("mpk: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (cmd, rest) = args.split_first().map(|(c, r)| (c.as_str(), r)).unwrap_or(("help", &[][..]));

    match cmd {
        "help" | "-h" | "--help" => {
            print!("{USAGE}");
            Ok(())
        }
        "search" => passthrough_nonroot("apt-cache", &prepend("search", rest)),
        "info" => passthrough_nonroot("apt-cache", &prepend("show", rest)),
        "list" => list(rest),
        "size" => size(),

        // mutating -> need root
        "install" => {
            ensure_root()?;
            need_args(rest, "install")?;
            apt(&apt_args(&["install", "--no-install-recommends", "-y"], rest))
        }
        "remove" => {
            ensure_root()?;
            need_args(rest, "remove")?;
            apt(&apt_args(&["purge", "-y"], rest))?;
            apt(&["autoremove", "--purge", "-y"])
        }
        "update" => {
            ensure_root()?;
            apt(&["update"])
        }
        "upgrade" => {
            ensure_root()?;
            apt(&["-y", "upgrade"])
        }
        "install-tier" => {
            ensure_root()?;
            let tier = rest.first().map(String::as_str).unwrap_or("");
            install_tier(tier)
        }
        other => bail!("unknown command '{other}'\n\n{USAGE}"),
    }
}

// ---------------------------------------------------------------------------
fn apt(args: &[&str]) -> Result<()> {
    let status = Command::new("apt-get")
        .args(args)
        .env("DEBIAN_FRONTEND", "noninteractive")
        .status()
        .context("failed to run apt-get")?;
    if !status.success() {
        bail!("apt-get {:?} exited with {status}", args.first().unwrap_or(&""));
    }
    Ok(())
}

fn passthrough_nonroot(bin: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(bin).args(args).status().with_context(|| format!("run {bin}"))?;
    if !status.success() {
        bail!("{bin} exited with {status}");
    }
    Ok(())
}

fn list(rest: &[String]) -> Result<()> {
    let want_tier = rest.iter().any(|a| a == "--tier");
    let out = Command::new("dpkg-query")
        .args(["-W", "-f=${Package}\t${Installed-Size}\n"])
        .output()
        .context("dpkg-query")?;
    if !out.status.success() {
        bail!("dpkg-query failed");
    }
    let installed = String::from_utf8_lossy(&out.stdout);

    let tier_of = |pkg: &str| -> &'static str {
        for (name, file) in [("core", "core.list"), ("compat", "compat.list"), ("optional", "optional.list")] {
            if let Ok(s) = std::fs::read_to_string(Path::new(TIER_DIR).join(file)) {
                if s.lines().any(|l| {
                    let l = l.split('#').next().unwrap_or("").trim();
                    l == pkg
                }) {
                    return name;
                }
            }
        }
        "extra"
    };

    let mut rows: Vec<(&str, i64)> = installed
        .lines()
        .filter_map(|l| {
            let (p, s) = l.split_once('\t')?;
            Some((p, s.trim().parse::<i64>().unwrap_or(0)))
        })
        .collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1));

    for (pkg, kb) in rows {
        if want_tier {
            println!("{:<8} {:>8} KiB  {}", tier_of(pkg), kb, pkg);
        } else {
            println!("{:>8} KiB  {}", kb, pkg);
        }
    }
    Ok(())
}

fn size() -> Result<()> {
    println!("Mavind disk usage (real, from du):\n");
    let targets = [
        ("kernel + modules", "/usr/lib/modules"),
        ("firmware", "/usr/lib/firmware"),
        ("shared libraries", "/usr/lib"),
        ("binaries", "/usr/bin"),
        ("Wine (compat)", "/usr/lib/wine"),
        ("fonts", "/usr/share/fonts"),
        ("icons/themes", "/usr/share/icons"),
        ("locales", "/usr/share/locale"),
        ("/var", "/var"),
        ("/home", "/home"),
        ("whole system (/)", "/"),
    ];
    for (label, path) in targets {
        if Path::new(path).exists() {
            let out = Command::new("du").args(["-sxh", path]).output();
            if let Ok(o) = out {
                let s = String::from_utf8_lossy(&o.stdout);
                let sz = s.split_whitespace().next().unwrap_or("?");
                println!("  {sz:>8}   {label}");
            }
        }
    }
    println!("\nTargets: core ~1 GB installed, ~2 GB RAM. See Mavind Settings > Storage.");
    Ok(())
}

fn install_tier(tier: &str) -> Result<()> {
    if !matches!(tier, "compat" | "optional") {
        bail!("install-tier: tier must be 'compat' or 'optional'");
    }
    let file = Path::new(TIER_DIR).join(format!("{tier}.list"));
    let body = std::fs::read_to_string(&file).with_context(|| format!("read {}", file.display()))?;
    let pkgs: Vec<String> = body
        .lines()
        .map(|l| l.split('#').next().unwrap_or("").trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    if pkgs.is_empty() {
        bail!("tier '{tier}' list is empty ({}). For 'optional', uncomment what you want first.", file.display());
    }
    if tier == "compat" {
        // Wine needs i386
        let _ = Command::new("dpkg").args(["--add-architecture", "i386"]).status();
        apt(&["update"])?;
    }
    println!("Installing {} packages from the {tier} tier…", pkgs.len());
    apt(&apt_args(&["install", "--no-install-recommends", "-y"], &pkgs))
}

// ---------------------------------------------------------------------------
fn ensure_root() -> Result<()> {
    if is_root() {
        return Ok(());
    }
    // Re-exec the whole command under sudo.
    let exe = std::env::current_exe().unwrap_or_else(|_| "mpk".into());
    let args: Vec<String> = std::env::args().skip(1).collect();
    eprintln!("mpk: this needs root — re-running under sudo");
    let err = Command::new("sudo").arg(exe).args(&args).exec(); // replaces process on success
    bail!("failed to elevate with sudo: {err}");
}

fn is_root() -> bool {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("Uid:"))
                .and_then(|l| l.split_whitespace().nth(1))
                .map(|u| u == "0")
        })
        .unwrap_or(false)
}

fn need_args(rest: &[String], what: &str) -> Result<()> {
    if rest.is_empty() {
        bail!("{what}: need at least one package name");
    }
    Ok(())
}
fn prepend<'a>(first: &'a str, rest: &'a [String]) -> Vec<&'a str> {
    std::iter::once(first).chain(rest.iter().map(String::as_str)).collect()
}
/// Build an argv for apt-get: fixed leading flags + the user's package names.
fn apt_args<'a>(lead: &[&'a str], pkgs: &'a [String]) -> Vec<&'a str> {
    lead.iter().copied().chain(pkgs.iter().map(String::as_str)).collect()
}
