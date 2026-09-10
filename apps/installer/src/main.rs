//! Mavind graphical installer — a Windows-Setup-style wizard.
//! Language → Keyboard → Action → Version → System check → Disk → Summary →
//! Installing → Done. Account creation is left to the OOBE on first boot.

mod plan;
mod probe;
mod run;

use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    Align, Application, ApplicationWindow, Box as GtkBox, Button, CheckButton, DropDown, Entry,
    Expander, Label, ListBox, ListBoxRow, Orientation, PolicyType, ProgressBar, ScrolledWindow,
    SelectionMode, Stack, StackTransitionType, TextView,
};
use plan::InstallPlan;
use probe::{Check, Facts};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

const APP_ID: &str = "os.mavind.Installer";

// label, locale, default keymap
const LANGS: &[(&str, &str, &str)] = &[
    ("English", "en_US.UTF-8", "us"),
    ("Deutsch", "de_DE.UTF-8", "de"),
    ("Français", "fr_FR.UTF-8", "fr"),
    ("Español", "es_ES.UTF-8", "es"),
    ("Italiano", "it_IT.UTF-8", "it"),
    ("Português", "pt_PT.UTF-8", "pt"),
    ("Nederlands", "nl_NL.UTF-8", "nl"),
    ("Polski", "pl_PL.UTF-8", "pl"),
];
const KEYMAPS: &[(&str, &str)] = &[
    ("English (US)", "us"),
    ("English (UK)", "gb"),
    ("German", "de"),
    ("German (Switzerland)", "ch"),
    ("French", "fr"),
    ("Spanish", "es"),
    ("Italian", "it"),
    ("Portuguese", "pt"),
    ("Dutch", "nl"),
    ("Polish", "pl"),
];

const PAGES: &[&str] = &[
    "language", "keyboard", "action", "version", "syscheck", "disk", "summary", "installing", "done",
];
const TITLES: &[&str] = &[
    "Choose your language",
    "Keyboard layout",
    "What do you want to do?",
    "Choose the version to install",
    "Checking this PC",
    "Where to install Mavind",
    "Ready to install",
    "Installing Mavind",
    "Mavind is installed",
];

struct Ctx {
    win: ApplicationWindow,
    stack: Stack,
    title: Label,
    back: Button,
    next: Button,
    disk_list: ListBox,
    summary_box: GtkBox,
    progress: ProgressBar,
    prog_label: Label,
    log_view: TextView,
    m: RefCell<Model>,
}

struct Model {
    page: usize,
    plan: InstallPlan,
    facts: Facts,
    checks_ok: bool,
    disks: Vec<probe::Disk>,
    selected_disk: Option<usize>,
    rx: Option<std::sync::mpsc::Receiver<run::Msg>>,
}

type Shared = Rc<Ctx>;

fn main() -> glib::ExitCode {
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(build);
    app.run_with_args(&[] as &[&str])
}

