//! Minimal GTK4 front-end over `core::Engine`. Long-running actions (install,
//! repair, dxvk) are launched in a `foot` terminal so the user sees Wine's
//! output and the safety prompt; quick actions call the engine directly.

use crate::engine::Engine;
use gtk4::prelude::*;
use gtk4::{
    glib, Align, Application, ApplicationWindow, Box as GtkBox, Button, Label, ListBox,
    Orientation, PolicyType, ScrolledWindow, SelectionMode,
};
use std::path::PathBuf;
use std::process::Command;
use std::rc::Rc;

const APP_ID: &str = "os.mavind.WindowsApps";

pub fn run(install_file: Option<PathBuf>) -> glib::ExitCode {
    let app = Application::builder().application_id(APP_ID).build();
    let file = Rc::new(install_file);
    app.connect_activate(move |app| build(app, (*file).clone()));
    app.run_with_args(&[] as &[&str])
}

fn build(app: &Application, install_file: Option<PathBuf>) {
    let css = gtk4::CssProvider::new();
    css.load_from_string(
        "window{background:#1c1c22;color:#e6e6ec}\
         .h{font-weight:bold;font-size:15px;color:#b08cf6}\
         .dim{color:#9a9aa6;font-size:11px}\
         row{padding:6px 4px;border-bottom:1px solid #2a2a33}\
         button{margin:0 2px}",
    );
    gtk4::style_context_add_provider_for_display(
        &gtk4::gdk::Display::default().unwrap(),
        &css,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let root = GtkBox::new(Orientation::Vertical, 8);
    root.set_margin_top(12);
    root.set_margin_bottom(12);
    root.set_margin_start(12);
    root.set_margin_end(12);

    let title = Label::new(Some("Mavind Windows Apps"));
    title.add_css_class("h");
    title.set_halign(Align::Start);
    root.append(&title);

    let sub = Label::new(Some(engine_status_line().as_str()));
    sub.add_css_class("dim");
    sub.set_halign(Align::Start);
    sub.set_wrap(true);
    root.append(&sub);

    let list = ListBox::new();
    list.set_selection_mode(SelectionMode::None);
    let scroller = ScrolledWindow::builder()
        .hscrollbar_policy(PolicyType::Never)
        .vexpand(true)
        .child(&list)
        .build();
    root.append(&scroller);

    let bar = GtkBox::new(Orientation::Horizontal, 6);
    let install_btn = Button::with_label("Install .exe / .msi…");
    let refresh_btn = Button::with_label("Refresh");
    bar.append(&install_btn);
    bar.append(&refresh_btn);
    root.append(&bar);

    let window = ApplicationWindow::builder()
        .application(app)
        .title("Windows Apps")
        .default_width(560)
        .default_height(460)
        .child(&root)
        .build();

    let repopulate = {
        let list = list.clone();
        move || populate(&list)
    };
    repopulate();
    {
        let repopulate = repopulate.clone();
        refresh_btn.connect_clicked(move |_| repopulate());
    }
    {
        let win = window.clone();
        let repopulate = repopulate.clone();
        install_btn.connect_clicked(move |_| pick_and_install(&win, repopulate.clone()));
    }

    window.present();

    if let Some(f) = install_file {
        terminal(&["mavind-wine", "install", &f.to_string_lossy()]);
    }
}

fn engine_status_line() -> String {
    if Engine::wine_available() {
        "Right-click a .exe in Minder → Open with Mavind Windows Apps, or use Install below. \
         Not every Windows program works — check the compatibility note."
            .into()
    } else {
        "⚠  Wine is not installed. Run  mpk install-tier compat  to enable Windows apps.".into()
    }
}

fn populate(list: &ListBox) {
    while let Some(c) = list.first_child() {
        list.remove(&c);
    }
    let eng = match Engine::new() {
        Ok(e) => e,
        Err(e) => {
            list.append(&Label::new(Some(&format!("error: {e}"))));
            return;
        }
    };
    let reg = eng.load().unwrap_or_default();
    if reg.apps.is_empty() {
        let l = Label::new(Some("No Windows apps installed yet."));
        l.add_css_class("dim");
        l.set_margin_top(12);
        list.append(&l);
        return;
    }
    let eng = Rc::new(eng);
    for a in reg.apps.values().cloned() {
        let row = GtkBox::new(Orientation::Horizontal, 6);
        let info = GtkBox::new(Orientation::Vertical, 0);
        let name = Label::new(Some(&a.name));
        name.set_halign(Align::Start);
        let meta = Label::new(Some(&format!(
            "prefix {} · {}{}",
            a.prefix,
            a.arch,
            a.dxvk.as_ref().map(|v| format!(" · dxvk {v}")).unwrap_or_default()
        )));
        meta.add_css_class("dim");
        meta.set_halign(Align::Start);
        info.append(&name);
        info.append(&meta);
        info.set_hexpand(true);
        row.append(&info);

        let mk = |label: &str| {
            let b = Button::with_label(label);
            b.set_valign(Align::Center);
            b
        };
        let run = mk("Run");
        let cfg = mk("Configure");
        let fix = mk("Repair");
        let del = mk("Uninstall");
        row.append(&run);
        row.append(&cfg);
        row.append(&fix);
        row.append(&del);

        {
            let eng = eng.clone();
            let id = a.id.clone();
            run.connect_clicked(move |_| {
                let _ = eng.run(&id);
            });
        }
        {
            let eng = eng.clone();
            let id = a.id.clone();
            cfg.connect_clicked(move |_| {
                let _ = eng.configure(&id);
            });
        }
        {
            let id = a.id.clone();
            fix.connect_clicked(move |_| terminal(&["mavind-wine", "repair", &id]));
        }
        {
            let id = a.id.clone();
            del.connect_clicked(move |_| terminal(&["mavind-wine", "uninstall", &id]));
        }
        list.append(&row);
    }
}

fn pick_and_install<F: Fn() + 'static + Clone>(win: &ApplicationWindow, refresh: F) {
    let dialog = gtk4::FileDialog::builder().title("Choose a Windows installer").build();
    let filter = gtk4::FileFilter::new();
    filter.set_name(Some("Windows programs (*.exe, *.msi)"));
    filter.add_pattern("*.exe");
    filter.add_pattern("*.msi");
    let filters = gtk4::gio::ListStore::new::<gtk4::FileFilter>();
    filters.append(&filter);
    dialog.set_filters(Some(&filters));

    let win = win.clone();
    dialog.open(Some(&win), gtk4::gio::Cancellable::NONE, move |res| {
        if let Ok(file) = res {
            if let Some(path) = file.path() {
                terminal(&["mavind-wine", "install", &path.to_string_lossy()]);
                // give the install a moment, then refresh on next idle
                let refresh = refresh.clone();
                glib::timeout_add_seconds_local_once(2, move || refresh());
            }
        }
    });
}

/// Launch a command inside a foot terminal so Wine output is visible.
fn terminal(argv: &[&str]) {
    let mut c = Command::new("foot");
    c.arg("-e");
    c.args(argv);
    if c.spawn().is_err() {
        // no terminal? run detached and hope for the best
        let _ = Command::new(argv[0]).args(&argv[1..]).spawn();
    }
}
