//! Mavind Settings — the essential panels only (spec §9).
//! Data is read live from the system; nothing here is mocked.

mod sys;

use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    Align, Application, ApplicationWindow, Box as GtkBox, Button, Label, Orientation, PolicyType,
    Scale, ScrolledWindow, Stack, StackSidebar, Switch, ToggleButton,
};
use std::process::Command;

const APP_ID: &str = "os.mavind.Settings";

fn main() -> glib::ExitCode {
    let argv: Vec<String> = std::env::args().collect();

    // Headless toggle used by the labwc menu / shell.
    if argv.iter().any(|a| a == "--toggle-performance") {
        let now = !sys::perf_enabled();
        match sys::set_perf_enabled(now) {
            Ok(()) => println!("Performance Mode: {}", if now { "ON" } else { "OFF" }),
            Err(e) => eprintln!("mavind-settings: {e}"),
        }
        return glib::ExitCode::SUCCESS;
    }

    let start_panel = argv
        .iter()
        .position(|a| a == "--panel")
        .and_then(|i| argv.get(i + 1))
        .cloned();

    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(move |app| build(app, start_panel.clone()));
    app.run_with_args(&[] as &[&str])
}

fn build(app: &Application, start_panel: Option<String>) {
    mavind_theme::load_app_css(include_str!("style.css"));

    let stack = Stack::new();
    stack.set_hexpand(true);
    stack.set_vexpand(true);

    add_panel(&stack, "about", "About", panel_about());
    add_panel(&stack, "appearance", "Appearance", panel_appearance());
    add_panel(&stack, "display", "Display", panel_display());
    add_panel(&stack, "sound", "Sound", panel_sound());
    add_panel(&stack, "network", "Network", panel_network());
    add_panel(&stack, "bluetooth", "Bluetooth", panel_simple(
        "Bluetooth",
        &["Managed by BlueZ.", "Pair devices with:  bluetoothctl", "Toggle radio:  rfkill block/unblock bluetooth"],
    ));
    add_panel(&stack, "storage", "Storage", panel_storage());
    add_panel(&stack, "applications", "Applications", panel_applications());
    add_panel(&stack, "windows-apps", "Windows Apps", panel_windows_apps());
    add_panel(&stack, "users", "Users", panel_users());
    add_panel(&stack, "security", "Security", panel_security());
    add_panel(&stack, "updates", "Updates", panel_updates());

    let sidebar = StackSidebar::new();
    sidebar.set_stack(&stack);
    sidebar.set_width_request(180);

    let split = GtkBox::new(Orientation::Horizontal, 0);
    split.append(&sidebar);
    split.append(&stack);

    if let Some(name) = start_panel {
        stack.set_visible_child_name(&name);
    }

    ApplicationWindow::builder()
        .application(app)
        .title("Mavind Settings")
        .default_width(760)
        .default_height(560)
        .child(&split)
        .build()
        .present();
}

fn add_panel(stack: &Stack, id: &str, title: &str, body: GtkBox) {
    let outer = GtkBox::new(Orientation::Vertical, 0);
    outer.set_margin_top(18);
    outer.set_margin_bottom(18);
    outer.set_margin_start(20);
    outer.set_margin_end(20);
    let t = Label::new(Some(title));
    t.add_css_class("title");
    t.set_halign(Align::Start);
    outer.append(&t);
    let scroller = ScrolledWindow::builder()
        .hscrollbar_policy(PolicyType::Never)
        .vexpand(true)
        .child(&body)
        .build();
    outer.append(&scroller);
    stack.add_titled(&outer, Some(id), title);
}

// ---- panel builders --------------------------------------------------
fn col() -> GtkBox {
    GtkBox::new(Orientation::Vertical, 6)
}

fn kv(key: &str, val: &str) -> GtkBox {
    let row = GtkBox::new(Orientation::Horizontal, 12);
    let k = Label::new(Some(key));
    k.add_css_class("k");
    k.set_width_chars(16);
    k.set_xalign(0.0);
    let v = Label::new(Some(val));
    v.set_xalign(0.0);
    v.set_wrap(true);
    v.set_selectable(true);
    row.append(&k);
    row.append(&v);
    row
}

