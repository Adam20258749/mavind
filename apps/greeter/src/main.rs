//! Mavind's graphical login screen — a small greetd client. Runs as the sole
//! layer-shell client of its own labwc session (system/greeter/session), the
//! same "-s <app>" hand-off pattern the OOBE uses.

mod accounts;
mod greetd;

use accounts::Account;
use gtk4::gdk::Key;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    Align, Application, ApplicationWindow, Box as GtkBox, Button, ContentFit,
    EventControllerKey, Label, Orientation, Overlay, PasswordEntry, Picture, Stack,
    StackTransitionType,
};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::cell::RefCell;
use std::process::Command;
use std::rc::Rc;
use std::sync::mpsc::Receiver;
use std::time::Duration;

const APP_ID: &str = "os.mavind.Greeter";

struct Ctx {
    win: ApplicationWindow,
    stack: Stack,
    picker_box: GtkBox,
    avatar: Label,
    name_label: Label,
    user_label: Label,
    pass: PasswordEntry,
    error: Label,
    login_btn: Button,
    back_btn: Button,
    accounts: Vec<Account>,
    selected: RefCell<Option<String>>,
    busy: RefCell<bool>,
    rx: RefCell<Option<Receiver<greetd::LoginResult>>>,
}
type Shared = Rc<Ctx>;

fn main() -> glib::ExitCode {
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(build);
    app.run_with_args(&[] as &[&str])
}

