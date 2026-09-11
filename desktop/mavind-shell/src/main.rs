//! Mavind desktop shell — two minimal Wayland panels (wlr-layer-shell) for
//! labwc, styled after a macOS-style layout: a thin top menu bar and a
//! floating bottom dock.
//!
//! Deliberately small. It provides:
//!   * top bar: a system menu (lock/logout/power) + status + a live clock
//!   * bottom dock: pinned app launchers + the Start Menu trigger
//!   * status area: volume %, network state, battery % (all read from the
//!     real system — sysfs / wpctl / nmcli — never faked)
//!
//! Taskbar (wlr-foreign-toplevel-management) is the next increment — see ROADMAP.

mod power;
mod status;

use gtk4::prelude::*;
use gtk4::{glib, Application, ApplicationWindow, Box as GtkBox, Button, Label, Orientation};
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use std::process::Command;

const APP_ID: &str = "os.mavind.Shell";
const TICK_SECONDS: u32 = 1;

// (icon name, tooltip, command, args)
const DOCK_APPS: &[(&str, &str, &str, &[&str])] = &[
    ("view-app-grid-symbolic", "Launchpad  (Super+Space)", "mavind-launcher", &[]),
    ("system-file-manager-symbolic", "Files", "minder", &[]),
    ("web-browser-symbolic", "Mrowser", "mrowser", &[]),
    ("utilities-terminal-symbolic", "Terminal", "foot", &[]),
    ("system-run-symbolic", "Windows Apps", "mavind-windows-apps", &[]),
    ("utilities-system-monitor-symbolic", "System Monitor", "mavind-system-monitor", &[]),
    ("preferences-system-symbolic", "Settings", "mavind-settings", &[]),
];

fn main() -> glib::ExitCode {
    // Handle CLI-only modes before starting GTK.
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("--version") | Some("-v") => {
            println!("mavind-shell {}", env!("CARGO_PKG_VERSION"));
            return glib::ExitCode::SUCCESS;
        }
        Some("--lock") => {
            lock_session();
            return glib::ExitCode::SUCCESS;
        }
        Some("--help") | Some("-h") => {
            println!("usage: mavind-shell [--lock] [--version]");
            return glib::ExitCode::SUCCESS;
        }
        _ => {}
    }

    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_panels);
    // Don't let GTK parse the leftover argv (we handled our own flags above).
    app.run_with_args(&[] as &[&str])
}

fn build_panels(app: &Application) {
    mavind_theme::load_app_css(include_str!("style.css"));

    let display = gtk4::gdk::Display::default().expect("no display");
    let monitors = display.monitors();

    // One top bar + one dock per monitor. If enumeration yields nothing yet,
    // still show one of each.
    let n = monitors.n_items();
    if n == 0 {
        app.add_window(&make_topbar(app, None));
        app.add_window(&make_dock(app, None));
    } else {
        for i in 0..n {
            let monitor = monitors.item(i).and_then(|o| o.downcast::<gtk4::gdk::Monitor>().ok());
            app.add_window(&make_topbar(app, monitor.clone()));
            app.add_window(&make_dock(app, monitor));
        }
    }
}