fn card(children: &[&gtk4::Widget]) -> GtkBox {
    let c = GtkBox::new(Orientation::Vertical, 6);
    c.add_css_class("card");
    for w in children {
        c.append(*w);
    }
    c
}

fn panel_about() -> GtkBox {
    let os = sys::os_release();
    let (used, total) = sys::mem_used_kb();
    let root = sys::root_fs();
    let c = col();
    let rows = [
        kv("Name", os.get("PRETTY_NAME").map(String::as_str).unwrap_or("Mavind")),
        kv("Version", os.get("VERSION").map(String::as_str).unwrap_or("?")),
        kv("Kernel", &sys::uname_r()),
        kv("CPU", &sys::cpu_model()),
        kv(
            "Memory",
            &format!("{} used of {}", kib(used), kib(total)),
        ),
        kv(
            "Disk (/)",
            &root
                .map(|f| format!("{} free of {}", bytes(f.free), bytes(f.total)))
                .unwrap_or_else(|| "?".into()),
        ),
    ];
    let refs: Vec<&gtk4::Widget> = rows.iter().map(|r| r.upcast_ref()).collect();
    c.append(&card(&refs));

    let hint = Label::new(Some(
        "Mavind targets ~1 GB disk (core) and ~2 GB RAM. See Storage for the real numbers.",
    ));
    hint.set_wrap(true);
    hint.set_xalign(0.0);
    hint.add_css_class("k");
    c.append(&hint);
    c
}

