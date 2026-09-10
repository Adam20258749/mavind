//! Mavind OOBE — runs once, fullscreen, on the first boot of an installed system,
//! before greetd. Welcome → Region → Account → Privacy → Apply → hand off to login.
//!
//! The GUI collects answers, writes /run/mavind-oobe.json (root-only), and runs
//! `mavind-oobe-apply` which makes the system changes. It runs as root already
//! (started by mavind-oobe.service), so no polkit prompt.

use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    Align, Application, ApplicationWindow, Box as GtkBox, Button, CheckButton, DropDown, Entry,
    Label, Orientation, PasswordEntry, Stack, StackTransitionType,
};
use serde::Serialize;
use std::cell::RefCell;
use std::fs;
use std::rc::Rc;

const APP_ID: &str = "os.mavind.Oobe";

const PAGES: &[&str] = &["welcome", "region", "account", "privacy", "applying"];
const TITLES: &[&str] = &[
    "Welcome to Mavind",
    "Region & time",
    "Create your account",
    "Privacy",
    "Finishing setup",
];

// A small, ordered timezone list. `mavind-oobe-apply` accepts any valid tz name.
const ZONES: &[&str] = &[
    "UTC",
    "Europe/Berlin",
    "Europe/London",
    "Europe/Paris",
    "Europe/Madrid",
    "Europe/Rome",
    "Europe/Amsterdam",
    "Europe/Warsaw",
    "America/New_York",
    "America/Chicago",
    "America/Denver",
    "America/Los_Angeles",
    "America/Sao_Paulo",
    "Asia/Dubai",
    "Asia/Kolkata",
    "Asia/Shanghai",
    "Asia/Tokyo",
    "Australia/Sydney",
];

#[derive(Default, Serialize)]
struct Answers {
    fullname: String,
    username: String,
    password: String,
    hostname: String,
    timezone: String,
    ntp: bool,
    autologin: bool,
}

struct Ctx {
    win: ApplicationWindow,
    stack: Stack,
    title: Label,
    back: Button,
    next: Button,
    err: Label,
    // account fields
    e_full: Entry,
    e_user: Entry,
    e_pass: PasswordEntry,
    e_pass2: PasswordEntry,
    e_host: Entry,
    c_auto: CheckButton,
    tz: DropDown,
    c_ntp: CheckButton,
    m: RefCell<usize>, // page index
}

type Shared = Rc<Ctx>;

fn main() -> glib::ExitCode {
    // `--apply` path is handled by the shell helper; the binary is GUI-only.
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(build);
    app.run_with_args(&[] as &[&str])
}

