//! Entry point for both `mavind-windows-apps` (GUI) and `mavind-wine` (CLI).
//!
//! Dispatch:
//!   * argv[0] basename `mavind-wine`   -> CLI
//!   * first arg `--cli`                -> CLI (rest of args)
//!   * first arg is a known subcommand  -> CLI
//!   * first arg is an existing .exe/.msi path -> GUI, offer to install it
//!   * otherwise                        -> GUI

mod cli;
mod engine;
mod gui;

use std::path::PathBuf;

fn main() -> std::process::ExitCode {
    let argv: Vec<String> = std::env::args().collect();
    let prog = argv
        .first()
        .map(|p| PathBuf::from(p))
        .and_then(|p| p.file_name().map(|s| s.to_string_lossy().into_owned()))
        .unwrap_or_default();
    let args = &argv[1..];

    // ---- CLI routes ---------------------------------------------------
    if prog == "mavind-wine" {
        return finish(cli::main(args));
    }
    if let Some(first) = args.first() {
        if first == "--cli" {
            return finish(cli::main(&args[1..]));
        }
        if cli::is_subcommand(first) {
            return finish(cli::main(args));
        }
    }

    // ---- GUI routes -------------------------------------------------
    let install_file = args
        .iter()
        .map(PathBuf::from)
        .find(|p| {
            p.is_file()
                && p.extension()
                    .map(|e| e.eq_ignore_ascii_case("exe") || e.eq_ignore_ascii_case("msi"))
                    .unwrap_or(false)
        });

    let code = gui::run(install_file);
    if code == gtk4::glib::ExitCode::SUCCESS {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::FAILURE
    }
}

fn finish(r: anyhow::Result<()>) -> std::process::ExitCode {
    match r {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("mavind-wine: {e:#}");
            std::process::ExitCode::FAILURE
        }
    }
}
