//! Text interface for `mavind-wine` / `mavind-windows-apps --cli`.

use crate::engine::*;
use anyhow::{bail, Result};
use std::io::{self, Write};
use std::path::Path;

pub const USAGE: &str = "\
mavind-wine — manage Windows applications on Mavind

  mavind-wine list
  mavind-wine install <file.exe|file.msi> [--prefix NAME] [--isolated] [--name \"App\"]
  mavind-wine run <app>
  mavind-wine shortcut <app> [--menu] [--desktop]
  mavind-wine configure <app>
  mavind-wine repair <app>
  mavind-wine uninstall <app> [--purge-prefix]
  mavind-wine dxvk install|remove <app>
  mavind-wine vkd3d install|remove <app>
  mavind-wine prefix create <name> [--arch win64|win32]
  mavind-wine prefix list
  mavind-wine prefix rm <name>
  mavind-wine hint <name-or-file>

<app> is an id (from `list`) or a name substring.
";

pub fn is_subcommand(s: &str) -> bool {
    matches!(
        s,
        "list" | "install" | "run" | "shortcut" | "configure" | "repair" | "uninstall"
            | "dxvk" | "vkd3d" | "prefix" | "hint" | "help" | "--help" | "-h"
    )
}

pub fn main(args: &[String]) -> Result<()> {
    let eng = Engine::new()?;
    let cmd = args.first().map(String::as_str).unwrap_or("help");
    let rest = &args[1..];

    match cmd {
        "help" | "--help" | "-h" => {
            print!("{USAGE}");
            Ok(())
        }
        "list" => cmd_list(&eng),
        "install" => cmd_install(&eng, rest),
        "run" => {
            let app = rest.first().ok_or_else(|| anyhow::anyhow!("run: need <app>"))?;
            eng.run(app)
        }
        "shortcut" => {
            let app = rest.first().ok_or_else(|| anyhow::anyhow!("shortcut: need <app>"))?;
            let menu = rest.iter().any(|a| a == "--menu") || !rest.iter().any(|a| a == "--desktop");
            let desktop = rest.iter().any(|a| a == "--desktop");
            let p = eng.make_shortcut(app, menu, desktop)?;
            println!("wrote {}", p.display());
            Ok(())
        }
        "configure" => eng.configure(need(rest, "configure")?),
        "repair" => eng.repair(need(rest, "repair")?),
        "uninstall" => eng.uninstall(need(rest, "uninstall")?, rest.iter().any(|a| a == "--purge-prefix")),
        "dxvk" | "vkd3d" => cmd_overlay(&eng, cmd, rest),
        "prefix" => cmd_prefix(&eng, rest),
        "hint" => {
            let q = need(rest, "hint")?;
            match hint(q) {
                Some((n, v, note)) => println!("{n}: {v}\n  {note}"),
                None => println!("no local hint for '{q}' — try running it in an isolated prefix"),
            }
            Ok(())
        }
        other => {
            bail!("unknown command '{other}'\n\n{USAGE}");
        }
    }
}

fn need<'a>(rest: &'a [String], what: &str) -> Result<&'a str> {
    rest.first()
        .map(String::as_str)
        .filter(|s| !s.starts_with('-'))
        .ok_or_else(|| anyhow::anyhow!("{what}: need <app>"))
}

fn cmd_list(eng: &Engine) -> Result<()> {
    let reg = eng.load()?;
    if reg.apps.is_empty() {
        println!("No Windows apps installed yet.  Install one:\n  mavind-wine install <file.exe>");
        if !Engine::wine_available() {
            println!("\n⚠  Wine is not installed. Add the compatibility tier:\n  mpk install-tier compat");
        }
        return Ok(());
    }
    println!("{:<22} {:<24} {:<10} {:<8} {}", "ID", "NAME", "PREFIX", "DXVK", "COMPAT");
    for a in reg.apps.values() {
        let verdict = hint(&a.name).map(|(_, v, _)| v).unwrap_or_else(|| "untested".into());
        println!(
            "{:<22} {:<24} {:<10} {:<8} {}",
            a.id,
            trunc(&a.name, 24),
            trunc(&a.prefix, 10),
            a.dxvk.as_deref().unwrap_or("-"),
            verdict,
        );
    }
    Ok(())
}

