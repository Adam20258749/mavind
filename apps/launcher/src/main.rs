//! Mavind's Start Menu — a search-first app launcher. Each invocation is a
//! fresh, short-lived process (spawned by the panel button / Super key),
//! same model as the wofi popup it replaces, but Mavind-branded and themed.

mod apps;

use apps::AppEntry;
use gtk4::gdk::Key;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    Align, Application, ApplicationWindow, Box as GtkBox, Button, Entry, EventControllerKey,
    FlowBox, GestureClick, Image, Label, Orientation, Popover, PolicyType, ScrolledWindow,
    SelectionMode, ToggleButton,
};
use gtk4_layer_shell::{KeyboardMode, Layer, LayerShell};
use std::cell::RefCell;
use std::process::Command;
use std::rc::Rc;

const APP_ID: &str = "os.mavind.Launcher";
const CHIPS: &[(&str, &str)] = &[
    ("All", ""),
    ("System", "System"),
    ("Utility", "Utility"),
    ("Network", "Network"),
    ("Windows Apps", "Wine"),
];

struct Ctx {
    win: ApplicationWindow,
    search: Entry,
    flow: FlowBox,
    status: Label,
    all: Vec<AppEntry>,
    shown: RefCell<Vec<AppEntry>>,
    category: RefCell<String>,
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
    win.set_layer(Layer::Overlay);
    win.set_namespace("mavind-launcher");
    win.set_anchor(gtk4_layer_shell::Edge::Top, true);
    win.set_anchor(gtk4_layer_shell::Edge::Bottom, true);
    win.set_anchor(gtk4_layer_shell::Edge::Left, true);
    win.set_anchor(gtk4_layer_shell::Edge::Right, true);
    win.set_keyboard_mode(KeyboardMode::Exclusive);

    let backdrop = GtkBox::new(Orientation::Vertical, 0);
    backdrop.add_css_class("backdrop");
    backdrop.set_hexpand(true);
    backdrop.set_vexpand(true);

    let card = GtkBox::new(Orientation::Vertical, 12);
    card.add_css_class("card");
    card.set_halign(Align::Center);
    card.set_valign(Align::Center);
    card.set_size_request(680, 520);
    card.set_margin_top(24);
    card.set_margin_bottom(24);
    card.set_margin_start(24);
    card.set_margin_end(24);

    // swallow clicks that land on the card so the backdrop's dismiss handler
    // (added below) doesn't treat them as "clicked outside".
    let eat = GestureClick::new();
    eat.set_propagation_phase(gtk4::PropagationPhase::Capture);
    eat.connect_pressed(|g, _, _, _| {
        g.set_state(gtk4::EventSequenceState::Claimed);
    });
    card.add_controller(eat);

    // --- search --------------------------------------------------------
    let search = Entry::builder().placeholder_text("Search apps").build();
    search.add_css_class("search");
    search.set_margin_top(20);
    search.set_margin_start(20);
    search.set_margin_end(20);