fn build(app: &Application) {
    let css = gtk4::CssProvider::new();
    css.load_from_string(
        "window{background:#1c1c22;color:#e6e6ec}\
         .h{font-size:22px;font-weight:700;color:#fff}\
         .dim{color:#9a9aa6}\
         .err{color:#ff5c6c;font-weight:600}\
         entry,passwordentry{min-height:34px}",
    );
    gtk4::style_context_add_provider_for_display(
        &gtk4::gdk::Display::default().unwrap(),
        &css,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let stack = Stack::new();
    stack.set_transition_type(StackTransitionType::SlideLeftRight);
    stack.set_vexpand(true);

    let title = Label::new(Some(TITLES[0]));
    title.add_css_class("h");
    title.set_halign(Align::Start);
    title.set_margin_top(26);
    title.set_margin_start(32);

    let err = Label::new(None);
    err.add_css_class("err");
    err.set_halign(Align::Start);
    err.set_margin_start(32);

    let back = Button::with_label("Back");
    let next = Button::with_label("Next");
    next.add_css_class("suggested-action");
    let footer = GtkBox::new(Orientation::Horizontal, 8);
    footer.set_margin_top(8);
    footer.set_margin_bottom(20);
    footer.set_margin_start(32);
    footer.set_margin_end(32);
    let sp = GtkBox::new(Orientation::Horizontal, 0);
    sp.set_hexpand(true);
    footer.append(&back);
    footer.append(&sp);
    footer.append(&next);

    let root = GtkBox::new(Orientation::Vertical, 6);
    root.append(&title);
    root.append(&stack);
    root.append(&err);
    root.append(&footer);

    let win = ApplicationWindow::builder()
        .application(app)
        .title("Mavind Setup")
        .default_width(760)
        .default_height(540)
        .child(&root)
        .build();
    win.fullscreen();

    let e_full = Entry::builder().placeholder_text("Your name").build();
    let e_user = Entry::builder().placeholder_text("username").build();
    let e_pass = PasswordEntry::builder().show_peek_icon(true).build();
    let e_pass2 = PasswordEntry::builder().show_peek_icon(true).build();
    let e_host = Entry::builder().text("mavind").build();
    let c_auto = CheckButton::with_label("Log in automatically");
    let tz = DropDown::from_strings(ZONES);
    tz.set_selected(1); // Europe/Berlin as a sensible non-UTC default
    let c_ntp = CheckButton::with_label("Set time automatically (recommended)");
    c_ntp.set_active(true);

    let ctx: Shared = Rc::new(Ctx {
        win: win.clone(),
        stack: stack.clone(),
        title,
        back: back.clone(),
        next: next.clone(),
        err,
        e_full: e_full.clone(),
        e_user: e_user.clone(),
        e_pass: e_pass.clone(),
        e_pass2: e_pass2.clone(),
        e_host: e_host.clone(),
        c_auto: c_auto.clone(),
        tz: tz.clone(),
        c_ntp: c_ntp.clone(),
        m: RefCell::new(0),
    });

    stack.add_named(&page_welcome(), Some("welcome"));
    stack.add_named(&page_region(&ctx), Some("region"));
    stack.add_named(&page_account(&ctx), Some("account"));
    stack.add_named(&page_privacy(), Some("privacy"));
    stack.add_named(&page_applying(), Some("applying"));

    // Auto-derive username from the name, but stop once the user edits it directly.
    let user_touched = Rc::new(RefCell::new(false));
    {
        let ut = user_touched.clone();
        e_user.connect_changed(move |e| {
            if e.has_focus() {
                *ut.borrow_mut() = true;
            }
        });
    }
    {
        let e_user = e_user.clone();
        let ut = user_touched.clone();
        e_full.connect_changed(move |e| {
            if *ut.borrow() {
                return;
            }
            let first = e
                .text()
                .split_whitespace()
                .next()
                .unwrap_or("")
                .chars()
                .filter(|c| c.is_ascii_alphanumeric())
                .collect::<String>()
                .to_lowercase();
            e_user.set_text(&first);
        });
    }

    {
        let c = ctx.clone();
        back.connect_clicked(move |_| nav(&c, -1));
    }
    {
        let c = ctx.clone();
        next.connect_clicked(move |_| nav(&c, 1));
    }

    show(&ctx);
    win.present();
}

fn nav(c: &Shared, delta: i32) {
    let cur = *c.m.borrow();
    c.err.set_text("");

    if PAGES[cur] == "privacy" && delta > 0 {
        match collect(c) {
            Ok(ans) => finish(c, ans),
            Err(e) => c.err.set_text(&e),
        }
        return;
    }
    if PAGES[cur] == "account" && delta > 0 {
        if let Err(e) = validate_account(c) {
            c.err.set_text(&e);
            return;
        }
    }

    let n = (cur as i32 + delta).clamp(0, (PAGES.len() - 2) as i32) as usize; // never nav into "applying"
    *c.m.borrow_mut() = n;
    show(c);
}

fn show(c: &Ctx) {
    let i = *c.m.borrow();
    c.stack.set_visible_child_name(PAGES[i]);
    c.title.set_label(TITLES[i]);
    c.back.set_visible(i > 0 && PAGES[i] != "applying");
    c.next.set_visible(PAGES[i] != "applying");
    c.next.set_label(if PAGES[i] == "privacy" { "Finish" } else { "Next" });
}

// ---- pages -------------------------------------------------------------
fn wrap(inner: &impl IsA<gtk4::Widget>) -> GtkBox {
    let b = GtkBox::new(Orientation::Vertical, 12);
    b.set_margin_start(32);
    b.set_margin_end(32);
    b.set_margin_top(10);
    b.append(inner);
    b
}

fn field(label: &str, w: &impl IsA<gtk4::Widget>) -> GtkBox {
    let row = GtkBox::new(Orientation::Vertical, 3);
    let l = Label::new(Some(label));
    l.add_css_class("dim");
    l.set_halign(Align::Start);
    row.append(&l);
    row.append(w);
    row
}

fn page_welcome() -> GtkBox {
    let l = Label::new(Some(
        "Mavind is installed. A few quick questions and you're in.\n\nThis takes about a minute.",
    ));
    l.set_halign(Align::Start);
    l.set_wrap(true);
    wrap(&l)
}

fn page_region(c: &Shared) -> GtkBox {
    let col = GtkBox::new(Orientation::Vertical, 12);
    col.append(&field("Time zone", &c.tz));
    col.append(&c.c_ntp);
    wrap(&col)
}

fn page_account(c: &Shared) -> GtkBox {
    let col = GtkBox::new(Orientation::Vertical, 10);
    col.append(&field("Your name", &c.e_full));
    col.append(&field("Username", &c.e_user));
    col.append(&field("Password", &c.e_pass));
    col.append(&field("Confirm password", &c.e_pass2));
    col.append(&field("Computer name", &c.e_host));
    col.append(&c.c_auto);
    wrap(&col)
}

fn page_privacy() -> GtkBox {
    let l = Label::new(Some(
        "Mavind collects no telemetry, no usage data, no diagnostics — there is \
         nothing here to switch off.\n\nUpdates are only ever installed when you \
         ask for them.",
    ));
    l.set_halign(Align::Start);
    l.set_wrap(true);
    wrap(&l)
}

fn page_applying() -> GtkBox {
    let l = Label::new(Some("Applying your settings and starting Mavind…"));
    l.set_halign(Align::Start);
    let sp = gtk4::Spinner::new();
    sp.start();
    let col = GtkBox::new(Orientation::Vertical, 14);
    col.append(&l);
    col.append(&sp);
    wrap(&col)
}

// ---- validation + finish --------------------------------------------
fn validate_account(c: &Ctx) -> Result<(), String> {
    let user = c.e_user.text().to_string();
    let pass = c.e_pass.text().to_string();
    let pass2 = c.e_pass2.text().to_string();
    let host = c.e_host.text().to_string();

    if user.is_empty() || !user.chars().all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_')
        || user.chars().next().map(|ch| ch.is_ascii_digit()).unwrap_or(true)
    {
        return Err("Username must be lowercase letters/digits, starting with a letter.".into());
    }
    if pass.len() < 4 {
        return Err("Password must be at least 4 characters.".into());
    }
    if pass != pass2 {
        return Err("The passwords don't match.".into());
    }
    if host.is_empty() || !host.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '-') {
        return Err("Computer name must be letters, digits or '-'.".into());
    }
    Ok(())
}