fn cmd_install(eng: &Engine, rest: &[String]) -> Result<()> {
    let mut file: Option<&String> = None;
    let mut prefix: Option<&str> = None;
    let mut name: Option<&str> = None;
    let mut isolated = false;
    let mut it = rest.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--prefix" => prefix = it.next().map(String::as_str),
            "--name" => name = it.next().map(String::as_str),
            "--isolated" => isolated = true,
            s if !s.starts_with('-') => file = Some(a),
            s => bail!("install: unknown option {s}"),
        }
    }
    let file = file.ok_or_else(|| anyhow::anyhow!("install: need a .exe or .msi path"))?;
    let path = Path::new(file);

    // Safety gate (spec §16): warn before the first run of an unverified file.
    let facts = inspect_file(path)?;
    if !facts.is_pe {
        eprintln!("warning: {} does not look like a Windows program", path.display());
    }
    let known = hint(path.file_stem().and_then(|s| s.to_str()).unwrap_or(""));
    eprintln!(
        "\n\u{26a0}  About to run an unverified Windows program:\n  \
         file   {}\n  size   {}\n  sha256 {}\n  compat {}\n",
        facts.path.display(),
        human(facts.size),
        facts.sha256,
        known.as_ref().map(|(_, v, n)| format!("{v} — {n}")).unwrap_or_else(|| "untested on Mavind".into()),
    );
    if isolated {
        eprintln!("It will run in an ISOLATED Wine prefix (its own throwaway C: drive).");
    } else {
        eprintln!("It will run in the SHARED Wine prefix '{}'. Pass --isolated to sandbox it.", prefix.unwrap_or("default"));
    }
    if !confirm("Continue? [y/N] ")? {
        bail!("cancelled");
    }

    let id = eng.install(path, prefix, isolated, name)?;
    println!("\nInstalled as '{id}'.  Run it:  mavind-wine run {id}");
    println!("Add a launcher:  mavind-wine shortcut {id} --menu");
    Ok(())
}

fn cmd_overlay(eng: &Engine, which_one: &str, rest: &[String]) -> Result<()> {
    let action = rest.first().map(String::as_str).unwrap_or("");
    let app = rest.get(1).map(String::as_str).unwrap_or("");
    let remove = matches!(action, "remove" | "uninstall");
    if app.is_empty() {
        bail!("{which_one}: usage: {which_one} install|remove <app>");
    }
    match which_one {
        "dxvk" => eng.dxvk(app, remove),
        _ => eng.vkd3d(app, remove),
    }
}

fn cmd_prefix(eng: &Engine, rest: &[String]) -> Result<()> {
    match rest.first().map(String::as_str) {
        Some("list") => {
            for p in eng.prefix_list() {
                println!("{p}");
            }
            Ok(())
        }
        Some("create") => {
            let name = rest.get(1).ok_or_else(|| anyhow::anyhow!("prefix create: need <name>"))?;
            let arch = rest
                .iter()
                .position(|a| a == "--arch")
                .and_then(|i| rest.get(i + 1))
                .map(|s| Arch::parse(s))
                .transpose()?
                .unwrap_or(Arch::Win64);
            let d = eng.prefix_create(name, arch)?;
            println!("prefix ready: {}", d.display());
            Ok(())
        }
        Some("rm") => {
            let name = rest.get(1).ok_or_else(|| anyhow::anyhow!("prefix rm: need <name>"))?;
            eng.prefix_remove(name)?;
            println!("removed prefix '{name}'");
            Ok(())
        }
        _ => bail!("prefix: expected list | create | rm"),
    }
}

fn confirm(prompt: &str) -> Result<bool> {
    print!("{prompt}");
    io::stdout().flush()?;
    let mut s = String::new();
    io::stdin().read_line(&mut s)?;
    Ok(matches!(s.trim().to_lowercase().as_str(), "y" | "yes"))
}

fn trunc(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(n - 1).collect::<String>())
    }
}
fn human(b: u64) -> String {
    let (mut v, u) = (b as f64, ["B", "KiB", "MiB", "GiB"]);
    let mut i = 0;
    while v >= 1024.0 && i < 3 {
        v /= 1024.0;
        i += 1;
    }
    format!("{v:.1} {}", u[i])
}