    // --- category chips -------------------------------------------
    let chip_row = GtkBox::new(Orientation::Horizontal, 6);
    chip_row.set_margin_start(20);
    chip_row.set_margin_end(20);
    let mut anchor: Option<ToggleButton> = None;
    let mut chip_buttons: Vec<(ToggleButton, &'static str)> = Vec::new();
    for (label, cat) in CHIPS {
        let b = ToggleButton::with_label(label);
        b.add_css_class("chip");
        b.set_has_frame(false);
        match &anchor {
            Some(a) => b.set_group(Some(a)),
            None => anchor = Some(b.clone()),
        }
        if *cat == "" {
            b.set_active(true);
        }
        chip_row.append(&b);
        chip_buttons.push((b, *cat));
    }

    // --- app grid --------------------------------------------------
    let flow = FlowBox::new();
    flow.set_selection_mode(SelectionMode::None);
    flow.set_homogeneous(true);
    flow.set_max_children_per_line(6);
    flow.set_min_children_per_line(3);
    flow.set_row_spacing(4);
    flow.set_column_spacing(4);
    flow.set_valign(Align::Start);
    let scroller = ScrolledWindow::builder()
        .hscrollbar_policy(PolicyType::Never)
        .vexpand(true)
        .child(&flow)
        .build();
    scroller.set_margin_start(12);
    scroller.set_margin_end(12);

    let status = Label::new(None);
    status.add_css_class("empty-hint");
    status.set_margin_top(30);

    // --- footer: user + settings + power --------------------------
    let footer = GtkBox::new(Orientation::Horizontal, 10);
    footer.add_css_class("footer");
    footer.set_margin_top(6);
    footer.set_margin_bottom(14);
    footer.set_margin_start(20);
    footer.set_margin_end(20);
    let user = std::env::var("USER").unwrap_or_else(|_| "you".into());
    let avatar = Label::new(Some(&user.chars().next().unwrap_or('M').to_uppercase().to_string()));
    avatar.add_css_class("mavind-accent");
    let uname = Label::new(Some(&user));
    uname.add_css_class("user-name");
    let spacer = GtkBox::new(Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    let settings_btn = Button::from_icon_name("preferences-system-symbolic");
    settings_btn.set_has_frame(false);
    settings_btn.set_tooltip_text(Some("Settings"));
    let power_btn = Button::from_icon_name("system-shutdown-symbolic");
    power_btn.set_has_frame(false);
    power_btn.set_tooltip_text(Some("Power"));
    footer.append(&avatar);
    footer.append(&uname);
    footer.append(&spacer);
    footer.append(&settings_btn);
    footer.append(&power_btn);

    card.append(&search);
    card.append(&chip_row);
    card.append(&scroller);
    card.append(&status);
    card.append(&footer);
    backdrop.append(&card);
    win.set_child(Some(&backdrop));

    let ctx: Shared = Rc::new(Ctx {
        win: win.clone(),
        search: search.clone(),
        flow: flow.clone(),
        status: status.clone(),
        all: apps::scan(),
        shown: RefCell::new(Vec::new()),
        category: RefCell::new(String::new()),
    });

    // --- wiring ----------------------------------------------------
    {
        let c = ctx.clone();
        backdrop.set_can_target(true);
        let close = GestureClick::new();
        close.connect_pressed(move |_, _, _, _| c.win.close());
        backdrop.add_controller(close);
    }
    {
        let c = ctx.clone();
        search.connect_changed(move |e| rebuild(&c, &e.text()));
    }
    {
        let c = ctx.clone();
        search.connect_activate(move |_| {
            if let Some(first) = c.shown.borrow().first().cloned() {
                apps::launch(&first);
                c.win.close();
            }
        });
    }
    for (btn, cat) in &chip_buttons {
        let c = ctx.clone();
        let cat = cat.to_string();
        btn.connect_toggled(move |b| {
            if b.is_active() {
                *c.category.borrow_mut() = cat.clone();
                let text = c.search.text().to_string();
                rebuild(&c, &text);
            }
        });
    }
    {
        let win = win.clone();
        settings_btn.connect_clicked(move |_| {
            let _ = Command::new("mavind-settings").spawn();
            win.close();
        });
    }
    {
        let c = ctx.clone();
        power_btn.connect_clicked(move |btn| show_power_menu(&c.win, btn));
    }

    let keys = EventControllerKey::new();
    {
        let c = ctx.clone();
        keys.connect_key_pressed(move |_, key, _, _| {
            if key == Key::Escape {
                c.win.close();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
    }
    win.add_controller(keys);

    rebuild(&ctx, "");
    win.present();
    search.grab_focus();
}

fn rebuild(c: &Ctx, query: &str) {
    while let Some(ch) = c.flow.first_child() {
        c.flow.remove(&ch);
    }
    let q = query.to_lowercase();
    let cat = c.category.borrow().clone();
    let matches: Vec<AppEntry> = c
        .all
        .iter()
        .filter(|a| q.is_empty() || a.name.to_lowercase().contains(&q))
        .filter(|a| cat.is_empty() || a.categories.iter().any(|x| x == &cat))
        .cloned()
        .collect();

    if matches.is_empty() {
        c.status.set_text("No matching apps");
        c.status.set_visible(true);
    } else {
        c.status.set_visible(false);
    }

    for entry in &matches {
        let tile = Button::new();
        tile.add_css_class("tile");
        tile.set_has_frame(false);
        let col = GtkBox::new(Orientation::Vertical, 6);
        col.set_halign(Align::Center);
        let icon = match apps::icon_path_or_name(&entry.icon) {
            apps::IconRef::Name(n) => Image::from_icon_name(&n),
            apps::IconRef::Path(p) => Image::from_file(p),
        };
        icon.set_pixel_size(44);
        let label = Label::new(Some(&entry.name));
        label.add_css_class("tile-label");
        label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        label.set_max_width_chars(12);
        label.set_lines(2);
        label.set_wrap(true);
        label.set_justify(gtk4::Justification::Center);
        col.append(&icon);
        col.append(&label);
        tile.set_child(Some(&col));

        let e2 = entry.clone();
        let win = c.win.clone();
        tile.connect_clicked(move |_| {
            apps::launch(&e2);
            win.close();
        });
        c.flow.insert(&tile, -1);
    }
    *c.shown.borrow_mut() = matches;
}

fn show_power_menu(win: &ApplicationWindow, anchor: &impl IsA<gtk4::Widget>) {
    let menu = GtkBox::new(Orientation::Vertical, 2);
    menu.set_margin_top(6);
    menu.set_margin_bottom(6);
    menu.set_margin_start(6);
    menu.set_margin_end(6);
    let pop = Popover::new();
    pop.set_child(Some(&menu));
    pop.set_parent(anchor);
    pop.set_autohide(true);

    let win = win.clone();
    for (label, cmd) in [
        ("Lock", vec!["mavind-shell", "--lock"]),
        ("Log Out", vec!["labwc", "--exit"]),
        ("Suspend", vec!["systemctl", "suspend"]),
        ("Restart", vec!["systemctl", "reboot"]),
        ("Shut Down", vec!["systemctl", "poweroff"]),
    ] {
        let b = Button::with_label(label);
        b.set_has_frame(false);
        b.set_halign(Align::Fill);
        let pop2 = pop.clone();
        let win2 = win.clone();
        b.connect_clicked(move |_| {
            pop2.popdown();
            let _ = Command::new(cmd[0]).args(&cmd[1..]).spawn();
            win2.close();
        });
        menu.append(&b);
    }
    pop.popup();
}