fn collect(c: &Ctx) -> Result<Answers, String> {
    validate_account(c)?;
    let tz = ZONES
        .get(c.tz.selected() as usize)
        .copied()
        .unwrap_or("UTC")
        .to_string();
    Ok(Answers {
        fullname: c.e_full.text().to_string(),
        username: c.e_user.text().to_string(),
        password: c.e_pass.text().to_string(),
        hostname: c.e_host.text().to_string(),
        timezone: tz,
        ntp: c.c_ntp.is_active(),
        autologin: c.c_auto.is_active(),
    })
}

fn finish(c: &Shared, ans: Answers) {
    *c.m.borrow_mut() = PAGES.iter().position(|p| *p == "applying").unwrap();
    show(c);
    c.back.set_visible(false);
    c.next.set_visible(false);

    let json = serde_json::to_vec(&ans).unwrap_or_default();
    let write_res = fs::write("/run/mavind-oobe.json", &json).and_then(|_| {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions("/run/mavind-oobe.json", fs::Permissions::from_mode(0o600))
        }
        #[cfg(not(unix))]
        Ok(())
    });
    if let Err(e) = write_res {
        c.err.set_text(&format!("could not write answers: {e}"));
        return;
    }

    // Run the apply helper (fast: useradd/hostname/tz/greetd), then close.
    // mavind-oobe-session sees us exit and hands off to greetd.
    let win = c.win.clone();
    glib::timeout_add_seconds_local_once(1, move || {
        let ok = std::process::Command::new("mavind-oobe-apply")
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !ok {
            let _ = std::process::Command::new("mavind-oobe-apply")
                .arg("--verbose")
                .status();
        }
        if let Some(app) = win.application() {
            app.quit();
        }
        win.close();
    });
}