fn panel_appearance() -> GtkBox {
    let c = col();
    let current = mavind_theme::Appearance::load();

    // Swatch colours are data (hex from ACCENT_SWATCHES), so they get their
    // own tiny generated provider rather than hand-written CSS classes.
    let mut swatch_css = String::new();
    for (name, hex) in mavind_theme::ACCENT_SWATCHES {
        swatch_css.push_str(&format!(".swatch-{} {{ background-color: {hex}; }}\n", name.to_lowercase()));
    }
    let swatch_provider = gtk4::CssProvider::new();
    swatch_provider.load_from_string(&swatch_css);
    gtk4::style_context_add_provider_for_display(
        &gtk4::gdk::Display::default().unwrap(),
        &swatch_provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    // --- theme: dark / light --------------------------------------
    let theme_title = Label::new(Some("Theme"));
    theme_title.add_css_class("k");
    theme_title.set_xalign(0.0);
    c.append(&theme_title);

    let theme_row = GtkBox::new(Orientation::Horizontal, 8);
    let dark_btn = ToggleButton::with_label("Dark");
    let light_btn = ToggleButton::with_label("Light");
    dark_btn.add_css_class("theme-toggle");
    light_btn.add_css_class("theme-toggle");
    light_btn.set_group(Some(&dark_btn));
    light_btn.set_active(current.theme == "light");
    dark_btn.set_active(current.theme != "light");
    theme_row.append(&dark_btn);
    theme_row.append(&light_btn);
    c.append(&card(&[theme_row.upcast_ref()]));

    dark_btn.connect_toggled(|b| {
        if b.is_active() {
            let mut ap = mavind_theme::Appearance::load();
            ap.theme = "dark".into();
            let _ = ap.save();
            mavind_theme::reload_theme_css();
        }
    });
    light_btn.connect_toggled(|b| {
        if b.is_active() {
            let mut ap = mavind_theme::Appearance::load();
            ap.theme = "light".into();
            let _ = ap.save();
            mavind_theme::reload_theme_css();
        }
    });

    // --- accent color -------------------------------------------------
    let accent_title = Label::new(Some("Accent color"));
    accent_title.add_css_class("k");
    accent_title.set_xalign(0.0);
    accent_title.set_margin_top(10);
    c.append(&accent_title);

    let swatch_row = GtkBox::new(Orientation::Horizontal, 8);
    let mut swatches: Vec<Button> = Vec::new();
    for (name, hex) in mavind_theme::ACCENT_SWATCHES {
        let b = Button::new();
        b.add_css_class("swatch");
        b.add_css_class(&format!("swatch-{}", name.to_lowercase()));
        b.set_tooltip_text(Some(name));
        if current.accent.eq_ignore_ascii_case(hex) {
            b.add_css_class("active");
        }
        swatch_row.append(&b);
        swatches.push(b);
    }
    c.append(&card(&[swatch_row.upcast_ref()]));

    for (i, (_, hex)) in mavind_theme::ACCENT_SWATCHES.iter().enumerate() {
        let siblings = swatches.clone();
        let hex = hex.to_string();
        swatches[i].connect_clicked(move |b| {
            for s in &siblings {
                s.remove_css_class("active");
            }
            b.add_css_class("active");
            let mut ap = mavind_theme::Appearance::load();
            ap.accent = hex.clone();
            let _ = ap.save();
            mavind_theme::reload_theme_css();
        });
    }

    // --- glass ------------------------------------------------------
    let glass_row = GtkBox::new(Orientation::Horizontal, 12);
    let glass_lbl = Label::new(Some("Glass effects  (translucent panels, menus and cards)"));
    glass_lbl.set_xalign(0.0);
    glass_lbl.set_hexpand(true);
    glass_lbl.set_wrap(true);
    let glass_sw = Switch::new();
    glass_sw.set_active(current.glass);
    glass_sw.set_valign(Align::Center);
    glass_sw.connect_state_set(|_, on| {
        let mut ap = mavind_theme::Appearance::load();
        ap.glass = on;
        let _ = ap.save();
        mavind_theme::reload_theme_css();
        glib::Propagation::Proceed
    });
    glass_row.append(&glass_lbl);
    glass_row.append(&glass_sw);
    c.append(&card(&[glass_row.upcast_ref()]));

    let glass_note = Label::new(Some(
        "\"Glass\" means translucent, tinted panels — labwc has no live background blur \
         for Wayland surfaces to render against, so this is not a real blur.",
    ));
    glass_note.add_css_class("k");
    glass_note.set_xalign(0.0);
    glass_note.set_wrap(true);
    c.append(&glass_note);

    // --- wallpaper ----------------------------------------------------
    let wp_title = Label::new(Some("Wallpaper"));
    wp_title.add_css_class("k");
    wp_title.set_xalign(0.0);
    wp_title.set_margin_top(10);
    c.append(&wp_title);

    let wp_btn = Button::with_label("Choose an image…");
    wp_btn.set_halign(Align::Start);
    wp_btn.connect_clicked(|_| {
        let dialog = gtk4::FileDialog::builder().title("Choose a wallpaper").build();
        let filter = gtk4::FileFilter::new();
        filter.set_name(Some("Images"));
        for pat in ["*.png", "*.jpg", "*.jpeg", "*.bmp", "*.webp"] {
            filter.add_pattern(pat);
        }
        let filters = gtk4::gio::ListStore::new::<gtk4::FileFilter>();
        filters.append(&filter);
        dialog.set_filters(Some(&filters));

        dialog.open(None::<&ApplicationWindow>, gtk4::gio::Cancellable::NONE, |res| {
            if let Ok(file) = res {
                if let Some(path) = file.path() {
                    if let Err(e) = std::fs::copy(&path, mavind_theme::wallpaper_path()) {
                        eprintln!("mavind-settings: could not set wallpaper: {e}");
                    }
                }
            }
        });
    });
    c.append(&wp_btn);
    let wp_note = Label::new(Some("Applies next time the desktop or login screen starts."));
    wp_note.add_css_class("k");
    wp_note.set_xalign(0.0);
    c.append(&wp_note);

    c
}

fn panel_display() -> GtkBox {
    let c = col();

    // Performance Mode
    let perf_row = GtkBox::new(Orientation::Horizontal, 12);
    let perf_lbl = Label::new(Some("Performance Mode  (disable shadows, solid wallpaper, lighter renderer)"));
    perf_lbl.set_xalign(0.0);
    perf_lbl.set_wrap(true);
    perf_lbl.set_hexpand(true);
    let sw = Switch::new();
    sw.set_active(sys::perf_enabled());
    sw.set_valign(Align::Center);
    sw.connect_state_set(|_, on| {
        let _ = sys::set_perf_enabled(on);
        glib::Propagation::Proceed
    });
    perf_row.append(&perf_lbl);
    perf_row.append(&sw);
    c.append(&card(&[perf_row.upcast_ref()]));

    let outputs = sys::outputs();
    let title = Label::new(Some("Outputs"));
    title.set_xalign(0.0);
    title.add_css_class("k");
    c.append(&title);
    if outputs.is_empty() {
        c.append(&Label::new(Some("(wlr-randr returned nothing — not in a Wayland session?)")));
    } else {
        for o in outputs {
            let l = Label::new(Some(&o));
            l.set_xalign(0.0);
            l.set_selectable(true);
            c.append(&l);
        }
    }
    let note = Label::new(Some("Resolution/scale changes: use  wlr-randr  or set them in ~/.config/labwc/."));
    note.add_css_class("k");
    note.set_xalign(0.0);
    note.set_wrap(true);
    c.append(&note);
    c
}

fn panel_sound() -> GtkBox {
    let c = col();
    let cur = sys::volume_percent().unwrap_or(0);
    let row = GtkBox::new(Orientation::Horizontal, 12);
    row.append(&Label::new(Some("Output volume")));
    let scale = Scale::with_range(Orientation::Horizontal, 0.0, 150.0, 1.0);
    scale.set_value(cur as f64);
    scale.set_hexpand(true);
    scale.set_draw_value(true);
    scale.connect_value_changed(|s| sys::set_volume_percent(s.value() as i32));
    row.append(&scale);
    c.append(&card(&[row.upcast_ref()]));

    let more = Button::with_label("Open audio mixer (pw-top)");
    more.connect_clicked(|_| {
        let _ = Command::new("foot").args(["-e", "pw-top"]).spawn();
    });
    c.append(&more);
    c
}

fn panel_network() -> GtkBox {
    let c = col();
    for (dev, ty, state) in sys::nm_devices() {
        if ty == "loopback" {
            continue;
        }
        c.append(&kv(&format!("{dev} ({ty})"), &state));
    }
    let wifi = sys::wifi_list();
    if !wifi.is_empty() {
        let t = Label::new(Some("Visible Wi-Fi networks"));
        t.add_css_class("k");
        t.set_xalign(0.0);
        t.set_margin_top(8);
        c.append(&t);
        for (ssid, sig) in wifi.into_iter().take(12) {
            c.append(&kv(&ssid, &format!("signal {sig}")));
        }
    }
    let b = Button::with_label("Manage connections (nmtui)");
    b.set_margin_top(8);
    b.connect_clicked(|_| {
        let _ = Command::new("foot").args(["-e", "nmtui"]).spawn();
    });
    c.append(&b);
    c
}

fn panel_storage() -> GtkBox {
    let c = col();
    if let Some(f) = sys::root_fs() {
        let used = f.total.saturating_sub(f.free);
        c.append(&kv("Filesystem", &f.mount));
        c.append(&kv("Used", &format!("{} of {}", bytes(used), bytes(f.total))));
        c.append(&kv("Free", &bytes(f.free)));
    }
    let t = Label::new(Some("Where space goes (real du):"));
    t.add_css_class("k");
    t.set_xalign(0.0);
    t.set_margin_top(10);
    c.append(&t);
    for (label, path) in [
        ("Kernel + modules", "/usr/lib/modules"),
        ("Firmware", "/usr/lib/firmware"),
        ("Libraries", "/usr/lib"),
        ("Wine (compat)", "/usr/lib/wine"),
        ("Your home", "/home"),
        ("Logs + cache (/var)", "/var"),
    ] {
        if let Some(sz) = sys::du_h(path) {
            c.append(&kv(label, &format!("{sz}   ({path})")));
        }
    }
    let tgt = Label::new(Some(
        "Target: core install ≈ 1 GB. If this system is larger, the Wine (compat) tier is why — it is optional.",
    ));
    tgt.set_wrap(true);
    tgt.set_xalign(0.0);
    tgt.add_css_class("k");
    tgt.set_margin_top(10);
    c.append(&tgt);
    c
}

fn panel_applications() -> GtkBox {
    let c = col();
    let mut count = 0;
    for dir in [
        "/usr/share/applications",
        &format!("{}/.local/share/applications", std::env::var("HOME").unwrap_or_default()),
    ] {
        if let Ok(rd) = std::fs::read_dir(dir) {
            for e in rd.flatten() {
                if e.path().extension().map(|x| x == "desktop").unwrap_or(false) {
                    count += 1;
                }
            }
        }
    }
    c.append(&kv("Installed .desktop entries", &count.to_string()));
    c.append(&Label::new(Some("Add or remove components with:  mpk install <pkg>  /  mpk remove <pkg>")));
    let b = Button::with_label("Open a terminal");
    b.connect_clicked(|_| {
        let _ = Command::new("foot").spawn();
    });
    b.set_halign(Align::Start);
    b.set_margin_top(6);
    c.append(&b);
    c
}

fn panel_windows_apps() -> GtkBox {
    let c = col();
    c.append(&kv(
        "Wine",
        &sys::wine_version().unwrap_or_else(|| "not installed (mpk install-tier compat)".into()),
    ));
    c.append(&kv("Wine prefixes", &sys::prefixes_count().to_string()));
    let open = Button::with_label("Open Mavind Windows Apps");
    open.set_halign(Align::Start);
    open.connect_clicked(|_| {
        let _ = Command::new("mavind-windows-apps").spawn();
    });
    c.append(&open);
    let note = Label::new(Some(
        "Right-click a .exe in Minder → Open with Mavind Windows Apps. Not every program works.",
    ));
    note.set_wrap(true);
    note.set_xalign(0.0);
    note.add_css_class("k");
    c.append(&note);
    c
}

fn panel_users() -> GtkBox {
    let c = col();
    for u in sys::users() {
        c.append(&Label::new(Some(&u)));
    }
    let b = Button::with_label("Change my password");
    b.set_halign(Align::Start);
    b.set_margin_top(8);
    b.connect_clicked(|_| {
        let _ = Command::new("foot").args(["-e", "passwd"]).spawn();
    });
    c.append(&b);
    c
}

fn panel_security() -> GtkBox {
    let c = col();
    c.append(&kv("Firewall", &sys::firewall_status()));
    c.append(&kv("Root account", "locked (use sudo)"));
    c.append(&kv("Unknown .exe warning", "on — shown before first run of any unverified program"));
    let note = Label::new(Some(
        "Enable a firewall:  mpk install ufw && sudo ufw enable",
    ));
    note.add_css_class("k");
    note.set_xalign(0.0);
    c.append(&note);
    c
}

fn panel_updates() -> GtkBox {
    let c = col();
    let n = sys::upgradable_count();
    c.append(&kv("Updates available", &n.to_string()));
    let b = Button::with_label("Check & install updates");
    b.set_halign(Align::Start);
    b.connect_clicked(|_| {
        let _ = Command::new("foot")
            .args(["-e", "sh", "-c", "mpk update && mpk upgrade; echo; read -p 'done - press enter'"])
            .spawn();
    });
    c.append(&b);
    let note = Label::new(Some("Mavind never updates in the background. Updates are always user-initiated."));
    note.add_css_class("k");
    note.set_xalign(0.0);
    note.set_wrap(true);
    c.append(&note);
    c
}

fn panel_simple(_title: &str, lines: &[&str]) -> GtkBox {
    let c = col();
    for l in lines {
        let lbl = Label::new(Some(l));
        lbl.set_xalign(0.0);
        lbl.set_wrap(true);
        c.append(&lbl);
    }
    c
}

// ---- fmt ----------------------------------------------------------
fn kib(kb: u64) -> String {
    bytes(kb * 1024)
}
fn bytes(b: u64) -> String {
    const U: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut v = b as f64;
    let mut i = 0;
    while v >= 1024.0 && i < 4 {
        v /= 1024.0;
        i += 1;
    }
    format!("{v:.1} {}", U[i])
}