fn build(app: &Application) {
    mavind_theme::load_app_css(include_str!("style.css"));

    let win = ApplicationWindow::builder().application(app).build();
    win.init_layer_shell();
    win.set_layer(Layer::Top);
    win.set_namespace("mavind-greeter");
    for edge in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
        win.set_anchor(edge, true);
    }
    win.set_keyboard_mode(KeyboardMode::Exclusive);

    let root = Overlay::new();
    let base = GtkBox::new(Orientation::Vertical, 0);
    base.set_hexpand(true);
    base.set_vexpand(true);
    root.set_child(Some(&base));

    let wallpaper = mavind_theme::wallpaper_path();
    if wallpaper.is_file() {
        let bg = Picture::for_filename(&wallpaper);
        bg.set_content_fit(ContentFit::Cover);
        bg.set_hexpand(true);
        bg.set_vexpand(true);
        bg.set_can_target(false);
        root.add_overlay(&bg);

        let scrim = GtkBox::new(Orientation::Vertical, 0);
        scrim.add_css_class("scrim");
        scrim.set_hexpand(true);
        scrim.set_vexpand(true);
        scrim.set_can_target(false);
        root.add_overlay(&scrim);
    }

    // --- clock -------------------------------------------------------
    let clock = Label::new(None);
    clock.add_css_class("clock");
    let date = Label::new(None);
    date.add_css_class("date");
    let clock_col = GtkBox::new(Orientation::Vertical, 4);
    clock_col.set_halign(Align::Center);
    clock_col.set_valign(Align::Start);
    clock_col.set_margin_top(72);
    clock_col.append(&clock);
    clock_col.append(&date);
    root.add_overlay(&clock_col);
    tick_clock(clock, date);

    // --- picker page ---------------------------------------------------
    let picker_box = GtkBox::new(Orientation::Vertical, 14);
    picker_box.set_halign(Align::Center);
    let picker_card = GtkBox::new(Orientation::Vertical, 0);
    picker_card.add_css_class("card");
    picker_card.append(&picker_box);

    // --- auth page -------------------------------------------------
    let back_btn = Button::from_icon_name("go-previous-symbolic");
    back_btn.add_css_class("back-btn");
    back_btn.set_has_frame(false);
    back_btn.set_halign(Align::Start);
    let back_row = GtkBox::new(Orientation::Horizontal, 0);
    back_row.append(&back_btn);

    let avatar = Label::new(None);
    avatar.add_css_class("avatar");
    avatar.set_halign(Align::Center);
    let name_label = Label::new(None);
    name_label.add_css_class("display-name");
    name_label.set_halign(Align::Center);
    name_label.set_margin_top(10);
    let user_label = Label::new(None);
    user_label.add_css_class("dim");
    user_label.set_halign(Align::Center);
    user_label.set_margin_bottom(6);

    let pass = PasswordEntry::builder()
        .show_peek_icon(true)
        .placeholder_text("Password")
        .build();
    pass.add_css_class("login-pass");

    let error = Label::new(None);
    error.add_css_class("login-error");
    error.set_wrap(true);
    error.set_visible(false);

    let login_btn = Button::with_label("Log In");
    login_btn.add_css_class("suggested-action");
    login_btn.set_margin_top(4);

    let auth_col = GtkBox::new(Orientation::Vertical, 8);
    auth_col.set_size_request(280, -1);
    auth_col.append(&back_row);
    auth_col.append(&avatar);
    auth_col.append(&name_label);
    auth_col.append(&user_label);
    auth_col.append(&pass);
    auth_col.append(&error);
    auth_col.append(&login_btn);

    let auth_card = GtkBox::new(Orientation::Vertical, 0);
    auth_card.add_css_class("card");
    auth_card.append(&auth_col);

    let stack = Stack::new();
    stack.set_transition_type(StackTransitionType::Crossfade);
    stack.add_named(&picker_card, Some("picker"));
    stack.add_named(&auth_card, Some("auth"));
    stack.set_halign(Align::Center);
    stack.set_valign(Align::Center);
    root.add_overlay(&stack);

    // --- power bar (available without logging in) -------------------
    let power_bar = GtkBox::new(Orientation::Horizontal, 8);
    power_bar.add_css_class("power-bar");
    power_bar.set_halign(Align::End);
    power_bar.set_valign(Align::End);
    for (icon, tip, cmd) in [
        ("system-suspend-symbolic", "Suspend", vec!["systemctl", "suspend"]),
        ("system-reboot-symbolic", "Restart", vec!["systemctl", "reboot"]),
        ("system-shutdown-symbolic", "Shut Down", vec!["systemctl", "poweroff"]),
    ] {
        let b = Button::from_icon_name(icon);
        b.set_has_frame(false);
        b.set_tooltip_text(Some(tip));
        b.connect_clicked(move |_| {
            let _ = Command::new(cmd[0]).args(&cmd[1..]).spawn();
        });
        power_bar.append(&b);
    }
    root.add_overlay(&power_bar);

    win.set_child(Some(&root));

    let accounts = accounts::list();
    let ctx: Shared = Rc::new(Ctx {
        win: win.clone(),
        stack: stack.clone(),
        picker_box,
        avatar,
        name_label,
        user_label,
        pass: pass.clone(),
        error,
        login_btn: login_btn.clone(),
        back_btn: back_btn.clone(),
        accounts: accounts.clone(),
        selected: RefCell::new(None),
        busy: RefCell::new(false),
        rx: RefCell::new(None),
    });

    build_picker(&ctx);

    {
        let c = ctx.clone();
        back_btn.connect_clicked(move |_| show_picker(&c));
    }
    {
        let c = ctx.clone();
        login_btn.connect_clicked(move |_| try_login(&c));
    }
    {
        let c = ctx.clone();
        pass.connect_activate(move |_| try_login(&c));
    }

    let keys = EventControllerKey::new();
    {
        let c = ctx.clone();
        keys.connect_key_pressed(move |_, key, _, _| {
            if key == Key::Escape
                && c.accounts.len() > 1
                && c.stack.visible_child_name().as_deref() == Some("auth")
            {
                show_picker(&c);
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
    }
    win.add_controller(keys);

    match accounts.len() {
        0 => {
            ctx.stack.set_visible_child_name("auth");
            ctx.avatar.set_label("!");
            ctx.name_label.set_label("No local accounts found");
            ctx.user_label.set_label("Create one from the installer or OOBE");
            ctx.pass.set_sensitive(false);
            ctx.login_btn.set_sensitive(false);
            ctx.back_btn.set_visible(false);
        }
        1 => select_account(&ctx, &accounts[0].username),
        _ => ctx.stack.set_visible_child_name("picker"),
    }

    win.present();
}

fn build_picker(c: &Shared) {
    while let Some(ch) = c.picker_box.first_child() {
        c.picker_box.remove(&ch);
    }
    let title = Label::new(Some("Who's signing in?"));
    title.add_css_class("display-name");
    c.picker_box.append(&title);

    let grid = GtkBox::new(Orientation::Horizontal, 10);
    grid.set_halign(Align::Center);
    for acc in c.accounts.clone() {
        let tile = Button::new();
        tile.add_css_class("tile");
        tile.set_has_frame(false);
        let col = GtkBox::new(Orientation::Vertical, 8);
        col.set_halign(Align::Center);
        let av = Label::new(Some(&initials(&acc.full_name)));
        av.add_css_class("avatar");
        let name = Label::new(Some(&acc.full_name));
        name.add_css_class("display-name");
        col.append(&av);
        col.append(&name);
        tile.set_child(Some(&col));

        let c2 = c.clone();
        let username = acc.username.clone();
        tile.connect_clicked(move |_| select_account(&c2, &username));
        grid.append(&tile);
    }
    c.picker_box.append(&grid);
}

fn show_picker(c: &Shared) {
    *c.selected.borrow_mut() = None;
    c.error.set_visible(false);
    c.pass.set_text("");
    c.stack.set_visible_child_name("picker");
}

fn select_account(c: &Shared, username: &str) {
    let Some(acc) = c.accounts.iter().find(|a| a.username == username).cloned() else {
        return;
    };
    *c.selected.borrow_mut() = Some(acc.username.clone());
    c.avatar.set_label(&initials(&acc.full_name));
    c.name_label.set_label(&acc.full_name);
    c.user_label.set_label(&format!("@{}", acc.username));
    c.error.set_visible(false);
    c.pass.set_text("");
    c.pass.set_sensitive(true);
    c.login_btn.set_sensitive(true);
    c.login_btn.set_label("Log In");
    c.back_btn.set_visible(c.accounts.len() > 1);
    c.stack.set_visible_child_name("auth");
    c.pass.grab_focus();
}

fn try_login(c: &Shared) {
    if *c.busy.borrow() {
        return;
    }
    let Some(username) = c.selected.borrow().clone() else {
        return;
    };
    let password = c.pass.text().to_string();
    if password.is_empty() {
        c.error.set_label("Enter your password");
        c.error.set_visible(true);
        return;
    }

    *c.busy.borrow_mut() = true;
    c.pass.set_sensitive(false);
    c.login_btn.set_sensitive(false);
    c.login_btn.set_label("Signing in…");
    c.error.set_visible(false);

    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let result = greetd::attempt_login(&username, &password);
        let _ = tx.send(result);
    });
    *c.rx.borrow_mut() = Some(rx);

    let c2 = c.clone();
    glib::timeout_add_local(Duration::from_millis(150), move || {
        let msg = {
            let rx_ref = c2.rx.borrow();
            match &*rx_ref {
                Some(rx) => rx.try_recv().ok(),
                None => return glib::ControlFlow::Break,
            }
        };
        let Some(result) = msg else {
            return glib::ControlFlow::Continue;
        };
        *c2.rx.borrow_mut() = None;
        *c2.busy.borrow_mut() = false;

        match result {
            greetd::LoginResult::Success => {
                // greetd tears this session down and starts the real one; leave
                // the form disabled so nothing flashes interactive in between.
                c2.login_btn.set_label("Welcome");
            }
            greetd::LoginResult::Rejected(reason) => {
                c2.login_btn.set_label("Log In");
                c2.error.set_label(&reason);
                c2.error.set_visible(true);
                c2.pass.set_text("");
                c2.pass.set_sensitive(true);
                c2.login_btn.set_sensitive(true);
                c2.pass.grab_focus();
            }
            greetd::LoginResult::Error(reason) => {
                c2.login_btn.set_label("Log In");
                c2.error.set_label(&format!("Sign-in unavailable: {reason}"));
                c2.error.set_visible(true);
                c2.pass.set_sensitive(true);
                c2.login_btn.set_sensitive(true);
            }
        }
        glib::ControlFlow::Break
    });
}

fn tick_clock(clock: Label, date: Label) {
    fn update(clock: &Label, date: &Label) {
        let Ok(now) = glib::DateTime::now_local() else {
            return;
        };
        if let Ok(t) = now.format("%l:%M %p") {
            clock.set_label(t.trim());
        }
        if let Ok(d) = now.format("%A, %B %e") {
            date.set_label(d.trim());
        }
    }
    update(&clock, &date);
    glib::timeout_add_local(Duration::from_millis(1000), move || {
        update(&clock, &date);
        glib::ControlFlow::Continue
    });
}

fn initials(name: &str) -> String {
    let out: String = name
        .split_whitespace()
        .take(2)
        .filter_map(|w| w.chars().next())
        .map(|c| c.to_ascii_uppercase())
        .collect();
    if out.is_empty() {
        "?".into()
    } else {
        out
    }
}