fn build(app: &Application) {
    let css = gtk4::CssProvider::new();
    css.load_from_string(include_str!("style.css"));
    gtk4::style_context_add_provider_for_display(
        &gtk4::gdk::Display::default().unwrap(),
        &css,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let facts = Facts::gather();
    let disks = facts.disks.clone();

    let stack = Stack::new();
    stack.set_transition_type(StackTransitionType::SlideLeftRight);
    stack.set_vexpand(true);

    let title = Label::new(Some(TITLES[0]));
    title.add_css_class("wiz-title");
    title.set_halign(Align::Start);
    title.set_margin_top(22);
    title.set_margin_start(28);
    title.set_margin_bottom(4);

    let back = Button::with_label("Back");
    let next = Button::with_label("Next");
    next.add_css_class("suggested-action");
    let footer = GtkBox::new(Orientation::Horizontal, 8);
    footer.set_margin_top(10);
    footer.set_margin_bottom(16);
    footer.set_margin_start(28);
    footer.set_margin_end(28);
    let spacer = GtkBox::new(Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    footer.append(&back);
    footer.append(&spacer);
    footer.append(&next);

    let root = GtkBox::new(Orientation::Vertical, 0);
    root.append(&title);
    root.append(&stack);
    root.append(&footer);

    let win = ApplicationWindow::builder()
        .application(app)
        .title("Install Mavind")
        .default_width(780)
        .default_height(560)
        .child(&root)
        .build();

    let disk_list = ListBox::new();
    disk_list.set_selection_mode(SelectionMode::Single);
    disk_list.add_css_class("cards");
    let summary_box = GtkBox::new(Orientation::Vertical, 6);
    let progress = ProgressBar::new();
    progress.set_show_text(true);
    let prog_label = Label::new(Some("Preparing…"));
    prog_label.set_halign(Align::Start);
    let log_view = TextView::new();
    log_view.set_editable(false);
    log_view.set_monospace(true);

    let ctx: Shared = Rc::new(Ctx {
        win: win.clone(),
        stack: stack.clone(),
        title,
        back: back.clone(),
        next: next.clone(),
        disk_list,
        summary_box,
        progress,
        prog_label,
        log_view,
        m: RefCell::new(Model {
            page: 0,
            plan: InstallPlan::default(),
            facts,
            checks_ok: false,
            disks,
            selected_disk: None,
            rx: None,
        }),
    });

    stack.add_named(&page_language(&ctx), Some("language"));
    stack.add_named(&page_keyboard(&ctx), Some("keyboard"));
    stack.add_named(&page_action(), Some("action"));
    stack.add_named(&page_version(&ctx), Some("version"));
    stack.add_named(&page_syscheck(&ctx), Some("syscheck"));
    stack.add_named(&page_disk(&ctx), Some("disk"));
    stack.add_named(&page_summary(&ctx), Some("summary"));
    stack.add_named(&page_installing(&ctx), Some("installing"));
    stack.add_named(&page_done(), Some("done"));

    {
        let c = ctx.clone();
        back.connect_clicked(move |_| nav(&c, -1));
    }
    {
        let c = ctx.clone();
        next.connect_clicked(move |_| nav(&c, 1));
    }

    show_page(&ctx);
    win.present();
}

// ---------------------------------------------------------------------------
fn nav(c: &Shared, delta: i32) {
    let cur = c.m.borrow().page;
    match (PAGES[cur], delta > 0) {
        ("summary", true) => {
            start_install(c);
            return;
        }
        ("done", true) => {
            let _ = std::process::Command::new("systemctl").arg("reboot").spawn();
            return;
        }
        _ => {}
    }
    let mut n = cur as i32 + delta;
    if PAGES.get(n as usize) == Some(&"installing") {
        n += delta; // never step onto "installing" via the buttons
    }
    let n = n.clamp(0, PAGES.len() as i32 - 1) as usize;
    c.m.borrow_mut().page = n;
    show_page(c);
}

fn show_page(c: &Ctx) {
    let idx = c.m.borrow().page;
    let name = PAGES[idx];
    c.stack.set_visible_child_name(name);
    c.title.set_label(TITLES[idx]);

    match name {
        "disk" => rebuild_disk_list(c),
        "summary" => rebuild_summary(c),
        _ => {}
    }

    let (back_vis, next_label, next_ok, next_vis) = match name {
        "language" => (false, "Next", true, true),
        "syscheck" => (true, "Next", c.m.borrow().checks_ok, true),
        "disk" => (true, "Next", c.m.borrow().selected_disk.is_some(), true),
        "summary" => (true, "Install", true, true),
        "installing" => (false, "Next", false, false),
        "done" => (false, "Restart now", true, true),
        _ => (true, "Next", true, true),
    };
    c.back.set_visible(back_vis);
    c.next.set_label(next_label);
    c.next.set_sensitive(next_ok);
    c.next.set_visible(next_vis);
}

// ---------------------------------------------------------------------------
fn page_shell(inner: &impl IsA<gtk4::Widget>) -> ScrolledWindow {
    let b = GtkBox::new(Orientation::Vertical, 12);
    b.set_margin_start(28);
    b.set_margin_end(28);
    b.set_margin_top(8);
    b.append(inner);
    ScrolledWindow::builder()
        .hscrollbar_policy(PolicyType::Never)
        .child(&b)
        .build()
}

fn page_language(c: &Shared) -> ScrolledWindow {
    let labels: Vec<&str> = LANGS.iter().map(|l| l.0).collect();
    let dd = DropDown::from_strings(&labels);
    dd.set_halign(Align::Start);
    let hint = Label::new(Some("This sets the language for setup and the default for Mavind."));
    hint.add_css_class("dim");
    hint.set_halign(Align::Start);
    let col = GtkBox::new(Orientation::Vertical, 10);
    col.append(&dd);
    col.append(&hint);

    let c = c.clone();
    dd.connect_selected_notify(move |d| {
        if let Some(l) = LANGS.get(d.selected() as usize) {
            let mut m = c.m.borrow_mut();
            m.plan.locale = l.1.to_string();
            m.plan.keymap = l.2.to_string();
        }
    });
    page_shell(&col)
}

fn page_keyboard(c: &Shared) -> ScrolledWindow {
    let labels: Vec<&str> = KEYMAPS.iter().map(|k| k.0).collect();
    let dd = DropDown::from_strings(&labels);
    dd.set_halign(Align::Start);
    let test = Entry::builder()
        .placeholder_text("Type here to test the layout")
        .build();
    let col = GtkBox::new(Orientation::Vertical, 12);
    col.append(&dd);
    col.append(&test);

    let c = c.clone();
    dd.connect_selected_notify(move |d| {
        if let Some(k) = KEYMAPS.get(d.selected() as usize) {
            c.m.borrow_mut().plan.keymap = k.1.to_string();
        }
    });
    page_shell(&col)
}

fn page_action() -> ScrolledWindow {
    let card = GtkBox::new(Orientation::Vertical, 4);
    card.add_css_class("card");
    card.add_css_class("selected");
    let t = Label::new(Some("Install Mavind"));
    t.add_css_class("card-title");
    t.set_halign(Align::Start);
    let d = Label::new(Some(
        "Erase a disk and install a fresh copy of Mavind. You'll finish setup \
         (account, region) on first boot.",
    ));
    d.add_css_class("dim");
    d.set_halign(Align::Start);
    d.set_wrap(true);
    card.append(&t);
    card.append(&d);

    let soon = Label::new(Some("Repair and dual-boot options are coming later."));
    soon.add_css_class("dim");
    soon.set_halign(Align::Start);

    let col = GtkBox::new(Orientation::Vertical, 14);
    col.append(&card);
    col.append(&soon);
    page_shell(&col)
}

fn page_version(c: &Shared) -> ScrolledWindow {
    let col = GtkBox::new(Orientation::Vertical, 8);
    let releases = plan::available_releases();
    if let Some(first) = releases.first() {
        c.m.borrow_mut().plan.version_id = first.version_id.clone();
    }
    let mut anchor: Option<CheckButton> = None;
    for (i, r) in releases.iter().enumerate() {
        let text = if r.pretty.is_empty() {
            format!("{}   ·   {}", r.version_id, r.channel)
        } else {
            format!("{}   ·   {}", r.pretty, r.channel)
        };
        let rb = CheckButton::with_label(&text);
        match &anchor {
            Some(a) => rb.set_group(Some(a)),
            None => anchor = Some(rb.clone()),
        }
        if i == 0 {
            rb.set_active(true);
        }
        let c = c.clone();
        let vid = r.version_id.clone();
        rb.connect_toggled(move |b| {
            if b.is_active() {
                c.m.borrow_mut().plan.version_id = vid.clone();
            }
        });
        col.append(&rb);
    }
    let hint = Label::new(Some(
        "The version string changes with every Mavind update. Newer media list more versions here.",
    ));
    hint.add_css_class("dim");
    hint.set_halign(Align::Start);
    hint.set_wrap(true);
    col.append(&hint);
    page_shell(&col)
}

fn page_syscheck(c: &Shared) -> ScrolledWindow {
    let col = GtkBox::new(Orientation::Vertical, 8);
    let checks = probe::evaluate(&c.m.borrow().facts);
    let ok = !checks.iter().any(|x| x.is_blocker());
    c.m.borrow_mut().checks_ok = ok;

    for chk in &checks {
        let (icon, text, cls) = match chk {
            Check::Pass(t) => ("\u{2713}", t.as_str(), "ok"),
            Check::Warn(t) => ("!", t.as_str(), "warn"),
            Check::Fail(t) => ("\u{2717}", t.as_str(), "fail"),
        };
        let row = GtkBox::new(Orientation::Horizontal, 10);
        let i = Label::new(Some(icon));
        i.add_css_class(cls);
        i.set_width_chars(2);
        let l = Label::new(Some(text));
        l.set_halign(Align::Start);
        l.set_wrap(true);
        row.append(&i);
        row.append(&l);
        col.append(&row);
    }
    if !ok {
        let f = Label::new(Some(
            "This PC does not meet the minimum requirements. Fix the marked items to continue.",
        ));
        f.add_css_class("fail");
        f.set_halign(Align::Start);
        f.set_wrap(true);
        f.set_margin_top(8);
        col.append(&f);
    }
    page_shell(&col)
}

fn page_disk(c: &Shared) -> ScrolledWindow {
    let warn = Label::new(Some(
        "\u{26a0}  The selected disk will be completely erased. Back up anything you need first.",
    ));
    warn.add_css_class("warn");
    warn.set_halign(Align::Start);
    warn.set_wrap(true);

    let col = GtkBox::new(Orientation::Vertical, 12);
    col.append(&warn);
    col.append(&c.disk_list);

    let cc = c.clone();
    c.disk_list.connect_row_selected(move |_, row| {
        let idx = row.map(|r| r.index() as usize);
        {
            let mut m = cc.m.borrow_mut();
            m.selected_disk = idx;
            if let Some(i) = idx {
                if let Some(d) = m.disks.get(i).cloned() {
                    m.plan.disk = d.name;
                }
            }
        }
        cc.next.set_sensitive(cc.m.borrow().selected_disk.is_some());
    });
    page_shell(&col)
}

fn rebuild_disk_list(c: &Ctx) {
    while let Some(ch) = c.disk_list.first_child() {
        c.disk_list.remove(&ch);
    }
    let disks = c.m.borrow().disks.clone();
    if disks.is_empty() {
        let l = Label::new(Some("No disks found. Attach a disk and reopen the installer."));
        l.add_css_class("dim");
        c.disk_list.append(&l);
        return;
    }
    for d in &disks {
        let row = ListBoxRow::new();
        let b = GtkBox::new(Orientation::Vertical, 2);
        b.set_margin_top(8);
        b.set_margin_bottom(8);
        b.set_margin_start(10);
        b.set_margin_end(10);
        let name = Label::new(Some(&format!("{}   —   {}", d.name, probe::human_size(d.size))));
        name.set_halign(Align::Start);
        name.add_css_class("card-title");
        let meta_txt = format!(
            "{}{}",
            if d.model.is_empty() { "disk".to_string() } else { d.model.clone() },
            if d.bus.is_empty() { String::new() } else { format!("  ·  {}", d.bus) },
        );
        let meta = Label::new(Some(&meta_txt));
        meta.set_halign(Align::Start);
        meta.add_css_class("dim");
        b.append(&name);
        b.append(&meta);
        row.set_child(Some(&b));
        c.disk_list.append(&row);
    }
}

fn page_summary(c: &Shared) -> ScrolledWindow {
    page_shell(&c.summary_box)
}

fn rebuild_summary(c: &Ctx) {
    while let Some(ch) = c.summary_box.first_child() {
        c.summary_box.remove(&ch);
    }
    let m = c.m.borrow();
    let rows = [
        ("Action", "Install Mavind (fresh)".to_string()),
        ("Version", m.plan.version_id.clone()),
        ("Language", m.plan.locale.clone()),
        ("Keyboard", m.plan.keymap.clone()),
        ("Firmware", m.plan.firmware.to_uppercase()),
        (
            "Disk (will be erased)",
            if m.plan.disk.is_empty() { "\u{2014}".to_string() } else { m.plan.disk.clone() },
        ),
    ];
    drop(m);
    for (k, v) in rows {
        let row = GtkBox::new(Orientation::Horizontal, 12);
        let kl = Label::new(Some(k));
        kl.add_css_class("dim");
        kl.set_width_chars(20);
        kl.set_xalign(0.0);
        let vl = Label::new(Some(&v));
        vl.set_xalign(0.0);
        vl.set_selectable(true);
        row.append(&kl);
        row.append(&vl);
        c.summary_box.append(&row);
    }
    let note = Label::new(Some(
        "Click Install to erase the disk and write Mavind. You'll create your account when it restarts.",
    ));
    note.add_css_class("dim");
    note.set_halign(Align::Start);
    note.set_wrap(true);
    note.set_margin_top(10);
    c.summary_box.append(&note);
}

fn page_installing(c: &Shared) -> ScrolledWindow {
    c.progress.set_fraction(0.0);
    let col = GtkBox::new(Orientation::Vertical, 12);
    col.append(&c.prog_label);
    col.append(&c.progress);

    let exp = Expander::new(Some("Details"));
    let sc = ScrolledWindow::builder()
        .min_content_height(220)
        .child(&c.log_view)
        .build();
    exp.set_child(Some(&sc));
    col.append(&exp);
    page_shell(&col)
}

fn page_done() -> ScrolledWindow {
    let col = GtkBox::new(Orientation::Vertical, 10);
    let big = Label::new(Some("Mavind is installed on this PC."));
    big.add_css_class("wiz-title");
    big.set_halign(Align::Start);
    let l = Label::new(Some(
        "Remove the installation media, then restart. On first boot you'll set up your account and region.",
    ));
    l.set_halign(Align::Start);
    l.set_wrap(true);
    col.append(&big);
    col.append(&l);
    page_shell(&col)
}

// ---------------------------------------------------------------------------
fn start_install(c: &Shared) {
    let path = match c.m.borrow().plan.write() {
        Ok(p) => p,
        Err(e) => {
            append_log(c, &format!("could not write install plan: {e}"));
            return;
        }
    };
    append_log(c, &format!("plan written to {}", path.display()));
    let rx = run::start(&path);
    c.m.borrow_mut().rx = Some(rx);
    c.m.borrow_mut().page = PAGES.iter().position(|p| *p == "installing").unwrap();
    show_page(c);

    let c = c.clone();
    glib::timeout_add_local(Duration::from_millis(120), move || {
        let mut done: Option<i32> = None;
        {
            let m = c.m.borrow();
            if let Some(rx) = &m.rx {
                while let Ok(msg) = rx.try_recv() {
                    match msg {
                        run::Msg::Progress(p, text) => {
                            c.progress.set_fraction(p as f64 / 100.0);
                            c.progress.set_text(Some(&format!("{p}%")));
                            if !text.is_empty() {
                                c.prog_label.set_label(&text);
                            }
                        }
                        run::Msg::Log(line) => append_log(&c, &line),
                        run::Msg::Done(code) => done = Some(code),
                    }
                }
            } else {
                return glib::ControlFlow::Break;
            }
        }
        if let Some(code) = done {
            c.m.borrow_mut().rx = None;
            if code == 0 {
                c.m.borrow_mut().page = PAGES.iter().position(|p| *p == "done").unwrap();
                show_page(&c);
            } else {
                c.prog_label
                    .set_label(&format!("Installation failed (exit {code}). See Details."));
                c.next.set_visible(true);
                c.next.set_label("Close");
                c.next.set_sensitive(true);
            }
            return glib::ControlFlow::Break;
        }
        glib::ControlFlow::Continue
    });
}

fn append_log(c: &Ctx, line: &str) {
    let buf = c.log_view.buffer();
    buf.insert(&mut buf.end_iter(), line);
    buf.insert(&mut buf.end_iter(), "\n");
    let mark = buf.create_mark(None, &buf.end_iter(), false);
    c.log_view.scroll_to_mark(&mark, 0.0, false, 0.0, 0.0);
}
