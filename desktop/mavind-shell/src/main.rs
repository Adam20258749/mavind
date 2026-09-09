//! Mavind desktop shell — a minimal Wayland panel (wlr-layer-shell) for labwc.
//!
//! Deliberately small. It provides:
//!   * a launcher button  -> spawns `mavind-launcher`
//!   * a live clock
//!   * a status area: volume %, network state, battery % (all read from the
//!     real system — sysfs / wpctl / nmcli — never faked)
//!   * a power menu (lock / logout / suspend / reboot / poweroff)
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
    load_css();

    let display = gtk4::gdk::Display::default().expect("no display");
    let monitors = display.monitors();

    // One panel per monitor. If enumeration yields nothing yet, still show one.
    let n = monitors.n_items();
    if n == 0 {
        app.add_window(&make_panel(app, None));
    } else {
        for i in 0..n {
            let monitor = monitors
                .item(i)
                .and_then(|o| o.downcast::<gtk4::gdk::Monitor>().ok());
            app.add_window(&make_panel(app, monitor));
        }
    }
}

fn make_panel(app: &Application, monitor: Option<gtk4::gdk::Monitor>) -> ApplicationWindow {
    let window = ApplicationWindow::builder()
        .application(app)
        .default_height(40)
        .build();

    window.init_layer_shell();
    window.set_layer(Layer::Top);
    window.set_namespace("mavind-shell");
    window.set_anchor(Edge::Left, true);
    window.set_anchor(Edge::Right, true);
    window.set_anchor(Edge::Bottom, true);
    window.set_anchor(Edge::Top, false);
    window.auto_exclusive_zone_enable();
    if let Some(m) = monitor {
        window.set_monitor(&m);
    }

    let root = GtkBox::new(Orientation::Horizontal, 6);
    root.add_css_class("mavind-panel");

    // ---- left: launcher -------------------------------------------------
    let launcher = Button::with_label("Mavind");
    launcher.add_css_class("launcher");
    launcher.set_tooltip_text(Some("Applications  (Super+Space)"));
    launcher.connect_clicked(|_| spawn("mavind-launcher", &[]));
    root.append(&launcher);

    let files = Button::from_icon_name("system-file-manager-symbolic");
    files.set_tooltip_text(Some("Files"));
    files.connect_clicked(|_| spawn("minder", &[]));
    root.append(&files);

    // ---- center: taskbar placeholder ---------------------------------
    let spacer_l = GtkBox::new(Orientation::Horizontal, 0);
    spacer_l.set_hexpand(true);
    root.append(&spacer_l);

    let taskbar = Label::new(None);
    taskbar.add_css_class("taskbar");
    root.append(&taskbar);

    let spacer_r = GtkBox::new(Orientation::Horizontal, 0);
    spacer_r.set_hexpand(true);
    root.append(&spacer_r);

    // ---- right: status + clock + power ------------------------------
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

    let powerbtn = Button::from_icon_name("system-shutdown-symbolic");
    powerbtn.set_tooltip_text(Some("Power"));
    powerbtn.add_css_class("power");
    {
        let win = window.clone();
        powerbtn.connect_clicked(move |btn| power::show_menu(&win, btn));
    }

    for w in [&net, &vol, &bat] {
        root.append(w);
    }
    root.append(&clock);
    root.append(&powerbtn);

    window.set_child(Some(&root));
    window.present();

    // ---- periodic refresh -----------------------------------------
    let refresh = {
        let clock = clock.clone();
        let net = net.clone();
        let vol = vol.clone();
        let bat = bat.clone();
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
            glib::ControlFlow::Continue
        }
    };
    refresh.clone()();
    glib::timeout_add_seconds_local(TICK_SECONDS, refresh);

    window
}

fn load_css() {
    let provider = gtk4::CssProvider::new();
    provider.load_from_string(include_str!("style.css"));
    gtk4::style_context_add_provider_for_display(
        &gtk4::gdk::Display::default().expect("no display"),
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
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