fn make_topbar(app: &Application, monitor: Option<gtk4::gdk::Monitor>) -> ApplicationWindow {
    let window = ApplicationWindow::builder().application(app).default_height(28).build();

    window.init_layer_shell();
    window.set_layer(Layer::Top);
    window.set_namespace("mavind-topbar");
    window.set_anchor(Edge::Left, true);
    window.set_anchor(Edge::Right, true);
    window.set_anchor(Edge::Top, true);
    window.auto_exclusive_zone_enable();
    if let Some(m) = &monitor {
        window.set_monitor(m);
    }

    let root = GtkBox::new(Orientation::Horizontal, 4);
    root.add_css_class("mavind-topbar");

    // ---- left: system menu ------------------------------------------
    let sysmenu = Button::with_label("Mavind");
    sysmenu.add_css_class("sysmenu");
    sysmenu.set_has_frame(false);
    sysmenu.set_tooltip_text(Some("System menu"));
    {
        let win = window.clone();
        sysmenu.connect_clicked(move |btn| power::show_menu(&win, btn));
    }
    root.append(&sysmenu);

    let spacer = GtkBox::new(Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    root.append(&spacer);

    // ---- right: status + clock ---------------------------------------
    let net = Label::new(Some(""));
    net.add_css_class("status");
    let vol = Label::new(Some(""));
    vol.add_css_class("status");
    let bat = Label::new(Some(""));
    bat.add_css_class("status");

    let clock = Button::with_label("");
    clock.add_css_class("clock");
    clock.set_has_frame(false);
    clock.connect_clicked(|_| spawn("mavind-settings", &["--panel", "about"]));

    for w in [&net, &vol, &bat] {
        root.append(w);
    }
    root.append(&clock);

    window.set_child(Some(&root));
    window.present();

    // ---- periodic refresh -----------------------------------------
    let refresh = {
        let clock = clock.clone();
        let net = net.clone();
        let vol = vol.clone();
        let bat = bat.clone();
        let last_theme_change = std::cell::Cell::new(mavind_theme::theme_css_mtime());
        move || {
            clock.set_label(&status::clock_text());
            net.set_label(&status::network_text());
            vol.set_label(&status::volume_text());
            match status::battery_text() {
                Some(t) => {
                    bat.set_label(&t);
                    bat.set_visible(true);
                }
                None => bat.set_visible(false),
            }
            // Pick up live Appearance changes (Settings -> Appearance) without
            // a dedicated file-watcher — this timer already runs every second.
            let now = mavind_theme::theme_css_mtime();
            if now != last_theme_change.get() {
                last_theme_change.set(now);
                mavind_theme::reload_theme_css();
            }
            glib::ControlFlow::Continue
        }
    };
    refresh.clone()();
    glib::timeout_add_seconds_local(TICK_SECONDS, refresh);

    window
}

fn make_dock(app: &Application, monitor: Option<gtk4::gdk::Monitor>) -> ApplicationWindow {
    let window = ApplicationWindow::builder().application(app).build();

    window.init_layer_shell();
    window.set_layer(Layer::Top);
    window.set_namespace("mavind-dock");
    // Bottom edge only: wlr-layer-shell centers the surface on the other
    // axis, so this floats bottom-centered rather than spanning the screen.
    window.set_anchor(Edge::Bottom, true);
    window.set_margin(Edge::Bottom, 10);
    window.auto_exclusive_zone_enable();
    if let Some(m) = &monitor {
        window.set_monitor(m);
    }

    let root = GtkBox::new(Orientation::Horizontal, 6);
    root.add_css_class("mavind-dock");

    for (icon, tooltip, cmd, extra_args) in DOCK_APPS {
        let b = Button::from_icon_name(icon);
        b.add_css_class("dock-icon");
        b.set_has_frame(false);
        b.set_tooltip_text(Some(tooltip));
        let cmd = cmd.to_string();
        let extra_args: Vec<String> = extra_args.iter().map(|s| s.to_string()).collect();
        b.connect_clicked(move |_| {
            let args: Vec<&str> = extra_args.iter().map(String::as_str).collect();
            spawn(&cmd, &args);
        });
        root.append(&b);
    }

    window.set_child(Some(&root));
    window.present();
    window
}

/// Spawn a detached child; never blocks the panel, never panics on failure.
fn spawn(cmd: &str, args: &[&str]) {
    if let Err(e) = Command::new(cmd).args(args).spawn() {
        eprintln!("mavind-shell: failed to spawn {cmd}: {e}");
    }
}

/// Lock the session. Prefer swaylock; degrade gracefully if it's absent.
fn lock_session() {
    let ok = Command::new("swaylock")
        .args(["-f", "-c", "1c1c22"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !ok {
        eprintln!("mavind-shell: swaylock unavailable; session not locked");
    }
}
